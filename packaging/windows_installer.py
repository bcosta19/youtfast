"""Build a graphical Windows NSIS setup installer from the bundled Windows directory."""

import argparse
import os
import re
import shutil
import subprocess
from collections import defaultdict
from pathlib import Path

from release import ROOT, check


def nsis_quote(value: str) -> str:
    """Escape characters that NSIS interprets inside double-quoted strings."""
    return value.replace("$", "$$").replace('"', '$\\"')


def vi_product_version(version: str) -> str:
    match = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)", version)
    if match is None:
        raise ValueError(f"Expected MAJOR.MINOR.PATCH version for Windows installer, got {version!r}")
    return f"{match.group(1)}.{match.group(2)}.{match.group(3)}.0"


def collect_bundle_entries(bundle_dir: Path) -> tuple[list[tuple[Path, Path]], list[Path]]:
    """Return sorted (relative_path, source_file) pairs and bottom-up relative directories."""
    if not (bundle_dir / "youtfast.exe").is_file():
        raise ValueError(f"Missing youtfast.exe in Windows bundle directory: {bundle_dir}")
    files: list[tuple[Path, Path]] = []
    dirs: set[Path] = set()
    for path in sorted(bundle_dir.rglob("*")):
        rel = path.relative_to(bundle_dir)
        if path.is_dir():
            dirs.add(rel)
        elif path.is_file():
            files.append((rel, path))
            for parent in rel.parents:
                if parent != Path("."):
                    dirs.add(parent)
    bottom_up_dirs = sorted(dirs, key=lambda item: (-len(item.parts), str(item)))
    return files, bottom_up_dirs


def render_file_directives(bundle_dir: Path) -> tuple[str, str]:
    """Generate explicit SetOutPath/File and Delete/RMDir NSIS blocks for bundle_dir."""
    files, bottom_up_dirs = collect_bundle_entries(bundle_dir)
    by_parent: dict[Path, list[tuple[Path, Path]]] = defaultdict(list)
    for rel, src in files:
        by_parent[rel.parent].append((rel, src))

    install_lines: list[str] = []
    for parent in sorted(by_parent, key=lambda item: (len(item.parts), str(item))):
        if parent == Path("."):
            out_dir = "$INSTDIR"
        else:
            out_dir = "$INSTDIR\\" + "\\".join(nsis_quote(part) for part in parent.parts)
        install_lines.append(f'  SetOutPath "{out_dir}"')
        for rel, src in by_parent[parent]:
            install_lines.append(f'  File "/oname={nsis_quote(rel.name)}" "{nsis_quote(str(src))}"')

    uninstall_lines: list[str] = []
    for rel, _ in files:
        target = "$INSTDIR\\" + "\\".join(nsis_quote(part) for part in rel.parts)
        uninstall_lines.append(f'  Delete "{target}"')
    for rel_dir in bottom_up_dirs:
        target = "$INSTDIR\\" + "\\".join(nsis_quote(part) for part in rel_dir.parts)
        uninstall_lines.append(f'  RMDir "{target}"')

    return "\n".join(install_lines), "\n".join(uninstall_lines)


def render_nsi(
    version: str,
    bundle_dir: Path,
    out_file: Path,
    root: Path = ROOT,
) -> str:
    template = (root / "packaging/windows/youtfast.nsi.in").read_text(encoding="utf-8")
    icon_file = root / "packaging/icons/youtfast.ico"
    license_file = root / "LICENSE"
    if not icon_file.is_file() or not license_file.is_file():
        raise ValueError("Windows installer requires packaging/icons/youtfast.ico and LICENSE")
    install_files, uninstall_files = render_file_directives(bundle_dir)
    replacements = {
        "@OUT_FILE@": nsis_quote(str(out_file)),
        "@VI_VERSION@": vi_product_version(version),
        "@VERSION@": nsis_quote(version),
        "@ICON_FILE@": nsis_quote(str(icon_file)),
        "@LICENSE_FILE@": nsis_quote(str(license_file)),
        "@INSTALL_FILES@": install_files,
        "@UNINSTALL_FILES@": uninstall_files,
    }
    for placeholder, value in replacements.items():
        template = template.replace(placeholder, value)
    return template


def find_makensis() -> Path:
    for candidate in ("makensis", "makensis.exe"):
        found = shutil.which(candidate)
        if found:
            return Path(found)
    for env_var in ("ProgramFiles(x86)", "ProgramFiles"):
        base = os.environ.get(env_var)
        if base:
            candidate = Path(base) / "NSIS/makensis.exe"
            if candidate.is_file():
                return candidate
    raise ValueError("makensis was not found on PATH or in Program Files/NSIS")


def build_installer(bundle_dir: Path, dist: Path | None = None, root: Path = ROOT) -> Path:
    current = check(root)
    dist_dir = dist or (root / "dist")
    dist_dir.mkdir(parents=True, exist_ok=True)
    out_file = (dist_dir / f"youtfast-v{current}-x86_64-windows-setup.exe").resolve()
    script_path = dist_dir / f"youtfast-v{current}-windows-setup.nsi"
    script_path.write_text(render_nsi(current, bundle_dir.resolve(), out_file, root=root), encoding="utf-8")
    try:
        subprocess.run([str(find_makensis()), "-V2", str(script_path)], check=True)
    finally:
        script_path.unlink(missing_ok=True)
    if not out_file.is_file():
        raise ValueError(f"Expected NSIS output installer at {out_file}")
    return out_file


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path, help="Unpacked dist/youtfast-vX.Y.Z-x86_64-pc-windows-gnu directory")
    parser.add_argument("--dist", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    print(build_installer(args.bundle, dist=args.dist))
