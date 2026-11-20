//! Platform bundle replacement (macOS `.app`) and post-exit staging (Windows).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::detect::find_app_bundle;
use super::install::{set_executable_mode, verify_extracted_binary};
use super::{ERROR_INSTALL, ERROR_UNSUPPORTED};

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
            let mut cmd = super::hidden_command("powershell.exe");
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
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
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

pub(super) fn prepare_and_verify_macos_app(
    new_app: &Path,
    new_bin: &Path,
    expected_version: &str,
) -> Result<(), String> {
    set_executable_mode(new_bin)?;
    #[cfg(target_os = "macos")]
    {
        let _ = super::hidden_command("/usr/bin/xattr")
            .args(["-dr", "com.apple.quarantine", &new_app.to_string_lossy()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        let verified = super::hidden_command("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict", &new_app.to_string_lossy()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !verified {
            return Err(super::ERROR_INTEGRITY.to_owned());
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = new_app;
    verify_extracted_binary(new_bin, expected_version)
}

pub(super) fn replace_macos_bundle(new_app: &Path, resolved_exe: &Path) -> Result<(), String> {
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
        super::hidden_command("/usr/bin/ditto")
            .args([new_app, &staged_app])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
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

pub(super) fn stage_windows_setup(
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

pub(super) fn stage_windows_portable(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::test_root;

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
            crate::updater::hidden_command("powershell.exe")
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
}
