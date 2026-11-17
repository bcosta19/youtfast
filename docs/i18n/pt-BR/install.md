# Instalando o YouTFast

[Voltar à visão geral](README.md) · [Guia do usuário](user-guide.md) ·
[Detalhes das plataformas](../../distribution.md) · [Este guia no site](https://youtfast.pages.dev/install)

[English](../../install.md) · **Português (Brasil)** · [Español](../es/install.md) ·
[Français](../fr/install.md) · [Deutsch](../de/install.md) · [Italiano](../it/install.md)

*Esta é uma tradução do [documento original em inglês](../../install.md); se houver diferenças entre os dois, vale a versão em inglês.*

O YouTFast roda em Linux Intel/AMD de 64 bits (x86_64), Windows 10/11 de 64 bits
e Macs com Apple Silicon no macOS 15 ou mais recente. Não há versão para celular,
nem build para Linux/Windows em ARM, nem build para Macs Intel.

Todos os arquivos vêm da
[versão mais recente no GitHub](https://github.com/bcosta19/youtfast/releases/latest).
O Flathub e o AUR não são usados no momento (use diretamente o bundle Flatpak ou o
pacote Arch). No macOS, há um tap pessoal do Homebrew em
`bcosta19/homebrew-tap`; o Winget (`microsoft/winget-pkgs`) tem manifestos
prontos, aguardando a aceitação do PR.

## 1. Encontre o seu caminho

| Seu computador | Siga | O que você instala |
|---|---|---|
| Qualquer Linux (instalador rápido com 1 comando) | [Linux: instalador rápido](#linux-instalador-rápido) | Escolhe automaticamente o pacote Arch ou o bundle Flatpak e confere o SHA-256 |
| Qualquer Linux com Flatpak (Fedora, Mint, Pop!_OS, elementary, Ubuntu com Flatpak adicionado, Steam Deck…) | [Linux: Flatpak](#linux-flatpak) | Um bundle que já contém libmpv, yt-dlp e Deno |
| Arch Linux, Manjaro, EndeavourOS, CachyOS, Omarchy | [Linux: pacote Arch](#linux-pacote-arch) | Um pacote pacman; o pacman instala todo o resto |
| Ubuntu 24.04+, Debian 13+, Linux Mint 22+ sem Flatpak | [Linux: Ubuntu e Debian](#linux-ubuntu-e-debian) | O arquivo compactado nativo mais três etapas no sistema |
| Fedora 40+ sem Flatpak | [Linux: Fedora](#linux-fedora) | O arquivo compactado nativo mais três etapas no sistema |
| Windows 10 ou 11 (64 bits) | [Windows](#windows) | Instalador por usuário (`.exe`), comando único no PowerShell ou `.zip` portátil |
| Mac com Apple Silicon (M1 ou mais recente), macOS 15+ | [macOS](#macos) | Um app, mais yt-dlp e Deno pelo Homebrew |

Não sabe qual caminho seguir no Linux? Rode o [instalador rápido para Linux](#linux-instalador-rápido)
ou instale o bundle **Flatpak**: ele não precisa de mais nada do seu sistema.
Para saber se o seu Linux é novo o bastante para o arquivo compactado nativo,
rode `ldd --version`; a primeira linha precisa terminar em 2.39 ou mais.

A reprodução nativa no Linux também exige a **libmpv 0.42+ compilada com Lua**.
Confira a versão do pacote libmpv instalado (`apt-cache policy libmpv2`,
`dnf info --installed mpv-libs` ou `pacman -Q mpv`). As versões de distribuições
abaixo descrevem a base de glibc, não uma garantia de que o mpv padrão delas seja
novo o bastante. Se o seu for mais antigo, atualize a libmpv a partir de uma fonte
confiável ou use uma versão Flatpak que contenha o mpv 0.42+. Atualizar só o
YouTFast não atualiza a libmpv de um sistema nativo.

## Linux: instalador rápido

O script de instalação baixa do GitHub os arquivos da versão mais recente,
confere o hash SHA-256 contra o `checksums.txt` e instala o pacote Arch (via
`sudo pacman -U` em distribuições baseadas no Arch) ou o bundle Flatpak
(`--user` em todas as outras):

```sh
curl -fsSL https://youtfast.pages.dev/install.sh | sh
```

Você também pode escolher o método explicitamente ou remover uma instalação
Flatpak / local do usuário:

```sh
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method flatpak
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method arch
curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --uninstall
```

## Linux: Flatpak

Você precisa de: Flatpak instalado. Muitas distribuições já o trazem; se não for o
caso, siga a
[página de configuração do Flatpak para a sua distribuição](https://flatpak.org/setup/).

1. Abra um terminal.
2. Baixe o bundle e confira-o:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   echo "Installing YouTFast $V"
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-v$V-x86_64.flatpak" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

   A última linha precisa imprimir `youtfast-v…-x86_64.flatpak: OK`.
3. Instale-o para o seu usuário. Na primeira vez, o Flatpak também baixa o
   runtime Freedesktop de que precisa; responda `y` quando ele perguntar.

   ```sh
   flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
   flatpak install --user "./youtfast-v$V-x86_64.flatpak"
   ```

4. Abra o **YouTFast** pelo menu de aplicativos ou rode
   `flatpak run io.github.bcosta19.YouTFast`.

**Atualizar:** repita as etapas 2 e 3 (ou rode `install.sh --method flatpak` em
**Configurações → Sobre**). O bundle em sandbox não tem canal de atualização
automática.
**Remover:** `flatpak uninstall --user io.github.bcosta19.YouTFast`.

O Flatpak não consegue ler o perfil do seu navegador. Para entrar em uma conta,
escolha um arquivo cookies.txt no app, ou continue sem conta.

## Linux: pacote Arch

Você precisa de: um sistema baseado no Arch com `sudo`.

1. Abra um terminal.
2. Baixe o pacote e confira-o:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-$V-1-x86_64.pkg.tar.zst" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

3. Instale-o. O pacman também instala o mpv, o yt-dlp, o Deno e as outras
   bibliotecas de que o YouTFast precisa. As versões que contêm a correção da
   #84 exigem o mpv 0.42+; pacotes publicados antes não impõem isso. Confira
   `pacman -Q mpv` e atualize o sistema com `sudo pacman -Syu` se necessário:

   ```sh
   sudo pacman -U "./youtfast-$V-1-x86_64.pkg.tar.zst"
   ```

4. Abra o **YouTFast** pelo menu de aplicativos ou rode `youtfast`.

**Atualizar:** o YouTFast atualiza por **Configurações → Sobre** (baixa e
verifica o pacote `.pkg.tar.zst` e executa `pkexec pacman -U`), ou repita as
etapas 2 e 3 com a nova versão. Mantenha o sistema atualizado com
`sudo pacman -Syu` para que o yt-dlp continue atual.
**Remover:** `sudo pacman -R youtfast`.

## Linux: Ubuntu e Debian

Você precisa de: Ubuntu 24.04 ou mais recente, Debian 13 ou mais recente, ou uma
distribuição baseada neles (Linux Mint 22+, Pop!_OS 24.04+), e `sudo`.

1. Instale as bibliotecas que o YouTFast usa, mais `pipx` e `unzip` para a
   próxima etapa:

   ```sh
   sudo apt-get update
   sudo apt-get install libmpv2 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 \
     libgl1 libfontconfig1 pipx unzip curl
   ```

2. [Instale o yt-dlp e o Deno atuais](#yt-dlp-e-deno-no-linux) como descrito
   abaixo. Não use o pacote apt `yt-dlp`: ele é antigo demais para reproduzir
   vídeos do YouTube.
3. [Instale o arquivo compactado do YouTFast](#instalar-o-arquivo-compactado-nativo)
   como descrito abaixo.

## Linux: Fedora

Você precisa de: Fedora 40 ou mais recente e `sudo`.

1. Instale as bibliotecas que o YouTFast usa, mais `pipx` e `unzip`:

   ```sh
   sudo dnf install mpv-libs libxkbcommon-x11 mesa-libGL fontconfig pipx unzip curl
   ```

2. [Instale o yt-dlp e o Deno atuais](#yt-dlp-e-deno-no-linux).
3. [Instale o arquivo compactado do YouTFast](#instalar-o-arquivo-compactado-nativo).

Outras distribuições com glibc 2.39 ou mais recente podem seguir as mesmas três
etapas: instale a libmpv 0.42+ (compilada com Lua), a libxkbcommon, a biblioteca
de cliente do Wayland, o OpenGL e o fontconfig com os nomes usados pela sua
distribuição.

### yt-dlp e Deno no Linux

O YouTFast pede ao yt-dlp os streams de cada vídeo, e o yt-dlp precisa do Deno e
do componente EJS dele para ler as páginas do YouTube. Estes comandos instalam os
três para o seu usuário, em `~/.local/bin`:

```sh
pipx ensurepath
pipx install "yt-dlp[default]"
curl -fsSL https://deno.land/install.sh | DENO_INSTALL="$HOME/.local" sh -s -- -y --no-modify-path
```

`yt-dlp[default]` inclui o componente EJS. Se `pipx ensurepath` disser que mudou
o seu `PATH`, **saia da sessão e entre de novo** uma vez, para que os programas
abertos pelo menu de aplicativos também encontrem o yt-dlp. Confira em um novo
terminal:

```sh
yt-dlp --version
deno --version
```

Mantenha o yt-dlp atualizado: quando os vídeos pararem de carregar, rode
`pipx upgrade yt-dlp` primeiro.

### Instalar o arquivo compactado nativo

1. Baixe o arquivo compactado e confira-o:

   ```sh
   V=$(curl -fsSL https://api.github.com/repos/bcosta19/youtfast/releases/latest | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
   base=https://github.com/bcosta19/youtfast/releases/download/v$V
   curl -LO "$base/youtfast-v$V-x86_64-unknown-linux-gnu.tar.gz" -LO "$base/checksums.txt"
   sha256sum -c --ignore-missing checksums.txt
   ```

2. Descompacte-o e instale o programa, o ícone e a entrada de menu para o seu
   usuário:

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

   O último comando imprime a versão; ele não abre a janela.
3. Abra o **YouTFast** pelo menu de aplicativos ou rode `youtfast`.

**Atualizar:** o YouTFast se atualiza sozinho em **Configurações → Sobre**.
Mantenha o programa onde a etapa 2 o colocou, para que o atualizador possa
substituí-lo. Atualize o yt-dlp com `pipx upgrade yt-dlp` e as bibliotecas com
as atualizações do seu sistema.
**Remover:**

```sh
rm ~/.local/bin/youtfast ~/.local/share/applications/youtfast.desktop \
  ~/.local/share/icons/hicolor/scalable/apps/youtfast.svg
```

## Windows

Você precisa de: Windows 10 ou 11 de 64 bits. O Winget (Instalador de
Aplicativo), incluído nas versões recentes do Windows, é usado se o `yt-dlp` ou o
`Deno` ainda não estiverem instalados.

### Opção A: instalador do Windows (.exe) — recomendado

1. Baixe `youtfast-v<version>-x86_64-windows-setup.exe` da
   [versão mais recente](https://github.com/bcosta19/youtfast/releases/latest),
   ou rode o instalador verificado do PowerShell, com um único comando:

   ```powershell
   irm https://youtfast.pages.dev/install.ps1 | iex
   ```

2. Ao abrir o `.exe`, o Windows SmartScreen avisa sobre apps sem assinatura de
   publicador, que o YouTFast não tem: escolha **Mais informações** e depois
   **Executar assim mesmo**. Para conferir o arquivo antes, rode
   `Get-FileHash .\youtfast-v*-x86_64-windows-setup.exe` e compare com o
   `checksums.txt`.
3. Siga o assistente de instalação. Ele instala por usuário em
   `%LOCALAPPDATA%\Programs\YouTFast` (sem pedir permissão de Administrador),
   cria atalhos no Menu Iniciar e na Área de Trabalho e instala o `yt-dlp` e o
   `Deno` que faltarem via `winget`, se essa opção estiver ativada. Saia da
   sessão e entre de novo se você precisou instalar o `yt-dlp` ou o `Deno` pela
   primeira vez.

### Opção B: ZIP portátil (.zip)

1. Instale o yt-dlp e o Deno. Abra o **Terminal** ou o **PowerShell** e rode:

   ```powershell
   winget install --id yt-dlp.yt-dlp
   winget install --id DenoLand.Deno
   ```

   Depois, **saia da sessão do Windows e entre de novo**, para que os programas
   abertos a partir daí os encontrem. Confira em um novo terminal:
   `yt-dlp --version` e `deno --version` devem imprimir uma versão.
2. Baixe `youtfast-v<version>-x86_64-pc-windows-gnu.zip` da
   [versão mais recente](https://github.com/bcosta19/youtfast/releases/latest).
3. Opcional, para conferir o download: na pasta Downloads, rode
   `Get-FileHash .\youtfast-v*-x86_64-pc-windows-gnu.zip` e compare o `Hash`
   com a linha desse arquivo no `checksums.txt` da versão (maiúsculas e
   minúsculas não importam).
4. Clique com o botão direito no ZIP, escolha **Extrair Tudo...** e selecione um
   lugar permanente, por exemplo `Documents\YouTFast`. Mantenha todos os
   arquivos da pasta juntos: o programa precisa das DLLs ao lado dele.
5. Abra o `youtfast.exe` na pasta extraída e passe pelo SmartScreen (**Mais
   informações** → **Executar assim mesmo**).

**Atualizar:** o YouTFast se atualiza sozinho em **Configurações → Sobre**.
Atualize o yt-dlp com `winget upgrade --id yt-dlp.yt-dlp` quando os vídeos
pararem de carregar.
**Remover:** use **Configurações → Aplicativos → Aplicativos instalados →
YouTFast** do Windows (ou rode o `Uninstall.exe`) se instalou pelo `.exe`, ou
apague a pasta extraída se usa o `.zip` portátil.

## macOS

Você precisa de: um Mac com Apple Silicon (M1 ou mais recente) no macOS 15 ou
mais recente, e o [Homebrew](https://brew.sh/).

### Opção A: tap do Homebrew — recomendado

```sh
brew install --cask bcosta19/tap/youtfast
```

O cask instala o `YouTFast.app`, cria o link do comando `youtfast`, instala o
`yt-dlp` e o `deno` automaticamente e remove o `com.apple.quarantine` do pacote
do app com assinatura ad-hoc. O tap acompanha cada versão publicada: atualize
com `brew upgrade --cask youtfast` ou por **Configurações → Sobre** no app.

### Opção B: ZIP manual

1. Instale o yt-dlp e o Deno. No **Terminal**:

   ```sh
   brew install yt-dlp deno
   ```

2. Baixe `youtfast-v<version>-aarch64-apple-darwin.zip` da
   [versão mais recente](https://github.com/bcosta19/youtfast/releases/latest).
3. Opcional, para conferir o download: no Terminal, rode
   `shasum -a 256 ~/Downloads/youtfast-v*-aarch64-apple-darwin.zip` e
   compare o resultado com a linha desse arquivo no `checksums.txt` da
   versão.
4. Dê um clique duplo no ZIP para descompactá-lo e arraste o **YouTFast.app**
   para **Aplicativos**.
5. Abra o YouTFast em Aplicativos. O app não é notarizado pela Apple, então na
   primeira vez o macOS o recusa: clique em **OK**, abra **Ajustes do Sistema →
   Privacidade e Segurança**, role até a mensagem sobre o YouTFast, clique em
   **Abrir Mesmo Assim** e confirme com a sua senha. As próximas aberturas
   acontecem normalmente.

**Atualizar:** o YouTFast se atualiza sozinho em **Configurações → Sobre**.
Atualize o yt-dlp com `brew upgrade yt-dlp` quando os vídeos pararem de carregar.
**Remover:** arraste o YouTFast.app de Aplicativos para o Lixo.

## Seus dados

Remover o YouTFast mantém sua biblioteca e suas configurações. Apague também
estas pastas para uma remoção completa:

| Instalado com | Biblioteca | Configurações |
|---|---|---|
| Pacote Arch ou arquivo compactado nativo | `~/.local/share/youtfast` | `~/.config/youtfast` |
| Flatpak | `~/.var/app/io.github.bcosta19.YouTFast` | (mesma pasta) |
| Windows | `%APPDATA%\youtfast\data` | `%APPDATA%\youtfast\config` |
| macOS | `~/Library/Application Support/youtfast` | (mesma pasta) |

Uma sessão conectada fica guardada no armazenamento de credenciais do sistema,
não nestas pastas: saia da conta no app antes de removê-lo.

## Se algo der errado

| O que você vê | O que fazer |
|---|---|
| A janela abre, mas os vídeos nunca começam | Atualize o yt-dlp (veja o seu caminho acima). Confira se `yt-dlp --version` e `deno --version` funcionam em um novo terminal. |
| `O YouTFast não conseguiu iniciar o yt-dlp…` | O yt-dlp não está instalado, ou o YouTFast foi aberto com um `PATH` de antes da instalação dele. Confira se `yt-dlp --version` e `deno --version` funcionam em um novo terminal e instale o que faltar (veja o seu caminho acima). No Windows, saia da sessão e entre de novo depois de instalá-los e abra o YouTFast pelo Menu Iniciar. |
| `O yt-dlp não conseguiu obter as transmissões deste vídeo…` | Atualize o yt-dlp (veja o seu caminho acima) e confira se `deno --version` funciona em um novo terminal. Se persistir com um yt-dlp atual, relate o problema. |
| `O mpv não conseguiu reconhecer o formato da transmissão.` (0.9.0 e anteriores) | Essas versões mostram isso quando o yt-dlp falta ou falha: siga as duas linhas acima. |
| De vez em quando: `Não foi possível reproduzir este vídeo. O player não identificou a causa. Tente reabrir o vídeo.` | Confira a versão do pacote libmpv instalado. Antes da 0.42, o hook do yt-dlp dela pode abrir os streams cedo demais e receber HTTP 403. Atualize a libmpv para 0.42+ com Lua, ou use uma versão Flatpak que a contenha. Reabrir depois de alguns segundos é um contorno temporário, não uma correção. Se persistir na 0.42+, relate o problema; essa mensagem pode ter outras causas. |
| `youtfast: command not found` depois do arquivo compactado nativo | Use a entrada do menu, ou saia da sessão e entre de novo para que `~/.local/bin` esteja no `PATH`. |
| `error while loading shared libraries: libmpv.so.2` | Instale a libmpv da sua distribuição (etapa 1 do seu caminho), ou use o [Flatpak](#linux-flatpak). |
| `GLIBC_2.39 not found` | O seu Linux é mais antigo do que o arquivo compactado nativo suporta: use o [Flatpak](#linux-flatpak). |
| Windows: "O Windows protegeu o computador" | Escolha **Mais informações** e depois **Executar assim mesmo** (veja [Windows](#windows), etapa 5). |
| macOS: "Não é possível abrir “YouTFast”" | Use **Abrir Mesmo Assim** em Privacidade e Segurança (veja [macOS](#macos), etapa 5). |

Ainda com problemas? [Abra uma issue](https://github.com/bcosta19/youtfast/issues)
informando o seu sistema, como você instalou o YouTFast e o que aconteceu. Pode
escrever em português.

Um checksum mostra que o download está completo e inalterado; ele não é uma
assinatura de publicador. Para compilar o YouTFast você mesmo, veja o
[CONTRIBUTING.md](../../../CONTRIBUTING.md#getting-started).
