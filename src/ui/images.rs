//! Thumbnails and avatars, which egui's loaders keep decoded (about
//! 230 KB a thumbnail, and its texture) until told to forget them. Each
//! draw marks its URL; one not drawn for a while is forgotten, so memory
//! follows the pages on screen instead of every page visited. Drawn again,
//! it loads again from YouTube's image servers.

use std::collections::HashMap;

use egui::{Context, Id, Image, Ui};

/// How long an image stays decoded after it was last drawn, in seconds:
/// long enough that going back to a page does not load it all again.
const KEEP: f64 = 60.0;

/// When each image was last drawn, in egui's input time.
#[derive(Clone, Default)]
struct Drawn(HashMap<String, f64>);

fn id() -> Id {
    Id::new("youtfast-remote-images")
}

/// The image at `url`, marked as drawn now.
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
    Image::new(url)
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
