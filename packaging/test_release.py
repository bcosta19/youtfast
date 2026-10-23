import importlib.util
import json
import re
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

spec = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
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
        for name in ["src/storage/account.rs", "vendor/rustypipe/LICENSE", "packaging/test_release.py"]:
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


if __name__ == "__main__":
    unittest.main()
