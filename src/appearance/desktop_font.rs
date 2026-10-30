//! The interface font the Linux desktop is configured with.
//!
//! fastframe-fonts asks fontconfig for `system-ui`, which most distributions
//! do not alias to the font the desktop actually uses: on Omarchy
//! (Hyprland, no GNOME session) it answers Liberation Sans while GNOME's
//! settings say Adwaita Sans. So on Linux the desktop's own setting comes
//! first, read from (in this order, KDE's file first on a KDE session):
//!
//! 1. `org.gnome.desktop.interface font-name`, through `gsettings`,
//! 2. `gtk-font-name` in GTK 4's, then GTK 3's `settings.ini`,
//! 3. `font=` in KDE's `kdeglobals`.
//!
//! The family is then looked up with `fc-match`, one weight at a time like
//! the library does, and the faces replace the interface face in the
//! `FontDefinitions` under the library's own keys. When nothing is
//! configured, or the family is not installed, the caller falls back to the
//! library's `system-ui`, then to Inter.
//!
//! Everything here runs on the font thread (`theme::SystemFonts`), never on
//! the interface thread, and the answer is kept for the life of the process.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::sync::{Arc, OnceLock};

use egui::{FontData, FontDefinitions};
use fastframe_fonts::Weight;

/// The desktop's face, one per [`Weight::ALL`].
#[derive(Debug)]
pub struct Interface {
    pub family: String,
    faces: Vec<Arc<FontData>>,
}

static FACE: OnceLock<Option<Interface>> = OnceLock::new();

/// The configured face, looked for once per process. Blocks while the
/// first lookup runs (a few `gsettings` and `fc-match` runs, each stopped
/// after a second): call it only off the interface thread.
pub fn interface() -> Option<&'static Interface> {
    FACE.get_or_init(|| {
        let found = lookup();
        match &found {
            Some(face) => log::info!("desktop interface font: {}", face.family),
            None => log::info!("no desktop interface font found"),
        }
        found
    })
    .as_ref()
}

/// The face if the lookup has already ended, without starting it.
pub fn cached() -> Option<&'static Interface> {
    FACE.get().and_then(Option::as_ref)
}

impl Interface {
    /// Puts the desktop's face where the interface face is: the library's
    /// font data keys of each weight it registered.
    pub fn apply(&self, fonts: &mut FontDefinitions) {
        for (weight, face) in Weight::ALL.iter().zip(&self.faces) {
            if fonts.font_data.contains_key(weight.name()) {
                fonts
                    .font_data
                    .insert(weight.name().to_owned(), Arc::clone(face));
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn lookup() -> Option<Interface> {
    None
}

#[cfg(target_os = "linux")]
fn lookup() -> Option<Interface> {
    configured_families()
        .into_iter()
        .find_map(|family| resolve(&family))
}

/// The family the desktop is configured with, for the issue report.
pub fn configured_family() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        configured_families().into_iter().next()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Every configured family, the most likely first.
#[cfg(target_os = "linux")]
fn configured_families() -> Vec<String> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| home.map(|home| home.join(".config")));
    let read = |relative: &str| {
        config
            .as_ref()
            .and_then(|config| std::fs::read_to_string(config.join(relative)).ok())
    };
    let kde = std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| {
        desktop
            .split(':')
            .any(|part| part.eq_ignore_ascii_case("kde"))
    });
    let gnome = || {
        run(
            "gsettings",
            &["get", "org.gnome.desktop.interface", "font-name"],
        )
        .and_then(|text| parse_gsettings(&text))
    };
    let gtk = || {
        ["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"]
            .iter()
            .find_map(|file| read(file).and_then(|text| parse_gtk_ini(&text)))
    };
    let plasma = || read("kdeglobals").and_then(|text| parse_kdeglobals(&text));
    let mut families = Vec::new();
    if kde {
        families.extend(plasma());
        families.extend(gnome());
        families.extend(gtk());
    } else {
        families.extend(gnome());
        families.extend(gtk());
        families.extend(plasma());
    }
    families.dedup();
    families
}

/// How long each external tool gets, all of a lookup's runs together.
#[cfg(target_os = "linux")]
const LIMIT: std::time::Duration = std::time::Duration::from_secs(1);

/// What a program printed, if it ended well within [`LIMIT`]; otherwise it
/// is stopped.
#[cfg(target_os = "linux")]
fn run(program: &str, args: &[&str]) -> Option<String> {
    let child = spawn(program, args)?;
    printed(child, std::time::Instant::now() + LIMIT)
}

#[cfg(target_os = "linux")]
fn spawn(program: &str, args: &[&str]) -> Option<std::process::Child> {
    std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
}

#[cfg(target_os = "linux")]
fn printed(mut child: std::process::Child, deadline: std::time::Instant) -> Option<String> {
    use std::io::Read as _;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    if !status.success() {
        return None;
    }
    let mut output = String::new();
    child.stdout.take()?.read_to_string(&mut output).ok()?;
    Some(output)
}

/// The family's four weights, asked of `fc-match` side by side.
#[cfg(target_os = "linux")]
fn resolve(family: &str) -> Option<Interface> {
    const FORMAT: &str = "%{index}|%{family}|%{file}";
    let children: Vec<_> = Weight::ALL
        .iter()
        .map(|weight| {
            let pattern = format!(
                "{}:weight={}:slant=0",
                escape_pattern(family),
                fontconfig_weight(*weight)
            );
            spawn("fc-match", &["-f", FORMAT, &pattern])
        })
        .collect();
    let deadline = std::time::Instant::now() + LIMIT;
    let answers: Vec<Option<(u32, std::path::PathBuf)>> = children
        .into_iter()
        .map(|child| {
            let text = printed(child?, deadline)?;
            parse_fc_match(&text, family)
        })
        .collect();
    let regular = answers.first()?.clone()?;
    // A weight fontconfig could not give from this family takes regular.
    // Kept for the life of the process, as the library keeps its faces: the
    // four weights share one copy of a variable file.
    let mut loaded: Vec<(std::path::PathBuf, &'static [u8])> = Vec::new();
    let mut bytes_of = |path: &std::path::Path| -> Option<&'static [u8]> {
        if let Some((_, bytes)) = loaded.iter().find(|(known, _)| known == path) {
            return Some(bytes);
        }
        let bytes = std::fs::read(path).ok()?;
        if !is_font(&bytes) {
            return None;
        }
        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        loaded.push((path.to_path_buf(), bytes));
        Some(bytes)
    };
    let mut faces = Vec::new();
    for (weight, answer) in Weight::ALL.iter().zip(&answers) {
        let (index, path) = answer.clone().unwrap_or_else(|| regular.clone());
        let (index, path) = match bytes_of(&path) {
            Some(_) => (index, path),
            None => regular.clone(),
        };
        let bytes = bytes_of(&path)?;
        faces.push(face(bytes, index, *weight));
    }
    Some(Interface {
        family: family.to_owned(),
        faces,
    })
}

/// A face of `bytes` at `weight`: a face that varies in weight is set to
/// it, a static one is drawn as it is.
fn face(bytes: &'static [u8], index: u32, weight: Weight) -> Arc<FontData> {
    let mut data = FontData::from_static(bytes);
    data.index = index;
    if has_variations(bytes, index) {
        data.tweak.coords = egui::epaint::text::VariationCoords::new([(b"wght", weight.value())]);
    }
    Arc::new(data)
}

/// fontconfig's weight scale for a [`Weight`].
fn fontconfig_weight(weight: Weight) -> u32 {
    match weight {
        Weight::Regular => 80,
        Weight::Medium => 100,
        Weight::SemiBold => 180,
        Weight::Bold => 200,
    }
}

/// A family name as a fontconfig pattern reads it: its separators escaped.
fn escape_pattern(family: &str) -> String {
    let mut escaped = String::with_capacity(family.len());
    for character in family.chars() {
        if matches!(character, '\\' | '-' | ':' | ',' | '=' | '_') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

/// The face and file in `fc-match` output (`index|families|file`), if the
/// face is of the family asked for: fontconfig answers another one when it
/// does not have that family. A named instance of a variable face sets the
/// index's upper bits; the weight's coordinates are set from the axes.
fn parse_fc_match(output: &str, wanted: &str) -> Option<(u32, std::path::PathBuf)> {
    let mut fields = output.lines().next()?.splitn(3, '|');
    let index: u32 = fields.next()?.trim().parse().ok()?;
    let families = fields.next()?;
    let file = fields.next()?.trim();
    if file.is_empty()
        || !families
            .split(',')
            .any(|family| family.trim().eq_ignore_ascii_case(wanted.trim()))
    {
        return None;
    }
    Some((index & 0xFFFF, std::path::PathBuf::from(file)))
}

/// A TrueType, OpenType or collection file.
fn is_font(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some([0, 1, 0, 0] | b"OTTO" | b"true" | b"ttcf")
    )
}

/// Whether face `index` of the file has an `fvar` table (a variable face),
/// found in the table directory.
fn has_variations(bytes: &[u8], index: u32) -> bool {
    let be16 = |at: usize| -> Option<usize> {
        let pair = bytes.get(at..at + 2)?;
        Some(usize::from(u16::from_be_bytes([pair[0], pair[1]])))
    };
    let be32 = |at: usize| -> Option<usize> {
        let quad = bytes.get(at..at + 4)?;
        usize::try_from(u32::from_be_bytes([quad[0], quad[1], quad[2], quad[3]])).ok()
    };
    let start = if bytes.starts_with(b"ttcf") {
        let Some(offset) = usize::try_from(index)
            .ok()
            .and_then(|index| be32(12 + 4 * index))
        else {
            return false;
        };
        offset
    } else {
        0
    };
    let Some(tables) = be16(start + 4) else {
        return false;
    };
    (0..tables).any(|table| {
        let at = start + 12 + 16 * table;
        bytes.get(at..at + 4) == Some(b"fvar")
    })
}

/// `'Adwaita Sans 11'`, `gsettings get`'s answer, as a family.
fn parse_gsettings(output: &str) -> Option<String> {
    family_of(output.trim().trim_matches('\'').trim_matches('"'))
}

/// `gtk-font-name=Adwaita Sans 11` in a GTK `settings.ini`.
fn parse_gtk_ini(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "gtk-font-name").then(|| family_of(value.trim()))?
    })
}

/// `font=Noto Sans,10,-1,5,50,0,0,0,0,0` in KDE's `kdeglobals` (the
/// `[General]` group; the other groups' `font=` lines are for toolbars and
/// menus).
fn parse_kdeglobals(text: &str) -> Option<String> {
    let mut general = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            general = line == "[General]";
        } else if general && let Some(value) = line.strip_prefix("font=") {
            let family = value.split(',').next()?.trim();
            return (!family.is_empty()).then(|| family.to_owned());
        }
    }
    None
}

/// The family in a Pango font description (`Adwaita Sans Bold 11`): the
/// first of a list, without the size and the style words at the end.
fn family_of(description: &str) -> Option<String> {
    const STYLES: [&str; 14] = [
        "regular",
        "normal",
        "bold",
        "italic",
        "oblique",
        "light",
        "medium",
        "semibold",
        "semi-bold",
        "extrabold",
        "extra-bold",
        "thin",
        "heavy",
        "book",
    ];
    let first = description.split(',').next()?;
    let mut words: Vec<&str> = first.split_whitespace().collect();
    if words.last().is_some_and(|word| word.parse::<f32>().is_ok()) {
        words.pop();
    }
    while words.len() > 1
        && words
            .last()
            .is_some_and(|word| STYLES.contains(&word.to_ascii_lowercase().as_str()))
    {
        words.pop();
    }
    let family = words.join(" ");
    (!family.is_empty()).then_some(family)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gsettings_answers_lose_their_quotes_and_size() {
        assert_eq!(
            parse_gsettings("'Adwaita Sans 11'\n").as_deref(),
            Some("Adwaita Sans")
        );
        assert_eq!(
            parse_gsettings("'Cantarell Bold 11'\n").as_deref(),
            Some("Cantarell")
        );
        assert_eq!(parse_gsettings("''\n"), None);
        assert_eq!(parse_gsettings("'11'\n"), None);
    }

    #[test]
    fn a_gtk_settings_file_names_its_font() {
        let ini = "[Settings]\ngtk-theme-name=Adwaita\ngtk-font-name=Inter Variable 10\n\
                   gtk-cursor-theme-size=24\n";
        assert_eq!(parse_gtk_ini(ini).as_deref(), Some("Inter Variable"));
        assert_eq!(parse_gtk_ini("[Settings]\ngtk-theme-name=x\n"), None);
        assert_eq!(
            parse_gtk_ini("gtk-font-name = Noto Sans, Sans 11").as_deref(),
            Some("Noto Sans")
        );
    }

    #[test]
    fn kdeglobals_gives_the_general_font_only() {
        let text = "[WM]\nfont=Other,9\n[General]\nTerminalApplication=konsole\n\
                    font=Noto Sans,10,-1,5,50,0,0,0,0,0\nfixed=Hack,10\n\
                    [Toolbar]\nfont=Last,8\n";
        assert_eq!(parse_kdeglobals(text).as_deref(), Some("Noto Sans"));
        assert_eq!(parse_kdeglobals("[General]\nfixed=Hack,10\n"), None);
    }

    #[test]
    fn a_family_keeps_its_own_digits_and_words() {
        assert_eq!(
            family_of("Source Sans 3 12").as_deref(),
            Some("Source Sans 3")
        );
        assert_eq!(
            family_of("Noto Sans Medium 11").as_deref(),
            Some("Noto Sans")
        );
        assert_eq!(family_of("Sans").as_deref(), Some("Sans"));
        assert_eq!(family_of("Bold").as_deref(), Some("Bold"));
    }

    #[test]
    fn fc_match_answers_only_count_for_the_family_asked() {
        let answer = "524288|Adwaita Sans|/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf\n";
        assert_eq!(
            parse_fc_match(answer, "adwaita sans"),
            Some((0, "/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf".into()))
        );
        // fontconfig's substitute for a family it does not have.
        let other = "0|Liberation Sans|/usr/share/fonts/liberation/LiberationSans-Regular.ttf\n";
        assert_eq!(parse_fc_match(other, "Adwaita Sans"), None);
        assert_eq!(parse_fc_match("", "x"), None);
    }

    #[test]
    fn family_names_are_escaped_for_fontconfig() {
        assert_eq!(escape_pattern("Adwaita Sans"), "Adwaita Sans");
        assert_eq!(escape_pattern("Noto Sans-X:y"), r"Noto Sans\-X\:y");
    }

    /// A table directory naming `tags`, at `at` in a buffer.
    fn directory(tags: &[&[u8; 4]]) -> Vec<u8> {
        let mut bytes = vec![0, 1, 0, 0];
        bytes.extend((tags.len() as u16).to_be_bytes());
        bytes.extend([0; 6]);
        for tag in tags {
            bytes.extend(*tag);
            bytes.extend([0; 12]);
        }
        bytes
    }

    #[test]
    fn a_variable_face_is_told_by_its_fvar_table() {
        assert!(has_variations(&directory(&[b"cmap", b"fvar"]), 0));
        assert!(!has_variations(&directory(&[b"cmap", b"glyf"]), 0));
        assert!(!has_variations(&[0, 1], 0));
        // A collection points at each face's directory.
        let mut collection = b"ttcf".to_vec();
        collection.extend([0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 16]);
        collection.extend(directory(&[b"fvar"]));
        assert!(has_variations(&collection, 0));
        assert!(!has_variations(&collection, 1));
    }

    #[test]
    fn only_font_files_are_read() {
        assert!(is_font(&directory(&[])));
        assert!(is_font(b"OTTO...."));
        assert!(!is_font(b"<html>"));
    }

    #[test]
    fn the_desktops_face_takes_the_interface_keys_and_no_others() {
        let mut fonts = fastframe_fonts::FontSetup::default()
            .system_fallbacks(false)
            .definitions();
        let before: Vec<_> = fonts.font_data.keys().cloned().collect();
        let marker = |byte: u8| Arc::new(FontData::from_owned(vec![0, 1, 0, 0, byte]));
        let face = Interface {
            family: "Test".into(),
            faces: (1..=4).map(marker).collect(),
        };
        face.apply(&mut fonts);
        for (position, weight) in Weight::ALL.iter().enumerate() {
            assert_eq!(fonts.font_data[weight.name()].font[4], position as u8 + 1);
        }
        let mut after: Vec<_> = fonts.font_data.keys().cloned().collect();
        let mut before = before;
        after.sort();
        before.sort();
        assert_eq!(before, after);
    }
}
