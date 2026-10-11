# Releasing YouTFast

## Version and gates

Use stable SemVer tags `vMAJOR.MINOR.PATCH`. `Cargo.toml`, the YouTFast entry in
`Cargo.lock`, desktop AppStream metadata, and the committed release notes must
agree. Version 0.1.0 is the first early-stage release, not a promise of API or
file-format stability across all future 0.x releases.

1. Prepare the version, release date and notes in `packaging/release-notes/vX.Y.Z.md`.
2. Run packaging tests and `python3 packaging/release.py check` plus the Rust
   checks from CONTRIBUTING.md. Do not run real-account flows to debug releases.
3. Commit on `main`, preserving a linear history, then push. All required CI
   jobs on that exact commit must pass before tagging. Build work happens on
   hosted GitHub runners, not a maintainer's laptop.
4. Create an annotated version tag on that tested commit and push the tag.
   Never move or overwrite a published tag.
5. The Release workflow reuses CI, checks tag/version agreement and ancestry
   from `main`, builds Linux x86_64, captures a signed-out preview, archives
   corresponding source including locked Rust dependencies, and builds Flatpak.
   Publication waits for every required job. No missing artifact is silently
   skipped and written notes are mandatory.
6. Verify the published version, notes, assets and `checksums.txt`. Download
   through `gh release download` while private and run `sha256sum --check`.
   Check the Linux `--version` smoke test and Flatpak sandbox smoke test in logs.

The publish job alone has `contents: write`. Contributor checks use read-only
permissions and do not receive maintainer secrets. Build artifacts are produced
from the tagged commit; source trees and releases do not include git history,
raw RustyPipe reports, cookies, settings or user-owned libraries.

## Artifacts

- `youtfast-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`: Ubuntu 24.04 ABI baseline,
  glibc 2.39+, dynamic system libmpv and documented desktop dependencies.
- `youtfast-vX.Y.Z-source-with-dependencies.tar.gz`: repository sources and
  vendored locked Rust dependencies, notices and offline Cargo configuration.
- `youtfast-vX.Y.Z-x86_64.flatpak`: Freedesktop 25.08 runtime, native components
  and yt-dlp packaged in the sandbox.
- `youtfast-vX.Y.Z-flatpak-sources.flatpak`: exported native component sources.
- `youtfast-vX.Y.Z-linux.png`: fresh, signed-out profile preview from CI.
- `checksums.txt`: SHA-256 digests for every asset above.

The Flatpak does not have home/host filesystem access, browser-profile access,
`flatpak-spawn --host`, or permission to arbitrary D-Bus services. File selection
uses the portal. Secret Service and MPRIS access are limited to the names needed
by the app. Theme files outside the sandbox are not implicitly readable.

To install a downloaded bundle after installing Flatpak and adding the Flathub
runtime remote through your distribution's supported instructions:

```sh
flatpak install --user ./youtfast-v0.1.0-x86_64.flatpak
flatpak run io.github.bcosta19.YouTFast
```

A release bundle is not a Flathub listing and does not provide an update remote.
A public Flathub submission requires a public source repository, offline source
build manifest, metadata/screenshots and review. This repository remains private
by the maintainer's decision; no public listing or publication is claimed.

## Other platforms and manual checks

Windows and macOS compilation/headless checks are groundwork, not portable
releases. Before distributing them, bundle and audit libmpv/runtime dependencies,
check clean-machine startup, interactive OpenGL playback and the native keyring,
and define Windows signing and macOS signing/notarization as applicable.

CI does not certify real YouTube availability, Google account safety, playback
on every GPU, or native credential-manager limits. Run manual tests with a
separate disposable account only when deliberately requested.

## Before changing visibility

The repository once tracked a raw RustyPipe error report. It is excluded from
current source packages and no longer tracked, but remains in earlier commits.
Do not make the repository public before a separate history/privacy review and
explicit approval of any history cleanup. Never rewrite published history as
part of routine release preparation.
