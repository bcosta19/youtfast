//! Local application diagnostics. Every record passes the privacy redactor
//! before reaching stderr or the bounded, owner-only log files.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 2 * 1024 * 1024;

pub fn init(filter: &str) -> Result<(), log::SetLoggerError> {
    let file = directories::ProjectDirs::from("", "", "youtfast").and_then(|dirs| {
        let state = dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir());
        Rotating::open(state.join("logs"), MAX_BYTES).ok()
    });
    if file.is_none() {
        eprintln!("YouTFast could not open its local diagnostic log.");
    }
    let mut builder = logger(
        &std::env::var("RUST_LOG").unwrap_or_else(|_| filter.to_owned()),
        file,
    );
    builder.try_init()?;
    log::info!(
        "Starting youtfast {} on {} ({})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    Ok(())
}

fn logger(filter: &str, file: Option<Rotating>) -> env_logger::Builder {
    let mut builder = env_logger::Builder::new();
    builder.parse_filters(filter);
    builder.target(env_logger::Target::Pipe(Box::new(Output { file })));
    builder.format(|out, record| {
        let raw = record.args().to_string();
        let message = crate::redact_log(record, &raw).map_or(raw, |redacted| redacted.into_owned());
        writeln!(
            out,
            "[{} {} {}] {}",
            out.timestamp(),
            record.level(),
            if record.target().starts_with("youtfast") {
                "youtfast"
            } else if record.target() == "mpv" {
                "mpv"
            } else {
                "dependency"
            },
            message
        )
    });
    builder
}

struct Output {
    file: Option<Rotating>,
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let _ = io::stderr().write_all(bytes);
        if let Some(file) = &mut self.file
            && file.write_all(bytes).is_err()
        {
            self.file = None;
            eprintln!("YouTFast stopped writing its diagnostic log.");
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = &mut self.file {
            let _ = file.flush();
        }
        Ok(())
    }
}

struct Rotating {
    path: PathBuf,
    file: Option<File>,
    length: u64,
    limit: u64,
}

fn regular(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() => {
            Err(io::Error::other("Non-regular diagnostic target"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn open_file(path: &Path) -> io::Result<File> {
    regular(path)?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

impl Rotating {
    fn open(dir: PathBuf, limit: u64) -> io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        }
        let path = dir.join("youtfast.log");
        regular(&path)?;
        // Windows append handles lack the write access `set_len` needs, so an
        // oversized log is capped through its own handle before appending.
        if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() > limit) {
            OpenOptions::new().write(true).open(&path)?.set_len(limit)?;
        }
        let file = open_file(&path)?;
        let length = file.metadata()?.len().min(limit);
        Ok(Self {
            path,
            file: Some(file),
            length,
            limit,
        })
    }
}

impl Write for Rotating {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // The redactor produces short messages, including for external payloads.
        let bytes = &bytes[..bytes.len().min(self.limit as usize)];
        if self.length + bytes.len() as u64 > self.limit {
            let previous = self.path.with_extension("log.1");
            regular(&previous)?;
            // Close before renaming for Windows as well as Unix.
            drop(self.file.take());
            if previous.exists() {
                std::fs::remove_file(&previous)?;
            }
            std::fs::rename(&self.path, &previous)?;
            self.file = Some(open_file(&self.path)?);
            self.length = 0;
        }
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("Diagnostic output closed"))?
            .write_all(bytes)?;
        self.length += bytes.len() as u64;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("Diagnostic output closed"))?
            .flush()
    }
}

/// These exact codes are safe to preserve even under verbose logging. Never
/// accept a prefix: an appended dependency error could carry a signed URL.
pub fn controlled(message: &str) -> bool {
    matches!(
        message,
        "history: playback report failed"
            | "image: download failed"
            | "image: HTTP access denied"
            | "image: unavailable"
            | "image: rate limited"
            | "image: server error"
            | "image: response too large"
            | "image: invalid data"
            | "image: cache repaired"
            | "animation: cached"
            | "animation: streaming"
            | "animation: preview only"
            | "animation: invalid data"
            | "animation: memory budget reached"
            | "animation: worker unavailable"
            | "animation: decode budget reached"
            | "image: source refresh failed"
            | "image: source refreshed"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_rotate_without_growing_past_the_two_file_budget() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("diagnostic-rotation-{}", std::process::id()));
        let mut log = Rotating::open(dir.clone(), 64).unwrap();
        for _ in 0..50 {
            log.write_all(b"controlled diagnostic\n").unwrap();
        }
        drop(log);
        assert!(std::fs::metadata(dir.join("youtfast.log")).unwrap().len() <= 64);
        assert!(std::fs::metadata(dir.join("youtfast.log.1")).unwrap().len() <= 64);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_oversized_existing_log_is_capped_and_still_appended() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("diagnostic-oversized-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("youtfast.log"), [b'x'; 200]).unwrap();
        let mut log = Rotating::open(dir.clone(), 64).unwrap();
        assert_eq!(log.length, 64);
        log.write_all(b"next\n").unwrap();
        drop(log);
        assert_eq!(std::fs::read(dir.join("youtfast.log")).unwrap(), b"next\n");
        assert_eq!(
            std::fs::metadata(dir.join("youtfast.log.1")).unwrap().len(),
            64
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn diagnostic_codes_never_accept_appended_private_data() {
        assert!(controlled("animation: streaming"));
        assert!(controlled("history: playback report failed"));
        assert!(!controlled(
            "history: playback report failed synthetic-private-url"
        ));
        assert!(!controlled(
            "animation: streaming https://example.invalid/signed?secret=x"
        ));
    }
    #[test]
    fn persisted_records_are_redacted_before_writing_and_owner_only() {
        use log::Log;
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("diagnostic-privacy-{}", std::process::id()));
        let file = Rotating::open(dir.clone(), 1024).unwrap();
        let logger = logger("trace", Some(file)).build();
        logger.log(
            &log::Record::builder()
                .target("youtfast::images")
                .level(log::Level::Warn)
                .args(format_args!("image: HTTP access denied"))
                .build(),
        );
        logger.log(
            &log::Record::builder()
                .target("https://private.invalid/signed?secret=synthetic")
                .level(log::Level::Warn)
                .args(format_args!("cookie=synthetic account=synthetic"))
                .build(),
        );
        logger.flush();
        drop(logger);
        let content = std::fs::read_to_string(dir.join("youtfast.log")).unwrap();
        assert!(content.contains("image: HTTP access denied"));
        for private in ["synthetic", "private.invalid", "cookie="] {
            assert!(!content.contains(private));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(dir.join("youtfast.log"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
