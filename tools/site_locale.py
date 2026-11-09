"""Render the Portuguese site pages as crawlable HTML, without browser JavaScript."""
from html import escape
from html.parser import HTMLParser
import json
from urllib.parse import urlsplit

# Each page: its public path (Cloudflare Pages serves `install.html` as
# `/install`), and its Portuguese title and description.
PAGES = {
    'index.html': ('', 'YouTFast — Cliente YouTube nativo para desktop',
                   'YouTFast é um cliente YouTube gratuito e de código aberto para Linux, '
                   'Windows e macOS. Reprodução nativa, biblioteca local e sem telemetria.'),
    'install.html': ('install', 'Instalar o YouTFast — Linux, Windows e macOS',
                     'Passo a passo para instalar, atualizar e remover o YouTFast no Linux '
                     '(Flatpak, Arch, Ubuntu, Debian, Fedora), no Windows e no macOS.'),
}
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
    def __init__(self, origin, page):
        super().__init__(convert_charrefs=True)
        self.origin = origin
        self.page = page
        self.slug, self.title, self.description = PAGES[page]
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
            text = self.title
        for key in ('aria-label', 'alt', 'content'):
            if key in attrs:
                attrs[key] = LABELS.get(attrs[key], attrs[key])
        if tag == 'meta':
            field = attrs.get('property', attrs.get('name'))
            if field in ('description', 'og:description', 'twitter:description'):
                attrs['content'] = self.description
            elif field in ('og:title', 'twitter:title'):
                attrs['content'] = self.title
            elif field == 'og:url':
                attrs['content'] = page_url(self.origin, self.page, True)
            elif field == 'og:locale':
                attrs['content'] = 'pt_BR'
            elif field == 'og:locale:alternate':
                attrs['content'] = 'en_US'
        if tag == 'link' and attrs.get('rel') == 'canonical':
            attrs['href'] = page_url(self.origin, self.page, True)
        for key in ('href', 'src', 'poster'):
            value = attrs.get(key, '')
            url = urlsplit(value)
            # Links between pages stay in the same language's folder.
            if url.path and not url.scheme and not url.netloc and not is_page(url.path):
                attrs[key] = '../' + value
        if attrs.get('id') == 'language':
            attrs.update(href='../' + ('' if self.page == 'index.html' else self.page), lang='en',
                         **{'aria-label': 'Switch language to English'})
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
                    item['description'] = self.description
                if item['@type'] == 'WebPage':
                    url = page_url(self.origin, self.page, True)
                    item.update(url=url, name=self.title, inLanguage='pt-BR')
                    item['@id'] = url + '#webpage'
            self.output.append(json.dumps(schema, ensure_ascii=False, indent=2))
        elif self.stack and self.stack[-1][0] in ('script', 'style'):
            self.output.append(data)
        else:
            self.output.append(escape(data, quote=False))


def is_page(path):
    return path in ('./', 'index.html') or path in PAGES


def page_url(origin, page, portuguese):
    return f"{origin}/{'pt/' if portuguese else ''}{PAGES[page][0]}"


def portuguese_page(source, origin, page='index.html'):
    parser = PortuguesePage(origin, page)
    parser.feed(source)
    parser.close()
    if parser.stack:
        raise ValueError('Unclosed HTML element')
    return ''.join(parser.output)
