//! The application: its state, and the actions the views ask for.

use std::collections::HashMap;

use anyhow::Context as _;
use eframe::glow;
use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

use crate::backend::{Backend, Command, Event, Payload, Request, Slot};
use crate::instance::{self, Queue as RemoteQueue};
use crate::library::Library;
use crate::model::{
    AccountSource, Action, Channel, HistoryRow, Listing, PlaylistSummary, Quality, Resolution,
    Results, Source, Video, View,
};
use crate::mpv::{self, Mpv, NativeDisplay, VideoSurface};
use crate::paths::Dirs;
use crate::settings::{Settings, ThemeChoice};
use crate::theme::{self, Palette};
use crate::{account, ui, util, youtube};

/// The signed-in account, as the interface shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum AccountState {
    #[default]
    SignedOut,
    SigningIn,
    SignedIn(AccountSource),
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
    /// The home feed.
    pub feed: Listing<Video>,
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
}

impl State {
    pub fn signed_in(&self) -> bool {
        matches!(self.account, AccountState::SignedIn(_))
    }

    /// Where account-or-device pages take their list from now.
    pub fn source(&self) -> Source {
        if self.signed_in() {
            self.source
        } else {
            Source::Device
        }
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
    /// What later launches ask the running copy, drained each frame.
    remote: RemoteQueue,
}

impl App {
    /// `open` is the command line's link or search, if any.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        open: Option<String>,
        guard: &instance::Guard,
    ) -> anyhow::Result<Self> {
        let ctx = &cc.egui_ctx;
        guard.set_context(ctx);
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
            back: Vec::new(),
            custom_themes,
            theme_waker,
            theme_transition: fastframe_theme::Transition::default(),
            remote: guard.commands(),
        };
        app.refresh_theme_list();
        if let Some(source) = app.state.library.account.clone() {
            app.state.account = AccountState::SigningIn;
            app.backend.send(Command::Restore(source));
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
                mpv::Event::Failed => {
                    self.state.player_error = Some(
                        "Could not play this video. Updating yt-dlp usually fixes this.".into(),
                    );
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
            Event::SignedIn(source) => {
                self.state.account = AccountState::SignedIn(source.clone());
                self.state.source = Source::Account;
                if self.state.library.account.as_ref() != Some(&source) {
                    self.state.library.account = Some(source);
                    self.save_library();
                }
                // The account's subscriptions keep their channels out of the
                // RSS half of the feed, so they come first.
                self.request(Slot::Subscriptions, Request::Subscriptions);
                self.state.feed = Listing::Idle;
                self.load(self.state.view, true);
            }
            Event::SignInFailed(error) => self.state.account = AccountState::Failed(error),
            Event::SessionExpired(error) => {
                self.state.account =
                    AccountState::Failed(format!("{error} Sign in again to see your account."));
            }
            Event::SignedOut => {
                self.state.account = AccountState::SignedOut;
                self.state.library.account = None;
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
        match (slot, result) {
            (Slot::Search, Ok(Payload::Videos(videos))) => put(&mut state.results, Ok(videos)),
            (Slot::Feed, Ok(Payload::Videos(videos))) => put(&mut state.feed, Ok(videos)),
            (Slot::WatchLater, Ok(Payload::Videos(videos))) => {
                put(&mut state.account_watch_later, Ok(videos));
            }
            (Slot::Liked, Ok(Payload::Videos(videos))) => put(&mut state.liked, Ok(videos)),
            (Slot::Page, Ok(Payload::Videos(videos))) => put(&mut state.page_videos, Ok(videos)),
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
            (Slot::Feed, Err(error)) => put(&mut state.feed, Err(error)),
            (Slot::WatchLater, Err(error)) => put(&mut state.account_watch_later, Err(error)),
            (Slot::Liked, Err(error)) => put(&mut state.liked, Err(error)),
            (Slot::Page, Err(error)) => put(&mut state.page_videos, Err(error)),
            (Slot::Subscriptions, Err(error)) => put(&mut state.account_subscriptions, Err(error)),
            (Slot::History, Err(error)) => put(&mut state.account_history, Err(error)),
            (Slot::Playlists, Err(error)) => put(&mut state.playlists, Err(error)),
            (slot, Ok(_)) => log::error!("{slot:?} received another kind of list"),
        }
    }

    /// Asks the backend for a list, marking it loading.
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
        }
        self.backend.send(Command::Load {
            slot,
            generation: self.generation,
            request,
        });
    }

    /// Loads what a view shows: always when `force`d (Refresh, a new
    /// sign-in), otherwise only what is not there yet or failed.
    fn load(&mut self, view: View, force: bool) {
        fn stale<T>(listing: &Listing<T>) -> bool {
            matches!(listing, Listing::Idle | Listing::Failed(_))
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
            View::Channel | View::Playlist if force || stale(&state.page_videos) => {
                let request = match &state.page {
                    Some(Page::Channel(channel)) => Request::Channel(channel.id.clone()),
                    Some(Page::Playlist { id, .. }) => Request::Playlist(id.clone()),
                    None => return,
                };
                self.request(Slot::Page, request);
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
                self.load(view, false);
            }
            Action::OpenChannel(channel) => {
                self.open_page(ctx, View::Channel, Page::Channel(channel))
            }
            Action::OpenPlaylist { id, name } => {
                self.open_page(ctx, View::Playlist, Page::Playlist { id, name });
            }
            Action::Refresh => self.load(self.state.view, true),
            Action::SetSource(source) => {
                self.state.source = source;
                self.load(self.state.view, false);
            }
            Action::TogglePause => self.with_player(|player| {
                let paused = player.status().paused;
                player.set_paused(!paused)
            }),
            Action::SeekTo { seconds, exact } => {
                self.with_player(|player| player.seek_to(seconds, exact));
            }
            Action::SeekBy(seconds) => self.with_player(|player| player.seek_by(seconds)),
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
            Action::SignInWithFile => {
                let picked = rfd::AsyncFileDialog::new()
                    .set_title("Choose the cookies.txt exported from YouTube")
                    .add_filter("cookies.txt", &["txt"])
                    .pick_file();
                self.backend.send(Command::SignInWithFile(Box::pin(picked)));
            }
            Action::SignInWithBrowser(browser) => {
                self.backend.send(Command::SignInWithBrowser(browser));
            }
            Action::SignOut => self.backend.send(Command::SignOut),
            Action::SetBrowser(browser) => self.state.browser = browser,
            Action::DismissNotice => self.state.notice = None,
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
        if let Some(player) = &self.player {
            self.state.player_error = player
                .load(&video.url(), start)
                .err()
                .map(|error| error.to_string());
        }
        self.state.watching = Some(video);
    }

    fn open_page(&mut self, ctx: &egui::Context, view: View, page: Page) {
        if self.state.page.as_ref() != Some(&page) {
            self.state.page_videos = Listing::Idle;
        }
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
        let Some(player) = &self.player else {
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
