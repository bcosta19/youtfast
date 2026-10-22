//! The application: its state, and the actions the views ask for.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use anyhow::Context as _;
use eframe::glow;
use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

use crate::backend::{
    Backend, Command, CommentsKey, Event, PageItems, Payload, Request, SignInMethod, Slot,
};
use crate::innertube;
use crate::instance::{self, Queue as RemoteQueue};
use crate::library::{Account, Import, Library};
use crate::model::{
    AccountSource, Action, Channel, ChannelAbout, ChannelHeader, ChannelTab, CommentRow, Details,
    FeedFilter, Fetch, HistoryRow, Listing, PlaylistSummary, Quality, Rating, Resolution, Results,
    SaveTarget, Segment, Source, Video, View,
};
use crate::mpv::{self, Mpv, NativeDisplay, VideoSurface};
use crate::paths::Dirs;
use crate::settings::{Settings, ThemeChoice};
use crate::theme::{self, Palette};
use crate::{account, alerts, ui, util, youtube};

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

/// What the views draw from.
#[derive(Default)]
pub struct State {
    pub view: View,
    /// The search field's text.
    pub query: String,
    pub results: Results,
    pub search_options: innertube::SearchOptions,
    pub search_query: String,
    pub results_channels: Listing<Channel>,
    pub results_playlists: Listing<PlaylistSummary>,
    pub history_query: String,
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
    pub watching: Option<Video>,
    /// The list the video being watched was opened from.
    pub up_next: Vec<Video>,
    /// Shown over the video in place of the picture.
    pub player_error: Option<String>,
    pub fullscreen: bool,
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
    pub extras: Fetch<crate::video_extras::Extras>,
    pub transcript_open: bool,
    pub transcript_query: String,
    pub transcript_follow: bool,
    pub playing_playlist: Option<String>,
    pub playing_more: Option<innertube::More>,
    pub playing_more_state: Fetch<()>,
    pub playing_waiting: Option<PlaylistAdvance>,
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

    /// The feed's videos home shows: For you's, or the first
    /// `feed_shown` of the feed through the chip.
    pub fn visible_feed(&self) -> Vec<&Video> {
        if self.feed_filter == FeedFilter::ForYou {
            return self.for_you.items().iter().collect();
        }
        let now = util::now();
        let videos = self.feed.items();
        videos[..videos.len().min(self.feed_shown)]
            .iter()
            .filter(|video| {
                self.feed_filter
                    .keeps(video, self.library.watched(&video.id), now)
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
    media: crate::now_playing::MediaControls,
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
        fastframe_icons::install::<crate::icons::Icon>(ctx);
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
            generations: HashMap::new(),
            details_generation: 0,
            playing_generation: 0,
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
            media: crate::now_playing::MediaControls::start(ctx),
            fonts,
            _instance: guard,
        };
        app.set_alerts_timer();
        app.refresh_theme_list();
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
            Event::SaveTargets { id, result } => self.on_save_targets(id, result),
            Event::SaveFailed {
                id,
                playlist,
                saved,
                error,
            } => self.on_save_failed(id, playlist, saved, error),
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
                if self
                    .state
                    .watching
                    .as_ref()
                    .is_some_and(|video| video.id == id)
                {
                    self.state.segments = segments;
                    self.state.skipped.clear();
                }
            }
            Event::Notice(notice) => self.state.notice = Some(notice),
        }
    }

    fn apply(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Search(text) => {
                let text = text.trim();
                if text.is_empty() {
                    return;
                }
                if let Some(id) = youtube::video_id_from_link(text) {
                    self.open(Video::from_id(id));
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
            Action::SetSource(source) => {
                self.state.source = source;
                self.load(self.state.view, false);
            }
            Action::TogglePause => {
                if let Some(paused) = self.player.as_ref().map(|player| player.status().paused) {
                    // Unpausing shows the video: in the mini player, or back
                    // on its page once that was closed.
                    if paused {
                        self.show_playing();
                    }
                    self.with_player(|player| player.set_paused(!paused));
                }
            }
            Action::SetPaused(paused) => {
                if !paused {
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
                self.state.settings.sidebar_collapsed = !self.state.settings.sidebar_collapsed;
                self.save_settings();
            }
            Action::SetQuality(quality) => self.set_quality(quality),
            Action::SetTheme(choice) => self.set_theme(choice),
            Action::SetLanguage(language) => {
                self.state.settings.language = language;
                self.state.osd = None;
                self.state.notice = None;
                self.save_settings();
                ctx.request_repaint();
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
                    .set_title(crate::i18n::gettext(
                        self.state.settings.language,
                        "Import from Google Takeout",
                    ))
                    .add_filter(
                        crate::i18n::gettext(
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
            Action::MarkRenaming { id, name } => self.state.renaming = Some((id, name)),
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
            Action::Dequeue(id) => self.state.queue.retain(|video| video.id != id),
            Action::ClearQueue => self.state.queue.clear(),
            Action::SetSponsorBlock(on) => self.set_sponsorblock(on),
            Action::SkipSegment => self.skip_segment(),
            Action::ReturnToWatch => self.return_to_watch(),
            Action::CloseMiniPlayer => self.close_mini_player(),
        }
    }

    fn save_library(&self) {
        self.library_revision.set(self.library_revision.get() + 1);
        self.backend
            .send(Command::SaveLibrary(self.state.library.clone()));
    }

    fn save_settings(&self) {
        self.backend
            .send(Command::SaveSettings(self.state.settings.clone()));
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.fonts.as_ref().is_some_and(|fonts| fonts.poll(ctx)) {
            self.fonts = None;
        }
        self.poll();
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
                crate::now_playing::Effect::Action(action) => self.apply(ctx, *action),
                crate::now_playing::Effect::Raise => focus(ctx),
                crate::now_playing::Effect::Quit => {
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
mod loading;
mod navigation;
mod notifications;
pub mod player;
mod themes;
mod video_actions;
mod watch;
