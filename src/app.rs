//! The application: its state, and the actions the views ask for.

use std::collections::HashMap;

use anyhow::Context as _;
use eframe::glow;
use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

use crate::backend::{
    Backend, Command, CommentsKey, Event, PageItems, Payload, Request, SignInMethod, Slot,
};
use crate::innertube;
use crate::instance::{self, Queue as RemoteQueue};
use crate::library::{Account, Library};
use crate::model::{
    AccountSource, Action, Channel, ChannelAbout, ChannelHeader, ChannelTab, CommentRow, Details,
    FeedFilter, Fetch, HistoryRow, Listing, PlaylistSummary, Quality, Resolution, Results, Source,
    Video, View,
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
    /// The signed-in account's lists.
    pub account_subscriptions: Listing<Channel>,
    pub account_history: Listing<HistoryRow>,
    pub account_watch_later: Listing<Video>,
    pub liked: Listing<Video>,
    pub playlists: Listing<PlaylistSummary>,
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
    /// Each comment's replies by comment id, when opened.
    pub replies: HashMap<String, Fetch<Vec<CommentRow>>>,
    /// The description expanded past its first rows.
    pub description_open: bool,
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
}

impl State {
    pub fn signed_in(&self) -> bool {
        matches!(self.account, AccountState::SignedIn(_))
    }

    /// The account the app is signed in as, by its library id.
    pub fn active_account(&self) -> Option<&Account> {
        self.library
            .accounts
            .iter()
            .find(|account| Some(account.id) == self.library.active_account)
    }

    /// Where account-or-device pages take their list from now.
    pub fn source(&self) -> Source {
        if self.signed_in() {
            self.source
        } else {
            Source::Device
        }
    }

    /// The feed's videos home shows: the first `feed_shown`, through the
    /// chip.
    pub fn visible_feed(&self) -> Vec<&Video> {
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
}

/// Where the chosen quality was kept in eframe's storage before the
/// settings file; read once to bring it over.
const QUALITY_KEY: &str = "quality";

/// How many watch pages' details are kept: some 15 to 30 KB each.
const DETAILS_KEPT: usize = 50;
/// How long kept details are shown instead of asked for again, in seconds;
/// views and likes move, and `r` asks again at once.
const DETAILS_FRESH: i64 = 30 * 60;

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
    /// The watch pages seen lately, so going back to one asks YouTube
    /// again only for its comments.
    details_cache: util::Recent<Details>,
    /// The comments page in flight, to put its token back on failure.
    more_comments_token: Option<String>,
    /// The channel the open page shows, to detect a channel switch.
    open_channel: Option<String>,
    /// The account the add form's name belongs to (id assigned on submit).
    /// Where Back goes: the pages a video, channel or playlist was opened
    /// from.
    back: Vec<View>,
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
    /// What later launches ask the running copy, drained each frame.
    remote: RemoteQueue,
    /// The desktop's media controls.
    media: crate::now_playing::MediaControls,
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
    ) -> anyhow::Result<Self> {
        let ctx = &cc.egui_ctx;
        guard.set_context(ctx);
        crate::fonts::install(ctx);
        egui_extras::install_image_loaders(ctx);
        fastframe_icons::install::<crate::icons::Icon>(ctx);
        let dirs = Dirs::new().context("could not make the data folders")?;
        let backend =
            Backend::start(ctx.clone(), dirs.clone()).context("could not start the backend")?;

        let browsers = account::installed_browsers();
        let mut settings = Settings::load(&dirs.settings());
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
            selected_theme(&settings),
            &theme_waker,
        );
        let palette = settings.palette();
        theme::apply(ctx, &palette);
        let mut state = State {
            library: Library::load(&dirs.library()),
            themes_dir: dirs.config.join("themes").display().to_string(),
            palette,
            settings,
            browser: browsers.first().copied().unwrap_or("chromium").to_owned(),
            browsers,
            ..State::default()
        };
        let (player, surface) =
            match Mpv::new(ctx, state.settings.quality.limit(), state.settings.volume) {
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
                        state.player_error =
                            Some("This window has no OpenGL context for video".into());
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
            details_cache: util::Recent::new(DETAILS_KEPT),
            more_comments_token: None,
            open_channel: None,
            back: Vec::new(),
            custom_themes,
            theme_waker,
            theme_transition: fastframe_theme::Transition::default(),
            cursor_key: (View::default(), 0),
            remote: guard.commands(),
            media: crate::now_playing::MediaControls::start(ctx),
            _instance: guard,
        };
        app.set_alerts_timer();
        app.refresh_theme_list();
        if let Some(active) = app.state.library.active_account
            && let Some(account) = app
                .state
                .library
                .accounts
                .iter()
                .find(|account| account.id == active)
                .cloned()
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

        let Some(player) = &mut self.player else {
            return;
        };
        let events = player.poll();
        // yt-dlp's answer is there once a video has loaded.
        let json = events
            .contains(&mpv::Event::Loaded)
            .then(|| player.ytdl_json())
            .flatten();
        for event in events {
            match event {
                mpv::Event::Loaded => self.state.player_error = None,
                mpv::Event::Failed(failure) => {
                    self.state.player_error = Some(failure_message(failure).into());
                }
                mpv::Event::Finished => {
                    if let Some(video) = &self.state.watching {
                        self.state.library.mark_finished(&video.id);
                        self.save_library();
                    }
                }
            }
        }
        if let Some(json) = json {
            // Kept from before a quality change until there is news.
            self.state.resolutions = youtube::available_resolutions(&json);
            if let (Some(details), Some(watching)) =
                (youtube::ytdl_details(&json), self.state.watching.as_mut())
                && details.id == watching.id
            {
                watching.complete_from(&details);
                let watching = watching.clone();
                self.state.library.update_video(&watching);
                self.save_library();
            }
        }
    }

    /// Which browser's session mpv's yt-dlp plays with; `None` plays
    /// signed out. It applies from the next video loaded.
    fn set_playback_cookies(&mut self, browser: Option<String>) {
        let Some(player) = &mut self.player else {
            return;
        };
        if let Some(browser) = &browser {
            log::info!("videos play signed in, through {browser}");
        }
        if let Err(error) = player.set_cookies_from_browser(browser) {
            log::warn!("mpv: {error}");
        }
    }

    /// Shows a video's details on the watch page, fresh or from the cache,
    /// and asks for its first comments page.
    fn show_details(&mut self, details: Details) {
        // Complete what the watch page and the library know.
        if let Some(watching) = self.state.watching.as_mut()
            && watching.id == details.id
        {
            let filled = Video {
                id: details.id.clone(),
                title: details.title.clone(),
                channel: Some(details.channel.name.clone()),
                channel_id: Some(details.channel.id.clone()),
                duration: None,
                views: Some(details.views),
                live: details.live,
                published_at: None,
                published: details.published.clone(),
                short: false,
            };
            watching.complete_from(&filled);
            let watching = watching.clone();
            self.state.library.update_video(&watching);
            self.save_library();
        }
        // The first comments page comes right behind it.
        if let Some(token) = details.comments_token.clone() {
            self.state.comments = Fetch::Loading;
            self.state.comments_visitor_data = details.visitor_data.clone();
            self.backend.send(Command::LoadComments {
                generation: self.details_generation,
                key: CommentsKey::Top,
                token,
                visitor_data: details.visitor_data.clone(),
            });
        }
        self.state.details = Fetch::Ready(details);
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Loaded {
                slot,
                generation,
                result,
            } => {
                if self.generations.get(&slot) == Some(&generation) {
                    self.fill(slot, result);
                }
            }
            Event::Details { generation, result } => {
                let result = *result;
                if generation != self.details_generation {
                    return;
                }
                match result {
                    Ok(details) => {
                        self.details_cache
                            .put(details.id.clone(), util::now(), details.clone());
                        self.show_details(details);
                    }
                    Err(error) => self.state.details = Fetch::Failed(error),
                }
            }
            Event::Comments {
                generation,
                key,
                result,
            } => {
                if generation != self.details_generation {
                    return;
                }
                match (key, result) {
                    (CommentsKey::Top, Ok((rows, next))) => {
                        let mut comments = self.state.comments.ready().cloned().unwrap_or_default();
                        comments.extend(rows);
                        self.state.comments = Fetch::Ready(comments);
                        self.state.comments_next = next;
                    }
                    (CommentsKey::Top, Err(error)) => {
                        self.state.comments = Fetch::Failed(error);
                    }
                    (CommentsKey::Replies(id), Ok((rows, _))) => {
                        self.state.replies.insert(id, Fetch::Ready(rows));
                    }
                    (CommentsKey::Replies(id), Err(error)) => {
                        self.state.replies.insert(id, Fetch::Failed(error));
                    }
                }
            }
            Event::Imported(Ok(import)) => {
                let (subscriptions, history) = self.state.library.merge(import);
                self.state.notice = Some(format!(
                    "Imported {subscriptions} new subscriptions and {history} new videos of history."
                ));
                self.save_library();
                self.state.feed = Listing::Idle;
                self.load(self.state.view, false);
            }
            Event::Imported(Err(error)) => self.state.notice = Some(error),
            Event::SigningIn => self.state.account = AccountState::SigningIn,
            Event::AccountAdded(account) => {
                self.state.library.accounts.push(account);
                self.save_library();
            }
            Event::SignedIn(id) => {
                // The browser for playback follows once it is known.
                self.set_playback_cookies(None);
                self.state.account = AccountState::SignedIn(id);
                self.state.library.active_account = Some(id);
                self.save_library();
                self.state.source = Source::Account;
                // The account's subscriptions keep their channels out of the
                // RSS half of the feed, so they come first.
                self.request(Slot::Subscriptions, Request::Subscriptions);
                self.state.feed = Listing::Idle;
                self.state.shorts = Listing::Idle;
                self.load(self.state.view, true);
            }
            Event::SignInFailed(error) => self.state.account = AccountState::Failed(error),
            Event::PlaybackCookies { account, browser } => {
                if self.state.account == AccountState::SignedIn(account) {
                    self.set_playback_cookies(Some(browser));
                }
            }
            Event::SessionExpired(error) => {
                self.set_playback_cookies(None);
                self.state.account =
                    AccountState::Failed(format!("{error} Sign in again to see your account."));
            }
            Event::SignedOut => {
                self.set_playback_cookies(None);
                self.state.account = AccountState::SignedOut;
                // The account stays listed: its session is kept to switch
                // back.
                self.state.library.active_account = None;
                self.save_library();
                self.state.account_subscriptions = Listing::Idle;
                self.state.account_history = Listing::Idle;
                self.state.account_watch_later = Listing::Idle;
                self.state.liked = Listing::Idle;
                self.state.playlists = Listing::Idle;
                self.state.feed = Listing::Idle;
                if matches!(self.state.view, View::Liked | View::Playlists) {
                    self.state.view = View::Home;
                }
                self.load(self.state.view, true);
            }
            Event::CheckAlerts => self.check_alerts(),
            Event::Notice(notice) => self.state.notice = Some(notice),
        }
    }

    /// Puts a loaded list where its slot's view reads it.
    fn fill(&mut self, slot: Slot, result: Result<Payload, String>) {
        fn put<T>(listing: &mut Listing<T>, result: Result<Vec<T>, String>) {
            *listing = match result {
                Ok(items) => Listing::Ready(items),
                Err(error) => Listing::Failed(error),
            };
        }
        let state = &mut self.state;
        let mut feed_page = None;
        match (slot, result) {
            (Slot::Search, Ok(Payload::Videos(videos))) => put(&mut state.results, Ok(videos)),
            (Slot::Feed, Ok(Payload::Feed { videos, more })) => {
                feed_page = Some(videos.clone());
                // Shorts leave the list for the shelf.
                let (shorts, videos): (Vec<Video>, Vec<Video>) =
                    videos.into_iter().partition(|video| video.short);
                if !shorts.is_empty() {
                    state.shorts = Listing::Ready(shorts);
                }
                state.feed_more = more;
                state.feed_more_state = Fetch::Idle;
                state.feed_shown = 40;
                put(&mut state.feed, Ok(videos));
                self.ask_shelf();
            }
            (Slot::Feed, Ok(Payload::More { items, more })) => {
                state.feed_more = more;
                state.feed_more_state = Fetch::Idle;
                if let PageItems::Videos(videos) = items {
                    state.feed_shown +=
                        merge_feed_page(state.feed.items_mut(), state.shorts.items_mut(), videos);
                }
            }
            (Slot::Shorts, Ok(Payload::Videos(videos))) => {
                // RSS shorts leave the list and join the shelf, after the
                // account's, deduped.
                let known: std::collections::HashSet<String> = state
                    .shorts
                    .items()
                    .iter()
                    .map(|video| video.id.clone())
                    .collect();
                state.shorts.items_mut().extend(
                    videos
                        .into_iter()
                        .filter(|video| !known.contains(&video.id)),
                );
                let on_shelf: std::collections::HashSet<String> = state
                    .shorts
                    .items()
                    .iter()
                    .map(|video| video.id.clone())
                    .collect();
                if let Listing::Ready(rows) = &mut state.feed {
                    rows.retain(|video| !on_shelf.contains(&video.id));
                }
            }
            (Slot::WatchLater, Ok(Payload::Videos(videos))) => {
                put(&mut state.account_watch_later, Ok(videos));
            }
            (Slot::Liked, Ok(Payload::Videos(videos))) => put(&mut state.liked, Ok(videos)),
            (Slot::Page, Ok(Payload::Videos(videos))) => put(&mut state.page_videos, Ok(videos)),
            (
                Slot::Page,
                Ok(Payload::ChannelPage {
                    header,
                    items,
                    more,
                }),
            ) => {
                state.channel_header = Some(header);
                state.page_more = more;
                state.page_more_state = Fetch::Idle;
                match items {
                    PageItems::Videos(videos) => put(&mut state.page_videos, Ok(videos)),
                    PageItems::Playlists(items) => put(&mut state.page_playlists, Ok(items)),
                }
            }
            (Slot::Page, Ok(Payload::About(about))) => {
                state.channel_about = Fetch::Ready(about);
            }
            (Slot::Page, Ok(Payload::More { items, more })) => {
                state.page_more = more;
                state.page_more_state = Fetch::Idle;
                match items {
                    PageItems::Videos(videos) => {
                        state.page_videos.items_mut().extend(videos);
                    }
                    PageItems::Playlists(items) => {
                        state.page_playlists.items_mut().extend(items);
                    }
                }
            }
            (Slot::Subscriptions, Ok(Payload::Channels(channels))) => {
                put(&mut state.account_subscriptions, Ok(channels));
            }
            (Slot::History, Ok(Payload::History(rows))) => {
                put(&mut state.account_history, Ok(rows))
            }
            (Slot::Playlists, Ok(Payload::Playlists(lists))) => {
                put(&mut state.playlists, Ok(lists))
            }
            (Slot::Search, Err(error)) => put(&mut state.results, Err(error)),
            (Slot::Feed, Err(error)) => {
                // A continuation failing keeps the feed already shown.
                if matches!(state.feed_more_state, Fetch::Loading) {
                    state.feed_more_state = Fetch::Failed(error);
                } else {
                    put(&mut state.feed, Err(error));
                }
            }
            (Slot::Shorts, Err(_)) => {}
            (Slot::Alerts, Ok(Payload::Feed { videos, .. })) => {
                feed_page = Some(videos);
            }
            (Slot::Alerts, Err(error)) => {
                log::warn!("the alerts check failed: {error}");
            }
            (Slot::WatchLater, Err(error)) => put(&mut state.account_watch_later, Err(error)),
            (Slot::Liked, Err(error)) => put(&mut state.liked, Err(error)),
            (Slot::Page, Err(error)) => {
                // A continuation failing keeps the rows already shown.
                if matches!(state.page_more_state, Fetch::Loading) {
                    state.page_more_state = Fetch::Failed(error);
                } else {
                    match state.channel_tab {
                        ChannelTab::About => state.channel_about = Fetch::Failed(error),
                        ChannelTab::Playlists => put(&mut state.page_playlists, Err(error)),
                        _ => put(&mut state.page_videos, Err(error)),
                    }
                }
            }
            (Slot::Subscriptions, Err(error)) => put(&mut state.account_subscriptions, Err(error)),
            (Slot::History, Err(error)) => put(&mut state.account_history, Err(error)),
            (Slot::Playlists, Err(error)) => put(&mut state.playlists, Err(error)),
            (slot, Ok(_)) => log::error!("{slot:?} received another kind of list"),
        }
        // The feed's own load doubles as a check, only while alerts are on.
        if let Some(videos) = feed_page {
            if self.state.settings.alerts {
                self.note_alerts(videos);
            } else {
                self.state.alerts_checking = false;
            }
        }
    }

    /// Asks the backend for a list, marking it loading.
    /// After the feed arrives, ask the Shorts tabs of the device
    /// subscriptions it shows (up to 12, ordered by their newest video),
    /// for Home's shelf.
    fn ask_shelf(&mut self) {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let channels: Vec<String> = self
            .state
            .feed
            .items()
            .iter()
            .filter_map(|video| video.channel_id.clone())
            .filter(|id| {
                self.state
                    .library
                    .subscriptions
                    .iter()
                    .any(|channel| channel.id == *id)
            })
            .filter(|id| seen.insert(id.clone()))
            .take(12)
            .collect();
        if channels.is_empty() {
            return;
        }
        // The shelf keeps what the feed already put there.
        self.send_request(Slot::Shorts, Request::Shorts { channels });
    }

    /// `request` without touching the listing: a request whose answer
    /// augments what is already shown (the shelf's shorts).
    fn send_request(&mut self, slot: Slot, request: Request) {
        self.generation += 1;
        self.generations.insert(slot, self.generation);
        self.backend.send(Command::Load {
            slot,
            generation: self.generation,
            request,
        });
    }

    fn request(&mut self, slot: Slot, request: Request) {
        self.generation += 1;
        self.generations.insert(slot, self.generation);
        let state = &mut self.state;
        match slot {
            Slot::Search => state.results = Listing::Loading,
            Slot::Feed => state.feed = Listing::Loading,
            Slot::Subscriptions => state.account_subscriptions = Listing::Loading,
            Slot::History => state.account_history = Listing::Loading,
            Slot::WatchLater => state.account_watch_later = Listing::Loading,
            Slot::Liked => state.liked = Listing::Loading,
            Slot::Playlists => state.playlists = Listing::Loading,
            Slot::Page => state.page_videos = Listing::Loading,
            Slot::Shorts => state.shorts = Listing::Loading,
            Slot::Alerts => {}
        }
        self.backend.send(Command::Load {
            slot,
            generation: self.generation,
            request,
        });
    }

    /// The check the timer (or `[ check now ]`) runs: the feed's sources
    /// again, on its own slot so Home's list is undisturbed.
    fn check_alerts(&mut self) {
        if !self.state.settings.alerts || self.state.alerts_checking {
            return;
        }
        self.state.alerts_checking = true;
        let account = self.state.signed_in();
        let channels: Vec<Channel> = self
            .state
            .library
            .subscriptions
            .iter()
            .filter(|channel| !self.state.subscribed_in_account(&channel.id))
            .cloned()
            .collect();
        self.send_request(Slot::Alerts, Request::Alerts { channels, account });
    }

    /// What a check (or the feed's own load) found: keep the unseen,
    /// notify the rest, stamp the library.
    fn note_alerts(&mut self, videos: Vec<Video>) {
        self.state.alerts_checking = false;
        self.state.last_checked = Some(util::now());
        let Some(found) = alerts::compute(
            videos,
            self.state.library.alerts_seen_at,
            self.state.library.alerts_notified_at,
            util::now(),
        ) else {
            return;
        };
        let changed = self.state.library.alerts_seen_at != Some(found.seen_at)
            || self.state.library.alerts_notified_at != found.notified_at;
        self.state.library.alerts_seen_at = Some(found.seen_at);
        self.state.library.alerts_notified_at = found.notified_at;
        if changed {
            self.save_library();
        }
        // `new` is everything unseen since the last visit; replace, so a
        // second check does not repeat the badge's rows.
        self.state.alerts = found.new;
        if self.state.settings.desktop_notifications
            && self.state.settings.alerts
            && !found.to_notify.is_empty()
        {
            self.backend.send(Command::Notify {
                videos: found.to_notify,
            });
        }
    }

    /// Starts or stops the alerts timer to match the settings.
    fn set_alerts_timer(&self) {
        let minutes = if self.state.settings.alerts {
            self.state.settings.alerts_every_minutes
        } else {
            0
        };
        self.backend.send(Command::SetAlertsEvery(minutes));
    }

    /// The account the add form describes:  must be set and new;
    /// its id is the first free one.
    fn new_account(&self, source: AccountSource) -> Option<Account> {
        let name = self.state.account_name.trim();
        if name.is_empty() || self.state.library.account_name_taken(name) {
            return None;
        }
        let id = self
            .state
            .library
            .accounts
            .iter()
            .map(|account| account.id)
            .max()
            .unwrap_or(0)
            + 1;
        Some(Account {
            id,
            name: name.to_owned(),
            source,
        })
    }

    /// Loads what a view shows: always when `force`d (Refresh, a new
    /// sign-in), otherwise only what is not there yet or failed.
    fn load(&mut self, view: View, force: bool) {
        fn stale<T>(listing: &Listing<T>) -> bool {
            matches!(listing, Listing::Idle | Listing::Failed(_))
        }
        fn stale_fetch<T>(fetch: &Fetch<T>) -> bool {
            matches!(fetch, Fetch::Idle | Fetch::Failed(_))
        }
        let account = self.state.source() == Source::Account;
        let state = &self.state;
        match view {
            View::Home if force || stale(&state.feed) => {
                let signed_in = state.signed_in();
                let channels: Vec<Channel> = state
                    .library
                    .subscriptions
                    .iter()
                    .filter(|channel| !state.subscribed_in_account(&channel.id))
                    .cloned()
                    .collect();
                self.state.feed_shown = 40;
                self.state.feed_more = None;
                self.state.feed_more_state = Fetch::Idle;
                self.state.shorts = Listing::Idle;
                if channels.is_empty() && !signed_in {
                    self.state.feed = Listing::Ready(Vec::new());
                } else {
                    self.request(
                        Slot::Feed,
                        Request::Feed {
                            channels,
                            account: signed_in,
                        },
                    );
                }
            }
            View::Subscriptions if account && (force || stale(&state.account_subscriptions)) => {
                self.request(Slot::Subscriptions, Request::Subscriptions);
            }
            View::History if account && (force || stale(&state.account_history)) => {
                self.request(Slot::History, Request::History);
            }
            View::WatchLater if account && (force || stale(&state.account_watch_later)) => {
                self.request(Slot::WatchLater, Request::WatchLater);
            }
            View::Liked if force || stale(&state.liked) => {
                self.request(Slot::Liked, Request::Liked)
            }
            View::Playlists if force || stale(&state.playlists) => {
                self.request(Slot::Playlists, Request::Playlists);
            }
            View::Playlist if force || stale(&state.page_videos) => {
                if let Some(Page::Playlist { id, .. }) = &state.page {
                    let request = Request::Playlist(id.clone());
                    self.request(Slot::Page, request);
                }
            }
            View::Channel => {
                let Some(Page::Channel(channel)) = &state.page else {
                    return;
                };
                let id = channel.id.clone();
                match &self.state.channel_tab {
                    // The About pane has no rows to refresh.
                    ChannelTab::About => {
                        if force || stale_fetch(&self.state.channel_about) {
                            self.generation += 1;
                            self.generations.insert(Slot::Page, self.generation);
                            self.state.channel_about = Fetch::Loading;
                            self.backend.send(Command::Load {
                                slot: Slot::Page,
                                generation: self.generation,
                                request: Request::ChannelAbout(id),
                            });
                        }
                    }
                    ChannelTab::Playlists => {
                        if force || stale(&self.state.page_playlists) {
                            self.generation += 1;
                            self.generations.insert(Slot::Page, self.generation);
                            self.state.page_playlists = Listing::Loading;
                            self.backend.send(Command::Load {
                                slot: Slot::Page,
                                generation: self.generation,
                                request: Request::ChannelTab {
                                    id,
                                    tab: ChannelTab::Playlists,
                                },
                            });
                        }
                    }
                    tab if force || stale(&self.state.page_videos) => {
                        self.request(
                            Slot::Page,
                            Request::ChannelTab {
                                id,
                                tab: tab.clone(),
                            },
                        );
                    }
                    _ => {}
                }
            }
            _ => {}
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
                self.leave_video(ctx);
                self.back.clear();
                self.state.view = View::Results;
                self.request(Slot::Search, Request::Search(text.to_owned()));
            }
            Action::Open(video) => self.open(video),
            Action::Back => {
                self.leave_video(ctx);
                self.state.view = self.back.pop().unwrap_or(View::Home);
                self.load(self.state.view, false);
            }
            Action::Navigate(view) => {
                self.leave_video(ctx);
                self.back.clear();
                self.state.view = view;
                if view == View::Notifications {
                    // Opening the page marks it all seen, but the rows
                    // stay drawn for this visit.
                    self.state.alerts_visit = self.state.alerts.clone();
                    self.state.alerts.clear();
                    if self.state.library.alerts_seen_at != Some(util::now()) {
                        self.state.library.alerts_seen_at = Some(util::now());
                        self.save_library();
                    }
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
                    self.generation += 1;
                    self.details_generation = self.generation;
                    self.state.details = Fetch::Loading;
                    self.state.comments = Fetch::Idle;
                    self.state.comments_next = None;
                    self.state.comments_more = Fetch::Idle;
                    self.more_comments_token = None;
                    self.state.replies.clear();
                    self.backend.send(Command::LoadDetails {
                        generation: self.details_generation,
                        id: video.id,
                    });
                }
            }
            Action::SetSource(source) => {
                self.state.source = source;
                self.load(self.state.view, false);
            }
            Action::TogglePause => {
                if let Some(paused) = self.player.as_ref().map(|player| player.status().paused) {
                    // Unpausing shows the video: come back to its page.
                    if paused {
                        self.return_to_watch();
                    }
                    self.with_player(|player| player.set_paused(!paused));
                }
            }
            Action::SetPaused(paused) => {
                if !paused {
                    self.return_to_watch();
                }
                self.with_player(|player| player.set_paused(paused));
            }
            Action::SeekTo { seconds, exact } => {
                self.with_player(|player| player.seek_to(seconds, exact));
                self.media.seeked(seconds);
            }
            Action::SeekBy(seconds) => {
                self.with_player(|player| player.seek_by(seconds));
                let at = self
                    .player
                    .as_ref()
                    .map_or(seconds, |player| player.status().position + seconds);
                self.media.seeked(at);
            }
            Action::SetVolume(volume) => {
                // Kept in the settings, saved in `on_exit`: not per tick.
                self.state.settings.volume = volume;
                self.with_player(|player| player.set_volume(volume));
            }
            Action::ToggleFullscreen => self.set_fullscreen(ctx, !self.state.fullscreen),
            Action::ExitFullscreen => self.set_fullscreen(ctx, false),
            Action::SetQuality(quality) => self.set_quality(quality),
            Action::SetTheme(choice) => {
                // The listed palette is drawn at once, not after the rescan.
                if let ThemeChoice::Custom(filename) = &choice
                    && let Some(theme) = self.custom_themes.find(filename)
                {
                    self.state.settings.custom_theme_cache = Some(theme.clone());
                }
                self.state.settings.theme = choice;
                self.save_settings();
                // A custom choice scans again so its cache fills at once.
                if matches!(self.state.settings.theme, ThemeChoice::Custom(_)) {
                    self.load_themes();
                } else {
                    self.refresh_theme_list();
                }
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
                    .set_title("Import from Google Takeout")
                    .add_filter("Takeout subscriptions or history", &["csv", "json"])
                    .pick_file();
                self.backend.send(Command::ImportTakeout(Box::pin(picked)));
            }
            Action::AddAccountFile => {
                // The name makes the account; the pick happens on the way.
                let Some(account) = self.new_account(AccountSource::File) else {
                    return;
                };
                let picked = rfd::AsyncFileDialog::new()
                    .set_title("Choose the cookies.txt exported from YouTube")
                    .add_filter("cookies.txt", &["txt"])
                    .pick_file();
                self.backend.send(Command::AddAccount {
                    account,
                    method: SignInMethod::File(Box::pin(picked)),
                });
            }
            Action::AddAccountBrowser => {
                let mut spec = self.state.browser.clone();
                let profile = self.state.account_profile.trim();
                if !profile.is_empty() {
                    spec = format!("{spec}:{profile}");
                }
                let Some(account) = self.new_account(AccountSource::Browser(spec)) else {
                    return;
                };
                self.backend.send(Command::AddAccount {
                    account,
                    method: SignInMethod::Browser,
                });
            }
            Action::SetAccountName(name) => self.state.account_name = name,
            Action::SetAccountProfile(profile) => self.state.account_profile = profile,
            Action::SwitchAccount(id) => {
                if self.state.library.active_account != Some(id)
                    && let Some(account) = self
                        .state
                        .library
                        .accounts
                        .iter()
                        .find(|account| account.id == id)
                        .cloned()
                {
                    self.state.account = AccountState::SigningIn;
                    self.backend.send(Command::Switch(account));
                }
            }
            Action::RenameAccount { id, name } => {
                let name = name.trim();
                if !name.is_empty()
                    && let Some(account) = self
                        .state
                        .library
                        .accounts
                        .iter_mut()
                        .find(|account| account.id == id)
                {
                    account.name = name.to_owned();
                    self.save_library();
                }
                self.state.renaming = None;
            }
            Action::MarkRenaming { id, name } => self.state.renaming = Some((id, name)),
            Action::MarkRemoving(id) => self.state.removing = Some(id),
            Action::CancelRemoving => self.state.removing = None,
            Action::RemoveAccount(id) => {
                if self.state.library.active_account == Some(id) {
                    self.backend.send(Command::SignOut);
                }
                self.state
                    .library
                    .accounts
                    .retain(|account| account.id != id);
                self.save_library();
                self.backend.send(Command::RemoveAccount(id));
                self.state.removing = None;
            }
            Action::SignOut => self.backend.send(Command::SignOut),
            Action::SetBrowser(browser) => self.state.browser = browser,
            Action::DismissNotice => self.state.notice = None,
            Action::LoadMoreComments => {
                // Take the token so a second click asks for the same page
                // again only after a retry.
                if matches!(self.state.comments_more, Fetch::Loading) {
                    return;
                }
                if let Some(token) = self
                    .state
                    .comments_next
                    .take()
                    .or_else(|| self.more_comments_token.clone())
                {
                    self.more_comments_token = Some(token.clone());
                    self.state.comments_more = Fetch::Loading;
                    self.backend.send(Command::LoadComments {
                        generation: self.details_generation,
                        key: CommentsKey::Top,
                        token,
                        visitor_data: self.state.comments_visitor_data.clone(),
                    });
                }
            }
            Action::ToggleReplies(id) => {
                if self.state.replies.contains_key(&id) {
                    self.state.replies.remove(&id);
                } else {
                    let token = self
                        .state
                        .comments
                        .ready()
                        .and_then(|comments| comments.iter().find(|row| row.id == id))
                        .and_then(|row| row.replies_token.clone());
                    self.state.replies.insert(id.clone(), Fetch::Loading);
                    if let Some(token) = token {
                        self.backend.send(Command::LoadComments {
                            generation: self.details_generation,
                            key: CommentsKey::Replies(id),
                            token,
                            visitor_data: self.state.comments_visitor_data.clone(),
                        });
                    } else {
                        self.state.replies.insert(
                            id,
                            Fetch::Failed("the replies' token is missing".to_owned()),
                        );
                    }
                }
            }
            Action::ToggleDescription => {
                self.state.description_open = !self.state.description_open;
            }
            Action::SetFeedFilter(filter) => self.state.feed_filter = filter,
            Action::CopyLink(url) => {
                ctx.copy_text(url);
                self.state.notice = Some("Link copied".to_owned());
            }
            Action::OpenUrl(url) => {
                if url.starts_with("https://") || url.starts_with("http://") {
                    ctx.open_url(egui::OpenUrl::new_tab(url));
                }
            }
            Action::SetChannelTab(tab) => {
                if self.state.channel_tab == tab {
                    return;
                }
                self.state.channel_tab = tab;
                self.state.page_more = None;
                self.state.page_more_state = Fetch::Idle;
                // The current tab's list reloads; the other tab keeps what
                // it has but draws nothing until picked.
                self.generation += 1;
                self.generations.insert(Slot::Page, self.generation);
                match &self.state.channel_tab {
                    ChannelTab::About => self.state.channel_about = Fetch::Loading,
                    ChannelTab::Playlists => self.state.page_playlists = Listing::Loading,
                    _ => self.state.page_videos = Listing::Loading,
                }
                if let Some(Page::Channel(channel)) = &self.state.page {
                    let request = match &self.state.channel_tab {
                        ChannelTab::About => Request::ChannelAbout(channel.id.clone()),
                        tab => Request::ChannelTab {
                            id: channel.id.clone(),
                            tab: tab.clone(),
                        },
                    };
                    self.backend.send(Command::Load {
                        slot: Slot::Page,
                        generation: self.generation,
                        request,
                    });
                }
            }
            Action::CheckAlertsNow => self.check_alerts(),
            Action::MarkAlertsSeen => {
                self.state.alerts.clear();
                self.state.alerts_visit.clear();
                self.state.library.alerts_seen_at = Some(util::now());
                self.save_library();
            }
            Action::SetAlerts(on) => {
                self.state.settings.alerts = on;
                self.save_settings();
                self.set_alerts_timer();
            }
            Action::SetDesktopNotifications(on) => {
                self.state.settings.desktop_notifications = on;
                self.save_settings();
            }
            Action::SetAlertsEvery(minutes) => {
                self.state.settings.alerts_every_minutes =
                    crate::settings::clamp_alerts_every(minutes);
                self.save_settings();
                self.set_alerts_timer();
            }
            Action::SetChannelQuery(text) => self.state.channel_query = text,
            Action::LoadMore => {
                if self.state.view == View::Home {
                    let len = self.state.feed.items().len();
                    if self.state.feed_shown < len {
                        // More rows are already here; draw them.
                        self.state.feed_shown += 40;
                    } else if !matches!(self.state.feed_more_state, Fetch::Loading)
                        && let Some(more) = self.state.feed_more.clone()
                    {
                        self.state.feed_more_state = Fetch::Loading;
                        let generation = self.generations.get(&Slot::Feed).copied().unwrap_or(0);
                        self.backend.send(Command::Load {
                            slot: Slot::Feed,
                            generation,
                            request: Request::MoreFeed(more),
                        });
                    }
                    return;
                }
                // A running continuation cannot be asked twice.
                if self.state.view == View::Channel
                    && !matches!(self.state.page_more_state, Fetch::Loading)
                    && let Some(more) = self.state.page_more.clone()
                {
                    self.state.page_more_state = Fetch::Loading;
                    // The slot's generation, not the latest anywhere: a
                    // request on another slot must not strand this one.
                    let generation = self.generations.get(&Slot::Page).copied().unwrap_or(0);
                    self.backend.send(Command::Load {
                        slot: Slot::Page,
                        generation,
                        request: Request::More {
                            more,
                            playlists: self.state.channel_tab == ChannelTab::Playlists,
                        },
                    });
                }
            }
        }
    }

    fn open(&mut self, mut video: Video) {
        log::info!("opening video {}", video.id);
        self.remember_position();
        if self.state.view != View::Watch {
            self.state.up_next = self.visible_videos();
            self.back.push(self.state.view);
        }
        // A video opened from a link knows only its id; the history may know
        // more until yt-dlp answers.
        if let Some(known) = self
            .state
            .library
            .history
            .iter()
            .find(|watched| watched.video.id == video.id)
        {
            video.complete_from(&known.video);
        }
        let start = self.state.library.resume_position(&video.id);
        self.state.library.record_watch(video.clone(), util::now());
        self.save_library();

        self.state.view = View::Watch;
        self.state.seek_preview = None;
        self.state.resolutions.clear();
        // The watch page's other panes start fresh and fill as they load.
        self.state.details = Fetch::Loading;
        self.state.comments = Fetch::Idle;
        self.state.comments_next = None;
        self.state.comments_more = Fetch::Idle;
        self.more_comments_token = None;
        self.state.comments_visitor_data = None;
        self.state.replies.clear();
        self.state.description_open = false;
        self.generation += 1;
        self.details_generation = self.generation;
        if let Some(player) = &self.player {
            self.state.player_error = player
                .load(&video.url(), start)
                .err()
                .map(|error| error.to_string());
        }
        let id = video.id.clone();
        self.state.watching = Some(video);
        match self.details_cache.get(&id, util::now(), DETAILS_FRESH) {
            Some(details) => self.show_details(details.clone()),
            None => self.backend.send(Command::LoadDetails {
                generation: self.details_generation,
                id,
            }),
        }
    }

    fn open_page(&mut self, ctx: &egui::Context, view: View, page: Page) {
        if self.state.page.as_ref() != Some(&page) {
            self.state.page_videos = Listing::Idle;
            self.state.page_more = None;
            self.state.page_more_state = Fetch::Idle;
        }
        // Another channel restarts its page: tabs, header, search field.
        let channel_id = match &page {
            Page::Channel(channel) => Some(channel.id.clone()),
            _ => None,
        };
        if view == View::Channel && self.open_channel.as_deref() != channel_id.as_deref() {
            self.state.channel_header = None;
            self.state.channel_tab = ChannelTab::default();
            self.state.page_playlists = Listing::Idle;
            self.state.channel_about = Fetch::Idle;
            self.state.channel_query.clear();
        }
        self.open_channel = channel_id;
        self.leave_video(ctx);
        if self.state.view != view {
            self.back.push(self.state.view);
        }
        self.state.page = Some(page);
        self.state.view = view;
        self.load(view, false);
    }

    /// The videos on screen, to offer as Up next beside one of them.
    fn visible_videos(&self) -> Vec<Video> {
        let state = &self.state;
        let account = state.source() == Source::Account;
        match state.view {
            View::Home => state.feed.items().to_vec(),
            View::Results => state.results.items().to_vec(),
            View::Liked => state.liked.items().to_vec(),
            View::Channel | View::Playlist => state.page_videos.items().to_vec(),
            View::WatchLater if account => state.account_watch_later.items().to_vec(),
            View::WatchLater => state.library.watch_later.clone(),
            View::History if account => state
                .account_history
                .items()
                .iter()
                .map(|row| row.video.clone())
                .collect(),
            View::History => state
                .library
                .history
                .iter()
                .map(|watched| watched.video.clone())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Remembers where the video being watched was left.
    fn remember_position(&mut self) {
        if let (Some(video), Some(player)) = (&self.state.watching, &self.player) {
            let status = player.status();
            if !status.idle {
                self.state
                    .library
                    .set_position(&video.id, status.position, status.duration);
                self.save_library();
            }
        }
    }

    fn save_library(&self) {
        self.backend
            .send(Command::SaveLibrary(self.state.library.clone()));
    }

    fn save_settings(&self) {
        self.backend
            .send(Command::SaveSettings(self.state.settings.clone()));
    }

    /// Lists the themes folder again, reading the selected palette first.
    fn load_themes(&mut self) {
        self.custom_themes.start(
            self.dirs.config.join("themes"),
            selected_theme(&self.state.settings),
            &self.theme_waker,
        );
        self.refresh_theme_list();
    }

    /// Takes the selected and the desktop's palettes from a finished scan.
    fn adopt_themes(&mut self) {
        let mut changed = false;
        if let ThemeChoice::Custom(filename) = &self.state.settings.theme
            && let Some(theme) = self.custom_themes.find(filename)
            && self.state.settings.custom_theme_cache.as_ref() != Some(theme)
        {
            self.state.settings.custom_theme_cache = Some(theme.clone());
            changed = true;
        }
        if self.custom_themes.follows_omarchy() {
            if let Some(theme) = self.custom_themes.system_theme()
                && self.state.settings.system_theme_cache.as_ref() != Some(theme)
            {
                self.state.settings.system_theme_cache = Some(theme.clone());
                changed = true;
            }
        } else if self.state.settings.system_theme_cache.take().is_some() {
            changed = true;
        }
        if changed {
            self.save_settings();
        }
        self.refresh_theme_list();
    }

    /// Applies the settings' palette when it differs from the one drawn,
    /// revealing the change from the middle of the window outwards.
    fn update_theme(&mut self, ctx: &egui::Context) {
        let palette = self.state.settings.palette();
        if palette == self.state.palette {
            return;
        }
        self.theme_transition.begin(ctx);
        if self.theme_transition.holding(ctx) {
            return;
        }
        theme::apply(ctx, &palette);
        self.state.palette = palette;
    }

    /// What the settings page lists and says under the theme setting.
    fn refresh_theme_list(&mut self) {
        let state = &mut self.state;
        state.themes = self
            .custom_themes
            .picker_themes()
            .map(|theme| {
                (
                    theme.filename.clone(),
                    fastframe_theme::display_name(&theme.filename).to_owned(),
                )
            })
            .collect();
        state.follows_omarchy = self.custom_themes.follows_omarchy();
        let selected = match &state.settings.theme {
            ThemeChoice::Custom(filename) => Some(filename.as_str()),
            _ => None,
        };
        state.theme_status = theme::catalog_status(&self.custom_themes, selected).into_owned();
    }

    /// Bounds the streams from now on, and reloads the video being watched
    /// where it was, so the new quality shows at once.
    fn set_quality(&mut self, quality: Quality) {
        if quality == self.state.settings.quality {
            return;
        }
        log::info!("quality set to {quality:?}");
        self.state.settings.quality = quality;
        self.save_settings();
        let Some(player) = &mut self.player else {
            return;
        };
        if let Err(error) = player.set_max_resolution(quality.limit()) {
            log::warn!("mpv: {error}");
            return;
        }
        let status = player.status();
        if let Some(video) = &self.state.watching
            && !status.idle
            && let Err(error) = player.reload(&video.url(), status.position, status.paused)
        {
            log::warn!("mpv: {error}");
        }
    }

    /// Playing while another page is on screen comes back to the watch
    /// page: an undrawn render-API video output stalls.
    fn return_to_watch(&mut self) {
        if self.state.watching.is_some() && self.state.view != View::Watch {
            self.back.push(self.state.view);
            self.state.view = View::Watch;
        }
    }

    /// Leaving the watch page pauses the video (nothing draws it there) and
    /// remembers where it was left.
    fn leave_video(&mut self, ctx: &egui::Context) {
        self.set_fullscreen(ctx, false);
        if self.state.view == View::Watch {
            self.remember_position();
            self.with_player(|player| player.set_paused(true));
        }
    }

    fn set_fullscreen(&mut self, ctx: &egui::Context, fullscreen: bool) {
        if self.state.fullscreen != fullscreen {
            self.state.fullscreen = fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
        }
    }

    fn with_player(&self, command: impl FnOnce(&Mpv) -> Result<(), mpv::Error>) {
        if let Some(player) = &self.player
            && let Err(error) = command(player)
        {
            log::warn!("mpv: {error}");
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        ui::images::forget_unseen(ctx);
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
                crate::now_playing::Effect::Action(action) => self.apply(ctx, action),
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
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.update_theme(ui.ctx());
        let mut actions = Vec::new();
        let status = self.player.as_ref().map(Mpv::status);
        ui::show(
            ui,
            &mut self.state,
            status,
            self.surface.as_ref(),
            &mut actions,
        );
        self.theme_transition.paint(ui.ctx());
        let ctx = ui.ctx().clone();
        for action in actions {
            self.apply(&ctx, action);
        }
    }

    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        // The position of the video playing, written here and now: the
        // backend may not get to a queued save before the process ends.
        if let (Some(video), Some(player)) = (&self.state.watching, &self.player) {
            let status = player.status();
            self.state
                .library
                .set_position(&video.id, status.position, status.duration);
        }
        if let Err(error) = self.state.library.save(&self.dirs.library()) {
            log::error!("could not save the library: {error}");
        }
        // The volume only lands here: the slider updates it without saving.
        if let Err(error) = self.state.settings.save(&self.dirs.settings()) {
            log::error!("could not save the settings: {error}");
        }
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

/// The palette filename a `Custom` theme choice selects, for a scan.
fn selected_theme(settings: &Settings) -> Option<String> {
    match &settings.theme {
        ThemeChoice::Custom(filename) => Some(filename.clone()),
        _ => None,
    }
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

/// A feed continuation page: shorts to the shelf, the rest appended
/// without repeats. Returns how many rows landed in the feed.
fn merge_feed_page(feed: &mut Vec<Video>, shorts: &mut Vec<Video>, page: Vec<Video>) -> usize {
    let (shelf, rest): (Vec<Video>, Vec<Video>) = page.into_iter().partition(|video| video.short);
    let on_shelf: std::collections::HashSet<String> =
        shorts.iter().map(|video| video.id.clone()).collect();
    shorts.extend(
        shelf
            .into_iter()
            .filter(|video| !on_shelf.contains(&video.id)),
    );
    let known: std::collections::HashSet<String> =
        feed.iter().map(|video| video.id.clone()).collect();
    let fresh: Vec<Video> = rest
        .into_iter()
        .filter(|video| !known.contains(&video.id))
        .collect();
    let landed = fresh.len();
    feed.extend(fresh);
    landed
}

/// What the watch page says when a video could not play.
fn failure_message(failure: mpv::Failure) -> &'static str {
    match failure {
        mpv::Failure::BotCheck => {
            "YouTube is limiting requests from this connection. Try again later."
        }
        mpv::Failure::AgeRestricted => "This video is age-restricted and needs signing in.",
        mpv::Failure::Unavailable => "This video is private or unavailable.",
        mpv::Failure::Other => "Could not play this video. Updating yt-dlp usually fixes this.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_continuation_page_splits_shorts_and_skips_repeats() {
        let mut feed = vec![Video::from_id("a".to_owned())];
        let mut shorts = Vec::new();
        let mut new = Video::from_id("b".to_owned());
        new.short = true;
        let page = vec![
            Video::from_id("a".to_owned()),
            new,
            Video::from_id("c".to_owned()),
        ];
        assert_eq!(merge_feed_page(&mut feed, &mut shorts, page), 1);
        assert_eq!(
            feed.iter()
                .map(|video| video.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "c"]
        );
        assert_eq!(
            shorts
                .iter()
                .map(|video| video.id.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
    }
}
