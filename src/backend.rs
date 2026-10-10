//! The interface's handle to a tokio runtime on its own thread.
//!
//! As in Spotifast and ZapFast, the two sides talk only through [`Command`]
//! (interface to runtime) and [`Event`] (runtime to interface), and every
//! event wakes the window. Nothing here blocks the interface thread.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::mpsc;

use rustypipe::client::RustyPipe;
use tokio::sync::mpsc as async_mpsc;
use tokio::sync::watch;

use crate::library::Account;
use crate::library::{Import, Library};
use crate::model::{
    AccountSource, Channel, ChannelAbout, ChannelHeader, HistoryRow, PlaylistSummary, Rating,
    SaveTarget, Video,
};
use crate::paths::Dirs;
use crate::settings::Settings;
use crate::{account, innertube, takeout, youtube};

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
}

pub enum Request {
    Search(String),
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
    },
    /// A channel page's About pane.
    ChannelAbout(String),
    /// The next page of the channel tab's list.
    More {
        more: crate::innertube::More,
        /// `true` to continue the playlists tab, `false` a video list.
        playlists: bool,
    },
    /// The next page of the account's feed.
    MoreFeed(crate::innertube::More),
    /// The timer's check of the feed's sources.
    Alerts {
        channels: Vec<Channel>,
        account: bool,
    },
    /// The Shorts tabs of the feed's channels, for Home's shelf.
    Shorts {
        channels: Vec<String>,
    },
}

pub enum Payload {
    Videos(Vec<Video>),
    /// The feed's first page and its continuation.
    Feed {
        videos: Vec<Video>,
        more: Option<crate::innertube::More>,
    },
    Channels(Vec<Channel>),
    History(Vec<HistoryRow>),
    Playlists(Vec<PlaylistSummary>),
    /// A channel tab's first page: the header plus videos or playlists.
    ChannelPage {
        header: ChannelHeader,
        items: PageItems,
        more: Option<crate::innertube::More>,
    },
    /// A channel's About tab.
    About(ChannelAbout),
    /// A continued page of a channel tab's list.
    More {
        items: PageItems,
        more: Option<crate::innertube::More>,
    },
}

/// What a channel tab's rows are.
pub enum PageItems {
    Videos(Vec<Video>),
    Playlists(Vec<PlaylistSummary>),
}

/// Which comment list a [`Command::LoadComments`] fills.
#[derive(Clone, Debug, PartialEq)]
pub enum CommentsKey {
    /// The video's own comments.
    Top,
    /// A comment's replies.
    Replies(String),
}

pub enum Command {
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
    },
    /// A page of comments or replies.
    LoadComments {
        generation: u64,
        key: CommentsKey,
        token: String,
        visitor_data: Option<String>,
    },
    ImportTakeout(Picked),
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
    /// Forget an account: its keyring entry is deleted too.
    RemoveAccount(u32),
    SaveLibrary(Library),
    SaveSettings(Settings),
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
        result: Result<(Vec<crate::model::CommentRow>, Option<String>), String>,
    },
    Imported(Result<Import, String>),
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
    SignedOut,
    /// Something the user should know that belongs to no page.
    Notice(String),
    /// The alerts timer fired; the app sends the check request.
    CheckAlerts,
}

pub struct Backend {
    commands: async_mpsc::UnboundedSender<Command>,
    events: mpsc::Receiver<Event>,
}

/// What every task on the runtime shares.
#[derive(Clone)]
struct Context {
    rp: RustyPipe,
    dirs: Dirs,
    events: mpsc::Sender<Event>,
    waker: egui::Context,
    /// The running alerts timer, restarted when the interval changes.
    alerts_timer: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// The newest library and settings to write; one writer each takes
    /// them in turn (see [`writer`]).
    library_saves: Arc<watch::Sender<Option<Library>>>,
    settings_saves: Arc<watch::Sender<Option<Settings>>>,
    /// Held by each change to the account, so they reach YouTube in the
    /// order they were made.
    account_writes: Arc<tokio::sync::Mutex<()>>,
}

impl Context {
    fn send(&self, event: Event) {
        if self.events.send(event).is_ok() {
            self.waker.request_repaint();
        }
    }
}

impl Backend {
    /// Starts the runtime. It stops when the Backend is dropped.
    pub fn start(waker: egui::Context, dirs: Dirs) -> anyhow::Result<Self> {
        let (commands, mut incoming) = async_mpsc::unbounded_channel();
        let (outgoing, events) = mpsc::channel();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("youtfast-worker")
            .enable_all()
            .build()?;
        let (library_saves, library_snapshots) = watch::channel(None);
        let (settings_saves, settings_snapshots) = watch::channel(None);
        let context = Context {
            rp: innertube::client(&dirs)?,
            dirs,
            events: outgoing,
            waker,
            alerts_timer: Arc::new(tokio::sync::Mutex::new(None)),
            library_saves: Arc::new(library_saves),
            settings_saves: Arc::new(settings_saves),
            account_writes: Arc::new(tokio::sync::Mutex::new(())),
        };
        std::thread::Builder::new()
            .name("youtfast-backend".into())
            .spawn(move || {
                runtime.block_on(async move {
                    tokio::spawn(writer(
                        context.clone(),
                        "library",
                        context.dirs.library(),
                        library_snapshots,
                        Library::save,
                    ));
                    tokio::spawn(writer(
                        context.clone(),
                        "settings",
                        context.dirs.settings(),
                        settings_snapshots,
                        Settings::save,
                    ));
                    while let Some(command) = incoming.recv().await {
                        let context = context.clone();
                        tokio::spawn(async move { handle(command, context).await });
                    }
                });
            })?;
        Ok(Self { commands, events })
    }

    pub fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            log::error!("the backend has stopped");
        }
    }

    /// Every event that arrived since the last call.
    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

async fn handle(command: Command, context: Context) {
    match command {
        Command::Load {
            slot,
            generation,
            request,
        } => {
            let result = load(&context.rp, request).await;
            if let Err(error) = &result {
                log::warn!("loading {slot:?} failed: {error}");
            }
            context.send(Event::Loaded {
                slot,
                generation,
                result,
            });
        }
        Command::LoadDetails { generation, id } => {
            let result = innertube::details(&context.rp, &id).await;
            if let Err(error) = &result {
                log::warn!("loading the details of {id} failed: {error}");
            }
            context.send(Event::Details {
                generation,
                result: Box::new(result),
            });
        }
        Command::LoadComments {
            generation,
            key,
            token,
            visitor_data,
        } => {
            let result = innertube::comments(&context.rp, &token, visitor_data.as_deref()).await;
            if let Err(error) = &result {
                log::warn!("loading comments failed: {error}");
            }
            context.send(Event::Comments {
                generation,
                key,
                result,
            });
        }
        Command::LoadRating(id) => {
            let result = innertube::rating(&context.rp, &id).await;
            if let Err(error) = &result {
                log::warn!("loading the rating of {id} failed: {error}");
            }
            context.send(Event::Rating { id, result });
        }
        Command::Rate {
            id,
            rating,
            previous,
        } => {
            // One change at a time, in the order clicked.
            let _turn = context.account_writes.lock().await;
            if let Err(error) = innertube::rate(&context.rp, &id, rating).await {
                log::warn!("rating {id} failed: {error}");
                context.send(Event::RateFailed {
                    id,
                    previous,
                    error,
                });
            }
        }
        Command::LoadSaveTargets(id) => {
            let result = innertube::save_targets(&context.rp, &id).await;
            if let Err(error) = &result {
                log::warn!("loading the save menu of {id} failed: {error}");
            }
            context.send(Event::SaveTargets { id, result });
        }
        Command::SetSaved {
            id,
            playlist,
            saved,
        } => {
            let _turn = context.account_writes.lock().await;
            if let Err(error) = innertube::set_saved(&context.rp, &id, &playlist, saved).await {
                log::warn!("saving {id} to {playlist} failed: {error}");
                context.send(Event::SaveFailed {
                    id,
                    playlist,
                    saved,
                    error,
                });
            }
        }
        Command::ImportTakeout(picked) => {
            let Some(file) = picked.await else {
                return;
            };
            let import = takeout::parse(&file.read().await);
            context.send(Event::Imported(import));
        }
        Command::AddAccount { account, method } => {
            context.send(Event::SigningIn);
            let result = match method {
                SignInMethod::File(picked) => match picked.await {
                    Some(file) => {
                        let text = String::from_utf8_lossy(&file.read().await).into_owned();
                        match account::cookie_header(&text) {
                            Ok(cookie) => start_session(&context, cookie, account.id).await,
                            Err(error) => Err(error),
                        }
                    }
                    None => return,
                },
                SignInMethod::Browser => {
                    let AccountSource::Browser(spec) = &account.source else {
                        return;
                    };
                    session_from_browser(&context, spec, account.id).await
                }
            };
            finish_sign_in(&context, account, result);
        }
        Command::Switch(account) => {
            // Leave the current session first so the store only ever
            // holds one live one.
            innertube::sign_out(&context.rp).await;
            restore(&context, &account).await;
        }
        Command::SignOut => {
            innertube::sign_out(&context.rp).await;
            context.send(Event::SignedOut);
        }
        Command::RemoveAccount(id) => {
            if let Err(error) = account::delete_session(id).await {
                context.send(Event::Notice(format!(
                    "The account was removed, but its session could not be deleted from the keyring: {error}"
                )));
            }
        }
        Command::SetAlertsEvery(minutes) => {
            // Replace the timer: an abort is safe for a task that only
            // sleeps and sends.
            let mut guard = context.alerts_timer.lock().await;
            if let Some(timer) = guard.take() {
                timer.abort();
            }
            if minutes > 0 {
                let context = context.clone();
                *guard = Some(tokio::spawn(async move {
                    // The first check comes soon after start, not only
                    // after a full interval.
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    context.send(Event::CheckAlerts);
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(u64::from(minutes) * 60))
                            .await;
                        context.send(Event::CheckAlerts);
                    }
                }));
            }
        }
        Command::Notify { videos } => {
            // notify-rust's zbus backend blocks on D-Bus; off the runtime.
            tokio::task::spawn_blocking(move || {
                let (summary, body) = crate::alerts::notify_text(&videos);
                if let Err(error) = notify_rust::Notification::new()
                    .appname("YouTFast")
                    .summary(&summary)
                    .body(&body)
                    .icon("youtfast")
                    .show()
                {
                    log::warn!("the desktop notification failed: {error}");
                }
            });
        }
        Command::SaveLibrary(library) => {
            context.library_saves.send_replace(Some(library));
        }
        Command::SaveSettings(settings) => {
            context.settings_saves.send_replace(Some(settings));
        }
    }
}

/// Writes the newest snapshot of one file, one write at a time: saves sent
/// during a write collapse into the next, and an older snapshot never
/// lands after a newer one. `what` names the file ("library"); a failed
/// write is logged and shown.
async fn writer<T: Clone + Send + Sync + 'static>(
    context: Context,
    what: &'static str,
    path: PathBuf,
    mut snapshots: watch::Receiver<Option<T>>,
    save: fn(&T, &Path) -> std::io::Result<()>,
) {
    while snapshots.changed().await.is_ok() {
        let Some(value) = snapshots.borrow_and_update().clone() else {
            continue;
        };
        let path = path.clone();
        match tokio::task::spawn_blocking(move || save(&value, &path)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                log::error!("could not save the {what}: {error}");
                context.send(Event::Notice(format!("Could not save the {what}: {error}")));
            }
            Err(error) => log::error!("saving the {what} failed: {error}"),
        }
    }
}

async fn load(rp: &RustyPipe, request: Request) -> Result<Payload, String> {
    let text = |error: rustypipe::error::Error| error.to_string();
    Ok(match request {
        Request::Search(query) => Payload::Videos(youtube::search(&query).await?),
        Request::Feed { channels, account } => {
            let (videos, more) = innertube::feed(rp, channels, account).await?;
            Payload::Feed { videos, more }
        }
        Request::MoreFeed(more)
        | Request::More {
            more,
            playlists: false,
        } => {
            let (videos, next) = innertube::more_videos(rp, &more).await?;
            Payload::More {
                items: PageItems::Videos(videos),
                more: next,
            }
        }
        Request::Shorts { channels } => {
            Payload::Videos(innertube::channel_shorts(rp, channels, 6).await)
        }
        Request::Alerts { channels, account } => {
            let (videos, _) = innertube::feed(rp, channels, account).await?;
            Payload::Feed { videos, more: None }
        }
        Request::Subscriptions => {
            Payload::Channels(innertube::subscriptions(rp).await.map_err(text)?)
        }
        Request::History => Payload::History(innertube::history(rp).await.map_err(text)?),
        Request::WatchLater => Payload::Videos(innertube::watch_later(rp).await.map_err(text)?),
        Request::Liked => Payload::Videos(innertube::liked(rp).await.map_err(text)?),
        Request::Playlists => Payload::Playlists(innertube::playlists(rp).await.map_err(text)?),
        Request::Playlist(id) => {
            Payload::Videos(innertube::playlist_videos(rp, &id).await.map_err(text)?)
        }
        Request::ChannelTab { id, tab } => match tab {
            crate::model::ChannelTab::Playlists => {
                let (header, items, more) = innertube::channel_playlists_page(rp, &id).await?;
                Payload::ChannelPage {
                    header,
                    items: PageItems::Playlists(items),
                    more,
                }
            }
            _ => {
                let (header, items, more) = innertube::channel_tab(rp, &id, &tab).await?;
                Payload::ChannelPage {
                    header,
                    items: PageItems::Videos(items),
                    more,
                }
            }
        },
        Request::ChannelAbout(id) => Payload::About(innertube::channel_about(rp, &id).await?),
        Request::More {
            more,
            playlists: true,
        } => {
            let (items, next) = innertube::more_playlists(rp, &more).await?;
            Payload::More {
                items: PageItems::Playlists(items),
                more: next,
            }
        }
    })
}

/// Checks the session with YouTube, then keeps it in the credential store.
async fn start_session(context: &Context, cookie: String, id: u32) -> Result<(), String> {
    sign_in(
        context,
        cookie.clone(),
        "YouTube did not accept this session. Sign in again on youtube.com and retry.",
    )
    .await?;
    account::save_session(id, cookie).await
}

/// Reads a browser's session and starts it as account `id`.
async fn session_from_browser(context: &Context, spec: &str, id: u32) -> Result<(), String> {
    let cookie = account::cookies_from_browser(spec).await?;
    start_session(context, cookie, id).await
}

/// Starts using a session; `rejected` is what to say when YouTube does
/// not accept it.
async fn sign_in(context: &Context, cookie: String, rejected: &str) -> Result<(), String> {
    innertube::sign_in(&context.rp, cookie)
        .await
        .map_err(|error| {
            if innertube::is_signed_out(&error) {
                rejected.to_owned()
            } else {
                format!("Could not reach YouTube: {error}")
            }
        })
}

fn finish_sign_in(context: &Context, account: Account, result: Result<(), String>) {
    match result {
        Ok(()) => {
            log::info!("signed in as {}", account.name);
            context.send(Event::AccountAdded(account.clone()));
            signed_in(context, &account);
        }
        Err(error) => {
            log::warn!("sign-in for {} failed: {error}", account.name);
            context.send(Event::SignInFailed(error));
        }
    }
}

/// Signs in with the kept session. A session from a browser that YouTube
/// has since rotated is read from the browser again, once.
async fn restore(context: &Context, account: &Account) {
    // The account older files kept as the single `account` has its
    // session under the unnumbered key; move it on the way in.
    let stored = if account.id == 1 {
        account::migrate_legacy_session().await
    } else {
        account::load_session(account.id).await
    };
    let kept = match stored {
        Ok(Some(cookie)) => sign_in(context, cookie, "Your YouTube session has expired.").await,
        Ok(None) => Err("No session is kept on this computer any more.".to_owned()),
        Err(error) => Err(error),
    };
    let error = match kept {
        Ok(()) => {
            signed_in(context, account);
            return;
        }
        Err(error) => error,
    };
    if let AccountSource::Browser(browser) = &account.source {
        log::info!("renewing the session from {browser}: {error}");
        let renewed = session_from_browser(context, browser, account.id).await;
        if renewed.is_ok() {
            signed_in(context, account);
            return;
        }
    }
    context.send(Event::SessionExpired(error));
}

/// Says who signed in; for a browser's account, then finds how yt-dlp
/// opens that browser, for playback.
fn signed_in(context: &Context, account: &Account) {
    context.send(Event::SignedIn(account.id));
    let AccountSource::Browser(spec) = &account.source else {
        return;
    };
    let (context, spec, id) = (context.clone(), spec.clone(), account.id);
    tokio::spawn(async move {
        match account::ytdl_browser(&spec).await {
            Ok(browser) => context.send(Event::PlaybackCookies {
                account: id,
                browser,
            }),
            Err(error) => log::warn!("videos will play signed out: {error}"),
        }
    });
}
