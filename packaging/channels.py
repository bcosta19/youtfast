"""Render distribution channel manifests (Flathub, Winget, Homebrew Cask) from release assets."""

import argparse
import hashlib
import json
import tarfile
from pathlib import Path

from release import APP_ID, ROOT, check


def sha256_file(path: Path) -> str:
    if not path.is_file():
        raise ValueError(f"Missing release asset: {path}")
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def render_flathub(version: str, source_sha256: str, root: Path = ROOT, source_url: str | None = None) -> dict:
    if len(source_sha256) != 64:
        raise ValueError("Source archive SHA-256 must be 64 hex characters")
    manifest = json.loads((root / f"packaging/flatpak/{APP_ID}.json").read_text())
    manifest["sdk-extensions"] = ["org.freedesktop.Sdk.Extension.rust-stable"]
    manifest["build-options"] = {
        "append-path": "/usr/lib/sdk/rust-stable/bin",
        "env": {
            "CARGO_HOME": "/run/build/youtfast/cargo",
        },
    }
    url = source_url or (
        f"https://github.com/bcosta19/youtfast/releases/download/v{version}/"
        f"youtfast-v{version}-source-with-dependencies.tar.gz"
    )
    for module in manifest["modules"]:
        if module["name"] == "youtfast":
            module["build-commands"] = [
                "cargo build --release --locked --offline",
                "install -Dm755 target/release/youtfast /app/bin/youtfast",
                *module["build-commands"][1:],
            ]
            module["sources"] = [
                {
                    "type": "archive",
                    "url": url,
                    "sha256": source_sha256,
                }
            ]
            break
    else:
        raise ValueError("youtfast module not found in Flatpak manifest")
    return manifest


def render_winget(version: str, windows_sha256: str, installer_url: str | None = None) -> str:
    if len(windows_sha256) != 64:
        raise ValueError("Windows archive SHA-256 must be 64 hex characters")
    url = installer_url or (
        f"https://github.com/bcosta19/youtfast/releases/download/v{version}/"
        f"youtfast-v{version}-x86_64-pc-windows-gnu.zip"
    )
    upper_sha = windows_sha256.upper()
    return (
        "# yaml-language-server: $schema=https://aka.ms/winget-manifest.singleton.1.6.0.schema.json\n"
        "PackageIdentifier: bcosta19.YouTFast\n"
        f"PackageVersion: {version}\n"
        "PackageLocale: en-US\n"
        "Publisher: bcosta19\n"
        "PublisherUrl: https://github.com/bcosta19\n"
        "PackageName: YouTFast\n"
        "PackageUrl: https://github.com/bcosta19/youtfast\n"
        "License: GPL-3.0-only\n"
        f"LicenseUrl: https://github.com/bcosta19/youtfast/blob/v{version}/LICENSE\n"
        "ShortDescription: Small native YouTube client built with Rust, egui, and libmpv\n"
        "Commands:\n"
        "  - youtfast\n"
        "Dependencies:\n"
        "  PackageDependencies:\n"
        "    - PackageIdentifier: yt-dlp.yt-dlp\n"
        "    - PackageIdentifier: DenoLand.Deno\n"
        "Installers:\n"
        "  - Architecture: x64\n"
        "    InstallerType: zip\n"
        "    NestedInstallerType: portable\n"
        "    NestedInstallerFiles:\n"
        f"      - RelativeFilePath: youtfast-v{version}-x86_64-pc-windows-gnu\\youtfast.exe\n"
        "        PortableCommandAlias: youtfast\n"
        f"    InstallerUrl: {url}\n"
        f"    InstallerSha256: {upper_sha}\n"
        "ManifestType: singleton\n"
        "ManifestVersion: 1.6.0\n"
    )


def render_homebrew_cask(version: str, macos_sha256: str, archive_url: str | None = None) -> str:
    if len(macos_sha256) != 64:
        raise ValueError("macOS archive SHA-256 must be 64 hex characters")
    url = archive_url or (
        "https://github.com/bcosta19/youtfast/releases/download/"
        "v#{version}/youtfast-v#{version}-aarch64-apple-darwin.zip"
    )
    return (
        'cask "youtfast" do\n'
        f'  version "{version}"\n'
        f'  sha256 "{macos_sha256.lower()}"\n'
        "\n"
        f'  url "{url}"\n'
        '  name "YouTFast"\n'
        '  desc "Small native YouTube client built with Rust, egui, and libmpv"\n'
        '  homepage "https://github.com/bcosta19/youtfast"\n'
        "\n"
        "  depends_on arch: :arm64\n"
        '  depends_on macos: ">= :sequoia"\n'
        '  depends_on formula: "deno"\n'
        '  depends_on formula: "yt-dlp"\n'
        "\n"
        '  app "youtfast-v#{version}-aarch64-apple-darwin/YouTFast.app"\n'
        '  binary "#{appdir}/YouTFast.app/Contents/MacOS/youtfast"\n'
        "\n"
        "  postflight do\n"
        '    system_command "/usr/bin/xattr",\n'
        '                   args: ["-dr", "com.apple.quarantine", "#{appdir}/YouTFast.app"]\n'
        "  end\n"
        "\n"
        "  zap trash: [\n"
        '    "~/Library/Application Support/youtfast",\n'
        '    "~/Library/Caches/youtfast",\n'
        "  ]\n"
        "end\n"
    )


def build_channels(dist: Path, output: Path, root: Path = ROOT) -> Path:
    current = check(root)
    source_archive = dist / f"youtfast-v{current}-source-with-dependencies.tar.gz"
    windows_archive = dist / f"youtfast-v{current}-x86_64-pc-windows-gnu.zip"
    macos_archive = dist / f"youtfast-v{current}-aarch64-apple-darwin.zip"

    flathub = render_flathub(current, sha256_file(source_archive), root=root)
    winget = render_winget(current, sha256_file(windows_archive))
    cask = render_homebrew_cask(current, sha256_file(macos_archive))

    output.mkdir(parents=True, exist_ok=True)
    flathub_dir = output / "flathub"
    winget_dir = output / "winget"
    homebrew_dir = output / "homebrew/Casks"
    flathub_dir.mkdir(parents=True, exist_ok=True)
    winget_dir.mkdir(parents=True, exist_ok=True)
    homebrew_dir.mkdir(parents=True, exist_ok=True)

    (flathub_dir / f"{APP_ID}.json").write_text(json.dumps(flathub, indent=2) + "\n")
    (winget_dir / f"bcosta19.YouTFast.yaml").write_text(winget)
    (homebrew_dir / "youtfast.rb").write_text(cask)
    return output


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dist", type=Path, nargs="?", default=ROOT / "dist")
    args = parser.parse_args()
    current = check()
    channels_dir = build_channels(args.dist, args.dist / "channels")
    bundle_path = args.dist / f"youtfast-v{current}-channels.tar.gz"
    with tarfile.open(bundle_path, "w:gz") as archive:
        archive.add(channels_dir, arcname="channels")
