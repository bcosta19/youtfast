//! Thumbnails kept on disk (`~/.cache/youtfast/thumbnails`): a URL fetched
//! once answers from a file from then on, so coming back to a page does
//! not re-fetch every image it draws. The files are cache, never state:
//! losing them only costs a fetch.

use std::path::{Path, PathBuf};

/// How many thumbnails stay before the oldest are pruned at startup.
const KEEP: usize = 2000;

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
pub fn fetch(url: &str, path: &Path) {
    let Ok(response) = ehttp::fetch_blocking(&ehttp::Request::get(url)) else {
        return;
    };
    if response.status != 200 {
        return;
    }
    let _ = crate::paths::write_atomic(path, &response.bytes);
}

/// Keeps the newest [`KEEP`] files; the rest, oldest first, are deleted.
/// Runs once, off the interface thread, at startup.
pub fn prune(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(PathBuf, std::time::SystemTime)> = read
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let modified = entry.metadata().ok()?.modified().ok()?;
            entry
                .file_type()
                .ok()?
                .is_file()
                .then_some((entry.path(), modified))
        })
        .collect();
    if files.len() <= KEEP {
        return;
    }
    files.sort_by_key(|(_, modified)| *modified);
    let remove = files.len() - KEEP;
    for (path, _) in &files[..remove] {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
