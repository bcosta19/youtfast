# Distribution and platform qualification

0.x releases target Linux x86_64, Windows x86_64 and macOS Apple Silicon. Workflows
prepare candidates; new platform packaging is not certified by interactive
testing just because these files exist. See [ROADMAP.md](../ROADMAP.md) for
outstanding gates.

## Package matrix

| Target | Candidate artifact | Runtime requirements | Updates |
|---|---|---|---|
| Linux x86_64 native | `*-x86_64-unknown-linux-gnu.tar.gz` | glibc 2.39+, system libmpv 0.42+ with Lua, graphics libraries, current yt-dlp and JS runtime | In-app updater (`Settings → About`, SHA-256 verified) or replace archive; update system dependencies separately |
| Linux x86_64 Flatpak | `*-x86_64.flatpak` and Flathub source manifest (`channels.tar.gz`) | Flatpak, Freedesktop 25.08 runtime, graphics drivers and portals | Install next bundle or run `install.sh --method flatpak`; Flathub submission is not pursued |
| Arch Linux x86_64 | `.pkg.tar.zst` and `*-aur.tar.gz` recipe | System dependencies installed by pacman; AUR source builds also need Rust/Cargo | Install `.pkg.tar.zst` directly with `sudo pacman -U` or `install.sh --method arch` |
| Windows x86_64 | `*-x86_64-windows-setup.exe` (NSIS), `*-x86_64-pc-windows-gnu.zip` and multi-file Winget manifest (`channels.tar.gz`) | Bundled libmpv/imported DLLs; current yt-dlp and Deno on PATH | In-app updater (`Settings → About`), setup `.exe`, or Winget once PR accepted |
| macOS 15+ Apple Silicon | `*-aarch64-apple-darwin.zip` with `.app` and Homebrew Cask (`channels.tar.gz`) | Bundled libmpv/dylibs; current yt-dlp and Deno | In-app updater (`Settings → About`), replace `.app`, or personal Homebrew tap (`bcosta19/homebrew-tap`) once published |

Windows/macOS candidates include a native dependency inventory and collected
notices. These are inputs to review, **not corresponding source archives**.
The platform jobs also collect matching native sources, patches, recipes and
notices into separate native-source archives, which assembly requires and
checksums. Review them before publication. The Rust source archive alone does
not contain the MSYS2/Homebrew native sources.

Windows ships an unsigned per-user NSIS setup installer (`*-windows-setup.exe`,
installing under `%LOCALAPPDATA%\Programs\YouTFast` so the in-app updater remains
writable) and portable ZIP (`*-pc-windows-gnu.zip`), without a publisher
signature (Authenticode) or MSIX store package. macOS uses ad-hoc
signing for internal bundle consistency; it is not a Developer ID signature,
notarization or proof that Gatekeeper accepts the download. Never recommend
disabling system security globally. Intel Macs/universal binaries, ARM Windows
and Linux ARM64 are not built.

The supported YouTube playback baseline is **libmpv 0.42+ with Lua**, distinct
from the older client ABI needed to link the executable. Its yt-dlp hook waits
for formats' `available_at` time instead of opening streams early and getting
HTTP 403. Flatpak pins mpv 0.42.0 and compatible libplacebo; Windows/macOS
bundlers reject older mpv packages, and the Arch recipe requires `mpv>=0.42.0`.
Older published artifacts are unchanged: use a release containing this fix.
Native Linux users must check their libmpv package version and upgrade it
separately; glibc 2.39+ and the `libmpv.so.2` soname alone are not enough.

Native packages keep extraction tools external so users can update them as
YouTube changes. Install current yt-dlp with its matching EJS component and a
supported JS runtime; Deno is enabled by default by yt-dlp. See
[upstream EJS setup](https://github.com/yt-dlp/yt-dlp/wiki/EJS).
Browser-session import additionally invokes `python3` with the `yt_dlp` Python
module; a standalone yt-dlp executable does not provide that module. This
optional integration is a separate platform QA gate. Cookies.txt selection and
signed-out use do not need browser-profile access.

Step-by-step installation, update and removal for each platform is in
[Installing YouTFast](install.md). This page covers what the packages are and
what still needs checking.

## Linux and Flatpak

Flatpak is the preferred route across Linux distributions without depending on
the host's libmpv or glibc baseline. It still needs matching CPU architecture,
a supported Flatpak installation/runtime, drivers and portals. Our bundle is
x86_64 only. See [Flatpak's introduction](https://docs.flatpak.org/en/latest/introduction.html).

The runtime is a separate download. The manifest includes libmpv, yt-dlp, EJS
and Deno. Host browser profiles/theme directories are not granted access; choose a
cookies.txt through the portal or remain signed out. Do not broaden permissions
to make a smoke test pass.

The native archive installs into the user's `~/.local` (see the install guide)
so the in-app updater can replace the binary in place; it falls back to
`~/.local/bin/youtfast` when the running binary is not writable.

A bundle is not a Flathub listing or automatic-update channel. A repository is
the preferred Flatpak distribution channel for updates; see [single-file bundles](https://docs.flatpak.org/en/latest/single-file-bundles.html).
The bundle manifest (`packaging/flatpak/io.github.bcosta19.YouTFast.json`) installs
a prebuilt YouTFast binary, while `packaging/channels.py` renders the corresponding
Flathub source-build manifest into `channels.tar.gz`. Flathub submission still needs
metadata/screenshots, lint, runtime tests and submission review. Follow
[Flathub's submission procedure](https://docs.flathub.org/docs/for-app-authors/submission)
and its linked requirements/policies at submission time.

## Arch

See [Arch maintenance](../packaging/arch/README.md) for recipe generation,
clean-chroot validation, `.SRCINFO` and per-release AUR updates. AUR submission
is separate from a GitHub release. `yay -S youtfast` becomes an instruction only
after the actual entry is available. AUR does not add the app to official
pacman repositories. Do not advertise `pacman -S youtfast` without an actual
configured binary repository.

## Required clean-machine QA

Record candidate run URL, digest, OS version/architecture and outcome in the
draft review. Test the extracted/installed artifact, not the build directory.
Use synthetic data and a new app profile; never inspect a user's browser,
keyring or library to debug packaging.

| Check | Required evidence |
|---|---|
| Startup | Desktop/Finder/Explorer launch and correct version without build-toolchain libraries on PATH |
| Playback | Confirm bundled/system libmpv 0.42+ with Lua; repeated signed-out public-video opens, audio/picture, seek, captions, full screen and mini player |
| Graphics | OpenGL rendering; record GPU/driver and actual hardware-decoding result |
| Integration | File chooser/portal, media controls, notifications, keyboard, all six interface languages, light/dark and HiDPI |
| Persistence | New-profile settings/library survive restart; writes stay in documented paths |
| Credentials | Native store unavailable/locked behaviour; login only with a deliberately approved disposable account |
| Packaging | No missing DLL/dylib; notices, native sources and digests match shipped artifacts |
| Lifecycle | Install/extract, launch, replace and remove; user data retention is explicit |

Test Linux Wayland and X11, Flatpak on a different supported Linux distribution,
Windows without MSYS2 on PATH and macOS from Finder outside the checkout.
Windows Credential Manager can reject large YouTube sessions; do not claim
account support passed based on compilation or `--version`. Headless CI does
not prove playback, portal or credential-store behaviour.
