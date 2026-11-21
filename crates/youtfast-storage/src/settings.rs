//! The choices kept between runs: video quality and volume. One JSON file,
//! written atomically off the interface thread.

use std::path::Path;

use serde::{Deserialize, Serialize};

use youtfast_appearance::theme::{CustomTheme, Palette};
use youtfast_core::model::{Quality, SponsorCategories};

fn on() -> bool {
    true
}

fn default_volume() -> f64 {
    100.0
}

fn default_caption_scale() -> f64 {
    1.0
}

/// Maximum number of per-channel playback speeds remembered.
pub const MAX_CHANNEL_SPEEDS: usize = 500;
/// Minimum and maximum supported playback speeds.
pub const MIN_SPEED: f64 = 0.25;
pub const MAX_SPEED: f64 = 2.0;

fn default_playback_speed() -> f64 {
    1.0
}

/// Clamps finite playback speeds to mpv's supported range; non-finite
/// values fall back to normal speed.
pub fn sanitize_speed(speed: f64) -> f64 {
    if speed.is_finite() {
        speed.clamp(MIN_SPEED, MAX_SPEED)
    } else {
        1.0
    }
}

/// A remembered playback speed for one channel, serialized in recency order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelSpeed {
    pub channel_id: String,
    pub speed: f64,
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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceSync {
    pub enabled: bool,
    pub port: u16,
    pub peer_address: String,
    pub pairing_key: String,
    pub every_minutes: u32,
}

impl std::fmt::Debug for DeviceSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceSync")
            .field("enabled", &self.enabled)
            .field("port", &self.port)
            .field("every_minutes", &self.every_minutes)
            .finish_non_exhaustive()
    }
}

impl Default for DeviceSync {
    fn default() -> Self {
        Self {
            enabled: false,
            port: crate::sync::DEFAULT_SYNC_PORT,
            peer_address: String::new(),
            pairing_key: String::new(),
            every_minutes: 15,
        }
    }
}

impl DeviceSync {
    pub fn sanitized(mut self) -> Self {
        if self.port < crate::sync::MIN_SYNC_PORT {
            self.port = crate::sync::DEFAULT_SYNC_PORT;
        }
        self.peer_address = truncate_chars(
            self.peer_address.trim(),
            crate::sync::MAX_PEER_ADDRESS_CHARS,
        )
        .to_owned();
        self.pairing_key =
            truncate_chars(self.pairing_key.trim(), crate::sync::MAX_PAIRING_KEY_CHARS).to_owned();
        self.every_minutes = clamp_alerts_every(self.every_minutes);
        self
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> &str {
    input
        .char_indices()
        .nth(max_chars)
        .map_or(input, |(end, _)| &input[..end])
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub device_sync: DeviceSync,
    /// The interface language; older settings start in English too.
    #[serde(default)]
    pub language: youtfast_i18n::Locale,
    /// The face that draws the interface; older settings use the system's.
    #[serde(default)]
    pub interface_font: youtfast_appearance::theme::InterfaceFont,
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
    /// Global playback speed, 0.25 to 2.0; older settings start at 1.0.
    #[serde(
        default = "default_playback_speed",
        deserialize_with = "read_playback_speed"
    )]
    pub playback_speed: f64,
    /// Per-channel playback speeds, oldest first, bounded to [`MAX_CHANNEL_SPEEDS`].
    #[serde(
        default,
        deserialize_with = "read_channel_speeds",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub channel_speeds: Vec<ChannelSpeed>,
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
    /// Per-category SponsorBlock behaviors.
    #[serde(default)]
    pub sponsorblock_categories: SponsorCategories,
    /// Replace clickbait titles with community titles through DeArrow
    /// (sponsor.ajay.app), asked by a prefix of the id's hash so the video
    /// is not named. Off by default.
    #[serde(default)]
    pub dearrow_titles: bool,
    /// Replace thumbnails with community thumbnails through DeArrow
    /// (dearrow-thumb.ajay.app). Sends the video ID to DeArrow's thumbnail
    /// server; off by default.
    #[serde(default)]
    pub dearrow_thumbnails: bool,
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
    /// Hide watched videos on Home and Subscriptions.
    #[serde(default)]
    pub hide_watched: bool,
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

fn read_playback_speed<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    let speed = f64::deserialize(deserializer)?;
    Ok(sanitize_speed(speed))
}

fn read_channel_speeds<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<ChannelSpeed>, D::Error> {
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    let mut clean: Vec<ChannelSpeed> = Vec::new();
    for value in values {
        if let Ok(entry) = serde_json::from_value::<ChannelSpeed>(value)
            && !entry.channel_id.is_empty()
        {
            let speed = sanitize_speed(entry.speed);
            clean.retain(|existing| existing.channel_id != entry.channel_id);
            clean.push(ChannelSpeed {
                channel_id: entry.channel_id,
                speed,
            });
        }
    }
    if clean.len() > MAX_CHANNEL_SPEEDS {
        let excess = clean.len() - MAX_CHANNEL_SPEEDS;
        clean.drain(..excess);
    }
    Ok(clean)
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
            device_sync: DeviceSync::default(),
            language: youtfast_i18n::Locale::default(),
            interface_font: youtfast_appearance::theme::InterfaceFont::default(),
            history_sync: HistorySync::default(),
            playback_buffer: PlaybackBuffer::default(),
            quality: Quality::default(),
            volume: default_volume(),
            playback_speed: default_playback_speed(),
            channel_speeds: Vec::new(),
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
            sponsorblock_categories: SponsorCategories::default(),
            dearrow_titles: false,
            dearrow_thumbnails: false,
            update_notifications: on(),
            auto_update: false,
            last_notified_version: None,
            last_updated_version: None,
            last_updated_changelog: None,
            hide_watched: false,
        }
    }
}

impl Settings {
    /// Reads the settings. A missing file is the defaults; an unreadable
    /// one is moved aside rather than overwritten by the next save.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        crate::paths::load_json(path, "settings")
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        crate::paths::save_json(path, self)
    }

    /// The remembered speed for `channel_id`, if any.
    pub fn channel_speed(&self, channel_id: &str) -> Option<f64> {
        self.channel_speeds
            .iter()
            .rev()
            .find(|entry| entry.channel_id == channel_id)
            .map(|entry| sanitize_speed(entry.speed))
    }

    /// Stores `speed` for `channel_id`, moving it to most recent and keeping
    /// at most [`MAX_CHANNEL_SPEEDS`] entries (dropping the oldest).
    pub fn set_channel_speed(&mut self, channel_id: String, speed: f64) {
        if channel_id.is_empty() {
            return;
        }
        let speed = sanitize_speed(speed);
        self.channel_speeds
            .retain(|entry| entry.channel_id != channel_id);
        self.channel_speeds.push(ChannelSpeed { channel_id, speed });
        if self.channel_speeds.len() > MAX_CHANNEL_SPEEDS {
            let excess = self.channel_speeds.len() - MAX_CHANNEL_SPEEDS;
            self.channel_speeds.drain(..excess);
        }
    }

    /// Removes any remembered speed for `channel_id`. Returns whether an entry
    /// was removed.
    pub fn remove_channel_speed(&mut self, channel_id: &str) -> bool {
        let before = self.channel_speeds.len();
        self.channel_speeds
            .retain(|entry| entry.channel_id != channel_id);
        self.channel_speeds.len() != before
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
            .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../target").to_owned());
        std::fs::create_dir_all(&dir).unwrap();
        Path::new(&dir).join(format!(
            "youtfast-{name}-{}.json",
            youtfast_core::util::now()
        ))
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
            language: youtfast_i18n::Locale::PortugueseBrazil,
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
        use youtfast_appearance::theme::InterfaceFont;
        assert_eq!(Settings::default().interface_font, InterfaceFont::System);
        let settings: Settings = serde_json::from_str(r#"{"interface_font":"inter"}"#).unwrap();
        assert_eq!(settings.interface_font, InterfaceFont::Inter);
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains(r#""interface_font":"inter""#));
        let again: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(again.interface_font, InterfaceFont::Inter);
    }

    #[test]
    fn the_saved_interface_font_reads_older_and_unknown_values() {
        use youtfast_appearance::theme::InterfaceFont;
        let read = |json: &str| serde_json::from_str::<InterfaceFont>(json).unwrap();
        assert_eq!(read(r#""system""#), InterfaceFont::System);
        assert_eq!(read(r#""inter""#), InterfaceFont::Inter);
        assert_eq!(
            read(r#"{"family":"Adwaita Sans"}"#),
            InterfaceFont::Family("Adwaita Sans".into())
        );
        // What this version does not know is the system's face, not an
        // error that would set the whole settings file aside.
        for unknown in [
            r#""serif""#,
            r#"{"family":" "}"#,
            r#"{"file":"/x"}"#,
            "7",
            "null",
        ] {
            assert_eq!(read(unknown), InterfaceFont::System, "{unknown}");
        }
        let family = InterfaceFont::Family("Fira Sans".into());
        let json = serde_json::to_string(&family).unwrap();
        assert_eq!(json, r#"{"family":"Fira Sans"}"#);
        assert_eq!(read(&json), family);
        assert_eq!(
            serde_json::to_string(&InterfaceFont::Inter).unwrap(),
            r#""inter""#
        );
        // Inside a settings file, an unknown font leaves the rest readable.
        let settings: Settings =
            serde_json::from_str(r#"{"interface_font":{"file":"/x"},"volume":40}"#).unwrap();
        assert_eq!(settings.interface_font, InterfaceFont::System);
        assert!((settings.volume - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn an_older_file_loads_as_auto() {
        let settings: Settings =
            serde_json::from_str(r#"{"quality":{"Max":720},"volume":40}"#).unwrap();
        assert_eq!(settings.theme, ThemeChoice::Auto);
        assert_eq!(settings.language, youtfast_i18n::Locale::English);
        assert_eq!(
            settings.interface_font,
            youtfast_appearance::theme::InterfaceFont::System
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

    #[test]
    fn playback_speed_defaults_for_old_files_and_round_trips() {
        let old: Settings = serde_json::from_str(r#"{"volume": 40.0}"#).unwrap();
        assert_eq!(old.playback_speed, 1.0);
        assert!(old.channel_speeds.is_empty());

        let mut settings = Settings {
            playback_speed: 1.5,
            ..Settings::default()
        };
        settings.set_channel_speed("UC_one".into(), 1.75);
        settings.set_channel_speed("UC_two".into(), 0.75);
        let path = path("speed-roundtrip");
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), settings);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn channel_speeds_stay_bounded_dropping_oldest_and_sanitize_damaged_entries() {
        let mut settings = Settings::default();
        for i in 0..(MAX_CHANNEL_SPEEDS + 5) {
            settings.set_channel_speed(format!("UC_{i}"), 1.25);
        }
        assert_eq!(settings.channel_speeds.len(), MAX_CHANNEL_SPEEDS);
        // Oldest 5 (0..5) were dropped; refreshing an existing one moves it to newest.
        for i in 0..5 {
            assert_eq!(settings.channel_speed(&format!("UC_{i}")), None);
        }
        assert_eq!(settings.channel_speed("UC_5"), Some(1.25));
        settings.set_channel_speed("UC_5".into(), 1.5);
        settings.set_channel_speed("UC_extra".into(), 2.0);
        assert_eq!(settings.channel_speeds.len(), MAX_CHANNEL_SPEEDS);
        assert_eq!(settings.channel_speed("UC_5"), Some(1.5));
        assert_eq!(settings.channel_speed("UC_6"), None);
        assert!(settings.remove_channel_speed("UC_5"));
        assert!(!settings.remove_channel_speed("UC_5"));
        assert_eq!(settings.channel_speed("UC_5"), None);

        let damaged = r#"{
            "playback_speed": 99.0,
            "channel_speeds": [
                {"channel_id": "", "speed": 1.5},
                {"channel_id": "UC_a", "speed": 0.1},
                {"channel_id": "UC_a", "speed": 1.5},
                "invalid",
                {"channel_id": "UC_b", "speed": 5.0}
            ]
        }"#;
        let parsed: Settings = serde_json::from_str(damaged).unwrap();
        assert_eq!(parsed.playback_speed, 2.0);
        assert_eq!(parsed.channel_speed("UC_a"), Some(1.5));
        assert_eq!(parsed.channel_speed("UC_b"), Some(2.0));
        assert_eq!(parsed.channel_speeds.len(), 2);
    }

    #[test]
    fn sponsorblock_categories_default_to_original_behavior_and_survive_damaged_entries() {
        use youtfast_core::model::{SponsorBehavior, SponsorCategory};

        // Old settings.json with only `"sponsorblock": true` matches legacy behavior.
        let old_path = path("sb-old");
        std::fs::write(&old_path, r#"{"sponsorblock": true}"#).unwrap();
        let old = Settings::load(&old_path).unwrap();
        assert!(old.sponsorblock);
        assert_eq!(
            old.sponsorblock_categories.get(SponsorCategory::Sponsor),
            SponsorBehavior::AutoSkip
        );
        assert_eq!(
            old.sponsorblock_categories.get(SponsorCategory::Selfpromo),
            SponsorBehavior::AutoSkip
        );
        assert_eq!(
            old.sponsorblock_categories
                .get(SponsorCategory::Interaction),
            SponsorBehavior::AutoSkip
        );
        for cat in [
            SponsorCategory::Intro,
            SponsorCategory::Outro,
            SponsorCategory::Preview,
            SponsorCategory::MusicOfftopic,
            SponsorCategory::Filler,
        ] {
            assert_eq!(
                old.sponsorblock_categories.get(cat),
                SponsorBehavior::ManualSkip
            );
        }
        std::fs::remove_file(&old_path).unwrap();

        // Invalid/unknown values inside sponsorblock_categories fall back per category
        // without moving the file aside.
        let damaged_path = path("sb-damaged");
        std::fs::write(
            &damaged_path,
            r#"{
                "volume": 55.0,
                "sponsorblock": true,
                "sponsorblock_categories": {
                    "sponsor": "ignore",
                    "intro": "auto_skip",
                    "outro": "seek_bar_only",
                    "selfpromo": "not_a_behavior",
                    "preview": 42,
                    "future_category": "auto_skip"
                }
            }"#,
        )
        .unwrap();
        let loaded = Settings::load(&damaged_path).unwrap();
        assert!(damaged_path.exists());
        assert_eq!(loaded.volume, 55.0);
        assert!(loaded.sponsorblock);
        assert_eq!(
            loaded.sponsorblock_categories.get(SponsorCategory::Sponsor),
            SponsorBehavior::Ignore
        );
        assert_eq!(
            loaded.sponsorblock_categories.get(SponsorCategory::Intro),
            SponsorBehavior::AutoSkip
        );
        assert_eq!(
            loaded.sponsorblock_categories.get(SponsorCategory::Outro),
            SponsorBehavior::SeekBarOnly
        );
        assert_eq!(
            loaded
                .sponsorblock_categories
                .get(SponsorCategory::Selfpromo),
            SponsorBehavior::AutoSkip
        );
        assert_eq!(
            loaded.sponsorblock_categories.get(SponsorCategory::Preview),
            SponsorBehavior::ManualSkip
        );
        std::fs::remove_file(&damaged_path).unwrap();

        // Non-object sponsorblock_categories also falls back cleanly.
        let non_object: Settings =
            serde_json::from_str(r#"{"sponsorblock_categories": "invalid"}"#).unwrap();
        assert_eq!(
            non_object.sponsorblock_categories,
            SponsorCategories::default()
        );

        // Customized categories round-trip through save/load.
        let rt_path = path("sb-roundtrip");
        let mut settings = Settings {
            sponsorblock: true,
            ..Settings::default()
        };
        settings
            .sponsorblock_categories
            .set(SponsorCategory::Intro, SponsorBehavior::AutoSkip);
        settings
            .sponsorblock_categories
            .set(SponsorCategory::Filler, SponsorBehavior::SeekBarOnly);
        settings
            .sponsorblock_categories
            .set(SponsorCategory::Sponsor, SponsorBehavior::Ignore);
        settings.save(&rt_path).unwrap();
        assert_eq!(Settings::load(&rt_path).unwrap(), settings);
        std::fs::remove_file(&rt_path).unwrap();
    }

    #[test]
    fn hide_watched_defaults_false_for_old_files_and_round_trips() {
        let old: Settings = serde_json::from_str(r#"{"volume": 40.0}"#).unwrap();
        assert!(!old.hide_watched);

        let settings = Settings {
            hide_watched: true,
            ..Settings::default()
        };
        let path = path("hide-watched-roundtrip");
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert!(loaded.hide_watched);
        assert_eq!(loaded, settings);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn dearrow_settings_default_to_false_and_round_trip() {
        let defaults = Settings::default();
        assert!(!defaults.dearrow_titles);
        assert!(!defaults.dearrow_thumbnails);

        let legacy: Settings = serde_json::from_str(r#"{"volume": 65.0}"#).unwrap();
        assert!(!legacy.dearrow_titles);
        assert!(!legacy.dearrow_thumbnails);

        let enabled = Settings {
            dearrow_titles: true,
            dearrow_thumbnails: true,
            ..Settings::default()
        };
        let json = serde_json::to_string(&enabled).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), enabled);
    }

    #[test]
    fn device_sync_defaults_for_old_files_and_sanitizes_fields() {
        let old: Settings = serde_json::from_str(r#"{"volume": 40.0}"#).unwrap();
        assert_eq!(old.device_sync, DeviceSync::default());
        assert!(!old.device_sync.enabled);
        assert_eq!(old.device_sync.port, crate::sync::DEFAULT_SYNC_PORT);
        assert_eq!(old.device_sync.every_minutes, 15);

        let sanitized = DeviceSync {
            enabled: true,
            port: 80,
            peer_address: format!("  {}  ", "a".repeat(300)),
            pairing_key: format!("  {}  ", "b".repeat(200)),
            every_minutes: 42,
        }
        .sanitized();
        assert_eq!(sanitized.port, crate::sync::DEFAULT_SYNC_PORT);
        assert_eq!(
            sanitized.peer_address.chars().count(),
            crate::sync::MAX_PEER_ADDRESS_CHARS
        );
        assert_eq!(
            sanitized.pairing_key.chars().count(),
            crate::sync::MAX_PAIRING_KEY_CHARS
        );
        assert_eq!(sanitized.every_minutes, 30);

        let settings = Settings {
            device_sync: DeviceSync {
                enabled: true,
                port: 40000,
                peer_address: "100.64.0.2:40000".into(),
                pairing_key: crate::sync::generate_pairing_key(),
                every_minutes: 5,
            },
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), settings);
    }
}
