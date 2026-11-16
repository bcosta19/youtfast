<p align="center">
  <img src="../../../packaging/icons/youtfast.svg" width="96" height="96" alt="YouTFast-Symbol">
</p>
<h1 align="center">YouTFast</h1>
<p align="center"><strong>YouTube, nativ und schnell.</strong></p>
<p align="center">
  <a href="https://github.com/bcosta19/youtfast/actions/workflows/ci.yml"><img src="https://github.com/bcosta19/youtfast/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"></a>
  <a href="https://github.com/bcosta19/youtfast/releases/latest"><img src="https://img.shields.io/github/v/release/bcosta19/youtfast?sort=semver" alt="Neueste Version"></a>
  <a href="https://github.com/bcosta19/youtfast/releases"><img src="https://img.shields.io/github/downloads/bcosta19/youtfast/total" alt="Downloads"></a>
  <a href="../../../LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--only-blue" alt="Lizenz: GPL-3.0-only"></a>
  <img src="https://img.shields.io/badge/rust-1.98%2B-orange?logo=rust" alt="Rust 1.98+">
  <img src="https://img.shields.io/badge/platforms-Linux%20%7C%20Windows%20%7C%20macOS-lightgrey" alt="Plattformen: Linux, Windows, macOS">
</p>
<p align="center">
  <a href="https://github.com/bcosta19/youtfast/releases">Versionen</a> ·
  <a href="https://youtfast.pages.dev/">Website</a> ·
  <a href="user-guide.md">Benutzerhandbuch</a> ·
  <a href="../../../CHANGELOG.md">Änderungsprotokoll</a> ·
  <a href="../../../CONTRIBUTING.md">Mitwirken</a>
</p>
<p align="center">
  <a href="../../../README.md">English</a> ·
  <a href="../pt-BR/README.md">Português (Brasil)</a> ·
  <a href="../es/README.md">Español</a> ·
  <a href="../fr/README.md">Français</a> ·
  <strong>Deutsch</strong> ·
  <a href="../it/README.md">Italiano</a>
</p>

*Dies ist eine Übersetzung des [englischen Originals](../../../README.md). Bei Abweichungen gilt die englische Fassung.*

Ein kleiner nativer YouTube-Desktop-Client, entwickelt mit **Rust, egui und libmpv**.
Suchen, ansehen und die eigene Mediathek pflegen – in einer Desktop-Oberfläche, mit
Hardware-Videodekodierung, wo sie unterstützt wird. Keine Browser-Engine, keine
Telemetrie und kein von YouTFast betriebenes Backend. Sie starten abgemeldet;
Konten sind optional.

<picture>
  <source type="image/avif" srcset="../../../assets/demo/youtfast-demo.avif">
  <img src="../../../assets/demo/youtfast-demo.webp" alt="Demo von YouTFast 0.7.0: Suche, Video-Tabs, Wiedergabe, Vorschaubilder in der Zeitleiste, Kinomodus und Vollbild, Miniplayer, Benachrichtigungen, Designs, sechs Sprachen und Tastaturkürzel">
</picture>

*YouTFast 0.7.0 unter Linux, abgemeldet: eine echte Bildschirmaufnahme, bearbeitet
mit Zooms und Beschriftungen; Ladezeiten sind herausgeschnitten und einige
Navigationsschritte beschleunigt.
[In voller Qualität herunterladen](../../../assets/demo/youtfast-demo.mp4) (MP4, 1080p, 62 Sekunden, 13,4 MB, ohne Ton).
Die Filme sind [Spring](https://studio.blender.org/films/spring/) und
[Sprite Fright](https://studio.blender.org/films/sprite-fright/) vom Blender
Studio ([CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)).*

> [!NOTE]
> **YouTFast ist jung und braucht noch Arbeit.** Es hat einen einzigen Maintainer
> und befindet sich in aktiver `0.x`-Entwicklung: Es spielt bereits ab, sucht und
> verwaltet eine Mediathek, aber rechnen Sie mit Ecken, Kanten und fehlenden
> Teilen. Wenn etwas kaputtgeht oder sich weniger ausgereift anfühlt als YouTube im
> Browser, ist das ein guter Ansatzpunkt zum Helfen. Siehe
> [wie Sie mitwirken können](#hilfe-gesucht).

## Was Sie tun können

| Funktion | In YouTFast |
|---|---|
| Ansehen | Video im Fenster, Vollbild, Kinomodus, Nur-Audio-Modus und ein kompakter Miniplayer |
| Entdecken | Suchfilter, Kanal-Tabs, Playlists, Shorts und seitenweise geladene Feeds |
| Die Kontrolle behalten | Untertitel, Kapitel, bearbeitbare Warteschlange, einstellbarer Videopuffer, Tastaturkürzel und Desktop-Mediensteuerung |
| Eine Mediathek führen | Abos auf dem Gerät, Später ansehen, Verlauf/Fortsetzen, History sync (Verlaufssynchronisierung) und Import aus Google Takeout |
| Lesen und erkunden | Beschreibungen, Kommentare und Anhänge; optionale Transkripte und Vorschaubilder in der Zeitleiste |
| Nach Ihrem Geschmack | Sechs Oberflächensprachen (EN, PT-BR, ES, FR, DE, IT), helle/dunkle Designs, Farbpaletten und native Omarchy-Integration unter Linux |

Optionale Konten nutzen den Anmeldedatenspeicher des Systems. Bewertungen und
Playlist-Änderungen wirken sich auf das ausgewählte Konto aus; Abos und „Später
ansehen“ auf dem Gerät bleiben lokal.
[Mehr über Konten und gespeicherte Daten](user-guide.md#ihre-mediathek-mit-oder-ohne-konto).

## Leichter als eine Website im Fenster

Viele „YouTube-Desktop-Apps“ sind die Website in Electron. Gemessen auf einem
Linux-Laptop mit denselben Suchergebnissen (`"blender open movie"`) und
1080p-Wiedergabe von *Spring* im Vollbild, abgemeldet
([Methode, alle Durchläufe und Einschränkungen](../../benchmarks.md)):

| | YouTFast 0.7.0 | Electron 44 + youtube.com |
|---|---:|---:|
| Suche: Speicher (PSS, alle Prozesse) | **136 MiB** | 662 MiB |
| Suche: CPU im Leerlauf | **0,0 %** | 5,6 % eines Kerns |
| Wiedergabe (1080p): Speicher | **206 MiB** | 625 MiB |
| Wiedergabe (1080p): CPU | **13,9 %** | 41,2 % eines Kerns |
| Prozesse | **1** | 8 |
| Erstes Fenster auf dem Bildschirm | **0,5 s** | 0,7 s |
| Auf dem Datenträger | **29 MB** (13 MB Download) | 316 MB (301 MiB) allein die Laufzeitumgebung |

Messwerte von anderen Rechnern sind willkommen.

## Installation

**[Schritt-für-Schritt-Installationsanleitung](install.md)** für jedes System:
Flatpak, Arch, Ubuntu/Debian, Fedora, Windows und macOS, inklusive Aktualisieren
und Entfernen. Auch auf der Website: <https://youtfast.pages.dev/install>.

Veröffentlichte Versionen sind unter GitHub Releases aufgeführt. Eine
Versionserhöhung im Repository bedeutet nicht, dass die zugehörigen Pakete
bereits veröffentlicht wurden. Pakete werden ohne Bildschirm auf von GitHub
gehosteten Runnern gebaut und getestet; interaktive Prüfungen auf sauberen
Windows- und macOS-Rechnern laufen noch. Download über
[GitHub Releases](https://github.com/bcosta19/youtfast/releases).

| Plattform | Paket | Voraussetzungen |
|---|---|---|
| Linux x86_64 | Flatpak-Bundle oder `install.sh` | Flatpak, Freedesktop-25.08-Laufzeitumgebung sowie Desktop-Treiber/-Portale |
| Linux x86_64 | Natives Archiv | glibc 2.39+, System-libmpv 0.42+ mit Lua und aktuelles yt-dlp/Deno; unterstützt Updates in der App |
| Windows x86_64 | Setup-`.exe` (NSIS), `install.ps1` oder portables ZIP | Aktuelles yt-dlp/Deno im PATH (die Setup-`.exe` installiert fehlende über winget); unterstützt Updates in der App; keine Herausgebersignatur |
| macOS 15+ Apple Silicon | ZIP mit App und dylibs | Aktuelles yt-dlp/Deno; unterstützt Updates in der App; ad hoc signiert, nicht notarisiert |
| Arch Linux x86_64 | Pacman-Paket (`pacman -U` oder `install.sh`) und Quell-PKGBUILD | Von der CI in einem sauberen Arch-Container gebaut und installiert; AUR-Registrierung derzeit geschlossen |

Native Archive/Bundles enthalten einen Updater in der App unter
**Einstellungen → Kanalinfo** (SHA-256-Prüfung gegen `checksums.txt`, mit
optionalen automatischen Minor-/Patch-Updates und manuellen Hauptversions-Updates
mit einem Klick). Release-Builds erzeugen außerdem fertige Manifeste für
Flathub, Winget und Homebrew Cask (`channels.tar.gz`).
Das Flatpak bündelt seine Wiedergabe-/Extraktionswerkzeuge und richtet sich an
kompatible Linux-Distributionen auf x86_64. Es ist noch kein Flathub-Eintrag und
keine Update-Quelle für die App. Das Arch-Rezept ist noch kein AUR-Eintrag und
kein offizielles pacman-Paket.

[Plattformdetails, Prüfsummen und Grenzen](../../distribution.md) ·
[Pflege des Arch-Pakets](../../../packaging/arch/README.md)

## Aus dem Quellcode bauen

Installieren Sie den Compiler und die Systemabhängigkeiten laut
[CONTRIBUTING.md](../../../CONTRIBUTING.md#getting-started) und führen Sie dann im
ausgecheckten Repository aus:

```sh
cargo run --locked --release
cargo run --locked --release -- "https://youtu.be/dQw4w9WgXcQ"
cargo run --locked --release -- "rust egui"
```

Die Wiedergabe benötigt **libmpv 0.42 oder neuer mit Lua-Unterstützung**, ein
aktuelles yt-dlp und die passende EJS/JS-Laufzeitumgebung. Ältere mpv-Versionen
können Stream-URLs öffnen, bevor sie verfügbar sind, was zu sporadischen
HTTP-403-Fehlern führt. Der optionale Import einer Browsersitzung benötigt
außerdem `python3` mit dem Modul `yt_dlp`. Siehe die
[Voraussetzungen](user-guide.md#voraussetzungen).

**Auf TV übertragen** unterstützt YouTube-TVs über natives DIAL/Lounge,
einschließlich kompatibler Samsung-TVs ohne Chromecast. Die Unterstützung für
Google Cast nutzt zusätzlich ein separat installiertes
[`catt`](https://github.com/skorokithakis/catt)-Programm im `PATH`. Die
Schaltfläche neben „Nur Audio“ im Player oder in der oberen Leiste öffnet die
TV-Auswahl und die Fernbedienung. Ein TV kann auch über seine lokale IP gefunden
werden. Einrichtung, unterstützte Ziele und Grenzen finden Sie unter
[Auf TV übertragen](user-guide.md#auf-tv-übertragen).

## Hilfe gesucht

YouTFast wird mit jedem Fehlerbericht, jedem Testlauf und jedem Pull-Request
besser, und Sie müssen kein Rust können, um zu helfen:

- **Ausprobieren und melden, was nicht funktioniert.** Besonders unter Windows
  und macOS, die bisher nur von der CI getestet wurden. Schritte zum
  Reproduzieren helfen am meisten.
- **Ein Issue übernehmen.** Suchen Sie nach
  [good first issues](https://github.com/bcosta19/youtfast/labels/good%20first%20issue)
  und [help wanted](https://github.com/bcosta19/youtfast/labels/help%20wanted).
- **Übersetzen.** Englisch, brasilianisches Portugiesisch, Spanisch, Französisch, Deutsch und Italienisch gibt es bereits; weitere Sprachen sind willkommen.
- **Paketieren.** Hilfe bei Winget, einem Homebrew-Tap und Distributionspaketen ist willkommen.
- **Messen.** Führen Sie die [Benchmarks](../../benchmarks.md) auf Ihrem Rechner aus.

Beginnen Sie mit [CONTRIBUTING.md](../../../CONTRIBUTING.md); Fragen in Issues sind willkommen.

## Anleitungen und Projektstatus

| Sie möchten… | Lesen Sie |
|---|---|
| Steuerung, Mediathek, Konten und Designs kennenlernen | [Benutzerhandbuch](user-guide.md) |
| YouTFast installieren, aktualisieren oder entfernen | [Installationsanleitung](install.md) |
| Ein Paket wählen oder die Plattformabdeckung prüfen | [Distribution](../../distribution.md) |
| Inhalte und Ausschlüsse einer Version sehen | [Änderungsprotokoll](../../../CHANGELOG.md) |
| Noch nicht umgesetzte Arbeit sehen | [Roadmap](../../../ROADMAP.md) |
| Einen Fehler melden oder mitwirken | [Mitwirken](../../../CONTRIBUTING.md) |
| Ein Sicherheitsproblem vertraulich melden | [Sicherheitsrichtlinie](../../../SECURITY.md) |
| Den Ressourcenverbrauch vergleichen | [Benchmarks](../../benchmarks.md) |
| Eine Version vorbereiten | [Release-Prozess](../../../RELEASING.md) |

Mediathek und Einstellungen bleiben auf diesem Gerät. Konto-Cookies gehören in
den nativen Anmeldedatenspeicher, und lokale Diagnosedaten schwärzen private
Details. Der Kontozugriff ist inoffiziell und kann eine Google-Verifizierung,
eine Abmeldung oder eine Sperrung auslösen. Nutzen Sie die App abgemeldet, es
sei denn, Sie entscheiden sich bewusst für diese Integration.
[Speicherung, Datenschutz und Kontodetails](user-guide.md#ihre-mediathek-mit-oder-ohne-konto).

## Verwandte Projekte und Danksagungen

- [Spotifast](https://github.com/crmne/spotifast) — nativer Spotify-Client.
- [ZapFast](https://github.com/crmne/zapfast) — nativer WhatsApp-Client.
- [fastframe](https://github.com/crmne/fastframe) — gemeinsame Grundlagen für
  native egui-Desktopanwendungen.

YouTFast folgt den nativen Desktop-Konventionen dieser Projekte und nutzt
gemeinsame Forks von egui/winit. Dank auch an [mpv](https://mpv.io),
[yt-dlp](https://github.com/yt-dlp/yt-dlp),
[egui](https://github.com/emilk/egui) und [Lucide](https://lucide.dev).
Der InnerTube-Client von YouTFast wird in diesem Repository entwickelt; frühere
Versionen nutzten [RustyPipe](https://codeberg.org/ThetaDev/rustypipe).

## Lizenz und Haftungsausschluss

[GPL-3.0-only](../../../LICENSE). Abhängigkeiten behalten ihre eigenen Lizenzen
und Hinweise; siehe [THIRD_PARTY_NOTICES.md](../../../THIRD_PARTY_NOTICES.md).

YouTFast ist ein inoffizieller Client und steht in keiner Verbindung zu YouTube
oder Google. Die Nutzung eines inoffiziellen Clients kann gegen die
Nutzungsbedingungen von YouTube verstoßen. Die Nutzung erfolgt auf eigenes
Risiko.
