---
title: Compatibility
nav_order: 5
---

# Compatibility

## Broker Targets

| Target | Status | Notes |
|--------|--------|-------|
| Apache Kafka | Primary | Baseline deployment target |
| Redpanda | Compatibility target | Validate visible examples and docs against Kafka-compatible APIs |

## Validation Expectations

- Validate published standalone StreamForge config examples with `streamforge-validate`
- Validate Kubernetes and operator examples with their own schema and apply checks
- Document any broker-specific caveats instead of implying perfect parity

## Current Compatibility Promise

StreamForge is Kafka-first, and Redpanda is a compatibility target for Kafka-compatible replication workflows. Support claims should stay scoped to the visible example and validation matrix in the repo. If a workflow depends on broker-specific features outside that matrix, document the limitation before claiming support.
