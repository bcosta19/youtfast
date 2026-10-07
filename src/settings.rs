//! The choices kept between runs: video quality and volume. One JSON file,
//! written atomically off the interface thread.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::Quality;

fn default_volume() -> f64 {
    100.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// The most a video may stream at.
    #[serde(default)]
    pub quality: Quality,
    /// mpv's volume, 0 to 100.
    #[serde(default = "default_volume")]
    pub volume: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            quality: Quality::default(),
            volume: default_volume(),
        }
    }
}

impl Settings {
    /// Reads the settings. A missing file is the defaults; an unreadable
    /// one is moved aside rather than overwritten by the next save.
    pub fn load(path: &Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(error) => {
                log::error!("could not read the settings: {error}");
                return Self::default();
            }
        };
        match serde_json::from_str(&text) {
            Ok(settings) => settings,
            Err(error) => {
                let mut aside = path.as_os_str().to_owned();
                aside.push(format!(".unreadable-{}", crate::util::now()));
                log::error!("the settings are unreadable ({error}); keeping them as {aside:?}");
                if let Err(error) = std::fs::rename(path, &aside) {
                    log::error!("could not move the unreadable settings aside: {error}");
                }
                Self::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        crate::paths::write_atomic(path, &json)
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
    fn round_trip() {
        let path = path("settings");
        let settings = Settings {
            quality: Quality::Max(720),
            volume: 40.0,
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
        let found = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| {
                entry.file_name().to_string_lossy().starts_with(&format!(
                    "{}.unreadable-",
                    path.file_name().unwrap().to_string_lossy()
                ))
            });
        assert!(found, "{aside} should exist");
    }
}
