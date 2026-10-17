"""Collect native sources matching the packages used by the candidate runners.

MSYS2 publishes full matching source packages. Homebrew fetches sources,
resources and patches through the exact installed formula recipes. Downloaded
upstream build scripts are archived, never executed by this collector.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tarfile
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

from release import ROOT, check

MAX_SOURCE_BYTES = 1024 * 1024 * 1024


def run(*args, env=None):
    result = subprocess.run([str(arg) for arg in args], env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode:
        raise ValueError(f"Native source command failed: {Path(args[0]).name}")
    return result.stdout.decode("utf-8")


def fields(text, separator=" = "):
    result = {}
    for line in text.splitlines():
        key, found, value = line.strip().partition(separator)
        if found:
            result.setdefault(key, []).append(value)
    return result


def msys_inventory(owners):
    database = Path(run("cygpath", "-m", "/var/lib/pacman/local").strip())
    result = []
    for owner in sorted(owners):
        name, version = run("pacman", "-Q", owner).split()
        text = (database / f"{name}-{version}" / "desc").read_text(encoding="utf-8")
        metadata = {}
        for block in text.split("\n\n"):
            lines = block.strip().splitlines()
            if lines and lines[0].startswith("%"):
                metadata[lines[0].strip("%")] = lines[1:]
        base = metadata.get("BASE", [None])[0]
        if not base or metadata.get("VERSION") != [version]:
            raise ValueError("MSYS2 installed source identity is incomplete")
        result.append({"name": name, "version": version, "base": base,
                       "license": metadata.get("LICENSE", []), "installed_metadata": text})
    return result


def download(url, destination, maximum=MAX_SOURCE_BYTES):
    parsed = urllib.parse.urlsplit(url)
    if parsed.scheme != "https" or parsed.username or parsed.password or parsed.hostname != "repo.msys2.org":
        raise ValueError("Native source download requires the official HTTPS MSYS2 server")
    digest = hashlib.sha256()
    total = 0
    with urllib.request.urlopen(url, timeout=60) as response, destination.open("xb") as output:
        while chunk := response.read(1024 * 1024):
            total += len(chunk)
            if total > maximum:
                raise ValueError("Native source package exceeds the download limit")
            output.write(chunk)
            digest.update(chunk)
    return digest.hexdigest()


def validate_msys_source(text, base, installed_version):
    metadata = fields(text)
    epoch = metadata.get("epoch", [None])[0]
    version = metadata.get("pkgver", [""])[0] + "-" + metadata.get("pkgrel", [""])[0]
    if epoch and epoch != "0":
        version = epoch + ":" + version
    if metadata.get("pkgbase") != [base] or version != installed_version:
        raise ValueError("MSYS2 source identity does not match the bundled binary package")


def collect_windows(candidate, output):
    packages = json.loads((candidate / "native-dependencies.json").read_text())["packages"]
    sources = {}
    for package in packages:
        base, version = package["base"], package["version"]
        if not re.fullmatch(r"[A-Za-z0-9+_.-]+", base) or not re.fullmatch(r"[A-Za-z0-9:+_.~-]+", version):
            raise ValueError("Invalid MSYS2 source identity")
        key = (base, version)
        if key in sources:
            continue
        filename = f"{base}-{version.split(':', 1)[-1]}.src.tar.zst"
        url = f"https://repo.msys2.org/mingw/sources/{filename}"
        archive = output / filename
        digest = download(url, archive)
        members = run("tar", "-tf", archive).splitlines()
        srcinfo = next((name for name in members if name == f"{base}/.SRCINFO"), None)
        if srcinfo is None or len(members) < 4:
            raise ValueError("MSYS2 full source package or metadata is missing")
        validate_msys_source(run("tar", "-xOf", archive, srcinfo), base, version)
        sources[key] = {"base": base, "version": version, "url": url,
                        "archive": filename, "sha256": digest}
    if not sources:
        raise ValueError("No bundled Windows dependencies were identified")
    shutil.copy2(candidate / "native-dependencies.json", output / "native-dependencies.json")
    licenses = candidate / "native-licenses"
    shutil.copytree(licenses, output / "notices")
    return list(sources.values())


def matching_brew_version(formula):
    version = formula["versions"]["stable"]
    if formula.get("revision", 0):
        version += "_" + str(formula["revision"])
    if not any(item["version"] == version for item in formula["installed"]):
        raise ValueError("Homebrew formula changed since installation; rebuild before collecting sources")
    return version


def collect_macos(candidate, output):
    inventory = json.loads((candidate / "native-dependencies.json").read_text())
    recipes = output / "recipes"
    recipes.mkdir()
    formula_paths = []
    sources = []
    for formula in inventory["formulae"]:
        name = formula["name"]
        version = matching_brew_version(formula)
        prefix = Path(run("brew", "--prefix", name).strip()).resolve()
        if prefix.name != version:
            raise ValueError("Homebrew installed keg does not match candidate metadata")
        recipe = prefix / f".brew/{name}.rb"
        expected = formula["ruby_source_checksum"]["sha256"]
        if hashlib.sha256(recipe.read_bytes()).hexdigest() != expected:
            raise ValueError("Installed Homebrew recipe differs from the recorded source identity")
        copied = recipes / f"{name}.rb"
        shutil.copy2(recipe, copied)
        shutil.copy2(prefix / "INSTALL_RECEIPT.json", recipes / f"{name}-receipt.json")
        formula_paths.append(copied)
        sources.append({"formula": name, "version": version, "recipe_sha256": expected})
    if not sources:
        raise ValueError("No bundled macOS dependencies were identified")
    # A dedicated cache includes all downloaded resources/patches without
    # accidentally archiving the maintainer's existing cache or credentials.
    environment = os.environ.copy()
    environment.update(HOMEBREW_CACHE=str((output / "downloads").resolve()),
                       HOMEBREW_NO_AUTO_UPDATE="1", HOMEBREW_NO_ANALYTICS="1")
    run("brew", "fetch", "--build-from-source", "--retry", *formula_paths, env=environment)
    if not any(path.is_file() for path in (output / "downloads").rglob("*")):
        raise ValueError("Homebrew did not collect source downloads")
    shutil.copy2(candidate / "native-dependencies.json", output / "native-dependencies.json")
    shutil.copytree(candidate / "YouTFast.app/Contents/Resources/licenses", output / "notices")
    return sources


def archive_sources(directory, destination, identities):
    inventory = []
    for path in sorted(directory.rglob("*")):
        if path.is_symlink() and not path.resolve().is_relative_to(directory.resolve()):
            raise ValueError("Native source cache contains an external symlink")
        if not path.is_file():
            continue
        if not path.resolve().is_relative_to(directory.resolve()):
            raise ValueError("Native source cache contains an external symlink")
        with path.open("rb") as source:
            digest = hashlib.file_digest(source, "sha256").hexdigest()
        inventory.append({"path": path.relative_to(directory).as_posix(), "sha256": digest})
    (directory / "source-manifest.json").write_text(json.dumps({
        "identities": identities, "files": inventory,
        "review": "Collected sources require license/build review before publication; inventories alone are not sources.",
    }, indent=2))

    def safe_link(info):
        if info.issym():
            path = directory.parent / info.name
            info.linkname = os.path.relpath(path.resolve(), path.parent.resolve())
        return info

    with tarfile.open(destination, "w:gz") as archive:
        archive.add(directory, arcname=directory.name, filter=safe_link)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=["windows", "macos"])
    parser.add_argument("candidate", type=Path)
    args = parser.parse_args()
    current = check()
    output = ROOT / f"dist/native-sources/youtfast-v{current}-{args.platform}-native-sources"
    output.mkdir(parents=True, exist_ok=False)
    identities = (collect_windows if args.platform == "windows" else collect_macos)(args.candidate, output)
    archive_sources(output, ROOT / f"dist/{output.name}.tar.gz", identities)
