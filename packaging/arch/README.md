# Arch package maintenance

The template is preparation for the AUR package `youtfast`, built from source.
It is not an existing AUR listing, binary pacman repository or tested Arch build.
Check the name's availability and set the maintainer contact before submission.

The Release Arch job renders the template using the exact source archive's
SHA-256 and Cargo version/MSRV. It builds/tests as an unprivileged user in a
fresh official Arch container pinned by digest, installs the resulting package,
runs namcap and checks `--version`. Only then does it export the AUR recipe
with `.SRCINFO` and the `.pkg.tar.zst` binary. These jobs still need a hosted run.
There is no `SKIP` checksum, mutable branch or guessed release digest. The source
asset includes the locked Rust dependencies and offline Cargo configuration;
build and check run with `--frozen`. System libmpv/yt-dlp remain Arch packages.

To generate it locally from an already built final source asset:

```sh
python3 packaging/arch.py dist/youtfast-v0.1.0-source-with-dependencies.tar.gz
```

Do not upload the `.in` template. In a separate Arch packaging checkout, review
the generated `PKGBUILD`, add the maintainer contact, then run:

```sh
makepkg --verifysource
makepkg --printsrcinfo > .SRCINFO
extra-x86_64-build
namcap PKGBUILD youtfast-0.1.0-1-x86_64.pkg.tar.zst
```

`extra-x86_64-build` comes from `devtools`; it installs/builds in a clean chroot.
Inspect installed files, license notices, desktop launch, playback and optional
keyring behaviour before submitting **only** `PKGBUILD` and `.SRCINFO` to the
separate AUR Git repository. Publication needs an AUR account/SSH setup and an
explicit maintainer action; GitHub Actions does not push to AUR.

After the accepted entry is public, users can run `yay -S youtfast`. Before
that, do not advertise that command as working. `makepkg` builds a local package;
`sudo pacman -U ./youtfast-0.1.0-1-x86_64.pkg.tar.zst` installs it. `pacman -S`
requires inclusion in an official or separately configured binary repository.

For every upstream release, regenerate the recipe with the final archive and
new version/hash, reset `pkgrel=1`, regenerate `.SRCINFO` and repeat the chroot
test. A packaging-only fix increments `pkgrel` without retagging upstream.
If a shared-library update requires rebuilding, communicate it to users.

References: [AUR submission guidelines](https://wiki.archlinux.org/title/AUR_submission_guidelines),
[Rust packaging](https://wiki.archlinux.org/title/Rust_package_guidelines),
[AUR helpers](https://wiki.archlinux.org/title/AUR_helpers).
