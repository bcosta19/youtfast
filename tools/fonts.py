#!/usr/bin/env python3
"""Regenerates assets/fonts/NotoFallback.ttf, the last fallback face.

People write comments and channel names with characters from blocks that
neither Inter nor the installed faces fastframe-fonts looks for cover on
every platform: "small caps" (ᴛʜɪs, ꜰ, ꜱ), styled letters (𝓗𝓮𝓵𝓵𝓸, ⓑⓤⓑⓑⓛⓔ),
symbols and the faces of kaomoji (ʕ•ᴥ•ʔ, ᕕ( ᐛ )ᕗ, ༼ つ ◕_◕ ༽つ,
(╯°□°)╯︵ ┻━┻, ⸜(｡˃ ᵕ ˂ )⸝). Without a face for them egui draws
the replacement box. This takes those blocks from Noto fonts, at
regular weight, and merges them into one small file the app bundles
(crates/youtfast-appearance/src/emoji.rs).

The sources are pinned to a google/fonts commit and checked by SHA-256.
Needs fontTools (`pip install fonttools`) and network access. Run it after
changing the blocks or the sources, and commit the result.
"""
import hashlib
import io
import urllib.request
from pathlib import Path

from fontTools import subset
from fontTools.merge import Merger
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "assets/fonts/NotoFallback.ttf"
COMMIT = "29b4ec6df2522a8aee279414b78a07910b065874"
FAMILY = "YouTFast Noto Fallback"

# (path in google/fonts, SHA-256, code point ranges taken from it)
SOURCES = [
    (
        "ofl/notosans/NotoSans%5Bwdth%2Cwght%5D.ttf",
        "bfb7bb691513f12e734dc346c03a03f784912432d7e3fa8e56efcf906fe86b3d",
        [
            (0x1D00, 0x1DBF),  # Phonetic Extensions (and Supplement): ᴀ ᴥ ᵕ ᴗ
            (0x2C60, 0x2C7F),  # Latin Extended-C
            (0x2E00, 0x2E7F),  # Supplemental Punctuation: ⸜ ⸝
            (0xA720, 0xA7FF),  # Latin Extended-D: ꜰ ꜱ
        ],
    ),
    (
        "ofl/notosansmono/NotoSansMono%5Bwdth%2Cwght%5D.ttf",
        "2cb2adb378a8f574213e23df697050b83c54c27df465a2015552740b2769a081",
        [(0x2500, 0x259F)],  # Box Drawing, Block Elements: ╯┻━ ░▒▓█
    ),
    (
        "ofl/notosanssymbols2/NotoSansSymbols2-Regular.ttf",
        "7d5fb73b7ca67a6798101741f5d280a3d016a56a197afcd4199dbb57b4b82a21",
        [
            (0x2190, 0x21FF),  # Arrows
            (0x2300, 0x23FF),  # Miscellaneous Technical
            (0x25A0, 0x25FF),  # Geometric Shapes: ◉ ◕
            (0x2600, 0x27BF),  # Miscellaneous Symbols, Dingbats: ✧ ✩ ✿
            (0x27F0, 0x27FF),  # Supplemental Arrows-A
            (0x2800, 0x297F),  # Braille Patterns, Supplemental Arrows-B: ⣿
            (0x2B00, 0x2BFF),  # Miscellaneous Symbols and Arrows
        ],
    ),
    (
        "ofl/notosansmath/NotoSansMath-Regular.ttf",
        "3f495fe933c06786e4d5f6d86b8ee70b6753a68ee3b9d87528726de0f6e2c47d",
        [
            (0x2100, 0x214F),  # Letterlike Symbols: ℌ
            (0x2200, 0x22FF),  # Mathematical Operators: ⊂ ⋆
            (0x27C0, 0x27EF),  # Miscellaneous Mathematical Symbols-A: ⟡
            (0x1D400, 0x1D7FF),  # Mathematical Alphanumeric Symbols: 𝓗𝓮𝓵𝓵𝓸 𝐁𝐨𝐥𝐝 𝕯
        ],
    ),
    (
        "ofl/notosanssymbols/NotoSansSymbols%5Bwght%5D.ttf",
        "f7e7e04b4a24b6c78893d50cbfd2b2f6cae49617ab047bfef668d252adb128f7",
        [
            (0x2100, 0x218F),  # Letterlike Symbols, Number Forms: ℌ Ⅻ
            (0x2460, 0x24FF),  # Enclosed Alphanumerics: ⓑⓤⓑⓑⓛⓔ
            (0x1F100, 0x1F1FF),  # Enclosed Alphanumeric Supplement: 🅷 🆇
        ],
    ),
    (
        "ofl/notosansyi/NotoSansYi-Regular.ttf",
        "ee4de376a1e4f3c4bc7e116f4f46538348716b8408b86a0fc18c1c6128d2e56d",
        [(0xA490, 0xA4CF)],  # Yi Radicals: ꒰ ꒱
    ),
    (
        "ofl/notosanskannada/NotoSansKannada%5Bwdth%2Cwght%5D.ttf",
        "cca4f3b3a8cb12fb261f1b43baf5d2f7f59d90fe123d41f0065ed3a183997ec9",
        # ಠ alone, the eyes of ಠ_ಠ: Kannada text comes from the
        # installed faces, ahead of this one.
        [(0x0CA0, 0x0CA0)],
    ),
    (
        "ofl/notosanscanadianaboriginal/NotoSansCanadianAboriginal%5Bwght%5D.ttf",
        "81b0f7e7ca48b28e343bb6a523a066743acedfbb29f374cbe4501e6d5f08f7af",
        [
            (0x1400, 0x167F),  # Unified Canadian Aboriginal Syllabics: ᕕ ᐛ ᐢ
            (0x18B0, 0x18FF),  # and its extension
        ],
    ),
    (
        "ofl/notoseriftibetan/NotoSerifTibetan%5Bwght%5D.ttf",
        "060ec022b04c306de3f58d051fb0e1cf81a5b610c5910fbfc43bba154c057cda",
        [
            (0x0F00, 0x0F17),  # Tibetan marks and punctuation
            (0x0F1A, 0x0F1F),
            (0x0F3A, 0x0F3D),  # ༺ ༻ ༼ ༽
        ],
    ),
]


def download(path, digest):
    url = f"https://raw.githubusercontent.com/google/fonts/{COMMIT}/{path}"
    with urllib.request.urlopen(url) as response:
        data = response.read()
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError(f"{path} does not match its pinned SHA-256")
    return data


def regular(data, ranges):
    """The face at regular weight (and normal width), only `ranges`."""
    font = TTFont(io.BytesIO(data))
    if "fvar" in font:
        axes = {axis.axisTag: axis for axis in font["fvar"].axes}
        location = {"wght": 400}
        if "wdth" in axes:
            location["wdth"] = 100
        font = instancer.instantiateVariableFont(font, location)
    options = subset.Options()
    options.layout_features = ["*"]
    options.hinting = False
    options.name_IDs = ["*"]
    options.notdef_outline = True
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=[c for start, end in ranges for c in range(start, end + 1)])
    subsetter.subset(font)
    # Vertical metrics and style tables are not drawn with, and only some
    # sources carry them, which the merger cannot reconcile.
    for tag in ("STAT", "vhea", "vmtx", "gasp"):
        if tag in font:
            del font[tag]
    buffer = io.BytesIO()
    font.save(buffer)
    buffer.seek(0)
    return buffer


def rename(font):
    names = font["name"]
    for record in list(names.names):
        if record.nameID in (1, 3, 4, 6, 16, 17, 21, 22):
            names.removeNames(nameID=record.nameID)
    names.setName(FAMILY, 1, 3, 1, 0x409)
    names.setName("Regular", 2, 3, 1, 0x409)
    names.setName(f"{FAMILY} Regular", 3, 3, 1, 0x409)
    names.setName(f"{FAMILY} Regular", 4, 3, 1, 0x409)
    names.setName(FAMILY.replace(" ", "") + "-Regular", 6, 3, 1, 0x409)


def line_box(font, source):
    """The vertical metrics of `source`: the merger keeps the tallest of
    each (Tibetan's), which would put this face's baseline far from the
    interface face's."""
    for table, fields in {
        "hhea": ("ascent", "descent", "lineGap"),
        "OS/2": ("sTypoAscender", "sTypoDescender", "sTypoLineGap", "usWinAscent", "usWinDescent"),
    }.items():
        for field in fields:
            setattr(font[table], field, getattr(source[table], field))
    font["OS/2"].fsSelection = source["OS/2"].fsSelection


def main():
    parts = [regular(download(path, digest), ranges) for path, digest, ranges in SOURCES]
    latin = TTFont(parts[0])
    parts[0].seek(0)
    merged = Merger().merge(parts)
    line_box(merged, latin)
    rename(merged)
    # The same sources give the same bytes: no save time in the file.
    merged.recalcTimestamp = False
    merged["head"].created = merged["head"].modified = 0
    merged.save(OUTPUT)
    print(f"{OUTPUT.relative_to(ROOT)}: {OUTPUT.stat().st_size} bytes")


if __name__ == "__main__":
    main()
