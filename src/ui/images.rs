//! One bounded download supplies both display and disk. Animations use their
//! own adaptive decoder; ordinary thumbnails retain egui's image loader.

mod animation;
pub use animation::install as install_animation_loader;

use crate::i18n::{Locale, gettext as tr};
use crate::thumbs::Failure;
use egui::{Context, Id, Image, Ui};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const KEEP: f64 = 60.0;
#[derive(Clone, Default)]
struct Drawn(HashMap<String, f64>);
static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
struct Failed {
    reason: Failure,
    attempt: u32,
    retry_at: Option<f64>,
    manual_after: f64,
}
struct Cache {
    dir: PathBuf,
    asked: HashSet<String>,
    failed: HashMap<String, Failed>,
    loaded: HashSet<String>,
    wanted: Vec<(String, bool)>,
    expired: Vec<(String, bool)>,
    refreshed: HashSet<String>,
    repaired: HashSet<String>,
}
impl Cache {
    /// Queues `url` for the backend; false when it is already on its way
    /// or waiting to be retried.
    fn request(&mut self, url: &str, now: f64) -> bool {
        if self
            .failed
            .get(url)
            .is_some_and(|f| f.retry_at.is_none_or(|at| now < at))
            || !self.asked.insert(url.to_owned())
        {
            return false;
        }
        self.wanted.push((url.to_owned(), false));
        true
    }
    fn completed(&mut self, url: &str, result: &Result<Arc<[u8]>, Failure>, now: f64, seen: bool) {
        self.asked.remove(url);
        if !seen {
            self.failed.remove(url);
            return;
        }
        match result {
            Ok(_) => {
                self.failed.remove(url);
            }
            Err(reason) => {
                let attempt = self.failed.get(url).map_or(1, |f| f.attempt + 1);
                self.failed.insert(
                    url.to_owned(),
                    Failed {
                        reason: *reason,
                        attempt,
                        retry_at: reason.retry_delay(attempt).map(|delay| now + delay),
                        manual_after: if let Failure::RateLimited(seconds) = reason {
                            now + (*seconds).max(30) as f64
                        } else {
                            now
                        },
                    },
                );
                if *reason == Failure::Access && self.refreshed.insert(url.to_owned()) {
                    self.expired.push((url.to_owned(), false));
                }
            }
        }
    }
    fn retry(&mut self, url: &str, now: f64, repair: bool) -> bool {
        if self.asked.contains(url)
            || self.failed.get(url).is_some_and(|f| now < f.manual_after)
            || (repair && !self.repaired.insert(url.to_owned()))
        {
            return false;
        }
        if self
            .failed
            .get(url)
            .is_some_and(|f| f.reason == Failure::Access)
        {
            self.expired.push((url.to_owned(), true));
        }
        self.failed.remove(url);
        self.loaded.remove(url);
        self.asked.insert(url.to_owned());
        self.wanted.push((url.to_owned(), true));
        true
    }
}
pub fn init(dir: PathBuf) {
    // Directory creation and all disk reads/writes belong to the backend.
    *CACHE.lock().unwrap() = Some(Cache {
        dir,
        asked: HashSet::new(),
        failed: HashMap::new(),
        loaded: HashSet::new(),
        wanted: Vec::new(),
        expired: Vec::new(),
        refreshed: HashSet::new(),
        repaired: HashSet::new(),
    });
}
pub fn drain_wanted() -> Vec<(String, bool)> {
    CACHE
        .lock()
        .unwrap()
        .as_mut()
        .map_or_else(Vec::new, |c| std::mem::take(&mut c.wanted))
}
pub fn drain_expired() -> Vec<(String, bool)> {
    CACHE
        .lock()
        .unwrap()
        .as_mut()
        .map_or_else(Vec::new, |c| std::mem::take(&mut c.expired))
}
fn id() -> Id {
    Id::new("youtfast-remote-images")
}
#[cfg(test)]
pub(crate) fn drawn_count(ctx: &Context) -> usize {
    ctx.data(|data| {
        data.get_temp::<Drawn>(id())
            .map_or(0, |drawn| drawn.0.len())
    })
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
/// Only the backend accesses files or the network, including a disk cache hit.
pub fn remote<'a>(ui: &Ui, url: &'a str) -> Image<'a> {
    #[cfg(feature = "demo")]
    if ui
        .ctx()
        .data(|d| d.get_temp::<bool>(egui::Id::new("synthetic-images")))
        .unwrap_or(false)
    {
        return Image::from_bytes(
            "bytes://youtfast-demo.svg",
            include_bytes!("../../assets/demo-thumbnail.svg"),
        );
    }
    let ctx = ui.ctx();
    let now = ctx.input(|input| input.time);
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Drawn>(id())
            .0
            .insert(url.to_owned(), now);
    });
    if let Some(cache) = CACHE.lock().unwrap().as_mut() {
        if cache.loaded.contains(url) {
            return Image::new(memory_uri(&cache.dir, url));
        }
        // The next pass hands the request to the backend; it should not
        // wait for unrelated input to come.
        if cache.request(url, now) {
            ctx.request_repaint();
        }
        if let Some(retry) = cache.failed.get(url).and_then(|f| f.retry_at) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                (retry - now).clamp(0.1, 60.0),
            ));
        }
    }
    Image::from_bytes(
        "bytes://youtfast-thumbnail-pending.svg",
        &b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>"[..],
    )
}

pub fn attachment(ui: &mut Ui, url: &str, rect: egui::Rect, locale: Locale) {
    let image = remote(ui, url);
    let status = CACHE.lock().unwrap().as_ref().and_then(|c| {
        if c.loaded.contains(url) {
            Some(Ok(memory_uri(&c.dir, url)))
        } else {
            c.failed.get(url).map(|f| Err(f.reason))
        }
    });
    match status {
        Some(Ok(uri)) => match ui.ctx().try_load_bytes(&uri) {
            Ok(egui::load::BytesPoll::Ready { bytes, .. })
                if egui::has_gif_magic_header(&bytes) || egui::has_webp_header(&bytes) =>
            {
                animation::install(ui.ctx());
                let bytes = match bytes {
                    egui::load::Bytes::Shared(bytes) => bytes,
                    egui::load::Bytes::Static(bytes) => Arc::from(bytes),
                };
                if let Some(repair) = animation::show(ui, &uri, bytes, rect, locale) {
                    retry(ui.ctx(), url, repair);
                }
            }
            _ => {
                if ui
                    .ctx()
                    .try_load_image(&uri, egui::load::SizeHint::default())
                    .is_err()
                {
                    let manual = ui
                        .put(
                            rect,
                            egui::Button::new(tr(locale, "Image data invalid · Retry")),
                        )
                        .clicked();
                    retry(ui.ctx(), url, !manual);
                } else {
                    ui.put(
                        rect,
                        image
                            .fit_to_exact_size(rect.size())
                            .corner_radius(8)
                            .alt_text(tr(locale, "Comment image")),
                    );
                }
            }
        },
        Some(Err(reason)) => {
            let label = match reason {
                Failure::Access => "Image unavailable · Retry source",
                Failure::Unavailable => "Image removed or unavailable · Retry",
                Failure::RateLimited(_) => "Image temporarily limited · Retry later",
                Failure::TooLarge => "Image exceeds safety limits · Retry",
                Failure::Invalid => "Image data invalid · Retry",
                _ => "Could not load image · Retry",
            };
            if ui.put(rect, egui::Button::new(tr(locale, label))).clicked() {
                retry(ui.ctx(), url, false);
            }
        }
        None => {
            ui.put(rect, egui::Label::new(tr(locale, "Loading image…")));
        }
    }
}
fn retry(ctx: &Context, url: &str, repair: bool) {
    if let Some(c) = CACHE.lock().unwrap().as_mut()
        && c.retry(url, ctx.input(|i| i.time), repair)
    {
        let uri = memory_uri(&c.dir, url);
        animation::install(ctx);
        animation::forget(ctx, &uri);
        ctx.forget_image(&uri);
        ctx.request_repaint();
    }
}
pub fn completed(ctx: &Context, url: &str, result: Result<Arc<[u8]>, Failure>) {
    let now = ctx.input(|input| input.time);
    let seen = ctx.data(|data| {
        data.get_temp::<Drawn>(id())
            .is_some_and(|drawn| drawn.0.contains_key(url))
    });
    if let Some(cache) = CACHE.lock().unwrap().as_mut() {
        cache.completed(url, &result, now, seen);
        if seen && let Ok(bytes) = result {
            ctx.include_bytes(memory_uri(&cache.dir, url), bytes);
            cache.loaded.insert(url.to_owned());
        }
        if let Some(retry) = cache.failed.get(url).and_then(|f| f.retry_at) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                (retry - now).clamp(0.1, 60.0),
            ));
        }
    }
}
pub fn forget_unseen(ctx: &Context) {
    animation::install(ctx);
    animation::sweep(ctx);
    let now = ctx.input(|input| input.time);
    let stale = ctx.data_mut(|data| {
        take_stale(
            &mut data.get_temp_mut_or_default::<Drawn>(id()).0,
            now,
            KEEP,
        )
    });
    if let Some(c) = CACHE.lock().unwrap().as_mut() {
        for url in stale {
            c.failed.remove(&url);
            c.loaded.remove(&url);
            c.refreshed.remove(&url);
            c.repaired.remove(&url);
            let uri = memory_uri(&c.dir, &url);
            animation::forget(ctx, &uri);
            ctx.forget_image(&uri);
        }
    }
}
fn take_stale(drawn: &mut HashMap<String, f64>, now: f64, keep: f64) -> Vec<String> {
    let stale: Vec<_> = drawn
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
    fn cache() -> Cache {
        Cache {
            dir: PathBuf::new(),
            asked: HashSet::new(),
            failed: HashMap::new(),
            loaded: HashSet::new(),
            wanted: Vec::new(),
            expired: Vec::new(),
            refreshed: HashSet::new(),
            repaired: HashSet::new(),
        }
    }
    #[test]
    fn retries_are_bounded_respect_server_delay_and_ignore_late_failures() {
        let mut c = cache();
        for (now, expected) in [(0.0, 1), (11.0, 2), (32.0, 3)] {
            c.request("synthetic", now);
            c.request("synthetic", now);
            assert_eq!(c.wanted.len(), expected);
            c.completed("synthetic", &Err(Failure::Server), now, true);
        }
        c.request("synthetic", 1000.0);
        assert_eq!(c.wanted.len(), 3);
        c.completed("late", &Err(Failure::Network), 1000.0, false);
        assert!(!c.failed.contains_key("late"));
        c.completed("limited", &Err(Failure::RateLimited(300)), 0.0, true);
        c.request("limited", 299.0);
        assert!(!c.retry("limited", 299.0, false));
        c.request("limited", 300.0);
        assert_eq!(c.wanted.last(), Some(&("limited".to_owned(), false)));
    }
    #[test]
    fn a_url_is_queued_once_while_on_its_way() {
        let mut c = cache();
        assert!(c.request("synthetic", 0.0));
        assert!(!c.request("synthetic", 0.0));
        assert_eq!(c.wanted.len(), 1);
    }
    #[test]
    fn expired_source_and_corrupt_cache_are_retried_only_once() {
        let mut c = cache();
        for _ in 0..2 {
            c.completed("expired", &Err(Failure::Access), 0.0, true);
        }
        assert_eq!(c.expired, [("expired".to_owned(), false)]);
        assert!(c.retry("bad", 0.0, true));
        c.completed("bad", &Err(Failure::Invalid), 0.0, true);
        assert!(!c.retry("bad", 0.0, true));
    }
    #[test]
    fn only_unseen_images_are_forgotten() {
        let mut drawn = HashMap::from([("old".to_owned(), 10.0), ("recent".to_owned(), 100.0)]);
        assert_eq!(take_stale(&mut drawn, 120.0, 60.0), ["old"]);
        assert!(drawn.contains_key("recent"));
    }
}
