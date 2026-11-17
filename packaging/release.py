import argparse
import datetime
import re
import shutil
import subprocess
import tarfile
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parent.parent
APP_ID = "io.github.bcosta19.YouTFast"
REPO_URL = "https://github.com/bcosta19/youtfast"
SEMVER = r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
PUBLIC_FILES = [
    "README.md", "CHANGELOG.md", "ROADMAP.md", "CONTRIBUTING.md", "RELEASING.md",
    "SECURITY.md", "CODE_OF_CONDUCT.md", "AGENTS.md", "LICENSE", "THIRD_PARTY_NOTICES.md",
    "docs/actions.md", "docs/distribution.md", "docs/install.md", "docs/user-guide.md",
    "docs/ARCHITECTURE.md",
    "packaging/arch/README.md", "assets/icons/LICENSE.txt",
    "assets/fonts/NotoEmoji-LICENSE.txt", "assets/fonts/NotoFallback-LICENSE.txt",
    "assets/fonts/OpenMoji-LICENSE.txt",
    "packaging/icons/youtfast.svg", "assets/screenshots/search.png",
]


def workspace_package(root=ROOT):
    """The workspace's shared package fields (version, license, rust-version)."""
    return tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]


def version(root=ROOT):
    return workspace_package(root)["version"]


def validate_tag(tag, package_version):
    if not re.fullmatch(rf"v{SEMVER}", tag):
        raise ValueError("Release tag must be vMAJOR.MINOR.PATCH")
    if tag != f"v{package_version}":
        raise ValueError("Tag does not match Cargo.toml")


def forbidden_path(name):
    parts = Path(name).parts
    return any(part in {"rustypipe_reports", "target", ".env", "cookies.txt"} or part.startswith(".env.") for part in parts)


def validate_changelog(text, current, metadata_date, require_date=False):
    if text.count("## [Unreleased]") != 1:
        raise ValueError("Changelog requires one Unreleased section")
    entries = re.findall(rf"^## \[{re.escape(current)}\] - (.+)$", text, re.MULTILINE)
    if len(entries) != 1:
        raise ValueError("Changelog requires one entry for the release version")
    date = entries[0]
    if date == "Unreleased" and not require_date:
        return
    try:
        parsed = datetime.date.fromisoformat(date)
    except ValueError as error:
        raise ValueError("Freeze the changelog publication date before a tagged release or dry run") from error
    if parsed.isoformat() != date or date != metadata_date:
        raise ValueError("Changelog and AppStream release dates must agree")


def check(root=ROOT, tag=None, candidate=False):
    current = version(root)
    validate_tag(tag or f"v{current}", current)
    lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    package = next(item for item in lock["package"] if item["name"] == "youtfast")
    if package["version"] != current:
        raise ValueError("Cargo.lock version does not match Cargo.toml")
    metadata = ET.parse(root / f"packaging/flatpak/{APP_ID}.metainfo.xml").getroot()
    if metadata.findtext("id") != APP_ID:
        raise ValueError("Desktop application ID mismatch")
    desktop_release = metadata.find("releases/release")
    if desktop_release.get("version") != current:
        raise ValueError("Desktop release version does not match Cargo.toml")
    validate_changelog((root / "CHANGELOG.md").read_text(encoding="utf-8"), current,
                       desktop_release.get("date"), require_date=tag is not None and not candidate)
    notes_path = root / f"packaging/release-notes/v{current}.md"
    if not notes_path.is_file():
        raise ValueError("Committed release notes are required")
    notes = notes_path.read_text(encoding="utf-8")
    for section in ["## Included", "## Not included", "## Known limitations"]:
        if section not in notes:
            raise ValueError("Release notes must describe included features, exclusions and limitations")
    if workspace_package(root)["license"] != "GPL-3.0-only":
        raise ValueError("Project license must match release documentation")
    if not (root / "LICENSE").is_file():
        raise ValueError("LICENSE is required")
    names = subprocess.check_output(["git", "ls-files", "-z"], cwd=root).decode().split("\0")
    if any(forbidden_path(name) for name in names if name):
        raise ValueError("Generated reports, credentials or build output are tracked")
    return current


def copy_public_files(directory):
    for item in PUBLIC_FILES:
        output = directory / item
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / item, output)


def linux():
    current = check()
    name = f"youtfast-v{current}-x86_64-unknown-linux-gnu"
    dist = ROOT / "dist"
    directory = dist / name
    directory.mkdir(parents=True, exist_ok=False)
    shutil.copy2(ROOT / "target/release/youtfast", directory / "youtfast")
    copy_public_files(directory)
    for item in ["packaging/portable.txt", "packaging/applications/youtfast.desktop", "packaging/icons/youtfast.svg", "contrib/omarchy/youtfast-theme"]:
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


def version_key(value):
    if not re.fullmatch(SEMVER, value):
        raise ValueError("Version must be MAJOR.MINOR.PATCH")
    return tuple(int(part) for part in value.split("."))


def validate_date(value):
    if datetime.date.fromisoformat(value).isoformat() != value:
        raise ValueError("Release date must be YYYY-MM-DD")
    return value


def bump_manifest(text, old, new):
    """Cargo.toml: the version in [workspace.package], which every crate inherits."""
    section = re.search(r"^\[workspace\.package\]\n(?:(?!\[).*\n)*", text, re.MULTILINE)
    if section is None:
        raise ValueError("Cargo.toml has no [workspace.package] section")
    body, count = re.subn(rf'^version = "{re.escape(old)}"$', f'version = "{new}"', section.group(0), flags=re.MULTILINE)
    if count != 1:
        raise ValueError("Cargo.toml's workspace version does not match the current version")
    return text[:section.start()] + body + text[section.end():]


def bump_lock(text, old, new):
    """Cargo.lock: the workspace's own crates, which have no `source`."""
    members = [item["name"] for item in tomllib.loads(text)["package"]
               if item["name"].startswith("youtfast") and "source" not in item]
    for name in members:
        text, count = re.subn(rf'^(name = "{re.escape(name)}"\nversion = )"{re.escape(old)}"$',
                              rf'\1"{new}"', text, flags=re.MULTILINE)
        if count != 1:
            raise ValueError(f"Cargo.lock entry for {name} does not match the current version")
    return text


def add_appstream_release(text, new, date, summary):
    entry = (f'    <release version="{new}" date="{date}">\n'
             f"      <description><p>{escape(summary)}</p></description>\n"
             "    </release>\n")
    if text.count("  <releases>\n") != 1:
        raise ValueError("AppStream metadata needs one <releases> list")
    return text.replace("  <releases>\n", "  <releases>\n" + entry, 1)


def release_changelog(text, old, new, date):
    """Move Unreleased into a dated section; return the new text and the moved entries."""
    head, marker, rest = text.partition("## [Unreleased]\n")
    if not marker:
        raise ValueError("Changelog requires one Unreleased section")
    following = re.search(r"^## \[", rest, re.MULTILINE)
    if following is None:
        raise ValueError("Changelog has no released section after Unreleased")
    entries = rest[:following.start()].strip()
    if not entries:
        raise ValueError("Nothing under Unreleased to release")
    link = f"[Unreleased]: {REPO_URL}/compare/v{old}...HEAD\n"
    if link not in text:
        raise ValueError("Changelog's Unreleased link does not compare from the current version")
    text = (f"{head}## [Unreleased]\n\n## [{new}] - {date}\n\n{entries}\n\n"
            + rest[following.start():])
    text = text.replace(link, f"[Unreleased]: {REPO_URL}/compare/v{new}...HEAD\n"
                              f"[{new}]: {REPO_URL}/compare/v{old}...v{new}\n")
    return text, entries


def render_release_notes(previous, entries, old, new, summary):
    """Included comes from the changelog; the remaining sections carry over for review."""
    included = "\n".join(line for line in entries.splitlines()
                         if line.strip() and not line.startswith("### "))
    after_included = previous.partition("\n## Included\n")[2]
    following = re.search(r"^## ", after_included, re.MULTILINE)
    if following is None:
        raise ValueError("Previous release notes have no sections after Included")
    carried = after_included[following.start():].partition("\n[Changes since")[0].rstrip()
    for section in ["## Not included", "## Known limitations"]:
        if section not in carried:
            raise ValueError(f"Previous release notes lack {section}")
    return (f"# YouTFast {new}\n\n{summary}\n\n## Included\n\n{included}\n\n{carried}\n\n"
            f"[Changes since {old}]({REPO_URL}/compare/v{old}...v{new})\n"
            f"· [Distribution guide]({REPO_URL}/blob/v{new}/docs/distribution.md)\n")


def prepare(new, summary, date, root=ROOT):
    """Bump every versioned file to `new` and draft its release notes."""
    old = version(root)
    if version_key(new) <= version_key(old):
        raise ValueError(f"{new} is not newer than {old}")
    validate_date(date)
    if not summary.strip():
        raise ValueError("A one-sentence summary is required")
    notes = root / f"packaging/release-notes/v{new}.md"
    if notes.exists():
        raise ValueError(f"{notes.relative_to(root)} already exists")
    files = {name: (root / name).read_text(encoding="utf-8")
             for name in ["Cargo.toml", "Cargo.lock", "CHANGELOG.md", f"packaging/flatpak/{APP_ID}.metainfo.xml"]}
    changelog, entries = release_changelog(files["CHANGELOG.md"], old, new, date)
    previous = (root / f"packaging/release-notes/v{old}.md").read_text(encoding="utf-8")
    updated = {
        "Cargo.toml": bump_manifest(files["Cargo.toml"], old, new),
        "Cargo.lock": bump_lock(files["Cargo.lock"], old, new),
        "CHANGELOG.md": changelog,
        f"packaging/flatpak/{APP_ID}.metainfo.xml": add_appstream_release(
            files[f"packaging/flatpak/{APP_ID}.metainfo.xml"], new, date, summary.strip()),
        notes.relative_to(root).as_posix(): render_release_notes(previous, entries, old, new, summary.strip()),
    }
    # Everything is computed before the first write: a failure leaves the tree as it was.
    for name, text in updated.items():
        (root / name).write_text(text, encoding="utf-8")
    return old


def freeze(date, root=ROOT):
    """Set the current version's publication date in the changelog and AppStream."""
    current = version(root)
    validate_date(date)
    changelog = root / "CHANGELOG.md"
    text, count = re.subn(rf"^## \[{re.escape(current)}\] - .+$", f"## [{current}] - {date}",
                          changelog.read_text(encoding="utf-8"), flags=re.MULTILINE)
    if count != 1:
        raise ValueError("Changelog requires one entry for the release version")
    metadata = root / f"packaging/flatpak/{APP_ID}.metainfo.xml"
    xml, count = re.subn(rf'<release version="{re.escape(current)}" date="[^"]*"',
                         f'<release version="{current}" date="{date}"', metadata.read_text(encoding="utf-8"))
    if count != 1:
        raise ValueError("AppStream metadata requires one release for the current version")
    changelog.write_text(text, encoding="utf-8")
    metadata.write_text(xml, encoding="utf-8")
    return current


def publication_notes(checksums, root=ROOT):
    """The committed notes plus install commands and the assets' SHA-256 digests."""
    current = version(root)
    notes = (root / f"packaging/release-notes/v{current}.md").read_text(encoding="utf-8").rstrip() + "\n"
    digests = checksums.read_text(encoding="utf-8").strip()
    if not digests:
        raise ValueError("checksums.txt is empty")
    install = (
        "## Install\n\n"
        "- **Linux** (Arch package or Flatpak, SHA-256 verified): "
        "`curl -fsSL https://youtfast.pages.dev/install.sh | sh`\n"
        "- **Windows** (PowerShell): `irm https://youtfast.pages.dev/install.ps1 | iex`, "
        "or the `x86_64-windows-setup.exe` below\n"
        "- **macOS** (Apple Silicon): `brew install --cask bcosta19/tap/youtfast`\n"
        f"- Every system, step by step: [installation guide]({REPO_URL}/blob/v{current}/docs/install.md)\n\n"
        "## SHA-256 checksums\n\n"
        f"```text\n{digests}\n```\n\n"
        "Check downloads with `sha256sum --check --ignore-missing checksums.txt`. "
        "Checksums detect corruption; they are not publisher signatures.\n"
    )
    body, links, footer = notes.partition("\n[Changes since")
    return f"{body.rstrip()}\n\n{install}" + (f"\n[Changes since{footer}" if links else "")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    check_command = commands.add_parser("check", help="Validate version, metadata, changelog and notes")
    check_command.add_argument("--tag")
    check_command.add_argument("--candidate", action="store_true", help="Allow a pending publication date for non-publishing dry runs")
    commands.add_parser("linux", help="Package target/release/youtfast")
    commands.add_parser("source", help="Archive the source with locked dependencies")
    prepare_command = commands.add_parser("prepare", help="Bump to a new version and draft its notes")
    prepare_command.add_argument("version")
    prepare_command.add_argument("--summary", required=True, help="One sentence for AppStream and the notes")
    prepare_command.add_argument("--date", default=datetime.date.today().isoformat())
    freeze_command = commands.add_parser("freeze", help="Set the current version's publication date")
    freeze_command.add_argument("--date", default=datetime.date.today().isoformat())
    notes_command = commands.add_parser("notes", help="Print the draft's notes with install steps and checksums")
    notes_command.add_argument("--checksums", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "check":
        print(check(tag=args.tag, candidate=args.candidate))
    elif args.command == "linux":
        linux()
    elif args.command == "source":
        source()
    elif args.command == "prepare":
        old = prepare(args.version, args.summary, args.date)
        print(f"{old} -> {args.version}: review packaging/release-notes/v{args.version}.md, then run check")
    elif args.command == "freeze":
        print(f"{freeze(args.date)} dated {args.date}")
    else:
        print(publication_notes(args.checksums), end="")
