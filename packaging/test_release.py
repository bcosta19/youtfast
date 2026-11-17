import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tomllib
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

spec = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def test_release_check_reads_utf8_with_non_utf8_default_encoding(self):
        env = os.environ.copy()
        env.update(LC_ALL="C", PYTHONCOERCECLOCALE="0", PYTHONUTF8="0")
        result = subprocess.run(
            [sys.executable, str(release.ROOT / "packaging/release.py"), "check"],
            cwd=release.ROOT, env=env, capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode("ascii", errors="replace"))

    def test_draft_changelog_requires_a_date_only_when_freezing_a_tag(self):
        draft = "## [Unreleased]\n\n## [0.1.0] - Unreleased\n"
        release.validate_changelog(draft, "0.1.0", "2026-10-17")
        with self.assertRaisesRegex(ValueError, "Freeze"):
            release.validate_changelog(draft, "0.1.0", "2026-10-17", require_date=True)
        frozen = draft.replace("- Unreleased", "- 2026-10-17")
        release.validate_changelog(frozen, "0.1.0", "2026-10-17", require_date=True)
        with self.assertRaisesRegex(ValueError, "dates must agree"):
            release.validate_changelog(frozen, "0.1.0", "2026-10-11", require_date=True)

    def test_changelog_rejects_missing_duplicate_and_invalid_version_dates(self):
        for text in ["", "## [Unreleased]\n", "## [Unreleased]\n## [0.1.0] - invalid",
                     "## [Unreleased]\n## [0.1.0] - 2026-02-30",
                     "## [Unreleased]\n## [0.1.0] - Unreleased\n## [0.1.0] - Unreleased"]:
            with self.subTest(text=text), self.assertRaises(ValueError):
                release.validate_changelog(text, "0.1.0", "2026-10-17")

    def test_release_tag_requires_exact_stable_version(self):
        release.validate_tag("v0.1.0", "0.1.0")
        for tag in ["0.1.0", "v0.2.0", "v0.1.0/other", "v0.1.0; echo bad", "v0.1.0-rc.1", "v00.1.0"]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.validate_tag(tag, "0.1.0")

    def test_sensitive_and_generated_paths_are_rejected(self):
        for name in ["rustypipe_reports/raw.json", "target/out", ".env", ".env.local", "data/cookies.txt"]:
            self.assertTrue(release.forbidden_path(name))
        for name in ["src/storage/account.rs", "assets/icons/LICENSE.txt", "packaging/test_release.py"]:
            self.assertFalse(release.forbidden_path(name))

    def test_workflows_pin_actions_and_keep_contributor_tokens_read_only(self):
        for path in (release.ROOT / ".github/workflows").glob("*.yml"):
            text = path.read_text()
            self.assertNotIn("pull_request_target:", text)
            self.assertNotIn("secrets: inherit", text)
            self.assertIn("contents: read", text.partition("jobs:")[0])
            for action in re.findall(r"uses:\s+(\S+)", text):
                if not action.startswith("./"):
                    self.assertRegex(action, r"^[\w/-]+@[0-9a-f]{40}$")
        updates = (release.ROOT / ".github/dependabot.yml").read_text()
        self.assertEqual(updates.count("default-days: 7"), 2)

    def test_desktop_release_matches_application_version(self):
        root = ET.parse(release.ROOT / f"packaging/flatpak/{release.APP_ID}.metainfo.xml").getroot()
        self.assertEqual(root.findtext("id"), release.APP_ID)
        self.assertEqual(root.findtext("project_license"), "GPL-3.0-only")
        self.assertEqual(root.find("releases/release").get("version"), release.version())

    def test_flatpak_meson_libraries_use_the_runtime_library_directory(self):
        manifest = json.loads((release.ROOT / f"packaging/flatpak/{release.APP_ID}.json").read_text())
        for module in manifest["modules"]:
            if module.get("buildsystem") == "meson":
                with self.subTest(module=module["name"]):
                    self.assertIn("--libdir=lib", module["config-opts"])

    def test_flatpak_meson_notices_are_copied_from_the_source_directory(self):
        manifest = json.loads((release.ROOT / f"packaging/flatpak/{release.APP_ID}.json").read_text())
        modules = {module["name"]: module for module in manifest["modules"]}
        for name, directory, files in [
            ("libplacebo", "libplacebo", ["LICENSE"]),
            ("libmpv", "mpv", ["Copyright", "LICENSE.GPL", "LICENSE.LGPL"]),
        ]:
            for filename in files:
                with self.subTest(module=name, filename=filename):
                    self.assertIn(
                        f"install -Dm644 ../{filename} /app/share/licenses/{directory}/{filename}",
                        modules[name]["post-install"],
                    )

    def test_flatpak_uses_bounded_permissions_and_pinned_sources(self):
        manifest = json.loads((release.ROOT / f"packaging/flatpak/{release.APP_ID}.json").read_text())
        self.assertEqual(manifest["app-id"], release.APP_ID)
        self.assertNotIn("--filesystem=host", manifest["finish-args"])
        self.assertNotIn("--filesystem=home", manifest["finish-args"])
        self.assertNotIn("--device=all", manifest["finish-args"])
        for module in manifest["modules"]:
            for source in module["sources"]:
                if source["type"] in {"archive", "file"} and "url" in source:
                    self.assertEqual(len(source["sha256"]), 64)
                if source["type"] == "git":
                    self.assertEqual(len(source["commit"]), 40)


CHANGELOG = """# Changelog

## [Unreleased]

### Fixed

- Fixed playback.

## [0.1.0] - 2026-10-17

### Added

- First.

[Unreleased]: https://github.com/bcosta19/youtfast/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/bcosta19/youtfast/releases/tag/v0.1.0
"""

NOTES = """# YouTFast 0.1.0

First.

## Included

- First.

## Not included

- Signing.

## Known limitations

- Young.

[Changes since 0.0.9](https://github.com/bcosta19/youtfast/compare/v0.0.9...v0.1.0)
"""


class PrepareTests(unittest.TestCase):
    def fixture(self):
        root = release.ROOT / "target" / f"test-release-prepare-{os.getpid()}-{self._testMethodName}"
        shutil.rmtree(root, ignore_errors=True)
        self.addCleanup(shutil.rmtree, root, ignore_errors=True)
        (root / "packaging/flatpak").mkdir(parents=True)
        (root / "packaging/release-notes").mkdir(parents=True)
        (root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.0"\n\n[workspace.dependencies]\nfoo = { version = "0.1.0" }\n')
        (root / "Cargo.lock").write_text(
            'version = 4\n\n[[package]]\nname = "other"\nversion = "0.1.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n\n'
            '[[package]]\nname = "youtfast"\nversion = "0.1.0"\n\n[[package]]\nname = "youtfast-core"\nversion = "0.1.0"\n')
        (root / "CHANGELOG.md").write_text(CHANGELOG)
        (root / f"packaging/flatpak/{release.APP_ID}.metainfo.xml").write_text(
            '<component>\n  <releases>\n    <release version="0.1.0" date="2026-10-17"/>\n  </releases>\n</component>\n')
        (root / "packaging/release-notes/v0.1.0.md").write_text(NOTES)
        return root

    def test_prepare_bumps_every_versioned_file_and_drafts_notes_from_the_changelog(self):
        root = self.fixture()
        release.prepare("0.2.0", "Fixes playback.", "2026-11-01", root=root)
        self.assertIn('version = "0.2.0"', (root / "Cargo.toml").read_text())
        self.assertIn('foo = { version = "0.1.0" }', (root / "Cargo.toml").read_text())
        lock = tomllib.loads((root / "Cargo.lock").read_text())
        self.assertEqual({item["name"]: item["version"] for item in lock["package"]},
                         {"other": "0.1.0", "youtfast": "0.2.0", "youtfast-core": "0.2.0"})
        metadata = ET.parse(root / f"packaging/flatpak/{release.APP_ID}.metainfo.xml").getroot()
        self.assertEqual(metadata.find("releases/release").attrib, {"version": "0.2.0", "date": "2026-11-01"})
        changelog = (root / "CHANGELOG.md").read_text()
        release.validate_changelog(changelog, "0.2.0", "2026-11-01", require_date=True)
        self.assertIn("## [Unreleased]\n\n## [0.2.0] - 2026-11-01\n\n### Fixed\n\n- Fixed playback.\n\n## [0.1.0]", changelog)
        self.assertIn("[Unreleased]: https://github.com/bcosta19/youtfast/compare/v0.2.0...HEAD\n"
                      "[0.2.0]: https://github.com/bcosta19/youtfast/compare/v0.1.0...v0.2.0\n", changelog)
        notes = (root / "packaging/release-notes/v0.2.0.md").read_text()
        self.assertIn("## Included\n\n- Fixed playback.\n\n## Not included\n\n- Signing.", notes)
        self.assertIn("compare/v0.1.0...v0.2.0", notes)
        self.assertNotIn("v0.0.9", notes)

    def test_prepare_refuses_without_writing_anything(self):
        root = self.fixture()
        (root / "CHANGELOG.md").write_text(CHANGELOG.replace("### Fixed\n\n- Fixed playback.\n\n", ""))
        before = {path: path.read_bytes() for path in root.rglob("*") if path.is_file()}
        for new in ["0.2.0", "0.1.0", "0.0.9", "v0.2.0"]:
            with self.assertRaises(ValueError):
                release.prepare(new, "Summary.", "2026-11-01", root=root)
        self.assertEqual({path: path.read_bytes() for path in root.rglob("*") if path.is_file()}, before)

    def test_freeze_moves_the_changelog_and_appstream_dates_together(self):
        root = self.fixture()
        release.freeze("2026-11-02", root=root)
        metadata = ET.parse(root / f"packaging/flatpak/{release.APP_ID}.metainfo.xml").getroot()
        release.validate_changelog((root / "CHANGELOG.md").read_text(), "0.1.0",
                                   metadata.find("releases/release").get("date"), require_date=True)

    def test_publication_notes_add_install_steps_and_digests_before_the_links(self):
        root = self.fixture()
        (root / "checksums.txt").write_text(f"{'a' * 64}  youtfast-v0.1.0-x86_64.flatpak\n")
        notes = release.publication_notes(root / "checksums.txt", root=root)
        self.assertIn("## Install\n", notes)
        self.assertIn(f"{'a' * 64}  youtfast-v0.1.0-x86_64.flatpak", notes)
        self.assertLess(notes.index("## SHA-256 checksums"), notes.index("[Changes since 0.0.9]"))
        (root / "checksums.txt").write_text("\n")
        with self.assertRaises(ValueError):
            release.publication_notes(root / "checksums.txt", root=root)


if __name__ == "__main__":
    unittest.main()
