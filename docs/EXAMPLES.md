---
title: Examples
nav_order: 6
---

# Examples

Use these example packs when you want a validated starting point for StreamForge.

## Local Demo

- [Redpanda Demo](https://github.com/rahulbsw/streamforge/tree/main/examples/redpanda) - Fastest local walkthrough for selective replication to analytics and PII-safe topics

## Production-Oriented Configs

- [PII Redaction](https://github.com/rahulbsw/streamforge/blob/main/examples/production/pii-redaction.yaml) - Filter and reshape events before they cross trust boundaries
- [CDC to Data Lake](https://github.com/rahulbsw/streamforge/blob/main/examples/production/cdc-to-datalake.yaml) - Shape Debezium-style CDC topics for analytics and lake ingestion
- [Minimal Standalone Pipeline](https://github.com/rahulbsw/streamforge/blob/main/examples/configs/config.example.yaml) - Smallest standalone config for a single source and destination

## Operator and CRD Examples

- [Pipeline CRDs](https://github.com/rahulbsw/streamforge/tree/main/examples/pipelines) - Operator-backed manifests for Kubernetes deployment

## Validation

The promoted example configs on this page are validated with `streamforge-validate` and mirrored in CI so the public examples stay in sync with the current runtime parser and config schema.
