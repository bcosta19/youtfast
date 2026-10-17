import hashlib
import shutil
import subprocess
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

import arch
import bundle
from release import ROOT, copy_public_files


class BundleTests(unittest.TestCase):
    def setUp(self):
        # target itself belongs to mbx; fixtures always use unique children.
        (ROOT / "target").mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="packaging-test-", dir=ROOT / "target")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_windows_dependency_graph_includes_transitive_dlls_and_stops_at_system(self):
        prefix = self.root / "ucrt64"
        (prefix / "bin").mkdir(parents=True)
        system = self.root / "System32"
        system.mkdir()
        for name in ["libmpv-2.dll", "libavcodec.dll"]:
            (prefix / "bin" / name).touch()
        (system / "KERNEL32.dll").touch()
        binary = self.root / "youtfast.exe"
        imports = {
            "youtfast.exe": "DLL Name: LIBMPV-2.dll\nDLL Name: KERNEL32.dll",
            "libmpv-2.dll": "DLL Name: libavcodec.dll\nDLL Name: api-ms-win-core-test.dll",
            "libavcodec.dll": "DLL Name: libmpv-2.dll",
        }
        actual = bundle.collect_dlls(binary, prefix, system, lambda path: imports[path.name])
        self.assertEqual({path.name for path in actual}, {"libmpv-2.dll", "libavcodec.dll"})

    def test_missing_non_system_windows_dependency_fails_instead_of_shipping(self):
        with self.assertRaisesRegex(ValueError, "Unresolved runtime DLL"):
            bundle.collect_dlls(self.root / "app.exe", self.root, self.root,
                                lambda _: "DLL Name: missing.dll")

    def test_macos_accepts_bundled_and_system_dependencies_but_rejects_build_machine_paths(self):
        executable_dir = self.root / "YouTFast.app/Contents/MacOS"
        executable_dir.mkdir(parents=True)
        library = executable_dir.parent / "Frameworks/libmpv.dylib"
        library.parent.mkdir()
        library.touch()
        binary = executable_dir / "youtfast-bin"
        header = "app:\n"
        suffix = " (compatibility version 1.0.0, current version 2.0.0)\n"
        valid = header + "@executable_path/../Frameworks/libmpv.dylib" + suffix + "/usr/lib/libSystem.B.dylib" + suffix
        bundle.verify_macho([binary], executable_dir, lambda _: valid)
        for dependency in ["/opt/homebrew/lib/libmpv.dylib", "@rpath/libmpv.dylib",
                           "@loader_path/missing.dylib"]:
            with self.subTest(dependency=dependency), self.assertRaises(ValueError):
                bundle.verify_macho([binary], executable_dir, lambda _: header + dependency + suffix)
        outside = self.root / "outside.dylib"
        outside.touch()
        with self.assertRaisesRegex(ValueError, "escapes"):
            bundle.verify_macho([binary], executable_dir,
                                lambda _: header + "@executable_path/../../../outside.dylib" + suffix)

    def test_windows_zip_keeps_full_semver_filename_and_package_tree(self):
        directory = self.root / "youtfast-v0.1.0-x86_64-pc-windows-gnu"
        directory.mkdir()
        (directory / "youtfast.exe").write_bytes(b"synthetic executable")
        path = bundle.archive(directory)
        self.assertEqual(path.name, directory.name + ".zip")
        with zipfile.ZipFile(path) as archive:
            self.assertEqual(archive.read(f"{directory.name}/youtfast.exe"), b"synthetic executable")

    def test_package_keeps_readme_installation_links_and_dependency_notices(self):
        directory = self.root / "package"
        copy_public_files(directory)
        for path in ["docs/distribution.md", "docs/actions.md", "RELEASING.md",
                     "CONTRIBUTING.md", "SECURITY.md", "CHANGELOG.md",
                     "assets/icons/LICENSE.txt", "vendor/rustypipe/LICENSE"]:
            with self.subTest(path=path):
                self.assertEqual((directory / path).read_bytes(), (ROOT / path).read_bytes())

    def test_arch_recipe_hashes_the_exact_source_asset_and_has_valid_shell_syntax(self):
        archive = self.root / "youtfast-v0.1.0-source-with-dependencies.tar.gz"
        archive.write_bytes(b"synthetic source asset")
        with patch.object(arch, "check", return_value="0.1.0"):
            output = arch.render(archive, self.root / "recipe")
        recipe = output / "PKGBUILD"
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        self.assertIn(digest, recipe.read_text())
        self.assertNotIn("SKIP", recipe.read_text())
        subprocess.run(["bash", "-n", str(recipe)], check=True)
        if shutil.which("makepkg"):
            info = subprocess.check_output(["makepkg", "--printsrcinfo"], cwd=output, text=True)
            self.assertIn("pkgver = 0.1.0", info)
            self.assertIn(f"sha256sums = {digest}", info)
            self.assertIn("arch = x86_64", info)

    def test_arch_refuses_another_version_or_absent_asset(self):
        with patch.object(arch, "check", return_value="0.1.0"):
            for name in ["missing.tar.gz", "youtfast-v0.2.0-source-with-dependencies.tar.gz"]:
                with self.subTest(name=name), self.assertRaises(ValueError):
                    arch.render(self.root / name, self.root / "unused")


if __name__ == "__main__":
    unittest.main()
