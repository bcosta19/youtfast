//! Where YouTFast keeps its files, and how it writes them.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
        only_the_owner(&dirs.data);
        only_the_owner(&dirs.config);
        only_the_owner(&dirs.cache);
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

/// Restricts `dir` to its owner; the library and settings behind it are
/// nobody else's reading. On other platforms the defaults apply.
#[cfg(unix)]
fn only_the_owner(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)) {
        log::warn!("could not restrict {} to its owner: {error}", dir.display());
    }
}

#[cfg(not(unix))]
fn only_the_owner(_: &Path) {}

/// Replaces `path` with `contents` in one step: the data goes to a file
/// beside it, reaches the disk, and is renamed over the old one, so a crash
/// leaves either the old file or the new, never half of one. Each write
/// has its own temporary file, so writes racing on one path (the backend's
/// saves, the one at exit) never rename or fill each other's.
pub fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    static WRITES: AtomicU64 = AtomicU64::new(0);
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(
        ".{}-{}.tmp",
        std::process::id(),
        WRITES.fetch_add(1, Ordering::Relaxed)
    ));
    let temporary = PathBuf::from(temporary);
    let written = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        // What YouTFast writes is its user's alone: the file is born
        // readable only to them, never open to the umask's default
        // between creation and a later chmod.
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_racing_on_one_file_all_land_whole() {
        let dir = std::env::var("CARGO_TARGET_TMPDIR")
            .unwrap_or_else(|_| format!("{}/target", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).unwrap();
        let path = Path::new(&dir).join(format!("youtfast-race-{}.json", crate::util::now()));
        let writers: Vec<_> = (0..4u8)
            .map(|writer| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let contents = vec![writer; 64 * 1024];
                    (0..50)
                        .map(|_| write_atomic(&path, &contents))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for writer in writers {
            for result in writer.join().unwrap() {
                result.unwrap();
            }
        }
        // One writer's whole contents, never a mix.
        let written = std::fs::read(&path).unwrap();
        assert_eq!(written.len(), 64 * 1024);
        assert!(written.iter().all(|byte| *byte == written[0]));
        std::fs::remove_file(&path).unwrap();
    }

    /// A written file is readable by its owner alone.
    #[cfg(unix)]
    #[test]
    fn writes_are_the_owners_alone() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::var("CARGO_TARGET_TMPDIR")
            .unwrap_or_else(|_| format!("{}/target", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).unwrap();
        let path = Path::new(&dir).join(format!("youtfast-mode-{}.json", crate::util::now()));
        write_atomic(&path, b"data").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::remove_file(&path).unwrap();
    }
}
