---
title: Delivery guarantees
nav_order: 5
parent: Usage Guide
---

# Delivery guarantees

StreamForge exposes different reliability and scheduling controls. They are not
interchangeable: stronger throughput-oriented modes deliberately give up
features needed to associate a failed Kafka acknowledgement with its source
record.

## What the modes mean

| Configuration | Processing and delivery behavior | Important limitation |
|---|---|---|
| Default auto commit + `acknowledged` delivery | Each send awaits Kafka's delivery result; Kafka commits offsets on its own interval | A commit is not coordinated with completion of application processing |
| Manual commit + `legacy_batch` + `acknowledged` delivery | A batch is committed only after every record in that batch completes successfully | A crash after delivery and before commit can produce duplicates |
| Auto commit + `partition_ordered` | Each source partition is assigned to a bounded FIFO worker lane | Explicit rebalance-safe offset coordination is not implemented |
| Auto commit + `queued` delivery | Processing returns after librdkafka accepts a record; acknowledgements are tracked in the background | Auto commit, one processing attempt, and a disabled DLQ are required |

The default configuration uses auto commit for backward compatibility. Do not
describe the default as strictly at-least-once or exactly-once.

StreamForge does not implement Kafka transactions across source offsets and
destination records. Exactly-once delivery is therefore not a current
guarantee.

## At-least-once operating profile

For pipelines that must not commit a batch before every destination send has
been acknowledged, use the legacy batch processor with manual commits:

```yaml
commit_strategy:
  manual_commit: true
  commit_mode: sync

performance:
  processing_mode: legacy_batch
  producer_delivery_mode: acknowledged

retry:
  max_attempts: 3

dlq:
  enabled: true
  topic: streamforge-dlq
  max_dlq_retries: 3
```

In this profile:

1. StreamForge consumes a bounded batch.
2. Records in the batch may process concurrently.
3. Each destination send waits for Kafka's delivery result.
4. Recoverable failures are retried according to the retry policy.
5. Errors whose recovery action is DLQ are acknowledged only after the DLQ send
   succeeds.
6. StreamForge commits the consumer state only when the whole batch succeeds.
7. A failed batch or exhausted commit retry stops the pipeline.

This is an at-least-once operating profile, so downstream consumers must tolerate
duplicates. A record can be delivered and then replayed if StreamForge stops
before its source offset is committed.

`commit_interval_ms` is present in the configuration schema, but the current
legacy loop commits after successful processing batches. Size batches with the
`performance.consumer_batch_size` and
`performance.consumer_batch_timeout_ms` controls.

## Ordering

- Kafka defines order within a source partition, not across partitions.
- `legacy_batch` processes records concurrently and does not promise completion
  order within a batch.
- `partition_ordered` provides FIFO worker lanes for source partitions, but it
  currently requires auto commit.
- Destination partition selection can change ordering. Preserve a stable key or
  explicitly choose a suitable partitioning field when order matters.
- Adding source partitions can change the mapping of keyed records.

Do not claim both strict per-partition processing order and the manual-commit
profile until offset coordination for `partition_ordered` is implemented and
verified.

## Queued delivery

Queued delivery is opt-in:

```yaml
commit_strategy:
  manual_commit: false

performance:
  producer_delivery_mode: queued
  producer_max_in_flight: 10000

retry:
  max_attempts: 1

dlq:
  enabled: false
```

Configuration validation rejects queued delivery with manual commits, multiple
processing attempts, or an enabled DLQ. Delayed delivery failures cannot retain
the original envelope for retry or DLQ routing.

Use `streamforge_messages_delivered_total` to observe broker-acknowledged
deliveries. Enqueue or processor completion is not a delivery guarantee.

## Dead-letter queue behavior

The DLQ producer uses acknowledgements from all in-sync replicas and retries its
own send. A successful DLQ send allows the source record to count as processed;
an exhausted DLQ send returns an error and prevents the manual-commit batch from
committing.

DLQ records retain the original key and value and add `x-streamforge-*` error
metadata. Protect the DLQ like the source topic: it can contain original payloads
and headers.

Before enabling a DLQ:

- create the topic with appropriate replication and retention;
- restrict read and write ACLs;
- alert on DLQ writes and delivery failures;
- test inspection, correction, and replay;
- make replay idempotent and preserve an audit trail.

## Failure matrix

| Failure | Manual acknowledged profile | Likely result |
|---|---|---|
| Destination rejects a record | Processing retries, routes to DLQ, or fails | No source commit until the configured recovery succeeds |
| DLQ send fails | Batch fails | Source records remain eligible for replay |
| Destination succeeds, process stops before commit | Offset is not committed | Duplicate delivery is possible |
| Commit retries are exhausted | Pipeline stops | Successfully delivered records in the batch may replay |
| Consumer group rebalances during work | Ownership can change | Duplicate processing is possible |
| Queued delivery fails after enqueue | Failure is recorded asynchronously | Original record cannot be retried or sent to DLQ |

## Production verification

Exercise delivery behavior with failures, not only a steady-state load:

1. stop the destination broker during processing;
2. interrupt StreamForge after delivery but before a manual commit;
3. make the DLQ unavailable;
4. add and remove a consumer to force a rebalance;
5. restart from the last committed offsets;
6. verify destination duplicates, missing records, DLQ records, and committed
   offsets against the expected profile.

See [Observability](OBSERVABILITY_QUICKSTART.md) for the metrics endpoints and
[Troubleshooting](TROUBLESHOOTING.md) for incident procedures.
