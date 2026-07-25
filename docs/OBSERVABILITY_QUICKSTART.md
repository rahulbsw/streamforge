---
title: Observability
nav_order: 1
parent: Operations
---

# Observability

StreamForge exposes Prometheus metrics and a simple process health endpoint.
Use them with Kafka consumer-group and destination-topic observations to monitor
the full pipeline.

## Enable the endpoints

```yaml
observability:
  metrics_enabled: true
  metrics_port: 9090
  metrics_path: /metrics
  lag_monitoring_enabled: true
  lag_monitoring_interval_secs: 30
```

Start StreamForge:

```bash
CONFIG_FILE=config.yaml target/release/streamforge
```

The HTTP server listens on all interfaces. It serves `/metrics` and `/health`;
the current server route is `/metrics` even if a different `metrics_path` value
is configured.

Test from the same private network:

```bash
curl --fail http://streamforge.internal:9090/health
curl --fail http://streamforge.internal:9090/metrics
```

`/health` returns `OK` when the HTTP process responds. It is not a readiness
check for source consumption or destination delivery.

## Keep the endpoint private

The metrics server does not provide TLS or authentication. Do not expose it to
the public internet.

- In Kubernetes, use a `ClusterIP` service and restrict ingress to the
  monitoring namespace with a `NetworkPolicy`.
- In Docker, publish the port only on a private interface or scrape it through
  a private container network.
- If a proxy is required, add authentication and TLS there.

Metrics labels and operational values can reveal topic names and traffic
patterns. Apply the same access controls used for other production telemetry.

## Prometheus scrape configuration

```yaml
scrape_configs:
  - job_name: streamforge
    static_configs:
      - targets:
          - streamforge.internal:9090
    scrape_interval: 15s
    scrape_timeout: 10s
```

For Kubernetes, a `ServiceMonitor` can select the private metrics service when
the Prometheus Operator is installed.

## Useful metrics

### Pipeline flow

```promql
rate(streamforge_messages_consumed_total[5m])
```

```promql
sum by (destination) (
  rate(streamforge_messages_delivered_total[5m])
)
```

`streamforge_messages_delivered_total` counts successful Kafka delivery
acknowledgements. Prefer it over enqueue or processor completion when measuring
delivery.

### Errors

```promql
sum by (type) (
  rate(streamforge_processing_errors_total[5m])
)
```

```promql
sum by (destination) (
  rate(streamforge_filter_errors_total[5m])
)
```

```promql
sum by (destination) (
  rate(streamforge_transform_errors_total[5m])
)
```

### Lag

```promql
sum(streamforge_consumer_lag)
```

```promql
max by (topic, partition) (streamforge_consumer_lag)
```

Lag metrics appear after the consumer has partition assignments and the lag
monitor completes a collection interval.

### Processing latency

```promql
histogram_quantile(
  0.95,
  sum by (le, destination) (
    rate(streamforge_processing_duration_seconds_bucket[5m])
  )
)
```

### Saturation

```promql
streamforge_messages_in_flight
```

Pair application metrics with container CPU, throttling, memory, restart, and
network metrics from the runtime platform.

## Alert strategy

Use workload-specific service objectives rather than copied numeric thresholds.
At minimum, detect:

- StreamForge unavailable;
- source traffic present while broker-acknowledged delivery stops;
- sustained consumer-lag growth;
- processing errors or DLQ traffic;
- repeated restarts;
- memory approaching its limit;
- sustained CPU throttling;
- abnormal processing-latency changes.

Example availability rule:

```yaml
groups:
  - name: streamforge
    rules:
      - alert: StreamForgeUnavailable
        expr: up{job="streamforge"} == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: StreamForge metrics endpoint is unavailable
```

Choose the `for` duration and severity according to the pipeline objective.

## Validate the signal path

1. Confirm Prometheus reports the target as healthy.
2. Produce a controlled source record.
3. Observe the consumed counter.
4. Verify the destination record with an independent Kafka consumer.
5. Observe the delivered counter for that destination.
6. Confirm lag reflects the committed consumer-group position.
7. Send an intentionally rejected test record in a non-production pipeline and
   verify error and DLQ monitoring.
8. Stop the test instance and verify the availability alert.

If metrics disagree with Kafka offsets or destination records, treat Kafka as
the delivery source of truth and investigate instrumentation before publishing
performance results.

See [Operations](OPERATIONS.md) for incident workflows and
[Delivery guarantees](DELIVERY_GUARANTEES.md) for counter interpretation in
acknowledged and queued modes.
