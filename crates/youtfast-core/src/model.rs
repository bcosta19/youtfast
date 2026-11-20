//! The app's own types. Views see these, never yt-dlp's JSON, InnerTube's
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
        const HEAD: &str = "https://i.ytimg.com/vi/";
        const TAIL: &str = "/mqdefault.jpg";
        let mut url = String::with_capacity(HEAD.len() + self.id.len() + TAIL.len());
        url.push_str(HEAD);
        url.push_str(&self.id);
        url.push_str(TAIL);
        url
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

/// Extra channel metadata learned in memory from InnerTube responses.
///
/// Kept outside [`Channel`] so stored subscriptions and Takeout merges stay
/// compact while the Subscriptions and search views can render richer cards.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChannelMeta {
    pub handle: Option<String>,
    pub subscribers: Option<u64>,
    pub videos: Option<u64>,
    pub description: Option<String>,
    pub verified: bool,
}

impl ChannelMeta {
    /// Merges newly parsed metadata into `self` without erasing richer
    /// fields learned from channel pages or search results.
    pub fn merge_from(&mut self, incoming: Self) {
        if incoming.handle.is_some() {
            self.handle = incoming.handle;
        }
        if incoming.subscribers.is_some() {
            self.subscribers = incoming.subscribers;
        }
        if incoming.videos.is_some() {
            self.videos = incoming.videos;
        }
        if incoming
            .description
            .as_deref()
            .is_some_and(|text| !text.trim().is_empty())
        {
            self.description = incoming.description;
        }
        self.verified |= incoming.verified;
    }

    pub fn is_empty(&self) -> bool {
        self.handle.is_none()
            && self.subscribers.is_none()
            && self.videos.is_none()
            && self.description.is_none()
            && !self.verified
    }
}

/// Sort order for the Subscriptions grid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SubscriptionSort {
    #[default]
    Name,
    Recent,
    Watched,
}

impl SubscriptionSort {
    pub const ALL: [Self; 3] = [Self::Name, Self::Recent, Self::Watched];
}

#[cfg(test)]
mod channel_meta_tests {
    use super::ChannelMeta;

    #[test]
    fn channel_meta_merge_preserves_existing_fields_when_incoming_is_sparse() {
        let mut meta = ChannelMeta {
            handle: Some("@Computerphile".into()),
            subscribers: Some(2_640_000),
            videos: Some(850),
            description: Some("Videos about computers and stuff.".into()),
            verified: true,
        };
        meta.merge_from(ChannelMeta {
            handle: None,
            subscribers: Some(2_650_000),
            videos: None,
            description: Some("   ".into()),
            verified: false,
        });
        assert_eq!(meta.handle.as_deref(), Some("@Computerphile"));
        assert_eq!(meta.subscribers, Some(2_650_000));
        assert_eq!(meta.videos, Some(850));
        assert_eq!(
            meta.description.as_deref(),
            Some("Videos about computers and stuff.")
        );
        assert!(meta.verified);
    }
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

/// A SponsorBlock segment category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SponsorCategory {
    Sponsor,
    Selfpromo,
    Interaction,
    Intro,
    Outro,
    Preview,
    MusicOfftopic,
    Filler,
}

impl SponsorCategory {
    /// All categories in canonical display order.
    pub const ALL: [Self; 8] = [
        Self::Sponsor,
        Self::Selfpromo,
        Self::Interaction,
        Self::Intro,
        Self::Outro,
        Self::Preview,
        Self::MusicOfftopic,
        Self::Filler,
    ];

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "sponsor" => Some(Self::Sponsor),
            "selfpromo" => Some(Self::Selfpromo),
            "interaction" => Some(Self::Interaction),
            "intro" => Some(Self::Intro),
            "outro" => Some(Self::Outro),
            "preview" => Some(Self::Preview),
            "music_offtopic" => Some(Self::MusicOfftopic),
            "filler" => Some(Self::Filler),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sponsor => "sponsor",
            Self::Selfpromo => "selfpromo",
            Self::Interaction => "interaction",
            Self::Intro => "intro",
            Self::Outro => "outro",
            Self::Preview => "preview",
            Self::MusicOfftopic => "music_offtopic",
            Self::Filler => "filler",
        }
    }

    /// Default behavior matching YouTFast's original SponsorBlock behavior.
    pub fn default_behavior(self) -> SponsorBehavior {
        match self {
            Self::Sponsor | Self::Selfpromo | Self::Interaction => SponsorBehavior::AutoSkip,
            Self::Intro | Self::Outro | Self::Preview | Self::MusicOfftopic | Self::Filler => {
                SponsorBehavior::ManualSkip
            }
        }
    }

    /// Display name for Settings → Playback.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Sponsor => "Sponsor",
            Self::Selfpromo => "Self-promotion",
            Self::Interaction => "Interaction reminder",
            Self::Intro => "Intro",
            Self::Outro => "Outro",
            Self::Preview => "Preview",
            Self::MusicOfftopic => "Non-music section",
            Self::Filler => "Filler",
        }
    }

    /// OSD message shown when a segment of this category is automatically skipped.
    pub fn skipped_osd(self) -> &'static str {
        match self {
            Self::Sponsor => "Sponsor skipped",
            Self::Selfpromo => "Self-promotion skipped",
            Self::Interaction => "Interaction reminder skipped",
            Self::Intro => "Intro skipped",
            Self::Outro => "Outro skipped",
            Self::Preview => "Preview skipped",
            Self::MusicOfftopic => "Non-music section skipped",
            Self::Filler => "Filler skipped",
        }
    }

    /// OSD message shown when a segment of this category is automatically muted.
    pub fn muted_osd(self) -> &'static str {
        match self {
            Self::Sponsor => "Sponsor muted",
            Self::Selfpromo => "Self-promotion muted",
            Self::Interaction => "Interaction reminder muted",
            Self::Intro => "Intro muted",
            Self::Outro => "Outro muted",
            Self::Preview => "Preview muted",
            Self::MusicOfftopic => "Non-music section muted",
            Self::Filler => "Filler muted",
        }
    }
}

/// How YouTFast handles segments in a SponsorBlock category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SponsorBehavior {
    AutoSkip,
    ManualSkip,
    SeekBarOnly,
    Ignore,
}

impl SponsorBehavior {
    pub const ALL: [Self; 4] = [
        Self::AutoSkip,
        Self::ManualSkip,
        Self::SeekBarOnly,
        Self::Ignore,
    ];

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "auto_skip" => Some(Self::AutoSkip),
            "manual_skip" => Some(Self::ManualSkip),
            "seek_bar_only" => Some(Self::SeekBarOnly),
            "ignore" => Some(Self::Ignore),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::AutoSkip => "Skip automatically",
            Self::ManualSkip => "Show skip button",
            Self::SeekBarOnly => "Only mark on the seek bar",
            Self::Ignore => "Ignore",
        }
    }
}

/// Per-category SponsorBlock behaviors persisted in settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SponsorCategories {
    pub sponsor: SponsorBehavior,
    pub selfpromo: SponsorBehavior,
    pub interaction: SponsorBehavior,
    pub intro: SponsorBehavior,
    pub outro: SponsorBehavior,
    pub preview: SponsorBehavior,
    pub music_offtopic: SponsorBehavior,
    pub filler: SponsorBehavior,
}

impl Default for SponsorCategories {
    fn default() -> Self {
        Self {
            sponsor: SponsorCategory::Sponsor.default_behavior(),
            selfpromo: SponsorCategory::Selfpromo.default_behavior(),
            interaction: SponsorCategory::Interaction.default_behavior(),
            intro: SponsorCategory::Intro.default_behavior(),
            outro: SponsorCategory::Outro.default_behavior(),
            preview: SponsorCategory::Preview.default_behavior(),
            music_offtopic: SponsorCategory::MusicOfftopic.default_behavior(),
            filler: SponsorCategory::Filler.default_behavior(),
        }
    }
}

impl SponsorCategories {
    pub fn get(self, category: SponsorCategory) -> SponsorBehavior {
        match category {
            SponsorCategory::Sponsor => self.sponsor,
            SponsorCategory::Selfpromo => self.selfpromo,
            SponsorCategory::Interaction => self.interaction,
            SponsorCategory::Intro => self.intro,
            SponsorCategory::Outro => self.outro,
            SponsorCategory::Preview => self.preview,
            SponsorCategory::MusicOfftopic => self.music_offtopic,
            SponsorCategory::Filler => self.filler,
        }
    }

    pub fn set(&mut self, category: SponsorCategory, behavior: SponsorBehavior) {
        match category {
            SponsorCategory::Sponsor => self.sponsor = behavior,
            SponsorCategory::Selfpromo => self.selfpromo = behavior,
            SponsorCategory::Interaction => self.interaction = behavior,
            SponsorCategory::Intro => self.intro = behavior,
            SponsorCategory::Outro => self.outro = behavior,
            SponsorCategory::Preview => self.preview = behavior,
            SponsorCategory::MusicOfftopic => self.music_offtopic = behavior,
            SponsorCategory::Filler => self.filler = behavior,
        }
    }

    /// Resolves the configured behavior for a segment category string.
    /// Unknown categories default to `ManualSkip` (matching legacy behavior).
    pub fn behavior_for(self, category: &str) -> SponsorBehavior {
        SponsorCategory::parse(category).map_or(SponsorBehavior::ManualSkip, |cat| self.get(cat))
    }
}

struct ResilientBehavior(Option<SponsorBehavior>);

impl<'de> Deserialize<'de> for ResilientBehavior {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BehaviorVisitor;

        impl<'de> serde::de::Visitor<'de> for BehaviorVisitor {
            type Value = ResilientBehavior;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a SponsorBehavior string or any value")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(ResilientBehavior(SponsorBehavior::parse(v)))
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(ResilientBehavior(None))
            }

            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(ResilientBehavior(None))
            }

            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Self::Value, E> {
                Ok(ResilientBehavior(None))
            }

            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Self::Value, E> {
                Ok(ResilientBehavior(None))
            }

            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Self::Value, E> {
                Ok(ResilientBehavior(None))
            }

            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Self::Value, E> {
                Ok(ResilientBehavior(None))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(ResilientBehavior(None))
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                while map
                    .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
                    .is_some()
                {}
                Ok(ResilientBehavior(None))
            }
        }

        deserializer.deserialize_any(BehaviorVisitor)
    }
}

impl<'de> Deserialize<'de> for SponsorCategories {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CategoriesVisitor;

        impl<'de> serde::de::Visitor<'de> for CategoriesVisitor {
            type Value = SponsorCategories;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a map of SponsorBlock category behaviors")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut categories = SponsorCategories::default();
                while let Some((key, value)) = map.next_entry::<String, ResilientBehavior>()? {
                    if let Some(category) = SponsorCategory::parse(&key)
                        && let Some(behavior) = value.0
                    {
                        categories.set(category, behavior);
                    }
                }
                Ok(categories)
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(SponsorCategories::default())
            }

            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(SponsorCategories::default())
            }

            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Self::Value, E> {
                Ok(SponsorCategories::default())
            }

            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Self::Value, E> {
                Ok(SponsorCategories::default())
            }

            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Self::Value, E> {
                Ok(SponsorCategories::default())
            }

            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Self::Value, E> {
                Ok(SponsorCategories::default())
            }

            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self::Value, E> {
                Ok(SponsorCategories::default())
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(SponsorCategories::default())
            }
        }

        deserializer.deserialize_any(CategoriesVisitor)
    }
}

/// The action SponsorBlock associates with a segment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentAction {
    #[default]
    Skip,
    Mute,
}

/// A part of the video SponsorBlock's users marked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    /// "sponsor", "selfpromo", "intro"…
    pub category: String,
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub action: SegmentAction,
}

impl Segment {
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

/// What the watch page knows about the video being watched.
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
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
    pub fn keeps(self, video: &Video, watched: bool, today: &crate::util::Today) -> bool {
        match self {
            FeedFilter::ForYou | FeedFilter::All => true,
            FeedFilter::Today => today.bucket(video.published_at) == crate::util::AgeBucket::Today,
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

/// What a paged list's rows are.
pub enum PageItems {
    Videos(Vec<Video>),
    Channels(Vec<Channel>),
    History(Vec<HistoryRow>),
    Playlists(Vec<PlaylistSummary>),
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

#[cfg(test)]
mod tests {
    use super::{FeedFilter, Video};

    #[test]
    fn the_feed_chips_keep_what_they_say() {
        let now = crate::util::now();
        let today = crate::util::Today::at(now);
        let mut fresh = Video::from_id("a".to_owned());
        fresh.published_at = Some(now);
        let mut old = Video::from_id("b".to_owned());
        old.published_at = Some(now - 30 * 86_400);
        let mut live = Video::from_id("c".to_owned());
        live.live = true;

        assert!(FeedFilter::All.keeps(&old, true, &today));
        assert!(FeedFilter::Today.keeps(&fresh, false, &today));
        assert!(!FeedFilter::Today.keeps(&old, false, &today));
        assert!(!FeedFilter::Today.keeps(&live, false, &today));
        assert!(FeedFilter::Unwatched.keeps(&old, false, &today));
        assert!(!FeedFilter::Unwatched.keeps(&fresh, true, &today));
        assert!(FeedFilter::Live.keeps(&live, false, &today));
        assert!(!FeedFilter::Live.keeps(&fresh, false, &today));
    }

    #[test]
    fn sponsor_categories_defaults_and_setters() {
        use super::{SegmentAction, SponsorBehavior, SponsorCategories, SponsorCategory};

        let mut categories = SponsorCategories::default();
        assert_eq!(
            categories.behavior_for("sponsor"),
            SponsorBehavior::AutoSkip
        );
        assert_eq!(
            categories.behavior_for("selfpromo"),
            SponsorBehavior::AutoSkip
        );
        assert_eq!(
            categories.behavior_for("interaction"),
            SponsorBehavior::AutoSkip
        );
        for cat in ["intro", "outro", "preview", "music_offtopic", "filler"] {
            assert_eq!(categories.behavior_for(cat), SponsorBehavior::ManualSkip);
        }
        assert_eq!(
            categories.behavior_for("unknown"),
            SponsorBehavior::ManualSkip
        );

        categories.set(SponsorCategory::Sponsor, SponsorBehavior::Ignore);
        categories.set(SponsorCategory::Intro, SponsorBehavior::AutoSkip);
        assert_eq!(
            categories.get(SponsorCategory::Sponsor),
            SponsorBehavior::Ignore
        );
        assert_eq!(
            categories.get(SponsorCategory::Intro),
            SponsorBehavior::AutoSkip
        );
        assert_eq!(SegmentAction::default(), SegmentAction::Skip);
    }
}
