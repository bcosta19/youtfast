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
    let mut command = Command::new("python3");
    // The release build is a GUI app on Windows; without this flag every
    // import would flash a console window.
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let output = command
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
        // stderr is yt-dlp's and Python's text, not ours: it is matched
        // for the known failures below but never logged, since what it
        // carries besides them is not ours to vouch for.
        log::warn!("reading browser cookies failed");
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
                // Newer Firefox keeps its profiles in the XDG folder.
                "firefox" if config.join("mozilla/firefox").exists() => {
                    config.join("mozilla/firefox")
                }
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

/// A browser profile the sign-in form offers: one click signs in with its
/// YouTube login, so nobody has to know the profile's folder name.
#[derive(Clone, Debug, PartialEq)]
pub struct BrowserProfile {
    /// yt-dlp's name for the browser.
    pub browser: &'static str,
    /// The profile as yt-dlp takes it after `browser:` (a folder under the
    /// browser's own, or a whole path); `None` is the browser's default.
    pub profile: Option<String>,
    /// The name the browser shows for it ("Personal", "Person 1").
    pub name: String,
    /// The Google account the browser itself is signed in to, if any: a
    /// hint for picking, shown only in the form.
    pub email: Option<String>,
}

impl BrowserProfile {
    /// `browser[:profile]`, as an account's source keeps it.
    pub fn spec(&self) -> String {
        match &self.profile {
            Some(profile) => format!("{}:{profile}", self.browser),
            None => self.browser.to_owned(),
        }
    }

    /// The account name to suggest: the e-mail's user, else the profile's
    /// own name unless it is a browser's placeholder.
    pub fn suggested_name(&self) -> String {
        if let Some(email) = &self.email
            && let Some(user) = email.split('@').next()
            && !user.is_empty()
        {
            return user.to_owned();
        }
        let placeholder = self.name.is_empty()
            || self.name == "Default"
            || self.name.starts_with("Person ")
            || self.name.starts_with("Profile ")
            || self.name.starts_with("default");
        if placeholder {
            crate::model::browser_label(self.browser)
        } else {
            self.name.clone()
        }
    }
}

/// Every profile of every browser on this computer, as far as the
/// browsers' own profile lists tell (Chromium's `Local State`, Firefox's
/// `profiles.ini`). Only names are read, never cookies.
pub fn browser_profiles() -> Vec<BrowserProfile> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    let mut profiles = Vec::new();
    for browser in installed_browsers() {
        if browser == "firefox" {
            // yt-dlp looks in ~/.mozilla/firefox; a profile kept in the
            // XDG folder goes as its whole path.
            let classic = home.join(".mozilla/firefox");
            let xdg = config.join("mozilla/firefox");
            for (root, whole_path) in [(classic, false), (xdg, true)] {
                let Ok(ini) = std::fs::read_to_string(root.join("profiles.ini")) else {
                    continue;
                };
                for (path, name, relative) in firefox_profiles(&ini) {
                    let profile = if relative && !whole_path {
                        path
                    } else if relative {
                        root.join(&path).display().to_string()
                    } else {
                        path
                    };
                    profiles.push(BrowserProfile {
                        browser,
                        profile: Some(profile),
                        name,
                        email: None,
                    });
                }
            }
            continue;
        }
        let Some(dir) = chromium_dir(browser, &config) else {
            continue;
        };
        let found = std::fs::read_to_string(dir.join("Local State"))
            .map(|json| chromium_profiles(&json))
            .unwrap_or_default();
        if found.is_empty() {
            profiles.push(BrowserProfile {
                browser,
                profile: None,
                name: String::new(),
                email: None,
            });
        }
        for (folder, name, email) in found {
            profiles.push(BrowserProfile {
                browser,
                // "Default" is what yt-dlp reads with no profile at all.
                profile: (folder != "Default").then_some(folder),
                name,
                email,
            });
        }
    }
    profiles
}

/// Where a Chromium-family browser keeps its profiles.
fn chromium_dir(browser: &str, config: &std::path::Path) -> Option<PathBuf> {
    Some(config.join(match browser {
        "chromium" => "chromium",
        "chrome" => "google-chrome",
        "brave" => "BraveSoftware/Brave-Browser",
        "vivaldi" => "vivaldi",
        "edge" => "microsoft-edge",
        "opera" => "opera",
        _ => return None,
    }))
}

/// `(folder, name, e-mail)` of each profile in a Chromium `Local State`,
/// "Default" first, then by folder.
fn chromium_profiles(local_state: &str) -> Vec<(String, String, Option<String>)> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(local_state) else {
        return Vec::new();
    };
    let Some(cache) = json["profile"]["info_cache"].as_object() else {
        return Vec::new();
    };
    let mut profiles: Vec<(String, String, Option<String>)> = cache
        .iter()
        .map(|(folder, info)| {
            let name = info["name"]
                .as_str()
                .or_else(|| info["gaia_name"].as_str())
                .unwrap_or(folder)
                .to_owned();
            let email = info["user_name"]
                .as_str()
                .filter(|email| email.contains('@'))
                .map(str::to_owned);
            (folder.clone(), name, email)
        })
        .collect();
    profiles.sort_by_key(|(folder, _, _)| (folder != "Default", folder.clone()));
    profiles
}

/// `(path, name, relative)` of each profile in a Firefox `profiles.ini`,
/// the default one first.
fn firefox_profiles(ini: &str) -> Vec<(String, String, bool)> {
    let mut profiles = Vec::new();
    let mut defaults = Vec::new();
    let mut section = String::new();
    let (mut path, mut name, mut relative) = (None, None, true);
    let mut flush = |section: &str,
                     path: &mut Option<String>,
                     name: &mut Option<String>,
                     relative: &mut bool| {
        if section.starts_with("Profile")
            && let Some(found) = path.take()
        {
            let label = name.take().unwrap_or_else(|| found.clone());
            profiles.push((found, label, *relative));
        }
        *name = None;
        *relative = true;
    };
    for line in ini.lines().map(str::trim) {
        if let Some(header) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            flush(&section, &mut path, &mut name, &mut relative);
            section = header.to_owned();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "Path" => path = Some(value.to_owned()),
            "Name" => name = Some(value.to_owned()),
            "IsRelative" => relative = value != "0",
            // The [Install…] sections name the profile each install uses.
            "Default" if section.starts_with("Install") => defaults.push(value.to_owned()),
            _ => {}
        }
    }
    flush(&section, &mut path, &mut name, &mut relative);
    profiles.sort_by_key(|(path, _, _)| !defaults.contains(path));
    profiles
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

/// One credential-store call at a time: the timeout abandons the *wait*,
/// never the call, so a store that hangs serializes its failures instead
/// of stacking abandoned threads.
static STORE_CALLS: std::sync::LazyLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(tokio::sync::Semaphore::new(1)));

/// Runs a blocking credential-store call off the runtime's workers, and
/// gives up on a store that does not answer (a keyring waiting on a
/// password prompt nobody sees).
async fn with_store<T: Send + 'static>(
    call: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    with_store_timeout(STORE_CALLS.clone(), STORE_TIMEOUT, call).await
}

async fn with_store_timeout<T: Send + 'static>(
    store: std::sync::Arc<tokio::sync::Semaphore>,
    timeout: std::time::Duration,
    call: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let Ok(Ok(turn)) = tokio::time::timeout(timeout, store.acquire_owned()).await else {
        return Err("The system keyring did not respond.".to_owned());
    };
    match tokio::time::timeout(
        timeout,
        tokio::task::spawn_blocking(move || {
            let _turn = turn;
            call()
        }),
    )
    .await
    {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("The system keyring failed.".to_owned()),
        Err(_) => Err("The system keyring did not respond.".to_owned()),
    }
}

pub async fn save_session(id: u32, cookie: String) -> Result<(), String> {
    with_store(move || {
        // Windows' Credential Manager caps a secret at 2560 bytes, which a
        // YouTube session can exceed; the store's own error would not say
        // why, and it is not shown anyway.
        #[cfg(windows)]
        if cookie.len() > 2560 {
            return Err(
                "This session is too large for the Windows Credential Manager \
                 (2560 bytes), so it cannot be kept there."
                    .to_owned(),
            );
        }
        entry(&session_key(id))?
            .set_secret(cookie.as_bytes())
            .map_err(store_error)
    })
    .await
}

pub async fn load_session(id: u32) -> Result<Option<String>, String> {
    read_secret(session_key(id)).await
}

pub async fn delete_session(id: u32) -> Result<(), String> {
    delete_secret(session_key(id)).await
}

/// The session kept under `key`, if any.
async fn read_secret(key: String) -> Result<Option<String>, String> {
    with_store(move || match entry(&key)?.get_secret() {
        Ok(secret) => String::from_utf8(secret)
            .map(Some)
            .map_err(|_| "The stored session is damaged.".to_owned()),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(error) => Err(store_error(error)),
    })
    .await
}

/// Deletes the entry under `key`; one already gone counts as deleted.
async fn delete_secret(key: String) -> Result<(), String> {
    with_store(move || match entry(&key)?.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(error) => Err(store_error(error)),
    })
    .await
}

/// The single account of older versions was `youtube-session`: move it
/// under account 1's key. Reads back the moved session before removing
/// the old entry, so a failure leaves it in place.
pub async fn migrate_legacy_session() -> Result<Option<String>, String> {
    if let Some(session) = load_session(1).await? {
        return Ok(Some(session));
    }
    let Some(cookie) = read_secret(COOKIE_KEY.to_owned()).await? else {
        return Ok(None);
    };
    save_session(1, cookie).await?;
    let back = load_session(1).await?;
    if back.is_some() {
        delete_secret(COOKIE_KEY.to_owned()).await?;
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
        if let Err(error) = crate::storage::paths::write_atomic(&self.path, clean.as_bytes()) {
            log::warn!("could not write rustypipe's cache: {error}");
        }
    }

    fn read(&self) -> Option<String> {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|data| without_credentials(&data))
    }
}

/// Names under which rustypipe keeps a session, dropped wherever they
/// appear in the cache. rustypipe today puts them at the top; a later
/// version could nest them, so the strip walks the whole JSON.
const CREDENTIAL_KEYS: &[&str] = &[
    "auth_cookie",
    "oauth_token",
    "cookie",
    "cookies",
    "access_token",
    "refresh_token",
    "id_token",
    "authorization",
];

/// What a session looks like inside a value: the signed-in cookie names
/// and a Google access token's `ya29.` prefix. The cache is not written
/// at all while any remains, under any key.
const SESSION_MARKERS: &[&str] = &["SAPISID=", "__Secure-3PAPISID=", "ya29."];

/// The cache keeps only the fields it is written for (`clients`,
/// `deobf`); the session is taken out of what remains, or `None` when
/// the JSON is not the object expected or still names a session
/// somewhere (then nothing is written at all).
fn without_credentials(data: &str) -> Option<String> {
    let mut cache: serde_json::Map<String, serde_json::Value> = serde_json::from_str(data).ok()?;
    cache.retain(|key, _| matches!(key.as_str(), "clients" | "deobf"));
    for value in cache.values_mut() {
        strip_credentials(value);
    }
    let clean = serde_json::to_string(&cache).ok()?;
    // A session under a name not known here refuses the whole write.
    if SESSION_MARKERS.iter().any(|marker| clean.contains(marker)) {
        return None;
    }
    Some(clean)
}

/// Drops every credential-named field of `value`, at any depth.
fn strip_credentials(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|key, _| !CREDENTIAL_KEYS.contains(&key.as_str()));
            for nested in map.values_mut() {
                strip_credentials(nested);
            }
        }
        serde_json::Value::Array(items) => {
            for nested in items {
                strip_credentials(nested);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn timed_out_calls_keep_the_store_permit_until_the_blocking_call_finishes() {
        let store = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let (release, wait) = std::sync::mpsc::channel();
        let timeout = std::time::Duration::from_millis(50);
        let first = with_store_timeout(store.clone(), timeout, move || {
            let _ = wait.recv_timeout(std::time::Duration::from_secs(2));
            Ok(())
        })
        .await;
        let available = store.available_permits();
        let ran = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let second_ran = ran.clone();
        let second = with_store_timeout(store.clone(), timeout, move || {
            second_ran.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .await;
        release.send(()).unwrap();
        let permit = tokio::time::timeout(std::time::Duration::from_secs(2), store.acquire_owned())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(available, 0);
        assert!(first.is_err() && second.is_err());
        assert!(!ran.load(std::sync::atomic::Ordering::SeqCst));
        drop(permit);
    }

    #[test]
    fn chromium_profiles_come_with_their_names_and_accounts() {
        let local_state = r#"{"profile": {"info_cache": {
            "Profile 1": {"name": "Work", "user_name": "me@work.example"},
            "Default": {"name": "Person 1", "user_name": ""}
        }}}"#;
        assert_eq!(
            chromium_profiles(local_state),
            vec![
                ("Default".to_owned(), "Person 1".to_owned(), None),
                (
                    "Profile 1".to_owned(),
                    "Work".to_owned(),
                    Some("me@work.example".to_owned())
                ),
            ]
        );
        assert!(chromium_profiles("{}").is_empty());
    }

    #[test]
    fn firefox_profiles_put_the_default_first() {
        let ini = "[Profile1]\nName=default\nIsRelative=1\nPath=aaaa.default\n\n\
                   [Profile0]\nName=Personal\nIsRelative=1\nPath=bbbb.default-release\n\n\
                   [Profile2]\nName=Elsewhere\nIsRelative=0\nPath=/data/ff\n\n\
                   [Install4F96D1932A9F858E]\nDefault=bbbb.default-release\nLocked=1\n";
        assert_eq!(
            firefox_profiles(ini),
            vec![
                (
                    "bbbb.default-release".to_owned(),
                    "Personal".to_owned(),
                    true
                ),
                ("aaaa.default".to_owned(), "default".to_owned(), true),
                ("/data/ff".to_owned(), "Elsewhere".to_owned(), false),
            ]
        );
    }

    #[test]
    fn a_profile_suggests_a_name_for_its_account() {
        let profile = |name: &str, email: Option<&str>| BrowserProfile {
            browser: "chrome",
            profile: None,
            name: name.to_owned(),
            email: email.map(str::to_owned),
        };
        assert_eq!(
            profile("Person 1", Some("ana@gmail.com")).suggested_name(),
            "ana"
        );
        assert_eq!(profile("Work", None).suggested_name(), "Work");
        assert_eq!(profile("Person 1", None).suggested_name(), "Google Chrome");
        assert_eq!(
            BrowserProfile {
                profile: Some("Profile 2".to_owned()),
                ..profile("x", None)
            }
            .spec(),
            "chrome:Profile 2"
        );
    }

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
    fn legacy_cache_credentials_cannot_restore_an_unselected_account() {
        use rustypipe::cache::CacheStorage;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("legacy-cache-{}.json", std::process::id()));
        std::fs::write(&path, br#"{"clients":{"desktop":{"version":"2.2026"}},"auth_cookie":"SAPISID=synthetic-secret","oauth_token":{"access_token":"ya29.synthetic-secret"}}"#).unwrap();
        let clean = CacheFile { path: path.clone() }.read().unwrap();
        std::fs::remove_file(path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&clean).unwrap();
        assert_eq!(value["clients"]["desktop"]["version"], "2.2026");
        assert!(value["auth_cookie"].is_null() && value["oauth_token"].is_null());
        assert!(!clean.contains("synthetic-secret"));
    }

    #[test]
    fn cache_never_keeps_the_session() {
        let cache = r#"{"clients": {"desktop": {"version": "2.2026"}},
                        "auth_cookie": {"cookie": "SAPISID=secret"},
                        "oauth_token": {"access_token": "ya29.secret"},
                        "a_field_added_later": {"cookie": "SAPISID=secret2"}}"#;
        let clean = without_credentials(cache).unwrap();
        assert!(!clean.contains("secret"));
        assert!(!clean.contains("SAPISID"));
        assert!(clean.contains("2.2026"));
        // A credential nested inside a kept field is stripped too.
        let nested = r#"{"deobf": {"data": {"code": "x"}, "auth_cookie": "SAPISID=s"}}"#;
        let clean = without_credentials(nested).unwrap();
        assert!(!clean.contains("SAPISID"));
        assert!(clean.contains("code"));
        // A session under a name not known refuses the whole write.
        let hidden = r#"{"clients": {"desktop": {"sid": "SAPISID=hidden"}}}"#;
        assert_eq!(without_credentials(hidden), None);
        assert_eq!(without_credentials("not json"), None);
    }
}
