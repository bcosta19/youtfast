//! Lists the other computers on Tailscale and NetBird through their own
//! command-line clients (`status --json`), so device sync can find peers
//! without an address. Best-effort: a missing, stopped, or slow client
//! contributes no peers. The clients' output is never logged.

use std::net::IpAddr;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use youtfast_storage::sync::{
    netbird_peer_ips, netbird_self_ips, tailscale_peer_ips, tailscale_self_ips,
};

const STATUS_TIMEOUT: Duration = Duration::from_secs(3);
const OUTPUT_LIMIT: u64 = 4 * 1024 * 1024;

// A desktop application's PATH often lacks the clients' install folders
// (macOS app bundles, Windows Program Files), so the usual locations follow
// the bare name.
#[cfg(target_os = "macos")]
const TAILSCALE: &[&str] = &[
    "tailscale",
    "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
    "/opt/homebrew/bin/tailscale",
    "/usr/local/bin/tailscale",
];
#[cfg(windows)]
const TAILSCALE: &[&str] = &["tailscale", r"C:\Program Files\Tailscale\tailscale.exe"];
#[cfg(not(any(target_os = "macos", windows)))]
const TAILSCALE: &[&str] = &["tailscale"];

#[cfg(target_os = "macos")]
const NETBIRD: &[&str] = &[
    "netbird",
    "/opt/homebrew/bin/netbird",
    "/usr/local/bin/netbird",
];
#[cfg(windows)]
const NETBIRD: &[&str] = &["netbird", r"C:\Program Files\NetBird\netbird.exe"];
#[cfg(not(any(target_os = "macos", windows)))]
const NETBIRD: &[&str] = &["netbird"];

/// Mesh and private addresses of the peers Tailscale and NetBird report.
pub(super) async fn mesh_peers() -> Vec<IpAddr> {
    let (tailscale, netbird) = tokio::join!(status(TAILSCALE), status(NETBIRD));
    let mut ips = tailscale
        .map(|json| tailscale_peer_ips(&json))
        .unwrap_or_default();
    ips.extend(
        netbird
            .map(|json| netbird_peer_ips(&json))
            .unwrap_or_default(),
    );
    ips.sort();
    ips.dedup();
    ips
}

/// This computer's own mesh addresses reported by Tailscale and NetBird, so
/// local VPN interfaces (including macOS `utunN`) can be identified accurately.
pub(super) async fn local_mesh_ips() -> (Vec<IpAddr>, Vec<IpAddr>) {
    let (tailscale, netbird) = tokio::join!(status(TAILSCALE), status(NETBIRD));
    (
        tailscale
            .map(|json| tailscale_self_ips(&json))
            .unwrap_or_default(),
        netbird
            .map(|json| netbird_self_ips(&json))
            .unwrap_or_default(),
    )
}

/// `status --json` from the first of `programs` that exists.
async fn status(programs: &[&str]) -> Option<Vec<u8>> {
    for program in programs {
        match run(program).await {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return None,
            Ok(output) => return output,
        }
    }
    None
}

async fn run(program: &str) -> std::io::Result<Option<Vec<u8>>> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(["status", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = command.spawn()?;
    let Some(stdout) = child.stdout.take() else {
        return Ok(None);
    };
    let read = async {
        let mut output = Vec::new();
        stdout
            .take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut output)
            .await
            .ok()?;
        let success = child.wait().await.ok()?.success();
        (success && output.len() as u64 <= OUTPUT_LIMIT).then_some(output)
    };
    Ok(tokio::time::timeout(STATUS_TIMEOUT, read)
        .await
        .ok()
        .flatten())
}
