import hashlib
import io
import json
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import native_sources
from release import ROOT


def write_source_package(destination, files):
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w") as archive:
        directory = tarfile.TarInfo("mingw-w64-example")
        directory.type = tarfile.DIRTYPE
        archive.addfile(directory)
        for name, data in files.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            archive.addfile(info, io.BytesIO(data))
    # The zstd tool rather than tarfile: CI's Python predates zstd support.
    destination.write_bytes(subprocess.run(["zstd", "-q", "-c"], input=buffer.getvalue(),
                                           stdout=subprocess.PIPE, check=True).stdout)


class NativeSourcesTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "target").mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="native-sources-test-", dir=ROOT / "target")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_msys_source_must_match_base_version_revision_and_epoch(self):
        text = "pkgbase = mingw-w64-mpv\npkgver = 0.41.0\npkgrel = 6\n"
        native_sources.validate_msys_source(text, "mingw-w64-mpv", "0.41.0-6")
        native_sources.validate_msys_source(text + "epoch = 0\n", "mingw-w64-mpv", "0.41.0-6")
        native_sources.validate_msys_source(text + "epoch = 1\n", "mingw-w64-mpv", "1:0.41.0-6")
        for base, version in [("another-package", "0.41.0-6"), ("mingw-w64-mpv", "0.41.0-7"),
                              ("mingw-w64-mpv", "1:0.41.0-6")]:
            with self.subTest(base=base, version=version), self.assertRaises(ValueError):
                native_sources.validate_msys_source(text, base, version)

    def test_homebrew_rejects_newer_recipe_instead_of_pairing_it_with_old_binaries(self):
        formula = {"versions": {"stable": "0.41.0"}, "revision": 6,
                   "installed": [{"version": "0.41.0_6"}]}
        self.assertEqual(native_sources.matching_brew_version(formula), "0.41.0_6")
        formula["revision"] = 7
        with self.assertRaisesRegex(ValueError, "changed since installation"):
            native_sources.matching_brew_version(formula)

    def test_download_rejects_credentials_plain_http_and_untrusted_hosts(self):
        credential_url = "https://user:" + "x" * 8 + "@repo.msys2.org/source"
        for url in ["http://repo.msys2.org/source", "https://example.com/source",
                    credential_url]:
            with self.subTest(url=url), self.assertRaises(ValueError):
                native_sources.download(url, self.root / "unused")

    def test_source_archive_contains_source_bytes_manifests_and_portable_internal_links(self):
        sources = self.root / "sources"
        sources.mkdir()
        (sources / "upstream.c").write_bytes(b"synthetic upstream source")
        (sources / "alias.c").symlink_to(sources / "upstream.c")
        target = self.root / "native-sources.tar.gz"
        native_sources.archive_sources(sources, target, [{"base": "synthetic", "version": "1-1"}])
        with tarfile.open(target) as archive:
            self.assertEqual(archive.extractfile("sources/upstream.c").read(), b"synthetic upstream source")
            manifest = json.load(archive.extractfile("sources/source-manifest.json"))
            entry = next(item for item in manifest["files"] if item["path"] == "upstream.c")
            self.assertEqual(entry["sha256"], hashlib.sha256(b"synthetic upstream source").hexdigest())
            self.assertEqual(archive.getmember("sources/alias.c").linkname, "upstream.c")

    def test_source_archive_rejects_a_link_to_the_maintainers_external_cache(self):
        sources = self.root / "sources"
        sources.mkdir()
        external = self.root / "outside"
        external.write_text("private cache placeholder")
        (sources / "external").symlink_to(external)
        with self.assertRaisesRegex(ValueError, "external symlink"):
            native_sources.archive_sources(sources, self.root / "unused.tar.gz", [])

    def test_windows_collects_each_source_base_once_and_validates_its_metadata(self):
        candidate = self.root / "candidate"
        candidate.mkdir()
        (candidate / "native-licenses").mkdir()
        (candidate / "native-licenses/LICENSE").write_text("upstream notice")
        packages = [{"name": name, "base": "mingw-w64-example", "version": "1.0-2"}
                    for name in ["example", "example-libs"]]
        (candidate / "native-dependencies.json").write_text(json.dumps({"packages": packages}))
        output = self.root / "sources"
        output.mkdir()

        def download(_url, destination):
            write_source_package(destination, {
                "mingw-w64-example/.SRCINFO": b"pkgbase = mingw-w64-example\npkgver = 1.0\npkgrel = 2\n",
                "mingw-w64-example/PKGBUILD": b"# synthetic recipe",
                "mingw-w64-example/source.tar.gz": b"synthetic upstream source",
            })
            return "f" * 64

        # The archive is read without invoking any external tar.
        with patch.object(native_sources, "download", side_effect=download) as fetch, \
                patch.object(native_sources, "run", side_effect=AssertionError):
            sources = native_sources.collect_windows(candidate, output)
        self.assertEqual(fetch.call_count, 1)
        self.assertEqual(sources[0]["base"], "mingw-w64-example")
        self.assertTrue((output / "notices/LICENSE").is_file())

    def test_msys_source_metadata_falls_back_to_zstd_without_python_support(self):
        archive = self.root / "example.src.tar.zst"
        write_source_package(archive, {"mingw-w64-example/.SRCINFO": b"pkgbase = mingw-w64-example\n"})
        real_open = tarfile.open

        def without_zstd(*args, **kwargs):
            if len(args) > 1 and args[1] == "r|zst":
                raise tarfile.CompressionError("unknown compression type 'zst'")
            return real_open(*args, **kwargs)

        with patch.object(native_sources.tarfile, "open", side_effect=without_zstd):
            count, srcinfo = native_sources.msys_source_metadata(archive, "mingw-w64-example")
        self.assertEqual(count, 2)
        self.assertEqual(srcinfo, "pkgbase = mingw-w64-example\n")

    def test_macos_collects_source_downloads_using_the_installed_recipe_and_a_private_cache(self):
        candidate = self.root / "candidate"
        licenses = candidate / "YouTFast.app/Contents/Resources/licenses"
        licenses.mkdir(parents=True)
        (licenses / "LICENSE").write_text("synthetic upstream notice")
        keg = self.root / "Cellar/example/1.0_2"
        (keg / ".brew").mkdir(parents=True)
        checksum = "a" * 64
        recipe = f'# synthetic formula; never executed by this test\nsha256 "{checksum}"\n'.encode()
        (keg / ".brew/example.rb").write_bytes(recipe)
        (keg / "INSTALL_RECEIPT.json").write_text("{}")
        # The current homebrew-core recipe digest may differ from the keg's.
        formula = {"name": "example", "versions": {"stable": "1.0"}, "revision": 2,
                   "installed": [{"version": "1.0_2"}],
                   "urls": {"stable": {"url": "https://example.invalid/example-1.0.tar.gz",
                                       "checksum": checksum}},
                   "ruby_source_checksum": {"sha256": "0" * 64}}
        (candidate / "native-dependencies.json").write_text(json.dumps({"formulae": [formula]}))
        output = self.root / "sources"
        output.mkdir()

        def run(*args, env=None):
            if args[:2] == ("brew", "--prefix"):
                return str(keg)
            self.assertEqual(args[:4], ("brew", "fetch", "--build-from-source", "--retry"))
            cache = Path(env["HOMEBREW_CACHE"])
            self.assertTrue(cache.is_relative_to(output.resolve()))
            cache.mkdir()
            (cache / "upstream-source.tar.gz").write_bytes(b"synthetic upstream source")
            return ""

        with patch.object(native_sources, "run", side_effect=run):
            result = native_sources.collect_macos(candidate, output)
        self.assertEqual(result[0]["version"], "1.0_2")
        self.assertEqual(result[0]["source_identity"], checksum)
        self.assertEqual(result[0]["recipe_sha256"], hashlib.sha256(recipe).hexdigest())
        self.assertEqual((output / "recipes/example.rb").read_bytes(), recipe)
        self.assertTrue((output / "notices/LICENSE").is_file())

        formula["urls"]["stable"]["checksum"] = "b" * 64
        (candidate / "native-dependencies.json").write_text(json.dumps({"formulae": [formula]}))
        shutil.rmtree(output)
        output.mkdir()
        with patch.object(native_sources, "run", side_effect=run), \
                self.assertRaisesRegex(ValueError, "example recipe does not declare"):
            native_sources.collect_macos(candidate, output)


if __name__ == "__main__":
    unittest.main()
