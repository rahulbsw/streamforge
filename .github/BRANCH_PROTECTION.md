# Branch Protection Rules

This document describes the recommended branch protection rules for this repository. These settings must be configured in GitHub's UI under **Settings → Branches → Branch protection rules**.

---

## Quick Setup

### 1. Navigate to Branch Protection Settings

1. Go to repository **Settings**
2. Click **Branches** in the left sidebar
3. Click **Add branch protection rule**
4. Enter branch name pattern: `main`

### 2. Apply Protection Rules

Copy and apply the settings from the sections below.

---

## Protection Rules for `main` Branch

### ✅ **Require Pull Request Reviews**

**Enable:** ✅ **Require a pull request before merging**

Settings:
- **Required approvals:** `1`
- ✅ **Dismiss stale pull request approvals when new commits are pushed**
- ✅ **Require review from Code Owners** (if CODEOWNERS file exists)
- ❌ **Restrict who can dismiss pull request reviews** (optional, for large teams)
- ❌ **Allow specified actors to bypass required pull requests** (not recommended)
- ✅ **Require approval of the most recent reviewable push**

**Rationale:** Ensures all code is reviewed before merging, maintaining code quality and catching issues early.

---

### ✅ **Require Status Checks to Pass**

**Enable:** ✅ **Require status checks to pass before merging**

Settings:
- ✅ **Require branches to be up to date before merging**

**Required Status Checks** (select all that apply):
```
✅ Rust - Test
✅ Rust - Build (ubuntu-latest)
✅ Rust - Build (macos-latest)
✅ Rust - Build (windows-latest)
✅ Rust - Security Audit (allow to continue on error)
✅ Operator - Test
✅ UI - Test & Lint
✅ Helm - Validate Chart
✅ Validate PR Metadata
✅ Dependency Review
✅ Require Label
✅ Check Documentation Updated (warning only)
✅ Check Changelog Updated (warning only)
```

**Rationale:** Ensures all CI checks pass, preventing broken code from being merged.

---

### ✅ **Require Conversation Resolution**

**Enable:** ✅ **Require conversation resolution before merging**

**Rationale:** Ensures all PR comments and discussions are addressed before merging.

---

### ✅ **Require Signed Commits**

**Enable:** ✅ **Require signed commits**

**Rationale:** Ensures commits are cryptographically signed, verifying author identity and preventing commit tampering.

**Setup Guide for Contributors:**
```bash
# Configure GPG key for signing
git config --global user.signingkey <YOUR_GPG_KEY_ID>
git config --global commit.gpgsign true

# Or use SSH signing (GitHub supported)
git config --global gpg.format ssh
git config --global user.signingkey ~/.ssh/id_ed25519.pub
```

---

### ✅ **Require Linear History**

**Enable:** ✅ **Require linear history**

**Rationale:** Enforces squash or rebase merging, maintaining a clean, linear git history.

**Merge Options:**
- ✅ Allow squash merging (recommended)
- ✅ Allow rebase merging
- ❌ Allow merge commits (not recommended for linear history)

---

### ✅ **Require Deployments to Succeed**

**Enable:** ❌ **Require deployments to succeed before merging** (optional)

**Rationale:** Only enable if you have automated deployment previews.

---

### ❌ **Lock Branch**

**Enable:** ❌ **Lock branch** (emergency only)

**Rationale:** Only use during critical incidents to prevent any changes.

---

### ✅ **Do Not Allow Bypassing**

**Enable:** ✅ **Do not allow bypassing the above settings**

**Rationale:** Ensures even administrators follow the same rules.

**Exception:** For emergency hotfixes, temporarily disable or use a hotfix process.

---

### ✅ **Restrict Force Pushes**

**Enable:** ✅ **Restrict who can push to matching branches**

**Allow:** `Specify who can push` → Add only: `CI/CD service accounts` (if needed)

**Rationale:** Prevents accidental or malicious history rewriting.

---

### ✅ **Restrict Deletions**

**Enable:** ✅ **Allow deletions** → **UNCHECK**

**Rationale:** Prevents accidental branch deletion.

---

## Additional Repository Settings

Navigate to **Settings → General**:

### Merge Button Settings

**Recommended configuration:**
- ✅ **Allow squash merging** (recommended)
  - Default to: `Default commit message`
- ✅ **Allow rebase merging** (optional)
- ❌ **Allow merge commits** (disable for cleaner history)

**Auto-merge:**
- ✅ **Allow auto-merge** (enables auto-merge after approvals)

**Auto-delete branches:**
- ✅ **Automatically delete head branches** (cleanup after merge)

---

## Protected Branch Patterns

### Additional Branches to Protect

| Branch Pattern | Protection Level | Notes |
|----------------|------------------|-------|
| `main` | Full protection | Production branch |
| `release/*` | Full protection | Release branches |
| `develop` | Medium protection | Development integration branch |
| `hotfix/*` | Medium protection | Emergency fixes |

---

## Setting Up Additional Patterns

### Protect Release Branches

**Pattern:** `release/*`

**Settings:**
- Same as `main` branch
- Additional: Require specific reviewers from release team
- Additional: Require deployment success (if applicable)

### Protect Hotfix Branches

**Pattern:** `hotfix/*`

**Settings:**
- Same as `main` but:
  - Allow bypass by repository administrators (for emergencies)
  - Required approvals: 2 (higher scrutiny)

---

## GitHub Actions Permissions

Navigate to **Settings → Actions → General**:

### Workflow Permissions

**Recommended:**
- ✅ **Read and write permissions**
- ✅ **Allow GitHub Actions to create and approve pull requests**

**Rationale:** Allows workflows to create automated PRs (dependency updates, etc.)

---

## Required Labels

Set up repository labels for the auto-labeling workflow:

### Size Labels
- `size/XS` - < 10 lines changed
- `size/S` - < 50 lines changed
- `size/M` - < 200 lines changed
- `size/L` - < 500 lines changed
- `size/XL` - 500+ lines changed

### Type Labels
- `bug` - Bug fixes
- `enhancement` - New features
- `documentation` - Documentation changes
- `maintenance` - Maintenance and refactoring
- `dependencies` - Dependency updates
- `breaking-change` - Breaking changes
- `security` - Security-related changes
- `performance` - Performance improvements

### Priority Labels
- `priority/critical` - Must be fixed ASAP
- `priority/high` - Important
- `priority/medium` - Normal priority
- `priority/low` - Nice to have

### Status Labels
- `needs-review` - Awaiting review
- `needs-changes` - Changes requested
- `approved` - Approved and ready to merge
- `on-hold` - Blocked or paused
- `wip` - Work in progress

### Component Labels
- `component/kafka` - Kafka-related
- `component/dsl` - Filter/Transform DSL
- `component/operator` - Kubernetes operator
- `component/ui` - Web UI
- `component/helm` - Helm charts
- `component/docker` - Docker/containers

### Area Labels
- `area/core` - Core application
- `area/config` - Configuration
- `area/security` - Security features

---

## Rulesets (New Feature)

GitHub now supports **Rulesets** as a more flexible alternative to branch protection rules.

### When to Use Rulesets

- More granular control over protections
- Apply rules to multiple branches with patterns
- Role-based bypass permissions
- Insights and compliance tracking

### Converting to Rulesets

1. Navigate to **Settings → Rules → Rulesets**
2. Click **New ruleset → New branch ruleset**
3. Configure targeting (branch patterns)
4. Add rules (same as branch protection settings above)
5. Configure bypass permissions if needed

---

## Enforcement Timeline

### Phase 1: Soft Launch (Weeks 1-2)
- Enable basic protections:
  - Require PR reviews (1 approval)
  - Require CI to pass
  - Require conversation resolution
- **Goal:** Get team comfortable with PR process

### Phase 2: Full Protection (Weeks 3-4)
- Add remaining protections:
  - Require signed commits
  - Require linear history
  - Require Code Owner reviews
- **Goal:** Full protection enabled

### Phase 3: Optimize (Ongoing)
- Monitor for pain points
- Adjust required checks
- Add/remove specific checks based on reliability
- **Goal:** Balance safety and velocity

---

## Verification Checklist

Use this checklist to verify branch protection is properly configured:

### Main Branch Protection
- [ ] Branch protection rule exists for `main`
- [ ] Requires pull request reviews (minimum 1)
- [ ] Dismisses stale approvals
- [ ] Requires Code Owner review
- [ ] Requires status checks to pass
- [ ] Requires branches to be up to date
- [ ] All CI checks are required
- [ ] Requires conversation resolution
- [ ] Requires signed commits
- [ ] Requires linear history
- [ ] Does not allow bypassing
- [ ] Restricts force pushes
- [ ] Prevents branch deletion

### Repository Settings
- [ ] Only squash or rebase merging allowed
- [ ] Auto-merge enabled
- [ ] Auto-delete head branches enabled
- [ ] Actions have appropriate permissions

### Labels
- [ ] All required labels created
- [ ] Auto-labeling workflow enabled

### Testing
- [ ] Try creating PR without required checks → Should be blocked
- [ ] Try merging without approval → Should be blocked
- [ ] Try force pushing to main → Should be blocked
- [ ] Try deleting main branch → Should be blocked

---

## Bypassing Protections (Emergency Only)

### When to Bypass
- **Critical production incident** requiring immediate hotfix
- **Broken CI** that's blocking all PRs incorrectly
- **GitHub outage** affecting required checks

### How to Bypass

1. **Temporary Disable:**
   - Go to **Settings → Branches**
   - Edit branch protection rule
   - Temporarily disable specific requirement
   - **IMPORTANT:** Re-enable after emergency

2. **Using Admin Override:**
   - Repository admins can merge despite protections (if not restricted)
   - **Document why** in PR description
   - **Create follow-up issue** to fix properly

3. **Emergency Hotfix Process:**
   ```bash
   # Create hotfix branch from main
   git checkout -b hotfix/critical-fix main

   # Make minimal fix
   git commit -S -m "hotfix: critical production issue"

   # Push and create PR
   git push -u origin hotfix/critical-fix

   # If protections must be bypassed:
   # 1. Document reason in PR
   # 2. Get approval from 2+ maintainers
   # 3. Temporarily disable protection
   # 4. Merge
   # 5. RE-ENABLE protection immediately
   # 6. Create post-mortem issue
   ```

---

## References

- [GitHub Branch Protection Documentation](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
- [GitHub Rulesets Documentation](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets)
- [Requiring Signed Commits](https://docs.github.com/en/authentication/managing-commit-signature-verification/about-commit-signature-verification)
- [GitHub Actions Permissions](https://docs.github.com/en/actions/security-guides/automatic-token-authentication)

---

**Last Updated:** April 2026  
**Maintained By:** @rahulbsw  
**Review Schedule:** Quarterly
