# Project Governance

## Overview

Streamforge is an open source project maintained by Rahul Jain with contributions from the community. This document describes how the project is governed and how decisions are made.

## Project Goals

1. **Performance**: Maintain best-in-class performance for Kafka streaming
2. **Reliability**: Ensure production-grade stability and correctness
3. **Usability**: Keep the tool simple, well-documented, and easy to use
4. **Security**: Follow security best practices and respond quickly to vulnerabilities
5. **Community**: Build an inclusive, welcoming community

## Roles and Responsibilities

### Maintainer

**Current Maintainer**: Rahul Jain (@rahulbsw)

**Responsibilities:**
- Review and merge pull requests
- Triage and respond to issues
- Make final decisions on features and direction
- Release new versions
- Manage security vulnerabilities
- Enforce Code of Conduct

**Authority:**
- Accept or reject pull requests
- Grant or revoke commit access
- Appoint new maintainers

### Contributors

Anyone who contributes to the project through:
- Code contributions (pull requests)
- Documentation improvements
- Bug reports
- Feature requests
- Community support

**Rights:**
- Submit pull requests
- Open issues
- Participate in discussions
- Receive credit for contributions

### Committers

Contributors who have made significant contributions may be granted commit access.

**Criteria:**
- Multiple high-quality merged PRs
- Understanding of codebase and project goals
- Active participation in code reviews
- Demonstrated commitment to project

**Responsibilities:**
- Review pull requests
- Triage issues
- Help maintain code quality
- Mentor new contributors

## Decision Making

### Routine Decisions

Day-to-day decisions (bug fixes, minor features, documentation updates) can be made by any maintainer or committer through pull requests.

**Process:**
1. Open pull request
2. Wait for review (at least 1 approval)
3. Address feedback
4. Merge when approved

### Major Decisions

Major changes (breaking changes, new features, architectural changes) require broader discussion.

**Process:**
1. Open GitHub issue for discussion
2. Allow at least 7 days for community input
3. Consider feedback and alternatives
4. Maintainer makes final decision
5. Document decision in issue

**Examples of major decisions:**
- Breaking API changes
- New major features
- Changes to project direction
- License changes
- Governance changes

### Emergency Decisions

Security fixes and critical bugs can be fast-tracked.

**Process:**
1. Create PR with fix
2. Get expedited review
3. Merge and release quickly
4. Announce in release notes

## Release Process

### Version Numbering

Streamforge follows [Semantic Versioning](https://semver.org/):

- **Major** (X.0.0): Breaking changes
- **Minor** (0.X.0): New features, backward compatible
- **Patch** (0.0.X): Bug fixes, backward compatible

### Release Cadence

- **Patch releases**: As needed for bug fixes (weekly if needed)
- **Minor releases**: Monthly or when significant features are ready
- **Major releases**: When breaking changes are necessary (rare)

### Release Checklist

1. Update CHANGELOG.md
2. Run all tests: `cargo test`
3. Run benchmarks: `cargo bench`
4. Update version in Cargo.toml
5. Create git tag
6. Build release binary
7. Publish to crates.io
8. Create GitHub release
9. Update documentation
10. Announce release

## Contribution Process

See [CONTRIBUTING.md](docs/CONTRIBUTING.md) for detailed contribution guidelines.

**Summary:**
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run `cargo test` and `cargo clippy`
6. Submit pull request
7. Respond to review feedback
8. Celebrate when merged! 🎉

## Code Review

### Review Standards

All code must be reviewed before merging:

**Required:**
- ✅ Passes all tests
- ✅ Passes clippy lints
- ✅ Includes tests for new features
- ✅ Updates documentation if needed
- ✅ Follows Rust style guidelines

**Encouraged:**
- Clear commit messages
- Small, focused changes
- Well-documented code
- Performance considerations

### Review Timeline

- **Simple changes**: 1-2 days
- **Medium changes**: 3-7 days
- **Large changes**: 1-2 weeks

Reviewers will make best effort to review promptly. Contributors can ping reviewers after reasonable waiting time.

## Conflict Resolution

### Technical Disagreements

1. Discuss in GitHub issue or PR
2. Present technical arguments and tradeoffs
3. Seek additional input from community
4. Maintainer makes final decision

### Code of Conduct Violations

1. Report to rahul.oracle.db@gmail.com
2. Maintainer investigates
3. Action taken per Code of Conduct
4. Decision communicated to parties

### Appeals

Decisions can be appealed by:
1. Opening a new GitHub issue
2. Providing new information or arguments
3. Maintainer reconsiders

## Communication Channels

### GitHub Issues

Primary channel for:
- Bug reports
- Feature requests
- Technical discussions

### GitHub Discussions

For:
- General questions
- Ideas and brainstorming
- Community support
- Show and tell

### Pull Requests

For:
- Code contributions
- Documentation updates
- Code reviews

### Email

For:
- Security vulnerabilities
- Code of Conduct violations
- Private concerns

Contact: rahul.oracle.db@gmail.com

## Recognition

### Contributors

All contributors are recognized in:
- CHANGELOG.md for their contributions
- GitHub contributors page
- Release notes

### Major Contributors

Contributors with significant impact may be:
- Mentioned in README.md
- Granted committer status
- Invited to maintainer team

## Roadmap

See [ROADMAP.md](ROADMAP.md) for planned features and direction.

Major features are discussed in GitHub issues before implementation.

## License

All contributions must be compatible with Apache License 2.0.

By contributing, you agree to license your contribution under the Apache License 2.0.

## Amendments

This governance document can be amended through:
1. Pull request to this file
2. Discussion period (at least 7 days)
3. Maintainer approval

## References

This governance model is inspired by:
- [Rust Project Governance](https://www.rust-lang.org/governance)
- [Node.js Governance](https://github.com/nodejs/node/blob/main/GOVERNANCE.md)
- [Apache Software Foundation](https://www.apache.org/foundation/governance/)

---

**Last Updated**: 2025-03-09
**Version**: 1.0
