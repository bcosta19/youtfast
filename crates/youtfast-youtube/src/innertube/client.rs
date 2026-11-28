//! The InnerTube HTTP client, session authentication (`SAPISIDHASH`), and
//! wire helpers (protobuf and base64).
//!
//! Session cookies live only in memory (`Client::cookie`) and are never
//! written to disk or logs.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use url::Url;

use crate::backoff::{RequestGate, RequestIntent, RequestPermit, retry_after};
use youtfast_core::{privacy::YOUTUBE_RATE_LIMITED, util};

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
    RateLimited,
    HttpStatus(u16, Box<str>),
    Http(String),
    Extraction(Box<str>),
    Other(Box<str>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auth(err) => write!(f, "authentication error: {err}"),
            Self::RateLimited => f.write_str(YOUTUBE_RATE_LIMITED),
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
    gate: RequestGate,
    user_authorization: Option<Arc<AtomicBool>>,
    cookie: Arc<RwLock<Option<String>>>,
    visitor_data: Arc<RwLock<Option<String>>>,
}

impl Client {
    pub fn new(builder: reqwest::ClientBuilder) -> Result<Self, Error> {
        Self::with_gate(builder, RequestGate::default())
    }

    pub fn with_gate(builder: reqwest::ClientBuilder, gate: RequestGate) -> Result<Self, Error> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = builder.timeout(REQUEST_TIMEOUT).build()?;
        let no_redirect = super::http()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            http,
            no_redirect,
            gate,
            user_authorization: None,
            cookie: Arc::new(RwLock::new(None)),
            visitor_data: Arc::new(RwLock::new(None)),
        })
    }

    pub fn default_client() -> Result<Self, Error> {
        Self::new(super::http())
    }

    pub fn gate(&self) -> &RequestGate {
        &self.gate
    }

    pub fn for_user_request(&self) -> Self {
        Self {
            user_authorization: Some(Arc::new(AtomicBool::new(true))),
            ..self.clone()
        }
    }

    pub fn public_client(&self) -> Self {
        Self {
            cookie: Arc::new(RwLock::new(None)),
            visitor_data: Arc::new(RwLock::new(None)),
            ..self.clone()
        }
    }

    fn intent(&self, consume: bool) -> RequestIntent {
        let authorized = self.user_authorization.as_ref().is_some_and(|available| {
            if consume {
                available.swap(false, Ordering::Relaxed)
            } else {
                available.load(Ordering::Relaxed)
            }
        });
        if authorized {
            RequestIntent::UserInitiated
        } else {
            RequestIntent::Background
        }
    }

    fn begin_request(&self) -> Result<RequestPermit, Error> {
        self.gate
            .begin(self.intent(true))
            .map_err(|_| Error::RateLimited)
    }

    pub fn check_paused(&self) -> Result<(), Error> {
        self.gate
            .status()
            .allows(self.intent(false), std::time::Instant::now())
            .then_some(())
            .ok_or(Error::RateLimited)
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
        let permit = self.begin_request()?;
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

        let value = read_json(req.json(&payload).send().await?, permit).await?;
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
        let permit = self.begin_request()?;
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
        read_json(response, permit).await
    }

    /// Fetches a plain GET text response (for channel RSS and search suggestions).
    pub async fn get_text(&self, url: &str) -> Result<String, Error> {
        let (response, permit) = self.get(url).await?;
        let text = response.text().await?;
        permit.success();
        Ok(text)
    }

    pub(crate) async fn get(&self, url: &str) -> Result<(reqwest::Response, RequestPermit), Error> {
        let permit = self.begin_request()?;
        let response = self
            .http
            .get(url)
            .header("User-Agent", USER_AGENT)
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await?;
        check_get(response, permit).await
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
        let permit = self.begin_request()?;
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
        let (_, permit) = check_get(response, permit).await?;
        permit.success();
        Ok(())
    }
}

pub(super) fn is_sorry(url: &Url) -> bool {
    matches!(url.host_str(), Some("www.google.com" | "google.com"))
        && (url.path() == "/sorry" || url.path().starts_with("/sorry/"))
}

pub(super) fn limiting_status(code: u16, message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    code == 429
        || message.contains("confirm you're not a bot")
        || message.contains("unusual traffic")
        || (code == 403 && (message.contains("rate limit") || message.contains("quota")))
}

/// InnerTube can describe a bot check inside an otherwise successful HTTP response.
pub fn response_reports_limit(code: u16, value: &Value) -> bool {
    ["/error/message", "/playabilityStatus/reason"]
        .into_iter()
        .any(|field| {
            value
                .pointer(field)
                .and_then(Value::as_str)
                .is_some_and(|text| limiting_status(code, text))
        })
}

fn retry_delay(response: &reqwest::Response) -> Option<Duration> {
    retry_after(
        response.headers().get("retry-after")?.to_str().ok()?,
        jiff::Timestamp::now(),
    )
}

fn limited(permit: RequestPermit, retry_after: Option<Duration>) -> Error {
    permit.limited(retry_after);
    Error::RateLimited
}

fn check_response(
    response: reqwest::Response,
    permit: RequestPermit,
) -> Result<(reqwest::Response, RequestPermit), Error> {
    let status = response.status();
    let sorry_redirect = status.is_redirection()
        && response
            .headers()
            .get("location")
            .and_then(|location| location.to_str().ok())
            .and_then(|url| response.url().join(url).ok())
            .is_some_and(|url| is_sorry(&url));
    if limiting_status(status.as_u16(), "") || sorry_redirect || is_sorry(response.url()) {
        return Err(limited(permit, retry_delay(&response)));
    }
    Ok((response, permit))
}

async fn check_get(
    response: reqwest::Response,
    permit: RequestPermit,
) -> Result<(reqwest::Response, RequestPermit), Error> {
    let (mut response, permit) = check_response(response, permit)?;
    let status = response.status();
    if status.is_success() {
        return Ok((response, permit));
    }
    // RSS and timed text can get a plain refusal rather than an API error.
    if status == reqwest::StatusCode::FORBIDDEN {
        let delay = retry_delay(&response);
        if let Ok(Some(chunk)) = response.chunk().await
            && limiting_status(status.as_u16(), &String::from_utf8_lossy(&chunk))
        {
            return Err(limited(permit, delay));
        }
    }
    Err(Error::HttpStatus(
        status.as_u16(),
        status.canonical_reason().unwrap_or("HTTP error").into(),
    ))
}

async fn read_json(response: reqwest::Response, permit: RequestPermit) -> Result<Value, Error> {
    let (response, permit) = check_response(response, permit)?;
    let status = response.status();
    let delay = retry_delay(&response);
    let text = response.text().await?;
    let json = serde_json::from_str::<Value>(&text);
    let message = json
        .as_ref()
        .ok()
        .and_then(|v| v.pointer("/error/message"))
        .and_then(Value::as_str);
    if json
        .as_ref()
        .is_ok_and(|value| response_reports_limit(status.as_u16(), value))
        || (!status.is_success() && limiting_status(status.as_u16(), &text))
    {
        return Err(limited(permit, delay));
    }
    if !status.is_success() {
        return Err(Error::HttpStatus(
            status.as_u16(),
            message
                .unwrap_or_else(|| status.canonical_reason().unwrap_or("HTTP error"))
                .into(),
        ));
    }
    let value = json.map_err(|err| Error::Extraction(format!("invalid JSON: {err}").into()))?;
    permit.success();
    Ok(value)
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
    fn one_load_spends_one_probe_authorization_across_client_clones() {
        let (rp, _proxy) = paused_client_with_local_proxy();
        rp.gate().elapse_pause();
        let user = rp.for_user_request();
        let child = user.public_client().clone();
        assert!(user.check_paused().is_ok());
        let first = child.begin_request().unwrap();
        assert!(first.is_probe());
        drop(first);
        assert_eq!(user.begin_request().err(), Some(Error::RateLimited));
        assert!(rp.for_user_request().begin_request().is_ok());
    }

    #[tokio::test]
    async fn an_open_circuit_prevents_recommendation_network_requests() {
        let (rp, proxy) = paused_client_with_local_proxy();
        // A recommendation would normally send an authenticated browse POST.
        *rp.cookie.write().unwrap() = Some("SAPISID=synthetic".into());
        assert_eq!(
            super::super::recommended(&rp).await.unwrap_err(),
            YOUTUBE_RATE_LIMITED
        );
        assert_eq!(
            proxy.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[tokio::test]
    async fn an_open_circuit_prevents_rss_network_requests() {
        use crate::innertube::{RssCache, feed, rss};
        use youtfast_core::model::Channel;

        let (rp, proxy) = paused_client_with_local_proxy();
        let channel = Channel {
            id: "UCsynthetic".into(),
            ..Channel::default()
        };
        assert_eq!(
            rss::fetch(&rp, &channel.id).await.unwrap_err(),
            Error::RateLimited
        );
        assert_eq!(
            feed(&rp, vec![channel], false, &RssCache::default())
                .await
                .unwrap_err(),
            YOUTUBE_RATE_LIMITED
        );
        assert_eq!(
            proxy.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    fn paused_client_with_local_proxy() -> (Client, std::net::TcpListener) {
        let proxy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        proxy.set_nonblocking(true).unwrap();
        let builder = reqwest::Client::builder()
            .timeout(Duration::from_millis(250))
            .proxy(reqwest::Proxy::all(format!("http://{}", proxy.local_addr().unwrap())).unwrap());
        let gate = RequestGate::default();
        gate.begin(RequestIntent::Background).unwrap().limited(None);
        (Client::with_gate(builder, gate).unwrap(), proxy)
    }

    async fn local_response(status: &str, headers: &str, body: &str) -> reqwest::Response {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let wire = format!(
            "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n{body}",
            body.len()
        );
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let _received = stream.read(&mut request).await.unwrap();
            stream.write_all(wire.as_bytes()).await.unwrap();
        });
        let response = super::super::http()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap()
            .get(format!("http://{addr}/"))
            .send()
            .await
            .unwrap();
        server.await.unwrap();
        response
    }

    #[tokio::test]
    async fn innertube_http_429_sorry_redirect_and_bot_check_open_the_shared_circuit() {
        use crate::backoff::Status;
        use std::time::Instant;

        for (status, headers, body, delay) in [
            ("429 Too Many Requests", "Retry-After: 360\r\n", "", 360),
            (
                "302 Found",
                "Location: https://www.google.com/sorry/index?continue=synthetic\r\n",
                "",
                120,
            ),
            (
                "403 Forbidden",
                "",
                r#"{"error":{"message":"Sign in to confirm you're not a bot"}}"#,
                120,
            ),
            (
                "200 OK",
                "",
                r#"{"playabilityStatus":{"reason":"Sign in to confirm you're not a bot"}}"#,
                120,
            ),
        ] {
            let gate = RequestGate::default();
            let permit = gate.begin(RequestIntent::Background).unwrap();
            let response = local_response(status, headers, body).await;
            let before = Instant::now();
            assert_eq!(
                read_json(response, permit).await.unwrap_err(),
                Error::RateLimited
            );
            let Status::PausedUntil(until) = gate.status() else {
                panic!("circuit stayed closed")
            };
            assert!(until >= before + Duration::from_secs(delay));
        }
        let gate = RequestGate::default();
        let permit = gate.begin(RequestIntent::Background).unwrap();
        let response = local_response(
            "403 Forbidden",
            "",
            r#"{"error":{"message":"Invalid credentials"}}"#,
        )
        .await;
        assert!(super::super::is_signed_out(
            &read_json(response, permit).await.unwrap_err()
        ));
        assert_eq!(gate.status(), Status::Ready);
    }

    #[test]
    fn retry_after_http_dates_use_the_supplied_wall_clock() {
        let now = "2015-10-21T07:20:00Z".parse().unwrap();
        assert_eq!(
            retry_after("Wed, 21 Oct 2015 07:28:00 GMT", now),
            Some(Duration::from_secs(480))
        );
        assert_eq!(
            retry_after("Wed, 21 Oct 2015 07:19:00 GMT", now),
            Some(Duration::ZERO)
        );
        assert_eq!(retry_after("invalid", now), None);
    }

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
