# Branch Protection

Configure protection in **Settings → Rules → Rulesets** (preferred) or
**Settings → Branches**. Apply full protection to `main` and `release/*`;
`develop` and `hotfix/*` may use medium protection. Review settings quarterly.

## Required `main` rules

- Require a pull request and one approval.
- Dismiss stale approvals, require approval of the latest reviewable push, and
  require Code Owner review from [CODEOWNERS](CODEOWNERS).
- Require the branch to be current and all selected status checks to pass.
- Require resolved conversations, signed commits, and linear history.
- Disallow bypass, force pushes, branch deletion, and direct pushes except an
  explicitly required CI/CD service account.
- Do not lock the branch or require deployments unless an incident or a real
  deployment-preview workflow requires it.

Allow squash merging, optionally allow rebase merging, and disable merge
commits. Enable auto-merge and automatic deletion of merged head branches.

## Status checks

Select the exact contexts reported by the current workflows. The core set is:

```text
Rust - Test
Rust - Build (ubuntu-latest)
Rust - Build (macos-latest)
Rust - Security Audit
Rust - Benchmarks
Operator - Test
Operator - Build Image
UI - Test & Lint
UI - Build Image
Helm - Validate Chart
Validate PR Metadata
Dependency Review
Require Label
Check Documentation Updated
Check Changelog Updated
Check Breaking Changes
Security Review Required
```

The documentation and changelog jobs currently emit warnings rather than
failing when an update may be missing; requiring them still ensures the jobs
ran. Do not configure the obsolete `Rust - Build (windows-latest)` context.

If the separate Docker Build workflow is part of the protected gate, require
`Build Streamforge Image`, `Build Operator Image`, and `Build UI Image` only
after each context has completed successfully on a pull request. Avoid adding a
path-filtered or conditional context that is absent on unrelated pull requests.

## Other patterns

| Pattern | Protection |
| --- | --- |
| `main` | Rules above |
| `release/*` | Same as `main`, plus release-team review and deployment success when configured |
| `develop` | Pull request, CI, and conversation resolution |
| `hotfix/*` | Same as `main`, with two approvals; emergency bypass only as below |

## Repository automation

Under **Settings → Actions → General**, grant read/write workflow permissions
and allow GitHub Actions to create and approve pull requests only when the
repository automation requires those capabilities.

Maintain the labels consumed by `.github/workflows/auto-label.yml`,
`.github/labeler.yml`, and `.github/workflows/pr-checks.yml`:

- size: `size/XS`, `size/S`, `size/M`, `size/L`, `size/XL`;
- type: `bug`, `enhancement`, `documentation`, `maintenance`, `dependencies`,
  `breaking-change`, `security`, `performance`;
- priority: `priority/critical`, `priority/high`, `priority/medium`,
  `priority/low`;
- status: `needs-review`, `needs-changes`, `approved`, `on-hold`, `wip`;
- component: `component/kafka`, `component/dsl`, `component/operator`,
  `component/ui`, `component/helm`, `component/docker`; and
- area: `area/core`, `area/config`, `area/security`.

The `Require Label` check accepts `bug`, `enhancement`, `documentation`,
`maintenance`, or `dependencies`.

## Verify enforcement

- Open a pull request and confirm review, current-branch, conversation, and
  selected status requirements appear.
- Confirm a merge without approval or a required check is blocked.
- Confirm force-pushing to and deleting `main` are blocked.
- Confirm Code Owners are requested and auto-labeling supplies expected labels.
- Confirm only squash or rebase merging is available.

## Emergency bypass

Bypass only for a critical production incident, a falsely blocking CI failure,
or a GitHub outage. Use a minimal `hotfix/*` branch and signed commit, document
the reason in the pull request, obtain two maintainer approvals when possible,
temporarily disable only the blocking rule, merge, immediately restore the
rule, and open a postmortem/follow-up issue. Never leave protection weakened.

References: [protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches),
[rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets),
[signed commits](https://docs.github.com/en/authentication/managing-commit-signature-verification/about-commit-signature-verification),
and [Actions permissions](https://docs.github.com/en/actions/security-guides/automatic-token-authentication).

Maintainer: [@rahulbsw](https://github.com/rahulbsw). Last updated: August 2026.
