#!/usr/bin/env python3
"""Builds the missing Unicode 16/17 emoji outlines from OpenMoji 17.0.

Built with fontTools 4.68.0 for byte-identical regeneration. The pinned
upstream font is CC BY-SA 4.0; the subset keeps that license and is
attributed in assets/fonts/OpenMoji-LICENSE.txt.
"""
import hashlib
import io
import urllib.request
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
COMMIT = "f9fc506a3f913be9897ab0181d611d4c910a4104"
SHA256 = "bfa18b01a9038fe06f0fe72c72bb6ad38e9de4d6ec99951e170f8e5cb776159a"
# Unicode 16 and 17 single-codepoint emoji absent from Noto Emoji 3.002.
CHARACTERS = "🫩🫆🪾🫜🪉🪏🫟🫪🛘🫍🫈🪊🪎🫯"


def main():
    url = (f"https://raw.githubusercontent.com/hfg-gmuend/openmoji/{COMMIT}/"
           "font/OpenMoji-black-glyf/OpenMoji-black-glyf.ttf")
    with urllib.request.urlopen(url, timeout=60) as response:
        data = response.read()
    if hashlib.sha256(data).hexdigest() != SHA256:
        raise ValueError("OpenMoji does not match its pinned SHA-256")
    font = TTFont(io.BytesIO(data))
    options = subset.Options()
    options.hinting = False
    options.notdef_outline = True
    subsetter = subset.Subsetter(options)
    subsetter.populate(text=CHARACTERS)
    subsetter.subset(font)
    # Identify the modified subset instead of the full upstream font.
    for name_id, value in {
        1: "YouTFast Emoji Supplement", 2: "Regular",
        3: "YouTFast Emoji Supplement Regular",
        4: "YouTFast Emoji Supplement Regular",
        6: "YouTFastEmojiSupplement-Regular",
    }.items():
        font["name"].removeNames(nameID=name_id)
        font["name"].setName(value, name_id, 3, 1, 0x409)
    font.recalcTimestamp = False
    font["head"].created = font["head"].modified = 0
    output = ROOT / "assets/fonts/EmojiSupplement.ttf"
    font.save(output)
    print(f"{output.relative_to(ROOT)}: {output.stat().st_size} bytes")


if __name__ == "__main__":
    main()
