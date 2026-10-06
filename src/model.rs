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
}

impl Video {
    /// A video known only by its id, as from a pasted link.
    pub fn from_id(id: String) -> Self {
        Self {
            id,
            title: String::new(),
            channel: None,
            channel_id: None,
            duration: None,
            views: None,
            live: false,
            published_at: None,
            published: None,
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
        self.duration = self.duration.or(other.duration);
        self.views = self.views.or(other.views);
        self.published_at = self.published_at.or(other.published_at);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
            AccountSource::Browser(browser) => browser_label(browser).to_owned(),
        }
    }
}

/// The browsers yt-dlp reads cookies from, by its names for them.
pub const BROWSERS: [&str; 7] = [
    "chromium", "chrome", "brave", "firefox", "vivaldi", "edge", "opera",
];

pub fn browser_label(browser: &str) -> &str {
    match browser {
        "chromium" => "Chromium",
        "chrome" => "Google Chrome",
        "brave" => "Brave",
        "firefox" => "Firefox",
        "vivaldi" => "Vivaldi",
        "edge" => "Microsoft Edge",
        "opera" => "Opera",
        other => other,
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
    Account,
}

/// What a view asks the app to do. Views push these while drawing; the app
/// applies them after the frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// A search, or a YouTube link to open.
    Search(String),
    Open(Video),
    /// Goes back to where the current page was opened from.
    Back,
    Navigate(View),
    OpenChannel(Channel),
    OpenPlaylist {
        id: String,
        name: String,
    },
    /// Loads the page on screen again.
    Refresh,
    SetSource(Source),
    TogglePause,
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
    SetQuality(Quality),
    /// Subscribes on this device, or unsubscribes.
    ToggleSubscription(Channel),
    /// Adds to this device's Watch later, or removes.
    ToggleWatchLater(Video),
    ClearHistory,
    ImportTakeout,
    SignInWithFile,
    SignInWithBrowser(String),
    SignOut,
    SetBrowser(String),
    DismissNotice,
}
