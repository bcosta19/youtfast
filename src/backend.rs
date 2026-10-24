//! The interface's handle to a tokio runtime on its own thread.
//!
//! As in Spotifast and ZapFast, the two sides talk only through [`Command`]
//! (interface to runtime) and [`Event`] (runtime to interface), and every
//! event wakes the window. Nothing here blocks the interface thread.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;

use rustypipe::client::RustyPipe;
use tokio::sync::mpsc as async_mpsc;
use tokio::sync::watch;

use crate::model::{AccountSource, Video};
use crate::storage::library::Account;
use crate::storage::library::Library;
use crate::storage::paths::Dirs;
use crate::storage::settings::Settings;
use crate::{storage::account, storage::takeout, youtube::innertube};

mod protocol;
pub use protocol::{Command, CommentsKey, Event, PageItems, Payload, Request, SignInMethod, Slot};

#[derive(Default)]
struct SaveGate(std::sync::Mutex<bool>);

impl SaveGate {
    fn write(
        &self,
        final_save: bool,
        save: impl FnOnce() -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let mut closed = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *closed {
            return Ok(());
        }
        *closed = final_save;
        save()
    }
}

pub struct Backend {
    commands: async_mpsc::UnboundedSender<(u64, Command)>,
    events: mpsc::Receiver<(u64, Event)>,
    epoch: std::cell::Cell<u64>,
    waker: egui::Context,
    library_saves: Arc<watch::Sender<Option<Library>>>,
    settings_saves: Arc<watch::Sender<Option<Settings>>>,
    library_gate: Arc<SaveGate>,
    settings_gate: Arc<SaveGate>,
}

/// What every task on the runtime shares.
#[derive(Clone)]
struct Context {
    rp: RustyPipe,
    dirs: Dirs,
    events: mpsc::Sender<(u64, Event)>,
    epoch: u64,
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
    /// The `Load` running on each slot: a newer request aborts it, since
    /// its answer would be dropped on arrival anyway.
    loads: Arc<std::sync::Mutex<HashMap<Slot, tokio::task::JoinHandle<()>>>>,
    /// Channel RSS answers kept a while (see `innertube::RssCache`).
    rss: Arc<innertube::RssCache>,
    /// Thumbnails being downloaded to the disk cache; a URL goes in while
    /// its fetch runs so it is not fetched twice at once.
    thumb_pending: Arc<std::sync::Mutex<HashSet<String>>>,
    /// At most this many thumbnail downloads at a time.
    thumb_sem: Arc<tokio::sync::Semaphore>,
}

impl Context {
    fn send(&self, event: Event) {
        if self.events.send((self.epoch, event.sanitize())).is_ok() {
            self.waker.request_repaint();
        }
    }
}

impl Backend {
    /// Starts the runtime. It stops when the Backend is dropped.
    pub fn start(waker: egui::Context, dirs: Dirs) -> anyhow::Result<Self> {
        let (commands, mut incoming) = async_mpsc::unbounded_channel::<(u64, Command)>();
        let (outgoing, events) = mpsc::channel();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("youtfast-worker")
            .enable_all()
            .build()?;
        let (library_saves, library_snapshots) = watch::channel(None);
        let (settings_saves, settings_snapshots) = watch::channel(None);
        let library_gate = Arc::new(SaveGate::default());
        let settings_gate = Arc::new(SaveGate::default());
        let writer_library_gate = library_gate.clone();
        let writer_settings_gate = settings_gate.clone();
        let mut context = Context {
            rp: innertube::client(&dirs)?,
            dirs,
            events: outgoing,
            epoch: 0,
            waker,
            alerts_timer: Arc::new(tokio::sync::Mutex::new(None)),
            library_saves: Arc::new(library_saves),
            settings_saves: Arc::new(settings_saves),
            account_writes: Arc::new(tokio::sync::Mutex::new(())),
            loads: Arc::new(std::sync::Mutex::new(HashMap::new())),
            rss: Arc::new(std::sync::Mutex::new(HashMap::new())),
            thumb_pending: Arc::new(std::sync::Mutex::new(HashSet::new())),
            thumb_sem: Arc::new(tokio::sync::Semaphore::new(
                crate::youtube::thumbs::DOWNLOADS,
            )),
        };
        let backend = Self {
            commands,
            events,
            epoch: std::cell::Cell::new(0),
            waker: context.waker.clone(),
            library_saves: context.library_saves.clone(),
            settings_saves: context.settings_saves.clone(),
            library_gate,
            settings_gate,
        };
        std::thread::Builder::new()
            .name("youtfast-backend".into())
            .spawn(move || {
                runtime.block_on(async move {
                    {
                        // The thumbnail cache is bounded once per run.
                        let dir = context.dirs.cache.join("thumbnails");
                        tokio::task::spawn_blocking(move || crate::youtube::thumbs::prune(&dir));
                    }
                    tokio::spawn(writer(
                        context.clone(),
                        "library",
                        context.dirs.library(),
                        library_snapshots,
                        writer_library_gate,
                        Library::save,
                    ));
                    tokio::spawn(writer(
                        context.clone(),
                        "settings",
                        context.dirs.settings(),
                        settings_snapshots,
                        writer_settings_gate,
                        Settings::save,
                    ));
                    while let Some((epoch, command)) = incoming.recv().await {
                        if command.changes_session() {
                            context.epoch = epoch;
                            for (_, task) in context.loads.lock().unwrap().drain() {
                                task.abort();
                            }
                            match innertube::client(&context.dirs) {
                                Ok(client) => context.rp = client,
                                Err(_) => {
                                    innertube::sign_out(&context.rp).await;
                                    context.send(if matches!(command, Command::SignOut) {
                                        Event::SignedOut
                                    } else {
                                        Event::SignInFailed(
                                            "Could not start the YouTube session.".into(),
                                        )
                                    });
                                    continue;
                                }
                            }
                        }
                        dispatch(command.ordered(), handle(command, context.clone())).await;
                    }
                });
            })?;
        Ok(backend)
    }

    pub fn session_epoch(&self) -> u64 {
        self.epoch.get()
    }

    pub fn context(&self) -> &egui::Context {
        &self.waker
    }

    pub fn save_final(&self, library: &Library, settings: &Settings, dirs: &Dirs) {
        if let Err(error) = self
            .library_gate
            .write(true, || library.save(&dirs.library()))
        {
            log::error!("could not save the library: {error}");
        }
        if let Err(error) = self
            .settings_gate
            .write(true, || settings.save(&dirs.settings()))
        {
            log::error!("could not save the settings: {error}");
        }
    }

    pub fn send(&self, command: Command) {
        let command = match command {
            Command::SaveLibrary(library) => {
                self.library_saves.send_replace(Some(library));
                return;
            }
            Command::SaveSettings(settings) => {
                self.settings_saves.send_replace(Some(settings));
                return;
            }
            command => command,
        };
        if command.changes_session() {
            self.epoch.set(self.epoch.get() + 1);
        }
        if self.commands.send((self.epoch.get(), command)).is_err() {
            log::error!("the backend has stopped");
        }
    }

    /// Every event that arrived since the last call.
    pub fn poll(&self) -> Vec<Event> {
        let events: Vec<_> = self
            .events
            .try_iter()
            .filter_map(|(epoch, event)| {
                if !event.current(epoch, self.epoch.get()) {
                    return None;
                }
                let epoch = if matches!(&event, Event::SignInFailed(_) | Event::SessionExpired(_)) {
                    self.send(Command::SignOut);
                    self.epoch.get()
                } else {
                    epoch
                };
                Some((epoch, event))
            })
            .collect();
        events
            .into_iter()
            .filter_map(|(epoch, event)| event.current(epoch, self.epoch.get()).then_some(event))
            .collect()
    }
}

async fn dispatch(ordered: bool, task: impl Future<Output = ()> + Send + 'static) {
    if ordered {
        task.await;
    } else {
        tokio::spawn(task);
    }
}

/// A load may not outlive this: a request that never answers shows the
/// listing as failed, with a fixed message, rather than a spinner forever.
const LOAD_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);

/// The deadline of a load. The feed has none: it reads the RSS of every device
/// subscription, eight at a time, so a cold cache of a large Takeout outgrows
/// any fixed minute, and each channel already answers its own failure.
fn deadline_of(request: &Request) -> Option<std::time::Duration> {
    match request {
        Request::Feed { .. } => None,
        _ => Some(LOAD_DEADLINE),
    }
}

/// Fixed failure texts: they never echo the payload, which a panic message of
/// a dependency can carry.
const LOAD_NO_ANSWER: &str = "the listing's task stopped before an answer";
const LOAD_OVERDUE: &str = "the listing's task outlived its minute";

/// Aborts the load it watches when dropped. Dropping the watcher (the slot's
/// request was superseded and its task aborted) is what stops the load.
struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Runs `future` in its own task and answers its result, a panic (an
/// `unwrap` on a missing line in rustypipe, say) or an overdue load with a
/// failure whose text carries no payload. Every ending sends the
/// `Event::Loaded`: the interface is never left loading forever.
///
/// Only builds that unwind (development builds and tests) ever see the
/// panic arm: the release build sets `panic = "abort"`, so there a panic
/// ends the process before the task can report it. What a release build
/// gets from this is the deadline.
///
/// The outcome of the inner task is only inspected after a completion: the
/// watcher aborts what still runs when it itself is dropped (also on the
/// deadline), so a superseded slot's load stops with its watcher's abort.
async fn watched<R: Send + 'static>(
    future: impl Future<Output = Result<R, String>> + Send + 'static,
    deadline: Option<std::time::Duration>,
) -> Result<R, String> {
    let load_task = tokio::spawn(future);
    // A named `let` keeps it alive to the end of the scope, when the abort of
    // an unfinished load (also one stopped by the deadline) is asked for.
    let _watcher = AbortOnDrop(load_task.abort_handle());
    let outcome = match deadline {
        Some(deadline) => tokio::time::timeout(deadline, load_task).await,
        None => Ok(load_task.await),
    };
    match outcome {
        // The load's own failure keeps its controlled text.
        Ok(Ok(Ok(answer))) => Ok(answer),
        Ok(Ok(Err(error))) => Err(error),
        Ok(Err(_)) => {
            log::error!("a load task panicked; its payload stays hidden");
            Err(LOAD_NO_ANSWER.to_owned())
        }
        Err(_) => {
            log::warn!("a load task outlived its deadline");
            Err(LOAD_OVERDUE.to_owned())
        }
    }
}

async fn handle(command: Command, context: Context) {
    match command {
        Command::MarkWatched { id, session } => {
            if session != context.epoch {
                return;
            }
            let _turn = context.account_writes.lock().await;
            match tokio::time::timeout(
                std::time::Duration::from_secs(20),
                innertube::mark_watched(&context.rp, &id),
            )
            .await
            {
                Ok(Ok(())) => context.send(Event::HistoryMarked),
                _ => log::warn!("history: playback report failed"),
            }
        }
        Command::Load {
            slot,
            generation,
            request,
        } => {
            let worker = context.clone();
            let loader = context.clone();
            log::debug!("loading {slot:?} started");
            let deadline = deadline_of(&request);
            let task = tokio::spawn(async move {
                // Watched: a load that never answers still answers the
                // interface, and so does a panic where builds unwind
                // (`watched`).
                let result = watched(async move { load(&loader, request).await }, deadline).await;
                if let Err(error) = &result {
                    log::warn!("loading {slot:?} failed: {error}");
                }
                worker.send(Event::Loaded {
                    slot,
                    generation,
                    result,
                });
            });
            // The slot's older request still on its way is now useless.
            if let Some(previous) = context.loads.lock().unwrap().insert(slot, task) {
                previous.abort();
            }
        }
        Command::LoadPlaylistMore {
            generation,
            id,
            more,
        } => {
            let result = innertube::more_videos(&context.rp, &more).await;
            context.send(Event::PlayingPlaylist {
                generation,
                id,
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
        Command::LoadExtras {
            generation,
            id,
            language,
        } => {
            let result = crate::youtube::extras::load(&context.rp, &id, &language).await;
            context.send(Event::Extras { generation, result });
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
                token,
                result,
            });
        }
        Command::Suggest(query) => match innertube::suggestions(&context.rp, &query).await {
            Ok(items) => context.send(Event::Suggestions { query, items }),
            Err(error) => log::warn!("search suggestions failed: {error}"),
        },
        Command::LoadSegments(id) => {
            let lookup = id.clone();
            match tokio::task::spawn_blocking(move || {
                crate::youtube::sponsorblock::segments(&lookup)
            })
            .await
            {
                Ok(Ok(segments)) => context.send(Event::Segments { id, segments }),
                Ok(Err(error)) => log::warn!("{error}"),
                Err(error) => log::warn!("the SponsorBlock lookup stopped: {error}"),
            }
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
            let path = file.path().to_owned();
            let import = tokio::task::spawn_blocking(move || {
                match std::fs::File::open(path) {
                    Ok(file) => takeout::parse_reader(std::io::BufReader::new(file)),
                    // An inaccessible file is not read without a size limit.
                    Err(_) => Err("Could not open this Takeout file.".into()),
                }
            })
            .await
            .unwrap_or_else(|_| Err("Could not import this Takeout file.".into()));
            context.send(Event::Imported(import));
        }
        Command::MergeImport {
            mut library,
            import,
            revision,
        } => {
            match tokio::task::spawn_blocking(move || {
                let counts = library.merge(import);
                Event::ImportMerged {
                    library,
                    counts,
                    revision,
                }
            })
            .await
            {
                Ok(event) => context.send(event),
                Err(_) => context.send(Event::Notice("Could not merge this Takeout file.".into())),
            }
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
                    None => {
                        context.send(Event::SignedOut);
                        return;
                    }
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
        Command::Notify { videos, language } => {
            // notify-rust's zbus backend blocks on D-Bus; off the runtime.
            tokio::task::spawn_blocking(move || {
                let (summary, body) = crate::logic::alerts::notify_text(&videos, language);
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
        Command::CheckForUpdates { manual } => {
            let result = tokio::task::spawn_blocking(crate::desktop::updater::check_latest)
                .await
                .unwrap_or_else(|_| Err("Could not check for updates. Retry later.".to_owned()));
            if result.is_err() {
                log::warn!("update check failed");
            }
            context.send(Event::UpdateChecked { result, manual });
        }
        Command::InstallUpdate { release, auto } => {
            let cache_dir = context.dirs.cache.clone();
            let to_install = release.clone();
            let result = tokio::task::spawn_blocking(move || {
                crate::desktop::updater::install_release(&to_install, &cache_dir)
            })
            .await
            .unwrap_or_else(|_| Err("Could not install the update on this system.".to_owned()));
            if result.is_err() {
                log::warn!("update installation failed");
            } else {
                log::info!("updated youtfast");
            }
            context.send(Event::UpdateInstalled {
                release,
                auto,
                result,
            });
        }
        Command::NotifyUpdate {
            version,
            kind,
            auto_installed,
            language,
        } => {
            tokio::task::spawn_blocking(move || {
                let (summary, body) = crate::desktop::updater::notify_update_text(
                    &version,
                    kind,
                    auto_installed,
                    language,
                );
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
        Command::CacheThumbnails(urls) => {
            let dir = context.dirs.cache.join("thumbnails");
            let _ = std::fs::create_dir_all(&dir);
            for (url, refresh) in urls {
                if !context.thumb_pending.lock().unwrap().insert(url.clone()) {
                    continue;
                }
                let path = crate::youtube::thumbs::path(&dir, &url);
                let context = context.clone();
                tokio::spawn(async move {
                    // A kept file answers at once: only downloads wait for
                    // a turn, so a page's cached thumbnails never queue
                    // behind its new ones.
                    let kept = if refresh {
                        None
                    } else {
                        let path = path.clone();
                        tokio::task::spawn_blocking(move || crate::youtube::thumbs::cached(&path))
                            .await
                            .ok()
                            .flatten()
                    };
                    let result = match kept {
                        Some(bytes) => Ok(bytes),
                        None => {
                            let _permit = context.thumb_sem.acquire().await;
                            let task = url.clone();
                            tokio::task::spawn_blocking(move || {
                                crate::youtube::thumbs::load_or_fetch(&task, &path, refresh)
                            })
                            .await
                            .unwrap_or(Err(crate::youtube::thumbs::Failure::Network))
                        }
                    };
                    if let Err(reason) = &result {
                        log::warn!("{}", reason.code());
                    }
                    context.thumb_pending.lock().unwrap().remove(&url);
                    context.send(Event::Thumbnail { url, result });
                });
            }
        }
    }
}

/// How long a save waits for more saves to pile up behind it: a burst —
/// a video's open, its details and its finish can be seconds apart —
/// writes once, not once each. `on_exit` saves directly, so this delay
/// never puts the last state at risk.
const SAVE_DEBOUNCE: std::time::Duration = std::time::Duration::from_secs(1);

/// Writes the newest snapshot of one file, one write at a time: saves sent
/// during a write collapse into the next, and an older snapshot never
/// lands after a newer one. `what` names the file ("library"); a failed
/// write is logged and shown.
async fn writer<T: Clone + Send + Sync + 'static>(
    context: Context,
    what: &'static str,
    path: PathBuf,
    mut snapshots: watch::Receiver<Option<T>>,
    gate: Arc<SaveGate>,
    save: fn(&T, &Path) -> std::io::Result<()>,
) {
    while snapshots.changed().await.is_ok() {
        tokio::time::sleep(SAVE_DEBOUNCE).await;
        let Some(value) = snapshots.borrow_and_update().clone() else {
            continue;
        };
        let path = path.clone();
        let gate = gate.clone();
        match tokio::task::spawn_blocking(move || gate.write(false, || save(&value, &path))).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                log::error!("could not save the {what}: {error}");
                context.send(Event::Notice(format!("Could not save the {what}: {error}")));
            }
            Err(error) => log::error!("saving the {what} failed: {error}"),
        }
    }
}

/// The seeds' related videos, asked for together, ranked. A seed whose
/// details fail is skipped.
async fn local_recommendations(
    rp: &RustyPipe,
    seeds: Vec<crate::logic::recommend::Seed>,
    taste: &crate::logic::recommend::Taste,
) -> Vec<Video> {
    let mut tasks = tokio::task::JoinSet::new();
    for seed in seeds {
        let rp = rp.clone();
        tasks.spawn(async move {
            let related = innertube::details(&rp, &seed.id)
                .await
                .map(|details| details.related);
            (seed.weight, related)
        });
    }
    let mut related = Vec::new();
    while let Some(done) = tasks.join_next().await {
        match done {
            Ok((weight, Ok(videos))) => related.push((weight, videos)),
            Ok((_, Err(error))) => log::warn!("a seed's related videos failed: {error}"),
            Err(error) => log::warn!("a seed's lookup stopped: {error}"),
        }
    }
    // The order the answers came in is not the seeds': keep it stable.
    related.sort_by(|a, b| b.0.total_cmp(&a.0));
    crate::logic::recommend::rank(&related, taste)
}

async fn load(context: &Context, request: Request) -> Result<Payload, String> {
    let rp = &context.rp;
    let text = innertube::text;
    Ok(match request {
        Request::Search { query, options } => {
            let (items, more) = innertube::search_filtered(rp, &query, options).await?;
            Payload::Search { items, more }
        }
        Request::MoreSearch { more, kind } => {
            let (items, more) = match kind {
                innertube::SearchKind::Videos => {
                    let (items, more) = innertube::more_videos(rp, &more).await?;
                    (PageItems::Videos(items), more)
                }
                innertube::SearchKind::Channels => {
                    let (items, more) = innertube::more_channels(rp, &more).await?;
                    (PageItems::Channels(items), more)
                }
                innertube::SearchKind::Playlists => {
                    let (items, more) = innertube::more_playlists(rp, &more).await?;
                    (PageItems::Playlists(items), more)
                }
            };
            Payload::More { items, more }
        }
        Request::Feed { channels, account } => {
            let (videos, more) = innertube::feed(rp, channels, account, &context.rss).await?;
            Payload::Feed { videos, more }
        }
        Request::ForYou {
            account,
            seeds,
            taste,
        } => {
            if account {
                match innertube::recommended(rp).await {
                    Ok((videos, more)) if !videos.is_empty() => {
                        return Ok(Payload::ForYou {
                            videos,
                            more,
                            from_account: true,
                        });
                    }
                    Ok(_) => log::info!("the account has no recommendations; making some here"),
                    Err(error) => log::warn!("the account's recommendations failed: {error}"),
                }
            }
            Payload::ForYou {
                videos: local_recommendations(rp, seeds, &taste).await,
                more: None,
                from_account: false,
            }
        }
        Request::MoreFeed(more)
        | Request::MoreVideos(more)
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
            let (videos, _) = innertube::feed(rp, channels, account, &context.rss).await?;
            Payload::Feed { videos, more: None }
        }
        Request::Subscriptions => {
            let (items, more) = innertube::subscriptions(rp).await.map_err(text)?;
            Payload::Channels { items, more }
        }
        Request::MoreSubscriptions(more) => {
            let (items, next) = innertube::more_channels(rp, &more).await?;
            Payload::More {
                items: PageItems::Channels(items),
                more: next,
            }
        }
        Request::History => {
            let (items, more) = innertube::history(rp).await.map_err(text)?;
            Payload::History { items, more }
        }
        Request::MoreHistory(more) => {
            let (items, next) = innertube::more_history(rp, &more).await?;
            Payload::More {
                items: PageItems::History(items),
                more: next,
            }
        }
        Request::WatchLater => {
            let (items, more) = innertube::watch_later(rp).await.map_err(text)?;
            Payload::VideoList { items, more }
        }
        Request::Liked => {
            let (items, more) = innertube::liked(rp).await.map_err(text)?;
            Payload::VideoList { items, more }
        }
        Request::Playlists => {
            let (items, more) = innertube::playlists(rp).await.map_err(text)?;
            Payload::Playlists { items, more }
        }
        Request::Playlist(id) => {
            let (details, items, more) = innertube::playlist_details(rp, &id).await?;
            Payload::PlaylistPage {
                details,
                items,
                more,
            }
        }
        Request::ChannelTab { id, tab, order } => match tab {
            crate::model::ChannelTab::Playlists => {
                let (header, items, more) = innertube::channel_playlists_page(rp, &id).await?;
                Payload::ChannelPage {
                    header,
                    items: PageItems::Playlists(items),
                    more,
                }
            }
            _ => {
                let (header, items, more) = innertube::channel_tab(rp, &id, &tab, order).await?;
                Payload::ChannelPage {
                    header,
                    items: PageItems::Videos(items),
                    more,
                }
            }
        },
        Request::ChannelAbout { id, needs_header } => {
            let (header, about) = innertube::channel_about(rp, &id, needs_header).await?;
            Payload::About {
                header: header.map(Box::new),
                about,
            }
        }
        Request::More {
            more,
            playlists: true,
        }
        | Request::MorePlaylists(more) => {
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
    let result = account::save_session(id, cookie).await;
    if result.is_err() {
        innertube::sign_out(&context.rp).await;
    }
    result
}

/// Reads a browser's session and starts it as account `id`.
async fn session_from_browser(context: &Context, spec: &str, id: u32) -> Result<(), String> {
    let cookie = account::cookies_from_browser(spec).await?;
    start_session(context, cookie, id).await
}

/// Starts using a session; `rejected` is what to say when YouTube does
/// not accept it.
async fn sign_in(context: &Context, cookie: String, rejected: &str) -> Result<(), String> {
    let result = innertube::sign_in(&context.rp, cookie).await;
    if result.is_err() {
        innertube::sign_out(&context.rp).await;
    }
    result.map_err(|error| {
        if innertube::is_signed_out(&error) {
            rejected.to_owned()
        } else {
            innertube::text(error)
        }
    })
}

fn finish_sign_in(context: &Context, account: Account, result: Result<(), String>) {
    match result {
        Ok(()) => {
            log::info!("signed in");
            context.send(Event::AccountAdded(account.clone()));
            signed_in(context, &account);
        }
        Err(error) => {
            log::warn!("sign-in failed: {error}");
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
        log::info!("renewing the session");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Channel, Rating};

    #[tokio::test]
    async fn a_load_that_panics_still_answers_with_a_controlled_failure() {
        let answer: Result<(), String> = watched(
            async { panic!("a synthetic panic") },
            Some(std::time::Duration::from_secs(1)),
        )
        .await;
        assert_eq!(answer.unwrap_err(), LOAD_NO_ANSWER);
    }

    #[tokio::test]
    async fn an_overdue_load_answers_with_the_deadline_failure() {
        let answer = watched(
            std::future::pending::<Result<(), String>>(),
            Some(std::time::Duration::from_millis(5)),
        )
        .await;
        assert_eq!(answer.unwrap_err(), LOAD_OVERDUE);
    }

    #[tokio::test]
    async fn a_normal_load_passes_its_answer_through() {
        let answer = watched(
            async { Ok::<u8, String>(7) },
            Some(std::time::Duration::from_secs(1)),
        )
        .await;
        assert_eq!(answer, Ok(7));
    }

    #[tokio::test]
    async fn a_failing_load_keeps_its_own_controlled_text() {
        let answer = watched(
            async { Err::<u8, String>("the feed is gone".into()) },
            Some(std::time::Duration::from_secs(1)),
        )
        .await;
        assert_eq!(answer.unwrap_err(), "the feed is gone");
    }

    #[tokio::test]
    async fn a_load_without_deadline_still_answers_a_panic() {
        let answer: Result<(), String> = watched(async { panic!("a synthetic panic") }, None).await;
        assert_eq!(answer.unwrap_err(), LOAD_NO_ANSWER);
    }

    #[test]
    fn only_the_feed_loads_without_deadline() {
        let feed = Request::Feed {
            channels: Vec::new(),
            account: false,
        };
        assert_eq!(deadline_of(&feed), None);
        assert_eq!(deadline_of(&Request::Subscriptions), Some(LOAD_DEADLINE));
    }

    #[tokio::test]
    async fn dropping_the_watcher_stops_the_load_it_watches() {
        let task = tokio::spawn(std::future::pending::<()>());
        let watcher = AbortOnDrop(task.abort_handle());
        drop(watcher);
        // A task of `pending` can only end aborted.
        assert!(task.await.is_err());
    }

    #[test]
    fn search_requests_keep_filters_kind_generation_and_session_epoch() {
        let (backend, _outgoing, mut incoming) = scoped_backend(7);
        for &kind in innertube::SearchKind::ALL {
            let options = innertube::SearchOptions {
                kind,
                order: innertube::SearchOrder::Date,
                date: innertube::SearchDate::Day,
                length: innertube::SearchLength::Medium,
                live: true,
            };
            backend.send(Command::Load {
                slot: Slot::Search,
                generation: 42,
                request: Request::Search {
                    query: "Consulta sintética".into(),
                    options,
                },
            });
            let Ok((
                epoch,
                Command::Load {
                    slot,
                    generation,
                    request:
                        Request::Search {
                            query,
                            options: received,
                        },
                },
            )) = incoming.try_recv()
            else {
                panic!("search request lost")
            };
            assert_eq!((epoch, slot, generation), (7, Slot::Search, 42));
            assert_eq!(query, "Consulta sintética");
            assert_eq!(received, options);
        }
    }

    #[test]
    fn account_changes_drop_search_results_and_playing_playlist_pages_from_old_sessions() {
        let (backend, outgoing, _incoming) = scoped_backend(2);
        for epoch in [1, 2] {
            for &kind in innertube::SearchKind::ALL {
                let items = match kind {
                    innertube::SearchKind::Videos => {
                        PageItems::Videos(vec![Video::from_id("synthetic".into())])
                    }
                    innertube::SearchKind::Channels => {
                        PageItems::Channels(vec![Channel::default()])
                    }
                    innertube::SearchKind::Playlists => PageItems::Playlists(vec![]),
                };
                outgoing
                    .send((
                        epoch,
                        Event::Loaded {
                            slot: Slot::Search,
                            generation: epoch,
                            result: Ok(Payload::Search { items, more: None }),
                        },
                    ))
                    .unwrap();
            }
            outgoing
                .send((
                    epoch,
                    Event::PlayingPlaylist {
                        generation: 1,
                        id: "playlist".into(),
                        result: Ok((vec![Video::from_id("synthetic".into())], None)),
                    },
                ))
                .unwrap();
        }
        let events = backend.poll();
        assert_eq!(events.len(), 4);
        assert!(
            events
                .iter()
                .filter_map(|event| match event {
                    Event::Loaded { generation, .. } => Some(*generation),
                    _ => None,
                })
                .all(|generation| generation == 2)
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, Event::PlayingPlaylist { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn authentication_failure_invalidates_the_entire_search_and_playlist_batch() {
        let (backend, outgoing, mut incoming) = scoped_backend(3);
        outgoing
            .send((
                3,
                Event::Loaded {
                    slot: Slot::Search,
                    generation: 1,
                    result: Ok(Payload::Search {
                        items: PageItems::Channels(vec![Channel::default()]),
                        more: None,
                    }),
                },
            ))
            .unwrap();
        outgoing
            .send((3, Event::SessionExpired("Sessão expirada".into())))
            .unwrap();
        outgoing
            .send((
                3,
                Event::PlayingPlaylist {
                    generation: 1,
                    id: "playlist".into(),
                    result: Ok((Vec::new(), None)),
                },
            ))
            .unwrap();
        let events = backend.poll();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], Event::SessionExpired(_)));
        assert!(matches!(incoming.try_recv(), Ok((4, Command::SignOut))));
    }

    #[test]
    fn search_and_playing_playlist_failures_are_controlled_before_the_interface() {
        let error = "Cookie: SAPISID=synthetic-secret https://youtube.com/private";
        let events = [
            Event::Loaded {
                slot: Slot::Search,
                generation: 1,
                result: Err(error.into()),
            },
            Event::PlayingPlaylist {
                generation: 1,
                id: "playlist".into(),
                result: Err(error.into()),
            },
        ];
        for event in events {
            let message = match event.sanitize() {
                Event::Loaded {
                    result: Err(error), ..
                }
                | Event::PlayingPlaylist {
                    result: Err(error), ..
                } => error,
                _ => panic!("expected a controlled failure"),
            };
            assert!(!message.contains("synthetic-secret") && !message.contains("https://"));
        }
    }

    type ScopedBackend = (
        Backend,
        mpsc::Sender<(u64, Event)>,
        async_mpsc::UnboundedReceiver<(u64, Command)>,
    );

    fn scoped_backend(epoch: u64) -> ScopedBackend {
        let (commands, incoming) = async_mpsc::unbounded_channel();
        let (outgoing, events) = mpsc::channel();
        (
            Backend {
                commands,
                events,
                epoch: std::cell::Cell::new(epoch),
                waker: egui::Context::default(),
                library_saves: Arc::new(watch::channel(None).0),
                settings_saves: Arc::new(watch::channel(None).0),
                library_gate: Arc::new(SaveGate::default()),
                settings_gate: Arc::new(SaveGate::default()),
            },
            outgoing,
            incoming,
        )
    }

    #[test]
    fn failed_authentication_invalidates_private_responses_before_and_after_it_in_the_batch() {
        for failure in [
            Event::SessionExpired("expired".into()),
            Event::SignInFailed("failed".into()),
        ] {
            let (backend, outgoing, mut incoming) = scoped_backend(1);
            outgoing
                .send((
                    1,
                    Event::Rating {
                        id: "before-failure".into(),
                        result: Ok(Rating::Like),
                    },
                ))
                .unwrap();
            outgoing.send((1, failure)).unwrap();
            outgoing
                .send((
                    1,
                    Event::Rating {
                        id: "same-video".into(),
                        result: Ok(Rating::Like),
                    },
                ))
                .unwrap();
            outgoing
                .send((
                    1,
                    Event::SaveTargets {
                        id: "same-video".into(),
                        result: Ok(Vec::new()),
                    },
                ))
                .unwrap();
            outgoing.send((0, Event::CheckAlerts)).unwrap();
            assert_eq!(backend.poll().len(), 2);
            assert!(matches!(incoming.try_recv(), Ok((2, Command::SignOut))));
        }
    }

    #[test]
    fn private_external_errors_are_sanitized_before_reaching_the_interface() {
        let error = "https://youtube.com/private Cookie: SAPISID=synthetic-secret";
        let events = [
            Event::Loaded {
                slot: Slot::History,
                generation: 1,
                result: Err(error.into()),
            },
            Event::Details {
                generation: 1,
                result: Box::new(Err(error.into())),
            },
            Event::RateFailed {
                id: "video".into(),
                previous: Rating::None,
                error: error.into(),
            },
            Event::SignInFailed(error.into()),
        ];
        for event in events {
            let event = event.sanitize();
            let error = match event {
                Event::Loaded {
                    result: Err(error), ..
                }
                | Event::RateFailed { error, .. }
                | Event::SignInFailed(error) => error,
                Event::Details { result, .. } => result.unwrap_err(),
                _ => panic!("unexpected event"),
            };
            assert!(!error.contains("synthetic-secret") && !error.contains("https://"));
        }
    }

    #[test]
    fn the_final_save_waits_for_an_old_write_and_rejects_later_old_snapshots() {
        let gate = Arc::new(SaveGate::default());
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "final-save-{}-{}.json",
                std::process::id(),
                crate::util::now()
            ));
        let (started, ready) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let old_gate = gate.clone();
        let old_path = path.clone();
        let old = std::thread::spawn(move || {
            old_gate.write(false, || {
                started.send(()).unwrap();
                wait.recv().unwrap();
                crate::storage::paths::write_atomic(&old_path, b"old")
            })
        });
        ready
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let final_gate = gate.clone();
        let final_path = path.clone();
        let final_write = std::thread::spawn(move || {
            final_gate.write(true, || {
                crate::storage::paths::write_atomic(&final_path, b"final")
            })
        });
        release.send(()).unwrap();
        old.join().unwrap().unwrap();
        final_write.join().unwrap().unwrap();
        gate.write(false, || {
            crate::storage::paths::write_atomic(&path, b"queued-old")
        })
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"final");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn changing_accounts_drops_old_private_responses_even_for_the_same_video() {
        let (backend, outgoing, _incoming) = scoped_backend(2);
        for epoch in [1, 2] {
            outgoing
                .send((
                    epoch,
                    Event::Rating {
                        id: "same-video".into(),
                        result: Ok(Rating::Like),
                    },
                ))
                .unwrap();
            outgoing
                .send((
                    epoch,
                    Event::SaveTargets {
                        id: "same-video".into(),
                        result: Ok(Vec::new()),
                    },
                ))
                .unwrap();
            outgoing
                .send((epoch, Event::SignedIn(epoch as u32)))
                .unwrap();
            outgoing
                .send((
                    epoch,
                    Event::Loaded {
                        slot: Slot::History,
                        generation: 1,
                        result: Ok(Payload::Videos(Vec::new())),
                    },
                ))
                .unwrap();
        }
        outgoing.send((0, Event::CheckAlerts)).unwrap();
        assert_eq!(backend.poll().len(), 5);
    }

    #[tokio::test]
    async fn session_changes_wait_for_earlier_writes() {
        let steps = Arc::new(std::sync::Mutex::new(Vec::new()));
        let (release, wait) = tokio::sync::oneshot::channel();
        let (started, ready) = tokio::sync::oneshot::channel();
        let history = steps.clone();
        let worker = tokio::spawn(async move {
            dispatch(true, async move {
                history.lock().unwrap().push("write-A");
                started.send(()).unwrap();
                wait.await.unwrap();
                history.lock().unwrap().push("finished-A");
            })
            .await;
            let switched = steps.clone();
            dispatch(true, async move {
                switched.lock().unwrap().push("switch-B");
            })
            .await;
            steps
        });
        ready.await.unwrap();
        release.send(()).unwrap();
        let steps = worker.await.unwrap();
        assert_eq!(
            *steps.lock().unwrap(),
            ["write-A", "finished-A", "switch-B"]
        );
        assert!(
            Command::Rate {
                id: "x".into(),
                rating: Rating::Like,
                previous: Rating::None
            }
            .ordered()
        );
        assert!(
            Command::SetSaved {
                id: "x".into(),
                playlist: "p".into(),
                saved: true
            }
            .ordered()
        );
        assert!(
            Command::MarkWatched {
                id: "synthetic".into(),
                session: 1
            }
            .ordered()
        );
        assert!(!Event::HistoryMarked.current(1, 2));
        assert!(Event::HistoryMarked.current(2, 2));
        assert!(Command::SignOut.ordered());
        assert!(
            Command::Switch(Account {
                id: 2,
                name: "B".into(),
                source: AccountSource::File
            })
            .ordered()
        );
    }

    #[tokio::test]
    async fn kept_thumbnails_answer_while_downloads_hold_every_turn() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("kept-thumbnail-turns-{}", std::process::id()));
        let dirs = Dirs {
            data: root.join("data"),
            config: root.join("config"),
            cache: root.join("cache"),
        };
        let url = "https://i.ytimg.com/vi/synthetic/mqdefault.jpg".to_owned();
        let kept = crate::youtube::thumbs::path(&dirs.cache.join("thumbnails"), &url);
        std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
        let mut png = Vec::new();
        image::ImageEncoder::write_image(
            image::codecs::png::PngEncoder::new(&mut png),
            &[0, 0, 0, 255],
            1,
            1,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
        std::fs::write(&kept, &png).unwrap();
        let (outgoing, events) = mpsc::channel();
        let context = Context {
            rp: innertube::client(&dirs).unwrap(),
            dirs,
            events: outgoing,
            epoch: 0,
            waker: egui::Context::default(),
            alerts_timer: Arc::default(),
            library_saves: Arc::new(watch::channel(None).0),
            settings_saves: Arc::new(watch::channel(None).0),
            account_writes: Arc::default(),
            loads: Arc::default(),
            rss: Arc::default(),
            thumb_pending: Arc::default(),
            thumb_sem: Arc::new(tokio::sync::Semaphore::new(
                crate::youtube::thumbs::DOWNLOADS,
            )),
        };
        // Slow downloads hold every turn.
        let _turns = context
            .thumb_sem
            .clone()
            .acquire_many_owned(crate::youtube::thumbs::DOWNLOADS as u32)
            .await
            .unwrap();
        handle(
            Command::CacheThumbnails(vec![(url.clone(), false)]),
            context,
        )
        .await;
        let answer = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if let Ok((_, event)) = events.try_recv() {
                    break event;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("a kept thumbnail answers without a download turn");
        assert!(matches!(
            answer,
            Event::Thumbnail { url: answered, result: Ok(bytes) }
                if answered == url && bytes.as_ref() == png.as_slice()
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
