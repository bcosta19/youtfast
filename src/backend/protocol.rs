//! What the interface and the runtime say to each other: [`Command`]s
//! one way, [`Event`]s the other, and the list requests and answers they
//! carry.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::model::{
    Channel, ChannelAbout, ChannelHeader, HistoryRow, PlaylistSummary, Rating, SaveTarget, Video,
};
use crate::storage::library::{Account, Import, Library};
use crate::storage::settings::Settings;
use crate::youtube::innertube;

/// A file the user is choosing in the native dialog. Made on the interface
/// thread (macOS requires it there) and awaited on the runtime.
pub type Picked = Pin<Box<dyn Future<Output = Option<rfd::FileHandle>> + Send>>;

/// The list a request fills. One request per slot is current; an answer
/// to an older one is dropped (see `App::generations`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    Search,
    Feed,
    Subscriptions,
    History,
    WatchLater,
    Liked,
    Playlists,
    /// A playlist's or a channel's videos.
    Page,
    /// Home's shorts shelf.
    Shorts,
    /// The timer's new-upload check (same sources as the feed).
    Alerts,
    /// Home's For you.
    ForYou,
}

pub enum Request {
    Search {
        query: String,
        options: innertube::SearchOptions,
    },
    MoreSearch {
        more: innertube::More,
        kind: innertube::SearchKind,
    },
    /// The home feed: the account's when signed in, plus these channels'
    /// RSS.
    Feed {
        channels: Vec<Channel>,
        account: bool,
    },
    Subscriptions,
    History,
    WatchLater,
    Liked,
    Playlists,
    Playlist(String),
    /// A channel page's tab: its videos, shorts or live list, its
    /// playlists, or its search.
    ChannelTab {
        id: String,
        tab: crate::model::ChannelTab,
        order: innertube::ChannelOrder,
    },
    /// A channel page's About pane.
    ChannelAbout {
        id: String,
        needs_header: bool,
    },
    /// The next page of the channel tab's or a playlist's list.
    More {
        more: crate::youtube::innertube::More,
        /// `true` to continue the playlists tab, `false` a video list.
        playlists: bool,
    },
    /// The next page of the account's feed.
    MoreFeed(crate::youtube::innertube::More),
    /// The next page of the account's subscriptions.
    MoreSubscriptions(crate::youtube::innertube::More),
    /// The next page of the account's history.
    MoreHistory(crate::youtube::innertube::More),
    /// The next page of an account video list (Watch later, liked).
    MoreVideos(crate::youtube::innertube::More),
    /// The next page of the account's playlists.
    MorePlaylists(crate::youtube::innertube::More),
    /// The timer's check of the feed's sources.
    Alerts {
        channels: Vec<Channel>,
        account: bool,
    },
    /// The Shorts tabs of the feed's channels, for Home's shelf.
    Shorts {
        channels: Vec<String>,
    },
    /// Home's For you: the account's recommendations when `account` and
    /// it has some, else the seeds' related videos ranked by the taste.
    ForYou {
        account: bool,
        seeds: Vec<crate::logic::recommend::Seed>,
        taste: crate::logic::recommend::Taste,
    },
}

pub enum Payload {
    Search {
        items: PageItems,
        more: Option<innertube::More>,
    },
    PlaylistPage {
        details: innertube::PlaylistDetails,
        items: Vec<Video>,
        more: Option<innertube::More>,
    },
    Videos(Vec<Video>),
    /// The feed's first page and its continuation.
    Feed {
        videos: Vec<Video>,
        more: Option<crate::youtube::innertube::More>,
    },
    /// A video list's first page and its continuation (Watch later,
    /// liked videos, a playlist's videos).
    VideoList {
        items: Vec<Video>,
        more: Option<crate::youtube::innertube::More>,
    },
    /// The first page of the account's subscriptions and its continuation.
    Channels {
        items: Vec<Channel>,
        more: Option<crate::youtube::innertube::More>,
    },
    /// The first page of the account's history and its continuation.
    History {
        items: Vec<HistoryRow>,
        more: Option<crate::youtube::innertube::More>,
    },
    /// The first page of the account's playlists and its continuation.
    Playlists {
        items: Vec<PlaylistSummary>,
        more: Option<crate::youtube::innertube::More>,
    },
    /// A channel tab's first page: the header plus videos or playlists.
    ChannelPage {
        header: ChannelHeader,
        items: PageItems,
        more: Option<crate::youtube::innertube::More>,
    },
    /// A channel's About tab.
    About {
        header: Option<Box<ChannelHeader>>,
        about: ChannelAbout,
    },
    /// Home's For you, and whether it is the account's own.
    ForYou {
        videos: Vec<Video>,
        more: Option<crate::youtube::innertube::More>,
        from_account: bool,
    },
    /// A continued page of a paged list.
    More {
        items: PageItems,
        more: Option<crate::youtube::innertube::More>,
    },
}

/// What a paged list's rows are.
pub enum PageItems {
    Videos(Vec<Video>),
    Channels(Vec<Channel>),
    History(Vec<HistoryRow>),
    Playlists(Vec<PlaylistSummary>),
}

/// Which comment list a [`Command::LoadComments`] fills.
#[derive(Clone, Debug, PartialEq)]
pub enum CommentsKey {
    /// The video's own comments.
    Top,
    /// A comment's replies.
    Replies(String),
    /// Renew only the attachment URL, leaving the comment and paging intact.
    ImageRefresh { comment: String, old_url: String },
}

pub enum Command {
    /// Report one started playback in this session only.
    MarkWatched {
        id: String,
        session: u64,
    },
    LoadPlaylistMore {
        generation: u64,
        id: String,
        more: crate::youtube::innertube::More,
    },
    LoadExtras {
        generation: u64,
        id: String,
        language: String,
    },
    Load {
        slot: Slot,
        generation: u64,
        request: Request,
    },
    /// The watch page's metadata, description, chapters and the comments'
    /// first token.
    LoadDetails {
        generation: u64,
        id: String,
    },
    /// The alerts timer: 0 turns it off, a number of minutes restarts it.
    SetAlertsEvery(u32),
    /// Put new uploads on the desktop.
    Notify {
        videos: Vec<Video>,
        language: crate::appearance::i18n::Locale,
    },
    /// A page of comments or replies.
    LoadComments {
        generation: u64,
        key: CommentsKey,
        token: String,
        visitor_data: Option<String>,
    },
    ImportTakeout(Picked),
    MergeImport {
        library: Box<Library>,
        import: Import,
        revision: u64,
    },
    /// The account's rating of a video.
    LoadRating(String),
    /// Rate a video on the account (`Rating::None` clears it).
    Rate {
        id: String,
        rating: Rating,
        /// The rating shown before, to put back if YouTube refuses.
        previous: Rating,
    },
    /// The account's playlists, each with whether it holds the video.
    LoadSaveTargets(String),
    /// Add the video to an account playlist, or take it out.
    SetSaved {
        id: String,
        playlist: String,
        saved: bool,
    },
    /// A new account signs in: file or browser read first, then the
    /// session lands in the keyring under the account's id.
    AddAccount {
        account: Account,
        method: SignInMethod,
    },
    /// Sign back in as this account with its kept session.
    Switch(Account),
    /// Leave the current account; its session stays in the keyring.
    SignOut,
    /// Reset a failed session without clearing the user's selected account.
    ResetSession,
    /// Forget an account: its keyring entry is deleted too.
    RemoveAccount(u32),
    SaveLibrary(Library),
    SaveSettings(Settings),
    /// Thumbnail URLs the interface is drawing from the network, for the
    /// disk cache to keep next time.
    CacheThumbnails(Vec<(String, bool)>),
    /// What YouTube suggests for the search being typed.
    Suggest(String),
    /// The SponsorBlock segments of a video, by its id.
    LoadSegments(String),
    /// Check GitHub Releases for a newer version of YouTFast.
    CheckForUpdates {
        manual: bool,
    },
    /// Download, verify SHA-256 and install `release`.
    InstallUpdate {
        release: crate::desktop::updater::ReleaseInfo,
        auto: bool,
    },
    /// Show a desktop notification about a new or automatically installed version.
    NotifyUpdate {
        version: String,
        kind: crate::desktop::updater::UpdateKind,
        auto_installed: bool,
        language: crate::appearance::i18n::Locale,
    },
}

/// How a new account proves itself.
pub enum SignInMethod {
    /// A cookies.txt the user picked; the file is read on the backend.
    File(Picked),
    /// A browser's cookies, through yt-dlp (`browser` or
    /// `browser:profile`, already in the account's source).
    Browser,
}

pub enum Event {
    HistoryMarked,
    PlayingPlaylist {
        generation: u64,
        id: String,
        result: Result<(Vec<Video>, Option<crate::youtube::innertube::More>), String>,
    },
    Extras {
        generation: u64,
        result: Result<crate::youtube::extras::Extras, String>,
    },
    Loaded {
        slot: Slot,
        generation: u64,
        result: Result<Payload, String>,
    },
    Details {
        generation: u64,
        result: Box<Result<crate::model::Details, String>>,
    },
    Comments {
        generation: u64,
        key: CommentsKey,
        token: String,
        result: Result<(Vec<crate::model::CommentRow>, Option<String>), String>,
    },
    Imported(Result<Import, String>),
    ImportMerged {
        library: Box<Library>,
        counts: (usize, usize),
        revision: u64,
    },
    Rating {
        id: String,
        result: Result<Rating, String>,
    },
    /// A rating the account refused; the app puts `previous` back.
    RateFailed {
        id: String,
        previous: Rating,
        error: String,
    },
    RateSucceeded {
        id: String,
        rating: Rating,
    },
    SaveTargets {
        id: String,
        result: Result<Vec<SaveTarget>, String>,
    },
    /// A playlist change the account refused; the app undoes it.
    SaveFailed {
        id: String,
        playlist: String,
        saved: bool,
        error: String,
    },
    SaveSucceeded {
        id: String,
        playlist: String,
        saved: bool,
    },
    SigningIn,
    /// The account signed in, by its library id.
    SignedIn(u32),
    /// How mpv's yt-dlp reads the signed-in account's browser
    /// (`--cookies-from-browser`), so videos play signed in too.
    PlaybackCookies {
        account: u32,
        browser: String,
    },
    /// A new account's session was checked and stored; the app keeps it.
    AccountAdded(Account),
    SignInFailed(String),
    /// The kept session no longer works, and could not be renewed.
    SessionExpired(String),
    /// The kept session could not be checked; retry the selected account.
    SessionUnavailable {
        id: u32,
        error: String,
    },
    SignedOut,
    /// Something the user should know that belongs to no page.
    Notice(String),
    /// The alerts timer fired; the app sends the check request.
    CheckAlerts,
    Thumbnail {
        url: String,
        result: Result<Arc<[u8]>, crate::youtube::thumbs::Failure>,
    },
    Suggestions {
        query: String,
        items: Vec<String>,
    },
    Segments {
        id: String,
        segments: Vec<crate::model::Segment>,
    },
    UpdateChecked {
        result: Result<Option<crate::desktop::updater::ReleaseInfo>, String>,
        manual: bool,
    },
    UpdateInstalled {
        release: crate::desktop::updater::ReleaseInfo,
        auto: bool,
        result: Result<(), String>,
    },
}

impl Command {
    pub(super) fn account_write(&self) -> bool {
        matches!(
            self,
            Self::MarkWatched { .. } | Self::Rate { .. } | Self::SetSaved { .. }
        )
    }

    pub(super) fn changes_session(&self) -> bool {
        matches!(
            self,
            Self::AddAccount { .. } | Self::Switch(_) | Self::SignOut | Self::ResetSession
        )
    }

    pub(super) fn ordered(&self) -> bool {
        self.changes_session()
            || matches!(
                self,
                Self::RemoveAccount(_)
                    | Self::Load { .. }
                    | Self::SaveLibrary(_)
                    | Self::SaveSettings(_)
                    | Self::SetAlertsEvery(_)
            )
    }
}

impl Event {
    pub(super) fn sanitize(mut self) -> Self {
        match &mut self {
            Self::PlayingPlaylist {
                result: Err(error), ..
            } => *error = crate::safe_error(error),
            Self::Extras {
                result: Err(error), ..
            } => *error = crate::safe_error(error),
            Self::Loaded {
                result: Err(error), ..
            }
            | Self::Comments {
                result: Err(error), ..
            }
            | Self::Rating {
                result: Err(error), ..
            }
            | Self::SaveTargets {
                result: Err(error), ..
            }
            | Self::RateFailed { error, .. }
            | Self::SaveFailed { error, .. }
            | Self::SignInFailed(error)
            | Self::SessionExpired(error)
            | Self::SessionUnavailable { error, .. } => *error = crate::safe_error(error),
            Self::Details { result, .. } => {
                if let Err(error) = result.as_mut() {
                    *error = crate::safe_error(error);
                }
            }
            _ => {}
        }
        self
    }

    pub(super) fn current(&self, epoch: u64, current: u64) -> bool {
        epoch == current
            || matches!(
                self,
                Self::Imported(_)
                    | Self::ImportMerged { .. }
                    | Self::Notice(_)
                    | Self::CheckAlerts
                    | Self::Thumbnail { .. }
                    | Self::Suggestions { .. }
                    | Self::Segments { .. }
                    | Self::UpdateChecked { .. }
                    | Self::UpdateInstalled { .. }
            )
    }
}
