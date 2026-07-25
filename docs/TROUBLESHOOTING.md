---
title: Troubleshooting
nav_order: 3
parent: Operations
---

# Troubleshooting

Start with evidence from the running revision. Avoid changing offsets, scaling,
or relaxing security controls until the failure mode is clear.

## Collect a safe diagnostic snapshot

```bash
kubectl get pods -n streamforge -o wide
kubectl describe pod -n streamforge STREAMFORGE_POD
kubectl logs -n streamforge STREAMFORGE_POD --previous --tail=200
kubectl top pod -n streamforge STREAMFORGE_POD
kubectl get events -n streamforge --sort-by=.lastTimestamp
```

```bash
kafka-consumer-groups --bootstrap-server kafka.internal:9092 \
  --describe --group PIPELINE_APPID
```

```bash
curl --fail --silent http://PRIVATE_STREAMFORGE_ADDRESS:9090/health
curl --fail --silent http://PRIVATE_STREAMFORGE_ADDRESS:9090/metrics
```

Redact passwords, tokens, certificates, message values, and sensitive headers
before sharing configuration, logs, metrics labels, or DLQ samples.

## Process does not start

### Configuration error

Validate the same file mounted in the workload:

```bash
target/release/streamforge-validate config.yaml --fail-on-warnings
```

Confirm that `CONFIG_FILE` points to an existing readable `.yaml`, `.yml`, or
`.json` file. A missing path causes the current binary to fall back to a built-in
test configuration, so treat a “config file not found” warning as a deployment
failure.

### Kafka connection or authentication error

Verify:

- bootstrap hostname and port resolve from the workload;
- egress policy allows the broker and DNS;
- the configured security protocol matches the listener;
- certificate paths exist inside the container;
- the CA and client certificates are current;
- SASL mechanism and credentials match the broker;
- ACLs allow source reads, consumer-group access, and destination writes.

Do not disable hostname verification or expose Kafka publicly to bypass a
connection problem. See [Security](SECURITY_CONFIGURATION.md).

### Metrics server cannot bind

The server binds the configured port on `0.0.0.0`. Check for a port conflict:

```bash
kubectl logs -n streamforge STREAMFORGE_POD | rg "metrics|bind"
```

Use a private `ClusterIP` service or local port-forward. The endpoint has no
built-in authentication.

## Kubernetes workload problems

### `Pending`

Use `kubectl describe pod` and events to distinguish insufficient resources,
unbound volumes, scheduling constraints, and missing service accounts. Adjust
only the constraint reported by the scheduler.

### `ImagePullBackOff`

Verify the exact repository, tag or digest, registry credentials, node
architecture, and image pull policy. The repository contains Dockerfiles and a
local Helm chart; do not assume an image tag has been published.

### `CrashLoopBackOff`

Inspect previous logs and the termination reason:

- exit during startup: configuration, secret mount, or Kafka initialization;
- `OOMKilled`: memory limit or bounded-queue sizing;
- repeated readiness failures: remember that `/health` reports HTTP process
  health, not end-to-end Kafka delivery;
- immediate operator-created pod failures: compare the generated `ConfigMap`
  with the operator CR and runtime schema.

## Records are consumed but not delivered

1. Compare `streamforge_messages_consumed_total` with
   `streamforge_messages_delivered_total`.
2. Check `streamforge_processing_errors_total` by `type`.
3. Check destination topic, ACL, broker availability, and partition metadata.
4. Inspect filter-fail and filtered counters; a filter can intentionally remove
   records.
5. Sample the destination with an independent Kafka consumer.
6. If using queued delivery, remember that enqueue completion precedes broker
   acknowledgement.

For a single-destination pipeline, use broker acknowledgements and destination
Kafka offsets as the authoritative delivery signals.

## Consumer lag grows

Break the lag down by partition:

```bash
kafka-consumer-groups --bootstrap-server kafka.internal:9092 \
  --describe --group PIPELINE_APPID
```

Then check:

- whether every partition has an active owner;
- source key skew and hot partitions;
- CPU throttling and memory pressure;
- processing duration and error rate;
- destination broker latency and retry activity;
- batch, worker queue, and producer in-flight bounds;
- repeated group rebalances.

More replicas help only while unassigned source partitions remain. Change one
tuning value at a time and confirm that lag begins to recover.

## Duplicate records

Duplicates are expected in an at-least-once profile when delivery succeeds but
the source offset is not committed before a crash or rebalance.

Check for:

- restarts between destination delivery and offset commit;
- commit failures;
- consumer-group rebalances;
- manual offset resets;
- producer retries after an ambiguous acknowledgement;
- multiple pipelines writing the same destination.

Use a stable event identifier and make downstream processing idempotent. See
[Delivery guarantees](DELIVERY_GUARANTEES.md).

## Records enter the DLQ

Read only enough metadata to classify the problem:

```bash
kafka-console-consumer --bootstrap-server kafka.internal:9092 \
  --topic streamforge-dlq \
  --property print.headers=true \
  --max-messages 1
```

Common categories:

- JSON parse failure: verify the producer contract and tombstone handling;
- filter or transform failure: validate the expression against a representative
  payload;
- producer failure: inspect destination connectivity, ACLs, and broker health;
- DLQ delivery failure: restore the DLQ topic or broker before restarting a
  manual-commit pipeline.

Do not purge or replay the DLQ as a diagnostic step. Correct the cause, test the
replay into an isolated destination, then execute an approved replay plan.

## Memory pressure

Inspect the container limit and workload shape. Memory scales with message size,
batch size, processing concurrency, per-worker queue capacity, destination
fan-out, and queued producer depth.

Reduce bounded concurrency controls before increasing them:

```yaml
performance:
  consumer_batch_size: 50
  parallelism_factor: 2
  worker_queue_capacity: 256
  producer_max_in_flight: 1000
```

These are diagnostic examples, not universal production values. Re-test latency,
lag, and delivery behavior after each change.

## CPU is high or throughput regresses

Profile the target workload before choosing an optimization. Check:

- JSON payload size and parsing cost;
- regex and compound filters;
- transforms and destination fan-out;
- compression;
- Kafka wait time;
- CPU throttling;
- changes in broker, network, image, or configuration.

Use [Performance](PERFORMANCE.md) to create a controlled comparison. Do not use
an old headline throughput number as an expected value.

## Offset recovery

Before changing offsets:

1. stop all consumers in the group;
2. record current offsets and log-end offsets;
3. identify the exact topic and partitions;
4. preview the proposed reset;
5. document whether records will replay or be skipped;
6. obtain approval and execute once;
7. restart and verify destination results.

An offset reset is a data operation, not a routine restart procedure.

## Escalation bundle

Provide:

- StreamForge commit, image tag, and digest;
- redacted configuration and validation output;
- deployment revision and recent changes;
- source and destination Kafka versions and topology;
- pod status, termination reason, and relevant logs;
- consumer-group assignment and lag by partition;
- a bounded metrics snapshot;
- exact reproduction steps and timestamps.

Open a GitHub issue only after removing credentials and payload data. Link to the
repository through the “StreamForge on GitHub” navigation item.
