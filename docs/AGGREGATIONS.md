---
title: Aggregations
---

# Aggregations

Use destination-level aggregations when you want StreamForge to turn a selected event stream into a smaller Kafka metrics stream. This is still selective replication: you filter and shape events first, then emit derived rollups to a destination topic.

## What Is Supported

### Window Type

- `tumbling`

### Metrics

- `count`
- `sum`
- `avg`
- `approx_distinct`
- `quantiles`

Aggregations live inside a routing destination:

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "orders-metrics-1m"
      filter: "/event_type,==,order_completed"
      transform: "CONSTRUCT:region=/region:customer_id=/customer_id:amount=/amount"
      aggregation:
        group_by:
          - name: region
            path: "/region"
        window:
          type: tumbling
          size_seconds: 60
          emit_interval_seconds: 5
        metrics:
          - name: order_count
            op: count
          - name: gross_amount
            op: sum
            path: "/amount"
          - name: avg_amount
            op: avg
            path: "/amount"
          - name: unique_customers
            op: approx_distinct
            path: "/customer_id"
          - name: amount_quantiles
            op: quantiles
            path: "/amount"
            percentiles: [0.5, 0.95, 0.99]
```

## Execution Model

- Aggregations run after the destination `filter` and value `transform`.
- Aggregated records are emitted to the destination `output` topic.
- The emitted record key is the canonical JSON encoding of the ordered `group_by` name/value pair list, for example `[{"name":"region","value":"us"}]`.
- `quantiles` outputs use the currently implemented key format: `p0.5`, `p0.95`, `p0.99`.

## Output JSON Shape

Each emitted aggregate record has this structure:

```json
{
  "window": {
    "start_ms": 1715472000000,
    "end_ms": 1715472060000,
    "type": "tumbling",
    "size_seconds": 60
  },
  "group": {
    "region": "us"
  },
  "metrics": {
    "order_count": 42,
    "gross_amount": 10425.5,
    "avg_amount": 248.22619047619048,
    "unique_customers": 39.8,
    "amount_quantiles": {
      "p0.5": 199.0,
      "p0.95": 489.0,
      "p0.99": 915.0
    }
  }
}
```

Notes:

- `approx_distinct` is sketch-based, so it returns an estimate.
- `quantiles` returns an object keyed by the configured percentile list.

## Out of Scope

This first aggregation lane intentionally does not include:

- joins
- SQL
- sliding windows
- session windows
- durable aggregation state
- key/header/timestamp transforms on aggregation destinations

If you need broad stateful stream processing semantics, use a tool built for that job. StreamForge keeps the scope narrow: selective replication plus lightweight derived metrics.

## Validation

Validate the published examples before running them:

```bash
cargo run --quiet --bin streamforge-validate -- examples/aggregation/orders-windowed-metrics.yaml
cargo run --quiet --bin streamforge-validate -- examples/aggregation/orders-quantiles.yaml
```
