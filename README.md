# YouTFast

**YouTube, native and fast.** A YouTube client written in Rust with
[egui](https://github.com/emilk/egui), playing video through
[libmpv](https://mpv.io) with hardware decoding. No browser engine.

A sibling of [Spotifast](../spotifast) and [ZapFast](../zapfast), and built the
same way: same egui and winit forks, same `ui → Action → app` architecture,
and the [fastframe](https://github.com/crmne/fastframe) crates as they are
needed.

> Early prototype. Search, in-window playback, a library on this computer and
> an optional YouTube account are in place; see [AGENTS.md](AGENTS.md#roadmap)
> for what comes next.

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
and comments, loaded through rustypipe. Channel pages have tabs — videos,
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
`watch-history.json`. Nothing here touches a Google account.

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

MIT. The icons are [Lucide](https://lucide.dev) (ISC).
