import hashlib
import os
import stat
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INSTALL_SH = ROOT / "website/install.sh"
INSTALL_PS1 = ROOT / "website/install.ps1"


class InstallerScriptTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "target").mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="installer-test-", dir=ROOT / "target")
        self.addCleanup(self.temporary.cleanup)
        self.tmp = Path(self.temporary.name)

    def write_executable(self, path: Path, content: str):
        path.write_text(content, encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def make_native_tarball(self, version: str) -> Path:
        stem = f"youtfast-v{version}-x86_64-unknown-linux-gnu"
        bundle = self.tmp / "build" / stem
        (bundle / "packaging/icons").mkdir(parents=True)
        (bundle / "packaging/applications").mkdir(parents=True)
        self.write_executable(bundle / "youtfast", f"#!/bin/sh\necho youtfast {version}\n")
        (bundle / "packaging/icons/youtfast.svg").write_text("<svg/>", encoding="utf-8")
        (bundle / "packaging/applications/youtfast.desktop").write_text(
            "[Desktop Entry]\nName=YouTFast\nExec=youtfast %u\n",
            encoding="utf-8",
        )
        archive = self.tmp / "server" / f"{stem}.tar.gz"
        archive.parent.mkdir(parents=True, exist_ok=True)
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(bundle, arcname=stem)
        return archive

    def base_env(self, server_dir: Path, home_dir: Path, extra_bin: Path) -> dict[str, str]:
        self.write_executable(
            extra_bin / "uname",
            "#!/bin/sh\nif [ \"$1\" = \"-s\" ]; then echo Linux; else echo x86_64; fi\n",
        )
        self.write_executable(
            extra_bin / "curl",
            f"""#!/bin/sh
url=""
out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    -H) shift 2 ;;
    *) url="$1"; shift ;;
  esac
done
case "$url" in
  https://api.github.com/repos/bcosta19/youtfast/releases/latest)
    printf '{{"tag_name": "v0.6.2"}}\\n'
    ;;
  https://github.com/bcosta19/youtfast/releases/download/v0.6.2/*)
    name="${{url##*/}}"
    if [ -n "$out" ]; then
      cp "{server_dir}/$name" "$out"
    else
      cat "{server_dir}/$name"
    fi
    ;;
  *)
    exit 1
    ;;
esac
""",
        )
        env = os.environ.copy()
        env["HOME"] = str(home_dir)
        env["TMPDIR"] = str(self.tmp)
        env["PATH"] = f"{extra_bin}:/usr/bin:/bin"
        return env

    def test_shell_syntax_is_valid_posix_sh(self):
        subprocess.run(["sh", "-n", str(INSTALL_SH)], check=True)
        self.assertIn("Invoke-RestMethod", INSTALL_PS1.read_text(encoding="utf-8"))

    def test_native_install_verifies_checksum_and_uninstall_cleans_up(self):
        archive = self.make_native_tarball("0.6.2")
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        (archive.parent / "checksums.txt").write_text(f"{digest}  {archive.name}\n", encoding="utf-8")

        home = self.tmp / "home"
        home.mkdir()
        fake_bin = self.tmp / "bin"
        fake_bin.mkdir()
        env = self.base_env(archive.parent, home, fake_bin)

        subprocess.run(["sh", str(INSTALL_SH), "--method", "native", "--yes"], env=env, capture_output=True, check=True)
        binary = home / ".local/bin/youtfast"
        desktop = home / ".local/share/applications/io.github.bcosta19.YouTFast.desktop"
        icon = home / ".local/share/icons/hicolor/scalable/apps/youtfast.svg"
        self.assertTrue(binary.is_file())
        self.assertIn(f"Exec={binary} %u", desktop.read_text(encoding="utf-8"))
        self.assertTrue(icon.is_file())

        subprocess.run(["sh", str(INSTALL_SH), "--uninstall", "--yes"], env=env, capture_output=True, check=True)
        self.assertFalse(binary.exists())
        self.assertFalse(desktop.exists())
        self.assertFalse(icon.exists())

    def test_tampered_checksum_aborts_before_installing(self):
        archive = self.make_native_tarball("0.6.2")
        (archive.parent / "checksums.txt").write_text(f"{'0' * 64}  {archive.name}\n", encoding="utf-8")

        home = self.tmp / "home"
        home.mkdir()
        fake_bin = self.tmp / "bin"
        fake_bin.mkdir()
        env = self.base_env(archive.parent, home, fake_bin)

        result = subprocess.run(
            ["sh", str(INSTALL_SH), "--method", "native", "--yes"],
            env=env,
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SHA-256 verification failed", result.stderr)
        self.assertFalse((home / ".local/bin/youtfast").exists())

    def test_flatpak_method_verifies_checksum_and_installs_user_bundle(self):
        server = self.tmp / "server"
        server.mkdir()
        bundle = server / "youtfast-v0.6.2-x86_64.flatpak"
        bundle.write_bytes(b"synthetic flatpak bundle")
        digest = hashlib.sha256(bundle.read_bytes()).hexdigest()
        (server / "checksums.txt").write_text(f"{digest}  {bundle.name}\n", encoding="utf-8")

        home = self.tmp / "home"
        home.mkdir()
        fake_bin = self.tmp / "bin"
        fake_bin.mkdir()
        log_file = self.tmp / "flatpak.log"
        self.write_executable(
            fake_bin / "flatpak",
            f"#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{log_file}'\n",
        )
        env = self.base_env(server, home, fake_bin)

        subprocess.run(["sh", str(INSTALL_SH), "--method", "flatpak", "--yes"], env=env, capture_output=True, check=True)
        logged = log_file.read_text(encoding="utf-8")
        self.assertIn("remote-add --user --if-not-exists flathub", logged)
        self.assertIn("install --user -y ", logged)
        self.assertIn(bundle.name, logged)

    def test_arch_method_verifies_checksum_and_invokes_pacman_upgrade(self):
        server = self.tmp / "server"
        server.mkdir()
        pkg = server / "youtfast-0.6.2-1-x86_64.pkg.tar.zst"
        pkg.write_bytes(b"synthetic arch pkg")
        digest = hashlib.sha256(pkg.read_bytes()).hexdigest()
        (server / "checksums.txt").write_text(f"{digest}  {pkg.name}\n", encoding="utf-8")

        home = self.tmp / "home"
        home.mkdir()
        fake_bin = self.tmp / "bin"
        fake_bin.mkdir()
        log_file = self.tmp / "pacman.log"
        self.write_executable(fake_bin / "sudo", "#!/bin/sh\nexec \"$@\"\n")
        self.write_executable(
            fake_bin / "pacman",
            f"#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{log_file}'\n",
        )
        env = self.base_env(server, home, fake_bin)

        subprocess.run(["sh", str(INSTALL_SH), "--method", "arch", "--yes"], env=env, capture_output=True, check=True)
        logged = log_file.read_text(encoding="utf-8")
        self.assertIn("-U --noconfirm ", logged)
        self.assertIn(pkg.name, logged)


if __name__ == "__main__":
    unittest.main()
