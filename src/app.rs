//! The application: its state, and the actions the views ask for.

use std::collections::{HashMap, HashSet};

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
            themes::selected_theme(&settings),
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
                result,
            } => self.on_comments(generation, key, result),
            Event::Imported(Ok(import)) => self.on_imported(import),
            Event::Imported(Err(error)) => self.state.notice = Some(error),
            Event::SigningIn => self.state.account = AccountState::SigningIn,
            Event::AccountAdded(account) => {
                self.state.library.accounts.push(account);
                self.save_library();
            }
            Event::SignedIn(id) => self.on_signed_in(id),
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
            Event::SignedOut => self.on_signed_out(),
            Event::CheckAlerts => self.check_alerts(),
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
                    self.mark_notifications_visited();
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
            Action::SetTheme(choice) => self.set_theme(choice),
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
            Action::AddAccountFile => self.add_account_file(),
            Action::AddAccountBrowser => self.add_account_browser(),
            Action::SetAccountName(name) => self.state.account_name = name,
            Action::SetAccountProfile(profile) => self.state.account_profile = profile,
            Action::SwitchAccount(id) => self.switch_account(id),
            Action::RenameAccount { id, name } => self.rename_account(id, &name),
            Action::MarkRenaming { id, name } => self.state.renaming = Some((id, name)),
            Action::MarkRemoving(id) => self.state.removing = Some(id),
            Action::CancelRemoving => self.state.removing = None,
            Action::RemoveAccount(id) => self.remove_account(id),
            Action::SignOut => self.backend.send(Command::SignOut),
            Action::SetBrowser(browser) => self.state.browser = browser,
            Action::DismissNotice => self.state.notice = None,
            Action::LoadMoreComments => self.load_more_comments(),
            Action::ToggleReplies(id) => self.toggle_replies(id),
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
            Action::SetChannelTab(tab) => self.set_channel_tab(tab),
            Action::CheckAlertsNow => self.check_alerts(),
            Action::MarkAlertsSeen => self.mark_alerts_seen(),
            Action::SetAlerts(on) => self.set_alerts(on),
            Action::SetDesktopNotifications(on) => {
                self.state.settings.desktop_notifications = on;
                self.save_settings();
            }
            Action::SetAlertsEvery(minutes) => self.set_alerts_every(minutes),
            Action::SetChannelQuery(text) => self.state.channel_query = text,
            Action::LoadMore => self.load_more(),
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
mod notifications;
mod themes;
mod watch;
