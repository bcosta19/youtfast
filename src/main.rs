//! YouTFast: YouTube, native and fast.

mod account;
mod app;
mod backend;
mod innertube;
mod library;
mod model;
mod mpv;
mod paths;
mod settings;
mod takeout;
mod ui;
mod util;
mod youtube;

use clap::Parser;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// A YouTube link to open, or something to search for.
    open: Option<String>,
    /// Log debug messages.
    #[arg(long)]
    verbose: bool,
}

fn main() -> eframe::Result {
    let cli = Cli::parse();
    let filter = if cli.verbose {
        "youtfast=debug,mpv=debug"
    } else {
        "youtfast=info,mpv=warn"
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
    let open = cli.open;
    eframe::run_native(
        "YouTFast",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, open)?))),
    )
}
