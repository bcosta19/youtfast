# Public opening and 0.1.0 checklist

Review date: 2026-10-17. Checked items refer only to the evidence recorded in
[release-review-2026-10-17.md](release-review-2026-10-17.md). Public source opening
and binary release publication have separate gates. The source repository
was opened on 2026-10-17 as a new repository with the cleaned history.

## Privacy and history

- [x] Locate the known historical raw report without opening its contents.
  It is absent from the current tree and present in the original history.
- [x] Verify a private Git bundle backup and prepare a cleaned isolated mirror
  with maintainer approval. Preserve 79 commits, authors, dates, messages and
  the current `main` tree; remove the sensitive objects and backup refs only
  from the isolated mirror. Original checkout/refs stay unchanged.
- [x] Run a redacted credential-pattern inventory of reachable local history.
  No findings in scanned blobs; known report objects were excluded, not read.
  This is not a certification that every kind of personal data has been found.
- [x] Scan 35 historical Actions log archives in memory, 6 issue/PR bodies and
  6 comments for the same patterns. No matching values or raw logs were saved.
- [x] Avoid GitHub-retained views instead of requesting their removal. All six
  old PR refs reached the report commit and cannot be removed by force-push,
  so the original repository was renamed `youtfast-private-archive`, archived
  and kept private, and a new `bcosta19/youtfast` received only the cleaned
  `main` (77 commits). Old Actions logs, caches and PRs stay in the archive.
- [x] Replace the personal author e-mail with the GitHub noreply address in the
  cleaned history. Names, dates, messages and trees are byte-identical.
- [x] Delete the 8 old workflow artifacts (about 586 MiB) from the archive.
- [x] Verify anonymously that the old report commit and the archive return 404
  and that the public repository only exposes `main` and Dependabot refs.
- [ ] If real credentials were exposed, revoke them through the owning service
  without sharing their values. The report was not opened to determine this;
  it stayed private throughout.

GitHub exposes Actions history/logs when a repository becomes public. See
[visibility consequences](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility)
and [sensitive-data removal](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository).

## Public source repository

- [x] Confirm the original repository settings (no secrets, variables,
  environments, webhooks, deploy keys, stars, forks or issues to migrate).
- [x] Review templates, CODEOWNERS, SHA-pinned Actions and the project/dependency
  license notices. Current application distribution remains GPL-3.0-only.
- [x] Improve README presentation and verify sibling-project URLs. Validate
  local rendering at 1280 and 390 pixels with synthetic screenshot data.
- [x] Commit the reviewed source tree, leaving local analysis documents out.
- [x] Configure description, topics, read-only Actions token, allowed Actions
  with required SHA pinning, private vulnerability reporting, Dependabot
  alerts/security updates, secret scanning and push protection.
- [x] Open the repository after the privacy/history gates. README and the
  screenshot load anonymously. `main` requires `release-gate`, resolved
  conversations and linear history, and blocks force pushes and deletion.

Opening the source does not mean 0.1.0 binaries are released or tested.

## Binary release 0.1.0

- [x] Local rustfmt, Clippy, tests and strict rustdoc pass, also with demo/all
  features: 202 passed, 2 deliberate network tests ignored per test run.
- [x] Packaging/privacy/source regression tests, candidate version checks,
  desktop/AppStream metadata, local links and YAML/shell syntax pass. See the
  evidence record for the exact count and unavailable validators.
- [x] Prepare source-identity collectors and require Windows/macOS source
  archives at assembly. Verify one real upstream mpv/MSYS2 source package.
- [x] Build, namcap-check and install the Arch package in a clean official
  container on hosted runners, exporting `.pkg.tar.zst`, PKGBUILD and `.SRCINFO`.
- [x] Required hosted CI/package jobs pass on the tagged commit (`8ffe761`,
  Release run 39055941759), and every asset was downloaded and checked against
  `checksums.txt`. An empty Arch debug split package was removed from the
  draft and from the checksum list; later recipes disable it.
- [ ] Review the complete native source closures/notices from those actual
  Windows/macOS runs. The published source archives (1.25 GB Windows, 486 MB
  macOS) were collected by the runners but have not been reviewed by a person.
- [ ] Perform/record clean-machine platform QA from [distribution.md](distribution.md).
  Local demo rendering and `--version` do not prove playback/keyring behaviour.
  Tracked in [#5](https://github.com/bcosta19/youtfast/issues/5) (Windows) and
  [#6](https://github.com/bcosta19/youtfast/issues/6) (macOS); promote the
  pre-release to latest afterwards.
- [x] Freeze the publication date (2026-10-17), align changelog/AppStream/notes
  and create the annotated `v0.1.0` tag. Before any release existed the tag
  was moved to each packaging fix; it is immutable from publication on.
- [x] Inspect the draft, notes and every asset, publish as a pre-release
  (2026-10-17) and verify anonymous downloads against the published checksums.

## Separate distribution channels

- [ ] Submit the tested recipe to AUR after maintainer/account/name review and
  public source downloads exist. Advertise `yay -S youtfast` only afterward.
- [ ] Submit a reviewed source-build Flatpak manifest to Flathub and complete
  its review before claiming a listing/update remote.

These channels are explicitly excluded from 0.1.0's initial GitHub publication;
they are tracked work, not claims of availability or hidden release blockers.
