<p align="center">
  <img src="../../../packaging/icons/youtfast.svg" width="96" height="96" alt="Icône de YouTFast">
</p>
<h1 align="center">YouTFast</h1>
<p align="center"><strong>YouTube, natif et rapide.</strong></p>
<p align="center">
  <a href="https://github.com/bcosta19/youtfast/actions/workflows/ci.yml"><img src="https://github.com/bcosta19/youtfast/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"></a>
  <a href="https://github.com/bcosta19/youtfast/releases/latest"><img src="https://img.shields.io/github/v/release/bcosta19/youtfast?sort=semver" alt="Dernière version"></a>
  <a href="https://github.com/bcosta19/youtfast/releases"><img src="https://img.shields.io/github/downloads/bcosta19/youtfast/total" alt="Téléchargements"></a>
  <a href="../../../LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--only-blue" alt="Licence : GPL-3.0-only"></a>
  <img src="https://img.shields.io/badge/rust-1.98%2B-orange?logo=rust" alt="Rust 1.98+">
  <img src="https://img.shields.io/badge/platforms-Linux%20%7C%20Windows%20%7C%20macOS-lightgrey" alt="Plateformes : Linux, Windows, macOS">
</p>
<p align="center">
  <a href="https://github.com/bcosta19/youtfast/releases">Versions</a> ·
  <a href="https://youtfast.pages.dev/">Site web</a> ·
  <a href="user-guide.md">Guide d'utilisation</a> ·
  <a href="../../../CHANGELOG.md">Journal des modifications</a> ·
  <a href="../../../CONTRIBUTING.md">Contribuer</a>
</p>
<p align="center">
  <a href="../../../README.md">English</a> ·
  <a href="../pt-BR/README.md">Português (Brasil)</a> ·
  <a href="../es/README.md">Español</a> ·
  <strong>Français</strong> ·
  <a href="../de/README.md">Deutsch</a> ·
  <a href="../it/README.md">Italiano</a>
</p>

*Ceci est une traduction du [document original en anglais](../../../README.md) ; en cas de divergence, la version anglaise fait foi.*

Un petit client YouTube natif pour ordinateur, conçu avec **Rust, egui et libmpv**.
Recherchez, regardez et gérez votre bibliothèque dans une interface de bureau,
avec le décodage vidéo matériel lorsqu'il est pris en charge. Pas de moteur de
navigateur, pas de télémétrie, pas de serveur exploité par YouTFast. Vous
démarrez sans être connecté ; les comptes sont facultatifs.

<picture>
  <source type="image/avif" srcset="../../../assets/demo/youtfast-demo.avif">
  <img src="../../../assets/demo/youtfast-demo.webp" alt="Démonstration de YouTFast 0.7.0 : recherche, onglets vidéo, lecture, aperçus sur la barre de progression, mode cinéma et plein écran, mini-lecteur, notifications, thèmes, six langues et raccourcis clavier">
</picture>

*YouTFast 0.7.0 sous Linux, sans être connecté : un véritable enregistrement
d'écran, monté avec des zooms et des légendes ; les temps de chargement sont
coupés et certaines navigations sont accélérées.
[Téléchargez-le en pleine qualité](../../../assets/demo/youtfast-demo.mp4) (MP4, 1080p, 62 secondes, 13,4 Mo, sans son).
Les films sont [Spring](https://studio.blender.org/films/spring/) et
[Sprite Fright](https://studio.blender.org/films/sprite-fright/) du Blender
Studio ([CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)).*

> [!NOTE]
> **YouTFast est jeune et a encore besoin de travail.** Il n'a qu'un seul
> mainteneur et est en développement actif `0.x` : il lit déjà les vidéos,
> effectue des recherches et gère une bibliothèque, mais attendez-vous à des
> imperfections et à des manques. Si quelque chose ne fonctionne pas ou semble
> moins abouti que YouTube dans le navigateur, c'est une excellente occasion
> d'aider. Voir [comment contribuer](#votre-aide-est-la-bienvenue).

## Ce que vous pouvez faire

| Fonctionnalité | Dans YouTFast |
|---|---|
| Regarder | Vidéo dans la fenêtre, plein écran, mode cinéma, mode audio seul et mini-lecteur compact |
| Découvrir | Filtres de recherche, onglets de chaîne, playlists, Shorts et flux paginés |
| Garder le contrôle | Sous-titres, chapitres, modification de la file d'attente, tampon vidéo configurable, raccourcis clavier et contrôles multimédias du bureau |
| Gérer une bibliothèque | Abonnements sur l'appareil, À regarder plus tard, historique/reprise, History sync (synchronisation de l'historique) et import depuis Google Takeout |
| Lire et explorer | Descriptions, commentaires et pièces jointes ; transcriptions et aperçus sur la barre de progression en option |
| Personnaliser | Six langues d'interface (EN, PT-BR, ES, FR, DE, IT), thèmes clair/sombre, palettes et intégration native à Omarchy sous Linux |

Les comptes facultatifs utilisent le gestionnaire d'identifiants du système. Les
évaluations et les modifications de playlists du compte s'appliquent au compte
sélectionné ; les abonnements et la liste À regarder plus tard de l'appareil
restent locaux. [En savoir plus sur les comptes et les données stockées](user-guide.md#votre-bibliothèque-avec-ou-sans-compte).

## Plus léger qu'un site web dans une fenêtre

Beaucoup d'« applications YouTube pour ordinateur » ne sont que le site web
dans Electron. Mesuré sur un même ordinateur portable Linux avec les mêmes
résultats de recherche (`"blender open movie"`) et la lecture en 1080p de
*Spring*, en plein écran, sans être connecté
([méthode, toutes les mesures et réserves](../../benchmarks.md)) :

| | YouTFast 0.7.0 | Electron 44 + youtube.com |
|---|---:|---:|
| Recherche : mémoire (PSS, tous les processus) | **136 Mio** | 662 Mio |
| Recherche : processeur au repos | **0,0 %** | 5,6 % d'un cœur |
| Lecture (1080p) : mémoire | **206 Mio** | 625 Mio |
| Lecture (1080p) : processeur | **13,9 %** | 41,2 % d'un cœur |
| Processus | **1** | 8 |
| Première fenêtre à l'écran | **0,5 s** | 0,7 s |
| Sur le disque | **29 Mo** (13 Mo à télécharger) | 316 Mo (301 Mio) pour le seul environnement d'exécution |

Les mesures provenant d'autres machines sont les bienvenues.

## Installation

**[Guide d'installation pas à pas](install.md)**, pour chaque système :
Flatpak, Arch, Ubuntu/Debian, Fedora, Windows et macOS, avec la mise à jour et
la désinstallation. Également sur le site web : <https://youtfast.pages.dev/install>.

Les versions publiées sont listées dans les GitHub Releases. Un changement de
numéro de version dans le dépôt ne signifie pas que ses paquets ont été publiés.
Les paquets sont compilés et testés sans interface graphique sur des machines
hébergées par GitHub ; les vérifications interactives sur des machines Windows
et macOS vierges sont encore en cours. Téléchargez depuis les
[GitHub Releases](https://github.com/bcosta19/youtfast/releases).

| Plateforme | Paquet | Prérequis |
|---|---|---|
| Linux x86_64 | Bundle Flatpak ou `install.sh` | Flatpak, environnement d'exécution Freedesktop 25.08 et pilotes/portails du bureau |
| Linux x86_64 | Archive native | glibc 2.39+, libmpv 0.42+ du système avec Lua et yt-dlp/Deno à jour ; mises à jour dans l'application prises en charge |
| Windows x86_64 | Installateur `.exe` (NSIS), `install.ps1` ou ZIP portable | yt-dlp/Deno à jour dans le PATH (l'installateur `.exe` installe ceux qui manquent via winget) ; mises à jour dans l'application prises en charge ; sans signature d'éditeur |
| macOS 15+ Apple Silicon | ZIP avec l'application et les dylibs | yt-dlp/Deno à jour ; mises à jour dans l'application prises en charge ; signature ad hoc, non notarié |
| Arch Linux x86_64 | Paquet pacman (`pacman -U` ou `install.sh`) et PKGBUILD source | Compilé et installé dans un conteneur Arch vierge par la CI ; inscription sur l'AUR actuellement fermée |

Les archives et bundles natifs incluent un outil de mise à jour intégré dans
**Paramètres → À propos** (vérification SHA-256 par rapport à `checksums.txt`,
avec mises à jour mineures/correctives automatiques en option et mises à jour
majeures manuelles en un clic). Les builds de publication génèrent aussi les
manifestes Flathub, Winget et Homebrew Cask prêts à l'emploi (`channels.tar.gz`).
Le Flatpak embarque ses outils de lecture et d'extraction et cible les
distributions Linux compatibles en x86_64. Il n'est pas encore publié sur
Flathub ni proposé via un dépôt de mises à jour. La recette Arch n'est pas encore
une entrée de l'AUR ni un paquet pacman officiel.

[Détails des plateformes, sommes de contrôle et limites](../../distribution.md) ·
[Maintenance du paquet Arch](../../../packaging/arch/README.md)

## Compiler depuis les sources

Installez le compilateur et les dépendances système indiqués dans
[CONTRIBUTING.md](../../../CONTRIBUTING.md#getting-started), puis lancez depuis le dépôt cloné :

```sh
cargo run --locked --release
cargo run --locked --release -- "https://youtu.be/dQw4w9WgXcQ"
cargo run --locked --release -- "rust egui"
```

La lecture nécessite **libmpv 0.42 ou plus récent avec la prise en charge de
Lua**, un yt-dlp à jour et l'environnement d'exécution EJS/JS correspondant. Les
versions plus anciennes de mpv peuvent ouvrir les URL de flux avant qu'elles ne
soient disponibles, ce qui provoque des échecs HTTP 403 intermittents. L'import
facultatif de la session du navigateur nécessite aussi `python3` avec le module
`yt_dlp`. Voir les [prérequis](user-guide.md#prérequis).

**Diffuser sur la TV** prend en charge les TV YouTube via DIAL/Lounge en natif,
y compris les TV Samsung compatibles sans Chromecast. La prise en charge de
Google Cast utilise en plus un exécutable
[`catt`](https://github.com/skorokithakis/catt) installé séparément dans le
`PATH`. Le bouton à côté de Audio seul dans le lecteur, ou dans la barre
supérieure, ouvre le sélecteur de TV et la télécommande. Une TV peut aussi être
trouvée par son adresse IP locale. Voir la
[diffusion sur la TV](user-guide.md#diffuser-sur-la-tv) pour la configuration,
les appareils pris en charge et les limites.

## Votre aide est la bienvenue

YouTFast s'améliore à chaque rapport de bug, chaque test et chaque pull request,
et il n'est pas nécessaire de connaître Rust pour aider :

- **Essayez-le et dites-nous ce qui ne marche pas.** Surtout sous Windows et
  macOS, qui n'ont été testés que par la CI jusqu'ici. Les étapes pour
  reproduire le problème sont ce qui aide le plus.
- **Choisissez une issue.** Cherchez les
  [good first issues](https://github.com/bcosta19/youtfast/labels/good%20first%20issue)
  et [help wanted](https://github.com/bcosta19/youtfast/labels/help%20wanted).
- **Traduisez.** L'anglais, le portugais du Brésil, l'espagnol, le français, l'allemand et l'italien existent ; d'autres langues sont les bienvenues.
- **Empaquetez-le.** Toute aide pour Winget, le tap Homebrew et l'empaquetage pour les distributions est la bienvenue.
- **Mesurez-le.** Lancez les [benchmarks](../../benchmarks.md) sur votre machine.

Commencez par [CONTRIBUTING.md](../../../CONTRIBUTING.md) ; les questions dans les issues sont les bienvenues.

## Guides et état du projet

| Vous voulez… | Lisez |
|---|---|
| Découvrir les commandes, la bibliothèque, les comptes et les thèmes | [Guide d'utilisation](user-guide.md) |
| Installer, mettre à jour ou désinstaller YouTFast | [Guide d'installation](install.md) |
| Choisir un paquet ou vérifier la couverture des plateformes | [Distribution](../../distribution.md) |
| Voir le contenu et les exclusions de chaque version | [Journal des modifications](../../../CHANGELOG.md) |
| Voir ce qui n'est pas encore implémenté | [Feuille de route](../../../ROADMAP.md) |
| Signaler un bug ou contribuer | [Contribuer](../../../CONTRIBUTING.md) |
| Signaler un problème de sécurité en privé | [Politique de sécurité](../../../SECURITY.md) |
| Comparer l'utilisation des ressources | [Benchmarks](../../benchmarks.md) |
| Préparer une version | [Processus de publication](../../../RELEASING.md) |

La bibliothèque et les paramètres restent sur cet appareil. Les cookies du
compte sont conservés dans le gestionnaire d'identifiants natif, et les
diagnostics locaux masquent les informations privées. L'accès au compte n'est
pas officiel et peut déclencher une vérification Google, une déconnexion ou une
suspension. Utilisez l'application sans être connecté, sauf si vous choisissez
délibérément cette intégration.
[Stockage, confidentialité et détails sur les comptes](user-guide.md#votre-bibliothèque-avec-ou-sans-compte).

## Projets liés et remerciements

- [Spotifast](https://github.com/crmne/spotifast) — client Spotify natif.
- [ZapFast](https://github.com/crmne/zapfast) — client WhatsApp natif.
- [fastframe](https://github.com/crmne/fastframe) — fondations partagées pour
  les applications de bureau egui natives.

YouTFast suit les conventions des applications de bureau natives de ces projets
et utilise des forks partagés d'egui/winit. Merci également à [mpv](https://mpv.io),
[yt-dlp](https://github.com/yt-dlp/yt-dlp),
[egui](https://github.com/emilk/egui) et [Lucide](https://lucide.dev).
Le client InnerTube de YouTFast est développé dans ce dépôt ; les versions
précédentes utilisaient [RustyPipe](https://codeberg.org/ThetaDev/rustypipe).

## Licence et avertissement

[GPL-3.0-only](../../../LICENSE). Les dépendances conservent leurs propres
licences et mentions ; voir [THIRD_PARTY_NOTICES.md](../../../THIRD_PARTY_NOTICES.md).

YouTFast est un client non officiel, sans lien avec YouTube ni Google.
L'utilisation d'un client non officiel peut contrevenir aux conditions
d'utilisation de YouTube. Utilisez-le à vos propres risques.
