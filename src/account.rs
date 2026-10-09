//! The signed-in YouTube account: its session cookies, where they come
//! from, and where they are kept.
//!
//! YouTube has no sign-in for third-party clients that reaches history,
//! the home feed or Watch later (its Data API dropped them), so the session
//! is the one a browser holds: the `.youtube.com` cookies, from a
//! cookies.txt file or read from the browser through yt-dlp. Those cookies
//! are the account's session, so they only ever live in the platform
//! credential store, never in a file and never in the log. rustypipe's
//! cache is written through [`CacheFile`], which strips them.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;

const SERVICE: &str = "youtfast";
/// One entry per account: `youtube-session-<id>`. `youtube-session` was
/// the single account's key before there were names.
const COOKIE_KEY: &str = "youtube-session";
/// How long a credential store may take before it counts as unavailable.
const STORE_TIMEOUT: Duration = Duration::from_secs(20);

/// The `Cookie` header value for YouTube from a Netscape cookies.txt:
/// `.youtube.com` cookies for every path under `/`, as rustypipe and yt-dlp
/// read them. Lines for other sites are skipped.
pub fn cookie_header(cookies_txt: &str) -> Result<String, String> {
    let mut pairs = Vec::new();
    for line in cookies_txt.lines() {
        let line = line.trim();
        let line = match line.strip_prefix("#HttpOnly_") {
            Some(line) => line,
            None if line.is_empty() || line.starts_with('#') => continue,
            None => line,
        };
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 7 {
            return Err("This is not a cookies.txt file (Netscape format).".to_owned());
        }
        let [domain, subdomains, path, _secure, _expires, name, value] = [
            fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6],
        ];
        if domain == ".youtube.com" && subdomains.eq_ignore_ascii_case("TRUE") && path == "/" {
            pairs.push(format!("{name}={value}"));
        }
    }
    let header = pairs.join("; ");
    if !is_signed_in(&header) {
        return Err(
            "These cookies have no signed-in YouTube session. Sign in to \
                    youtube.com before exporting them."
                .to_owned(),
        );
    }
    Ok(header)
}

/// Whether a cookie header carries a Google session: YouTube signs
/// requests with SAPISID (or its __Secure- twin).
fn is_signed_in(header: &str) -> bool {
    header
        .split("; ")
        .any(|pair| pair.starts_with("SAPISID=") || pair.starts_with("__Secure-3PAPISID="))
}

/// Reads this browser's YouTube cookies with yt-dlp's cookie extraction,
/// which knows each browser's database and encryption. Only `.youtube.com`
/// cookies come back, through a pipe; nothing is written to disk.
///
/// Chromium-based browsers on Linux encrypt cookies with a key kept in the
/// desktop keyring. yt-dlp guesses which keyring from the desktop, and
/// guesses wrong on Hyprland and other window managers, so each is tried.
const EXTRACT_COOKIES: &str = r#"
import shutil, sys
try:
    from yt_dlp.cookies import YDLLogger, extract_cookies_from_browser
except ImportError:
    # yt-dlp installed as a single-file zipapp can be imported from itself.
    path = shutil.which("yt-dlp")
    if path:
        sys.path.insert(0, path)
    from yt_dlp.cookies import YDLLogger, extract_cookies_from_browser

class Logger(YDLLogger):
    # Without a YoutubeDL behind it, yt-dlp's logger drops every message,
    # including the one saying why a keyring could not be read.
    def warning(self, message, only_once=False):
        print(message, file=sys.stderr)

    def error(self, message, *, is_error=True):
        print(message, file=sys.stderr)

browser = sys.argv[1]
profile = sys.argv[2] if len(sys.argv) > 2 and sys.argv[2] else None
# "keyring" prints only which keyring opened the browser, not the cookies.
only_keyring = len(sys.argv) > 3 and sys.argv[3] == "keyring"
keyrings = [None]
if browser not in ("firefox", "safari"):
    keyrings += ["GNOMEKEYRING", "KWALLET6", "KWALLET5", "BASICTEXT"]
for keyring in keyrings:
    try:
        jar = extract_cookies_from_browser(browser, profile, logger=Logger(), keyring=keyring)
    except Exception as error:
        print(f"{keyring}: {error}", file=sys.stderr)
        continue
    cookies = [c for c in jar if c.domain == ".youtube.com"]
    if any(c.name in ("SAPISID", "__Secure-3PAPISID") and c.value for c in cookies):
        if only_keyring:
            print(keyring or "")
            sys.exit(0)
        for c in cookies:
            print("\t".join([c.domain, "TRUE", c.path, "TRUE" if c.secure else "FALSE",
                             str(c.expires or 0), c.name, c.value or ""]))
        sys.exit(0)
sys.exit(3)
"#;

/// `browser` is yt-dlp's name, optionally `browser:profile`.
pub fn split_browser(spec: &str) -> (String, Option<String>) {
    match spec.split_once(':') {
        Some((browser, profile)) => (browser.to_owned(), Some(profile.to_owned())),
        None => (spec.to_owned(), None),
    }
}

pub async fn cookies_from_browser(spec: &str) -> Result<String, String> {
    let cookies = extract(spec, "cookies").await?;
    cookie_header(&cookies)
}

/// yt-dlp's `--cookies-from-browser` value for this browser, naming the
/// keyring that opened it: the yt-dlp mpv runs would guess as wrongly as
/// it does here. Nothing secret is in it.
pub async fn ytdl_browser(spec: &str) -> Result<String, String> {
    let keyring = extract(spec, "keyring").await?;
    let (browser, profile) = split_browser(spec);
    Ok(ytdl_browser_spec(
        &browser,
        keyring.trim(),
        profile.as_deref(),
    ))
}

/// `browser[+keyring][:profile]`, as yt-dlp takes it.
fn ytdl_browser_spec(browser: &str, keyring: &str, profile: Option<&str>) -> String {
    let mut spec = browser.to_owned();
    if !keyring.is_empty() {
        spec.push('+');
        spec.push_str(&keyring.to_lowercase());
    }
    if let Some(profile) = profile {
        spec.push(':');
        spec.push_str(profile);
    }
    spec
}

/// Runs [`EXTRACT_COOKIES`] on a browser; `what` is `cookies` or
/// `keyring`, and the script's output comes back as text.
async fn extract(spec: &str, what: &str) -> Result<String, String> {
    let (browser, profile) = split_browser(spec);
    let output = Command::new("python3")
        .arg("-c")
        .arg(EXTRACT_COOKIES)
        .arg(&browser)
        .arg(profile.unwrap_or_default())
        .arg(what)
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|error| format!("could not run Python for yt-dlp: {error}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        // stderr names files and keyrings, never cookie values.
        log::warn!("reading {browser}'s cookies failed: {}", stderr.trim());
        let browser = crate::model::browser_label(&browser);
        return Err(if stderr.contains("No module named 'yt_dlp'") {
            "yt-dlp is not installed for Python, so the browser cannot be read.".to_owned()
        } else if stderr.contains("secretstorage not available") {
            format!(
                "{browser} keeps its cookies encrypted with the desktop keyring, which \
                 needs Python's secretstorage module (python-secretstorage on Arch)."
            )
        } else if stderr.contains("could not find") && stderr.contains("cookies database") {
            format!("{browser} has no cookies here. Is it installed and used?")
        } else {
            format!(
                "{browser} has no signed-in YouTube session. Sign in on youtube.com there first."
            )
        });
    }
    String::from_utf8(output.stdout).map_err(|_| "the browser's cookies were not text".to_owned())
}

/// The browsers whose data is on this computer, by yt-dlp's names.
pub fn installed_browsers() -> Vec<&'static str> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    crate::model::BROWSERS
        .into_iter()
        .filter(|browser| {
            let place = match *browser {
                "chromium" => config.join("chromium"),
                "chrome" => config.join("google-chrome"),
                "brave" => config.join("BraveSoftware/Brave-Browser"),
                "firefox" => home.join(".mozilla/firefox"),
                "vivaldi" => config.join("vivaldi"),
                "edge" => config.join("microsoft-edge"),
                "opera" => config.join("opera"),
                _ => return false,
            };
            place.exists()
        })
        .collect()
}

fn credential_store() -> Result<Arc<keyring_core::api::CredentialStore>, keyring_core::Error> {
    #[cfg(target_os = "linux")]
    let store = zbus_secret_service_keyring_store::Store::new()?;
    #[cfg(target_os = "macos")]
    let store = apple_native_keyring_store::keychain::Store::new()?;
    #[cfg(windows)]
    let store = windows_native_keyring_store::Store::new()?;
    Ok(store)
}

/// What a credential store failure means to the user. The store's own
/// error text can carry platform data, so it is never shown or logged.
fn store_error(error: keyring_core::Error) -> String {
    match error {
        keyring_core::Error::NoStorageAccess(_) => {
            "The system keyring is locked. Unlock it to stay signed in.".to_owned()
        }
        _ => "The system keyring is unavailable, so the session cannot be kept.".to_owned(),
    }
}

fn entry(key: &str) -> Result<keyring_core::Entry, String> {
    credential_store()
        .and_then(|store| store.build(SERVICE, key, None))
        .map_err(store_error)
}

/// The account's keyring key.
fn session_key(id: u32) -> String {
    format!("{COOKIE_KEY}-{id}")
}

/// Runs a blocking credential-store call off the runtime's workers, and
/// gives up on a store that does not answer (a keyring waiting on a
/// password prompt nobody sees).
async fn with_store<T: Send + 'static>(
    call: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    match tokio::time::timeout(STORE_TIMEOUT, tokio::task::spawn_blocking(call)).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("The system keyring failed.".to_owned()),
        Err(_) => Err("The system keyring did not respond.".to_owned()),
    }
}

pub async fn save_session(id: u32, cookie: String) -> Result<(), String> {
    with_store(move || {
        entry(&session_key(id))?
            .set_secret(cookie.as_bytes())
            .map_err(store_error)
    })
    .await
}

pub async fn load_session(id: u32) -> Result<Option<String>, String> {
    with_store(move || match entry(&session_key(id))?.get_secret() {
        Ok(secret) => String::from_utf8(secret)
            .map(Some)
            .map_err(|_| "The stored session is damaged.".to_owned()),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(error) => Err(store_error(error)),
    })
    .await
}

pub async fn delete_session(id: u32) -> Result<(), String> {
    with_store(move || match entry(&session_key(id))?.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(error) => Err(store_error(error)),
    })
    .await
}

/// The single account of older versions was `youtube-session`: move it
/// under account 1's key. Reads back the moved session before removing
/// the old entry, so a failure leaves it in place.
pub async fn migrate_legacy_session() -> Result<Option<String>, String> {
    if load_session(1).await?.is_some() {
        return load_session(1).await;
    }
    let old = with_store(|| match entry(COOKIE_KEY)?.get_secret() {
        Ok(secret) => String::from_utf8(secret)
            .map(Some)
            .map_err(|_| "The stored session is damaged.".to_owned()),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(error) => Err(store_error(error)),
    })
    .await?;
    let Some(cookie) = old else {
        return Ok(None);
    };
    save_session(1, cookie).await?;
    let back = load_session(1).await?;
    if back.is_some() {
        with_store(|| match entry(COOKIE_KEY)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(error) => Err(store_error(error)),
        })
        .await?;
    }
    Ok(back)
}

/// rustypipe's cache file (client versions, the player's deobfuscation
/// data), with the session taken out: rustypipe keeps the cookies and any
/// OAuth token in the same JSON, which must not reach the disk.
pub struct CacheFile {
    pub path: PathBuf,
}

impl rustypipe::cache::CacheStorage for CacheFile {
    fn write(&self, data: &str) {
        let Some(clean) = without_credentials(data) else {
            log::warn!("not writing rustypipe's cache: it was not the JSON expected");
            return;
        };
        if let Err(error) = crate::paths::write_atomic(&self.path, clean.as_bytes()) {
            log::warn!("could not write rustypipe's cache: {error}");
        }
    }

    fn read(&self) -> Option<String> {
        std::fs::read_to_string(&self.path).ok()
    }
}

/// The cache without `auth_cookie` and `oauth_token`, or `None` when it is
/// not the JSON object expected (then nothing is written at all).
fn without_credentials(data: &str) -> Option<String> {
    let mut cache: serde_json::Map<String, serde_json::Value> = serde_json::from_str(data).ok()?;
    cache.remove("auth_cookie");
    cache.remove("oauth_token");
    serde_json::to_string(&cache).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yt_dlp_is_told_the_keyring_and_profile() {
        assert_eq!(ytdl_browser_spec("firefox", "", None), "firefox");
        assert_eq!(
            ytdl_browser_spec("chromium", "GNOMEKEYRING", None),
            "chromium+gnomekeyring"
        );
        assert_eq!(
            ytdl_browser_spec("chromium", "KWALLET6", Some("Profile 1")),
            "chromium+kwallet6:Profile 1"
        );
    }

    #[test]
    fn a_browser_spec_splits_at_the_profile() {
        assert_eq!(split_browser("chromium"), ("chromium".to_owned(), None));
        assert_eq!(
            split_browser("chromium:Profile 1"),
            ("chromium".to_owned(), Some("Profile 1".to_owned()))
        );
    }

    #[test]
    fn the_label_names_the_browser_and_profile() {
        assert_eq!(crate::model::browser_label("chromium"), "Chromium");
        assert_eq!(
            crate::model::browser_label("chromium:Profile 1"),
            "Chromium (Profile 1)"
        );
        assert_eq!(crate::model::browser_label("firefox"), "Firefox");
    }

    const COOKIES: &str = "# Netscape HTTP Cookie File\n\
        # This is a generated file! Do not edit.\n\
        \n\
        .youtube.com\tTRUE\t/\tTRUE\t1790000000\tPREF\tf6=40000000\n\
        #HttpOnly_.youtube.com\tTRUE\t/\tTRUE\t1790000000\tSID\tsid-value\n\
        .youtube.com\tTRUE\t/\tTRUE\t1790000000\tSAPISID\tsapisid/value\n\
        .youtube.com\tTRUE\t/feed\tTRUE\t1790000000\tPATHED\tskipped\n\
        www.youtube.com\tFALSE\t/\tTRUE\t1790000000\tHOSTONLY\tskipped\n\
        .google.com\tTRUE\t/\tTRUE\t1790000000\tNID\tother-site\n";

    #[test]
    fn header_from_cookies_txt() {
        assert_eq!(
            cookie_header(COOKIES).unwrap(),
            "PREF=f6=40000000; SID=sid-value; SAPISID=sapisid/value"
        );
    }

    #[test]
    fn signed_out_cookies_are_refused() {
        let signed_out = ".youtube.com\tTRUE\t/\tTRUE\t1790000000\tPREF\tf6=40000000\n";
        assert!(
            cookie_header(signed_out)
                .unwrap_err()
                .contains("no signed-in")
        );
        assert!(cookie_header("<html>not cookies</html>").is_err());
    }

    #[test]
    fn cache_never_keeps_the_session() {
        let cache = r#"{"clients": {"desktop": {"version": "2.2026"}},
                        "auth_cookie": {"cookie": "SAPISID=secret"},
                        "oauth_token": {"access_token": "secret"}}"#;
        let clean = without_credentials(cache).unwrap();
        assert!(!clean.contains("secret"));
        assert!(clean.contains("2.2026"));
        assert_eq!(without_credentials("not json"), None);
    }
}
