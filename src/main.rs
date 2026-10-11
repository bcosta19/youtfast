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
mod recommend;
mod settings;
mod sponsorblock;
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
