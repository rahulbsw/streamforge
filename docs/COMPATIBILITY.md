---
title: Compatibility
nav_order: 5
---

# Compatibility

## Broker Targets

| Target | Status | Notes |
|--------|--------|-------|
| Apache Kafka | Primary | Baseline deployment target |
| Redpanda | Explicitly supported | Validate examples and docs against Redpanda-compatible APIs |

## Validation Expectations

- Validate all published examples with `streamforge-validate`
- Keep at least one local Redpanda example pack under `examples/redpanda/`
- Document any broker-specific caveats instead of implying perfect parity

## Current Compatibility Promise

StreamForge is Kafka-first and ships explicit Redpanda examples. If a workflow depends on broker-specific features outside the current example and validation matrix, document the limitation before claiming support.
