//! The interface's colours: YouTFast's own dark and light, the palettes
//! the themes folder and the shared presets add, and Omarchy's live
//! palette.
//! fastframe-theme reads the files; [`Palette`] is what a file can set and
//! [`apply`] puts on egui.

use std::borrow::Cow;

use egui::{Color32, Stroke};

use super::desktop_font;

/// A palette file from the themes directory.
pub type CustomTheme = fastframe_theme::CustomTheme<Palette>;
/// The palette files, and the Omarchy palette where the desktop has one.
pub type Catalog = fastframe_theme::Catalog<Palette>;

/// The template Omarchy renders `youtfast.json` from, when neither Omarchy
/// nor the user has installed one.
pub const OMARCHY_TEMPLATE: &str = include_str!("../../../contrib/omarchy/youtfast.json.tpl");

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// The visual direction and motion language of the interface.
///
/// Built-in Dark/Light themes adapt their palette to the selected style while
/// custom JSON palettes and Omarchy themes keep their own colours and adopt
/// the style's geometry, surfaces and transitions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceStyle {
    /// macOS Sequoia / Apple TV+ precision: hairline dividers, refined corners,
    /// floating player dock, and smooth cubic-out transitions.
    #[default]
    Cupertino,
    /// visionOS / Liquid Glass floating chrome: inset translucent sidebar,
    /// specular top-edge highlights, rounded glass dock, and springy motion.
    LiquidGlass,
    /// Apple Music / editorial focus: compact chrome, tight corners, fast
    /// silky transitions, and maximum space for video and feeds.
    Cinema,
    /// Warm graphite and alabaster surfaces, generous spacing and corners,
    /// and soft tactile response.
    WarmStudio,
    /// GNOME Adwaita HIG: clean Linux header bar, boxed lists, balanced contrast,
    /// and moderate rounded corners.
    Adwaita,
    /// Microsoft Fluent 2 Mica: layered surfaces, subtle top-edge light,
    /// 8 px corners, and crisp active indicators.
    Fluent,
    /// Dieter Rams / Braun studio hardware: sharp 1 px borders, tactile bevels,
    /// and high-visibility tabular readouts.
    Industrial,
    /// Technical tiling-WM style (Hyprland / Zed / Ghostty): compact 44 px chrome,
    /// sharp 4 px corners, 1 px outlines, and fast 100 ms transitions.
    Technical,
    /// Tactile modern surfaces (Things 3 / Nova): physical top highlight and
    /// bottom shade bevels with responsive press depth.
    Tactile,
    /// Google Material 3 Tonal: surfaces tinted with the active accent colour
    /// and generous rounded containers.
    MaterialTonal,
    /// Low-stimulation warm paper and ink tones for comfortable long reading.
    Paper,
    /// Original YouTFast look (v0.9.x): 56 px top bar, 8 px corners, 120 ms fades.
    Classic,
}

/// How settings present a fixed list of time intervals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntervalPickerStyle {
    /// Individually styled options.
    Chips,
    /// Options share an animated selection track.
    Segmented,
}

impl InterfaceStyle {
    /// Control used for interval settings, independent of divider decoration.
    pub const fn interval_picker(self) -> IntervalPickerStyle {
        match self {
            Self::MaterialTonal | Self::Classic => IntervalPickerStyle::Chips,
            Self::Cupertino
            | Self::LiquidGlass
            | Self::Cinema
            | Self::WarmStudio
            | Self::Adwaita
            | Self::Fluent
            | Self::Industrial
            | Self::Technical
            | Self::Tactile
            | Self::Paper => IntervalPickerStyle::Segmented,
        }
    }

    /// Every style offered in Settings → Appearance, in presentation order.
    pub const ALL: [Self; 12] = [
        Self::Cupertino,
        Self::LiquidGlass,
        Self::Cinema,
        Self::WarmStudio,
        Self::Adwaita,
        Self::Fluent,
        Self::Industrial,
        Self::Technical,
        Self::Tactile,
        Self::MaterialTonal,
        Self::Paper,
        Self::Classic,
    ];

    /// Short display name for pickers (kept consistent across languages as style titles).
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cupertino => "Cupertino Quiet",
            Self::LiquidGlass => "Liquid Glass",
            Self::Cinema => "Cinema Minimal",
            Self::WarmStudio => "Warm Studio",
            Self::Adwaita => "Adwaita (GNOME)",
            Self::Fluent => "Fluent Mica",
            Self::Industrial => "Industrial (Braun)",
            Self::Technical => "Technical (Tiling)",
            Self::Tactile => "Tactile Modern",
            Self::MaterialTonal => "Material Tonal",
            Self::Paper => "Paper",
            Self::Classic => "Classic",
        }
    }

    /// Translatable one-line summary shown under the style picker.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Cupertino => {
                "Quiet precision with hairline dividers, a floating player dock, and smooth transitions."
            }
            Self::LiquidGlass => {
                "Floating sidebar and controls with specular highlights, rounded glass surfaces, and spring motion."
            }
            Self::Cinema => {
                "Compact chrome, crisp corners, and fast silky transitions that keep the focus on video."
            }
            Self::WarmStudio => {
                "Warm graphite and alabaster tones with generous spacing, softer corners, and tactile feedback."
            }
            Self::Adwaita => {
                "Clean Linux desktop look with boxed cards, balanced contrast, and soft corners."
            }
            Self::Fluent => {
                "Layered surfaces with subtle top highlights, 8 px corners, and crisp active indicators."
            }
            Self::Industrial => {
                "Studio-hardware precision with sharp 1 px borders, tactile bevels, and high-visibility tabular readouts."
            }
            Self::Technical => {
                "Compact tiling-friendly chrome with 4 px corners, sharp outlines, and fast keyboard-first transitions."
            }
            Self::Tactile => {
                "Physical button feel with subtle top bevels, crisp short shadows, and responsive press depth."
            }
            Self::MaterialTonal => {
                "Tonal surface elevation tinted by the accent colour with generous rounded containers."
            }
            Self::Paper => {
                "Low-stimulation warm paper and ink tones designed for comfortable long reading sessions."
            }
            Self::Classic => "The original YouTFast appearance and timing.",
        }
    }

    /// Geometry, elevation and motion metrics for this style.
    pub const fn spec(self) -> StyleSpec {
        match self {
            Self::Cupertino => StyleSpec {
                top_bar_height: 50.0,
                tab_bar_height: 34.0,
                sidebar_open: 232.0,
                sidebar_rail: 66.0,
                sidebar_inset: 0,
                sidebar_radius: 0,
                sidebar_animate: true,
                card_radius: 14,
                card_margin: 14,
                thumb_radius: 10,
                ctrl_radius: 8,
                modal_radius: 16,
                pip_radius: 14,
                mini_radius: 14,
                mini_frame_pad: 0,
                animation_time: 0.22,
                floating_dock: true,
                dock_inset: 10.0,
                dock_radius: 12,
                specular_highlight: true,
                hairline_dividers: true,
                grouped_palette: true,
                show_search_submit_button: false,
                hover_lift: 1.5,
                press_inset: 0.6,
                nav_accent_pill: true,
                floating_chrome: false,
                tonal_elevation: 0.0,
                tactile_bevel: false,
                sharp_borders: false,
            },
            Self::LiquidGlass => StyleSpec {
                top_bar_height: 54.0,
                tab_bar_height: 36.0,
                sidebar_open: 238.0,
                sidebar_rail: 68.0,
                sidebar_inset: 8,
                sidebar_radius: 16,
                sidebar_animate: true,
                card_radius: 18,
                card_margin: 14,
                thumb_radius: 13,
                ctrl_radius: 11,
                modal_radius: 20,
                pip_radius: 18,
                mini_radius: 18,
                mini_frame_pad: 0,
                animation_time: 0.26,
                floating_dock: true,
                dock_inset: 14.0,
                dock_radius: 16,
                specular_highlight: true,
                hairline_dividers: true,
                grouped_palette: true,
                show_search_submit_button: false,
                hover_lift: 2.0,
                press_inset: 0.9,
                nav_accent_pill: false,
                floating_chrome: true,
                tonal_elevation: 0.0,
                tactile_bevel: false,
                sharp_borders: false,
            },
            Self::Cinema => StyleSpec {
                top_bar_height: 46.0,
                tab_bar_height: 32.0,
                sidebar_open: 220.0,
                sidebar_rail: 60.0,
                sidebar_inset: 0,
                sidebar_radius: 0,
                sidebar_animate: true,
                card_radius: 10,
                card_margin: 12,
                thumb_radius: 8,
                ctrl_radius: 7,
                modal_radius: 12,
                pip_radius: 10,
                mini_radius: 10,
                mini_frame_pad: 0,
                animation_time: 0.16,
                floating_dock: false,
                dock_inset: 0.0,
                dock_radius: 0,
                specular_highlight: false,
                hairline_dividers: true,
                grouped_palette: true,
                show_search_submit_button: false,
                hover_lift: 0.0,
                press_inset: 0.4,
                nav_accent_pill: true,
                floating_chrome: false,
                tonal_elevation: 0.0,
                tactile_bevel: false,
                sharp_borders: false,
            },
            Self::WarmStudio => StyleSpec {
                top_bar_height: 54.0,
                tab_bar_height: 36.0,
                sidebar_open: 244.0,
                sidebar_rail: 70.0,
                sidebar_inset: 6,
                sidebar_radius: 14,
                sidebar_animate: true,
                card_radius: 16,
                card_margin: 16,
                thumb_radius: 12,
                ctrl_radius: 10,
                modal_radius: 18,
                pip_radius: 14,
                mini_radius: 14,
                mini_frame_pad: 0,
                animation_time: 0.24,
                floating_dock: true,
                dock_inset: 10.0,
                dock_radius: 14,
                specular_highlight: true,
                hairline_dividers: true,
                grouped_palette: true,
                show_search_submit_button: false,
                hover_lift: 1.5,
                press_inset: 0.8,
                nav_accent_pill: false,
                floating_chrome: false,
                tonal_elevation: 0.0,
                tactile_bevel: false,
                sharp_borders: false,
            },
            Self::Adwaita => StyleSpec {
                top_bar_height: 48.0,
                tab_bar_height: 34.0,
                sidebar_open: 236.0,
                sidebar_rail: 66.0,
                sidebar_inset: 0,
                sidebar_radius: 0,
                sidebar_animate: true,
                card_radius: 12,
                card_margin: 14,
                thumb_radius: 10,
                ctrl_radius: 8,
                modal_radius: 14,
                pip_radius: 12,
                mini_radius: 12,
                mini_frame_pad: 0,
                animation_time: 0.20,
                floating_dock: false,
                dock_inset: 0.0,
                dock_radius: 0,
                specular_highlight: false,
                hairline_dividers: true,
                grouped_palette: true,
                show_search_submit_button: false,
                hover_lift: 1.0,
                press_inset: 0.5,
                nav_accent_pill: false,
                floating_chrome: false,
                tonal_elevation: 0.0,
                tactile_bevel: false,
                sharp_borders: false,
            },
            Self::Fluent => StyleSpec {
                top_bar_height: 48.0,
                tab_bar_height: 34.0,
                sidebar_open: 232.0,
                sidebar_rail: 64.0,
                sidebar_inset: 0,
                sidebar_radius: 0,
                sidebar_animate: true,
                card_radius: 8,
                card_margin: 14,
                thumb_radius: 6,
                ctrl_radius: 6,
                modal_radius: 10,
                pip_radius: 8,
                mini_radius: 8,
                mini_frame_pad: 0,
                animation_time: 0.18,
                floating_dock: true,
                dock_inset: 8.0,
                dock_radius: 8,
                specular_highlight: true,
                hairline_dividers: true,
                grouped_palette: true,
                show_search_submit_button: false,
                hover_lift: 1.0,
                press_inset: 0.6,
                nav_accent_pill: true,
                floating_chrome: false,
                tonal_elevation: 0.0,
                tactile_bevel: false,
                sharp_borders: false,
            },
            Self::Industrial => StyleSpec {
                top_bar_height: 48.0,
                tab_bar_height: 32.0,
                sidebar_open: 224.0,
                sidebar_rail: 64.0,
                sidebar_inset: 0,
                sidebar_radius: 0,
                sidebar_animate: true,
                card_radius: 4,
                card_margin: 12,
                thumb_radius: 3,
                ctrl_radius: 3,
                modal_radius: 6,
                pip_radius: 4,
                mini_radius: 4,
                mini_frame_pad: 0,
                animation_time: 0.14,
                floating_dock: false,
                dock_inset: 0.0,
                dock_radius: 0,
                specular_highlight: false,
                hairline_dividers: true,
                grouped_palette: true,
                show_search_submit_button: false,
                hover_lift: 0.0,
                press_inset: 0.8,
                nav_accent_pill: true,
                floating_chrome: false,
                tonal_elevation: 0.0,
                tactile_bevel: true,
                sharp_borders: true,
            },
            Self::Technical => StyleSpec {
                top_bar_height: 44.0,
                tab_bar_height: 30.0,
                sidebar_open: 216.0,
                sidebar_rail: 58.0,
                sidebar_inset: 0,
                sidebar_radius: 0,
                sidebar_animate: true,
                card_radius: 4,
                card_margin: 12,
                thumb_radius: 3,
                ctrl_radius: 3,
                modal_radius: 5,
                pip_radius: 4,
                mini_radius: 4,
                mini_frame_pad: 0,
                animation_time: 0.10,
                floating_dock: false,
                dock_inset: 0.0,
                dock_radius: 0,
                specular_highlight: false,
                hairline_dividers: true,
                grouped_palette: true,
                show_search_submit_button: false,
                hover_lift: 0.0,
                press_inset: 0.3,
                nav_accent_pill: true,
                floating_chrome: false,
                tonal_elevation: 0.0,
                tactile_bevel: false,
                sharp_borders: true,
            },
            Self::Tactile => StyleSpec {
                top_bar_height: 52.0,
                tab_bar_height: 36.0,
                sidebar_open: 236.0,
                sidebar_rail: 68.0,
                sidebar_inset: 0,
                sidebar_radius: 0,
                sidebar_animate: true,
                card_radius: 12,
                card_margin: 14,
                thumb_radius: 9,
                ctrl_radius: 8,
                modal_radius: 14,
                pip_radius: 12,
                mini_radius: 12,
                mini_frame_pad: 0,
                animation_time: 0.20,
                floating_dock: true,
                dock_inset: 10.0,
                dock_radius: 12,
                specular_highlight: true,
                hairline_dividers: true,
                grouped_palette: true,
                show_search_submit_button: false,
                hover_lift: 1.5,
                press_inset: 1.0,
                nav_accent_pill: true,
                floating_chrome: false,
                tonal_elevation: 0.0,
                tactile_bevel: true,
                sharp_borders: false,
            },
            Self::MaterialTonal => StyleSpec {
                top_bar_height: 54.0,
                tab_bar_height: 36.0,
                sidebar_open: 244.0,
                sidebar_rail: 72.0,
                sidebar_inset: 0,
                sidebar_radius: 0,
                sidebar_animate: true,
                card_radius: 16,
                card_margin: 16,
                thumb_radius: 12,
                ctrl_radius: 10,
                modal_radius: 20,
                pip_radius: 16,
                mini_radius: 16,
                mini_frame_pad: 0,
                animation_time: 0.22,
                floating_dock: true,
                dock_inset: 10.0,
                dock_radius: 16,
                specular_highlight: false,
                hairline_dividers: false,
                grouped_palette: true,
                show_search_submit_button: false,
                hover_lift: 1.5,
                press_inset: 0.7,
                nav_accent_pill: false,
                floating_chrome: false,
                tonal_elevation: 0.08,
                tactile_bevel: false,
                sharp_borders: false,
            },
            Self::Paper => StyleSpec {
                top_bar_height: 50.0,
                tab_bar_height: 34.0,
                sidebar_open: 232.0,
                sidebar_rail: 64.0,
                sidebar_inset: 0,
                sidebar_radius: 0,
                sidebar_animate: true,
                card_radius: 10,
                card_margin: 14,
                thumb_radius: 8,
                ctrl_radius: 7,
                modal_radius: 12,
                pip_radius: 10,
                mini_radius: 10,
                mini_frame_pad: 0,
                animation_time: 0.14,
                floating_dock: false,
                dock_inset: 0.0,
                dock_radius: 0,
                specular_highlight: false,
                hairline_dividers: true,
                grouped_palette: true,
                show_search_submit_button: false,
                hover_lift: 0.0,
                press_inset: 0.3,
                nav_accent_pill: false,
                floating_chrome: false,
                tonal_elevation: 0.0,
                tactile_bevel: false,
                sharp_borders: false,
            },
            Self::Classic => StyleSpec {
                top_bar_height: 56.0,
                tab_bar_height: 40.0,
                sidebar_open: 240.0,
                sidebar_rail: 72.0,
                sidebar_inset: 0,
                sidebar_radius: 0,
                sidebar_animate: false,
                card_radius: RADIUS + 4,
                card_margin: 12,
                thumb_radius: RADIUS,
                ctrl_radius: RADIUS_SMALL + 2,
                modal_radius: RADIUS + 4,
                pip_radius: 0,
                mini_radius: RADIUS + 1,
                mini_frame_pad: 3,
                animation_time: 0.12,
                floating_dock: false,
                dock_inset: 0.0,
                dock_radius: 0,
                specular_highlight: false,
                hairline_dividers: false,
                grouped_palette: false,
                show_search_submit_button: true,
                hover_lift: 0.0,
                press_inset: 0.0,
                nav_accent_pill: false,
                floating_chrome: false,
                tonal_elevation: 0.0,
                tactile_bevel: false,
                sharp_borders: false,
            },
        }
    }
}

impl<'de> serde::Deserialize<'de> for InterfaceStyle {
    /// Unknown style names saved by a newer version fall back to the default
    /// instead of discarding the settings file.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum Known {
            Cupertino,
            LiquidGlass,
            Cinema,
            WarmStudio,
            Adwaita,
            Fluent,
            Industrial,
            Technical,
            Tactile,
            MaterialTonal,
            Paper,
            Classic,
        }
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Saved {
            Known(Known),
            Other(serde::de::IgnoredAny),
        }
        Ok(match Saved::deserialize(deserializer)? {
            Saved::Known(Known::Cupertino) => Self::Cupertino,
            Saved::Known(Known::LiquidGlass) => Self::LiquidGlass,
            Saved::Known(Known::Cinema) => Self::Cinema,
            Saved::Known(Known::WarmStudio) => Self::WarmStudio,
            Saved::Known(Known::Adwaita) => Self::Adwaita,
            Saved::Known(Known::Fluent) => Self::Fluent,
            Saved::Known(Known::Industrial) => Self::Industrial,
            Saved::Known(Known::Technical) => Self::Technical,
            Saved::Known(Known::Tactile) => Self::Tactile,
            Saved::Known(Known::MaterialTonal) => Self::MaterialTonal,
            Saved::Known(Known::Paper) => Self::Paper,
            Saved::Known(Known::Classic) => Self::Classic,
            Saved::Other(_) => Self::default(),
        })
    }
}

/// Geometry, elevation and motion parameters resolved from [`InterfaceStyle`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleSpec {
    pub top_bar_height: f32,
    pub tab_bar_height: f32,
    pub sidebar_open: f32,
    pub sidebar_rail: f32,
    pub sidebar_inset: i8,
    pub sidebar_radius: u8,
    pub sidebar_animate: bool,
    pub card_radius: u8,
    pub card_margin: i8,
    pub thumb_radius: u8,
    pub ctrl_radius: u8,
    pub modal_radius: u8,
    pub pip_radius: u8,
    pub mini_radius: u8,
    pub mini_frame_pad: i8,
    pub animation_time: f32,
    pub floating_dock: bool,
    pub dock_inset: f32,
    pub dock_radius: u8,
    pub specular_highlight: bool,
    pub hairline_dividers: bool,
    pub grouped_palette: bool,
    pub show_search_submit_button: bool,
    pub hover_lift: f32,
    pub press_inset: f32,
    pub nav_accent_pill: bool,
    pub floating_chrome: bool,
    pub tonal_elevation: f32,
    pub tactile_bevel: bool,
    pub sharp_borders: bool,
}

fn style_id() -> egui::Id {
    egui::Id::new("youtfast-interface-style")
}

/// The active [`InterfaceStyle`] stored on `ctx`, defaulting to [`InterfaceStyle::default`].
pub fn current_style(ctx: &egui::Context) -> InterfaceStyle {
    ctx.data(|data| data.get_temp::<InterfaceStyle>(style_id()))
        .unwrap_or_default()
}

/// Stores `style` on `ctx` so widgets can adapt without threading it through every signature.
fn set_current_style(ctx: &egui::Context, style: InterfaceStyle) {
    ctx.data_mut(|data| data.insert_temp(style_id(), style));
}

impl Palette {
    /// Built-in dark or light palette tailored to `style`.
    pub fn for_style(style: InterfaceStyle, dark: bool) -> Self {
        match (style, dark) {
            (InterfaceStyle::Classic, dark) => Self {
                on_accent: Color32::BLACK,
                ..if dark { Self::dark() } else { Self::light() }
            },
            (InterfaceStyle::Cupertino, true) => Self {
                dark: true,
                window: Color32::from_rgb(0x0b, 0x0b, 0x0d),
                panel: Color32::from_rgb(0x12, 0x12, 0x15),
                surface: Color32::from_rgb(0x1b, 0x1b, 0x1f),
                surface_hover: Color32::from_rgb(0x25, 0x25, 0x2b),
                surface_active: Color32::from_rgb(0x32, 0x32, 0x3a),
                outline: Color32::from_rgb(0x2d, 0x2d, 0x34),
                text: Color32::from_rgb(0xf5, 0xf5, 0xf7),
                secondary: Color32::from_rgb(0xa8, 0xa8, 0xb0),
                dim: Color32::from_rgb(0x6e, 0x6e, 0x76),
                accent: Color32::from_rgb(0xff, 0x37, 0x5f),
                accent_hover: Color32::from_rgb(0xff, 0x51, 0x73),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xff, 0x45, 0x3a),
                warning: Color32::from_rgb(0xff, 0xd6, 0x0a),
                overlay: Color32::from_rgb(0x1d, 0x1d, 0x22),
                shadow: Color32::from_black_alpha(175),
                red: Color32::from_rgb(0xff, 0x45, 0x3a),
                green: Color32::from_rgb(0x30, 0xd1, 0x58),
                yellow: Color32::from_rgb(0xff, 0xd6, 0x0a),
                blue: Color32::from_rgb(0x0a, 0x84, 0xff),
                magenta: Color32::from_rgb(0xbf, 0x5a, 0xf2),
                cyan: Color32::from_rgb(0x5a, 0xc8, 0xfa),
            },
            (InterfaceStyle::Cupertino, false) => Self {
                dark: false,
                window: Color32::from_rgb(0xf5, 0xf5, 0xf7),
                panel: Color32::from_rgb(0xec, 0xec, 0xf0),
                surface: Color32::from_rgb(0xff, 0xff, 0xff),
                surface_hover: Color32::from_rgb(0xef, 0xef, 0xf3),
                surface_active: Color32::from_rgb(0xdc, 0xdc, 0xe2),
                outline: Color32::from_rgb(0xd6, 0xd6, 0xde),
                text: Color32::from_rgb(0x1d, 0x1d, 0x1f),
                secondary: Color32::from_rgb(0x58, 0x58, 0x5e),
                dim: Color32::from_rgb(0x8e, 0x8e, 0x93),
                accent: Color32::from_rgb(0xff, 0x2d, 0x55),
                accent_hover: Color32::from_rgb(0xe0, 0x20, 0x46),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xd7, 0x00, 0x15),
                warning: Color32::from_rgb(0xb2, 0x76, 0x00),
                overlay: Color32::from_rgb(0xff, 0xff, 0xff),
                shadow: Color32::from_black_alpha(42),
                red: Color32::from_rgb(0xd7, 0x00, 0x15),
                green: Color32::from_rgb(0x1d, 0x83, 0x39),
                yellow: Color32::from_rgb(0xb2, 0x76, 0x00),
                blue: Color32::from_rgb(0x00, 0x66, 0xcc),
                magenta: Color32::from_rgb(0x89, 0x44, 0xab),
                cyan: Color32::from_rgb(0x00, 0x71, 0xa4),
            },
            (InterfaceStyle::LiquidGlass, true) => Self {
                dark: true,
                window: Color32::from_rgb(0x07, 0x08, 0x0c),
                panel: Color32::from_rgb(0x13, 0x15, 0x1f),
                surface: Color32::from_rgb(0x1b, 0x1e, 0x2b),
                surface_hover: Color32::from_rgb(0x25, 0x29, 0x3a),
                surface_active: Color32::from_rgb(0x31, 0x36, 0x4a),
                outline: Color32::from_rgb(0x36, 0x3c, 0x54),
                text: Color32::from_rgb(0xf7, 0xf8, 0xfc),
                secondary: Color32::from_rgb(0xad, 0xb2, 0xc4),
                dim: Color32::from_rgb(0x6b, 0x71, 0x87),
                accent: Color32::from_rgb(0xff, 0x37, 0x5f),
                accent_hover: Color32::from_rgb(0xff, 0x5c, 0x7c),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xff, 0x45, 0x3a),
                warning: Color32::from_rgb(0xff, 0xd6, 0x0a),
                overlay: Color32::from_rgb(0x19, 0x1c, 0x28),
                shadow: Color32::from_black_alpha(185),
                red: Color32::from_rgb(0xff, 0x45, 0x3a),
                green: Color32::from_rgb(0x30, 0xd1, 0x58),
                yellow: Color32::from_rgb(0xff, 0xd6, 0x0a),
                blue: Color32::from_rgb(0x40, 0x9c, 0xff),
                magenta: Color32::from_rgb(0xc7, 0x7d, 0xff),
                cyan: Color32::from_rgb(0x64, 0xd2, 0xff),
            },
            (InterfaceStyle::LiquidGlass, false) => Self {
                dark: false,
                window: Color32::from_rgb(0xee, 0xf1, 0xf7),
                panel: Color32::from_rgb(0xf6, 0xf8, 0xfc),
                surface: Color32::from_rgb(0xff, 0xff, 0xff),
                surface_hover: Color32::from_rgb(0xe8, 0xec, 0xf4),
                surface_active: Color32::from_rgb(0xd7, 0xdd, 0xe9),
                outline: Color32::from_rgb(0xcc, 0xd4, 0xe4),
                text: Color32::from_rgb(0x12, 0x15, 0x20),
                secondary: Color32::from_rgb(0x52, 0x59, 0x6c),
                dim: Color32::from_rgb(0x88, 0x90, 0xa4),
                accent: Color32::from_rgb(0xff, 0x2d, 0x55),
                accent_hover: Color32::from_rgb(0xe0, 0x20, 0x46),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xd7, 0x00, 0x15),
                warning: Color32::from_rgb(0xb2, 0x76, 0x00),
                overlay: Color32::from_rgb(0xfb, 0xfc, 0xff),
                shadow: Color32::from_black_alpha(48),
                red: Color32::from_rgb(0xd7, 0x00, 0x15),
                green: Color32::from_rgb(0x1d, 0x83, 0x39),
                yellow: Color32::from_rgb(0xb2, 0x76, 0x00),
                blue: Color32::from_rgb(0x00, 0x66, 0xcc),
                magenta: Color32::from_rgb(0x89, 0x44, 0xab),
                cyan: Color32::from_rgb(0x00, 0x71, 0xa4),
            },
            (InterfaceStyle::Cinema, true) => Self {
                dark: true,
                window: Color32::from_rgb(0x09, 0x09, 0x0a),
                panel: Color32::from_rgb(0x0d, 0x0d, 0x0f),
                surface: Color32::from_rgb(0x15, 0x15, 0x18),
                surface_hover: Color32::from_rgb(0x1f, 0x1f, 0x24),
                surface_active: Color32::from_rgb(0x2c, 0x2c, 0x33),
                outline: Color32::from_rgb(0x26, 0x26, 0x2c),
                text: Color32::from_rgb(0xf3, 0xf3, 0xf5),
                secondary: Color32::from_rgb(0x9f, 0x9f, 0xa8),
                dim: Color32::from_rgb(0x60, 0x60, 0x68),
                accent: Color32::from_rgb(0xff, 0x2d, 0x47),
                accent_hover: Color32::from_rgb(0xff, 0x47, 0x5e),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xff, 0x45, 0x3a),
                warning: Color32::from_rgb(0xf5, 0xc4, 0x3d),
                overlay: Color32::from_rgb(0x16, 0x16, 0x1a),
                shadow: Color32::from_black_alpha(185),
                red: Color32::from_rgb(0xff, 0x45, 0x3a),
                green: Color32::from_rgb(0x2e, 0xc7, 0x54),
                yellow: Color32::from_rgb(0xf5, 0xc4, 0x3d),
                blue: Color32::from_rgb(0x3e, 0xa6, 0xff),
                magenta: Color32::from_rgb(0xc7, 0x7d, 0xff),
                cyan: Color32::from_rgb(0x5a, 0xc8, 0xfa),
            },
            (InterfaceStyle::Cinema, false) => Self {
                dark: false,
                window: Color32::from_rgb(0xfc, 0xfc, 0xfd),
                panel: Color32::from_rgb(0xf4, 0xf4, 0xf6),
                surface: Color32::from_rgb(0xff, 0xff, 0xff),
                surface_hover: Color32::from_rgb(0xec, 0xec, 0xf0),
                surface_active: Color32::from_rgb(0xdc, 0xdc, 0xe2),
                outline: Color32::from_rgb(0xdc, 0xdc, 0xe2),
                text: Color32::from_rgb(0x11, 0x11, 0x14),
                secondary: Color32::from_rgb(0x59, 0x59, 0x61),
                dim: Color32::from_rgb(0x90, 0x90, 0x98),
                accent: Color32::from_rgb(0xe6, 0x26, 0x40),
                accent_hover: Color32::from_rgb(0xc9, 0x1e, 0x36),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xcc, 0x00, 0x00),
                warning: Color32::from_rgb(0xb2, 0x76, 0x00),
                overlay: Color32::from_rgb(0xff, 0xff, 0xff),
                shadow: Color32::from_black_alpha(40),
                red: Color32::from_rgb(0xcc, 0x00, 0x00),
                green: Color32::from_rgb(0x10, 0x75, 0x16),
                yellow: Color32::from_rgb(0xb2, 0x76, 0x00),
                blue: Color32::from_rgb(0x06, 0x5f, 0xd4),
                magenta: Color32::from_rgb(0x8e, 0x44, 0xad),
                cyan: Color32::from_rgb(0x06, 0x5f, 0xd4),
            },
            (InterfaceStyle::WarmStudio, true) => Self {
                dark: true,
                window: Color32::from_rgb(0x11, 0x10, 0x10),
                panel: Color32::from_rgb(0x17, 0x16, 0x15),
                surface: Color32::from_rgb(0x21, 0x1f, 0x1e),
                surface_hover: Color32::from_rgb(0x2c, 0x29, 0x27),
                surface_active: Color32::from_rgb(0x3a, 0x36, 0x33),
                outline: Color32::from_rgb(0x35, 0x31, 0x2e),
                text: Color32::from_rgb(0xf5, 0xf1, 0xeb),
                secondary: Color32::from_rgb(0xb5, 0xae, 0xa6),
                dim: Color32::from_rgb(0x76, 0x70, 0x69),
                accent: Color32::from_rgb(0xf0, 0x44, 0x38),
                accent_hover: Color32::from_rgb(0xff, 0x5b, 0x4f),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xff, 0x55, 0x4a),
                warning: Color32::from_rgb(0xf4, 0xc3, 0x4a),
                overlay: Color32::from_rgb(0x24, 0x22, 0x20),
                shadow: Color32::from_black_alpha(175),
                red: Color32::from_rgb(0xff, 0x55, 0x4a),
                green: Color32::from_rgb(0x3a, 0xc7, 0x62),
                yellow: Color32::from_rgb(0xf4, 0xc3, 0x4a),
                blue: Color32::from_rgb(0x52, 0xa8, 0xff),
                magenta: Color32::from_rgb(0xcf, 0x86, 0xff),
                cyan: Color32::from_rgb(0x63, 0xc7, 0xe8),
            },
            (InterfaceStyle::WarmStudio, false) => Self {
                dark: false,
                window: Color32::from_rgb(0xf7, 0xf5, 0xf1),
                panel: Color32::from_rgb(0xef, 0xec, 0xe6),
                surface: Color32::from_rgb(0xff, 0xff, 0xff),
                surface_hover: Color32::from_rgb(0xea, 0xe6, 0xdf),
                surface_active: Color32::from_rgb(0xdd, 0xd7, 0xce),
                outline: Color32::from_rgb(0xd8, 0xd1, 0xc7),
                text: Color32::from_rgb(0x1e, 0x1b, 0x18),
                secondary: Color32::from_rgb(0x5a, 0x54, 0x4c),
                dim: Color32::from_rgb(0x94, 0x8d, 0x84),
                accent: Color32::from_rgb(0xdc, 0x35, 0x2c),
                accent_hover: Color32::from_rgb(0xc4, 0x28, 0x20),
                on_accent: Color32::from_rgb(0xff, 0xff, 0xff),
                danger: Color32::from_rgb(0xc8, 0x24, 0x1c),
                warning: Color32::from_rgb(0xa8, 0x6e, 0x00),
                overlay: Color32::from_rgb(0xff, 0xfd, 0xfa),
                shadow: Color32::from_black_alpha(44),
                red: Color32::from_rgb(0xc8, 0x24, 0x1c),
                green: Color32::from_rgb(0x1b, 0x7c, 0x35),
                yellow: Color32::from_rgb(0xa8, 0x6e, 0x00),
                blue: Color32::from_rgb(0x0b, 0x61, 0xc2),
                magenta: Color32::from_rgb(0x88, 0x42, 0xa8),
                cyan: Color32::from_rgb(0x0b, 0x6e, 0x99),
            },
            (InterfaceStyle::Adwaita, true) => Self {
                dark: true,
                window: Color32::from_rgb(0x1e, 0x1e, 0x1e),
                panel: Color32::from_rgb(0x24, 0x24, 0x24),
                surface: Color32::from_rgb(0x2c, 0x2c, 0x2c),
                surface_hover: Color32::from_rgb(0x36, 0x36, 0x36),
                surface_active: Color32::from_rgb(0x42, 0x42, 0x42),
                outline: Color32::from_rgb(0x3e, 0x3e, 0x3e),
                text: Color32::from_rgb(0xff, 0xff, 0xff),
                secondary: Color32::from_rgb(0xc0, 0xc0, 0xc4),
                dim: Color32::from_rgb(0x7a, 0x7a, 0x80),
                accent: Color32::from_rgb(0x35, 0x84, 0xe4),
                accent_hover: Color32::from_rgb(0x4a, 0x95, 0xee),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xf6, 0x61, 0x51),
                warning: Color32::from_rgb(0xf6, 0xd3, 0x2d),
                overlay: Color32::from_rgb(0x2e, 0x2e, 0x2e),
                shadow: Color32::from_black_alpha(150),
                red: Color32::from_rgb(0xf6, 0x61, 0x51),
                green: Color32::from_rgb(0x33, 0xd1, 0x7a),
                yellow: Color32::from_rgb(0xf6, 0xd3, 0x2d),
                blue: Color32::from_rgb(0x62, 0xa0, 0xea),
                magenta: Color32::from_rgb(0xc0, 0x61, 0xcb),
                cyan: Color32::from_rgb(0x62, 0xa0, 0xea),
            },
            (InterfaceStyle::Adwaita, false) => Self {
                dark: false,
                window: Color32::from_rgb(0xfa, 0xfa, 0xfa),
                panel: Color32::from_rgb(0xeb, 0xeb, 0xeb),
                surface: Color32::from_rgb(0xff, 0xff, 0xff),
                surface_hover: Color32::from_rgb(0xf0, 0xf0, 0xf0),
                surface_active: Color32::from_rgb(0xde, 0xde, 0xde),
                outline: Color32::from_rgb(0xd6, 0xd6, 0xd6),
                text: Color32::from_rgb(0x1e, 0x1e, 0x1e),
                secondary: Color32::from_rgb(0x5e, 0x5c, 0x64),
                dim: Color32::from_rgb(0x8e, 0x8c, 0x94),
                accent: Color32::from_rgb(0x1c, 0x71, 0xd8),
                accent_hover: Color32::from_rgb(0x16, 0x5f, 0xb8),
                on_accent: Color32::from_rgb(0xff, 0xff, 0xff),
                danger: Color32::from_rgb(0xc0, 0x1c, 0x28),
                warning: Color32::from_rgb(0xa5, 0x6c, 0x00),
                overlay: Color32::from_rgb(0xff, 0xff, 0xff),
                shadow: Color32::from_black_alpha(36),
                red: Color32::from_rgb(0xc0, 0x1c, 0x28),
                green: Color32::from_rgb(0x1d, 0x7e, 0x45),
                yellow: Color32::from_rgb(0xa5, 0x6c, 0x00),
                blue: Color32::from_rgb(0x1c, 0x71, 0xd8),
                magenta: Color32::from_rgb(0x81, 0x3d, 0x9c),
                cyan: Color32::from_rgb(0x1c, 0x71, 0xd8),
            },
            (InterfaceStyle::Fluent, true) => Self {
                dark: true,
                window: Color32::from_rgb(0x19, 0x19, 0x19),
                panel: Color32::from_rgb(0x20, 0x20, 0x20),
                surface: Color32::from_rgb(0x27, 0x27, 0x27),
                surface_hover: Color32::from_rgb(0x32, 0x32, 0x32),
                surface_active: Color32::from_rgb(0x3d, 0x3d, 0x3d),
                outline: Color32::from_rgb(0x3a, 0x3a, 0x3a),
                text: Color32::from_rgb(0xf5, 0xf5, 0xf5),
                secondary: Color32::from_rgb(0xb8, 0xb8, 0xbc),
                dim: Color32::from_rgb(0x76, 0x76, 0x7a),
                accent: Color32::from_rgb(0x60, 0xcd, 0xff),
                accent_hover: Color32::from_rgb(0x7a, 0xd6, 0xff),
                on_accent: Color32::from_rgb(0x0f, 0x11, 0x14),
                danger: Color32::from_rgb(0xff, 0x6b, 0x6b),
                warning: Color32::from_rgb(0xfc, 0xe1, 0x00),
                overlay: Color32::from_rgb(0x2a, 0x2a, 0x2a),
                shadow: Color32::from_black_alpha(165),
                red: Color32::from_rgb(0xff, 0x6b, 0x6b),
                green: Color32::from_rgb(0x6c, 0xcb, 0x5f),
                yellow: Color32::from_rgb(0xfc, 0xe1, 0x00),
                blue: Color32::from_rgb(0x60, 0xcd, 0xff),
                magenta: Color32::from_rgb(0xd4, 0x8f, 0xff),
                cyan: Color32::from_rgb(0x60, 0xcd, 0xff),
            },
            (InterfaceStyle::Fluent, false) => Self {
                dark: false,
                window: Color32::from_rgb(0xf3, 0xf3, 0xf3),
                panel: Color32::from_rgb(0xee, 0xee, 0xee),
                surface: Color32::from_rgb(0xff, 0xff, 0xff),
                surface_hover: Color32::from_rgb(0xf5, 0xf5, 0xf5),
                surface_active: Color32::from_rgb(0xe5, 0xe5, 0xe5),
                outline: Color32::from_rgb(0xd9, 0xd9, 0xd9),
                text: Color32::from_rgb(0x1a, 0x1a, 0x1a),
                secondary: Color32::from_rgb(0x5d, 0x5d, 0x63),
                dim: Color32::from_rgb(0x8e, 0x8e, 0x94),
                accent: Color32::from_rgb(0x00, 0x67, 0xc0),
                accent_hover: Color32::from_rgb(0x00, 0x54, 0x9e),
                on_accent: Color32::from_rgb(0xff, 0xff, 0xff),
                danger: Color32::from_rgb(0xc4, 0x2b, 0x1c),
                warning: Color32::from_rgb(0x9d, 0x5d, 0x00),
                overlay: Color32::from_rgb(0xff, 0xff, 0xff),
                shadow: Color32::from_black_alpha(40),
                red: Color32::from_rgb(0xc4, 0x2b, 0x1c),
                green: Color32::from_rgb(0x0f, 0x7b, 0x0f),
                yellow: Color32::from_rgb(0x9d, 0x5d, 0x00),
                blue: Color32::from_rgb(0x00, 0x67, 0xc0),
                magenta: Color32::from_rgb(0x88, 0x3b, 0xa8),
                cyan: Color32::from_rgb(0x00, 0x67, 0xc0),
            },
            (InterfaceStyle::Industrial, true) => Self {
                dark: true,
                window: Color32::from_rgb(0x12, 0x13, 0x14),
                panel: Color32::from_rgb(0x17, 0x18, 0x1a),
                surface: Color32::from_rgb(0x1e, 0x20, 0x22),
                surface_hover: Color32::from_rgb(0x28, 0x2b, 0x2e),
                surface_active: Color32::from_rgb(0x34, 0x37, 0x3b),
                outline: Color32::from_rgb(0x3a, 0x3e, 0x42),
                text: Color32::from_rgb(0xf2, 0xf3, 0xf4),
                secondary: Color32::from_rgb(0xa8, 0xad, 0xb3),
                dim: Color32::from_rgb(0x6e, 0x73, 0x78),
                accent: Color32::from_rgb(0xff, 0x55, 0x00),
                accent_hover: Color32::from_rgb(0xff, 0x6d, 0x24),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xff, 0x44, 0x38),
                warning: Color32::from_rgb(0xff, 0xb8, 0x00),
                overlay: Color32::from_rgb(0x20, 0x22, 0x25),
                shadow: Color32::from_black_alpha(155),
                red: Color32::from_rgb(0xff, 0x44, 0x38),
                green: Color32::from_rgb(0x26, 0xd0, 0x62),
                yellow: Color32::from_rgb(0xff, 0xb8, 0x00),
                blue: Color32::from_rgb(0x4d, 0xa3, 0xff),
                magenta: Color32::from_rgb(0xc7, 0x7d, 0xff),
                cyan: Color32::from_rgb(0x4d, 0xc7, 0xe8),
            },
            (InterfaceStyle::Industrial, false) => Self {
                dark: false,
                window: Color32::from_rgb(0xe9, 0xe8, 0xe4),
                panel: Color32::from_rgb(0xdf, 0xde, 0xd9),
                surface: Color32::from_rgb(0xf4, 0xf3, 0xef),
                surface_hover: Color32::from_rgb(0xe8, 0xe7, 0xe2),
                surface_active: Color32::from_rgb(0xd8, 0xd6, 0xcf),
                outline: Color32::from_rgb(0xc2, 0xc0, 0xb8),
                text: Color32::from_rgb(0x16, 0x17, 0x18),
                secondary: Color32::from_rgb(0x55, 0x57, 0x5a),
                dim: Color32::from_rgb(0x85, 0x87, 0x8a),
                accent: Color32::from_rgb(0xe6, 0x48, 0x00),
                accent_hover: Color32::from_rgb(0xc7, 0x3e, 0x00),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xc8, 0x24, 0x1c),
                warning: Color32::from_rgb(0xa2, 0x6b, 0x00),
                overlay: Color32::from_rgb(0xf7, 0xf6, 0xf2),
                shadow: Color32::from_black_alpha(46),
                red: Color32::from_rgb(0xc8, 0x24, 0x1c),
                green: Color32::from_rgb(0x16, 0x7c, 0x3b),
                yellow: Color32::from_rgb(0xa2, 0x6b, 0x00),
                blue: Color32::from_rgb(0x0b, 0x5e, 0xb8),
                magenta: Color32::from_rgb(0x88, 0x42, 0xa8),
                cyan: Color32::from_rgb(0x0b, 0x6b, 0x8f),
            },
            (InterfaceStyle::Technical, true) => Self {
                dark: true,
                window: Color32::from_rgb(0x0a, 0x0b, 0x0d),
                panel: Color32::from_rgb(0x0f, 0x11, 0x14),
                surface: Color32::from_rgb(0x16, 0x19, 0x1e),
                surface_hover: Color32::from_rgb(0x1f, 0x23, 0x2a),
                surface_active: Color32::from_rgb(0x2a, 0x2f, 0x38),
                outline: Color32::from_rgb(0x2d, 0x33, 0x3d),
                text: Color32::from_rgb(0xe8, 0xec, 0xf2),
                secondary: Color32::from_rgb(0x9a, 0xa3, 0xb2),
                dim: Color32::from_rgb(0x61, 0x6a, 0x78),
                accent: Color32::from_rgb(0x00, 0xd1, 0xff),
                accent_hover: Color32::from_rgb(0x38, 0xdc, 0xff),
                on_accent: Color32::from_rgb(0x09, 0x0b, 0x0e),
                danger: Color32::from_rgb(0xff, 0x53, 0x70),
                warning: Color32::from_rgb(0xff, 0xcb, 0x6b),
                overlay: Color32::from_rgb(0x17, 0x1a, 0x20),
                shadow: Color32::from_black_alpha(150),
                red: Color32::from_rgb(0xff, 0x53, 0x70),
                green: Color32::from_rgb(0x3b, 0xd6, 0x71),
                yellow: Color32::from_rgb(0xff, 0xcb, 0x6b),
                blue: Color32::from_rgb(0x52, 0xb0, 0xff),
                magenta: Color32::from_rgb(0xc7, 0x92, 0xea),
                cyan: Color32::from_rgb(0x00, 0xd1, 0xff),
            },
            (InterfaceStyle::Technical, false) => Self {
                dark: false,
                window: Color32::from_rgb(0xf7, 0xf8, 0xfa),
                panel: Color32::from_rgb(0xee, 0xf0, 0xf4),
                surface: Color32::from_rgb(0xff, 0xff, 0xff),
                surface_hover: Color32::from_rgb(0xee, 0xf1, 0xf6),
                surface_active: Color32::from_rgb(0xdf, 0xe4, 0xec),
                outline: Color32::from_rgb(0xcb, 0xd2, 0xde),
                text: Color32::from_rgb(0x0f, 0x14, 0x19),
                secondary: Color32::from_rgb(0x4f, 0x59, 0x66),
                dim: Color32::from_rgb(0x84, 0x8e, 0x9c),
                accent: Color32::from_rgb(0x00, 0x70, 0xf3),
                accent_hover: Color32::from_rgb(0x00, 0x5c, 0xc8),
                on_accent: Color32::from_rgb(0xff, 0xff, 0xff),
                danger: Color32::from_rgb(0xd1, 0x24, 0x2f),
                warning: Color32::from_rgb(0x9a, 0x67, 0x00),
                overlay: Color32::from_rgb(0xff, 0xff, 0xff),
                shadow: Color32::from_black_alpha(36),
                red: Color32::from_rgb(0xd1, 0x24, 0x2f),
                green: Color32::from_rgb(0x1a, 0x7f, 0x37),
                yellow: Color32::from_rgb(0x9a, 0x67, 0x00),
                blue: Color32::from_rgb(0x00, 0x70, 0xf3),
                magenta: Color32::from_rgb(0x82, 0x50, 0xdf),
                cyan: Color32::from_rgb(0x09, 0x69, 0xda),
            },
            (InterfaceStyle::Tactile, true) => Self {
                dark: true,
                window: Color32::from_rgb(0x16, 0x17, 0x1a),
                panel: Color32::from_rgb(0x1c, 0x1e, 0x22),
                surface: Color32::from_rgb(0x24, 0x27, 0x2c),
                surface_hover: Color32::from_rgb(0x2f, 0x33, 0x3a),
                surface_active: Color32::from_rgb(0x3b, 0x40, 0x48),
                outline: Color32::from_rgb(0x3e, 0x43, 0x4c),
                text: Color32::from_rgb(0xf4, 0xf5, 0xf7),
                secondary: Color32::from_rgb(0xaf, 0xb5, 0xc0),
                dim: Color32::from_rgb(0x70, 0x76, 0x82),
                accent: Color32::from_rgb(0xff, 0x4f, 0x5e),
                accent_hover: Color32::from_rgb(0xff, 0x68, 0x75),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xff, 0x4f, 0x5e),
                warning: Color32::from_rgb(0xf5, 0xc4, 0x3d),
                overlay: Color32::from_rgb(0x26, 0x29, 0x2f),
                shadow: Color32::from_black_alpha(170),
                red: Color32::from_rgb(0xff, 0x4f, 0x5e),
                green: Color32::from_rgb(0x32, 0xd2, 0x66),
                yellow: Color32::from_rgb(0xf5, 0xc4, 0x3d),
                blue: Color32::from_rgb(0x4b, 0xa3, 0xff),
                magenta: Color32::from_rgb(0xc8, 0x82, 0xff),
                cyan: Color32::from_rgb(0x5a, 0xc8, 0xfa),
            },
            (InterfaceStyle::Tactile, false) => Self {
                dark: false,
                window: Color32::from_rgb(0xf1, 0xf2, 0xf5),
                panel: Color32::from_rgb(0xe7, 0xe9, 0xee),
                surface: Color32::from_rgb(0xff, 0xff, 0xff),
                surface_hover: Color32::from_rgb(0xf0, 0xf2, 0xf6),
                surface_active: Color32::from_rgb(0xe0, 0xe3, 0xea),
                outline: Color32::from_rgb(0xcf, 0xd4, 0xde),
                text: Color32::from_rgb(0x18, 0x1a, 0x1f),
                secondary: Color32::from_rgb(0x58, 0x5e, 0x6a),
                dim: Color32::from_rgb(0x8b, 0x91, 0x9e),
                accent: Color32::from_rgb(0xe6, 0x39, 0x46),
                accent_hover: Color32::from_rgb(0xc9, 0x2a, 0x37),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xc9, 0x2a, 0x37),
                warning: Color32::from_rgb(0xa8, 0x6e, 0x00),
                overlay: Color32::from_rgb(0xff, 0xff, 0xff),
                shadow: Color32::from_black_alpha(44),
                red: Color32::from_rgb(0xc9, 0x2a, 0x37),
                green: Color32::from_rgb(0x1b, 0x7c, 0x35),
                yellow: Color32::from_rgb(0xa8, 0x6e, 0x00),
                blue: Color32::from_rgb(0x0b, 0x61, 0xc2),
                magenta: Color32::from_rgb(0x88, 0x42, 0xa8),
                cyan: Color32::from_rgb(0x0b, 0x6e, 0x99),
            },
            (InterfaceStyle::MaterialTonal, true) => Self {
                dark: true,
                window: Color32::from_rgb(0x14, 0x12, 0x18),
                panel: Color32::from_rgb(0x1b, 0x18, 0x21),
                surface: Color32::from_rgb(0x22, 0x1f, 0x29),
                surface_hover: Color32::from_rgb(0x2c, 0x28, 0x36),
                surface_active: Color32::from_rgb(0x37, 0x32, 0x42),
                outline: Color32::from_rgb(0x44, 0x3d, 0x52),
                text: Color32::from_rgb(0xf4, 0xef, 0xfa),
                secondary: Color32::from_rgb(0xc4, 0xbc, 0xd1),
                dim: Color32::from_rgb(0x7f, 0x77, 0x8c),
                accent: Color32::from_rgb(0xd0, 0xbc, 0xff),
                accent_hover: Color32::from_rgb(0xde, 0xcf, 0xff),
                on_accent: Color32::from_rgb(0x1d, 0x19, 0x2b),
                danger: Color32::from_rgb(0xf2, 0xb8, 0xb5),
                warning: Color32::from_rgb(0xf5, 0xce, 0x6e),
                overlay: Color32::from_rgb(0x25, 0x21, 0x2d),
                shadow: Color32::from_black_alpha(165),
                red: Color32::from_rgb(0xf2, 0xb8, 0xb5),
                green: Color32::from_rgb(0xa8, 0xda, 0xb5),
                yellow: Color32::from_rgb(0xf5, 0xce, 0x6e),
                blue: Color32::from_rgb(0xa8, 0xc7, 0xfa),
                magenta: Color32::from_rgb(0xd0, 0xbc, 0xff),
                cyan: Color32::from_rgb(0x9d, 0xd3, 0xe8),
            },
            (InterfaceStyle::MaterialTonal, false) => Self {
                dark: false,
                window: Color32::from_rgb(0xfe, 0xf7, 0xff),
                panel: Color32::from_rgb(0xf3, 0xed, 0xf7),
                surface: Color32::from_rgb(0xff, 0xff, 0xff),
                surface_hover: Color32::from_rgb(0xf2, 0xec, 0xf7),
                surface_active: Color32::from_rgb(0xe4, 0xdc, 0xed),
                outline: Color32::from_rgb(0xd4, 0xca, 0xdb),
                text: Color32::from_rgb(0x1d, 0x1b, 0x20),
                secondary: Color32::from_rgb(0x5b, 0x55, 0x63),
                dim: Color32::from_rgb(0x8a, 0x83, 0x94),
                accent: Color32::from_rgb(0x67, 0x50, 0xa4),
                accent_hover: Color32::from_rgb(0x53, 0x3e, 0x8a),
                on_accent: Color32::from_rgb(0xff, 0xff, 0xff),
                danger: Color32::from_rgb(0xb3, 0x26, 0x1e),
                warning: Color32::from_rgb(0x9a, 0x64, 0x00),
                overlay: Color32::from_rgb(0xff, 0xff, 0xff),
                shadow: Color32::from_black_alpha(40),
                red: Color32::from_rgb(0xb3, 0x26, 0x1e),
                green: Color32::from_rgb(0x19, 0x6b, 0x3b),
                yellow: Color32::from_rgb(0x9a, 0x64, 0x00),
                blue: Color32::from_rgb(0x0b, 0x57, 0xd0),
                magenta: Color32::from_rgb(0x67, 0x50, 0xa4),
                cyan: Color32::from_rgb(0x00, 0x67, 0x7d),
            },
            (InterfaceStyle::Paper, true) => Self {
                dark: true,
                window: Color32::from_rgb(0x19, 0x18, 0x16),
                panel: Color32::from_rgb(0x20, 0x1e, 0x1c),
                surface: Color32::from_rgb(0x27, 0x25, 0x22),
                surface_hover: Color32::from_rgb(0x32, 0x2f, 0x2b),
                surface_active: Color32::from_rgb(0x3e, 0x3a, 0x35),
                outline: Color32::from_rgb(0x3b, 0x37, 0x32),
                text: Color32::from_rgb(0xeb, 0xe6, 0xdc),
                secondary: Color32::from_rgb(0xb5, 0xae, 0xa2),
                dim: Color32::from_rgb(0x78, 0x72, 0x68),
                accent: Color32::from_rgb(0xd9, 0x77, 0x57),
                accent_hover: Color32::from_rgb(0xe6, 0x89, 0x6a),
                on_accent: Color32::BLACK,
                danger: Color32::from_rgb(0xe5, 0x6b, 0x5c),
                warning: Color32::from_rgb(0xdf, 0xb4, 0x58),
                overlay: Color32::from_rgb(0x29, 0x27, 0x23),
                shadow: Color32::from_black_alpha(150),
                red: Color32::from_rgb(0xe5, 0x6b, 0x5c),
                green: Color32::from_rgb(0x78, 0xb4, 0x7a),
                yellow: Color32::from_rgb(0xdf, 0xb4, 0x58),
                blue: Color32::from_rgb(0x74, 0xa4, 0xcf),
                magenta: Color32::from_rgb(0xba, 0x94, 0xc7),
                cyan: Color32::from_rgb(0x74, 0xb5, 0xb8),
            },
            (InterfaceStyle::Paper, false) => Self {
                dark: false,
                window: Color32::from_rgb(0xf6, 0xf2, 0xea),
                panel: Color32::from_rgb(0xed, 0xe7, 0xdc),
                surface: Color32::from_rgb(0xfb, 0xf9, 0xf4),
                surface_hover: Color32::from_rgb(0xef, 0xea, 0xe0),
                surface_active: Color32::from_rgb(0xe2, 0xdb, 0xd0),
                outline: Color32::from_rgb(0xd4, 0xcc, 0xbe),
                text: Color32::from_rgb(0x24, 0x21, 0x1c),
                secondary: Color32::from_rgb(0x62, 0x5c, 0x52),
                dim: Color32::from_rgb(0x91, 0x8a, 0x7e),
                accent: Color32::from_rgb(0xb8, 0x4b, 0x2c),
                accent_hover: Color32::from_rgb(0x9c, 0x3c, 0x20),
                on_accent: Color32::from_rgb(0xff, 0xff, 0xff),
                danger: Color32::from_rgb(0xb0, 0x34, 0x28),
                warning: Color32::from_rgb(0x96, 0x66, 0x08),
                overlay: Color32::from_rgb(0xfd, 0xfb, 0xf7),
                shadow: Color32::from_black_alpha(36),
                red: Color32::from_rgb(0xb0, 0x34, 0x28),
                green: Color32::from_rgb(0x2b, 0x6e, 0x38),
                yellow: Color32::from_rgb(0x96, 0x66, 0x08),
                blue: Color32::from_rgb(0x24, 0x5f, 0x99),
                magenta: Color32::from_rgb(0x76, 0x42, 0x8a),
                cyan: Color32::from_rgb(0x21, 0x68, 0x75),
            },
        }
    }

    /// Tints neutral surfaces slightly toward `self.accent` for Material Tonal
    /// elevation, keeping luminance ordering intact.
    pub fn with_tonal_elevation(mut self, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 0.12);
        if amount <= 0.0 {
            return self;
        }
        let blend = |base: Color32, target: Color32, t: f32| {
            Color32::from(egui::lerp(
                egui::Rgba::from(base)..=egui::Rgba::from(target),
                t,
            ))
        };
        self.panel = blend(self.panel, self.accent, amount * 0.45);
        self.surface = blend(self.surface, self.accent, amount * 0.6);
        self.surface_hover = blend(self.surface_hover, self.accent, amount * 0.75);
        self.surface_active = blend(self.surface_active, self.accent, amount * 0.9);
        self
    }

    /// YouTFast's classic dark colours.
    pub fn dark() -> Self {
        Self {
            dark: true,
            window: Color32::from_rgb(0x0f, 0x0f, 0x0f),
            panel: Color32::from_rgb(0x14, 0x14, 0x16),
            surface: Color32::from_rgb(0x27, 0x27, 0x2b),
            surface_hover: Color32::from_rgb(0x34, 0x34, 0x3a),
            surface_active: Color32::from_rgb(0x45, 0x45, 0x45),
            outline: Color32::from_rgb(0x36, 0x36, 0x3c),
            text: Color32::from_rgb(0xf1, 0xf1, 0xf1),
            secondary: Color32::from_rgb(0xb5, 0xb5, 0xb5),
            dim: Color32::from_rgb(0x71, 0x71, 0x71),
            accent: Color32::from_rgb(0xff, 0x00, 0x33),
            accent_hover: Color32::from_rgb(0xff, 0x33, 0x55),
            on_accent: Color32::WHITE,
            danger: Color32::from_rgb(0xff, 0x4e, 0x45),
            warning: Color32::from_rgb(0xf2, 0xc9, 0x4c),
            overlay: Color32::from_rgb(0x28, 0x28, 0x2d),
            shadow: Color32::from_black_alpha(165),
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
            panel: Color32::from_rgb(0xf8, 0xf8, 0xfa),
            surface: Color32::from_rgb(0xf1, 0xf1, 0xf4),
            surface_hover: Color32::from_rgb(0xe5, 0xe5, 0xe9),
            surface_active: Color32::from_rgb(0xdb, 0xdb, 0xdb),
            outline: Color32::from_rgb(0xdf, 0xdf, 0xe5),
            text: Color32::from_rgb(0x0f, 0x0f, 0x0f),
            secondary: Color32::from_rgb(0x60, 0x60, 0x60),
            dim: Color32::from_rgb(0x90, 0x90, 0x90),
            accent: Color32::from_rgb(0xff, 0x00, 0x33),
            accent_hover: Color32::from_rgb(0xcc, 0x00, 0x29),
            on_accent: Color32::WHITE,
            danger: Color32::from_rgb(0xcc, 0x00, 0x00),
            warning: Color32::from_rgb(0xb5, 0x81, 0x00),
            overlay: Color32::from_rgb(0xff, 0xff, 0xff),
            shadow: Color32::from_black_alpha(45),
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

/// Which face draws the interface. The settings keep the choice.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceFont {
    /// The face the desktop draws its own interface with, falling back to
    /// Inter when it cannot be found or read.
    #[default]
    System,
    /// The bundled Inter, the same on every machine.
    Inter,
    /// An installed family, by name ([`installed_families`]), falling back
    /// to Inter when it is not installed any more.
    Family(String),
}

impl<'de> serde::Deserialize<'de> for InterfaceFont {
    /// A value this version does not know (a later one's) is the system's
    /// face instead of an error that would set the whole file aside.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Known {
            System,
            Inter,
            Family(String),
        }
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Saved {
            Known(Known),
            Other(serde::de::IgnoredAny),
        }
        Ok(match Saved::deserialize(deserializer)? {
            Saved::Known(Known::Inter) => Self::Inter,
            Saved::Known(Known::Family(family)) if !family.trim().is_empty() => {
                Self::Family(family)
            }
            Saved::Known(Known::System | Known::Family(_)) | Saved::Other(_) => Self::System,
        })
    }
}

/// Counts font swaps: text laid out before one is drawn from an atlas that
/// no longer exists, so anything that keeps galleys must forget them.
static FONTS_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Changes whenever the fonts do.
pub fn fonts_epoch() -> u64 {
    FONTS_EPOCH.load(std::sync::atomic::Ordering::Relaxed)
}

fn set_fonts(ctx: &egui::Context, fonts: egui::FontDefinitions) {
    FONTS_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ctx.set_fonts(fonts);
}

impl InterfaceFont {
    /// The choices every machine has; the installed families follow them.
    pub const ALL: [Self; 2] = [Self::System, Self::Inter];
}

/// The installed families the settings offer, once the font thread has
/// listed them; empty until then, and where there is no list (outside
/// Linux).
pub fn installed_families() -> &'static [String] {
    INSTALLED_FAMILIES.get().map_or(&[], Vec::as_slice)
}

static INSTALLED_FAMILIES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// The family of figures that keep their width while they count: Inter's
/// tabular digits, whichever face draws the rest of the interface.
const TABULAR: &str = "inter-tabular";

/// Text whose digits change every second (the player's position), set so
/// the label does not jitter. The system's faces have proportional digits;
/// Inter's are tabular.
pub fn tabular(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(TABULAR.into()))
}

/// Whether the face drawing the interface is the bundled Inter: the
/// regular weight's bytes are Inter's.
fn draws_with_inter(fonts: &egui::FontDefinitions) -> bool {
    fonts
        .font_data
        .get(fastframe_fonts::INTER_REGULAR)
        .is_some_and(|data| same_font(&data.font, fastframe_fonts::INTER))
}

fn same_font(font: &[u8], inter: &[u8]) -> bool {
    font.len() == inter.len() && (std::ptr::eq(font.as_ptr(), inter.as_ptr()) || font == inter)
}

/// Whether the last complete lookup of the system's face, or of a chosen
/// family, ended with Inter drawing instead.
static FACE_MISSING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the face asked for (the system's, or a chosen family) could not
/// be used, so Inter draws instead. False until such a lookup ends.
pub fn face_missing() -> bool {
    FACE_MISSING.load(std::sync::atomic::Ordering::Relaxed)
}

/// The interface face at its four weights, the bundled emoji face, the
/// installed faces behind it for other scripts and the bundled fallback
/// last ([`super::coverage`]), drawn the way the desktop renders text. Always
/// Inter and not the system's face: the same on every machine, for tests
/// and the demo.
pub fn install_fonts(ctx: &egui::Context) {
    set_fonts(ctx, font_definitions(&InterfaceFont::Inter, true));
}

/// The fonts for `choice`. With `system_fallbacks` this also looks for the
/// system's faces, running tools that can take a second, so it belongs on
/// the font thread; without it nothing is looked for, and a system face not
/// found yet is Inter until the search ends.
///
/// The system face is, in order: the desktop's configured font (Linux),
/// the platform's interface face (`Primary::System`: San Francisco, Segoe
/// UI, fontconfig's `system-ui`), then Inter.
fn font_definitions(choice: &InterfaceFont, system_fallbacks: bool) -> egui::FontDefinitions {
    let (fonts, missing) = looked_up(choice, system_fallbacks);
    // Inter asks for no face, so it leaves the last answer as it is: the
    // tests and the demo install it without touching what the app found.
    if let Some(missing) = missing.filter(|_| *choice != InterfaceFont::Inter) {
        FACE_MISSING.store(missing, std::sync::atomic::Ordering::Relaxed);
    }
    fonts
}

/// [`font_definitions`], and whether the face asked for was missing so
/// Inter draws instead; `None` when nothing was looked for.
fn looked_up(
    choice: &InterfaceFont,
    system_fallbacks: bool,
) -> (egui::FontDefinitions, Option<bool>) {
    use fastframe_fonts::Primary;
    // The face that replaces the library's, and the library's own choice
    // under it. Without `system_fallbacks` only what is already known is
    // used, and Inter draws until the search ends.
    let (replacement, primary) = match choice {
        InterfaceFont::Inter => (Replacement::None, Primary::Inter),
        InterfaceFont::System => {
            let desktop = if system_fallbacks {
                desktop_font::interface()
            } else {
                desktop_font::cached()
            };
            match desktop {
                Some(desktop) => (Replacement::Desktop(desktop), Primary::Inter),
                None if system_fallbacks => (Replacement::None, Primary::System),
                None => (Replacement::None, Primary::Inter),
            }
        }
        InterfaceFont::Family(name) => {
            let family = if system_fallbacks {
                desktop_font::family(name)
            } else {
                desktop_font::cached_family(name).flatten()
            };
            (
                family.map_or(Replacement::None, Replacement::Family),
                Primary::Inter,
            )
        }
    };
    let mut fonts = super::coverage::register(fastframe_fonts::FontSetup::default())
        .primary(primary)
        .system_fallbacks(system_fallbacks)
        .definitions();
    match &replacement {
        Replacement::None => {}
        Replacement::Desktop(face) => face.apply(&mut fonts),
        Replacement::Family(face) => face.apply(&mut fonts),
    }
    let missing = system_fallbacks.then(|| {
        *choice != InterfaceFont::Inter
            && matches!(replacement, Replacement::None)
            && draws_with_inter(&fonts)
    });
    super::coverage::append_fallback(&mut fonts);
    add_tabular(&mut fonts);
    text_rendering().apply_to(&mut fonts);
    (fonts, missing)
}

/// The face put where the library's interface face is.
enum Replacement {
    None,
    Desktop(&'static desktop_font::Interface),
    Family(std::sync::Arc<desktop_font::Interface>),
}

/// Registers the [`tabular`] family: Inter's digits ahead of whatever the
/// proportional family holds. With Inter drawing the interface, its
/// regular weight already is that.
fn add_tabular(fonts: &mut egui::FontDefinitions) {
    let mut family = fonts.families[&egui::FontFamily::Proportional].clone();
    if !draws_with_inter(fonts) {
        // The variable font's default instance is the regular weight.
        let data = egui::FontData::from_static(fastframe_fonts::INTER);
        fonts
            .font_data
            .insert(TABULAR.to_owned(), std::sync::Arc::new(data));
        family.insert(0, TABULAR.to_owned());
    }
    fonts
        .families
        .insert(egui::FontFamily::Name(TABULAR.into()), family);
}

/// The installed faces for the scripts Inter does not draw, looked for on
/// a thread of their own. Finding them probes every installed font, which
/// takes a second or more when the files are not in memory, so the window
/// opens with the rest of [`install_fonts`]'s setup and they follow.
pub struct SystemFonts {
    choice: InterfaceFont,
    found: std::sync::mpsc::Receiver<egui::FontDefinitions>,
    /// Woken when the search ends after the window opened.
    waker: std::sync::Arc<std::sync::OnceLock<egui::Context>>,
}

impl SystemFonts {
    /// Starts looking; called before the window is made, so the search
    /// overlaps its setup.
    pub fn find(choice: InterfaceFont) -> Self {
        let searched = choice.clone();
        let (sender, fonts) = Self::channel(choice);
        let wake = std::sync::Arc::clone(&fonts.waker);
        // A thread that cannot start leaves the channel disconnected, and
        // `poll` then searches on the caller's thread.
        let _ = std::thread::Builder::new()
            .name("youtfast-fonts".into())
            .spawn(move || {
                let sent = sender.send(font_definitions(&searched, true)).is_ok();
                // After the faces: the menu can wait, the text cannot.
                INSTALLED_FAMILIES.get_or_init(desktop_font::installed_families);
                if sent && let Some(ctx) = wake.get() {
                    ctx.request_repaint();
                }
            });
        fonts
    }

    fn channel(
        choice: InterfaceFont,
    ) -> (std::sync::mpsc::SyncSender<egui::FontDefinitions>, Self) {
        let (sender, found) = std::sync::mpsc::sync_channel(1);
        let fonts = Self {
            choice,
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
        set_fonts(ctx, font_definitions(&self.choice, false));
        false
    }

    /// Swaps in the complete fonts once found; true when they are in.
    pub fn poll(&self, ctx: &egui::Context) -> bool {
        match self.found.try_recv() {
            Ok(fonts) => set_fonts(ctx, fonts),
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                set_fonts(ctx, font_definitions(&self.choice, true));
            }
        }
        // New fonts take effect from the next pass.
        ctx.request_repaint();
        true
    }
}

/// Applies the palette to egui's own widgets using the active [`InterfaceStyle`].
pub fn apply(ctx: &egui::Context, palette: &Palette) {
    apply_style(ctx, palette, current_style(ctx));
}

/// Applies `palette` and `interface_style` to egui's own widgets so dialogs,
/// menus, shadows, radii, and animation timing agree with the chosen style.
pub fn apply_style(ctx: &egui::Context, palette: &Palette, interface_style: InterfaceStyle) {
    set_current_style(ctx, interface_style);
    let spec = interface_style.spec();
    let (theme, mut visuals) = if palette.dark {
        (egui::Theme::Dark, egui::Visuals::dark())
    } else {
        (egui::Theme::Light, egui::Visuals::light())
    };
    // First: `set_visuals` writes the style of the theme in effect.
    ctx.set_theme(theme);
    visuals.dark_mode = palette.dark;
    text_rendering().apply_to_visuals(&mut visuals);
    visuals.panel_fill = palette.window;
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
    visuals.window_corner_radius = egui::CornerRadius::same(spec.modal_radius);
    visuals.menu_corner_radius = egui::CornerRadius::same(spec.card_radius.min(14));
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [
            0,
            if spec.sharp_borders {
                3
            } else if spec.floating_chrome {
                10
            } else {
                6
            },
        ],
        blur: if spec.sharp_borders {
            10
        } else if spec.floating_chrome {
            32
        } else {
            24
        },
        spread: 0,
        color: palette.shadow,
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [
            0,
            if spec.sharp_borders {
                2
            } else if spec.floating_chrome {
                8
            } else {
                4
            },
        ],
        blur: if spec.sharp_borders {
            8
        } else if spec.floating_chrome {
            24
        } else {
            16
        },
        spread: 0,
        color: palette.shadow,
    };
    let corner = egui::CornerRadius::same(spec.ctrl_radius);
    let widget_stroke = if spec.sharp_borders {
        Stroke::new(1.0, palette.outline)
    } else {
        Stroke::NONE
    };
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = corner;
        widget.bg_stroke = widget_stroke;
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
            bar_width: 7.0,
            floating_width: 5.0,
            floating_allocated_width: 10.0,
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
        style.animation_time = spec.animation_time;
        style.url_in_tooltip = false;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Color32) -> f64 {
        let linear = |value: u8| {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }

    #[test]
    fn built_in_text_and_letter_avatars_meet_wcag_aa() {
        for style in InterfaceStyle::ALL {
            for dark in [true, false] {
                let palette = Palette::for_style(style, dark);
                let check_contrast = |foreground, background| {
                    let a = luminance(foreground);
                    let b = luminance(background);
                    let contrast = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                    assert!(
                        contrast >= 4.5,
                        "{style:?} dark={dark}: {foreground:?} on {background:?}: {contrast}"
                    );
                };
                for foreground in [palette.text, palette.secondary] {
                    for background in [
                        palette.window,
                        palette.surface,
                        palette.surface_hover,
                        palette.surface_active,
                    ] {
                        check_contrast(foreground, background);
                    }
                }
                check_contrast(palette.on_accent, palette.accent);
                // Window/surface steps stay ordered: hover, then selected.
                let steps = [
                    palette.surface,
                    palette.surface_hover,
                    palette.surface_active,
                ]
                .map(luminance);
                assert!(
                    if palette.dark {
                        steps[0] < steps[1] && steps[1] < steps[2]
                    } else {
                        steps[0] > steps[1] && steps[1] > steps[2]
                    },
                    "{style:?} dark={dark}: {steps:?}"
                );
            }
        }
    }

    #[test]
    fn every_interface_style_has_valid_spec_and_metadata() {
        for style in InterfaceStyle::ALL {
            let spec = style.spec();
            assert!(!style.label().is_empty());
            assert!(!style.description().is_empty());
            assert!(spec.top_bar_height >= 40.0);
            assert!(spec.sidebar_open > spec.sidebar_rail);
            assert!(spec.animation_time > 0.0);
        }
    }

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
            include_str!("../../../contrib/omarchy/youtfast-theme"),
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
    fn inter_is_told_from_another_face_by_its_bytes() {
        let inter = fastframe_fonts::INTER;
        assert!(same_font(inter, inter));
        let copy = inter.to_vec();
        assert!(same_font(&copy, inter));
        assert!(!same_font(&inter[1..], inter));
        let mut changed = inter.to_vec();
        let middle = changed.len() / 2;
        changed[middle] ^= 0xff;
        assert!(!same_font(&changed, inter));
        assert!(!same_font(&[], inter));
    }

    #[test]
    fn the_interface_face_is_inter_unless_another_took_its_place() {
        let mut fonts = font_definitions(&InterfaceFont::Inter, false);
        assert!(draws_with_inter(&fonts));
        // What the library registers when the system face was found: the
        // same key, another face.
        fonts.font_data.insert(
            fastframe_fonts::INTER_REGULAR.to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(vec![0, 1, 0, 0])),
        );
        assert!(!draws_with_inter(&fonts));
        fonts.font_data.clear();
        assert!(!draws_with_inter(&fonts));
    }

    #[test]
    fn counting_figures_keep_inters_digits_with_either_face() {
        let tabular_family = || egui::FontFamily::Name(TABULAR.into());
        let inter = font_definitions(&InterfaceFont::Inter, false);
        assert_eq!(
            inter.families[&tabular_family()],
            inter.families[&egui::FontFamily::Proportional]
        );
        assert!(!inter.font_data.contains_key(TABULAR));

        let mut other = font_definitions(&InterfaceFont::Inter, false);
        other.font_data.insert(
            fastframe_fonts::INTER_REGULAR.to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(vec![0, 1, 0, 0])),
        );
        add_tabular(&mut other);
        let family = &other.families[&tabular_family()];
        assert_eq!(family[0], TABULAR);
        assert_eq!(
            family[1..],
            other.families[&egui::FontFamily::Proportional][..]
        );
        assert!(same_font(
            &other.font_data[TABULAR].font,
            fastframe_fonts::INTER
        ));
    }

    #[test]
    fn the_system_choice_builds_fonts_with_the_same_families() {
        let system = font_definitions(&InterfaceFont::System, false); // nothing looked for
        let inter = font_definitions(&InterfaceFont::Inter, false);
        assert_eq!(
            system
                .families
                .keys()
                .collect::<std::collections::HashSet<_>>(),
            inter
                .families
                .keys()
                .collect::<std::collections::HashSet<_>>()
        );
        // The notice follows what happened to the lookup: Inter drew
        // because neither the desktop's font nor the platform's was found.
        let (searched, missing) = looked_up(&InterfaceFont::System, true);
        assert_eq!(
            missing,
            Some(desktop_font::cached().is_none() && draws_with_inter(&searched))
        );
        assert_eq!(looked_up(&InterfaceFont::System, false).1, None);
    }

    #[test]
    fn a_family_that_is_not_installed_draws_with_inter_and_says_so() {
        let choice = InterfaceFont::Family("YouTFast test family nobody installs".into());
        // Before the search: Inter, and nothing to say yet.
        let (fonts, missing) = looked_up(&choice, false);
        assert!(draws_with_inter(&fonts));
        assert_eq!(missing, None);
        let (fonts, missing) = looked_up(&choice, true);
        assert!(draws_with_inter(&fonts));
        assert_eq!(missing, Some(true));
        // Inter itself is never missing.
        assert_eq!(looked_up(&InterfaceFont::Inter, true).1, Some(false));
    }

    #[test]
    fn the_window_opens_without_installed_faces_and_takes_them_once_found() {
        let ctx = egui::Context::default();
        let (sender, fonts) = SystemFonts::channel(InterfaceFont::Inter);
        assert!(!fonts.install(&ctx));
        assert!(!fonts.poll(&ctx));
        assert!(fonts_in(&ctx) == font_definitions(&InterfaceFont::Inter, false));
        // What the search found, told apart by a family of its own.
        let mut found = font_definitions(&InterfaceFont::Inter, false);
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
        let (sender, fonts) = SystemFonts::channel(InterfaceFont::Inter);
        drop(sender);
        assert!(fonts.install(&ctx));
        assert!(fonts_in(&ctx) == font_definitions(&InterfaceFont::Inter, true));
    }
}
