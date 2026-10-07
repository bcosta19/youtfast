//! The interface's colours: YouTube's own dark and light, the palettes the
//! themes folder and the shared presets add, and Omarchy's live palette.
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
pub const OMARCHY_TEMPLATE: &str = include_str!("../contrib/omarchy/youtfast.json.tpl");

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
}

impl Palette {
    /// YouTube's dark colours (youtube.com's dark greys, its brand red).
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
        }
    }

    /// YouTube's light colours.
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
            _ => return false,
        }
        true
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

/// Applies the palette to egui's own widgets so dialogs, menus, and text
/// fields agree with the views. Corner radii and spacing keep egui's own.
pub fn apply(ctx: &egui::Context, palette: &Palette) {
    let (theme, mut visuals) = if palette.dark {
        (egui::Theme::Dark, egui::Visuals::dark())
    } else {
        (egui::Theme::Light, egui::Visuals::light())
    };
    // First: `set_visuals` writes the style of the theme in effect.
    ctx.set_theme(theme);
    visuals.dark_mode = palette.dark;
    visuals.panel_fill = palette.panel;
    visuals.window_fill = palette.overlay;
    visuals.extreme_bg_color = palette.surface;
    visuals.faint_bg_color = palette.surface;
    visuals.code_bg_color = palette.surface;
    visuals.override_text_color = Some(palette.text);
    visuals.weak_text_color = Some(palette.secondary);
    visuals.hyperlink_color = palette.accent;
    visuals.selection.bg_fill = palette.accent.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, palette.accent);
    visuals.window_stroke = Stroke::new(1.0, palette.outline);
    visuals.window_shadow.color = palette.shadow;
    visuals.popup_shadow.color = palette.shadow;
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke = Stroke::new(1.0, palette.text);
        widget.expansion = 0.0;
    }
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
    visuals.slider_trailing_fill = true;
    visuals.handle_shape = egui::style::HandleShape::Circle;
    ctx.set_visuals(visuals);
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
    fn the_omarchy_template_is_the_base_one() {
        assert_eq!(OMARCHY_TEMPLATE, fastframe_theme::omarchy::BASE_TEMPLATE);
    }

    #[test]
    fn the_omarchy_hook_is_the_crate_one() {
        assert_eq!(
            include_str!("../contrib/omarchy/youtfast-theme"),
            fastframe_theme::omarchy::hook_script("youtfast")
        );
    }
}
