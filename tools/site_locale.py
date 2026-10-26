"""Render the Portuguese landing page as crawlable HTML, without browser JavaScript."""
from html import escape
from html.parser import HTMLParser
import json
from urllib.parse import urlsplit

TITLE = 'YouTFast — Cliente YouTube nativo para desktop'
DESCRIPTION = ('YouTFast é um cliente YouTube gratuito e de código aberto para Linux, '
               'Windows e macOS. Reprodução nativa, biblioteca local e sem telemetria.')
LABELS = {
    'Skip to content': 'Pular para o conteúdo',
    'YouTFast desktop application playing a video with its local library and recommendations.':
        'Aplicativo YouTFast reproduzindo um vídeo com biblioteca local e recomendações.',
    'Main navigation': 'Navegação principal',
    'Project links': 'Links do projeto',
    'Watch the YouTFast demo': 'Assistir à demonstração do YouTFast',
    'YouTFast product demonstration': 'Demonstração do YouTFast',
    'YouTFast playing Spring by Blender Studio, with the video library and recommendations in its native window.':
        'YouTFast reproduzindo Spring, do Blender Studio, com biblioteca e recomendações na janela nativa.',
}
VOID = {'area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta',
        'param', 'source', 'track', 'wbr'}


class PortuguesePage(HTMLParser):
    def __init__(self, origin):
        super().__init__(convert_charrefs=True)
        self.origin = origin
        self.output = []
        self.stack = []

    def handle_decl(self, decl):
        self.output.append(f'<!{decl}>')

    def handle_comment(self, data):
        self.output.append(f'<!--{data}-->')

    def handle_starttag(self, tag, pairs):
        attrs = dict(pairs)
        text = attrs.get('data-pt')
        if tag == 'html':
            attrs['lang'] = 'pt-BR'
        if tag == 'title':
            text = TITLE
        for key in ('aria-label', 'alt', 'content'):
            if key in attrs:
                attrs[key] = LABELS.get(attrs[key], attrs[key])
        if tag == 'meta':
            field = attrs.get('property', attrs.get('name'))
            if field in ('description', 'og:description', 'twitter:description'):
                attrs['content'] = DESCRIPTION
            elif field in ('og:title', 'twitter:title'):
                attrs['content'] = TITLE
            elif field == 'og:url':
                attrs['content'] = self.origin + '/pt/'
            elif field == 'og:locale':
                attrs['content'] = 'pt_BR'
            elif field == 'og:locale:alternate':
                attrs['content'] = 'en_US'
        if tag == 'link' and attrs.get('rel') == 'canonical':
            attrs['href'] = self.origin + '/pt/'
        for key in ('href', 'src', 'poster'):
            value = attrs.get(key, '')
            url = urlsplit(value)
            if url.path and not url.scheme and not url.netloc:
                attrs[key] = '../' + value
        if attrs.get('id') == 'language':
            attrs.update(href='../', lang='en', **{'aria-label': 'Switch language to English'})
            text = 'EN'
        if attrs.get('id') == 'theme':
            attrs['aria-label'] = 'Tema de cores'
        attributes = ''.join(' ' + k if v is None else f' {k}="{escape(v, quote=True)}"'
                             for k, v in attrs.items())
        self.output.append(f'<{tag}{attributes}>')
        if tag not in VOID:
            self.stack.append((tag, text, attrs.get('type') == 'application/ld+json'))
            if text is not None:
                self.output.append(escape(text.replace('\\n', '\n')))

    def handle_endtag(self, tag):
        if self.stack:
            opened, _, _ = self.stack.pop()
            if opened != tag:
                raise ValueError(f'Unbalanced HTML: {opened} / {tag}')
        self.output.append(f'</{tag}>')

    def handle_data(self, data):
        if self.stack and self.stack[-1][1] is not None:
            return
        if self.stack and self.stack[-1][2]:
            schema = json.loads(data)
            for item in schema['@graph']:
                if item['@type'] == 'SoftwareApplication':
                    item['description'] = DESCRIPTION
                if item['@type'] == 'WebPage':
                    item.update(url=self.origin + '/pt/', name=TITLE, inLanguage='pt-BR')
                    item['@id'] = self.origin + '/pt/#webpage'
            self.output.append(json.dumps(schema, ensure_ascii=False, indent=2))
        elif self.stack and self.stack[-1][0] in ('script', 'style'):
            self.output.append(data)
        else:
            self.output.append(escape(data, quote=False))


def portuguese_page(source, origin):
    parser = PortuguesePage(origin)
    parser.feed(source)
    parser.close()
    if parser.stack:
        raise ValueError('Unclosed HTML element')
    return ''.join(parser.output)
