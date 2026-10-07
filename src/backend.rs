//! The interface's handle to a tokio runtime on its own thread.
//!
//! As in Spotifast and ZapFast, the two sides talk only through [`Command`]
//! (interface to runtime) and [`Event`] (runtime to interface), and every
//! event wakes the window. Nothing here blocks the interface thread.

use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc;

use rustypipe::client::RustyPipe;
use tokio::sync::mpsc as async_mpsc;

use crate::library::{Import, Library};
use crate::model::{
    AccountSource, Channel, ChannelAbout, ChannelHeader, HistoryRow, PlaylistSummary, Video,
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
}

pub enum Payload {
    Videos(Vec<Video>),
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
    /// A page of comments or replies.
    LoadComments {
        generation: u64,
        key: CommentsKey,
        token: String,
        visitor_data: Option<String>,
    },
    ImportTakeout(Picked),
    SignInWithFile(Picked),
    SignInWithBrowser(String),
    /// Signs in again with the session kept in the credential store.
    Restore(AccountSource),
    SignOut,
    SaveLibrary(Library),
    SaveSettings(Settings),
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
    SigningIn,
    SignedIn(AccountSource),
    SignInFailed(String),
    /// The kept session no longer works, and could not be renewed.
    SessionExpired(String),
    SignedOut,
    /// Something the user should know that belongs to no page.
    Notice(String),
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
        let context = Context {
            rp: innertube::client(&dirs)?,
            dirs,
            events: outgoing,
            waker,
        };
        std::thread::Builder::new()
            .name("youtfast-backend".into())
            .spawn(move || {
                runtime.block_on(async move {
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
        Command::ImportTakeout(picked) => {
            let Some(file) = picked.await else {
                return;
            };
            let import = takeout::parse(&file.read().await);
            context.send(Event::Imported(import));
        }
        Command::SignInWithFile(picked) => {
            let Some(file) = picked.await else {
                return;
            };
            context.send(Event::SigningIn);
            let text = String::from_utf8_lossy(&file.read().await).into_owned();
            let result = match account::cookie_header(&text) {
                Ok(cookie) => start_session(&context, cookie).await,
                Err(error) => Err(error),
            };
            finish_sign_in(&context, AccountSource::File, result);
        }
        Command::SignInWithBrowser(browser) => {
            context.send(Event::SigningIn);
            let result = match account::cookies_from_browser(&browser).await {
                Ok(cookie) => start_session(&context, cookie).await,
                Err(error) => Err(error),
            };
            finish_sign_in(&context, AccountSource::Browser(browser), result);
        }
        Command::Restore(source) => restore(&context, source).await,
        Command::SignOut => {
            innertube::sign_out(&context.rp).await;
            if let Err(error) = account::delete_session().await {
                context.send(Event::Notice(format!(
                    "Signed out, but the session could not be removed from the keyring: {error}"
                )));
            }
            context.send(Event::SignedOut);
        }
        Command::SaveLibrary(library) => {
            let path = context.dirs.library();
            match tokio::task::spawn_blocking(move || library.save(&path)).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    log::error!("could not save the library: {error}");
                    context.send(Event::Notice(format!(
                        "Could not save the library: {error}"
                    )));
                }
                Err(error) => log::error!("saving the library failed: {error}"),
            }
        }
        Command::SaveSettings(settings) => {
            let path = context.dirs.settings();
            match tokio::task::spawn_blocking(move || settings.save(&path)).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    log::error!("could not save the settings: {error}");
                    context.send(Event::Notice(format!(
                        "Could not save the settings: {error}"
                    )));
                }
                Err(error) => log::error!("saving the settings failed: {error}"),
            }
        }
    }
}

async fn load(rp: &RustyPipe, request: Request) -> Result<Payload, String> {
    let text = |error: rustypipe::error::Error| error.to_string();
    Ok(match request {
        Request::Search(query) => Payload::Videos(youtube::search(&query).await?),
        Request::Feed { channels, account } => {
            Payload::Videos(innertube::feed(rp, channels, account).await?)
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
        Request::More { more, playlists } => {
            if playlists {
                let (items, next) = innertube::more_playlists(rp, &more).await?;
                Payload::More {
                    items: PageItems::Playlists(items),
                    more: next,
                }
            } else {
                let (items, next) = innertube::more_videos(rp, &more).await?;
                Payload::More {
                    items: PageItems::Videos(items),
                    more: next,
                }
            }
        }
    })
}

/// Checks the session with YouTube, then keeps it in the credential store.
async fn start_session(context: &Context, cookie: String) -> Result<(), String> {
    innertube::sign_in(&context.rp, cookie.clone())
        .await
        .map_err(|error| {
            if innertube::is_signed_out(&error) {
                "YouTube did not accept this session. Sign in again on youtube.com and retry."
                    .to_owned()
            } else {
                format!("Could not reach YouTube: {error}")
            }
        })?;
    account::save_session(cookie).await
}

fn finish_sign_in(context: &Context, source: AccountSource, result: Result<(), String>) {
    match result {
        Ok(()) => {
            log::info!("signed in with {}", source.label());
            context.send(Event::SignedIn(source));
        }
        Err(error) => {
            log::warn!("sign-in with {} failed: {error}", source.label());
            context.send(Event::SignInFailed(error));
        }
    }
}

/// Signs in with the kept session. A session from a browser that YouTube
/// has since rotated is read from the browser again, once.
async fn restore(context: &Context, source: AccountSource) {
    let kept = match account::load_session().await {
        Ok(Some(cookie)) => innertube::sign_in(&context.rp, cookie)
            .await
            .map_err(|error| {
                if innertube::is_signed_out(&error) {
                    "Your YouTube session has expired.".to_owned()
                } else {
                    format!("Could not reach YouTube: {error}")
                }
            }),
        Ok(None) => Err("No session is kept on this computer any more.".to_owned()),
        Err(error) => Err(error),
    };
    let error = match kept {
        Ok(()) => {
            context.send(Event::SignedIn(source));
            return;
        }
        Err(error) => error,
    };
    if let AccountSource::Browser(browser) = &source {
        log::info!("renewing the session from {browser}: {error}");
        let renewed = match account::cookies_from_browser(browser).await {
            Ok(cookie) => start_session(context, cookie).await,
            Err(error) => Err(error),
        };
        if renewed.is_ok() {
            context.send(Event::SignedIn(source));
            return;
        }
    }
    context.send(Event::SessionExpired(error));
}
