---
title: Quickstart
nav_order: 2
---

# Quick Start Guide

## Goal

In five minutes, run StreamForge locally and replicate one source topic into:
- `analytics-orders` for downstream analytics
- `pii-safe-orders` for a lower-trust consumer

## 1. Start Redpanda

```bash
docker compose -f examples/redpanda/docker-compose.yml up -d
```

## 2. Validate the Demo Config

```bash
cargo run --quiet --bin streamforge-validate -- examples/redpanda/selective-replication.yaml
```

## 3. Run StreamForge

```bash
CONFIG_FILE=examples/redpanda/selective-replication.yaml ./target/release/streamforge
```

## 4. Produce Sample Orders

Send one record with `region=us` and `amount>=100`, then verify:
- `analytics-orders` gets the shaped analytics payload
- `pii-safe-orders` gets a non-PII summary payload keyed by a hash of `customer.email`
