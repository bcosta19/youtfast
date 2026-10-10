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
    wanted: Vec<String>,
}

/// Starts the disk cache: thumbnails fetched once live under `dir` and
/// answer from a file afterwards.
pub fn init(dir: PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    *CACHE.lock().unwrap() = Some(Cache {
        dir,
        asked: HashSet::new(),
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
                if path.exists() {
                    Some(path)
                } else if cache.asked.insert(url.to_owned()) {
                    cache.wanted.push(url.to_owned());
                    None
                } else {
                    None
                }
            }
            None => None,
        }
    };
    match file {
        Some(path) => Image::new(crate::thumbs::uri(&path)),
        None => Image::new(url),
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
    fn only_images_not_drawn_lately_are_forgotten() {
        let mut drawn = HashMap::from([("old".to_owned(), 10.0), ("recent".to_owned(), 100.0)]);
        assert_eq!(take_stale(&mut drawn, 120.0, 60.0), vec!["old".to_owned()]);
        assert_eq!(drawn.keys().collect::<Vec<_>>(), vec!["recent"]);
        assert!(take_stale(&mut drawn, 150.0, 60.0).is_empty());
    }
}
