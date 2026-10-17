# Roadmap

This file tracks work that is not shipped. It is not a release commitment.
[CHANGELOG.md](CHANGELOG.md) records implemented changes; frozen release notes
describe exactly what each version includes and excludes.

## Required before publishing 0.1.0

- Complete the [public-release checklist](docs/public-release-checklist.md):
  privacy/history review, Actions logs/artifacts, repository settings and source
  redistribution review. A raw report in old Git history is a known blocker.
- Run the hosted Release dry run on the intended commit and inspect every
  platform artifact. Local preparation is not a successful hosted build.
- Qualify Linux native, Flatpak, Windows x86_64 and macOS Apple Silicon using
  clean profiles and the [platform checklist](docs/distribution.md).
- For Windows/macOS bundles, collect the corresponding native dependency
  sources and notices matching the exact binaries. Packaging inventories alone
  do not satisfy this gate.
- Replace the pending changelog date, align AppStream metadata, freeze notes
  and approve the final draft only after the gates pass.

## Distribution follow-ups

| Channel | Work remaining | Completion evidence |
|---|---|---|
| Arch AUR `youtfast` | Generate recipe from final source asset, clean-chroot build, maintainer/name review, submit to AUR | Public AUR entry and successful clean install with `yay` |
| Flathub | Source-build manifest instead of the prebuilt application module; metadata/screenshots, lint and submission review | Accepted listing, install and update through the remote |
| Signed pacman repository | Signing-key ownership, package builds, repository database, hosting and update policy | Verified `pacman -S` install/update from configured repository |
| Windows | Publisher signing, installer and update channel | Clean-machine install/update/uninstall and signature verification |
| macOS | Developer ID, hardened runtime/notarization, installer and Intel/universal support | Clean-machine Gatekeeper/playback tests on each architecture |
| Linux ARM64 | Native and Flatpak builds plus GPU validation | Published architecture-specific packages and tested playback |

## Product follow-ups

- Live chat and downloads need their own design and resource/privacy limits.
- Cookies.txt accounts have no signed-in playback retry; any future session
  handoff to extraction tools must preserve the no-session-file boundary.
- Improve upstream extraction compatibility as YouTube changes.
- Extend native platform accessibility and interactive coverage.

Promote an item into a release only after implementation and validation; do not
copy this list into `Unreleased` as if it were finished work.
