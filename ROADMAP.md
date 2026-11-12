# Roadmap

This file tracks work that is not shipped. It is not a release commitment.
[CHANGELOG.md](CHANGELOG.md) records implemented changes; frozen release notes
describe exactly what each version includes and excludes.

## Open release gates

0.1.0 (pre-release) through 0.6.0, plus 0.6.2, are published; the 0.6.1
tag/draft is preserved and 0.7.0 is being prepared as the next candidate. Before
publishing 0.7.0 or promoting a release to latest:

- Validate the exact candidate with the hosted Release dry run, inspect every
  package and its checksums, and review the matching Windows and macOS native
  source archives and notices. The previous release's artifacts do not qualify
  the new candidate.
- Run clean-machine QA from the [platform checklist](docs/distribution.md) on
  Windows ([#5](https://github.com/bcosta19/youtfast/issues/5)) and macOS
  ([#6](https://github.com/bcosta19/youtfast/issues/6)). Local demo rendering
  and `--version` do not prove playback or keyring behaviour.

## Distribution follow-ups

| Channel | Work remaining | Completion evidence |
|---|---|---|
| Arch AUR `youtfast` | Deferred while AUR new-account registrations remain closed (`HTTP 503`); Arch users install `.pkg.tar.zst` directly via `pacman -U` or `install.sh` | Re-evaluate if AUR registration opens |
| Flathub | Not pursued directly; Linux users install the standalone `.flatpak` bundle directly or via `install.sh` | Optional future decision |
| Signed pacman / Flatpak repository | Signing-key ownership, repository hosting and update policy | Verified repository install/update |
| Windows | Publisher signing (Authenticode) and `microsoft/winget-pkgs` PR submission from the generated multi-file manifests (`channels.tar.gz`) | Clean-machine install/update/uninstall, Winget validation, and signature verification |
| macOS | Developer ID, hardened runtime/notarization, DMG installer, `bcosta19/homebrew-tap` repository setup (`channels.tar.gz`), and Intel/universal support | Clean-machine Gatekeeper/playback tests and `brew install --cask` test |
| Linux ARM64 | Native and Flatpak builds plus GPU validation | Published architecture-specific packages and tested playback |

## Product follow-ups

- Qualify the optional `catt` Google Cast adapter on physical Chromecast and
  Google TV devices, and native Windows/macOS. Check resume, live playback,
  disconnect, network loss and the TV's reported video identity. Bundling the
  helper, Flatpak qualification, live remote-status synchronization and broader
  DIAL TV model qualification remain separate work.
- Live chat and downloads need their own design and resource/privacy limits.
- Cookies.txt accounts have no signed-in playback retry; any future session
  handoff to extraction tools must preserve the no-session-file boundary.
- Improve upstream extraction compatibility as YouTube changes.
- Extend native platform accessibility and interactive coverage.

Promote an item into a release only after implementation and validation; do not
copy this list into `Unreleased` as if it were finished work.
