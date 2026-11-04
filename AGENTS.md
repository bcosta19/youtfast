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
  It lives only in the platform credential store (`crates/youtfast-storage/src/account.rs`): never
  in a file, settings, the library, rustypipe's cache, a log line or an
  error message. Credential-store errors are mapped to fixed messages,
  because their text can carry platform data.
- Do not read the user's browser cookies, library or history to debug.
  Hand the user the command and let them report back.
- Do not broaden a task into adjacent features or a general refactor.

## Architecture

[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) is the map: how a frame runs,
what each crate holds and the invariants that hold across them. Read it, and
the section of each crate you change, before changing code, and keep it
current in the same PR.

The repository is a Cargo workspace: the root `Cargo.toml` holds the shared
package fields, dependency versions, profiles and patches, and each crate in
`crates/` inherits them (`.workspace = true`). From the bottom up:

| Crate | Holds |
|---|---|
| `youtfast-i18n` | the interface languages, their catalogs compiled from `assets/i18n` by its build script |
| `youtfast-mpv` | the player (libmpv), the only crate with FFI |
| `youtfast-appearance` | palette, icons, fonts, the desktop's theme and font settings |
| `youtfast-core` | the shared types (`model`), formatting helpers (`util`), YouTube link parsing (`links`) and the fixed messages external errors become (`privacy`); no IO |
| `youtfast-storage` | what stays on this computer: library, settings, sessions (`account`), Takeout, file paths |
| `youtfast-desktop` | single instance, local logs and their redactor, release updater |
| `youtfast-logic` | pure decisions over video lists: new-upload alerts, history, local recommendations, video tabs |
| `youtfast-youtube` | the network: InnerTube (`innertube`), yt-dlp (`ytdlp`), SponsorBlock, captions and storyboards (`extras`), the thumbnails' cache; the response-format tests (`tests/youtube_api.rs`) |
| `youtfast-cast` | casting to TVs: Google Cast, DIAL (`dial`) and YouTube's Lounge API (`lounge`) |
| `youtfast-backend` | the runtime thread; `protocol` is its `Command`s and `Event`s |
| `youtfast-app` | the state, the actions the views push (`Action`), the remote images (`images`), the desktop's media controls (`now_playing`) |
| `youtfast-ui` | the views; `frame` draws one frame |
| `youtfast` | the binary: the command line, the window's startup, the offline demo |

- A crate depends only on crates below it in ARCHITECTURE.md's rows. A new
  module goes in the crate that matches its job; if it needs a crate above
  it, it belongs higher up. The application never calls the views
  (`App::new` is handed `youtfast_ui::frame`).
- Only `youtfast-mpv` has `unsafe` code: the workspace denies `unsafe_code`
  everywhere else (the frame-cost benchmark's test-only allocator excepted).
- A dependency is declared once, in the root `[workspace.dependencies]` with
  the comment saying why; a crate lists it as `name.workspace = true`.
- Every crate's name starts with `youtfast`: the log redactor and the log
  filters recognise the application by that prefix of its log targets.

## Roadmap

See `ROADMAP.md` for pending work and `CHANGELOG.md` for implemented features.
RustyPipe search, i18n, persistence and the offline demo are in the tree.
0.x releases target Linux x86_64, Windows x86_64 and macOS Apple
Silicon; candidate packaging is not completed interactive platform qualification.

## YouTube response formats

YouTube changes InnerTube's parameters and responses without notice. A change
either gets requests refused, or makes rustypipe skip items silently (lists
deserialize through `MapResult`/`VecSkipError`), so a tab comes back empty or
a field takes another's value without any error.

- Before changing anything that builds requests to YouTube or reads its
  responses (the rustypipe fork, `crates/youtfast-youtube/src/innertube*`, `crates/youtfast-backend/src/lib.rs`
  loads), and whenever a listing comes back empty, wrong or refused, read
  [docs/youtube-api-format.md](docs/youtube-api-format.md). Check the known
  changes (`YT-n`) first: a new symptom is often an old one in another place.
- Whenever a bug turns out to come from a YouTube format change, in the same
  PR as the fix:
  1. add a `YT-n` entry to `docs/youtube-api-format.md` (symptom, old
     format, new format, fix, tests);
  2. add regression tests named `ytn_…` to `crates/youtfast-youtube/tests/youtube_api.rs` that pass
     on the new format and fail on the old one. Prove the second part: put
     the old behaviour back for a moment and watch the test fail.
- `crates/youtfast-youtube/tests/youtube_api.rs` reaches the fork's code through
  `rustypipe::client::regression`; add a hook there when a test needs a code
  path it does not reach yet. Keep the JSON trimmed and synthetic: no raw
  responses, real tokens of accounts, or personal data.
- A fix inside rustypipe is a commit on the fork's `youtfast` branch, then
  `rev` in `Cargo.toml` (and Cargo.lock) moved to it in the same PR as its
  `YT-n` entry and tests. rustypipe's own unit tests do not run in this
  repository's CI: a guarantee that must hold belongs in
  `crates/youtfast-youtube/tests/youtube_api.rs`.
- Confirm a fix live, with a request to YouTube outside the app, before
  calling it done: recorded fixtures keep passing after YouTube changes.

## Definition of done

- Add focused tests for changed behaviour (see `crates/youtfast-youtube/src/ytdlp.rs` and
  `crates/youtfast-core/src/util.rs`).
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
