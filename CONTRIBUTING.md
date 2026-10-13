# Contributing to YouTFast

YouTFast is a small native YouTube client built with Rust, egui and libmpv.
Contributions should improve complete desktop workflows without introducing a
browser engine, telemetry, a hosted backend or alternate video sources.

## Getting started

The repository is currently private. Only invited collaborators can open issues,
submit pull requests or download releases. Nothing is published on Flathub yet.

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
user-visible behaviour. Interface changes need matching before/after captures,
in light and dark themes and at representative window sizes, without private data.

Fork the project if access permits, or use a topic branch as an invited outside
contributor. Open a pull request against `main`. Maintainer work stays directly on
`main`; external pull requests are squash-merged with contributor credit. Keep
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
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps
```

Automatic GitHub Actions triggers are temporarily paused to conserve runner
minutes, including tag releases. Checks and release validation remain available
through manual requests. Restore the `push`, `pull_request` and `merge_group`
triggers in CI and the `push` tag trigger in Release to resume automation.
Linux is the release target for 0.1.0. Windows and macOS jobs
exercise native compilation and headless tests; green jobs do not certify
interactive playback, account login or portable installers on those platforms.
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
create by itself. The current private repository plan does not support it.
When available, require the `release-gate` check for outside pull requests,
require one review, dismiss stale approvals, require CODEOWNERS review, prohibit
force pushes/deletions and preserve linear history. Keep any maintainer bypass
explicit and do not use it to merge failing changes.

## License

New contributions are provided under GPL-3.0-only, the project's license, unless
explicitly documented otherwise. Retain existing copyright and license notices.
Vendored RustyPipe and other dependencies keep their own notices and licenses;
do not replace upstream license files. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
