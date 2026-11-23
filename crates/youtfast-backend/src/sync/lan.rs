//! Finds paired devices on the local network: a search broadcasts a query
//! on the listen port and every device sharing the key answers. Datagrams
//! carry only a port, a nonce and an HMAC under the key
//! (`youtfast_storage::sync::announce_query`); a reply is bound to the
//! query's nonce, and the encrypted TCP exchange still verifies every device
//! found. Mesh VPNs such as Tailscale do not carry broadcasts; they are
//! covered by the clients' peer lists and by the peers devices share.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use youtfast_storage::sync::{
    ANNOUNCE_BYTES, PAIRING_KEY_BYTES, announce_query, announce_reply, is_private_or_mesh_ip,
    read_announce_query, read_announce_reply,
};

/// How long a search collects replies.
const REPLY_WINDOW: Duration = Duration::from_millis(800);
/// Most replies one search keeps.
const MAX_REPLIES: usize = 32;
/// Most queries answered per second, so replayed or flooded queries can't
/// turn the listener into a reflector.
const ANSWERS_PER_SECOND: u32 = 16;

/// Answers queries made with the current key until the task is aborted.
/// `key` reads the key in force, `None` while sync is off or reconfiguring.
pub(super) async fn serve(
    socket: UdpSocket,
    port: u16,
    key: impl Fn() -> Option<[u8; PAIRING_KEY_BYTES]>,
) {
    let mut buffer = [0; ANNOUNCE_BYTES + 1];
    let mut window = Instant::now();
    let mut answered = 0;
    loop {
        let Ok((len, from)) = socket.recv_from(&mut buffer).await else {
            // A transient error (an ICMP unreachable on some systems) must
            // not stop answering; wait a moment instead of spinning.
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };
        if !is_private_or_mesh_ip(from.ip()) {
            continue;
        }
        let Some(key) = key() else {
            continue;
        };
        let Some((_, nonce)) = read_announce_query(&buffer[..len], &key) else {
            continue;
        };
        if window.elapsed() >= Duration::from_secs(1) {
            window = Instant::now();
            answered = 0;
        }
        if answered >= ANSWERS_PER_SECOND {
            continue;
        }
        answered += 1;
        // Never wait on a full send buffer: a dropped reply is retried by
        // the searcher's next round.
        let _ = socket.try_send_to(&announce_reply(port, &key, &nonce), from);
    }
}

/// Devices on the local networks that answer a query made with `key`.
pub(super) async fn search(port: u16, key: &[u8; PAIRING_KEY_BYTES]) -> Vec<SocketAddr> {
    let targets: Vec<SocketAddr> = broadcast_addresses()
        .into_iter()
        .map(|ip| SocketAddr::new(IpAddr::V4(ip), port))
        .collect();
    query(&targets, port, key).await
}

/// Sends a query to every target and collects the replies to it.
pub(super) async fn query(
    targets: &[SocketAddr],
    port: u16,
    key: &[u8; PAIRING_KEY_BYTES],
) -> Vec<SocketAddr> {
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await else {
        return Vec::new();
    };
    if socket.set_broadcast(true).is_err() {
        return Vec::new();
    }
    let (datagram, nonce) = announce_query(port, key);
    for target in targets {
        let _ = socket.send_to(&datagram, target).await;
    }
    let mut found = Vec::new();
    let collect = async {
        let mut buffer = [0; ANNOUNCE_BYTES + 1];
        while found.len() < MAX_REPLIES {
            let Ok((len, from)) = socket.recv_from(&mut buffer).await else {
                // Windows reports an unreachable earlier target here.
                tokio::time::sleep(Duration::from_millis(10)).await;
                continue;
            };
            if !is_private_or_mesh_ip(from.ip()) {
                continue;
            }
            if let Some(reply_port) = read_announce_reply(&buffer[..len], key, &nonce) {
                let addr = SocketAddr::new(from.ip(), reply_port);
                if !found.contains(&addr) {
                    found.push(addr);
                }
            }
        }
    };
    let _ = tokio::time::timeout(REPLY_WINDOW, collect).await;
    found
}

/// The directed broadcast address of every private IPv4 network this
/// computer is on, plus the limited broadcast address.
fn broadcast_addresses() -> Vec<Ipv4Addr> {
    let mut addresses: Vec<Ipv4Addr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|interface| match interface.addr {
            if_addrs::IfAddr::V4(v4)
                if !v4.ip.is_loopback()
                    && v4.ip.is_private()
                    && v4.netmask != Ipv4Addr::BROADCAST =>
            {
                Some(Ipv4Addr::from(u32::from(v4.ip) | !u32::from(v4.netmask)))
            }
            _ => None,
        })
        .collect();
    addresses.push(Ipv4Addr::BROADCAST);
    addresses.sort();
    addresses.dedup();
    addresses
}
