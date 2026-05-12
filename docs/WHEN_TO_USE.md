---
title: When to Use StreamForge
nav_order: 4
---

# When to Use StreamForge

## Choose StreamForge When You Need

- selective replication to analytics or lake pipelines
- field-level filtering and transformation
- PII-safe replication to lower-trust environments
- a lighter operational footprint than Kafka Connect

## Choose MirrorMaker 2 When You Need

- active-active replication
- consumer offset synchronization
- topic and ACL mirroring
- Kafka Connect ecosystem integration

## Choose Arroyo When You Need

- stateful streaming SQL
- joins and richer window semantics
- a general stream processing engine

## Positioning Rule

Good claim:
- StreamForge is more capable than MirrorMaker 2 for selective replication and data shaping.

Bad claim:
- StreamForge is better than MirrorMaker 2 in every replication scenario.
