"""Translated documentation stays complete, in step with English and linked correctly."""

import re
import unittest
from pathlib import Path

from release import ROOT, workspace_package

# English sources and the translation each language keeps of them, under docs/i18n/<language>/.
DOCS = ["README.md", "docs/user-guide.md", "docs/install.md"]
LANGUAGES = ["pt-BR", "es", "fr", "de", "it"]

FENCE = re.compile(r"^(```|~~~)")
HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")
MARKDOWN_LINK = re.compile(r"\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
HTML_LINK = re.compile(r"\b(?:href|src|srcset)=\"([^\"]+)\"")


def translation(source, language):
    return ROOT / "docs/i18n" / language / Path(source).name


def prose_lines(path):
    """Lines outside fenced code blocks."""
    fenced = False
    for line in path.read_text(encoding="utf-8").splitlines():
        if FENCE.match(line):
            fenced = not fenced
        elif not fenced:
            yield line


def headings(path):
    return [(len(match[1]), match[2]) for line in prose_lines(path) if (match := HEADING.match(line))]


def anchors(path):
    """GitHub's heading anchors: inline markup dropped, punctuation removed, spaces as hyphens."""
    seen = {}
    result = set()
    for _, text in headings(path):
        text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", text)
        slug = re.sub(r"[^\w\- ]", "", text.lower()).replace(" ", "-")
        count = seen.get(slug, 0)
        seen[slug] = count + 1
        result.add(slug if count == 0 else f"{slug}-{count}")
    return result


def relative_targets(path):
    for line in prose_lines(path):
        for target in MARKDOWN_LINK.findall(line) + HTML_LINK.findall(line):
            if not re.match(r"[a-z][a-z0-9+.-]*:", target):
                yield target


def broken_links(path):
    broken = []
    for target in relative_targets(path):
        file, _, fragment = target.partition("#")
        resolved = (path.parent / file).resolve() if file else path
        if not resolved.exists():
            broken.append(target)
        elif fragment and resolved.suffix == ".md" and fragment not in anchors(resolved):
            broken.append(target)
    return broken


def badge_urls(path):
    return [
        url
        for line in prose_lines(path)
        for url in HTML_LINK.findall(line)
        if url.startswith("https://img.shields.io/") or "/actions/workflows/" in url
    ]


class TranslatedDocsTests(unittest.TestCase):
    def test_every_language_translates_every_document(self):
        missing = [
            str(translation(source, language).relative_to(ROOT))
            for source in DOCS
            for language in LANGUAGES
            if not translation(source, language).is_file()
        ]
        self.assertEqual(missing, [], "translate these with the English change (AGENTS.md)")

    def test_translations_keep_the_english_section_structure(self):
        for source in DOCS:
            expected = [level for level, _ in headings(ROOT / source)]
            for language in LANGUAGES:
                with self.subTest(document=source, language=language):
                    actual = [level for level, _ in headings(translation(source, language))]
                    self.assertEqual(actual, expected, "sections differ from English: update the translation")

    def test_relative_links_and_anchors_resolve(self):
        paths = [ROOT / source for source in DOCS]
        paths += [translation(source, language) for source in DOCS for language in LANGUAGES]
        for path in paths:
            with self.subTest(document=str(path.relative_to(ROOT))):
                self.assertEqual(broken_links(path), [])

    def test_every_document_links_all_other_languages(self):
        for source in DOCS:
            family = [ROOT / source] + [translation(source, language) for language in LANGUAGES]
            for path in family:
                linked = {
                    (path.parent / target.partition("#")[0]).resolve()
                    for target in relative_targets(path)
                    if target.partition("#")[0]
                }
                missing = [
                    str(sibling.relative_to(ROOT))
                    for sibling in family
                    if sibling != path and sibling.resolve() not in linked
                ]
                with self.subTest(document=str(path.relative_to(ROOT))):
                    self.assertEqual(missing, [], "language switcher is missing sibling links")

    def test_readme_badges_match_workspace_and_translations(self):
        pkg = workspace_package(ROOT)
        english = badge_urls(ROOT / "README.md")
        self.assertTrue(any(f"rust-{pkg['rust-version']}%2B" in url for url in english))
        self.assertTrue(any(pkg["license"].replace("-", "--") in url for url in english))
        for language in LANGUAGES:
            with self.subTest(language=language):
                self.assertEqual(badge_urls(translation("README.md", language)), english)


if __name__ == "__main__":
    unittest.main()
