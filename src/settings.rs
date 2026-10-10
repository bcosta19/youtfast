//! The choices kept between runs: video quality and volume. One JSON file,
//! written atomically off the interface thread.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::Quality;
use crate::theme::{CustomTheme, Palette};

fn on() -> bool {
    true
}

fn default_volume() -> f64 {
    100.0
}

/// Which colours the interface draws with.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    /// The desktop's palette where it has one (Omarchy), dark otherwise.
    #[default]
    Auto,
    Dark,
    Light,
    /// A palette file from the themes folder, by its filename.
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// The most a video may stream at.
    #[serde(default)]
    pub quality: Quality,
    /// mpv's volume, 0 to 100.
    #[serde(default = "default_volume")]
    pub volume: f64,
    #[serde(default)]
    pub theme: ThemeChoice,
    /// The last palette the selected file gave, so start-up draws it before
    /// the scan finishes (and while the file is gone).
    #[serde(
        default,
        deserialize_with = "fastframe_theme::read_cached_theme",
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_theme_cache: Option<CustomTheme>,
    /// The desktop's last palette (Omarchy), so Automatic survives a
    /// restart and the desktop's theme being unreadable.
    #[serde(
        default,
        deserialize_with = "fastframe_theme::read_cached_theme",
        skip_serializing_if = "Option::is_none"
    )]
    pub system_theme_cache: Option<CustomTheme>,
    /// Check the subscriptions for new uploads on a timer.
    #[serde(default = "on")]
    pub alerts: bool,
    /// Put the new uploads on the desktop too.
    #[serde(default = "on")]
    pub desktop_notifications: bool,
    /// How often, in minutes: 5, 15, 30 or 60 (the nearest wins).
    #[serde(default = "default_alerts_every")]
    pub alerts_every_minutes: u32,
}

fn default_alerts_every() -> u32 {
    15
}

/// The allowed check intervals; any other value clamps to the nearest.
pub fn clamp_alerts_every(minutes: u32) -> u32 {
    const CHOICES: [u32; 4] = [5, 15, 30, 60];
    CHOICES
        .iter()
        .min_by_key(|choice| (choice.abs_diff(minutes), std::cmp::Reverse(**choice)))
        .copied()
        .unwrap_or(15)
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            quality: Quality::default(),
            volume: default_volume(),
            theme: ThemeChoice::default(),
            custom_theme_cache: None,
            system_theme_cache: None,
            alerts: on(),
            desktop_notifications: on(),
            alerts_every_minutes: default_alerts_every(),
        }
    }
}

impl Settings {
    /// Reads the settings. A missing file is the defaults; an unreadable
    /// one is moved aside rather than overwritten by the next save.
    pub fn load(path: &Path) -> Self {
        crate::paths::load_json(path, "settings")
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        crate::paths::save_json(path, self)
    }

    /// The colours the interface draws with now: the selected palette's
    /// cache when it has one, else a built-in.
    pub fn palette(&self) -> Palette {
        match &self.theme {
            ThemeChoice::Custom(filename) => self
                .custom_theme_cache
                .as_ref()
                .filter(|theme| &theme.filename == filename)
                .map(|theme| theme.palette.clone())
                .unwrap_or_else(Palette::dark),
            ThemeChoice::Auto => self
                .system_theme_cache
                .as_ref()
                .map(|theme| theme.palette.clone())
                .unwrap_or_else(Palette::dark),
            ThemeChoice::Dark => Palette::dark(),
            ThemeChoice::Light => Palette::light(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A fresh file under the build tree (never /tmp).
    fn path(name: &str) -> PathBuf {
        let dir = std::env::var("CARGO_TARGET_TMPDIR")
            .unwrap_or_else(|_| format!("{}/target", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).unwrap();
        Path::new(&dir).join(format!("youtfast-{name}-{}.json", crate::util::now()))
    }

    #[test]
    fn the_alerts_settings_clamp_and_default() {
        assert_eq!(clamp_alerts_every(0), 5);
        assert_eq!(clamp_alerts_every(7), 5);
        assert_eq!(clamp_alerts_every(10), 15);
        assert_eq!(clamp_alerts_every(45), 60);
        assert_eq!(clamp_alerts_every(300), 60);
        let settings = Settings::default();
        assert!(settings.alerts);
        assert!(settings.desktop_notifications);
        assert_eq!(settings.alerts_every_minutes, 15);
        // An older file (none of the new keys) loads them too.
        let parsed: Settings =
            serde_json::from_str(r#"{"volume": 40.0, "theme": "dark"}"#).unwrap();
        assert_eq!(parsed.alerts_every_minutes, 15);
        assert!(parsed.alerts);
        assert!(parsed.desktop_notifications);
        assert_eq!(parsed.volume, 40.0);
    }

    #[test]
    fn round_trip() {
        let path = path("settings");
        let settings = Settings {
            quality: Quality::Max(720),
            volume: 40.0,
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_missing_file_is_the_defaults() {
        assert_eq!(Settings::load(&path("missing")), Settings::default());
    }

    #[test]
    fn an_empty_file_is_the_defaults() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn a_partial_file_keeps_what_it_has() {
        let settings: Settings = serde_json::from_str(r#"{"quality":{"Max":720}}"#).unwrap();
        assert_eq!(settings.quality, Quality::Max(720));
        assert_eq!(settings.volume, 100.0);
    }

    #[test]
    fn an_unreadable_file_is_moved_aside() {
        let path = path("bad-settings");
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        assert!(!path.exists());
        let aside = format!("{}.unreadable-*", path.display());
        let prefix = format!(
            "{}.unreadable-",
            path.file_name().unwrap().to_string_lossy()
        );
        let found = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
            .unwrap_or_else(|| panic!("{aside} should exist"));
        std::fs::remove_file(found.path()).unwrap();
    }

    #[test]
    fn an_older_file_loads_as_auto() {
        let settings: Settings =
            serde_json::from_str(r#"{"quality":{"Max":720},"volume":40}"#).unwrap();
        assert_eq!(settings.theme, ThemeChoice::Auto);
        assert!(settings.custom_theme_cache.is_none());
        assert!(settings.system_theme_cache.is_none());
    }

    #[test]
    fn the_palette_follows_the_theme_choice() {
        let with_theme = |theme| Settings {
            theme,
            ..Settings::default()
        };
        assert_eq!(with_theme(ThemeChoice::Auto).palette(), Palette::dark());
        assert_eq!(with_theme(ThemeChoice::Light).palette(), Palette::light());
        assert_eq!(with_theme(ThemeChoice::Dark).palette(), Palette::dark());
        // Automatic picks the desktop's cached palette when there is one.
        let mut omarchy = Palette::light();
        omarchy.accent = egui::Color32::from_rgb(1, 2, 3);
        let mut settings = Settings {
            system_theme_cache: Some(CustomTheme {
                filename: "omarchy.json".into(),
                palette: omarchy.clone(),
            }),
            ..Settings::default()
        };
        assert_eq!(settings.palette(), omarchy);
        // A custom palette comes from its cache, matching the filename.
        let mut nord = Palette::dark();
        nord.accent = egui::Color32::from_rgb(4, 5, 6);
        settings.custom_theme_cache = Some(CustomTheme {
            filename: "Nord.json".into(),
            palette: nord.clone(),
        });
        settings.theme = ThemeChoice::Custom("Nord.json".into());
        assert_eq!(settings.palette(), nord);
        // A cache for another file does not count.
        settings.theme = ThemeChoice::Custom("Gone.json".into());
        assert_eq!(settings.palette(), Palette::dark());
    }

    #[test]
    fn a_damaged_theme_cache_keeps_the_other_settings() {
        let text = r#"{
            "theme": {"custom": "gruvbox.json"},
            "volume": 37,
            "custom_theme_cache": {"palette": "broken"},
            "system_theme_cache": {"filename": "omarchy.json"}
        }"#;
        let settings: Settings = serde_json::from_str(text).unwrap();
        assert_eq!(settings.theme, ThemeChoice::Custom("gruvbox.json".into()));
        assert_eq!(settings.volume, 37.0);
        assert!(settings.custom_theme_cache.is_none());
        assert!(settings.system_theme_cache.is_none());
        // An intact cache survives, and round-trips.
        let settings = Settings {
            theme: ThemeChoice::Custom("Nord.json".into()),
            custom_theme_cache: Some(CustomTheme {
                filename: "Nord.json".into(),
                palette: Palette::dark(),
            }),
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), settings);
    }
}
