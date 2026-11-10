# YouTFast architecture

> The map of how YouTFast is shaped, for someone reading or changing the
> code: how a frame runs, what each crate holds, and what holds across them.
> [AGENTS.md](../AGENTS.md) and [CONTRIBUTING.md](../CONTRIBUTING.md) hold the
> rules for changing it; [youtube-api-format.md](youtube-api-format.md) holds
> YouTube's format changes.

## Purpose

YouTFast is a small native YouTube client: Rust, egui for the interface and
libmpv for playback, with no browser engine, telemetry or hosted backend.
YouTube's data comes through InnerTube (rustypipe) and playback through mpv's
yt-dlp hook. What it keeps (the library, the settings, the signed-in sessions)
stays on the user's computer.

## Crates

The repository is a Cargo workspace. The root `Cargo.toml` holds the shared
package fields, every dependency's version and features, the profiles and the
patches; each crate in `crates/` inherits them with `.workspace = true`.

A crate depends only on crates in lower rows:

```text
7  youtfast                          the binary
6  youtfast-ui                       the views
5  youtfast-app                      state, actions, the frame
4  youtfast-backend   youtfast-cast
3  youtfast-logic     youtfast-youtube
2  youtfast-storage   youtfast-desktop
1  youtfast-core
0  youtfast-i18n      youtfast-mpv   youtfast-appearance
```

| Crate | Holds |
|---|---|
| `youtfast-i18n` | the interface languages; its build script compiles the catalogs in `assets/i18n` |
| `youtfast-mpv` | the player: libmpv's client API (`ffi`), `Mpv`, and its OpenGL picture (`render`) |
| `youtfast-appearance` | palette (`theme`), icons, fonts, the desktop's theme and font settings |
| `youtfast-core` | the shared types (`model`), formatting helpers (`util`), YouTube link parsing (`links`), the fixed messages external errors become (`privacy`) |
| `youtfast-storage` | what stays on this computer: library, settings, sessions (`account`), Takeout, file paths |
| `youtfast-desktop` | one running copy per user (`instance`), the local logs and their redactor (`diagnostics`), the release updater |
| `youtfast-logic` | pure decisions over video lists: new-upload alerts, history, local recommendations, video tabs |
| `youtfast-youtube` | the network: InnerTube (`innertube`), yt-dlp (`ytdlp`), SponsorBlock, captions and storyboards (`extras`), the thumbnails' cache |
| `youtfast-cast` | casting to TVs: Google Cast, DIAL (`dial`) and YouTube's Lounge API (`lounge`) |
| `youtfast-backend` | the runtime thread; `protocol` is its `Command`s and `Event`s |
| `youtfast-app` | the state, the actions the views push (`Action`), how they are carried out, the remote images (`images`), the desktop's media controls (`now_playing`) |
| `youtfast-ui` | the views; `frame` draws one frame |
| `youtfast` | the command line, the window's startup (`main.rs`) and the offline demo harness (`demo.rs`, feature `demo`) |

A new module goes in the crate that matches its job. If it needs a crate in a
higher row, it belongs higher up, or the dependency goes the wrong way.

## A frame

1. `main` parses the command line, hands it to a running copy if there is one
   (`desktop::instance`), looks for the installed fonts on a thread
   (`theme::SystemFonts`), starts the logs (`desktop::diagnostics`) and opens
   the window with `App::new(..., youtfast_ui::frame)`.
2. Each frame, `App::logic` takes in what happened since the last one: the
   backend's `Event`s (`App::poll`), the player's status, the themes on disk,
   the remote images to fetch or forget, a second launch's request, the
   desktop's media controls. Then it runs the playback, prefetch and search
   suggestion ticks.
3. `App::ui` calls the views (`ui::frame`, handed to `App::new` as
   `app::Views`). They draw from `app::State` and `mpv::Status` and push
   `app::Action`s; a view only changes its own state (the search text, the
   seek bar while dragged).
4. `App::apply` carries out each action after the frame: navigation and the
   player at once, anything slow as a `Command` to the backend.
5. The backend runs it on its tokio runtime and answers with an `Event`, which
   wakes the window. An answer to an older request is dropped
   (`App::generations`), and events carry the session epoch they belong to.

The application never calls the views directly and the views never reach the
backend: `ui` depends on `app`, `app` on `backend`, never the other way round.

## Invariants

- **Layers.** No crate depends on one in its own or a higher row; Cargo
  refuses cycles, and the rows above say which direction is allowed.
- **Unsafe code.** Only `youtfast-mpv` has `unsafe` code (libmpv's C API).
  The workspace denies `unsafe_code`; the frame-cost benchmark's counting
  allocator (`youtfast-ui`, test-only) is the one other allowed place.
- **The interface thread never blocks.** Requests, saves, Takeout imports,
  the credential store and notifications run on the backend's runtime or on
  blocking threads it owns; only `App::on_exit` writes the last state
  directly.
- **The session stays in the credential store.** YouTube cookies live only in
  the platform credential store (`storage::account`): never in a file, the
  settings, the library, rustypipe's cache, a log line or an error message.
- **Controlled messages.** External errors become one of `privacy`'s fixed
  messages, and every log record passes `diagnostics::redact_log`. The
  redactor and the log filters (`youtfast=info`) recognise the application by
  the `youtfast` prefix of its log targets, so every crate's name starts with
  `youtfast`.
- **No IO in core.** `youtfast-core` performs no IO and draws nothing, so the
  types and helpers every crate shares stay cheap to build and test.

## Crate by crate

### youtfast-mpv

- `src/ffi.rs` declares the libmpv functions used, by hand (client API 2.x).
  `src/lib.rs` wraps them: `Mpv` owns the player, observes the properties the
  interface shows, and is polled once per frame from `App::logic`; its wakeup
  callback only requests a repaint. `Status::video_aspect` is the displayed
  aspect (`video-out-params/aspect`, after autorotate), which the watch page
  uses to size the picture. libmpv is linked from the system.
- mpv's network cache (the demuxer cache) defaults in `Mpv::new` to mpv's
  standard 150 MiB ahead and 50 MiB behind (`PlaybackBuffer::Standard`), and
  `Settings::playback_buffer` can switch it to 50 MiB ahead and 20 MiB behind
  (`PlaybackBuffer::Compact`) to keep the process near 200 MB on long videos.
- Playback hands the watch link to mpv, whose ytdl hook asks yt-dlp for the
  streams (`mpv::FORMAT`, bounded by the quality chosen, 1080p on Auto).
  Updating yt-dlp fixes YouTube breakage without a release. Videos play signed
  out: with a session, yt-dlp only has YouTube's web clients, which without a
  PO token offer little more than one 360p stream. A video refused signed out
  (bot check, age limit, private) is tried once more through a browser
  account's session (`Mpv::retry_signed_in`: yt-dlp reads the browser itself,
  `cookies-from-browser`, naming the keyring that opened it:
  `account::ytdl_browser`); a cookies.txt account has no such retry.
- `src/render.rs` draws the picture inside the egui window. libmpv's OpenGL
  render API draws into a framebuffer of ours, sized to the video's rectangle
  in physical pixels; an `egui_glow::CallbackFn` blits it onto egui's target.
  The render context is made lazily inside the paint callback (where the GL
  context is current), and freed in `App::on_exit`, before mpv. A dropped
  surface never frees GL objects: without a current context that would be
  undefined. The wl_display or X11 Display goes to mpv so hardware decoding
  can share frames with OpenGL (VAAPI on Linux). Scrolling keeps the
  framebuffer at the full video's size, even outside the window; reinstate
  egui's scissor after mpv renders and before blitting.
- The eframe fork exposes `CreationContext::get_proc_address`, which the
  render API needs. That is one more reason to stay on the shared egui and
  winit forks; move their revisions together with the sibling apps.

### youtfast-appearance

- `src/theme.rs` is the palette: YouTube's dark and light defaults, the
  mapping onto `egui::Visuals`, and the fastframe-theme catalogue's desktop
  integration (shared presets, Omarchy on Linux, followed live).
  `Settings::palette` resolves the choice through the caches; a change is
  revealed by `fastframe_theme::Transition`. The Omarchy hook
  (`contrib/omarchy/youtfast-theme`) calls `youtfast reload-themes`; the
  palette is also followed through filesystem notifications.
- `src/icons.rs` lists the Lucide icons fastframe-icons shares with the
  sibling apps; `fastframe_icons::install` serves them to egui.
- The window does not wait for the installed fonts that draw the scripts
  Inter lacks: finding them probes every installed font (1.5 s from disk), so
  `theme::SystemFonts` looks on a thread started in `main`, and `App` swaps
  them in once found. `theme::install_fonts` is the synchronous path for tests
  and the demo.
- `InterfaceFont::Family` is an installed family chosen in the settings. On
  Linux `desktop_font::family` resolves it like the desktop's own font
  (`fc-match`, one weight at a time) and `desktop_font::installed_families`
  lists what `fc-list :lang=en` knows, both on the font thread; the list
  reaches the views as `State::font_families`. A font file read for a face
  stays in memory for the life of the process, once per file, because egui
  shares a face's bytes between weights only when they are `'static`. A
  family that cannot be loaded draws with Inter and sets
  `theme::face_missing`. A saved value this version does not know reads as
  the system's face instead of failing the settings file.

### youtfast-storage

- `src/library.rs` is what this computer keeps, with or without an account:
  subscriptions, history (with resume positions) and Watch later, in one JSON
  file (`paths::Dirs::library`). Saves go to the backend
  (`Command::SaveLibrary`) and are atomic (`paths::write_atomic`); an
  unreadable file is moved aside, never overwritten. New fields need
  `#[serde(default)]` so older files keep loading. `on_exit` saves directly.
- `src/settings.rs` is what survives between runs (quality, volume, the theme
  choice and its palette caches), in one JSON file at `paths::Dirs::settings`.
  Loads and saves mirror `library.rs`; the backend writes it
  (`Command::SaveSettings`), and `on_exit` saves directly.
- `src/takeout.rs` imports Google Takeout's `subscriptions.csv` (columns by
  position: the header is localized) and `watch-history.json`.
- `src/account.rs` holds the sessions: cookies.txt parsing, reading a
  browser's cookies through yt-dlp's `extract_cookies_from_browser` (a short
  Python script whose output is piped, never written; it tries each Linux
  keyring because yt-dlp guesses wrong on Hyprland), and the credential store,
  called through `spawn_blocking` with a timeout. Each named account
  (`library::Account`) has its own keyring entry `youtube-session-<id>`; the
  single unnumbered entry of older files moves to account 1 on restore. A
  browser source may carry a profile (`browser:profile`). rustypipe's cache
  goes through `account::CacheFile`, which strips `auth_cookie` and
  `oauth_token`; its error reports are off (`no_reporter`) because they hold
  whole responses. A session from a browser is read again from the browser
  when it expires (`backend::restore`).

### youtfast-desktop

- `src/instance.rs` is one running copy per user (fastframe-instance): a
  second launch hands `show`, `open <text>` or `reload-themes` to the window
  already open and exits; the handler only parses and queues, `App` drains the
  queue.
- `src/diagnostics.rs` writes the local logs: every record passes the privacy
  redactor (`redact_log`) before reaching stderr or the bounded, owner-only
  log files.
- `src/updater.rs` checks GitHub Releases for newer stable versions,
  classifies them by SemVer (`UpdateKind::{Patch, Minor, Major}`), verifies
  release archives against `checksums.txt` (SHA-256), and installs them
  atomically. Settings → About exposes system update notifications, opt-in
  automatic updates for minor/patch versions with persisted changelog notes,
  and manual 1-click installation for major versions (`should_auto_update`
  never auto-installs `UpdateKind::Major`).

### youtfast-logic

- `src/history.rs` merges account day sections with device-only history,
  deduplicates video ids and keeps local resume metadata; History's filters
  are independent of other pages' `Source` in merged modes.
- `src/alerts.rs` is the new-upload check's brain: a pure split of a feed's
  videos into unseen (`alerts_seen_at`) and not-yet-notified
  (`alerts_notified_at`) stamps the library keeps. A tokio timer in the
  backend (restarted by `Command::SetAlertsEvery`; the feed's own load runs
  the same computation without a request) fires `Event::CheckAlerts`, and
  `Command::Notify` shows the result through notify-rust on a blocking thread.
  YouTube's real notifications have no API; this is the replacement the user
  chose.
- `src/recommend.rs` ranks the related videos of the history's latest videos
  for Home's For you when the account has no recommendations, or there is no
  account.

### youtfast-youtube

- `src/innertube.rs` is everything through rustypipe: the home feed (the
  account's subscription feed plus RSS for channels subscribed on this
  device, merged and deduplicated), a channel's RSS, and the account's
  subscriptions, history, Watch later, liked videos and playlists. The watch
  page's description, chapters, recommended videos and comments come from it
  too (`details`, `comments`; comment requests carry the watched video id so
  spans in comments, replies and descriptions map links on that video to
  seeks, including `0:00`), and channel pages are its tabs (`channel_tab`, `channel_about`):
  videos, shorts, live, playlists and the channel's own search, paged through
  `More` continuations (`more_videos`, `more_playlists`), as is the home feed
  (`more_videos` again). A channel tab's videos come without their channel;
  `More::owner` carries the header's channel to every page, which fills the
  name, id and avatar of the channel's own videos (`Owner::fill`).
- Home's For you is the account's own recommendations
  (`src/innertube/home.rs`: `browse` `FEwhat_to_watch` through rustypipe's
  `raw`, read here, continued by rustypipe).
- Playlists (Watch later and liked videos included) come in a newer layout
  rustypipe fails to read; when it does, `src/innertube/playlist.rs` reads
  them through `raw` (`browse` `VL<id>`) and pages them itself
  (`More::ours`) — its shorts land on the shelf, filled further by
  `channel_shorts` (each device channel's Shorts tab).
- rustypipe has no posts or podcasts tabs, no write actions and no home
  recommendations call: subscribing acts on the local library. The watch
  page's rating and the account playlists of `[ save ]` are explicit writes to
  the account (`src/innertube/edit.rs`, through rustypipe's `raw` request to
  InnerTube's `like/*`, `playlist/get_add_to_playlist` and
  `browse/edit_playlist`); ratings are kept 30 minutes (`App::ratings`), and
  each write is shown at once and undone if YouTube refuses it.
- Opt-in History sync reports started playbacks through
  `src/innertube/history_sync.rs`: an authenticated raw `player` request
  supplies the playback tracking URL, and the forked client's
  `history_tracking` transport sends it with the same session, only to HTTPS
  YouTube hosts and without redirects. This does not change mpv's signed-out
  extraction. A failed report is `NotReported`, which carries nothing; URLs,
  cookies and response bodies never reach the interface or logs.
  `youtfast-app`'s `history_sync.rs` gates one report per playback on two
  seconds of active play and the session active at its start.
- rustypipe is YouTFast's fork (github.com/bcosta19/rustypipe, branch
  `youtfast`, pinned by `rev` in the root `[patch.crates-io]`): 0.11.4 with
  upstream PR #80 (codeberg.org/ThetaDev/rustypipe) applied, for comment
  authors' avatars and the comment count, and with a failed visitor-data
  fetch returned as an error instead of unwrapped (a panic aborts the release
  build), reqwest errors kept with their causes (the only place a refused
  certificate shows up), history reports sent through the shared client, and
  video lockups keeping their channel's avatar (YT-6). Channel pages follow
  YouTube's October 2026 layout: `order_ctoken` builds the sort chips' token
  (`{1: target, 3: order}` under field 8, 7 on Shorts), Shorts items read
  their `thumbnailViewModel`, the Playlists tab sends its new `params`,
  podcast lockups count as playlists, and header metadata parts are told apart
  by their field (`MetadataPart` is untagged). The `regression` feature (a
  dev-dependency feature only) exposes `client::regression`, the hooks
  `tests/youtube_api.rs` calls. Drop the fork once a release carries these
  changes.
- rustypipe's reqwest client checks certificates through the operating
  system (`innertube::http`, rustls-platform-verifier). Do not go back to a
  root list read at startup: a fresh Windows holds few roots and downloads the
  rest only when its own verifier asks, so rustypipe failed there until a
  browser had reached Google. Thumbnails, SponsorBlock and the updater go
  through ureq, which bundles Mozilla's roots.
- `src/ytdlp.rs` parses yt-dlp's extraction JSON; search goes through
  rustypipe. Search suggestions come from rustypipe's `search_suggestion`,
  signed out, once typing pauses.
- `src/sponsorblock.rs` asks sponsor.ajay.app for a video's segments by the
  first four hex digits of its id's SHA-256, never the id; it is off unless
  the settings turn it on.
- `tests/youtube_api.rs` holds the response-format regression tests
  ([youtube-api-format.md](youtube-api-format.md)).

### youtfast-cast

- `src/lib.rs` is the Google Cast adapter: the `catt` helper owns Cast
  discovery, and a lazy worker serializes commands outside the interface and
  the YouTube session's runtime. No account cookies, raw helper output or TV
  addresses are saved.
- `src/dial.rs` is local DIAL discovery and app lifecycle; discovery only
  reads. Samsung also advertises AirPlay when SSDP is filtered: that is a
  discovery hint, never permission to send media through AirPlay.
- `src/lounge.rs` is YouTube on TV's Lounge transport, independent of
  rustypipe's account session: pairing identifiers and tokens live only in the
  worker, and no cookies, response bodies or transport errors leave it.

### youtfast-backend

- `src/lib.rs` is the interface's handle to a tokio runtime on its own
  thread. The two sides talk only through `Command` and `Event`
  (`src/protocol.rs`), and every event wakes the window.
- Lists the views show are `model::Listing`s filled by `Command::Load`
  requests, one current request per `backend::Slot`; an answer to an older one
  is dropped (`App::generations`). Account-or-device pages switch with
  `model::Source`.

### youtfast-app

- `src/lib.rs` is `App`: it applies the views' actions after the frame,
  through its children (loading, watch, accounts, notifications, themes).
- Remote images go through `images::remote`, which lets egui forget the ones
  not drawn for a minute; the watch page's details are kept for the last 50
  videos (`App::details_cache`), for half an hour.
- `src/player.rs` is the logic behind the player's controls: what plays next
  (the queue, Up next, then related), the kept captions language, the skips.
  mpv runs with `keep-open`, so the end of a video is `eof-reached` turning
  on, not an end-file event.
- `src/now_playing.rs` is the desktop's media controls
  (fastframe-now-playing; MPRIS on Linux): it maps their commands onto
  `Action`s and reports what mpv plays; the units differ (`SeekBy` is
  milliseconds, `SetVolume` is 0..1).

### youtfast-ui

- The top-bar bell opens a notifications popup over the current view. It
  shares the full page's visit snapshot, mark-seen action and video rows;
  opening either surface marks the current alerts seen. New arrivals while a
  surface is open join that snapshot and are marked seen.
- `src/player.rs` is the player as YouTube draws it: the controls over the
  video's bottom edge (shown while the pointer moves, kept while paused), the
  seek bar with chapters and SponsorBlock's segments, the settings menu
  (speed, quality, captions, autoplay), notes over the picture, the countdown
  to the next video and the live badge. A live stream has no timeline: mpv
  only seeks inside its cache (some 25 s), so it gets the badge and "back to
  live" instead of a DVR bar. Shorts are drawn upright and loop
  (`loop-file`). `src/watch.rs` sizes the watch page's video box to the
  displayed aspect (square and portrait videos use a 16:9 box), capped at
  1280 px. It reserves 72 px for the title and a 24 px bottom inset (channel
  info and actions scroll below; a window too short for the minimum player
  height scrolls), targets YouTube's short-window minimums (360 px for 16:9
  and narrower, 240 px for wider) when the column allows, and gives Up next
  the remaining 380–600 px with large rows. Theater mode fills the content
  width with mpv letterboxing and keeps YouTube's 113 px metadata strip below
  the player, leaving room for the title and channel row above the fold when
  the window is tall enough; metadata and Up next keep their columns beneath
  it in a wide window.
- Leaving the watch page hands the video to the mini player
  (`player::mini`), which keeps drawing it: an undrawn render-API video
  output stalls. Closing the mini player pauses the video; playing again from
  the desktop's media controls then comes back to the watch page
  (`App::show_playing`). The optional `Settings::scroll_miniplayer` also shows
  it when the inline video scrolls completely above the watch page's clip.
  Returning from it sets `State::watch_scroll_to_video` to reveal the inline
  player.
- `src/widgets.rs` lays out rich-text spans in descriptions, comments and
  replies. Clicking a web link sends `Action::OpenUrl`, whose existing scheme
  restrictions apply; timestamps seek, and the context menu copies the text.
  Link hit testing and hover underlines use each glyph's logical rectangle,
  translated by its row; glyph positions alone denote the font baseline.
  The shared `eframe` dependency enables `links`: without it the native
  integration ignores `OpenUrl` commands instead of opening the browser.
- Controls fade into their hover and pressed looks instead of switching in
  one frame: `widgets::hover_fade` and `widgets::press_fade` wrap egui's
  `animate_bool`, which only asks for new frames while a fade runs, so an
  idle window still draws nothing. Their length is the style's
  `animation_time` (`theme::apply`).
- `src/settings.rs` centres its column in the page; a page with room for it
  also lists the groups in an index beside the column. The index stays put
  while the page scrolls, marks the group at the top of the page and scrolls
  to the one clicked; what it remembers between frames is view state kept in
  egui's memory. A row whose control is a switch takes the click anywhere on
  it (`switch_row`): the row's click is registered where the previous frame
  drew it, before the switch, so the switch stays above it.
- `src/bench.rs` measures what one frame costs on the busiest views:
  `cargo test --release -p youtfast-ui --lib frame_cost -- --ignored
  --nocapture --test-threads=1`.

### youtfast

- `src/main.rs` is the command line and the window's startup; `build.rs`
  embeds `packaging/icons/youtfast.ico` in the Windows executable through
  windres, called directly (the runners' build-script host is MSVC while the
  target is GNU). That icon and `youtfast.icns` (the macOS bundle's) are
  generated from the SVGs by `tools/icons.py`; regenerate them when the mark
  changes.
- `src/demo.rs` is the offline UI harness behind the `demo` feature: no
  backend, keyring, user files, network, media controls or single-instance
  channel.

## Building and testing

The commands in [CONTRIBUTING.md](../CONTRIBUTING.md#checks) cover the whole
workspace. While working on one crate, `cargo test -p <crate>` builds and runs
only that crate and the ones below it, for example
`cargo test -p youtfast-storage`. The response-format tests run with
`cargo test -p youtfast-youtube --test youtube_api`.
