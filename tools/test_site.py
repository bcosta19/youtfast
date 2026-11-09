"""Regression checks for crawlable localized product information."""
from html.parser import HTMLParser
from pathlib import Path
import json
import re
import unittest

from build_site import ORIGIN, VERSION, Page, render_pages, stamp, validate_pages
from site_locale import portuguese_page

WEBSITE = Path(__file__).resolve().parents[1] / 'website'
SOURCE = stamp((WEBSITE / 'index.html').read_text())
INSTALL = stamp((WEBSITE / 'install.html').read_text())


class Metadata(HTMLParser):
    def __init__(self, html):
        super().__init__()
        self.tags = []
        self.schema = []
        self.in_schema = False
        self.feed(html)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        self.tags.append((tag, attrs))
        self.in_schema = tag == 'script' and attrs.get('type') == 'application/ld+json'

    def handle_endtag(self, tag):
        if tag == 'script':
            self.in_schema = False

    def handle_data(self, data):
        if self.in_schema:
            self.schema.extend(json.loads(data)['@graph'])

    def meta(self, name):
        return next(a['content'] for t, a in self.tags
                    if t == 'meta' and a.get('name', a.get('property')) == name)


class WebsiteTests(unittest.TestCase):
    def test_portuguese_content_is_present_without_javascript(self):
        html = portuguese_page(SOURCE, ORIGIN)
        self.assertIn('<html lang="pt-BR">', html)
        self.assertIn('>Perguntas frequentes.</h2>', html)
        self.assertIn('>Assista ao YouTube no computador, de um jeito mais leve e rápido.</h1>', html)
        self.assertIn('>Baixar o Flatpak (recomendado)</a>', html)
        self.assertNotIn('>Common questions.</h2>', html)
        self.assertIn('aria-label="Links do projeto"', html)
        self.assertIn('src="../site.js"', html)
        self.assertIn('poster="../assets/player.webp"', html)
        install = portuguese_page(INSTALL, ORIGIN, 'install.html')
        self.assertIn('>Instale o YouTFast.</h1>', install)
        self.assertIn('<title>Instalar o YouTFast — Linux, Windows e macOS</title>', install)
        self.assertIn('href="../styles.css"', install)
        # Links between pages stay in Portuguese; the language switch leads
        # to the same page in English.
        self.assertIn('href="index.html#download"', install)
        self.assertIn('id="language" href="../install.html"', install)
        self.assertIn('href="install.html"', html)

    def test_downloads_link_to_the_current_release_assets(self):
        self.assertNotIn('{{', SOURCE + INSTALL)
        assets = [f'youtfast-v{VERSION}-x86_64.flatpak',
                  f'youtfast-v{VERSION}-x86_64-unknown-linux-gnu.tar.gz',
                  f'youtfast-{VERSION}-1-x86_64.pkg.tar.zst',
                  f'youtfast-v{VERSION}-x86_64-pc-windows-gnu.zip',
                  f'youtfast-v{VERSION}-aarch64-apple-darwin.zip', 'checksums.txt']
        for source in (SOURCE, INSTALL):
            for asset in assets:
                with self.subTest(asset=asset):
                    self.assertIn(f'/releases/download/v{VERSION}/{asset}', source)

    def test_download_urls_follow_the_release_workflow_naming(self):
        workflow = (Path(__file__).resolve().parents[1] / '.github/workflows/release.yml').read_text()
        names = {m.replace('${RELEASE_TAG#v}', VERSION).replace('$env:RELEASE_TAG', 'v' + VERSION)
                 .replace('$RELEASE_TAG', 'v' + VERSION)
                 for m in re.findall(r'youtfast-(?:\$\{RELEASE_TAG#v\}|\$env:RELEASE_TAG|\$RELEASE_TAG)[^"\s]*', workflow)}
        names = {'youtfast-' + n.split('youtfast-', 1)[1] if n.startswith('youtfast-') else n for n in names}
        # Links and the commands shown on the install page.
        urls = re.findall(r'releases/download/v([^/"\s]+)/([^"\s<]+)', SOURCE + INSTALL)
        self.assertTrue(urls)
        for version, asset in urls:
            self.assertEqual(version, VERSION)
            if asset != 'checksums.txt':
                self.assertIn(asset, names)
        self.assertIn('checksums.txt', workflow)

    def test_canonical_urls_and_language_alternates_match_each_page(self):
        for page, slug, source in [('index.html', '', SOURCE), ('install.html', 'install', INSTALL)]:
            for path, html in [('/' + slug, source),
                               ('/pt/' + slug, portuguese_page(source, ORIGIN, page))]:
                meta = Metadata(html)
                canonical = [a['href'] for t, a in meta.tags
                             if t == 'link' and a.get('rel') == 'canonical']
                self.assertEqual(canonical, [ORIGIN + path])
                self.assertEqual(meta.meta('og:url'), ORIGIN + path)
                alternates = {a['hreflang']: a['href'] for t, a in meta.tags
                              if t == 'link' and a.get('rel') == 'alternate'}
                self.assertEqual(alternates, {'en': f'{ORIGIN}/{slug}', 'pt-BR': f'{ORIGIN}/pt/{slug}',
                                              'x-default': f'{ORIGIN}/{slug}'})

    def test_structured_data_describes_the_real_desktop_app(self):
        for html in [SOURCE, portuguese_page(SOURCE, ORIGIN)]:
            meta = Metadata(html)
            app = next(x for x in meta.schema if x['@type'] == 'SoftwareApplication')
            self.assertEqual(app['name'], 'YouTFast')
            self.assertEqual(app['description'], meta.meta('description'))
            self.assertEqual(app['offers']['price'], '0')
            self.assertEqual(app['applicationCategory'], 'MultimediaApplication')
            self.assertIn('macOS 15+', app['operatingSystem'])
            self.assertNotIn('aggregateRating', app)
            self.assertNotIn('review', app)
            self.assertIn('/releases/latest', app['downloadUrl'])
            self.assertEqual(meta.meta('twitter:description'), app['description'])

    def test_all_localized_links_and_assets_resolve_inside_the_public_allowlist(self):
        validate_pages(render_pages())

    def test_a_link_into_a_missing_section_of_another_page_is_rejected(self):
        pages = render_pages()
        pages['install.html'] = pages['install.html'].replace(
            'href="index.html#download"', 'href="index.html#no-such-section"')
        with self.assertRaisesRegex(ValueError, 'Missing section: index.html#no-such-section'):
            validate_pages(pages)

    def test_translation_escapes_markup_but_preserves_inline_scripts(self):
        source = '<html><body><p data-pt="A &amp; B &lt;texto&gt;">Old</p><script>const x = 1 < 2;</script></body></html>'
        html = portuguese_page(source, ORIGIN)
        self.assertIn('>A &amp; B &lt;texto&gt;</p>', html)
        self.assertIn('const x = 1 < 2;', html)


if __name__ == '__main__':
    unittest.main()
