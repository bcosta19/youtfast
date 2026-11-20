//! GitHub Releases API checks, validated HTTP downloads, and SHA-256 verification.

use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::changelog::{bundled_changelog_for, clean_changelog};
use super::detect::{InstallTarget, detect_install_target};
use super::{
    CURRENT_VERSION, ERROR_CHECK, ERROR_DOWNLOAD, ERROR_INTEGRITY, InstallMethod, RELEASES_URL,
    ReleaseInfo, Version,
};

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/bcosta19/youtfast/releases/latest";

const MAX_API_BYTES: usize = 256 * 1024;
pub(super) const MAX_CHECKSUMS_BYTES: usize = 64 * 1024;
pub(super) const MAX_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
const MAX_HTTP_ATTEMPTS: usize = 3;

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
            Some(
                "github.com"
                    | "api.github.com"
                    | "objects.githubusercontent.com"
                    | "release-assets.githubusercontent.com"
            )
        )
}

fn metadata_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(20)))
        .timeout_per_read(Some(Duration::from_secs(20)))
        .timeout_global(Some(Duration::from_secs(45)))
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent(format!("youtfast/{CURRENT_VERSION}"))
        .build()
        .new_agent()
}

pub(super) fn download_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_per_read(Some(Duration::from_secs(30)))
        .timeout_global(Some(Duration::from_secs(900)))
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent(format!("youtfast/{CURRENT_VERSION}"))
        .build()
        .new_agent()
}

#[derive(Debug)]
enum FetchError {
    Retryable(Option<Duration>),
    Fatal,
    TooLarge,
}

fn retry_after_delay(response: &ureq::http::Response<ureq::Body>) -> Option<Duration> {
    let secs = response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())?;
    Some(Duration::from_secs(secs.clamp(1, 15)))
}

fn open_validated_response(
    agent: &ureq::Agent,
    initial_url: &str,
) -> Result<ureq::http::Response<ureq::Body>, FetchError> {
    let mut current_url = initial_url.to_owned();
    for _ in 0..=MAX_REDIRECTS {
        if !allowed_release_url(&current_url) {
            return Err(FetchError::Fatal);
        }
        let parsed = url::Url::parse(&current_url).map_err(|_| FetchError::Fatal)?;
        let mut request = agent.get(&current_url);
        if parsed.host_str() == Some("api.github.com") {
            request = request.header("Accept", "application/vnd.github+json");
        }
        let response = match request.call() {
            Ok(response) => response,
            Err(error) => match error {
                ureq::Error::Io(_)
                | ureq::Error::Timeout(_)
                | ureq::Error::HostNotFound
                | ureq::Error::ConnectionFailed => return Err(FetchError::Retryable(None)),
                _ => return Err(FetchError::Fatal),
            },
        };
        let status = response.status().as_u16();
        if (301..=308).contains(&status) && status != 304 {
            let Some(location) = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
            else {
                return Err(FetchError::Fatal);
            };
            let next = parsed.join(location).map_err(|_| FetchError::Fatal)?;
            if !allowed_release_url(next.as_str()) {
                return Err(FetchError::Fatal);
            }
            current_url = next.to_string();
            continue;
        }
        if (200..300).contains(&status) {
            return Ok(response);
        }
        if status == 429 || (500..=599).contains(&status) {
            return Err(FetchError::Retryable(retry_after_delay(&response)));
        }
        return Err(FetchError::Fatal);
    }
    Err(FetchError::Fatal)
}

fn retry_fetch<T>(
    attempts: usize,
    mut sleep: impl FnMut(Duration),
    mut op: impl FnMut() -> Result<T, FetchError>,
) -> Result<T, FetchError> {
    for attempt in 0..attempts {
        match op() {
            Ok(value) => return Ok(value),
            Err(FetchError::Retryable(delay)) if attempt + 1 < attempts => {
                let backoff = delay.unwrap_or_else(|| Duration::from_secs(1 << attempt));
                sleep(backoff);
            }
            Err(error) => return Err(error),
        }
    }
    Err(FetchError::Fatal)
}

fn fetch_bytes_with<S: FnMut(Duration)>(
    agent: &ureq::Agent,
    url: &str,
    max_bytes: usize,
    sleep: S,
) -> Result<Vec<u8>, FetchError> {
    retry_fetch(MAX_HTTP_ATTEMPTS, sleep, || {
        let response = open_validated_response(agent, url)?;
        if let Some(len) = content_length(&response)
            && len > max_bytes as u64
        {
            return Err(FetchError::TooLarge);
        }
        let mut reader = response
            .into_body()
            .into_reader()
            .take((max_bytes + 1) as u64);
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(|_| FetchError::Retryable(None))?;
        if bytes.len() > max_bytes {
            return Err(FetchError::TooLarge);
        }
        Ok(bytes)
    })
}

fn content_length(response: &ureq::http::Response<ureq::Body>) -> Option<u64> {
    response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
}

pub(super) fn fetch_bytes(url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    fetch_bytes_with(&metadata_agent(), url, max_bytes, std::thread::sleep)
        .map_err(|_| ERROR_DOWNLOAD.to_owned())
}

pub(super) fn download_verified_file_with<S: FnMut(Duration)>(
    agent: &ureq::Agent,
    url: &str,
    dest: &Path,
    expected_sha256: &str,
    max_bytes: usize,
    sleep: S,
) -> Result<(), String> {
    let outcome = retry_fetch(MAX_HTTP_ATTEMPTS, sleep, || {
        let response = open_validated_response(agent, url)?;
        let expected_len = content_length(&response);
        if expected_len.is_some_and(|len| len > max_bytes as u64) {
            return Err(FetchError::TooLarge);
        }
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(dest)
            .map_err(|_| FetchError::Fatal)?;
        let mut writer = BufWriter::new(file);
        let mut hasher = Sha256::new();
        let mut reader = response.into_body().into_reader();
        let mut buf = [0u8; 64 * 1024];
        let mut total = 0usize;
        loop {
            let read = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => {
                    let _ = std::fs::remove_file(dest);
                    return Err(FetchError::Retryable(None));
                }
            };
            total = total.saturating_add(read);
            if total > max_bytes {
                drop(writer);
                let _ = std::fs::remove_file(dest);
                return Err(FetchError::TooLarge);
            }
            hasher.update(&buf[..read]);
            if writer.write_all(&buf[..read]).is_err() {
                drop(writer);
                let _ = std::fs::remove_file(dest);
                return Err(FetchError::Fatal);
            }
        }
        if writer.flush().is_err() {
            let _ = std::fs::remove_file(dest);
            return Err(FetchError::Fatal);
        }
        drop(writer);
        if expected_len.is_some_and(|len| total as u64 != len) {
            let _ = std::fs::remove_file(dest);
            return Err(FetchError::Retryable(None));
        }
        let actual: String = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if !actual.eq_ignore_ascii_case(expected_sha256.trim()) {
            let _ = std::fs::remove_file(dest);
            return Err(FetchError::TooLarge);
        }
        Ok(())
    });
    match outcome {
        Ok(()) => Ok(()),
        Err(FetchError::TooLarge) => Err(ERROR_INTEGRITY.to_owned()),
        Err(FetchError::Retryable(_) | FetchError::Fatal) => Err(ERROR_DOWNLOAD.to_owned()),
    }
}

/// Checks GitHub Releases for a newer version than `CURRENT_VERSION`.
pub fn check_latest() -> Result<Option<ReleaseInfo>, String> {
    let target = detect_install_target();
    check_latest_for(LATEST_RELEASE_API, CURRENT_VERSION, target)
}

fn default_target_for_platform(os: &str, arch: &str) -> InstallTarget {
    let method = match (os, arch) {
        ("linux", "x86_64") => InstallMethod::LinuxNative,
        ("macos", "aarch64") => InstallMethod::MacosBundle,
        ("windows", "x86_64") => InstallMethod::WindowsPortable,
        _ => InstallMethod::ManualOnly,
    };
    InstallTarget {
        method,
        can_install: method.supports_in_app_install(),
    }
}

pub fn check_latest_from(
    api_url: &str,
    current_version: &str,
    os: &str,
    arch: &str,
) -> Result<Option<ReleaseInfo>, String> {
    check_latest_for(
        api_url,
        current_version,
        default_target_for_platform(os, arch),
    )
}

pub fn check_latest_for(
    api_url: &str,
    current_version: &str,
    target: InstallTarget,
) -> Result<Option<ReleaseInfo>, String> {
    let bytes = fetch_bytes(api_url, MAX_API_BYTES).map_err(|_| ERROR_CHECK.to_owned())?;
    let text = String::from_utf8(bytes).map_err(|_| ERROR_CHECK.to_owned())?;
    parse_release_json_for(&text, current_version, target)
}

pub fn parse_release_json(
    json: &str,
    current_version: &str,
    os: &str,
    arch: &str,
) -> Result<Option<ReleaseInfo>, String> {
    parse_release_json_for(json, current_version, default_target_for_platform(os, arch))
}

pub fn parse_release_json_for(
    json: &str,
    current_version: &str,
    target: InstallTarget,
) -> Result<Option<ReleaseInfo>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| ERROR_CHECK.to_owned())?;
    if value["draft"].as_bool() == Some(true) || value["prerelease"].as_bool() == Some(true) {
        return Ok(None);
    }
    let tag = value["tag_name"]
        .as_str()
        .ok_or_else(|| ERROR_CHECK.to_owned())?;
    let current = Version::parse(current_version).ok_or_else(|| ERROR_CHECK.to_owned())?;
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
    let expected_asset = target.method.asset_name(&version);
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
        method: target.method,
        install_supported: target.can_install,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::{UpdateKind, should_auto_install, test_root};
    use std::io::{Read, Write};
    use std::net::TcpListener;

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
                    "name": "youtfast-0.3.0-1-x86_64.pkg.tar.zst",
                    "browser_download_url": "https://github.com/bcosta19/youtfast/releases/download/v0.3.0/youtfast-0.3.0-1-x86_64.pkg.tar.zst"
                },
                {
                    "name": "youtfast-v0.3.0-x86_64-windows-setup.exe",
                    "browser_download_url": "https://github.com/bcosta19/youtfast/releases/download/v0.3.0/youtfast-v0.3.0-x86_64-windows-setup.exe"
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
        assert_eq!(info.method, InstallMethod::LinuxNative);
        assert!(info.can_self_update());
        assert!(info.can_auto_update());
        assert!(info.changelog.contains("Added"));
        assert!(
            info.changelog
                .contains("• Automatic minor and patch updates with changelog display.")
        );
        assert_eq!(
            info.asset_name.as_deref(),
            Some("youtfast-v0.3.0-x86_64-unknown-linux-gnu.tar.gz")
        );

        let arch = parse_release_json_for(
            json,
            "0.2.0",
            InstallTarget {
                method: InstallMethod::ArchPacman,
                can_install: true,
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            arch.asset_name.as_deref(),
            Some("youtfast-0.3.0-1-x86_64.pkg.tar.zst")
        );
        assert!(arch.can_self_update());
        assert!(!arch.can_auto_update());
        assert!(!should_auto_install(true, &arch));

        let flatpak = parse_release_json_for(
            json,
            "0.2.0",
            InstallTarget {
                method: InstallMethod::Flatpak,
                can_install: false,
            },
        )
        .unwrap()
        .unwrap();
        assert!(!flatpak.can_self_update());
        assert!(!flatpak.can_auto_update());

        // Same or older version returns None.
        assert!(
            parse_release_json(json, "0.3.0", "linux", "x86_64")
                .unwrap()
                .is_none()
        );
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

    #[test]
    fn allowed_urls_accept_github_cdn_and_reject_untrusted_hosts() {
        assert!(allowed_release_url(
            "https://api.github.com/repos/bcosta19/youtfast/releases/latest"
        ));
        assert!(allowed_release_url(
            "https://objects.githubusercontent.com/github-production-release-asset/1"
        ));
        assert!(allowed_release_url(
            "https://release-assets.githubusercontent.com/github-production-release-asset/1"
        ));
        assert!(!allowed_release_url(
            "http://github.com/bcosta19/youtfast/releases"
        ));
        assert!(!allowed_release_url("https://evil.example/payload.tar.gz"));
    }

    #[test]
    fn download_retries_transient_503_and_follows_validated_redirect() {
        let root = test_root("retry");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let payload = b"verified-release-archive-bytes";
        let expected_sha: String = Sha256::digest(payload)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            for step in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut req = [0u8; 1024];
                let _ = stream.read(&mut req);
                match step {
                    0 => {
                        let _ = stream.write_all(
                            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                    }
                    1 => {
                        let header = format!(
                            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        );
                        let _ = stream.write_all(header.as_bytes());
                    }
                    _ => {
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            payload.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(payload);
                    }
                }
            }
        });

        let dest = root.join("download.bin");
        download_verified_file_with(
            &download_agent(),
            &format!("http://127.0.0.1:{port}/start"),
            &dest,
            &expected_sha,
            1024,
            |_| {},
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), payload);

        // Redirect to an untrusted host must be rejected.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let bad_server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut req = [0u8; 512];
            let _ = stream.read(&mut req);
            let _ = stream.write_all(
                b"HTTP/1.1 302 Found\r\nLocation: https://evil.example/asset\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        });
        let err = download_verified_file_with(
            &download_agent(),
            &format!("http://127.0.0.1:{port}/start"),
            &dest,
            &expected_sha,
            1024,
            |_| {},
        )
        .unwrap_err();
        bad_server.join().unwrap();
        assert_eq!(err, ERROR_DOWNLOAD);

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
}
