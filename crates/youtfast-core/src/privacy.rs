//! The fixed messages external errors become. A dependency's error text can
//! carry URLs, cookies, profile names or account data, so the interface and
//! the logs only ever show one of these.

/// TLS refused YouTube's certificate.
pub const UNTRUSTED_CERTIFICATE: &str = "Could not verify YouTube's certificate. Check the system date and any proxy or antivirus inspecting HTTPS, then retry.";

pub const YOUTUBE_RATE_LIMITED: &str = "YouTube is limiting requests. Retry later.";

pub fn safe_error(error: &str) -> String {
    if [
        UNTRUSTED_CERTIFICATE,
        "The system keyring could not complete the request. Unlock it and retry.",
        "The system keyring is locked. Unlock it to stay signed in.",
        "The system keyring is unavailable, so the session cannot be kept.",
        "The system keyring did not respond.",
        "The system keyring failed.",
        "This session is too large for the Windows Credential Manager (2560 bytes), so it cannot be kept there.",
        "yt-dlp is not installed for Python, so the browser cannot be read.",
        "Install python-secretstorage, unlock the desktop keyring, and retry browser sign-in.",
        "YouTube refused the request from this connection. Retry later.",
        YOUTUBE_RATE_LIMITED,
        "YouTube needs a valid session. Sign in again.",
        "The request timed out. Retry.",
        "YouTube returned an unreadable response. Retry.",
        "Could not complete the YouTube request. Retry.",
        "Could not check for updates. Retry later.",
        "Could not download the update. Retry later.",
        "The downloaded update failed integrity verification.",
        "Could not install the update on this system.",
        "Automatic installation is not supported for this package format.",
        "Update authentication was cancelled or unavailable.",
        "The downloaded update is incompatible with this system.",
        "The installation directory is not writable.",
        "Generate a new pairing key and copy it to both devices.",
        "Enter the other device's Tailscale, NetBird, or local network address.",
        "Device sync only connects to private network, Tailscale, or NetBird addresses.",
        "Could not reach the other device. Check that YouTFast is open with Device sync enabled and that both devices are on the same Tailscale, NetBird, or local network.",
        "The other device rejected the pairing key. Check that both devices use the same key.",
        "The other device uses an incompatible sync version. Update YouTFast on both devices.",
        "The library exceeds the 64 MiB sync limit.",
        "Could not read the sync response from the other device.",
        "Could not start the device sync listener on the selected port.",
        "No other device with this pairing key was found. Check that YouTFast is open with Device sync and the same key on your other computer, or enter its address.",
    ]
    .contains(&error)
    {
        return error.to_owned();
    }
    let error = error.to_ascii_lowercase();
    // rustls refused the certificate (see `innertube::Error`'s reqwest causes).
    if error.contains("invalid peer certificate") {
        UNTRUSTED_CERTIFICATE
    } else if error.contains("python-secretstorage") {
        "Install python-secretstorage, unlock the desktop keyring, and retry browser sign-in."
    } else if error.contains("keyring") {
        "The system keyring could not complete the request. Unlock it and retry."
    } else if error.contains("bot") || error.contains("unusual traffic") {
        "YouTube refused the request from this connection. Retry later."
    } else if error.contains("rate limit") || error.contains("429") {
        YOUTUBE_RATE_LIMITED
    } else if error.contains("cookie")
        || error.contains("session")
        || error.contains("sign in")
        || error.contains("signed out")
        || error.contains("authentication")
    {
        "YouTube needs a valid session. Sign in again."
    } else if error.contains("timeout") || error.contains("timed out") {
        "The request timed out. Retry."
    } else if error.contains("json") || error.contains("deserialize") {
        "YouTube returned an unreadable response. Retry."
    } else {
        "Could not complete the YouTube request. Retry."
    }
    .to_owned()
}
