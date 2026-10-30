# YouTFast user guide

[Back to the overview](../README.md) · [Installation and platforms](distribution.md)

## Requirements

| What | Why | Arch | Debian/Ubuntu |
|---|---|---|---|
| libmpv | Playback, linked at build time | `mpv` | `libmpv-dev` |
| yt-dlp | The streams mpv plays and playback retries | `yt-dlp` | `yt-dlp` (keep it current) |
| Deno / matching yt-dlp EJS | YouTube extraction challenges | `deno` and distribution EJS package | See [yt-dlp setup](https://github.com/yt-dlp/yt-dlp/wiki/EJS) |
| Rust 1.98 | Pinned in `rust-toolchain.toml` | `rustup` | `rustup` |
| python-secretstorage | Optional: signing in with a Chromium-based browser's login | `python-secretstorage` | `python3-secretstorage` |

mpv must be built with Lua (the usual packages are): it resolves YouTube
links through its `ytdl_hook` script. Keep yt-dlp and its JS runtime on `PATH`.
Browser import also needs `python3` with the `yt_dlp` module; an executable-only
yt-dlp install does not supply that Python module.

## Running

```sh
cargo run --locked --release
cargo run --locked --release -- "https://youtu.be/dQw4w9WgXcQ"   # open a link
cargo run --locked --release -- "rust egui"                      # or search
```

| Key | Action |
|---|---|
| Space, K | Play or pause |
| ←, → | Back or forward 5 s |
| J, L | Back or forward 10 s |
| Ctrl+← / → (Option on macOS) | Previous / next chapter |
| Home, End; 0–9 | Beginning / end; position in tens of percent |
| ↑, ↓; M | Scroll page vertically outside the player; volume ±5% inside the player; mute (Shorts use ↑/↓ for videos) |
| C; <, >; comma, period | Captions; speed; previous / next paused frame |
| Shift+N, Shift+P | Next / previous in the playback source |
| I; T; Shift+A | Mini player; theater mode; audio-only mode |
| F, double click | Full screen |
| /; ? | Focus search; open shortcut help |
| Alt+← / →; mouse back / forward | Browse back / forward (Alt keys on Linux/Windows) |
| Esc | Dismiss a menu/help/countdown, leave full screen, or go back |

Text inputs and menus keep their own keys. Focused buttons own Space, and a
focused volume slider keeps its arrow keys. Outside the player, ↑/↓ scroll the
active page vertically and J/K move item selection (`g`/`G` jump to the top or
bottom). Hold the video to temporarily play at 2×; releasing restores the
previous speed. The player menu also offers repeat, caption size/background,
copying a timestamped link, and opening the video in a browser.

Search uses RustyPipe directly, with separate video, channel and playlist
results. Video filters include date, duration, live status and ordering.
Back/forward restores the page, query, filters, selection and scroll position.
Cards have channel links, a menu and quick Watch later/queue actions. The queue
can be reordered or trimmed; the mini player is a compact frameless 16:9 video
box in the bottom-right corner (click to return to the watch page, or hover for
play/pause and close controls). Playlist playback continues beyond the initially
loaded page. Device history supports filtering and removing individual entries.

Settings → Sidebar chooses which sidebar buttons are shown and their order
(switch to hide, arrows to move, Restore default to undo). Account and Settings
cannot be hidden. Liked videos and Playlists still appear only when signed in.
The number keys 1 to 9 open the buttons as the sidebar shows them.

Settings → Playback → Miniplayer while scrolling keeps a video visible in the
corner miniplayer when you scroll past it to read comments. It is off by default.
Scrolling back reveals the inline player again; clicking the miniplayer returns
to the video at the top of the page. Closing the miniplayer pauses playback.

Settings → Playback → Video buffer chooses between mpv's standard demuxer cache
(**Large — 150 MiB / 50 MiB**, the default, buffering several minutes ahead for
unstable connections and instant seeking) and **Small — 50 MiB / 20 MiB** (to
keep RAM usage around ~200 MB and use less data when skipping videos early).
Audio-only mode (`Shift+A` or the headphones button in the player's controls)
streams and decodes
only the audio track (`ba/b` and `vid=no`) for the current video, resetting to
standard video when opening another video or restarting the app.

The watch page also shows the description (timestamps seek), chapters
and comments, loaded through rustypipe. Comments and replies also display
image and animated GIF/WebP attachments. Attachments use the bounded thumbnail
cache, decode in the background and animate only while visible.
Channel pages have tabs — videos,
shorts, live, playlists and about — and a search of the channel's own
videos.

Channel pages include a banner and video ordering. Playlists show their cover,
metadata and the currently playing item. Shorts use a portrait stage with
next/previous controls and wheel navigation. The interface starts in English.
Choose English, Português (Brasil), Español, Français, Deutsch or Italiano under
Settings → Appearance → Language; the change applies immediately and is kept
between runs, including for desktop notifications. Older settings also default
to English. Video titles, comments, and caption preferences keep their own
languages.

The watch page can show a searchable, synchronized transcript and timeline
thumbnail previews when YouTube supplies the required public metadata. These
optional requests start on demand, run without an account, and use bounded
downloads. Missing captions/storyboards or an upstream restriction may leave
the feature unavailable; playback remains through libmpv.

For synthetic UI previews, build with `--features demo` and run:

```sh
cargo run --locked --features demo -- --demo --demo-view watch --demo-size 1280x800
cargo run --locked --features demo -- --demo --demo-view home --demo-light
cargo run --locked --features demo -- --demo --demo-view settings --demo-language pt-BR
```

Demo mode opens an isolated window without accounts, user files, network,
playback or desktop media registration. It renders fixture states, not every
backend interaction. `--demo-capture target/example.png` captures that window
and closes it. The playback bindings follow
[YouTube's published shortcuts](https://support.google.com/youtube/answer/7631406),
with the list and Shorts adaptations described above.

The desktop's media keys and panels (MPRIS, so `playerctl` works too)
control playback, and a second launch hands a link or search to the
window already open instead of opening another.

## Cast to TV

The TV picker supports three paths. YouTube TVs with DIAL/Lounge use native
networking and do not require a helper. This includes Samsung, LG, Roku, Fire
TV and Android/Google TV models that expose YouTube over DIAL; when a TV offers
both DIAL and Google Cast, the picker keeps both connection options. Local
IP addresses distinguish TVs with the same name. Any device
whose YouTube app shows **Link with TV code** (Apple TV, game consoles, a TV on
another network or behind guest-network isolation) can be linked with that
code instead. Google Cast support is optional and uses
[`catt`](https://github.com/skorokithakis/catt) installed separately on `PATH`.
For example, if you already use pipx, install it with `pipx install catt`.
Follow catt's installation instructions for your operating system, check
`catt --version` in the environment that launches YouTFast, then restart
YouTFast if you changed `PATH`. YouTFast never installs the helper automatically.
The helper and its Python dependencies are not bundled in release archives.
Flatpak cannot use an arbitrary host helper; this integration is not yet
qualified for the Flatpak package.

1. Turn on the TV or Chromecast and connect it to the computer's local network.
   Guest-network isolation and blocked multicast can prevent discovery; the
   search is sent on every private network interface, so VPNs and container
   bridges do not hide the TV's network. On Windows, the firewall can drop the
   TVs' replies to a network-wide search: allow YouTFast when Windows asks, or
   use the IP or TV-code options below. This version accepts private/link-local
   IP addresses. Google Cast uses port 8009; YouTube DIAL uses the endpoint
   advertised by the TV.
2. Open a YouTube video in YouTFast and click **Cast to TV** beside **Audio only**
   in the player controls, or use the button in the top bar.
   Discovery runs only when you open the picker for the first time or press
   **Search for TVs**; there is no background network scan.
   YouTube TVs are discovered through SSDP and verified DIAL endpoints;
   AirPlay and Google Cast announcements are also checked for YouTube DIAL
   support. If a TV is missing, enter its IP under **Find a YouTube TV by IP
   address** and press **Find TV**. Manual lookup asks that address for its
   DIAL endpoint, then tries the common paths on ports 8080, 8008 and 8060.
   No app is launched during discovery or IP lookup.
   To link with a code, open YouTube on the TV, then **Settings → Link with TV
   code**, type the 12-digit code under **Link a TV with a code** and press
   **Link TV**. The code is used once and not kept; the linked TV is reached
   through YouTube's servers and stays in the list until YouTFast exits.
3. Choose the device. YouTFast sends the canonical video link and the local
   position rounded down to whole seconds. Live videos request the live edge.
   The local player keeps playing until the TV reports the requested video
   in the playing state, waiting up to a minute for ads and the TV's account
   picker. For DIAL, choosing the TV joins its YouTube app when it is already
   open, or opens it, and establishes an in-memory Lounge connection; an idle
   connection is renewed automatically. If the TV asks you to sign in, pick an
   account or watch as a guest, choose on the TV; YouTFast never handles that
   login.
   A late confirmation never pauses another local
   video, including a newly opened playback of the same video.
4. Use the panel's explicit TV controls: play, pause, back/forward ten seconds,
   volume down/up ten percentage points, and **Stop on TV**. Commands are sent
   one at a time; the panel does not continuously synchronize TV status.
5. Closing the panel just hides it. Reopen it from the top bar to control the
   same target. **Disconnect; keep TV playing**, or exiting YouTFast, leaves
   playback on the TV and discards the in-memory control session. **Stop on
   TV** asks a Cast receiver to stop, or closes the DIAL YouTube app; a TV that
   does not allow closing it, or one linked with a code, stops the video. Neither action
   automatically resumes the local player.

The panel is a separate remote: the local player's controls, keyboard shortcuts
and desktop media keys continue to control the computer. The Cast button's dot
means a target is retained, not that playback is still running. A successful
initial confirmation is historical; TV-side changes are not polled. A timeout
or error may occur after the TV starts: check the TV before sending again. The
panel retains an uncertain target so you can still stop it or disconnect.

Google Cast targets are Chromecast, Google TV Streamer,
and compatible Google TV, Android TV, LG, Samsung and other TVs with Cast
enabled. Support depends on the exact model and firmware, not the brand.
Known Google speakers (Home, Nest Mini, Nest Audio, Chromecast Audio) are left
out of the list; other audio-only Cast devices may still appear and are not
video targets. A TV that does not report its volume (for example, with sound
on an HDMI-ARC receiver) keeps volume on its own remote.
DIAL targets need a working YouTube app exposing a screen identifier and
access to YouTube Lounge. AirPlay is only a discovery hint; media is not sent
through AirPlay, Miracast or DLNA. Native Windows/macOS and Flatpak qualification
are pending. The Lounge API is undocumented and may change independently of
YouTFast; its protocol references are the ytcast and casttube projects.

The TV's YouTube application and account handle playback, including ads and
access restrictions. YouTFast does not transfer its account session, browser
cookies or local library. With **Skip sponsors (SponsorBlock)** enabled, YouTFast
also checks the sent video's TV position and sends remote seeks over DIAL/Lounge
or Google Cast (`catt`). It uses the same automatic categories as local playback:
sponsors, self-promotion and interaction reminders. Keep YouTFast running and
connected; disconnecting or disabling SponsorBlock stops further automatic seeks.
This applies to the video explicitly sent, not subsequent videos chosen by the
TV's autoplay. Live streams, paused playback and missing seek telemetry are not
skipped. Checks run about every two seconds near a segment and up to ten seconds
apart when the next one is far. TV position polling and helper startup can delay a skip; the last two
seconds of a segment are left alone. Lounge advertisement events suppress sponsor
seeks; `catt` does not expose a reliable separate ad flag, so Cast relies on the
receiver's play state and seek capability. This does not block YouTube ads.
Physical TV/Chromecast and native platform qualification remain pending.

Local captions settings, local autoplay/queue and account history-sync rules are
not transferred to the TV;
YouTube on the TV may record its own history. Discovery results and the selected
TV are kept only in memory. Raw helper output and errors are never displayed
or logged, including with verbose logging.

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

**History sync** in Settings → History applies while signed in:

- **Separate** (the default) keeps the Account and This device lists separate.
- **Merged in YouTFast** opens History on **All**, combining both lists without
  duplicate videos. **Account** and **This device** remain available as filters.
  Search filters the combined list; local resume positions and watched notes
  take precedence. Nothing is sent to YouTube.
- **Merged and sent to YouTube** also reports videos that actually start playing
  here, after two seconds of active playback, to the account active when playback
  starts. Quickly passed-over Shorts are not reported. This sends playback reports
  to Google from your account; Google may treat it as automated access. Failed
  reports do not interrupt playback. The account list refreshes when History is
  next opened after a successful report.

The merged list keeps YouTube's day headings. Because account history has day
labels rather than exact watch times, ordering between the two sources is an
approximation; unfamiliar day labels retain account order. The remove button is
available only for videos kept on this device and deletes only that local entry.
Account history stays read-only for deletion. The setting has no effect while
signed out.

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
choice. Subscribing and the device's Watch later act locally. Rating a video or
editing an account playlist through the watch page changes that selected
account, with rollback if YouTube refuses it. History sync can additionally
report started playback to that account when you explicitly choose **Merged and
sent to YouTube**; the default sends no playback reports.

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

## Interface font

Settings → Appearance → Interface font. **System** (the default) draws
YouTFast with your desktop's own interface font: San Francisco on macOS,
Segoe UI on Windows and, on Linux, the font your desktop is set to use
(GNOME's interface font, GTK's or KDE's setting), else what fontconfig's
`system-ui` names.
**Inter** is the font YouTFast ships with, the same on every computer. The
change applies at once. Other scripts (Arabic, Chinese, Devanagari, ...) are
drawn with the fonts installed on the computer either way.

If the system's font cannot be found or read, YouTFast draws with Inter and
the Appearance group says so. **Report this** opens your browser at a new
GitHub issue with the YouTFast version, your operating system, the desktop
session, the font your desktop is set to use and what `fc-match system-ui`
prints already written in; you read it,
add the font your desktop uses, and decide whether to submit. YouTFast sends
nothing by itself. The player's time keeps Inter's fixed-width digits with
either choice, so it does not jiggle while it counts.

## Themes

Settings → Appearance. **Automatic** follows the Omarchy desktop's theme
live on Linux, and is dark elsewhere. **Dark** and **Light** are YouTFast's
own colours. The eight shared palettes (Catppuccin, Nord, Rosé Pine, Tokyo
Night, and more) are installed into `~/.config/youtfast/themes` on the
first run as plain JSON files you can edit, copy or delete; add your own
there in the same format (a `base` plus named colours, see
[fastframe-theme](https://github.com/crmne/fastframe)). A source build does
not activate the Omarchy hook (`contrib/omarchy/youtfast-theme`). Native Linux
packages include it for optional installation. Flatpak does not activate host
hooks or implicitly read host themes. Native theme following also works without
the hook where theme files are accessible.

## About and updates

Settings → About shows the installed YouTFast version, license, and bundled
changelog, and checks GitHub Releases for newer stable versions:

- **Notify about new versions** (on by default) sends a system desktop
  notification when a new release is published and highlights Settings in the
  sidebar, with quick actions in the notification toast to update or open About.
- **Automatic updates (minor and patch)** (off by default) automatically
  downloads, verifies (SHA-256 against the release's `checksums.txt`), and
  installs minor (`0.x.0`) and patch (`0.0.x`) updates in the background, then
  displays the changelog of what changed in Settings → About.
- **Major versions (`x.0.0`)** never install automatically; they require
  clicking the **Update** button in the app to confirm and install the release.

## Help

Read [CONTRIBUTING.md](../CONTRIBUTING.md) for bug reports and
[SECURITY.md](../SECURITY.md) for private account/security issues.
