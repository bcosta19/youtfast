# Changelog

Notable user-facing changes are recorded here, following
[Keep a Changelog](https://keepachangelog.com/en/2.0.0/) and
[Semantic Versioning](https://semver.org/). A prepared entry is not proof that a
release has been published; the [GitHub releases](https://github.com/bcosta19/youtfast/releases)
are the distribution record.

## [Unreleased]

### Fixed

- A network failure while fetching YouTube's visitor data, which the first
  requests of a session do, no longer closes the app: the page shows the
  error and can be retried.

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

[Unreleased]: https://github.com/bcosta19/youtfast/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/bcosta19/youtfast/releases/tag/v0.1.0
