// The page and all its links also work without JavaScript.
const languageButton = document.querySelector('#language');
const themeControl = document.querySelector('#theme-control');
const themeSelect = document.querySelector('#theme');
const systemTheme = window.matchMedia('(prefers-color-scheme: dark)');
let language = document.documentElement.lang === 'pt-BR' ? 'pt' : 'en';
let themePreference = document.documentElement.dataset.theme || 'system';

function applyTheme(preference, persist = true) {
  themePreference = preference;
  if (preference === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = preference;
  const resolved = preference === 'system' ? (systemTheme.matches ? 'dark' : 'light') : preference;
  for (const meta of document.querySelectorAll('meta[name="theme-color"]')) {
    meta.content = resolved === 'dark' ? '#1c1727' : '#f9f7ff';
  }
  themeSelect.value = preference;
  themeSelect.setAttribute('aria-label', language === 'en' ? 'Color theme' : 'Tema de cores');
  if (persist) {
    try {
      if (preference === 'system') localStorage.removeItem('youtfast-theme');
      else localStorage.setItem('youtfast-theme', preference);
    } catch {}
  }
}

function setLanguage(next) {
  language = next;
  document.documentElement.lang = next === 'pt' ? 'pt-BR' : 'en';
  for (const element of document.querySelectorAll('[data-en][data-pt]')) {
    element.textContent = element.dataset[next].replaceAll('\\n', '\n');
  }
  languageButton.textContent = next === 'en' ? 'PT-BR' : 'EN';
  languageButton.href = next === 'en' ? 'pt/' : (window.location.pathname.includes('/pt/') ? '../' : './');
  languageButton.lang = next === 'en' ? 'pt-BR' : 'en';
  languageButton.setAttribute('aria-label', next === 'en' ? 'Mudar idioma para português' : 'Switch language to English');
  document.querySelector('nav[aria-label]').setAttribute('aria-label', next === 'en' ? 'Main navigation' : 'Navegação principal');
  document.querySelector('.footer nav').setAttribute('aria-label', next === 'en' ? 'Project links' : 'Links do projeto');
  document.querySelector('.product-image').setAttribute('aria-label', next === 'en' ? 'Watch the YouTFast demo' : 'Assistir à demonstração do YouTFast');
  document.querySelector('.product-image img').alt = next === 'en'
    ? 'YouTFast playing Spring by Blender Studio, with the video library and recommendations in its native window.'
    : 'YouTFast reproduzindo Spring, do Blender Studio, com biblioteca e recomendações na janela nativa.';
  document.querySelector('video').setAttribute('aria-label', next === 'en' ? 'YouTFast product demonstration' : 'Demonstração do YouTFast');
  applyTheme(themePreference, false);
}
// Point the main button at the visitor's platform. Without JavaScript it keeps
// pointing at the whole download section.
const osLabels = {
  windows: ['Download for Windows (free)', 'Baixar para Windows (grátis)'],
  macos: ['Download for macOS (free)', 'Baixar para macOS (grátis)'],
  linux: ['Download for Linux (free)', 'Baixar para Linux (grátis)'],
};

function detectPlatform() {
  const platform = (navigator.userAgentData?.platform || navigator.platform || '').toLowerCase();
  const agent = navigator.userAgent.toLowerCase();
  if (/android|iphone|ipad|ipod/.test(agent) || navigator.maxTouchPoints > 1 && /mac/.test(platform)) return null;
  if (/win/.test(platform) || /windows/.test(agent)) return 'windows';
  if (/mac/.test(platform) || /macintosh/.test(agent)) return 'macos';
  if (/linux|x11|cros/.test(platform + agent)) return 'linux';
  return null;
}

const detected = detectPlatform();
if (detected) {
  const button = document.querySelector('#os-download');
  const label = button.querySelector('span');
  [label.dataset.en, label.dataset.pt] = osLabels[detected];
  button.href = `#download-${detected}`;
  const card = document.querySelector(`#download-${detected}`);
  card.classList.add('is-detected');
  card.querySelector('.recommended').hidden = false;
}

themeControl.hidden = false;
languageButton.hidden = false;
themeSelect.addEventListener('change', () => applyTheme(themeSelect.value));
systemTheme.addEventListener('change', () => {
  if (themePreference === 'system') applyTheme('system', false);
});
setLanguage(new URLSearchParams(window.location.search).get('lang') === 'pt' ? 'pt' : language);
