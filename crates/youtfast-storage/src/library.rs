//! What YouTFast keeps on this computer, with or without an account:
//! subscriptions, watch history (with where each video was left), and
//! Watch later. One JSON file, written atomically off the interface thread.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use youtfast_core::model::{AccountSource, Channel, Video};

/// History keeps the most recent this many videos.
pub(crate) const HISTORY_LIMIT: usize = 1000;
/// Maximum number of hidden video ids kept in the local library.
pub const HIDDEN_VIDEOS_LIMIT: usize = 5_000;
/// Maximum number of blocked channels kept in the local library.
pub const BLOCKED_CHANNELS_LIMIT: usize = 1_000;
/// A video left within this many seconds of its start or end starts over.
const RESUME_MARGIN: f64 = 15.0;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(from = "RawLibrary")]
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
    /// Video ids hidden via "Not interested", oldest first, bounded to
    /// [`HIDDEN_VIDEOS_LIMIT`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hidden_videos: Vec<String>,
    /// Channels blocked via "Don't recommend channel", oldest first, bounded
    /// to [`BLOCKED_CHANNELS_LIMIT`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    blocked_channels: Vec<Channel>,
    /// The single account older files carried; folded into `accounts` on
    /// load, then never written again.
    #[serde(skip_serializing)]
    pub account: Option<AccountSource>,
    /// The history's video ids with where each was left, for `watched` and
    /// `resume_position` without scanning it. Kept by `reindex_watched` and
    /// the position setters; rebuilt on load, never written.
    #[serde(skip)]
    watched_ids: HashMap<String, f64>,
    /// Changes whenever the history's rows or their titles change (not their
    /// resume positions), so views can cache what they derive from it.
    /// Unique across libraries; 0 until the first change.
    #[serde(skip)]
    history_stamp: u64,
    /// In-memory index of `hidden_videos` for O(1) lookups.
    #[serde(skip)]
    hidden_ids: HashSet<String>,
    /// In-memory index of `blocked_channels` for O(1) lookups.
    #[serde(skip)]
    blocked_ids: HashSet<String>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawLibrary {
    subscriptions: Vec<Channel>,
    history: Vec<Watched>,
    watch_later: Vec<Video>,
    accounts: Vec<Account>,
    active_account: Option<u32>,
    alerts_seen_at: Option<i64>,
    alerts_notified_at: Option<i64>,
    hidden_videos: Vec<String>,
    blocked_channels: Vec<Channel>,
    account: Option<AccountSource>,
}

impl From<RawLibrary> for Library {
    fn from(raw: RawLibrary) -> Self {
        let mut library = Self {
            subscriptions: raw.subscriptions,
            history: raw.history,
            watch_later: raw.watch_later,
            accounts: raw.accounts,
            active_account: raw.active_account,
            alerts_seen_at: raw.alerts_seen_at,
            alerts_notified_at: raw.alerts_notified_at,
            hidden_videos: raw.hidden_videos,
            blocked_channels: raw.blocked_channels,
            account: raw.account,
            watched_ids: HashMap::new(),
            history_stamp: 0,
            hidden_ids: HashSet::new(),
            blocked_ids: HashSet::new(),
        };
        library.reindex_watched();
        library.reindex_filters();
        library
    }
}

/// Hands out `Library::history_stamp`s.
static HISTORY_STAMPS: AtomicU64 = AtomicU64::new(1);

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

/// An imported watch entry, whose source may not supply resume data.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ImportedWatch {
    pub video: Video,
    pub at: i64,
    /// `Some(0.0)` means finished; `None` means no resume data was provided.
    #[serde(default)]
    pub position: Option<f64>,
}

/// What a Google Takeout or library backup file brought.
#[derive(Debug, Default, PartialEq)]
pub struct Import {
    pub subscriptions: Vec<Channel>,
    pub history: Vec<ImportedWatch>,
    pub watch_later: Vec<Video>,
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
        Ok(library)
    }

    /// Rebuilds the in-memory `hidden_ids` and `blocked_ids` indexes and
    /// enforces deduplication and size bounds after deserialization.
    fn reindex_filters(&mut self) {
        let capacity = self.hidden_videos.len().min(HIDDEN_VIDEOS_LIMIT);
        let mut hidden_ids = HashSet::with_capacity(capacity);
        let mut clean_hidden = Vec::with_capacity(capacity);
        for id in self.hidden_videos.drain(..).rev() {
            if !id.is_empty() && hidden_ids.insert(id.clone()) {
                clean_hidden.push(id);
                if clean_hidden.len() == HIDDEN_VIDEOS_LIMIT {
                    break;
                }
            }
        }
        clean_hidden.reverse();
        self.hidden_videos = clean_hidden;
        self.hidden_ids = hidden_ids;

        let capacity = self.blocked_channels.len().min(BLOCKED_CHANNELS_LIMIT);
        let mut blocked_ids = HashSet::with_capacity(capacity);
        let mut clean_blocked = Vec::with_capacity(capacity);
        for channel in self.blocked_channels.drain(..).rev() {
            if !channel.id.is_empty() && blocked_ids.insert(channel.id.clone()) {
                clean_blocked.push(channel);
                if clean_blocked.len() == BLOCKED_CHANNELS_LIMIT {
                    break;
                }
            }
        }
        clean_blocked.reverse();
        self.blocked_channels = clean_blocked;
        self.blocked_ids = blocked_ids;
    }

    /// Rebuilds the video-id index `watched` reads after the history
    /// changed (load, a recorded watch, a Takeout merge, a clear).
    fn reindex_watched(&mut self) {
        // The first entry of an id is the one `resume_position` reads.
        let mut ids = HashMap::with_capacity(self.history.len());
        for watched in &self.history {
            ids.entry(watched.video.id.clone())
                .or_insert(watched.position);
        }
        self.watched_ids = ids;
        self.touch_history();
    }

    fn touch_history(&mut self) {
        self.history_stamp = HISTORY_STAMPS.fetch_add(1, Ordering::Relaxed);
    }

    /// Changes whenever the history's rows or their titles do; see the field.
    pub fn history_stamp(&self) -> u64 {
        self.history_stamp
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

    /// Individually hidden video IDs, in order from oldest to newest.
    pub fn hidden_videos(&self) -> &[String] {
        &self.hidden_videos
    }

    /// Blocked channels, in order from oldest to newest.
    pub fn blocked_channels(&self) -> &[Channel] {
        &self.blocked_channels
    }

    /// Marks a video as "Not interested" on this device, keeping at most
    /// [`HIDDEN_VIDEOS_LIMIT`] entries (dropping the oldest).
    pub fn hide_video(&mut self, id: String) -> bool {
        if id.is_empty() || !self.hidden_ids.insert(id.clone()) {
            return false;
        }
        self.hidden_videos.push(id);
        if self.hidden_videos.len() > HIDDEN_VIDEOS_LIMIT {
            let excess = self.hidden_videos.len() - HIDDEN_VIDEOS_LIMIT;
            for dropped in self.hidden_videos.drain(..excess) {
                self.hidden_ids.remove(&dropped);
            }
        }
        true
    }

    pub fn is_video_hidden(&self, id: &str) -> bool {
        self.hidden_ids.contains(id)
    }

    /// Clears all hidden videos. Returns whether any were present.
    pub fn clear_hidden_videos(&mut self) -> bool {
        if self.hidden_videos.is_empty() && self.hidden_ids.is_empty() {
            return false;
        }
        self.hidden_videos.clear();
        self.hidden_ids.clear();
        true
    }

    /// Blocks a channel from Home, Subscriptions, and local recommendations,
    /// keeping at most [`BLOCKED_CHANNELS_LIMIT`] entries (dropping the oldest).
    pub fn block_channel(&mut self, channel: Channel) -> bool {
        if channel.id.is_empty() || !self.blocked_ids.insert(channel.id.clone()) {
            return false;
        }
        self.blocked_channels.push(channel);
        if self.blocked_channels.len() > BLOCKED_CHANNELS_LIMIT {
            let excess = self.blocked_channels.len() - BLOCKED_CHANNELS_LIMIT;
            for dropped in self.blocked_channels.drain(..excess) {
                self.blocked_ids.remove(&dropped.id);
            }
        }
        true
    }

    /// Unblocks a channel by id. Returns whether the channel was blocked.
    pub fn unblock_channel(&mut self, channel_id: &str) -> bool {
        if !self.blocked_ids.remove(channel_id) {
            return false;
        }
        self.blocked_channels
            .retain(|channel| channel.id != channel_id);
        true
    }

    pub fn is_channel_blocked(&self, channel_id: &str) -> bool {
        self.blocked_ids.contains(channel_id)
    }

    /// Whether a video is hidden either individually or via its blocked channel.
    pub fn is_hidden(&self, video: &Video) -> bool {
        self.is_video_hidden(&video.id)
            || video
                .channel_id
                .as_deref()
                .is_some_and(|id| self.is_channel_blocked(id))
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
        self.watched_ids.contains_key(video_id)
    }

    /// Where to start the video: where it was left, unless that was right at
    /// its start or end.
    pub fn resume_position(&self, video_id: &str) -> Option<f64> {
        let position = *self.watched_ids.get(video_id)?;
        (position >= RESUME_MARGIN).then_some(position)
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
        self.sync_position(video_id);
    }

    /// The video was watched to its end: next time it starts over.
    pub fn mark_finished(&mut self, video_id: &str) {
        if let Some(watched) = self
            .history
            .iter_mut()
            .find(|watched| watched.video.id == video_id)
        {
            watched.position = 0.0;
            self.sync_position(video_id);
        }
    }

    /// Copies the first history entry of this id's position into the index.
    fn sync_position(&mut self, video_id: &str) {
        if let Some(watched) = self.history.iter().find(|w| w.video.id == video_id)
            && let Some(slot) = self.watched_ids.get_mut(video_id)
        {
            *slot = watched.position;
        }
    }

    /// Replaces what the library knows about a video (its title once a
    /// video opened from a link has loaded, for example).
    pub fn update_video(&mut self, video: &Video) {
        for watched in &mut self.history {
            if watched.video.id == video.id {
                watched.video = video.clone();
                self.history_stamp = HISTORY_STAMPS.fetch_add(1, Ordering::Relaxed);
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
        self.touch_history();
    }

    /// Removes this video's watch and resume position while keeping the index in sync.
    pub fn remove_history(&mut self, id: &str) -> bool {
        let before = self.history.len();
        self.history.retain(|watched| watched.video.id != id);
        self.watched_ids.remove(id);
        self.touch_history();
        before != self.history.len()
    }

    /// A local search by title or channel; never sends watch history to YouTube.
    pub fn history_matching<'a>(&'a self, query: &str) -> impl Iterator<Item = &'a Watched> {
        let query = query.trim().to_lowercase();
        self.history.iter().filter(move |watched| {
            query.is_empty()
                || watched.video.title.to_lowercase().contains(&query)
                || watched
                    .video
                    .channel
                    .as_ref()
                    .is_some_and(|name| name.to_lowercase().contains(&query))
        })
    }

    /// Adds what a Takeout or backup file brought. Returns how many
    /// subscriptions and history entries were new.
    pub fn merge(&mut self, import: Import) -> (usize, usize) {
        let (subscriptions, history, _) = self.merge_with_watch_later(import);
        (subscriptions, history)
    }

    /// Adds what a Takeout or backup file brought. Returns how many
    /// subscriptions, history entries, and Watch later videos were new.
    pub fn merge_with_watch_later(&mut self, import: Import) -> (usize, usize, usize) {
        let mut known_channels: HashSet<String> = self
            .subscriptions
            .iter()
            .map(|channel| channel.id.clone())
            .collect();
        let mut new_subscriptions = 0;
        for channel in import.subscriptions {
            if !channel.id.trim().is_empty() && known_channels.insert(channel.id.clone()) {
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
        let original_len = self.history.len();
        for watched in import.history {
            if watched.video.id.trim().is_empty() {
                continue;
            }
            let position = watched
                .position
                .filter(|position| position.is_finite() && *position >= 0.0);
            match known_videos.entry(watched.video.id.clone()) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    let known = &mut self.history[*entry.get()];
                    if watched.at > known.at
                        && let Some(position) = position
                    {
                        known.position = position;
                    }
                    known.at = known.at.max(watched.at);
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(self.history.len());
                    self.history.push(Watched {
                        video: watched.video,
                        at: watched.at,
                        position: position.unwrap_or_default(),
                    });
                }
            }
        }
        self.history.sort_by_key(|watched| Reverse(watched.at));
        self.history.truncate(HISTORY_LIMIT);
        let new_history = self
            .history
            .iter()
            .filter(|watched| known_videos[&watched.video.id] >= original_len)
            .count();
        self.reindex_watched();

        let mut known_watch_later: HashSet<String> = self
            .watch_later
            .iter()
            .map(|video| video.id.clone())
            .collect();
        let mut new_watch_later = 0;
        for video in import.watch_later {
            if self.watch_later.len() >= crate::backup::WATCH_LATER_LIMIT {
                break;
            }
            if !video.id.trim().is_empty() && known_watch_later.insert(video.id.clone()) {
                self.watch_later.push(video);
                new_watch_later += 1;
            }
        }

        (new_subscriptions, new_history, new_watch_later)
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
    fn removing_one_history_entry_removes_its_resume_without_changing_the_others() {
        let mut library = Library::default();
        library.record_watch(Video::from_id("aaaaaaaaaaa".into()), 1);
        library.set_position("aaaaaaaaaaa", 45.0, 300.0);
        library.record_watch(Video::from_id("bbbbbbbbbbb".into()), 2);
        library.set_position("bbbbbbbbbbb", 90.0, 300.0);
        assert!(library.remove_history("aaaaaaaaaaa"));
        assert!(!library.watched("aaaaaaaaaaa"));
        assert_eq!(library.resume_position("aaaaaaaaaaa"), None);
        assert!(library.watched("bbbbbbbbbbb"));
        assert_eq!(library.resume_position("bbbbbbbbbbb"), Some(90.0));
        assert!(!library.remove_history("aaaaaaaaaaa"));
        assert_eq!(library.history.len(), 1);
    }

    #[test]
    fn local_history_search_matches_title_or_channel_without_changing_watch_order() {
        let mut library = Library::default();
        let mut first = Video::from_id("aaaaaaaaaaa".into());
        first.title = "Introdução ao Rust".into();
        first.channel = Some("Escola de programação".into());
        let mut second = Video::from_id("bbbbbbbbbbb".into());
        second.title = "Astronomia".into();
        second.channel = Some("Ciência".into());
        library.record_watch(first, 1);
        library.record_watch(second, 2);
        assert_eq!(
            library.history_matching(" RUST ").next().unwrap().video.id,
            "aaaaaaaaaaa"
        );
        assert_eq!(
            library.history_matching("CIÊNCIA").next().unwrap().video.id,
            "bbbbbbbbbbb"
        );
        assert_eq!(library.history_matching("não existe").count(), 0);
        assert_eq!(
            library
                .history_matching(" ")
                .map(|row| row.video.id.as_str())
                .collect::<Vec<_>>(),
            ["bbbbbbbbbbb", "aaaaaaaaaaa"]
        );
    }

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
                let watched = ImportedWatch {
                    video: Video::from_id(format!("{index:011}")),
                    at: index,
                    position: None,
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
                history,
                ..Import::default()
            }),
            (1, HISTORY_LIMIT - 1)
        );
        assert_eq!(library.history.len(), HISTORY_LIMIT);
        assert_eq!(library.history[0].at, 19_999);
        assert_eq!(library.history[0].position, 42.0);
        assert_eq!(library.history.last().unwrap().at, 19_000);
        assert!(library.watched("00000019999"));
        assert!(!library.watched("00000000000"));
    }

    #[test]
    fn import_counts_only_new_rows_that_stay_in_a_full_history() {
        let mut library = Library {
            history: (0..HISTORY_LIMIT)
                .map(|index| Watched {
                    video: video(&format!("{index:011}")),
                    at: index as i64,
                    position: 0.0,
                })
                .collect(),
            ..Default::default()
        };
        let old = ImportedWatch {
            video: video("older000001"),
            at: -1,
            position: None,
        };
        assert_eq!(
            library.merge(Import {
                history: vec![old.clone()],
                ..Import::default()
            }),
            (0, 0)
        );
        let recent = ImportedWatch {
            video: video("newer000001"),
            at: HISTORY_LIMIT as i64,
            position: None,
        };
        assert_eq!(
            library.merge(Import {
                history: vec![old, recent],
                ..Import::default()
            }),
            (0, 1)
        );
        assert_eq!(library.history.len(), HISTORY_LIMIT);
        assert!(library.watched("newer000001"));
        assert!(!library.watched("older000001"));
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
            .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../target").to_owned());
        std::fs::create_dir_all(&dir).unwrap();
        let path = Path::new(&dir).join(format!(
            "library-migrate-{}.json",
            youtfast_core::util::now()
        ));
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
                ImportedWatch {
                    video: video("aaaaaaaaaaa"),
                    at: 5,
                    position: None,
                },
                ImportedWatch {
                    video: video("bbbbbbbbbbb"),
                    at: 20,
                    position: None,
                },
            ],
            ..Import::default()
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
            history: vec![ImportedWatch {
                video: video("bbbbbbbbbbb"),
                at: 2,
                position: Some(30.0),
            }],
            ..Import::default()
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

    #[test]
    fn resume_positions_follow_the_index_and_the_stamp_follows_the_rows() {
        let mut library = Library::default();
        let stamp = library.history_stamp();
        library.record_watch(video("aaaaaaaaaaa"), 1);
        assert_ne!(library.history_stamp(), stamp);
        let stamp = library.history_stamp();
        library.set_position("aaaaaaaaaaa", 100.0, 600.0);
        assert_eq!(library.resume_position("aaaaaaaaaaa"), Some(100.0));
        library.mark_finished("aaaaaaaaaaa");
        assert_eq!(library.resume_position("aaaaaaaaaaa"), None);
        assert!(library.watched("aaaaaaaaaaa"));
        // A position is not a row: views keep what they cached.
        assert_eq!(library.history_stamp(), stamp);
        library.set_position("aaaaaaaaaaa", 100.0, 600.0);
        library.remove_history("aaaaaaaaaaa");
        assert_eq!(library.resume_position("aaaaaaaaaaa"), None);
        assert!(!library.watched("aaaaaaaaaaa"));
        assert_ne!(library.history_stamp(), stamp);
    }

    #[test]
    fn merging_watch_later_preserves_existing_deduplicates_and_respects_limit() {
        let mut library = Library::default();
        library.toggle_watch_later(video("existing001"));
        let import = Import {
            watch_later: (0..crate::backup::WATCH_LATER_LIMIT + 5)
                .map(|i| {
                    if i == 0 {
                        video("existing001")
                    } else {
                        video(&format!("wl{i:09}"))
                    }
                })
                .collect(),
            ..Import::default()
        };
        let (subs, hist, watch_later) = library.merge_with_watch_later(import);
        assert_eq!((subs, hist), (0, 0));
        assert_eq!(watch_later, crate::backup::WATCH_LATER_LIMIT - 1);
        assert_eq!(library.watch_later.len(), crate::backup::WATCH_LATER_LIMIT);
        assert_eq!(library.watch_later[0].id, "existing001");
        assert_eq!(library.watch_later[1].id, "wl000000001");
    }

    #[test]
    fn hidden_videos_and_blocked_channels_deduplicate_stay_bounded_and_round_trip() {
        let mut library = Library::default();
        assert!(!library.hide_video(String::new()));
        assert!(!library.clear_hidden_videos());
        assert!(!library.block_channel(channel("", "Empty")));
        assert!(!library.unblock_channel("UCmissing"));

        for i in 0..(HIDDEN_VIDEOS_LIMIT + 5) {
            assert!(library.hide_video(format!("vid{i:08}")));
        }
        assert!(!library.hide_video("vid00000010".into()));
        assert_eq!(library.hidden_videos.len(), HIDDEN_VIDEOS_LIMIT);
        // Oldest 5 (0..5) were dropped.
        for i in 0..5 {
            assert!(!library.is_video_hidden(&format!("vid{i:08}")));
        }
        assert!(library.is_video_hidden("vid00000005"));

        for i in 0..(BLOCKED_CHANNELS_LIMIT + 3) {
            assert!(library.block_channel(channel(&format!("UC_{i}"), &format!("Channel {i}"))));
        }
        assert!(!library.block_channel(channel("UC_10", "Channel 10")));
        assert_eq!(library.blocked_channels.len(), BLOCKED_CHANNELS_LIMIT);
        for i in 0..3 {
            assert!(!library.is_channel_blocked(&format!("UC_{i}")));
        }
        assert!(library.is_channel_blocked("UC_3"));

        let mut v_from_blocked = video("fresh000001");
        v_from_blocked.channel_id = Some("UC_3".into());
        assert!(library.is_hidden(&v_from_blocked));
        let v_hidden = video("vid00000005");
        assert!(library.is_hidden(&v_hidden));
        let mut v_clean = video("clean000001");
        v_clean.channel_id = Some("UC_clean".into());
        assert!(!library.is_hidden(&v_clean));

        // Round-trip through JSON keeps indexes synced.
        let json = serde_json::to_string(&library).unwrap();
        let mut reloaded: Library = serde_json::from_str(&json).unwrap();
        assert!(reloaded.is_video_hidden("vid00000005"));
        assert!(reloaded.is_channel_blocked("UC_3"));
        assert!(reloaded.is_hidden(&v_from_blocked));
        assert!(reloaded.unblock_channel("UC_3"));
        assert!(!reloaded.is_channel_blocked("UC_3"));
        assert!(!reloaded.is_hidden(&v_from_blocked));
        assert!(reloaded.clear_hidden_videos());
        assert!(!reloaded.is_video_hidden("vid00000005"));

        // Older library.json without hidden_videos / blocked_channels loads cleanly.
        let old: Library = serde_json::from_str(r#"{"subscriptions":[]}"#).unwrap();
        assert!(old.hidden_videos.is_empty());
        assert!(old.blocked_channels.is_empty());
    }

    #[test]
    fn load_normalizes_oversized_duplicate_filters_and_save_keeps_them_normalized() {
        let dir = std::env::var("CARGO_TARGET_TMPDIR")
            .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../target").to_owned());
        std::fs::create_dir_all(&dir).unwrap();
        let path =
            Path::new(&dir).join(format!("library-feed-filters-{}.json", std::process::id()));
        let hidden_videos: Vec<_> = (0..HIDDEN_VIDEOS_LIMIT + 3)
            .flat_map(|i| [format!("vid{i}"), format!("vid{i}")])
            .chain([String::new()])
            .collect();
        let blocked_channels: Vec<_> = (0..BLOCKED_CHANNELS_LIMIT + 3)
            .flat_map(|i| {
                let id = format!("UC_{i}");
                [channel(&id, "Older"), channel(&id, "Latest")]
            })
            .chain([channel("", "Empty")])
            .collect();
        let json = serde_json::json!({
            "hidden_videos": hidden_videos,
            "blocked_channels": blocked_channels,
        });
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();

        let library = Library::load(&path).unwrap();
        assert_eq!(library.hidden_videos().len(), HIDDEN_VIDEOS_LIMIT);
        assert_eq!(library.hidden_videos()[0], "vid3");
        assert_eq!(library.blocked_channels().len(), BLOCKED_CHANNELS_LIMIT);
        assert_eq!(library.blocked_channels()[0], channel("UC_3", "Latest"));
        assert!(library.is_video_hidden("vid3"));
        assert!(library.is_channel_blocked("UC_3"));
        assert!(!library.is_video_hidden("vid2"));
        assert!(!library.is_channel_blocked("UC_2"));
        assert!(!library.is_video_hidden(""));
        assert!(!library.is_channel_blocked(""));

        library.save(&path).unwrap();
        let reloaded = Library::load(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(reloaded.hidden_videos(), library.hidden_videos());
        assert_eq!(reloaded.blocked_channels(), library.blocked_channels());
        assert!(reloaded.is_video_hidden("vid3"));
        assert!(reloaded.is_channel_blocked("UC_3"));
    }
}
