---
name: Bug Report
about: Report a bug or unexpected behavior
title: '[BUG] '
labels: bug
assignees: ''
---

## Bug Description

A clear and concise description of what the bug is.

## Steps to Reproduce

1. Configure with '...'
2. Run command '...'
3. See error

## Expected Behavior

What you expected to happen.

## Actual Behavior

What actually happened.

## Current Evidence

Paste the exact command, file, status, or reproducible observation that proves
the current state. Redact credentials, payloads, and personal data.

## Configuration

```yaml
# Your config.yaml (sanitize sensitive data)
```

## Environment

- **StreamForge Version**: [e.g., 1.1.0]
- **OS**: [e.g., Ubuntu 22.04]
- **Rust Version**: [e.g., 1.75.0]
- **Kafka Version**: [e.g., 3.6.0]

## Logs

```
# Relevant log output
```

## Dependencies

List prerequisite issues, services, artifacts, environments, or write `None`.

## Acceptance Tests

- [ ] Reproduction fails before the fix and passes after it
- [ ] Relevant unit/integration/release checks pass

## Documentation Changes

List affected guides/examples/index links, or explain why none change.

## Cleanup Obligations

List obsolete code, tests, flags, dependencies, examples, or compatibility
paths to remove or migrate.

## Security Impact

State credential, authorization, data exposure, input-validation, dependency,
and network impact.

## Rollback Behavior

Describe how to return to the prior behavior and what state cannot be undone.

## Additional Context

Add any other context about the problem here.
