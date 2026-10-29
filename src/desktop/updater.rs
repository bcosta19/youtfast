//! Application version information, changelog extraction, GitHub release
//! checks, and verified self-update installation.
//!
//! Minor (`0.x.0`) and patch (`0.0.x`) updates can be installed automatically
//! when the user opts in; major (`x.0.0`) updates always require explicit
//! user confirmation via the update button in the interface.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::appearance::i18n::{Locale, format as trf, gettext as tr};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RELEASES_URL: &str = "https://github.com/bcosta19/youtfast/releases";
const LATEST_RELEASE_API: &str = "https://api.github.com/repos/bcosta19/youtfast/releases/latest";
const BUNDLED_CHANGELOG: &str = include_str!("../../CHANGELOG.md");

const MAX_API_BYTES: usize = 256 * 1024;
const MAX_CHECKSUMS_BYTES: usize = 64 * 1024;
const MAX_ARCHIVE_BYTES: usize = 150 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().strip_prefix('v').unwrap_or(raw.trim());
        // Pre-release tags (e.g. "0.3.0-rc.1") are not stable releases.
        if trimmed.contains('-') {
            return None;
        }
        let core = trimmed.split('+').next()?;
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }

    pub fn classify_update(current: Self, candidate: Self) -> Option<UpdateKind> {
        if candidate <= current {
            return None;
        }
        if candidate.major > current.major {
            Some(UpdateKind::Major)
        } else if candidate.minor > current.minor {
            Some(UpdateKind::Minor)
        } else {
            Some(UpdateKind::Patch)
        }
    }
}

/// How a newer release relates to the installed version under Semantic Versioning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateKind {
    Patch,
    Minor,
    Major,
}

impl UpdateKind {
    pub fn is_major(self) -> bool {
        matches!(self, Self::Major)
    }

    pub fn is_minor_or_patch(self) -> bool {
        matches!(self, Self::Patch | Self::Minor)
    }
}

/// Returns whether an update of `kind` should be installed automatically
/// without waiting for manual confirmation. Major versions never auto-install.
pub fn should_auto_update(auto_update_minor_patch: bool, kind: UpdateKind) -> bool {
    auto_update_minor_patch && kind.is_minor_or_patch()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub version: String,
    pub kind: UpdateKind,
    pub changelog: String,
    pub html_url: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub checksums_url: Option<String>,
}

impl ReleaseInfo {
    /// Whether this release carries both a platform archive and a checksums
    /// manifest for verified in-app installation.
    pub fn can_self_update(&self) -> bool {
        self.asset_name.is_some() && self.asset_url.is_some() && self.checksums_url.is_some()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseInfo),
    Installing(ReleaseInfo),
    Installed {
        version: String,
        kind: UpdateKind,
        changelog: String,
        auto: bool,
    },
    Failed(String),
}

pub fn platform_asset_name(version: &str, os: &str, arch: &str) -> Option<String> {
    match (os, arch) {
        ("linux", "x86_64") => Some(format!(
            "youtfast-v{version}-x86_64-unknown-linux-gnu.tar.gz"
        )),
        ("macos", "aarch64") => Some(format!("youtfast-v{version}-aarch64-apple-darwin.zip")),
        ("windows", "x86_64") => Some(format!("youtfast-v{version}-x86_64-pc-windows-gnu.zip")),
        _ => None,
    }
}

/// Extracts and cleans the changelog section for `version` from the bundled
/// `CHANGELOG.md`, falling back to `Unreleased` highlights if needed.
pub fn bundled_changelog_for(version: &str) -> String {
    extract_changelog_section(BUNDLED_CHANGELOG, version)
        .or_else(|| extract_changelog_section(BUNDLED_CHANGELOG, "Unreleased"))
        .unwrap_or_default()
}

pub fn extract_changelog_section(markdown: &str, version: &str) -> Option<String> {
    let target = format!("## [{version}]");
    let mut inside = false;
    let mut lines = Vec::new();
    for line in markdown.lines() {
        if line.starts_with("## [") {
            if inside {
                break;
            }
            if line.starts_with(&target) {
                inside = true;
            }
            continue;
        }
        if inside {
            if line.starts_with('[') && line.contains("]: http") {
                continue;
            }
            lines.push(line);
        }
    }
    let cleaned = clean_changelog(&lines.join("\n"));
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Normalizes Markdown release notes into clean, readable lines for the UI.
pub fn clean_changelog(markdown: &str) -> String {
    let mut out = Vec::new();
    let mut current_bullet = String::new();
    let flush_bullet = |bullet: &mut String, out: &mut Vec<String>| {
        if !bullet.is_empty() {
            out.push(format!("• {}", strip_markdown_links(bullet.trim())));
            bullet.clear();
        }
    };
    for raw_line in markdown.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            flush_bullet(&mut current_bullet, &mut out);
            continue;
        }
        if line.starts_with('[') && line.contains("]: http") {
            continue;
        }
        if let Some(heading) = line
            .strip_prefix("### ")
            .or_else(|| line.strip_prefix("## "))
        {
            flush_bullet(&mut current_bullet, &mut out);
            if !out.is_empty() {
                out.push(String::new());
            }
            out.push(strip_markdown_links(heading.trim()));
        } else if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            flush_bullet(&mut current_bullet, &mut out);
            current_bullet.push_str(item.trim());
        } else if !current_bullet.is_empty() {
            current_bullet.push(' ');
            current_bullet.push_str(line);
        } else {
            out.push(strip_markdown_links(line));
        }
    }
    flush_bullet(&mut current_bullet, &mut out);
    out.join("\n").trim().to_owned()
}

fn strip_markdown_links(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        result.push_str(&rest[..open]);
        let after_open = &rest[open + 1..];
        if let Some(close) = after_open.find(']') {
            let label = &after_open[..close];
            let after_close = &after_open[close + 1..];
            if !label.contains('[')
                && let Some(inside_paren) = after_close.strip_prefix('(')
                && let Some(end_paren) = inside_paren.find(')')
            {
                result.push_str(label);
                rest = &inside_paren[end_paren + 1..];
                continue;
            }
        }
        result.push('[');
        rest = after_open;
    }
    result.push_str(rest);
    result.replace('`', "")
}

fn allowed_release_url(raw: &str) -> bool {
    let Ok(parsed) = url::Url::parse(raw) else {
        return false;
    };
    #[cfg(test)]
    if parsed.scheme() == "http" && matches!(parsed.host_str(), Some("127.0.0.1" | "localhost")) {
        return true;
    }
    parsed.scheme() == "https"
        && matches!(
            parsed.host_str(),
            Some("github.com" | "api.github.com" | "objects.githubusercontent.com")
        )
}

fn http_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent(format!("youtfast/{CURRENT_VERSION}"))
        .build()
        .new_agent()
}

fn fetch_bytes(url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    if !allowed_release_url(url) {
        return Err("Could not download the update. Retry later.".to_owned());
    }
    let agent = http_agent();
    let response = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|_| "Could not check for updates. Retry later.".to_owned())?;
    let mut reader = response
        .into_body()
        .into_reader()
        .take((max_bytes + 1) as u64);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not download the update. Retry later.".to_owned())?;
    if bytes.len() > max_bytes {
        return Err("Could not download the update. Retry later.".to_owned());
    }
    Ok(bytes)
}

/// Checks GitHub Releases for a newer version than `CURRENT_VERSION`.
pub fn check_latest() -> Result<Option<ReleaseInfo>, String> {
    check_latest_from(
        LATEST_RELEASE_API,
        CURRENT_VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

pub fn check_latest_from(
    api_url: &str,
    current_version: &str,
    os: &str,
    arch: &str,
) -> Result<Option<ReleaseInfo>, String> {
    let bytes = fetch_bytes(api_url, MAX_API_BYTES)
        .map_err(|_| "Could not check for updates. Retry later.".to_owned())?;
    let text = String::from_utf8(bytes)
        .map_err(|_| "Could not check for updates. Retry later.".to_owned())?;
    parse_release_json(&text, current_version, os, arch)
}

pub fn parse_release_json(
    json: &str,
    current_version: &str,
    os: &str,
    arch: &str,
) -> Result<Option<ReleaseInfo>, String> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|_| "Could not check for updates. Retry later.".to_owned())?;
    if value["draft"].as_bool() == Some(true) || value["prerelease"].as_bool() == Some(true) {
        return Ok(None);
    }
    let tag = value["tag_name"]
        .as_str()
        .ok_or_else(|| "Could not check for updates. Retry later.".to_owned())?;
    let current = Version::parse(current_version)
        .ok_or_else(|| "Could not check for updates. Retry later.".to_owned())?;
    let Some(candidate) = Version::parse(tag) else {
        return Ok(None);
    };
    let Some(kind) = Version::classify_update(current, candidate) else {
        return Ok(None);
    };
    let version = format!(
        "{}.{}.{}",
        candidate.major, candidate.minor, candidate.patch
    );
    let body = value["body"].as_str().unwrap_or_default();
    let mut changelog = clean_changelog(body);
    if changelog.is_empty() {
        changelog = bundled_changelog_for(&version);
    }
    let html_url = value["html_url"]
        .as_str()
        .filter(|url| allowed_release_url(url))
        .unwrap_or(RELEASES_URL)
        .to_owned();
    let expected_asset = platform_asset_name(&version, os, arch);
    let mut asset_url = None;
    let mut checksums_url = None;
    if let Some(assets) = value["assets"].as_array() {
        for asset in assets {
            let Some(name) = asset["name"].as_str() else {
                continue;
            };
            let Some(download_url) = asset["browser_download_url"]
                .as_str()
                .filter(|url| allowed_release_url(url))
            else {
                continue;
            };
            if expected_asset.as_deref() == Some(name) {
                asset_url = Some(download_url.to_owned());
            } else if name == "checksums.txt" {
                checksums_url = Some(download_url.to_owned());
            }
        }
    }
    Ok(Some(ReleaseInfo {
        version,
        kind,
        changelog,
        html_url,
        asset_name: expected_asset,
        asset_url,
        checksums_url,
    }))
}

pub fn parse_checksum_for(checksums_text: &str, asset_name: &str) -> Option<String> {
    for line in checksums_text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(digest), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        let clean_name = name.strip_prefix('*').unwrap_or(name);
        if clean_name == asset_name
            && digest.len() == 64
            && digest.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Some(digest.to_ascii_lowercase());
        }
    }
    None
}

pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> bool {
    let actual: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    actual.eq_ignore_ascii_case(expected_hex.trim())
}

/// Downloads, verifies SHA-256 against `checksums.txt`, and installs `release`
/// over the current executable.
pub fn install_release(release: &ReleaseInfo, cache_dir: &Path) -> Result<(), String> {
    let target_exe = std::env::current_exe()
        .map_err(|_| "Could not install the update on this system.".to_owned())?;
    install_release_to(release, &target_exe, cache_dir)
}

pub fn install_release_to(
    release: &ReleaseInfo,
    target_exe: &Path,
    cache_dir: &Path,
) -> Result<(), String> {
    let (Some(asset_name), Some(asset_url), Some(checksums_url)) = (
        release.asset_name.as_deref(),
        release.asset_url.as_deref(),
        release.checksums_url.as_deref(),
    ) else {
        return Err("Automatic installation is not supported for this package format.".to_owned());
    };
    let checksums_bytes = fetch_bytes(checksums_url, MAX_CHECKSUMS_BYTES)
        .map_err(|_| "Could not download the update. Retry later.".to_owned())?;
    let checksums_text = String::from_utf8(checksums_bytes)
        .map_err(|_| "The downloaded update failed integrity verification.".to_owned())?;
    let expected_sha = parse_checksum_for(&checksums_text, asset_name)
        .ok_or_else(|| "The downloaded update failed integrity verification.".to_owned())?;
    let archive_bytes = fetch_bytes(asset_url, MAX_ARCHIVE_BYTES)
        .map_err(|_| "Could not download the update. Retry later.".to_owned())?;
    if !verify_sha256(&archive_bytes, &expected_sha) {
        return Err("The downloaded update failed integrity verification.".to_owned());
    }
    install_archive(
        &archive_bytes,
        asset_name,
        &release.version,
        target_exe,
        cache_dir,
    )
}

pub fn install_archive(
    archive_bytes: &[u8],
    asset_name: &str,
    version: &str,
    target_exe: &Path,
    staging_root: &Path,
) -> Result<(), String> {
    let staging = staging_root.join(format!(
        "update-{version}-{}-{}",
        std::process::id(),
        crate::util::now()
    ));
    std::fs::create_dir_all(&staging)
        .map_err(|_| "Could not install the update on this system.".to_owned())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o700));
    }
    let result = install_in_staging(archive_bytes, asset_name, target_exe, &staging);
    let _ = std::fs::remove_dir_all(&staging);
    result
}

fn install_in_staging(
    archive_bytes: &[u8],
    asset_name: &str,
    target_exe: &Path,
    staging: &Path,
) -> Result<(), String> {
    let archive_path = staging.join(asset_name);
    std::fs::write(&archive_path, archive_bytes)
        .map_err(|_| "Could not install the update on this system.".to_owned())?;

    if asset_name.ends_with(".tar.gz") {
        let status = std::process::Command::new("tar")
            .args([
                "-xzf",
                &archive_path.to_string_lossy(),
                "-C",
                &staging.to_string_lossy(),
            ])
            .status()
            .map_err(|_| "Could not install the update on this system.".to_owned())?;
        if !status.success() {
            return Err("Could not install the update on this system.".to_owned());
        }
        let stem = asset_name.trim_end_matches(".tar.gz");
        let extracted_bin = staging.join(stem).join("youtfast");
        if !extracted_bin.is_file() {
            return Err("Could not install the update on this system.".to_owned());
        }
        replace_unix_binary(&extracted_bin, target_exe)
    } else if asset_name.ends_with(".zip") {
        extract_zip(&archive_path, staging)?;
        let stem = asset_name.trim_end_matches(".zip");
        let bundle_root = staging.join(stem);
        let macos_app = bundle_root.join("YouTFast.app");
        let macos_bin = macos_app.join("Contents/MacOS/youtfast");
        let windows_bin = bundle_root.join("youtfast.exe");
        if macos_bin.is_file() {
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("/usr/bin/xattr")
                    .args(["-dr", "com.apple.quarantine", &macos_app.to_string_lossy()])
                    .status();
            }
            replace_macos_or_unix_bundle(&macos_app, &macos_bin, target_exe)
        } else if windows_bin.is_file() {
            replace_windows_bundle(&bundle_root, target_exe)
        } else {
            Err("Could not install the update on this system.".to_owned())
        }
    } else {
        Err("Automatic installation is not supported for this package format.".to_owned())
    }
}

fn extract_zip(archive_path: &Path, staging: &Path) -> Result<(), String> {
    #[cfg(windows)]
    let status = std::process::Command::new("tar.exe")
        .args([
            "-xf",
            &archive_path.to_string_lossy(),
            "-C",
            &staging.to_string_lossy(),
        ])
        .status();
    #[cfg(not(windows))]
    let status = std::process::Command::new("unzip")
        .args([
            "-q",
            &archive_path.to_string_lossy(),
            "-d",
            &staging.to_string_lossy(),
        ])
        .status();
    match status {
        Ok(status) if status.success() => Ok(()),
        _ => Err("Could not install the update on this system.".to_owned()),
    }
}

/// Strips the `" (deleted)"` suffix Linux's `/proc/self/exe` appends when the
/// running binary has already been replaced once during the current session.
fn clean_exe_path(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    match raw.strip_suffix(" (deleted)") {
        Some(clean) => PathBuf::from(clean),
        None => path.to_path_buf(),
    }
}

fn replace_macos_or_unix_bundle(
    new_app: &Path,
    new_bin: &Path,
    target_exe: &Path,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(new_bin, std::fs::Permissions::from_mode(0o755))
            .map_err(|_| "Could not install the update on this system.".to_owned())?;
    }
    let resolved = clean_exe_path(
        &std::fs::canonicalize(target_exe).unwrap_or_else(|_| target_exe.to_path_buf()),
    );
    // When running from inside a macOS `.app` bundle (`.../YouTFast.app/Contents/MacOS/youtfast`),
    // swap the whole `.app` bundle so `Contents/Info.plist`, `Contents/Frameworks`
    // and the bundle signature remain coherent.
    if let Some(existing_app) = resolved
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|dir| dir.extension().and_then(|e| e.to_str()) == Some("app"))
        && let Some(apps_dir) = existing_app.parent()
    {
        static APP_SWAPS: AtomicU64 = AtomicU64::new(0);
        let suffix = format!(
            "{}-{}",
            std::process::id(),
            APP_SWAPS.fetch_add(1, Ordering::Relaxed)
        );
        let staged_app = apps_dir.join(format!(".YouTFast.app.new-{suffix}"));
        let backup_app = apps_dir.join(format!(".YouTFast.app.old-{suffix}"));
        let _ = std::fs::remove_dir_all(&staged_app);
        let _ = std::fs::remove_dir_all(&backup_app);
        let copied = std::process::Command::new("cp")
            .args([
                "-R",
                &new_app.to_string_lossy(),
                &staged_app.to_string_lossy(),
            ])
            .status()
            .is_ok_and(|s| s.success());
        if copied {
            if std::fs::rename(existing_app, &backup_app).is_ok() {
                if std::fs::rename(&staged_app, existing_app).is_ok() {
                    let _ = std::fs::remove_dir_all(&backup_app);
                    return Ok(());
                }
                let _ = std::fs::rename(&backup_app, existing_app);
            }
            let _ = std::fs::remove_dir_all(&staged_app);
        }
    }
    replace_unix_binary(new_bin, &resolved)
}

fn replace_unix_binary(new_bin: &Path, target_exe: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(new_bin, std::fs::Permissions::from_mode(0o755))
            .map_err(|_| "Could not install the update on this system.".to_owned())?;
    }
    let resolved = clean_exe_path(
        &std::fs::canonicalize(target_exe).unwrap_or_else(|_| target_exe.to_path_buf()),
    );
    if atomic_replace_file(new_bin, &resolved).is_ok() {
        return Ok(());
    }
    // Fallback to ~/.local/bin/youtfast when the running binary is in a
    // read-only system directory (such as /usr/bin or /app/bin).
    if let Some(base) = directories::BaseDirs::new() {
        let user_bin_dir = base.home_dir().join(".local/bin");
        if std::fs::create_dir_all(&user_bin_dir).is_ok() {
            let user_bin = user_bin_dir.join("youtfast");
            if atomic_replace_file(new_bin, &user_bin).is_ok() {
                return Ok(());
            }
        }
    }
    Err("Could not install the update on this system.".to_owned())
}

fn atomic_replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(destination) {
        Ok(metadata) if !metadata.is_file() => {
            return Err(std::io::Error::other(
                "refusing to replace a non-regular file",
            ));
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    static UPDATES: AtomicU64 = AtomicU64::new(0);
    let parent = destination
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent directory"))?;
    let tmp = parent.join(format!(
        ".youtfast-update-{}-{}.tmp",
        std::process::id(),
        UPDATES.fetch_add(1, Ordering::Relaxed)
    ));
    let bytes = std::fs::read(source)?;
    let written = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o755);
        let mut file = options.open(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, destination)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

fn replace_windows_bundle(bundle_root: &Path, target_exe: &Path) -> Result<(), String> {
    static WIN_UPDATES: AtomicU64 = AtomicU64::new(0);
    let target_dir = target_exe
        .parent()
        .ok_or_else(|| "Could not install the update on this system.".to_owned())?;
    let old_exe = target_exe.with_extension(format!(
        "exe.old-{}-{}",
        std::process::id(),
        WIN_UPDATES.fetch_add(1, Ordering::Relaxed)
    ));
    if target_exe.exists() {
        std::fs::rename(target_exe, &old_exe)
            .map_err(|_| "Could not install the update on this system.".to_owned())?;
    }
    let new_exe = bundle_root.join("youtfast.exe");
    if std::fs::copy(&new_exe, target_exe).is_err() {
        let _ = std::fs::rename(&old_exe, target_exe);
        return Err("Could not install the update on this system.".to_owned());
    }
    if let Ok(entries) = std::fs::read_dir(bundle_root) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("dll")
                && let Some(name) = path.file_name()
            {
                let _ = std::fs::copy(&path, target_dir.join(name));
            }
        }
    }
    // yt-dlp and Deno ship in the ZIP too, and a newer release carries newer
    // ones. They are best effort: a copy that is in use or missing from an
    // older ZIP leaves the existing one, and playback falls back to PATH.
    for tool in ["yt-dlp.exe", "deno.exe"] {
        let from = bundle_root.join(tool);
        if from.is_file() {
            let _ = std::fs::copy(&from, target_dir.join(tool));
        }
    }
    let _ = std::fs::remove_file(&old_exe);
    Ok(())
}

/// Summary and body for the desktop notification when an update is found or
/// automatically installed.
pub fn notify_update_text(
    version: &str,
    kind: UpdateKind,
    auto_installed: bool,
    language: Locale,
) -> (String, String) {
    if auto_installed {
        (
            trf(
                language,
                "YouTFast updated to v{version}",
                &[("version", version.to_owned())],
            ),
            tr(
                language,
                "A minor or patch update was installed automatically. See what changed in Settings → About.",
            )
            .into_owned(),
        )
    } else if kind.is_major() {
        (
            trf(
                language,
                "New major version: YouTFast v{version}",
                &[("version", version.to_owned())],
            ),
            tr(
                language,
                "Major updates require manual confirmation. Open Settings → About to review the changelog and install.",
            )
            .into_owned(),
        )
    } else {
        (
            trf(
                language,
                "YouTFast v{version} is available",
                &[("version", version.to_owned())],
            ),
            tr(
                language,
                "A new version is ready to install. Open Settings → About to see what changed and update.",
            )
            .into_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parses_versions_and_classifies_patch_minor_and_major_updates() {
        let base = Version::parse("v0.2.0").unwrap();
        assert_eq!(
            base,
            Version {
                major: 0,
                minor: 2,
                patch: 0
            }
        );
        assert_eq!(
            Version::classify_update(base, Version::parse("0.1.9").unwrap()),
            None
        );
        assert_eq!(
            Version::classify_update(base, Version::parse("0.2.0").unwrap()),
            None
        );
        assert_eq!(
            Version::classify_update(base, Version::parse("v0.2.1").unwrap()),
            Some(UpdateKind::Patch)
        );
        assert_eq!(
            Version::classify_update(base, Version::parse("v0.3.0").unwrap()),
            Some(UpdateKind::Minor)
        );
        assert_eq!(
            Version::classify_update(base, Version::parse("v1.0.0").unwrap()),
            Some(UpdateKind::Major)
        );
    }

    #[test]
    fn auto_update_applies_only_to_minor_and_patch_never_major() {
        assert!(!should_auto_update(false, UpdateKind::Patch));
        assert!(!should_auto_update(false, UpdateKind::Minor));
        assert!(!should_auto_update(false, UpdateKind::Major));
        assert!(should_auto_update(true, UpdateKind::Patch));
        assert!(should_auto_update(true, UpdateKind::Minor));
        assert!(!should_auto_update(true, UpdateKind::Major));
    }

    #[test]
    fn parses_release_json_and_extracts_changelog_and_assets() {
        let json = r####"{
            "tag_name": "v0.3.0",
            "draft": false,
            "html_url": "https://github.com/bcosta19/youtfast/releases/tag/v0.3.0",
            "body": "### Added\n- Automatic minor and patch updates with changelog display.\n\n### Fixed\n- Faster startup.",
            "assets": [
                {
                    "name": "youtfast-v0.3.0-x86_64-unknown-linux-gnu.tar.gz",
                    "browser_download_url": "https://github.com/bcosta19/youtfast/releases/download/v0.3.0/youtfast-v0.3.0-x86_64-unknown-linux-gnu.tar.gz"
                },
                {
                    "name": "checksums.txt",
                    "browser_download_url": "https://github.com/bcosta19/youtfast/releases/download/v0.3.0/checksums.txt"
                }
            ]
        }"####;
        let info = parse_release_json(json, "0.2.0", "linux", "x86_64")
            .unwrap()
            .unwrap();
        assert_eq!(info.version, "0.3.0");
        assert_eq!(info.kind, UpdateKind::Minor);
        assert!(info.changelog.contains("Added"));
        assert!(
            info.changelog
                .contains("• Automatic minor and patch updates with changelog display.")
        );
        assert_eq!(
            info.asset_name.as_deref(),
            Some("youtfast-v0.3.0-x86_64-unknown-linux-gnu.tar.gz")
        );
        assert!(info.asset_url.is_some());
        assert!(info.checksums_url.is_some());

        // Same or older version returns None.
        assert!(
            parse_release_json(json, "0.3.0", "linux", "x86_64")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn bundled_changelog_extracts_current_version_notes() {
        let notes = bundled_changelog_for(CURRENT_VERSION);
        assert!(notes.contains("Added"));
        assert!(notes.contains("About section and built-in updater"));
        let notes_020 = bundled_changelog_for("0.2.0");
        assert!(notes_020.contains("Spanish, French, German and Italian"));
    }

    #[test]
    fn checksum_verification_accepts_valid_digest_and_rejects_tampered_bytes() {
        let payload = b"synthetic-youtfast-binary-payload";
        let expected: String = Sha256::digest(payload)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let checksums = format!(
            "{expected}  youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz\n\
             0000000000000000000000000000000000000000000000000000000000000000  other.zip\n"
        );
        let parsed = parse_checksum_for(
            &checksums,
            "youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz",
        )
        .unwrap();
        assert_eq!(parsed, expected);
        assert!(verify_sha256(payload, &parsed));
        assert!(!verify_sha256(b"tampered-payload", &parsed));
    }

    #[cfg(unix)]
    #[test]
    fn install_archive_replaces_target_binary_atomically() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("updater-install-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let pkg_dir = root.join("youtfast-v0.2.1-x86_64-unknown-linux-gnu");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(
            pkg_dir.join("youtfast"),
            b"#!/bin/sh\necho youtfast 0.2.1\n",
        )
        .unwrap();

        let archive_path = root.join("youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz");
        let status = std::process::Command::new("tar")
            .args([
                "-czf",
                &archive_path.to_string_lossy(),
                "-C",
                &root.to_string_lossy(),
                "youtfast-v0.2.1-x86_64-unknown-linux-gnu",
            ])
            .status()
            .unwrap();
        assert!(status.success());

        let target_exe = root.join("installed-youtfast");
        std::fs::write(&target_exe, b"#!/bin/sh\necho youtfast 0.2.0\n").unwrap();
        let archive_bytes = std::fs::read(&archive_path).unwrap();

        install_archive(
            &archive_bytes,
            "youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz",
            "0.2.1",
            &target_exe,
            &root,
        )
        .unwrap();

        let updated = std::fs::read_to_string(&target_exe).unwrap();
        assert_eq!(updated, "#!/bin/sh\necho youtfast 0.2.1\n");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn checksum_parser_skips_blank_and_comment_lines_and_prereleases_are_ignored() {
        let checksums = "\n# Release checksums\n\n\
            aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  other.tar.gz\n\
            bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb *youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz\n";
        assert_eq!(
            parse_checksum_for(checksums, "youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz")
                .as_deref(),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );

        assert!(Version::parse("v0.3.0-rc.1").is_none());
        let prerelease_json = r####"{
            "tag_name": "v0.3.0",
            "draft": false,
            "prerelease": true,
            "html_url": "https://github.com/bcosta19/youtfast/releases/tag/v0.3.0",
            "body": "### Added\n- RC build",
            "assets": []
        }"####;
        assert!(
            parse_release_json(prerelease_json, "0.2.0", "linux", "x86_64")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn strip_markdown_links_preserves_bracketed_ui_labels_and_clean_exe_path_strips_deleted() {
        let line = "- Added `[ save ]` button in [watch page](https://example.com/watch) and `[ load more ]`.";
        let cleaned = strip_markdown_links(line);
        assert_eq!(
            cleaned,
            "- Added [ save ] button in watch page and [ load more ]."
        );
        assert_eq!(
            clean_exe_path(Path::new("/usr/local/bin/youtfast (deleted)")),
            PathBuf::from("/usr/local/bin/youtfast")
        );
    }
}
