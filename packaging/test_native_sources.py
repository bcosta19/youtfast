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
        native_sources.validate_msys_source(text + "epoch = 1\n", "mingw-w64-mpv", "1~0.41.0-6")
        for base, version in [("another-package", "0.41.0-6"), ("mingw-w64-mpv", "0.41.0-7"),
                              ("mingw-w64-mpv", "1:0.41.0-6"), ("mingw-w64-mpv", "1~0.41.0-6")]:
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
            if args[:2] == ("brew", "cat"):
                self.assertEqual(args[2:], ("example",))
                return "# current homebrew-core recipe\n"
            self.assertEqual(args, ("brew", "fetch", "--build-from-source", "--retry", "example"))
            cache = Path(env["HOMEBREW_CACHE"])
            self.assertTrue(cache.is_relative_to(output.resolve()))
            cache.mkdir()
            (cache / "upstream-source.tar.gz").write_bytes(b"synthetic upstream source")
            return ""

        with patch.object(native_sources, "run", side_effect=run):
            result = native_sources.collect_macos(candidate, output)
        self.assertEqual((output / "recipes/example.current.rb").read_text(),
                         "# current homebrew-core recipe\n")
        self.assertEqual(result[0]["version"], "1.0_2")
        self.assertTrue(result[0]["installed_recipe_declares_current_source"])
        self.assertEqual(result[0]["recipe_sha256"], hashlib.sha256(recipe).hexdigest())
        self.assertEqual((output / "recipes/example.rb").read_bytes(), recipe)
        self.assertTrue((output / "notices/LICENSE").is_file())

        # A newer homebrew-core entry is recorded, not mistaken for the keg's.
        formula["urls"]["stable"]["checksum"] = "b" * 64
        (candidate / "native-dependencies.json").write_text(json.dumps({"formulae": [formula]}))
        shutil.rmtree(output)
        output.mkdir()
        with patch.object(native_sources, "run", side_effect=run):
            result = native_sources.collect_macos(candidate, output)
        self.assertFalse(result[0]["installed_recipe_declares_current_source"])
        self.assertEqual((output / "recipes/example.rb").read_bytes(), recipe)


    def test_only_recipes_without_patches_or_build_resources_are_plain(self):
        dav1d = b"""class Dav1d < Formula
  url "https://code.videolan.org/videolan/dav1d/-/archive/1.5.4/dav1d-1.5.4.tar.bz2"
  sha256 "2abfb0c89212e6e4733a54e0ae509ec00a5b845a6360946f918806e14aedb011"
  resource "homebrew-00000000.ivf", :test do
    url "https://code.videolan.org/videolan/dav1d-test-data/raw/1.1.0/8-bit/data/00000000.ivf"
  end
end
"""
        self.assertTrue(native_sources.recipe_is_plain(dav1d))
        self.assertFalse(native_sources.recipe_is_plain(dav1d + b'  resource "lib" do\n  end\n'))
        self.assertFalse(native_sources.recipe_is_plain(dav1d + b"  patch do\n  end\n"))
        self.assertFalse(native_sources.recipe_is_plain(dav1d + b"  patch :DATA\n"))

    def test_macos_falls_back_to_the_declared_tarball_only_for_plain_recipes(self):
        candidate = self.root / "candidate"
        licenses = candidate / "YouTFast.app/Contents/Resources/licenses"
        licenses.mkdir(parents=True)
        (licenses / "LICENSE").write_text("synthetic upstream notice")
        checksum = "c" * 64
        kegs = {}
        formulae = []
        for name, extra in [("plain", b""), ("patched", b"  patch do\n  end\n")]:
            keg = self.root / f"Cellar/{name}/1.0"
            (keg / ".brew").mkdir(parents=True)
            (keg / f".brew/{name}.rb").write_bytes(f'sha256 "{checksum}"\n'.encode() + extra)
            (keg / "INSTALL_RECEIPT.json").write_text("{}")
            kegs[name] = keg
            formulae.append({"name": name, "versions": {"stable": "1.0"}, "installed": [{"version": "1.0"}],
                             "urls": {"stable": {"url": f"https://example.invalid/{name}-1.0.tar.gz",
                                                 "checksum": checksum}}})
        downloaded = []

        def run(*args, env=None):
            if args[:2] == ("brew", "--prefix"):
                return str(kegs[args[2]])
            if args[:2] == ("brew", "cat"):
                return ""
            if args[-1] in failing or len(args) > 5:
                raise ValueError("Native source command failed: brew: synthetic Homebrew error")
            Path(env["HOMEBREW_CACHE"]).mkdir(exist_ok=True)
            (Path(env["HOMEBREW_CACHE"]) / f"{args[-1]}.tar.gz").write_bytes(b"source")
            return ""

        def fetch(url, destination, sha256):
            self.assertEqual(sha256, checksum)
            destination.write_bytes(b"upstream source")
            downloaded.append(url)

        for failing, expect_error in [({"plain"}, False), ({"patched"}, True)]:
            (candidate / "native-dependencies.json").write_text(json.dumps({"formulae": formulae}))
            output = self.root / "sources"
            shutil.rmtree(output, ignore_errors=True)
            output.mkdir()
            with self.subTest(failing=failing), \
                    patch.object(native_sources, "run", side_effect=run), \
                    patch.object(native_sources, "download_checked", side_effect=fetch):
                if expect_error:
                    with self.assertRaisesRegex(ValueError, "could not fetch patched"):
                        native_sources.collect_macos(candidate, output)
                else:
                    result = {item["formula"]: item for item in native_sources.collect_macos(candidate, output)}
                    self.assertEqual(result["plain"]["fetched_by"], "upstream-url")
                    self.assertEqual(result["patched"]["fetched_by"], "homebrew")
                    self.assertEqual(downloaded, ["https://example.invalid/plain-1.0.tar.gz"])
                    self.assertTrue((output / "downloads/upstream/plain--plain-1.0.tar.gz").is_file())

    def test_checked_download_rejects_insecure_urls(self):
        for url in ["http://example.com/source.tar.gz", "https://user:secret@example.com/source.tar.gz"]:
            with self.subTest(url=url), self.assertRaises(ValueError):
                native_sources.download_checked(url, self.root / "unused", "0" * 64)

if __name__ == "__main__":
    unittest.main()
