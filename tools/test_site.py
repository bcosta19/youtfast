"""Regression checks for crawlable localized product information."""
from html.parser import HTMLParser
from pathlib import Path
import json
import unittest

from build_site import ORIGIN, Page
from site_locale import portuguese_page

SOURCE = (Path(__file__).resolve().parents[1] / 'website/index.html').read_text()


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
        self.assertIn('>Um cliente YouTube nativo para o seu desktop.</h1>', html)
        self.assertIn('>Ver pacotes para Linux ↗</a>', html)
        self.assertNotIn('>Common questions.</h2>', html)
        self.assertIn('aria-label="Links do projeto"', html)
        self.assertIn('src="../site.js"', html)
        self.assertIn('poster="../assets/player.webp"', html)

    def test_canonical_urls_and_language_alternates_match_each_page(self):
        for path, html in [('/', SOURCE), ('/pt/', portuguese_page(SOURCE, ORIGIN))]:
            meta = Metadata(html)
            canonical = [a['href'] for t, a in meta.tags
                         if t == 'link' and a.get('rel') == 'canonical']
            self.assertEqual(canonical, [ORIGIN + path])
            self.assertEqual(meta.meta('og:url'), ORIGIN + path)
            alternates = {a['hreflang']: a['href'] for t, a in meta.tags
                          if t == 'link' and a.get('rel') == 'alternate'}
            self.assertEqual(alternates, {'en': ORIGIN + '/', 'pt-BR': ORIGIN + '/pt/',
                                          'x-default': ORIGIN + '/'})

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
        for name, html in [('index.html', SOURCE),
                           ('pt/index.html', portuguese_page(SOURCE, ORIGIN))]:
            page = Page(name)
            page.feed(html)
            page.validate()

    def test_translation_escapes_markup_but_preserves_inline_scripts(self):
        source = '<html><body><p data-pt="A &amp; B &lt;texto&gt;">Old</p><script>const x = 1 < 2;</script></body></html>'
        html = portuguese_page(source, ORIGIN)
        self.assertIn('>A &amp; B &lt;texto&gt;</p>', html)
        self.assertIn('const x = 1 < 2;', html)


if __name__ == '__main__':
    unittest.main()
