//! Download orchestration, archive extraction, atomic binary swap, and relaunch.

use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::bundle::{
    prepare_and_verify_macos_app, replace_macos_bundle, stage_windows_portable, stage_windows_setup,
};
use super::detect::{clean_exe_path, dir_is_writable, find_app_bundle, is_effective_root};
use super::fetch::{
    MAX_ARCHIVE_BYTES, MAX_CHECKSUMS_BYTES, download_agent, download_verified_file_with,
    fetch_bytes, parse_checksum_for,
};
use super::{
    ERROR_AUTH, ERROR_INCOMPATIBLE, ERROR_INSTALL, ERROR_INTEGRITY, ERROR_NOT_WRITABLE,
    ERROR_UNSUPPORTED, InstallMethod, InstallOutcome, ReleaseInfo, Version, hidden_command,
};

const STALE_PENDING_CLEANUP_SECS: u64 = 15 * 60;

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

pub(super) fn set_executable_mode(path: &Path) -> Result<(), String> {
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

fn run_with_etxtbsy_retry<T>(mut run: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    // On Linux, a concurrent thread spawning a child process can briefly hold
    // a newly closed O_WRONLY fd across fork() before execve() closes O_CLOEXEC,
    // causing execve() of the freshly written binary to fail with ETXTBSY.
    let mut attempts = 0;
    loop {
        match run() {
            Ok(value) => return Ok(value),
            Err(error)
                if error.kind() == std::io::ErrorKind::ExecutableFileBusy && attempts < 10 =>
            {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    }
}

pub(super) fn verify_extracted_binary(bin: &Path, expected_version: &str) -> Result<(), String> {
    let output = run_with_etxtbsy_retry(|| {
        hidden_command(&bin.to_string_lossy())
            .arg("--version")
            .stdin(Stdio::null())
            .output()
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::{RELEASES_URL, UpdateKind, test_root};
    use std::io::{Read, Write};
    use std::net::TcpListener;

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

    #[test]
    fn etxtbsy_retry_recovers_transient_busy_without_retrying_other_errors() {
        let mut calls = 0;
        let ok = run_with_etxtbsy_retry(|| {
            calls += 1;
            if calls < 3 {
                Err(std::io::Error::from(std::io::ErrorKind::ExecutableFileBusy))
            } else {
                Ok("verified")
            }
        })
        .unwrap();
        assert_eq!(ok, "verified");
        assert_eq!(calls, 3);

        let mut perm_calls = 0;
        let err = run_with_etxtbsy_retry::<()>(|| {
            perm_calls += 1;
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        })
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(perm_calls, 1);
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
}
