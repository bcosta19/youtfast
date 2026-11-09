# Installing YouTFast

[Back to the overview](../README.md) · [User guide](user-guide.md) ·
[Platform details](distribution.md) · [This guide on the website](https://youtfast.pages.dev/install)

YouTFast runs on 64-bit Intel/AMD Linux (x86_64), 64-bit Windows 10/11 and
Macs with Apple Silicon on macOS 15 or newer. There is no phone version,
no ARM Linux/Windows build and no Intel Mac build.

Every file comes from the
[latest GitHub release](https://github.com/bcosta19/youtfast/releases/latest).
YouTFast is not yet in Flathub, the AUR, Winget or Homebrew.

## 1. Find your route

| Your computer | Follow | What you install |
|---|---|---|
| Any Linux with Flatpak (Fedora, Mint, Pop!_OS, elementary, Ubuntu with Flatpak added, Steam Deck…) | [Linux: Flatpak](#linux-flatpak) | One bundle that already contains libmpv, yt-dlp and Deno |
| Arch Linux, Manjaro, EndeavourOS, CachyOS, Omarchy | [Linux: Arch package](#linux-arch-package) | A pacman package; pacman installs everything else |
| Ubuntu 24.04+, Debian 13+, Linux Mint 22+ without Flatpak | [Linux: Ubuntu and Debian](#linux-ubuntu-and-debian) | The native archive plus three system steps |
| Fedora 40+ without Flatpak | [Linux: Fedora](#linux-fedora) | The native archive plus three system steps |
| Windows 10 or 11 (64-bit) | [Windows](#windows) | A ZIP folder, plus yt-dlp and Deno through Winget |
| Mac with Apple Silicon (M1 or newer), macOS 15+ | [macOS](#macos) | An app, plus yt-dlp and Deno through Homebrew |

Not sure which Linux route? Use **Flatpak**: it needs nothing else from your
system. To see whether your Linux is new enough for the native archive, run
`ldd --version`; the first line must end in 2.39 or higher.

## Linux: Flatpak

You need: Flatpak installed. Many distributions ship it; otherwise follow
[Flatpak's setup page for your distribution](https://flatpak.org/setup/).

1. Open a terminal.
2. Download the bundle and check it:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   echo "Installing YouTFast $V"
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-v$V-x86_64.flatpak" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

   The last line must print `youtfast-v…-x86_64.flatpak: OK`.
3. Install it for your user. The first time, Flatpak also downloads the
   Freedesktop runtime it needs; answer `y` when asked.

   ```sh
   flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
   flatpak install --user "./youtfast-v$V-x86_64.flatpak"
   ```

4. Open **YouTFast** from your application menu, or run
   `flatpak run io.github.bcosta19.YouTFast`.

**Update:** repeat steps 2 and 3 with the new release. The bundle has no
automatic update channel yet.
**Remove:** `flatpak uninstall --user io.github.bcosta19.YouTFast`.

The Flatpak cannot read your browser's profile. To sign in, choose a
cookies.txt file in the app, or stay signed out.

## Linux: Arch package

You need: an Arch-based system with `sudo`.

1. Open a terminal.
2. Download the package and check it:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-$V-1-x86_64.pkg.tar.zst" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

3. Install it. pacman also installs mpv, yt-dlp, Deno and the other
   libraries YouTFast needs:

   ```sh
   sudo pacman -U "./youtfast-$V-1-x86_64.pkg.tar.zst"
   ```

4. Open **YouTFast** from your application menu, or run `youtfast`.

**Update:** repeat steps 2 and 3 with the new release; keep the system
updated with `sudo pacman -Syu` so yt-dlp stays current.
**Remove:** `sudo pacman -R youtfast`.

## Linux: Ubuntu and Debian

You need: Ubuntu 24.04 or newer, Debian 13 or newer, or a distribution based
on them (Linux Mint 22+, Pop!_OS 24.04+), and `sudo`.

1. Install the libraries YouTFast uses, plus `pipx` and `unzip` for the next
   step:

   ```sh
   sudo apt-get update
   sudo apt-get install libmpv2 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 \
     libgl1 libfontconfig1 pipx unzip curl
   ```

2. [Install the current yt-dlp and Deno](#yt-dlp-and-deno-on-linux) as
   described below. Do not use the `yt-dlp` apt package: it is too old to
   play YouTube videos.
3. [Install the YouTFast archive](#install-the-native-archive) as described
   below.

## Linux: Fedora

You need: Fedora 40 or newer and `sudo`.

1. Install the libraries YouTFast uses, plus `pipx` and `unzip`:

   ```sh
   sudo dnf install mpv-libs libxkbcommon-x11 mesa-libGL fontconfig pipx unzip curl
   ```

2. [Install the current yt-dlp and Deno](#yt-dlp-and-deno-on-linux).
3. [Install the YouTFast archive](#install-the-native-archive).

Other distributions with glibc 2.39 or newer can follow the same three
steps: install libmpv (built with Lua, as usual), libxkbcommon, Wayland's
client library, OpenGL and fontconfig under your distribution's names.

### yt-dlp and Deno on Linux

YouTFast asks yt-dlp for each video's streams, and yt-dlp needs Deno and its
EJS component to read YouTube's pages. These commands install all three for
your user, in `~/.local/bin`:

```sh
pipx ensurepath
pipx install "yt-dlp[default]"
curl -fsSL https://deno.land/install.sh | DENO_INSTALL="$HOME/.local" sh -s -- -y --no-modify-path
```

`yt-dlp[default]` includes the EJS component. If `pipx ensurepath` says it
changed your `PATH`, **log out and back in** once, so programs opened from the
application menu find yt-dlp too. Check it in a new terminal:

```sh
yt-dlp --version
deno --version
```

Keep yt-dlp current: when videos stop loading, run `pipx upgrade yt-dlp`
first.

### Install the native archive

1. Download the archive and check it:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-v$V-x86_64-unknown-linux-gnu.tar.gz" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

2. Unpack it and install the program, its icon and its menu entry for your
   user:

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

   The last command prints the version; it does not open the window.
3. Open **YouTFast** from your application menu, or run `youtfast`.

**Update:** YouTFast updates itself from **Settings → About**. Keep the
program where step 2 put it, so the updater can replace it. Update yt-dlp
with `pipx upgrade yt-dlp` and the libraries with your system's updates.
**Remove:**

```sh
rm ~/.local/bin/youtfast ~/.local/share/applications/youtfast.desktop \
  ~/.local/share/icons/hicolor/scalable/apps/youtfast.svg
```

## Windows

You need: 64-bit Windows 10 or 11, and Winget (App Installer), which recent
Windows versions include.

1. Install yt-dlp and Deno. Open **Terminal** or **PowerShell** and run:

   ```powershell
   winget install --id yt-dlp.yt-dlp
   winget install --id DenoLand.Deno
   ```

   Then **sign out of Windows and back in**, so newly started programs find
   them. Check in a new terminal: `yt-dlp --version` and `deno --version`
   both print a version.
2. Download `youtfast-v<version>-x86_64-pc-windows-gnu.zip` from the
   [latest release](https://github.com/bcosta19/youtfast/releases/latest).
3. Optional, to check the download: in the Downloads folder run
   `Get-FileHash .\youtfast-v*-x86_64-pc-windows-gnu.zip` and compare the
   `Hash` with the line for that file in the release's `checksums.txt`
   (case does not matter).
4. Right-click the ZIP, choose **Extract All…**, and pick a lasting place,
   for example `Documents\YouTFast`. Keep every file of the folder together:
   the program needs the DLLs next to it.
5. Open `youtfast.exe` in the extracted folder. Windows SmartScreen warns
   about apps without a publisher signature, which YouTFast does not have:
   choose **More info**, then **Run anyway**.
6. Optional: right-click `youtfast.exe` → **Show more options** → **Send to**
   → **Desktop (create shortcut)**, or pin it to Start.

**Update:** YouTFast updates itself from **Settings → About**. Update yt-dlp
with `winget upgrade --id yt-dlp.yt-dlp` when videos stop loading.
**Remove:** delete the extracted folder.

## macOS

You need: a Mac with Apple Silicon (M1 or newer) on macOS 15 or newer, and
[Homebrew](https://brew.sh/).

1. Install yt-dlp and Deno. In **Terminal**:

   ```sh
   brew install yt-dlp deno
   ```

2. Download `youtfast-v<version>-aarch64-apple-darwin.zip` from the
   [latest release](https://github.com/bcosta19/youtfast/releases/latest).
3. Optional, to check the download: in Terminal, run
   `shasum -a 256 ~/Downloads/youtfast-v*-aarch64-apple-darwin.zip` and
   compare the result with the line for that file in the release's
   `checksums.txt`.
4. Double-click the ZIP to unpack it, then drag **YouTFast.app** into
   **Applications**.
5. Open YouTFast from Applications. The app is not notarized by Apple, so the
   first time macOS refuses it: click **Done**, open **System Settings →
   Privacy & Security**, scroll to the message about YouTFast, click
   **Open Anyway** and confirm with your password. Later launches open
   normally.

**Update:** YouTFast updates itself from **Settings → About**. Update yt-dlp
with `brew upgrade yt-dlp` when videos stop loading.
**Remove:** drag YouTFast.app from Applications to the Trash.

## Your data

Removing YouTFast keeps your library and settings. Delete these folders as
well for a clean removal:

| Installed with | Library | Settings |
|---|---|---|
| Arch package or native archive | `~/.local/share/youtfast` | `~/.config/youtfast` |
| Flatpak | `~/.var/app/io.github.bcosta19.YouTFast` | (same folder) |
| Windows | `%APPDATA%\youtfast\data` | `%APPDATA%\youtfast\config` |
| macOS | `~/Library/Application Support/youtfast` | (same folder) |

A signed-in session is kept in the system's credential store, not in these
folders: sign out in the app before removing it.

## If something goes wrong

| What you see | What to do |
|---|---|
| The window opens but videos never start | Update yt-dlp (see your route above). Check that `yt-dlp --version` and `deno --version` work in a new terminal. |
| `youtfast: command not found` after the native archive | Use the menu entry, or log out and back in so `~/.local/bin` is on `PATH`. |
| `error while loading shared libraries: libmpv.so.2` | Install your distribution's libmpv (step 1 of your route), or use the [Flatpak](#linux-flatpak). |
| `GLIBC_2.39 not found` | Your Linux is older than the native archive supports: use the [Flatpak](#linux-flatpak). |
| Windows: "Windows protected your PC" | Choose **More info**, then **Run anyway** (see [Windows](#windows), step 5). |
| macOS: "YouTFast cannot be opened" | Use **Open Anyway** in Privacy & Security (see [macOS](#macos), step 5). |

Still stuck? [Open an issue](https://github.com/bcosta19/youtfast/issues) with
your system, how you installed YouTFast and what happened. Portuguese is
welcome.

A checksum shows that the download is complete and unchanged; it is not a
publisher signature. To build YouTFast yourself instead, see
[CONTRIBUTING.md](../CONTRIBUTING.md#getting-started).
