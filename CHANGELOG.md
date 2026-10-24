# Changelog

Notable user-facing changes are recorded here, following
[Keep a Changelog](https://keepachangelog.com/en/2.0.0/) and
[Semantic Versioning](https://semver.org/). A prepared entry is not proof that a
release has been published; the [GitHub releases](https://github.com/bcosta19/youtfast/releases)
are the distribution record.

## [Unreleased]

### Added

- About section and built-in updater in Settings → About (`src/desktop/updater.rs`):
  shows the installed version and bundled changelog, checks GitHub Releases for new
  versions with optional system desktop notifications, supports automatic SHA-256-verified
  updates for minor (`0.x.0`) and patch (`0.0.x`) releases with changelog notes of what
  changed, and requires manual confirmation via the in-app Update button for major
  (`x.0.0`) releases.
- Distribution channel manifest generator (`packaging/channels.py`): produces a
  Flathub source-build manifest (`io.github.bcosta19.YouTFast.json`), a Windows
  Winget singleton manifest (`bcosta19.YouTFast.yaml`), and a macOS Homebrew
  Cask (`youtfast.rb`) with exact release asset SHA-256 digests.
- Configurable video buffer in Settings → Playback (`PlaybackBuffer`): defaults
  to mpv's standard 150 MiB ahead / 50 MiB behind for smoother playback on
  unstable connections and faster seeking, with a compact 50 MiB / 20 MiB option
  for lower RAM and data usage.
- Audio-only mode: stream and decode only the audio track (`ba/b` and `vid=no`)
  for the current video, reducing bandwidth and CPU/GPU usage during background
  listening. Accessible via the player Quality menu or `Shift+A`. Resets to
  standard video when opening another video or restarting the app.
- History sync settings: keep account and device history separate, merge them
  in YouTFast with local resume positions, or also send started playback to the
  signed-in account's YouTube history. Merged history has All, Account and This
  device filters; removing an entry only changes this device.

### Fixed

- The miniplayer is now a compact frameless video box with rounded corners,
  without an outer window title bar or bottom metadata bar.
- `↑` and `↓` outside the player now scroll the active page vertically instead
  of stepping sideways through grid selections; `j` and `k` keep item cursor
  navigation.
- Page scrolling now uses `fastframe-scroll`: mouse wheel notches scroll 120
  points instead of egui's 40-point default, and Linux touchpad gestures are
  scaled with momentum glide and axis locking.
- History row controls stay aligned; selected text, helper text and counts
  have readable contrast, and letter avatars remain visible on selected rows.
- Channel banners crop without stretching, back/forward use matching arrows,
  and watched thumbnails show only the played portion of their progress bar.
- The sidebar toggle (`b`, or the top bar's button) overrides the automatic
  rail on watch and narrow layouts on its first press. Its choice then rules
  every page for that run; the collapsed preference continues to be saved.
- Scrolling a channel page works from anywhere on the view (including over
  the banner, header, tab bar, and chips), scrolling the whole page rather
  than restricting scroll interaction to the video grid box.

## [0.2.0] - 2026-10-22

### Added

- Spanish, French, German and Italian interface languages, chosen in
  Settings alongside English and Brazilian Portuguese.
- Optional miniplayer while scrolling to the watch page's comments, enabled
  in Playback settings. Returning from it reveals the inline video again.
- Opening or pasting a channel link (`/@handle`, `/channel/UC…`, `/c/name`,
  `/user/name`) opens the channel page instead of searching for the URL.

### Changed

- The window opens without waiting for the installed fonts that draw the
  scripts Inter lacks; they are found meanwhile and put in once ready. After a
  while without opening the app, its first frame came 1.5 s sooner on the
  benchmark machine ([docs/benchmarks.md](docs/benchmarks.md#startup)).
- Thumbnails already on disk show at once instead of waiting behind the
  page's downloads, and downloads keep their connections open.
- The player no longer loads mpv's own interface scripts (statistics,
  console, menus, command palette), which YouTFast never shows: six fewer
  threads and a little less memory from the start.

### Fixed

- Switching to About while a channel link is loading keeps the channel's
  resolved identity, header and Subscribe button.
- Scrolling a playing video keeps its aspect ratio and clips it below the
  toolbar instead of drawing over the search field.
- A network failure while fetching YouTube's visitor data, which the first
  requests of a session do, no longer closes the app: the page shows the
  error and can be retried.
- A listing whose loading hangs now shows, after a minute, an error that can
  be retried instead of loading forever. The subscriptions feed keeps no time
  limit, since many channels take long.
- Sorting a channel's videos, Shorts or live streams by Latest, Popular or
  Oldest works again: YouTube had started refusing the request.
- A channel's Shorts tab lists its Shorts again instead of showing none.
- A channel's Playlists tab lists its playlists and podcasts again instead of
  showing none.
- A channel's header shows its real subscriber and video counts: it showed
  the video count as subscribers, and "2.2K videos" as 22.

## [0.1.0] - 2026-10-17

First release, published as a pre-release for Linux x86_64, Windows x86_64 and
macOS Apple Silicon. Interactive Windows/macOS qualification is still pending.

### Added

- Native libmpv playback, hardware decoding where available, full screen,
  theater mode, movable mini player, keyboard and desktop media controls.
- Search for videos, channels and playlists; filters, channel tabs, paginated
  feeds/playlists, queue editing, Shorts and browser-style navigation.
- Captions, chapters, comments and image/GIF attachments; optional transcripts,
  timeline previews and SponsorBlock integration.
- Device subscriptions, Watch later, history/resume, bounded Takeout imports
  and new-upload notifications.
- Optional named accounts through the system credential store; account ratings
  and playlist edits, with rollback when YouTube refuses a write.
- English and Brazilian Portuguese, immediate language switching, light/dark
  themes, shared palettes and Omarchy integration for native Linux installs.
- Offline demo fixtures and bounded, redacted local diagnostic logs.
- Release documentation, source archives with locked Rust dependencies,
  checksums, Linux/Flatpak packaging, Windows/macOS candidate/source bundling
  and a clean Arch package build with an AUR recipe.

### Fixed

- Account switching/expiration rejects stale private responses and dismisses
  account menus; writes remain attached to their originating session.
- Corrupt or unreadable local data is preserved; exit saves cannot be replaced
  by older snapshots, and imports cannot overwrite newer local edits.
- Bounded shared thumbnail downloads, retry/backoff and attachment decoding
  reduce duplicate requests and uncontrolled memory use.
- Playback failures report controlled error categories; external diagnostics
  and dependency logs cannot expose account/session data.
- Clipboard actions, scrolling/selection and newer YouTube playlist layouts.

### Not included in 0.1.0

- Flathub/AUR publication, a signed pacman repository, and automatic app updates.
- Linux ARM64, macOS Intel/universal binaries, AppImage, DEB/RPM, Windows MSI/MSIX
  and a macOS DMG installer.
- Windows publisher signing and macOS Developer ID signing/notarization.
- Live chat, downloads and an official Google OAuth sign-in flow.
- Account/browser access from inside Flatpak beyond its explicit portal/keyring
  permissions. Browser profiles remain outside the sandbox.

These exclusions are scope decisions, not completed features promised for a
specific next version. See [ROADMAP.md](ROADMAP.md) for follow-up gates.

[Unreleased]: https://github.com/bcosta19/youtfast/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/bcosta19/youtfast/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/bcosta19/youtfast/releases/tag/v0.1.0
