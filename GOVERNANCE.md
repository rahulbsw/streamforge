# Project Governance

StreamForge is an open source project maintained by Rahul Jain with community
contributions. Its priorities are performance, reliability, usability,
security, and an inclusive community.

## Roles

### Maintainer

**Rahul Jain ([@rahulbsw](https://github.com/rahulbsw))** is the current
maintainer. The maintainer reviews and merges pull requests, triages issues,
sets project direction, releases versions, handles vulnerabilities, and
enforces the Code of Conduct. The maintainer has final authority to accept or
reject changes, grant or revoke commit access, and appoint maintainers.

### Committers

Commit access may be granted after multiple high-quality merged pull requests,
demonstrated knowledge of the project, active reviews, and sustained
participation. Committers review pull requests, triage issues, maintain quality,
and mentor contributors.

### Contributors

Anyone may open issues or discussions, submit code or documentation, support
other users, participate in decisions, and receive credit for their work.

## Decisions

- **Routine:** bug fixes, minor features, and documentation changes proceed by
  pull request with at least one approval and resolved feedback. A maintainer or
  committer may merge the approved change.
- **Major:** breaking APIs, major features, project direction, licensing, and
  governance changes require a public issue, at least seven days for community
  input, consideration of alternatives, a documented decision, and final
  maintainer approval.
- **Emergency:** critical bugs and security fixes may receive expedited review,
  merge, and release, followed by release-note disclosure when safe.

Technical disagreements should document evidence and trade-offs in an issue or
pull request, seek additional input, and defer to the maintainer’s final
decision. An appeal requires a new issue with new information.

## Reviews and releases

All code changes require review, passing tests and lints, appropriate tests,
and documentation where behavior changes. Small focused changes, clear commit
messages, and performance impact analysis are encouraged. Target review times
are 1–2 days for simple changes, 3–7 days for medium changes, and 1–2 weeks for
large changes, subject to maintainer availability.

StreamForge uses [Semantic Versioning](https://semver.org/): major releases may
break compatibility, minor releases add backward-compatible features, and
patch releases contain backward-compatible fixes. Patch releases are made as
needed, minor releases monthly or when ready, and major releases only when
necessary. The maintainer owns publishing; the
[release workflow](https://github.com/rahulbsw/streamforge/blob/main/.github/workflows/release.yml)
and [release notes](docs/releases/README.md) are the operational sources of
truth for validation, packaging, artifacts, containers, charts, and rollback.

## Communication and conduct

- [GitHub Issues](https://github.com/rahulbsw/streamforge/issues): bugs,
  feature proposals, and technical decisions.
- [GitHub Discussions](https://github.com/rahulbsw/streamforge/discussions):
  questions, ideas, community support, and show-and-tell.
- Pull requests: code, documentation, and review.
- `rahul.oracle.db@gmail.com`: vulnerabilities, Code of Conduct reports, and
  other private concerns.

Do not report vulnerabilities publicly; follow the
[Security Policy](SECURITY.md). Code of Conduct reports are investigated by the
maintainer and handled under the [Code of Conduct](CODE_OF_CONDUCT.md).

Contributors are credited in `CHANGELOG.md`, release notes, and GitHub’s
contributors page. Significant contributors may be recognized in the README,
granted committer status, or invited to maintain the project.

## Project changes

The [roadmap](ROADMAP.md) records planned direction. Governance amendments use
a pull request to this file, at least seven days of discussion, and maintainer
approval. All contributions must be compatible with the
[Apache License 2.0](LICENSE) and are submitted under that license.

This model draws on the
[Rust project](https://www.rust-lang.org/governance),
[Node.js](https://github.com/nodejs/node/blob/main/GOVERNANCE.md), and the
[Apache Software Foundation](https://www.apache.org/foundation/governance/).

Version 1.0. Last updated: 2025-03-09.
