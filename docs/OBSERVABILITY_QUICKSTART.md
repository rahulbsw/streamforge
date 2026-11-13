---
title: Observability
nav_order: 1
parent: Operations
---

# Observability

This is the current metrics, readiness, logging, and Kubernetes monitoring
guide. Historical design documents are archived and are not metric catalogs.

## Runtime configuration

```yaml
observability:
  metrics_enabled: true
  metrics_port: 9090
  metrics_path: /metrics
  metrics_bind_address: 0.0.0.0
  lag_monitoring_enabled: true
  lag_monitoring_interval_secs: 30
```

```bash
RUST_LOG=info \
STREAMFORGE_LOG_FORMAT=json \
CONFIG_FILE=config.yaml \
target/release/streamforge
```

`STREAMFORGE_LOG_FORMAT` accepts `text` or `json`. JSON emits one object per
line. `RUST_LOG` remains the tracing level/filter setting.

The server exposes:

- `GET /health` — process liveness; HTTP 200 with literal `OK`;
- `GET /ready` — structured runtime/Kafka readiness; HTTP 200 or 503;
- the configured metrics path — Prometheus text exposition.

```bash
curl --fail http://streamforge.internal:9090/health
curl --fail http://streamforge.internal:9090/ready
curl --fail http://streamforge.internal:9090/metrics
```

Readiness is intentionally separate from liveness. Kafka initialization,
metadata failures, consumer errors, and recovery update readiness without
changing `/health`.

## Logging safety and fields

Operational logs use structured fields such as pipeline, topic, partition,
offset, destination, retry attempt, delivery state, and error category when the
event provides them.

Payloads, credentials, message keys, headers, filter expressions, and transform
expressions are not logged by default. Do not add them to routine production
events. Treat logs as sensitive topology/operations data even with payload
redaction.

## Keep endpoints private

The observability server does not provide TLS or authentication:

- use a Kubernetes `ClusterIP` Service and NetworkPolicy;
- bind a local/container deployment to a private or loopback interface;
- add authentication and TLS at a trusted proxy when required.

Metric labels can expose topic/destination names. Use the same access controls
as other production telemetry.

## Metrics catalog

Catalog version: `v1` for the v1.2 track. Existing names remain compatible
through v1.x unless formally deprecated.

| Metric | Type | Labels | Meaning |
| --- | --- | --- | --- |
| `streamforge_build_info` | gauge | `version` | Build identity; value 1 |
| `streamforge_ready` | gauge | none | 1 ready, 0 not ready |
| `streamforge_uptime_seconds` | gauge | none | Process uptime |
| `streamforge_kafka_connections` | gauge | `type` | Active consumer/producer connection state |
| `streamforge_messages_consumed_total` | counter | none | Source records consumed |
| `streamforge_messages_produced_total` | counter | `destination` | Records accepted by destination processing |
| `streamforge_messages_delivered_total` | counter | `destination` | Broker-acknowledged records |
| `streamforge_messages_filtered_total` | counter | `destination`, `reason` | Records not delivered by a route |
| `streamforge_processing_errors_total` | counter | `type` | Parse, processing, or Kafka errors |
| `streamforge_processing_duration_seconds` | histogram | `destination` | Per-destination processing latency |
| `streamforge_batch_processing_duration_seconds` | histogram | none | Legacy batch duration |
| `streamforge_processing_rate_mps` | gauge | none | Current message processing rate |
| `streamforge_messages_in_flight` | gauge | none | Records currently processing |
| `streamforge_filter_evaluations_total` | counter | `destination`, `result` | Filter outcomes |
| `streamforge_filter_duration_seconds` | histogram | `filter_type` | Filter evaluation latency |
| `streamforge_filter_errors_total` | counter | `destination` | Filter errors |
| `streamforge_transform_operations_total` | counter | `destination`, `transform_type` | Transform operations |
| `streamforge_transform_duration_seconds` | histogram | `transform_type` | Transform latency |
| `streamforge_transform_errors_total` | counter | `destination`, `transform_type` | Transform errors |
| `streamforge_wasm_invocations_total` | counter | `module`, `kind`, `status` | Bounded UDF invocation outcomes |
| `streamforge_wasm_duration_seconds` | histogram | `module`, `kind` | UDF invocation duration |
| `streamforge_wasm_input_bytes` | histogram | `module`, `kind` | Serialized UDF input size |
| `streamforge_wasm_output_bytes` | histogram | `module`, `kind` | Serialized UDF output size |
| `streamforge_wasm_active_invocations` | gauge | `module`, `kind` | Active UDF invocations |
| `streamforge_wasm_compilations_total` | counter | `module`, `status` | Startup verification/compilation outcomes |
| `streamforge_wasm_compilation_duration_seconds` | histogram | `module` | Startup verification/compilation duration |
| `streamforge_key_transforms_total` | counter | `destination`, `operation` | Key operations |
| `streamforge_header_operations_total` | counter | `destination`, `operation` | Header operations |
| `streamforge_timestamp_operations_total` | counter | `destination`, `operation` | Timestamp operations |
| `streamforge_consumer_lag` | gauge | `topic`, `partition` | Lag per assigned partition |
| `streamforge_consumer_offset` | gauge | `topic`, `partition` | Current consumer position |
| `streamforge_consumer_high_watermark` | gauge | `topic`, `partition` | Broker high watermark |
| `streamforge_time_since_last_commit_seconds` | gauge | none | Time since offset commit |
| `streamforge_aggregation_updates_total` | counter | `destination`, `metric`, `status` | Aggregation updates |
| `streamforge_aggregation_windows_open` | gauge | `destination` | Active windows |
| `streamforge_aggregation_flushes_total` | counter | `destination`, `status` | Window flush outcomes |
| `streamforge_aggregation_records_emitted_total` | counter | `destination` | Aggregated records emitted |

Prometheus histogram families also export `_bucket`, `_sum`, and `_count`.

## Label-cardinality policy

- `destination`, `topic`, and `partition` come from configured/assigned
  topology and must remain bounded by the pipeline configuration.
- `type`, `reason`, `result`, `transform_type`, `kind`, `operation`, and
  `status` use code-defined enumerations. UDF `module` values come only from
  the bounded startup configuration.
- never use payload values, keys, headers, error strings, user IDs, request IDs,
  or credentials as metric labels.
- a new label or label value source requires a cardinality test and catalog
  update in the same release.

## Useful queries

Broker-acknowledged delivery:

```promql
sum by (destination) (
  rate(streamforge_messages_delivered_total[5m])
)
```

Errors:

```promql
sum by (type) (
  rate(streamforge_processing_errors_total[5m])
)
```

Lag:

```promql
sum(streamforge_consumer_lag)
```

p95 processing latency:

```promql
histogram_quantile(
  0.95,
  sum by (le, destination) (
    rate(streamforge_processing_duration_seconds_bucket[5m])
  )
)
```

## Kubernetes assets

When `monitoring.enabled=true`, the chart installs the metrics Service and
PrometheusRule. ServiceMonitor and Grafana dashboard creation retain their own
values toggles. The Prometheus Operator/Grafana must already be installed.

The operator configures pipeline containers with named port `metrics` on 9090,
readiness `/ready`, liveness `/health`, and JSON logging.

Validate the rendered chart, PromQL with `promtool`, Prometheus target, alert
state, and dashboard queries before release. Rendering alone does not prove a
live signal path.

## Signal-path release check

1. Confirm the target is up and `/ready` is 200.
2. Produce a controlled source record.
3. Observe consumed and broker-delivered counters.
4. Verify destination output independently from Kafka.
5. Interrupt Kafka and verify `/ready` becomes 503.
6. Restore Kafka and verify readiness recovers.
7. Exercise a non-production error/DLQ path.
8. Confirm alert and dashboard behavior.

Kafka offsets and destination records remain the delivery source of truth when
instrumentation disagrees.

See [Operations](OPERATIONS.md), [Troubleshooting](TROUBLESHOOTING.md), and
[Delivery guarantees](DELIVERY_GUARANTEES.md).
