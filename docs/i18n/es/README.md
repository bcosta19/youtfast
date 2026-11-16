<p align="center">
  <img src="../../../packaging/icons/youtfast.svg" width="96" height="96" alt="Icono de YouTFast">
</p>
<h1 align="center">YouTFast</h1>
<p align="center"><strong>YouTube, nativo y rápido.</strong></p>
<p align="center">
  <a href="https://github.com/bcosta19/youtfast/actions/workflows/ci.yml"><img src="https://github.com/bcosta19/youtfast/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"></a>
  <a href="https://github.com/bcosta19/youtfast/releases/latest"><img src="https://img.shields.io/github/v/release/bcosta19/youtfast?sort=semver" alt="Última versión"></a>
  <a href="https://github.com/bcosta19/youtfast/releases"><img src="https://img.shields.io/github/downloads/bcosta19/youtfast/total" alt="Descargas"></a>
  <a href="../../../LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--only-blue" alt="Licencia: GPL-3.0-only"></a>
  <img src="https://img.shields.io/badge/rust-1.98%2B-orange?logo=rust" alt="Rust 1.98+">
  <img src="https://img.shields.io/badge/platforms-Linux%20%7C%20Windows%20%7C%20macOS-lightgrey" alt="Plataformas: Linux, Windows, macOS">
</p>
<p align="center">
  <a href="https://github.com/bcosta19/youtfast/releases">Versiones</a> ·
  <a href="https://youtfast.pages.dev/">Sitio web</a> ·
  <a href="user-guide.md">Guía de usuario</a> ·
  <a href="../../../CHANGELOG.md">Registro de cambios</a> ·
  <a href="../../../CONTRIBUTING.md">Contribuir</a>
</p>
<p align="center">
  <a href="../../../README.md">English</a> ·
  <a href="../pt-BR/README.md">Português (Brasil)</a> ·
  <strong>Español</strong> ·
  <a href="../fr/README.md">Français</a> ·
  <a href="../de/README.md">Deutsch</a> ·
  <a href="../it/README.md">Italiano</a>
</p>

*Esta es una traducción del [documento original en inglés](../../../README.md). Si
difieren, prevalece la versión en inglés.*

Un pequeño cliente de escritorio nativo de YouTube creado con **Rust, egui y libmpv**.
Busca, mira y conserva tu biblioteca en una interfaz de escritorio, con
decodificación de vídeo por hardware donde esté disponible. Sin motor de navegador,
sin telemetría y sin servidores operados por YouTFast. Empiezas sin iniciar sesión;
las cuentas son opcionales.

<picture>
  <source type="image/avif" srcset="../../../assets/demo/youtfast-demo.avif">
  <img src="../../../assets/demo/youtfast-demo.webp" alt="Demostración de YouTFast 0.7.0: búsqueda, pestañas de vídeo, reproducción, vistas previas en la línea de tiempo, modo cine y pantalla completa, minirreproductor, notificaciones, temas, seis idiomas y atajos de teclado">
</picture>

*YouTFast 0.7.0 en Linux, sin iniciar sesión: una grabación de pantalla real, editada
con zooms y subtítulos; se recortaron las esperas de carga y parte de la navegación
está acelerada.
[Descárgala en calidad completa](../../../assets/demo/youtfast-demo.mp4) (MP4, 1080p, 62 segundos, 13.4 MB, sin sonido).
Las películas son [Spring](https://studio.blender.org/films/spring/) y
[Sprite Fright](https://studio.blender.org/films/sprite-fright/) de Blender
Studio ([CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)).*

> [!NOTE]
> **YouTFast es joven y todavía necesita trabajo.** Tiene un único mantenedor y
> está en desarrollo activo `0.x`: ya reproduce, busca y conserva una biblioteca,
> pero espera asperezas y piezas que faltan. Si algo
> falla o se siente menos pulido que YouTube en el navegador, ese es un excelente
> lugar para ayudar. Consulta [cómo contribuir](#se-busca-ayuda).

## Qué puedes hacer

| Función | En YouTFast |
|---|---|
| Mirar | Vídeo dentro de la ventana, pantalla completa, modo cine, modo solo audio y un minirreproductor compacto |
| Descubrir | Filtros de búsqueda, pestañas de canal, listas de reproducción, Shorts y feeds paginados |
| Mantener el control | Subtítulos, capítulos, edición de la cola, búfer de vídeo configurable, atajos de teclado y controles multimedia del escritorio |
| Conservar una biblioteca | Suscripciones en el dispositivo, Ver más tarde, historial/reanudación, History sync (sincronización del historial) e importación desde Google Takeout |
| Leer y explorar | Descripciones, comentarios y adjuntos; transcripciones y vistas previas en la línea de tiempo opcionales |
| Hacerlo tuyo | Seis idiomas de interfaz (EN, PT-BR, ES, FR, DE, IT), temas claro/oscuro, paletas e integración nativa con Omarchy en Linux |

Las cuentas opcionales usan el almacén de credenciales del sistema. Las valoraciones
y las ediciones de listas de reproducción afectan a la cuenta seleccionada; las
suscripciones y Ver más tarde del dispositivo se quedan en local.
[Lee sobre las cuentas y los datos almacenados](user-guide.md#tu-biblioteca-con-o-sin-cuenta).

## Más ligero que un sitio web en una ventana

Muchas "apps de escritorio de YouTube" son el sitio web dentro de Electron. Medido
en un portátil Linux con los mismos resultados de búsqueda (`"blender open movie"`)
y la reproducción en 1080p de *Spring*, a pantalla completa, sin iniciar sesión
([método, todas las ejecuciones y salvedades](../../benchmarks.md)):

| | YouTFast 0.7.0 | Electron 44 + youtube.com |
|---|---:|---:|
| Búsqueda: memoria (PSS, todos los procesos) | **136 MiB** | 662 MiB |
| Búsqueda: CPU en reposo | **0.0%** | 5.6% de un núcleo |
| Reproducción (1080p): memoria | **206 MiB** | 625 MiB |
| Reproducción (1080p): CPU | **13.9%** | 41.2% de un núcleo |
| Procesos | **1** | 8 |
| Primera ventana en pantalla | **0.5 s** | 0.7 s |
| En disco | **29 MB** (13 MB de descarga) | 316 MB (301 MiB) solo el runtime |

Se agradecen cifras de otras máquinas.

## Instalación

**[Guía de instalación paso a paso](install.md)**, para cada sistema:
Flatpak, Arch, Ubuntu/Debian, Fedora, Windows y macOS, con actualización y
desinstalación. También en el sitio web: <https://youtfast.pages.dev/install>.

Las versiones publicadas aparecen en GitHub Releases. Un cambio de versión en el
repositorio no significa que sus paquetes se hayan publicado.
Los paquetes se compilan y prueban sin interfaz gráfica en runners alojados en
GitHub; las comprobaciones interactivas en máquinas Windows y macOS limpias siguen
en curso. Descárgalos desde
[GitHub Releases](https://github.com/bcosta19/youtfast/releases).

| Plataforma | Paquete | Requisitos |
|---|---|---|
| Linux x86_64 | Paquete Flatpak o `install.sh` | Flatpak, runtime Freedesktop 25.08 y controladores/portales del escritorio |
| Linux x86_64 | Archivo nativo | glibc 2.39+, libmpv 0.42+ del sistema con Lua y yt-dlp/Deno actuales; admite actualizaciones desde la app |
| Windows x86_64 | Instalador `.exe` (NSIS), `install.ps1` o ZIP portátil | yt-dlp/Deno actuales en el PATH (el instalador `.exe` instala los que falten mediante winget); admite actualizaciones desde la app; sin firma de editor |
| macOS 15+ Apple Silicon | ZIP con la app y las dylibs | yt-dlp/Deno actuales; admite actualizaciones desde la app; firma ad hoc, sin notarización |
| Arch Linux x86_64 | Paquete de pacman (`pacman -U` o `install.sh`) y PKGBUILD de código fuente | Compilado e instalado por la CI en un contenedor Arch limpio; el registro en AUR está cerrado actualmente |

Los archivos/paquetes nativos incluyen un actualizador integrado en **Configuración → Información**
(verificado con SHA-256 frente a `checksums.txt`, con actualizaciones automáticas
opcionales minor/patch y actualizaciones mayores manuales con un clic). Las
compilaciones de versión también generan manifiestos ya rellenados para Flathub,
Winget y Homebrew Cask (`channels.tar.gz`).
El Flatpak incluye sus herramientas de reproducción/extracción y está pensado para
distribuciones Linux compatibles en x86_64. Todavía no está publicado en Flathub
ni es un remoto de actualizaciones de la app.
La receta de Arch todavía no es una entrada de AUR ni un paquete oficial de pacman.

[Detalles de plataforma, sumas de verificación y límites](../../distribution.md) ·
[Mantenimiento del paquete de Arch](../../../packaging/arch/README.md)

## Compilar desde el código fuente

Instala el compilador y las dependencias del sistema según
[CONTRIBUTING.md](../../../CONTRIBUTING.md#getting-started) y luego ejecuta desde el checkout:

```sh
cargo run --locked --release
cargo run --locked --release -- "https://youtu.be/dQw4w9WgXcQ"
cargo run --locked --release -- "rust egui"
```

La reproducción necesita **libmpv 0.42 o posterior con soporte de Lua**, un yt-dlp
actual y su runtime EJS/JS correspondiente. Las versiones anteriores de mpv pueden
abrir las URL de los streams antes de que estén disponibles, lo que provoca fallos
HTTP 403 intermitentes. La importación opcional de la sesión del navegador también
necesita `python3` con el módulo `yt_dlp`. Consulta los
[requisitos](user-guide.md#requisitos).

**Transmitir a la TV** admite televisores con YouTube mediante DIAL/Lounge nativos,
incluidos televisores Samsung compatibles sin Chromecast. La compatibilidad con
Google Cast usa además un ejecutable [`catt`](https://github.com/skorokithakis/catt)
instalado por separado en el `PATH`. El botón junto a Solo audio en el reproductor,
o en la barra superior, abre el selector de TV y el mando a distancia. También se
puede encontrar una TV por su IP local. Consulta
[Transmitir a la TV](user-guide.md#transmitir-a-la-tv) para la configuración, los destinos compatibles y los límites.

## Se busca ayuda

YouTFast mejora con cada informe de error, prueba y pull request, y no
necesitas saber Rust para ayudar:

- **Pruébalo y cuéntanos qué falla.** Sobre todo en Windows y macOS, que
  hasta ahora solo se han probado en la CI. Los pasos para reproducir el problema son lo que más ayuda.
- **Elige una incidencia.** Busca
  [good first issues](https://github.com/bcosta19/youtfast/labels/good%20first%20issue)
  y [help wanted](https://github.com/bcosta19/youtfast/labels/help%20wanted).
- **Traduce.** Existen inglés, portugués de Brasil, español, francés, alemán e italiano; se agradecen más idiomas.
- **Empaquétalo.** Se agradece ayuda con Winget, el tap de Homebrew y el empaquetado para distribuciones.
- **Mídelo.** Ejecuta las [pruebas de rendimiento](../../benchmarks.md) en tu máquina.

Empieza por [CONTRIBUTING.md](../../../CONTRIBUTING.md); las preguntas en las incidencias son bienvenidas.

## Guías y estado del proyecto

| Quieres… | Lee |
|---|---|
| Aprender los controles, la biblioteca, las cuentas y los temas | [Guía de usuario](user-guide.md) |
| Instalar, actualizar o desinstalar YouTFast | [Guía de instalación](install.md) |
| Elegir un paquete o comprobar la cobertura de plataformas | [Distribución](../../distribution.md) |
| Ver el contenido y las exclusiones de cada versión | [Registro de cambios](../../../CHANGELOG.md) |
| Ver el trabajo que aún no está implementado | [Hoja de ruta](../../../ROADMAP.md) |
| Informar de un error o contribuir | [Contribuir](../../../CONTRIBUTING.md) |
| Informar de un problema de seguridad en privado | [Política de seguridad](../../../SECURITY.md) |
| Comparar el uso de recursos | [Pruebas de rendimiento](../../benchmarks.md) |
| Preparar una versión | [Proceso de publicación](../../../RELEASING.md) |

La biblioteca y la configuración se quedan en este dispositivo. Las cookies de la
cuenta se guardan en el almacén de credenciales nativo, y los diagnósticos locales
ocultan los datos privados. El acceso con cuenta no es oficial y puede provocar
verificaciones de Google, el cierre de la sesión o la suspensión de la cuenta.
Usa la app sin iniciar sesión salvo que elijas esa integración deliberadamente.
[Almacenamiento, privacidad y detalles de las cuentas](user-guide.md#tu-biblioteca-con-o-sin-cuenta).

## Proyectos relacionados y agradecimientos

- [Spotifast](https://github.com/crmne/spotifast) — cliente nativo de Spotify.
- [ZapFast](https://github.com/crmne/zapfast) — cliente nativo de WhatsApp.
- [fastframe](https://github.com/crmne/fastframe) — bases compartidas para
  aplicaciones de escritorio nativas con egui.

YouTFast sigue las convenciones de escritorio nativo de estos proyectos y usa forks
compartidos de egui/winit. Gracias también a [mpv](https://mpv.io),
[yt-dlp](https://github.com/yt-dlp/yt-dlp),
[egui](https://github.com/emilk/egui) y [Lucide](https://lucide.dev).
El cliente InnerTube de YouTFast se desarrolla en este repositorio; las versiones
anteriores usaban [RustyPipe](https://codeberg.org/ThetaDev/rustypipe).

## Licencia y aviso legal

[GPL-3.0-only](../../../LICENSE). Las dependencias conservan sus propias licencias y avisos;
consulta [THIRD_PARTY_NOTICES.md](../../../THIRD_PARTY_NOTICES.md).

YouTFast es un cliente no oficial, sin relación con YouTube ni con Google. Usar un
cliente no oficial puede infringir los términos de servicio de YouTube. Úsalo bajo
tu propia responsabilidad.
