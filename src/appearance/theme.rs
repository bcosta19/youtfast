//! The interface's colours: YouTFast's own dark and light, the palettes
//! the themes folder and the shared presets add, and Omarchy's live
//! palette.
//! fastframe-theme reads the files; [`Palette`] is what a file can set and
//! [`apply`] puts on egui.

use std::borrow::Cow;

use egui::{Color32, Stroke};

/// A palette file from the themes directory.
pub type CustomTheme = fastframe_theme::CustomTheme<Palette>;
/// The palette files, and the Omarchy palette where the desktop has one.
pub type Catalog = fastframe_theme::Catalog<Palette>;

/// The template Omarchy renders `youtfast.json` from, when neither Omarchy
/// nor the user has installed one.
pub const OMARCHY_TEMPLATE: &str = include_str!("../../contrib/omarchy/youtfast.json.tpl");

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Palette {
    pub dark: bool,
    pub window: Color32,
    pub panel: Color32,
    pub surface: Color32,
    pub surface_hover: Color32,
    pub surface_active: Color32,
    pub outline: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub on_accent: Color32,
    pub danger: Color32,
    pub warning: Color32,
    pub overlay: Color32,
    pub shadow: Color32,
    /// ANSI colours for data, the way `ls --color` uses them: LIVE and
    /// errors, watched progress, durations, channels, key hints, links.
    /// `red` and `yellow` follow `danger` and `warning` unless a file says
    /// otherwise (see `derive`).
    pub red: Color32,
    pub green: Color32,
    pub yellow: Color32,
    pub blue: Color32,
    pub magenta: Color32,
    pub cyan: Color32,
}

impl Palette {
    /// YouTFast's dark colours.
    pub fn dark() -> Self {
        Self {
            dark: true,
            window: Color32::from_rgb(0x0f, 0x0f, 0x0f),
            panel: Color32::from_rgb(0x0f, 0x0f, 0x0f),
            surface: Color32::from_rgb(0x27, 0x27, 0x27),
            surface_hover: Color32::from_rgb(0x3f, 0x3f, 0x3f),
            surface_active: Color32::from_rgb(0x4d, 0x4d, 0x4d),
            outline: Color32::from_rgb(0x30, 0x30, 0x30),
            text: Color32::from_rgb(0xf1, 0xf1, 0xf1),
            secondary: Color32::from_rgb(0xaa, 0xaa, 0xaa),
            dim: Color32::from_rgb(0x71, 0x71, 0x71),
            accent: Color32::from_rgb(0xff, 0x00, 0x33),
            accent_hover: Color32::from_rgb(0xff, 0x33, 0x55),
            on_accent: Color32::from_rgb(0xff, 0xff, 0xff),
            danger: Color32::from_rgb(0xff, 0x4e, 0x45),
            warning: Color32::from_rgb(0xf2, 0xc9, 0x4c),
            overlay: Color32::from_rgb(0x21, 0x21, 0x21),
            shadow: Color32::from_black_alpha(140),
            red: Color32::from_rgb(0xff, 0x4e, 0x45),
            green: Color32::from_rgb(0x2b, 0xa6, 0x40),
            yellow: Color32::from_rgb(0xf2, 0xc9, 0x4c),
            blue: Color32::from_rgb(0x3e, 0xa6, 0xff),
            magenta: Color32::from_rgb(0xc7, 0x7d, 0xff),
            cyan: Color32::from_rgb(0x3e, 0xa6, 0xff),
        }
    }

    /// YouTFast's light colours.
    pub fn light() -> Self {
        Self {
            dark: false,
            window: Color32::from_rgb(0xff, 0xff, 0xff),
            panel: Color32::from_rgb(0xff, 0xff, 0xff),
            surface: Color32::from_rgb(0xf2, 0xf2, 0xf2),
            surface_hover: Color32::from_rgb(0xe5, 0xe5, 0xe5),
            surface_active: Color32::from_rgb(0xd9, 0xd9, 0xd9),
            outline: Color32::from_rgb(0xe5, 0xe5, 0xe5),
            text: Color32::from_rgb(0x0f, 0x0f, 0x0f),
            secondary: Color32::from_rgb(0x60, 0x60, 0x60),
            dim: Color32::from_rgb(0x90, 0x90, 0x90),
            accent: Color32::from_rgb(0xff, 0x00, 0x33),
            accent_hover: Color32::from_rgb(0xcc, 0x00, 0x29),
            on_accent: Color32::from_rgb(0xff, 0xff, 0xff),
            danger: Color32::from_rgb(0xcc, 0x00, 0x00),
            warning: Color32::from_rgb(0xb5, 0x81, 0x00),
            overlay: Color32::from_rgb(0xff, 0xff, 0xff),
            shadow: Color32::from_black_alpha(50),
            red: Color32::from_rgb(0xcc, 0x00, 0x00),
            green: Color32::from_rgb(0x10, 0x75, 0x16),
            yellow: Color32::from_rgb(0xb5, 0x81, 0x00),
            blue: Color32::from_rgb(0x06, 0x5f, 0xd4),
            magenta: Color32::from_rgb(0x8e, 0x44, 0xad),
            cyan: Color32::from_rgb(0x06, 0x5f, 0xd4),
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::dark()
    }
}

impl fastframe_theme::Palette for Palette {
    fn base(base: fastframe_theme::Base) -> Self {
        match base {
            fastframe_theme::Base::Dark => Self::dark(),
            fastframe_theme::Base::Light => Self::light(),
        }
    }

    fn set(&mut self, name: &str, color: Color32) -> bool {
        match name {
            "window" => self.window = color,
            "panel" => self.panel = color,
            "surface" => self.surface = color,
            "surface_hover" => self.surface_hover = color,
            "surface_active" => self.surface_active = color,
            "outline" => self.outline = color,
            "text" => self.text = color,
            "secondary" => self.secondary = color,
            "dim" => self.dim = color,
            "accent" => self.accent = color,
            "accent_hover" => self.accent_hover = color,
            "on_accent" => self.on_accent = color,
            "danger" => self.danger = color,
            "warning" => self.warning = color,
            "overlay" => self.overlay = color,
            "shadow" => self.shadow = color,
            "red" => self.red = color,
            "green" => self.green = color,
            "yellow" => self.yellow = color,
            "blue" => self.blue = color,
            "magenta" => self.magenta = color,
            "cyan" => self.cyan = color,
            _ => return false,
        }
        true
    }

    /// A file that gives a base colour but not the ANSI role reuses it:
    /// red and yellow read as danger and warning, links as the accent.
    /// Green and magenta keep the base defaults when not given.
    fn derive(&mut self, given: &std::collections::BTreeSet<&str>) {
        if !given.contains("red") {
            self.red = self.danger;
        }
        if !given.contains("yellow") {
            self.yellow = self.warning;
        }
        if !given.contains("blue") {
            self.blue = self.accent;
        }
        if !given.contains("cyan") {
            self.cyan = self.accent;
        }
    }
}

/// Adds the desktop's palettes to a normal launch: the eight shared
/// palettes, installed into the themes folder as files on the first
/// launch, and Omarchy's on Linux, with the packaged template.
pub fn enable_desktop_themes(catalog: &mut Catalog) {
    catalog.enable_desktop_themes(fastframe_theme::DesktopThemes {
        slug: "youtfast",
        omarchy_template: OMARCHY_TEMPLATE,
        // The template has not changed since it first shipped.
        omarchy_previous_templates: &[],
        presets: true,
    });
}

/// The status line under the theme setting, empty when all is well.
pub fn catalog_status(catalog: &Catalog, selected: Option<&str>) -> Cow<'static, str> {
    use fastframe_theme::{Problem, Status};
    match catalog.status(selected) {
        None => Cow::Borrowed(""),
        Some(Status::Loading) => Cow::Borrowed("Loading palettes…"),
        Some(Status::SelectedUnavailable) => Cow::Borrowed(
            "The selected palette is unavailable; keeping the last usable appearance.",
        ),
        Some(Status::Problem(Problem::Unreadable)) => {
            Cow::Borrowed("The themes folder could not be read.")
        }
        Some(Status::Problem(Problem::TooManyEntries)) => {
            Cow::Borrowed("The themes folder has more than 512 entries.")
        }
        Some(Status::Problem(Problem::TooManyThemes)) => {
            Cow::Borrowed("Only the first 128 palettes in the folder are listed.")
        }
        Some(Status::Problem(Problem::OmarchyUnreadable)) => Cow::Borrowed(
            "The Omarchy palette could not be read; keeping the last usable appearance.",
        ),
        Some(Status::Problem(_)) => Cow::Borrowed("The palettes could not be listed."),
    }
}

/// The corner radius of cards, menus and thumbnails; small for badges.
pub const RADIUS: u8 = 8;
pub const RADIUS_SMALL: u8 = 4;

/// The interface face at a weight, as the siblings name them.
pub fn regular(size: f32) -> egui::FontId {
    fastframe_fonts::Weight::Regular.font_id(size)
}

pub fn medium(size: f32) -> egui::FontId {
    fastframe_fonts::Weight::Medium.font_id(size)
}

pub fn semibold(size: f32) -> egui::FontId {
    fastframe_fonts::Weight::SemiBold.font_id(size)
}

pub fn bold(size: f32) -> egui::FontId {
    fastframe_fonts::Weight::Bold.font_id(size)
}

/// How the desktop renders text, read once per process. Tests take the
/// platform's default instead of asking the desktop.
static TEXT_RENDERING: std::sync::LazyLock<fastframe_text::TextRendering> =
    std::sync::LazyLock::new(|| {
        if cfg!(test) {
            fastframe_text::TextRendering::platform_default()
        } else {
            fastframe_text::detect()
        }
    });

fn text_rendering() -> fastframe_text::TextRendering {
    *TEXT_RENDERING
}

/// Inter at its four weights (or the platform's interface face), the
/// installed faces behind it for other scripts, drawn the way the desktop
/// renders text: the siblings' setup.
pub fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions(true));
}

fn font_definitions(system_fallbacks: bool) -> egui::FontDefinitions {
    let mut fonts = fastframe_fonts::FontSetup::default()
        .system_fallbacks(system_fallbacks)
        .definitions();
    text_rendering().apply_to(&mut fonts);
    fonts
}

/// The installed faces for the scripts Inter does not draw, looked for on
/// a thread of their own. Finding them probes every installed font, which
/// takes a second or more when the files are not in memory, so the window
/// opens with the rest of [`install_fonts`]'s setup and they follow.
pub struct SystemFonts {
    found: std::sync::mpsc::Receiver<egui::FontDefinitions>,
    /// Woken when the search ends after the window opened.
    waker: std::sync::Arc<std::sync::OnceLock<egui::Context>>,
}

impl SystemFonts {
    /// Starts looking; called before the window is made, so the search
    /// overlaps its setup.
    pub fn find() -> Self {
        let (sender, fonts) = Self::channel();
        let wake = std::sync::Arc::clone(&fonts.waker);
        // A thread that cannot start leaves the channel disconnected, and
        // `poll` then searches on the caller's thread.
        let _ = std::thread::Builder::new()
            .name("youtfast-fonts".into())
            .spawn(move || {
                if sender.send(font_definitions(true)).is_ok()
                    && let Some(ctx) = wake.get()
                {
                    ctx.request_repaint();
                }
            });
        fonts
    }

    fn channel() -> (std::sync::mpsc::SyncSender<egui::FontDefinitions>, Self) {
        let (sender, found) = std::sync::mpsc::sync_channel(1);
        let fonts = Self {
            found,
            waker: std::sync::Arc::default(),
        };
        (sender, fonts)
    }

    /// Gives `ctx` every font if the search is over, else everything but
    /// the installed faces. Returns whether the fonts are complete.
    pub fn install(&self, ctx: &egui::Context) -> bool {
        // Known before looking, so a search ending in between still wakes
        // the window.
        let _ = self.waker.set(ctx.clone());
        if self.poll(ctx) {
            return true;
        }
        ctx.set_fonts(font_definitions(false));
        false
    }

    /// Swaps in the complete fonts once found; true when they are in.
    pub fn poll(&self, ctx: &egui::Context) -> bool {
        match self.found.try_recv() {
            Ok(fonts) => ctx.set_fonts(fonts),
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => install_fonts(ctx),
        }
        // New fonts take effect from the next pass.
        ctx.request_repaint();
        true
    }
}

/// Applies the palette to egui's own widgets so dialogs, menus, and text
/// fields agree with the views, with the siblings' corners and spacing.
pub fn apply(ctx: &egui::Context, palette: &Palette) {
    let (theme, mut visuals) = if palette.dark {
        (egui::Theme::Dark, egui::Visuals::dark())
    } else {
        (egui::Theme::Light, egui::Visuals::light())
    };
    // First: `set_visuals` writes the style of the theme in effect.
    ctx.set_theme(theme);
    visuals.dark_mode = palette.dark;
    text_rendering().apply_to_visuals(&mut visuals);
    visuals.panel_fill = palette.panel;
    visuals.window_fill = palette.overlay;
    visuals.extreme_bg_color = palette.surface;
    visuals.faint_bg_color = palette.surface;
    visuals.code_bg_color = palette.surface;
    visuals.override_text_color = Some(palette.text);
    visuals.weak_text_color = Some(palette.secondary);
    visuals.hyperlink_color = palette.blue;
    visuals.selection.bg_fill = palette.accent.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, palette.accent);
    visuals.window_stroke = Stroke::new(1.0, palette.outline);
    visuals.window_corner_radius = egui::CornerRadius::same(RADIUS + 2);
    visuals.menu_corner_radius = egui::CornerRadius::same(RADIUS);
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: palette.shadow,
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: palette.shadow,
    };
    let corner = egui::CornerRadius::same(RADIUS_SMALL + 2);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = corner;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke = Stroke::new(1.0, palette.text);
        widget.expansion = 0.0;
    }
    visuals.widgets.noninteractive.corner_radius = corner;
    visuals.widgets.noninteractive.bg_fill = palette.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.outline);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.inactive.bg_fill = palette.surface;
    visuals.widgets.inactive.weak_bg_fill = palette.surface;
    visuals.widgets.hovered.bg_fill = palette.surface_hover;
    visuals.widgets.hovered.weak_bg_fill = palette.surface_hover;
    visuals.widgets.active.bg_fill = palette.surface_active;
    visuals.widgets.active.weak_bg_fill = palette.surface_active;
    visuals.widgets.open.bg_fill = palette.surface_hover;
    visuals.widgets.open.weak_bg_fill = palette.surface_hover;
    visuals.text_cursor.stroke = Stroke::new(2.0, palette.accent);
    visuals.striped = false;
    visuals.slider_trailing_fill = true;
    visuals.handle_shape = egui::style::HandleShape::Circle;
    ctx.set_visuals(visuals);
    ctx.all_styles_mut(|style| {
        use egui::FontFamily::{Monospace, Proportional};
        use egui::{FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Small, FontId::new(12.0, Proportional)),
            (TextStyle::Body, FontId::new(14.0, Proportional)),
            (TextStyle::Button, FontId::new(14.0, Proportional)),
            (TextStyle::Heading, FontId::new(20.0, Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, Monospace)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size = egui::vec2(40.0, 28.0);
        style.spacing.menu_margin = egui::Margin::same(6);
        style.spacing.window_margin = egui::Margin::same(16);
        style.spacing.scroll = egui::style::ScrollStyle {
            bar_width: 8.0,
            floating_width: 6.0,
            floating_allocated_width: 0.0,
            handle_min_length: 28.0,
            bar_inner_margin: 3.0,
            bar_outer_margin: 2.0,
            dormant_background_opacity: 0.0,
            dormant_handle_opacity: 0.0,
            active_background_opacity: 0.0,
            active_handle_opacity: 0.55,
            interact_handle_opacity: 0.85,
            foreground_color: true,
            ..egui::style::ScrollStyle::floating()
        };
        style.interaction.selectable_labels = true;
        style.interaction.tooltip_delay = 0.4;
        style.animation_time = 0.12;
        style.url_in_tooltip = false;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_palette_file_overrides_its_base() {
        let palette: Palette = fastframe_theme::parse_palette(
            r##"{"base":"light","colors":{"accent":"#907aa9","surface":"#102030"}}"##,
        )
        .unwrap();
        assert!(!palette.dark);
        assert_eq!(palette.window, Palette::light().window);
        assert_eq!(palette.accent, Color32::from_rgb(0x90, 0x7a, 0xa9));
        assert_eq!(palette.surface, Color32::from_rgb(0x10, 0x20, 0x30));
    }

    #[test]
    fn an_unknown_colour_is_rejected() {
        assert_eq!(
            fastframe_theme::parse_palette::<Palette>(r##"{"colors":{"typo":"#ffffff"}}"##)
                .unwrap_err(),
            "unknown color: typo"
        );
    }

    #[test]
    fn the_omarchy_template_fills_the_ansi_roles() {
        let colors = "mode\tdark\nbackground\t#101418\nforeground\t#e8eef2\
            \naccent\t#4f9fdd\nred\t#e05a5a\ngreen\t#7fb069\nyellow\t#d9a440\
            \nblue\t#5b9bd5\nmagenta\t#b48ead\ncyan\t#6fb3b8\n";
        let rendered =
            fastframe_theme::omarchy::render_seed::<Palette>(OMARCHY_TEMPLATE, colors).unwrap();
        let palette: Palette = fastframe_theme::parse_palette(&rendered).unwrap();
        assert_eq!(palette.window, Color32::from_rgb(0x10, 0x14, 0x18));
        assert_eq!(palette.green, Color32::from_rgb(0x7f, 0xb0, 0x69));
        assert_eq!(palette.magenta, Color32::from_rgb(0xb4, 0x8e, 0xad));
        assert_eq!(palette.blue, Color32::from_rgb(0x5b, 0x9b, 0xd5));
    }

    #[test]
    fn the_ansi_roles_derive_from_the_base_colours() {
        // A file that leaves them out gets red and yellow from danger and
        // warning, links from the accent, and keeps the base's green and
        // magenta.
        let palette: Palette = fastframe_theme::parse_palette(
            r##"{"colors":{"danger":"#112233","warning":"#445566","accent":"#778899"}}"##,
        )
        .unwrap();
        assert_eq!(palette.red, Color32::from_rgb(0x11, 0x22, 0x33));
        assert_eq!(palette.yellow, Color32::from_rgb(0x44, 0x55, 0x66));
        assert_eq!(palette.blue, Color32::from_rgb(0x77, 0x88, 0x99));
        assert_eq!(palette.cyan, Color32::from_rgb(0x77, 0x88, 0x99));
        assert_eq!(palette.green, Palette::dark().green);
        assert_eq!(palette.magenta, Palette::dark().magenta);
        // A file that gives them keeps them.
        let given: Palette =
            fastframe_theme::parse_palette(r##"{"colors":{"red":"#aabbcc"}}"##).unwrap();
        assert_eq!(given.red, Color32::from_rgb(0xaa, 0xbb, 0xcc));
    }

    #[test]
    fn the_omarchy_hook_is_the_crate_one() {
        assert_eq!(
            include_str!("../../contrib/omarchy/youtfast-theme"),
            fastframe_theme::omarchy::hook_script("youtfast")
        );
    }

    fn fonts_in(ctx: &egui::Context) -> egui::FontDefinitions {
        // Fonts given to egui take effect from the next pass.
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        ctx.fonts(|fonts| fonts.definitions().clone())
    }

    #[test]
    fn the_window_opens_without_installed_faces_and_takes_them_once_found() {
        let ctx = egui::Context::default();
        let (sender, fonts) = SystemFonts::channel();
        assert!(!fonts.install(&ctx));
        assert!(!fonts.poll(&ctx));
        assert!(fonts_in(&ctx) == font_definitions(false));
        // What the search found, told apart by a family of its own.
        let mut found = font_definitions(false);
        found.families.insert(
            egui::FontFamily::Name("found".into()),
            vec![fastframe_fonts::INTER_REGULAR.to_owned()],
        );
        sender.send(found.clone()).unwrap();
        assert!(fonts.poll(&ctx));
        assert!(fonts_in(&ctx) == found);
    }

    #[test]
    fn a_search_that_never_answers_is_done_on_the_spot() {
        let ctx = egui::Context::default();
        let (sender, fonts) = SystemFonts::channel();
        drop(sender);
        assert!(fonts.install(&ctx));
        assert!(fonts_in(&ctx) == font_definitions(true));
    }
}
