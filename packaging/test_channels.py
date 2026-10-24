import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

PACKAGING = Path(__file__).resolve().parent
if str(PACKAGING) not in sys.path:
    sys.path.insert(0, str(PACKAGING))

spec = importlib.util.spec_from_file_location("channels", PACKAGING / "channels.py")
channels = importlib.util.module_from_spec(spec)
spec.loader.exec_module(channels)


class ChannelsTests(unittest.TestCase):
    def test_flathub_manifest_builds_from_locked_offline_source_archive(self):
        digest = "a" * 64
        manifest = channels.render_flathub("0.2.0", digest)
        self.assertEqual(
            manifest["sdk-extensions"],
            ["org.freedesktop.Sdk.Extension.rust-stable"],
        )
        youtfast = next(m for m in manifest["modules"] if m["name"] == "youtfast")
        self.assertEqual(
            youtfast["build-commands"][0],
            "cargo build --release --locked --offline",
        )
        self.assertEqual(
            youtfast["sources"],
            [
                {
                    "type": "archive",
                    "url": "https://github.com/bcosta19/youtfast/releases/download/v0.2.0/youtfast-v0.2.0-source-with-dependencies.tar.gz",
                    "sha256": digest,
                }
            ],
        )

    def test_winget_manifest_declares_portable_zip_and_dependencies(self):
        digest = "b" * 64
        manifest = channels.render_winget("0.2.0", digest)
        self.assertIn("PackageIdentifier: bcosta19.YouTFast", manifest)
        self.assertIn("PackageVersion: 0.2.0", manifest)
        self.assertIn("PackageIdentifier: yt-dlp.yt-dlp", manifest)
        self.assertIn("PackageIdentifier: DenoLand.Deno", manifest)
        self.assertIn(
            "RelativeFilePath: youtfast-v0.2.0-x86_64-pc-windows-gnu\\youtfast.exe",
            manifest,
        )
        self.assertIn(f"InstallerSha256: {digest.upper()}", manifest)

    def test_homebrew_cask_declares_dependencies_app_and_binary(self):
        digest = "C" * 64
        cask = channels.render_homebrew_cask("0.2.0", digest)
        self.assertIn('cask "youtfast" do', cask)
        self.assertIn('version "0.2.0"', cask)
        self.assertIn(f'sha256 "{digest.lower()}"', cask)
        self.assertIn("depends_on macos: :sequoia", cask)
        self.assertIn('depends_on formula: "deno"', cask)
        self.assertIn('depends_on formula: "yt-dlp"', cask)
        self.assertIn('app "youtfast-v#{version}-aarch64-apple-darwin/YouTFast.app"', cask)
        self.assertIn('binary "#{appdir}/YouTFast.app/Contents/MacOS/youtfast"', cask)

    def test_build_channels_writes_all_three_manifests(self):
        current = channels.check()
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp)
            for name in [
                f"youtfast-v{current}-source-with-dependencies.tar.gz",
                f"youtfast-v{current}-x86_64-pc-windows-gnu.zip",
                f"youtfast-v{current}-aarch64-apple-darwin.zip",
            ]:
                (dist / name).write_bytes(name.encode())
            out = channels.build_channels(dist, dist / "channels")
            self.assertTrue((out / f"flathub/{channels.APP_ID}.json").is_file())
            self.assertTrue((out / "winget/bcosta19.YouTFast.yaml").is_file())
            self.assertTrue((out / "homebrew/Casks/youtfast.rb").is_file())


if __name__ == "__main__":
    unittest.main()
