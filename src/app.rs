//! The application: its state, and the actions the views ask for.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use anyhow::Context as _;
use eframe::glow;
use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

use crate::appearance::theme::{self, Palette};
use crate::backend::{
    Backend, Command, CommentsKey, Event, PageItems, Payload, Request, SignInMethod, Slot,
};
use crate::desktop::instance::{self, Queue as RemoteQueue};
use crate::model::{
    AccountSource, Action, Channel, ChannelAbout, ChannelHeader, ChannelTab, CommentRow, Details,
    FeedFilter, Fetch, HistoryRow, Listing, PlaylistSummary, Quality, Rating, Resolution, Results,
    SaveTarget, Segment, Source, Video, View,
};
use crate::mpv::{self, Mpv, NativeDisplay, VideoSurface};
use crate::storage::library::{Account, Import, Library};
use crate::storage::paths::Dirs;
use crate::storage::settings::{Settings, ThemeChoice};
use crate::youtube::innertube;
use crate::{logic::alerts, storage::account, ui, util, youtube::ytdlp};

/// The signed-in account, as the interface shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum AccountState {
    #[default]
    SignedOut,
    SigningIn,
    SignedIn(u32),
    /// Signing in failed, or the kept session stopped working.
    Failed(String),
}

/// The playlist or channel a page shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Channel(Channel),
    Playlist { id: String, name: String },
}

/// What [`State::history_cache`] was built from; any change rebuilds it.
#[derive(Debug)]
struct HistoryKey {
    library: u64,
    device_len: usize,
    account: u64,
    account_len: usize,
    source: Option<Source>,
    merged: bool,
    query: String,
    requested_at: Option<i64>,
    /// `merge` reads the clock when no request time is known; its sections
    /// only move at midnight (UTC).
    day: i64,
}

impl HistoryKey {
    fn of(state: &State) -> Self {
        Self {
            library: state.library.history_stamp(),
            device_len: state.library.history.len(),
            account: state.account_history_generation,
            account_len: state.account_history.items().len(),
            source: state.history_source(),
            merged: state.merged_history(),
            query: state.history_query.clone(),
            requested_at: state.history_requested_at,
            day: Self::day(state),
        }
    }

    fn day(state: &State) -> i64 {
        match state.history_requested_at {
            Some(_) => 0,
            None => util::now().div_euclid(86_400),
        }
    }

    /// Whether `state` would give the same key, without building one.
    fn is_current(&self, state: &State) -> bool {
        self.library == state.library.history_stamp()
            && self.device_len == state.library.history.len()
            && self.account == state.account_history_generation
            && self.account_len == state.account_history.items().len()
            && self.source == state.history_source()
            && self.merged == state.merged_history()
            && self.query == state.history_query
            && self.requested_at == state.history_requested_at
            && self.day == Self::day(state)
    }
}

/// History's rows as indices, with the day sections they fall into.
#[derive(Debug)]
pub struct HistoryCache {
    key: HistoryKey,
    pub entries: Vec<crate::logic::history::Entry>,
    pub runs: Vec<crate::logic::history::Run>,
}

/// What the views draw from.
#[derive(Default)]
pub struct State {
    pub video_tabs: crate::logic::video_tabs::VideoTabs,
    pub cast: cast::State,
    pub view: View,
    /// The search field's text.
    pub query: String,
    pub results: Results,
    pub search_options: innertube::SearchOptions,
    pub search_query: String,
    pub results_channels: Listing<Channel>,
    pub results_playlists: Listing<PlaylistSummary>,
    pub history_query: String,
    pub history_filter: crate::logic::history::HistoryFilter,
    pub history_needs_refresh: bool,
    pub history_requested_at: Option<i64>,
    pub channel_order: innertube::ChannelOrder,
    pub playlist_details: Option<innertube::PlaylistDetails>,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    /// The results' next page, and a page on its way (or its error).
    pub results_more: Option<innertube::More>,
    pub results_more_state: Fetch<()>,
    /// The home feed (long videos; shorts go to the shelf).
    pub feed: Listing<Video>,
    /// How many feed rows are drawn; the rest sit behind scrolling.
    pub feed_shown: usize,
    /// Home's chip: which of the drawn rows it shows.
    pub feed_filter: FeedFilter,
    /// The account feed's next page, when it has one.
    pub feed_more: Option<innertube::More>,
    /// A feed continuation on its way, or its error.
    pub feed_more_state: Fetch<()>,
    /// Home's shorts shelf.
    pub shorts: Listing<Video>,
    /// Home's For you, its next page (the account's only) and a page on
    /// its way.
    pub for_you: Listing<Video>,
    pub for_you_more: Option<innertube::More>,
    pub for_you_more_state: Fetch<()>,
    /// For you came from the account, not from this computer's history.
    pub for_you_from_account: bool,
    /// When the feed last landed; one fresh enough already is the
    /// alerts check's answer.
    pub feed_loaded_at: Option<i64>,
    /// The signed-in account's lists, each with the next page's
    /// directions and a page on its way (or its error).
    pub account_subscriptions: Listing<Channel>,
    pub subscriptions_more: Option<innertube::More>,
    pub subscriptions_more_state: Fetch<()>,
    pub account_history: Listing<HistoryRow>,
    /// Bumped whenever `account_history` changes, so `history_rows` knows
    /// when to rebuild.
    pub account_history_generation: u64,
    /// History's merged, filtered rows (see [`State::history_cache`]).
    pub history_cache: std::cell::RefCell<Option<HistoryCache>>,
    pub history_more: Option<innertube::More>,
    pub history_more_state: Fetch<()>,
    pub account_watch_later: Listing<Video>,
    pub watch_later_more: Option<innertube::More>,
    pub watch_later_more_state: Fetch<()>,
    pub liked: Listing<Video>,
    pub liked_more: Option<innertube::More>,
    pub liked_more_state: Fetch<()>,
    pub playlists: Listing<PlaylistSummary>,
    pub playlists_more: Option<innertube::More>,
    pub playlists_more_state: Fetch<()>,
    pub page: Option<Page>,
    pub page_videos: Listing<Video>,
    /// What this computer keeps, with or without an account.
    pub library: Library,
    pub account: AccountState,
    /// Account or device, on pages that exist both ways.
    pub source: Source,
    /// Browsers to sign in with, as found on this computer.
    pub browsers: Vec<&'static str>,
    pub browser: String,
    /// A message for the user that belongs to no page.
    pub notice: Option<String>,
    /// The `?` overlay's bindings list is open.
    pub keys_open: bool,
    /// The keyboard cursor: the selected row in the current view's list.
    pub cursor: usize,
    /// Set on the frame the cursor moved, so only that frame scrolls
    /// the row into view — asking every frame glues the page to it.
    pub cursor_moved: bool,
    /// Vertical page scroll requested by the arrow keys this frame, in points.
    pub page_scroll: f32,
    pub watching: Option<Video>,
    /// The list the video being watched was opened from.
    pub up_next: Vec<Video>,
    /// Shown over the video in place of the picture.
    pub player_error: Option<String>,
    pub fullscreen: bool,
    /// Whether only audio is being streamed and decoded. Kept only for the
    /// current playback, not between runs.
    pub audio_only: bool,
    /// The sidebar toggle has been pressed this run: since then the saved
    /// choice rules every page, over the watch page's automatic rail.
    pub sidebar_toggled: bool,
    /// The seek bar's position while it is dragged.
    pub seek_preview: Option<f64>,
    /// The choices kept between runs (quality, volume).
    pub settings: Settings,
    /// The colours the interface draws, resolved from the settings.
    pub palette: Palette,
    /// The palettes the settings page lists, as (filename, display name).
    pub themes: Vec<(String, String)>,
    /// Whether the desktop's palette is followed (Omarchy on Linux).
    pub follows_omarchy: bool,
    /// The system's interface font was asked for and could not be used, so
    /// Inter draws instead (`theme::system_face_missing`).
    pub system_font_missing: bool,
    /// What the theme setting says under it; empty when all is well.
    pub theme_status: String,
    /// Where the palette files live, for the settings page to say.
    pub themes_dir: String,
    /// What the video being watched offers, highest first; empty until it
    /// loads.
    pub resolutions: Vec<Resolution>,
    /// rustypipe's view of the video: description, chapters, comments.
    pub details: Fetch<Details>,
    /// The comments loaded so far.
    pub comments: Fetch<Vec<CommentRow>>,
    /// The next comments page's token.
    pub comments_next: Option<String>,
    /// The next comments page on its way, or its error; a double click on
    /// `[ load more ]` cannot send the same page twice.
    pub comments_more: Fetch<()>,
    /// Visitor data the comments requests carry.
    pub comments_visitor_data: Option<String>,
    /// Original page tokens, kept only in memory to renew attachment URLs.
    pub comment_pages: HashMap<String, String>,
    pub comment_image_refreshes: HashSet<String>,
    /// Each comment's replies by comment id, when opened.
    pub replies: HashMap<String, Fetch<Vec<CommentRow>>>,
    /// The description expanded past its first rows.
    pub description_open: bool,
    /// The comments (and replies) expanded past their first rows, by id.
    pub comments_open: HashSet<String>,
    /// The account's rating of the video being watched.
    pub rating: Fetch<Rating>,
    /// The rating the like count was taken with, so a new rating moves the
    /// count shown by one.
    pub rating_base: Rating,
    /// Account writes in flight, keyed by video or video and playlist.
    pub rating_pending: HashSet<String>,
    pub save_pending: HashSet<(String, String)>,
    /// The save menu's account playlists, asked for when it first opens.
    pub save_targets: Fetch<Vec<SaveTarget>>,
    /// The share menu's `Start at` box: links start where the video is.
    pub share_at_time: bool,
    /// The channel page's header, kept while the same channel is open.
    pub channel_header: Option<ChannelHeader>,
    /// Which tab the channel page shows.
    pub channel_tab: ChannelTab,
    /// The channel page's playlists tab.
    pub page_playlists: Listing<PlaylistSummary>,
    /// The channel page's About pane.
    pub channel_about: Fetch<ChannelAbout>,
    /// The channel tab's next page, when there is one.
    pub page_more: Option<innertube::More>,
    /// The continuation on its way, or its error.
    pub page_more_state: Fetch<()>,
    /// The channel page's own search field's text.
    pub channel_query: String,
    /// The add-account form's fields.
    pub account_name: String,
    pub account_profile: String,
    /// The browser profiles found on this computer, each signed in with
    /// one click.
    pub browser_profiles: Vec<account::BrowserProfile>,
    /// The form's Advanced part (a browser and profile by hand) is open.
    pub account_advanced: bool,
    /// The account row being renamed, and the field's text.
    pub renaming: Option<(u32, String)>,
    /// The account row asking `remove?`.
    pub removing: Option<u32>,
    /// Unseen uploads from the last check.
    pub alerts: Vec<Video>,
    /// The rows the Notifications view shows during this visit.
    pub alerts_visit: Vec<Video>,
    /// Unix seconds of the last check (the page's hint line).
    pub last_checked: Option<i64>,
    /// A check is in flight.
    pub alerts_checking: bool,
    /// The player's controls over the video stay until then: the pointer
    /// moved over it lately.
    pub controls_until: Option<Instant>,
    /// The player's own menu (speed, quality, captions), drawn over the
    /// video.
    pub player_menu: PlayerMenu,
    /// A short note over the video ("+10 s", "1.5×") and when it goes.
    pub osd: Option<(String, Instant)>,
    /// What plays next, before Up next: `Add to queue` and `Play next`.
    pub queue: Vec<Video>,
    /// The video the end-of-video countdown leads to, and when it starts.
    pub autoplay: Option<(Video, Instant)>,
    /// SponsorBlock's segments of the video being watched.
    pub segments: Vec<Segment>,
    /// The segments already skipped, by index: seeking back into one plays
    /// it.
    pub skipped: HashSet<usize>,
    /// What YouTube suggests for the search being typed.
    pub suggestions: Vec<String>,
    /// The suggestion the arrow keys picked, if any.
    pub suggestion_cursor: Option<usize>,
    /// The mini player was closed: the video stays paused until reopened.
    pub mini_closed: bool,
    /// A return from the mini player reveals the inline video again.
    pub watch_scroll_to_video: bool,
    pub playback_loop: bool,
    pub extras: Fetch<crate::youtube::extras::Extras>,
    pub transcript_open: bool,
    pub transcript_query: String,
    pub transcript_follow: bool,
    pub playing_playlist: Option<String>,
    pub playing_more: Option<innertube::More>,
    pub playing_more_state: Fetch<()>,
    pub playing_waiting: Option<PlaylistAdvance>,
    /// The wheel step, touchpad scale and glide, and axis lock for scrolling.
    pub scrolling: fastframe_scroll::Scrolling,
    /// Status of checking for or installing a YouTFast release update.
    pub update: crate::desktop::updater::UpdateState,
    /// Whether the full installed changelog is expanded in Settings → About.
    pub full_changelog_open: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaylistAdvance {
    Manual,
    Autoplay,
}

/// Which page of the player's menu is open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerMenu {
    #[default]
    Closed,
    Main,
    Speed,
    Quality,
    Captions,
    CaptionStyle,
}

impl State {
    /// The sidebar's toggle (`b`, or the top bar's button).
    pub fn toggle_sidebar(&mut self, automatically_collapsed: bool) {
        // The watch page starts folded to its rail, so the first press there
        // opens the sidebar instead of folding it again. Narrow windows use
        // the same automatic rail. From the first use the saved choice rules
        // every page, remembered for the run.
        if !self.sidebar_toggled {
            let was_collapsed = self.settings.sidebar_collapsed || automatically_collapsed;
            self.settings.sidebar_collapsed = !was_collapsed;
        } else {
            self.settings.sidebar_collapsed = !self.settings.sidebar_collapsed;
        }
        self.sidebar_toggled = true;
    }

    /// The video being watched is a live stream now: yt-dlp's word once it
    /// has loaded, the list's or rustypipe's until then.
    pub fn watching_live(&self) -> bool {
        self.watching.as_ref().is_some_and(|video| video.live)
    }

    /// The video being watched is a Short: drawn upright, looping.
    pub fn watching_short(&self) -> bool {
        self.watching.as_ref().is_some_and(|video| video.short)
    }

    pub fn signed_in(&self) -> bool {
        matches!(self.account, AccountState::SignedIn(_))
    }

    /// The account the app is signed in as, by its library id.
    pub fn active_account(&self) -> Option<&Account> {
        self.library
            .active_account
            .and_then(|id| self.library.account(id))
    }

    /// Where account-or-device pages take their list from now.
    pub fn source(&self) -> Source {
        if self.signed_in() {
            self.source
        } else {
            Source::Device
        }
    }

    /// History has its own filter in merged modes; other pages keep `source`.
    pub fn merged_history(&self) -> bool {
        self.signed_in()
            && self.settings.history_sync != crate::storage::settings::HistorySync::Separate
    }

    /// None denotes All, available only to signed-in merged history.
    pub fn history_source(&self) -> Option<Source> {
        use crate::logic::history::HistoryFilter;
        if !self.merged_history() {
            return Some(self.source());
        }
        match self.history_filter {
            HistoryFilter::All => None,
            HistoryFilter::Account => Some(Source::Account),
            HistoryFilter::Device => Some(Source::Device),
        }
    }

    fn build_history_rows(&self) -> Vec<crate::logic::history::Row<'_>> {
        use crate::logic::history;
        let mut rows = match self.history_source() {
            None => history::merge(
                self.account_history.items(),
                &self.library.history,
                self.history_requested_at.unwrap_or_else(util::now),
            ),
            Some(Source::Device) => self
                .library
                .history_matching(&self.history_query)
                .map(|watched| history::Row {
                    video: &watched.video,
                    when: None,
                    device: Some(watched),
                })
                .collect(),
            Some(Source::Account) if self.merged_history() => {
                history::account_rows(self.account_history.items(), &self.library.history)
            }
            Some(Source::Account) => self
                .account_history
                .items()
                .iter()
                .map(|row| history::Row {
                    video: &row.video,
                    when: row.when.clone(),
                    device: None,
                })
                .collect(),
        };
        if self.history_source() != Some(Source::Account) || self.merged_history() {
            let query = self.history_query.trim().to_lowercase();
            rows.retain(|row| row.matches_lowered(&query));
        }
        rows
    }

    /// History's rows as the page shows them, rebuilt only when what they
    /// come from changes (the device history, the account's, the filter, the
    /// search or the day) rather than on every frame.
    pub fn history_cache(&self) -> std::cell::Ref<'_, HistoryCache> {
        let current = self
            .history_cache
            .borrow()
            .as_ref()
            .is_some_and(|cache| cache.key.is_current(self));
        if !current {
            let rows = self.build_history_rows();
            let entries = crate::logic::history::entries(
                &rows,
                self.account_history.items(),
                &self.library.history,
            );
            let runs = crate::logic::history::runs(&entries);
            *self.history_cache.borrow_mut() = Some(HistoryCache {
                key: HistoryKey::of(self),
                entries,
                runs,
            });
        }
        std::cell::Ref::map(self.history_cache.borrow(), |cache| {
            cache.as_ref().expect("filled above")
        })
    }

    /// The cached entry as a row over the lists it indexes.
    pub fn history_row(
        &self,
        entry: &crate::logic::history::Entry,
    ) -> crate::logic::history::Row<'_> {
        use crate::logic::history::Origin;
        let device = entry
            .device
            .and_then(|index| self.library.history.get(index));
        let video = match entry.video {
            Origin::Device(index) => &self.library.history[index].video,
            Origin::Account(index) => &self.account_history.items()[index].video,
        };
        crate::logic::history::Row {
            video,
            when: entry.when.clone(),
            device,
        }
    }

    /// All of History's rows; prefer [`State::history_cache`] where a frame
    /// draws only some of them.
    pub fn history_rows(&self) -> Vec<crate::logic::history::Row<'_>> {
        self.history_cache()
            .entries
            .iter()
            .map(|entry| self.history_row(entry))
            .collect()
    }

    /// The feed's videos home shows: For you's, or the first
    /// `feed_shown` of the feed through the chip.
    pub fn visible_feed(&self) -> Vec<&Video> {
        if self.feed_filter == FeedFilter::ForYou {
            return self.for_you.items().iter().collect();
        }
        let today = util::Today::at(util::now());
        let videos = self.feed.items();
        videos[..videos.len().min(self.feed_shown)]
            .iter()
            .filter(|video| {
                self.feed_filter
                    .keeps(video, self.library.watched(&video.id), &today)
            })
            .collect()
    }

    /// Whether the account (when its list is known) or this device
    /// subscribes to the channel.
    pub fn subscribed_in_account(&self, channel_id: &str) -> bool {
        self.account_subscriptions
            .items()
            .iter()
            .any(|channel| channel.id == channel_id)
    }

    /// The library's subscriptions the account does not have; the feed and
    /// the alerts check read these by RSS.
    pub fn device_only_subscriptions(&self) -> Vec<Channel> {
        self.library
            .subscriptions
            .iter()
            .filter(|channel| !self.subscribed_in_account(&channel.id))
            .cloned()
            .collect()
    }
}

/// Applies the sidebar action through the shared state transition used by
/// the app, demo and UI regression test.
pub(crate) fn apply_sidebar_action(state: &mut State, action: Action, width: f32) -> bool {
    let Action::ToggleSidebar = action else {
        return false;
    };
    let automatically_collapsed = sidebar_uses_automatic_rail(state, width);
    state.toggle_sidebar(automatically_collapsed);
    true
}

/// Whether the current view/window would use the rail before the user
/// overrides that automatic choice in this run.
pub(crate) fn sidebar_uses_automatic_rail(state: &State, width: f32) -> bool {
    state.view == View::Watch || width < 900.0
}

/// Where the chosen quality was kept in eframe's storage before the
/// settings file; read once to bring it over.
const QUALITY_KEY: &str = "quality";

/// How many watch pages' details are kept: some 15 to 30 KB each.
const DETAILS_KEPT: usize = 50;
/// How long kept details are shown instead of asked for again, in seconds;
/// views and likes move, and `r` asks again at once.
const DETAILS_FRESH: i64 = 30 * 60;
/// How long the volume stays still before it is saved: a drag of the
/// slider is one save, not one per frame.
const VOLUME_SAVE_AFTER: Duration = Duration::from_secs(1);
/// How many videos' ratings are kept, and for how long in seconds: the
/// app's own changes update them, so only a rating changed elsewhere (in
/// a browser) shows late.
const RATINGS_KEPT: usize = 200;
const RATING_FRESH: i64 = 30 * 60;

pub struct App {
    cast_worker: Option<crate::desktop::cast::Worker>,
    local_playback_generation: u64,
    state: State,
    backend: Backend,
    dirs: Dirs,
    /// `None` when mpv could not start; `state.player_error` says why.
    player: Option<Mpv>,
    surface: Option<VideoSurface>,
    /// Bumped by every request; each slot remembers its latest, so a late
    /// answer to an older request is dropped.
    generation: u64,
    generations: HashMap<Slot, u64>,
    /// The generation the details/comments requests of the video being
    /// watched were sent with; a later `open` drops their answers.
    details_generation: u64,
    playing_generation: u64,
    playback_report: history_sync::PlaybackReport,
    playback_tick: Instant,
    /// The watch pages seen lately, so going back to one asks YouTube
    /// again only for its comments.
    details_cache: util::Recent<Details>,
    /// The account's ratings of the videos watched lately (see
    /// [`RATING_FRESH`]).
    ratings: util::Recent<Rating>,
    /// The comments page in flight, to put its token back on failure.
    more_comments_token: Option<String>,
    library_revision: std::cell::Cell<u64>,
    /// The channel the open page shows, to detect a channel switch.
    open_channel: Option<String>,
    /// The shelf's Shorts tabs were asked for this feed already; a new
    /// feed load asks again, but only while Home is showing the shelf.
    shelf_asked: bool,
    /// Where Back goes: the pages a video, channel or playlist was opened
    /// from.
    back: Vec<navigation::Location>,
    forward: Vec<navigation::Location>,
    /// The palettes the settings page offers, scanned off the interface
    /// thread.
    custom_themes: theme::Catalog,
    /// Wakes the window when a theme scan finishes or a watched file
    /// changes.
    theme_waker: fastframe_theme::Waker,
    /// A change of palette revealed from the middle of the window outwards.
    theme_transition: fastframe_theme::Transition,
    /// The cursor's reset marker: the view and listing it was zeroed for.
    cursor_key: (View, usize),
    /// When the volume last changed, while it is not yet saved.
    volume_changed: Option<Instant>,
    /// The search text suggestions were last asked for, and when the
    /// field last changed: they are asked once typing pauses.
    suggested: String,
    query_changed: Option<(String, Instant)>,
    /// The kept captions language still has to be picked for the video
    /// loaded.
    captions_pending: bool,
    /// What later launches ask the running copy, drained each frame.
    remote: RemoteQueue,
    /// The desktop's media controls.
    media: crate::desktop::now_playing::MediaControls,
    /// The installed fonts for other scripts, until they are in.
    fonts: Option<theme::SystemFonts>,
    /// Held so the instance lock and channel outlive the app; the app
    /// creator closure is dropped once it has run.
    _instance: std::sync::Arc<instance::Guard>,
}

impl App {
    /// `open` is the command line's link or search, if any.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        open: Option<String>,
        guard: std::sync::Arc<instance::Guard>,
        fonts: theme::SystemFonts,
    ) -> anyhow::Result<Self> {
        let ctx = &cc.egui_ctx;
        guard.set_context(ctx);
        let fonts = (!fonts.install(ctx)).then_some(fonts);
        egui_extras::install_image_loaders(ctx);
        ui::images::install_animation_loader(ctx);
        fastframe_icons::install::<crate::appearance::icons::Icon>(ctx);
        let dirs = Dirs::new().context("could not make the data folders")?;
        ui::images::init(dirs.cache.join("thumbnails"));
        let backend =
            Backend::start(ctx.clone(), dirs.clone()).context("could not start the backend")?;

        let browsers = account::installed_browsers();
        let mut settings =
            Settings::load(&dirs.settings()).context("could not safely load the settings")?;
        // The quality eframe's storage kept before the settings file
        // existed moves over once.
        if !dirs.settings().exists()
            && let Some(quality) = cc
                .storage
                .and_then(|storage| eframe::get_value::<Quality>(storage, QUALITY_KEY))
        {
            settings.quality = quality;
        }
        let mut custom_themes = theme::Catalog::default();
        theme::enable_desktop_themes(&mut custom_themes);
        let theme_ctx = ctx.clone();
        let theme_waker = fastframe_theme::Waker::new(move || theme_ctx.request_repaint());
        custom_themes.start(
            dirs.config.join("themes"),
            themes::selected_theme(&settings),
            &theme_waker,
        );
        let palette = settings.palette();
        theme::apply(ctx, &palette);
        let mut state = State {
            library: Library::load(&dirs.library()).context("could not safely load the library")?,
            themes_dir: dirs.config.join("themes").display().to_string(),
            palette,
            settings,
            browser: browsers.first().copied().unwrap_or("chromium").to_owned(),
            browser_profiles: account::browser_profiles(),
            browsers,
            ..State::default()
        };
        let (player, surface) = match Mpv::new(
            ctx,
            state.settings.quality.limit(),
            state.settings.volume,
            player::caption_languages(&state.settings),
        ) {
            Ok(player) => {
                let (ahead, back) = state.settings.playback_buffer.limits();
                if let Err(error) = player.set_buffer_limits(ahead, back) {
                    log::warn!("could not apply playback buffer limits: {error}");
                }
                let surface = cc.get_proc_address.clone().map(|get_proc_address| {
                    VideoSurface::new(
                        player.handle(),
                        get_proc_address,
                        native_display(cc),
                        ctx.clone(),
                    )
                });
                if surface.is_none() {
                    state.player_error = Some("This window has no OpenGL context for video".into());
                }
                (Some(player), surface)
            }
            Err(error) => {
                log::error!("could not start mpv: {error}");
                state.player_error = Some(format!("Could not start mpv ({error})"));
                (None, None)
            }
        };

        let mut app = Self {
            state,
            backend,
            dirs,
            player,
            surface,
            generation: 0,
            cast_worker: None,
            local_playback_generation: 0,
            generations: HashMap::new(),
            details_generation: 0,
            playing_generation: 0,
            playback_report: history_sync::PlaybackReport::default(),
            playback_tick: Instant::now(),
            details_cache: util::Recent::new(DETAILS_KEPT),
            ratings: util::Recent::new(RATINGS_KEPT),
            more_comments_token: None,
            library_revision: std::cell::Cell::new(0),
            open_channel: None,
            shelf_asked: false,
            back: Vec::new(),
            forward: Vec::new(),
            custom_themes,
            theme_waker,
            theme_transition: fastframe_theme::Transition::default(),
            cursor_key: (View::default(), 0),
            volume_changed: None,
            suggested: String::new(),
            query_changed: None,
            captions_pending: false,
            remote: guard.commands(),
            media: crate::desktop::now_playing::MediaControls::start(ctx),
            fonts,
            _instance: guard,
        };
        app.set_alerts_timer();
        app.refresh_theme_list();
        if let (Some(version), Some(changelog)) = (
            &app.state.settings.last_updated_version,
            &app.state.settings.last_updated_changelog,
        ) && !changelog.is_empty()
            && crate::desktop::updater::Version::parse(version)
                >= crate::desktop::updater::Version::parse(crate::desktop::updater::CURRENT_VERSION)
        {
            app.state.full_changelog_open = true;
        }
        if app.state.settings.update_notifications || app.state.settings.auto_update {
            app.state.update = crate::desktop::updater::UpdateState::Checking;
            app.backend.send(Command::CheckForUpdates { manual: false });
        }
        if let Some(active) = app.state.library.active_account
            && let Some(account) = app.state.library.account(active).cloned()
        {
            app.state.account = AccountState::SigningIn;
            app.backend.send(Command::Switch(account));
        }
        app.load(View::Home, false);
        if let Some(open) = open {
            app.state.query = open.clone();
            app.apply(ctx, Action::Search(open));
        }
        Ok(app)
    }

    /// Takes in what the backend and mpv have to say.
    fn poll(&mut self) {
        // The themes folder or Omarchy's palette changed on disk.
        if self.custom_themes.needs_reload() {
            self.load_themes();
        }
        if self.custom_themes.poll() {
            self.adopt_themes();
        }
        for event in self.backend.poll() {
            self.handle(event);
        }
        self.poll_player();
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::HistoryMarked => {
                self.state.history_needs_refresh = true;
            }
            Event::PlayingPlaylist {
                generation,
                id,
                result,
            } => {
                match self.state.finish_playlist_page(
                    self.playing_generation,
                    generation,
                    &id,
                    result,
                ) {
                    Some(reason) if !self.await_playlist_page(reason) => match reason {
                        PlaylistAdvance::Manual => self.play_next(),
                        PlaylistAdvance::Autoplay if self.state.settings.autoplay => {
                            self.start_autoplay()
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            Event::Extras { generation, result } if generation == self.details_generation => {
                self.state.extras = match result {
                    Ok(extras) => Fetch::Ready(extras),
                    Err(error) => Fetch::Failed(error),
                };
            }
            Event::Extras { .. } => {}
            Event::Loaded {
                slot,
                generation,
                result,
            } => {
                if self.generations.get(&slot) == Some(&generation) {
                    self.fill(slot, result);
                }
            }
            Event::Details { generation, result } => self.on_details(generation, *result),
            Event::Comments {
                generation,
                key,
                token,
                result,
            } => self.on_comments(generation, key, token, result),
            Event::Imported(Ok(import)) => self.on_imported(import),
            Event::Imported(Err(error)) => self.state.notice = Some(error),
            Event::ImportMerged {
                library,
                counts,
                revision,
            } => self.on_import_merged(*library, counts, revision),
            Event::Rating { id, result } => self.on_rating(id, result),
            Event::RateFailed {
                id,
                previous,
                error,
            } => self.on_rate_failed(id, previous, error),
            Event::RateSucceeded { id, rating } => self.on_rate_succeeded(id, rating),
            Event::SaveTargets { id, result } => self.on_save_targets(id, result),
            Event::SaveFailed {
                id,
                playlist,
                saved,
                error,
            } => self.on_save_failed(id, playlist, saved, error),
            Event::SaveSucceeded {
                id,
                playlist,
                saved,
            } => self.on_save_succeeded(id, playlist, saved),
            Event::SigningIn => self.state.account = AccountState::SigningIn,
            Event::AccountAdded(account) => {
                self.state.library.accounts.push(account);
                self.save_library();
            }
            Event::SignedIn(id) => self.on_signed_in(id),
            Event::SignInFailed(error) => {
                self.on_signed_out();
                self.state.account = AccountState::Failed(error);
            }
            Event::PlaybackCookies { account, browser } => {
                if self.state.account == AccountState::SignedIn(account) {
                    self.set_playback_cookies(Some(browser));
                }
            }
            Event::SessionExpired(error) => {
                self.on_signed_out();
                self.state.account =
                    AccountState::Failed(format!("{error} Sign in again to see your account."));
            }
            Event::SessionUnavailable { id, error } => {
                self.on_session_unavailable(id, error);
            }
            Event::SignedOut => self.on_signed_out(),
            Event::CheckAlerts => self.check_alerts(),
            Event::Thumbnail { url, result } => {
                ui::images::completed(self.backend.context(), &url, result)
            }
            Event::Suggestions { query, items } => {
                if query == self.state.query.trim() {
                    self.state.suggestions = items;
                    self.state.suggestion_cursor = None;
                }
            }
            Event::Segments { id, segments } => {
                if self.state.settings.sponsorblock {
                    self.state.cast.receive_segments(&id, &segments);
                }
                self.state.accept_segments(&id, segments);
            }
            Event::Notice(notice) => self.state.notice = Some(notice),
            Event::UpdateChecked { result, manual } => self.on_update_checked(result, manual),
            Event::UpdateInstalled {
                release,
                auto,
                result,
            } => self.on_update_installed(release, auto, result),
        }
    }

    fn apply(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Cast(action) => self.cast_action(ctx, action),
            Action::Search(text) => {
                let text = text.trim();
                if text.is_empty() {
                    return;
                }
                if let Some(id) = ytdlp::video_id_from_link(text) {
                    self.state.suggestions.clear();
                    self.open(Video::from_id(id));
                    return;
                }
                if let Some(channel) = ytdlp::channel_from_link(text) {
                    self.state.suggestions.clear();
                    self.open_page(ctx, View::Channel, Page::Channel(channel));
                    return;
                }
                self.push_location();
                self.leave_video(ctx);
                self.state.suggestions.clear();
                self.suggested = text.to_owned();
                self.state.query = text.to_owned();
                self.state.search_query = text.to_owned();
                self.state.view = View::Results;
                self.load(View::Results, true);
            }
            Action::Open(video) => self.open(video),
            Action::OpenVideoTab(video) => self.state.video_tabs.add(video),
            Action::SelectVideoTab(id) => {
                if let Some(video) = self.state.video_tabs.video(&id).cloned() {
                    if self.state.watching.as_ref().is_some_and(|v| v.id == id) {
                        self.return_to_watch();
                    } else {
                        self.select_video_tab(video);
                    }
                }
            }
            Action::NewVideoTab => {
                // Leaving the watch page keeps the video playing in the mini player.
                self.apply(ctx, Action::Navigate(View::Home));
                self.state.video_tabs.request_new_tab();
            }
            Action::CloseVideoTab(id) => {
                let active = self.state.video_tabs.active() == Some(id.as_str());
                self.remember_position();
                self.state.video_tabs.close(&id);
                if active {
                    if let Some(video) = self.state.video_tabs.selected().cloned() {
                        self.select_video_tab(video);
                    } else {
                        self.close_mini_player();
                        if let Some(player) = &mut self.player
                            && let Err(error) = player.stop()
                        {
                            log::warn!("mpv: {error}");
                        }
                        self.playback_report.cancel();
                        self.reset_watch_panes();
                        self.state.watching = None;
                        self.state.fullscreen = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                        self.state.view = View::Home;
                    }
                }
            }
            Action::LoadExtras => self.load_extras(),
            Action::ToggleTranscript => {
                self.state.transcript_open = !self.state.transcript_open;
                self.state.transcript_follow = true;
                if self.state.transcript_open {
                    self.load_extras();
                }
            }
            Action::SeekChapter(forward) => {
                if let (Some(status), Some(details)) = (
                    self.player.as_ref().map(Mpv::status),
                    self.state.details.ready(),
                ) && let Some(seconds) =
                    navigation::chapter_target(&details.chapters, status.position, forward)
                {
                    self.apply(
                        ctx,
                        Action::SeekTo {
                            seconds,
                            exact: true,
                        },
                    );
                }
            }
            Action::RetryPlayback => self.retry_playback(),
            Action::ToggleLoop => self.toggle_loop(),
            Action::MoveQueue { from, to } => self.move_queue(from, to),
            Action::SetCaptionScale(scale) => self.set_caption_scale(scale),
            Action::SetCaptionBackground(on) => self.set_caption_background(on),
            Action::ToggleMiniPlayer => {
                if self.state.view == View::Watch && self.back.is_empty() {
                    self.apply(ctx, Action::Navigate(View::Home));
                } else if self.state.view == View::Watch {
                    self.apply(ctx, Action::Back);
                } else {
                    self.return_to_watch();
                }
            }
            Action::SetScrollMiniplayer(on) => {
                self.state.settings.scroll_miniplayer = on;
                self.save_settings();
            }
            Action::SetSidebar(entries) => {
                self.state.settings.sidebar = crate::storage::settings::sanitize_sidebar(entries);
                self.save_settings();
            }
            Action::SetPlaybackBuffer(buffer) => {
                self.state.settings.playback_buffer = buffer;
                if let Some(player) = &self.player {
                    let (ahead, back) = buffer.limits();
                    if let Err(error) = player.set_buffer_limits(ahead, back) {
                        log::warn!("could not set playback buffer limits: {error}");
                    }
                }
                self.save_settings();
            }
            Action::Back => self.travel(ctx, false),
            Action::Forward => self.travel(ctx, true),
            Action::Navigate(view) => {
                if self.state.view != view {
                    self.push_location();
                }
                self.leave_video(ctx);
                self.state.view = view;
                if view == View::Notifications {
                    self.mark_notifications_visited();
                }
                // A browser profile made since start-up shows up too.
                if view == View::Account {
                    self.state.browser_profiles = account::browser_profiles();
                }
                self.load(view, false);
            }
            Action::OpenChannel(channel) => {
                self.open_page(ctx, View::Channel, Page::Channel(channel))
            }
            Action::OpenPlaylist { id, name } => {
                self.open_page(ctx, View::Playlist, Page::Playlist { id, name });
            }
            Action::Refresh => {
                self.load(self.state.view, true);
                // On the watch page, `r` also re-requests the details.
                if self.state.view == View::Watch
                    && let Some(video) = self.state.watching.clone()
                {
                    self.refresh_details(video);
                }
            }
            Action::SetSearchOptions(options) => {
                self.state.search_options = options;
                self.load(View::Results, true);
            }
            Action::SetChannelOrder(order) => {
                self.state.channel_order = order;
                self.load(View::Channel, true);
            }
            Action::SetHistoryQuery(query) => {
                self.state.history_query = query;
                self.state.cursor = 0;
            }
            Action::RemoveHistory(id) => {
                self.state.library.remove_history(&id);
                self.save_library();
            }
            Action::PlayPlaylist => {
                if let Some(video) = self.state.page_videos.items().first().cloned() {
                    self.open(video);
                }
            }
            Action::SetHistorySync(sync) => {
                self.state.settings.history_sync = sync;
                self.state.history_filter = crate::logic::history::HistoryFilter::All;
                self.state.cursor = 0;
                self.save_settings();
            }
            Action::SetHistoryFilter(filter) => {
                self.state.history_filter = filter;
                self.state.cursor = 0;
                self.load(View::History, false);
            }
            Action::SetSource(source) => {
                self.state.source = source;
                self.load(self.state.view, false);
            }
            Action::TogglePause => {
                if let Some(paused) = self.player.as_ref().map(|player| player.status().paused) {
                    // Unpausing shows the video: in the mini player, or back
                    // on its page once that was closed.
                    if paused {
                        self.state.cast.resume_local_playback();
                        self.show_playing();
                    }
                    self.with_player(|player| player.set_paused(!paused));
                }
            }
            Action::SetPaused(paused) => {
                if !paused {
                    self.state.cast.resume_local_playback();
                    self.show_playing();
                }
                self.with_player(|player| player.set_paused(paused));
            }
            // A seek before the video has opened would fail in mpv: a
            // shortcut typed as the window appears, or the desktop's media
            // controls while it loads.
            Action::SeekTo { .. } | Action::SeekBy(_)
                if !self
                    .player
                    .as_ref()
                    .is_some_and(|player| player.status().seekable) => {}
            Action::SeekTo { seconds, exact } => {
                self.with_player(|player| player.seek_to(seconds, exact));
                self.media.seeked(seconds);
            }
            Action::SeekBy(seconds) => {
                self.show_osd(if seconds < 0.0 {
                    format!("−{:.0} s", -seconds)
                } else {
                    format!("+{seconds:.0} s")
                });
                self.with_player(|player| player.seek_by(seconds));
                let at = self
                    .player
                    .as_ref()
                    .map_or(seconds, |player| player.status().position + seconds);
                self.media.seeked(at);
            }
            Action::SetVolume(volume) => {
                self.state.settings.volume = volume;
                if self
                    .player
                    .as_ref()
                    .is_some_and(|player| player.status().muted)
                {
                    self.with_player(|player| player.set_muted(false));
                }
                self.with_player(|player| player.set_volume(volume));
                self.volume_changed = Some(Instant::now());
                ctx.request_repaint_after(VOLUME_SAVE_AFTER);
            }
            Action::ToggleFullscreen => self.set_fullscreen(ctx, !self.state.fullscreen),
            Action::ExitFullscreen => self.set_fullscreen(ctx, false),
            Action::ToggleTheater => {
                self.state.settings.theater = !self.state.settings.theater;
                self.save_settings();
            }
            Action::ToggleSidebar => {
                apply_sidebar_action(
                    &mut self.state,
                    Action::ToggleSidebar,
                    ctx.content_rect().width(),
                );
                self.save_settings();
            }
            Action::ToggleAudioOnly => self.toggle_audio_only(),
            Action::SetQuality(quality) => self.set_quality(quality),
            Action::SetTheme(choice) => self.set_theme(choice),
            Action::SetLanguage(language) => {
                self.state.settings.language = language;
                self.state.osd = None;
                self.state.notice = None;
                self.save_settings();
                ctx.request_repaint();
            }
            Action::SetInterfaceFont(choice) => {
                self.state.settings.interface_font = choice;
                self.save_settings();
                // The search for other scripts' faces runs again; once the
                // first has finished it answers at once.
                let fonts = theme::SystemFonts::find(choice);
                self.fonts = (!fonts.install(ctx)).then_some(fonts);
                self.state.system_font_missing = theme::system_face_missing();
                ctx.request_repaint();
            }
            Action::ReportInterfaceFont => {
                // fc-match runs off the interface thread; nothing is sent,
                // the browser opens at a form the user submits or closes.
                let ctx = ctx.clone();
                let _ = std::thread::Builder::new()
                    .name("youtfast-font-report".into())
                    .spawn(move || {
                        let url = crate::appearance::font_report::Report::gather().issue_url();
                        ctx.open_url(egui::OpenUrl::new_tab(url));
                        ctx.request_repaint();
                    });
            }
            Action::ToggleSubscription(channel) => {
                self.state.library.toggle_subscription(channel);
                self.save_library();
                // The feed shows the change on its next visit.
                self.state.feed = Listing::Idle;
            }
            Action::ToggleWatchLater(video) => {
                self.state.library.toggle_watch_later(video);
                self.save_library();
            }
            Action::ClearHistory => {
                self.state.library.clear_history();
                self.save_library();
            }
            Action::ImportTakeout => {
                let picked = rfd::AsyncFileDialog::new()
                    .set_title(crate::appearance::i18n::gettext(
                        self.state.settings.language,
                        "Import from Google Takeout",
                    ))
                    .add_filter(
                        crate::appearance::i18n::gettext(
                            self.state.settings.language,
                            "Takeout subscriptions or history",
                        ),
                        &["csv", "json"],
                    )
                    .pick_file();
                self.backend.send(Command::ImportTakeout(Box::pin(picked)));
            }
            Action::AddAccountFile => self.add_account_file(),
            Action::AddAccountBrowser => self.add_account_browser(),
            Action::AddAccountProfile(index) => self.add_account_profile(index),
            Action::ToggleAccountAdvanced => {
                self.state.account_advanced = !self.state.account_advanced;
            }
            Action::SetAccountName(name) => self.state.account_name = name,
            Action::SetAccountProfile(profile) => self.state.account_profile = profile,
            Action::SwitchAccount(id) => self.switch_account(id),
            Action::RenameAccount { id, name } => self.rename_account(id, &name),
            Action::SetRenameDraft { id, name } => {
                if let Some((editing, draft)) = &mut self.state.renaming
                    && *editing == id
                {
                    *draft = name;
                }
            }
            Action::MarkRenaming { id, name } => {
                self.state.renaming = Some((id, name));
                ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("rename account")));
            }
            Action::MarkRemoving(id) => self.state.removing = Some(id),
            Action::CancelRemoving => self.state.removing = None,
            Action::RemoveAccount(id) => self.remove_account(id),
            Action::SignOut => self.sign_out(),
            Action::SetBrowser(browser) => self.state.browser = browser,
            Action::DismissNotice => self.state.notice = None,
            Action::LoadMoreComments => self.load_more_comments(),
            Action::ToggleReplies(id) => self.toggle_replies(id),
            Action::ToggleDescription => {
                self.state.description_open = !self.state.description_open;
            }
            Action::ToggleCommentText(id) => {
                if !self.state.comments_open.remove(&id) {
                    self.state.comments_open.insert(id);
                }
            }
            Action::Rate(rating) => self.rate(rating),
            Action::LoadSaveTargets => self.load_save_targets(),
            Action::SetSaved { playlist, saved } => self.set_saved(playlist, saved),
            Action::SetShareAtTime(on) => self.state.share_at_time = on,
            Action::SetFeedFilter(filter) => self.state.feed_filter = filter,
            Action::CopyLink(url) => {
                ctx.copy_text(url);
                self.state.notice = Some("Link copied".to_owned());
            }
            Action::OpenUrl(url) => {
                // Web pages, and the share menu's e-mail.
                if url.starts_with("https://")
                    || url.starts_with("http://")
                    || url.starts_with("mailto:")
                {
                    ctx.open_url(egui::OpenUrl::new_tab(url));
                }
            }
            Action::SetChannelTab(tab) => self.set_channel_tab(tab),
            Action::CheckAlertsNow => self.check_alerts_now(),
            Action::MarkAlertsSeen => self.mark_alerts_seen(),
            Action::SetAlerts(on) => self.set_alerts(on),
            Action::SetDesktopNotifications(on) => {
                self.state.settings.desktop_notifications = on;
                self.save_settings();
            }
            Action::SetAlertsEvery(minutes) => self.set_alerts_every(minutes),
            Action::SetChannelQuery(text) => self.state.channel_query = text,
            Action::LoadMore => self.load_more(),
            Action::SetSpeed(speed) => self.set_speed(speed),
            Action::ToggleMute => self.toggle_mute(),
            Action::FrameStep(forward) => self.with_player(|player| player.frame_step(forward)),
            Action::SeekFraction(fraction) => self.seek_fraction(fraction),
            Action::SetSubtitle(id) => self.set_subtitle(id),
            Action::ToggleCaptions => self.toggle_captions(),
            Action::GoLive => self.go_live(),
            Action::PlayNext => {
                if !self.await_playlist_page(PlaylistAdvance::Manual) {
                    self.play_next();
                }
            }
            Action::PlayPrevious => self.play_previous(),
            Action::CancelAutoplay => {
                self.state.autoplay = None;
                self.state.playing_waiting = None;
            }
            Action::SetAutoplay(on) => {
                self.state.set_autoplay_choice(on);
                self.save_settings();
            }
            Action::Enqueue { video, next } => self.enqueue(video, next),
            Action::Dequeue(id) => {
                self.state.queue.retain(|video| video.id != id);
                self.state.refresh_autoplay();
            }
            Action::ClearQueue => {
                self.state.queue.clear();
                self.state.refresh_autoplay();
            }
            Action::SetSponsorBlock(on) => self.set_sponsorblock(on),
            Action::SkipSegment => self.skip_segment(),
            Action::ReturnToWatch => self.return_to_watch(),
            Action::CloseMiniPlayer => self.close_mini_player(),
            Action::CheckForUpdates => {
                if !matches!(
                    self.state.update,
                    crate::desktop::updater::UpdateState::Checking
                        | crate::desktop::updater::UpdateState::Installing(_)
                ) {
                    self.state.update = crate::desktop::updater::UpdateState::Checking;
                    self.backend.send(Command::CheckForUpdates { manual: true });
                }
            }
            Action::InstallUpdate => {
                if let crate::desktop::updater::UpdateState::Available(release) =
                    self.state.update.clone()
                {
                    if release.can_self_update() {
                        self.state.update =
                            crate::desktop::updater::UpdateState::Installing(release.clone());
                        self.backend.send(Command::InstallUpdate {
                            release,
                            auto: false,
                        });
                    } else {
                        self.apply(ctx, Action::OpenUrl(release.html_url));
                    }
                }
            }
            Action::SetUpdateNotifications(on) => {
                self.state.settings.update_notifications = on;
                self.save_settings();
                if on
                    && matches!(
                        self.state.update,
                        crate::desktop::updater::UpdateState::Idle
                    )
                {
                    self.state.update = crate::desktop::updater::UpdateState::Checking;
                    self.backend
                        .send(Command::CheckForUpdates { manual: false });
                }
            }
            Action::SetAutoUpdate(on) => {
                self.state.settings.auto_update = on;
                self.save_settings();
                if on {
                    if let crate::desktop::updater::UpdateState::Available(release) =
                        self.state.update.clone()
                        && crate::desktop::updater::should_auto_update(true, release.kind)
                        && release.can_self_update()
                    {
                        self.state.update =
                            crate::desktop::updater::UpdateState::Installing(release.clone());
                        self.backend.send(Command::InstallUpdate {
                            release,
                            auto: true,
                        });
                    } else if matches!(
                        self.state.update,
                        crate::desktop::updater::UpdateState::Idle
                    ) {
                        self.state.update = crate::desktop::updater::UpdateState::Checking;
                        self.backend
                            .send(Command::CheckForUpdates { manual: false });
                    }
                }
            }
            Action::ToggleFullChangelog => {
                self.state.full_changelog_open = !self.state.full_changelog_open;
            }
        }
    }

    fn on_update_checked(
        &mut self,
        result: Result<Option<crate::desktop::updater::ReleaseInfo>, String>,
        manual: bool,
    ) {
        use crate::desktop::updater::{UpdateState, should_auto_update};
        match result {
            Ok(Some(release)) => {
                if should_auto_update(self.state.settings.auto_update, release.kind)
                    && release.can_self_update()
                {
                    self.state.update = UpdateState::Installing(release.clone());
                    self.backend.send(Command::InstallUpdate {
                        release,
                        auto: true,
                    });
                } else {
                    let already_notified = self.state.settings.last_notified_version.as_deref()
                        == Some(release.version.as_str());
                    if self.state.settings.update_notifications && (!already_notified || manual) {
                        self.state.settings.last_notified_version = Some(release.version.clone());
                        self.save_settings();
                        self.backend.send(Command::NotifyUpdate {
                            version: release.version.clone(),
                            kind: release.kind,
                            auto_installed: false,
                            language: self.state.settings.language,
                        });
                    }
                    self.state.notice = Some(if release.kind.is_major() {
                        "A new major version of YouTFast is available.".to_owned()
                    } else {
                        "A new version of YouTFast is available.".to_owned()
                    });
                    self.state.update = UpdateState::Available(release);
                }
            }
            Ok(None) => {
                self.state.update = UpdateState::UpToDate;
                if manual {
                    self.state.notice = Some("YouTFast is up to date.".to_owned());
                }
            }
            Err(error) => {
                self.state.update = UpdateState::Failed(error.clone());
                if manual {
                    self.state.notice = Some(error);
                }
            }
        }
    }

    fn on_update_installed(
        &mut self,
        release: crate::desktop::updater::ReleaseInfo,
        auto: bool,
        result: Result<(), String>,
    ) {
        use crate::desktop::updater::UpdateState;
        match result {
            Ok(()) => {
                self.state.settings.last_updated_version = Some(release.version.clone());
                self.state.settings.last_updated_changelog = Some(release.changelog.clone());
                self.state.full_changelog_open = true;
                self.save_settings();
                if auto && self.state.settings.update_notifications {
                    self.backend.send(Command::NotifyUpdate {
                        version: release.version.clone(),
                        kind: release.kind,
                        auto_installed: true,
                        language: self.state.settings.language,
                    });
                }
                self.state.notice =
                    Some("Update installed. Restart YouTFast to use the new version.".to_owned());
                self.state.update = UpdateState::Installed {
                    version: release.version,
                    kind: release.kind,
                    changelog: release.changelog,
                    auto,
                };
            }
            Err(error) => {
                self.state.notice = Some(error.clone());
                self.state.update = UpdateState::Failed(error);
            }
        }
    }

    fn save_library(&self) {
        self.library_revision.set(self.library_revision.get() + 1);
        self.backend
            .send(Command::SaveLibrary(self.state.library.clone()));
    }

    fn save_settings(&self) {
        self.backend
            .send(Command::SaveSettings(Box::new(self.state.settings.clone())));
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.fonts.as_ref().is_some_and(|fonts| fonts.poll(ctx)) {
            self.fonts = None;
        }
        self.state.system_font_missing = theme::system_face_missing();
        self.poll();
        self.poll_cast(ctx);
        ui::images::forget_unseen(ctx);
        for (url, manual) in ui::images::drain_expired() {
            self.refresh_comment_image(url, manual);
        }
        // Thumbnails drawn from the network go to the disk cache.
        let wanted = ui::images::drain_wanted();
        if !wanted.is_empty() {
            self.backend.send(Command::CacheThumbnails(wanted));
        }
        // What later launches sent to the running copy.
        let remote: Vec<instance::Remote> = std::mem::take(
            &mut *self
                .remote
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        for remote in remote {
            match remote {
                instance::Remote::Show => focus(ctx),
                instance::Remote::Open(text) => {
                    focus(ctx);
                    self.apply(ctx, Action::Search(text));
                }
                instance::Remote::ReloadThemes => self.load_themes(),
            }
        }
        // What the desktop's media controls asked for, and what they show.
        for effect in self.media.commands(&self.state) {
            match effect {
                crate::desktop::now_playing::Effect::Action(action) => self.apply(ctx, *action),
                crate::desktop::now_playing::Effect::Raise => focus(ctx),
                crate::desktop::now_playing::Effect::Quit => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close)
                }
            }
        }
        self.media
            .update(&self.state, self.player.as_ref().map(Mpv::status));
        // The keyboard cursor restarts at the top of a changed list.
        let key = (self.state.view, ui::cursor_key(&self.state));
        if key != self.cursor_key {
            self.state.cursor = 0;
            self.cursor_key = key;
        }
        self.playback_tick(ctx);
        self.prefetch_playlist();
        self.suggest_tick(ctx);
        // The volume, once it has stayed still for a moment.
        if self
            .volume_changed
            .is_some_and(|changed| changed.elapsed() >= VOLUME_SAVE_AFTER)
        {
            self.volume_changed = None;
            self.save_settings();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.update_theme(ui.ctx());
        let mut actions = Vec::new();
        self.state.can_go_back = !self.back.is_empty();
        self.state.can_go_forward = !self.forward.is_empty();
        let status = self.player.as_ref().map(Mpv::status);
        ui::show(
            ui,
            &mut self.state,
            status,
            self.surface.as_ref(),
            &mut actions,
        );
        ui::release_speed_hold(ui.ctx(), &self.state, &mut actions);
        self.theme_transition.paint(ui.ctx());
        let ctx = ui.ctx().clone();
        for action in actions {
            self.apply(&ctx, action);
        }
    }

    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        // Finish cancelling any pending TV helper before the process can exit.
        self.cast_worker.take();
        // The position of the video playing, written here and now: the
        // backend may not get to a queued save before the process ends.
        if let (Some(video), Some(player)) = (&self.state.watching, &self.player)
            && !video.live
        {
            let status = player.status();
            self.state
                .library
                .set_position(&video.id, status.position, status.duration);
        }
        // A volume changed in the last second is not saved yet.
        self.backend
            .save_final(&self.state.library, &self.state.settings, &self.dirs);
        // The renderer goes first, with the GL context current, and mpv
        // after it, as libmpv requires.
        if let (Some(surface), Some(gl)) = (self.surface.take(), gl) {
            surface.destroy(gl);
        }
        self.player = None;
    }
}

/// Brings the window up, as a second launch or a desktop click asks.
fn focus(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
}

/// The display mpv needs to share hardware-decoded frames with OpenGL.
fn native_display(cc: &eframe::CreationContext<'_>) -> NativeDisplay {
    match cc.display_handle().map(|handle| handle.as_raw()) {
        Ok(RawDisplayHandle::Wayland(handle)) => NativeDisplay::Wayland(handle.display.as_ptr()),
        Ok(RawDisplayHandle::Xlib(handle)) => {
            handle.display.map_or(NativeDisplay::None, |display| {
                NativeDisplay::X11(display.as_ptr())
            })
        }
        _ => NativeDisplay::None,
    }
}

mod accounts;
pub mod cast;
mod history_sync;
mod loading;
mod navigation;
mod notifications;
pub mod player;
mod themes;
mod video_actions;
mod watch;
