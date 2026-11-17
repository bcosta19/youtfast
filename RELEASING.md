# Releasing YouTFast

## Release policy

Published versions and their dates are the
[GitHub releases](https://github.com/bcosta19/youtfast/releases) and
[CHANGELOG.md](CHANGELOG.md); remaining gates are tracked in
[ROADMAP.md](ROADMAP.md). The existing 0.6.1 tag and draft are preserved.

Use stable SemVer tags `vMAJOR.MINOR.PATCH`. During 0.x, use patches for
compatible fixes and minor versions for features or incompatible changes;
explain settings/library migrations and preserve user data. Do not promise
file-format stability merely because a tag is stable. Release when gates pass;
there is no scheduled release or automatic version bump.

Each meaningful PR updates [CHANGELOG.md](CHANGELOG.md) under `Unreleased`, using
Added, Changed, Deprecated, Removed, Fixed or Security where applicable. Record
implemented changes only. Unimplemented/deferred work belongs in
[ROADMAP.md](ROADMAP.md). At freeze, move only changes present in the release
commit into its version section, leave a fresh `Unreleased` section and update
comparison links. Released entries/tags/assets are immutable; fix mistakes with
a patch release instead of replacing downloads.

## Prepare and validate

1. Never commit raw API reports, logs, cookies or personal data; the public
   history was cleaned once and must stay clean.
2. Prepare a reviewed candidate branch; the final tag must be reachable from `main`.
   One command moves `Unreleased` into the new version and aligns every versioned file:

   ```sh
   python3 packaging/release.py prepare X.Y.Z --summary 'One sentence on what changed.'
   ```

   It sets `Cargo.toml` and the workspace's `Cargo.lock` entries, adds the
   AppStream release (the summary is its description), dates the changelog
   section, updates its comparison links and drafts
   `packaging/release-notes/vX.Y.Z.md`: **Included** from the changelog entries,
   and **Packages**, **Not included** and **Known limitations** carried over
   from the previous notes. Edit the notes until they are true for this
   version. The date defaults to today; if the tag is pushed on another day,
   `python3 packaging/release.py freeze --date YYYY-MM-DD` moves the
   changelog and AppStream dates together. The first release links to its
   tag; later versions link to their previous-tag comparisons.
3. Run CONTRIBUTING.md's packaging/Rust/documentation checks and actionlint
   when available. Do not use real accounts to debug releases. Preserve
   unrelated local changes and inspect the exact commit/source export.
4. Commit/push only when authorized. On that exact commit, manually dispatch
   **Release**, selecting the intended ref and entering `vX.Y.Z`. The dry run
   reuses full CI and builds every candidate on hosted runners without creating
   a release. Download artifacts before their seven-day retention expires.
   See [Actions policy](docs/actions.md).
5. Inspect all packages and complete [platform QA](docs/distribution.md).
   Windows/macOS also need matching native dependency sources, patches, build
   instructions and notices; inventories are not sufficient. If a required
   platform fails, fix it and repeat; do not silently drop it.
6. After approval, create an annotated tag on the validated commit and push
   that tag. Never move an existing distributed tag. Automatic tag jobs run
   only when public or opted in with `YOUTFAST_AUTOMATIC_CI=true`.

For a future authorized tag operation, with the validated commit selected:

```sh
git tag -a v0.9.0 -m 'YouTFast 0.9.0'
git push origin v0.9.0
```

## Draft, review and publication

The tag workflow validates version/date/notes, annotated-tag type and ancestry
from `main`, reuses CI and requires Linux, source, Arch, Flatpak, Windows and macOS
jobs. Only its final draft job has `contents: write`. It creates a draft with
committed notes and checksums; it never publishes automatically. Contributor
builds have read-only permissions and receive no maintainer secrets.

Review the **tag build's** artifacts again: they are rebuilt, not the same bytes
as an earlier dry run. Review the matching native source archives and
notices produced by the platform collectors. If attachments need correction,
recompute `checksums.txt` to cover the final inventory and verify it. Keep the
draft unpublished while any privacy, source or platform gate is pending.
Collectors fail on missing or mismatched source identities. A successful
collection still requires a license/build review; do not treat it as a legal
completeness certification.

Once approved, publish the reviewed draft in GitHub's release UI, or explicitly:

```sh
gh release edit v0.9.0 --draft=false --latest
```

The draft's notes are the committed notes followed by install commands and the
contents of `checksums.txt` (`release.py notes`). Verify notes, release date and
downloads as an anonymous reader after publication. `gh release download v0.9.0`
retrieves a draft's assets for review. Run `sha256sum --check checksums.txt` in
the download directory. Checksums detect corruption; they are not publisher
signatures. Never claim code signing/notarization or interactive coverage that
has not been performed.

If any required job fails, no draft should be created. Re-run failed jobs for
transient failures. If code changes are needed before the first tag, validate a
new commit. After a tag is distributed, fix code through a new version. If a
draft already exists, inspect/reconcile it manually; do not delete or overwrite
released assets as a retry strategy.

## After publication

Publishing (not a pre-release) starts the **Publish** workflow. It refuses a
draft, verifies `channels.tar.gz` against the release's `checksums.txt`, and then:

- commits `Casks/youtfast.rb` to `bcosta19/homebrew-tap` (never to an older
  version than the tap already offers);
- submits the Winget manifests to `microsoft/winget-pkgs` with `wingetcreate`
  (pinned by digest). Microsoft's review accepts or rejects the pull request;
- deploys the website from the tag and waits until `/install` and
  `/pt/install` link to the new version.

A failed job does not affect the release; fix the cause and run **Publish**
manually with the tag and the failed channel. A manual `website` run deploys
the branch it is started from, after checking that its `Cargo.toml` version is
a published release.

### One-time setup

Create a `publish` environment (Settings → Environments), limit its deployment
branches and tags to `main` and `v*`, and add these environment secrets:

| Secret | Scope |
|---|---|
| `HOMEBREW_TAP_TOKEN` | Fine-grained token for `bcosta19/homebrew-tap` only, Contents read/write |
| `WINGET_TOKEN` | Classic token with `public_repo`, used by `wingetcreate` to fork `winget-pkgs` and open the pull request |
| `CLOUDFLARE_API_TOKEN` | Cloudflare API token with Pages edit permission for the account |
| `CLOUDFLARE_ACCOUNT_ID` | The Cloudflare account that owns the `youtfast` Pages project |

A missing secret fails only its job, with a message naming it.

## Expected asset inventory

Named assets below have the `youtfast-vX.Y.Z-` prefix, except the Arch pacman
package (`youtfast-X.Y.Z-1-x86_64.pkg.tar.zst`) and `checksums.txt`.

| Asset | Purpose |
|---|---|
| `x86_64-unknown-linux-gnu.tar.gz` | Ubuntu 24.04/glibc 2.39+ baseline; system libmpv |
| `x86_64.flatpak` | Freedesktop 25.08 runtime; sandboxed app/native components |
| `flatpak-sources.flatpak` | Flatpak builder's exported corresponding sources |
| `x86_64-pc-windows-gnu.zip` | Windows portable executable, imported DLL closure, notices/inventory |
| `x86_64-windows-setup.exe` | Per-user NSIS Windows setup installer built from the verified Windows bundle |
| `aarch64-apple-darwin.zip` | macOS 15+ Apple Silicon app, dylibs, notices/inventory |
| `source-with-dependencies.tar.gz` | App and locked Rust sources/notices; offline Cargo config |
| `aur.tar.gz` | Tested PKGBUILD and `.SRCINFO`, with source SHA-256; not an AUR submission |
| Arch `.pkg.tar.zst` | Candidate built/installed in a clean Arch container; install locally with `pacman -U` |
| `windows-native-sources.tar.gz` | Matching full MSYS2 source packages, metadata/notices and manifest |
| `macos-native-sources.tar.gz` | Matching Homebrew recipes, source/resource/patch downloads, notices and manifest |
| `channels.tar.gz` | Rendered Flathub source manifest, multi-file Winget manifests (`bcosta19.YouTFast`) and Homebrew Cask (`Casks/youtfast.rb`) with exact asset SHA-256 digests |
| `linux.png` | Fresh signed-out profile preview from CI |

`checksums.txt` covers assembled candidates. Final publication must also contain
reviewed Windows/macOS native source archives and their digests, now required
by the assembly job. Sources contain
no Git history, raw reports, cookies or user files. GitHub's automatic tag source
ZIP does **not** replace the source-with-dependencies or native source assets.

Installation, architecture limits, external yt-dlp/Deno requirements and signing
status are in [distribution.md](docs/distribution.md). Native archives are
portable bundles, Windows also builds a per-user NSIS setup installer, a Flatpak
bundle is not a Flathub listing, and an AUR recipe is not a pacman repository.
Publish each channel only after its build/review gates pass;
[Arch maintenance](packaging/arch/README.md) describes per-release package updates.
