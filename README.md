# YouTFast

**YouTube, native and fast.** A YouTube client written in Rust with
[egui](https://github.com/emilk/egui), playing video through
[libmpv](https://mpv.io) with hardware decoding. No browser engine.

A sibling of [Spotifast](../spotifast) and [ZapFast](../zapfast), and built the
same way: same egui and winit forks, same `ui → Action → app` architecture,
and the [fastframe](https://github.com/crmne/fastframe) crates as they are
needed.

> Version 0.1.0 is the first early-stage release. Linux is the initial release
> target; Windows and macOS build/test jobs do not yet imply portable installers
> or validated interactive playback. The repository and releases currently remain
> private to invited collaborators.

## Installation and releases

Release assets include a Linux x86_64 archive, a Flatpak bundle, corresponding
source with locked Rust dependencies, and SHA-256 checksums. The native archive
requires glibc 2.39 or newer (Ubuntu 24.04 baseline), system libmpv and
libxkbcommon-x11 on X11; it is not
an AppImage. The Flatpak uses the Freedesktop 25.08 runtime and does not need
host libmpv or yt-dlp. It is a release bundle, not a Flathub listing or update
remote. See [RELEASING.md](RELEASING.md) for installation and verification.

To contribute, start with [CONTRIBUTING.md](CONTRIBUTING.md). Read
[SECURITY.md](SECURITY.md) before reporting account or data-handling issues.
CI runs on pull requests and pushes to `main`; release tags must match package
and desktop metadata and pass all checks before assets are published.

## Requirements

| What | Why | Arch | Debian/Ubuntu |
|---|---|---|---|
| libmpv | Playback, linked at build time | `mpv` | `libmpv-dev` |
| yt-dlp | Search, and the streams mpv plays | `yt-dlp` | `yt-dlp` (keep it current) |
| Rust 1.98 | Pinned in `rust-toolchain.toml` | `rustup` | `rustup` |
| python-secretstorage | Optional: signing in with a Chromium-based browser's login | `python-secretstorage` | `python3-secretstorage` |

mpv must be built with Lua (the usual packages are): it resolves YouTube
links through its `ytdl_hook` script. Set `YOUTFAST_YTDLP` to use a yt-dlp
that is not on `PATH`.

## Running

```sh
cargo run --release
cargo run --release -- "https://youtu.be/dQw4w9WgXcQ"   # open a link
cargo run --release -- "rust egui"                      # or search
```

| Key | Action |
|---|---|
| Space, K | Play or pause |
| ←, → | Back or forward 5 s |
| J, L | Back or forward 10 s |
| F, double click | Full screen |
| Esc | Leave full screen, then go back to the results |

The watch page also shows the description (timestamps seek), chapters
and comments, loaded through rustypipe. Comments and replies also display
image and animated GIF/WebP attachments. Attachments use the bounded thumbnail
cache, decode in the background and animate only while visible.
Channel pages have tabs — videos,
shorts, live, playlists and about — and a search of the channel's own
videos.

The desktop's media keys and panels (MPRIS, so `playerctl` works too)
control playback, and a second launch hands a link or search to the
window already open instead of opening another.

## Your library, with or without an account

**Home** is the newest of your subscriptions — the account's feed or the
device's RSS channels — day-grouped and paged: scroll to the end and more
arrives. Shorts sit on a shelf above the list: the account's are marked
by YouTube, the device channels' come from their Shorts tabs.

**Without an account**, YouTFast keeps subscriptions, watch history (with
where each video was left, to resume it) and Watch later on this computer,
in `~/.local/share/youtfast/library.json`, and its settings (video quality,
volume) in `~/.config/youtfast/settings.json`. The home feed comes from each
subscribed channel's RSS. Bring an existing library over from
[Google Takeout](https://takeout.google.com): choose YouTube and YouTube
Music, with JSON as the history format, and import `subscriptions.csv` and
`watch-history.json`. Nothing here touches a Google account. Imports accept
files up to 64 MiB and 20,000 unique subscriptions, and retain the 1,000 most
recent unique history entries. Parsing and merging run in the background.
If the library changes during the merge, retry the import; those newer
changes are never replaced by an older import snapshot.

An unreadable library or settings file is backed up before defaults can be
saved. A read or backup failure stops startup rather than overwriting the
original. Saves at exit wait for in-progress saves and prevent older queued
snapshots from replacing the final state.

**With your YouTube accounts**, each account is named and kept in the
keyring under its own entry: add several, switch from the account page
or with `a`, and point a browser sign-in at a profile (`Profile 1`,
`Default`, …). The feed, subscriptions, history, Watch later,
liked videos and playlists come from the account. YouTube has no sign-in for
apps like this one, so YouTFast uses the session a browser already has:

- **Your browser's login**: YouTFast reads the browser's YouTube cookies
  through yt-dlp, and reads them again whenever they expire.
- **A cookies.txt file**: exported from a private window, which keeps the
  browser's own session out of it.

The session lives only in the system keyring (Secret Service, Keychain,
Credential Manager), never in a file or the log. **Using an account this way
may be treated by Google as automated access**: the account could be asked to
verify itself, signed out, or suspended. A secondary account is the safe
choice. Subscribing and saving to Watch later from YouTFast act on this
computer; changing the account itself needs youtube.com.

Account changes clear private lists and caches immediately and dismiss open
menus. Pending writes finish on their originating session before switching,
and late responses cannot update the new account's interface. A session
expiration or sign-in failure also discards private responses from the same
pending batch. Credentials found in an older rustypipe cache are ignored;
only the selected account's keyring entry can restore its session.

Logs use fixed diagnostic categories rather than account names, video IDs,
browser profiles, URLs or external error text. This redaction also applies
to dependencies and to `--verbose` / `RUST_LOG`; more verbose logging does
not opt out of privacy protection. Thumbnail downloads are shared between
display and the disk cache, capped at 4 MiB and four million pixels each,
with at most three automatic attempts for transient failures. HTTP 429 honors
`Retry-After` (seconds or HTTP-date); access failures renew the original comment
page once per comment without attaching account cookies to image downloads. Removed images,
unchanged/expired links, unsupported data and exhausted limits show a retry
control. A corrupt cache item can be replaced independently of other images.
The disk cache is pruned during use to 2,000 files and 128 MiB; `no-store`
responses are only kept for the current display.

Application-wide diagnostics persist locally under
`$XDG_STATE_HOME/youtfast/logs` (normally `~/.local/state/youtfast/logs` on Linux).
`youtfast.log` rotates into `youtfast.log.1`; each is limited to 2 MiB. These
files cover the whole application, including image download and decoder failure
categories. They are not uploaded. On Unix the directory is owner-only (0700)
and log files are 0600. The same privacy redactor runs before stderr and files,
including for dependency diagnostics. `--verbose` adds cached/streaming animation
mode categories; there are no per-frame log records. Logging starts when this
binary next owns a running instance; rebuilding does not update an existing
process. To follow the current log on Linux (including rotation):

```sh
tail -F "${XDG_STATE_HOME:-$HOME/.local/state}/youtfast/logs/youtfast.log"
```

On platforms without an XDG state directory, logs use the `logs` subdirectory
of the platform's local application data directory for YouTFast. If opening or writing the
file fails, diagnostics continue on stderr.

Comment GIF/WebP playback uses one decoder worker and one reused texture per
visible attachment, with at most two active attachments. Frames are reduced to
fit 320×200 while preserving aspect ratio. Small animations cache up to about
8 MiB; longer ones continue sequentially instead of retaining every source-size
frame or rejecting the whole clip at a frame-count threshold. Controlled frame,
canvas and texture reservations share a 64 MiB budget; decoder/library/driver
bookkeeping and the ordinary thumbnail cache are additional process memory.
A frame which takes over 200 ms to decode stops further automatic decoding
and retains a preview; this is an observed work limit, not preemption of a
native decoder in the middle of a frame. Offscreen animations are reclaimed,
pause controls stop frame requests, and
finite source loop counts are respected. Decode failures after a successful
frame retain that frame as a preview. External blocks, removed media or an
unusable source may still prevent a preview or playback.

## New-upload alerts

YouTube's own notifications have no API, so YouTFast checks your
subscriptions on a timer instead (every 15 minutes by default; 5, 15, 30
or 60 in Settings) and lists what's new under `[7] Notifications`. New
uploads also come to the desktop as notifications. Both can be turned
off there; the first run only starts the clock, so a fresh install
doesn't flood you.

## Themes

Settings → Appearance. **Automatic** follows the Omarchy desktop's theme
live on Linux, and is dark elsewhere. **Dark** and **Light** are YouTFast's
own colours. The eight shared palettes (Catppuccin, Nord, Rosé Pine, Tokyo
Night, and more) are installed into `~/.config/youtfast/themes` on the
first run as plain JSON files you can edit, copy or delete; add your own
there in the same format (a `base` plus named colours, see
[fastframe-theme](https://github.com/crmne/fastframe)). A source build does
not install the Omarchy hook (`contrib/omarchy/youtfast-theme`); packaged
builds do, and following the desktop works either way.

## Disclaimer

YouTFast is an unofficial client and is not affiliated with YouTube or
Google. Using an unofficial client may be against YouTube's terms of service.
Use it at your own risk.

## License

GPL-3.0-only. See [LICENSE](LICENSE) and
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Dependencies retain their own
licenses and notices. The icons are [Lucide](https://lucide.dev) (ISC, with
MIT notices for Feather-derived icons).
