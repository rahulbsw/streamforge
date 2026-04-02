# GitHub Repository Configuration

This directory contains all GitHub-specific configuration files including workflows, templates, and automation rules.

---

## 📁 Directory Structure

```
.github/
├── workflows/              # GitHub Actions workflows
│   ├── ci.yml             # Main CI pipeline (tests, builds, security)
│   ├── docker.yml         # Docker image builds
│   ├── release.yml        # Release automation
│   ├── pr-checks.yml      # Pull request validation checks
│   ├── auto-label.yml     # Automatic labeling
│   └── stale.yml          # Stale issue/PR management
├── ISSUE_TEMPLATE/        # Issue templates
│   ├── bug_report.md      # Bug report template
│   └── feature_request.md # Feature request template
├── CODEOWNERS             # Code ownership and review assignment
├── labeler.yml            # File-based auto-labeling rules
├── pull_request_template.md # PR template
├── BRANCH_PROTECTION.md   # Branch protection setup guide
└── README.md             # This file
```

---

## 🔧 Workflows

### CI/CD Workflows

#### `ci.yml` - Continuous Integration
**Triggers:** Push to `main`, Pull requests to `main`

**Jobs:**
- **Rust Tests** - Run all tests with coverage
- **Rust Build** - Multi-platform builds (Linux, macOS, Windows)
- **Rust Security** - Security audit with cargo-audit
- **Rust Benchmarks** - Build and validate benchmarks
- **Operator Tests** - Kubernetes operator tests
- **UI Tests** - Frontend tests and linting
- **Helm Validation** - Chart linting and templating

**Status:** ✅ Required for merge

#### `docker.yml` - Docker Builds
**Triggers:** Push to `main`, tags

**Jobs:**
- Build and push Docker images
- Multi-architecture support (amd64, arm64)

**Status:** ⚠️ Optional (for releases)

#### `release.yml` - Release Automation
**Triggers:** Push tags (`v*`)

**Jobs:**
- Create GitHub release
- Build release artifacts
- Publish to crates.io
- Push Docker images

**Status:** ⚠️ Release only

### Pull Request Workflows

#### `pr-checks.yml` - PR Validation
**Triggers:** Pull request events

**Checks:**
1. **PR Metadata** - Validates PR title follows conventional commits
2. **PR Size** - Warns on large PRs (>500 lines), fails on very large (>1500 lines)
3. **Dependency Review** - Scans for vulnerable dependencies
4. **Label Requirements** - Ensures PR has required labels
5. **Documentation Check** - Warns if code changes without doc updates
6. **Changelog Check** - Warns if CHANGELOG.md not updated
7. **Breaking Changes** - Flags breaking changes for extra review
8. **Security Review** - Flags security-sensitive code changes

**Status:** ✅ Required for merge (most checks)

#### `auto-label.yml` - Automatic Labeling
**Triggers:** PR/Issue opened or updated

**Functions:**
- Labels PRs based on changed files (using `labeler.yml`)
- Adds size labels (XS, S, M, L, XL)
- Labels issues based on content
- Welcomes first-time contributors

**Status:** ℹ️ Informational

#### `stale.yml` - Stale Management
**Triggers:** Daily schedule, manual

**Functions:**
- Marks inactive issues stale after 60 days
- Closes stale issues after 14 days
- Marks inactive PRs stale after 30 days
- Closes stale PRs after 7 days
- Respects exempt labels (pinned, security, etc.)

**Status:** ℹ️ Maintenance

---

## 📋 Templates

### Issue Templates

#### Bug Report (`ISSUE_TEMPLATE/bug_report.md`)
**Use When:** Reporting a bug or unexpected behavior

**Includes:**
- Expected vs actual behavior
- Steps to reproduce
- Environment information
- Configuration details

#### Feature Request (`ISSUE_TEMPLATE/feature_request.md`)
**Use When:** Suggesting new features or enhancements

**Includes:**
- Problem description
- Proposed solution
- Alternatives considered
- Implementation ideas

### Pull Request Template

**Location:** `pull_request_template.md`

**Sections:**
- Description and type of change
- Related issues
- Changes made
- Testing performed
- Configuration examples
- Checklist (style, tests, docs)
- Performance impact

**Required Fields:**
- Type of change (bug fix, feature, etc.)
- Testing checklist
- Documentation updates

---

## 👥 Code Ownership

### CODEOWNERS File

Automatically requests reviews from designated owners when PRs touch specific files.

**Current Owners:**
- **Global:** @rahulbsw (all files)
- **Critical Files:** Security, configuration, CI/CD
- **Components:** Core, Kafka, DSL, Operator, UI

**How It Works:**
1. PR touches files in owned paths
2. GitHub automatically requests review from owner
3. PR cannot merge without owner approval (if required)

**Adding Owners:**
```
/path/to/code @username @org/team-name
```

---

## 🏷️ Labels

### Automatic Labels

Configured in `labeler.yml`, automatically applied based on:

#### File-Based Labels
- `area/*` - Code area (core, kafka, dsl, config, security)
- `component/*` - Component (operator, ui, helm, docker)
- `documentation` - Doc changes
- `dependencies` - Dependency updates
- `ci/cd` - CI/CD changes
- `tests` - Test changes
- `benchmarks` - Benchmark changes

#### Size Labels
- `size/XS` - < 10 lines
- `size/S` - < 50 lines
- `size/M` - < 200 lines
- `size/L` - < 500 lines
- `size/XL` - 500+ lines

#### Branch-Based Labels
- `version/patch` - Bug fixes (fix/*, hotfix/*)
- `version/minor` - Features (feature/*, feat/*)
- `version/major` - Breaking changes (breaking/*, major/*)

### Manual Labels (Require Reviewer Action)

#### Type Labels (Required - at least one)
- `bug` - Bug fixes
- `enhancement` - New features
- `documentation` - Documentation only
- `maintenance` - Refactoring, cleanup
- `dependencies` - Dependency updates

#### Priority Labels
- `priority/critical` - Production incidents, security issues
- `priority/high` - Important, blocking work
- `priority/medium` - Normal priority
- `priority/low` - Nice to have, backlog

#### Status Labels
- `needs-review` - Awaiting review
- `needs-changes` - Changes requested
- `approved` - Ready to merge
- `on-hold` - Blocked or paused
- `wip` - Work in progress (don't merge)

#### Special Labels
- `breaking-change` - Breaking API/config changes
- `security` - Security-related changes
- `performance` - Performance improvements
- `good-first-issue` - Good for newcomers
- `help-wanted` - Looking for contributors
- `pinned` - Never mark as stale
- `blocked` - Waiting on external dependency
- `skip-changelog` - Don't require changelog update

---

## 🛡️ Branch Protection

**See:** [`BRANCH_PROTECTION.md`](BRANCH_PROTECTION.md) for complete setup guide

### Quick Summary

#### Main Branch (`main`)

**Required Checks:**
- ✅ At least 1 approval
- ✅ All CI tests pass
- ✅ Conversations resolved
- ✅ Signed commits
- ✅ Linear history
- ✅ Up-to-date with base
- ✅ Code owner review

**Restrictions:**
- ❌ No force pushes
- ❌ No deletions
- ❌ No bypassing rules

**Allowed Merge Types:**
- ✅ Squash merge (recommended)
- ✅ Rebase merge
- ❌ Merge commits (disabled)

### Protected Patterns

| Pattern | Protection | Use |
|---------|-----------|-----|
| `main` | Full | Production code |
| `release/*` | Full | Release branches |
| `hotfix/*` | Medium | Emergency fixes |

---

## 🚀 Workflow Usage

### For Contributors

#### Creating a Pull Request

1. **Branch Naming:**
   ```bash
   feature/add-new-filter    # New features
   fix/kafka-connection      # Bug fixes
   docs/update-readme        # Documentation
   perf/optimize-dsl         # Performance
   ```

2. **Commit Messages:**
   ```bash
   # Follow conventional commits
   feat: add regex filter support
   fix: resolve kafka connection timeout
   docs: update configuration guide
   perf: optimize filter evaluation
   ```

3. **Creating PR:**
   - Fill out PR template completely
   - Link related issues
   - Add appropriate labels (automated + manual)
   - Request reviews from code owners
   - Ensure all checks pass

#### During Review

1. **Address feedback:**
   - Mark conversations as resolved
   - Push new commits (don't force push)
   - Re-request review when ready

2. **Keep PR updated:**
   - Rebase or merge main regularly
   - Resolve conflicts promptly

3. **Before merging:**
   - All checks green ✅
   - Approved by required reviewers ✅
   - Conversations resolved ✅
   - CHANGELOG updated (or skip-changelog label) ✅

### For Maintainers

#### Reviewing Pull Requests

1. **Automated checks:**
   - Wait for CI to complete
   - Review any warnings/failures
   - Check automated labels are correct

2. **Code review:**
   - Check code quality
   - Verify tests added/updated
   - Review documentation changes
   - Check for breaking changes

3. **Approval:**
   - Approve if satisfactory
   - Request changes if needed
   - Comment for discussion

4. **Merging:**
   - Use **Squash and merge** (default)
   - Edit commit message if needed
   - Ensure linear history maintained

#### Managing Issues

1. **Triage:**
   - Review new issues daily
   - Add appropriate labels
   - Assign to team members
   - Link to milestones if applicable

2. **Label management:**
   - Add priority labels
   - Add type labels
   - Add component labels
   - Use good-first-issue for newcomers

3. **Close/archive:**
   - Close resolved issues
   - Close duplicates with reference
   - Let stale bot handle inactive issues

---

## 🔍 Monitoring

### Check Workflow Status

```bash
# View recent workflow runs
gh run list

# View specific workflow
gh run view <run-id>

# Watch workflow in real-time
gh run watch
```

### Common Issues

#### CI Failing on PR

1. **Check logs:**
   ```bash
   gh run view <run-id> --log
   ```

2. **Common causes:**
   - Tests failing
   - Clippy warnings
   - Formatting issues
   - Security vulnerabilities

3. **Fix locally:**
   ```bash
   cargo test
   cargo clippy --fix
   cargo fmt
   cargo audit
   ```

#### PR Checks Not Running

1. **Verify triggers:**
   - Check workflow `on:` conditions
   - Ensure PR targets correct branch

2. **Re-run checks:**
   - Push new commit
   - Or use GitHub UI to re-run

3. **Check permissions:**
   - Verify Actions enabled
   - Check workflow permissions

---

## 📚 Additional Resources

### Documentation
- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Branch Protection Rules](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches)
- [CODEOWNERS Documentation](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners)

### Project Documentation
- [Contributing Guide](../docs/CONTRIBUTING.md)
- [Security Policy](../SECURITY.md)
- [Code of Conduct](../CODE_OF_CONDUCT.md)

### Support
- [Open an Issue](https://github.com/rahulbsw/streamforge/issues/new/choose)
- [Discussions](https://github.com/rahulbsw/streamforge/discussions)
- [Support Guide](../SUPPORT.md)

---

## 🔄 Maintenance

### Regular Tasks

- **Weekly:** Review open PRs and issues
- **Monthly:** Review stale items
- **Quarterly:** Update workflows and check for Action updates
- **Annually:** Review and update branch protection rules

### Updating Workflows

1. **Test changes:**
   - Create branch
   - Modify workflow
   - Test on PR before merging

2. **Version updates:**
   - Keep Action versions current
   - Test after updating
   - Check for breaking changes

3. **Monitor:**
   - Check workflow success rates
   - Review execution times
   - Optimize slow jobs

---

**Last Updated:** April 2026  
**Maintainer:** @rahulbsw  
**Questions:** See [SUPPORT.md](../SUPPORT.md)
