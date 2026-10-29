#!/usr/bin/env python3
"""Build and validate the dependency-free public site into dist/site."""

from html.parser import HTMLParser
import re
from pathlib import Path
from posixpath import normpath
from shutil import copyfile
import tomllib
from urllib.parse import unquote, urlsplit
from site_locale import portuguese_page

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "dist" / "site"
ORIGIN = "https://youtfast.pages.dev"
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
_released = re.search(rf"^## \[{re.escape(VERSION)}\] - (\d{{4}}-\d{{2}}-\d{{2}})$",
                      (ROOT / "CHANGELOG.md").read_text(), re.M)
if not _released:
    raise ValueError(f"CHANGELOG.md has no dated entry for {VERSION}")
RELEASE_DATE = _released.group(1)
FILES = {
    "index.html": ROOT / "website/index.html",
    "styles.css": ROOT / "website/styles.css",
    "cinema.css": ROOT / "website/cinema.css",
    "assets/fonts/NotoSans-Regular.woff2": ROOT / "website/assets/fonts/NotoSans-Regular.woff2",
    "assets/fonts/NotoSans-Bold.woff2": ROOT / "website/assets/fonts/NotoSans-Bold.woff2",
    "assets/fonts/OFL.txt": ROOT / "website/assets/fonts/OFL.txt",
    "site.js": ROOT / "website/site.js",
    "assets/player.webp": ROOT / "website/assets/player.webp",
    "assets/youtfast.svg": ROOT / "packaging/icons/youtfast.svg",
    "assets/youtfast-demo.mp4": ROOT / "assets/demo/youtfast-demo.mp4",
}


def stamp(source):
    """Fill the release version (from Cargo.toml) into the page source."""
    return source.replace("{{VERSION}}", VERSION).replace("{{DATE}}", RELEASE_DATE)


class Page(HTMLParser):
    def __init__(self, path="index.html"):
        super().__init__(convert_charrefs=True)
        self.ids = set()
        self.references = []
        self.headings = 0
        self.path = path

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
                target = normpath(str(Path(self.path).parent / unquote(url.path)))
                if url.path.endswith("/"):
                    target = normpath(target + "/index.html")
                if url.path.startswith("/") or target.startswith("../"):
                    raise ValueError(f"Path is not project-Pages safe: {reference}")
                if target not in set(FILES) | {"pt/index.html"}:
                    raise ValueError(f"Asset missing from publication allowlist: {reference}")
            if url.fragment and url.fragment not in self.ids:
                raise ValueError(f"Missing section: {reference}")


def build():
    source = stamp(FILES["index.html"].read_text())
    if "{{" in source:
        raise ValueError("Unresolved template placeholder in website/index.html")
    localized = portuguese_page(source, ORIGIN)
    for name, html in [("index.html", source), ("pt/index.html", localized)]:
        page = Page(name)
        page.feed(html)
        page.validate()
    generated = {
        "index.html": source,
        "pt/index.html": localized,
        "404.html": ('<!doctype html><html lang="en"><meta charset="utf-8">'
                     '<meta name="viewport" content="width=device-width,initial-scale=1">'
                     '<meta name="robots" content="noindex">'
                     '<title>Page not found — YouTFast</title>'
                     '<body><main><h1>Page not found / Página não encontrada</h1>'
                     '<p><a href="/">YouTFast in English</a> · '
                     '<a href="/pt/">YouTFast em português</a></p></main></body></html>'),
        "robots.txt": f"User-agent: *\nAllow: /\n\nSitemap: {ORIGIN}/sitemap.xml\n",
        "sitemap.xml": (
            '<?xml version="1.0" encoding="UTF-8"?>\n'
            '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" '
            'xmlns:xhtml="http://www.w3.org/1999/xhtml">\n'
            + "".join(
                f"  <url><loc>{ORIGIN}{path}</loc>"
                f'<xhtml:link rel="alternate" hreflang="en" href="{ORIGIN}/"/>'
                f'<xhtml:link rel="alternate" hreflang="pt-BR" href="{ORIGIN}/pt/"/>'
                f'<xhtml:link rel="alternate" hreflang="x-default" href="{ORIGIN}/"/>'
                "</url>\n" for path in ["/", "/pt/"]
            ) + "</urlset>\n"
        ),
    }
    # Publish only the explicit public assets above, never the repository root.
    for name, source in FILES.items():
        if name == "index.html":
            continue  # written above with the release version filled in
        if not source.is_file() or source.is_symlink():
            raise ValueError(f"Missing or symbolic source asset: {source.relative_to(ROOT)}")
        target = OUTPUT / name
        for path in (ROOT / "dist", OUTPUT, target.parent, target):
            if path.is_symlink():
                raise ValueError("Refusing a symbolic output path")
        target.parent.mkdir(parents=True, exist_ok=True)
        copyfile(source, target)
    for name, content in generated.items():
        target = OUTPUT / name
        if any(path.is_symlink() for path in [target.parent, target]):
            raise ValueError("Refusing a symbolic generated output path")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")
    expected = set(FILES) | set(generated) | {".nojekyll"}
    nojekyll = OUTPUT / ".nojekyll"
    if nojekyll.is_symlink():
        raise ValueError("Refusing a symbolic .nojekyll output")
    nojekyll.touch()
    actual = {str(path.relative_to(OUTPUT)) for path in OUTPUT.rglob("*") if path.is_file()}
    if actual != expected or any(path.is_symlink() for path in OUTPUT.rglob("*")):
        raise ValueError("Unexpected files in dist/site; publish only the site allowlist")
    size = sum((OUTPUT / name).stat().st_size for name in expected)
    print(f"Validated {len(expected)} public files ({size / 1024 / 1024:.1f} MiB): {OUTPUT}")


if __name__ == "__main__":
    build()
