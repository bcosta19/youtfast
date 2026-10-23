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
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() => {
            return Err(std::io::Error::other(
                "Refusing to replace a non-regular file",
            ));
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
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
        std::fs::rename(&temporary, path)?;
        sync_parent(path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

/// Reads a JSON file; `what` names it in the log ("library"). A missing
/// file is the default; an unreadable one is moved aside rather than
/// overwritten by the next save.
pub fn load_json<T: DeserializeOwned + Default>(path: &Path, what: &str) -> std::io::Result<T> {
    load_json_with(path, what, preserve_unreadable)
}

fn load_json_with<T: DeserializeOwned + Default>(
    path: &Path,
    what: &str,
    preserve: impl FnOnce(&Path, &[u8]) -> std::io::Result<PathBuf>,
) -> std::io::Result<T> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(error) => return Err(error),
    };
    match serde_json::from_slice(&bytes) {
        Ok(value) => Ok(value),
        Err(_) => {
            preserve(path, &bytes)?;
            log::error!("the {what} file was unreadable; a private backup was kept");
            Ok(T::default())
        }
    }
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent(_: &Path) -> std::io::Result<()> {
    Ok(())
}

fn preserve_unreadable(path: &Path, bytes: &[u8]) -> std::io::Result<PathBuf> {
    static BACKUPS: AtomicU64 = AtomicU64::new(0);
    loop {
        let mut aside = path.as_os_str().to_owned();
        aside.push(format!(
            ".unreadable-{}-{}-{}",
            crate::util::now(),
            std::process::id(),
            BACKUPS.fetch_add(1, Ordering::Relaxed)
        ));
        let aside = PathBuf::from(aside);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = match options.open(&aside) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            let _ = std::fs::remove_file(&aside);
            return Err(error);
        }
        sync_parent(&aside)?;
        std::fs::remove_file(path)?;
        sync_parent(path)?;
        return Ok(aside);
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

    fn test_path(name: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "review-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ))
    }

    #[test]
    fn invalid_utf8_is_preserved_before_defaults_can_be_saved() {
        let path = test_path("utf8");
        let original = b"{\"history\": [\xff]}";
        std::fs::write(&path, original).unwrap();
        let loaded: serde_json::Value = load_json(&path, "test").unwrap();
        assert!(loaded.is_null());
        assert!(!path.exists());
        let prefix = format!(
            "{}.unreadable-",
            path.file_name().unwrap().to_string_lossy()
        );
        let backups: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
            .collect();
        assert_eq!(backups.len(), 1);
        save_json(&path, &loaded).unwrap();
        assert_eq!(std::fs::read(backups[0].path()).unwrap(), original);
        std::fs::remove_file(backups[0].path()).unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_failed_backup_is_an_error_not_a_default_value() {
        let path = test_path("backup-failure");
        std::fs::write(&path, b"broken").unwrap();
        let result = load_json_with::<serde_json::Value>(&path, "test", |_, _| {
            Err(std::io::ErrorKind::PermissionDenied.into())
        });
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        std::fs::remove_file(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn permission_denied_read_never_becomes_defaults() {
        use std::os::unix::fs::PermissionsExt;
        let path = test_path("permission-denied");
        std::fs::write(&path, b"{\"private\":true}").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o0)).unwrap();
        let result = load_json::<serde_json::Value>(&path, "test");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(bytes, b"{\"private\":true}");
    }

    #[cfg(unix)]
    #[test]
    fn atomic_writes_never_replace_a_symbolic_link() {
        let original = test_path("link-original");
        let link = test_path("link");
        std::fs::write(&original, b"original").unwrap();
        std::os::unix::fs::symlink(&original, &link).unwrap();
        assert!(write_atomic(&link, b"replacement").is_err());
        assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(std::fs::read(&original).unwrap(), b"original");
        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(original).unwrap();
    }

    #[test]
    fn read_errors_are_not_treated_as_missing_files() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        assert!(load_json::<serde_json::Value>(&dir, "test").is_err());
    }

    #[test]
    fn backups_created_in_the_same_second_never_replace_each_other() {
        let path = test_path("backups");
        std::fs::write(&path, b"first").unwrap();
        let first = preserve_unreadable(&path, b"first").unwrap();
        std::fs::write(&path, b"second").unwrap();
        let second = preserve_unreadable(&path, b"second").unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), b"first");
        assert_eq!(std::fs::read(&second).unwrap(), b"second");
        std::fs::remove_file(first).unwrap();
        std::fs::remove_file(second).unwrap();
    }

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
