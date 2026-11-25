//! Direct device-to-device library sync over Tailscale, NetBird, and local
//! networks. Keeps per-item timestamps and deletion tombstones so merges across
//! computers are commutative, idempotent, and never expose YouTube account
//! credentials or avatar URLs.

use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::backup::{SUBSCRIPTIONS_LIMIT, WATCH_LATER_LIMIT};
use crate::library::{
    BLOCKED_CHANNELS_LIMIT, DevicePlaylist, HIDDEN_VIDEOS_LIMIT, HISTORY_LIMIT,
    PLAYLIST_VIDEOS_LIMIT, PLAYLISTS_LIMIT, Watched,
};
use crate::takeout::{Limited, read_bom_prefix};
use youtfast_core::model::{Channel, Video};

pub const SYNC_VERSION: u32 = 2;
pub const DEFAULT_SYNC_PORT: u16 = 38452;
pub const MIN_SYNC_PORT: u16 = 1024;
pub const PAIRING_KEY_BYTES: usize = 32;
pub const MAX_PAIRING_KEY_CHARS: usize = 128;
pub const MAX_PEER_ADDRESS_CHARS: usize = 253;
pub const TOMBSTONES_LIMIT: usize = 2_000;
pub const MAX_SYNC_BYTES: u64 = 64 * 1024 * 1024;
/// Most device addresses one snapshot shares with a peer.
pub const MAX_SHARED_PEERS: usize = 32;

pub const ERR_PAIRING_KEY_INVALID: &str = "Generate a new pairing key and copy it to both devices.";
pub const ERR_PEER_MISSING: &str =
    "Enter the other device's Tailscale, NetBird, or local network address.";
pub const ERR_PRIVATE_ONLY: &str =
    "Device sync only connects to private network, Tailscale, or NetBird addresses.";
pub const ERR_UNREACHABLE: &str = "Could not reach the other device. Check that YouTFast is open with Device sync enabled and that both devices are on the same Tailscale, NetBird, or local network.";
pub const ERR_KEY_REJECTED: &str =
    "The other device rejected the pairing key. Check that both devices use the same key.";
pub const ERR_INCOMPATIBLE_VERSION: &str =
    "The other device uses an incompatible sync version. Update YouTFast on both devices.";
pub const ERR_SIZE_LIMIT: &str = "The library exceeds the 64 MiB sync limit.";
pub const ERR_INVALID_RESPONSE: &str = "Could not read the sync response from the other device.";
pub const ERR_LISTENER_FAILED: &str =
    "Could not start the device sync listener on the selected port.";
pub const ERR_NO_PEERS_FOUND: &str = "No other device with this pairing key was found. Check that YouTFast is open with Device sync and the same key on your other computer, or enter its address.";

/// Generates a random 256-bit key in readable groups of four bytes.
pub fn generate_pairing_key() -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(71);
    for (i, byte) in rand::random::<[u8; PAIRING_KEY_BYTES]>()
        .into_iter()
        .enumerate()
    {
        if i != 0 && i % 4 == 0 {
            out.push('-');
        }
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 15)]));
    }
    out
}

/// Decodes a generated key, ignoring whitespace, hyphens, and ASCII case.
pub fn normalize_pairing_key(key: &str) -> Option<[u8; PAIRING_KEY_BYTES]> {
    let mut digits = key.chars().filter(|c| !c.is_whitespace() && *c != '-');
    let mut bytes = [0; PAIRING_KEY_BYTES];
    for byte in &mut bytes {
        let high = digits.next()?.to_digit(16)? as u8;
        let low = digits.next()?.to_digit(16)? as u8;
        *byte = high << 4 | low;
    }
    digits.next().is_none().then_some(bytes)
}

/// Whether `ip` lies in `100.64.0.0/10` (CGNAT range used by Tailscale and NetBird).
pub fn is_cgnat_mesh_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 100 && (octets[1] & 0b1100_0000) == 64
}

/// Whether `ip` belongs to a loopback, private LAN, link-local, ULA, or
/// Tailscale/NetBird mesh address range.
pub fn is_private_or_mesh_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || is_cgnat_mesh_v4(v4)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                is_private_or_mesh_ip(IpAddr::V4(v4))
            } else {
                v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local()
            }
        }
    }
}

/// Which network or VPN carries a local sync address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SyncNetwork {
    Tailscale,
    NetBird,
    WireGuard,
    ZeroTier,
    Vpn,
    LocalNetwork,
}

impl SyncNetwork {
    /// Display name for tooltips and accessibility labels.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Tailscale => "Tailscale",
            Self::NetBird => "NetBird",
            Self::WireGuard => "WireGuard",
            Self::ZeroTier => "ZeroTier",
            Self::Vpn => "VPN",
            Self::LocalNetwork => "Local network",
        }
    }
}

/// A reachable local address for device sync and its detected network.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LocalSyncAddress {
    pub ip: IpAddr,
    pub network: SyncNetwork,
}

impl LocalSyncAddress {
    /// Formats this address with `port` as `ip:port` (or `[ip]:port` for IPv6).
    #[must_use]
    pub fn endpoint(self, port: u16) -> String {
        SocketAddr::new(self.ip, port).to_string()
    }
}

/// Whether an interface name belongs to a local container or VM bridge that
/// other computers cannot reach directly.
#[must_use]
pub fn is_container_interface(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    if lower.starts_with("vethernet") {
        return lower.contains("wsl")
            || lower.contains("docker")
            || lower.contains("default switch");
    }
    // Docker's default bridge names are `br-` plus 12 hex digits. A LAN
    // bridge can also start with `br-` (for example, `br-lan`).
    if lower
        .strip_prefix("br-")
        .is_some_and(|id| id.len() == 12 && id.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return true;
    }
    const PREFIXES: &[&str] = &[
        "docker", "podman", "veth", "virbr", "lxc", "lxdbr", "incusbr", "cni", "flannel", "cali",
        "cilium", "kube-", "vboxnet", "vmnet",
    ];
    PREFIXES.iter().any(|prefix| lower.starts_with(prefix))
}

/// Classifies a local IPv4 interface by its adapter name, point-to-point flag,
/// and address range. On macOS, where VPN tunnels share `utunN` names,
/// Tailscale and NetBird are refined by [`apply_mesh_identities`].
#[must_use]
pub fn classify_sync_interface(name: &str, ip: Ipv4Addr, is_p2p: bool) -> SyncNetwork {
    let lower = name.trim().to_ascii_lowercase();
    if lower.contains("tailscale") {
        return SyncNetwork::Tailscale;
    }
    if lower.contains("netbird") || is_indexed_prefix(&lower, "wt") {
        return SyncNetwork::NetBird;
    }
    if lower.contains("wireguard")
        || lower == "wg"
        || lower
            .strip_prefix("wg")
            .and_then(|rest| rest.as_bytes().first())
            .is_some_and(|b| b.is_ascii_digit() || matches!(b, b'-' | b'_'))
    {
        return SyncNetwork::WireGuard;
    }
    if lower.contains("zerotier")
        || lower
            .strip_prefix("zt")
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_alphanumeric()))
    {
        return SyncNetwork::ZeroTier;
    }
    if lower.contains("openvpn")
        || lower.contains("mullvad")
        || lower.contains("proton")
        || lower.contains("nordlynx")
        || lower.contains("nebula")
        || lower.contains("headscale")
        || lower.contains("vpn")
        || is_indexed_prefix(&lower, "tun")
        || is_indexed_prefix(&lower, "tap")
        || is_indexed_prefix(&lower, "utun")
        || is_indexed_prefix(&lower, "ppp")
        || lower.starts_with("ipsec")
        || is_p2p
        || is_cgnat_mesh_v4(ip)
    {
        return SyncNetwork::Vpn;
    }
    SyncNetwork::LocalNetwork
}

fn is_indexed_prefix(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

fn sort_sync_addresses(addresses: &mut Vec<LocalSyncAddress>) {
    addresses.sort_by_key(|entry| (entry.ip, entry.network));
    addresses.dedup_by_key(|entry| entry.ip);
    addresses.sort_by_key(|entry| {
        let ip_rank = match entry.ip {
            IpAddr::V4(v4) => (
                !is_cgnat_mesh_v4(v4),
                !v4.octets().starts_with(&[192, 168]),
                v4.octets()[0] != 10,
            ),
            IpAddr::V6(_) => (true, true, true),
        };
        (entry.network, ip_rank, entry.ip)
    });
}

fn collect_sync_endpoints<'a>(
    interfaces: impl IntoIterator<Item = (&'a str, IpAddr, bool, bool)>,
) -> Vec<LocalSyncAddress> {
    let mut addresses: Vec<LocalSyncAddress> = interfaces
        .into_iter()
        .filter_map(|(name, ip, is_loopback, is_p2p)| match ip {
            IpAddr::V4(v4)
                if !is_loopback
                    && is_private_or_mesh_ip(IpAddr::V4(v4))
                    && !is_container_interface(name) =>
            {
                Some(LocalSyncAddress {
                    ip: IpAddr::V4(v4),
                    network: classify_sync_interface(name, v4, is_p2p),
                })
            }
            _ => None,
        })
        .collect();
    sort_sync_addresses(&mut addresses);
    addresses
}

/// Non-loopback local IPv4 addresses reachable over VPN or LAN, excluding
/// container/VM bridges, classified by detected network and sorted with VPNs
/// before RFC 1918 LAN addresses.
#[must_use]
pub fn local_sync_endpoints() -> Vec<LocalSyncAddress> {
    let interfaces = if_addrs::get_if_addrs().unwrap_or_default();
    collect_sync_endpoints(interfaces.iter().map(|interface| {
        (
            interface.name.as_str(),
            interface.ip(),
            interface.is_loopback(),
            interface.is_p2p(),
        )
    }))
}

/// Refines [`LocalSyncAddress`] entries using local IPs reported by
/// `tailscale status --json` and `netbird status --json`, then re-sorts them.
pub fn apply_mesh_identities(
    addresses: &mut Vec<LocalSyncAddress>,
    tailscale_ips: &[IpAddr],
    netbird_ips: &[IpAddr],
) {
    for entry in addresses.iter_mut() {
        if tailscale_ips.contains(&entry.ip) {
            entry.network = SyncNetwork::Tailscale;
        } else if netbird_ips.contains(&entry.ip) {
            entry.network = SyncNetwork::NetBird;
        }
    }
    sort_sync_addresses(addresses);
}

/// Non-loopback local IPv4 addresses reachable over Tailscale, NetBird, or LAN,
/// sorted with mesh addresses (`100.64.0.0/10`) first, then RFC 1918 LAN.
pub fn local_sync_addresses() -> Vec<IpAddr> {
    let mut addresses: Vec<Ipv4Addr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|interface| match interface.ip() {
            IpAddr::V4(ip) if !interface.is_loopback() && is_private_or_mesh_ip(IpAddr::V4(ip)) => {
                Some(ip)
            }
            _ => None,
        })
        .collect();
    addresses.sort_by_key(|ip| {
        (
            !is_cgnat_mesh_v4(*ip),
            !ip.octets().starts_with(&[192, 168]),
            ip.octets()[0] != 10,
            *ip,
        )
    });
    addresses.dedup();
    addresses.into_iter().map(IpAddr::V4).collect()
}

/// This computer's own addresses from `tailscale status --json`, keeping only
/// mesh and private addresses.
#[must_use]
pub fn tailscale_self_ips(json: &[u8]) -> Vec<IpAddr> {
    #[derive(Deserialize)]
    struct Status {
        #[serde(rename = "Self", default)]
        self_node: Option<SelfNode>,
    }
    #[derive(Deserialize)]
    struct SelfNode {
        #[serde(rename = "TailscaleIPs", default)]
        ips: Option<Vec<IpAddr>>,
    }
    let Ok(status) = serde_json::from_slice::<Status>(json) else {
        return Vec::new();
    };
    let mut found: Vec<IpAddr> = status
        .self_node
        .and_then(|node| node.ips)
        .unwrap_or_default()
        .into_iter()
        .filter(|ip| is_private_or_mesh_ip(*ip))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// This computer's own address from `netbird status --json`, keeping only mesh
/// and private addresses.
#[must_use]
pub fn netbird_self_ips(json: &[u8]) -> Vec<IpAddr> {
    #[derive(Deserialize)]
    struct Status {
        #[serde(rename = "netbirdIp", default)]
        ip: Option<String>,
    }
    let Ok(status) = serde_json::from_slice::<Status>(json) else {
        return Vec::new();
    };
    status
        .ip
        .as_deref()
        .and_then(|raw| raw.split('/').next())
        .and_then(|raw| raw.parse::<IpAddr>().ok())
        .filter(|ip| is_private_or_mesh_ip(*ip))
        .into_iter()
        .collect()
}

/// Online peers in `tailscale status --json`, one address each (IPv4 first),
/// keeping only mesh and private addresses. Unknown or missing fields yield
/// no peers rather than an error: discovery is best-effort.
pub fn tailscale_peer_ips(json: &[u8]) -> Vec<IpAddr> {
    #[derive(Deserialize)]
    struct Status {
        #[serde(rename = "Peer", default)]
        peer: Option<HashMap<String, Peer>>,
    }
    #[derive(Deserialize)]
    struct Peer {
        #[serde(rename = "TailscaleIPs", default)]
        ips: Option<Vec<IpAddr>>,
        #[serde(rename = "Online", default)]
        online: bool,
    }
    let Ok(status) = serde_json::from_slice::<Status>(json) else {
        return Vec::new();
    };
    let mut found: Vec<IpAddr> = status
        .peer
        .unwrap_or_default()
        .into_values()
        .filter(|peer| peer.online)
        .filter_map(|peer| {
            let ips: Vec<IpAddr> = peer
                .ips
                .unwrap_or_default()
                .into_iter()
                .filter(|ip| is_private_or_mesh_ip(*ip))
                .collect();
            ips.iter().find(|ip| ip.is_ipv4()).or(ips.first()).copied()
        })
        .collect();
    found.sort();
    found
}

/// Peers in `netbird status --json` with mesh or private addresses. Every
/// listed peer is kept: with lazy connections a reachable peer reads `Idle`.
pub fn netbird_peer_ips(json: &[u8]) -> Vec<IpAddr> {
    #[derive(Deserialize)]
    struct Status {
        #[serde(default)]
        peers: Option<Peers>,
    }
    #[derive(Deserialize)]
    struct Peers {
        #[serde(default)]
        details: Option<Vec<Peer>>,
    }
    #[derive(Deserialize)]
    struct Peer {
        #[serde(rename = "netbirdIp", default)]
        ip: String,
    }
    let Ok(status) = serde_json::from_slice::<Status>(json) else {
        return Vec::new();
    };
    let mut found: Vec<IpAddr> = status
        .peers
        .and_then(|peers| peers.details)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|peer| peer.ip.split('/').next()?.parse().ok())
        .filter(|ip| is_private_or_mesh_ip(*ip))
        .collect();
    found.sort();
    found
}

/// Size of a local-network announcement datagram.
pub const ANNOUNCE_BYTES: usize = 43;
const ANNOUNCE_MAGIC: &[u8; 8] = b"YTFSYNC1";
const ANNOUNCE_BODY: usize = 11 + 16;
const ANNOUNCE_TAG: usize = 16;
const QUERY: u8 = 1;
const REPLY: u8 = 2;

/// The random nonce of a local-network query; a reply is bound to it.
pub type AnnounceNonce = [u8; 16];

/// A query asking the local network which devices share the key, carrying
/// this device's listen port, and its nonce. The tag (HMAC-SHA-256 under the
/// key) proves the key without revealing it; the random nonce keeps queries
/// from identifying the key or the device.
pub fn announce_query(
    port: u16,
    key: &[u8; PAIRING_KEY_BYTES],
) -> ([u8; ANNOUNCE_BYTES], AnnounceNonce) {
    let nonce = rand::random();
    (seal(QUERY, port, &nonce, &[], key), nonce)
}

/// The answer to the query with `query` nonce, carrying this device's listen port.
pub fn announce_reply(
    port: u16,
    key: &[u8; PAIRING_KEY_BYTES],
    query: &AnnounceNonce,
) -> [u8; ANNOUNCE_BYTES] {
    seal(REPLY, port, &rand::random(), query, key)
}

/// The listen port and nonce of a query made with the same key.
pub fn read_announce_query(
    datagram: &[u8],
    key: &[u8; PAIRING_KEY_BYTES],
) -> Option<(u16, AnnounceNonce)> {
    let port = open(datagram, QUERY, &[], key)?;
    let mut nonce = [0; 16];
    nonce.copy_from_slice(&datagram[11..ANNOUNCE_BODY]);
    Some((port, nonce))
}

/// The listen port in a reply made with the same key to the `query` nonce.
pub fn read_announce_reply(
    datagram: &[u8],
    key: &[u8; PAIRING_KEY_BYTES],
    query: &AnnounceNonce,
) -> Option<u16> {
    open(datagram, REPLY, query, key)
}

fn seal(
    kind: u8,
    port: u16,
    nonce: &AnnounceNonce,
    bound: &[u8],
    key: &[u8; PAIRING_KEY_BYTES],
) -> [u8; ANNOUNCE_BYTES] {
    let mut out = [0; ANNOUNCE_BYTES];
    out[..8].copy_from_slice(ANNOUNCE_MAGIC);
    out[8] = kind;
    out[9..11].copy_from_slice(&port.to_be_bytes());
    out[11..ANNOUNCE_BODY].copy_from_slice(nonce);
    let tag = announce_mac(&out[..ANNOUNCE_BODY], bound, key)
        .finalize()
        .into_bytes();
    out[ANNOUNCE_BODY..].copy_from_slice(&tag[..ANNOUNCE_TAG]);
    out
}

fn open(datagram: &[u8], kind: u8, bound: &[u8], key: &[u8; PAIRING_KEY_BYTES]) -> Option<u16> {
    if datagram.len() != ANNOUNCE_BYTES || &datagram[..8] != ANNOUNCE_MAGIC || datagram[8] != kind {
        return None;
    }
    let (body, tag) = datagram.split_at(ANNOUNCE_BODY);
    announce_mac(body, bound, key)
        .verify_truncated_left(tag)
        .ok()?;
    let port = u16::from_be_bytes([datagram[9], datagram[10]]);
    (port >= MIN_SYNC_PORT).then_some(port)
}

fn announce_mac(body: &[u8], bound: &[u8], key: &[u8; PAIRING_KEY_BYTES]) -> Hmac<Sha256> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC takes any key size");
    mac.update(b"youtfast device sync announcement");
    mac.update(body);
    mac.update(bound);
    mac
}

/// Parses a peer address such as `100.64.0.2`, `100.64.0.2:38452`,
/// `desktop.tailnet.ts.net`, or `[fd7a:115c:a1e0::1]:38452`.
pub fn parse_peer_target(input: &str, default_port: u16) -> Result<(String, u16), &'static str> {
    if default_port == 0 {
        return Err(ERR_PEER_MISSING);
    }
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ERR_PEER_MISSING);
    }
    let without_scheme = if let Some((scheme, rest)) = trimmed.split_once("://") {
        if !scheme.eq_ignore_ascii_case("http") {
            return Err(ERR_PEER_MISSING);
        }
        rest
    } else {
        trimmed
    };
    if without_scheme.is_empty()
        || without_scheme
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control() || matches!(ch, '/' | '?' | '#' | '@'))
    {
        return Err(ERR_PEER_MISSING);
    }

    let (host, port) = if let Some(rest) = without_scheme.strip_prefix('[') {
        let Some((addr, after)) = rest.split_once(']') else {
            return Err(ERR_PEER_MISSING);
        };
        let Ok(v6) = addr.parse::<std::net::Ipv6Addr>() else {
            return Err(ERR_PEER_MISSING);
        };
        let port = if after.is_empty() {
            default_port
        } else if let Some(port_str) = after.strip_prefix(':') {
            parse_nonzero_port(port_str)?
        } else {
            return Err(ERR_PEER_MISSING);
        };
        (v6.to_string(), port)
    } else if let Ok(v6) = without_scheme.parse::<std::net::Ipv6Addr>() {
        (v6.to_string(), default_port)
    } else if let Some((host_part, port_str)) = without_scheme.rsplit_once(':') {
        if host_part.contains(':') {
            return Err(ERR_PEER_MISSING);
        }
        let port = parse_nonzero_port(port_str)?;
        (host_part.to_owned(), port)
    } else {
        (without_scheme.to_owned(), default_port)
    };

    if host.is_empty() || host.len() > MAX_PEER_ADDRESS_CHARS {
        return Err(ERR_PEER_MISSING);
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !is_private_or_mesh_ip(ip) {
            return Err(ERR_PRIVATE_ONLY);
        }
        return Ok((host, port));
    }
    if !host
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
        || host.starts_with('.')
        || host.starts_with('-')
    {
        return Err(ERR_PEER_MISSING);
    }
    Ok((host, port))
}

fn parse_nonzero_port(raw: &str) -> Result<u16, &'static str> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ERR_PEER_MISSING);
    }
    raw.parse::<u16>()
        .ok()
        .filter(|&port| port >= 1)
        .ok_or(ERR_PEER_MISSING)
}

/// Per-item modification timestamps and deletion tombstones kept in `library.json`
/// so cross-device merges handle removals, resume updates, and clears cleanly.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncMetadata {
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub position_at: HashMap<String, i64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub removed_history: HashMap<String, i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_cleared_at: Option<i64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub subscribed_at: HashMap<String, i64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub unsubscribed_at: HashMap<String, i64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub watch_later_at: HashMap<String, i64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub removed_watch_later: HashMap<String, i64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub playlists_at: HashMap<String, i64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub deleted_playlists: HashMap<String, i64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub hidden_at: HashMap<String, i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden_cleared_at: Option<i64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub blocked_at: HashMap<String, i64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub unblocked_at: HashMap<String, i64>,
}

impl SyncMetadata {
    pub fn is_empty(&self) -> bool {
        self.position_at.is_empty()
            && self.removed_history.is_empty()
            && self.history_cleared_at.is_none()
            && self.subscribed_at.is_empty()
            && self.unsubscribed_at.is_empty()
            && self.watch_later_at.is_empty()
            && self.removed_watch_later.is_empty()
            && self.playlists_at.is_empty()
            && self.deleted_playlists.is_empty()
            && self.hidden_at.is_empty()
            && self.hidden_cleared_at.is_none()
            && self.blocked_at.is_empty()
            && self.unblocked_at.is_empty()
    }

    pub(crate) fn monotonic(now: i64, floor: i64) -> i64 {
        now.max(floor.saturating_add(1))
    }

    pub(crate) fn record_watch_timestamp(&self, id: &str, now: i64) -> i64 {
        let floor = self
            .removed_history
            .get(id)
            .copied()
            .into_iter()
            .chain(self.history_cleared_at)
            .max();
        floor.map_or(now, |floor| Self::monotonic(now, floor))
    }

    pub(crate) fn touch_position(&mut self, id: &str, watched_at: i64, now: i64) {
        let prev = self.position_at.get(id).copied().unwrap_or(i64::MIN);
        let ts = now.max(watched_at).max(prev.saturating_add(1));
        self.position_at.insert(id.to_owned(), ts);
    }

    pub(crate) fn mark_history_removed(&mut self, id: &str, watched_at: Option<i64>, now: i64) {
        let prev_pos = self.position_at.remove(id).unwrap_or(i64::MIN);
        let prev_rem = self.removed_history.get(id).copied().unwrap_or(i64::MIN);
        let floor = prev_pos.max(prev_rem).max(watched_at.unwrap_or(i64::MIN));
        let ts = Self::monotonic(now, floor);
        self.removed_history.insert(id.to_owned(), ts);
        bound_tombstones(&mut self.removed_history, TOMBSTONES_LIMIT);
    }

    pub(crate) fn bump_history_cleared(&mut self, max_watched_at: Option<i64>, now: i64) {
        let floor = self
            .history_cleared_at
            .unwrap_or(i64::MIN)
            .max(max_watched_at.unwrap_or(i64::MIN));
        let ts = Self::monotonic(now, floor);
        self.history_cleared_at = Some(ts);
        self.position_at.clear();
        self.removed_history.clear();
    }

    pub(crate) fn mark_subscribed(&mut self, id: &str, now: i64) {
        let floor = self
            .unsubscribed_at
            .remove(id)
            .unwrap_or(i64::MIN)
            .max(self.subscribed_at.get(id).copied().unwrap_or(i64::MIN));
        self.subscribed_at
            .insert(id.to_owned(), Self::monotonic(now, floor));
    }

    pub(crate) fn mark_unsubscribed(&mut self, id: &str, now: i64) {
        let floor = self
            .subscribed_at
            .remove(id)
            .unwrap_or(i64::MIN)
            .max(self.unsubscribed_at.get(id).copied().unwrap_or(i64::MIN));
        self.unsubscribed_at
            .insert(id.to_owned(), Self::monotonic(now, floor));
        bound_tombstones(&mut self.unsubscribed_at, TOMBSTONES_LIMIT);
    }

    pub(crate) fn mark_watch_later_added(&mut self, id: &str, now: i64) {
        let floor = self
            .removed_watch_later
            .remove(id)
            .unwrap_or(i64::MIN)
            .max(self.watch_later_at.get(id).copied().unwrap_or(i64::MIN));
        self.watch_later_at
            .insert(id.to_owned(), Self::monotonic(now, floor));
    }

    pub(crate) fn mark_watch_later_removed(&mut self, id: &str, now: i64) {
        let floor = self.watch_later_at.remove(id).unwrap_or(i64::MIN).max(
            self.removed_watch_later
                .get(id)
                .copied()
                .unwrap_or(i64::MIN),
        );
        self.removed_watch_later
            .insert(id.to_owned(), Self::monotonic(now, floor));
        bound_tombstones(&mut self.removed_watch_later, TOMBSTONES_LIMIT);
    }

    pub(crate) fn touch_playlist(&mut self, id: &str, now: i64) {
        let floor = self
            .deleted_playlists
            .remove(id)
            .unwrap_or(i64::MIN)
            .max(self.playlists_at.get(id).copied().unwrap_or(i64::MIN));
        self.playlists_at
            .insert(id.to_owned(), Self::monotonic(now, floor));
    }

    pub(crate) fn mark_playlist_deleted(&mut self, id: &str, now: i64) {
        let floor = self
            .playlists_at
            .remove(id)
            .unwrap_or(i64::MIN)
            .max(self.deleted_playlists.get(id).copied().unwrap_or(i64::MIN));
        self.deleted_playlists
            .insert(id.to_owned(), Self::monotonic(now, floor));
        bound_tombstones(&mut self.deleted_playlists, TOMBSTONES_LIMIT);
    }

    pub(crate) fn mark_hidden(&mut self, id: &str, now: i64) {
        let floor = self
            .hidden_cleared_at
            .unwrap_or(i64::MIN)
            .max(self.hidden_at.get(id).copied().unwrap_or(i64::MIN));
        self.hidden_at
            .insert(id.to_owned(), Self::monotonic(now, floor));
    }

    pub(crate) fn bump_hidden_cleared(&mut self, now: i64) {
        let max_hidden = self.hidden_at.values().copied().max().unwrap_or(i64::MIN);
        let floor = self.hidden_cleared_at.unwrap_or(i64::MIN).max(max_hidden);
        self.hidden_cleared_at = Some(Self::monotonic(now, floor));
        self.hidden_at.clear();
    }

    pub(crate) fn mark_blocked(&mut self, id: &str, now: i64) {
        let floor = self
            .unblocked_at
            .remove(id)
            .unwrap_or(i64::MIN)
            .max(self.blocked_at.get(id).copied().unwrap_or(i64::MIN));
        self.blocked_at
            .insert(id.to_owned(), Self::monotonic(now, floor));
    }

    pub(crate) fn mark_unblocked(&mut self, id: &str, now: i64) {
        let floor = self
            .blocked_at
            .remove(id)
            .unwrap_or(i64::MIN)
            .max(self.unblocked_at.get(id).copied().unwrap_or(i64::MIN));
        self.unblocked_at
            .insert(id.to_owned(), Self::monotonic(now, floor));
        bound_tombstones(&mut self.unblocked_at, TOMBSTONES_LIMIT);
    }

    pub(crate) fn sanitize_and_prune(
        &mut self,
        subscriptions: &HashSet<String>,
        history: &HashSet<String>,
        watch_later: &HashSet<String>,
        playlists: &HashSet<String>,
        hidden: &HashSet<String>,
        blocked: &HashSet<String>,
    ) {
        self.position_at.retain(|id, _| history.contains(id));
        self.removed_history
            .retain(|id, ts| !id.is_empty() && self.history_cleared_at.is_none_or(|c| *ts > c));
        self.subscribed_at
            .retain(|id, _| subscriptions.contains(id));
        self.unsubscribed_at.retain(|id, _| !id.is_empty());
        self.watch_later_at.retain(|id, _| watch_later.contains(id));
        self.removed_watch_later.retain(|id, _| !id.is_empty());
        self.playlists_at.retain(|id, _| playlists.contains(id));
        self.deleted_playlists
            .retain(|id, _| DevicePlaylist::is_sync_id(id));
        self.hidden_at.retain(|id, _| hidden.contains(id));
        self.blocked_at.retain(|id, _| blocked.contains(id));
        self.unblocked_at.retain(|id, _| !id.is_empty());

        bound_tombstones(&mut self.removed_history, TOMBSTONES_LIMIT);
        bound_tombstones(&mut self.unsubscribed_at, TOMBSTONES_LIMIT);
        bound_tombstones(&mut self.removed_watch_later, TOMBSTONES_LIMIT);
        bound_tombstones(&mut self.deleted_playlists, TOMBSTONES_LIMIT);
        bound_tombstones(&mut self.unblocked_at, TOMBSTONES_LIMIT);
    }
}

pub(crate) fn bound_tombstones(map: &mut HashMap<String, i64>, limit: usize) {
    if map.len() <= limit {
        return;
    }
    let mut entries: Vec<(String, i64)> = map.drain().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    entries.truncate(limit);
    *map = entries.into_iter().collect();
}

/// Summary of what changed locally after merging an incoming peer snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncOutcome {
    pub changed: bool,
    pub history_changed: bool,
    pub subscriptions_changed: bool,
    pub filters_changed: bool,
    pub playlists_changed: bool,
}

/// Portable device library snapshot exchanged between paired YouTFast instances.
/// Never carries accounts, sessions, cookies, avatar URLs, or alert timestamps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncSnapshot {
    pub version: u32,
    #[serde(default)]
    pub listen_port: u16,
    /// Devices the sender completed a keyed exchange with, so paired devices
    /// learn about each other through any one of them.
    #[serde(default)]
    pub peers: Vec<SocketAddr>,
    #[serde(default)]
    pub subscriptions: Vec<Channel>,
    #[serde(default)]
    pub history: Vec<Watched>,
    #[serde(default)]
    pub watch_later: Vec<Video>,
    #[serde(default)]
    pub playlists: Vec<DevicePlaylist>,
    #[serde(default)]
    pub hidden_videos: Vec<String>,
    #[serde(default)]
    pub blocked_channels: Vec<Channel>,
    #[serde(default)]
    pub sync: SyncMetadata,
}

impl SyncSnapshot {
    /// Strips avatars and enforces all storage bounds on an untrusted peer snapshot.
    pub fn sanitized(mut self) -> Self {
        let mut seen_peers = HashSet::new();
        self.peers.retain(|peer| {
            peer.port() >= MIN_SYNC_PORT
                && is_private_or_mesh_ip(peer.ip())
                && seen_peers.insert(*peer)
        });
        self.peers.truncate(MAX_SHARED_PEERS);
        let mut seen_subs = HashSet::new();
        self.subscriptions.retain_mut(|channel| {
            channel.avatar = None;
            !channel.id.trim().is_empty() && seen_subs.insert(channel.id.clone())
        });
        self.subscriptions.truncate(SUBSCRIPTIONS_LIMIT);
        let kept_subs: HashSet<String> = self.subscriptions.iter().map(|c| c.id.clone()).collect();

        let mut seen_hist = HashSet::new();
        self.history
            .sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.video.id.cmp(&b.video.id)));
        self.history.retain_mut(|watched| {
            watched.video.channel_avatar = None;
            if !watched.position.is_finite() || watched.position < 0.0 {
                watched.position = 0.0;
            }
            !watched.video.id.trim().is_empty() && seen_hist.insert(watched.video.id.clone())
        });
        self.history.truncate(HISTORY_LIMIT);
        let kept_hist: HashSet<String> = self.history.iter().map(|w| w.video.id.clone()).collect();

        let mut seen_wl = HashSet::new();
        self.watch_later.retain_mut(|video| {
            video.channel_avatar = None;
            !video.id.trim().is_empty() && seen_wl.insert(video.id.clone())
        });
        self.watch_later.truncate(WATCH_LATER_LIMIT);
        let kept_wl: HashSet<String> = self.watch_later.iter().map(|v| v.id.clone()).collect();

        let mut seen_pl = HashSet::new();
        self.playlists.retain_mut(|playlist| {
            if !DevicePlaylist::is_sync_id(&playlist.id) || !seen_pl.insert(playlist.id.clone()) {
                return false;
            }
            let trimmed = playlist.name.trim();
            if trimmed.is_empty() {
                return false;
            }
            if trimmed.len() != playlist.name.len() {
                playlist.name = trimmed.to_owned();
            }
            let mut seen_videos = HashSet::new();
            playlist.videos.retain_mut(|video| {
                video.channel_avatar = None;
                !video.id.trim().is_empty() && seen_videos.insert(video.id.clone())
            });
            playlist.videos.truncate(PLAYLIST_VIDEOS_LIMIT);
            true
        });
        self.playlists.truncate(PLAYLISTS_LIMIT);
        let kept_pl: HashSet<String> = self.playlists.iter().map(|p| p.id.clone()).collect();

        let mut seen_hidden = HashSet::new();
        let mut clean_hidden =
            Vec::with_capacity(self.hidden_videos.len().min(HIDDEN_VIDEOS_LIMIT));
        for id in self.hidden_videos.drain(..).rev() {
            if !id.trim().is_empty() && seen_hidden.insert(id.clone()) {
                clean_hidden.push(id);
                if clean_hidden.len() == HIDDEN_VIDEOS_LIMIT {
                    break;
                }
            }
        }
        clean_hidden.reverse();
        self.hidden_videos = clean_hidden;
        let kept_hidden: HashSet<String> = self.hidden_videos.iter().cloned().collect();

        let mut seen_blocked = HashSet::new();
        let mut clean_blocked =
            Vec::with_capacity(self.blocked_channels.len().min(BLOCKED_CHANNELS_LIMIT));
        for mut channel in self.blocked_channels.drain(..).rev() {
            channel.avatar = None;
            if !channel.id.trim().is_empty() && seen_blocked.insert(channel.id.clone()) {
                clean_blocked.push(channel);
                if clean_blocked.len() == BLOCKED_CHANNELS_LIMIT {
                    break;
                }
            }
        }
        clean_blocked.reverse();
        self.blocked_channels = clean_blocked;
        let kept_blocked: HashSet<String> =
            self.blocked_channels.iter().map(|c| c.id.clone()).collect();

        self.sync.sanitize_and_prune(
            &kept_subs,
            &kept_hist,
            &kept_wl,
            &kept_pl,
            &kept_hidden,
            &kept_blocked,
        );
        self
    }
}

/// Serializes a sync snapshot into JSON, enforcing [`MAX_SYNC_BYTES`].
pub fn to_sync_json_bytes(snapshot: &SyncSnapshot) -> Result<Vec<u8>, &'static str> {
    struct Output {
        bytes: Vec<u8>,
        limit: usize,
        exceeded: bool,
    }

    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.limit - self.bytes.len() {
                self.exceeded = true;
                return Err(io::Error::other("sync size limit"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut output = Output {
        bytes: Vec::new(),
        limit: MAX_SYNC_BYTES as usize,
        exceeded: false,
    };
    serde_json::to_writer(&mut output, snapshot).map_err(|_| {
        if output.exceeded {
            ERR_SIZE_LIMIT
        } else {
            ERR_INVALID_RESPONSE
        }
    })?;
    Ok(output.bytes)
}

/// Parses a sync snapshot from bytes, enforcing [`MAX_SYNC_BYTES`] and [`SYNC_VERSION`].
pub fn parse_sync_bytes(bytes: &[u8]) -> Result<SyncSnapshot, &'static str> {
    parse_sync_reader(bytes)
}

/// Parses a sync snapshot from a reader, enforcing [`MAX_SYNC_BYTES`] and [`SYNC_VERSION`].
pub fn parse_sync_reader<R: Read>(reader: R) -> Result<SyncSnapshot, &'static str> {
    parse_sync_limited(reader, MAX_SYNC_BYTES)
}

fn parse_sync_limited<R: Read>(reader: R, limit: u64) -> Result<SyncSnapshot, &'static str> {
    let mut limited = Limited::new(reader, limit);
    let (prefix, length) = read_bom_prefix(&mut limited).map_err(|_| {
        if limited.exceeded {
            ERR_SIZE_LIMIT
        } else {
            ERR_INVALID_RESPONSE
        }
    })?;
    let snapshot = {
        let mut buf = io::BufReader::new(prefix[..length].chain(&mut limited));
        let mut decoder = serde_json::Deserializer::from_reader(&mut buf);
        match SyncSnapshot::deserialize(&mut decoder) {
            Ok(snapshot) => {
                if decoder.end().is_err() {
                    return Err(if limited.exceeded {
                        ERR_SIZE_LIMIT
                    } else {
                        ERR_INVALID_RESPONSE
                    });
                }
                snapshot
            }
            Err(_) => {
                return Err(if limited.exceeded {
                    ERR_SIZE_LIMIT
                } else {
                    ERR_INVALID_RESPONSE
                });
            }
        }
    };
    if snapshot.version != SYNC_VERSION {
        return Err(ERR_INCOMPATIBLE_VERSION);
    }
    Ok(snapshot.sanitized())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_keys_decode_to_32_bytes_and_reject_legacy_keys() {
        let key = generate_pairing_key();
        assert_eq!(key.len(), 71);
        assert_eq!(key.split('-').count(), 8);
        let decoded = normalize_pairing_key(&key).unwrap();
        assert_eq!(
            normalize_pairing_key(&format!(
                "  {}  ",
                key.replace('-', " ").to_ascii_uppercase()
            )),
            Some(decoded)
        );
        assert!(normalize_pairing_key("abcd-efgh-ijkl-mnop").is_none());
        assert!(normalize_pairing_key(&format!("{key}00")).is_none());
        assert!(normalize_pairing_key(&("x".to_owned() + &key[1..])).is_none());
    }

    #[test]
    fn private_and_mesh_ip_checks_accept_tailscale_netbird_and_lan_and_reject_public() {
        for addr in [
            "127.0.0.1",
            "10.0.0.5",
            "172.16.4.12",
            "192.168.1.42",
            "169.254.1.10",
            "100.64.0.1",
            "100.100.100.100",
            "100.127.255.254",
            "::1",
            "fd7a:115c:a1e0::1",
            "fe80::1",
            "::ffff:100.64.0.9",
        ] {
            let ip: IpAddr = addr.parse().unwrap();
            assert!(is_private_or_mesh_ip(ip), "{addr} should be allowed");
        }
        for addr in [
            "0.0.0.0",
            "1.1.1.1",
            "8.8.8.8",
            "100.63.255.255",
            "100.128.0.1",
            "2001:4860:4860::8888",
            "::ffff:8.8.8.8",
        ] {
            let ip: IpAddr = addr.parse().unwrap();
            assert!(!is_private_or_mesh_ip(ip), "{addr} should be rejected");
        }
    }

    #[test]
    fn parse_peer_target_accepts_mesh_lan_and_hostnames_and_rejects_public_or_invalid() {
        assert_eq!(
            parse_peer_target("100.64.0.2", DEFAULT_SYNC_PORT),
            Ok(("100.64.0.2".into(), DEFAULT_SYNC_PORT))
        );
        assert_eq!(
            parse_peer_target("http://192.168.1.50:40000", DEFAULT_SYNC_PORT),
            Ok(("192.168.1.50".into(), 40000))
        );
        assert_eq!(
            parse_peer_target("desktop.tail1234.ts.net", DEFAULT_SYNC_PORT),
            Ok(("desktop.tail1234.ts.net".into(), DEFAULT_SYNC_PORT))
        );
        assert_eq!(
            parse_peer_target("[fd7a:115c:a1e0::1]:39000", DEFAULT_SYNC_PORT),
            Ok(("fd7a:115c:a1e0::1".into(), 39000))
        );
        assert_eq!(
            parse_peer_target("fd7a:115c:a1e0::1", DEFAULT_SYNC_PORT),
            Ok(("fd7a:115c:a1e0::1".into(), DEFAULT_SYNC_PORT))
        );
        assert_eq!(
            parse_peer_target("8.8.8.8:38452", DEFAULT_SYNC_PORT),
            Err(ERR_PRIVATE_ONLY)
        );
        assert_eq!(
            parse_peer_target("https://100.64.0.2:38452", DEFAULT_SYNC_PORT),
            Err(ERR_PEER_MISSING)
        );
        assert_eq!(
            parse_peer_target("100.64.0.2/path", DEFAULT_SYNC_PORT),
            Err(ERR_PEER_MISSING)
        );
        assert_eq!(
            parse_peer_target("100.64.0.2:0", DEFAULT_SYNC_PORT),
            Err(ERR_PEER_MISSING)
        );
        assert_eq!(
            parse_peer_target("   ", DEFAULT_SYNC_PORT),
            Err(ERR_PEER_MISSING)
        );
    }

    #[test]
    fn snapshot_parsing_rejects_wrong_version_trailing_data_and_oversized_payloads() {
        let wrong_version = br#"{"version": 99}"#;
        assert_eq!(
            parse_sync_bytes(wrong_version).unwrap_err(),
            ERR_INCOMPATIBLE_VERSION
        );
        let trailing = br#"{"version": 1} extra"#;
        assert_eq!(
            parse_sync_bytes(trailing).unwrap_err(),
            ERR_INVALID_RESPONSE
        );
        let too_big = format!("{{\"version\":1}}{}", " ".repeat(20));
        assert_eq!(
            parse_sync_limited(too_big.as_bytes(), 14).unwrap_err(),
            ERR_SIZE_LIMIT
        );
    }

    #[test]
    fn mesh_status_parsers_keep_reachable_private_peers_once() {
        let tailscale = br#"{
            "Self": {"TailscaleIPs": ["100.85.31.24"], "Online": true},
            "Peer": {
                "nodekey:a": {"TailscaleIPs": ["fd7a:115c:a1e0::1", "100.95.0.2"], "Online": true},
                "nodekey:b": {"TailscaleIPs": ["100.95.0.3"], "Online": false},
                "nodekey:c": {"TailscaleIPs": ["fd7a:115c:a1e0::4"], "Online": true},
                "nodekey:d": {"TailscaleIPs": ["8.8.8.8"], "Online": true},
                "nodekey:e": {"Online": true, "Future": 1}
            }
        }"#;
        let ips =
            |list: &[&str]| -> Vec<IpAddr> { list.iter().map(|ip| ip.parse().unwrap()).collect() };
        assert_eq!(tailscale_self_ips(tailscale), ips(&["100.85.31.24"]));
        assert_eq!(
            tailscale_peer_ips(tailscale),
            ips(&["100.95.0.2", "fd7a:115c:a1e0::4"])
        );
        assert!(tailscale_self_ips(br#"{"Self": null}"#).is_empty());
        assert!(tailscale_peer_ips(br#"{"Peer": null}"#).is_empty());
        assert!(tailscale_peer_ips(b"not json").is_empty());

        let netbird = br#"{
            "netbirdIp": "100.90.0.1/16",
            "peers": {"total": 3, "connected": 1, "details": [
                {"fqdn": "a.netbird.cloud", "netbirdIp": "100.90.0.2", "status": "Connected"},
                {"fqdn": "b.netbird.cloud", "netbirdIp": "100.90.0.3/32", "status": "Idle"},
                {"fqdn": "c.netbird.cloud", "netbirdIp": "1.1.1.1", "status": "Connected"},
                {"fqdn": "d.netbird.cloud", "status": "Connecting"}
            ]}
        }"#;
        assert_eq!(netbird_self_ips(netbird), ips(&["100.90.0.1"]));
        assert_eq!(
            netbird_peer_ips(netbird),
            ips(&["100.90.0.2", "100.90.0.3"])
        );
        assert!(netbird_self_ips(br#"{"netbirdIp": "8.8.8.8/32"}"#).is_empty());
        assert!(netbird_peer_ips(br#"{"peers": {"details": null}}"#).is_empty());
    }

    #[test]
    fn container_filter_hides_virtual_bridges_and_keeps_real_lan_and_vpn_interfaces() {
        for hidden in [
            "docker0",
            "docker_gwbridge",
            "podman0",
            "br-9f8a7b6c5d4e",
            "BR-ABCDEF123456",
            "veth12ab34",
            "virbr0",
            "lxcbr0",
            "lxdbr0",
            "incusbr0",
            "cni0",
            "flannel.1",
            "cali12345",
            "cilium_host",
            "kube-ipvs0",
            "vboxnet0",
            "vmnet8",
            "vEthernet (WSL (Hyper-V firewall))",
            "vEthernet (DockerNAT)",
            "vEthernet (Default Switch)",
        ] {
            assert!(
                is_container_interface(hidden),
                "{hidden} should be filtered as container/VM bridge"
            );
        }
        for kept in [
            "eth0",
            "enp3s0",
            "en0",
            "wlan0",
            "wlp2s0",
            "br0",
            "br-lan",
            "br-0123456789az",
            "br-abcd1234",
            "Wi-Fi",
            "Ethernet",
            "tailscale0",
            "Tailscale",
            "wt0",
            "NetBird",
            "wg0",
            "ztkl9ab123",
            "utun4",
            "vEthernet (External)",
        ] {
            assert!(
                !is_container_interface(kept),
                "{kept} should not be filtered"
            );
        }
    }

    #[test]
    fn sync_interface_classification_and_mesh_refinement_identify_vpns_and_sort_them() {
        let lan = Ipv4Addr::new(192, 168, 1, 20);
        let mesh_ts = Ipv4Addr::new(100, 85, 31, 24);
        let mesh_nb = Ipv4Addr::new(100, 90, 0, 1);
        let wg_ip = Ipv4Addr::new(10, 10, 0, 2);
        let zt_ip = Ipv4Addr::new(10, 147, 20, 5);
        let ovpn_ip = Ipv4Addr::new(10, 8, 0, 4);

        assert_eq!(
            classify_sync_interface("tailscale0", mesh_ts, true),
            SyncNetwork::Tailscale
        );
        assert_eq!(
            classify_sync_interface("Tailscale", mesh_ts, true),
            SyncNetwork::Tailscale
        );
        assert_eq!(
            classify_sync_interface("wt0", mesh_nb, true),
            SyncNetwork::NetBird
        );
        assert_eq!(
            classify_sync_interface("NetBird", mesh_nb, true),
            SyncNetwork::NetBird
        );
        assert_eq!(
            classify_sync_interface("wg0", wg_ip, true),
            SyncNetwork::WireGuard
        );
        assert_eq!(
            classify_sync_interface("wg-home", wg_ip, true),
            SyncNetwork::WireGuard
        );
        assert_eq!(
            classify_sync_interface("WireGuard Tunnel", wg_ip, true),
            SyncNetwork::WireGuard
        );
        assert_eq!(
            classify_sync_interface("ztkl9ab123", zt_ip, false),
            SyncNetwork::ZeroTier
        );
        assert_eq!(
            classify_sync_interface("ZeroTier One [8056c2e21c000001]", zt_ip, false),
            SyncNetwork::ZeroTier
        );
        assert_eq!(
            classify_sync_interface("tun0", ovpn_ip, true),
            SyncNetwork::Vpn
        );
        assert_eq!(
            classify_sync_interface("utun4", mesh_ts, true),
            SyncNetwork::Vpn
        );
        assert_eq!(
            classify_sync_interface("wlp2s0", lan, false),
            SyncNetwork::LocalNetwork
        );
        assert_eq!(
            classify_sync_interface("Wi-Fi", lan, false),
            SyncNetwork::LocalNetwork
        );

        let mut collected = collect_sync_endpoints([
            ("lo", IpAddr::V4(Ipv4Addr::LOCALHOST), true, false),
            (
                "docker0",
                IpAddr::V4(Ipv4Addr::new(172, 17, 0, 1)),
                false,
                false,
            ),
            (
                "br-abcd1234ef56",
                IpAddr::V4(Ipv4Addr::new(172, 18, 0, 1)),
                false,
                false,
            ),
            ("br-lan", IpAddr::V4(lan), false, false),
            ("utun3", IpAddr::V4(mesh_nb), false, true),
            ("utun4", IpAddr::V4(mesh_ts), false, true),
            ("wg0", IpAddr::V4(wg_ip), false, true),
            ("eth0", IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), false, false),
        ]);
        assert_eq!(
            collected,
            vec![
                LocalSyncAddress {
                    ip: IpAddr::V4(wg_ip),
                    network: SyncNetwork::WireGuard,
                },
                LocalSyncAddress {
                    ip: IpAddr::V4(mesh_ts),
                    network: SyncNetwork::Vpn,
                },
                LocalSyncAddress {
                    ip: IpAddr::V4(mesh_nb),
                    network: SyncNetwork::Vpn,
                },
                LocalSyncAddress {
                    ip: IpAddr::V4(lan),
                    network: SyncNetwork::LocalNetwork,
                },
            ]
        );

        apply_mesh_identities(
            &mut collected,
            &[IpAddr::V4(mesh_ts)],
            &[IpAddr::V4(mesh_nb)],
        );
        assert_eq!(
            collected,
            vec![
                LocalSyncAddress {
                    ip: IpAddr::V4(mesh_ts),
                    network: SyncNetwork::Tailscale,
                },
                LocalSyncAddress {
                    ip: IpAddr::V4(mesh_nb),
                    network: SyncNetwork::NetBird,
                },
                LocalSyncAddress {
                    ip: IpAddr::V4(wg_ip),
                    network: SyncNetwork::WireGuard,
                },
                LocalSyncAddress {
                    ip: IpAddr::V4(lan),
                    network: SyncNetwork::LocalNetwork,
                },
            ]
        );
        assert_eq!(collected[0].endpoint(38452), "100.85.31.24:38452");
    }

    #[test]
    fn announcements_verify_only_with_the_same_key_and_the_query_they_answer() {
        let key = normalize_pairing_key(&generate_pairing_key()).unwrap();
        let other = normalize_pairing_key(&generate_pairing_key()).unwrap();
        let (query, nonce) = announce_query(38452, &key);
        assert_eq!(read_announce_query(&query, &key), Some((38452, nonce)));
        assert_eq!(read_announce_query(&query, &other), None);
        // A fresh nonce each time: the same device and key are not linkable.
        assert_ne!(query, announce_query(38452, &key).0);

        let reply = announce_reply(40000, &key, &nonce);
        assert_eq!(read_announce_reply(&reply, &key, &nonce), Some(40000));
        // A reply to an earlier query, replayed, does not answer this one.
        let (_, later) = announce_query(38452, &key);
        assert_eq!(read_announce_reply(&reply, &key, &later), None);
        assert_eq!(read_announce_query(&reply, &key), None);

        let mut forged = reply;
        forged[9..11].copy_from_slice(&38452u16.to_be_bytes());
        assert_eq!(read_announce_reply(&forged, &key, &nonce), None);
        assert_eq!(
            read_announce_query(&query[..ANNOUNCE_BYTES - 1], &key),
            None
        );
    }

    #[test]
    fn shared_peers_keep_only_private_endpoints_and_older_snapshots_still_parse() {
        let mut snapshot = crate::library::Library::default().sync_snapshot(38452);
        snapshot.peers = vec![
            "100.95.0.2:38452".parse().unwrap(),
            "100.95.0.2:38452".parse().unwrap(),
            "8.8.8.8:38452".parse().unwrap(),
            "192.168.1.5:80".parse().unwrap(),
        ];
        snapshot
            .peers
            .extend((0..40).map(|n| SocketAddr::new([10, 0, 0, n].into(), 38452)));
        let parsed = parse_sync_bytes(&to_sync_json_bytes(&snapshot).unwrap()).unwrap();
        assert_eq!(parsed.peers.len(), MAX_SHARED_PEERS);
        assert_eq!(parsed.peers[0], "100.95.0.2:38452".parse().unwrap());
        assert!(
            parsed
                .peers
                .iter()
                .all(|peer| peer.ip().is_ipv4() && peer.port() == 38452)
        );
        assert!(!parsed.peers.contains(&"8.8.8.8:38452".parse().unwrap()));

        // 0.10.0 sends no peers.
        let older = format!("{{\"version\":{SYNC_VERSION},\"listen_port\":38452}}");
        assert!(parse_sync_bytes(older.as_bytes()).unwrap().peers.is_empty());
    }
}
