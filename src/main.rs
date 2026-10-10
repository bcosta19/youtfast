//! YouTFast: YouTube, native and fast.

mod account;
mod alerts;
mod app;
mod backend;
mod icons;
mod innertube;
mod instance;
mod library;
mod model;
mod mpv;
mod now_playing;
mod paths;
mod settings;
mod takeout;
mod theme;
mod thumbs;
mod ui;
mod util;
mod youtube;

use std::sync::Arc;

use clap::Parser;

#[derive(Parser)]
#[command(version, about, args_conflicts_with_subcommands = true)]
struct Cli {
    /// A YouTube link to open, or something to search for.
    open: Option<String>,
    /// Log debug messages.
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

fn main() -> eframe::Result {
    let cli = Cli::parse();
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
    if let Err(error) = fastframe_log::Logging::new("youtfast", env!("CARGO_PKG_VERSION"))
        .filter(filter)
        .init()
    {
        eprintln!("could not start logging: {error}");
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("YouTFast")
            .with_app_id("youtfast")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([480.0, 360.0]),
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
    // The app holds the instance guard for the run; the creator closure is
    // dropped as soon as it returns, so it cannot keep it.
    let guard = Arc::new(guard);
    let open = cli.open;
    eframe::run_native(
        "YouTFast",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, open, guard)?))),
    )
}
