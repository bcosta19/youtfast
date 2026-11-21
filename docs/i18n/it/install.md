# Installare YouTFast

[Torna alla panoramica](README.md) · [Guida utente](user-guide.md) ·
[Dettagli delle piattaforme](../../distribution.md) · [Questa guida sul sito web](https://youtfast.pages.dev/install)

[English](../../install.md) · [Português (Brasil)](../pt-BR/install.md) · [Español](../es/install.md) ·
[Français](../fr/install.md) · [Deutsch](../de/install.md) · **Italiano**

*Questa è una traduzione del [documento originale in inglese](../../install.md); in caso di differenze, prevale la versione inglese.*

YouTFast funziona su Linux Intel/AMD a 64 bit (x86_64), su Windows 10/11 a 64 bit e
sui Mac con Apple Silicon con macOS 15 o successivo. Non esiste una versione per
telefono, né una build per Linux/Windows ARM, né una build per Mac Intel.

Ogni file proviene dalla
[release più recente su GitHub](https://github.com/bcosta19/youtfast/releases/latest).
Flathub e AUR non sono usati al momento (usa invece direttamente il bundle Flatpak o
il pacchetto Arch). Su macOS è disponibile un tap Homebrew personale in
`bcosta19/homebrew-tap`; per Winget (`microsoft/winget-pkgs`) ci sono manifest
pronti in attesa dell'accettazione della PR.

## 1. Trova il tuo percorso

| Il tuo computer | Segui | Cosa installi |
|---|---|---|
| Qualsiasi Linux (installer rapido con 1 comando) | [Linux: installer rapido](#linux-installer-rapido) | Sceglie automaticamente il pacchetto Arch o il bundle Flatpak e verifica lo SHA-256 |
| Qualsiasi Linux con Flatpak (Fedora, Mint, Pop!_OS, elementary, Ubuntu con Flatpak aggiunto, Steam Deck…) | [Linux: Flatpak](#linux-flatpak) | Un unico bundle che contiene già libmpv, yt-dlp e Deno |
| Arch Linux, Manjaro, EndeavourOS, CachyOS, Omarchy | [Linux: pacchetto Arch](#linux-pacchetto-arch) | Un pacchetto pacman; pacman installa tutto il resto |
| Ubuntu 24.04+, Debian 13+, Linux Mint 22+ senza Flatpak | [Linux: Ubuntu e Debian](#linux-ubuntu-e-debian) | L'archivio nativo più tre passaggi di sistema |
| Fedora 40+ senza Flatpak | [Linux: Fedora](#linux-fedora) | L'archivio nativo più tre passaggi di sistema |
| Windows 10 o 11 (64 bit) | [Windows](#windows) | Installer per utente (`.exe`), un comando PowerShell su una riga o `.zip` portatile |
| Mac con Apple Silicon (M1 o successivo), macOS 15+ | [macOS](#macos) | Un'app, più yt-dlp e Deno tramite Homebrew |

Non sai quale percorso Linux scegliere? Esegui l'[installer rapido per Linux](#linux-installer-rapido)
o installa il bundle **Flatpak**: non richiede nient'altro dal tuo sistema. Per
sapere se il tuo Linux è abbastanza recente per l'archivio nativo, esegui
`ldd --version`; la prima riga deve terminare con 2.39 o superiore.

La riproduzione nativa su Linux richiede inoltre **libmpv 0.42+ compilata con Lua**.
Controlla la versione del pacchetto libmpv installato (`apt-cache policy libmpv2`,
`dnf info --installed mpv-libs` o `pacman -Q mpv`). Le versioni delle distribuzioni
indicate sotto descrivono la base di glibc, non garantiscono che la loro mpv
predefinita sia abbastanza recente. Se la tua è più vecchia, aggiorna libmpv da una
fonte affidabile o usa una release Flatpak che contenga mpv 0.42+. Aggiornare solo
YouTFast non aggiorna la libmpv di un sistema nativo.

## Linux: installer rapido

Lo script di installazione scarica da GitHub i file della release più recente,
verifica il digest SHA-256 rispetto a `checksums.txt` e installa il pacchetto Arch
(tramite `sudo pacman -U` sulle distribuzioni basate su Arch) o il bundle Flatpak
(`--user` ovunque altrove):

```sh
curl -fsSL https://youtfast.pages.dev/install.sh | sh
```

Puoi anche scegliere esplicitamente il metodo o rimuovere un'installazione Flatpak /
locale per l'utente:

```sh
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method flatpak
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method arch
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --uninstall
```

## Linux: Flatpak

Ti serve: Flatpak installato. Molte distribuzioni lo includono; altrimenti segui la
[pagina di configurazione di Flatpak per la tua distribuzione](https://flatpak.org/setup/).

1. Apri un terminale.
2. Scarica il bundle e verificalo:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   echo "Installing YouTFast $V"
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-v$V-x86_64.flatpak" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

   L'ultima riga deve stampare `youtfast-v…-x86_64.flatpak: OK`.
3. Installalo per il tuo utente. La prima volta Flatpak scarica anche il runtime
   Freedesktop di cui ha bisogno; rispondi `y` quando richiesto.

   ```sh
   flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
   flatpak install --user "./youtfast-v$V-x86_64.flatpak"
   ```

4. Apri **YouTFast** dal menu delle applicazioni, oppure esegui
   `flatpak run io.github.bcosta19.YouTFast`.

**Aggiornare:** ripeti i passaggi 2 e 3 (oppure esegui
`install.sh --method flatpak` da **Impostazioni → Informazioni**). Il bundle in
sandbox non ha un canale di aggiornamento automatico.
**Rimuovere:** `flatpak uninstall --user io.github.bcosta19.YouTFast`.

Il Flatpak non può leggere il profilo del tuo browser. Per accedere, scegli un file
cookies.txt nell'app, oppure resta senza accesso.

## Linux: pacchetto Arch

Ti serve: un sistema basato su Arch con `sudo`.

1. Apri un terminale.
2. Scarica il pacchetto e verificalo:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-$V-1-x86_64.pkg.tar.zst" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

3. Installalo. pacman installa anche mpv, yt-dlp, Deno e le altre librerie di cui
   YouTFast ha bisogno. Le release che contengono la correzione #84 richiedono mpv
   0.42+; i pacchetti pubblicati in precedenza non lo impongono. Controlla
   `pacman -Q mpv` e, se serve, aggiorna il sistema con `sudo pacman -Syu`:

   ```sh
   sudo pacman -U "./youtfast-$V-1-x86_64.pkg.tar.zst"
   ```

4. Apri **YouTFast** dal menu delle applicazioni, oppure esegui `youtfast`.

**Aggiornare:** YouTFast si aggiorna tramite **Impostazioni → Informazioni**
(scarica e verifica il pacchetto `.pkg.tar.zst`, poi esegue `pkexec pacman -U`),
oppure ripeti i passaggi 2 e 3 con la nuova release. Mantieni il sistema
aggiornato con `sudo pacman -Syu` così yt-dlp resta recente.
**Rimuovere:** `sudo pacman -R youtfast`.

## Linux: Ubuntu e Debian

Ti serve: Ubuntu 24.04 o successivo, Debian 13 o successivo, o una distribuzione
basata su di essi (Linux Mint 22+, Pop!_OS 24.04+), e `sudo`.

1. Installa le librerie usate da YouTFast, più `pipx` e `unzip` per il passaggio
   successivo:

   ```sh
   sudo apt-get update
   sudo apt-get install libmpv2 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 \
     libgl1 libfontconfig1 pipx unzip curl
   ```

2. [Installa yt-dlp e Deno aggiornati](#yt-dlp-e-deno-su-linux) come descritto
   sotto. Non usare il pacchetto apt `yt-dlp`: è troppo vecchio per riprodurre i
   video di YouTube.
3. [Installa l'archivio di YouTFast](#installare-larchivio-nativo) come descritto
   sotto.

## Linux: Fedora

Ti serve: Fedora 40 o successivo e `sudo`.

1. Installa le librerie usate da YouTFast, più `pipx` e `unzip`:

   ```sh
   sudo dnf install mpv-libs libxkbcommon-x11 mesa-libGL fontconfig pipx unzip curl
   ```

2. [Installa yt-dlp e Deno aggiornati](#yt-dlp-e-deno-su-linux).
3. [Installa l'archivio di YouTFast](#installare-larchivio-nativo).

Le altre distribuzioni con glibc 2.39 o successiva possono seguire gli stessi tre
passaggi: installa libmpv 0.42+ (compilata con Lua), libxkbcommon, la libreria
client di Wayland, OpenGL e fontconfig con i nomi usati dalla tua distribuzione.

### yt-dlp e Deno su Linux

YouTFast chiede a yt-dlp i flussi di ogni video, e yt-dlp ha bisogno di Deno e del
suo componente EJS per leggere le pagine di YouTube. Questi comandi installano
tutti e tre per il tuo utente, in `~/.local/bin`:

```sh
pipx ensurepath
pipx install "yt-dlp[default]"
curl -fsSL https://deno.land/install.sh | DENO_INSTALL="$HOME/.local" sh -s -- -y --no-modify-path
```

`yt-dlp[default]` include il componente EJS. Se `pipx ensurepath` dice di aver
modificato il tuo `PATH`, **esci dalla sessione e accedi di nuovo** una volta, così
anche i programmi aperti dal menu delle applicazioni trovano yt-dlp. Verifica in
un nuovo terminale:

```sh
yt-dlp --version
deno --version
```

Mantieni yt-dlp aggiornato: quando i video smettono di caricarsi, esegui prima di
tutto `pipx upgrade yt-dlp`.

### Installare l'archivio nativo

1. Scarica l'archivio e verificalo:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-v$V-x86_64-unknown-linux-gnu.tar.gz" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

2. Estrailo e installa il programma, la sua icona e la sua voce di menu per il tuo
   utente:

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

   L'ultimo comando stampa la versione; non apre la finestra.
3. Apri **YouTFast** dal menu delle applicazioni, oppure esegui `youtfast`.

**Aggiornare:** YouTFast si aggiorna da solo da **Impostazioni → Informazioni**.
Lascia il programma dove l'ha messo il passaggio 2, così il sistema di
aggiornamento può sostituirlo. Aggiorna yt-dlp con `pipx upgrade yt-dlp` e le
librerie con gli aggiornamenti del tuo sistema.
**Rimuovere:**

```sh
rm ~/.local/bin/youtfast ~/.local/share/applications/youtfast.desktop \
  ~/.local/share/icons/hicolor/scalable/apps/youtfast.svg
```

## Windows

Ti serve: Windows 10 o 11 a 64 bit. Winget (Programma di installazione app),
incluso nelle versioni recenti di Windows, viene usato se `yt-dlp` o `Deno` non
sono già installati.

### Opzione A: installer Windows (.exe) — consigliata

1. Scarica `youtfast-v<version>-x86_64-windows-setup.exe` dalla
   [release più recente](https://github.com/bcosta19/youtfast/releases/latest), oppure
   esegui l'installer PowerShell verificato con un solo comando:

   ```powershell
   irm https://youtfast.pages.dev/install.ps1 | iex
   ```

2. All'apertura del `.exe`, Windows SmartScreen avvisa delle app senza firma
   dell'editore, che YouTFast non ha: scegli **Ulteriori informazioni**, poi
   **Esegui comunque**. Per verificare prima il file, esegui
   `Get-FileHash .\youtfast-v*-x86_64-windows-setup.exe` e confronta con
   `checksums.txt`.
3. Segui la procedura guidata dell'installer. Installa per l'utente corrente in
   `%LOCALAPPDATA%\Programs\YouTFast` (nessuna richiesta di amministratore), crea i
   collegamenti nel menu Start e sul desktop e installa `yt-dlp` e `Deno` mancanti
   tramite `winget`, se l'opzione è attiva. Esci da Windows e accedi di nuovo se hai
   dovuto installare `yt-dlp` o `Deno` per la prima volta.

### Opzione B: ZIP portatile (.zip)

1. Installa yt-dlp e Deno. Apri **Terminale** o **PowerShell** ed esegui:

   ```powershell
   winget install --id yt-dlp.yt-dlp
   winget install --id DenoLand.Deno
   ```

   Poi **esci da Windows e accedi di nuovo**, così i programmi avviati da quel
   momento li trovano. Verifica in un nuovo terminale: `yt-dlp --version` e
   `deno --version` stampano entrambi una versione.
2. Scarica `youtfast-v<version>-x86_64-pc-windows-gnu.zip` dalla
   [release più recente](https://github.com/bcosta19/youtfast/releases/latest).
3. Facoltativo, per verificare il download: nella cartella Download esegui
   `Get-FileHash .\youtfast-v*-x86_64-pc-windows-gnu.zip` e confronta l'`Hash` con
   la riga di quel file nel `checksums.txt` della release (maiuscole e minuscole
   non contano).
4. Fai clic con il tasto destro sullo ZIP, scegli **Estrai tutto…** e seleziona una
   posizione permanente, per esempio `Documents\YouTFast`. Tieni insieme tutti i
   file della cartella: il programma ha bisogno delle DLL accanto a sé.
5. Apri `youtfast.exe` nella cartella estratta e supera SmartScreen (**Ulteriori
   informazioni** → **Esegui comunque**).

**Aggiornare:** YouTFast si aggiorna da solo da **Impostazioni → Informazioni**.
Aggiorna yt-dlp con `winget upgrade --id yt-dlp.yt-dlp` quando i video smettono di
caricarsi.
**Rimuovere:** usa **Impostazioni → App → App installate → YouTFast** di Windows (o
esegui `Uninstall.exe`) se l'hai installato tramite `.exe`, oppure elimina la
cartella estratta se usi lo `.zip` portatile.
**Google Cast (facoltativo):** la trasmissione usa l'app di YouTube della TV e non
richiede strumenti extra. Per trovare anche dispositivi Chromecast, installa
catt con `pip install --user catt` (o `pipx install catt`): YouTFast controlla le
cartelle che usano pip e pipx, quindi non serve modificare il `PATH`, e un
YouTFast in esecuzione lo trova alla ricerca successiva.

## macOS

Ti serve: un Mac con Apple Silicon (M1 o successivo) con macOS 15 o successivo, e
[Homebrew](https://brew.sh/).

### Opzione A: tap Homebrew — consigliata

```sh
brew install --cask bcosta19/tap/youtfast
```

Il cask installa `YouTFast.app`, collega il comando `youtfast`, installa
automaticamente `yt-dlp` e `deno` e rimuove `com.apple.quarantine` dal bundle
dell'app con firma ad hoc. Il tap segue ogni release pubblicata: aggiorna con
`brew upgrade --cask youtfast` oppure con **Impostazioni → Informazioni** nell'app.

### Opzione B: ZIP manuale

1. Installa yt-dlp e Deno. Nel **Terminale**:

   ```sh
   brew install yt-dlp deno
   ```

2. Scarica `youtfast-v<version>-aarch64-apple-darwin.zip` dalla
   [release più recente](https://github.com/bcosta19/youtfast/releases/latest).
3. Facoltativo, per verificare il download: nel Terminale esegui
   `shasum -a 256 ~/Downloads/youtfast-v*-aarch64-apple-darwin.zip` e
   confronta il risultato con la riga di quel file nel `checksums.txt` della
   release.
4. Fai doppio clic sullo ZIP per estrarlo, poi trascina **YouTFast.app** in
   **Applicazioni**.
5. Apri YouTFast da Applicazioni. L'app non è notarizzata da Apple, quindi la prima
   volta macOS la rifiuta: fai clic su **Fine**, apri **Impostazioni di Sistema →
   Privacy e sicurezza**, scorri fino al messaggio su YouTFast, fai clic su
   **Apri comunque** e conferma con la password. Gli avvii successivi si aprono
   normalmente.

**Aggiornare:** YouTFast si aggiorna da solo da **Impostazioni → Informazioni**.
Aggiorna yt-dlp con `brew upgrade yt-dlp` quando i video smettono di caricarsi.
**Rimuovere:** trascina YouTFast.app da Applicazioni al Cestino.

## I tuoi dati

Rimuovere YouTFast conserva la raccolta e le impostazioni. Elimina anche queste
cartelle per una rimozione completa:

| Installato con | Raccolta | Impostazioni |
|---|---|---|
| Pacchetto Arch o archivio nativo | `~/.local/share/youtfast` | `~/.config/youtfast` |
| Flatpak | `~/.var/app/io.github.bcosta19.YouTFast` | (stessa cartella) |
| Windows | `%APPDATA%\youtfast\data` | `%APPDATA%\youtfast\config` |
| macOS | `~/Library/Application Support/youtfast` | (stessa cartella) |

Una sessione con accesso effettuato è conservata nell'archivio delle credenziali del
sistema, non in queste cartelle: esci dall'account nell'app prima di rimuoverla.

## Se qualcosa va storto

| Cosa vedi | Cosa fare |
|---|---|
| La finestra si apre ma i video non partono mai | Aggiorna yt-dlp (vedi il tuo percorso sopra). Verifica che `yt-dlp --version` e `deno --version` funzionino in un nuovo terminale. |
| `YouTFast non è riuscito ad avviare yt-dlp…` | yt-dlp non è installato, oppure YouTFast è stato aperto con un `PATH` precedente alla sua installazione. Verifica che `yt-dlp --version` e `deno --version` funzionino in un nuovo terminale e installa ciò che manca (vedi il tuo percorso sopra). Su Windows, esci dalla sessione e accedi di nuovo dopo averli installati, poi apri YouTFast dal menu Start. |
| `yt-dlp non è riuscito a ottenere gli stream di questo video…` | Aggiorna yt-dlp (vedi il tuo percorso sopra) e verifica che `deno --version` funzioni in un nuovo terminale. Se persiste con un yt-dlp aggiornato, segnalalo. |
| `mpv non è riuscito a riconoscere il formato dello stream.` (0.9.0 e precedenti) | Queste versioni lo mostrano quando yt-dlp manca o fallisce: segui le due righe sopra. |
| In modo intermittente: `Impossibile riprodurre questo video. Il player non ha identificato la causa. Prova a riaprire il video.` | Controlla la versione del pacchetto libmpv installato. Prima della 0.42 il suo hook per yt-dlp può aprire i flussi troppo presto e ricevere un HTTP 403. Aggiorna libmpv alla 0.42+ con Lua, oppure usa una release Flatpak che la contenga. Riaprire dopo qualche secondo è una soluzione temporanea, non una correzione. Se persiste con la 0.42+, segnalalo; questo messaggio può avere altre cause. |
| `youtfast: command not found` dopo l'archivio nativo | Usa la voce di menu, oppure esci dalla sessione e accedi di nuovo così `~/.local/bin` è nel `PATH`. |
| `error while loading shared libraries: libmpv.so.2` | Installa la libmpv della tua distribuzione (passaggio 1 del tuo percorso), oppure usa il [Flatpak](#linux-flatpak). |
| `GLIBC_2.39 not found` | Il tuo Linux è più vecchio di quanto supportato dall'archivio nativo: usa il [Flatpak](#linux-flatpak). |
| Windows: "PC protetto da Windows" | Scegli **Ulteriori informazioni**, poi **Esegui comunque** (vedi [Windows](#windows), passaggio 5). |
| macOS: "Impossibile aprire YouTFast" | Usa **Apri comunque** in Privacy e sicurezza (vedi [macOS](#macos), passaggio 5). |

Ancora bloccato? [Apri una issue](https://github.com/bcosta19/youtfast/issues)
indicando il tuo sistema, come hai installato YouTFast e cosa è successo. Le
segnalazioni in portoghese sono benvenute.

Un checksum dimostra che il download è completo e non modificato; non è una firma
dell'editore. Per compilare invece YouTFast da solo, vedi
[CONTRIBUTING.md](../../../CONTRIBUTING.md#getting-started).
