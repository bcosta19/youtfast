//! The choices kept between runs: video quality and volume. One JSON file,
//! written atomically off the interface thread.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::appearance::theme::{CustomTheme, Palette};
use crate::model::Quality;

fn on() -> bool {
    true
}

fn default_volume() -> f64 {
    100.0
}

fn default_caption_scale() -> f64 {
    1.0
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

/// How signed-in history relates to this device's history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistorySync {
    /// Keep the account and device lists separate, without reporting plays.
    #[default]
    Separate,
    /// Combine the lists only in YouTFast.
    Merged,
    /// Combine the lists and report started playback to the active account.
    SendToYoutube,
}

/// How much video mpv buffers in memory around the playback point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackBuffer {
    /// mpv's default: 150 MiB ahead and 50 MiB behind.
    #[default]
    Standard,
    /// Reduced cache: 50 MiB ahead and 20 MiB behind.
    Compact,
}

impl PlaybackBuffer {
    /// `demuxer-max-bytes` and `demuxer-max-back-bytes` for mpv.
    pub fn limits(self) -> (&'static str, &'static str) {
        match self {
            Self::Standard => ("150MiB", "50MiB"),
            Self::Compact => ("50MiB", "20MiB"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// The interface language; older settings start in English too.
    #[serde(default)]
    pub language: crate::appearance::i18n::Locale,
    /// The face that draws the interface; older settings use the system's.
    #[serde(default)]
    pub interface_font: crate::appearance::theme::InterfaceFont,
    /// Applies only while signed in; older files keep separate histories.
    #[serde(default)]
    pub history_sync: HistorySync,
    /// How much video mpv keeps buffered in memory; older files use mpv's
    /// default (150 MiB ahead, 50 MiB behind).
    #[serde(default)]
    pub playback_buffer: PlaybackBuffer,
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
    /// The watch page's theater mode: the video across the window, the
    /// rest under it.
    #[serde(default)]
    pub theater: bool,
    /// Show the mini player when the watch page's video scrolls out of view.
    #[serde(default)]
    pub scroll_miniplayer: bool,
    /// The sidebar collapsed to its rail of keys and avatars.
    #[serde(default)]
    pub sidebar_collapsed: bool,
    /// The sidebar's buttons in the order drawn, each shown or hidden. A
    /// saved list is read through [`sanitize_sidebar`], so it is complete.
    #[serde(default = "default_sidebar", deserialize_with = "read_sidebar")]
    pub sidebar: Vec<SidebarEntry>,
    /// At a video's end, the next one (the queue's, Up next's, then the
    /// first related) plays after a short countdown.
    #[serde(default = "on")]
    pub autoplay: bool,
    /// The captions' language ("en", "pt-BR"), or `None` while they are
    /// off. A video without that language shows none.
    #[serde(default)]
    pub captions: Option<String>,
    #[serde(default = "default_caption_scale")]
    pub caption_scale: f64,
    #[serde(default)]
    pub caption_background: bool,
    /// Skip sponsored segments through SponsorBlock (sponsor.ajay.app),
    /// asked by a prefix of the id's hash so the video is not named.
    #[serde(default)]
    pub sponsorblock: bool,
    /// Check for new YouTFast releases and show a system notification when
    /// an update is available.
    #[serde(default = "on")]
    pub update_notifications: bool,
    /// Automatically download and install minor and patch updates in the
    /// background; major versions still require manual confirmation.
    #[serde(default)]
    pub auto_update: bool,
    /// The release version most recently notified on the desktop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_notified_version: Option<String>,
    /// The version most recently installed by the built-in updater, kept so
    /// its changelog stays visible after restarting into the new release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated_version: Option<String>,
    /// The changelog notes of `last_updated_version`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated_changelog: Option<String>,
}

/// A button of the sidebar. `Unknown` stands for a name a newer version
/// saved; [`sanitize_sidebar`] drops it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarButton {
    Home,
    Subscriptions,
    History,
    WatchLater,
    Liked,
    Playlists,
    Notifications,
    Account,
    Settings,
    #[serde(other)]
    Unknown,
}

impl SidebarButton {
    /// Every button, in the default order.
    pub const ALL: [Self; 9] = [
        Self::Home,
        Self::Subscriptions,
        Self::History,
        Self::WatchLater,
        Self::Liked,
        Self::Playlists,
        Self::Notifications,
        Self::Account,
        Self::Settings,
    ];

    /// Account and Settings stay shown: hiding Settings would leave no way
    /// back to this list.
    pub fn can_hide(self) -> bool {
        !matches!(self, Self::Account | Self::Settings)
    }
}

/// One sidebar button and whether it is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarEntry {
    pub button: SidebarButton,
    #[serde(default = "on")]
    pub visible: bool,
}

/// All buttons, shown, in the default order.
pub fn default_sidebar() -> Vec<SidebarEntry> {
    SidebarButton::ALL
        .into_iter()
        .map(|button| SidebarEntry {
            button,
            visible: true,
        })
        .collect()
}

/// Makes a saved list usable: unknown buttons and repeats are dropped (the
/// first stays), buttons it lacks are appended shown in the default order,
/// and Account and Settings are shown.
pub fn sanitize_sidebar(entries: Vec<SidebarEntry>) -> Vec<SidebarEntry> {
    let mut clean: Vec<SidebarEntry> = Vec::with_capacity(SidebarButton::ALL.len());
    for entry in entries {
        if entry.button != SidebarButton::Unknown && !clean.iter().any(|e| e.button == entry.button)
        {
            clean.push(entry);
        }
    }
    for button in SidebarButton::ALL {
        if !clean.iter().any(|e| e.button == button) {
            clean.push(SidebarEntry {
                button,
                visible: true,
            });
        }
    }
    for entry in &mut clean {
        entry.visible |= !entry.button.can_hide();
    }
    clean
}

/// Reads the list entry by entry: a malformed entry is skipped instead of
/// failing the whole settings file.
fn read_sidebar<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<SidebarEntry>, D::Error> {
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(sanitize_sidebar(
        values
            .into_iter()
            .filter_map(|value| serde_json::from_value(value).ok())
            .collect(),
    ))
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
            language: crate::appearance::i18n::Locale::default(),
            interface_font: crate::appearance::theme::InterfaceFont::default(),
            history_sync: HistorySync::default(),
            playback_buffer: PlaybackBuffer::default(),
            quality: Quality::default(),
            volume: default_volume(),
            theme: ThemeChoice::default(),
            custom_theme_cache: None,
            system_theme_cache: None,
            alerts: on(),
            desktop_notifications: on(),
            alerts_every_minutes: default_alerts_every(),
            theater: false,
            scroll_miniplayer: false,
            sidebar_collapsed: false,
            sidebar: default_sidebar(),
            autoplay: on(),
            captions: None,
            caption_scale: 1.0,
            caption_background: false,
            sponsorblock: false,
            update_notifications: on(),
            auto_update: false,
            last_notified_version: None,
            last_updated_version: None,
            last_updated_changelog: None,
        }
    }
}

impl Settings {
    /// Reads the settings. A missing file is the defaults; an unreadable
    /// one is moved aside rather than overwritten by the next save.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        crate::storage::paths::load_json(path, "settings")
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        crate::storage::paths::save_json(path, self)
    }

    /// The colours the interface draws with now: the selected palette's
    /// cache when it has one, else a built-in.
    pub fn palette(&self) -> Palette {
        match &self.theme {
            ThemeChoice::Custom(filename) => self
                .custom_theme_cache
                .as_ref()
                .filter(|theme| &theme.filename == filename)
                .map(|theme| theme.palette)
                .unwrap_or_else(Palette::dark),
            ThemeChoice::Auto => self
                .system_theme_cache
                .as_ref()
                .map(|theme| theme.palette)
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
        assert!(!parsed.scroll_miniplayer);
        assert_eq!(parsed.volume, 40.0);
    }

    #[test]
    fn history_sync_defaults_for_old_files_and_round_trips_every_choice() {
        let old: Settings = serde_json::from_str(r#"{"volume": 40.0}"#).unwrap();
        assert_eq!(old.history_sync, HistorySync::Separate);
        assert_eq!(old.playback_buffer, PlaybackBuffer::Standard);
        for history_sync in [
            HistorySync::Separate,
            HistorySync::Merged,
            HistorySync::SendToYoutube,
        ] {
            let settings = Settings {
                history_sync,
                ..Settings::default()
            };
            let json = serde_json::to_string(&settings).unwrap();
            assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), settings);
        }
        for playback_buffer in [PlaybackBuffer::Standard, PlaybackBuffer::Compact] {
            let settings = Settings {
                playback_buffer,
                ..Settings::default()
            };
            let json = serde_json::to_string(&settings).unwrap();
            assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), settings);
        }
        assert!(old.update_notifications);
        assert!(!old.auto_update);
        assert!(old.last_notified_version.is_none());
        assert!(old.last_updated_version.is_none());
        assert!(old.last_updated_changelog.is_none());
    }

    #[test]
    fn round_trip() {
        let path = path("settings");
        let settings = Settings {
            language: crate::appearance::i18n::Locale::PortugueseBrazil,
            quality: Quality::Max(720),
            volume: 40.0,
            scroll_miniplayer: true,
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), settings);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_missing_file_is_the_defaults() {
        assert_eq!(
            Settings::load(&path("missing")).unwrap(),
            Settings::default()
        );
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
        assert_eq!(Settings::load(&path).unwrap(), Settings::default());
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
    fn the_interface_font_defaults_to_the_systems_and_is_kept() {
        use crate::appearance::theme::InterfaceFont;
        assert_eq!(Settings::default().interface_font, InterfaceFont::System);
        let settings: Settings = serde_json::from_str(r#"{"interface_font":"inter"}"#).unwrap();
        assert_eq!(settings.interface_font, InterfaceFont::Inter);
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains(r#""interface_font":"inter""#));
        let again: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(again.interface_font, InterfaceFont::Inter);
    }

    #[test]
    fn an_older_file_loads_as_auto() {
        let settings: Settings =
            serde_json::from_str(r#"{"quality":{"Max":720},"volume":40}"#).unwrap();
        assert_eq!(settings.theme, ThemeChoice::Auto);
        assert_eq!(settings.language, crate::appearance::i18n::Locale::English);
        assert_eq!(
            settings.interface_font,
            crate::appearance::theme::InterfaceFont::System
        );
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
                palette: omarchy,
            }),
            ..Settings::default()
        };
        assert_eq!(settings.palette(), omarchy);
        // A custom palette comes from its cache, matching the filename.
        let mut nord = Palette::dark();
        nord.accent = egui::Color32::from_rgb(4, 5, 6);
        settings.custom_theme_cache = Some(CustomTheme {
            filename: "Nord.json".into(),
            palette: nord,
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

    fn entry(button: SidebarButton, visible: bool) -> SidebarEntry {
        SidebarEntry { button, visible }
    }

    #[test]
    fn the_default_sidebar_lists_every_button_shown_in_the_original_order() {
        let buttons: Vec<_> = default_sidebar().iter().map(|e| e.button).collect();
        assert_eq!(buttons, SidebarButton::ALL);
        assert!(default_sidebar().iter().all(|e| e.visible));
        // A file from before the setting loads the default.
        let parsed: Settings = serde_json::from_str(r#"{"volume": 40.0}"#).unwrap();
        assert_eq!(parsed.sidebar, default_sidebar());
    }

    #[test]
    fn a_damaged_sidebar_list_is_completed_without_losing_the_other_settings() {
        let text = r#"{
            "volume": 37,
            "sidebar": [
                {"button": "history", "visible": false},
                {"button": "from_the_future"},
                {"button": "home"},
                {"button": "history", "visible": true},
                "nonsense",
                {"button": "settings", "visible": false}
            ]
        }"#;
        let settings: Settings = serde_json::from_str(text).unwrap();
        assert_eq!(settings.volume, 37.0);
        let got: Vec<_> = settings
            .sidebar
            .iter()
            .map(|e| (e.button, e.visible))
            .collect();
        use SidebarButton as B;
        assert_eq!(
            got,
            [
                (B::History, false),
                (B::Home, true),
                (B::Settings, true),
                (B::Subscriptions, true),
                (B::WatchLater, true),
                (B::Liked, true),
                (B::Playlists, true),
                (B::Notifications, true),
                (B::Account, true),
            ]
        );
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), settings);
    }

    #[test]
    fn account_and_settings_cannot_be_hidden() {
        let hidden = SidebarButton::ALL.map(|button| entry(button, false));
        for e in sanitize_sidebar(hidden.to_vec()) {
            assert_eq!(e.visible, !e.button.can_hide(), "{:?}", e.button);
        }
        assert!(!SidebarButton::Account.can_hide());
        assert!(!SidebarButton::Settings.can_hide());
        assert!(SidebarButton::Home.can_hide());
    }
}
