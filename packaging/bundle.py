"""Create native release candidates on their own hosted operating systems.

These packages need interactive QA and a corresponding-native-source review
before the draft release is published. Dependency inventories are not sources.
"""

import argparse
import json
import os
import plistlib
import re
import shutil
import subprocess
import zipfile
from pathlib import Path

from release import ROOT, check, copy_public_files
from native_sources import msys_inventory


def run(*args):
    return subprocess.check_output([str(arg) for arg in args], text=True)


def dll_imports(text):
    return re.findall(r"DLL Name:\s+([^\s]+)", text)


def collect_dlls(binary, prefix, system, inspect):
    """Resolve non-system dependencies transitively; never guess missing DLLs."""
    available = {path.name.lower(): path for path in (prefix / "bin").glob("*.dll")}
    system_names = {path.name.lower() for path in system.glob("*.dll")}
    queue = [binary]
    found = {}
    while queue:
        for name in dll_imports(inspect(queue.pop())):
            key = name.lower()
            if key in found:
                continue
            # Prefer the UCRT distribution when it also provides a library.
            if key in available:
                found[key] = available[key]
                queue.append(available[key])
            elif key not in system_names and not key.startswith(("api-ms-win-", "ext-ms-win-")):
                raise ValueError(f"Unresolved runtime DLL: {name}")
    return sorted(found.values())


def windows(binary, prefix, dist):
    current = check()
    directory = dist / f"youtfast-v{current}-x86_64-pc-windows-gnu"
    directory.mkdir(parents=True, exist_ok=False)
    shutil.copy2(binary, directory / "youtfast.exe")
    system = Path(os.environ["SystemRoot"]) / "System32"
    libraries = collect_dlls(binary, prefix, system,
                            lambda path: run(prefix / "bin/objdump.exe", "-p", path))
    if not any("mpv" in library.name.lower() for library in libraries):
        raise ValueError("The Windows candidate must bundle libmpv")
    owners = set()
    for library in libraries:
        shutil.copy2(library, directory / library.name)
        owners.add(run("pacman", "-Qqo", f"/{prefix.name}/bin/{library.name}").strip())
    inventory = run("pacman", "-Qi", *sorted(owners))
    (directory / "native-dependencies.txt").write_text(inventory)
    (directory / "native-dependencies.json").write_text(json.dumps({
        "packages": msys_inventory(owners),
        "libraries": [path.name for path in libraries],
    }, indent=2))
    licenses = prefix / "share/licenses"
    if not licenses.is_dir():
        raise ValueError("MSYS2 native dependency notices are missing")
    shutil.copytree(licenses, directory / "native-licenses")
    copy_public_files(directory)
    (directory / "INSTALL.txt").write_text(
        "YouTFast Windows x86_64 release candidate. Extract the entire ZIP, then run youtfast.exe.\n"
        "libmpv and its imported DLLs are included; keep them beside the executable.\n"
        "Install current yt-dlp and Deno separately and make them available on PATH.\n"
        "Browser-session import additionally needs Python with the yt_dlp module.\n"
        "This package has no publisher signature or installer. Interactive QA and\n"
        "matching native dependency sources must be reviewed before publication.\n"
    )
    return directory


def macho_imports(text):
    # otool's first line names the inspected binary; paths may contain spaces.
    return [line.strip().split(" (compatibility version", 1)[0]
            for line in text.splitlines()[1:] if line.strip()]


def macho_rpaths(text):
    """LC_RPATH entries from `otool -l` output, in load-command order."""
    return re.findall(r"cmd LC_RPATH\s+cmdsize \d+\s+path (.+?) \(offset \d+\)", text)


def dedupe_rpaths(path, inspect, tool):
    """dylibbundler rewrites every rpath to the bundle prefix, so a library
    with several (libmpv's Swift toolchain ones) ends up with duplicates, and
    macOS 15's dyld refuses to load it. Keep one of each."""
    entries = macho_rpaths(inspect(path))
    for entry in sorted({entry for entry in entries if entries.count(entry) > 1}):
        for _ in range(entries.count(entry)):
            if entry not in macho_rpaths(inspect(path)):
                break
            tool("install_name_tool", "-delete_rpath", entry, path)
        tool("install_name_tool", "-add_rpath", entry, path)
    remaining = macho_rpaths(inspect(path))
    if len(remaining) != len(set(remaining)):
        raise ValueError(f"Duplicate LC_RPATH remains in {path.name}")


def verify_macho(paths, executable_dir, inspect, inspect_rpaths=None):
    for path in paths:
        for dependency in macho_imports(inspect(path)):
            if dependency.startswith(("/usr/lib/", "/System/Library/")):
                continue
            if dependency.startswith("@executable_path/"):
                resolved = executable_dir / dependency.removeprefix("@executable_path/")
            elif dependency.startswith("@loader_path/"):
                resolved = path.parent / dependency.removeprefix("@loader_path/")
            else:
                raise ValueError("macOS candidate has an external or unresolved dependency")
            if not resolved.resolve().is_relative_to(executable_dir.parent.resolve()):
                raise ValueError("macOS dependency escapes the application bundle")
            if not resolved.is_file():
                raise ValueError("macOS candidate is missing a bundled dependency")
        if inspect_rpaths:
            rpaths = macho_rpaths(inspect_rpaths(path))
            if len(rpaths) != len(set(rpaths)):
                raise ValueError("macOS dyld rejects duplicate LC_RPATH entries")


def macos(binary, dist):
    current = check()
    if run("uname", "-m").strip() != "arm64":
        raise ValueError("The macOS candidate currently targets Apple Silicon only")
    directory = dist / f"youtfast-v{current}-aarch64-apple-darwin"
    app = directory / "YouTFast.app/Contents"
    executable_dir = app / "MacOS"
    executable_dir.mkdir(parents=True, exist_ok=False)
    executable = executable_dir / "youtfast"
    shutil.copy2(binary, executable)
    if run("lipo", "-archs", executable).strip() != "arm64":
        raise ValueError("The executable architecture does not match the macOS asset")
    with (app / "Info.plist").open("wb") as output:
        plistlib.dump({
            "CFBundleName": "YouTFast", "CFBundleDisplayName": "YouTFast",
            "CFBundleIdentifier": "io.github.bcosta19.YouTFast",
            "CFBundleExecutable": "youtfast", "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": current, "CFBundleVersion": current,
            "LSMinimumSystemVersion": "15.0", "NSHighResolutionCapable": True,
            "LSEnvironment": {"PATH": "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"},
        }, output)
    subprocess.run(["dylibbundler", "-b", "-cd", "-x", str(executable),
                    "-d", str(app / "Frameworks"),
                    "-p", "@executable_path/../Frameworks/"], check=True)
    libraries = list((app / "Frameworks").glob("*.dylib"))
    if not any("mpv" in path.name for path in libraries):
        raise ValueError("The macOS candidate must bundle libmpv")
    for path in [executable, *libraries]:
        dedupe_rpaths(path, lambda item: run("otool", "-l", item), run)
    verify_macho([executable, *libraries], executable_dir, lambda path: run("otool", "-L", path),
                 lambda path: run("otool", "-l", path))
    # Preserve the exact installed formula inventory and upstream notices.
    formulas = ["mpv", *run("brew", "deps", "mpv").split()]
    inventory = json.loads(run("brew", "info", "--json=v2", *formulas))
    (directory / "native-dependencies.json").write_text(json.dumps(inventory, indent=2))
    resources = app / "Resources/licenses"
    resources.mkdir(parents=True)
    for formula in formulas:
        installed = Path(run("brew", "--prefix", formula).strip())
        destination = resources / formula
        destination.mkdir()
        for item in installed.iterdir():
            if item.is_file() and re.match(r"(license|copying|copyright|notice)", item.name, re.I):
                shutil.copy2(item, destination / item.name)
        receipt = installed / "INSTALL_RECEIPT.json"
        if receipt.is_file():
            shutil.copy2(receipt, destination / receipt.name)
        formula_sources = installed / ".brew"
        if formula_sources.is_dir():
            shutil.copytree(formula_sources, destination / "formula")
    copy_public_files(resources / "youtfast")
    copy_public_files(directory)
    (directory / "INSTALL.txt").write_text(
        "YouTFast macOS 15+ Apple Silicon release candidate.\n"
        "Copy YouTFast.app to Applications. libmpv and linked dylibs are bundled.\n"
        "Install current yt-dlp and Deno separately, for example through Homebrew.\n"
        "Finder launches include /opt/homebrew/bin and /usr/local/bin on PATH.\n"
        "Browser-session import also needs Python with the yt_dlp module.\n"
        "The app is ad-hoc signed, not Developer ID signed or notarized. Gatekeeper\n"
        "may block it. Use macOS's normal trusted-app approval UI after verification.\n"
        "Interactive QA and matching native sources remain publication gates.\n"
    )
    subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(app.parent)], check=True)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app.parent)], check=True)
    return directory


def archive(directory):
    output = directory.parent / f"{directory.name}.zip"
    # macOS uses ditto so executable modes, bundle metadata and symlinks survive.
    if (directory / "YouTFast.app").is_dir():
        subprocess.run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent",
                        str(directory), str(output)], check=True)
    else:
        with zipfile.ZipFile(output, "x", zipfile.ZIP_DEFLATED) as bundle:
            for path in sorted(directory.rglob("*")):
                if path.is_file():
                    bundle.write(path, path.relative_to(directory.parent))
    return output


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=["windows", "macos"])
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--prefix", type=Path)
    args = parser.parse_args()
    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    if args.platform == "windows":
        if args.prefix is None:
            parser.error("Windows packaging requires --prefix for UCRT64")
        directory = windows(args.binary.resolve(), args.prefix.resolve(), dist)
    else:
        directory = macos(args.binary.resolve(), dist)
    print(archive(directory))
