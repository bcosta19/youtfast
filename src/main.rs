//! YouTFast: YouTube, native and fast.
// Release builds open no console window on Windows, like Spotifast and ZapFast.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod appearance;
mod backend;
#[cfg(feature = "demo")]
mod demo;
mod desktop;
mod logic;
mod model;
mod mpv;
mod storage;
mod ui;
mod util;
mod youtube;

use appearance::theme;
use desktop::{diagnostics, instance};

use std::sync::Arc;

use clap::Parser;

#[derive(Parser)]
#[command(version, about, args_conflicts_with_subcommands = true)]
struct Cli {
    /// Synthetic, offline UI; never opens account stores or the running instance.
    #[cfg(feature = "demo")]
    #[arg(long)]
    demo: bool,
    #[cfg(feature = "demo")]
    #[arg(long, default_value = "home")]
    demo_view: String,
    #[cfg(feature = "demo")]
    #[arg(long, default_value = "en", value_parser = ["en", "pt-BR"])]
    demo_language: String,
    #[cfg(feature = "demo")]
    #[arg(long)]
    demo_light: bool,
    #[cfg(feature = "demo")]
    #[arg(long, default_value = "1280x800")]
    demo_size: String,
    #[cfg(feature = "demo")]
    #[arg(long)]
    demo_capture: Option<std::path::PathBuf>,
    /// A YouTube link to open, or something to search for.
    open: Option<String>,
    /// Log debug categories (private data stays redacted).
    #[arg(long)]
    verbose: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Asks the running copy to list its palettes again (the Omarchy hook
    /// calls this). Does nothing when YouTFast is not running.
    ReloadThemes,
}

fn redact_log(record: &log::Record<'_>, message: &str) -> Option<std::borrow::Cow<'static, str>> {
    let startup = format!(
        "Starting youtfast {} on {} ({})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    if record.target() == "youtfast" && message == startup {
        return None;
    }
    if record.target().starts_with("youtfast") && diagnostics::controlled(message) {
        return None;
    }
    let category = if record.target() == "mpv" {
        "mpv reported a playback diagnostic"
    } else if record.target().starts_with("youtfast") {
        [
            "could not save the library",
            "could not save the settings",
            "the backend has stopped",
            "loading comments failed",
            "search suggestions failed",
            "loading the rating",
            "loading the save menu",
            "loading the details",
            "loading",
            "rating",
            "saving",
            "the SponsorBlock lookup stopped",
            "the desktop notification failed",
            "a seed's related videos failed",
            "a seed's lookup stopped",
            "the account has no recommendations",
            "the account's recommendations failed",
            "signed in",
            "sign-in failed",
            "renewing the session",
            "videos will play signed out",
            "could not start mpv",
            "the alerts check failed",
            "refused signed out",
            "opening video",
            "signed-in playback retry enabled",
            "quality set",
            "mpv",
            "reading browser cookies failed",
            "not writing rustypipe's cache",
            "could not write rustypipe's cache",
            "could not restrict",
            "the library file was unreadable",
            "the settings file was unreadable",
            "another copy is running",
            "no running copy answered",
            "could not reach the running copy",
            "could not start the video renderer",
            "could not make the video framebuffer",
            "video renderer left",
            "could not read yt-dlp's answer",
            "a feed task failed",
            "a shorts task failed",
            "update check failed",
            "update installation failed",
            "updated youtfast",
        ]
        .into_iter()
        .find(|prefix| message.starts_with(prefix))
        .unwrap_or("application diagnostic (private detail omitted)")
    } else {
        "dependency diagnostic (private detail omitted)"
    };
    Some(std::borrow::Cow::Borrowed(category))
}

/// TLS refused YouTube's certificate.
pub(crate) const UNTRUSTED_CERTIFICATE: &str = "Could not verify YouTube's certificate. Check the system date and any proxy or antivirus inspecting HTTPS, then retry.";

pub(crate) fn safe_error(error: &str) -> String {
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
        "YouTube is limiting requests. Retry later.",
        "YouTube needs a valid session. Sign in again.",
        "The request timed out. Retry.",
        "YouTube returned an unreadable response. Retry.",
        "Could not complete the YouTube request. Retry.",
        "Could not check for updates. Retry later.",
        "Could not download the update. Retry later.",
        "The downloaded update failed integrity verification.",
        "Could not install the update on this system.",
        "Automatic installation is not supported for this package format.",
    ]
    .contains(&error)
    {
        return error.to_owned();
    }
    let error = error.to_ascii_lowercase();
    // rustls refused the certificate (see `rustypipe::error`'s reqwest causes).
    if error.contains("invalid peer certificate") {
        UNTRUSTED_CERTIFICATE
    } else if error.contains("python-secretstorage") {
        "Install python-secretstorage, unlock the desktop keyring, and retry browser sign-in."
    } else if error.contains("keyring") {
        "The system keyring could not complete the request. Unlock it and retry."
    } else if error.contains("bot") || error.contains("unusual traffic") {
        "YouTube refused the request from this connection. Retry later."
    } else if error.contains("rate limit") || error.contains("429") {
        "YouTube is limiting requests. Retry later."
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

fn main() -> eframe::Result {
    let cli = Cli::parse();
    #[cfg(feature = "demo")]
    if cli.demo {
        return demo::run(&cli);
    }
    // A command-line action for the running copy never opens a window.
    if matches!(cli.command, Some(Command::ReloadThemes)) {
        instance::send("reload-themes");
        return Ok(());
    }
    let filter = if cli.verbose {
        "info,youtfast=debug,mpv=debug"
    } else {
        "warn,youtfast=info,mpv=warn"
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("YouTFast")
            .with_app_id("youtfast")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([480.0, 360.0])
            .with_icon(app_icon(128)),
        // The video is drawn by libmpv's OpenGL renderer, so OpenGL it is.
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    // One running copy: a later launch hands its link or search to the
    // window already open and exits.
    let request = cli
        .open
        .as_deref()
        .map(|text| format!("open {text}"))
        .unwrap_or_else(|| "show".to_owned());
    let guard = match instance::launch(&request) {
        instance::Launch::Only(guard) => guard,
        instance::Launch::Handed => {
            log::info!("another copy is running; handed the request to it");
            return Ok(());
        }
        instance::Launch::Declined => {
            log::info!("another copy is running but declined the request");
            return Ok(());
        }
    };
    // The installed fonts for other scripts are looked for while the window
    // and its OpenGL context are made.
    let fonts = theme::SystemFonts::find();
    // Only the owning instance opens the rotating log, avoiding writers from
    // short-lived launcher processes racing with log rotation.
    if diagnostics::init(filter).is_err() {
        eprintln!("could not start logging");
    }
    // The app holds the instance guard for the run; the creator closure is
    // dropped as soon as it returns, so it cannot keep it.
    let guard = Arc::new(guard);
    let open = cli.open;
    // eframe makes the app once its OpenGL painter works; an OpenGL error
    // after that (a lost context) is not a missing OpenGL 2.0.
    let created = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let creating = created.clone();
    let result = eframe::run_native(
        "YouTFast",
        options,
        Box::new(move |cc| {
            creating.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(Box::new(app::App::new(cc, open, guard, fonts)?))
        }),
    );
    if let Err(error) = &result
        && !created.load(std::sync::atomic::Ordering::Relaxed)
        && needs_opengl(error)
    {
        // A release build has no console on Windows, nor from the Dock on
        // macOS: without this the window just never appears.
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("YouTFast")
            .set_description(appearance::i18n::message(saved_language(), NEEDS_OPENGL))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }
    result
}

const NEEDS_OPENGL: &str = "YouTFast needs OpenGL 2.0 or newer to draw its window. Update the graphics driver; in a virtual machine, turn on 3D acceleration.";

/// Whether the window could not get the OpenGL it draws with: no driver
/// (a virtual machine without 3D acceleration offers Windows' OpenGL 1.1)
/// or no usable configuration.
fn needs_opengl(error: &eframe::Error) -> bool {
    matches!(
        error,
        eframe::Error::OpenGL(_) | eframe::Error::Glutin(_) | eframe::Error::NoGlutinConfigs(..)
    )
}

/// The interface language for a message shown before any window: read from
/// the settings without moving or writing them, English otherwise.
fn saved_language() -> appearance::i18n::Locale {
    storage::paths::Dirs::locate()
        .and_then(|dirs| std::fs::read(dirs.settings()).ok())
        .and_then(|bytes| serde_json::from_slice::<storage::settings::Settings>(&bytes).ok())
        .map(|settings| settings.language)
        .unwrap_or_default()
}

/// The app's mark, the one the desktop entry names: the full drawing, or
/// its flat variant when small enough that the shading would blur.
const MARK: &[u8] = include_bytes!("../packaging/icons/youtfast.svg");
const SMALL_MARK: &[u8] = include_bytes!("../packaging/icons/youtfast-small.svg");
const SMALL_BELOW: u32 = 40;

/// The window's icon at `size` pixels, an empty one if the drawing cannot
/// be rendered.
fn app_icon(size: u32) -> egui::IconData {
    let mark = if size < SMALL_BELOW { SMALL_MARK } else { MARK };
    let rgba = render_mark(mark, size).unwrap_or_default();
    if rgba.is_empty() {
        return egui::IconData::default();
    }
    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}

fn render_mark(mark: &[u8], size: u32) -> Option<Vec<u8>> {
    let tree = resvg::usvg::Tree::from_data(mark, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
    let scale = size as f32 / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Some(
        pixmap
            .pixels()
            .iter()
            .flat_map(|pixel| {
                let color = pixel.demultiply();
                [color.red(), color.green(), color.blue(), color.alpha()]
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_keyring_errors_keep_their_recovery_instructions() {
        for message in [
            "The system keyring is locked. Unlock it to stay signed in.",
            "The system keyring is unavailable, so the session cannot be kept.",
            "The system keyring did not respond.",
            "This session is too large for the Windows Credential Manager (2560 bytes), so it cannot be kept there.",
            "yt-dlp is not installed for Python, so the browser cannot be read.",
        ] {
            assert_eq!(safe_error(message), message);
            assert_eq!(safe_error(&safe_error(message)), message);
        }
        assert_eq!(
            safe_error("private-profile python-secretstorage synthetic-secret"),
            "Install python-secretstorage, unlock the desktop keyring, and retry browser sign-in."
        );
    }

    #[test]
    fn logs_and_user_errors_never_echo_external_activity_or_credentials() {
        let sensitive = "video123456 private-account browser:private-profile https://youtube.com/watch?v=video123456 Cookie: SAPISID=synthetic-secret imported-title";
        for target in [
            "mpv",
            "rustypipe::client",
            "reqwest",
            "youtfast::backend",
            "youtfast::app::watch",
            "egui_extras::loaders::http_loader",
        ] {
            for level in [
                log::Level::Info,
                log::Level::Warn,
                log::Level::Debug,
                log::Level::Trace,
            ] {
                let message = format!("loading the details of {sensitive} failed");
                use log::Log;
                #[derive(Default)]
                struct Capture(std::sync::Mutex<Vec<String>>);
                impl Log for Capture {
                    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
                        true
                    }
                    fn log(&self, record: &log::Record<'_>) {
                        let message = record.args().to_string();
                        self.0.lock().unwrap().push(
                            redact_log(record, &message)
                                .map_or(message, |redacted| redacted.into_owned()),
                        );
                    }
                    fn flush(&self) {}
                }
                let capture = Capture::default();
                capture.log(
                    &log::Record::builder()
                        .target(target)
                        .level(level)
                        .args(format_args!("{message}"))
                        .build(),
                );
                let redacted = capture.0.lock().unwrap()[0].clone();
                for secret in sensitive.split_whitespace() {
                    assert!(!redacted.contains(secret));
                }
                assert!(!redacted.is_empty());
            }
        }
        let controlled = redact_log(
            &log::Record::builder().target("youtfast::backend").build(),
            "could not save the library: synthetic-secret",
        )
        .unwrap();
        assert_eq!(controlled, "could not save the library");
        let error = safe_error(sensitive);
        assert!(error.contains("Sign in again"));
        assert!(
            !error.contains("synthetic-secret")
                && !error.contains("private-profile")
                && !error.contains("https://")
        );
    }

    #[test]
    fn startup_and_certificate_messages_are_kept_and_translated() {
        assert_eq!(safe_error(UNTRUSTED_CERTIFICATE), UNTRUSTED_CERTIFICATE);
        // rustypipe's message for a reqwest error, causes included.
        assert_eq!(
            safe_error(
                "http error: error sending request for url (https://www.youtube.com/youtubei/v1/search?prettyPrint=false): client error (Connect): invalid peer certificate: UnknownIssuer"
            ),
            UNTRUSTED_CERTIFICATE
        );
        for message in [UNTRUSTED_CERTIFICATE, NEEDS_OPENGL] {
            for locale in appearance::i18n::Locale::ALL {
                let translated = appearance::i18n::message(*locale, message);
                assert_eq!(
                    translated == message,
                    *locale == appearance::i18n::Locale::English,
                    "{locale:?}"
                );
            }
        }
    }

    #[test]
    fn only_a_missing_opengl_explains_itself_at_startup() {
        assert!(needs_opengl(&eframe::Error::OpenGL(
            eframe::egui_glow::PainterError::from("egui_glow requires opengl 2.0+. ".to_owned())
        )));
        assert!(!needs_opengl(&eframe::Error::AppCreation(
            "could not safely load the settings".into()
        )));
    }

    #[test]
    fn the_icon_renders_at_both_sizes() {
        for size in [24, 128] {
            let icon = app_icon(size);
            assert_eq!(icon.rgba.len(), (size * size * 4) as usize);
            // The middle is the mark or the disc, never see-through.
            let middle = ((size / 2 * size + size / 2) * 4 + 3) as usize;
            assert_eq!(icon.rgba[middle], 255);
        }
    }
}
