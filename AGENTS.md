# YouTFast agent guide

YouTFast is a small native YouTube client: Rust, egui, and libmpv for
playback. Follow `CONTRIBUTING.md` for contributor policy and `RELEASING.md`
for release gates. This repository can be built without sibling checkouts.
`../spotifast` and `../zapfast`, when available, are convention references;
this guide and the repository's own contribution policy take precedence.

Anything that talks to YouTube can break when YouTube changes its responses:
read [docs/youtube-api-format.md](docs/youtube-api-format.md) before working
on it, and follow [YouTube response formats](#youtube-response-formats).

## Product boundaries

- Keep it a small native client. No browser engine, no telemetry, no hosted
  backend, no YouTFast-operated account system.
- Video goes through libmpv. Do not decode video in-process the way ZapFast's
  `src/video.rs` does: that path is CPU-only and tops out far below what
  YouTube serves (VP9/AV1, 4K, 60 fps, HDR).
- YouTube data/search goes through RustyPipe; playback extraction uses yt-dlp.
  Do not scrape YouTube's HTML or reimplement signature or n-parameter
  deobfuscation here; that belongs to an extractor library.
- The signed-in session (YouTube cookies) is the user's Google session.
  It lives only in the platform credential store (`src/storage/account.rs`): never
  in a file, settings, the library, rustypipe's cache, a log line or an
  error message. Credential-store errors are mapped to fixed messages,
  because their text can carry platform data.
- Do not read the user's browser cookies, library or history to debug.
  Hand the user the command and let them report back.
- Do not broaden a task into adjacent features or a general refactor.

## Architecture

`src/` is grouped by domain. A new module goes in the group that matches
its job, not at the root:

| Folder | Holds |
|---|---|
| `ui/` | the views |
| `app.rs`, `app/` | the state and the actions the views push |
| `backend.rs`, `backend/` | the runtime thread; `protocol.rs` is its `Command`s and `Event`s |
| `youtube/` | the network: InnerTube (`innertube`), yt-dlp (`ytdlp`), SponsorBlock, captions and storyboards (`extras`), the thumbnails' cache |
| `storage/` | what stays on this computer: library, settings, sessions (`account`), Takeout, file paths |
| `logic/` | pure decisions over video lists: new-upload alerts, local recommendations |
| `mpv/` | the player |
| `desktop/` | single instance, media controls, local logs, release updater |
| `appearance/` | palette, icons, interface languages |
| `model.rs`, `util.rs` | the shared types and formatting helpers |

- `src/ui/` draws views from `app::State` and the player's `mpv::Status`, and
  pushes `model::Action`s. `src/app.rs` applies them after the frame,
  through its children in `src/app/` (loading, watch, accounts,
  notifications, themes). `src/app/history_sync.rs` gates one report per
  playback on two seconds of active play and the session active at its start. A view only changes its own state (search text,
  the seek bar while dragged).
  Remote images go through `ui::images::remote`, which lets egui forget
  the ones not drawn for a minute; the watch page's details are kept for
  the last 50 videos (`App::details_cache`), for half an hour.
- `src/backend.rs` is the interface's handle to a tokio runtime on its own
  thread. The two sides talk only through `Command` and `Event`
  (`src/backend/protocol.rs`), and every event wakes the window. Nothing blocks the interface thread.
- `src/storage/library.rs` is what this computer keeps, with or without an
  account: subscriptions, history (with resume positions) and Watch later,
  in one JSON file (`paths::Dirs::library`). Saves go to the backend
  (`Command::SaveLibrary`) and are atomic (`paths::write_atomic`); an
  unreadable file is moved aside, never overwritten. New fields need
  `#[serde(default)]` so older files keep loading. `on_exit` saves directly.
- `src/storage/takeout.rs` imports Google Takeout's `subscriptions.csv` (columns by
  position: the header is localized) and `watch-history.json`.
- `src/youtube/innertube.rs` is everything through rustypipe: the home feed (the
  account's subscription feed plus RSS for channels subscribed on this
  device, merged and deduplicated), a channel's RSS, and the account's
  subscriptions, history, Watch later, liked videos and playlists. The watch
  page's description, chapters, recommended videos and comments come from it
  too (`details`, `comments`; spans map YouTube links on the video itself to
  seeks), and channel pages are its tabs (`channel_tab`, `channel_about`):
  videos, shorts, live, playlists and the channel's own search, paged through
  `More` continuations (`more_videos`, `more_playlists`), as is the home
  feed (`more_videos` again). Home's For you is the account's own
  recommendations (`src/youtube/innertube/home.rs`: `browse` `FEwhat_to_watch`
  through rustypipe's `raw`, read here, continued by rustypipe), and when
  the account has none or there is no account, `src/logic/recommend.rs` ranks
  the related videos of the history's latest videos. Playlists (Watch later and liked videos included) come in a newer
  layout rustypipe fails to read; when it does, `src/youtube/innertube/playlist.rs`
  reads them through `raw` (`browse` `VL<id>`) and pages them itself
  (`More::ours`) — its shorts land on the shelf, filled further
  by `channel_shorts` (each device channel's Shorts tab). rustypipe has no
  posts or podcasts tabs, no write actions and no home recommendations call:
  subscribing acts on the local library. The watch page's rating and the
  account playlists of `[ save ]` are explicit writes to the account
  (`src/youtube/innertube/edit.rs`, through rustypipe's `raw` request to
  InnerTube's `like/*`, `playlist/get_add_to_playlist` and
  `browse/edit_playlist`); ratings are kept 30 minutes (`App::ratings`),
  and each write is shown at once and undone if YouTube refuses it.
  Opt-in History sync reports started playbacks through
  `src/youtube/innertube/history_sync.rs`: an authenticated raw `player`
  request supplies the playback tracking URL, and the vendored client's
  `history_tracking` transport sends it with the same session, only to
  HTTPS YouTube hosts and without redirects. This does not change mpv's
  signed-out extraction. Tracking errors have one fixed local message;
  URLs, cookies and response bodies never reach the interface or logs.
  rustypipe is a copy in `vendor/rustypipe` (`[patch.crates-io]`): 0.11.4
  with upstream PR #80 (codeberg.org/ThetaDev/rustypipe) applied, for
  comment authors' avatars and the comment count, and with a failed
  visitor-data fetch returned as an error instead of unwrapped (a panic
  aborts the release build), reqwest errors kept with their causes (the
  only place a refused certificate shows up), and history reports sent through
  the shared client. Channel pages follow YouTube's
  October 2026 layout: `order_ctoken` builds the sort chips' token (`{1: target, 3:
  order}` under field 8, 7 on Shorts), Shorts items read their
  `thumbnailViewModel`, the Playlists tab sends its new `params`, podcast
  lockups count as playlists, and header metadata parts are told apart by
  their field (`MetadataPart` is untagged). The `regression` feature (a
  dev-dependency feature only) exposes `client::regression`, the hooks
  `tests/youtube_api.rs` calls. Drop the copy once a release carries these
  changes.
- rustypipe's reqwest client checks certificates through the operating
  system (`innertube::http`, rustls-platform-verifier). Do not go back to a
  root list read at startup: a fresh Windows holds few roots and downloads
  the rest only when its own verifier asks, so rustypipe failed there until
  a browser had reached Google. Thumbnails, SponsorBlock and the updater go
  through ureq, which bundles Mozilla's roots.
- `packaging/icons/youtfast.ico` (embedded in the Windows executable by
  `build.rs` through windres, called directly: the runners' build-script host
  is MSVC while the target is GNU) and `youtfast.icns` (the macOS bundle's) are generated from the
  SVGs by `tools/icons.py`; regenerate them when the mark changes.
- `src/logic/history.rs` merges account day sections with device-only history,
  deduplicates video ids and keeps local resume metadata; History's filters are
  independent of other pages' `Source` in merged modes.
- `src/logic/alerts.rs` is the new-upload check's brain: a pure split of a
  feed's videos into unseen (`alerts_seen_at`) and not-yet-notified
  (`alerts_notified_at`) stamps the library keeps. A tokio timer in
  `src/backend.rs` (restarted by `Command::SetAlertsEvery`; the feed's own
  load runs the same computation without a request) fires
  `Event::CheckAlerts`, and `Command::Notify` shows the result through
  notify-rust on a blocking thread. YouTube's real notifications have no
  API; this is the replacement the user chose.
- `src/storage/account.rs` holds the sessions: cookies.txt parsing, reading a
  browser's cookies through yt-dlp's `extract_cookies_from_browser` (a short
  Python script whose output is piped, never written; it tries each Linux
  keyring because yt-dlp guesses wrong on Hyprland), and the credential
  store, called through `spawn_blocking` with a timeout. Each named account
  (`library::Account`) has its own keyring entry `youtube-session-<id>`;
  the single unnumbered entry of older files moves to account 1 on
  restore. A browser source may carry a profile (`browser:profile`). rustypipe's cache
  goes through `account::CacheFile`, which strips `auth_cookie` and
  `oauth_token`; its error reports are off (`no_reporter`) because they hold
  whole responses. A session from a browser is read again from the browser
  when it expires (`backend::restore`).
- Lists the views show are `model::Listing`s filled by `Command::Load`
  requests, one current request per `backend::Slot`; an answer to an older
  one is dropped (`App::generations`). Account-or-device pages switch with
  `model::Source`.
- `src/youtube/ytdlp.rs` parses extraction JSON; search goes through RustyPipe.
  Playback hands the watch link to mpv, whose ytdl hook asks yt-dlp for the streams
  (`mpv::FORMAT`, bounded by the quality chosen, 1080p on Auto). Updating yt-dlp fixes YouTube
  breakage without a release. On Windows the ZIP ships `yt-dlp.exe` and `deno.exe` beside
  `youtfast.exe` (pins and SHA-256 in `packaging/windows-tools.json`, bumped every release);
  `mpv/bundled.rs` points mpv's `ytdl_path` and yt-dlp's `js-runtimes` at them when present
  and leaves everything else to `PATH`. Videos play signed out: with a session,
  yt-dlp only has YouTube's web clients, which without a PO token offer
  little more than one 360p stream. A video refused signed out (bot
  check, age limit, private) is tried once more through a browser
  account's session (`Mpv::retry_signed_in`: yt-dlp reads the browser
  itself, `cookies-from-browser`, naming the keyring that opened it:
  `account::ytdl_browser`); a cookies.txt account has no such retry.
- `src/mpv/ffi.rs` declares the libmpv functions used, by hand (client API
  2.x). `src/mpv/mod.rs` wraps them: `Mpv` owns the player, observes the
  properties the interface shows, and is polled once per frame from
  `App::logic`; its wakeup callback only requests a repaint.
- mpv's network cache (the demuxer cache) defaults in `Mpv::new` to mpv's
  standard 150 MiB ahead and 50 MiB behind (`PlaybackBuffer::Standard`), and
  `Settings::playback_buffer` can switch it to 50 MiB ahead and 20 MiB behind
  (`PlaybackBuffer::Compact`) to keep the process near 200 MB on long videos.
- `src/mpv/render.rs` draws the picture inside the egui window. libmpv's
  OpenGL render API draws into a framebuffer of ours, sized to the video's
  rectangle in physical pixels; an `egui_glow::CallbackFn` blits it onto
  egui's target. The render context is made lazily inside the paint callback
  (where the GL context is current), and freed in `App::on_exit`, before mpv.
  A dropped surface never frees GL objects: without a current context that
  would be undefined. The wl_display or X11 Display goes to mpv so hardware
  decoding can share frames with OpenGL (VAAPI on Linux).
  Scrolling keeps the framebuffer at the full video's size, even outside the
  window; reinstate egui's scissor after mpv renders and before blitting.
- The eframe fork exposes `CreationContext::get_proc_address`, which the
  render API needs. That is one more reason to stay on the shared egui and
  winit forks; move their revisions together with the sibling apps.
- Leaving the watch page hands the video to the mini player
  (`ui::player::mini`), which keeps drawing it: an undrawn render-API video
  output stalls. Closing the mini player pauses the video; playing again
  from the desktop's media controls then comes back to the watch page
  (`App::show_playing`).
  The optional `Settings::scroll_miniplayer` also shows it when the inline
  video scrolls completely above the watch page's clip. Returning from it
  sets `State::watch_scroll_to_video` to reveal the inline player.
- `src/ui/player.rs` is the player as YouTube draws it: the controls over
  the video's bottom edge (shown while the pointer moves, kept while
  paused), the seek bar with chapters and SponsorBlock's segments, the
  settings menu (speed, quality, captions, autoplay), notes over the
  picture, the countdown to the next video and the live badge. A live
  stream has no timeline: mpv only seeks inside its cache (some 25 s), so
  it gets the badge and "back to live" instead of a DVR bar. Shorts are
  drawn upright and loop (`loop-file`). `src/app/player.rs` is the logic
  behind them: what plays next (the queue, Up next, then related), the
  kept captions language, the skips. mpv runs with `keep-open`, so the end
  of a video is `eof-reached` turning on, not an end-file event.
- `src/youtube/sponsorblock.rs` asks sponsor.ajay.app for a video's segments by
  the first four hex digits of its id's SHA-256, never the id; it is off
  unless the settings turn it on. Search suggestions come from rustypipe's
  `search_suggestion`, signed out, once typing pauses.
- `src/storage/settings.rs` is what survives between runs (quality, volume, the
  theme choice and its palette caches), in one JSON file at
  `paths::Dirs::settings`. Loads and saves mirror `library.rs`; the backend
  writes it (`Command::SaveSettings`), and `on_exit` saves directly.
- `src/appearance/icons.rs` lists the Lucide icons fastframe-icons shares with the
  sibling apps; `fastframe_icons::install` serves them to egui.
- The window does not wait for the installed fonts that draw the scripts
  Inter lacks: finding them probes every installed font (1.5 s from disk),
  so `theme::SystemFonts` looks on a thread started in `main`, and `App`
  swaps them in once found. `theme::install_fonts` is the synchronous path
  for tests and the demo.
- `src/appearance/theme.rs` is the palette: YouTube's dark and light defaults, the
  mapping onto `egui::Visuals`, and the fastframe-theme catalogue's
  desktop integration (shared presets, Omarchy on Linux, followed live).
  `Settings::palette` resolves the choice through the caches; a change is
  revealed by `fastframe_theme::Transition`. The Omarchy hook
  (`contrib/omarchy/youtfast-theme`) calls `youtfast reload-themes`; the
  palette is also followed through filesystem notifications.
- `src/desktop/instance.rs` is one running copy per user (fastframe-instance): a
  second launch hands `show`, `open <text>` or `reload-themes` to the
  window already open and exits; the handler only parses and queues, `App`
  drains the queue.
- `src/desktop/now_playing.rs` is the desktop's media controls
  (fastframe-now-playing; MPRIS on Linux): it maps their commands onto
  `Action`s and reports what mpv plays; the units differ (`SeekBy` is
  milliseconds, `SetVolume` is 0..1).
- `src/desktop/updater.rs` checks GitHub Releases for newer stable versions,
  classifies them by SemVer (`UpdateKind::{Patch, Minor, Major}`), verifies
  release archives against `checksums.txt` (SHA-256), and installs them
  atomically. Settings → About exposes system update notifications, opt-in
  automatic updates for minor/patch versions with persisted changelog notes,
  and manual 1-click installation for major versions (`should_auto_update`
  never auto-installs `UpdateKind::Major`).

## Roadmap

See `ROADMAP.md` for pending work and `CHANGELOG.md` for implemented features.
RustyPipe search, i18n, persistence and the offline demo are in the tree.
0.1.0, 0.2.0 and 0.3.0 target Linux x86_64, Windows x86_64 and macOS Apple
Silicon; candidate packaging is not completed interactive platform qualification.

## YouTube response formats

YouTube changes InnerTube's parameters and responses without notice. A change
either gets requests refused, or makes rustypipe skip items silently (lists
deserialize through `MapResult`/`VecSkipError`), so a tab comes back empty or
a field takes another's value without any error.

- Before changing anything that builds requests to YouTube or reads its
  responses (`vendor/rustypipe`, `src/youtube/innertube*`, `src/backend.rs`
  loads), and whenever a listing comes back empty, wrong or refused, read
  [docs/youtube-api-format.md](docs/youtube-api-format.md). Check the known
  changes (`YT-n`) first: a new symptom is often an old one in another place.
- Whenever a bug turns out to come from a YouTube format change, in the same
  PR as the fix:
  1. add a `YT-n` entry to `docs/youtube-api-format.md` (symptom, old
     format, new format, fix, tests);
  2. add regression tests named `ytn_…` to `tests/youtube_api.rs` that pass
     on the new format and fail on the old one. Prove the second part: put
     the old behaviour back for a moment and watch the test fail.
- `tests/youtube_api.rs` reaches the vendored code through
  `rustypipe::client::regression`; add a hook there when a test needs a code
  path it does not reach yet. Keep the JSON trimmed and synthetic: no raw
  responses, real tokens of accounts, or personal data.
- Unit tests inside `vendor/rustypipe` do not run in CI: the crate is not a
  workspace member and its fixtures are not vendored. A guarantee that must
  hold belongs in `tests/youtube_api.rs`.
- Confirm a fix live, with a request to YouTube outside the app, before
  calling it done: recorded fixtures keep passing after YouTube changes.

## Definition of done

- Add focused tests for changed behaviour (see `src/youtube/ytdlp.rs` and
  `src/util.rs`).
- A bug caused by a YouTube format change also gets its `YT-n` entry and
  regression tests ([YouTube response formats](#youtube-response-formats)).
- Run the checks before finishing:

  ```sh
  cargo fmt --all --check
  cargo clippy --locked --all-targets -- -D warnings
  cargo test --locked --all-targets
  ```

- Report platform coverage honestly: say what was run and what was only
  compiled. Windows/macOS candidates bundle libmpv but still need interactive
  QA and matching native-source review. Windows' Credential Manager caps a
  secret at 2560 bytes, which a YouTube session can exceed.

## Disk use

Builds go through [mbx](https://mr-boxington.jdx.dev) for mise users, as in
the sibling apps (`mise trust` once per checkout). Never put build output or
large scratch files in `/tmp`.

## Reliability regression rules

- A backend session owns a fresh RustyPipe client. Authentication changes and
  account writes are ordered; events carry a session epoch. Authentication
  failure invalidates private events from the entire pending batch, including
  events preceding the failure. Device-local imports and thumbnails remain
  independent of the account. Account changes also dismiss open menus.
- `Backend::send` publishes library/settings snapshots directly to their watch
  channels. Writers and `save_final` share a `SaveGate`; the final write waits
  for earlier writes and seals the gate against later old snapshots. Never
  bypass that gate for exit saves.
- `load_json` returns a result: read errors or failed backups stop startup.
  Invalid JSON and UTF-8 are preserved before defaults become writable.
  Atomic writes reject non-regular targets and synchronize the parent
  directory on Unix. Regression fixtures belong under unique paths inside
  `target/`, never at `target` itself: mbx manages that symbolic link.
- Takeout parsing and merging run off the interface thread. Limits are 64 MiB,
  20,000 unique subscriptions and the latest 1,000 unique history entries.
  A merge must match `App::library_revision` or be rejected without replacing
  local changes or restoring cleared history.
- Thumbnail display and disk persistence share one download. Responses are
  limited to 4 MiB, 4096 pixels per side and four million pixels; the disk cache
  is limited to 2,000 files and 128 MiB. Requests retry with backoff, and late
  failures must not recreate retry entries for images already forgotten.
  A kept file answers without a download turn (`thumbs::cached`); only
  downloads wait, `thumbs::DOWNLOADS` at a time, through one keep-alive
  agent.
- Logs and external errors use controlled messages, also under `--verbose`
  and `RUST_LOG`. `account::CacheFile` sanitizes both reads and writes, retaining
  only public client/deobfuscation data; a cache cannot restore credentials.
  Never weaken these guards to expose raw dependency or credential-store data.

## Contributor CI and releases

- Run `python3 -m unittest discover -s packaging -p 'test_*.py'` and
  `python3 packaging/release.py check` alongside the Rust checks. Validate
  workflows with actionlint when available. Version tags must match Cargo,
  Cargo.lock, AppStream metadata and committed written release notes.
- CI builds Linux, macOS and Windows on hosted GitHub runners. Do not perform
  heavy local release builds when the maintainer asks to use those runners.
  The release workflow creates a draft only after reusable CI and all required
  artifact jobs pass. Final publication requires privacy/source/platform review.
  Native platform tests are not interactive QA.
- Keep Actions pinned to reviewed SHAs and contributor permissions read-only.
  Never execute contributor code through `pull_request_target` with secrets.
- Never commit raw API reports, cookies, logs or personal data. The
  repository holds documents about the application only: diagnoses, reviews
  and notes of local work stay out of it.
- `main` is protected: `release-gate` required, linear history, no force
  pushes or deletion; the maintainer may push directly. Actions are limited to
  GitHub-owned, `msys2/setup-msys2` and `Swatinem/rust-cache`, SHA-pinned.
- GPL-3.0-only is the release license, with original dependency notices retained.
  Ship corresponding source, including locked Rust dependencies and native
  Flatpak sources, alongside binary assets. A Flatpak bundle is not a Flathub
  listing; public store submission and native installers remain separate work.
