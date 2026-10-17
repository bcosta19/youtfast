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
