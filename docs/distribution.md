# Distribution and platform qualification

0.1.0 targets Linux x86_64, Windows x86_64 and macOS Apple Silicon. Workflows
prepare candidates; new platform packaging is not certified by interactive
testing just because these files exist. See [ROADMAP.md](../ROADMAP.md) for
outstanding gates.

## Package matrix

| Target | Candidate artifact | Runtime requirements | Updates |
|---|---|---|---|
| Linux x86_64 native | `*-x86_64-unknown-linux-gnu.tar.gz` | glibc 2.39+, system libmpv/Lua, graphics libraries, current yt-dlp and JS runtime | Replace archive; update system dependencies separately |
| Linux x86_64 Flatpak | `*-x86_64.flatpak` | Flatpak, Freedesktop 25.08 runtime, graphics drivers and portals | Install next bundle; no app update remote yet |
| Arch Linux x86_64 | `.pkg.tar.zst` and `*-aur.tar.gz` recipe | System dependencies installed by pacman; AUR source builds also need Rust/Cargo | Local pacman package; AUR helper after entry acceptance |
| Windows x86_64 | `*-x86_64-pc-windows-gnu.zip` | Bundled libmpv/imported DLLs; current yt-dlp and Deno on PATH | Replace complete extracted directory |
| macOS 15+ Apple Silicon | `*-aarch64-apple-darwin.zip` with `.app` | Bundled libmpv/dylibs; current yt-dlp and Deno | Replace app; update extraction tools separately |

Windows/macOS candidates include a native dependency inventory and collected
notices. These are inputs to review, **not corresponding source archives**.
The platform jobs also collect matching native sources, patches, recipes and
notices into separate native-source archives, which assembly requires and
checksums. Review them before publication. The Rust source archive alone does
not contain the MSYS2/Homebrew native sources.

Windows has no publisher signature or MSI/MSIX installer. macOS uses ad-hoc
signing for internal bundle consistency; it is not a Developer ID signature,
notarization or proof that Gatekeeper accepts the download. Never recommend
disabling system security globally. Intel Macs/universal binaries, ARM Windows
and Linux ARM64 are not built.

Native packages keep extraction tools external so users can update them as
YouTube changes. Install current yt-dlp with its matching EJS component and a
supported JS runtime; Deno is enabled by default by yt-dlp. See
[upstream EJS setup](https://github.com/yt-dlp/yt-dlp/wiki/EJS).
Browser-session import additionally invokes `python3` with the `yt_dlp` Python
module; a standalone yt-dlp executable does not provide that module. This
optional integration is a separate platform QA gate. Cookies.txt selection and
signed-out use do not need browser-profile access.

## Linux and Flatpak

Flatpak is the preferred route across Linux distributions without depending on
the host's libmpv or glibc baseline. It still needs matching CPU architecture,
a supported Flatpak installation/runtime, drivers and portals. It does not make
a Linux package work on Windows/macOS. Our bundle is x86_64 only. See
[Flatpak's introduction](https://docs.flatpak.org/en/latest/introduction.html).

After installing Flatpak using the distribution's supported instructions:

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user ./youtfast-v0.1.0-x86_64.flatpak
flatpak run io.github.bcosta19.YouTFast
```

Download from the matching release and verify its checksum first. The runtime
is a separate download. The manifest includes libmpv, yt-dlp, EJS and Deno.
Host browser profiles/theme directories are not granted access; choose a
cookies.txt through the portal or remain signed out. Do not broaden permissions
to make a smoke test pass.

A bundle is not a Flathub listing or automatic-update channel. A repository is
the preferred Flatpak distribution channel for updates; see [single-file bundles](https://docs.flatpak.org/en/latest/single-file-bundles.html).
The existing manifest installs a prebuilt YouTFast binary. Flathub needs a
reviewed source-build manifest, metadata/screenshots, lint, runtime tests and
submission review. Follow [Flathub's submission procedure](https://docs.flathub.org/docs/for-app-authors/submission)
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
| Playback | Signed-out search, public video audio/picture, seek, captions, full screen and mini player |
| Graphics | OpenGL rendering; record GPU/driver and actual hardware-decoding result |
| Integration | File chooser/portal, media controls, notifications, keyboard, English/Portuguese, light/dark and HiDPI |
| Persistence | New-profile settings/library survive restart; writes stay in documented paths |
| Credentials | Native store unavailable/locked behaviour; login only with a deliberately approved disposable account |
| Packaging | No missing DLL/dylib; notices, native sources and digests match shipped artifacts |
| Lifecycle | Install/extract, launch, replace and remove; user data retention is explicit |

Test Linux Wayland and X11, Flatpak on a different supported Linux distribution,
Windows without MSYS2 on PATH and macOS from Finder outside the checkout.
Windows Credential Manager can reject large YouTube sessions; do not claim
account support passed based on compilation or `--version`. Headless CI does
not prove playback, portal or credential-store behaviour.
