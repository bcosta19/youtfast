# Releasing YouTFast

## Current state and release policy

0.1.0 was published on 2026-10-17 as a pre-release, and 0.2.0 was published on
2026-10-22. Their remaining gates, and those of later releases, are tracked in
[ROADMAP.md](ROADMAP.md).

0.3.0 is the next release. Its changelog and AppStream dates are frozen at
2026-10-24, so it can be tagged from `main`.

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
2. Prepare a reviewed candidate branch; the final tag must be reachable from `main`. Align `Cargo.toml`,
   the YouTFast entry in `Cargo.lock`, AppStream version/date, changelog
   version/date and `packaging/release-notes/vX.Y.Z.md`. A manual dry run may keep
   the pending changelog date; before tagging, freeze its ISO publication date and align
   AppStream. The current metadata date is provisional until freeze. Notes need
   **Included**,
   **Not included** and **Known limitations**. The first release links to its
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
git tag -a v0.3.0 -m 'YouTFast 0.3.0'
git push origin v0.3.0
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
gh release edit v0.3.0 --draft=false --latest
```

Verify notes, release date and downloads as an anonymous reader after
publication. `gh release download v0.3.0` retrieves a draft's assets for
review. Run `sha256sum --check checksums.txt` in the download directory.
Checksums detect corruption; they are not publisher signatures. Never claim
code signing/notarization or interactive coverage that has not been performed.

If any required job fails, no draft should be created. Re-run failed jobs for
transient failures. If code changes are needed before the first tag, validate a
new commit. After a tag is distributed, fix code through a new version. If a
draft already exists, inspect/reconcile it manually; do not delete or overwrite
released assets as a retry strategy.

## Expected asset inventory

Named assets below have the `youtfast-vX.Y.Z-` prefix, except the Arch pacman
package (`youtfast-X.Y.Z-1-x86_64.pkg.tar.zst`) and `checksums.txt`.

| Asset | Purpose |
|---|---|
| `x86_64-unknown-linux-gnu.tar.gz` | Ubuntu 24.04/glibc 2.39+ baseline; system libmpv |
| `x86_64.flatpak` | Freedesktop 25.08 runtime; sandboxed app/native components |
| `flatpak-sources.flatpak` | Flatpak builder's exported corresponding sources |
| `x86_64-pc-windows-gnu.zip` | Windows executable, imported DLL closure, notices/inventory |
| `aarch64-apple-darwin.zip` | macOS 15+ Apple Silicon app, dylibs, notices/inventory |
| `source-with-dependencies.tar.gz` | App and locked Rust sources/notices; offline Cargo config |
| `aur.tar.gz` | Tested PKGBUILD and `.SRCINFO`, with source SHA-256; not an AUR submission |
| Arch `.pkg.tar.zst` | Candidate built/installed in a clean Arch container; install locally with `pacman -U` |
| `windows-native-sources.tar.gz` | Matching full MSYS2 source packages, metadata/notices and manifest |
| `macos-native-sources.tar.gz` | Matching Homebrew recipes, source/resource/patch downloads, notices and manifest |
| `channels.tar.gz` | Rendered Flathub source manifest, Winget manifest and Homebrew Cask with exact asset SHA-256 digests |
| `linux.png` | Fresh signed-out profile preview from CI |

`checksums.txt` covers assembled candidates. Final publication must also contain
reviewed Windows/macOS native source archives and their digests, now required
by the assembly job. Sources contain
no Git history, raw reports, cookies or user files. GitHub's automatic tag source
ZIP does **not** replace the source-with-dependencies or native source assets.

Installation, architecture limits, external yt-dlp/Deno requirements and signing
status are in [distribution.md](docs/distribution.md). Native bundles are not
installers, a Flatpak bundle is not a Flathub listing, and an AUR recipe is not a
pacman repository. Publish each channel only after its build/review gates pass;
[Arch maintenance](packaging/arch/README.md) describes per-release package updates.
