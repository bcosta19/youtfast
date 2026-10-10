//! Where YouTFast keeps its files, and how it writes them.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Clone, Debug)]
pub struct Dirs {
    /// The library (`~/.local/share/youtfast` on Linux).
    pub data: PathBuf,
    /// The settings (`~/.config/youtfast` on Linux).
    pub config: PathBuf,
    /// rustypipe's client data, never credentials (`~/.cache/youtfast`).
    pub cache: PathBuf,
}

impl Dirs {
    pub fn new() -> anyhow::Result<Self> {
        let project = directories::ProjectDirs::from("", "", "youtfast")
            .ok_or_else(|| anyhow::anyhow!("no home directory to keep the library in"))?;
        let dirs = Self {
            data: project.data_dir().to_owned(),
            config: project.config_dir().to_owned(),
            cache: project.cache_dir().to_owned(),
        };
        std::fs::create_dir_all(&dirs.data)?;
        std::fs::create_dir_all(&dirs.config)?;
        std::fs::create_dir_all(&dirs.cache)?;
        Ok(dirs)
    }

    pub fn library(&self) -> PathBuf {
        self.data.join("library.json")
    }

    pub fn settings(&self) -> PathBuf {
        self.config.join("settings.json")
    }

    pub fn youtube_cache(&self) -> PathBuf {
        self.cache.join("rustypipe.json")
    }
}

/// Replaces `path` with `contents` in one step: the data goes to a file
/// beside it, reaches the disk, and is renamed over the old one, so a crash
/// leaves either the old file or the new, never half of one.
pub fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, path)
}

/// Reads a JSON file; `what` names it in the log ("library"). A missing
/// file is the default; an unreadable one is moved aside rather than
/// overwritten by the next save.
pub fn load_json<T: DeserializeOwned + Default>(path: &Path, what: &str) -> T {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return T::default(),
        Err(error) => {
            log::error!("could not read the {what}: {error}");
            return T::default();
        }
    };
    match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            let mut aside = path.as_os_str().to_owned();
            aside.push(format!(".unreadable-{}", crate::util::now()));
            log::error!("the {what} file is unreadable ({error}); keeping it as {aside:?}");
            if let Err(error) = std::fs::rename(path, &aside) {
                log::error!("could not move the unreadable {what} aside: {error}");
            }
            T::default()
        }
    }
}

/// Writes `value` as pretty JSON, atomically (see [`write_atomic`]).
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    write_atomic(path, &json)
}
