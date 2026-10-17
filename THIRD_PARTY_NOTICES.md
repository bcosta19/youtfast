# Third-party notices and corresponding source

YouTFast is distributed under GPL-3.0-only. Earlier snapshots declared MIT;
0.1.0 aligns distribution with the GPL RustyPipe dependency. Existing upstream
notices are retained. Dependency sources keep their own copyright and licenses.

- RustyPipe, by ThetaDev: GPLv3, with the original license and the existing
  comment/avatar patch in `vendor/rustypipe`. No upstream source is relicensed.
- libmpv, by mpv contributors: the upstream GPL/LGPL terms depend on build
  options. The native Linux archive links to the distribution's libmpv rather
  than bundling it. Install it through the distribution's package manager.
- egui, winit and fastframe retain their upstream notices and licenses. The
  release's source-with-dependencies archive includes the exact locked crate
  and git dependency sources, their license files and Cargo configuration for
  offline dependency resolution. `dependency-metadata.json` records package
  versions and declared licenses; this inventory is not a substitute for the
  actual license texts or a complete legal assessment.
- Lucide icons are ISC; Feather-derived icons carry the MIT notice. The
  combined original notice is `assets/icons/LICENSE.txt` and accompanies the
  binary archive.

The Flatpak builds its additional native components from checksum- or
commit-pinned sources. The `flatpak-sources.flatpak` asset exports the builder's
corresponding native sources and manifest. The separate source-with-dependencies
archive supplies the Rust application and all locked Rust dependency sources.
The native components include LuaJIT (MIT), libass (ISC), libplacebo (LGPL),
mpv (GPL/LGPL), yt-dlp and its EJS helper (their upstream terms), and Deno (MIT
with upstream third-party notices). License files installed by those modules
are preserved in the Flatpak. Freedesktop runtime components remain separately
managed by Flatpak and their own source distribution.

Recipients can obtain corresponding source alongside the binaries in the same
GitHub release, without relying only on Cargo registries or moving branches.
Do not redistribute binary-only copies without also meeting the
applicable source and notice obligations.

Windows/macOS candidate bundles include libmpv and linked native libraries.
Their MSYS2/Homebrew inventories and collected notices are review inputs, not
corresponding source. The platform collectors now produce separate source
archives: matching full source packages for MSYS2, and exact installed formula
recipes plus source/resource/patch downloads for Homebrew. Source identities
must match installed binaries; collection fails on a mismatch. Review source
completeness and required notices before publication. The locked Rust archive
alone does not supply those native source trees.
Keep the release in draft until source review and platform QA pass. Candidate
generation does not certify source completeness, signing or notarization.
