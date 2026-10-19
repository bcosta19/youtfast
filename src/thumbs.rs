//! Thumbnails kept on disk (`~/.cache/youtfast/thumbnails`): a URL fetched
//! once answers from a file from then on, so coming back to a page does
//! not re-fetch every image it draws. The files are cache, never state:
//! losing them only costs a fetch.

use std::path::{Path, PathBuf};

/// How many thumbnails stay before the oldest are pruned at startup.
const KEEP: usize = 2000;
const MAX_BYTES: u64 = 4 * 1024 * 1024;
const DISK_BYTES: u64 = 128 * 1024 * 1024;
/// How many thumbnails download at once; each keeps its connection open
/// for the next one (see [`agent`]).
pub const DOWNLOADS: usize = 4;
static DISK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Where `url`'s bytes belong: a file named by the URL's hash.
pub fn path(dir: &Path, url: &str) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(url, &mut hash);
    dir.join(format!("{:x}", std::hash::Hasher::finish(&hash)))
}

/// Controlled failures never retain a URL, response body or dependency error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Network,
    Access,
    Unavailable,
    RateLimited(u64),
    Server,
    TooLarge,
    Invalid,
}
impl Failure {
    pub fn code(self) -> &'static str {
        match self {
            Self::Network => "image: download failed",
            Self::Access => "image: HTTP access denied",
            Self::Unavailable => "image: unavailable",
            Self::RateLimited(_) => "image: rate limited",
            Self::Server => "image: server error",
            Self::TooLarge => "image: response too large",
            Self::Invalid => "image: invalid data",
        }
    }
    pub fn retry_delay(self, attempt: u32) -> Option<f64> {
        if attempt >= 3 {
            return None;
        }
        match self {
            Self::Network | Self::Server => Some(2.0_f64.powi(attempt as i32) * 5.0),
            Self::RateLimited(seconds) => Some(seconds.max(30) as f64),
            _ => None,
        }
    }
}

/// The kept copy at `path`, when it is a regular file that passes the same
/// checks as a download. Reading one needs no download turn.
pub fn cached(path: &Path) -> Option<std::sync::Arc<[u8]>> {
    use std::io::Read;
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    validate(&bytes).ok()?;
    Some(bytes.into())
}

/// A cache hit and a network response follow the same bounded validation path.
/// A corrupt regular cache entry is repaired once; no other files are touched.
pub fn load_or_fetch(
    url: &str,
    path: &Path,
    refresh: bool,
) -> Result<std::sync::Arc<[u8]>, Failure> {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if !meta.is_file() {
            return fetch(url, path);
        }
        if !refresh && let Some(bytes) = cached(path) {
            return Ok(bytes);
        }
        let _disk = DISK.lock().unwrap();
        let _ = std::fs::remove_file(path);
        log::debug!("image: cache repaired");
    }
    fetch(url, path)
}

pub fn fetch(url: &str, path: &Path) -> Result<std::sync::Arc<[u8]>, Failure> {
    let (bytes, store) = download(url)?;
    let _disk = DISK.lock().unwrap();
    if store
        && crate::paths::write_atomic(path, &bytes).is_ok()
        && let Some(dir) = path.parent()
    {
        prune_locked(dir, KEEP, DISK_BYTES);
    }
    Ok(bytes.into())
}

fn retry_after(value: Option<&str>) -> u64 {
    let Some(value) = value else {
        return 30;
    };
    if let Ok(seconds) = value.trim().parse::<u64>() {
        return seconds;
    }
    // HTTP-date (IMF-fixdate); an invalid header falls back to backoff.
    jiff::civil::DateTime::strptime("%a, %d %b %Y %H:%M:%S GMT", value)
        .ok()
        .and_then(|date| date.to_zoned(jiff::tz::TimeZone::UTC).ok())
        .map(|date| {
            (date.timestamp().as_second() - jiff::Timestamp::now().as_second()).max(0) as u64
        })
        .unwrap_or(30)
}
/// One HTTP client for every thumbnail: its connections stay open between
/// downloads, so a page of thumbnails does not pay a TLS handshake each.
fn agent() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(20)))
            .http_status_as_error(false)
            .max_idle_connections_per_host(DOWNLOADS)
            .build()
            .into()
    })
}

fn download(url: &str) -> Result<(Vec<u8>, bool), Failure> {
    use std::io::Read;
    let mut response = agent().get(url).call().map_err(|_| Failure::Network)?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(match status {
            401 | 403 | 410 => Failure::Access,
            429 => Failure::RateLimited(retry_after(
                response
                    .headers()
                    .get("retry-after")
                    .and_then(|h| h.to_str().ok()),
            )),
            500..=599 => Failure::Server,
            _ => Failure::Unavailable,
        });
    }
    let store = !response
        .headers()
        .get("cache-control")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|header| {
            header
                .split(',')
                .any(|v| v.trim().eq_ignore_ascii_case("no-store"))
        });
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure::Network)?;
    validate(&bytes)?;
    Ok((bytes, store))
}
fn validate(bytes: &[u8]) -> Result<(), Failure> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Failure::TooLarge);
    }
    let (width, height) = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| Failure::Invalid)?
        .into_dimensions()
        .map_err(|_| Failure::Invalid)?;
    if width == 0
        || height == 0
        || width > 4096
        || height > 4096
        || u64::from(width) * u64::from(height) > 4_000_000
    {
        return Err(Failure::TooLarge);
    }
    Ok(())
}

/// Keeps the newest [`KEEP`] files; the rest, oldest first, are deleted.
/// Runs once, off the interface thread, at startup.
pub fn prune(dir: &Path) {
    let _disk = DISK.lock().unwrap();
    prune_locked(dir, KEEP, DISK_BYTES);
}

fn prune_locked(dir: &Path, keep: usize, budget: u64) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<_> = read
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name();
            let name = name.to_str()?;
            if name.is_empty()
                || name.len() > 16
                || !name.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            entry.file_type().ok()?.is_file().then_some((
                entry.path(),
                metadata.modified().ok()?,
                metadata.len(),
            ))
        })
        .collect();
    let mut bytes: u64 = files.iter().map(|(_, _, bytes)| bytes).sum();
    let mut count = files.len();
    if count <= keep && bytes <= budget {
        return;
    }
    files.sort_by_key(|(_, modified, _)| *modified);
    for (path, _, size) in files {
        if count <= keep && bytes <= budget {
            break;
        }
        if std::fs::remove_file(path).is_ok() {
            count -= 1;
            bytes = bytes.saturating_sub(size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(responses: Vec<(u16, Vec<u8>)>) -> (String, std::thread::JoinHandle<usize>) {
        server_headers(responses, "")
    }
    fn server_headers(
        responses: Vec<(u16, Vec<u8>)>,
        headers: &str,
    ) -> (String, std::thread::JoinHandle<usize>) {
        let headers = headers.to_owned();
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/thumbnail", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let mut count = 0;
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let _ = stream.read(&mut [0; 8192]);
                let header = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",
                    body.len()
                );
                let _ = stream
                    .write_all(header.as_bytes())
                    .and_then(|()| stream.write_all(&body));
                count += 1;
            }
            count
        });
        (url, worker)
    }

    fn png() -> Vec<u8> {
        use image::ImageEncoder;
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&[0, 0, 0, 0], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        bytes
    }

    #[test]
    fn an_oversized_response_is_rejected_without_a_partial_file() {
        let (url, worker) = server(vec![(200, vec![0; MAX_BYTES as usize + 100])]);
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("oversized-thumbnail-{}", std::process::id()));
        assert!(fetch(&url, &path).is_err());
        assert!(!path.exists());
        assert_eq!(worker.join().unwrap(), 1);
    }

    #[test]
    fn transient_failures_retry_and_one_download_supplies_display_and_disk() {
        let image = png();
        let (url, worker) = server(vec![(503, Vec::new()), (200, image.clone())]);
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("retry-thumbnail-{}", std::process::id()));
        assert!(fetch(&url, &path).is_err());
        assert!(!path.exists());
        let displayed = fetch(&url, &path).unwrap();
        assert_eq!(displayed.as_ref(), image);
        assert_eq!(std::fs::read(&path).unwrap(), image);
        assert_eq!(worker.join().unwrap(), 2);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn disk_failures_do_not_discard_the_display_bytes() {
        let (url, worker) = server(vec![(200, png())]);
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("thumbnail-write-failure-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        assert!(fetch(&url, &dir).is_ok());
        assert_eq!(worker.join().unwrap(), 1);
        assert!(dir.is_dir());
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn pruning_enforces_both_file_count_and_total_bytes() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("prune-thumbnails-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        for index in 0..4 {
            std::fs::write(dir.join(index.to_string()), [0; 20]).unwrap();
        }
        prune_locked(&dir, 3, 45);
        let files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(files.len(), 2);
        for entry in files {
            std::fs::remove_file(entry.path()).unwrap();
        }
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn a_url_always_gets_the_same_safe_name() {
        let dir = Path::new("cache");
        let name = path(dir, "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg");
        assert_eq!(
            name,
            path(dir, "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg")
        );
        assert_ne!(
            name,
            path(dir, "https://i.ytimg.com/vi/other/hqdefault.jpg")
        );
        // No slash, no query: it is one file name in one directory.
        assert_eq!(name.components().count(), 2);
    }
    #[test]
    fn corrupt_cache_is_repaired_but_valid_cache_needs_no_network() {
        let bytes = png();
        let (url, worker) = server(vec![(200, bytes.clone())]);
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("corrupt-thumbnail-{}", std::process::id()));
        std::fs::write(&path, b"broken").unwrap();
        assert_eq!(load_or_fetch(&url, &path, false).unwrap().as_ref(), bytes);
        assert_eq!(worker.join().unwrap(), 1);
        assert_eq!(
            load_or_fetch("http://127.0.0.1:1/unreachable", &path, false)
                .unwrap()
                .as_ref(),
            bytes
        );
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn http_failures_are_classified_and_retry_after_supports_dates() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("failed-thumbnail-{}", std::process::id()));
        for (code, expected) in [
            (403, Failure::Access),
            (410, Failure::Access),
            (404, Failure::Unavailable),
            (429, Failure::RateLimited(30)),
            (503, Failure::Server),
        ] {
            let (url, worker) = server(vec![(code, Vec::new())]);
            assert_eq!(fetch(&url, &path).err(), Some(expected));
            assert_eq!(worker.join().unwrap(), 1);
            assert!(!path.exists());
        }
        assert_eq!(retry_after(Some("300")), 300);
        let future = jiff::Timestamp::now()
            .checked_add(std::time::Duration::from_secs(600))
            .unwrap()
            .to_zoned(jiff::tz::TimeZone::UTC)
            .strftime("%a, %d %b %Y %H:%M:%S GMT")
            .to_string();
        assert!((598..=600).contains(&retry_after(Some(&future))));
        assert_eq!(retry_after(Some("invalid")), 30);
    }
    #[test]
    fn only_a_regular_file_within_the_limits_that_decodes_is_kept() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("kept-thumbnails-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let image = png();
        std::fs::write(dir.join("valid"), &image).unwrap();
        std::fs::write(dir.join("corrupt"), b"broken").unwrap();
        std::fs::write(dir.join("large"), vec![0; MAX_BYTES as usize + 1]).unwrap();
        std::fs::create_dir(dir.join("folder")).unwrap();
        assert_eq!(
            cached(&dir.join("valid")).as_deref(),
            Some(image.as_slice())
        );
        for name in ["corrupt", "large", "folder", "missing"] {
            assert!(cached(&dir.join(name)).is_none(), "{name}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Answers `bodies` in turn on the first connection it accepts, keeping
    /// it open; returns how many it answered and how many connections came.
    fn keep_alive_server(
        bodies: Vec<Vec<u8>>,
    ) -> (String, std::thread::JoinHandle<(usize, usize)>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut answered = 0;
            for body in bodies {
                let mut head = Vec::new();
                let mut byte = [0; 1];
                while !head.ends_with(b"\r\n\r\n") && matches!(stream.read(&mut byte), Ok(1)) {
                    head.push(byte[0]);
                }
                if !head.ends_with(b"\r\n\r\n") {
                    break;
                }
                let header = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
                stream.write_all(header.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
                answered += 1;
            }
            listener.set_nonblocking(true).unwrap();
            let others = std::iter::from_fn(|| listener.accept().ok()).count();
            (answered, 1 + others)
        });
        (url, worker)
    }

    #[test]
    fn downloads_reuse_their_connection() {
        let image = png();
        let (url, worker) = keep_alive_server(vec![image.clone(), image.clone()]);
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("keep-alive-thumbnails-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        for (page, file) in [("first", "a"), ("second", "b")] {
            let bytes = fetch(&format!("{url}{page}"), &dir.join(file)).unwrap();
            assert_eq!(bytes.as_ref(), image);
        }
        assert_eq!(worker.join().unwrap(), (2, 1));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn response_headers_control_disk_storage_and_retry_delay() {
        let image = png();
        let (url, worker) = server_headers(
            vec![(200, image.clone()), (429, Vec::new())],
            "Cache-Control: no-store\r\nRetry-After: 120\r\n",
        );
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("no-store-thumbnail-{}", std::process::id()));
        assert_eq!(fetch(&url, &path).unwrap().as_ref(), image);
        assert!(!path.exists());
        assert_eq!(fetch(&url, &path).err(), Some(Failure::RateLimited(120)));
        assert_eq!(worker.join().unwrap(), 2);
    }
}
