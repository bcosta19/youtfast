import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

PACKAGING = Path(__file__).resolve().parent
if str(PACKAGING) not in sys.path:
    sys.path.insert(0, str(PACKAGING))

spec = importlib.util.spec_from_file_location("windows_installer", PACKAGING / "windows_installer.py")
windows_installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(windows_installer)

ROOT = PACKAGING.parent


class WindowsInstallerTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "target").mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="win-installer-test-", dir=ROOT / "target")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_nsi_uses_per_user_dir_explicit_files_and_skips_nested_winget_when_silent(self):
        bundle = self.root / "youtfast-v0.6.2-x86_64-pc-windows-gnu"
        (bundle / "native-licenses/mpv").mkdir(parents=True)
        (bundle / "youtfast.exe").write_bytes(b"exe")
        (bundle / "libmpv-2.dll").write_bytes(b"dll")
        (bundle / "native-licenses/mpv/LICENSE").write_text("GPL")
        out_file = self.root / "youtfast-v0.6.2-x86_64-windows-setup.exe"

        nsi = windows_installer.render_nsi("0.6.2", bundle, out_file)

        self.assertIn('InstallDir "$LOCALAPPDATA\\Programs\\YouTFast"', nsi)
        self.assertIn("RequestExecutionLevel user", nsi)
        self.assertIn('VIProductVersion "0.6.2.0"', nsi)
        self.assertIn('File "/oname=youtfast.exe"', nsi)
        self.assertIn('File "/oname=libmpv-2.dll"', nsi)
        self.assertIn('SetOutPath "$INSTDIR\\native-licenses\\mpv"', nsi)
        self.assertIn('Delete "$INSTDIR\\native-licenses\\mpv\\LICENSE"', nsi)
        self.assertIn('RMDir "$INSTDIR\\native-licenses\\mpv"', nsi)
        self.assertIn('RMDir "$INSTDIR\\native-licenses"', nsi)
        self.assertIn("${IfNot} ${Silent}", nsi)
        self.assertNotIn("RMDir /r", nsi)

    def test_rejects_bundle_missing_executable_or_bad_version(self):
        bundle = self.root / "empty"
        bundle.mkdir()
        with self.assertRaisesRegex(ValueError, "youtfast.exe"):
            windows_installer.render_nsi("0.6.2", bundle, self.root / "out.exe")
        with self.assertRaisesRegex(ValueError, "MAJOR.MINOR.PATCH"):
            windows_installer.vi_product_version("0.6.2-rc1")


if __name__ == "__main__":
    unittest.main()
