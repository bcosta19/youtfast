# Public opening and 0.1.0 checklist

Review date: 2026-10-17. Checked items refer only to the evidence recorded in
[release-review-2026-10-17.md](release-review-2026-10-17.md). Public source opening
and binary release publication have separate gates. No remote history,
repository setting or visibility was changed during this preparation.

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
- [ ] Approve/apply the remote rewrite using freshly checked force-with-lease
  guards. Review all current remote refs; do not push a stale mirror blindly.
- [ ] Resolve GitHub cached commits, PR references and any other retained views
  with GitHub Support as needed. A clean local mirror or force-push alone does
  not prove the old report is inaccessible on GitHub.
- [ ] Review the contents of 8 old workflow artifacts, or approve their removal
  with the obsolete runs. Their metadata was inventoried; contents were not
  certified. Caches were inventoried but not deleted.
- [ ] If real credentials were exposed, revoke them through the owning service
  without sharing their values. The report was not opened to determine this.

GitHub exposes Actions history/logs when a repository becomes public. See
[visibility consequences](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility)
and [sensitive-data removal](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository).

## Public source repository

- [x] Confirm `bcosta19/youtfast`, private visibility, `main`, Issues enabled,
  default Actions token read-only and PR approval by Actions disabled.
- [x] Review templates, CODEOWNERS, SHA-pinned Actions and the project/dependency
  license notices. Current application distribution remains GPL-3.0-only.
- [x] Improve README presentation and verify sibling-project URLs. Validate
  local rendering at 1280 and 390 pixels with synthetic screenshot data.
- [ ] Commit/review the exact intended source tree, preserving unrelated local
  documents and user changes. The prepared manifest/snapshot is not a commit.
- [ ] Approve/configure description/topics, Actions restrictions and private
  security reporting. The current private plan returned 403 for branch
  protection and 404 for private vulnerability-reporting status; these features
  must be checked/configured after opening or changing the plan.
- [ ] Approve the visibility change only after privacy/history gates pass.
  Verify README/assets anonymously, then configure public branch protection and
  the required `release-gate` check. Workflows alone do not enforce protection.

Opening reviewed source can precede binary qualification. This lets subsequent
standard hosted CI run under public-repository compute rules; it does not mean
0.1.0 binaries are released or tested. A private hosted dry run is another
option, consuming the owner's allowance.

## Binary release 0.1.0

- [x] Local rustfmt, Clippy, tests and strict rustdoc pass, also with demo/all
  features: 202 passed, 2 deliberate network tests ignored per test run.
- [x] Packaging/privacy/source regression tests, candidate version checks,
  desktop/AppStream metadata, local links and YAML/shell syntax pass. See the
  evidence record for the exact count and unavailable validators.
- [x] Prepare source-identity collectors and require Windows/macOS source
  archives at assembly. Verify one real upstream mpv/MSYS2 source package.
- [x] Prepare Arch build/install/namcap verification in a clean official
  container, export `.pkg.tar.zst`, PKGBUILD and `.SRCINFO`. Not executed yet.
- [ ] Required hosted CI/package jobs pass on the exact reviewed commit and all
  assets/checksums are downloaded and checked. New jobs are not certified by
  local unit tests or workflow syntax validation.
- [ ] Review the complete native source closures/notices from those actual
  Windows/macOS runs. Inventories and one upstream probe are not that closure.
- [ ] Perform/record clean-machine platform QA from [distribution.md](distribution.md).
  Local demo rendering and `--version` do not prove playback/keyring behaviour.
- [ ] Freeze the publication date, align changelog/AppStream/notes, update
  candidate/readiness notices and create the immutable annotated release tag.
- [ ] Inspect the tag build's final draft, notes/exclusions and every asset;
  publish only after approval, then verify anonymous downloads/checksums.

## Separate distribution channels

- [ ] Submit the tested recipe to AUR after maintainer/account/name review and
  public source downloads exist. Advertise `yay -S youtfast` only afterward.
- [ ] Submit a reviewed source-build Flatpak manifest to Flathub and complete
  its review before claiming a listing/update remote.

These channels are explicitly excluded from 0.1.0's initial GitHub publication;
they are tracked work, not claims of availability or hidden release blockers.
