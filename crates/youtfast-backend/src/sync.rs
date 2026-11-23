//! Direct device library sync over Tailscale, NetBird, and private LANs.

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, Semaphore, oneshot, watch};
use tokio::task::JoinSet;

use crate::{Context, Event};
use youtfast_storage::library::Library;
use youtfast_storage::settings::DeviceSync;
use youtfast_storage::sync::{
    ERR_INCOMPATIBLE_VERSION, ERR_INVALID_RESPONSE, ERR_LISTENER_FAILED, ERR_NO_PEERS_FOUND,
    ERR_PAIRING_KEY_INVALID, ERR_PRIVATE_ONLY, ERR_SIZE_LIMIT, ERR_UNREACHABLE, MIN_SYNC_PORT,
    PAIRING_KEY_BYTES, SyncSnapshot, is_private_or_mesh_ip, local_sync_addresses,
    normalize_pairing_key, parse_peer_target, parse_sync_bytes, to_sync_json_bytes,
};

mod discovery;
mod transport;
use transport::{PacketKind, SecurePeer};

const MAX_CONCURRENT_INCOMING: usize = 4;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const IO_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a search of Tailscale and NetBird is reused by automatic syncs.
const DISCOVERY_TTL: Duration = Duration::from_secs(10 * 60);
/// Most devices remembered between rounds.
const MAX_PEERS: usize = 32;
/// Most exchanges a round runs at once; every candidate is still tried.
const EXCHANGES_AT_ONCE: usize = 8;

#[derive(Clone, Default)]
struct ActiveConfig {
    generation: u64,
    settings: DeviceSync,
    /// Devices that completed an exchange with the current key, synced
    /// when no address is set.
    peers: Vec<SocketAddr>,
    /// When Tailscale and NetBird were last searched for peers.
    searched_at: Option<Instant>,
}

impl ActiveConfig {
    fn remember(&mut self, peer: SocketAddr) {
        if !self.peers.contains(&peer) && self.peers.len() < MAX_PEERS {
            self.peers.push(peer);
        }
    }
}

struct RunningListener {
    port: u16,
    shutdown: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

#[derive(Clone)]
pub(crate) struct SyncManager {
    active: Arc<std::sync::Mutex<ActiveConfig>>,
    generation: watch::Sender<u64>,
    listener: Arc<Mutex<Option<RunningListener>>>,
    timer: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    client_lock: Arc<Mutex<()>>,
    incoming_sem: Arc<Semaphore>,
}

impl Default for SyncManager {
    fn default() -> Self {
        Self {
            active: Arc::new(std::sync::Mutex::new(ActiveConfig::default())),
            generation: watch::channel(0).0,
            listener: Arc::new(Mutex::new(None)),
            timer: Arc::new(Mutex::new(None)),
            client_lock: Arc::new(Mutex::new(())),
            incoming_sem: Arc::new(Semaphore::new(MAX_CONCURRENT_INCOMING)),
        }
    }
}

impl SyncManager {
    fn active(&self) -> std::sync::MutexGuard<'_, ActiveConfig> {
        self.active
            .lock()
            .expect("device sync config lock poisoned")
    }

    async fn while_current<T>(&self, generation: u64, work: impl Future<Output = T>) -> Option<T> {
        let mut current = self.generation.subscribe();
        tokio::select! {
            biased;
            _ = current.wait_for(|&value| value != generation) => None,
            result = work => Some(result),
        }
    }
}

pub(crate) async fn configure(context: &Context, next: DeviceSync, generation: u64) {
    let next = next.sanitized();
    let should_listen = next.enabled && normalize_pairing_key(&next.pairing_key).is_some();
    let every_minutes = if should_listen { next.every_minutes } else { 0 };

    {
        let mut active = context.sync.active();
        if !should_listen
            || active.settings.pairing_key != next.pairing_key
            || active.settings.peer_address != next.peer_address
            || active.settings.port != next.port
        {
            active.peers.clear();
            active.searched_at = None;
        }
        active.settings = next.clone();
        active.generation = generation;
        context.sync.generation.send_replace(generation);
    }

    {
        let mut timer = context.sync.timer.lock().await;
        if let Some(handle) = timer.take() {
            handle.abort();
        }
        if every_minutes > 0 {
            let ctx = context.clone();
            *timer = Some(tokio::spawn(async move {
                let interval = Duration::from_secs(u64::from(every_minutes) * 60);
                loop {
                    tokio::time::sleep(interval).await;
                    ctx.send(Event::CheckDeviceSync { generation });
                }
            }));
        }
    }

    let mut listener = context.sync.listener.lock().await;
    if listener.as_ref().is_some_and(|running| {
        should_listen && running.port == next.port && !running.task.is_finished()
    }) {
        return;
    }
    if let Some(old) = listener.take() {
        let _ = old.shutdown.send(());
        let _ = tokio::time::timeout(Duration::from_secs(1), old.task).await;
    }
    if !should_listen {
        return;
    }

    match TcpListener::bind(("0.0.0.0", next.port)).await {
        Ok(tcp) => {
            let (shutdown, receiver) = oneshot::channel();
            let task = tokio::spawn(serve_loop(tcp, next.port, context.clone(), receiver));
            *listener = Some(RunningListener {
                port: next.port,
                shutdown,
                task,
            });
        }
        Err(_) => {
            log::warn!("device sync listener failed");
            context.send(Event::DeviceSyncCompleted {
                generation,
                result: Err(ERR_LISTENER_FAILED.to_owned()),
                manual: false,
            });
        }
    }
}

async fn serve_loop(
    tcp: TcpListener,
    port: u16,
    context: Context,
    mut shutdown: oneshot::Receiver<()>,
) {
    loop {
        let accepted = tokio::select! {
            biased;
            _ = &mut shutdown => break,
            accepted = tcp.accept() => accepted,
        };
        let Ok((stream, peer_addr)) = accepted else {
            log::warn!("device sync listener failed");
            break;
        };
        if !is_private_or_mesh_ip(peer_addr.ip()) {
            continue;
        }
        let (generation, key) = {
            let active = context.sync.active();
            if !active.settings.enabled || active.settings.port != port {
                continue;
            }
            let Some(key) = normalize_pairing_key(&active.settings.pairing_key) else {
                continue;
            };
            (active.generation, key)
        };
        let Ok(permit) = context.sync.incoming_sem.clone().try_acquire_owned() else {
            continue;
        };
        let ctx = context.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let work =
                tokio::time::timeout(IO_TIMEOUT, exchange_incoming(stream, &key, port, &ctx));
            if let Some(Ok(Ok(incoming))) = ctx.sync.while_current(generation, work).await {
                let mut active = ctx.sync.active();
                if active.generation != generation {
                    return;
                }
                if incoming.listen_port >= MIN_SYNC_PORT {
                    active.remember(SocketAddr::new(peer_addr.ip(), incoming.listen_port));
                }
                drop(active);
                ctx.send(Event::DeviceSyncIncoming {
                    generation,
                    snapshot: incoming,
                });
            }
        });
    }
}

async fn exchange_incoming(
    stream: TcpStream,
    key: &[u8; PAIRING_KEY_BYTES],
    port: u16,
    context: &Context,
) -> Result<Box<SyncSnapshot>, &'static str> {
    let mut peer = SecurePeer::handshake(stream, key, false).await?;
    let (kind, body) = peer.receive().await?;
    if kind != PacketKind::Request {
        return Err(ERR_INVALID_RESPONSE);
    }
    let incoming = match parse_snapshot(body).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let kind = match error {
                ERR_INCOMPATIBLE_VERSION => PacketKind::Incompatible,
                ERR_SIZE_LIMIT => PacketKind::TooLarge,
                _ => PacketKind::Invalid,
            };
            peer.send(kind, &[]).await?;
            return Err(error);
        }
    };
    // Exchange the latest local snapshot. Each app merges its peer's data
    // into the library it actually has when the event reaches the UI.
    let library = latest_library(&context.library_saves);
    let bytes = match serialize_snapshot(library, port).await {
        Ok(bytes) => bytes,
        Err(error) => {
            let kind = if error == ERR_SIZE_LIMIT {
                PacketKind::TooLarge
            } else {
                PacketKind::Invalid
            };
            peer.send(kind, &[]).await?;
            return Err(error);
        }
    };
    peer.send(PacketKind::Ok, &bytes).await?;
    Ok(incoming)
}

pub(crate) async fn sync_now(context: &Context, manual: bool, generation: u64) {
    let _flight = context.sync.client_lock.lock().await;
    let (settings, known, searched_at) = {
        let active = context.sync.active();
        if !active.settings.enabled || active.generation != generation {
            return;
        }
        (
            active.settings.clone(),
            active.peers.clone(),
            active.searched_at,
        )
    };
    let library = latest_library(&context.library_saves);
    let work = async {
        if settings.peer_address.trim().is_empty() {
            let search = manual || searched_at.is_none_or(|at| at.elapsed() >= DISCOVERY_TTL);
            sync_with_found_peers(&settings, known, search, library).await
        } else {
            let result = sync_with_address(&settings, library).await;
            (result.map(|snapshot| vec![*snapshot]), None)
        }
    };
    let Some((result, found)) = context.sync.while_current(generation, work).await else {
        return;
    };
    if let Some(found) = found {
        let mut active = context.sync.active();
        if active.generation != generation {
            return;
        }
        if found.searched {
            active.searched_at = Some(Instant::now());
        }
        // Keep devices that connected in while the round ran.
        let learned: Vec<SocketAddr> = std::mem::replace(&mut active.peers, found.reached)
            .into_iter()
            .filter(|peer| !found.known.contains(peer))
            .collect();
        for peer in learned {
            active.remember(peer);
        }
    }
    if let Err(error) = &result {
        log::warn!("device sync failed: {error}");
    }
    context.send(Event::DeviceSyncCompleted {
        generation,
        result,
        manual,
    });
}

/// The peers a round without an address reached, the ones it started from,
/// and whether it searched.
struct Found {
    reached: Vec<SocketAddr>,
    known: Vec<SocketAddr>,
    searched: bool,
}

/// Syncs with every known device and, when `search` is set or none of them
/// answers, with every Tailscale and NetBird peer that shares the key.
async fn sync_with_found_peers(
    settings: &DeviceSync,
    known: Vec<SocketAddr>,
    search: bool,
    library: Library,
) -> (Result<Vec<SyncSnapshot>, String>, Option<Found>) {
    let Some(key) = normalize_pairing_key(&settings.pairing_key) else {
        return (Err(ERR_PAIRING_KEY_INVALID.to_owned()), None);
    };
    let request = match serialize_snapshot(library, settings.port).await {
        Ok(bytes) => Arc::new(bytes),
        Err(error) => return (Err(error.to_owned()), None),
    };

    let mut replies = if search {
        Vec::new()
    } else {
        exchange_all(known.clone(), key, &request).await
    };
    // Known devices that all stopped answering may have changed address.
    let searched = search || (!known.is_empty() && !replies.iter().any(|(_, reply)| reply.is_ok()));
    if searched {
        let mut candidates = search_candidates(&known, settings.port).await;
        candidates.retain(|addr| !replies.iter().any(|(tried, _)| tried == addr));
        replies.extend(exchange_all(candidates, key, &request).await);
    }

    let mut reached = Vec::new();
    let mut snapshots = Vec::new();
    // Without any exchange, report the problem of a device that did accept the key.
    let mut failure = ERR_NO_PEERS_FOUND;
    for (addr, reply) in replies {
        match reply {
            Ok(snapshot) => {
                reached.push(addr);
                snapshots.push(*snapshot);
            }
            Err(error @ (ERR_INCOMPATIBLE_VERSION | ERR_SIZE_LIMIT | ERR_INVALID_RESPONSE)) => {
                failure = error;
            }
            Err(_) => {}
        }
    }
    let result = if snapshots.is_empty() {
        Err(failure.to_owned())
    } else {
        Ok(snapshots)
    };
    let found = Found {
        reached,
        known,
        searched,
    };
    (result, Some(found))
}

/// Known devices first, then every Tailscale and NetBird peer on the listen
/// port, without this computer's own addresses.
async fn search_candidates(known: &[SocketAddr], port: u16) -> Vec<SocketAddr> {
    let own: Vec<IpAddr> = local_sync_addresses();
    let mut candidates = known.to_vec();
    for ip in discovery::mesh_peers().await {
        let addr = SocketAddr::new(ip, port);
        if !own.contains(&ip) && !candidates.contains(&addr) {
            candidates.push(addr);
        }
    }
    log::info!("device sync is trying {} devices", candidates.len());
    candidates
}

/// Exchanges with every address, a few at a time to bound the buffered
/// snapshots (64 MiB each at most).
async fn exchange_all(
    addrs: Vec<SocketAddr>,
    key: [u8; PAIRING_KEY_BYTES],
    request: &Arc<Vec<u8>>,
) -> Vec<(SocketAddr, Result<Box<SyncSnapshot>, &'static str>)> {
    let slots = Arc::new(Semaphore::new(EXCHANGES_AT_ONCE));
    let mut exchanges = JoinSet::new();
    for addr in addrs {
        let request = Arc::clone(request);
        let slots = Arc::clone(&slots);
        exchanges.spawn(async move {
            let _slot = slots.acquire_owned().await;
            (addr, exchange(&[addr], &key, &request).await)
        });
    }
    let mut replies = Vec::new();
    while let Some(joined) = exchanges.join_next().await {
        if let Ok(reply) = joined {
            replies.push(reply);
        }
    }
    replies
}

async fn sync_with_address(
    settings: &DeviceSync,
    library: Library,
) -> Result<Box<SyncSnapshot>, String> {
    let key = normalize_pairing_key(&settings.pairing_key).ok_or(ERR_PAIRING_KEY_INVALID)?;
    let (host, port) = parse_peer_target(&settings.peer_address, settings.port)?;
    let addrs = resolve_private_targets(&host, port).await?;
    let request = serialize_snapshot(library, settings.port).await?;
    exchange(&addrs, &key, &request)
        .await
        .map_err(str::to_owned)
}

/// Sends this device's snapshot to the first of `addrs` that answers and
/// returns the peer's.
async fn exchange(
    addrs: &[SocketAddr],
    key: &[u8; PAIRING_KEY_BYTES],
    request: &[u8],
) -> Result<Box<SyncSnapshot>, &'static str> {
    let stream = connect_any(addrs).await?;
    tokio::time::timeout(IO_TIMEOUT, async {
        let mut peer = SecurePeer::handshake(stream, key, true).await?;
        peer.send(PacketKind::Request, request).await?;
        let (kind, body) = peer.receive().await?;
        match kind {
            PacketKind::Ok => parse_snapshot(body).await,
            PacketKind::Incompatible => Err(ERR_INCOMPATIBLE_VERSION),
            PacketKind::TooLarge => Err(ERR_SIZE_LIMIT),
            PacketKind::Request | PacketKind::Invalid => Err(ERR_INVALID_RESPONSE),
        }
    })
    .await
    .map_err(|_| ERR_UNREACHABLE)?
}

async fn serialize_snapshot(library: Library, port: u16) -> Result<Vec<u8>, &'static str> {
    tokio::task::spawn_blocking(move || to_sync_json_bytes(&library.sync_snapshot(port)))
        .await
        .map_err(|_| ERR_INVALID_RESPONSE)?
}

async fn parse_snapshot(body: Vec<u8>) -> Result<Box<SyncSnapshot>, &'static str> {
    tokio::task::spawn_blocking(move || parse_sync_bytes(&body).map(Box::new))
        .await
        .map_err(|_| ERR_INVALID_RESPONSE)?
}

async fn resolve_private_targets(host: &str, port: u16) -> Result<Vec<SocketAddr>, &'static str> {
    if let Ok(ip) = host.parse() {
        if !is_private_or_mesh_ip(ip) {
            return Err(ERR_PRIVATE_ONLY);
        }
        return Ok(vec![SocketAddr::new(ip, port)]);
    }
    let resolved: Vec<SocketAddr> =
        tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::lookup_host((host, port)))
            .await
            .map_err(|_| ERR_UNREACHABLE)?
            .map_err(|_| ERR_UNREACHABLE)?
            .collect();
    if resolved.is_empty() {
        return Err(ERR_UNREACHABLE);
    }
    let private: Vec<SocketAddr> = resolved
        .into_iter()
        .filter(|addr| is_private_or_mesh_ip(addr.ip()))
        .collect();
    if private.is_empty() {
        return Err(ERR_PRIVATE_ONLY);
    }
    Ok(private)
}

async fn connect_any(addrs: &[SocketAddr]) -> Result<TcpStream, &'static str> {
    tokio::time::timeout(CONNECT_TIMEOUT, async {
        for &addr in addrs {
            if let Ok(stream) = TcpStream::connect(addr).await {
                return Ok(stream);
            }
        }
        Err(ERR_UNREACHABLE)
    })
    .await
    .map_err(|_| ERR_UNREACHABLE)?
}

fn latest_library(saves: &watch::Sender<Option<Library>>) -> Library {
    saves.borrow().clone().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DEARROW_LOOKUPS, THUMB_DISK_READS};
    use std::sync::mpsc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use youtfast_core::model::Video;
    use youtfast_storage::paths::Dirs;
    use youtfast_storage::sync::{ERR_KEY_REJECTED, generate_pairing_key};
    use youtfast_youtube::innertube;

    fn test_context(library: Library) -> (Context, mpsc::Receiver<(u64, Event)>) {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join("target")
            .join(format!("backend-sync-test-{}", std::process::id()));
        let dirs = Dirs {
            data: root.join("data"),
            config: root.join("config"),
            cache: root.join("cache"),
        };
        let (outgoing, events) = mpsc::channel();
        let ctx = Context {
            rp: innertube::client().unwrap(),
            dirs,
            events: outgoing,
            epoch: 0,
            waker: egui::Context::default(),
            alerts_timer: Arc::default(),
            update_timer: Arc::default(),
            library_saves: Arc::new(watch::channel(Some(library)).0),
            settings_saves: Arc::new(watch::channel(None).0),
            account_writes: Arc::default(),
            loads: Arc::default(),
            rss: Arc::default(),
            thumb_pending: Arc::default(),
            thumb_sem: Arc::new(tokio::sync::Semaphore::new(
                youtfast_youtube::thumbs::DOWNLOADS,
            )),
            thumb_disk_sem: Arc::new(tokio::sync::Semaphore::new(THUMB_DISK_READS)),
            dearrow_sem: Arc::new(tokio::sync::Semaphore::new(DEARROW_LOOKUPS)),
            sync: SyncManager::default(),
        };
        (ctx, events)
    }

    #[tokio::test]
    async fn listener_and_client_exchange_libraries_only_with_matching_keys() {
        let mut server_lib = Library::default();
        server_lib.record_watch(Video::from_id("server00001".into()), 200);
        // Large enough to exercise multiple encrypted records.
        for n in 0..300 {
            let mut video = Video::from_id(format!("{n:011}"));
            video.title = "Synthetic title ".repeat(10);
            server_lib.record_watch(video, i64::from(n));
        }
        let (ctx, events) = test_context(server_lib);
        let tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = tcp.local_addr().unwrap().port();
        let key = generate_pairing_key();
        ctx.sync.active().settings = DeviceSync {
            enabled: true,
            port,
            pairing_key: key.clone(),
            ..DeviceSync::default()
        };
        let (shutdown, receiver) = oneshot::channel();
        let server = tokio::spawn(serve_loop(tcp, port, ctx.clone(), receiver));
        let mut client_lib = Library::default();
        client_lib.record_watch(Video::from_id("client00001".into()), 100);
        let wrong = DeviceSync {
            enabled: true,
            port: 39999,
            peer_address: format!("127.0.0.1:{port}"),
            pairing_key: generate_pairing_key(),
            ..DeviceSync::default()
        };
        assert_eq!(
            sync_with_address(&wrong, client_lib.clone())
                .await
                .unwrap_err(),
            ERR_KEY_REJECTED
        );
        assert!(events.try_recv().is_err());

        let settings = DeviceSync {
            pairing_key: key,
            ..wrong
        };
        let reply = sync_with_address(&settings, client_lib.clone())
            .await
            .unwrap();
        client_lib.merge_sync(*reply);
        assert!(client_lib.watched("server00001"));
        assert!(client_lib.watched("client00001"));
        assert_eq!(client_lib.history.len(), 302);
        let (_, event) = events.try_recv().unwrap();
        let Event::DeviceSyncIncoming {
            snapshot,
            generation: 0,
        } = event
        else {
            panic!("expected an authenticated library snapshot");
        };
        assert!(snapshot.history.iter().any(|w| w.video.id == "client00001"));
        assert_eq!(
            ctx.sync.active().peers,
            vec!["127.0.0.1:39999".parse().unwrap()]
        );
        let _ = shutdown.send(());
        server.await.unwrap();
    }

    async fn listening_device(key: &str, video: &str) -> (SocketAddr, oneshot::Sender<()>) {
        let mut library = Library::default();
        library.record_watch(Video::from_id(video.into()), 100);
        let (ctx, _events) = test_context(library);
        let tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = tcp.local_addr().unwrap();
        ctx.sync.active().settings = DeviceSync {
            enabled: true,
            port: addr.port(),
            pairing_key: key.to_owned(),
            ..DeviceSync::default()
        };
        let (shutdown, receiver) = oneshot::channel();
        tokio::spawn(serve_loop(tcp, addr.port(), ctx, receiver));
        (addr, shutdown)
    }

    #[tokio::test]
    async fn found_peers_sync_only_with_devices_sharing_the_key() {
        let key = generate_pairing_key();
        let (paired, _stop_paired) = listening_device(&key, "paired00001").await;
        let (stranger, _stop_stranger) =
            listening_device(&generate_pairing_key(), "strange0001").await;
        let closed = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap();
        let settings = DeviceSync {
            enabled: true,
            pairing_key: key,
            ..DeviceSync::default()
        };

        let (result, found) = sync_with_found_peers(
            &settings,
            vec![stranger, closed, paired],
            false,
            Library::default(),
        )
        .await;
        let snapshots = result.unwrap();
        assert_eq!(snapshots.len(), 1);
        assert!(
            snapshots[0]
                .history
                .iter()
                .any(|w| w.video.id == "paired00001")
        );
        let found = found.unwrap();
        assert_eq!(found.reached, vec![paired]);
        assert!(!found.searched);

        // Nothing known and the last search still fresh: no device is contacted.
        let (result, found) =
            sync_with_found_peers(&settings, Vec::new(), false, Library::default()).await;
        assert_eq!(result.unwrap_err(), ERR_NO_PEERS_FOUND);
        let found = found.unwrap();
        assert!(found.reached.is_empty() && !found.searched);
    }

    #[tokio::test]
    async fn an_unverified_listener_receives_only_the_empty_handshake() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer_address = listener.local_addr().unwrap().to_string();
        let unverified = async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut hello = [0; 48];
            stream.read_exact(&mut hello).await.unwrap();
            stream.write_all(&[0; 48]).await.unwrap();
            let mut remainder = Vec::new();
            stream.read_to_end(&mut remainder).await.unwrap();
            assert!(remainder.is_empty());
        };
        let mut library = Library::default();
        library.record_watch(Video::from_id("video000001".into()), 100);
        let settings = DeviceSync {
            peer_address,
            pairing_key: generate_pairing_key(),
            ..DeviceSync::default()
        };
        let ((), result) = tokio::join!(unverified, sync_with_address(&settings, library));
        assert_eq!(result.unwrap_err(), ERR_KEY_REJECTED);
    }

    #[tokio::test]
    async fn disabling_sync_cancels_an_outbound_handshake_without_an_event() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (ctx, events) = test_context(Library::default());
        ctx.sync.active().settings = DeviceSync {
            enabled: true,
            pairing_key: generate_pairing_key(),
            peer_address: listener.local_addr().unwrap().to_string(),
            ..DeviceSync::default()
        };
        let client = tokio::spawn({
            let ctx = ctx.clone();
            async move { sync_now(&ctx, true, 0).await }
        });
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut hello = [0; 48];
        socket.read_exact(&mut hello).await.unwrap();
        configure(&ctx, DeviceSync::default(), 1).await;
        tokio::time::timeout(Duration::from_secs(1), client)
            .await
            .unwrap()
            .unwrap();
        assert!(events.try_recv().is_err());
        assert_eq!(socket.read(&mut [0]).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn rotating_the_key_closes_authenticated_sessions_from_the_previous_generation() {
        let (ctx, events) = test_context(Library::default());
        let tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = tcp.local_addr().unwrap();
        let settings = DeviceSync {
            enabled: true,
            port: addr.port(),
            pairing_key: generate_pairing_key(),
            ..DeviceSync::default()
        };
        ctx.sync.active().settings = settings.clone();
        let (shutdown, receiver) = oneshot::channel();
        let task = tokio::spawn(serve_loop(tcp, addr.port(), ctx.clone(), receiver));
        *ctx.sync.listener.lock().await = Some(RunningListener {
            port: addr.port(),
            shutdown,
            task,
        });
        let socket = TcpStream::connect(addr).await.unwrap();
        let key = normalize_pairing_key(&settings.pairing_key).unwrap();
        let mut peer = SecurePeer::handshake(socket, &key, true).await.unwrap();

        let rotated = DeviceSync {
            pairing_key: generate_pairing_key(),
            ..settings
        };
        configure(&ctx, rotated, 1).await;
        let result = tokio::time::timeout(Duration::from_secs(1), peer.receive()).await;
        assert!(matches!(result, Ok(Err(ERR_UNREACHABLE))));
        assert!(events.try_recv().is_err());
        configure(&ctx, DeviceSync::default(), 2).await;
    }

    #[tokio::test]
    async fn public_target_is_rejected_before_connecting() {
        let settings = DeviceSync {
            enabled: true,
            peer_address: "8.8.8.8:38452".into(),
            pairing_key: generate_pairing_key(),
            ..DeviceSync::default()
        };
        assert_eq!(
            sync_with_address(&settings, Library::default())
                .await
                .unwrap_err(),
            ERR_PRIVATE_ONLY
        );
    }
}
