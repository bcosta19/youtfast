//! Remote images drawn through egui's loaders (`remote`), and the ones
//! not drawn for [`KEEP`] seconds forgotten again (`forget_unseen`):
//! egui keeps textures for anything it can still reach, so the second
//! half is what bounds memory. Thumbnails that went through
//! [`thumbs`](crate::thumbs)' disk cache answer from a file afterwards.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use egui::{Context, Id, Image, Ui};

/// How long an undrawn image stays before it is forgotten.
const KEEP: f64 = 60.0;

/// When each image was last drawn, in egui's input time.
#[derive(Clone, Default)]
struct Drawn(HashMap<String, f64>);

/// The thumbnails' directory on disk, and the URLs this run already sent
/// to the backend so they are not asked for twice.
static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

struct Cache {
    dir: PathBuf,
    asked: HashSet<String>,
    retry_at: HashMap<String, f64>,
    loaded: HashSet<String>,
    wanted: Vec<String>,
}

impl Cache {
    fn request(&mut self, url: &str, now: f64) {
        if self.retry_at.get(url).is_some_and(|retry| now < *retry)
            || !self.asked.insert(url.to_owned())
        {
            return;
        }
        self.retry_at.remove(url);
        self.wanted.push(url.to_owned());
    }

    fn completed(&mut self, url: &str, ready: bool, now: f64, seen: bool) {
        self.asked.remove(url);
        self.retry_at.remove(url);
        if !ready && seen {
            self.retry_at.insert(url.to_owned(), now + 30.0);
        }
    }
}

/// Starts the disk cache: thumbnails fetched once live under `dir` and
/// answer from a file afterwards.
pub fn init(dir: PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    *CACHE.lock().unwrap() = Some(Cache {
        dir,
        asked: HashSet::new(),
        retry_at: HashMap::new(),
        loaded: HashSet::new(),
        wanted: Vec::new(),
    });
}

/// The URLs seen since the last drain, for the backend to put on disk.
pub fn drain_wanted() -> Vec<String> {
    let mut cache = CACHE.lock().unwrap();
    match cache.as_mut() {
        Some(cache) => std::mem::take(&mut cache.wanted),
        None => Vec::new(),
    }
}

#[cfg(test)]
pub(crate) fn drawn_count(ctx: &Context) -> usize {
    ctx.data(|data| {
        data.get_temp::<Drawn>(id())
            .map_or(0, |drawn| drawn.0.len())
    })
}

fn id() -> Id {
    Id::new("youtfast-remote-images")
}

/// The image at `url`, marked as drawn now. When the URL's bytes are on
/// disk already they are drawn from there; otherwise the network answers
/// this time and the backend is asked to keep them.
pub fn remote<'a>(ui: &Ui, url: &'a str) -> Image<'a> {
    let ctx = ui.ctx();
    let now = ctx.input(|input| input.time);
    ctx.data_mut(|data| {
        let drawn = &mut data.get_temp_mut_or_default::<Drawn>(id()).0;
        match drawn.get_mut(url) {
            Some(at) => *at = now,
            None => {
                drawn.insert(url.to_owned(), now);
            }
        }
    });
    let file = {
        let mut cache = CACHE.lock().unwrap();
        match cache.as_mut() {
            Some(cache) => {
                let path = crate::thumbs::path(&cache.dir, url);
                if cache.loaded.contains(url) {
                    return Image::new(memory_uri(&cache.dir, url));
                }
                if path.exists() {
                    Some(path)
                } else {
                    cache.request(url, now);
                    if let Some(retry) = cache.retry_at.get(url) {
                        ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                            (retry - now).max(0.0),
                        ));
                    }
                    None
                }
            }
            None => None,
        }
    };
    match file {
        Some(path) => Image::new(crate::thumbs::uri(&path)),
        None => Image::from_bytes(
            "bytes://youtfast-thumbnail-pending.svg",
            &b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>"[..],
        ),
    }
}

fn memory_uri(dir: &std::path::Path, url: &str) -> String {
    format!(
        "bytes://thumbnail/{}",
        crate::thumbs::path(dir, url)
            .file_name()
            .unwrap()
            .to_string_lossy()
    )
}

pub fn completed(ctx: &Context, url: &str, ready: bool, bytes: Option<std::sync::Arc<[u8]>>) {
    let now = ctx.input(|input| input.time);
    let seen = ctx.data(|data| {
        data.get_temp::<Drawn>(id())
            .is_some_and(|drawn| drawn.0.contains_key(url))
    });
    if let Some(cache) = CACHE.lock().unwrap().as_mut() {
        cache.completed(url, ready, now, seen);
        if seen && let Some(bytes) = bytes {
            ctx.include_bytes(memory_uri(&cache.dir, url), bytes);
            cache.loaded.insert(url.to_owned());
        }
        ctx.forget_image(&crate::thumbs::uri(&crate::thumbs::path(&cache.dir, url)));
    }
    if !ready {
        ctx.request_repaint_after(std::time::Duration::from_secs(30));
    }
}

/// Forgets the images not drawn for [`KEEP`] seconds. Call once a frame.
pub fn forget_unseen(ctx: &Context) {
    let now = ctx.input(|input| input.time);
    let stale = ctx.data_mut(|data| {
        let drawn = &mut data.get_temp_mut_or_default::<Drawn>(id()).0;
        take_stale(drawn, now, KEEP)
    });
    for url in stale {
        if let Some(cache) = CACHE.lock().unwrap().as_mut() {
            cache.retry_at.remove(&url);
            cache.loaded.remove(&url);
            ctx.forget_image(&memory_uri(&cache.dir, &url));
        }
        ctx.forget_image(&url);
        // A cached URL is drawn through its `file://` URI, under which the
        // texture is stored; forgetting only the URL would never free it.
        let file = {
            let cache = CACHE.lock().unwrap();
            cache
                .as_ref()
                .map(|cache| crate::thumbs::uri(&crate::thumbs::path(&cache.dir, &url)))
        };
        if let Some(file) = file {
            ctx.forget_image(&file);
        }
    }
}

/// Removes and returns the URLs last drawn more than `keep` seconds ago.
fn take_stale(drawn: &mut HashMap<String, f64>, now: f64, keep: f64) -> Vec<String> {
    let stale: Vec<String> = drawn
        .iter()
        .filter(|(_, at)| now - **at > keep)
        .map(|(url, _)| url.clone())
        .collect();
    for url in &stale {
        drawn.remove(url);
    }
    stale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_downloads_are_not_duplicated_and_failures_retry_after_backoff() {
        let mut cache = Cache {
            dir: PathBuf::new(),
            asked: HashSet::new(),
            retry_at: HashMap::new(),
            loaded: HashSet::new(),
            wanted: Vec::new(),
        };
        cache.request("synthetic", 0.0);
        cache.request("synthetic", 1.0);
        assert_eq!(std::mem::take(&mut cache.wanted), ["synthetic"]);
        cache.completed("synthetic", false, 2.0, true);
        cache.request("synthetic", 31.0);
        assert!(cache.wanted.is_empty());
        cache.request("synthetic", 32.0);
        assert_eq!(std::mem::take(&mut cache.wanted), ["synthetic"]);
        cache.completed("synthetic", true, 33.0, true);
        assert!(cache.asked.is_empty());
        cache.request("forgotten", 34.0);
        cache.completed("forgotten", false, 100.0, false);
        assert!(cache.asked.is_empty() && cache.retry_at.is_empty());
    }

    #[test]
    fn only_images_not_drawn_lately_are_forgotten() {
        let mut drawn = HashMap::from([("old".to_owned(), 10.0), ("recent".to_owned(), 100.0)]);
        assert_eq!(take_stale(&mut drawn, 120.0, 60.0), vec!["old".to_owned()]);
        assert_eq!(drawn.keys().collect::<Vec<_>>(), vec!["recent"]);
        assert!(take_stale(&mut drawn, 150.0, 60.0).is_empty());
    }
}
