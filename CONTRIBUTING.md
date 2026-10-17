# Contributing to YouTFast

YouTFast is a small native YouTube client built with Rust, egui and libmpv.
Contributions should improve complete desktop workflows without introducing a
browser engine, telemetry, a hosted backend or alternate video sources.

## Getting started

The source repository is public; the 0.1.0 binaries are still being qualified.
AUR and Flathub submissions are separate work; neither listing is claimed here.

Use Rust from `rust-toolchain.toml`; rustup selects it automatically. On Ubuntu
24.04, install the build dependencies:

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libmpv-dev libxkbcommon-dev \
  libwayland-dev libgl1-mesa-dev libfontconfig1-dev libssl-dev
cargo build --locked
```

Runtime playback needs a current yt-dlp and libmpv built with Lua support.
Keep `Cargo.lock` committed and use `--locked`. mise/mbx is optional; a normal
rustup/Cargo checkout must work independently of sibling repositories.
Read [AGENTS.md](AGENTS.md) for architecture and privacy constraints and
[README.md](README.md) for runtime requirements.

## Issues and pull requests

Search existing issues first. Bug reports need the app version, distribution,
reproduction steps, expected behaviour and actual behaviour. Use synthetic
examples. Never attach cookies, browser profiles, private history, keyring
entries, Takeout exports or raw API response reports. See [SECURITY.md](SECURITY.md)
for vulnerabilities and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for conduct.

Discuss substantial features or interface redesigns before implementing them.
Keep pull requests focused, explain why the change is useful, and include
regression tests. Document changes to settings, files, network access and
user-visible behaviour. Record implemented changes in CHANGELOG.md under
`Unreleased`; future work goes in ROADMAP.md. Interface changes need matching
before/after captures in light/dark themes and representative sizes, without private data.

Fork the project, or use a topic branch if you have write access.
Open a PR against `main`; maintainer release changes should also be reviewable
through a topic branch/PR. External PRs are squash-merged with credit. Keep
history linear and do not rewrite published commits or release tags.

AI-assisted contributions follow the same rules: the contributor must understand
the code, verify it, and respond to review comments. A tool's output is not a
substitute for tests or maintainer review.

## Checks

Run these before submitting:

```sh
python3 -m unittest discover -s packaging -p 'test_*.py'
python3 packaging/release.py check
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --all-features
```

CI runs automatically on pull requests and pushes to `main`; Release runs on
`v*` tags and manual dry runs. See [Actions policy](docs/actions.md).
0.1.0 targets Linux x86_64, Windows x86_64 and macOS Apple Silicon; hosted
packaging/headless tests do not certify interactive playback, optional accounts
or clean-machine installation. Complete [platform QA](docs/distribution.md).
Do not run ignored network tests as part of default CI. Tests must not access
real credentials, browser data or user-owned library/settings files.

CI jobs have read-only repository permissions. Never introduce
`pull_request_target` execution of contributor code, privileged self-hosted
runners for external changes, or secret-bearing build scripts. Pin Actions to
reviewed commit SHAs; automated updates still need review.

## Maintainer release process

See [RELEASING.md](RELEASING.md). A release must name a commit reachable from
`main`, match Cargo and desktop metadata, include written release notes, and
finish all required checks and packaging before publication.

Branch protection is a GitHub repository setting, not something a workflow can
create by itself. `main` requires `release-gate`, resolved conversations and
linear history, and blocks force pushes and deletion. Require review where
another reviewer is available. The maintainer can still push directly; do not
use that bypass to land failing changes.

## License

New contributions are provided under GPL-3.0-only, the project's license, unless
explicitly documented otherwise. Retain existing copyright and license notices.
Vendored RustyPipe and other dependencies keep their own notices and licenses;
do not replace upstream license files. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
