//! Direct device library sync over Tailscale, NetBird, and private LANs.
//!
//! Without an address, a round syncs with the devices it already knows, the
//! devices they share, and, when searching, the Tailscale and NetBird peers
//! and the devices answering on the local network.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{Mutex, Semaphore, oneshot, watch};
use tokio::task::JoinSet;

use crate::{Context, Event};
use youtfast_storage::library::Library;
use youtfast_storage::settings::DeviceSync;
use youtfast_storage::sync::{
    ERR_INCOMPATIBLE_VERSION, ERR_INVALID_RESPONSE, ERR_LISTENER_FAILED, ERR_NO_PEERS_FOUND,
    ERR_PAIRING_KEY_INVALID, ERR_PRIVATE_ONLY, ERR_SIZE_LIMIT, ERR_UNREACHABLE, MIN_SYNC_PORT,
    PAIRING_KEY_BYTES, SyncSnapshot, apply_mesh_identities, is_private_or_mesh_ip,
    local_sync_addresses, local_sync_endpoints, normalize_pairing_key, parse_peer_target,
    parse_sync_bytes, to_sync_json_bytes,
};

mod discovery;
mod lan;
mod transport;
use transport::{PacketKind, SecurePeer};

const MAX_CONCURRENT_INCOMING: usize = 4;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const IO_TIMEOUT: Duration = Duration::from_secs(10);
// Text fields reconfigure sync on every edit; only probe after they settle.
const ADDRESS_REFRESH_DEBOUNCE: Duration = Duration::from_millis(500);
/// How long a search of Tailscale, NetBird and the local network is reused
/// by automatic syncs.
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
    /// Devices other devices shared, tried by the next round.
    suggested: Vec<SocketAddr>,
    /// Devices that connected in since the current round started.
    inbound: Vec<SocketAddr>,
    /// Shared devices that did not answer, and when; not suggested again
    /// until [`DISCOVERY_TTL`] passes.
    failed: Vec<(SocketAddr, Instant)>,
    /// When Tailscale, NetBird and the local network were last searched.
    searched_at: Option<Instant>,
}

impl ActiveConfig {
    fn remember(&mut self, peer: SocketAddr) {
        if !self.peers.contains(&peer) && self.peers.len() < MAX_PEERS {
            self.peers.push(peer);
        }
    }

    /// Queues shared devices that are new to this one; true if any was.
    fn suggest(&mut self, shared: &[SocketAddr], own: &[SocketAddr]) -> bool {
        self.failed.retain(|(_, at)| at.elapsed() < DISCOVERY_TTL);
        let mut added = false;
        for peer in shared {
            if self.suggested.len() >= MAX_PEERS {
                break;
            }
            let failed = self.failed.iter().any(|(addr, _)| addr == peer);
            if !failed
                && !own.contains(peer)
                && !self.peers.contains(peer)
                && !self.suggested.contains(peer)
            {
                self.suggested.push(*peer);
                added = true;
            }
        }
        added
    }

    /// Devices to share with a peer: none when the user chose one address.
    fn shared(&self) -> Vec<SocketAddr> {
        if self.settings.peer_address.trim().is_empty() {
            self.peers.clone()
        } else {
            Vec::new()
        }
    }
}

/// Aborts a task when dropped.
struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// This computer's own sync endpoints, which shared lists must not point to.
fn own_endpoints(port: u16) -> Vec<SocketAddr> {
    local_sync_addresses()
        .into_iter()
        .map(|ip| SocketAddr::new(ip, port))
        .collect()
}

async fn refresh_local_addresses(context: Context, generation: u64, delay: Duration) {
    let work = async {
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        let (local, (tailscale, netbird)) = tokio::join!(
            tokio::task::spawn_blocking(local_sync_endpoints),
            discovery::local_mesh_ips(),
        );
        let mut addresses = local.unwrap_or_default();
        apply_mesh_identities(&mut addresses, &tailscale, &netbird);
        addresses
    };
    if let Some(addresses) = context.sync.while_current(generation, work).await {
        context.send(Event::DeviceSyncAddresses {
            generation,
            addresses,
        });
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
            active.suggested.clear();
            active.inbound.clear();
            active.failed.clear();
            active.searched_at = None;
        }
        active.settings = next.clone();
        active.generation = generation;
        context.sync.generation.send_replace(generation);
    }

    if next.enabled {
        tokio::spawn(refresh_local_addresses(
            context.clone(),
            generation,
            ADDRESS_REFRESH_DEBOUNCE,
        ));
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
            // Without the datagram socket the local network can't find this
            // device, but known and shared devices still sync.
            let udp = UdpSocket::bind(("0.0.0.0", next.port)).await.ok();
            if udp.is_none() {
                log::warn!("device sync announcements are unavailable");
            }
            let (shutdown, receiver) = oneshot::channel();
            let task = tokio::spawn(serve_loop(tcp, udp, next.port, context.clone(), receiver));
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
    udp: Option<UdpSocket>,
    port: u16,
    context: Context,
    mut shutdown: oneshot::Receiver<()>,
) {
    // Queries are answered on their own task, so datagrams never delay
    // accepting connections; it stops with this loop.
    let _answers = udp.map(|udp| {
        let ctx = context.clone();
        AbortOnDrop(
            tokio::spawn(lan::serve(udp, port, move || current_key(&ctx, port))).abort_handle(),
        )
    });
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
        let (generation, key, shared, sharing) = {
            let active = context.sync.active();
            if !active.settings.enabled || active.settings.port != port {
                continue;
            }
            let Some(key) = normalize_pairing_key(&active.settings.pairing_key) else {
                continue;
            };
            let sharing = active.settings.peer_address.trim().is_empty();
            (active.generation, key, active.shared(), sharing)
        };
        let Ok(permit) = context.sync.incoming_sem.clone().try_acquire_owned() else {
            continue;
        };
        let ctx = context.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let work = tokio::time::timeout(
                IO_TIMEOUT,
                exchange_incoming(stream, &key, port, shared, &ctx),
            );
            if let Some(Ok(Ok(incoming))) = ctx.sync.while_current(generation, work).await {
                let own = own_endpoints(port);
                let mut active = ctx.sync.active();
                if active.generation != generation {
                    return;
                }
                if incoming.listen_port >= MIN_SYNC_PORT {
                    let peer = SocketAddr::new(peer_addr.ip(), incoming.listen_port);
                    active.remember(peer);
                    if !active.inbound.contains(&peer) && active.inbound.len() < MAX_PEERS {
                        active.inbound.push(peer);
                    }
                }
                // With one chosen address, shared devices are not contacted.
                let learned = sharing && active.suggest(&incoming.peers, &own);
                drop(active);
                ctx.send(Event::DeviceSyncIncoming {
                    generation,
                    snapshot: incoming,
                });
                // The device shared others this one has not met: sync with them.
                if learned {
                    ctx.send(Event::CheckDeviceSync { generation });
                }
            }
        });
    }
}

fn current_key(context: &Context, port: u16) -> Option<[u8; PAIRING_KEY_BYTES]> {
    let active = context.sync.active();
    if !active.settings.enabled || active.settings.port != port {
        return None;
    }
    normalize_pairing_key(&active.settings.pairing_key)
}

async fn exchange_incoming(
    stream: TcpStream,
    key: &[u8; PAIRING_KEY_BYTES],
    port: u16,
    shared: Vec<SocketAddr>,
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
    let bytes = match serialize_snapshot(library, port, shared).await {
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
    let (settings, known, suggested, searched_at) = {
        let mut active = context.sync.active();
        if !active.settings.enabled || active.generation != generation {
            return;
        }
        active.inbound.clear();
        (
            active.settings.clone(),
            active.peers.clone(),
            active.suggested.clone(),
            active.searched_at,
        )
    };
    if manual {
        tokio::spawn(refresh_local_addresses(
            context.clone(),
            generation,
            Duration::ZERO,
        ));
    }
    let library = latest_library(&context.library_saves);
    let work = async {
        if settings.peer_address.trim().is_empty() {
            let search = manual || searched_at.is_none_or(|at| at.elapsed() >= DISCOVERY_TTL);
            let round = Round {
                known,
                suggested,
                search,
                own: own_endpoints(settings.port),
            };
            sync_with_found_peers(&settings, round, library).await
        } else {
            let result = sync_with_address(&settings, library, Vec::new()).await;
            (result.map(|snapshot| vec![*snapshot]), None)
        }
    };
    let Some((result, found)) = context.sync.while_current(generation, work).await else {
        return;
    };
    let mut retry = false;
    if let Some(found) = found {
        let mut active = context.sync.active();
        if active.generation != generation {
            return;
        }
        if found.searched {
            active.searched_at = Some(Instant::now());
        }
        // The devices reached, plus those that connected in while the round ran.
        let inbound = std::mem::take(&mut active.inbound);
        active.peers.clear();
        for peer in found.reached.iter().chain(&inbound) {
            active.remember(*peer);
        }
        let now = Instant::now();
        for addr in &found.tried {
            if !active.peers.contains(addr) && !active.failed.iter().any(|(a, _)| a == addr) {
                active.failed.push((*addr, now));
            }
        }
        let excess = active.failed.len().saturating_sub(4 * MAX_PEERS);
        active.failed.drain(..excess);
        let peers = active.peers.clone();
        active
            .suggested
            .retain(|peer| !found.tried.contains(peer) && !peers.contains(peer));
        // Devices shared while the round ran get their own round.
        retry = !active.suggested.is_empty();
    }
    if let Err(error) = &result {
        log::warn!("device sync failed: {error}");
    }
    context.send(Event::DeviceSyncCompleted {
        generation,
        result,
        manual,
    });
    if retry {
        context.send(Event::CheckDeviceSync { generation });
    }
}

/// What a round without an address starts from.
struct Round {
    /// Devices that completed an exchange before.
    known: Vec<SocketAddr>,
    /// Devices other devices shared.
    suggested: Vec<SocketAddr>,
    /// Whether to search Tailscale, NetBird and the local network.
    search: bool,
    /// This computer's own endpoints.
    own: Vec<SocketAddr>,
}

/// The peers a round reached, every address it tried, and whether it searched.
struct Found {
    reached: Vec<SocketAddr>,
    tried: Vec<SocketAddr>,
    searched: bool,
}

/// Syncs with the known and shared devices; when `search` is set or none of
/// them answers, also with every device found on Tailscale, NetBird and the
/// local network; then once more with the devices the reached ones shared.
async fn sync_with_found_peers(
    settings: &DeviceSync,
    round: Round,
    library: Library,
) -> (Result<Vec<SyncSnapshot>, String>, Option<Found>) {
    let Some(key) = normalize_pairing_key(&settings.pairing_key) else {
        return (Err(ERR_PAIRING_KEY_INVALID.to_owned()), None);
    };
    let request = match serialize_snapshot(library, settings.port, round.known.clone()).await {
        Ok(bytes) => Arc::new(bytes),
        Err(error) => return (Err(error.to_owned()), None),
    };

    let mut first: Vec<SocketAddr> = round.known.clone();
    for peer in &round.suggested {
        if !first.contains(peer) {
            first.push(*peer);
        }
    }
    let mut replies = if round.search {
        Vec::new()
    } else {
        exchange_all(first.clone(), key, &request).await
    };
    // Devices that all stopped answering may have changed address.
    let searched =
        round.search || (!first.is_empty() && !replies.iter().any(|(_, reply)| reply.is_ok()));
    if searched {
        let mut candidates = search_candidates(&first, settings.port, &key, &round.own).await;
        candidates.retain(|addr| !replies.iter().any(|(tried, _)| tried == addr));
        replies.extend(exchange_all(candidates, key, &request).await);
    }
    // Devices the reached ones shared, so every paired device is met through any one.
    let mut shared = Vec::new();
    for (_, reply) in &replies {
        for peer in reply
            .as_ref()
            .map(|snapshot| &snapshot.peers)
            .into_iter()
            .flatten()
        {
            let new = !round.own.contains(peer)
                && !shared.contains(peer)
                && !replies.iter().any(|(tried, _)| tried == peer);
            if new && shared.len() < MAX_PEERS {
                shared.push(*peer);
            }
        }
    }
    replies.extend(exchange_all(shared, key, &request).await);

    let tried = replies.iter().map(|(addr, _)| *addr).collect();
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
        tried,
        searched,
    };
    (result, Some(found))
}

/// Known devices first, then every Tailscale and NetBird peer on the listen
/// port and every device answering on the local network, without this
/// computer's own endpoints.
async fn search_candidates(
    known: &[SocketAddr],
    port: u16,
    key: &[u8; PAIRING_KEY_BYTES],
    own: &[SocketAddr],
) -> Vec<SocketAddr> {
    let (mesh, local) = tokio::join!(discovery::mesh_peers(), lan::search(port, key));
    let mut candidates = known.to_vec();
    let found = mesh
        .into_iter()
        .map(|ip| SocketAddr::new(ip, port))
        .chain(local);
    for addr in found {
        if !own.contains(&addr) && !candidates.contains(&addr) {
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
    shared: Vec<SocketAddr>,
) -> Result<Box<SyncSnapshot>, String> {
    let key = normalize_pairing_key(&settings.pairing_key).ok_or(ERR_PAIRING_KEY_INVALID)?;
    let (host, port) = parse_peer_target(&settings.peer_address, settings.port)?;
    let addrs = resolve_private_targets(&host, port).await?;
    let request = serialize_snapshot(library, settings.port, shared).await?;
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

async fn serialize_snapshot(
    library: Library,
    port: u16,
    shared: Vec<SocketAddr>,
) -> Result<Vec<u8>, &'static str> {
    tokio::task::spawn_blocking(move || {
        let mut snapshot = library.sync_snapshot(port);
        snapshot.peers = shared;
        to_sync_json_bytes(&snapshot.sanitized())
    })
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
        let server = tokio::spawn(serve_loop(tcp, None, port, ctx.clone(), receiver));
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
            sync_with_address(&wrong, client_lib.clone(), Vec::new())
                .await
                .unwrap_err(),
            ERR_KEY_REJECTED
        );
        assert!(events.try_recv().is_err());

        let settings = DeviceSync {
            pairing_key: key,
            ..wrong
        };
        let reply = sync_with_address(&settings, client_lib.clone(), Vec::new())
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

    struct Device {
        addr: SocketAddr,
        ctx: Context,
        _stop: oneshot::Sender<()>,
    }

    /// A listening device on loopback, answering announcements on its port too.
    async fn listening_device(key: &str, video: &str) -> Device {
        let mut library = Library::default();
        library.record_watch(Video::from_id(video.into()), 100);
        let (ctx, _events) = test_context(library);
        let tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = tcp.local_addr().unwrap();
        let udp = UdpSocket::bind(addr).await.unwrap();
        ctx.sync.active().settings = DeviceSync {
            enabled: true,
            port: addr.port(),
            pairing_key: key.to_owned(),
            ..DeviceSync::default()
        };
        let (stop, receiver) = oneshot::channel();
        tokio::spawn(serve_loop(
            tcp,
            Some(udp),
            addr.port(),
            ctx.clone(),
            receiver,
        ));
        Device {
            addr,
            ctx,
            _stop: stop,
        }
    }

    fn round(known: Vec<SocketAddr>) -> Round {
        Round {
            known,
            suggested: Vec::new(),
            search: false,
            own: Vec::new(),
        }
    }

    #[tokio::test]
    async fn found_peers_sync_only_with_devices_sharing_the_key() {
        let key = generate_pairing_key();
        let paired = listening_device(&key, "paired00001").await;
        let stranger = listening_device(&generate_pairing_key(), "strange0001").await;
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

        let known = vec![stranger.addr, closed, paired.addr];
        let (result, found) =
            sync_with_found_peers(&settings, round(known), Library::default()).await;
        let snapshots = result.unwrap();
        assert_eq!(snapshots.len(), 1);
        assert!(
            snapshots[0]
                .history
                .iter()
                .any(|w| w.video.id == "paired00001")
        );
        let found = found.unwrap();
        assert_eq!(found.reached, vec![paired.addr]);
        assert!(!found.searched);

        // Nothing known and the last search still fresh: no device is contacted.
        let (result, found) =
            sync_with_found_peers(&settings, round(Vec::new()), Library::default()).await;
        assert_eq!(result.unwrap_err(), ERR_NO_PEERS_FOUND);
        let found = found.unwrap();
        assert!(found.reached.is_empty() && !found.searched);
    }

    #[tokio::test]
    async fn a_round_meets_the_devices_a_reached_device_shares() {
        let key = generate_pairing_key();
        let hub = listening_device(&key, "hubvideo001").await;
        let far = listening_device(&key, "farvideo001").await;
        let stranger = listening_device(&generate_pairing_key(), "strange0001").await;
        // The hub met `far` before; it also lists a device with another key.
        hub.ctx.sync.active().peers = vec![far.addr, stranger.addr];
        let settings = DeviceSync {
            enabled: true,
            pairing_key: key,
            ..DeviceSync::default()
        };

        let (result, found) =
            sync_with_found_peers(&settings, round(vec![hub.addr]), Library::default()).await;
        let videos: Vec<String> = result
            .unwrap()
            .iter()
            .flat_map(|snapshot| snapshot.history.iter().map(|w| w.video.id.clone()))
            .collect();
        assert!(videos.contains(&"hubvideo001".to_owned()));
        assert!(videos.contains(&"farvideo001".to_owned()));
        assert!(!videos.contains(&"strange0001".to_owned()));
        let mut reached = found.unwrap().reached;
        reached.sort();
        let mut expected = vec![hub.addr, far.addr];
        expected.sort();
        assert_eq!(reached, expected);
    }

    #[tokio::test]
    async fn an_inbound_exchange_queues_the_devices_the_peer_shares() {
        let key = generate_pairing_key();
        let device = listening_device(&key, "device00001").await;
        let other: SocketAddr = "127.0.0.1:40001".parse().unwrap();
        let settings = DeviceSync {
            enabled: true,
            port: 40000,
            peer_address: device.addr.to_string(),
            pairing_key: key,
            ..DeviceSync::default()
        };
        sync_with_address(&settings, Library::default(), vec![other])
            .await
            .unwrap();
        // The listener records the exchange just after replying.
        for _ in 0..100 {
            if !device.ctx.sync.active().suggested.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let active = device.ctx.sync.active();
        assert_eq!(active.peers, vec!["127.0.0.1:40000".parse().unwrap()]);
        assert_eq!(active.suggested, vec![other]);
    }

    #[tokio::test]
    async fn a_device_with_a_chosen_address_neither_shares_nor_queues_devices() {
        let key = generate_pairing_key();
        let device = listening_device(&key, "device00001").await;
        {
            let mut active = device.ctx.sync.active();
            active.settings.peer_address = "127.0.0.1:40002".into();
            active.peers = vec!["127.0.0.1:40003".parse().unwrap()];
        }
        let settings = DeviceSync {
            enabled: true,
            port: 40000,
            peer_address: device.addr.to_string(),
            pairing_key: key,
            ..DeviceSync::default()
        };
        let reply = sync_with_address(
            &settings,
            Library::default(),
            vec!["127.0.0.1:40001".parse().unwrap()],
        )
        .await
        .unwrap();
        assert!(reply.peers.is_empty());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(device.ctx.sync.active().suggested.is_empty());
    }

    #[tokio::test]
    async fn local_network_queries_find_only_devices_sharing_the_key() {
        let key = generate_pairing_key();
        let paired = listening_device(&key, "paired00001").await;
        let stranger = listening_device(&generate_pairing_key(), "strange0001").await;
        let normalized = normalize_pairing_key(&key).unwrap();
        let found = lan::query(&[paired.addr, stranger.addr], 40000, &normalized).await;
        assert_eq!(found, vec![paired.addr]);
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
        let ((), result) = tokio::join!(
            unverified,
            sync_with_address(&settings, library, Vec::new())
        );
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
        let task = tokio::spawn(serve_loop(tcp, None, addr.port(), ctx.clone(), receiver));
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
            sync_with_address(&settings, Library::default(), Vec::new())
                .await
                .unwrap_err(),
            ERR_PRIVATE_ONLY
        );
    }
}
