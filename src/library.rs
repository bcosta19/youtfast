//! What YouTFast keeps on this computer, with or without an account:
//! subscriptions, watch history (with where each video was left), and
//! Watch later. One JSON file, written atomically off the interface thread.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::{AccountSource, Channel, Video};

/// History keeps the most recent this many videos.
pub(crate) const HISTORY_LIMIT: usize = 1000;
/// A video left within this many seconds of its start or end starts over.
const RESUME_MARGIN: f64 = 15.0;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub subscriptions: Vec<Channel>,
    /// Most recent first.
    pub history: Vec<Watched>,
    /// Most recently added first.
    pub watch_later: Vec<Video>,
    /// The YouTube accounts signed in here, named. The sessions
    /// themselves are in the credential store, one entry per id.
    pub accounts: Vec<Account>,
    /// The account the app is signed in as, if any.
    pub active_account: Option<u32>,
    /// Unix seconds when the user last opened Notifications; uploads
    /// newer than it are "new".
    pub alerts_seen_at: Option<i64>,
    /// Unix seconds of the newest upload already shown as a desktop
    /// notification.
    pub alerts_notified_at: Option<i64>,
    /// The single account older files carried; folded into `accounts` on
    /// load, then never written again.
    #[serde(skip_serializing)]
    pub account: Option<AccountSource>,
    /// The history's video ids, for `watched` without scanning it. Kept
    /// by `reindex_watched`; rebuilt on load, never written.
    #[serde(skip)]
    watched_ids: HashSet<String>,
}

/// A named YouTube account; how its session was read is its `source`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Account {
    /// This library's own number: the keyring key and the active marker.
    pub id: u32,
    pub name: String,
    pub source: AccountSource,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Watched {
    pub video: Video,
    /// Unix seconds.
    pub at: i64,
    /// Where it was left, in seconds; zero once finished.
    #[serde(default)]
    pub position: f64,
}

/// What a Google Takeout file brought.
#[derive(Debug, Default)]
pub struct Import {
    pub subscriptions: Vec<Channel>,
    pub history: Vec<Watched>,
}

impl Library {
    /// Reads the library. A missing file is an empty library; an unreadable
    /// one is moved aside rather than overwritten by the next save.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let mut library: Library = crate::paths::load_json(path, "library")?;
        // Older files' single `account` becomes account 1.
        if library.accounts.is_empty()
            && let Some(source) = library.account.take()
        {
            library.accounts.push(Account {
                id: 1,
                name: "My account".to_owned(),
                source,
            });
            library.active_account = Some(1);
        }
        library.reindex_watched();
        Ok(library)
    }

    /// Rebuilds the video-id index `watched` reads after the history
    /// changed (load, a recorded watch, a Takeout merge, a clear).
    fn reindex_watched(&mut self) {
        self.watched_ids = self
            .history
            .iter()
            .map(|watched| watched.video.id.clone())
            .collect();
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        crate::paths::save_json(path, self)
    }

    /// The account with this id.
    pub fn account(&self, id: u32) -> Option<&Account> {
        self.accounts.iter().find(|account| account.id == id)
    }

    /// The account with this id, to change.
    pub fn account_mut(&mut self, id: u32) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|account| account.id == id)
    }

    /// Whether `name` is already an account's; the add form refuses it.
    pub fn account_name_taken(&self, name: &str) -> bool {
        self.accounts.iter().any(|account| account.name == name)
    }

    pub fn is_subscribed(&self, channel_id: &str) -> bool {
        self.subscriptions
            .iter()
            .any(|channel| channel.id == channel_id)
    }

    /// Subscribes, or unsubscribes when already subscribed. Returns whether
    /// the channel is subscribed now.
    pub fn toggle_subscription(&mut self, channel: Channel) -> bool {
        if take_first(&mut self.subscriptions, |known| known.id == channel.id).is_some() {
            false
        } else {
            self.subscriptions.push(channel);
            self.subscriptions
                .sort_by_cached_key(|channel| channel.name.to_lowercase());
            true
        }
    }

    pub fn in_watch_later(&self, video_id: &str) -> bool {
        self.watch_later.iter().any(|video| video.id == video_id)
    }

    /// Adds to Watch later, or removes when already there. Returns whether
    /// the video is there now.
    pub fn toggle_watch_later(&mut self, video: Video) -> bool {
        if take_first(&mut self.watch_later, |known| known.id == video.id).is_some() {
            false
        } else {
            self.watch_later.insert(0, video);
            true
        }
    }

    /// Puts the video at the top of the history, keeping where it was left.
    pub fn record_watch(&mut self, video: Video, now: i64) {
        let position = take_first(&mut self.history, |watched| watched.video.id == video.id)
            .map_or(0.0, |watched| watched.position);
        self.history.insert(
            0,
            Watched {
                video,
                at: now,
                position,
            },
        );
        self.history.truncate(HISTORY_LIMIT);
        self.reindex_watched();
    }

    /// Whether the video is in the history.
    pub fn watched(&self, video_id: &str) -> bool {
        self.watched_ids.contains(video_id)
    }

    /// Where to start the video: where it was left, unless that was right at
    /// its start or end.
    pub fn resume_position(&self, video_id: &str) -> Option<f64> {
        if !self.watched(video_id) {
            return None;
        }
        let watched = self
            .history
            .iter()
            .find(|watched| watched.video.id == video_id)?;
        (watched.position >= RESUME_MARGIN).then_some(watched.position)
    }

    /// Remembers where the video was left. Near its end counts as finished,
    /// so it starts over next time.
    pub fn set_position(&mut self, video_id: &str, position: f64, duration: f64) {
        let Some(watched) = self
            .history
            .iter_mut()
            .find(|watched| watched.video.id == video_id)
        else {
            return;
        };
        let finished = duration > 0.0 && position >= duration - RESUME_MARGIN;
        watched.position = if finished || !position.is_finite() {
            0.0
        } else {
            position
        };
    }

    /// The video was watched to its end: next time it starts over.
    pub fn mark_finished(&mut self, video_id: &str) {
        if let Some(watched) = self
            .history
            .iter_mut()
            .find(|watched| watched.video.id == video_id)
        {
            watched.position = 0.0;
        }
    }

    /// Replaces what the library knows about a video (its title once a
    /// video opened from a link has loaded, for example).
    pub fn update_video(&mut self, video: &Video) {
        for watched in &mut self.history {
            if watched.video.id == video.id {
                watched.video = video.clone();
            }
        }
        for later in &mut self.watch_later {
            if later.id == video.id {
                *later = video.clone();
            }
        }
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.watched_ids.clear();
    }

    /// Adds what a Takeout file brought. Returns how many subscriptions and
    /// history entries were new.
    pub fn merge(&mut self, import: Import) -> (usize, usize) {
        let mut known_channels: HashSet<String> = self
            .subscriptions
            .iter()
            .map(|channel| channel.id.clone())
            .collect();
        let mut new_subscriptions = 0;
        for channel in import.subscriptions {
            if known_channels.insert(channel.id.clone()) {
                self.subscriptions.push(channel);
                new_subscriptions += 1;
            }
        }
        self.subscriptions
            .sort_by_cached_key(|channel| channel.name.to_lowercase());

        let mut known_videos: HashMap<String, usize> = self
            .history
            .iter()
            .enumerate()
            .map(|(index, watched)| (watched.video.id.clone(), index))
            .collect();
        let mut new_history = 0;
        for watched in import.history {
            match known_videos.entry(watched.video.id.clone()) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    let known = &mut self.history[*entry.get()];
                    known.at = known.at.max(watched.at);
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(self.history.len());
                    self.history.push(watched);
                    new_history += 1;
                }
            }
        }
        self.history.sort_by_key(|watched| Reverse(watched.at));
        self.history.truncate(HISTORY_LIMIT);
        self.reindex_watched();
        (new_subscriptions, new_history)
    }
}

/// Removes and returns the first item of `list` that `matches` accepts.
fn take_first<T>(list: &mut Vec<T>, matches: impl FnMut(&T) -> bool) -> Option<T> {
    let index = list.iter().position(matches)?;
    Some(list.remove(index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_large_import_deduplicates_keeps_recent_history_and_preserves_resume_positions() {
        let mut library = Library::default();
        library.history.push(Watched {
            video: Video::from_id("00000019999".into()),
            at: 19_999,
            position: 42.0,
        });
        let history = (0..20_000)
            .flat_map(|index| {
                let watched = Watched {
                    video: Video::from_id(format!("{index:011}")),
                    at: index,
                    position: 0.0,
                };
                [watched.clone(), watched]
            })
            .collect();
        let channel = Channel {
            id: "UC_x5XG1OV2P6uZZ5FSM9Ttw".into(),
            name: "Synthetic".into(),
            avatar: None,
        };
        assert_eq!(
            library.merge(Import {
                subscriptions: vec![channel.clone(), channel],
                history
            }),
            (1, 19_999)
        );
        assert_eq!(library.history.len(), HISTORY_LIMIT);
        assert_eq!(library.history[0].at, 19_999);
        assert_eq!(library.history[0].position, 42.0);
        assert_eq!(library.history.last().unwrap().at, 19_000);
        assert!(library.watched("00000019999"));
        assert!(!library.watched("00000000000"));
    }

    fn video(id: &str) -> Video {
        Video {
            title: format!("Video {id}"),
            ..Video::from_id(id.to_owned())
        }
    }

    fn channel(id: &str, name: &str) -> Channel {
        Channel {
            id: id.to_owned(),
            name: name.to_owned(),
            avatar: None,
        }
    }

    #[test]
    fn subscriptions_toggle_and_stay_sorted() {
        let mut library = Library::default();
        assert!(library.toggle_subscription(channel("UC2", "beta")));
        assert!(library.toggle_subscription(channel("UC1", "Alpha")));
        let names: Vec<&str> = library
            .subscriptions
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(names, ["Alpha", "beta"]);
        assert!(!library.toggle_subscription(channel("UC2", "beta")));
        assert!(!library.is_subscribed("UC2"));
    }

    #[test]
    fn history_moves_to_top_and_keeps_position() {
        let mut library = Library::default();
        library.record_watch(video("aaaaaaaaaaa"), 1);
        library.set_position("aaaaaaaaaaa", 120.0, 600.0);
        library.record_watch(video("bbbbbbbbbbb"), 2);
        library.record_watch(video("aaaaaaaaaaa"), 3);
        assert_eq!(library.history.len(), 2);
        assert_eq!(library.history[0].video.id, "aaaaaaaaaaa");
        assert_eq!(library.history[0].at, 3);
        assert_eq!(library.resume_position("aaaaaaaaaaa"), Some(120.0));
    }

    #[test]
    fn resume_skips_the_edges() {
        let mut library = Library::default();
        library.record_watch(video("aaaaaaaaaaa"), 1);
        library.set_position("aaaaaaaaaaa", 5.0, 600.0);
        assert_eq!(library.resume_position("aaaaaaaaaaa"), None);
        library.set_position("aaaaaaaaaaa", 590.0, 600.0);
        assert_eq!(library.resume_position("aaaaaaaaaaa"), None);
        assert_eq!(library.history[0].position, 0.0);
        // A live stream has no duration; anything past the start resumes.
        library.set_position("aaaaaaaaaaa", 300.0, 0.0);
        assert_eq!(library.resume_position("aaaaaaaaaaa"), Some(300.0));
    }

    #[test]
    fn a_single_account_file_migrates_to_named_accounts() {
        let dir = std::env::var("CARGO_TARGET_TMPDIR")
            .unwrap_or_else(|_| format!("{}/target", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).unwrap();
        let path = Path::new(&dir).join(format!("library-migrate-{}.json", crate::util::now()));
        std::fs::write(
            &path,
            r#"{"subscriptions":[],"history":[],"watch_later":[],"account":{"Browser":"chromium"}}"#,
        )
        .unwrap();
        let library = Library::load(&path).unwrap();
        assert_eq!(
            library.accounts,
            vec![Account {
                id: 1,
                name: "My account".to_owned(),
                source: AccountSource::Browser("chromium".into()),
            }]
        );
        assert_eq!(library.active_account, Some(1));
        // And saving writes accounts, never the old field again.
        library.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(r#""accounts""#));
        assert!(!text.contains(r#""account":{"#));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn account_names_are_unique() {
        let mut library = Library::default();
        assert!(!library.account_name_taken("one"));
        library.accounts.push(Account {
            id: 1,
            name: "one".to_owned(),
            source: AccountSource::File,
        });
        assert!(library.account_name_taken("one"));
    }

    #[test]
    fn watch_later_toggles() {
        let mut library = Library::default();
        assert!(library.toggle_watch_later(video("aaaaaaaaaaa")));
        assert!(library.in_watch_later("aaaaaaaaaaa"));
        assert!(!library.toggle_watch_later(video("aaaaaaaaaaa")));
        assert!(library.watch_later.is_empty());
    }

    #[test]
    fn merging_a_takeout_skips_what_is_known() {
        let mut library = Library::default();
        library.toggle_subscription(channel("UC1", "Alpha"));
        library.record_watch(video("aaaaaaaaaaa"), 10);
        let import = Import {
            subscriptions: vec![channel("UC1", "Alpha"), channel("UC2", "Beta")],
            history: vec![
                Watched {
                    video: video("aaaaaaaaaaa"),
                    at: 5,
                    position: 0.0,
                },
                Watched {
                    video: video("bbbbbbbbbbb"),
                    at: 20,
                    position: 0.0,
                },
            ],
        };
        assert_eq!(library.merge(import), (1, 1));
        assert_eq!(library.subscriptions.len(), 2);
        let order: Vec<&str> = library
            .history
            .iter()
            .map(|w| w.video.id.as_str())
            .collect();
        assert_eq!(order, ["bbbbbbbbbbb", "aaaaaaaaaaa"]);
        assert_eq!(library.history[1].at, 10);
    }

    #[test]
    fn watched_follows_the_history_through_every_change() {
        let mut library = Library::default();
        assert!(!library.watched("aaaaaaaaaaa"));
        library.record_watch(video("aaaaaaaaaaa"), 1);
        assert!(library.watched("aaaaaaaaaaa"));
        // A Takeout merge and a reload see it too.
        library.merge(Import {
            subscriptions: Vec::new(),
            history: vec![Watched {
                video: video("bbbbbbbbbbb"),
                at: 2,
                position: 30.0,
            }],
        });
        assert!(library.watched("bbbbbbbbbbb"));
        assert_eq!(library.resume_position("bbbbbbbbbbb"), Some(30.0));
        let json = serde_json::to_string(&library).unwrap();
        let mut read: Library = serde_json::from_str(&json).unwrap();
        read.reindex_watched();
        assert!(read.watched("aaaaaaaaaaa"));
        assert!(read.watched("bbbbbbbbbbb"));
        assert!(!read.watched("ccccccccccc"));
        // Clearing leaves nothing watched.
        library.clear_history();
        assert!(!library.watched("aaaaaaaaaaa"));
        assert!(!library.watched("bbbbbbbbbbb"));
    }

    #[test]
    fn round_trips_and_reads_older_files() {
        let mut library = Library::default();
        library.toggle_subscription(channel("UC1", "Alpha"));
        library.accounts.push(Account {
            id: 1,
            name: "My account".to_owned(),
            source: AccountSource::Browser("chromium".into()),
        });
        library.active_account = Some(1);
        let json = serde_json::to_string(&library).unwrap();
        let read: Library = serde_json::from_str(&json).unwrap();
        assert_eq!(read.subscriptions, library.subscriptions);
        assert_eq!(read.accounts, library.accounts);
        assert_eq!(read.active_account, library.active_account);
        // Fields added later default when an older file lacks them.
        let old: Library = serde_json::from_str(
            r#"{"history": [{"video": {"id": "aaaaaaaaaaa", "title": "Old"}, "at": 1}]}"#,
        )
        .unwrap();
        assert_eq!(old.history[0].position, 0.0);
        assert!(old.subscriptions.is_empty());
    }
}
