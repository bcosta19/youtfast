"""Pinned yt-dlp and Deno for the Windows ZIP.

Playback needs both, and a Windows user has neither on PATH, so the ZIP ships
the official builds beside youtfast.exe (the app looks there first). Versions,
URLs and SHA-256 digests live in windows-tools.json; every download is checked
against them. YouTube breaks old yt-dlp often, so bump the pins for each
release (RELEASING.md): `python3 packaging/windows_tools.py latest` prints the
newest upstream versions and digests as a replacement manifest.
"""

import argparse
import hashlib
import json
import shutil
import tempfile
import urllib.request
import zipfile
from pathlib import Path

from native_sources import download_checked
from release import ROOT

MANIFEST = ROOT / "packaging/windows-tools.json"
EXECUTABLES = {"yt-dlp": "yt-dlp.exe", "deno": "deno.exe"}
MAX_DOWNLOAD_BYTES = 256 * 1024 * 1024
DENO_ASSET = "deno-x86_64-pc-windows-msvc.zip"


def load(path=MANIFEST):
    manifest = json.loads(path.read_text())
    if set(manifest) != set(EXECUTABLES):
        raise ValueError("windows-tools.json must pin exactly yt-dlp and deno")
    for name, tool in manifest.items():
        if len(tool["sha256"]) != 64 or not tool["notices"]:
            raise ValueError(f"{name} needs a SHA-256 digest and its notices")
    return manifest


def install(directory, manifest=None, fetch=download_checked):
    """Put yt-dlp.exe, deno.exe and their upstream notices into `directory`.

    `fetch(url, destination, sha256, maximum)` must verify the digest and
    refuse to overwrite; tests replace it so no network is needed.
    """
    manifest = manifest or load()
    notices = directory / "windows-tools-notices"
    notices.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="windows-tools-") as work:
        for name, tool in manifest.items():
            download = Path(work) / name
            fetch(tool["url"], download, tool["sha256"], MAX_DOWNLOAD_BYTES)
            target = directory / EXECUTABLES[name]
            if "member" in tool:
                # Only the named member is read; never a path from the archive.
                with zipfile.ZipFile(download) as archive:
                    with archive.open(tool["member"]) as source, target.open("xb") as output:
                        shutil.copyfileobj(source, output)
            else:
                shutil.copy2(download, target)
            if target.stat().st_size < 1024 * 1024:
                raise ValueError(f"{target.name} is implausibly small")
            for filename, notice in tool["notices"].items():
                fetch(notice["url"], notices / filename, notice["sha256"], MAX_DOWNLOAD_BYTES)
    # The pins this ZIP was built from, for the review of the candidate.
    (directory / "windows-tools.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return [directory / name for name in EXECUTABLES.values()]


def github(path):
    request = urllib.request.Request(f"https://api.github.com/repos/{path}", headers={
        "Accept": "application/vnd.github+json", "User-Agent": "youtfast-release"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def digest_of(asset):
    algorithm, _, value = asset.get("digest", "").partition(":")
    if algorithm != "sha256" or len(value) != 64:
        raise ValueError(f"{asset['name']} has no SHA-256 digest on GitHub")
    return value


def latest(api=github):
    """The newest stable releases as a manifest, without the notices' digests.

    Release digests come from GitHub's release metadata; confirm them against
    upstream's own checksum files (yt-dlp's SHA2-256SUMS, Deno's .sha256sum)
    before committing.
    """
    ytdlp = api("yt-dlp/yt-dlp/releases/latest")
    tag = ytdlp["tag_name"]
    asset = next(item for item in ytdlp["assets"] if item["name"] == "yt-dlp.exe")
    raw = f"https://raw.githubusercontent.com/yt-dlp/yt-dlp/{tag}"
    manifest = {"yt-dlp": {
        "version": tag, "url": asset["browser_download_url"], "sha256": digest_of(asset),
        "notices": {"yt-dlp-LICENSE.txt": {"url": f"{raw}/LICENSE"},
                    "yt-dlp-THIRD_PARTY_LICENSES.txt": {"url": f"{raw}/THIRD_PARTY_LICENSES.txt"}},
    }}
    deno = api("denoland/deno/releases/latest")
    tag = deno["tag_name"]
    asset = next(item for item in deno["assets"] if item["name"] == DENO_ASSET)
    manifest["deno"] = {
        "version": tag.removeprefix("v"), "url": asset["browser_download_url"],
        "sha256": digest_of(asset), "member": "deno.exe",
        "notices": {"deno-LICENSE.md": {
            "url": f"https://raw.githubusercontent.com/denoland/deno/{tag}/LICENSE.md"}},
    }
    return manifest


def with_notice_digests(manifest, read=None):
    """Fill in each notice's digest by reading the (small) text file."""
    read = read or (lambda url: urllib.request.urlopen(url, timeout=60).read())
    for tool in manifest.values():
        for notice in tool["notices"].values():
            notice["sha256"] = hashlib.sha256(read(notice["url"])).hexdigest()
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("check", help="validate windows-tools.json and show the pins")
    commands.add_parser("latest", help="print a manifest for the newest upstream releases")
    args = parser.parse_args()
    if args.command == "check":
        for tool_name, pinned in load().items():
            print(tool_name, pinned["version"], pinned["sha256"])
    else:
        print(json.dumps(with_notice_digests(latest()), indent=2))
