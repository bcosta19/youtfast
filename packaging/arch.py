"""Render an Arch source recipe from the exact, locally available release asset."""

import argparse
import hashlib
import tarfile
import tomllib
from pathlib import Path

from release import ROOT, check


def render(archive, output, root=ROOT):
    current = check(root)
    expected = f"youtfast-v{current}-source-with-dependencies.tar.gz"
    if archive.name != expected or not archive.is_file():
        raise ValueError(f"Expected the final {expected} source asset")
    with archive.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    package = tomllib.loads((root / "Cargo.toml").read_text())["package"]
    text = (root / "packaging/arch/PKGBUILD.in").read_text()
    for placeholder, value in {
        "@VERSION@": current,
        "@RUST_VERSION@": package["rust-version"],
        "@SHA256@": digest,
    }.items():
        text = text.replace(placeholder, value)
    output.mkdir(parents=True, exist_ok=False)
    (output / "PKGBUILD").write_text(text)
    (output / "README.md").write_text((root / "packaging/arch/README.md").read_text())
    return output


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    args = parser.parse_args()
    current = check()
    directory = render(args.archive, ROOT / "dist/aur/youtfast")
    with tarfile.open(ROOT / f"dist/youtfast-v{current}-aur.tar.gz", "w:gz") as bundle:
        bundle.add(directory, arcname="youtfast")
