# YouTFast agent guide

YouTFast is a small native YouTube client: Rust, egui, and libmpv for
playback. `../spotifast` and `../zapfast` are its references. When in doubt
about a convention (structure, naming, comments, release process), do what
they do; their `AGENTS.md` files apply here unless this file says otherwise.

## Product boundaries

- Keep it a small native client. No browser engine, no telemetry, no hosted
  backend, no YouTFast-operated account system.
- Video goes through libmpv. Do not decode video in-process the way ZapFast's
  `src/video.rs` does: that path is CPU-only and tops out far below what
  YouTube serves (VP9/AV1, 4K, 60 fps, HDR).
- YouTube data goes through yt-dlp for now (see `src/youtube.rs`). Do not
  scrape YouTube's HTML or reimplement signature or n-parameter
  deobfuscation here; that belongs to an extractor library.
- The signed-in session (YouTube cookies) is the user's Google session.
  It lives only in the platform credential store (`src/account.rs`): never
  in a file, settings, the library, rustypipe's cache, a log line or an
  error message. Credential-store errors are mapped to fixed messages,
  because their text can carry platform data.
- Do not read the user's browser cookies, library or history to debug.
  Hand the user the command and let them report back.
- Do not broaden a task into adjacent features or a general refactor.

## Architecture

- `src/ui/` draws views from `app::State` and the player's `mpv::Status`, and
  pushes `model::Action`s. `src/app.rs` applies them after the frame. A view
  only changes its own state (search text, the seek bar while dragged).
- `src/backend.rs` is the interface's handle to a tokio runtime on its own
  thread. The two sides talk only through `Command` and `Event`, and every
  event wakes the window. Nothing blocks the interface thread.
- `src/library.rs` is what this computer keeps, with or without an
  account: subscriptions, history (with resume positions) and Watch later,
  in one JSON file (`paths::Dirs::library`). Saves go to the backend
  (`Command::SaveLibrary`) and are atomic (`paths::write_atomic`); an
  unreadable file is moved aside, never overwritten. New fields need
  `#[serde(default)]` so older files keep loading. `on_exit` saves directly.
- `src/takeout.rs` imports Google Takeout's `subscriptions.csv` (columns by
  position: the header is localized) and `watch-history.json`.
- `src/innertube.rs` is everything through rustypipe: the home feed (the
  account's subscription feed plus RSS for channels subscribed on this
  device, merged and deduplicated), a channel's RSS, and the account's
  subscriptions, history, Watch later, liked videos and playlists. rustypipe
  has no write actions and no home recommendations: subscribing and Watch
  later act on the local library.
- `src/account.rs` holds the session: cookies.txt parsing, reading a
  browser's cookies through yt-dlp's `extract_cookies_from_browser` (a short
  Python script whose output is piped, never written; it tries each Linux
  keyring because yt-dlp guesses wrong on Hyprland), and the credential
  store, called through `spawn_blocking` with a timeout. rustypipe's cache
  goes through `account::CacheFile`, which strips `auth_cookie` and
  `oauth_token`; its error reports are off (`no_reporter`) because they hold
  whole responses. A session from a browser is read again from the browser
  when it expires (`backend::restore`).
- Lists the views show are `model::Listing`s filled by `Command::Load`
  requests, one current request per `backend::Slot`; an answer to an older
  one is dropped (`App::generations`). Account-or-device pages switch with
  `model::Source`.
- `src/youtube.rs` runs yt-dlp (`--flat-playlist --dump-single-json`) for
  search and parses its JSON into `model::Video`. Playback hands the watch
  link to mpv, whose ytdl hook asks yt-dlp for the streams
  (`mpv::FORMAT`, bounded by the quality chosen, 1080p on Auto). Updating yt-dlp fixes YouTube
  breakage without a release.
- `src/mpv/ffi.rs` declares the libmpv functions used, by hand (client API
  2.x). `src/mpv/mod.rs` wraps them: `Mpv` owns the player, observes the
  properties the interface shows, and is polled once per frame from
  `App::logic`; its wakeup callback only requests a repaint.
- mpv's network cache (the demuxer cache) is bounded in `Mpv::new`: 50 MiB
  ahead and 20 MiB behind, against mpv's 150 and 50. That keeps the process
  near 200 MB on long videos; seeking back further downloads again.
- `src/mpv/render.rs` draws the picture inside the egui window. libmpv's
  OpenGL render API draws into a framebuffer of ours, sized to the video's
  rectangle in physical pixels; an `egui_glow::CallbackFn` blits it onto
  egui's target. The render context is made lazily inside the paint callback
  (where the GL context is current), and freed in `App::on_exit`, before mpv.
  A dropped surface never frees GL objects: without a current context that
  would be undefined. The wl_display or X11 Display goes to mpv so hardware
  decoding can share frames with OpenGL (VAAPI on Linux).
- The eframe fork exposes `CreationContext::get_proc_address`, which the
  render API needs. That is one more reason to stay on the shared egui and
  winit forks; move their revisions together with the sibling apps.
- Leaving the watch page pauses the video: nothing draws it there, and an
  undrawn render-API video output stalls. Playing again from the desktop's
  media controls comes back to the watch page (`App::return_to_watch`).
- `src/settings.rs` is what survives between runs (quality, volume, the
  theme choice and its palette caches), in one JSON file at
  `paths::Dirs::settings`. Loads and saves mirror `library.rs`; the backend
  writes it (`Command::SaveSettings`), and `on_exit` saves directly.
- `src/icons.rs` lists the Lucide icons fastframe-icons shares with the
  sibling apps; `fastframe_icons::install` serves them to egui.
- `src/theme.rs` is the palette: YouTube's dark and light defaults, the
  mapping onto `egui::Visuals`, and the fastframe-theme catalogue's
  desktop integration (shared presets, Omarchy on Linux, followed live).
  `Settings::palette` resolves the choice through the caches; a change is
  revealed by `fastframe_theme::Transition`. The Omarchy hook
  (`contrib/omarchy/youtfast-theme`) calls `youtfast reload-themes`; the
  palette is also followed through filesystem notifications.
- `src/instance.rs` is one running copy per user (fastframe-instance): a
  second launch hands `show`, `open <text>` or `reload-themes` to the
  window already open and exits; the handler only parses and queues, `App`
  drains the queue.
- `src/now_playing.rs` is the desktop's media controls
  (fastframe-now-playing; MPRIS on Linux): it maps their commands onto
  `Action`s and reports what mpv plays; the units differ (`SeekBy` is
  milliseconds, `SetVolume` is 0..1).

## Roadmap

1. **Prototype (now):** search, watch page, in-window playback, full screen,
   keyboard shortcuts.
2. **Make it an app:** fastframe-i18n, window persistence, the `demo`
   feature with sample data and screenshots.
3. **Library and account (done, first version):** local subscriptions,
   history with resume, Watch later, Takeout import, RSS feed, channel
   pages; the account's feed, subscriptions, history, Watch later, liked
   videos and playlists through rustypipe.
4. **Search through rustypipe** (already in the tree) to drop the
   per-search yt-dlp start-up (about a second), keeping yt-dlp for streams.
   Playing age-restricted and members-only videos with the account needs
   the session handed to yt-dlp without writing it to a file.
5. **More:** comments, captions (mpv `sub-add`), chapters, SponsorBlock,
   mini player, downloads.

## Definition of done

- Add focused tests for changed behaviour (see `src/youtube.rs` and
  `src/util.rs`).
- Run the checks before finishing:

  ```sh
  cargo fmt --all --check
  cargo clippy --locked --all-targets -- -D warnings
  cargo test --locked --all-targets
  ```

- Report platform coverage honestly: say what was run and what was only
  compiled. Only Linux has been considered so far; Windows and macOS need
  libmpv bundled with the app, and Windows' Credential Manager caps a
  secret at 2560 bytes, which a YouTube session can exceed.

## Disk use

Builds go through [mbx](https://mr-boxington.jdx.dev) for mise users, as in
the sibling apps (`mise trust` once per checkout). Never put build output or
large scratch files in `/tmp`.
