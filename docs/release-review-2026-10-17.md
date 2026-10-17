# Release preparation evidence — 2026-10-17

This records verified local/read-only work and remaining gates. It is not a
published release, remote cleanup, legal certification or interactive platform
qualification. Private evidence is kept in the ignored `target/` review folder;
no raw report, logs, credential values or user-owned app data are included here.

## History and GitHub

- Original reachable local history: 79 commits. The redacted scan read 637
  non-report blobs, with no configured credential-pattern findings or oversized
  skipped blobs. A known report blob was deliberately never opened.
- Maintainer chose cleanup in an isolated copy. A private Git bundle backup was
  verified; the cleaned mirror keeps 79 commits and matching author/date/message
  metadata, and its `main` tree equals the original current tree. Known sensitive
  objects are absent after pruning in that isolated copy; the original refs and
  remote were not modified. Commit hashes necessarily change after a rewrite.
- Remote identity/visibility/default branch confirmed as `bcosta19/youtfast`,
  private, `main`. The current local main also contains 3 commits not yet on the
  remote. No public release exists.
- Pattern scan covered all 35 current workflow log archives, 6 issue/PR bodies,
  6 issue comments and 0 review comments. No pattern findings; log bodies were
  handled only in memory. This cannot rule out every kind of private data.
- Inventoried 8 old workflow artifacts, about 586 MiB, and 10 caches, about
  9.53 GiB. Artifact contents remain a separate review/removal decision.
- Repository description/topics are empty, Issues enabled, Actions token
  read-only and Actions PR approval disabled. Allowed Actions are unrestricted.
  Branch protection returned a plan-related 403; vulnerability-reporting status
  returned 404. No settings were changed or claimed enabled.

Remote force-push and cached/PR-reference cleanup still need an authorized next
step. GitHub explains why a force-push alone may leave old objects accessible:
[sensitive-data removal](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository).

## Code and packaging validation

- `cargo fmt --all --check` passed.
- Clippy with `--locked --offline --all-targets -- -D warnings` passed; repeated
  with `--all-features` to include the demo harness.
- Tests passed both default and all-features runs: 202 passed, 2 network tests
  intentionally ignored. Strict rustdoc passed both configurations.
- The vendored RustyPipe emits an upstream lifetime-syntax warning. It does not
  fail these checks; no unrelated vendored-code change was made to hide it.
- 27 Python packaging/privacy/source tests passed. These include DLL dependency
  resolution, rejection of escaped macOS library paths, actual-source digests,
  source/binary version mismatch rejection, portable source-cache symlinks and
  avoiding reads of the known historical report.
- Candidate `v0.1.0` metadata check passed without inventing a publication date.
  A publishing tag still requires a matching frozen changelog/AppStream date.
- Desktop-file/AppStream validation, local Markdown targets, YAML parsing, job
  dependencies and shell syntax passed. A later `actionlint` run over both
  workflows reported no findings; it validates syntax, not hosted execution.
- One actual full MSYS2 mpv source package was downloaded as data and matched to
  base/version/revision. No upstream build scripts were executed. This tests the
  collector path, not the complete Windows runtime source closure.

## Documentation and platform targets

- README now has the existing logo, a native offline-demo capture, feature and
  package tables, short entry points and a separate full user guide.
- `crmne/spotifast` and `crmne/zapfast` were verified through GitHub metadata;
  README uses those actual repositories, without sibling checkout links.
- Local rendered HTML was inspected at 1280 and 390 pixels, with both images
  loaded and no document-level horizontal overflow. This is a local rendering,
  not a screenshot of a published GitHub page.
- Windows x86_64 and macOS Apple Silicon candidate bundling/source collection
  are implemented but have not been executed on their target runners.
- macOS runner/minimum were aligned to macOS 15. The consulted current Homebrew
  mpv bottle metadata had no macOS 14 bottle; retaining a 14 compatibility claim
  would not match the proposed bundle baseline. [Formula metadata](https://formulae.brew.sh/api/formula/mpv.json).
- Arch job prepares a native pacman package and an AUR recipe with `.SRCINFO`,
  building as a non-root user in an official digest-pinned container. Hosted
  execution, installed GUI playback and AUR publication are still pending.
- No hosted workflow was triggered, heavy release build run locally, project
  commit created, remote push performed, visibility changed or release published.

Follow the [current checklist](public-release-checklist.md) for the remaining
public-opening, hosted-validation and binary-release gates.
