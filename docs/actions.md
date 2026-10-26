# GitHub Actions policy and cost

Verified against GitHub's documentation on 2026-10-17. Recheck the linked rules
before changing runner classes, retention or spending settings.

## Billing

Standard GitHub-hosted runners in public repositories have free compute usage,
including Linux, Windows and macOS. Private repositories use the owner's plan
allowance; the 90% notification concerns that billing period's included usage.
Making a repository public does not erase earlier usage. Larger runners remain
billable even for public repositories. Artifacts and caches have separate
storage rules; free compute is not an unlimited-storage promise.
See [GitHub Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions).
The account's current balance/plan has not been verified by this repository.

## Execution policy

CI handles pull requests to `main`, pushes to `main`, merge queues and manual
runs. The repository is public, so these run automatically on standard runners.
The `YOUTFAST_AUTOMATIC_CI` variable only matters for private copies, where it
opts in to automatic jobs that consume the owner's allowance.
Hosted jobs also have a six-hour GitHub execution limit; our build/test
timeouts are shorter (60/90 minutes). There is no monthly compute-minute quota
for public standard runners. See [Actions limits](https://docs.github.com/en/actions/reference/limits).
There are no scheduled builds.

The Release workflow handles `v*` tags and manual dry runs. The same visibility
guard applies to automatic tag builds. Its reusable CI validates all platforms;
every package job, including the clean Arch package build, must pass before
assembly and the final draft-creation job.
Final publication is a separate maintainer action on the reviewed draft. A
successful CI or draft is not a published release.

Superseded PR runs are cancelled. Release runs are not cancelled midway, and
intermediate artifacts expire after seven days. Release assets are the durable
downloads. Keep caches bounded; do not increase paid storage limits without
review. Avoid repeated full dry runs with no code changes.

## Repository settings

Configured on 2026-10-17; verify them in the repository settings rather than
inferring them from files in Git.

- Allowed Actions: GitHub-owned, `msys2/setup-msys2` and `Swatinem/rust-cache`,
  with SHA pinning required. Adding an Action means updating that list.
- Keep the default token read-only. Only draft creation gets `contents: write`.
  Website validation stays read-only and receives no deployment credentials.
- Require approval for workflows from outside contributors as appropriate;
  never run their code through `pull_request_target` with secrets.
- Use GitHub-hosted ephemeral runners for untrusted contributions.
- `main` requires the `release-gate` check, resolved conversations and linear
  history, and blocks force pushes/deletion. Admins are not enforced, so the
  maintainer can push directly; require review where another reviewer exists.
  See [protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches).
- Private vulnerability reporting, Dependabot alerts/security updates, secret
  scanning and push protection are enabled.
- Review artifact/cache usage. Do not use larger runners as an implicit upgrade.

## Project website

The Website workflow (`.github/workflows/pages.yml`) validates the bilingual
static site with the focused Python tests and `python3 tools/build_site.py`.
PRs, merge queues and site-related pushes to `main` run the same validation.
The workflow has read-only permissions and does not publish to GitHub Pages
or Cloudflare. Its only Action is GitHub-owned and SHA-pinned.

Production is https://youtfast.pages.dev/, with Portuguese at `/pt/`.
Cloudflare Pages uses Direct Upload of the explicit `dist/site` allowlist.
Publishing requires an authenticated maintainer running the documented command;
merging a PR does not automatically deploy. No Cloudflare credentials are
stored in the repository or passed to contributor jobs. See
[website maintenance](../website/README.md).
