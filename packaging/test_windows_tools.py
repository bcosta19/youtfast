import io
import json
import tempfile
import unittest
import zipfile
from pathlib import Path

import windows_tools
from release import ROOT


def deno_zip(member="deno.exe", payload=b"d" * (2 * 1024 * 1024)):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as archive:
        archive.writestr(member, payload)
    return buffer.getvalue()


class WindowsToolsTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "target").mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="packaging-test-", dir=ROOT / "target")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def manifest(self):
        return {
            "yt-dlp": {"version": "1", "url": "https://example.test/yt-dlp.exe", "sha256": "a" * 64,
                       "notices": {"yt-dlp-LICENSE.txt": {"url": "https://example.test/l", "sha256": "b" * 64}}},
            "deno": {"version": "2", "url": "https://example.test/deno.zip", "sha256": "c" * 64,
                     "member": "deno.exe",
                     "notices": {"deno-LICENSE.md": {"url": "https://example.test/m", "sha256": "d" * 64}}},
        }

    def fetcher(self, calls, archive=None):
        def fetch(url, destination, sha256, maximum):
            calls.append((url, sha256))
            if url.endswith("yt-dlp.exe"):
                destination.write_bytes(b"y" * (2 * 1024 * 1024))
            elif url.endswith("deno.zip"):
                destination.write_bytes(archive or deno_zip())
            else:
                destination.write_text("licence")
        return fetch

    def test_the_committed_pins_are_complete_https_and_sha256(self):
        manifest = windows_tools.load()
        for tool in manifest.values():
            self.assertTrue(tool["url"].startswith("https://github.com/"))
            self.assertRegex(tool["sha256"], r"^[0-9a-f]{64}$")
            for notice in tool["notices"].values():
                self.assertRegex(notice["sha256"], r"^[0-9a-f]{64}$")
                self.assertTrue(notice["url"].startswith("https://"))
        # The version in the URL is the pinned version.
        self.assertIn(f"/{manifest['yt-dlp']['version']}/", manifest["yt-dlp"]["url"])
        self.assertIn(f"/v{manifest['deno']['version']}/", manifest["deno"]["url"])

    def test_install_places_both_executables_and_checks_every_download(self):
        calls = []
        directory = self.root / "package"
        directory.mkdir()
        windows_tools.install(directory, self.manifest(), self.fetcher(calls))
        self.assertEqual((directory / "yt-dlp.exe").stat().st_size, 2 * 1024 * 1024)
        self.assertEqual((directory / "deno.exe").stat().st_size, 2 * 1024 * 1024)
        self.assertTrue((directory / "windows-tools-notices/yt-dlp-LICENSE.txt").is_file())
        self.assertTrue((directory / "windows-tools-notices/deno-LICENSE.md").is_file())
        self.assertEqual(json.loads((directory / "windows-tools.json").read_text()), self.manifest())
        # Each download carries the digest the manifest pins.
        self.assertEqual(sorted(sha for _, sha in calls), ["a" * 64, "b" * 64, "c" * 64, "d" * 64])

    def test_a_failed_digest_check_stops_the_package(self):
        def fetch(url, destination, sha256, maximum):
            raise ValueError("Upstream source digest mismatch")
        directory = self.root / "package"
        directory.mkdir()
        with self.assertRaisesRegex(ValueError, "mismatch"):
            windows_tools.install(directory, self.manifest(), fetch)
        self.assertFalse((directory / "yt-dlp.exe").exists())

    def test_deno_archive_without_its_executable_is_refused(self):
        directory = self.root / "package"
        directory.mkdir()
        with self.assertRaises(KeyError):
            windows_tools.install(directory, self.manifest(),
                                  self.fetcher([], deno_zip(member="other.exe")))

    def test_tiny_executables_are_refused(self):
        directory = self.root / "package"
        directory.mkdir()
        with self.assertRaisesRegex(ValueError, "implausibly small"):
            windows_tools.install(directory, self.manifest(),
                                  self.fetcher([], deno_zip(payload=b"x")))

    def test_latest_builds_a_manifest_from_github_release_metadata(self):
        def api(path):
            if path.startswith("yt-dlp"):
                return {"tag_name": "2030.01.01", "assets": [
                    {"name": "yt-dlp", "digest": "sha256:" + "0" * 64},
                    {"name": "yt-dlp.exe", "digest": "sha256:" + "e" * 64,
                     "browser_download_url": "https://github.com/yt-dlp/yt-dlp/releases/download/2030.01.01/yt-dlp.exe"}]}
            return {"tag_name": "v3.0.0", "assets": [
                {"name": windows_tools.DENO_ASSET, "digest": "sha256:" + "f" * 64,
                 "browser_download_url": "https://github.com/denoland/deno/releases/download/v3.0.0/" + windows_tools.DENO_ASSET}]}
        manifest = windows_tools.latest(api)
        self.assertEqual(manifest["yt-dlp"]["version"], "2030.01.01")
        self.assertEqual(manifest["yt-dlp"]["sha256"], "e" * 64)
        self.assertEqual(manifest["deno"]["version"], "3.0.0")
        self.assertIn("/2030.01.01/LICENSE", manifest["yt-dlp"]["notices"]["yt-dlp-LICENSE.txt"]["url"])
        filled = windows_tools.with_notice_digests(manifest, lambda url: b"text")
        self.assertEqual(len(filled["deno"]["notices"]["deno-LICENSE.md"]["sha256"]), 64)

    def test_latest_refuses_assets_without_a_published_digest(self):
        def api(path):
            return {"tag_name": "1", "assets": [{"name": "yt-dlp.exe", "browser_download_url": "u"}]}
        with self.assertRaisesRegex(ValueError, "no SHA-256 digest"):
            windows_tools.latest(api)


if __name__ == "__main__":
    unittest.main()
