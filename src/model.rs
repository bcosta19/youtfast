//! The app's own types. Views see these, never yt-dlp's JSON, rustypipe's
//! types or mpv's.

use serde::{Deserialize, Serialize};

/// A video as a search result, a feed entry, or the one being watched.
/// Also what the library stores, so new fields need `#[serde(default)]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Video {
    /// YouTube's 11-character id.
    pub id: String,
    /// Empty for a video opened from a link, until mpv reports the title.
    pub title: String,
    #[serde(default)]
    pub channel: Option<String>,
    /// The channel's `UC…` id, which its page and subscribing need.
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub channel_avatar: Option<String>,
    /// Seconds; `None` for live streams and when unknown (RSS has none).
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub views: Option<u64>,
    #[serde(default)]
    pub live: bool,
    /// When it was published, in Unix seconds, for ordering a feed.
    #[serde(default)]
    pub published_at: Option<i64>,
    /// YouTube's own wording ("2 days ago"), when it gave one.
    #[serde(default)]
    pub published: Option<String>,
    /// A Shorts-tab item (vertical, under a minute).
    #[serde(default)]
    pub short: bool,
}

impl Video {
    /// A video known only by its id, as from a pasted link.
    pub fn from_id(id: String) -> Self {
        Self {
            id,
            title: String::new(),
            channel: None,
            channel_id: None,
            channel_avatar: None,
            duration: None,
            views: None,
            live: false,
            published_at: None,
            published: None,
            short: false,
        }
    }

    pub fn url(&self) -> String {
        format!("https://www.youtube.com/watch?v={}", self.id)
    }

    /// The 320x180 thumbnail every video has.
    pub fn thumbnail(&self) -> String {
        format!("https://i.ytimg.com/vi/{}/mqdefault.jpg", self.id)
    }

    /// Fills what this one lacks from `other`, the same video described by
    /// another source (yt-dlp's answer for a video opened from a link).
    pub fn complete_from(&mut self, other: &Video) {
        if self.title.is_empty() {
            self.title.clone_from(&other.title);
        }
        self.channel = self.channel.take().or_else(|| other.channel.clone());
        self.channel_id = self.channel_id.take().or_else(|| other.channel_id.clone());
        self.channel_avatar = self
            .channel_avatar
            .take()
            .or_else(|| other.channel_avatar.clone());
        self.duration = self.duration.or(other.duration);
        self.views = self.views.or(other.views);
        self.published_at = self.published_at.or(other.published_at);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    /// YouTube's `UC…` id.
    pub id: String,
    pub name: String,
    /// The avatar's URL, when the source had one.
    #[serde(default)]
    pub avatar: Option<String>,
}

/// A playlist in the account's library.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaylistSummary {
    pub id: String,
    pub name: String,
    pub thumbnail: Option<String>,
    pub channel: Option<String>,
    pub videos: Option<u64>,
}

/// A single value being fetched, like [`Listing`] for a non-vec.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Fetch<T> {
    #[default]
    Idle,
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> Fetch<T> {
    /// The value when it is here.
    pub fn ready(&self) -> Option<&T> {
        match self {
            Fetch::Ready(value) => Some(value),
            _ => None,
        }
    }
}

/// A piece of a description or a comment: text, or a link that does
/// something inside the app or in the browser.
#[derive(Clone, Debug, PartialEq)]
pub enum Span {
    Text(String),
    /// A web link, opened in the browser.
    Link {
        text: String,
        url: String,
    },
    /// A `12:34` timestamp in the video being watched.
    Seek {
        text: String,
        seconds: u32,
    },
    /// A link to another video.
    Video {
        text: String,
        id: String,
    },
    /// A link to a channel.
    Channel {
        text: String,
        id: String,
    },
}

impl Span {
    /// What the span shows on screen.
    pub fn text(&self) -> &str {
        match self {
            Span::Text(text)
            | Span::Link { text, .. }
            | Span::Seek { text, .. }
            | Span::Video { text, .. }
            | Span::Channel { text, .. } => text,
        }
    }
}

/// A part of the video SponsorBlock's users marked.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    /// "sponsor", "selfpromo", "intro"…
    pub category: String,
    pub start: f64,
    pub end: f64,
}

impl Segment {
    /// Skipped as it starts; the other categories only offer a button.
    pub fn skipped_at_once(&self) -> bool {
        matches!(
            self.category.as_str(),
            "sponsor" | "selfpromo" | "interaction"
        )
    }

    /// The category as the skip button names it.
    pub fn label(&self) -> &'static str {
        match self.category.as_str() {
            "sponsor" => "sponsor",
            "selfpromo" => "self-promotion",
            "interaction" => "interaction reminder",
            "intro" => "intro",
            "outro" => "outro",
            "preview" => "preview",
            "music_offtopic" => "non-music section",
            "filler" => "filler",
            _ => "segment",
        }
    }
}

/// A chapter of the video: `[00:00] Intro`.
#[derive(Clone, Debug, PartialEq)]
pub struct Chapter {
    pub name: String,
    /// Where it starts, in seconds.
    pub seconds: u32,
}

/// What rustypipe knows about the video being watched.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Details {
    pub id: String,
    pub title: String,
    pub channel: Channel,
    /// The channel's subscriber count.
    pub subscribers: Option<u64>,
    pub views: u64,
    /// Hidden by the creator when `None`.
    pub likes: Option<u32>,
    /// YouTube's wording ("Aug 2, 2013").
    pub published: Option<String>,
    /// "2026-10-04", parsed from the date.
    pub published_date: Option<String>,
    pub live: bool,
    pub description: Vec<Span>,
    pub chapters: Vec<Chapter>,
    /// The recommended videos.
    pub related: Vec<Video>,
    /// The first comments page's continuation token.
    pub comments_token: Option<String>,
    /// How many comments YouTube reports.
    pub comment_count: Option<u64>,
    /// Needed to fetch comments.
    pub visitor_data: Option<String>,
}

/// A comment on the video, or a reply to one.
#[derive(Clone, Debug, PartialEq)]
pub struct CommentRow {
    pub id: String,
    pub author: Option<String>,
    /// The author's channel id, to open their page.
    pub author_id: Option<String>,
    /// The author's avatar, small.
    pub author_avatar: Option<String>,
    pub text: Vec<Span>,
    /// Image or animated GIF attached by the commenter.
    pub image: Option<String>,
    /// YouTube's wording ("3 days ago").
    pub published: String,
    pub likes: Option<u32>,
    pub replies: u32,
    /// The replies page's continuation token.
    pub replies_token: Option<String>,
    pub pinned: bool,
    /// Written by the channel's owner.
    pub by_owner: bool,
    /// Hearted by the channel's owner.
    pub hearted: bool,
}

/// The account's rating of a video.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rating {
    #[default]
    None,
    Like,
    Dislike,
}

/// A playlist of the account in the watch page's save menu, and whether
/// it holds the video.
#[derive(Clone, Debug, PartialEq)]
pub struct SaveTarget {
    /// `WL` for Watch later.
    pub id: String,
    pub name: String,
    pub saved: bool,
}

/// The tab a channel page shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ChannelTab {
    #[default]
    Videos,
    Shorts,
    Live,
    Playlists,
    About,
    /// The channel's own search; the text is the query.
    Search(String),
}

/// What a channel page's header shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelHeader {
    pub id: String,
    pub name: String,
    /// `@handle`
    pub handle: Option<String>,
    pub avatar: Option<String>,
    pub banner: Option<String>,
    pub subscribers: Option<u64>,
    pub videos: Option<u64>,
    /// Whether the channel has a Shorts tab.
    pub has_shorts: bool,
    /// Whether the channel has a Live tab.
    pub has_live: bool,
}

/// The channel's About tab.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelAbout {
    pub url: String,
    pub description: String,
    /// `(title, url)` links to elsewhere.
    pub links: Vec<(String, String)>,
    pub views: Option<u64>,
    /// "2013-08-02".
    pub created: Option<String>,
    pub country: Option<String>,
    pub subscribers: Option<u64>,
    pub videos: Option<u64>,
}

/// A line of history: the video, and when it was watched in words
/// ("Today", "Yesterday", "3 days ago").
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryRow {
    pub video: Video,
    pub when: Option<String>,
}

/// Something loaded from the network, as a view shows it.
#[derive(Clone, Debug, Default)]
pub enum Listing<T> {
    #[default]
    Idle,
    Loading,
    Ready(Vec<T>),
    Failed(String),
}

impl<T> Listing<T> {
    pub fn items(&self) -> &[T] {
        match self {
            Listing::Ready(items) => items,
            _ => &[],
        }
    }

    /// The loaded items mutably, to append a continuation.
    pub fn items_mut(&mut self) -> &mut Vec<T> {
        if !matches!(self, Listing::Ready(_)) {
            *self = Listing::Ready(Vec::new());
        }
        match self {
            Listing::Ready(items) => items,
            _ => unreachable!(),
        }
    }
}

/// Search results.
pub type Results = Listing<Video>;

/// How the account was signed in. Not a secret: the cookies themselves
/// live in the credential store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountSource {
    /// A cookies.txt file the user exported.
    File,
    /// The YouTube login of this browser ("chromium", "firefox"), read again
    /// whenever the copy the app holds expires.
    Browser(String),
}

impl AccountSource {
    pub fn label(&self) -> String {
        match self {
            AccountSource::File => "a cookies.txt file".to_owned(),
            AccountSource::Browser(browser) => browser_label(browser),
        }
    }
}

/// The browsers yt-dlp reads cookies from, by its names for them.
pub const BROWSERS: [&str; 7] = [
    "chromium", "chrome", "brave", "firefox", "vivaldi", "edge", "opera",
];

pub fn browser_label(spec: &str) -> String {
    let (browser, profile) = match spec.split_once(':') {
        Some((browser, profile)) => (browser, Some(profile)),
        None => (spec, None),
    };
    let name = match browser {
        "chromium" => "Chromium",
        "chrome" => "Google Chrome",
        "brave" => "Brave",
        "firefox" => "Firefox",
        "vivaldi" => "Vivaldi",
        "edge" => "Microsoft Edge",
        "opera" => "Opera",
        other => other,
    };
    // A profile given as a whole path shows as its folder.
    match profile.map(|profile| profile.rsplit('/').next().unwrap_or(profile)) {
        Some(profile) => format!("{name} ({profile})"),
        None => name.to_owned(),
    }
}

/// Where a page that exists both ways takes its list from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// The signed-in YouTube account.
    #[default]
    Account,
    /// What this app keeps on this computer.
    Device,
}

/// Home's chips: which of the feed's videos it shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FeedFilter {
    /// Recommendations rather than the subscriptions' uploads: the
    /// account's own, or made from the history here (`State::for_you`).
    #[default]
    ForYou,
    All,
    /// Published since local midnight.
    Today,
    /// Not in this computer's history.
    Unwatched,
    Live,
}

impl FeedFilter {
    pub const ALL: [FeedFilter; 5] = [
        FeedFilter::ForYou,
        FeedFilter::All,
        FeedFilter::Today,
        FeedFilter::Unwatched,
        FeedFilter::Live,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ForYou => "For you",
            Self::All => "All",
            Self::Today => "Today",
            Self::Unwatched => "Unwatched",
            Self::Live => "Live",
        }
    }

    /// Whether the video passes; `watched` says whether it is in the
    /// history.
    pub fn keeps(self, video: &Video, watched: bool, now: i64) -> bool {
        match self {
            FeedFilter::ForYou | FeedFilter::All => true,
            FeedFilter::Today => crate::util::age_bucket(video.published_at, now) == "today",
            FeedFilter::Unwatched => !watched,
            FeedFilter::Live => video.live,
        }
    }
}

/// The most a video may stream at. Remembered between runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    /// Up to [`Quality::AUTO`]: sharp on most screens, and light on the
    /// network and the decoder.
    #[default]
    Auto,
    /// Up to this resolution (1440 for 1440p).
    Max(u32),
}

impl Quality {
    pub const AUTO: u32 = 1080;

    /// The resolution streams are bounded by.
    pub fn limit(self) -> u32 {
        match self {
            Quality::Auto => Self::AUTO,
            Quality::Max(resolution) => resolution,
        }
    }
}

/// A resolution a video offers or plays at, by its smaller side as YouTube
/// labels it: 1080 for both 1920x1080 and a vertical 1080x1920.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub lines: u32,
    /// Frames per second, rounded.
    pub fps: u32,
}

impl Resolution {
    pub fn from_size(width: u32, height: u32, fps: u32) -> Self {
        Self {
            lines: width.min(height),
            fps,
        }
    }

    /// "1080p", or "1080p60" past 30 frames per second.
    pub fn label(self) -> String {
        if self.fps > 30 {
            format!("{}p{}", self.lines, self.fps)
        } else {
            format!("{}p", self.lines)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// The subscription feed.
    #[default]
    Home,
    Results,
    Watch,
    Subscriptions,
    History,
    WatchLater,
    /// The account's liked videos.
    Liked,
    /// The account's playlists.
    Playlists,
    /// One playlist's videos; `State::page` says which.
    Playlist,
    /// One channel's latest videos; `State::page` says which.
    Channel,
    /// New uploads since the last look.
    Notifications,
    Account,
    /// The choices between runs: the theme.
    Settings,
}

/// What a view asks the app to do. Views push these while drawing; the app
/// applies them after the frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    SetSearchOptions(crate::youtube::innertube::SearchOptions),
    SetChannelOrder(crate::youtube::innertube::ChannelOrder),
    SetHistoryQuery(String),
    RemoveHistory(String),
    PlayPlaylist,
    /// A search, or a YouTube link to open.
    Search(String),
    Open(Video),
    /// Goes back to where the current page was opened from.
    Back,
    Forward,
    Navigate(View),
    OpenChannel(Channel),
    OpenPlaylist {
        id: String,
        name: String,
    },
    /// Loads the page on screen again.
    Refresh,
    SetSource(Source),
    SetHistoryFilter(crate::logic::history::HistoryFilter),
    TogglePause,
    /// The desktop's media controls ask for play or pause directly.
    SetPaused(bool),
    /// Seconds from the start; `exact` once the seek bar is released.
    SeekTo {
        seconds: f64,
        exact: bool,
    },
    SeekBy(f64),
    /// 0 to 100.
    SetVolume(f64),
    ToggleFullscreen,
    ExitFullscreen,
    /// The watch page between its usual layout and theater mode.
    ToggleTheater,
    ToggleMiniPlayer,
    SetScrollMiniplayer(bool),
    SetPlaybackBuffer(crate::storage::settings::PlaybackBuffer),
    SetHistorySync(crate::storage::settings::HistorySync),
    SeekChapter(bool),
    RetryPlayback,
    ToggleLoop,
    MoveQueue {
        from: usize,
        to: usize,
    },
    SetCaptionScale(f64),
    SetCaptionBackground(bool),
    LoadExtras,
    ToggleTranscript,
    ToggleAudioOnly,
    SetQuality(Quality),
    SetTheme(crate::storage::settings::ThemeChoice),
    SetLanguage(crate::appearance::i18n::Locale),
    /// Subscribes on this device, or unsubscribes.
    ToggleSubscription(Channel),
    /// Adds to this device's Watch later, or removes.
    ToggleWatchLater(Video),
    ClearHistory,
    ImportTakeout,
    /// "sign in with cookies.txt" for the account being added.
    AddAccountFile,
    /// `sign in with <browser>` for the account being added, with the
    /// browser and profile typed under Advanced.
    AddAccountBrowser,
    /// A detected browser profile's `Sign in`, by its index in
    /// `State::browser_profiles`.
    AddAccountProfile(usize),
    /// The form's Advanced part opened or closed.
    ToggleAccountAdvanced,
    /// The account name/profile fields of the add form (view-owned).
    SetAccountName(String),
    SetAccountProfile(String),
    /// `[ switch ]` on an account row.
    SwitchAccount(u32),
    /// `[ rename ]` editing the name, then Enter saves.
    RenameAccount {
        id: u32,
        name: String,
    },
    /// `[ remove ]` after the inline confirm.
    RemoveAccount(u32),
    /// Open an account row's inline rename field / remove confirm.
    MarkRenaming {
        id: u32,
        name: String,
    },
    MarkRemoving(u32),
    /// Dismiss the remove confirm.
    CancelRemoving,
    /// `[ check now ]` on the Notifications page.
    CheckAlertsNow,
    /// `[ mark all seen ]` on the Notifications page.
    MarkAlertsSeen,
    /// The notifications settings pane's toggles.
    SetAlerts(bool),
    SetDesktopNotifications(bool),
    SetAlertsEvery(u32),
    /// `[ sign out ]`: the session stays kept for switching back.
    SignOut,
    SetBrowser(String),
    DismissNotice,
    /// The next page of comments.
    LoadMoreComments,
    /// Shows a comment's replies, loading them the first time; a second
    /// time hides them.
    ToggleReplies(String),
    /// The description's `[ more ]`/`[ less ]`.
    ToggleDescription,
    /// A long comment's `[ read more ]`/`[ show less ]`, by its id.
    ToggleCommentText(String),
    /// Rate the video being watched on the account.
    Rate(Rating),
    /// The save menu opened: ask for the account's playlists.
    LoadSaveTargets,
    /// Add the video being watched to an account playlist, or take it out.
    SetSaved {
        playlist: String,
        saved: bool,
    },
    /// The share menu's `Start at` box.
    SetShareAtTime(bool),
    /// The sidebar between open and its collapsed rail.
    ToggleSidebar,
    /// Home's chips.
    SetFeedFilter(FeedFilter),
    /// The watch page's `[ share ]`: the link to the clipboard.
    CopyLink(String),
    /// A link in a description or comment: opens in the browser. Only
    /// http/https are honoured.
    OpenUrl(String),
    /// Switch the channel page's tab.
    SetChannelTab(ChannelTab),
    /// The channel page's search field's text (a view-owned field).
    SetChannelQuery(String),
    /// The next page of the current channel tab's list.
    LoadMore,
    /// 0.25 to 2.
    SetSpeed(f64),
    ToggleMute,
    /// One frame ahead (`true`) or back, paused.
    FrameStep(bool),
    /// A part of the video from its start (0 to 1): the number keys.
    SeekFraction(f64),
    /// Shows the captions track (mpv's id) and keeps its language for the
    /// next videos; `None` turns captions off.
    SetSubtitle(Option<i64>),
    /// `c`: captions on in the language kept (or the first offered), or off.
    ToggleCaptions,
    /// A live stream back to its live edge.
    GoLive,
    /// The queue's next video, else Up next's, else the first related.
    PlayNext,
    /// The video before the one playing in Up next (a Short's ↑).
    PlayPrevious,
    /// Stops the countdown to the next video.
    CancelAutoplay,
    SetAutoplay(bool),
    /// Adds a video to the queue: at its end, or first (`next`).
    Enqueue {
        video: Video,
        next: bool,
    },
    /// Takes a video out of the queue, by its id.
    Dequeue(String),
    ClearQueue,
    SetSponsorBlock(bool),
    /// Jumps over the SponsorBlock segment being played.
    SkipSegment,
    /// The mini player's video, back on its watch page.
    ReturnToWatch,
    /// The mini player's close: pauses and hides it.
    CloseMiniPlayer,
}

#[cfg(test)]
mod tests {
    use super::{FeedFilter, Video};

    #[test]
    fn the_feed_chips_keep_what_they_say() {
        let now = crate::util::now();
        let mut fresh = Video::from_id("a".to_owned());
        fresh.published_at = Some(now);
        let mut old = Video::from_id("b".to_owned());
        old.published_at = Some(now - 30 * 86_400);
        let mut live = Video::from_id("c".to_owned());
        live.live = true;

        assert!(FeedFilter::All.keeps(&old, true, now));
        assert!(FeedFilter::Today.keeps(&fresh, false, now));
        assert!(!FeedFilter::Today.keeps(&old, false, now));
        assert!(!FeedFilter::Today.keeps(&live, false, now));
        assert!(FeedFilter::Unwatched.keeps(&old, false, now));
        assert!(!FeedFilter::Unwatched.keeps(&fresh, true, now));
        assert!(FeedFilter::Live.keeps(&live, false, now));
        assert!(!FeedFilter::Live.keeps(&fresh, false, now));
    }
}
