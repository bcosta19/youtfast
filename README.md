<p align="center">
  <img src="packaging/icons/youtfast.svg" width="96" height="96" alt="YouTFast icon">
</p>
<h1 align="center">YouTFast</h1>
<p align="center"><strong>YouTube, native and fast.</strong></p>
<p align="center">
  <a href="https://github.com/bcosta19/youtfast/releases">Releases</a> ·
  <a href="docs/user-guide.md">User guide</a> ·
  <a href="CHANGELOG.md">Changelog</a> ·
  <a href="CONTRIBUTING.md">Contribute</a>
</p>

A small native YouTube desktop client built with **Rust, egui and libmpv**.
Search, watch and keep your library in a desktop interface, with hardware video
decoding where supported. No browser engine, telemetry or YouTFast-operated
backend. Start signed out; accounts are optional.

![YouTFast watch page in the dark theme](assets/screenshots/watch-dark.png)

*Native demo capture using synthetic data. The preview does not connect to
YouTube, access accounts or play a real video.*

## What you can do

| Feature | In YouTFast |
|---|---|
| Watch | In-window video, full screen, theater mode and a movable mini player |
| Discover | Search filters, channel tabs, playlists, Shorts and paginated feeds |
| Stay in control | Captions, chapters, queue editing, keyboard shortcuts and desktop media controls |
| Keep a library | Device subscriptions, Watch later, history/resume and Google Takeout import |
| Read and explore | Descriptions, comments and attachments; optional transcripts and timeline previews |
| Make it yours | English or Português (Brasil), light/dark themes, palettes and native Linux Omarchy integration |

Optional accounts use the system credential store. Account ratings and playlist
edits affect the selected account; device subscriptions and device Watch later
stay local. [Read about accounts and stored data](docs/user-guide.md#your-library-with-or-without-an-account).

## Installation

**0.1.0 is being prepared.** The table describes candidate packages; check
[GitHub Releases](https://github.com/bcosta19/youtfast/releases) for actual
availability. Interactive platform qualification is still required.

| Platform | Package | Requirements |
|---|---|---|
| Linux x86_64 | Flatpak bundle | Flatpak, Freedesktop 25.08 runtime and desktop drivers/portals |
| Linux x86_64 | Native archive | glibc 2.39+, system libmpv and current yt-dlp/Deno |
| Windows x86_64 | ZIP with libmpv and DLLs | Current yt-dlp/Deno on PATH; no installer or publisher signature |
| macOS 15+ Apple Silicon | ZIP with app and dylibs | Current yt-dlp/Deno; ad-hoc signed, not notarized |
| Arch Linux x86_64 | Pacman package and source PKGBUILD | Hosted package validation and separate AUR submission pending |

The Flatpak bundles its playback/extraction tools and targets compatible Linux
distributions at x86_64. It is not yet a Flathub listing or an app update remote.
The Arch recipe is not yet an AUR entry or an official pacman package.

[Installation, checksums and platform limits](docs/distribution.md) ·
[Arch package maintenance](packaging/arch/README.md)

## Build from source

Install the compiler and system dependencies in
[CONTRIBUTING.md](CONTRIBUTING.md#getting-started), then run from the checkout:

```sh
cargo run --locked --release
cargo run --locked --release -- "https://youtu.be/dQw4w9WgXcQ"
cargo run --locked --release -- "rust egui"
```

Playback needs libmpv with Lua support, current yt-dlp and its matching EJS/JS
runtime. Optional browser-session import also needs `python3` with the `yt_dlp`
module. See the [requirements](docs/user-guide.md#requirements).

## Guides and project status

| You want to… | Read |
|---|---|
| Learn controls, library, accounts and themes | [User guide](docs/user-guide.md) |
| Choose a package or check platform coverage | [Distribution](docs/distribution.md) |
| See release contents and exclusions | [Changelog](CHANGELOG.md) |
| See work that is not implemented yet | [Roadmap](ROADMAP.md) |
| Report a bug or contribute | [Contributing](CONTRIBUTING.md) |
| Report a security issue privately | [Security policy](SECURITY.md) |
| Prepare a release or public opening | [Release process](RELEASING.md) and [checklist](docs/public-release-checklist.md) |

Library/settings stay on this device. Account cookies belong in the native
credential store, and local diagnostics redact private details. Account access
is unofficial and can trigger Google verification, sign-out or suspension.
Use the app signed out unless you deliberately choose that integration.
[Storage, privacy and account details](docs/user-guide.md#your-library-with-or-without-an-account).

## Related projects and acknowledgements

- [Spotifast](https://github.com/crmne/spotifast) — native Spotify client.
- [ZapFast](https://github.com/crmne/zapfast) — native WhatsApp client.
- [fastframe](https://github.com/crmne/fastframe) — shared foundations for native
  egui desktop applications.

YouTFast follows native desktop conventions from these projects and uses shared
egui/winit forks. Thanks also to [mpv](https://mpv.io),
[yt-dlp](https://github.com/yt-dlp/yt-dlp),
[RustyPipe](https://codeberg.org/ThetaDev/rustypipe),
[egui](https://github.com/emilk/egui) and [Lucide](https://lucide.dev).

## License and disclaimer

[GPL-3.0-only](LICENSE). Dependencies retain their own licenses and notices;
see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

YouTFast is an unofficial client, unaffiliated with YouTube or Google. Using an
unofficial client may conflict with YouTube's terms of service. Use it at your
own risk.
