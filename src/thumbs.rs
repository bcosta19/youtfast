//! Thumbnails kept on disk (`~/.cache/youtfast/thumbnails`): a URL fetched
//! once answers from a file from then on, so coming back to a page does
//! not re-fetch every image it draws. The files are cache, never state:
//! losing them only costs a fetch.

use std::path::{Path, PathBuf};

/// How many thumbnails stay before the oldest are pruned at startup.
const KEEP: usize = 2000;
const MAX_BYTES: u64 = 4 * 1024 * 1024;
const DISK_BYTES: u64 = 128 * 1024 * 1024;
static DISK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Where `url`'s bytes belong: a file named by the URL's hash.
pub fn path(dir: &Path, url: &str) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(url, &mut hash);
    dir.join(format!("{:x}", std::hash::Hasher::finish(&hash)))
}

/// `url` as a `file://` URI egui's image loaders read.
pub fn uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

/// Downloads `url` to `path`; failures are quiet: the interface just
/// keeps drawing the image from the network. The write is atomic so a
/// crash mid-download never leaves half a picture to be served whole.
pub fn fetch(url: &str, path: &Path) -> Result<std::sync::Arc<[u8]>, String> {
    let bytes = download(url)?;
    let _disk = DISK.lock().unwrap();
    if crate::paths::write_atomic(path, &bytes).is_ok()
        && let Some(dir) = path.parent()
    {
        prune_locked(dir, KEEP, DISK_BYTES);
    }
    Ok(bytes.into())
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut response = ehttp::Request::get(url)
        .with_timeout(Some(std::time::Duration::from_secs(20)))
        .fetch_raw_native(true)
        .map_err(|_| "thumbnail request failed")?;
    if !response.status().is_success() {
        return Err("thumbnail status failed".into());
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "thumbnail body failed")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("thumbnail is too large".into());
    }
    let (width, height) = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| "thumbnail format failed")?
        .into_dimensions()
        .map_err(|_| "thumbnail dimensions failed")?;
    if width == 0
        || height == 0
        || width > 4096
        || height > 4096
        || u64::from(width) * u64::from(height) > 4_000_000
    {
        return Err("thumbnail dimensions are too large".into());
    }
    Ok(bytes)
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
                    "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
}
