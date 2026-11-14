//! A glyph for every character people write in comments, titles,
//! descriptions and channel names, on every platform.
//!
//! Inter draws Latin, Greek and Cyrillic, and fastframe-fonts borrows an
//! installed face for each other script it knows. What neither had drew as
//! egui's replacement box:
//!
//! - Emoji. egui's own emoji face is an old subset of Noto Emoji, without
//!   every emoji from 2016 on (🤣, 🥹, 🫠, 🤌, 🥳…). [`NOTO_EMOJI`], the
//!   complete monochrome Noto Emoji (3.002, Unicode 15), sits right behind
//!   the interface face. The variation selector, joiners and skin-tone
//!   modifiers inside a sequence never draw on their own: the egui fork
//!   lays an emoji sequence out one emoji wide and draws a single glyph for
//!   it, the face's ligature or else its first emoji.
//! - "Small caps", styled letters, symbols and kaomoji (ᴛʜɪs, ꜰ, 𝓗𝓮𝓵𝓵𝓸,
//!   ⓑⓤⓑⓑⓛⓔ, ʕ•ᴥ•ʔ, ᕕ( ᐛ )ᕗ, ༼ ◕_◕ ༽, (╯°□°)╯︵ ┻━┻, ꒰ᐢ. .ᐢ꒱), from
//!   blocks the installed faces cover on some machines only.
//!   [`NOTO_FALLBACK`] holds those blocks, taken from Noto
//!   (`tools/fonts.py`), behind every other face. Writing systems (CJK,
//!   Indic, Arabic…) still come from the installed faces.
//!
//! Both are bundled, so no system font is needed. Over the monochrome emoji,
//! fastframe-emoji's plugin ([`install_colour_emoji`]) paints each emoji in
//! colour from the platform's emoji font: Apple Color Emoji on macOS, Segoe
//! UI Emoji on Windows, the desktop's colour emoji font on Linux, joined as
//! the platform joins sequences (skin tones, families, flags, keycaps).
//! Where no colour font is installed the monochrome glyph stays.

use std::sync::Arc;

/// Noto Emoji 3.002, monochrome, from Google Fonts (`ofl/notoemoji`),
/// SIL Open Font License 1.1 (`assets/fonts/NotoEmoji-LICENSE.txt`).
pub const NOTO_EMOJI: &[u8] = include_bytes!("../../../assets/fonts/NotoEmoji.ttf");

/// Symbol and letter blocks of Noto Sans, Noto Sans Symbols, Symbols 2 and
/// Math, and the kaomoji's few characters of Canadian syllabics, Tibetan,
/// Yi and Kannada, at regular weight, made by `tools/fonts.py`; SIL Open
/// Font License 1.1 (`assets/fonts/NotoFallback-LICENSE.txt`).
pub const NOTO_FALLBACK: &[u8] = include_bytes!("../../../assets/fonts/NotoFallback.ttf");

/// The font data key of [`NOTO_EMOJI`].
pub const EMOJI_KEY: &str = "youtfast-noto-emoji";

/// The font data key of [`NOTO_FALLBACK`].
pub const FALLBACK_KEY: &str = "youtfast-noto-fallback";

/// egui draws its own Noto Emoji at this size, so an emoji keeps the width
/// it had beside the text: the two faces share their advance (1.27 em).
const EMOJI_SCALE: f32 = 0.81;

/// Puts [`NOTO_FALLBACK`]'s baseline on Inter's, as fastframe-fonts does
/// for the faces it borrows: epaint centres a fallback's line box on the
/// interface face's. Inter's baseline is 0.3638 em above its box's centre
/// (ascender 1984, descender -494 of 2048), the fallback's 0.388 em (Noto
/// Sans' 1069 and -293 of 1000, which `tools/fonts.py` keeps).
const FALLBACK_BASELINE: f32 = (1984.0 / 2048.0 - 0.5 * (2478.0 / 2048.0)) - (1.069 - 0.5 * 1.362);

/// Adds the bundled faces to `setup`: [`NOTO_EMOJI`] right behind the
/// interface face in every family.
pub fn register(setup: fastframe_fonts::FontSetup) -> fastframe_fonts::FontSetup {
    let emoji = egui::FontData::from_static(NOTO_EMOJI).tweak(egui::FontTweak {
        scale: EMOJI_SCALE,
        ..Default::default()
    });
    setup.companion(EMOJI_KEY, Arc::new(emoji))
}

/// Puts [`NOTO_FALLBACK`] last in every family, behind the installed
/// faces, so it draws only what nothing else does.
pub fn append_fallback(fonts: &mut egui::FontDefinitions) {
    let fallback = egui::FontData::from_static(NOTO_FALLBACK).tweak(egui::FontTweak {
        y_offset_factor: FALLBACK_BASELINE,
        ..Default::default()
    });
    fonts
        .font_data
        .insert(FALLBACK_KEY.to_owned(), Arc::new(fallback));
    for family in fonts.families.values_mut() {
        family.push(FALLBACK_KEY.to_owned());
    }
}

/// Colours the emoji of every text `ctx` draws, in the platform's style.
/// The colour fonts are found and mapped on a thread of their own; until
/// they are, and wherever there is none, the monochrome glyphs show.
pub fn install_colour_emoji(ctx: &egui::Context) {
    fastframe_emoji::EmojiSetup::default()
        .system(true)
        .install();
    let _ = std::thread::Builder::new()
        .name("youtfast-emoji".into())
        .spawn(fastframe_emoji::warm_up);
    ctx.add_plugin(fastframe_emoji::EmojiPlugin::default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use skrifa::MetadataProvider as _;

    /// What people write in comments and names: emoji old and new, the
    /// sequences, "small caps", styled letters, symbols and kaomoji.
    const SAMPLES: &[&str] = &[
        "😂🤣❤️👍🙏🔥✨🥹🫠🫶🩷🪿🫨🤌🫡🥲🥳🤯😭💀",
        "👍🏽 👋🏻 👨\u{200D}👩\u{200D}👧 🏳️\u{200D}🌈 👩🏽\u{200D}💻 ❤️\u{200D}🔥",
        "🇧🇷 🇵🇹 🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F} 1️⃣ #️⃣ ☠️ ✌️ ☺️",
        "( ͡° ͜ʖ ͡°) ʕ•ᴥ•ʔ (╯°□°)╯ ┻━┻ ┬─┬ ಠ_ಠ ᕕ( ᐛ )ᕗ ⊂(◉‿◉) (◕‿◕✿)",
        "༼ ◕_◕ ༽ ⸜( ˃ ᵕ ˂ )⸝♡ ꒰ᐢ. .ᐢ꒱ ✧ ⋆°✩ ⟡ ˙ᵕ˙ (•̀ᴗ-)✧ ░▒▓█ ⣿⡿⠿",
        "ᴛʜɪs ɪs sᴍᴀʟʟ ᴄᴀᴘs ꜰᴀɴᴄʏ ꜱᴛʏʟᴇ ʇxǝʇ ⓑⓤⓑⓑⓛⓔ 🅱🅾🆇 Ⅻ ℌ",
        "𝓗𝓮𝓵𝓵𝓸 𝐁𝐨𝐥𝐝 𝔉𝔯𝔞𝔨𝔱𝔲𝔯 𝕯𝖔𝖚𝖇𝖑𝖊 𝙈𝙤𝙣𝙤 ¹²³ ₁₂₃ ᵃᵇᶜ",
        "➡ ⬆ ⇒ ✓ ✔ ✗ ❌ ♥ ❥ ☆ ★ ⚡ ✿ ❀ ‼ ∞ ≈ ≠ ₹ € ¥ ₿ ‰ ⏰ ⌘",
        "™ © ® ’“”–—…•·° ç ã é ñ ő Ж Ω",
    ];

    /// Kaomoji with Japanese in them: kana, the half-width and the vertical
    /// forms come from the installed Japanese face, which Windows and macOS
    /// always have and fastframe-fonts looks for, like every other script.
    const JAPANESE: &[&str] = &[
        "¯\\_(ツ)_/¯",
        "༼ つ ◕_◕ ༽つ",
        "✧･ﾟ: *✧･ﾟ:*",
        "⸜(｡˃ ᵕ ˂ )⸝",
        "(╯°□°)╯︵ ┻━┻",
    ];

    /// What layout does not draw a glyph of its own for: invisible
    /// characters, and the marks, selectors, skin tones and tags that ride
    /// on the character before them.
    fn rides_along(c: char) -> bool {
        c.is_whitespace()
            || matches!(c,
                '\u{0300}'..='\u{036F}' | '\u{20D0}'..='\u{20FF}' | '\u{FE00}'..='\u{FE0F}'
                | '\u{200B}'..='\u{200F}' | '\u{1F3FB}'..='\u{1F3FF}' | '\u{E0000}'..='\u{E007F}')
    }

    /// The face of `family` that draws `c`, as epaint picks it: the first
    /// whose character map has it.
    fn face_for(
        fonts: &egui::FontDefinitions,
        family: &egui::FontFamily,
        c: char,
    ) -> Option<String> {
        fonts.families[family].iter().find_map(|name| {
            let data = &fonts.font_data[name];
            let font = skrifa::FontRef::from_index(&data.font, data.index).ok()?;
            font.charmap()
                .map(c)
                .is_some_and(|glyph| glyph.to_u32() != 0)
                .then(|| name.clone())
        })
    }

    /// The fonts with nothing borrowed from the machine: what every
    /// platform is sure to have.
    fn bundled_only() -> egui::FontDefinitions {
        let mut fonts =
            register(fastframe_fonts::FontSetup::default().system_fallbacks(false)).definitions();
        append_fallback(&mut fonts);
        fonts
    }

    #[test]
    fn every_character_of_the_samples_has_a_face_on_every_platform() {
        let fonts = bundled_only();
        for weight in fastframe_fonts::Weight::ALL {
            let family = weight.family();
            let missing: Vec<String> = SAMPLES
                .iter()
                .flat_map(|sample| sample.chars())
                .filter(|c| !rides_along(*c) && face_for(&fonts, &family, *c).is_none())
                .map(|c| format!("{c} U+{:04X}", c as u32))
                .collect();
            assert!(missing.is_empty(), "{weight:?} draws boxes for {missing:?}");
        }
    }

    #[test]
    fn only_japanese_is_left_to_the_installed_faces() {
        let fonts = bundled_only();
        let family = egui::FontFamily::Proportional;
        for c in JAPANESE.iter().flat_map(|sample| sample.chars()) {
            if rides_along(c) || face_for(&fonts, &family, c).is_some() {
                continue;
            }
            assert!(
                matches!(c, '\u{3040}'..='\u{30FF}' | '\u{FE30}'..='\u{FE4F}' | '\u{FF61}'..='\u{FF9F}'),
                "{c} U+{:04X}",
                c as u32
            );
        }
    }

    #[test]
    fn emoji_come_from_the_complete_face_and_text_stays_inters() {
        let fonts = bundled_only();
        let family = egui::FontFamily::Proportional;
        for c in ['😀', '🤣', '🥹', '🫠', '🫨'] {
            assert_eq!(
                face_for(&fonts, &family, c).as_deref(),
                Some(EMOJI_KEY),
                "{c}"
            );
        }
        for c in ['a', 'ç', 'ã', '7', '©', '™', 'Ж'] {
            assert_eq!(
                face_for(&fonts, &family, c).as_deref(),
                Some(fastframe_fonts::INTER_REGULAR),
                "{c}"
            );
        }
    }

    #[test]
    fn the_emoji_face_follows_the_interface_face_and_the_fallback_comes_last() {
        let mut fonts = register(fastframe_fonts::FontSetup::default())
            .system_fallbacks(false)
            .definitions();
        append_fallback(&mut fonts);
        for weight in fastframe_fonts::Weight::ALL {
            let family = &fonts.families[&weight.family()];
            assert_eq!(family[1], EMOJI_KEY, "{weight:?}");
            assert_eq!(family.last().map(String::as_str), Some(FALLBACK_KEY));
        }
        assert_eq!(
            fonts.families[&egui::FontFamily::Monospace]
                .last()
                .map(String::as_str),
            Some(FALLBACK_KEY)
        );
    }

    /// A face's baseline above the centre of its line box, in ems, from
    /// the metrics epaint lays it out with.
    fn baseline_center(bytes: &[u8]) -> f32 {
        let font = skrifa::FontRef::new(bytes).expect("a font");
        let metrics = font.metrics(
            skrifa::instance::Size::unscaled(),
            skrifa::instance::LocationRef::default(),
        );
        let height = metrics.ascent - metrics.descent + metrics.leading;
        (metrics.ascent - 0.5 * height) / f32::from(metrics.units_per_em)
    }

    #[test]
    fn the_fallback_sits_on_inters_baseline() {
        let shift = baseline_center(fastframe_fonts::INTER) - baseline_center(NOTO_FALLBACK);
        assert!(
            (shift - FALLBACK_BASELINE).abs() < 1e-4,
            "{shift} vs {FALLBACK_BASELINE}"
        );
    }

    #[test]
    fn an_emoji_keeps_the_width_egui_gave_it() {
        let advance = |bytes: &[u8]| {
            let font = skrifa::FontRef::new(bytes).expect("a font");
            let glyph = font.charmap().map('😀').expect("😀");
            let metrics = font.glyph_metrics(
                skrifa::instance::Size::unscaled(),
                skrifa::instance::LocationRef::default(),
            );
            let units = f32::from(
                font.metrics(
                    skrifa::instance::Size::unscaled(),
                    skrifa::instance::LocationRef::default(),
                )
                .units_per_em,
            );
            metrics.advance_width(glyph).expect("an advance") / units
        };
        let defaults = egui::FontDefinitions::default();
        if let Some(builtin) = defaults.font_data.get("NotoEmoji-Regular") {
            assert!(
                (advance(&builtin.font) * builtin.tweak.scale - advance(NOTO_EMOJI) * EMOJI_SCALE)
                    .abs()
                    < 1e-3
            );
        }
    }
}
