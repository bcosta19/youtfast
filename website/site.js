// The page and all its links also work without JavaScript.
const languageButton = document.querySelector('#language');
let language = 'en';
function setLanguage(next) {
  language = next;
  document.documentElement.lang = next === 'pt' ? 'pt-BR' : 'en';
  for (const element of document.querySelectorAll('[data-en][data-pt]')) {
    element.textContent = element.dataset[next].replaceAll('\\n', '\n');
  }
  languageButton.textContent = next === 'en' ? 'PT-BR' : 'EN';
  languageButton.lang = next === 'en' ? 'pt-BR' : 'en';
  languageButton.setAttribute('aria-label', next === 'en' ? 'Mudar idioma para português' : 'Switch language to English');
  document.querySelector('nav[aria-label]').setAttribute('aria-label', next === 'en' ? 'Main navigation' : 'Navegação principal');
  document.querySelector('.product-image').setAttribute('aria-label', next === 'en' ? 'Watch the YouTFast demo' : 'Assistir à demonstração do YouTFast');
  document.querySelector('.product-image img').alt = next === 'en'
    ? 'YouTFast playing Spring by Blender Studio, with the video library and recommendations in its native window.'
    : 'YouTFast reproduzindo Spring, do Blender Studio, com biblioteca e recomendações na janela nativa.';
  document.querySelector('video').setAttribute('aria-label', next === 'en' ? 'YouTFast product demonstration' : 'Demonstração do YouTFast');
}
languageButton.hidden = false;
languageButton.addEventListener('click', () => setLanguage(language === 'en' ? 'pt' : 'en'));
if (new URLSearchParams(window.location.search).get('lang') === 'pt') setLanguage('pt');
