#!/usr/bin/env python3
"""Regenerates the platform icons from packaging/icons/*.svg.

youtfast.ico is embedded in the Windows executable (build.rs) and
youtfast.icns in the macOS bundle (packaging/bundle.py). Both carry PNG
images; below 40 pixels they draw the flat mark, as the window icon does
(`SMALL_BELOW` in src/main.rs). Needs rsvg-convert. Run it after changing
either SVG and commit the results.
"""
import struct
import subprocess
from pathlib import Path

ICONS = Path(__file__).resolve().parent.parent / "packaging/icons"
SMALL_BELOW = 40

ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]
# Apple's PNG icon types by pixel size; the @2x types share sizes with others.
ICNS_TYPES = [
    (b"icp4", 16), (b"icp5", 32), (b"ic11", 32), (b"ic12", 64), (b"ic07", 128),
    (b"ic08", 256), (b"ic13", 256), (b"ic09", 512), (b"ic14", 512), (b"ic10", 1024),
]


def render(size):
    mark = ICONS / ("youtfast-small.svg" if size < SMALL_BELOW else "youtfast.svg")
    return subprocess.run(
        ["rsvg-convert", "--width", str(size), "--height", str(size), str(mark)],
        check=True, capture_output=True,
    ).stdout


def ico(images):
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = len(header) + 16 * len(images)
    entries, data = b"", b""
    for size, png in images:
        # A width or height of 0 means 256.
        entries += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32,
                               len(png), offset + len(data))
        data += png
    return header + entries + data


def icns(images):
    body = b"".join(kind + struct.pack(">I", 8 + len(png)) + png for kind, png in images)
    return b"icns" + struct.pack(">I", 8 + len(body)) + body


def main():
    pngs = {size: render(size) for size in {*ICO_SIZES, *(size for _, size in ICNS_TYPES)}}
    (ICONS / "youtfast.ico").write_bytes(ico([(size, pngs[size]) for size in ICO_SIZES]))
    (ICONS / "youtfast.icns").write_bytes(icns([(kind, pngs[size]) for kind, size in ICNS_TYPES]))


if __name__ == "__main__":
    main()
