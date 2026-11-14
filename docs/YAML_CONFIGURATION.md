---
title: Configuration
nav_order: 4
---

# YAML Configuration Guide

YAML is the recommended configuration format for StreamForge. It keeps routing, filters, transforms, headers, and observability settings readable in code review.

## Minimal Pipeline

```yaml
appid: streamforge
bootstrap: kafka:9092
input: source-topic
output: destination-topic
offset: latest
threads: 4
```

## Format Detection

StreamForge detects the config format from the file extension.

```bash
CONFIG_FILE=config.yaml ./target/release/streamforge
CONFIG_FILE=config.yml ./target/release/streamforge
CONFIG_FILE=config.json ./target/release/streamforge
```

Use YAML for hand-written pipeline configs. JSON is still accepted for generated configs.

## Routing Shape

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "analytics-orders"
      description: "US orders over the analytics threshold"
      filter: "and($region == 'us', $amount >= 100)"
      transform: "construct(order_id=$order_id, customer_id=$customer.id, amount=$amount)"
      key_transform: "$order_id"
      headers:
        x-pipeline: "orders-analytics"
```

## V2 DSL in YAML

StreamForge docs use V2 DSL only.

Use compact expressions for short routes:

```yaml
filter: "$customer.tier == 'premium'"
transform: "construct(user_id=$user.id, tier=$customer.tier)"
key_transform: "$user.id"
```

Use folded strings for longer expressions:

```yaml
filter: >
  and(
    $event_type == 'order_completed',
    $customer.tier == 'premium',
    $order.amount >= 100
  )
transform: >
  construct(
    order_id=$order.id,
    customer_id=$customer.id,
    amount=$order.amount,
    region=$region,
    created_at=$event_time
  )
```

## Multi-Destination Example

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "validated-users"
      description: "Users with valid email format"
      filter: "regex(field('/user/email'), '^[^@]+@[^@]+\\.[^@]+$')"
      transform: "construct(id=$user.id, email=$user.email, name=$user.name)"
      key_transform: "$user.id"

    - output: "premium-orders"
      description: "Confirmed high-value orders"
      filter: "and($order.total > 500, $order.status == 'confirmed')"
      transform: "construct(order_id=$order.id, total=$order.total, customer=$customer.email)"
      key_transform: "$order.id"

    - output: "partner-safe-events"
      description: "Approved fields only for partner systems"
      filter: "and($consent.third_party == true, exists('/properties/non_pii'))"
      transform: "construct(event=$event_type, properties=$properties.non_pii)"
      key_transform: "hash('SHA256', $user.id)"
```

## PII Redaction Pattern

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "user-events-analytics"
      filter: "exists('/user/id')"
      transform: "construct(user_id=$user.id, event_type=$event_type, timestamp=$timestamp, region=$region)"
      key_transform: "hash('SHA256', $user.id)"

    - output: "events-third-party"
      filter: "$consent.third_party == true"
      transform: "construct(event=$event_type, properties=$properties.non_pii)"
      key_transform: "hash('SHA256', $user.id)"
```

## CDC Pattern

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "datalake-orders"
      filter: "or($payload.op == 'c', $payload.op == 'u')"
      transform: "field('/payload/after')"
      key_transform: "$payload.after.id"

    - output: "datalake-orders-deleted"
      filter: "$payload.op == 'd'"
      transform: "construct(id=$payload.before.id, deleted_at=$payload.ts_ms)"
      key_transform: "$payload.before.id"

    - output: "datalake-schema-changes"
      filter: "$payload.op == 's'"
      transform: "field('/payload')"
```

## Performance

```yaml
threads: 8
compression:
  compression_type: raw
  compression_algo: zstd
performance:
  fetch_min_bytes: 1048576
  fetch_max_wait_ms: 100
  max_partition_fetch_bytes: 5242880
  batch_size: 2000
  linger_ms: 20
```

## Retry and DLQ

```yaml
retry:
  max_attempts: 3
  initial_delay_ms: 100
  max_delay_ms: 30000
  multiplier: 2.0
  jitter: 0.1

dlq:
  enabled: true
  topic: "streamforge-dlq"
  include_original_headers: true
  include_stack_trace: false
  max_dlq_retries: 3
```

## Observability

```yaml
observability:
  metrics_enabled: true
  metrics_port: 8080
  metrics_path: "/metrics"
  lag_monitoring_enabled: true
  lag_monitoring_interval_secs: 30
```

## Validation

```bash
cargo run --quiet --bin streamforge-validate -- config.yaml
```

Validation checks the YAML shape, routing destinations, filters, transforms, key transforms, and supported operational settings before you deploy.
