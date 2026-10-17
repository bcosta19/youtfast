//! New-upload alerts: the timer asks the same sources as the home feed,
//! the app turns "what is newer than the last look" into a list, and
//! desktop notifications say what arrived since the last notification.
//!
//! YouTube's own notifications have no API; this checks subscriptions on
//! a timer instead.

use crate::i18n::{Locale, format as trf, gettext as tr};
use crate::model::Video;

/// At most this many unseen uploads are kept.
const ALERT_LIMIT: usize = 100;

/// What a check found.
pub struct Found {
    /// Unseen uploads, newest first, deduped.
    pub new: Vec<Video>,
    /// The ones a desktop notification should name.
    pub to_notify: Vec<Video>,
    /// The seen/notified stamps to store.
    pub seen_at: i64,
    pub notified_at: Option<i64>,
}

/// Splits `videos` into the unseen and the not-yet-notified, and moves
/// the stamps. The first run only sets `seen_at`: a brand-new user is
/// not buried under weeks of uploads.
pub fn compute(
    videos: Vec<Video>,
    seen_at: Option<i64>,
    notified_at: Option<i64>,
    now: i64,
) -> Option<Found> {
    let Some(seen_at) = seen_at else {
        // First run: mark "now" as the last look, tell nothing.
        return Some(Found {
            new: Vec::new(),
            to_notify: Vec::new(),
            seen_at: now,
            notified_at,
        });
    };
    let mut new: Vec<Video> = videos
        .into_iter()
        .filter(|video| video.published_at.is_some_and(|at| at > seen_at))
        .collect();
    // Newest first, deduped by id (the feed and the shelf may repeat).
    new.sort_by_key(|video| std::cmp::Reverse(video.published_at));
    let mut seen = std::collections::HashSet::new();
    new.retain(|video| seen.insert(video.id.clone()));
    new.truncate(ALERT_LIMIT);
    let newest = new.iter().filter_map(|video| video.published_at).max();
    let notified_floor = notified_at.unwrap_or(seen_at);
    let to_notify: Vec<Video> = new
        .iter()
        .filter(|video| video.published_at.is_some_and(|at| at > notified_floor))
        .cloned()
        .collect();
    let notified_at = newest
        .map(|at| at.max(notified_floor))
        .or(Some(notified_floor));
    Some(Found {
        new,
        to_notify,
        seen_at,
        notified_at,
    })
}

/// The desktop notification's summary and body.
pub fn notify_text(videos: &[Video], locale: Locale) -> (String, String) {
    let fallback = tr(locale, "A channel");
    match videos {
        [] => (String::new(), String::new()),
        [video] => {
            let channel = video.channel.as_deref().unwrap_or(&fallback);
            (
                trf(
                    locale,
                    "{channel} uploaded",
                    &[("channel", channel.to_owned())],
                ),
                video.title.clone(),
            )
        }
        _ => {
            let shown: Vec<String> = videos
                .iter()
                .take(3)
                .map(|video| {
                    format!(
                        "{}: {}",
                        video.channel.as_deref().unwrap_or(&fallback),
                        video.title
                    )
                })
                .collect();
            let mut body = shown.join("\n");
            let rest = videos.len() - shown.len();
            if rest > 0 {
                body.push('\n');
                body.push_str(&trf(
                    locale,
                    "and {count} more",
                    &[("count", rest.to_string())],
                ));
            }
            (
                trf(
                    locale,
                    "{count} new videos",
                    &[("count", videos.len().to_string())],
                ),
                body,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(id: &str, seconds: i64) -> Video {
        Video {
            published_at: Some(seconds),
            ..Video::from_id(id.to_owned())
        }
    }

    #[test]
    fn the_first_run_only_starts_the_clock() {
        let found = compute(vec![at("a", 100)], None, None, 500).unwrap();
        assert!(found.new.is_empty());
        assert!(found.to_notify.is_empty());
        assert_eq!(found.seen_at, 500);
    }

    #[test]
    fn new_and_notified_split_at_the_stamps() {
        let page = vec![at("a", 500), at("b", 300), at("c", 100)];
        let found = compute(page, Some(200), Some(300), 900).unwrap();
        // `new` is everything after the last look: a and b.
        assert_eq!(
            found
                .new
                .iter()
                .map(|video| video.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        // `to_notify` only what's newer than the last notification.
        assert_eq!(
            found
                .to_notify
                .iter()
                .map(|video| video.id.as_str())
                .collect::<Vec<_>>(),
            ["a"]
        );
        // And the stamp advances to the newest seen.
        assert_eq!(found.notified_at, Some(500));
    }

    #[test]
    fn repeats_and_overflow_are_trimmed() {
        let mut page: Vec<Video> = (0..150)
            .map(|index| at(&format!("v{index:03}"), index))
            .collect();
        page.push(at("v000", 0));
        let found = compute(page, Some(0), None, 1000).unwrap();
        assert_eq!(found.new.len(), ALERT_LIMIT);
        // Newest first.
        assert_eq!(found.new[0].id, "v149");
    }

    #[test]
    fn notify_text_is_compact() {
        let mut one = at("a", 1);
        one.channel = Some("Chan".to_owned());
        one.title = "Title".to_owned();
        assert_eq!(
            notify_text(&[one], Locale::English),
            ("Chan uploaded".to_owned(), "Title".to_owned())
        );
        let five: Vec<Video> = (0..5)
            .map(|index| {
                let mut video = at(&format!("v{index}"), index);
                video.channel = Some(format!("Ch{index}"));
                video.title = format!("T{index}");
                video
            })
            .collect();
        let (summary, body) = notify_text(&five, Locale::English);
        assert_eq!(summary, "5 new videos");
        assert!(body.contains("and 2 more"));
        let (summary, body) = notify_text(&five, Locale::PortugueseBrazil);
        assert_eq!(summary, "5 novos vídeos");
        assert!(body.contains("e mais 2"));
        // The language choice only changes the notification's own wording.
        assert!(body.contains("Ch0: T0"));
    }
}
