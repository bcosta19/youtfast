import argparse
import re
import shutil
import subprocess
import tarfile
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APP_ID = "io.github.bcosta19.YouTFast"


def version(root=ROOT):
    return tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]


def validate_tag(tag, package_version):
    if not re.fullmatch(r"v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", tag):
        raise ValueError("Release tag must be vMAJOR.MINOR.PATCH")
    if tag != f"v{package_version}":
        raise ValueError("Tag does not match Cargo.toml")


def forbidden_path(name):
    parts = Path(name).parts
    return any(part in {"rustypipe_reports", "target", ".env", "cookies.txt"} or part.startswith(".env.") for part in parts)


def check(root=ROOT, tag=None):
    current = version(root)
    validate_tag(tag or f"v{current}", current)
    lock = tomllib.loads((root / "Cargo.lock").read_text())
    package = next(item for item in lock["package"] if item["name"] == "youtfast")
    if package["version"] != current:
        raise ValueError("Cargo.lock version does not match Cargo.toml")
    metadata = ET.parse(root / f"packaging/flatpak/{APP_ID}.metainfo.xml").getroot()
    if metadata.findtext("id") != APP_ID:
        raise ValueError("Desktop application ID mismatch")
    if metadata.find("releases/release").get("version") != current:
        raise ValueError("Desktop release version does not match Cargo.toml")
    if not (root / f"packaging/release-notes/v{current}.md").is_file():
        raise ValueError("Committed release notes are required")
    if tomllib.loads((root / "Cargo.toml").read_text())["package"]["license"] != "GPL-3.0-only":
        raise ValueError("Project license must match release documentation")
    if not (root / "LICENSE").is_file():
        raise ValueError("LICENSE is required")
    names = subprocess.check_output(["git", "ls-files", "-z"], cwd=root).decode().split("\0")
    if any(forbidden_path(name) for name in names if name):
        raise ValueError("Generated reports, credentials or build output are tracked")
    return current


def linux():
    current = check()
    name = f"youtfast-v{current}-x86_64-unknown-linux-gnu"
    dist = ROOT / "dist"
    directory = dist / name
    directory.mkdir(parents=True, exist_ok=False)
    shutil.copy2(ROOT / "target/release/youtfast", directory / "youtfast")
    for item in ["README.md", "LICENSE", "THIRD_PARTY_NOTICES.md", "packaging/portable.txt", "packaging/applications/youtfast.desktop", "packaging/icons/youtfast.svg", "assets/icons/LICENSE.txt", "vendor/rustypipe/LICENSE"]:
        output = directory / item
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / item, output)
    with tarfile.open(dist / f"{name}.tar.gz", "w:gz") as archive:
        archive.add(directory, arcname=name)


def source():
    current = check()
    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    name = f"youtfast-v{current}-source-with-dependencies"
    directory = dist / name
    directory.mkdir(exist_ok=False)
    exported = dist / "source-tree.tar"
    subprocess.run(["git", "archive", "--format=tar", "-o", str(exported), "HEAD"], cwd=ROOT, check=True)
    with tarfile.open(exported) as archive:
        archive.extractall(directory, filter="data")
    exported.unlink()
    cargo = directory / ".cargo"
    cargo.mkdir(exist_ok=True)
    with (cargo / "config.toml").open("w") as config:
        subprocess.run(["cargo", "vendor", "--locked", "--versioned-dirs", "vendor-deps"], cwd=directory, stdout=config, check=True)
    with (directory / "dependency-metadata.json").open("w") as metadata:
        subprocess.run(["cargo", "metadata", "--locked", "--offline", "--format-version", "1"], cwd=directory, stdout=metadata, check=True)
    with tarfile.open(dist / f"{name}.tar.gz", "w:gz") as archive:
        archive.add(directory, arcname=name)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["check", "linux", "source"])
    parser.add_argument("--tag")
    args = parser.parse_args()
    if args.command == "check":
        print(check(tag=args.tag))
    elif args.command == "linux":
        linux()
    else:
        source()
