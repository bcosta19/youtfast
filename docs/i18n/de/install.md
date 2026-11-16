# YouTFast installieren

[Zurück zur Übersicht](README.md) · [Benutzerhandbuch](user-guide.md) ·
[Plattformdetails](../../distribution.md) · [Diese Anleitung auf der Website](https://youtfast.pages.dev/install)

[English](../../install.md) · [Português (Brasil)](../pt-BR/install.md) · [Español](../es/install.md) ·
[Français](../fr/install.md) · **Deutsch** · [Italiano](../it/install.md)

*Dies ist eine Übersetzung des [englischen Originals](../../install.md). Bei Abweichungen gilt die englische Fassung.*

YouTFast läuft unter 64-Bit-Linux auf Intel/AMD (x86_64), 64-Bit-Windows 10/11
und auf Macs mit Apple Silicon unter macOS 15 oder neuer. Es gibt keine Version
für Smartphones, keinen Build für ARM-Linux/-Windows und keinen Build für
Intel-Macs.

Jede Datei stammt aus der
[neuesten GitHub-Version](https://github.com/bcosta19/youtfast/releases/latest).
Flathub und das AUR werden derzeit nicht genutzt (verwenden Sie stattdessen
direkt das Flatpak-Bundle oder das Arch-Paket). Unter macOS steht ein
persönlicher Homebrew-Tap unter `bcosta19/homebrew-tap` bereit; für Winget
(`microsoft/winget-pkgs`) liegen vorbereitete Manifeste vor, deren PR noch
angenommen werden muss.

## 1. Finden Sie Ihren Weg

| Ihr Computer | Folgen Sie | Was Sie installieren |
|---|---|---|
| Jedes Linux (schneller Installer mit einem Befehl) | [Linux: Schnellinstallation](#linux-schnellinstallation) | Wählt automatisch das Arch-Paket oder das Flatpak-Bundle und prüft SHA-256 |
| Jedes Linux mit Flatpak (Fedora, Mint, Pop!_OS, elementary, Ubuntu mit nachinstalliertem Flatpak, Steam Deck…) | [Linux: Flatpak](#linux-flatpak) | Ein Bundle, das libmpv, yt-dlp und Deno bereits enthält |
| Arch Linux, Manjaro, EndeavourOS, CachyOS, Omarchy | [Linux: Arch-Paket](#linux-arch-paket) | Ein pacman-Paket; pacman installiert alles Weitere |
| Ubuntu 24.04+, Debian 13+, Linux Mint 22+ ohne Flatpak | [Linux: Ubuntu und Debian](#linux-ubuntu-und-debian) | Das native Archiv plus drei Systemschritte |
| Fedora 40+ ohne Flatpak | [Linux: Fedora](#linux-fedora) | Das native Archiv plus drei Systemschritte |
| Windows 10 oder 11 (64 Bit) | [Windows](#windows) | Installer pro Benutzer (`.exe`), PowerShell-Einzeiler oder portables `.zip` |
| Mac mit Apple Silicon (M1 oder neuer), macOS 15+ | [macOS](#macos) | Eine App, plus yt-dlp und Deno über Homebrew |

Unsicher, welcher Linux-Weg? Führen Sie die
[Linux-Schnellinstallation](#linux-schnellinstallation) aus oder installieren Sie
das **Flatpak**-Bundle: Es braucht nichts weiter von Ihrem System. Um zu sehen,
ob Ihr Linux neu genug für das native Archiv ist, führen Sie `ldd --version` aus;
die erste Zeile muss mit 2.39 oder höher enden.

Die native Wiedergabe unter Linux erfordert zusätzlich **libmpv 0.42+, mit Lua
gebaut**. Prüfen Sie die Version Ihres installierten libmpv-Pakets
(`apt-cache policy libmpv2`, `dnf info --installed mpv-libs` oder
`pacman -Q mpv`). Die unten genannten Distributionsversionen beschreiben die
glibc-Grundlage, nicht die Garantie, dass deren Standard-mpv neu genug ist. Ist
Ihres älter, aktualisieren Sie libmpv aus einer vertrauenswürdigen Quelle oder
verwenden Sie eine Flatpak-Version, die mpv 0.42+ enthält. Ein Update von
YouTFast allein aktualisiert nicht das libmpv eines nativen Systems.

## Linux: Schnellinstallation

Das Installationsskript lädt die Dateien der neuesten Version von GitHub herunter,
prüft den SHA-256-Hash gegen `checksums.txt` und installiert das Arch-Paket (über
`sudo pacman -U` auf Arch-basierten Distributionen) oder das Flatpak-Bundle
(überall sonst mit `--user`):

```sh
curl -fsSL https://youtfast.pages.dev/install.sh | sh
```

Sie können die Methode auch ausdrücklich wählen oder eine Flatpak- bzw. lokale
Benutzerinstallation entfernen:

```sh
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method flatpak
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method arch
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --uninstall
```

## Linux: Flatpak

Sie benötigen: ein installiertes Flatpak. Viele Distributionen liefern es mit;
andernfalls folgen Sie der
[Einrichtungsseite von Flatpak für Ihre Distribution](https://flatpak.org/setup/).

1. Öffnen Sie ein Terminal.
2. Laden Sie das Bundle herunter und prüfen Sie es:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   echo "Installing YouTFast $V"
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-v$V-x86_64.flatpak" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

   Die letzte Zeile muss `youtfast-v…-x86_64.flatpak: OK` ausgeben.
3. Installieren Sie es für Ihren Benutzer. Beim ersten Mal lädt Flatpak auch die
   benötigte Freedesktop-Laufzeitumgebung herunter; antworten Sie `y`, wenn Sie
   gefragt werden.

   ```sh
   flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
   flatpak install --user "./youtfast-v$V-x86_64.flatpak"
   ```

4. Öffnen Sie **YouTFast** über Ihr Anwendungsmenü oder führen Sie
   `flatpak run io.github.bcosta19.YouTFast` aus.

**Aktualisieren:** Wiederholen Sie die Schritte 2 und 3 mit der neuen Version.
Das Bundle hat noch keinen automatischen Update-Kanal.
**Entfernen:** `flatpak uninstall --user io.github.bcosta19.YouTFast`.

Das Flatpak kann das Profil Ihres Browsers nicht lesen. Um sich anzumelden,
wählen Sie in der App eine Datei cookies.txt, oder bleiben Sie abgemeldet.

## Linux: Arch-Paket

Sie benötigen: ein Arch-basiertes System mit `sudo`.

1. Öffnen Sie ein Terminal.
2. Laden Sie das Paket herunter und prüfen Sie es:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-$V-1-x86_64.pkg.tar.zst" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

3. Installieren Sie es. pacman installiert auch mpv, yt-dlp, Deno und die übrigen
   Bibliotheken, die YouTFast benötigt. Versionen mit der Korrektur aus #84
   erfordern mpv 0.42+; ältere veröffentlichte Pakete setzen dies nicht durch.
   Prüfen Sie `pacman -Q mpv` und aktualisieren Sie das System bei Bedarf mit
   `sudo pacman -Syu`:

   ```sh
   sudo pacman -U "./youtfast-$V-1-x86_64.pkg.tar.zst"
   ```

4. Öffnen Sie **YouTFast** über Ihr Anwendungsmenü oder führen Sie `youtfast` aus.

**Aktualisieren:** Wiederholen Sie die Schritte 2 und 3 mit der neuen Version;
halten Sie das System mit `sudo pacman -Syu` aktuell, damit yt-dlp aktuell bleibt.
**Entfernen:** `sudo pacman -R youtfast`.

## Linux: Ubuntu und Debian

Sie benötigen: Ubuntu 24.04 oder neuer, Debian 13 oder neuer oder eine darauf
basierende Distribution (Linux Mint 22+, Pop!_OS 24.04+) sowie `sudo`.

1. Installieren Sie die Bibliotheken, die YouTFast nutzt, plus `pipx` und `unzip`
   für den nächsten Schritt:

   ```sh
   sudo apt-get update
   sudo apt-get install libmpv2 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 \
     libgl1 libfontconfig1 pipx unzip curl
   ```

2. [Installieren Sie das aktuelle yt-dlp und Deno](#yt-dlp-und-deno-unter-linux)
   wie unten beschrieben. Verwenden Sie nicht das apt-Paket `yt-dlp`: Es ist zu
   alt, um YouTube-Videos abzuspielen.
3. [Installieren Sie das YouTFast-Archiv](#das-native-archiv-installieren) wie
   unten beschrieben.

## Linux: Fedora

Sie benötigen: Fedora 40 oder neuer und `sudo`.

1. Installieren Sie die Bibliotheken, die YouTFast nutzt, plus `pipx` und `unzip`:

   ```sh
   sudo dnf install mpv-libs libxkbcommon-x11 mesa-libGL fontconfig pipx unzip curl
   ```

2. [Installieren Sie das aktuelle yt-dlp und Deno](#yt-dlp-und-deno-unter-linux).
3. [Installieren Sie das YouTFast-Archiv](#das-native-archiv-installieren).

Andere Distributionen mit glibc 2.39 oder neuer können denselben drei Schritten
folgen: Installieren Sie libmpv 0.42+ (mit Lua gebaut), libxkbcommon, die
Client-Bibliothek von Wayland, OpenGL und fontconfig unter den Namen Ihrer
Distribution.

### yt-dlp und Deno unter Linux

YouTFast fragt yt-dlp nach den Streams jedes Videos, und yt-dlp benötigt Deno und
seine EJS-Komponente, um die Seiten von YouTube zu lesen. Diese Befehle
installieren alle drei für Ihren Benutzer in `~/.local/bin`:

```sh
pipx ensurepath
pipx install "yt-dlp[default]"
curl -fsSL https://deno.land/install.sh | DENO_INSTALL="$HOME/.local" sh -s -- -y --no-modify-path
```

`yt-dlp[default]` enthält die EJS-Komponente. Meldet `pipx ensurepath`, dass es
Ihren `PATH` geändert hat, **melden Sie sich einmal ab und wieder an**, damit
auch aus dem Anwendungsmenü geöffnete Programme yt-dlp finden. Prüfen Sie es in
einem neuen Terminal:

```sh
yt-dlp --version
deno --version
```

Halten Sie yt-dlp aktuell: Wenn Videos nicht mehr laden, führen Sie zuerst
`pipx upgrade yt-dlp` aus.

### Das native Archiv installieren

1. Laden Sie das Archiv herunter und prüfen Sie es:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-v$V-x86_64-unknown-linux-gnu.tar.gz" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

2. Entpacken Sie es und installieren Sie das Programm, sein Symbol und seinen
   Menüeintrag für Ihren Benutzer:

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

   Der letzte Befehl gibt die Version aus; er öffnet nicht das Fenster.
3. Öffnen Sie **YouTFast** über Ihr Anwendungsmenü oder führen Sie `youtfast` aus.

**Aktualisieren:** YouTFast aktualisiert sich selbst über
**Einstellungen → Kanalinfo**. Lassen Sie das Programm dort, wo Schritt 2 es
abgelegt hat, damit der Updater es ersetzen kann. Aktualisieren Sie yt-dlp mit
`pipx upgrade yt-dlp` und die Bibliotheken mit den Updates Ihres Systems.
**Entfernen:**

```sh
rm ~/.local/bin/youtfast ~/.local/share/applications/youtfast.desktop \
  ~/.local/share/icons/hicolor/scalable/apps/youtfast.svg
```

## Windows

Sie benötigen: 64-Bit-Windows 10 oder 11. Winget (App-Installer), in aktuellen
Windows-Versionen enthalten, wird verwendet, falls `yt-dlp` oder `Deno` noch nicht
installiert sind.

### Option A: Windows-Installer (.exe) — empfohlen

1. Laden Sie `youtfast-v<version>-x86_64-windows-setup.exe` aus der
   [neuesten Version](https://github.com/bcosta19/youtfast/releases/latest)
   herunter, oder führen Sie den geprüften PowerShell-Installer mit einem Befehl
   aus:

   ```powershell
   irm https://youtfast.pages.dev/install.ps1 | iex
   ```

2. Beim Öffnen der `.exe` warnt Windows SmartScreen vor Apps ohne
   Herausgebersignatur, die YouTFast nicht hat: Wählen Sie
   **Weitere Informationen** und dann **Trotzdem ausführen**. Um die Datei vorher
   zu prüfen, führen Sie `Get-FileHash .\youtfast-v*-x86_64-windows-setup.exe`
   aus und vergleichen Sie mit `checksums.txt`.
3. Folgen Sie dem Installationsassistenten. Er installiert pro Benutzer nach
   `%LOCALAPPDATA%\Programs\YouTFast` (ohne Administrator-Abfrage), legt
   Verknüpfungen im Startmenü und auf dem Desktop an und installiert fehlendes
   `yt-dlp` und `Deno` über `winget`, sofern aktiviert. Melden Sie sich ab und
   wieder an, wenn Sie `yt-dlp` oder `Deno` zum ersten Mal installieren mussten.

### Option B: Portables ZIP (.zip)

1. Installieren Sie yt-dlp und Deno. Öffnen Sie **Terminal** oder
   **PowerShell** und führen Sie aus:

   ```powershell
   winget install --id yt-dlp.yt-dlp
   winget install --id DenoLand.Deno
   ```

   **Melden Sie sich dann von Windows ab und wieder an**, damit neu gestartete
   Programme sie finden. Prüfen Sie in einem neuen Terminal: `yt-dlp --version`
   und `deno --version` geben beide eine Version aus.
2. Laden Sie `youtfast-v<version>-x86_64-pc-windows-gnu.zip` aus der
   [neuesten Version](https://github.com/bcosta19/youtfast/releases/latest)
   herunter.
3. Optional, um den Download zu prüfen: Führen Sie im Ordner Downloads
   `Get-FileHash .\youtfast-v*-x86_64-pc-windows-gnu.zip` aus und vergleichen Sie
   den `Hash` mit der Zeile für diese Datei in der `checksums.txt` der Version
   (Groß-/Kleinschreibung spielt keine Rolle).
4. Klicken Sie mit der rechten Maustaste auf das ZIP, wählen Sie
   **Alle extrahieren…** und wählen Sie einen dauerhaften Ort, zum Beispiel
   `Documents\YouTFast`. Lassen Sie alle Dateien des Ordners zusammen: Das
   Programm benötigt die DLLs neben sich.
5. Öffnen Sie `youtfast.exe` im entpackten Ordner und umgehen Sie SmartScreen
   (**Weitere Informationen** → **Trotzdem ausführen**).

**Aktualisieren:** YouTFast aktualisiert sich selbst über
**Einstellungen → Kanalinfo**. Aktualisieren Sie yt-dlp mit
`winget upgrade --id yt-dlp.yt-dlp`, wenn Videos nicht mehr laden.
**Entfernen:** Nutzen Sie unter Windows
**Einstellungen → Apps → Installierte Apps → YouTFast** (oder führen Sie
`Uninstall.exe` aus), wenn über die `.exe` installiert, oder löschen Sie den
entpackten Ordner, wenn Sie das portable `.zip` verwenden.

## macOS

Sie benötigen: einen Mac mit Apple Silicon (M1 oder neuer) unter macOS 15 oder
neuer sowie [Homebrew](https://brew.sh/).

### Option A: Homebrew-Tap — empfohlen

```sh
brew install --cask bcosta19/tap/youtfast
```

Der Cask installiert `YouTFast.app`, verlinkt den Befehl `youtfast`, installiert
`yt-dlp` und `deno` automatisch und entfernt `com.apple.quarantine` vom ad hoc
signierten App-Bundle. (Hinweis: Aktualisieren Sie den Tap-Cask zusammen mit
neuen YouTFast-Versionen, oder nutzen Sie in der App
**Einstellungen → Kanalinfo**.)

### Option B: Manuelles ZIP

1. Installieren Sie yt-dlp und Deno. Im **Terminal**:

   ```sh
   brew install yt-dlp deno
   ```

2. Laden Sie `youtfast-v<version>-aarch64-apple-darwin.zip` aus der
   [neuesten Version](https://github.com/bcosta19/youtfast/releases/latest)
   herunter.
3. Optional, um den Download zu prüfen: Führen Sie im Terminal
   `shasum -a 256 ~/Downloads/youtfast-v*-aarch64-apple-darwin.zip` aus und
   vergleichen Sie das Ergebnis mit der Zeile für diese Datei in der
   `checksums.txt` der Version.
4. Doppelklicken Sie auf das ZIP, um es zu entpacken, und ziehen Sie dann
   **YouTFast.app** in **Programme**.
5. Öffnen Sie YouTFast über Programme. Die App ist nicht von Apple notarisiert,
   daher verweigert macOS sie beim ersten Mal: Klicken Sie auf **Fertig**, öffnen
   Sie **Systemeinstellungen → Datenschutz & Sicherheit**, scrollen Sie zur
   Meldung über YouTFast, klicken Sie auf **Dennoch öffnen** und bestätigen Sie
   mit Ihrem Passwort. Spätere Starts öffnen normal.

**Aktualisieren:** YouTFast aktualisiert sich selbst über
**Einstellungen → Kanalinfo**. Aktualisieren Sie yt-dlp mit `brew upgrade yt-dlp`,
wenn Videos nicht mehr laden.
**Entfernen:** Ziehen Sie YouTFast.app aus Programme in den Papierkorb.

## Ihre Daten

Beim Entfernen von YouTFast bleiben Ihre Mediathek und Ihre Einstellungen
erhalten. Löschen Sie für eine vollständige Entfernung auch diese Ordner:

| Installiert mit | Mediathek | Einstellungen |
|---|---|---|
| Arch-Paket oder natives Archiv | `~/.local/share/youtfast` | `~/.config/youtfast` |
| Flatpak | `~/.var/app/io.github.bcosta19.YouTFast` | (derselbe Ordner) |
| Windows | `%APPDATA%\youtfast\data` | `%APPDATA%\youtfast\config` |
| macOS | `~/Library/Application Support/youtfast` | (derselbe Ordner) |

Eine angemeldete Sitzung liegt im Anmeldedatenspeicher des Systems, nicht in
diesen Ordnern: Melden Sie sich in der App ab, bevor Sie sie entfernen.

## Wenn etwas schiefgeht

| Was Sie sehen | Was zu tun ist |
|---|---|
| Das Fenster öffnet sich, aber Videos starten nie | Aktualisieren Sie yt-dlp (siehe Ihren Weg oben). Prüfen Sie, ob `yt-dlp --version` und `deno --version` in einem neuen Terminal funktionieren. |
| Sporadisch: `Dieses Video konnte nicht abgespielt werden. Die Ursache ist unbekannt. Versuchen Sie, das Video erneut zu öffnen.` | Prüfen Sie die Version des installierten libmpv-Pakets. Vor 0.42 kann sein yt-dlp-Hook Streams zu früh öffnen und HTTP 403 erhalten. Aktualisieren Sie libmpv auf 0.42+ mit Lua oder verwenden Sie eine Flatpak-Version, die es enthält. Das erneute Öffnen nach einigen Sekunden ist eine vorübergehende Umgehung, keine Lösung. Bleibt es mit 0.42+ bestehen, melden Sie es; diese Meldung kann andere Ursachen haben. |
| `youtfast: command not found` nach dem nativen Archiv | Nutzen Sie den Menüeintrag, oder melden Sie sich ab und wieder an, damit `~/.local/bin` im `PATH` ist. |
| `error while loading shared libraries: libmpv.so.2` | Installieren Sie das libmpv Ihrer Distribution (Schritt 1 Ihres Wegs) oder nutzen Sie das [Flatpak](#linux-flatpak). |
| `GLIBC_2.39 not found` | Ihr Linux ist älter, als das native Archiv unterstützt: Nutzen Sie das [Flatpak](#linux-flatpak). |
| Windows: „Der Computer wurde durch Windows geschützt“ | Wählen Sie **Weitere Informationen** und dann **Trotzdem ausführen** (siehe [Windows](#windows), Schritt 5). |
| macOS: „YouTFast kann nicht geöffnet werden“ | Nutzen Sie **Dennoch öffnen** unter Datenschutz & Sicherheit (siehe [macOS](#macos), Schritt 5). |

Kommen Sie nicht weiter? [Eröffnen Sie ein Issue](https://github.com/bcosta19/youtfast/issues)
mit Ihrem System, der Art, wie Sie YouTFast installiert haben, und dem, was
passiert ist. Portugiesisch ist willkommen.

Eine Prüfsumme zeigt, dass der Download vollständig und unverändert ist; sie ist
keine Herausgebersignatur. Um YouTFast stattdessen selbst zu bauen, siehe
[CONTRIBUTING.md](../../../CONTRIBUTING.md#getting-started).
