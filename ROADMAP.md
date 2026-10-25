# Roadmap

This file tracks work that is not shipped. It is not a release commitment.
[CHANGELOG.md](CHANGELOG.md) records implemented changes; frozen release notes
describe exactly what each version includes and excludes.

## Open release gates

0.1.0 (pre-release) and 0.2.0 are published; 0.3.0 is being prepared as a
candidate. Before publishing 0.3.0 or promoting a release to latest:

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
| Arch AUR `youtfast` | Generate recipe from final source asset, clean-chroot build, maintainer/name review, submit to AUR | Public AUR entry and successful clean install with `yay` |
| Flathub | Submit generated source-build manifest (`channels.tar.gz`), metadata/screenshots, lint and submission review | Accepted listing, install and update through the remote |
| Signed pacman repository | Signing-key ownership, package builds, repository database, hosting and update policy | Verified `pacman -S` install/update from configured repository |
| Windows | Publisher signing, MSI/MSIX installer and Winget repository submission (`channels.tar.gz`) | Clean-machine install/update/uninstall and signature verification |
| macOS | Developer ID, hardened runtime/notarization, DMG installer, Homebrew tap publication (`channels.tar.gz`) and Intel/universal support | Clean-machine Gatekeeper/playback tests on each architecture |
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
