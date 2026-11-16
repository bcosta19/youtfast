# Instalar YouTFast

[Volver a la presentación](README.md) · [Guía de usuario](user-guide.md) ·
[Detalles de plataforma](../../distribution.md) · [Esta guía en el sitio web](https://youtfast.pages.dev/install)

[English](../../install.md) · [Português (Brasil)](../pt-BR/install.md) · **Español** ·
[Français](../fr/install.md) · [Deutsch](../de/install.md) · [Italiano](../it/install.md)

*Esta es una traducción del [documento original en inglés](../../install.md). Si
difieren, prevalece la versión en inglés.*

YouTFast funciona en Linux de 64 bits para Intel/AMD (x86_64), en Windows 10/11 de
64 bits y en Mac con Apple Silicon con macOS 15 o posterior. No hay versión para
móviles, ni compilación para Linux/Windows ARM ni para Mac con Intel.

Todos los archivos vienen de la
[última versión en GitHub](https://github.com/bcosta19/youtfast/releases/latest).
Por ahora no se usan Flathub ni AUR (usa directamente el paquete Flatpak o el
paquete de Arch). En macOS hay un tap personal de Homebrew disponible en
`bcosta19/homebrew-tap`; Winget (`microsoft/winget-pkgs`) tiene manifiestos
preparados a la espera de que se acepte el PR.

## 1. Encuentra tu ruta

| Tu ordenador | Sigue | Qué instalas |
|---|---|---|
| Cualquier Linux (instalador rápido de un comando) | [Linux: instalador rápido](#linux-instalador-rápido) | Elige automáticamente el paquete de Arch o el paquete Flatpak y comprueba el SHA-256 |
| Cualquier Linux con Flatpak (Fedora, Mint, Pop!_OS, elementary, Ubuntu con Flatpak añadido, Steam Deck…) | [Linux: Flatpak](#linux-flatpak) | Un único paquete que ya contiene libmpv, yt-dlp y Deno |
| Arch Linux, Manjaro, EndeavourOS, CachyOS, Omarchy | [Linux: paquete de Arch](#linux-paquete-de-arch) | Un paquete de pacman; pacman instala todo lo demás |
| Ubuntu 24.04+, Debian 13+, Linux Mint 22+ sin Flatpak | [Linux: Ubuntu y Debian](#linux-ubuntu-y-debian) | El archivo nativo más tres pasos en el sistema |
| Fedora 40+ sin Flatpak | [Linux: Fedora](#linux-fedora) | El archivo nativo más tres pasos en el sistema |
| Windows 10 u 11 (64 bits) | [Windows](#windows) | Instalador por usuario (`.exe`), un comando de PowerShell o un `.zip` portátil |
| Mac con Apple Silicon (M1 o posterior), macOS 15+ | [macOS](#macos) | Una app, más yt-dlp y Deno mediante Homebrew |

¿No sabes qué ruta seguir en Linux? Ejecuta el [instalador rápido para Linux](#linux-instalador-rápido)
o instala el paquete **Flatpak**: no necesita nada más de tu sistema. Para saber si
tu Linux es lo bastante reciente para el archivo nativo, ejecuta `ldd --version`;
la primera línea debe terminar en 2.39 o superior.

La reproducción nativa en Linux requiere además **libmpv 0.42+ compilada con Lua**.
Comprueba la versión del paquete libmpv instalado (`apt-cache policy libmpv2`,
`dnf info --installed mpv-libs` o `pacman -Q mpv`). Las versiones de las
distribuciones indicadas abajo describen la base de glibc, no garantizan que su mpv
predeterminado sea lo bastante reciente. Si el tuyo es más antiguo, actualiza libmpv
desde una fuente de confianza o usa una versión Flatpak que incluya mpv 0.42+.
Actualizar solo YouTFast no actualiza la libmpv de un sistema nativo.

## Linux: instalador rápido

El script de instalación descarga de GitHub los archivos de la última versión,
verifica el resumen SHA-256 frente a `checksums.txt` e instala el paquete de Arch
(mediante `sudo pacman -U` en las distribuciones basadas en Arch) o el paquete
Flatpak (`--user` en todas las demás):

```sh
curl -fsSL https://youtfast.pages.dev/install.sh | sh
```

También puedes elegir el método explícitamente o desinstalar una instalación Flatpak
o local del usuario:

```sh
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method flatpak
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method arch
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --uninstall
```

## Linux: Flatpak

Necesitas: Flatpak instalado. Muchas distribuciones lo incluyen; si no, sigue
[la página de configuración de Flatpak para tu distribución](https://flatpak.org/setup/).

1. Abre una terminal.
2. Descarga el paquete y compruébalo:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   echo "Installing YouTFast $V"
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-v$V-x86_64.flatpak" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

   La última línea debe imprimir `youtfast-v…-x86_64.flatpak: OK`.
3. Instálalo para tu usuario. La primera vez, Flatpak también descarga el
   runtime de Freedesktop que necesita; responde `y` cuando te lo pregunte.

   ```sh
   flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
   flatpak install --user "./youtfast-v$V-x86_64.flatpak"
   ```

4. Abre **YouTFast** desde el menú de aplicaciones, o ejecuta
   `flatpak run io.github.bcosta19.YouTFast`.

**Actualizar:** repite los pasos 2 y 3 (o ejecuta `install.sh --method flatpak`
desde **Configuración → Información**). El paquete en entorno aislado no tiene
canal de actualización automática.
**Desinstalar:** `flatpak uninstall --user io.github.bcosta19.YouTFast`.

El Flatpak no puede leer el perfil de tu navegador. Para iniciar sesión, elige un
archivo cookies.txt en la app, o no inicies sesión.

## Linux: paquete de Arch

Necesitas: un sistema basado en Arch con `sudo`.

1. Abre una terminal.
2. Descarga el paquete y compruébalo:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-$V-1-x86_64.pkg.tar.zst" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

3. Instálalo. pacman también instala mpv, yt-dlp, Deno y las demás bibliotecas
   que YouTFast necesita. Las versiones que incluyen la corrección de #84 requieren
   mpv 0.42+; los paquetes publicados anteriormente no lo imponen. Comprueba
   `pacman -Q mpv` y actualiza el sistema con `sudo pacman -Syu` si hace falta:

   ```sh
   sudo pacman -U "./youtfast-$V-1-x86_64.pkg.tar.zst"
   ```

4. Abre **YouTFast** desde el menú de aplicaciones, o ejecuta `youtfast`.

**Actualizar:** YouTFast se actualiza desde **Configuración → Información**
(descarga y verifica el paquete `.pkg.tar.zst` y luego ejecuta `pkexec pacman -U`),
o repite los pasos 2 y 3 con la nueva versión. Mantén el sistema actualizado con
`sudo pacman -Syu` para que yt-dlp esté al día.
**Desinstalar:** `sudo pacman -R youtfast`.

## Linux: Ubuntu y Debian

Necesitas: Ubuntu 24.04 o posterior, Debian 13 o posterior, o una distribución
basada en ellos (Linux Mint 22+, Pop!_OS 24.04+), y `sudo`.

1. Instala las bibliotecas que usa YouTFast, más `pipx` y `unzip` para el siguiente
   paso:

   ```sh
   sudo apt-get update
   sudo apt-get install libmpv2 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 \
     libgl1 libfontconfig1 pipx unzip curl
   ```

2. [Instala yt-dlp y Deno actualizados](#yt-dlp-y-deno-en-linux) como se
   describe abajo. No uses el paquete apt `yt-dlp`: es demasiado antiguo para
   reproducir vídeos de YouTube.
3. [Instala el archivo de YouTFast](#instalar-el-archivo-nativo) como se describe
   abajo.

## Linux: Fedora

Necesitas: Fedora 40 o posterior y `sudo`.

1. Instala las bibliotecas que usa YouTFast, más `pipx` y `unzip`:

   ```sh
   sudo dnf install mpv-libs libxkbcommon-x11 mesa-libGL fontconfig pipx unzip curl
   ```

2. [Instala yt-dlp y Deno actualizados](#yt-dlp-y-deno-en-linux).
3. [Instala el archivo de YouTFast](#instalar-el-archivo-nativo).

Otras distribuciones con glibc 2.39 o posterior pueden seguir los mismos tres
pasos: instala libmpv 0.42+ (compilada con Lua), libxkbcommon, la biblioteca
cliente de Wayland, OpenGL y fontconfig con los nombres de tu distribución.

### yt-dlp y Deno en Linux

YouTFast pide a yt-dlp los streams de cada vídeo, y yt-dlp necesita Deno y su
componente EJS para leer las páginas de YouTube. Estos comandos instalan los tres
para tu usuario, en `~/.local/bin`:

```sh
pipx ensurepath
pipx install "yt-dlp[default]"
curl -fsSL https://deno.land/install.sh | DENO_INSTALL="$HOME/.local" sh -s -- -y --no-modify-path
```

`yt-dlp[default]` incluye el componente EJS. Si `pipx ensurepath` dice que cambió
tu `PATH`, **cierra la sesión y vuelve a iniciarla** una vez, para que los programas
abiertos desde el menú de aplicaciones también encuentren yt-dlp. Compruébalo en una
terminal nueva:

```sh
yt-dlp --version
deno --version
```

Mantén yt-dlp actualizado: cuando los vídeos dejen de cargar, ejecuta primero
`pipx upgrade yt-dlp`.

### Instalar el archivo nativo

1. Descarga el archivo y compruébalo:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-v$V-x86_64-unknown-linux-gnu.tar.gz" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

2. Descomprímelo e instala el programa, su icono y su entrada de menú para tu
   usuario:

   ```sh
   tar xzf "youtfast-v$V-x86_64-unknown-linux-gnu.tar.gz"
   cd "youtfast-v$V-x86_64-unknown-linux-gnu"
   install -Dm755 youtfast ~/.local/bin/youtfast
   install -Dm644 packaging/icons/youtfast.svg \
     ~/.local/share/icons/hicolor/scalable/apps/youtfast.svg
   sed "s|^Exec=youtfast|Exec=$HOME/.local/bin/youtfast|" packaging/applications/youtfast.desktop \
     | install -Dm644 /dev/stdin ~/.local/share/applications/youtfast.desktop
   ~/.local/bin/youtfast --version
   ```

   El último comando imprime la versión; no abre la ventana.
3. Abre **YouTFast** desde el menú de aplicaciones, o ejecuta `youtfast`.

**Actualizar:** YouTFast se actualiza solo desde **Configuración → Información**.
Deja el programa donde lo puso el paso 2, para que el actualizador pueda
sustituirlo. Actualiza yt-dlp con `pipx upgrade yt-dlp` y las bibliotecas con las
actualizaciones de tu sistema.
**Desinstalar:**

```sh
rm ~/.local/bin/youtfast ~/.local/share/applications/youtfast.desktop \
  ~/.local/share/icons/hicolor/scalable/apps/youtfast.svg
```

## Windows

Necesitas: Windows 10 u 11 de 64 bits. Winget (Instalador de aplicación), incluido
en las versiones recientes de Windows, se usa si `yt-dlp` o `Deno` aún no están
instalados.

### Opción A: instalador de Windows (.exe) — recomendado

1. Descarga `youtfast-v<version>-x86_64-windows-setup.exe` de la
   [última versión](https://github.com/bcosta19/youtfast/releases/latest), o
   ejecuta el instalador verificado de PowerShell con un solo comando:

   ```powershell
   irm https://youtfast.pages.dev/install.ps1 | iex
   ```

2. Al abrir el `.exe`, Windows SmartScreen avisa de las apps sin firma de editor, y
   YouTFast no la tiene: elige **Más información** y luego **Ejecutar de todas
   formas**. Para verificar antes el archivo, ejecuta
   `Get-FileHash .\youtfast-v*-x86_64-windows-setup.exe` y compáralo con
   `checksums.txt`.
3. Sigue el asistente de instalación. Instala por usuario en
   `%LOCALAPPDATA%\Programs\YouTFast` (sin pedir permisos de administrador), crea
   accesos directos en el menú Inicio y en el escritorio, e instala `yt-dlp` y
   `Deno` si faltan mediante `winget`, si está activado. Cierra la sesión y vuelve
   a iniciarla si tuviste que instalar `yt-dlp` o `Deno` por primera vez.

### Opción B: ZIP portátil (.zip)

1. Instala yt-dlp y Deno. Abre **Terminal** o **PowerShell** y ejecuta:

   ```powershell
   winget install --id yt-dlp.yt-dlp
   winget install --id DenoLand.Deno
   ```

   Después **cierra la sesión de Windows y vuelve a iniciarla**, para que los
   programas que se abran a partir de entonces los encuentren. Compruébalo en una
   terminal nueva: `yt-dlp --version` y `deno --version` deben imprimir una versión.
2. Descarga `youtfast-v<version>-x86_64-pc-windows-gnu.zip` de la
   [última versión](https://github.com/bcosta19/youtfast/releases/latest).
3. Opcional, para comprobar la descarga: en la carpeta Descargas ejecuta
   `Get-FileHash .\youtfast-v*-x86_64-pc-windows-gnu.zip` y compara el `Hash` con
   la línea de ese archivo en el `checksums.txt` de la versión (no importan las
   mayúsculas y minúsculas).
4. Haz clic derecho en el ZIP, elige **Extraer todo…** y escoge un lugar
   permanente, por ejemplo `Documents\YouTFast`. Mantén juntos todos los archivos de
   la carpeta: el programa necesita las DLL que tiene al lado.
5. Abre `youtfast.exe` en la carpeta extraída y omite SmartScreen (**Más
   información** → **Ejecutar de todas formas**).

**Actualizar:** YouTFast se actualiza solo desde **Configuración → Información**.
Actualiza yt-dlp con `winget upgrade --id yt-dlp.yt-dlp` cuando los vídeos dejen de
cargar.
**Desinstalar:** usa **Configuración → Aplicaciones → Aplicaciones instaladas →
YouTFast** de Windows (o ejecuta `Uninstall.exe`) si lo instalaste con el `.exe`, o
elimina la carpeta extraída si usas el `.zip` portátil.

## macOS

Necesitas: un Mac con Apple Silicon (M1 o posterior) con macOS 15 o posterior, y
[Homebrew](https://brew.sh/).

### Opción A: tap de Homebrew — recomendado

```sh
brew install --cask bcosta19/tap/youtfast
```

El cask instala `YouTFast.app`, enlaza el comando `youtfast`, instala
`yt-dlp` y `deno` automáticamente, y elimina `com.apple.quarantine` del paquete de
la app con firma ad hoc. (Nota: actualiza el cask del tap junto con las nuevas
versiones de YouTFast, o usa **Configuración → Información** dentro de la app.)

### Opción B: ZIP manual

1. Instala yt-dlp y Deno. En **Terminal**:

   ```sh
   brew install yt-dlp deno
   ```

2. Descarga `youtfast-v<version>-aarch64-apple-darwin.zip` de la
   [última versión](https://github.com/bcosta19/youtfast/releases/latest).
3. Opcional, para comprobar la descarga: en Terminal, ejecuta
   `shasum -a 256 ~/Downloads/youtfast-v*-aarch64-apple-darwin.zip` y
   compara el resultado con la línea de ese archivo en el `checksums.txt` de la
   versión.
4. Haz doble clic en el ZIP para descomprimirlo y luego arrastra **YouTFast.app** a
   **Aplicaciones**.
5. Abre YouTFast desde Aplicaciones. Apple no ha notarizado la app, así que la
   primera vez macOS la rechaza: haz clic en **OK**, abre **Ajustes del Sistema →
   Privacidad y seguridad**, desplázate hasta el mensaje sobre YouTFast, haz clic en
   **Abrir igualmente** y confirma con tu contraseña. Las siguientes veces se abre
   con normalidad.

**Actualizar:** YouTFast se actualiza solo desde **Configuración → Información**.
Actualiza yt-dlp con `brew upgrade yt-dlp` cuando los vídeos dejen de cargar.
**Desinstalar:** arrastra YouTFast.app desde Aplicaciones a la Papelera.

## Tus datos

Desinstalar YouTFast conserva tu biblioteca y tu configuración. Elimina también estas
carpetas para una desinstalación limpia:

| Instalado con | Biblioteca | Configuración |
|---|---|---|
| Paquete de Arch o archivo nativo | `~/.local/share/youtfast` | `~/.config/youtfast` |
| Flatpak | `~/.var/app/io.github.bcosta19.YouTFast` | (la misma carpeta) |
| Windows | `%APPDATA%\youtfast\data` | `%APPDATA%\youtfast\config` |
| macOS | `~/Library/Application Support/youtfast` | (la misma carpeta) |

Una sesión iniciada se guarda en el almacén de credenciales del sistema, no en estas
carpetas: cierra la sesión en la app antes de desinstalarla.

## Si algo sale mal

| Qué ves | Qué hacer |
|---|---|
| La ventana se abre pero los vídeos nunca empiezan | Actualiza yt-dlp (consulta tu ruta arriba). Comprueba que `yt-dlp --version` y `deno --version` funcionan en una terminal nueva. |
| `YouTFast no pudo iniciar yt-dlp…` | yt-dlp no está instalado, o YouTFast se abrió con un `PATH` anterior a su instalación. Comprueba que `yt-dlp --version` y `deno --version` funcionan en una terminal nueva e instala lo que falte (consulta tu ruta arriba). En Windows, cierra la sesión y vuelve a iniciarla después de instalarlos, y abre YouTFast desde el menú Inicio. |
| `yt-dlp no pudo obtener las transmisiones de este vídeo…` | Actualiza yt-dlp (consulta tu ruta arriba) y comprueba que `deno --version` funciona en una terminal nueva. Si persiste con un yt-dlp actual, infórmalo. |
| `mpv no pudo reconocer el formato de la transmisión.` (0.9.0 y anteriores) | Esas versiones lo muestran cuando yt-dlp falta o falla: sigue las dos filas anteriores. |
| De forma intermitente: `No se pudo reproducir este vídeo. El reproductor no identificó la causa. Intenta volver a abrir el vídeo.` | Comprueba la versión del paquete libmpv instalado. Antes de la 0.42, su hook de yt-dlp puede abrir los streams demasiado pronto y recibir HTTP 403. Actualiza libmpv a 0.42+ con Lua, o usa una versión Flatpak que la incluya. Volver a abrirlo tras unos segundos es una solución temporal, no un arreglo. Si persiste con 0.42+, infórmalo; este mensaje puede tener otras causas. |
| `youtfast: command not found` después del archivo nativo | Usa la entrada del menú, o cierra la sesión y vuelve a iniciarla para que `~/.local/bin` esté en el `PATH`. |
| `error while loading shared libraries: libmpv.so.2` | Instala la libmpv de tu distribución (paso 1 de tu ruta), o usa el [Flatpak](#linux-flatpak). |
| `GLIBC_2.39 not found` | Tu Linux es más antiguo de lo que admite el archivo nativo: usa el [Flatpak](#linux-flatpak). |
| Windows: "Windows protegió su PC" | Elige **Más información** y luego **Ejecutar de todas formas** (consulta [Windows](#windows), paso 5). |
| macOS: "No se puede abrir YouTFast" | Usa **Abrir igualmente** en Privacidad y seguridad (consulta [macOS](#macos), paso 5). |

¿Sigues atascado? [Abre una incidencia](https://github.com/bcosta19/youtfast/issues) con
tu sistema, cómo instalaste YouTFast y qué pasó. Se aceptan incidencias en
portugués.

Una suma de verificación demuestra que la descarga está completa y sin cambios; no es
una firma de editor. Para compilar YouTFast tú mismo, consulta
[CONTRIBUTING.md](../../../CONTRIBUTING.md#getting-started).
