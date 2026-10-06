//! Where YouTFast keeps its files, and how it writes them.

use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Dirs {
    /// The library (`~/.local/share/youtfast` on Linux).
    pub data: PathBuf,
    /// rustypipe's client data, never credentials (`~/.cache/youtfast`).
    pub cache: PathBuf,
}

impl Dirs {
    pub fn new() -> anyhow::Result<Self> {
        let project = directories::ProjectDirs::from("", "", "youtfast")
            .ok_or_else(|| anyhow::anyhow!("no home directory to keep the library in"))?;
        let dirs = Self {
            data: project.data_dir().to_owned(),
            cache: project.cache_dir().to_owned(),
        };
        std::fs::create_dir_all(&dirs.data)?;
        std::fs::create_dir_all(&dirs.cache)?;
        Ok(dirs)
    }

    pub fn library(&self) -> PathBuf {
        self.data.join("library.json")
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
