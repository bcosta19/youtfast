//! Application version information, changelog extraction, GitHub release
//! checks, installation-method detection, and verified release updates.
//!
//! Minor (`0.x.0`) and patch (`0.0.x`) updates can be installed automatically
//! when the user opts in and the active installation method supports
//! unattended replacement; major (`x.0.0`) updates and elevation-based system
//! packages always require explicit user confirmation.

mod bundle;
mod changelog;
mod detect;
mod fetch;
mod install;

pub use bundle::{PendingMode, PendingUpdate};
pub use changelog::{bundled_changelog_for, clean_changelog, extract_changelog_section};
pub use detect::{InstallProbe, InstallTarget, classify_install_target, detect_install_target};
pub use fetch::{
    check_latest, check_latest_for, check_latest_from, parse_checksum_for, parse_release_json,
    parse_release_json_for, verify_sha256,
};
pub use install::{
    cleanup_stale_staging, install_archive, install_release, install_release_to,
    spawn_relaunch_after_exit,
};

use std::process::Command;

use serde::{Deserialize, Serialize};
use youtfast_i18n::{Locale, format as trf, gettext as tr};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RELEASES_URL: &str = "https://github.com/bcosta19/youtfast/releases";

pub(super) const ERROR_CHECK: &str = "Could not check for updates. Retry later.";
pub(super) const ERROR_DOWNLOAD: &str = "Could not download the update. Retry later.";
pub(super) const ERROR_INTEGRITY: &str = "The downloaded update failed integrity verification.";
pub(super) const ERROR_INSTALL: &str = "Could not install the update on this system.";
pub(super) const ERROR_UNSUPPORTED: &str =
    "Automatic installation is not supported for this package format.";
pub(super) const ERROR_AUTH: &str = "Update authentication was cancelled or unavailable.";
pub(super) const ERROR_INCOMPATIBLE: &str =
    "The downloaded update is incompatible with this system.";
pub(super) const ERROR_NOT_WRITABLE: &str = "The installation directory is not writable.";

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().strip_prefix('v').unwrap_or(raw.trim());
        // Pre-release tags (e.g. "0.3.0-rc.1") are not stable releases.
        if trimmed.contains('-') {
            return None;
        }
        let core = trimmed.split('+').next()?;
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }

    pub fn classify_update(current: Self, candidate: Self) -> Option<UpdateKind> {
        if candidate <= current {
            return None;
        }
        if candidate.major > current.major {
            Some(UpdateKind::Major)
        } else if candidate.minor > current.minor {
            Some(UpdateKind::Minor)
        } else {
            Some(UpdateKind::Patch)
        }
    }
}

/// How a newer release relates to the installed version under Semantic Versioning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateKind {
    Patch,
    Minor,
    Major,
}

impl UpdateKind {
    pub fn is_major(self) -> bool {
        matches!(self, Self::Major)
    }

    pub fn is_minor_or_patch(self) -> bool {
        matches!(self, Self::Patch | Self::Minor)
    }
}

/// How the running YouTFast binary was installed, so updates use the matching
/// release asset and replacement strategy without leaving conflicting files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallMethod {
    Flatpak,
    ArchPacman,
    LinuxNative,
    WindowsSetup,
    WindowsPortable,
    MacosBundle,
    ManualOnly,
}

impl InstallMethod {
    pub fn asset_name(self, version: &str) -> Option<String> {
        match self {
            Self::Flatpak => Some(format!("youtfast-v{version}-x86_64.flatpak")),
            Self::ArchPacman => Some(format!("youtfast-{version}-1-x86_64.pkg.tar.zst")),
            Self::LinuxNative => Some(format!(
                "youtfast-v{version}-x86_64-unknown-linux-gnu.tar.gz"
            )),
            Self::WindowsSetup => Some(format!("youtfast-v{version}-x86_64-windows-setup.exe")),
            Self::WindowsPortable => Some(format!("youtfast-v{version}-x86_64-pc-windows-gnu.zip")),
            Self::MacosBundle => Some(format!("youtfast-v{version}-aarch64-apple-darwin.zip")),
            Self::ManualOnly => None,
        }
    }

    pub fn supports_in_app_install(self) -> bool {
        matches!(
            self,
            Self::ArchPacman
                | Self::LinuxNative
                | Self::WindowsSetup
                | Self::WindowsPortable
                | Self::MacosBundle
        )
    }

    /// Background auto-update only runs unattended when no privilege prompt is
    /// required; Arch `pacman -U` waits for an explicit user click.
    pub fn supports_auto_update(self) -> bool {
        matches!(
            self,
            Self::LinuxNative | Self::WindowsSetup | Self::WindowsPortable | Self::MacosBundle
        )
    }

    pub fn manual_update_command(self) -> Option<&'static str> {
        match self {
            Self::Flatpak => {
                Some("curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method flatpak")
            }
            Self::ArchPacman => {
                Some("curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method arch")
            }
            Self::LinuxNative => {
                Some("curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method native")
            }
            Self::WindowsSetup => Some("irm https://youtfast.pages.dev/install.ps1 | iex"),
            Self::WindowsPortable | Self::MacosBundle | Self::ManualOnly => None,
        }
    }
}

/// Returns whether an update of `kind` should be installed automatically
/// without waiting for manual confirmation. Major versions never auto-install.
pub fn should_auto_update(auto_update_minor_patch: bool, kind: UpdateKind) -> bool {
    auto_update_minor_patch && kind.is_minor_or_patch()
}

pub fn should_auto_install(auto_update_minor_patch: bool, release: &ReleaseInfo) -> bool {
    should_auto_update(auto_update_minor_patch, release.kind) && release.can_auto_update()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub version: String,
    pub kind: UpdateKind,
    pub method: InstallMethod,
    pub install_supported: bool,
    pub changelog: String,
    pub html_url: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub checksums_url: Option<String>,
}

impl ReleaseInfo {
    /// Whether this release supports verified in-app installation for the
    /// active package format and carries its matching asset and checksums.
    pub fn can_self_update(&self) -> bool {
        self.install_supported
            && self.method.supports_in_app_install()
            && self.asset_name.is_some()
            && self.asset_url.is_some()
            && self.checksums_url.is_some()
    }

    pub fn can_auto_update(&self) -> bool {
        self.can_self_update() && self.method.supports_auto_update()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallOutcome {
    Applied,
    StagedForExit(PendingUpdate),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseInfo),
    Installing(ReleaseInfo),
    Installed {
        version: String,
        kind: UpdateKind,
        changelog: String,
        auto: bool,
        pending_apply_on_exit: bool,
    },
    Failed(String),
}

pub fn platform_asset_name(version: &str, os: &str, arch: &str) -> Option<String> {
    match (os, arch) {
        ("linux", "x86_64") => InstallMethod::LinuxNative.asset_name(version),
        ("macos", "aarch64") => InstallMethod::MacosBundle.asset_name(version),
        ("windows", "x86_64") => InstallMethod::WindowsPortable.asset_name(version),
        _ => None,
    }
}

/// Summary and body for the desktop notification when an update is found or
/// automatically installed.
pub fn notify_update_text(
    version: &str,
    kind: UpdateKind,
    auto_installed: bool,
    language: Locale,
) -> (String, String) {
    if auto_installed {
        (
            trf(
                language,
                "YouTFast updated to v{version}",
                &[("version", version.to_owned())],
            ),
            tr(
                language,
                "A minor or patch update was installed automatically. See what changed in Settings → About.",
            )
            .into_owned(),
        )
    } else if kind.is_major() {
        (
            trf(
                language,
                "New major version: YouTFast v{version}",
                &[("version", version.to_owned())],
            ),
            tr(
                language,
                "Major updates require manual confirmation. Open Settings → About to review the changelog and install.",
            )
            .into_owned(),
        )
    } else {
        (
            trf(
                language,
                "YouTFast v{version} is available",
                &[("version", version.to_owned())],
            ),
            tr(
                language,
                "A new version is ready to install. Open Settings → About to see what changed and update.",
            )
            .into_owned(),
        )
    }
}

pub(super) fn hidden_command(program: &str) -> Command {
    let cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = cmd;
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
    #[cfg(not(windows))]
    cmd
}

#[cfg(test)]
pub(super) fn test_root(name: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .join("target")
        .join(format!(
            "updater-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions_and_classifies_patch_minor_and_major_updates() {
        let base = Version::parse("v0.2.0").unwrap();
        assert_eq!(
            base,
            Version {
                major: 0,
                minor: 2,
                patch: 0
            }
        );
        assert_eq!(
            Version::classify_update(base, Version::parse("0.1.9").unwrap()),
            None
        );
        assert_eq!(
            Version::classify_update(base, Version::parse("0.2.0").unwrap()),
            None
        );
        assert_eq!(
            Version::classify_update(base, Version::parse("v0.2.1").unwrap()),
            Some(UpdateKind::Patch)
        );
        assert_eq!(
            Version::classify_update(base, Version::parse("v0.3.0").unwrap()),
            Some(UpdateKind::Minor)
        );
        assert_eq!(
            Version::classify_update(base, Version::parse("v1.0.0").unwrap()),
            Some(UpdateKind::Major)
        );
    }

    #[test]
    fn auto_update_applies_only_to_minor_and_patch_never_major() {
        assert!(!should_auto_update(false, UpdateKind::Patch));
        assert!(!should_auto_update(false, UpdateKind::Minor));
        assert!(!should_auto_update(false, UpdateKind::Major));
        assert!(should_auto_update(true, UpdateKind::Patch));
        assert!(should_auto_update(true, UpdateKind::Minor));
        assert!(!should_auto_update(true, UpdateKind::Major));
    }

    #[test]
    fn controlled_update_errors_survive_privacy_filter_and_translate() {
        for msg in [
            ERROR_CHECK,
            ERROR_DOWNLOAD,
            ERROR_INTEGRITY,
            ERROR_INSTALL,
            ERROR_UNSUPPORTED,
            ERROR_AUTH,
            ERROR_INCOMPATIBLE,
            ERROR_NOT_WRITABLE,
        ] {
            assert_eq!(youtfast_core::privacy::safe_error(msg), msg);
            for locale in Locale::ALL {
                if *locale != Locale::English {
                    assert_ne!(
                        youtfast_i18n::message(*locale, msg),
                        msg,
                        "missing translation for {msg} in {locale:?}"
                    );
                }
            }
        }
    }
}
