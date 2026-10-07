//! One font for everything: the desktop's monospace, so the app shares
//! the terminal's face. On Linux `fc-match` names the file once at
//! start; anywhere else, or when it cannot be read, egui's bundled Hack
//! leads both families so the app stays mono.

use egui::{FontData, FontDefinitions, FontFamily};

/// The name the face is registered under.
const FAMILY: &str = "youtfast-mono";
/// A font file is never this large; anything bigger is not a font.
const LIMIT: u64 = 20 * 1024 * 1024;

/// Registers the desktop's monospace font for both families, falling
/// back to egui's bundled Hack. Defaults stay after it as glyph
/// fallback (box drawing, braille, the `❯` prompt).
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let family = match system_monospace() {
        Some(data) => {
            fonts
                .font_data
                .insert(FAMILY.to_owned(), FontData::from_owned(data).into());
            FAMILY
        }
        None => {
            log::info!("no desktop monospace font found; using the bundled Hack");
            // Hack is egui's bundled monospace face; lead Proportional
            // with it so everything is still mono.
            "Hack"
        }
    };
    for family_name in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family_name)
            .or_default()
            .insert(0, family.to_owned());
    }
    ctx.set_fonts(fonts);
}

/// The file `fc-match` resolves `monospace` to, when it is a readable
/// font file.
#[cfg(target_os = "linux")]
fn system_monospace() -> Option<Vec<u8>> {
    let output = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", "monospace"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = std::path::PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    read_font(&path)
}

#[cfg(not(target_os = "linux"))]
fn system_monospace() -> Option<Vec<u8>> {
    None
}

/// Reads a .ttf/.otf/.ttc file; anything else or unreadable is refused.
fn read_font(path: &std::path::Path) -> Option<Vec<u8>> {
    let font = path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(font.as_str(), "ttf" | "otf" | "ttc") {
        return None;
    }
    if path.metadata().ok()?.len() > LIMIT {
        return None;
    }
    std::fs::read(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_font_files_are_read() {
        assert!(read_font(std::path::Path::new("x.exe")).is_none());
        assert!(read_font(std::path::Path::new("x")).is_none());
        // Missing files of the right kind are refused too.
        assert!(read_font(std::path::Path::new("definitely/not/here.ttf")).is_none());
    }
}
