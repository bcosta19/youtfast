//! What YouTFast keeps on this computer, with or without an account:
//! subscriptions, watch history (with where each video was left), and
//! Watch later. One JSON file, written atomically off the interface thread.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::sync::{SYNC_VERSION, SyncMetadata, SyncOutcome, SyncSnapshot};
use youtfast_core::model::{AccountSource, Channel, Video};

/// History keeps the most recent this many videos.
pub(crate) const HISTORY_LIMIT: usize = 1000;
/// Maximum number of hidden video ids kept in the local library.
pub const HIDDEN_VIDEOS_LIMIT: usize = 5_000;
/// Maximum number of blocked channels kept in the local library.
pub const BLOCKED_CHANNELS_LIMIT: usize = 1_000;
/// Maximum number of device-local playlists kept in `library.json`.
pub const PLAYLISTS_LIMIT: usize = 200;
/// Maximum number of videos kept in a single device-local playlist.
pub const PLAYLIST_VIDEOS_LIMIT: usize = 5_000;
/// Prefix distinguishing device-local playlist IDs from YouTube playlist IDs.
const LOCAL_PLAYLIST_PREFIX: &str = "local-";
/// A video left within this many seconds of its start or end starts over.
const RESUME_MARGIN: f64 = 15.0;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DevicePlaylist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub videos: Vec<Video>,
}

impl DevicePlaylist {
    /// Whether `id` names a playlist stored on this computer rather than on YouTube.
    pub fn is_local_id(id: &str) -> bool {
        id.starts_with(LOCAL_PLAYLIST_PREFIX)
    }

    /// Whether additional videos no longer fit in this playlist.
    pub fn is_full(&self) -> bool {
        self.videos.len() >= PLAYLIST_VIDEOS_LIMIT
    }

    /// Only globally unique playlist IDs are accepted on the sync wire.
    pub(crate) fn is_sync_id(id: &str) -> bool {
        id.strip_prefix("local-u").is_some_and(|suffix| {
            suffix.len() == 32
                && suffix
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
    }

    fn new_id() -> String {
        format!("{LOCAL_PLAYLIST_PREFIX}u{:032x}", rand::random::<u128>())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(from = "RawLibrary")]
pub struct Library {
    pub subscriptions: Vec<Channel>,
    /// Most recent first.
    pub history: Vec<Watched>,
    /// Most recently added first.
    pub watch_later: Vec<Video>,
    /// Copy-on-write so snapshots share the collection and its unchanged playlists.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    playlists: Arc<Vec<Arc<DevicePlaylist>>>,
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
    /// Per-item modification timestamps and tombstones for device sync.
    #[serde(default, skip_serializing_if = "SyncMetadata::is_empty")]
    pub(crate) sync: SyncMetadata,
    /// Whether loading assigned fresh IDs that must be saved before syncing.
    #[serde(skip)]
    migrated_playlist_ids: bool,
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
    sync: SyncMetadata,
    account: Option<AccountSource>,
    playlists: Vec<DevicePlaylist>,
}

impl From<RawLibrary> for Library {
    fn from(raw: RawLibrary) -> Self {
        let mut library = Self {
            subscriptions: raw.subscriptions,
            history: raw.history,
            watch_later: raw.watch_later,
            playlists: Arc::new(raw.playlists.into_iter().map(Arc::new).collect()),
            accounts: raw.accounts,
            active_account: raw.active_account,
            alerts_seen_at: raw.alerts_seen_at,
            alerts_notified_at: raw.alerts_notified_at,
            hidden_videos: raw.hidden_videos,
            blocked_channels: raw.blocked_channels,
            sync: raw.sync,
            migrated_playlist_ids: false,
            account: raw.account,
            watched_ids: HashMap::new(),
            history_stamp: 0,
            hidden_ids: HashSet::new(),
            blocked_ids: HashSet::new(),
        };
        library.reindex_watched();
        library.reindex_filters();
        library.sanitize_playlists();
        library.migrate_playlist_ids();
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
    pub playlists: Vec<DevicePlaylist>,
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
        if library.migrated_playlist_ids {
            // Persist the assigned identities before a peer can see them;
            // a crash before the first background save must not create copies.
            library.save(path)?;
            library.migrated_playlist_ids = false;
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
        self.sync.position_at.retain(|id, _| ids.contains_key(id));
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
        let now = youtfast_core::util::now();
        if let Some(removed) = take_first(&mut self.subscriptions, |known| known.id == channel.id) {
            self.sync.mark_unsubscribed(&removed.id, now);
            false
        } else {
            self.sync.mark_subscribed(&channel.id, now);
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
        let now = youtfast_core::util::now();
        if let Some(removed) = take_first(&mut self.watch_later, |known| known.id == video.id) {
            self.sync.mark_watch_later_removed(&removed.id, now);
            false
        } else {
            self.sync.mark_watch_later_added(&video.id, now);
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
        self.sync.mark_hidden(&id, youtfast_core::util::now());
        self.hidden_videos.push(id);
        if self.hidden_videos.len() > HIDDEN_VIDEOS_LIMIT {
            let excess = self.hidden_videos.len() - HIDDEN_VIDEOS_LIMIT;
            for dropped in self.hidden_videos.drain(..excess) {
                self.hidden_ids.remove(&dropped);
                self.sync.hidden_at.remove(&dropped);
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
        self.sync.bump_hidden_cleared(youtfast_core::util::now());
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
        self.sync
            .mark_blocked(&channel.id, youtfast_core::util::now());
        self.blocked_channels.push(channel);
        if self.blocked_channels.len() > BLOCKED_CHANNELS_LIMIT {
            let excess = self.blocked_channels.len() - BLOCKED_CHANNELS_LIMIT;
            for dropped in self.blocked_channels.drain(..excess) {
                self.blocked_ids.remove(&dropped.id);
                self.sync.blocked_at.remove(&dropped.id);
            }
        }
        true
    }

    /// Unblocks a channel by id. Returns whether the channel was blocked.
    pub fn unblock_channel(&mut self, channel_id: &str) -> bool {
        if !self.blocked_ids.remove(channel_id) {
            return false;
        }
        self.sync
            .mark_unblocked(channel_id, youtfast_core::util::now());
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

    /// Drops malformed or duplicate playlist entries and clamps to storage bounds.
    fn sanitize_playlists(&mut self) {
        let mut seen_ids = HashSet::new();
        let mut kept = 0;
        Arc::make_mut(&mut self.playlists).retain_mut(|playlist| {
            if kept >= PLAYLISTS_LIMIT
                || !DevicePlaylist::is_local_id(&playlist.id)
                || playlist.id.len() <= LOCAL_PLAYLIST_PREFIX.len()
            {
                return false;
            }
            let name = playlist.name.trim();
            if name.is_empty() || !seen_ids.insert(playlist.id.clone()) {
                return false;
            }
            let trimmed_name = (name.len() != playlist.name.len()).then(|| name.to_owned());
            let playlist = Arc::make_mut(playlist);
            if let Some(name) = trimmed_name {
                playlist.name = name;
            }
            let mut seen_videos = HashSet::new();
            playlist
                .videos
                .retain(|video| !video.id.is_empty() && seen_videos.insert(video.id.clone()));
            playlist.videos.truncate(PLAYLIST_VIDEOS_LIMIT);
            kept += 1;
            true
        });
    }

    /// Older IDs were unique only on the device that allocated them. Re-key
    /// each retained playlist without treating a number from another device
    /// as the same playlist. Legacy tombstones cannot name any wire ID.
    fn migrate_playlist_ids(&mut self) {
        for playlist in Arc::make_mut(&mut self.playlists) {
            if !DevicePlaylist::is_sync_id(&playlist.id) {
                let playlist = Arc::make_mut(playlist);
                let old_id = std::mem::replace(&mut playlist.id, DevicePlaylist::new_id());
                self.migrated_playlist_ids = true;
                if let Some(at) = self.sync.playlists_at.remove(&old_id) {
                    self.sync.playlists_at.insert(playlist.id.clone(), at);
                }
            }
        }
        self.sync
            .playlists_at
            .retain(|id, _| DevicePlaylist::is_sync_id(id));
        self.sync
            .deleted_playlists
            .retain(|id, _| DevicePlaylist::is_sync_id(id));
    }

    /// Whether another device playlist can be allocated.
    pub fn can_create_playlist(&self) -> bool {
        self.playlists.len() < PLAYLISTS_LIMIT
    }

    /// Creates a new device-local playlist and returns its `local-` ID.
    pub fn create_playlist(&mut self, name: &str) -> Option<String> {
        let name = name.trim();
        if name.is_empty() || self.playlists.len() >= PLAYLISTS_LIMIT {
            return None;
        }
        let id = DevicePlaylist::new_id();
        self.sync.touch_playlist(&id, youtfast_core::util::now());
        Arc::make_mut(&mut self.playlists).push(Arc::new(DevicePlaylist {
            id: id.clone(),
            name: name.to_owned(),
            videos: Vec::new(),
        }));
        Some(id)
    }

    /// Renames a device-local playlist. Returns `false` if missing or blank.
    pub fn rename_playlist(&mut self, id: &str, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        let Some(index) = self.playlist_index(id) else {
            return false;
        };
        self.playlist_mut(index).name = name.to_owned();
        self.sync.touch_playlist(id, youtfast_core::util::now());
        true
    }

    /// Removes a device-local playlist by ID.
    pub fn delete_playlist(&mut self, id: &str) -> bool {
        let Some(index) = self.playlist_index(id) else {
            return false;
        };
        Arc::make_mut(&mut self.playlists).remove(index);
        // Legacy libraries may not yet have a high-water mark.
        self.sync
            .mark_playlist_deleted(id, youtfast_core::util::now());
        true
    }

    /// Iterates over device-only playlists in creation order.
    pub fn playlists(&self) -> impl ExactSizeIterator<Item = &DevicePlaylist> {
        self.playlists.iter().map(Arc::as_ref)
    }

    /// Returns a device-local playlist by ID.
    pub fn playlist(&self, id: &str) -> Option<&DevicePlaylist> {
        self.playlists().find(|playlist| playlist.id == id)
    }

    fn playlist_index(&self, id: &str) -> Option<usize> {
        self.playlists().position(|playlist| playlist.id == id)
    }

    fn playlist_mut(&mut self, index: usize) -> &mut DevicePlaylist {
        Arc::make_mut(&mut Arc::make_mut(&mut self.playlists)[index])
    }

    /// Whether `video_id` is in the device-local playlist `playlist_id`.
    pub fn in_playlist(&self, playlist_id: &str, video_id: &str) -> bool {
        self.playlist(playlist_id)
            .is_some_and(|playlist| playlist.videos.iter().any(|video| video.id == video_id))
    }

    /// Appends `video` to `playlist_id`, deduplicating by `video.id` and respecting bounds.
    pub fn add_to_playlist(&mut self, playlist_id: &str, video: Video) -> bool {
        if video.id.is_empty() {
            return false;
        }
        let Some(index) = self.playlist_index(playlist_id) else {
            return false;
        };
        let playlist = &self.playlists[index];
        if playlist.is_full() || playlist.videos.iter().any(|known| known.id == video.id) {
            return false;
        }
        self.playlist_mut(index).videos.push(video);
        self.sync
            .touch_playlist(playlist_id, youtfast_core::util::now());
        true
    }

    /// Removes `video_id` from `playlist_id`.
    pub fn remove_from_playlist(&mut self, playlist_id: &str, video_id: &str) -> bool {
        let Some(index) = self.playlist_index(playlist_id) else {
            return false;
        };
        let Some(row) = self.playlists[index]
            .videos
            .iter()
            .position(|video| video.id == video_id)
        else {
            return false;
        };
        self.playlist_mut(index).videos.remove(row);
        self.sync
            .touch_playlist(playlist_id, youtfast_core::util::now());
        true
    }

    /// Ensures `video` is present (`saved = true`) or absent (`saved = false`) in `playlist_id`.
    pub fn set_in_playlist(&mut self, playlist_id: &str, video: Video, saved: bool) -> bool {
        if saved {
            self.add_to_playlist(playlist_id, video)
        } else {
            self.remove_from_playlist(playlist_id, &video.id)
        }
    }

    /// Reorders a video within `playlist_id` from index `from` to `to`.
    pub fn move_playlist_video(&mut self, playlist_id: &str, from: usize, to: usize) -> bool {
        let Some(index) = self.playlist_index(playlist_id) else {
            return false;
        };
        let len = self.playlists[index].videos.len();
        if from < len && to < len && from != to {
            let videos = &mut self.playlist_mut(index).videos;
            let video = videos.remove(from);
            videos.insert(to, video);
            self.sync
                .touch_playlist(playlist_id, youtfast_core::util::now());
            true
        } else {
            false
        }
    }

    /// Puts the video at the top of the history, keeping where it was left.
    pub fn record_watch(&mut self, video: Video, now: i64) {
        let at = self.sync.record_watch_timestamp(&video.id, now);
        self.sync.removed_history.remove(&video.id);
        let position = take_first(&mut self.history, |watched| watched.video.id == video.id)
            .map_or(0.0, |watched| watched.position);
        self.history.insert(
            0,
            Watched {
                video,
                at,
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
        let watched_at = watched.at;
        self.sync
            .touch_position(video_id, watched_at, youtfast_core::util::now());
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
            let watched_at = watched.at;
            self.sync
                .touch_position(video_id, watched_at, youtfast_core::util::now());
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
        for index in 0..self.playlists.len() {
            if let Some(row) = self.playlists[index]
                .videos
                .iter()
                .position(|entry| entry.id == video.id && entry != video)
            {
                self.playlist_mut(index).videos[row] = video.clone();
            }
        }
    }

    pub fn clear_history(&mut self) {
        let max_at = self.history.iter().map(|w| w.at).max();
        self.sync
            .bump_history_cleared(max_at, youtfast_core::util::now());
        self.history.clear();
        self.watched_ids.clear();
        self.touch_history();
    }

    /// Removes this video's watch and resume position while keeping the index in sync.
    pub fn remove_history(&mut self, id: &str) -> bool {
        let watched_at = self
            .history
            .iter()
            .find(|watched| watched.video.id == id)
            .map(|watched| watched.at);
        let before = self.history.len();
        self.history.retain(|watched| watched.video.id != id);
        self.watched_ids.remove(id);
        if before != self.history.len() {
            self.sync
                .mark_history_removed(id, watched_at, youtfast_core::util::now());
            self.touch_history();
            true
        } else {
            self.touch_history();
            false
        }
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
        let now = youtfast_core::util::now();
        let mut known_channels: HashSet<String> = self
            .subscriptions
            .iter()
            .map(|channel| channel.id.clone())
            .collect();
        let mut new_subscriptions = 0;
        for channel in import.subscriptions {
            if !channel.id.trim().is_empty() && known_channels.insert(channel.id.clone()) {
                self.sync.mark_subscribed(&channel.id, now);
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
            if self
                .sync
                .removed_history
                .get(&watched.video.id)
                .is_some_and(|&removed| watched.at > removed)
            {
                self.sync.removed_history.remove(&watched.video.id);
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
                        self.sync.touch_position(&known.video.id, watched.at, now);
                    }
                    known.at = known.at.max(watched.at);
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(self.history.len());
                    if let Some(pos) = position
                        && pos > 0.0
                    {
                        self.sync
                            .touch_position(&watched.video.id, watched.at, watched.at);
                    }
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
                self.sync.mark_watch_later_added(&video.id, now);
                self.watch_later.push(video);
                new_watch_later += 1;
            }
        }

        for playlist in import.playlists {
            if !DevicePlaylist::is_local_id(&playlist.id)
                || playlist.id.len() <= LOCAL_PLAYLIST_PREFIX.len()
            {
                continue;
            }
            let name = playlist.name.trim();
            if name.is_empty() {
                continue;
            }
            if let Some(index) = self.playlist_index(&playlist.id) {
                let mut known_ids: HashSet<String> = self.playlists[index]
                    .videos
                    .iter()
                    .map(|video| video.id.clone())
                    .collect();
                let mut added_any = false;
                for video in playlist.videos {
                    if self.playlists[index].is_full() {
                        break;
                    }
                    if !video.id.is_empty() && known_ids.insert(video.id.clone()) {
                        self.playlist_mut(index).videos.push(video);
                        added_any = true;
                    }
                }
                if added_any {
                    self.sync.touch_playlist(&playlist.id, now);
                }
            } else if self.playlists.len() < PLAYLISTS_LIMIT {
                let mut seen = HashSet::new();
                let mut videos = Vec::new();
                for video in playlist.videos {
                    if videos.len() >= PLAYLIST_VIDEOS_LIMIT {
                        break;
                    }
                    if !video.id.is_empty() && seen.insert(video.id.clone()) {
                        videos.push(video);
                    }
                }
                let id = if DevicePlaylist::is_sync_id(&playlist.id) {
                    playlist.id
                } else {
                    DevicePlaylist::new_id()
                };
                self.sync.touch_playlist(&id, now);
                Arc::make_mut(&mut self.playlists).push(Arc::new(DevicePlaylist {
                    id,
                    name: name.to_owned(),
                    videos,
                }));
            }
        }

        (new_subscriptions, new_history, new_watch_later)
    }

    /// Exports a portable device snapshot for P2P sync, omitting accounts,
    /// sessions, alert timestamps, and avatar URLs.
    pub fn sync_snapshot(&self, listen_port: u16) -> SyncSnapshot {
        let subscriptions = self
            .subscriptions
            .iter()
            .map(|channel| Channel {
                id: channel.id.clone(),
                name: channel.name.clone(),
                avatar: None,
            })
            .collect();
        let history = self
            .history
            .iter()
            .map(|watched| {
                let mut video = watched.video.clone();
                video.channel_avatar = None;
                Watched {
                    video,
                    at: watched.at,
                    position: watched.position,
                }
            })
            .collect();
        let watch_later = self
            .watch_later
            .iter()
            .map(|video| {
                let mut video = video.clone();
                video.channel_avatar = None;
                video
            })
            .collect();
        let playlists = self
            .playlists()
            .map(|playlist| DevicePlaylist {
                id: playlist.id.clone(),
                name: playlist.name.clone(),
                videos: playlist
                    .videos
                    .iter()
                    .map(|video| {
                        let mut video = video.clone();
                        video.channel_avatar = None;
                        video
                    })
                    .collect(),
            })
            .collect();
        let blocked_channels = self
            .blocked_channels
            .iter()
            .map(|channel| Channel {
                id: channel.id.clone(),
                name: channel.name.clone(),
                avatar: None,
            })
            .collect();
        SyncSnapshot {
            version: SYNC_VERSION,
            listen_port,
            peers: Vec::new(),
            subscriptions,
            history,
            watch_later,
            playlists,
            hidden_videos: self.hidden_videos.clone(),
            blocked_channels,
            sync: self.sync.clone(),
        }
        .sanitized()
    }

    /// Commutative, idempotent Last-Write-Wins merge of an incoming device snapshot.
    pub fn merge_sync(&mut self, incoming: SyncSnapshot) -> SyncOutcome {
        let incoming = incoming.sanitized();

        let old_subs = self.subscriptions.clone();
        let old_hist = self.history.clone();
        let old_wl = self.watch_later.clone();
        let old_hidden = self.hidden_videos.clone();
        let old_blocked = self.blocked_channels.clone();
        let old_playlists: Vec<DevicePlaylist> = self.playlists().cloned().collect();
        let old_sync = self.sync.clone();

        // 1. Subscriptions
        let (new_subs, new_sub_at, new_unsub_at) = merge_channel_set(
            &self.subscriptions,
            &self.sync.subscribed_at,
            &self.sync.unsubscribed_at,
            &incoming.subscriptions,
            &incoming.sync.subscribed_at,
            &incoming.sync.unsubscribed_at,
            true,
            crate::backup::SUBSCRIPTIONS_LIMIT,
        );
        self.subscriptions = new_subs;
        self.sync.subscribed_at = new_sub_at;
        self.sync.unsubscribed_at = new_unsub_at;

        // 2. Watch later
        let (new_wl, new_wl_at, new_wl_rem) = merge_watch_later_set(
            &self.watch_later,
            &self.sync.watch_later_at,
            &self.sync.removed_watch_later,
            &incoming.watch_later,
            &incoming.sync.watch_later_at,
            &incoming.sync.removed_watch_later,
        );
        self.watch_later = new_wl;
        self.sync.watch_later_at = new_wl_at;
        self.sync.removed_watch_later = new_wl_rem;

        // 3. History & resume positions
        let merged_history =
            merge_history_set(&self.history, &self.sync, &incoming.history, &incoming.sync);
        self.history = merged_history.history;
        self.sync.position_at = merged_history.position_at;
        self.sync.removed_history = merged_history.removed;
        self.sync.history_cleared_at = merged_history.cleared_at;

        // 4. Device-local playlists
        let merged_playlists = merge_playlist_set(
            self.playlists(),
            &self.sync,
            &incoming.playlists,
            &incoming.sync,
        );
        self.playlists = Arc::new(
            merged_playlists
                .playlists
                .into_iter()
                .map(Arc::new)
                .collect(),
        );
        self.sync.playlists_at = merged_playlists.updated_at;
        self.sync.deleted_playlists = merged_playlists.deleted;

        // 5. Hidden videos
        let (new_hidden, new_hidden_at, new_hidden_cleared) = merge_hidden_set(
            &self.hidden_videos,
            &self.sync.hidden_at,
            self.sync.hidden_cleared_at,
            &incoming.hidden_videos,
            &incoming.sync.hidden_at,
            incoming.sync.hidden_cleared_at,
        );
        self.hidden_videos = new_hidden;
        self.sync.hidden_at = new_hidden_at;
        self.sync.hidden_cleared_at = new_hidden_cleared;

        // 6. Blocked channels
        let (new_blocked, new_blocked_at, new_unblocked_at) = merge_channel_set(
            &self.blocked_channels,
            &self.sync.blocked_at,
            &self.sync.unblocked_at,
            &incoming.blocked_channels,
            &incoming.sync.blocked_at,
            &incoming.sync.unblocked_at,
            false,
            BLOCKED_CHANNELS_LIMIT,
        );
        self.blocked_channels = new_blocked;
        self.sync.blocked_at = new_blocked_at;
        self.sync.unblocked_at = new_unblocked_at;

        let subscriptions_changed = self.subscriptions != old_subs;
        let history_changed = self.history != old_hist;
        let watch_later_changed = self.watch_later != old_wl;
        let filters_changed =
            self.hidden_videos != old_hidden || self.blocked_channels != old_blocked;
        let playlists_changed = self.playlists.len() != old_playlists.len()
            || self
                .playlists()
                .zip(old_playlists.iter())
                .any(|(a, b)| a != b);

        if history_changed {
            self.reindex_watched();
        }
        if filters_changed {
            self.hidden_ids = self.hidden_videos.iter().cloned().collect();
            self.blocked_ids = self.blocked_channels.iter().map(|c| c.id.clone()).collect();
        }

        let changed = subscriptions_changed
            || history_changed
            || watch_later_changed
            || playlists_changed
            || filters_changed
            || self.sync != old_sync;

        SyncOutcome {
            changed,
            history_changed,
            subscriptions_changed,
            filters_changed,
            playlists_changed,
        }
    }
}

fn max_opt(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

fn lww_alive(added: Option<i64>, removed: Option<i64>) -> bool {
    match (added, removed) {
        (Some(a), Some(r)) => a > r,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

fn merge_video_fields(preferred: &mut Video, fallback: &Video) {
    preferred.complete_from(fallback);
    if preferred.channel_avatar.is_none() {
        preferred
            .channel_avatar
            .clone_from(&fallback.channel_avatar);
    }
}

#[allow(clippy::too_many_arguments)]
fn merge_channel_set(
    local_list: &[Channel],
    local_added: &HashMap<String, i64>,
    local_removed: &HashMap<String, i64>,
    inc_list: &[Channel],
    inc_added: &HashMap<String, i64>,
    inc_removed: &HashMap<String, i64>,
    sort_by_name: bool,
    limit: usize,
) -> (Vec<Channel>, HashMap<String, i64>, HashMap<String, i64>) {
    let local_map: HashMap<&str, (usize, &Channel)> = local_list
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), (i, c)))
        .collect();
    let inc_map: HashMap<&str, (usize, &Channel)> = inc_list
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), (i, c)))
        .collect();
    let mut ids: HashSet<&str> = HashSet::new();
    ids.extend(local_map.keys().copied());
    ids.extend(inc_map.keys().copied());
    ids.extend(local_removed.keys().map(String::as_str));
    ids.extend(inc_removed.keys().map(String::as_str));

    struct Entry {
        channel: Channel,
        add_ts: i64,
        local_idx: usize,
        inc_idx: usize,
    }

    let mut entries = Vec::new();
    let mut new_added = HashMap::new();
    let mut new_removed = HashMap::new();

    for id in ids {
        if id.trim().is_empty() {
            continue;
        }
        let local_item = local_map.get(id).copied();
        let inc_item = inc_map.get(id).copied();
        let local_add = local_item.map(|_| local_added.get(id).copied().unwrap_or(0));
        let inc_add = inc_item.map(|_| inc_added.get(id).copied().unwrap_or(0));
        let best_add = max_opt(local_add, inc_add);
        let best_rem = max_opt(local_removed.get(id).copied(), inc_removed.get(id).copied());
        if lww_alive(best_add, best_rem) {
            let add_ts = best_add.unwrap_or(0);
            let (channel, local_idx, inc_idx) = match (local_item, inc_item) {
                (Some((li, lch)), Some((ii, ich))) => {
                    let mut chosen = if inc_add > local_add && !ich.name.trim().is_empty() {
                        ich.clone()
                    } else {
                        lch.clone()
                    };
                    if chosen.name.trim().is_empty() && !ich.name.trim().is_empty() {
                        chosen.name.clone_from(&ich.name);
                    }
                    if chosen.avatar.is_none() {
                        chosen.avatar.clone_from(&lch.avatar);
                    }
                    (chosen, li, ii)
                }
                (Some((li, lch)), None) => (lch.clone(), li, usize::MAX),
                (None, Some((ii, ich))) => (ich.clone(), usize::MAX, ii),
                (None, None) => unreachable!(),
            };
            entries.push(Entry {
                channel,
                add_ts,
                local_idx,
                inc_idx,
            });
        } else if let Some(rem) = best_rem {
            new_removed.insert(id.to_owned(), rem);
        }
    }

    if sort_by_name {
        entries.sort_by(|a, b| {
            a.channel
                .name
                .to_lowercase()
                .cmp(&b.channel.name.to_lowercase())
                .then_with(|| a.channel.id.cmp(&b.channel.id))
        });
        entries.truncate(limit);
    } else {
        entries.sort_by(|a, b| {
            a.add_ts
                .cmp(&b.add_ts)
                .then_with(|| a.local_idx.cmp(&b.local_idx))
                .then_with(|| a.inc_idx.cmp(&b.inc_idx))
                .then_with(|| a.channel.id.cmp(&b.channel.id))
        });
        if entries.len() > limit {
            entries.drain(..entries.len() - limit);
        }
    }

    let mut channels = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.add_ts > 0 {
            new_added.insert(entry.channel.id.clone(), entry.add_ts);
        }
        channels.push(entry.channel);
    }
    crate::sync::bound_tombstones(&mut new_removed, crate::sync::TOMBSTONES_LIMIT);
    (channels, new_added, new_removed)
}

fn merge_watch_later_set(
    local_list: &[Video],
    local_added: &HashMap<String, i64>,
    local_removed: &HashMap<String, i64>,
    inc_list: &[Video],
    inc_added: &HashMap<String, i64>,
    inc_removed: &HashMap<String, i64>,
) -> (Vec<Video>, HashMap<String, i64>, HashMap<String, i64>) {
    let local_map: HashMap<&str, (usize, &Video)> = local_list
        .iter()
        .enumerate()
        .map(|(i, v)| (v.id.as_str(), (i, v)))
        .collect();
    let inc_map: HashMap<&str, (usize, &Video)> = inc_list
        .iter()
        .enumerate()
        .map(|(i, v)| (v.id.as_str(), (i, v)))
        .collect();
    let mut ids: HashSet<&str> = HashSet::new();
    ids.extend(local_map.keys().copied());
    ids.extend(inc_map.keys().copied());
    ids.extend(local_removed.keys().map(String::as_str));
    ids.extend(inc_removed.keys().map(String::as_str));

    struct Entry {
        video: Video,
        add_ts: i64,
        local_idx: usize,
        inc_idx: usize,
    }

    let mut entries = Vec::new();
    let mut new_added = HashMap::new();
    let mut new_removed = HashMap::new();

    for id in ids {
        if id.trim().is_empty() {
            continue;
        }
        let local_item = local_map.get(id).copied();
        let inc_item = inc_map.get(id).copied();
        let local_add = local_item.map(|_| local_added.get(id).copied().unwrap_or(0));
        let inc_add = inc_item.map(|_| inc_added.get(id).copied().unwrap_or(0));
        let best_add = max_opt(local_add, inc_add);
        let best_rem = max_opt(local_removed.get(id).copied(), inc_removed.get(id).copied());
        if lww_alive(best_add, best_rem) {
            let add_ts = best_add.unwrap_or(0);
            let (video, local_idx, inc_idx) = match (local_item, inc_item) {
                (Some((li, lv)), Some((ii, iv))) => {
                    let mut merged = if inc_add > local_add {
                        let mut v = iv.clone();
                        merge_video_fields(&mut v, lv);
                        v
                    } else {
                        let mut v = lv.clone();
                        merge_video_fields(&mut v, iv);
                        v
                    };
                    if merged.channel_avatar.is_none() {
                        merged.channel_avatar.clone_from(&lv.channel_avatar);
                    }
                    (merged, li, ii)
                }
                (Some((li, lv)), None) => (lv.clone(), li, usize::MAX),
                (None, Some((ii, iv))) => (iv.clone(), usize::MAX, ii),
                (None, None) => unreachable!(),
            };
            entries.push(Entry {
                video,
                add_ts,
                local_idx,
                inc_idx,
            });
        } else if let Some(rem) = best_rem {
            new_removed.insert(id.to_owned(), rem);
        }
    }

    entries.sort_by(|a, b| {
        b.add_ts
            .cmp(&a.add_ts)
            .then_with(|| a.local_idx.cmp(&b.local_idx))
            .then_with(|| a.inc_idx.cmp(&b.inc_idx))
            .then_with(|| a.video.id.cmp(&b.video.id))
    });
    entries.truncate(crate::backup::WATCH_LATER_LIMIT);

    let mut videos = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.add_ts > 0 {
            new_added.insert(entry.video.id.clone(), entry.add_ts);
        }
        videos.push(entry.video);
    }
    crate::sync::bound_tombstones(&mut new_removed, crate::sync::TOMBSTONES_LIMIT);
    (videos, new_added, new_removed)
}

struct MergedHistory {
    history: Vec<Watched>,
    position_at: HashMap<String, i64>,
    removed: HashMap<String, i64>,
    cleared_at: Option<i64>,
}

fn merge_history_set(
    local_history: &[Watched],
    local_sync: &SyncMetadata,
    inc_history: &[Watched],
    inc_sync: &SyncMetadata,
) -> MergedHistory {
    let self_cleared = local_sync.history_cleared_at;
    let inc_cleared = inc_sync.history_cleared_at;
    let merged_cleared = max_opt(self_cleared, inc_cleared);

    let local_map: HashMap<&str, &Watched> = local_history
        .iter()
        .map(|w| (w.video.id.as_str(), w))
        .collect();
    let inc_map: HashMap<&str, &Watched> = inc_history
        .iter()
        .map(|w| (w.video.id.as_str(), w))
        .collect();

    let mut ids: HashSet<&str> = HashSet::new();
    ids.extend(local_map.keys().copied());
    ids.extend(inc_map.keys().copied());
    ids.extend(local_sync.removed_history.keys().map(String::as_str));
    ids.extend(inc_sync.removed_history.keys().map(String::as_str));

    let mut entries: Vec<(Watched, Option<i64>)> = Vec::new();
    let mut new_removed = HashMap::new();

    for id in ids {
        if id.trim().is_empty() {
            continue;
        }
        let rem_at = max_opt(
            local_sync.removed_history.get(id).copied(),
            inc_sync.removed_history.get(id).copied(),
        );
        let local_kept = local_map
            .get(id)
            .copied()
            .filter(|w| inc_cleared.is_none_or(|c| w.at > c) && rem_at.is_none_or(|r| w.at > r));
        let inc_kept = inc_map
            .get(id)
            .copied()
            .filter(|w| self_cleared.is_none_or(|c| w.at > c) && rem_at.is_none_or(|r| w.at > r));

        match (local_kept, inc_kept) {
            (Some(lw), Some(iw)) => {
                let at = lw.at.max(iw.at);
                let mut video = if iw.at > lw.at {
                    let mut v = iw.video.clone();
                    merge_video_fields(&mut v, &lw.video);
                    v
                } else {
                    let mut v = lw.video.clone();
                    merge_video_fields(&mut v, &iw.video);
                    v
                };
                if video.channel_avatar.is_none() {
                    video.channel_avatar.clone_from(&lw.video.channel_avatar);
                }
                let l_pos_explicit = local_sync.position_at.get(id).copied();
                let i_pos_explicit = inc_sync.position_at.get(id).copied();
                let l_pos_at = l_pos_explicit.unwrap_or(lw.at);
                let i_pos_at = i_pos_explicit.unwrap_or(iw.at);
                let (position, pos_ts) = if i_pos_at > l_pos_at {
                    (iw.position, i_pos_explicit.or(Some(i_pos_at)))
                } else if l_pos_at > i_pos_at {
                    (lw.position, l_pos_explicit.or(Some(l_pos_at)))
                } else {
                    (
                        lw.position.max(iw.position),
                        max_opt(l_pos_explicit, i_pos_explicit),
                    )
                };
                entries.push((
                    Watched {
                        video,
                        at,
                        position,
                    },
                    pos_ts,
                ));
            }
            (Some(lw), None) => {
                entries.push((lw.clone(), local_sync.position_at.get(id).copied()));
            }
            (None, Some(iw)) => {
                entries.push((iw.clone(), inc_sync.position_at.get(id).copied()));
            }
            (None, None) => {
                if let Some(rem) = rem_at
                    && merged_cleared.is_none_or(|c| rem > c)
                {
                    new_removed.insert(id.to_owned(), rem);
                }
            }
        }
    }

    entries.sort_by(|a, b| {
        b.0.at
            .cmp(&a.0.at)
            .then_with(|| a.0.video.id.cmp(&b.0.video.id))
    });
    entries.truncate(HISTORY_LIMIT);

    let mut history = Vec::with_capacity(entries.len());
    let mut new_pos_at = HashMap::new();
    for (watched, pos_ts) in entries {
        if let Some(ts) = pos_ts {
            new_pos_at.insert(watched.video.id.clone(), ts);
        }
        history.push(watched);
    }
    crate::sync::bound_tombstones(&mut new_removed, crate::sync::TOMBSTONES_LIMIT);
    MergedHistory {
        history,
        position_at: new_pos_at,
        removed: new_removed,
        cleared_at: merged_cleared,
    }
}

struct MergedPlaylists {
    playlists: Vec<DevicePlaylist>,
    updated_at: HashMap<String, i64>,
    deleted: HashMap<String, i64>,
}

fn merge_playlist_set<'a>(
    local_playlists: impl Iterator<Item = &'a DevicePlaylist>,
    local_sync: &SyncMetadata,
    inc_playlists: &[DevicePlaylist],
    inc_sync: &SyncMetadata,
) -> MergedPlaylists {
    let local_vec: Vec<&DevicePlaylist> = local_playlists.collect();
    let local_map: HashMap<&str, (usize, &DevicePlaylist)> = local_vec
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.as_str(), (i, *p)))
        .collect();
    let inc_map: HashMap<&str, (usize, &DevicePlaylist)> = inc_playlists
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.as_str(), (i, p)))
        .collect();

    let mut ids: HashSet<&str> = HashSet::new();
    ids.extend(local_map.keys().copied());
    ids.extend(inc_map.keys().copied());
    ids.extend(local_sync.deleted_playlists.keys().map(String::as_str));
    ids.extend(inc_sync.deleted_playlists.keys().map(String::as_str));

    struct Entry {
        playlist: DevicePlaylist,
        ts: i64,
        local_idx: usize,
        inc_idx: usize,
    }

    let mut entries = Vec::new();
    let mut new_deleted = HashMap::new();

    for id in ids {
        if !DevicePlaylist::is_sync_id(id) {
            continue;
        }
        let local_item = local_map.get(id).copied();
        let inc_item = inc_map.get(id).copied();
        let local_at = local_item.map(|_| local_sync.playlists_at.get(id).copied().unwrap_or(0));
        let inc_at = inc_item.map(|_| inc_sync.playlists_at.get(id).copied().unwrap_or(0));
        let best_at = max_opt(local_at, inc_at);
        let del_at = max_opt(
            local_sync.deleted_playlists.get(id).copied(),
            inc_sync.deleted_playlists.get(id).copied(),
        );
        if lww_alive(best_at, del_at) {
            let ts = best_at.unwrap_or(0);
            let (playlist, local_idx, inc_idx) = match (local_item, inc_item) {
                (Some((li, lp)), Some((ii, ip))) => {
                    let take_inc = inc_at > local_at
                        || (inc_at == local_at && ip.videos.len() > lp.videos.len());
                    let mut chosen = if take_inc { ip.clone() } else { lp.clone() };
                    if take_inc {
                        let local_avatars: HashMap<&str, &str> = lp
                            .videos
                            .iter()
                            .filter_map(|v| {
                                v.channel_avatar.as_deref().map(|av| (v.id.as_str(), av))
                            })
                            .collect();
                        for video in &mut chosen.videos {
                            if video.channel_avatar.is_none()
                                && let Some(&av) = local_avatars.get(video.id.as_str())
                            {
                                video.channel_avatar = Some(av.to_owned());
                            }
                        }
                    }
                    (chosen, li, ii)
                }
                (Some((li, lp)), None) => (lp.clone(), li, usize::MAX),
                (None, Some((ii, ip))) => (ip.clone(), usize::MAX, ii),
                (None, None) => unreachable!(),
            };
            entries.push(Entry {
                playlist,
                ts,
                local_idx,
                inc_idx,
            });
        } else if let Some(del) = del_at {
            new_deleted.insert(id.to_owned(), del);
        }
    }

    entries.sort_by(|a, b| {
        a.local_idx
            .cmp(&b.local_idx)
            .then_with(|| a.ts.cmp(&b.ts))
            .then_with(|| a.inc_idx.cmp(&b.inc_idx))
            .then_with(|| a.playlist.id.cmp(&b.playlist.id))
    });
    entries.truncate(PLAYLISTS_LIMIT);

    let mut playlists = Vec::with_capacity(entries.len());
    let mut new_at = HashMap::new();
    for entry in entries {
        if entry.ts > 0 {
            new_at.insert(entry.playlist.id.clone(), entry.ts);
        }
        playlists.push(entry.playlist);
    }
    crate::sync::bound_tombstones(&mut new_deleted, crate::sync::TOMBSTONES_LIMIT);
    MergedPlaylists {
        playlists,
        updated_at: new_at,
        deleted: new_deleted,
    }
}

fn merge_hidden_set(
    local_hidden: &[String],
    local_at: &HashMap<String, i64>,
    self_cleared: Option<i64>,
    inc_hidden: &[String],
    inc_at: &HashMap<String, i64>,
    inc_cleared: Option<i64>,
) -> (Vec<String>, HashMap<String, i64>, Option<i64>) {
    let merged_cleared = max_opt(self_cleared, inc_cleared);
    let mut candidates: HashMap<String, (i64, usize, usize)> = HashMap::new();

    for (idx, id) in local_hidden.iter().enumerate() {
        let at = local_at.get(id).copied().unwrap_or(0);
        if inc_cleared.is_some_and(|c| at <= c) {
            continue;
        }
        candidates.insert(id.clone(), (at, idx, usize::MAX));
    }
    for (idx, id) in inc_hidden.iter().enumerate() {
        let at = inc_at.get(id).copied().unwrap_or(0);
        if self_cleared.is_some_and(|c| at <= c) {
            continue;
        }
        candidates
            .entry(id.clone())
            .and_modify(|(best_at, _, inc_idx)| {
                *best_at = (*best_at).max(at);
                *inc_idx = idx;
            })
            .or_insert((at, usize::MAX, idx));
    }

    let mut rows: Vec<(String, i64, usize, usize)> = candidates
        .into_iter()
        .map(|(id, (at, li, ii))| (id, at, li, ii))
        .collect();
    rows.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.3.cmp(&b.3))
            .then_with(|| a.0.cmp(&b.0))
    });
    if rows.len() > HIDDEN_VIDEOS_LIMIT {
        rows.drain(..rows.len() - HIDDEN_VIDEOS_LIMIT);
    }

    let mut out = Vec::with_capacity(rows.len());
    let mut new_at = HashMap::new();
    for (id, at, _, _) in rows {
        if at > 0 {
            new_at.insert(id.clone(), at);
        }
        out.push(id);
    }
    (out, new_at, merged_cleared)
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
                playlists: Vec::new(),
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
            subscriptions: Vec::from([channel("UC1", "Alpha"), channel("UC2", "Beta")]),
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
            playlists: Vec::new(),
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
            playlists: Vec::new(),
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

    #[test]
    fn device_playlists_crud_reorder_and_deduplication() {
        let mut library = Library::default();
        assert_eq!(library.create_playlist("   "), None);
        let first = library.create_playlist("  Rust Talks  ").unwrap();
        let second = library.create_playlist("Systems").unwrap();
        assert!(DevicePlaylist::is_local_id(&first));
        assert!(DevicePlaylist::is_local_id(&second));
        assert_ne!(first, second);
        assert!(!DevicePlaylist::is_local_id("PL123456"));
        assert_eq!(library.playlist(&first).unwrap().name, "Rust Talks");

        assert!(library.rename_playlist(&first, "  Rust Deep Dives  "));
        assert!(!library.rename_playlist(&first, "   "));
        assert!(!library.rename_playlist("local-999", "Missing"));
        assert_eq!(library.playlist(&first).unwrap().name, "Rust Deep Dives");

        let v1 = video("vid00000001");
        let v2 = video("vid00000002");
        let v3 = video("vid00000003");
        assert!(library.add_to_playlist(&first, v1.clone()));
        assert!(!library.add_to_playlist(&first, v1.clone())); // deduplicates
        assert!(library.add_to_playlist(&first, v2.clone()));
        assert!(library.set_in_playlist(&first, v3.clone(), true));
        assert!(library.in_playlist(&first, "vid00000001"));
        assert!(library.in_playlist(&first, "vid00000003"));

        assert!(library.move_playlist_video(&first, 2, 0));
        assert!(!library.move_playlist_video(&first, 0, 0));
        assert!(!library.move_playlist_video(&first, 9, 0));
        let order: Vec<&str> = library
            .playlist(&first)
            .unwrap()
            .videos
            .iter()
            .map(|v| v.id.as_str())
            .collect();
        assert_eq!(order, ["vid00000003", "vid00000001", "vid00000002"]);

        assert!(library.set_in_playlist(&first, v1, false));
        assert!(!library.in_playlist(&first, "vid00000001"));
        assert!(library.remove_from_playlist(&first, "vid00000003"));
        assert!(!library.remove_from_playlist(&first, "vid00000003"));
        assert_eq!(library.playlist(&first).unwrap().videos.len(), 1);

        assert!(library.delete_playlist(&first));
        assert!(!library.delete_playlist(&first));
        assert!(library.playlist(&first).is_none());
        // New IDs remain unique even after deleting an earlier playlist.
        let third = library.create_playlist("After delete").unwrap();
        assert_ne!(third, second);
    }

    #[test]
    fn deleted_playlist_ids_are_not_reused_even_after_reload() {
        let mut library: Library =
            serde_json::from_str(r#"{"playlists":[{"id":"local-12","name":"Legacy"}]}"#).unwrap();
        let id = library.playlists().next().unwrap().id.clone();
        assert!(DevicePlaylist::is_sync_id(&id));
        assert!(library.delete_playlist(&id));
        let mut restored: Library =
            serde_json::from_slice(&serde_json::to_vec(&library).unwrap()).unwrap();
        assert_ne!(restored.create_playlist("After reload").unwrap(), id);
        assert!(restored.playlist(&id).is_none());
        assert!(restored.sync.deleted_playlists.contains_key(&id));
    }

    #[test]
    fn independent_new_and_legacy_playlists_survive_first_sync() {
        let mut laptop: Library = serde_json::from_str(
            r#"{
            "playlists":[{"id":"local-1","name":"Laptop legacy"}],
            "sync":{"playlists_at":{"local-1":10},"deleted_playlists":{"local-2":20}}
        }"#,
        )
        .unwrap();
        let mut desktop: Library = serde_json::from_str(
            r#"{
            "playlists":[{"id":"local-1","name":"Desktop legacy"}],
            "playlist_high_water":18446744073709551615
        }"#,
        )
        .unwrap();
        let legacy = laptop.playlists().next().unwrap().id.clone();
        assert!(DevicePlaylist::is_sync_id(&legacy));
        assert_eq!(laptop.sync.playlists_at[&legacy], 10);
        assert!(laptop.sync.deleted_playlists.is_empty());
        let persisted: Library =
            serde_json::from_str(&serde_json::to_string(&laptop).unwrap()).unwrap();
        assert!(persisted.playlist(&legacy).is_some());
        let laptop_new = laptop.create_playlist("Laptop new").unwrap();
        let desktop_new = desktop.create_playlist("Desktop new").unwrap();
        laptop.add_to_playlist(&laptop_new, video("aaaaaaaaaaa"));
        desktop.add_to_playlist(&desktop_new, video("bbbbbbbbbbb"));
        laptop.merge_sync(desktop.sync_snapshot(38452));
        desktop.merge_sync(laptop.sync_snapshot(38452));
        for peer in [&laptop, &desktop] {
            assert_eq!(peer.playlists().count(), 4);
            assert!(peer.in_playlist(&laptop_new, "aaaaaaaaaaa"));
            assert!(peer.in_playlist(&desktop_new, "bbbbbbbbbbb"));
        }
    }

    #[test]
    fn device_playlists_enforce_bounds_and_sanitize_on_load() {
        let mut library = Library::default();
        for i in 0..PLAYLISTS_LIMIT {
            assert!(
                library.create_playlist(&format!("Playlist {i}")).is_some(),
                "playlist {i} should fit"
            );
        }
        assert!(!library.can_create_playlist());
        assert_eq!(library.playlists.len(), PLAYLISTS_LIMIT);
        assert_eq!(library.create_playlist("Overflow"), None);

        let first_id = library.playlists[0].id.clone();
        for i in 0..PLAYLIST_VIDEOS_LIMIT {
            assert!(library.add_to_playlist(&first_id, video(&format!("{i:011}"))));
        }
        assert_eq!(
            library.playlist(&first_id).unwrap().videos.len(),
            PLAYLIST_VIDEOS_LIMIT
        );
        assert!(!library.add_to_playlist(&first_id, video("overflowvid")));
        assert!(library.playlist(&first_id).unwrap().is_full());
        assert!(library.remove_from_playlist(&first_id, "00000000000"));
        assert!(!library.playlist(&first_id).unwrap().is_full());

        // Legacy library JSON without `playlists` loads cleanly.
        let legacy: Library =
            serde_json::from_str(r#"{"subscriptions":[],"history":[],"watch_later":[]}"#).unwrap();
        assert!(legacy.playlists.is_empty());

        // Oversized or invalid playlists on disk are sanitized and clamped on load.
        let dir = std::env::var("CARGO_TARGET_TMPDIR")
            .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../target").to_owned());
        std::fs::create_dir_all(&dir).unwrap();
        let path = Path::new(&dir).join(format!(
            "library-playlists-sanitize-{}.json",
            youtfast_core::util::now()
        ));
        let mut raw_playlists = vec![
            DevicePlaylist {
                id: "PL_not_local".into(),
                name: "Invalid prefix".into(),
                videos: vec![video("aaaaaaaaaaa")],
            },
            DevicePlaylist {
                id: "local-".into(),
                name: "Empty suffix".into(),
                videos: vec![video("aaaaaaaaaaa")],
            },
            DevicePlaylist {
                id: "local-1".into(),
                name: "   ".into(),
                videos: vec![video("aaaaaaaaaaa")],
            },
        ];
        let mut oversized_videos: Vec<Video> = (0..PLAYLIST_VIDEOS_LIMIT + 10)
            .map(|i| video(&format!("v{i:010}")))
            .collect();
        oversized_videos.insert(1, oversized_videos[0].clone()); // duplicate
        oversized_videos.insert(2, Video::from_id(String::new())); // empty id
        raw_playlists.push(DevicePlaylist {
            id: "local-1".into(),
            name: "  Valid Kept  ".into(),
            videos: oversized_videos,
        });
        raw_playlists.push(DevicePlaylist {
            id: "local-1".into(),
            name: "Duplicate ID".into(),
            videos: vec![video("bbbbbbbbbbb")],
        });
        for i in 2..(PLAYLISTS_LIMIT + 5) {
            raw_playlists.push(DevicePlaylist {
                id: format!("local-{i}"),
                name: format!("List {i}"),
                videos: Vec::new(),
            });
        }
        let disk_lib = Library {
            playlists: Arc::new(raw_playlists.into_iter().map(Arc::new).collect()),
            ..Library::default()
        };
        disk_lib.save(&path).unwrap();
        let loaded = Library::load(&path).unwrap();
        assert_eq!(loaded.playlists.len(), PLAYLISTS_LIMIT);
        assert!(DevicePlaylist::is_sync_id(&loaded.playlists[0].id));
        assert_eq!(loaded.playlists[0].name, "Valid Kept");
        assert_eq!(loaded.playlists[0].videos.len(), PLAYLIST_VIDEOS_LIMIT);
        let loaded_again = Library::load(&path).unwrap();
        assert_eq!(loaded_again.playlists[0].id, loaded.playlists[0].id);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn playback_snapshots_share_unchanged_device_playlists() {
        let mut library = Library::default();
        let first = library.create_playlist("First").unwrap();
        let second = library.create_playlist("Second").unwrap();
        let mut saved = video("aaaaaaaaaaa");
        library.add_to_playlist(&first, saved.clone());
        library.add_to_playlist(&second, video("bbbbbbbbbbb"));
        library.record_watch(saved.clone(), 1);

        let snapshot = library.clone();
        library.set_position(&saved.id, 10.0, 100.0);
        library.update_video(&saved);
        assert!(Arc::ptr_eq(&snapshot.playlists, &library.playlists));

        saved.title = "Updated title".into();
        library.update_video(&saved);
        assert!(!Arc::ptr_eq(&snapshot.playlists, &library.playlists));
        assert!(!std::ptr::eq(
            snapshot.playlist(&first).unwrap(),
            library.playlist(&first).unwrap(),
        ));
        assert!(std::ptr::eq(
            snapshot.playlist(&second).unwrap(),
            library.playlist(&second).unwrap(),
        ));
        assert_ne!(snapshot.playlist(&first).unwrap().videos[0], saved);
        assert_eq!(library.playlist(&first).unwrap().videos[0], saved);
    }

    #[test]
    fn merging_import_combines_device_playlists() {
        let mut library = Library::default();
        let id = library.create_playlist("Local").unwrap();
        library.add_to_playlist(&id, video("aaaaaaaaaaa"));
        library.merge(Import {
            subscriptions: Vec::new(),
            history: Vec::new(),
            watch_later: Vec::new(),
            playlists: vec![
                DevicePlaylist {
                    id: id.clone(),
                    name: "Local".into(),
                    videos: vec![video("aaaaaaaaaaa"), video("bbbbbbbbbbb")],
                },
                DevicePlaylist {
                    id: "local-2".into(),
                    name: "Imported".into(),
                    videos: vec![video("ccccccccccc")],
                },
                DevicePlaylist {
                    id: "PLyoutube".into(),
                    name: "Ignored".into(),
                    videos: vec![video("ddddddddddd")],
                },
            ],
        });
        assert_eq!(library.playlists.len(), 2);
        assert_eq!(library.playlist(&id).unwrap().videos.len(), 2);
        let imported = library.playlists().find(|p| p.name == "Imported").unwrap();
        assert!(DevicePlaylist::is_sync_id(&imported.id));
        assert_eq!(imported.videos.len(), 1);
    }

    #[test]
    fn sync_snapshot_strips_accounts_alerts_and_avatars() {
        let mut library = Library::default();
        let mut ch = channel("UC1", "Channel One");
        ch.avatar = Some("https://private.invalid/avatar.png".into());
        library.toggle_subscription(ch);
        let mut v = video("vid00000001");
        v.channel_avatar = Some("https://private.invalid/v-avatar.png".into());
        library.record_watch(v.clone(), 100);
        library.toggle_watch_later(v.clone());
        let pl = library.create_playlist("List").unwrap();
        library.add_to_playlist(&pl, v);
        library.accounts.push(Account {
            id: 1,
            name: "Secret".into(),
            source: AccountSource::Browser("firefox:secret".into()),
        });
        library.active_account = Some(1);
        library.alerts_seen_at = Some(500);
        library.alerts_notified_at = Some(600);

        let snap = library.sync_snapshot(38452);
        let bytes = crate::sync::to_sync_json_bytes(&snap).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(!text.contains("accounts"));
        assert!(!text.contains("active_account"));
        assert!(!text.contains("alerts_seen_at"));
        assert!(!text.contains("firefox:secret"));
        assert!(!text.contains("private.invalid"));
        let parsed = crate::sync::parse_sync_bytes(&bytes).unwrap();
        assert_eq!(parsed.listen_port, 38452);
        assert_eq!(parsed.subscriptions.len(), 1);
        assert_eq!(parsed.subscriptions[0].avatar, None);
        assert_eq!(parsed.history[0].video.channel_avatar, None);
        assert_eq!(parsed.watch_later[0].channel_avatar, None);
        assert_eq!(parsed.playlists[0].videos[0].channel_avatar, None);
    }

    #[test]
    fn merge_sync_is_commutative_idempotent_and_syncs_resume_finished_and_deletions() {
        let mut laptop = Library::default();
        let mut desktop = Library::default();

        let mut ch1 = channel("UC_keep", "Keep");
        ch1.avatar = Some("https://example.invalid/keep.png".into());
        let ch2 = channel("UC_drop", "Drop");
        laptop.toggle_subscription(ch1.clone());
        laptop.toggle_subscription(ch2.clone());
        laptop.sync.subscribed_at.insert("UC_keep".into(), 10);
        laptop.sync.subscribed_at.insert("UC_drop".into(), 10);

        // Laptop watches video A and pauses at 120s.
        let mut vid_a = video("aaaaaaaaaaa");
        vid_a.duration = Some(600.0);
        laptop.record_watch(vid_a.clone(), 100);
        laptop.set_position(&vid_a.id, 120.0, 600.0);
        laptop.sync.position_at.insert(vid_a.id.clone(), 110);

        // Laptop adds Watch later and a device playlist.
        laptop.toggle_watch_later(video("wl000000001"));
        laptop.sync.watch_later_at.insert("wl000000001".into(), 10);
        let pl_id = laptop.create_playlist("Sync List").unwrap();
        laptop.add_to_playlist(&pl_id, video("pl000000001"));
        laptop.sync.playlists_at.insert(pl_id.clone(), 10);
        laptop.hide_video("hidden00001".into());
        laptop.sync.hidden_at.insert("hidden00001".into(), 10);
        laptop.block_channel(channel("UC_bad", "Bad"));
        laptop.sync.blocked_at.insert("UC_bad".into(), 10);

        // First sync: desktop receives everything from laptop.
        let outcome = desktop.merge_sync(laptop.sync_snapshot(38452));
        assert!(outcome.changed);
        assert!(outcome.history_changed);
        assert!(outcome.subscriptions_changed);
        assert!(outcome.filters_changed);
        assert!(outcome.playlists_changed);
        assert!(desktop.watched(&vid_a.id));
        assert_eq!(desktop.resume_position(&vid_a.id), Some(120.0));
        assert!(desktop.is_subscribed("UC_keep"));
        assert!(desktop.is_subscribed("UC_drop"));
        assert!(desktop.in_watch_later("wl000000001"));
        assert!(desktop.in_playlist(&pl_id, "pl000000001"));
        assert!(desktop.is_video_hidden("hidden00001"));
        assert!(desktop.is_channel_blocked("UC_bad"));

        // Second identical sync is idempotent and preserves laptop avatars.
        let repeat = desktop.merge_sync(laptop.sync_snapshot(38452));
        assert!(!repeat.changed);

        // Desktop finishes video A, unsubscribes UC_drop, removes wl000000001,
        // renames playlist, and unblocks UC_bad at newer timestamps.
        desktop.mark_finished(&vid_a.id);
        desktop.sync.position_at.insert(vid_a.id.clone(), 200);
        desktop.toggle_subscription(ch2);
        desktop.sync.unsubscribed_at.insert("UC_drop".into(), 200);
        desktop.toggle_watch_later(video("wl000000001"));
        desktop
            .sync
            .removed_watch_later
            .insert("wl000000001".into(), 200);
        desktop.rename_playlist(&pl_id, "Renamed Sync List");
        desktop.sync.playlists_at.insert(pl_id.clone(), 200);
        desktop.unblock_channel("UC_bad");
        desktop.sync.unblocked_at.insert("UC_bad".into(), 200);

        // Laptop merges desktop's updates.
        let back = laptop.merge_sync(desktop.sync_snapshot(38452));
        assert!(back.changed);
        assert!(back.history_changed);
        assert_eq!(laptop.resume_position(&vid_a.id), None);
        assert!(laptop.watched(&vid_a.id));
        assert!(laptop.is_subscribed("UC_keep"));
        assert_eq!(
            laptop.subscriptions[0].avatar.as_deref(),
            Some("https://example.invalid/keep.png")
        );
        assert!(!laptop.is_subscribed("UC_drop"));
        assert!(!laptop.in_watch_later("wl000000001"));
        assert_eq!(laptop.playlist(&pl_id).unwrap().name, "Renamed Sync List");
        assert!(!laptop.is_channel_blocked("UC_bad"));
    }

    #[test]
    fn a_same_tick_rewatch_survives_clear_and_removal_on_the_peer() {
        // Simulate a source clock ahead of the current wall clock, so both
        // tombstones advance past the supplied watch time deterministically.
        let now = 4_000_000_000;
        let watched = video("aaaaaaaaaaa");
        for clear_all in [false, true] {
            let mut source = Library::default();
            let mut peer = Library::default();
            source.record_watch(watched.clone(), now);
            if clear_all {
                source.clear_history();
            } else {
                assert!(source.remove_history(&watched.id));
            }
            peer.merge_sync(source.sync_snapshot(38452));
            assert!(!peer.watched(&watched.id));
            source.record_watch(watched.clone(), now);
            peer.merge_sync(source.sync_snapshot(38452));
            assert!(peer.watched(&watched.id));
        }
    }

    #[test]
    fn resume_sync_metadata_does_not_outlive_retained_history() {
        let mut library = Library::default();
        for i in 0..=HISTORY_LIMIT {
            let id = format!("{i:011}");
            library.record_watch(Video::from_id(id.clone()), i as i64);
            library.set_position(&id, 60.0, 300.0);
        }
        let evicted = "00000000000";
        assert!(!library.watched(evicted));
        assert!(!library.sync.position_at.contains_key(evicted));
        assert_eq!(library.sync.position_at.len(), HISTORY_LIMIT);

        // Clean old files that already contain orphaned resume metadata.
        library.sync.position_at.insert(evicted.into(), 100);
        let round_trip: Library =
            serde_json::from_str(&serde_json::to_string(&library).unwrap()).unwrap();
        assert!(!round_trip.sync.position_at.contains_key(evicted));
    }

    #[test]
    fn merge_sync_handles_history_removals_clear_and_rewatch() {
        let mut a = Library::default();
        let mut b = Library::default();
        a.record_watch(video("aaaaaaaaaaa"), 100);
        a.record_watch(video("bbbbbbbbbbb"), 120);
        b.merge_sync(a.sync_snapshot(38452));

        // Remove one video on B at 200.
        b.remove_history("aaaaaaaaaaa");
        b.sync.removed_history.insert("aaaaaaaaaaa".into(), 200);
        a.merge_sync(b.sync_snapshot(38452));
        assert!(!a.watched("aaaaaaaaaaa"));
        assert!(a.watched("bbbbbbbbbbb"));

        // Re-watching on A at 300 revives it on both.
        a.record_watch(video("aaaaaaaaaaa"), 300);
        b.merge_sync(a.sync_snapshot(38452));
        assert!(b.watched("aaaaaaaaaaa"));

        // Clearing history on A at 400 clears entries <= 400 on B, while keeping newer watch at 500.
        b.record_watch(video("ccccccccccc"), 500);
        a.clear_history();
        a.sync.history_cleared_at = Some(400);
        b.merge_sync(a.sync_snapshot(38452));
        assert!(!b.watched("aaaaaaaaaaa"));
        assert!(!b.watched("bbbbbbbbbbb"));
        assert!(b.watched("ccccccccccc"));

        // Commutativity check in reverse order.
        a.merge_sync(b.sync_snapshot(38452));
        assert_eq!(a.history.len(), 1);
        assert_eq!(a.history[0].video.id, "ccccccccccc");
    }
}
