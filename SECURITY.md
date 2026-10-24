# Security policy

## Supported versions

Security fixes target the latest release on the `0.x` line and current `main`.
YouTFast is an early-stage unofficial client. Native packages do not certify
interactive/account support on every platform; see [platform coverage](docs/distribution.md).

## Reporting a vulnerability

Do not post exploit details or private user data in an issue or pull request.
Use GitHub's **Security > Report a vulnerability**; private vulnerability
reporting is enabled for this repository. If that option is unavailable, ask
the maintainer `@bcosta19` for a private reporting channel without including
vulnerability details.

Include the affected version, impact and minimal reproduction with synthetic
credentials/data. Never send a Google session, cookies.txt, browser profile,
keyring export, Takeout archive or full YouTube API response. Ordinary UI bugs
belong in the bug form after any sensitive information has been removed.

## Trust boundaries

YouTube cookies are account credentials. They belong only in the platform
credential store, never logs, repository files, settings, library or caches.
Account changes must reject stale asynchronous responses. Network responses,
imports, images and external error text are untrusted inputs and need bounds
and controlled error messages.

Do not bypass privacy guards, dependency checks or CI security controls to make a
build pass. Release binaries are built on hosted GitHub runners from the tagged
commit, with locked dependencies, and distributed with SHA-256 checksums and
corresponding source. Checksums detect corruption; they are not publisher signing.
