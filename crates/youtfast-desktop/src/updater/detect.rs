//! Installation-method detection and filesystem writability probes.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{InstallMethod, hidden_command};

/// Snapshot of the environment used to classify the active installation method.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallProbe {
    pub os: String,
    pub arch: String,
    pub exe: PathBuf,
    pub flatpak_env: bool,
    pub pacman_owns: bool,
    pub pkexec_available: bool,
    pub graphical_auth_available: bool,
    pub is_root: bool,
    pub windows_uninstaller_present: bool,
    pub target_writable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstallTarget {
    pub method: InstallMethod,
    pub can_install: bool,
}

pub fn classify_install_target(probe: &InstallProbe) -> InstallTarget {
    if probe.os == "linux" && (probe.flatpak_env || probe.exe.starts_with("/app/")) {
        return InstallTarget {
            method: InstallMethod::Flatpak,
            can_install: false,
        };
    }
    if is_dev_or_readonly_managed_exe(&probe.exe) {
        return InstallTarget {
            method: InstallMethod::ManualOnly,
            can_install: false,
        };
    }
    match (probe.os.as_str(), probe.arch.as_str()) {
        ("linux", "x86_64") => {
            if is_system_usr_bin(&probe.exe) && probe.pacman_owns {
                let can_install =
                    probe.is_root || (probe.pkexec_available && probe.graphical_auth_available);
                InstallTarget {
                    method: InstallMethod::ArchPacman,
                    can_install,
                }
            } else if is_system_readonly_path(&probe.exe) && !probe.target_writable {
                InstallTarget {
                    method: InstallMethod::ManualOnly,
                    can_install: false,
                }
            } else {
                InstallTarget {
                    method: InstallMethod::LinuxNative,
                    can_install: probe.target_writable,
                }
            }
        }
        ("windows", "x86_64") => {
            let method = if probe.windows_uninstaller_present {
                InstallMethod::WindowsSetup
            } else {
                InstallMethod::WindowsPortable
            };
            InstallTarget {
                method,
                can_install: probe.target_writable,
            }
        }
        ("macos", "aarch64") => {
            if find_app_bundle(&probe.exe).is_some() {
                InstallTarget {
                    method: InstallMethod::MacosBundle,
                    can_install: probe.target_writable,
                }
            } else {
                InstallTarget {
                    method: InstallMethod::ManualOnly,
                    can_install: false,
                }
            }
        }
        _ => InstallTarget {
            method: InstallMethod::ManualOnly,
            can_install: false,
        },
    }
}

pub fn detect_install_target() -> InstallTarget {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let Ok(raw_exe) = std::env::current_exe() else {
        return InstallTarget {
            method: InstallMethod::ManualOnly,
            can_install: false,
        };
    };
    let exe = clean_exe_path(&std::fs::canonicalize(&raw_exe).unwrap_or(raw_exe));
    let flatpak_env =
        std::env::var_os("FLATPAK_ID").is_some() || Path::new("/.flatpak-info").is_file();
    let is_root = is_effective_root();
    let pacman_owns = os == "linux"
        && is_system_usr_bin(&exe)
        && command_succeeds("pacman", &["-Qq", "youtfast"]);
    let pkexec_available = os == "linux" && command_in_path("pkexec");
    let graphical_auth_available = os == "linux"
        && (std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some());
    let windows_uninstaller_present = os == "windows"
        && exe
            .parent()
            .is_some_and(|dir| dir.join("Uninstall.exe").is_file());
    let writable_dir = if os == "macos" {
        find_app_bundle(&exe)
            .and_then(|app| app.parent().map(Path::to_path_buf))
            .or_else(|| exe.parent().map(Path::to_path_buf))
    } else {
        exe.parent().map(Path::to_path_buf)
    };
    let target_writable = writable_dir.as_deref().is_some_and(dir_is_writable);
    classify_install_target(&InstallProbe {
        os: os.to_owned(),
        arch: arch.to_owned(),
        exe,
        flatpak_env,
        pacman_owns,
        pkexec_available,
        graphical_auth_available,
        is_root,
        windows_uninstaller_present,
        target_writable,
    })
}

fn is_dev_or_readonly_managed_exe(exe: &Path) -> bool {
    let raw = exe.to_string_lossy();
    if raw.starts_with("/nix/store/") {
        return true;
    }
    let parts: Vec<_> = exe
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect();
    for window in parts.windows(2) {
        if window[0] == "target" && matches!(window[1].as_ref(), "debug" | "release" | "ci") {
            return true;
        }
        if window[0] == ".cargo" && window[1] == "bin" {
            return true;
        }
    }
    // mbx resolves the checkout's `target` symlink to a versioned target
    // cache, so canonical paths no longer contain the checkout component.
    for window in parts.windows(4) {
        if window[0] == "targets"
            && window[1] == "v1"
            && window[2].len() == 64
            && window[2].bytes().all(|byte| byte.is_ascii_hexdigit())
            && matches!(window[3].as_ref(), "debug" | "release" | "ci")
        {
            return true;
        }
    }
    false
}

fn is_system_usr_bin(exe: &Path) -> bool {
    matches!(exe.to_str(), Some("/usr/bin/youtfast" | "/bin/youtfast"))
}

fn is_system_readonly_path(exe: &Path) -> bool {
    exe.starts_with("/usr/") || exe.starts_with("/opt/") || exe.starts_with("/bin/")
}

pub(super) fn find_app_bundle(exe: &Path) -> Option<PathBuf> {
    let contents = exe.parent()?.parent()?;
    let app = contents.parent()?;
    (app.extension().and_then(|ext| ext.to_str()) == Some("app")).then(|| app.to_path_buf())
}

pub(super) fn dir_is_writable(dir: &Path) -> bool {
    static PROBES: AtomicU64 = AtomicU64::new(0);
    if !dir.is_dir() {
        return false;
    }
    let probe = dir.join(format!(
        ".youtfast-write-probe-{}-{}",
        std::process::id(),
        PROBES.fetch_add(1, Ordering::Relaxed)
    ));
    let Ok(file) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    else {
        return false;
    };
    drop(file);
    let _ = std::fs::remove_file(&probe);
    true
}

pub(super) fn is_effective_root() -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata("/proc/self").is_ok_and(|meta| meta.uid() == 0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

fn command_in_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
}

fn command_succeeds(program: &str, args: &[&str]) -> bool {
    hidden_command(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Strips the `" (deleted)"` suffix Linux's `/proc/self/exe` appends when the
/// running binary has already been replaced once during the current session.
pub(super) fn clean_exe_path(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    match raw.strip_suffix(" (deleted)") {
        Some(clean) => PathBuf::from(clean),
        None => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_install_methods_and_matching_release_assets() {
        let base = InstallProbe {
            os: "linux".into(),
            arch: "x86_64".into(),
            exe: PathBuf::from("/home/user/.local/bin/youtfast"),
            flatpak_env: false,
            pacman_owns: false,
            pkexec_available: true,
            graphical_auth_available: true,
            is_root: false,
            windows_uninstaller_present: false,
            target_writable: true,
        };

        assert_eq!(
            classify_install_target(&base),
            InstallTarget {
                method: InstallMethod::LinuxNative,
                can_install: true,
            }
        );

        let flatpak = InstallProbe {
            exe: PathBuf::from("/app/bin/youtfast"),
            flatpak_env: true,
            ..base.clone()
        };
        assert_eq!(
            classify_install_target(&flatpak),
            InstallTarget {
                method: InstallMethod::Flatpak,
                can_install: false,
            }
        );

        let arch = InstallProbe {
            exe: PathBuf::from("/usr/bin/youtfast"),
            pacman_owns: true,
            target_writable: false,
            ..base.clone()
        };
        assert_eq!(
            classify_install_target(&arch),
            InstallTarget {
                method: InstallMethod::ArchPacman,
                can_install: true,
            }
        );
        assert_eq!(
            InstallMethod::ArchPacman.asset_name("0.8.2").as_deref(),
            Some("youtfast-0.8.2-1-x86_64.pkg.tar.zst")
        );
        assert!(!InstallMethod::ArchPacman.supports_auto_update());

        let arch_headless = InstallProbe {
            pkexec_available: false,
            ..arch
        };
        assert_eq!(
            classify_install_target(&arch_headless),
            InstallTarget {
                method: InstallMethod::ArchPacman,
                can_install: false,
            }
        );

        let dev_build = InstallProbe {
            exe: PathBuf::from("/home/user/youtfast/target/debug/youtfast"),
            ..base.clone()
        };
        assert_eq!(
            classify_install_target(&dev_build),
            InstallTarget {
                method: InstallMethod::ManualOnly,
                can_install: false,
            }
        );

        let mbx_target_cache = InstallProbe {
            exe: PathBuf::from(
                "/home/user/.cache/mbx/targets/v1/8ea2f9b44086efee995ec10ec99dade9ecc4f55c79c7a30d29dd2ae1d404bd4c/debug/youtfast",
            ),
            ..base.clone()
        };
        assert_eq!(
            classify_install_target(&mbx_target_cache),
            InstallTarget {
                method: InstallMethod::ManualOnly,
                can_install: false,
            }
        );

        let win_setup = InstallProbe {
            os: "windows".into(),
            arch: "x86_64".into(),
            exe: PathBuf::from(r"C:\Users\u\AppData\Local\Programs\YouTFast\youtfast.exe"),
            windows_uninstaller_present: true,
            target_writable: true,
            ..base.clone()
        };
        assert_eq!(
            classify_install_target(&win_setup),
            InstallTarget {
                method: InstallMethod::WindowsSetup,
                can_install: true,
            }
        );
        assert_eq!(
            InstallMethod::WindowsSetup.asset_name("0.8.2").as_deref(),
            Some("youtfast-v0.8.2-x86_64-windows-setup.exe")
        );

        let win_portable = InstallProbe {
            windows_uninstaller_present: false,
            ..win_setup
        };
        assert_eq!(
            classify_install_target(&win_portable),
            InstallTarget {
                method: InstallMethod::WindowsPortable,
                can_install: true,
            }
        );

        let macos = InstallProbe {
            os: "macos".into(),
            arch: "aarch64".into(),
            exe: PathBuf::from("/Applications/YouTFast.app/Contents/MacOS/youtfast"),
            target_writable: true,
            ..base
        };
        assert_eq!(
            classify_install_target(&macos),
            InstallTarget {
                method: InstallMethod::MacosBundle,
                can_install: true,
            }
        );
    }

    #[test]
    fn clean_exe_path_strips_deleted_suffix() {
        assert_eq!(
            clean_exe_path(Path::new("/usr/local/bin/youtfast (deleted)")),
            PathBuf::from("/usr/local/bin/youtfast")
        );
    }
}
