//! The yt-dlp and Deno that the Windows ZIP ships next to `youtfast.exe`.
//!
//! Windows users do not have either on `PATH`, so playback looks beside the
//! running executable first and only then falls back to `PATH`, where every
//! other platform (and a development build) keeps looking. The choice is
//! made by plain options, mpv's `ytdl_path` for the hook and yt-dlp's
//! `--js-runtimes` for the challenge solver, rather than by editing the
//! process's `PATH`: they name exactly the files that were checked, and
//! nothing else inherits them.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// What was found beside the executable.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tools {
    pub ytdlp: Option<PathBuf>,
    pub deno: Option<PathBuf>,
}

impl Tools {
    /// Looks in `dir` for `yt-dlp` and `deno` plus `suffix` (`.exe` on
    /// Windows); `exists` says whether a file is there. Elsewhere than
    /// Windows `dir` is `None` and nothing is found.
    pub fn find(dir: Option<&Path>, suffix: &str, exists: impl Fn(&Path) -> bool) -> Self {
        let in_dir = |name: &str| {
            let path = dir?.join(format!("{name}{suffix}"));
            // mpv takes `ytdl_path` as a comma-separated list, and the
            // value is later split again by mpv's option parser; a path
            // with a comma cannot be named safely, so it is left to PATH.
            (exists(&path) && !path.to_string_lossy().contains(',')).then_some(path)
        };
        Self {
            ytdlp: in_dir("yt-dlp"),
            deno: in_dir("deno"),
        }
    }

    /// The bundled tools of this process, looked up once.
    pub fn current() -> &'static Self {
        static TOOLS: OnceLock<Tools> = OnceLock::new();
        TOOLS.get_or_init(|| {
            if cfg!(windows) {
                let exe = std::env::current_exe().ok();
                Self::find(exe.as_deref().and_then(Path::parent), ".exe", Path::is_file)
            } else {
                Self::default()
            }
        })
    }

    /// The `script-opts` value that points mpv's ytdl hook at the bundled
    /// yt-dlp, `None` when there is none.
    pub fn script_opts(&self) -> Option<String> {
        let path = self.ytdlp.as_ref()?.to_str()?;
        Some(format!("ytdl_hook-ytdl_path={}", quoted(path)))
    }

    /// The yt-dlp option that selects the bundled Deno as its JavaScript
    /// runtime, `None` when there is none.
    pub fn js_runtime_option(&self) -> Option<String> {
        let path = self.deno.as_ref()?.to_str()?;
        Some(format!("js-runtimes={}", quoted(&format!("deno:{path}"))))
    }
}

/// mpv's `%length%value` quoting for a value in a key-value list, so that
/// spaces, commas or brackets in an install path cannot split it.
fn quoted(value: &str) -> String {
    format!("%{}%{value}", value.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beside(dir: &str, files: &'static [&'static str]) -> impl Fn(&Path) -> bool {
        let dir = PathBuf::from(dir);
        move |path| files.iter().any(|file| path == dir.join(file))
    }

    #[test]
    fn finds_what_is_beside_the_executable() {
        let exists = beside("app", &["yt-dlp.exe", "deno.exe"]);
        let tools = Tools::find(Some(Path::new("app")), ".exe", exists);
        assert_eq!(tools.ytdlp, Some(PathBuf::from("app").join("yt-dlp.exe")));
        assert_eq!(tools.deno, Some(PathBuf::from("app").join("deno.exe")));
    }

    #[test]
    fn a_missing_tool_falls_back_to_path_on_its_own() {
        let exists = beside("app", &["deno.exe"]);
        let tools = Tools::find(Some(Path::new("app")), ".exe", exists);
        assert_eq!(tools.ytdlp, None);
        assert_eq!(tools.script_opts(), None);
        assert!(tools.js_runtime_option().is_some());
    }

    #[test]
    fn no_directory_or_another_suffix_finds_nothing() {
        let exists = beside("app", &["yt-dlp.exe", "deno.exe"]);
        assert_eq!(Tools::find(None, ".exe", &exists), Tools::default());
        assert_eq!(
            Tools::find(Some(Path::new("app")), "", &exists),
            Tools::default()
        );
        assert_eq!(Tools::default().script_opts(), None);
        assert_eq!(Tools::default().js_runtime_option(), None);
    }

    #[test]
    fn a_path_with_a_comma_is_left_to_path() {
        let exists = |_: &Path| true;
        assert_eq!(
            Tools::find(Some(Path::new("a,b")), ".exe", exists),
            Tools::default()
        );
    }

    #[test]
    fn options_quote_the_paths_by_length() {
        let tools = Tools {
            ytdlp: Some(PathBuf::from(r"C:\Program Files\YouTFast\yt-dlp.exe")),
            deno: Some(PathBuf::from(r"C:\Program Files\YouTFast\deno.exe")),
        };
        assert_eq!(
            tools.script_opts().unwrap(),
            r"ytdl_hook-ytdl_path=%36%C:\Program Files\YouTFast\yt-dlp.exe"
        );
        assert_eq!(
            tools.js_runtime_option().unwrap(),
            r"js-runtimes=%39%deno:C:\Program Files\YouTFast\deno.exe"
        );
    }
}
