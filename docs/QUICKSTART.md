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
CONFIG_FILE=examples/redpanda/selective-replication.yaml \
  cargo run --release --bin streamforge
```

Leave StreamForge running in this terminal. Open a second terminal for the remaining steps.

## 4. Produce Sample Orders

Create the demo topics:

```bash
docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic create raw-orders analytics-orders pii-safe-orders
```

Produce one order that matches both destinations:

```bash
printf '%s\n' \
  '{"order_id":"ord-1001","customer":{"id":"cust-42","email":"alice@example.com"},"amount":125,"region":"us","created_at":"2026-05-12T15:04:05Z"}' \
  | docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
      rpk topic produce raw-orders
```

Verify the analytics-shaped payload:

```bash
docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic consume analytics-orders -n 1 --offset start
```

Verify the PII-safe summary payload:

```bash
docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic consume pii-safe-orders -n 1 --offset start
```

Expected result:
- `analytics-orders` contains `order_id`, `customer_id`, `amount`, `region`, and `created_at`
- `pii-safe-orders` contains `order_id`, `amount`, `region`, and `created_at`
- the `pii-safe-orders` record key is a SHA-256 hash of `customer.email`
