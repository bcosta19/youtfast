//! Application version information, changelog extraction, GitHub release
//! checks, installation-method detection, and verified release updates.
//!
//! Minor (`0.x.0`) and patch (`0.0.x`) updates can be installed automatically
//! when the user opts in and the active installation method supports
//! unattended replacement; major (`x.0.0`) updates and elevation-based system
//! packages always require explicit user confirmation.

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use youtfast_i18n::{Locale, format as trf, gettext as tr};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RELEASES_URL: &str = "https://github.com/bcosta19/youtfast/releases";
const LATEST_RELEASE_API: &str = "https://api.github.com/repos/bcosta19/youtfast/releases/latest";
const BUNDLED_CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

const MAX_API_BYTES: usize = 256 * 1024;
const MAX_CHECKSUMS_BYTES: usize = 64 * 1024;
const MAX_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
const MAX_HTTP_ATTEMPTS: usize = 3;
const STALE_PENDING_CLEANUP_SECS: u64 = 15 * 60;

const ERROR_CHECK: &str = "Could not check for updates. Retry later.";
const ERROR_DOWNLOAD: &str = "Could not download the update. Retry later.";
const ERROR_INTEGRITY: &str = "The downloaded update failed integrity verification.";
const ERROR_INSTALL: &str = "Could not install the update on this system.";
const ERROR_UNSUPPORTED: &str = "Automatic installation is not supported for this package format.";
const ERROR_AUTH: &str = "Update authentication was cancelled or unavailable.";
const ERROR_INCOMPATIBLE: &str = "The downloaded update is incompatible with this system.";
const ERROR_NOT_WRITABLE: &str = "The installation directory is not writable.";

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
const WINDOWS_APPLY_SCRIPT: &str = r#"param(
    [Parameter(Mandatory=$true)][int]$ParentPid,
    [Parameter(Mandatory=$true)][ValidateSet('setup','portable')][string]$Mode,
    [Parameter(Mandatory=$true)][string]$SourcePath,
    [Parameter(Mandatory=$true)][string]$TargetDir,
    [Parameter(Mandatory=$true)][string]$TargetExe,
    [Parameter(Mandatory=$true)][string]$StagingDir,
    [Parameter(Mandatory=$true)][string]$Relaunch
)

$ErrorActionPreference = 'Stop'

for ($i = 0; $i -lt 300; $i++) {
    $proc = Get-Process -Id $ParentPid -ErrorAction SilentlyContinue
    if (-not $proc) { break }
    Start-Sleep -Milliseconds 200
}
if (Get-Process -Id $ParentPid -ErrorAction SilentlyContinue) {
    exit 1
}

$applied = $false
if ($Mode -eq 'setup') {
    $proc = Start-Process -FilePath $SourcePath -ArgumentList ('/S /D=' + $TargetDir) -Wait -PassThru
    if ($proc.ExitCode -eq 0) {
        $applied = $true
    }
} elseif ($Mode -eq 'portable') {
    $backup = Join-Path $TargetDir ('.youtfast-update-backup-' + $ParentPid)
    $moved = New-Object System.Collections.Generic.List[string]
    $installed = New-Object System.Collections.Generic.List[string]
    try {
        New-Item -ItemType Directory -Path $backup -Force | Out-Null
        $oldLibraries = @()
        $oldManifestPath = Join-Path $TargetDir 'native-dependencies.json'
        if (Test-Path -LiteralPath $oldManifestPath -PathType Leaf) {
            try {
                $oldManifest = Get-Content -LiteralPath $oldManifestPath -Raw | ConvertFrom-Json
                $oldLibraries = @($oldManifest.libraries | Where-Object {
                    $_ -is [string] -and [IO.Path]::GetFileName($_) -ceq $_ -and
                    [IO.Path]::GetExtension($_) -ieq '.dll'
                })
            } catch {
                # An unreadable or legacy inventory cannot establish DLL ownership.
            }
        }
        $newItems = @(Get-ChildItem -LiteralPath $SourcePath -Force)
        $effectiveExeName = Split-Path -Leaf $TargetExe
        $newNames = @($newItems | ForEach-Object {
            if ($_.Name -eq 'youtfast.exe') { $effectiveExeName } else { $_.Name }
        })
        $replaceNames = @($oldLibraries) + @($newNames) + @(
            'native-dependencies.json', 'native-dependencies.txt', 'native-licenses'
        ) | Select-Object -Unique
        foreach ($name in $replaceNames) {
            $existing = Join-Path $TargetDir $name
            if (Test-Path -LiteralPath $existing) {
                Move-Item -LiteralPath $existing -Destination (Join-Path $backup $name) -Force
                [void]$moved.Add($name)
            }
        }
        foreach ($item in $newItems) {
            if ($item.Name -eq 'youtfast.exe') {
                $dest = Join-Path $TargetDir $effectiveExeName
            } else {
                $dest = Join-Path $TargetDir $item.Name
            }
            [void]$installed.Add((Split-Path -Leaf $dest))
            Copy-Item -LiteralPath $item.FullName -Destination $dest -Recurse -Force
        }
        $applied = $true
        Remove-Item -LiteralPath $backup -Recurse -Force -ErrorAction SilentlyContinue
    } catch {
        foreach ($name in $installed) {
            $cur = Join-Path $TargetDir $name
            if (Test-Path -LiteralPath $cur) {
                Remove-Item -LiteralPath $cur -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
        foreach ($name in $moved) {
            $cur = Join-Path $TargetDir $name
            $bak = Join-Path $backup $name
            if (Test-Path -LiteralPath $bak) {
                Move-Item -LiteralPath $bak -Destination $cur -Force -ErrorAction SilentlyContinue
            }
        }
        Remove-Item -LiteralPath $backup -Recurse -Force -ErrorAction SilentlyContinue
        exit 1
    }
}

if ($applied) {
    Get-ChildItem -LiteralPath $TargetDir -Filter '*.exe.old-*' -File -ErrorAction SilentlyContinue |
        Remove-Item -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $StagingDir -Recurse -Force -ErrorAction SilentlyContinue
}

if ($Relaunch -eq '1' -and (Test-Path -LiteralPath $TargetExe)) {
    Start-Process -FilePath $TargetExe
}
"#;

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

fn find_app_bundle(exe: &Path) -> Option<PathBuf> {
    let contents = exe.parent()?.parent()?;
    let app = contents.parent()?;
    (app.extension().and_then(|ext| ext.to_str()) == Some("app")).then(|| app.to_path_buf())
}

fn dir_is_writable(dir: &Path) -> bool {
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

fn is_effective_root() -> bool {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingMode {
    WindowsSetup,
    WindowsPortable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingUpdate {
    pub version: String,
    pub staging_dir: PathBuf,
    pub mode: PendingMode,
    pub target_dir: PathBuf,
    pub target_exe: PathBuf,
    pub source_path: PathBuf,
    pub script_path: PathBuf,
}

impl PendingUpdate {
    /// Launches the detached updater helper so Windows replaces locked binaries
    /// and DLLs cleanly once the current process has exited.
    pub fn spawn_on_exit(self, relaunch: bool) {
        #[cfg(windows)]
        {
            let mode = match self.mode {
                PendingMode::WindowsSetup => "setup",
                PendingMode::WindowsPortable => "portable",
            };
            let relaunch_arg = if relaunch { "1" } else { "0" };
            let pid = std::process::id().to_string();
            let mut cmd = hidden_command("powershell.exe");
            cmd.args([
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                &self.script_path.to_string_lossy(),
                "-ParentPid",
                &pid,
                "-Mode",
                mode,
                "-SourcePath",
                &self.source_path.to_string_lossy(),
                "-TargetDir",
                &self.target_dir.to_string_lossy(),
                "-TargetExe",
                &self.target_exe.to_string_lossy(),
                "-StagingDir",
                &self.staging_dir.to_string_lossy(),
                "-Relaunch",
                relaunch_arg,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
            if let Err(error) = cmd.spawn() {
                log::warn!("pending update helper failed: {error}");
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (self, relaunch);
        }
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

/// Extracts and cleans the changelog section for `version` from the bundled
/// `CHANGELOG.md`, falling back to `Unreleased` highlights if needed.
pub fn bundled_changelog_for(version: &str) -> String {
    extract_changelog_section(BUNDLED_CHANGELOG, version)
        .or_else(|| extract_changelog_section(BUNDLED_CHANGELOG, "Unreleased"))
        .unwrap_or_default()
}

pub fn extract_changelog_section(markdown: &str, version: &str) -> Option<String> {
    let target = format!("## [{version}]");
    let mut inside = false;
    let mut lines = Vec::new();
    for line in markdown.lines() {
        if line.starts_with("## [") {
            if inside {
                break;
            }
            if line.starts_with(&target) {
                inside = true;
            }
            continue;
        }
        if inside {
            if line.starts_with('[') && line.contains("]: http") {
                continue;
            }
            lines.push(line);
        }
    }
    let cleaned = clean_changelog(&lines.join("\n"));
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Normalizes Markdown release notes into clean, readable lines for the UI.
pub fn clean_changelog(markdown: &str) -> String {
    let mut out = Vec::new();
    let mut current_bullet = String::new();
    let flush_bullet = |bullet: &mut String, out: &mut Vec<String>| {
        if !bullet.is_empty() {
            out.push(format!("• {}", strip_markdown_links(bullet.trim())));
            bullet.clear();
        }
    };
    for raw_line in markdown.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            flush_bullet(&mut current_bullet, &mut out);
            continue;
        }
        if line.starts_with('[') && line.contains("]: http") {
            continue;
        }
        if let Some(heading) = line
            .strip_prefix("### ")
            .or_else(|| line.strip_prefix("## "))
        {
            flush_bullet(&mut current_bullet, &mut out);
            if !out.is_empty() {
                out.push(String::new());
            }
            out.push(strip_markdown_links(heading.trim()));
        } else if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            flush_bullet(&mut current_bullet, &mut out);
            current_bullet.push_str(item.trim());
        } else if !current_bullet.is_empty() {
            current_bullet.push(' ');
            current_bullet.push_str(line);
        } else {
            out.push(strip_markdown_links(line));
        }
    }
    flush_bullet(&mut current_bullet, &mut out);
    out.join("\n").trim().to_owned()
}

fn strip_markdown_links(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        result.push_str(&rest[..open]);
        let after_open = &rest[open + 1..];
        if let Some(close) = after_open.find(']') {
            let label = &after_open[..close];
            let after_close = &after_open[close + 1..];
            if !label.contains('[')
                && let Some(inside_paren) = after_close.strip_prefix('(')
                && let Some(end_paren) = inside_paren.find(')')
            {
                result.push_str(label);
                rest = &inside_paren[end_paren + 1..];
                continue;
            }
        }
        result.push('[');
        rest = after_open;
    }
    result.push_str(rest);
    result.replace('`', "")
}

fn allowed_release_url(raw: &str) -> bool {
    let Ok(parsed) = url::Url::parse(raw) else {
        return false;
    };
    #[cfg(test)]
    if parsed.scheme() == "http" && matches!(parsed.host_str(), Some("127.0.0.1" | "localhost")) {
        return true;
    }
    parsed.scheme() == "https"
        && matches!(
            parsed.host_str(),
            Some(
                "github.com"
                    | "api.github.com"
                    | "objects.githubusercontent.com"
                    | "release-assets.githubusercontent.com"
            )
        )
}

fn metadata_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(20)))
        .timeout_per_read(Some(Duration::from_secs(20)))
        .timeout_global(Some(Duration::from_secs(45)))
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent(format!("youtfast/{CURRENT_VERSION}"))
        .build()
        .new_agent()
}

fn download_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_per_read(Some(Duration::from_secs(30)))
        .timeout_global(Some(Duration::from_secs(900)))
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent(format!("youtfast/{CURRENT_VERSION}"))
        .build()
        .new_agent()
}

#[derive(Debug)]
enum FetchError {
    Retryable(Option<Duration>),
    Fatal,
    TooLarge,
}

fn retry_after_delay(response: &ureq::http::Response<ureq::Body>) -> Option<Duration> {
    let secs = response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())?;
    Some(Duration::from_secs(secs.clamp(1, 15)))
}

fn open_validated_response(
    agent: &ureq::Agent,
    initial_url: &str,
) -> Result<ureq::http::Response<ureq::Body>, FetchError> {
    let mut current_url = initial_url.to_owned();
    for _ in 0..=MAX_REDIRECTS {
        if !allowed_release_url(&current_url) {
            return Err(FetchError::Fatal);
        }
        let parsed = url::Url::parse(&current_url).map_err(|_| FetchError::Fatal)?;
        let mut request = agent.get(&current_url);
        if parsed.host_str() == Some("api.github.com") {
            request = request.header("Accept", "application/vnd.github+json");
        }
        let response = match request.call() {
            Ok(response) => response,
            Err(error) => match error {
                ureq::Error::Io(_)
                | ureq::Error::Timeout(_)
                | ureq::Error::HostNotFound
                | ureq::Error::ConnectionFailed => return Err(FetchError::Retryable(None)),
                _ => return Err(FetchError::Fatal),
            },
        };
        let status = response.status().as_u16();
        if (301..=308).contains(&status) && status != 304 {
            let Some(location) = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
            else {
                return Err(FetchError::Fatal);
            };
            let next = parsed.join(location).map_err(|_| FetchError::Fatal)?;
            if !allowed_release_url(next.as_str()) {
                return Err(FetchError::Fatal);
            }
            current_url = next.to_string();
            continue;
        }
        if (200..300).contains(&status) {
            return Ok(response);
        }
        if status == 429 || (500..=599).contains(&status) {
            return Err(FetchError::Retryable(retry_after_delay(&response)));
        }
        return Err(FetchError::Fatal);
    }
    Err(FetchError::Fatal)
}

fn retry_fetch<T>(
    attempts: usize,
    mut sleep: impl FnMut(Duration),
    mut op: impl FnMut() -> Result<T, FetchError>,
) -> Result<T, FetchError> {
    for attempt in 0..attempts {
        match op() {
            Ok(value) => return Ok(value),
            Err(FetchError::Retryable(delay)) if attempt + 1 < attempts => {
                let backoff = delay.unwrap_or_else(|| Duration::from_secs(1 << attempt));
                sleep(backoff);
            }
            Err(error) => return Err(error),
        }
    }
    Err(FetchError::Fatal)
}

fn fetch_bytes_with<S: FnMut(Duration)>(
    agent: &ureq::Agent,
    url: &str,
    max_bytes: usize,
    sleep: S,
) -> Result<Vec<u8>, FetchError> {
    retry_fetch(MAX_HTTP_ATTEMPTS, sleep, || {
        let response = open_validated_response(agent, url)?;
        if let Some(len) = content_length(&response)
            && len > max_bytes as u64
        {
            return Err(FetchError::TooLarge);
        }
        let mut reader = response
            .into_body()
            .into_reader()
            .take((max_bytes + 1) as u64);
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(|_| FetchError::Retryable(None))?;
        if bytes.len() > max_bytes {
            return Err(FetchError::TooLarge);
        }
        Ok(bytes)
    })
}

fn content_length(response: &ureq::http::Response<ureq::Body>) -> Option<u64> {
    response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
}

fn fetch_bytes(url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    fetch_bytes_with(&metadata_agent(), url, max_bytes, std::thread::sleep)
        .map_err(|_| ERROR_DOWNLOAD.to_owned())
}

fn download_verified_file_with<S: FnMut(Duration)>(
    agent: &ureq::Agent,
    url: &str,
    dest: &Path,
    expected_sha256: &str,
    max_bytes: usize,
    sleep: S,
) -> Result<(), String> {
    let outcome = retry_fetch(MAX_HTTP_ATTEMPTS, sleep, || {
        let response = open_validated_response(agent, url)?;
        let expected_len = content_length(&response);
        if expected_len.is_some_and(|len| len > max_bytes as u64) {
            return Err(FetchError::TooLarge);
        }
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(dest)
            .map_err(|_| FetchError::Fatal)?;
        let mut writer = BufWriter::new(file);
        let mut hasher = Sha256::new();
        let mut reader = response.into_body().into_reader();
        let mut buf = [0u8; 64 * 1024];
        let mut total = 0usize;
        loop {
            let read = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => {
                    let _ = std::fs::remove_file(dest);
                    return Err(FetchError::Retryable(None));
                }
            };
            total = total.saturating_add(read);
            if total > max_bytes {
                drop(writer);
                let _ = std::fs::remove_file(dest);
                return Err(FetchError::TooLarge);
            }
            hasher.update(&buf[..read]);
            if writer.write_all(&buf[..read]).is_err() {
                drop(writer);
                let _ = std::fs::remove_file(dest);
                return Err(FetchError::Fatal);
            }
        }
        if writer.flush().is_err() {
            let _ = std::fs::remove_file(dest);
            return Err(FetchError::Fatal);
        }
        drop(writer);
        if expected_len.is_some_and(|len| total as u64 != len) {
            let _ = std::fs::remove_file(dest);
            return Err(FetchError::Retryable(None));
        }
        let actual: String = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if !actual.eq_ignore_ascii_case(expected_sha256.trim()) {
            let _ = std::fs::remove_file(dest);
            return Err(FetchError::TooLarge);
        }
        Ok(())
    });
    match outcome {
        Ok(()) => Ok(()),
        Err(FetchError::TooLarge) => Err(ERROR_INTEGRITY.to_owned()),
        Err(FetchError::Retryable(_) | FetchError::Fatal) => Err(ERROR_DOWNLOAD.to_owned()),
    }
}

/// Checks GitHub Releases for a newer version than `CURRENT_VERSION`.
pub fn check_latest() -> Result<Option<ReleaseInfo>, String> {
    let target = detect_install_target();
    check_latest_for(LATEST_RELEASE_API, CURRENT_VERSION, target)
}

fn default_target_for_platform(os: &str, arch: &str) -> InstallTarget {
    let method = match (os, arch) {
        ("linux", "x86_64") => InstallMethod::LinuxNative,
        ("macos", "aarch64") => InstallMethod::MacosBundle,
        ("windows", "x86_64") => InstallMethod::WindowsPortable,
        _ => InstallMethod::ManualOnly,
    };
    InstallTarget {
        method,
        can_install: method.supports_in_app_install(),
    }
}

pub fn check_latest_from(
    api_url: &str,
    current_version: &str,
    os: &str,
    arch: &str,
) -> Result<Option<ReleaseInfo>, String> {
    check_latest_for(
        api_url,
        current_version,
        default_target_for_platform(os, arch),
    )
}

pub fn check_latest_for(
    api_url: &str,
    current_version: &str,
    target: InstallTarget,
) -> Result<Option<ReleaseInfo>, String> {
    let bytes = fetch_bytes(api_url, MAX_API_BYTES).map_err(|_| ERROR_CHECK.to_owned())?;
    let text = String::from_utf8(bytes).map_err(|_| ERROR_CHECK.to_owned())?;
    parse_release_json_for(&text, current_version, target)
}

pub fn parse_release_json(
    json: &str,
    current_version: &str,
    os: &str,
    arch: &str,
) -> Result<Option<ReleaseInfo>, String> {
    parse_release_json_for(json, current_version, default_target_for_platform(os, arch))
}

pub fn parse_release_json_for(
    json: &str,
    current_version: &str,
    target: InstallTarget,
) -> Result<Option<ReleaseInfo>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| ERROR_CHECK.to_owned())?;
    if value["draft"].as_bool() == Some(true) || value["prerelease"].as_bool() == Some(true) {
        return Ok(None);
    }
    let tag = value["tag_name"]
        .as_str()
        .ok_or_else(|| ERROR_CHECK.to_owned())?;
    let current = Version::parse(current_version).ok_or_else(|| ERROR_CHECK.to_owned())?;
    let Some(candidate) = Version::parse(tag) else {
        return Ok(None);
    };
    let Some(kind) = Version::classify_update(current, candidate) else {
        return Ok(None);
    };
    let version = format!(
        "{}.{}.{}",
        candidate.major, candidate.minor, candidate.patch
    );
    let body = value["body"].as_str().unwrap_or_default();
    let mut changelog = clean_changelog(body);
    if changelog.is_empty() {
        changelog = bundled_changelog_for(&version);
    }
    let html_url = value["html_url"]
        .as_str()
        .filter(|url| allowed_release_url(url))
        .unwrap_or(RELEASES_URL)
        .to_owned();
    let expected_asset = target.method.asset_name(&version);
    let mut asset_url = None;
    let mut checksums_url = None;
    if let Some(assets) = value["assets"].as_array() {
        for asset in assets {
            let Some(name) = asset["name"].as_str() else {
                continue;
            };
            let Some(download_url) = asset["browser_download_url"]
                .as_str()
                .filter(|url| allowed_release_url(url))
            else {
                continue;
            };
            if expected_asset.as_deref() == Some(name) {
                asset_url = Some(download_url.to_owned());
            } else if name == "checksums.txt" {
                checksums_url = Some(download_url.to_owned());
            }
        }
    }
    Ok(Some(ReleaseInfo {
        version,
        kind,
        method: target.method,
        install_supported: target.can_install,
        changelog,
        html_url,
        asset_name: expected_asset,
        asset_url,
        checksums_url,
    }))
}

pub fn parse_checksum_for(checksums_text: &str, asset_name: &str) -> Option<String> {
    for line in checksums_text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(digest), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        let clean_name = name.strip_prefix('*').unwrap_or(name);
        if clean_name == asset_name
            && digest.len() == 64
            && digest.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Some(digest.to_ascii_lowercase());
        }
    }
    None
}

pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> bool {
    let actual: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    actual.eq_ignore_ascii_case(expected_hex.trim())
}

/// Removes leftover temporary update folders and any staged `pending-update`
/// folders for versions already reached or surpassed.
pub fn cleanup_stale_staging(cache_dir: &Path, current_version: &str) {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return;
    };
    let current = Version::parse(current_version);
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let path = entry.path();
        if name.starts_with("update-") && path.is_dir() {
            let stale = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|modified| modified.elapsed().ok())
                .is_none_or(|age| age.as_secs() >= STALE_PENDING_CLEANUP_SECS);
            if stale {
                let _ = std::fs::remove_dir_all(&path);
            }
        }
    }
    let pending_root = cache_dir.join("pending-update");
    if let Ok(pending_entries) = std::fs::read_dir(&pending_root) {
        for entry in pending_entries.filter_map(Result::ok) {
            let name = entry.file_name();
            let dir_version = Version::parse(&name.to_string_lossy());
            let already_applied = match (current, dir_version) {
                (Some(cur), Some(staged)) => staged <= cur,
                _ => false,
            };
            if already_applied {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

/// Downloads, verifies SHA-256 against `checksums.txt`, and installs or stages
/// `release` according to the active installation method.
pub fn install_release(release: &ReleaseInfo, cache_dir: &Path) -> Result<InstallOutcome, String> {
    let target_exe = std::env::current_exe().map_err(|_| ERROR_INSTALL.to_owned())?;
    install_release_to(release, &target_exe, cache_dir)
}

pub fn install_release_to(
    release: &ReleaseInfo,
    target_exe: &Path,
    cache_dir: &Path,
) -> Result<InstallOutcome, String> {
    if !release.can_self_update() {
        return Err(ERROR_UNSUPPORTED.to_owned());
    }
    let (Some(asset_name), Some(asset_url), Some(checksums_url)) = (
        release.asset_name.as_deref(),
        release.asset_url.as_deref(),
        release.checksums_url.as_deref(),
    ) else {
        return Err(ERROR_UNSUPPORTED.to_owned());
    };
    let resolved_exe = clean_exe_path(
        &std::fs::canonicalize(target_exe).unwrap_or_else(|_| target_exe.to_path_buf()),
    );
    preflight_writable(release.method, &resolved_exe)?;

    let checksums_bytes = fetch_bytes(checksums_url, MAX_CHECKSUMS_BYTES)?;
    let checksums_text =
        String::from_utf8(checksums_bytes).map_err(|_| ERROR_INTEGRITY.to_owned())?;
    let expected_sha = parse_checksum_for(&checksums_text, asset_name)
        .ok_or_else(|| ERROR_INTEGRITY.to_owned())?;

    let staging = cache_dir.join(format!(
        "update-{}-{}-{}",
        release.version,
        std::process::id(),
        youtfast_core::util::now()
    ));
    std::fs::create_dir_all(&staging).map_err(|_| ERROR_INSTALL.to_owned())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if release.method == InstallMethod::ArchPacman {
            0o755
        } else {
            0o700
        };
        let _ = std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(mode));
    }

    let archive_path = staging.join(asset_name);
    let download_result = download_verified_file_with(
        &download_agent(),
        asset_url,
        &archive_path,
        &expected_sha,
        MAX_ARCHIVE_BYTES,
        std::thread::sleep,
    );
    if let Err(error) = download_result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }

    let result = apply_verified_download(
        release.method,
        &archive_path,
        asset_name,
        &release.version,
        &resolved_exe,
        &staging,
        cache_dir,
    );
    let _ = std::fs::remove_dir_all(&staging);
    result
}

fn preflight_writable(method: InstallMethod, resolved_exe: &Path) -> Result<(), String> {
    let dir = match method {
        InstallMethod::ArchPacman => return Ok(()),
        InstallMethod::Flatpak | InstallMethod::ManualOnly => {
            return Err(ERROR_UNSUPPORTED.to_owned());
        }
        InstallMethod::MacosBundle => find_app_bundle(resolved_exe)
            .and_then(|app| app.parent().map(Path::to_path_buf))
            .ok_or_else(|| ERROR_UNSUPPORTED.to_owned())?,
        InstallMethod::LinuxNative
        | InstallMethod::WindowsSetup
        | InstallMethod::WindowsPortable => resolved_exe
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| ERROR_INSTALL.to_owned())?,
    };
    if !dir_is_writable(&dir) {
        return Err(ERROR_NOT_WRITABLE.to_owned());
    }
    Ok(())
}

pub fn install_archive(
    archive_bytes: &[u8],
    asset_name: &str,
    version: &str,
    target_exe: &Path,
    staging_root: &Path,
) -> Result<(), String> {
    let method = if asset_name.ends_with(".tar.gz") {
        InstallMethod::LinuxNative
    } else if asset_name.ends_with(".zip") && asset_name.contains("apple-darwin") {
        InstallMethod::MacosBundle
    } else if asset_name.ends_with(".zip") {
        InstallMethod::WindowsPortable
    } else {
        return Err(ERROR_UNSUPPORTED.to_owned());
    };
    let staging = staging_root.join(format!(
        "update-{version}-{}-{}",
        std::process::id(),
        youtfast_core::util::now()
    ));
    std::fs::create_dir_all(&staging).map_err(|_| ERROR_INSTALL.to_owned())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o700));
    }
    let archive_path = staging.join(asset_name);
    if std::fs::write(&archive_path, archive_bytes).is_err() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(ERROR_INSTALL.to_owned());
    }
    let resolved_exe = clean_exe_path(
        &std::fs::canonicalize(target_exe).unwrap_or_else(|_| target_exe.to_path_buf()),
    );
    let result = apply_verified_download(
        method,
        &archive_path,
        asset_name,
        version,
        &resolved_exe,
        &staging,
        staging_root,
    );
    let _ = std::fs::remove_dir_all(&staging);
    result.map(|_| ())
}

fn apply_verified_download(
    method: InstallMethod,
    archive_path: &Path,
    asset_name: &str,
    version: &str,
    resolved_exe: &Path,
    staging: &Path,
    cache_dir: &Path,
) -> Result<InstallOutcome, String> {
    match method {
        InstallMethod::LinuxNative => {
            extract_tar_gz(archive_path, staging)?;
            let stem = asset_name.trim_end_matches(".tar.gz");
            let extracted_bin = staging.join(stem).join("youtfast");
            if !extracted_bin.is_file() {
                return Err(ERROR_INSTALL.to_owned());
            }
            set_executable_mode(&extracted_bin)?;
            let prepared = prepare_verified_adjacent_file(&extracted_bin, resolved_exe, version)?;
            replace_prepared_file(&prepared, resolved_exe).map_err(|_| ERROR_INSTALL.to_owned())?;
            Ok(InstallOutcome::Applied)
        }
        InstallMethod::ArchPacman => {
            install_arch_package(archive_path, version)?;
            Ok(InstallOutcome::Applied)
        }
        InstallMethod::MacosBundle => {
            extract_zip(archive_path, staging, true)?;
            let stem = asset_name.trim_end_matches(".zip");
            let macos_app = staging.join(stem).join("YouTFast.app");
            let macos_bin = macos_app.join("Contents/MacOS/youtfast");
            if !macos_bin.is_file() {
                return Err(ERROR_INSTALL.to_owned());
            }
            prepare_and_verify_macos_app(&macos_app, &macos_bin, version)?;
            replace_macos_bundle(&macos_app, resolved_exe)?;
            Ok(InstallOutcome::Applied)
        }
        InstallMethod::WindowsSetup => {
            let pending =
                stage_windows_setup(archive_path, asset_name, version, resolved_exe, cache_dir)?;
            Ok(InstallOutcome::StagedForExit(pending))
        }
        InstallMethod::WindowsPortable => {
            extract_zip(archive_path, staging, false)?;
            let stem = asset_name.trim_end_matches(".zip");
            let bundle_root = staging.join(stem);
            let staged_exe = bundle_root.join("youtfast.exe");
            if !staged_exe.is_file() {
                return Err(ERROR_INSTALL.to_owned());
            }
            verify_extracted_binary(&staged_exe, version)?;
            let pending = stage_windows_portable(&bundle_root, version, resolved_exe, cache_dir)?;
            Ok(InstallOutcome::StagedForExit(pending))
        }
        InstallMethod::Flatpak | InstallMethod::ManualOnly => Err(ERROR_UNSUPPORTED.to_owned()),
    }
}

fn hidden_command(program: &str) -> Command {
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

fn set_executable_mode(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(|_| ERROR_INSTALL.to_owned())?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn verify_extracted_binary(bin: &Path, expected_version: &str) -> Result<(), String> {
    let output = hidden_command(&bin.to_string_lossy())
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .map_err(|_| ERROR_INCOMPATIBLE.to_owned())?;
    if !output.status.success() {
        return Err(ERROR_INCOMPATIBLE.to_owned());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed == format!("youtfast {expected_version}") {
        Ok(())
    } else {
        Err(ERROR_INCOMPATIBLE.to_owned())
    }
}

fn extract_tar_gz(archive_path: &Path, staging: &Path) -> Result<(), String> {
    let status = hidden_command("tar")
        .args([
            "-xzf",
            &archive_path.to_string_lossy(),
            "-C",
            &staging.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| ERROR_INSTALL.to_owned())?;
    if status.success() {
        Ok(())
    } else {
        Err(ERROR_INSTALL.to_owned())
    }
}

fn extract_zip(archive_path: &Path, staging: &Path, macos_bundle: bool) -> Result<(), String> {
    let status = if cfg!(target_os = "macos") && macos_bundle {
        hidden_command("/usr/bin/ditto")
            .args([
                "-x",
                "-k",
                &archive_path.to_string_lossy(),
                &staging.to_string_lossy(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    } else if cfg!(windows) {
        hidden_command("tar.exe")
            .args([
                "-xf",
                &archive_path.to_string_lossy(),
                "-C",
                &staging.to_string_lossy(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    } else {
        hidden_command("unzip")
            .args([
                "-q",
                &archive_path.to_string_lossy(),
                "-d",
                &staging.to_string_lossy(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    };
    match status {
        Ok(status) if status.success() => Ok(()),
        _ => Err(ERROR_INSTALL.to_owned()),
    }
}

fn install_arch_package(pkg_path: &Path, expected_version: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(pkg_path, std::fs::Permissions::from_mode(0o644));
    }
    let inspect = hidden_command("pacman")
        .args(["-Qp", &pkg_path.to_string_lossy()])
        .stdin(Stdio::null())
        .output()
        .map_err(|_| ERROR_INCOMPATIBLE.to_owned())?;
    if !inspect.status.success() {
        return Err(ERROR_INTEGRITY.to_owned());
    }
    let meta = String::from_utf8_lossy(&inspect.stdout);
    let expected_prefix = format!("youtfast {expected_version}-");
    if !meta.trim().starts_with(&expected_prefix) {
        return Err(ERROR_INTEGRITY.to_owned());
    }
    let status = if is_effective_root() {
        hidden_command("pacman")
            .args(["-U", "--noconfirm", &pkg_path.to_string_lossy()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| ERROR_INSTALL.to_owned())?
    } else {
        let status = hidden_command("pkexec")
            .args(["pacman", "-U", "--noconfirm", &pkg_path.to_string_lossy()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| ERROR_AUTH.to_owned())?;
        if matches!(status.code(), Some(126 | 127)) {
            return Err(ERROR_AUTH.to_owned());
        }
        status
    };
    if status.success() {
        Ok(())
    } else {
        Err(ERROR_INSTALL.to_owned())
    }
}

fn prepare_and_verify_macos_app(
    new_app: &Path,
    new_bin: &Path,
    expected_version: &str,
) -> Result<(), String> {
    set_executable_mode(new_bin)?;
    #[cfg(target_os = "macos")]
    {
        let _ = hidden_command("/usr/bin/xattr")
            .args(["-dr", "com.apple.quarantine", &new_app.to_string_lossy()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let verified = hidden_command("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict", &new_app.to_string_lossy()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !verified {
            return Err(ERROR_INTEGRITY.to_owned());
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = new_app;
    verify_extracted_binary(new_bin, expected_version)
}

fn replace_macos_bundle(new_app: &Path, resolved_exe: &Path) -> Result<(), String> {
    let existing_app = find_app_bundle(resolved_exe).ok_or_else(|| ERROR_UNSUPPORTED.to_owned())?;
    let apps_dir = existing_app
        .parent()
        .ok_or_else(|| ERROR_INSTALL.to_owned())?;
    static APP_SWAPS: AtomicU64 = AtomicU64::new(0);
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        APP_SWAPS.fetch_add(1, Ordering::Relaxed)
    );
    let staged_app = apps_dir.join(format!(".YouTFast.app.new-{suffix}"));
    let backup_app = apps_dir.join(format!(".YouTFast.app.old-{suffix}"));
    let _ = std::fs::remove_dir_all(&staged_app);
    let _ = std::fs::remove_dir_all(&backup_app);
    let copied = if cfg!(target_os = "macos") {
        hidden_command("/usr/bin/ditto")
            .args([new_app, &staged_app])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    } else {
        copy_dir_recursive(new_app, &staged_app).is_ok()
    };
    if !copied {
        let _ = std::fs::remove_dir_all(&staged_app);
        return Err(ERROR_INSTALL.to_owned());
    }
    if std::fs::rename(&existing_app, &backup_app).is_err() {
        let _ = std::fs::remove_dir_all(&staged_app);
        return Err(ERROR_INSTALL.to_owned());
    }
    if std::fs::rename(&staged_app, &existing_app).is_err() {
        let _ = std::fs::rename(&backup_app, &existing_app);
        let _ = std::fs::remove_dir_all(&staged_app);
        return Err(ERROR_INSTALL.to_owned());
    }
    let _ = std::fs::remove_dir_all(&backup_app);
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn stage_windows_setup(
    archive_path: &Path,
    asset_name: &str,
    version: &str,
    resolved_exe: &Path,
    cache_dir: &Path,
) -> Result<PendingUpdate, String> {
    let target_dir = resolved_exe
        .parent()
        .ok_or_else(|| ERROR_INSTALL.to_owned())?
        .to_path_buf();
    let staging_dir = prepare_pending_dir(cache_dir, version)?;
    let source_path = staging_dir.join(asset_name);
    std::fs::copy(archive_path, &source_path).map_err(|_| ERROR_INSTALL.to_owned())?;
    let script_path = write_windows_apply_script(&staging_dir)?;
    Ok(PendingUpdate {
        version: version.to_owned(),
        staging_dir,
        mode: PendingMode::WindowsSetup,
        target_dir,
        target_exe: resolved_exe.to_path_buf(),
        source_path,
        script_path,
    })
}

fn stage_windows_portable(
    bundle_root: &Path,
    version: &str,
    resolved_exe: &Path,
    cache_dir: &Path,
) -> Result<PendingUpdate, String> {
    let target_dir = resolved_exe
        .parent()
        .ok_or_else(|| ERROR_INSTALL.to_owned())?
        .to_path_buf();
    let staging_dir = prepare_pending_dir(cache_dir, version)?;
    let source_path = staging_dir.join("bundle");
    copy_dir_recursive(bundle_root, &source_path).map_err(|_| ERROR_INSTALL.to_owned())?;
    let script_path = write_windows_apply_script(&staging_dir)?;
    Ok(PendingUpdate {
        version: version.to_owned(),
        staging_dir,
        mode: PendingMode::WindowsPortable,
        target_dir,
        target_exe: resolved_exe.to_path_buf(),
        source_path,
        script_path,
    })
}

fn prepare_pending_dir(cache_dir: &Path, version: &str) -> Result<PathBuf, String> {
    let dir = cache_dir.join("pending-update").join(format!("v{version}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|_| ERROR_INSTALL.to_owned())?;
    Ok(dir)
}

fn write_windows_apply_script(staging_dir: &Path) -> Result<PathBuf, String> {
    let script = staging_dir.join("apply-update.ps1");
    #[cfg(windows)]
    let body = WINDOWS_APPLY_SCRIPT;
    #[cfg(not(windows))]
    let body = "# staged on non-windows test host\n";
    std::fs::write(&script, body).map_err(|_| ERROR_INSTALL.to_owned())?;
    Ok(script)
}

/// Relaunches YouTFast cleanly after the running instance finishes exiting and
/// releases the single-instance lock.
pub fn spawn_relaunch_after_exit() {
    let Ok(raw_exe) = std::env::current_exe() else {
        return;
    };
    let resolved = clean_exe_path(&std::fs::canonicalize(&raw_exe).unwrap_or(raw_exe));
    #[cfg(unix)]
    {
        let pid = std::process::id().to_string();
        let launch_target = if cfg!(target_os = "macos") {
            find_app_bundle(&resolved)
        } else {
            None
        };
        let mut cmd = Command::new("/bin/sh");
        if let Some(app_bundle) = launch_target {
            cmd.args([
                "-c",
                "pid=\"$1\"; app=\"$2\"; i=0; while kill -0 \"$pid\" 2>/dev/null && [ \"$i\" -lt 100 ]; do sleep 0.1; i=$((i+1)); done; exec /usr/bin/open -n -a \"$app\"",
                "_",
                &pid,
                &app_bundle.to_string_lossy(),
            ]);
        } else {
            cmd.args([
                "-c",
                "pid=\"$1\"; exe=\"$2\"; i=0; while kill -0 \"$pid\" 2>/dev/null && [ \"$i\" -lt 100 ]; do sleep 0.1; i=$((i+1)); done; exec \"$exe\"",
                "_",
                &pid,
                &resolved.to_string_lossy(),
            ]);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Err(error) = cmd.spawn() {
            log::warn!("relaunch helper failed: {error}");
        }
    }
    #[cfg(windows)]
    {
        let pid = std::process::id().to_string();
        let mut cmd = hidden_command("powershell.exe");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            "param([int]$p,[string]$e) for($i=0;$i -lt 100;$i++){if(-not(Get-Process -Id $p -ErrorAction SilentlyContinue)){break};Start-Sleep -Milliseconds 100}; if(Test-Path -LiteralPath $e){Start-Process -FilePath $e}",
            "-p",
            &pid,
            "-e",
            &resolved.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        if let Err(error) = cmd.spawn() {
            log::warn!("relaunch helper failed: {error}");
        }
    }
}

/// Strips the `" (deleted)"` suffix Linux's `/proc/self/exe` appends when the
/// running binary has already been replaced once during the current session.
fn clean_exe_path(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    match raw.strip_suffix(" (deleted)") {
        Some(clean) => PathBuf::from(clean),
        None => path.to_path_buf(),
    }
}

fn prepare_adjacent_file(source: &Path, destination: &Path) -> std::io::Result<PathBuf> {
    match std::fs::symlink_metadata(destination) {
        Ok(metadata) if !metadata.is_file() => {
            return Err(std::io::Error::other(
                "refusing to replace a non-regular file",
            ));
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    static UPDATES: AtomicU64 = AtomicU64::new(0);
    let parent = destination
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent directory"))?;
    let tmp = parent.join(format!(
        ".youtfast-update-{}-{}.tmp",
        std::process::id(),
        UPDATES.fetch_add(1, Ordering::Relaxed)
    ));
    let written: std::io::Result<()> = (|| {
        let mut reader = BufReader::new(std::fs::File::open(source)?);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o755);
        let mut writer = BufWriter::new(options.open(&tmp)?);
        std::io::copy(&mut reader, &mut writer)?;
        writer.flush()?;
        let file = writer
            .into_inner()
            .map_err(std::io::IntoInnerError::into_error)?;
        file.sync_all()?;
        drop(file);
        Ok(())
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written.map(|()| tmp)
}

fn prepare_verified_adjacent_file(
    source: &Path,
    destination: &Path,
    expected_version: &str,
) -> Result<PathBuf, String> {
    let prepared =
        prepare_adjacent_file(source, destination).map_err(|_| ERROR_INSTALL.to_owned())?;
    if let Err(error) = verify_extracted_binary(&prepared, expected_version) {
        let _ = std::fs::remove_file(&prepared);
        return Err(error);
    }
    Ok(prepared)
}

fn replace_prepared_file(prepared: &Path, destination: &Path) -> std::io::Result<()> {
    let replaced = (|| {
        match std::fs::symlink_metadata(destination) {
            Ok(metadata) if !metadata.is_file() => {
                return Err(std::io::Error::other(
                    "refusing to replace a non-regular file",
                ));
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
        if prepared.parent() != destination.parent() {
            return Err(std::io::Error::other(
                "replacement file must be adjacent to destination",
            ));
        }
        std::fs::rename(prepared, destination)?;
        sync_parent(destination)
    })();
    if replaced.is_err() {
        let _ = std::fs::remove_file(prepared);
    }
    replaced
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;

    fn test_root(name: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join("target")
            .join(format!(
                "updater-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ))
    }

    #[cfg(unix)]
    #[test]
    fn linux_update_verifies_adjacent_candidate_before_replacing_target() {
        let root = test_root("adjacent-verification");
        let _ = std::fs::remove_dir_all(&root);
        let cache = root.join("cache");
        let install = root.join("install");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::create_dir_all(&install).unwrap();
        let source = cache.join("candidate");
        let target = install.join("youtfast");
        std::fs::write(&source, b"#!/bin/sh\nprintf 'youtfast 1.2.3\\n'\n").unwrap();
        std::fs::write(&target, b"old install").unwrap();

        let prepared = prepare_verified_adjacent_file(&source, &target, "1.2.3").unwrap();
        assert_eq!(prepared.parent(), target.parent());
        replace_prepared_file(&prepared, &target).unwrap();
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"#!/bin/sh\nprintf 'youtfast 1.2.3\\n'\n"
        );

        std::fs::write(&source, b"#!/bin/sh\nexit 1\n").unwrap();
        assert!(prepare_verified_adjacent_file(&source, &target, "1.2.3").is_err());
        assert_eq!(
            std::fs::read_dir(&install).unwrap().count(),
            1,
            "failed validation should remove the adjacent temporary candidate"
        );
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"#!/bin/sh\nprintf 'youtfast 1.2.3\\n'\n"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_portable_script_preserves_unowned_dlls_and_renamed_exe() {
        let root = test_root("windows-portable-apply");
        let _ = std::fs::remove_dir_all(&root);
        let target = root.join("install");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("renamed-youtfast.exe"), b"old exe").unwrap();
        std::fs::write(target.join("owned-old.dll"), b"old package dll").unwrap();
        std::fs::write(target.join("user-tool.dll"), b"user dll").unwrap();
        std::fs::write(
            target.join("native-dependencies.json"),
            r#"{"libraries":["owned-old.dll"]}"#,
        )
        .unwrap();

        let apply_bundle = |name: &str, include_old_manifest: bool| {
            let staging = root.join(name);
            let source = staging.join("bundle");
            std::fs::create_dir_all(&source).unwrap();
            std::fs::write(source.join("youtfast.exe"), b"new exe").unwrap();
            std::fs::write(source.join("owned-new.dll"), b"new package dll").unwrap();
            std::fs::write(
                source.join("native-dependencies.json"),
                r#"{"libraries":["owned-new.dll"]}"#,
            )
            .unwrap();
            let script = write_windows_apply_script(&staging).unwrap();
            if !include_old_manifest {
                std::fs::remove_file(target.join("native-dependencies.json")).unwrap();
                std::fs::write(target.join("legacy.dll"), b"legacy package or user dll").unwrap();
            }
            let args = [
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-ExecutionPolicy".to_owned(),
                "Bypass".to_owned(),
                "-File".to_owned(),
                script.to_string_lossy().into_owned(),
                "-ParentPid".to_owned(),
                "2147483647".to_owned(),
                "-Mode".to_owned(),
                "portable".to_owned(),
                "-SourcePath".to_owned(),
                source.to_string_lossy().into_owned(),
                "-TargetDir".to_owned(),
                target.to_string_lossy().into_owned(),
                "-TargetExe".to_owned(),
                target
                    .join("renamed-youtfast.exe")
                    .to_string_lossy()
                    .into_owned(),
                "-StagingDir".to_owned(),
                staging.to_string_lossy().into_owned(),
                "-Relaunch".to_owned(),
                "0".to_owned(),
            ];
            hidden_command("powershell.exe")
                .args(args)
                .status()
                .unwrap()
                .success()
        };

        assert!(apply_bundle("stage-with-manifest", true));
        assert_eq!(
            std::fs::read(target.join("renamed-youtfast.exe")).unwrap(),
            b"new exe"
        );
        assert!(!target.join("owned-old.dll").exists());
        assert_eq!(
            std::fs::read(target.join("user-tool.dll")).unwrap(),
            b"user dll"
        );

        assert!(apply_bundle("stage-legacy", false));
        assert_eq!(
            std::fs::read(target.join("renamed-youtfast.exe")).unwrap(),
            b"new exe"
        );
        assert_eq!(
            std::fs::read(target.join("legacy.dll")).unwrap(),
            b"legacy package or user dll"
        );
        assert!(!target.join("youtfast.exe").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

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
    fn parses_release_json_and_extracts_changelog_and_assets() {
        let json = r####"{
            "tag_name": "v0.3.0",
            "draft": false,
            "html_url": "https://github.com/bcosta19/youtfast/releases/tag/v0.3.0",
            "body": "### Added\n- Automatic minor and patch updates with changelog display.\n\n### Fixed\n- Faster startup.",
            "assets": [
                {
                    "name": "youtfast-v0.3.0-x86_64-unknown-linux-gnu.tar.gz",
                    "browser_download_url": "https://github.com/bcosta19/youtfast/releases/download/v0.3.0/youtfast-v0.3.0-x86_64-unknown-linux-gnu.tar.gz"
                },
                {
                    "name": "youtfast-0.3.0-1-x86_64.pkg.tar.zst",
                    "browser_download_url": "https://github.com/bcosta19/youtfast/releases/download/v0.3.0/youtfast-0.3.0-1-x86_64.pkg.tar.zst"
                },
                {
                    "name": "youtfast-v0.3.0-x86_64-windows-setup.exe",
                    "browser_download_url": "https://github.com/bcosta19/youtfast/releases/download/v0.3.0/youtfast-v0.3.0-x86_64-windows-setup.exe"
                },
                {
                    "name": "checksums.txt",
                    "browser_download_url": "https://github.com/bcosta19/youtfast/releases/download/v0.3.0/checksums.txt"
                }
            ]
        }"####;
        let info = parse_release_json(json, "0.2.0", "linux", "x86_64")
            .unwrap()
            .unwrap();
        assert_eq!(info.version, "0.3.0");
        assert_eq!(info.kind, UpdateKind::Minor);
        assert_eq!(info.method, InstallMethod::LinuxNative);
        assert!(info.can_self_update());
        assert!(info.can_auto_update());
        assert!(info.changelog.contains("Added"));
        assert!(
            info.changelog
                .contains("• Automatic minor and patch updates with changelog display.")
        );
        assert_eq!(
            info.asset_name.as_deref(),
            Some("youtfast-v0.3.0-x86_64-unknown-linux-gnu.tar.gz")
        );

        let arch = parse_release_json_for(
            json,
            "0.2.0",
            InstallTarget {
                method: InstallMethod::ArchPacman,
                can_install: true,
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            arch.asset_name.as_deref(),
            Some("youtfast-0.3.0-1-x86_64.pkg.tar.zst")
        );
        assert!(arch.can_self_update());
        assert!(!arch.can_auto_update());
        assert!(!should_auto_install(true, &arch));

        let flatpak = parse_release_json_for(
            json,
            "0.2.0",
            InstallTarget {
                method: InstallMethod::Flatpak,
                can_install: false,
            },
        )
        .unwrap()
        .unwrap();
        assert!(!flatpak.can_self_update());
        assert!(!flatpak.can_auto_update());

        // Same or older version returns None.
        assert!(
            parse_release_json(json, "0.3.0", "linux", "x86_64")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn bundled_changelog_extracts_current_version_notes() {
        let notes = bundled_changelog_for(CURRENT_VERSION);
        let current = extract_changelog_section(BUNDLED_CHANGELOG, CURRENT_VERSION)
            .expect("the compiled version has its own changelog section");
        assert!(!current.is_empty());
        assert_eq!(notes, current);
        let notes_030 = bundled_changelog_for("0.3.0");
        assert!(notes_030.contains("About section and built-in updater"));
        let notes_020 = bundled_changelog_for("0.2.0");
        assert!(notes_020.contains("Spanish, French, German and Italian"));
    }

    #[test]
    fn checksum_verification_accepts_valid_digest_and_rejects_tampered_bytes() {
        let payload = b"synthetic-youtfast-binary-payload";
        let expected: String = Sha256::digest(payload)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let checksums = format!(
            "{expected}  youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz\n\
             0000000000000000000000000000000000000000000000000000000000000000  other.zip\n"
        );
        let parsed = parse_checksum_for(
            &checksums,
            "youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz",
        )
        .unwrap();
        assert_eq!(parsed, expected);
        assert!(verify_sha256(payload, &parsed));
        assert!(!verify_sha256(b"tampered-payload", &parsed));
    }

    #[cfg(unix)]
    #[test]
    fn install_archive_verifies_new_binary_and_replaces_atomically() {
        let root = test_root("install");
        let _ = std::fs::remove_dir_all(&root);
        let pkg_dir = root.join("youtfast-v0.2.1-x86_64-unknown-linux-gnu");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let candidate = b"#!/bin/sh\ncase \"$0\" in */.youtfast-update-*.tmp) echo youtfast 0.2.1;; *) exit 1;; esac\n";
        std::fs::write(pkg_dir.join("youtfast"), candidate).unwrap();

        let archive_path = root.join("youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz");
        let status = std::process::Command::new("tar")
            .args([
                "-czf",
                &archive_path.to_string_lossy(),
                "-C",
                &root.to_string_lossy(),
                "youtfast-v0.2.1-x86_64-unknown-linux-gnu",
            ])
            .status()
            .unwrap();
        assert!(status.success());

        let target_exe = root.join("installed-youtfast");
        std::fs::write(&target_exe, b"#!/bin/sh\necho youtfast 0.2.0\n").unwrap();
        let archive_bytes = std::fs::read(&archive_path).unwrap();

        install_archive(
            &archive_bytes,
            "youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz",
            "0.2.1",
            &target_exe,
            &root,
        )
        .unwrap();

        assert_eq!(std::fs::read(&target_exe).unwrap(), candidate);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn install_archive_keeps_existing_binary_when_new_binary_fails_smoke_test() {
        let root = test_root("incompatible");
        let _ = std::fs::remove_dir_all(&root);
        let pkg_dir = root.join("youtfast-v0.2.1-x86_64-unknown-linux-gnu");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("youtfast"), b"#!/bin/sh\nexit 127\n").unwrap();

        let archive_path = root.join("youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz");
        let status = std::process::Command::new("tar")
            .args([
                "-czf",
                &archive_path.to_string_lossy(),
                "-C",
                &root.to_string_lossy(),
                "youtfast-v0.2.1-x86_64-unknown-linux-gnu",
            ])
            .status()
            .unwrap();
        assert!(status.success());

        let target_exe = root.join("installed-youtfast");
        std::fs::write(&target_exe, b"#!/bin/sh\necho youtfast 0.2.0\n").unwrap();
        let archive_bytes = std::fs::read(&archive_path).unwrap();

        let err = install_archive(
            &archive_bytes,
            "youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz",
            "0.2.1",
            &target_exe,
            &root,
        )
        .unwrap_err();
        assert_eq!(err, ERROR_INCOMPATIBLE);
        assert_eq!(
            std::fs::read_to_string(&target_exe).unwrap(),
            "#!/bin/sh\necho youtfast 0.2.0\n"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn allowed_urls_accept_github_cdn_and_reject_untrusted_hosts() {
        assert!(allowed_release_url(
            "https://api.github.com/repos/bcosta19/youtfast/releases/latest"
        ));
        assert!(allowed_release_url(
            "https://objects.githubusercontent.com/github-production-release-asset/1"
        ));
        assert!(allowed_release_url(
            "https://release-assets.githubusercontent.com/github-production-release-asset/1"
        ));
        assert!(!allowed_release_url(
            "http://github.com/bcosta19/youtfast/releases"
        ));
        assert!(!allowed_release_url("https://evil.example/payload.tar.gz"));
    }

    #[test]
    fn download_retries_transient_503_and_follows_validated_redirect() {
        let root = test_root("retry");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let payload = b"verified-release-archive-bytes";
        let expected_sha: String = Sha256::digest(payload)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            for step in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut req = [0u8; 1024];
                let _ = stream.read(&mut req);
                match step {
                    0 => {
                        let _ = stream.write_all(
                            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                    }
                    1 => {
                        let header = format!(
                            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        );
                        let _ = stream.write_all(header.as_bytes());
                    }
                    _ => {
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            payload.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(payload);
                    }
                }
            }
        });

        let dest = root.join("download.bin");
        download_verified_file_with(
            &download_agent(),
            &format!("http://127.0.0.1:{port}/start"),
            &dest,
            &expected_sha,
            1024,
            |_| {},
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), payload);

        // Redirect to an untrusted host must be rejected.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let bad_server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut req = [0u8; 512];
            let _ = stream.read(&mut req);
            let _ = stream.write_all(
                b"HTTP/1.1 302 Found\r\nLocation: https://evil.example/asset\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        });
        let err = download_verified_file_with(
            &download_agent(),
            &format!("http://127.0.0.1:{port}/start"),
            &dest,
            &expected_sha,
            1024,
            |_| {},
        )
        .unwrap_err();
        bad_server.join().unwrap();
        assert_eq!(err, ERROR_DOWNLOAD);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn checksum_parser_skips_blank_and_comment_lines_and_prereleases_are_ignored() {
        let checksums = "\n# Release checksums\n\n\
            aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  other.tar.gz\n\
            bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb *youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz\n";
        assert_eq!(
            parse_checksum_for(checksums, "youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz")
                .as_deref(),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );

        assert!(Version::parse("v0.3.0-rc.1").is_none());
        let prerelease_json = r####"{
            "tag_name": "v0.3.0",
            "draft": false,
            "prerelease": true,
            "html_url": "https://github.com/bcosta19/youtfast/releases/tag/v0.3.0",
            "body": "### Added\n- RC build",
            "assets": []
        }"####;
        assert!(
            parse_release_json(prerelease_json, "0.2.0", "linux", "x86_64")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn strip_markdown_links_preserves_bracketed_ui_labels_and_clean_exe_path_strips_deleted() {
        let line = "- Added `[ save ]` button in [watch page](https://example.com/watch) and `[ load more ]`.";
        let cleaned = strip_markdown_links(line);
        assert_eq!(
            cleaned,
            "- Added [ save ] button in watch page and [ load more ]."
        );
        assert_eq!(
            clean_exe_path(Path::new("/usr/local/bin/youtfast (deleted)")),
            PathBuf::from("/usr/local/bin/youtfast")
        );
    }

    #[cfg(unix)]
    #[test]
    fn install_release_to_rejects_flatpak_and_bad_checksum_without_touching_existing() {
        let root = test_root("end-to-end");
        let _ = std::fs::remove_dir_all(&root);
        let cache = root.join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        let target_exe = root.join("bin/youtfast");
        std::fs::create_dir_all(target_exe.parent().unwrap()).unwrap();
        std::fs::write(&target_exe, b"#!/bin/sh\necho youtfast 0.2.0\n").unwrap();

        let flatpak_release = ReleaseInfo {
            version: "0.2.1".into(),
            kind: UpdateKind::Patch,
            method: InstallMethod::Flatpak,
            install_supported: false,
            changelog: String::new(),
            html_url: RELEASES_URL.into(),
            asset_name: Some("youtfast-v0.2.1-x86_64.flatpak".into()),
            asset_url: Some("https://github.com/bcosta19/youtfast/releases/download/v0.2.1/youtfast-v0.2.1-x86_64.flatpak".into()),
            checksums_url: Some("https://github.com/bcosta19/youtfast/releases/download/v0.2.1/checksums.txt".into()),
        };
        assert_eq!(
            install_release_to(&flatpak_release, &target_exe, &cache).unwrap_err(),
            ERROR_UNSUPPORTED
        );

        let pkg_dir = root.join("youtfast-v0.2.1-x86_64-unknown-linux-gnu");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(
            pkg_dir.join("youtfast"),
            b"#!/bin/sh\necho youtfast 0.2.1\n",
        )
        .unwrap();
        let archive_path = root.join("youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz");
        let status = std::process::Command::new("tar")
            .args([
                "-czf",
                &archive_path.to_string_lossy(),
                "-C",
                &root.to_string_lossy(),
                "youtfast-v0.2.1-x86_64-unknown-linux-gnu",
            ])
            .status()
            .unwrap();
        assert!(status.success());
        let archive_bytes = std::fs::read(&archive_path).unwrap();
        let wrong_sha = "0".repeat(64);
        let checksums = format!("{wrong_sha}  youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz\n");

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut req = [0u8; 1024];
                let n = stream.read(&mut req).unwrap();
                let req_text = String::from_utf8_lossy(&req[..n]);
                let body = if req_text.contains("checksums.txt") {
                    checksums.as_bytes()
                } else {
                    archive_bytes.as_slice()
                };
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(body);
            }
        });

        let release = ReleaseInfo {
            version: "0.2.1".into(),
            kind: UpdateKind::Patch,
            method: InstallMethod::LinuxNative,
            install_supported: true,
            changelog: String::new(),
            html_url: RELEASES_URL.into(),
            asset_name: Some("youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz".into()),
            asset_url: Some(format!(
                "http://127.0.0.1:{port}/youtfast-v0.2.1-x86_64-unknown-linux-gnu.tar.gz"
            )),
            checksums_url: Some(format!("http://127.0.0.1:{port}/checksums.txt")),
        };
        let err = install_release_to(&release, &target_exe, &cache).unwrap_err();
        server.join().unwrap();
        assert_eq!(err, ERROR_INTEGRITY);
        assert_eq!(
            std::fs::read_to_string(&target_exe).unwrap(),
            "#!/bin/sh\necho youtfast 0.2.0\n"
        );

        // cleanup_stale_staging removes older pending versions and preserves newer ones.
        let old_pending = cache.join("pending-update/v0.2.0");
        let new_pending = cache.join("pending-update/v0.3.0");
        std::fs::create_dir_all(&old_pending).unwrap();
        std::fs::create_dir_all(&new_pending).unwrap();
        cleanup_stale_staging(&cache, "0.2.1");
        assert!(!old_pending.exists());
        assert!(new_pending.exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn windows_staging_and_macos_bundle_swap_preserve_install_layout() {
        let root = test_root("staging-and-bundle");
        let _ = std::fs::remove_dir_all(&root);
        let cache = root.join("cache");
        std::fs::create_dir_all(&cache).unwrap();

        // Windows setup staging creates a clean pending plan + script.
        let win_dir = root.join("win-setup");
        std::fs::create_dir_all(&win_dir).unwrap();
        let setup_src = root.join("youtfast-v0.8.2-x86_64-windows-setup.exe");
        std::fs::write(&setup_src, b"synthetic-nsis-setup").unwrap();
        let pending_setup = stage_windows_setup(
            &setup_src,
            "youtfast-v0.8.2-x86_64-windows-setup.exe",
            "0.8.2",
            &win_dir.join("youtfast.exe"),
            &cache,
        )
        .unwrap();
        assert_eq!(pending_setup.mode, PendingMode::WindowsSetup);
        assert!(pending_setup.source_path.is_file());
        assert!(pending_setup.script_path.is_file());

        // Windows portable staging copies the full DLL/exe bundle without touching live target_dir yet.
        let bundle_src = root.join("portable-bundle");
        std::fs::create_dir_all(bundle_src.join("native-licenses")).unwrap();
        std::fs::write(bundle_src.join("youtfast.exe"), b"new-exe").unwrap();
        std::fs::write(bundle_src.join("libmpv-2.dll"), b"new-dll").unwrap();
        std::fs::write(bundle_src.join("native-licenses/LICENSE"), b"gpl").unwrap();
        let win_portable_dir = root.join("win-portable");
        std::fs::create_dir_all(&win_portable_dir).unwrap();
        std::fs::write(win_portable_dir.join("youtfast.exe"), b"old-exe").unwrap();
        let pending_portable = stage_windows_portable(
            &bundle_src,
            "0.8.2",
            &win_portable_dir.join("youtfast.exe"),
            &cache,
        )
        .unwrap();
        assert_eq!(pending_portable.mode, PendingMode::WindowsPortable);
        assert_eq!(
            std::fs::read(pending_portable.source_path.join("libmpv-2.dll")).unwrap(),
            b"new-dll"
        );
        assert_eq!(
            std::fs::read(win_portable_dir.join("youtfast.exe")).unwrap(),
            b"old-exe"
        );

        // macOS bundle replacement swaps the entire .app directory atomically.
        let apps = root.join("Applications");
        let existing_app = apps.join("YouTFast.app");
        std::fs::create_dir_all(existing_app.join("Contents/MacOS")).unwrap();
        std::fs::create_dir_all(existing_app.join("Contents/Frameworks")).unwrap();
        std::fs::write(existing_app.join("Contents/MacOS/youtfast"), b"old-bin").unwrap();
        std::fs::write(
            existing_app.join("Contents/Frameworks/libmpv.2.dylib"),
            b"old-dylib",
        )
        .unwrap();

        let new_app = root.join("staged/YouTFast.app");
        std::fs::create_dir_all(new_app.join("Contents/MacOS")).unwrap();
        std::fs::create_dir_all(new_app.join("Contents/Frameworks")).unwrap();
        std::fs::write(new_app.join("Contents/MacOS/youtfast"), b"new-bin").unwrap();
        std::fs::write(
            new_app.join("Contents/Frameworks/libmpv.2.dylib"),
            b"new-dylib",
        )
        .unwrap();

        replace_macos_bundle(&new_app, &existing_app.join("Contents/MacOS/youtfast")).unwrap();
        assert_eq!(
            std::fs::read(existing_app.join("Contents/MacOS/youtfast")).unwrap(),
            b"new-bin"
        );
        assert_eq!(
            std::fs::read(existing_app.join("Contents/Frameworks/libmpv.2.dylib")).unwrap(),
            b"new-dylib"
        );

        let _ = std::fs::remove_dir_all(&root);
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
