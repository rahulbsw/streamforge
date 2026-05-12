# GitHub Automation and Community Config

This directory holds repository automation and contribution metadata for StreamForge.

User-facing docs live here instead:
- [../README.md](../README.md) for the project overview
- [../docs/index.md](../docs/index.md) for the published documentation site

## What Lives Here

- `workflows/` - CI, Docker, release, PR checks, and Pages deploy
- `ISSUE_TEMPLATE/` - issue intake templates
- `CODEOWNERS` - review ownership rules
- `labeler.yml` - file-based label mapping
- `pull_request_template.md` - PR checklist and structure
- `BRANCH_PROTECTION.md` - branch policy notes

## Maintainer Notes

- Keep this directory focused on GitHub behavior, not product positioning.
- If public messaging changes, update the root `README.md` and `docs/` first.
- If GitHub Pages output changes, review `.github/workflows/pages.yml`.
