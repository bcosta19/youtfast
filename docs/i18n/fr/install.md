# Installer YouTFast

[Retour à la présentation](README.md) · [Guide d'utilisation](user-guide.md) ·
[Détails des plateformes](../../distribution.md) · [Ce guide sur le site web](https://youtfast.pages.dev/install)

[English](../../install.md) · [Português (Brasil)](../pt-BR/install.md) · [Español](../es/install.md) ·
**Français** · [Deutsch](../de/install.md) · [Italiano](../it/install.md)

*Ceci est une traduction du [document original en anglais](../../install.md) ; en cas de divergence, la version anglaise fait foi.*

YouTFast fonctionne sous Linux 64 bits Intel/AMD (x86_64), Windows 10/11 64 bits
et sur les Mac Apple Silicon sous macOS 15 ou plus récent. Il n'existe pas de
version pour téléphone, pas de build pour Linux/Windows ARM ni pour Mac Intel.

Chaque fichier provient de la
[dernière version sur GitHub](https://github.com/bcosta19/youtfast/releases/latest).
Flathub et l'AUR ne sont pas utilisés pour le moment (utilisez plutôt
directement le bundle Flatpak ou le paquet Arch). Sur macOS, un tap Homebrew
personnel est disponible sous `bcosta19/homebrew-tap` ; Winget
(`microsoft/winget-pkgs`) dispose de manifestes prêts, en attente d'acceptation
de la PR.

## 1. Trouver votre méthode

| Votre ordinateur | Suivez | Ce que vous installez |
|---|---|---|
| N'importe quel Linux (installateur rapide en une commande) | [Linux : installateur rapide](#linux--installateur-rapide) | Choisit automatiquement le paquet Arch ou le bundle Flatpak et vérifie le SHA-256 |
| N'importe quel Linux avec Flatpak (Fedora, Mint, Pop!_OS, elementary, Ubuntu avec Flatpak ajouté, Steam Deck…) | [Linux : Flatpak](#linux--flatpak) | Un seul bundle qui contient déjà libmpv, yt-dlp et Deno |
| Arch Linux, Manjaro, EndeavourOS, CachyOS, Omarchy | [Linux : paquet Arch](#linux--paquet-arch) | Un paquet pacman ; pacman installe tout le reste |
| Ubuntu 24.04+, Debian 13+, Linux Mint 22+ sans Flatpak | [Linux : Ubuntu et Debian](#linux--ubuntu-et-debian) | L'archive native plus trois étapes système |
| Fedora 40+ sans Flatpak | [Linux : Fedora](#linux--fedora) | L'archive native plus trois étapes système |
| Windows 10 ou 11 (64 bits) | [Windows](#windows) | Installateur par utilisateur (`.exe`), commande PowerShell en une ligne ou `.zip` portable |
| Mac Apple Silicon (M1 ou plus récent), macOS 15+ | [macOS](#macos) | Une application, plus yt-dlp et Deno via Homebrew |

Vous ne savez pas quelle méthode Linux choisir ? Lancez
l'[installateur rapide pour Linux](#linux--installateur-rapide) ou installez le
bundle **Flatpak** : il n'a besoin de rien d'autre de votre système. Pour savoir
si votre Linux est assez récent pour l'archive native, lancez `ldd --version` ;
la première ligne doit se terminer par 2.39 ou plus.

La lecture sous Linux en natif nécessite en plus **libmpv 0.42+ compilé avec
Lua**. Vérifiez la version du paquet libmpv installé (`apt-cache policy libmpv2`,
`dnf info --installed mpv-libs` ou `pacman -Q mpv`). Les versions de
distribution ci-dessous décrivent le minimum glibc requis, sans garantir que
leur mpv par défaut soit assez récent. Si le vôtre est plus ancien, mettez à
jour libmpv depuis une source de confiance ou utilisez une version Flatpak
contenant mpv 0.42+. Mettre à jour YouTFast seul ne met pas à jour la libmpv
d'un système natif.

## Linux : installateur rapide

Le script d'installation télécharge les fichiers de la dernière version depuis
GitHub, vérifie l'empreinte SHA-256 par rapport à `checksums.txt`, et installe
le paquet Arch (via `sudo pacman -U` sur les distributions basées sur Arch) ou
le bundle Flatpak (`--user` partout ailleurs) :

```sh
curl -fsSL https://youtfast.pages.dev/install.sh | sh
```

Vous pouvez aussi choisir explicitement la méthode ou supprimer une installation
Flatpak / locale à l'utilisateur :

```sh
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method flatpak
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method arch
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --uninstall
```

## Linux : Flatpak

Il vous faut : Flatpak installé. Beaucoup de distributions le fournissent ;
sinon, suivez [la page de configuration de Flatpak pour votre distribution](https://flatpak.org/setup/).

1. Ouvrez un terminal.
2. Téléchargez le bundle et vérifiez-le :

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   echo "Installing YouTFast $V"
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-v$V-x86_64.flatpak" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

   La dernière ligne doit afficher `youtfast-v…-x86_64.flatpak: OK`.
3. Installez-le pour votre utilisateur. La première fois, Flatpak télécharge
   aussi l'environnement d'exécution Freedesktop dont il a besoin ; répondez `y`
   lorsqu'on vous le demande.

   ```sh
   flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
   flatpak install --user "./youtfast-v$V-x86_64.flatpak"
   ```

4. Ouvrez **YouTFast** depuis le menu des applications, ou lancez
   `flatpak run io.github.bcosta19.YouTFast`.

**Mise à jour :** répétez les étapes 2 et 3 (ou exécutez
`install.sh --method flatpak` depuis **Paramètres → À propos**). Le bundle en
bac à sable n'a pas de canal de mise à jour automatique.
**Désinstallation :** `flatpak uninstall --user io.github.bcosta19.YouTFast`.

Le Flatpak ne peut pas lire le profil de votre navigateur. Pour vous connecter,
choisissez un fichier cookies.txt dans l'application, ou restez déconnecté.

## Linux : paquet Arch

Il vous faut : un système basé sur Arch avec `sudo`.

1. Ouvrez un terminal.
2. Téléchargez le paquet et vérifiez-le :

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-$V-1-x86_64.pkg.tar.zst" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

3. Installez-le. pacman installe aussi mpv, yt-dlp, Deno et les autres
   bibliothèques dont YouTFast a besoin. Les versions contenant le correctif
   #84 nécessitent mpv 0.42+ ; les paquets publiés plus anciens ne l'imposent
   pas. Vérifiez `pacman -Q mpv` et mettez à jour le système avec
   `sudo pacman -Syu` si nécessaire :

   ```sh
   sudo pacman -U "./youtfast-$V-1-x86_64.pkg.tar.zst"
   ```

4. Ouvrez **YouTFast** depuis le menu des applications, ou lancez `youtfast`.

**Mise à jour :** YouTFast se met à jour via **Paramètres → À propos**
(télécharge et vérifie le paquet `.pkg.tar.zst`, puis exécute
`pkexec pacman -U`), ou répétez les étapes 2 et 3 avec la nouvelle version.
Gardez le système à jour avec `sudo pacman -Syu` pour que yt-dlp reste à jour.
**Désinstallation :** `sudo pacman -R youtfast`.

## Linux : Ubuntu et Debian

Il vous faut : Ubuntu 24.04 ou plus récent, Debian 13 ou plus récent, ou une
distribution basée sur ceux-ci (Linux Mint 22+, Pop!_OS 24.04+), et `sudo`.

1. Installez les bibliothèques qu'utilise YouTFast, plus `pipx` et `unzip` pour
   l'étape suivante :

   ```sh
   sudo apt-get update
   sudo apt-get install libmpv2 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 \
     libgl1 libfontconfig1 pipx unzip curl
   ```

2. [Installez yt-dlp et Deno à jour](#yt-dlp-et-deno-sous-linux) comme décrit
   ci-dessous. N'utilisez pas le paquet apt `yt-dlp` : il est trop ancien pour
   lire les vidéos YouTube.
3. [Installez l'archive YouTFast](#installer-larchive-native) comme décrit
   ci-dessous.

## Linux : Fedora

Il vous faut : Fedora 40 ou plus récent et `sudo`.

1. Installez les bibliothèques qu'utilise YouTFast, plus `pipx` et `unzip` :

   ```sh
   sudo dnf install mpv-libs libxkbcommon-x11 mesa-libGL fontconfig pipx unzip curl
   ```

2. [Installez yt-dlp et Deno à jour](#yt-dlp-et-deno-sous-linux).
3. [Installez l'archive YouTFast](#installer-larchive-native).

Les autres distributions avec glibc 2.39 ou plus récent peuvent suivre les mêmes
trois étapes : installez libmpv 0.42+ (compilé avec Lua), libxkbcommon, la
bibliothèque client de Wayland, OpenGL et fontconfig sous les noms utilisés par
votre distribution.

### yt-dlp et Deno sous Linux

YouTFast demande à yt-dlp les flux de chaque vidéo, et yt-dlp a besoin de Deno et
de son composant EJS pour lire les pages de YouTube. Ces commandes installent
les trois pour votre utilisateur, dans `~/.local/bin` :

```sh
pipx ensurepath
pipx install "yt-dlp[default]"
curl -fsSL https://deno.land/install.sh | DENO_INSTALL="$HOME/.local" sh -s -- -y --no-modify-path
```

`yt-dlp[default]` inclut le composant EJS. Si `pipx ensurepath` indique avoir
modifié votre `PATH`, **déconnectez-vous puis reconnectez-vous** une fois, pour
que les programmes ouverts depuis le menu des applications trouvent aussi
yt-dlp. Vérifiez dans un nouveau terminal :

```sh
yt-dlp --version
deno --version
```

Gardez yt-dlp à jour : lorsque les vidéos ne se chargent plus, lancez d'abord
`pipx upgrade yt-dlp`.

### Installer l'archive native

1. Téléchargez l'archive et vérifiez-la :

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-v$V-x86_64-unknown-linux-gnu.tar.gz" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

2. Décompressez-la et installez le programme, son icône et son entrée de menu
   pour votre utilisateur :

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

   La dernière commande affiche la version ; elle n'ouvre pas la fenêtre.
3. Ouvrez **YouTFast** depuis le menu des applications, ou lancez `youtfast`.

**Mise à jour :** YouTFast se met à jour lui-même depuis **Paramètres → À
propos**. Laissez le programme là où l'étape 2 l'a placé, pour que l'outil de
mise à jour puisse le remplacer. Mettez à jour yt-dlp avec `pipx upgrade yt-dlp`
et les bibliothèques avec les mises à jour de votre système.
**Désinstallation :**

```sh
rm ~/.local/bin/youtfast ~/.local/share/applications/youtfast.desktop \
  ~/.local/share/icons/hicolor/scalable/apps/youtfast.svg
```

## Windows

Il vous faut : Windows 10 ou 11 64 bits. Winget (Programme d'installation
d'application), inclus dans les versions récentes de Windows, est utilisé si
`yt-dlp` ou `Deno` ne sont pas déjà installés.

### Option A : installateur Windows (.exe) — recommandé

1. Téléchargez `youtfast-v<version>-x86_64-windows-setup.exe` depuis la
   [dernière version](https://github.com/bcosta19/youtfast/releases/latest), ou
   lancez l'installateur PowerShell vérifié en une commande :

   ```powershell
   irm https://youtfast.pages.dev/install.ps1 | iex
   ```

2. À l'ouverture du `.exe`, Windows SmartScreen avertit au sujet des
   applications sans signature d'éditeur, ce qui est le cas de YouTFast :
   choisissez **Informations complémentaires**, puis **Exécuter quand même**.
   Pour vérifier d'abord le fichier, lancez
   `Get-FileHash .\youtfast-v*-x86_64-windows-setup.exe` et comparez avec
   `checksums.txt`.
3. Suivez l'assistant d'installation. Il installe pour l'utilisateur courant dans
   `%LOCALAPPDATA%\Programs\YouTFast` (sans demande d'Administrateur), crée des
   raccourcis dans le menu Démarrer et sur le Bureau, et installe `yt-dlp` et
   `Deno` manquants via `winget` si l'option est activée. Déconnectez-vous puis
   reconnectez-vous si vous avez dû installer `yt-dlp` ou `Deno` pour la
   première fois.

### Option B : ZIP portable (.zip)

1. Installez yt-dlp et Deno. Ouvrez **Terminal** ou **PowerShell** et lancez :

   ```powershell
   winget install --id yt-dlp.yt-dlp
   winget install --id DenoLand.Deno
   ```

   Ensuite, **déconnectez-vous de Windows puis reconnectez-vous**, pour que les
   programmes nouvellement lancés les trouvent. Vérifiez dans un nouveau
   terminal : `yt-dlp --version` et `deno --version` affichent tous deux une
   version.
2. Téléchargez `youtfast-v<version>-x86_64-pc-windows-gnu.zip` depuis la
   [dernière version](https://github.com/bcosta19/youtfast/releases/latest).
3. Facultatif, pour vérifier le téléchargement : dans le dossier Téléchargements,
   lancez `Get-FileHash .\youtfast-v*-x86_64-pc-windows-gnu.zip` et comparez le
   `Hash` avec la ligne correspondant à ce fichier dans le `checksums.txt` de la
   version (la casse n'a pas d'importance).
4. Faites un clic droit sur le ZIP, choisissez **Extraire tout…**, et choisissez
   un emplacement durable, par exemple `Documents\YouTFast`. Gardez tous les
   fichiers du dossier ensemble : le programme a besoin des DLL placées à côté
   de lui.
5. Ouvrez `youtfast.exe` dans le dossier extrait et passez outre SmartScreen
   (**Informations complémentaires** → **Exécuter quand même**).

**Mise à jour :** YouTFast se met à jour lui-même depuis **Paramètres → À
propos**. Mettez à jour yt-dlp avec `winget upgrade --id yt-dlp.yt-dlp` lorsque
les vidéos ne se chargent plus.
**Désinstallation :** utilisez **Paramètres → Applications → Applications
installées → YouTFast** de Windows (ou lancez `Uninstall.exe`) si vous l'avez
installé via le `.exe`, ou supprimez le dossier extrait si vous utilisez le
`.zip` portable.

## macOS

Il vous faut : un Mac Apple Silicon (M1 ou plus récent) sous macOS 15 ou plus
récent, et [Homebrew](https://brew.sh/).

### Option A : tap Homebrew — recommandé

```sh
brew install --cask bcosta19/tap/youtfast
```

Le cask installe `YouTFast.app`, crée le lien de la commande `youtfast`,
installe automatiquement `yt-dlp` et `deno`, et retire `com.apple.quarantine`
du bundle de l'application signé ad hoc. (Remarque : mettez à jour le cask du
tap en même temps que les nouvelles versions de YouTFast, ou utilisez
**Paramètres → À propos** dans l'application.)

### Option B : ZIP manuel

1. Installez yt-dlp et Deno. Dans **Terminal** :

   ```sh
   brew install yt-dlp deno
   ```

2. Téléchargez `youtfast-v<version>-aarch64-apple-darwin.zip` depuis la
   [dernière version](https://github.com/bcosta19/youtfast/releases/latest).
3. Facultatif, pour vérifier le téléchargement : dans Terminal, lancez
   `shasum -a 256 ~/Downloads/youtfast-v*-aarch64-apple-darwin.zip` et
   comparez le résultat avec la ligne correspondant à ce fichier dans le
   `checksums.txt` de la version.
4. Double-cliquez sur le ZIP pour le décompresser, puis faites glisser
   **YouTFast.app** dans **Applications**.
5. Ouvrez YouTFast depuis Applications. L'application n'est pas notariée par
   Apple, donc la première fois macOS la refuse : cliquez sur **Terminé**,
   ouvrez **Réglages Système → Confidentialité et sécurité**, faites défiler
   jusqu'au message concernant YouTFast, cliquez sur **Ouvrir quand même** et
   confirmez avec votre mot de passe. Les lancements suivants s'ouvrent
   normalement.

**Mise à jour :** YouTFast se met à jour lui-même depuis **Paramètres → À
propos**. Mettez à jour yt-dlp avec `brew upgrade yt-dlp` lorsque les vidéos ne
se chargent plus.
**Désinstallation :** faites glisser YouTFast.app d'Applications vers la Corbeille.

## Vos données

Désinstaller YouTFast conserve votre bibliothèque et vos paramètres. Supprimez
aussi ces dossiers pour une désinstallation complète :

| Installé avec | Bibliothèque | Paramètres |
|---|---|---|
| Paquet Arch ou archive native | `~/.local/share/youtfast` | `~/.config/youtfast` |
| Flatpak | `~/.var/app/io.github.bcosta19.YouTFast` | (même dossier) |
| Windows | `%APPDATA%\youtfast\data` | `%APPDATA%\youtfast\config` |
| macOS | `~/Library/Application Support/youtfast` | (même dossier) |

Une session connectée est conservée dans le gestionnaire d'identifiants du
système, pas dans ces dossiers : déconnectez-vous dans l'application avant de
la désinstaller.

## En cas de problème

| Ce que vous voyez | Que faire |
|---|---|
| La fenêtre s'ouvre mais les vidéos ne démarrent jamais | Mettez à jour yt-dlp (voir votre méthode ci-dessus). Vérifiez que `yt-dlp --version` et `deno --version` fonctionnent dans un nouveau terminal. |
| `YouTFast n'a pas pu démarrer yt-dlp…` | yt-dlp n'est pas installé, ou YouTFast a été ouvert avec un `PATH` antérieur à son installation. Vérifiez que `yt-dlp --version` et `deno --version` fonctionnent dans un nouveau terminal et installez ce qui manque (voir votre méthode ci-dessus). Sous Windows, déconnectez-vous puis reconnectez-vous après les avoir installés, puis ouvrez YouTFast depuis le menu Démarrer. |
| `yt-dlp n'a pas pu obtenir les flux de cette vidéo…` | Mettez à jour yt-dlp (voir votre méthode ci-dessus) et vérifiez que `deno --version` fonctionne dans un nouveau terminal. Si le problème persiste avec un yt-dlp à jour, signalez-le. |
| `mpv n'a pas pu reconnaître le format du flux.` (0.9.0 et versions antérieures) | Ces versions l'affichent quand yt-dlp manque ou échoue : suivez les deux lignes ci-dessus. |
| Par intermittence : `Impossible de lire cette vidéo. Le lecteur n'a pas pu identifier la cause. Essayez de rouvrir la vidéo.` | Vérifiez la version du paquet libmpv installé. Avant 0.42, son hook yt-dlp peut ouvrir les flux trop tôt et recevoir une erreur HTTP 403. Mettez à jour libmpv vers 0.42+ avec Lua, ou utilisez une version Flatpak qui la contient. Rouvrir après quelques secondes est une solution de contournement temporaire, pas une correction. Si le problème persiste avec 0.42+, signalez-le ; ce message peut avoir d'autres causes. |
| `youtfast: command not found` après l'archive native | Utilisez l'entrée de menu, ou déconnectez-vous puis reconnectez-vous pour que `~/.local/bin` soit dans le `PATH`. |
| `error while loading shared libraries: libmpv.so.2` | Installez la libmpv de votre distribution (étape 1 de votre méthode), ou utilisez le [Flatpak](#linux--flatpak). |
| `GLIBC_2.39 not found` | Votre Linux est plus ancien que ce que l'archive native prend en charge : utilisez le [Flatpak](#linux--flatpak). |
| Windows : « Windows a protégé votre ordinateur » | Choisissez **Informations complémentaires**, puis **Exécuter quand même** (voir [Windows](#windows), étape 5). |
| macOS : « Impossible d'ouvrir YouTFast » | Utilisez **Ouvrir quand même** dans Confidentialité et sécurité (voir [macOS](#macos), étape 5). |

Toujours bloqué ? [Ouvrez une issue](https://github.com/bcosta19/youtfast/issues)
en indiquant votre système, comment vous avez installé YouTFast et ce qui s'est
passé. Le portugais est le bienvenu.

Une somme de contrôle montre que le téléchargement est complet et intact ; ce
n'est pas une signature d'éditeur. Pour compiler YouTFast vous-même, voir
[CONTRIBUTING.md](../../../CONTRIBUTING.md#getting-started).
