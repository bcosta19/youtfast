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
runs. Automatic jobs run only when the repository is public or the repository
variable `YOUTFAST_AUTOMATIC_CI` is explicitly `true`. Manual CI and Release
requests are deliberate opt-ins and may consume private-repository minutes.
Hosted jobs also have a six-hour GitHub execution limit; our build/test
timeouts are shorter (60/90 minutes). There is no monthly compute-minute quota
for public standard runners. See [Actions limits](https://docs.github.com/en/actions/reference/limits).
There are no scheduled builds. Do not enable the private opt-in just to prepare
documentation or publish it before the privacy review.

The Release workflow handles `v*` tags and manual dry runs. The same visibility
guard applies to automatic tag builds. Its reusable CI validates all platforms;
every package job, including the clean Arch package build, must pass before
assembly and the final draft-creation job.
Final publication is a separate maintainer action on the reviewed draft. A
successful CI or draft is not a published release.

Superseded PR runs are cancelled. Release runs are not cancelled midway, and
intermediate artifacts expire after seven days. Release assets are the durable
downloads. Keep caches bounded; do not increase paid storage limits without
review. While private, avoid repeated full dry runs with no code changes.

## Public repository settings

- Allow only needed Actions; keep third-party Actions pinned to reviewed SHAs.
- Keep the default token read-only. Only draft creation gets `contents: write`.
- Require approval for workflows from outside contributors as appropriate;
  never run their code through `pull_request_target` with secrets.
- Use GitHub-hosted ephemeral runners for untrusted contributions.
- Set `release-gate` as a required check on `main`, require review where another
  reviewer is available, block force pushes/deletions and keep linear history.
  Maintainer bypasses must be explicit. Workflows alone do not enforce this.
- Keep spending limits/alerts configured for other private projects and review
  artifact/cache usage. Do not use larger runners as an implicit upgrade.

Branch protection is available for public repositories on GitHub Free; private
repositories depend on the plan. These settings must actually be configured,
not inferred from files in Git. See [protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches).
