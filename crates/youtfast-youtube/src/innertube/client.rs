//! The InnerTube HTTP client, session authentication (`SAPISIDHASH`), and
//! wire helpers (protobuf and base64).
//!
//! Session cookies live only in memory (`Client::cookie`) and are never
//! written to disk or logs.

use std::fmt;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use url::Url;

use youtfast_core::util;

/// The web client's version, pinned in the source: there is no client-version
/// discovery and no disk cache, so `visitorData` is fetched once per run.
/// When YouTube starts refusing requests, refresh this and `USER_AGENT`
/// together (the web client's `/youtubei/v1/guide` answers the current one).
const WEB_CLIENT_VERSION: &str = "2.20261015.01.00";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const ORIGIN: &str = "https://www.youtube.com";
const REFERER: &str = "https://www.youtube.com/";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(25);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthError {
    NoLogin,
    Other(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLogin => f.write_str("not signed in"),
            Self::Other(msg) => f.write_str(msg),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Auth(AuthError),
    HttpStatus(u16, Box<str>),
    Http(String),
    Extraction(Box<str>),
    Other(Box<str>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auth(err) => write!(f, "authentication error: {err}"),
            Self::HttpStatus(code, msg) => write!(f, "http status {code}: {msg}"),
            Self::Http(msg) => write!(f, "http error: {msg}"),
            Self::Extraction(msg) => write!(f, "extraction error: {msg}"),
            Self::Other(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        let mut text = error.to_string();
        let mut source = std::error::Error::source(&error);
        while let Some(cause) = source {
            text.push_str(": ");
            text.push_str(&cause.to_string());
            source = cause.source();
        }
        Self::Http(text)
    }
}

/// Which InnerTube endpoint a continuation token belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContinuationEndpoint {
    Browse,
    Search,
    Next,
}

impl ContinuationEndpoint {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Browse => "browse",
            Self::Search => "search",
            Self::Next => "next",
        }
    }
}

/// The InnerTube client. Cheap to clone (`Arc` inside).
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    no_redirect: reqwest::Client,
    cookie: Arc<RwLock<Option<String>>>,
    visitor_data: Arc<RwLock<Option<String>>>,
}

impl Client {
    pub fn new(builder: reqwest::ClientBuilder) -> Result<Self, Error> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = builder.timeout(REQUEST_TIMEOUT).build()?;
        let no_redirect = super::http()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            http,
            no_redirect,
            cookie: Arc::new(RwLock::new(None)),
            visitor_data: Arc::new(RwLock::new(None)),
        })
    }

    pub fn default_client() -> Result<Self, Error> {
        Self::new(super::http())
    }

    /// Sets the session cookie after checking it with YouTube's account menu.
    pub async fn user_auth_set_cookie(&self, cookie: String) -> Result<(), Error> {
        if extract_sapisid(&cookie).is_none() {
            return Err(Error::Auth(AuthError::NoLogin));
        }
        *self
            .cookie
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(cookie);
        let check = self
            .post_raw(
                "account/account_menu",
                &json!({
                    "deviceTheme": "DEVICE_THEME_SUPPORTED",
                    "userInterfaceTheme": "USER_INTERFACE_THEME_DARK"
                }),
                true,
                None,
            )
            .await;
        match check {
            Ok(resp) => {
                let logged_in = resp
                    .pointer("/responseContext/mainAppWebResponseContext/loggedOut")
                    .and_then(Value::as_bool)
                    != Some(true)
                    && resp.get("actions").is_some();
                if logged_in {
                    Ok(())
                } else {
                    self.user_auth_remove_cookie().await;
                    Err(Error::Auth(AuthError::NoLogin))
                }
            }
            Err(err) => {
                self.user_auth_remove_cookie().await;
                Err(err)
            }
        }
    }

    pub async fn user_auth_remove_cookie(&self) {
        *self
            .cookie
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    pub fn has_cookie(&self) -> bool {
        self.cookie
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

    /// Fetches a fresh `visitorData` token from YouTube when needed.
    pub async fn get_visitor_data(&self) -> Result<String, Error> {
        let resp = self.post_raw("guide", &json!({}), false, None).await?;
        resp.pointer("/responseContext/visitorData")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| Error::Extraction("missing visitorData".into()))
    }

    /// Sends a JSON POST request to `/youtubei/v1/{endpoint}`.
    pub async fn post_raw(
        &self,
        endpoint: &str,
        body: &impl Serialize,
        authenticated: bool,
        visitor_data: Option<&str>,
    ) -> Result<Value, Error> {
        let mut payload = serde_json::to_value(body)
            .map_err(|err| Error::Other(format!("invalid request body: {err}").into()))?;
        let stored_visitor = self
            .visitor_data
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let visitor = visitor_data.or(if authenticated {
            None
        } else {
            stored_visitor.as_deref()
        });
        let mut client_ctx = json!({
            "clientName": "WEB",
            "clientVersion": WEB_CLIENT_VERSION,
            "hl": "en",
            "gl": "US",
        });
        if let Some(v) = visitor {
            client_ctx["visitorData"] = Value::String(v.to_owned());
        }
        if let Value::Object(map) = &mut payload {
            map.insert("context".to_owned(), json!({ "client": client_ctx }));
        }

        let url = format!("{ORIGIN}/youtubei/v1/{endpoint}?prettyPrint=false");
        let mut req = self
            .http
            .post(&url)
            .header("User-Agent", USER_AGENT)
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
            .header("X-Origin", ORIGIN)
            .header("X-Youtube-Client-Name", "1")
            .header("X-Youtube-Client-Version", WEB_CLIENT_VERSION);
        if let Some(v) = visitor {
            req = req.header("X-Goog-Visitor-Id", v);
        }
        if authenticated {
            let cookie = self
                .cookie
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
                .ok_or(Error::Auth(AuthError::NoLogin))?;
            let sapisid = extract_sapisid(&cookie).ok_or(Error::Auth(AuthError::NoLogin))?;
            let auth = sapisid_authorization(&sapisid, util::now());
            req = req
                .header("Cookie", cookie)
                .header("Authorization", auth)
                .header("X-Goog-AuthUser", "0");
        }

        let response = req.json(&payload).send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| {
                    v.pointer("/error/message")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .unwrap_or_else(|| status.canonical_reason().unwrap_or("HTTP error").to_owned());
            return Err(Error::HttpStatus(status.as_u16(), message.into()));
        }
        let value: Value = serde_json::from_str(&text)
            .map_err(|err| Error::Extraction(format!("invalid JSON: {err}").into()))?;
        if !authenticated
            && let Some(v) = value
                .pointer("/responseContext/visitorData")
                .and_then(Value::as_str)
        {
            *self
                .visitor_data
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(v.to_owned());
        }
        Ok(value)
    }

    /// Fetches an unauthenticated `player` response using the Android client
    /// context, which returns storyboards and timedtext caption tracks
    /// without requiring a web PoToken or signature timestamp.
    pub async fn post_player_unauthenticated(&self, video_id: &str) -> Result<Value, Error> {
        const ANDROID_VERSION: &str = "20.10.38";
        const ANDROID_UA: &str = "com.google.android.youtube/20.10.38 (Linux; U; Android 14) gzip";
        let payload = json!({
            "videoId": video_id,
            "racyCheckOk": true,
            "contentCheckOk": true,
            "context": {
                "client": {
                    "clientName": "ANDROID",
                    "clientVersion": ANDROID_VERSION,
                    "androidSdkVersion": 34,
                    "hl": "en",
                    "gl": "US"
                }
            }
        });
        let url = format!("{ORIGIN}/youtubei/v1/player?prettyPrint=false");
        let response = self
            .http
            .post(&url)
            .header("User-Agent", ANDROID_UA)
            .header("X-Youtube-Client-Name", "3")
            .header("X-Youtube-Client-Version", ANDROID_VERSION)
            .json(&payload)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(Error::HttpStatus(status.as_u16(), "HTTP error".into()));
        }
        serde_json::from_str(&text)
            .map_err(|err| Error::Extraction(format!("invalid JSON: {err}").into()))
    }

    /// Fetches a plain GET text response (for channel RSS and search suggestions).
    pub async fn get_text(&self, url: &str) -> Result<String, Error> {
        let response = self
            .http
            .get(url)
            .header("User-Agent", USER_AGENT)
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::HttpStatus(
                status.as_u16(),
                status.canonical_reason().unwrap_or("HTTP error").into(),
            ));
        }
        Ok(response.text().await?)
    }

    /// Checks that a playback tracking URL targets an HTTPS `youtube.com` host
    /// with no credentials, custom port, or fragment.
    pub fn history_tracking_url_allowed(raw: &str) -> bool {
        let Ok(url) = Url::parse(raw) else {
            return false;
        };
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.fragment().is_some()
        {
            return false;
        }
        let Some(host) = url.host_str() else {
            return false;
        };
        host == "youtube.com" || host.ends_with(".youtube.com")
    }

    /// Sends an authenticated GET request to a playback tracking URL without
    /// following redirects.
    pub async fn history_tracking_get(&self, url: &str) -> Result<(), Error> {
        if !Self::history_tracking_url_allowed(url) {
            return Err(Error::Other("disallowed tracking url".into()));
        }
        let cookie = self
            .cookie
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .ok_or(Error::Auth(AuthError::NoLogin))?;
        let sapisid = extract_sapisid(&cookie).ok_or(Error::Auth(AuthError::NoLogin))?;
        let auth = sapisid_authorization(&sapisid, util::now());
        let response = self
            .no_redirect
            .get(url)
            .header("User-Agent", USER_AGENT)
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
            .header("Cookie", cookie)
            .header("Authorization", auth)
            .header("X-Goog-AuthUser", "0")
            .send()
            .await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(Error::HttpStatus(
                response.status().as_u16(),
                "tracking request failed".into(),
            ))
        }
    }
}

/// Extracts `SAPISID` or `__Secure-3PAPISID` from a `Cookie` header.
fn extract_sapisid(cookie: &str) -> Option<String> {
    let mut fallback = None;
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("SAPISID=")
            && !val.is_empty()
        {
            return Some(val.to_owned());
        }
        if let Some(val) = part.strip_prefix("__Secure-3PAPISID=")
            && !val.is_empty()
        {
            fallback = Some(val.to_owned());
        }
    }
    fallback
}

/// Builds the `SAPISIDHASH` Authorization header for YouTube web requests.
fn sapisid_authorization(sapisid: &str, timestamp: i64) -> String {
    let input = format!("{timestamp} {sapisid} {ORIGIN}");
    let digest = sha1_hex(input.as_bytes());
    format!("SAPISIDHASH {timestamp}_{digest}")
}

/// SHA-1 (RFC 3174) for `SAPISIDHASH` header generation.
fn sha1_hex(data: &[u8]) -> String {
    use sha1::Digest as _;
    format!("{:x}", sha1::Sha1::digest(data))
}

// ---------------------------------------------------------------------------
// Wire helpers: Protobuf and Base64
// ---------------------------------------------------------------------------

pub fn push_varint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

pub fn proto_varint(field: u32, value: u64, out: &mut Vec<u8>) {
    push_varint(u64::from(field) << 3, out);
    push_varint(value, out);
}

pub fn proto_bytes(field: u32, bytes: &[u8], out: &mut Vec<u8>) {
    push_varint((u64::from(field) << 3) | 2, out);
    push_varint(bytes.len() as u64, out);
    out.extend_from_slice(bytes);
}

pub fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Standard base64 with `=`, `+`, `/` percent-encoded (`%3D`, `%2B`, `%2F`)
/// as YouTube's continuation tokens and `params` expect.
pub fn base64_urlencoded(bytes: &[u8]) -> String {
    let b64 = base64_encode(bytes);
    let mut out = String::with_capacity(b64.len() + 6);
    for ch in b64.chars() {
        match ch {
            '=' => out.push_str("%3D"),
            '+' => out.push_str("%2B"),
            '/' => out.push_str("%2F"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn urlencoded(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sapisid_authorization_hashes_timestamp_secret_and_origin() {
        assert_eq!(
            sapisid_authorization("synthetic_sapisid", 1_700_000_000),
            "SAPISIDHASH 1700000000_81dcb38ee6356657bc05f8810e5d9260a2da2bfc"
        );
    }

    #[test]
    fn extracts_sapisid_or_secure_3papisid() {
        assert_eq!(
            extract_sapisid("PREF=f6=400; SAPISID=abc/123; SID=xyz").as_deref(),
            Some("abc/123")
        );
        assert_eq!(
            extract_sapisid("PREF=f6=400; __Secure-3PAPISID=sec/456").as_deref(),
            Some("sec/456")
        );
        assert_eq!(extract_sapisid("PREF=f6=400"), None);
    }
}
