---
title: Operations
nav_order: 7
has_children: true
---

# Operate StreamForge

This runbook focuses on signals and procedures that apply to both container and
Kubernetes deployments. Establish alert thresholds from the normal behavior and
service objectives of each pipeline; hard-coded global thresholds are not
meaningful across different payloads, brokers, and traffic patterns.

## First-response checklist

1. Confirm the process or pod is running and inspect recent restarts.
2. Query the private `/health` endpoint.
3. Check source consumer-group assignment and lag with Kafka tooling.
4. Compare consumed records, broker-acknowledged deliveries, processing errors,
   and in-flight work.
5. Inspect recent configuration or image changes.
6. Check CPU, memory, network, and Kafka broker health.
7. Sample the DLQ without copying sensitive payloads into tickets or logs.

For Kubernetes:

```bash
kubectl get pods -n streamforge -o wide
kubectl logs -n streamforge deployment/streamforge --tail=200
kubectl top pods -n streamforge
kubectl get events -n streamforge --sort-by=.lastTimestamp
```

For Kafka:

```bash
kafka-consumer-groups --bootstrap-server kafka.internal:9092 \
  --describe --group PIPELINE_APPID
```

## Core service indicators

Monitor these together:

| Signal | What it answers |
|---|---|
| `streamforge_messages_consumed_total` | Is StreamForge receiving source records? |
| `streamforge_messages_delivered_total` | Are records being acknowledged by destination Kafka? |
| `streamforge_processing_errors_total` | Are parse, processing, or Kafka errors occurring? |
| `streamforge_consumer_lag` | Is the pipeline keeping up with each source partition? |
| `streamforge_messages_in_flight` | Is application work accumulating? |
| `streamforge_processing_duration_seconds` | Is per-destination processing latency changing? |
| Process restarts and resource use | Is the runtime unhealthy or constrained? |

Use Kafka destination offsets or an independent consumer to verify end-to-end
delivery. Application counters alone do not prove that every expected source
record reached the intended destination.

The metrics endpoint has no authentication. Keep it private as described in
[Deployment](DEPLOYMENT.md).

## Alert design

Create alerts from service objectives and observed baselines:

- page when the process is unavailable or broker acknowledgements stop while
  source traffic continues;
- alert when lag grows for a sustained interval rather than on a universal
  message-count threshold;
- alert on processing errors and any DLQ traffic relative to input volume;
- alert before memory exhaustion or sustained CPU throttling;
- alert on repeated restarts and consumer-group rebalance loops;
- use latency percentiles only after confirming the histogram has traffic.

Example PromQL:

```promql
up{job="streamforge"} == 0
```

```promql
sum by (type) (rate(streamforge_processing_errors_total[5m]))
```

```promql
sum(streamforge_consumer_lag)
```

```promql
sum(rate(streamforge_messages_delivered_total[5m])) by (destination)
```

## Scaling safely

Kafka source partitions bound useful consumer parallelism for one consumer
group. Adding replicas beyond the available partitions leaves consumers idle.

Scale only after identifying the constraint:

- **CPU saturated:** add CPU or replicas, then remeasure.
- **CPU available and destination waits dominate:** test bounded processing
  concurrency.
- **Memory pressure:** reduce batch size, worker queue bounds, or queued producer
  depth before increasing memory.
- **One hot partition:** inspect key distribution; more replicas cannot divide a
  single partition.
- **Kafka latency or errors:** fix broker or network capacity before increasing
  application concurrency.

Every replica change triggers a consumer-group rebalance. Scale gradually and
watch lag, duplicates, and destination errors through the rebalance.

See [Performance](PERFORMANCE.md) for a controlled tuning method.

## Configuration changes

StreamForge loads its configuration at process start. Apply a changed
`ConfigMap` with a rolling restart; do not assume hot reload:

```bash
target/release/streamforge-validate config.yaml --fail-on-warnings
kubectl apply -f streamforge-config.yaml
kubectl rollout restart deployment/streamforge -n streamforge
kubectl rollout status deployment/streamforge -n streamforge
```

Treat these as high-risk:

- changing `appid`, because it selects a different consumer group;
- changing `offset` or resetting committed offsets;
- changing keys or partitioning;
- switching between acknowledged and queued delivery;
- enabling auto commit on a reliability-sensitive pipeline;
- disabling the DLQ or changing its topic;
- changing filter or transform logic.

For high-risk changes, deploy a separate pipeline identity, compare controlled
outputs, and define how to retire or replay the previous pipeline.

## Incident procedures

### Lag is growing

1. Determine whether all expected partitions are assigned.
2. Compare input rate with broker-acknowledged delivery rate.
3. Inspect CPU throttling, memory pressure, and destination latency.
4. Break lag down by partition to detect skew.
5. Check processing errors and DLQ traffic.
6. Scale or tune one control at a time, then confirm that lag is recovering.

### Deliveries stop

1. Check destination broker reachability, authentication, ACLs, and topic
   existence.
2. Inspect processing errors and librdkafka logs.
3. Confirm the producer queue is not saturated.
4. In queued mode, check delivered counters rather than enqueue completion.
5. Preserve source offsets and avoid resets until the recovery consequence is
   understood.

### Process restarts or is killed

1. Inspect the previous container logs and termination reason.
2. Check configuration validation, secret mounts, and certificate expiry.
3. Check for OOM kills and CPU throttling.
4. Confirm the metrics port can bind and is not already in use.
5. Restart only after preserving enough diagnostics to identify recurrence.

### DLQ traffic appears

1. Record the error type, source topic, partition, offset, and deployed
   configuration revision.
2. Determine whether the failure is data-specific or affects all records.
3. Correct the producer data or pipeline expression.
4. Test replay in an isolated topic.
5. Replay with an idempotency strategy and verify the destination.

## Offset recovery

Offset changes can replay or skip data. Stop every consumer in the group before
changing offsets, preview the Kafka command when supported, record the old
offsets, and obtain approval for the exact topic, partition, and target offset.

Never reset to `latest` as a generic incident fix. It intentionally skips the
backlog.

## Maintenance

- Validate backup copies of configuration without storing secrets in Git.
- Track certificate and credential rotation dates.
- Test clean shutdown, destination outage, DLQ outage, and consumer rebalance.
- Re-run workload tests after Kafka, StreamForge, instance, filter, transform,
  or partition changes.
- Keep image digests and configuration revisions in the deployment record.
- Review ACLs and private-network controls regularly.

Continue with [Observability](OBSERVABILITY_QUICKSTART.md),
[Delivery guarantees](DELIVERY_GUARANTEES.md), and
[Troubleshooting](TROUBLESHOOTING.md).
