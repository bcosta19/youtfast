#!/usr/bin/env python3
"""Build and validate the dependency-free GitHub Pages site into dist/site."""

from html.parser import HTMLParser
from pathlib import Path
from shutil import copyfile
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "dist" / "site"
FILES = {
    "index.html": ROOT / "website/index.html",
    "styles.css": ROOT / "website/styles.css",
    "site.js": ROOT / "website/site.js",
    "assets/player.webp": ROOT / "website/assets/player.webp",
    "assets/youtfast.svg": ROOT / "packaging/icons/youtfast.svg",
    "assets/youtfast-demo.mp4": ROOT / "assets/demo/youtfast-demo.mp4",
}


class Page(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.ids = set()
        self.references = []
        self.headings = 0

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if "id" in attrs:
            if attrs["id"] in self.ids:
                raise ValueError(f"Duplicate HTML id: {attrs['id']}")
            self.ids.add(attrs["id"])
        self.headings += tag == "h1"
        if tag == "img" and "alt" not in attrs:
            raise ValueError("Image without alternative text")
        if ("data-en" in attrs) != ("data-pt" in attrs):
            raise ValueError("Incomplete English/Portuguese translation")
        for key in ("src", "href", "poster"):
            if attrs.get(key):
                self.references.append(attrs[key])

    def validate(self):
        if self.headings != 1:
            raise ValueError("The landing page must have exactly one h1")
        for reference in self.references:
            url = urlsplit(reference)
            if url.scheme or url.netloc:
                if url.scheme != "https":
                    raise ValueError(f"Unexpected external URL scheme: {reference}")
                continue
            if url.path:
                if url.path.startswith("/") or ".." in Path(unquote(url.path)).parts:
                    raise ValueError(f"Path is not project-Pages safe: {reference}")
                if unquote(url.path) not in FILES:
                    raise ValueError(f"Asset missing from publication allowlist: {reference}")
            if url.fragment and url.fragment not in self.ids:
                raise ValueError(f"Missing section: {reference}")


def build():
    page = Page()
    page.feed(FILES["index.html"].read_text())
    page.validate()
    # Publish only the explicit public assets above, never the repository root.
    for name, source in FILES.items():
        if not source.is_file() or source.is_symlink():
            raise ValueError(f"Missing or symbolic source asset: {source.relative_to(ROOT)}")
        target = OUTPUT / name
        for path in (ROOT / "dist", OUTPUT, target.parent, target):
            if path.is_symlink():
                raise ValueError("Refusing a symbolic output path")
        target.parent.mkdir(parents=True, exist_ok=True)
        copyfile(source, target)
    expected = set(FILES) | {".nojekyll"}
    nojekyll = OUTPUT / ".nojekyll"
    if nojekyll.is_symlink():
        raise ValueError("Refusing a symbolic .nojekyll output")
    nojekyll.touch()
    actual = {str(path.relative_to(OUTPUT)) for path in OUTPUT.rglob("*") if path.is_file()}
    if actual != expected or any(path.is_symlink() for path in OUTPUT.rglob("*")):
        raise ValueError("Unexpected files in dist/site; publish only the site allowlist")
    size = sum((OUTPUT / name).stat().st_size for name in FILES)
    print(f"Validated {len(FILES)} public files ({size / 1024 / 1024:.1f} MiB): {OUTPUT}")


if __name__ == "__main__":
    build()
