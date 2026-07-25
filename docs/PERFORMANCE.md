---
title: Performance
nav_order: 4
parent: Operations
---

# Performance

StreamForge performance depends on payload size, partition count, broker and
network latency, filter and transform complexity, destination fan-out, delivery
semantics, and available CPU and memory.

A throughput result belongs in public documentation only when it comes from a
reproducible end-to-end run and passes the count, error, warm-up, duration, and
variance gates below. Comparisons with another implementation additionally
require the same workload and delivery guarantees.

## Validated sustained baseline

On 2026-07-25, the private AWS harness measured a median output-delivery rate of
**106,693 messages/second** across three 120-second repetitions. The range was
106,652–106,799 messages/second with 0.058% coefficient of variation.

| Workload property | Value |
|---|---|
| Mode | `partition_ordered` processing with queued delivery |
| Host | AWS `c7i.2xlarge`, Intel Xeon Platinum 8488C |
| Parallelism | 8 Kafka partitions, 8 StreamForge threads |
| Ingress target | 135,000 messages/second |
| Warm-up | 1,000,000 untimed records per repetition |
| Timed validation | 16,200,000 records per repetition |
| Correctness | Exact input, consumed, produced, delivered, and output counts; zero errors |
| Resource use | 1.124 median mean CPU cores; 137.1 MiB median peak RSS |

The workload was a deterministic passthrough test on commit
`d848f118e62b41c7605250c69c9da087af688d0c`. It ran in a private subnet with no
public IP, internet gateway, NAT gateway, load balancer, SSH access, or public
security-group rule. Terraform destroyed all 45 resources after collection.

This is a StreamForge baseline, not a Java comparison. A matched Java/Rust run
remains required before making a relative implementation claim. The
[full result and validation contract](https://github.com/rahulbsw/streamforge/blob/main/docs/benchmarks/results/BENCHMARK_RESULTS.md)
are retained with the repository evidence.

## Runtime controls

```yaml
threads: 4

performance:
  consumer_batch_size: 100
  consumer_batch_timeout_ms: 100
  parallelism_factor: 10

  processing_mode: legacy_batch
  worker_queue_capacity: 1024

  producer_delivery_mode: acknowledged
  producer_max_in_flight: 10000

  fetch_min_bytes: 65536
  fetch_max_wait_ms: 500
  batch_size: 1000
  linger_ms: 10
```

These values illustrate the schema; they are not recommended production sizing.

| Field | Default | Effect |
|---|---:|---|
| `consumer_batch_size` | `100` | Maximum records collected for one application batch |
| `consumer_batch_timeout_ms` | `100` | Maximum legacy batch fill wait; queued-delivery drain delay in partition-ordered mode |
| `parallelism_factor` | `10` | Processing concurrency multiplier applied to `threads` |
| `processing_mode` | `legacy_batch` | Legacy batch barrier or bounded partition worker lanes |
| `worker_queue_capacity` | `1024` | Per-worker input bound in `partition_ordered` mode |
| `producer_delivery_mode` | `acknowledged` | Await Kafka acknowledgement or track delivery after enqueue |
| `producer_max_in_flight` | `10000` | Bound for queued delivery futures |

Effective legacy processing concurrency is:

```text
max(1, threads × parallelism_factor)
```

Configuration validation enforces reliability constraints:

- `partition_ordered` requires auto commit;
- `queued` delivery requires auto commit;
- `queued` delivery requires `retry.max_attempts: 1`;
- `queued` delivery requires `dlq.enabled: false`.

Review [Delivery guarantees](DELIVERY_GUARANTEES.md) before using either mode.

## Kafka client mappings

| Performance field | librdkafka property |
|---|---|
| `fetch_min_bytes` | `fetch.min.bytes` |
| `fetch_max_wait_ms` | `fetch.wait.max.ms` |
| `batch_size` | `batch.num.messages` |
| `linger_ms` | `linger.ms` |
| `queue_buffering_max_ms` | `queue.buffering.max.ms` |

`batch_size` is a message count. Configure librdkafka `batch.size` through
`producer_properties` when a byte limit is needed.

`linger.ms` and `queue.buffering.max.ms` are aliases. If both performance fields
are set, `linger_ms` wins. Explicit `consumer_properties` and
`producer_properties` override generated performance properties.

```yaml
performance:
  fetch_min_bytes: 65536
  batch_size: 1000

consumer_properties:
  fetch.min.bytes: "1"

producer_properties:
  batch.num.messages: "500"
  batch.size: "65536"
```

## Tuning procedure

1. Define the correctness and delivery profile.
2. Fix the source and destination topology, topic partitions, replication,
   acknowledgements, and security settings.
3. Use representative payload sizes, keys, headers, filters, transforms, and
   fan-out.
4. Warm the runtime and brokers before measurement.
5. Record broker-acknowledged completions, end-to-end latency, lag, errors, CPU,
   memory, network, and destination offsets.
6. Run multiple trials and report variance.
7. Change one control at a time.
8. Retain a change only if it improves the target without violating reliability,
   latency, error, or resource objectives.

Useful experiments:

- increase application batch size for steady traffic, then check latency and
  memory;
- reduce the batch timeout for low-volume latency;
- increase processing concurrency only while work is I/O-bound and bounded
  queues remain healthy;
- compare the legacy batch scheduler with partition-ordered lanes using a
  workload whose delivery constraints permit auto commit;
- sweep producer linger and queued depth only with the queued-mode reliability
  limitations explicitly accepted;
- use simple comparisons instead of regex when they express the same rule;
- avoid transforms on passthrough destinations;
- inspect key distribution before adding partitions or replicas.

## Partitioning

- A present key, including explicit JSON `null`, is hashed to an explicit target
  partition.
- An absent key delegates partition choice to librdkafka.
- Field partitioning selects an explicit partition from the configured JSON
  field.

Low-cardinality or skewed keys can create hot partitions. Measure per-partition
lag and delivery rate rather than relying only on totals.

## Benchmarks

Focused Criterion suites are available:

```bash
cargo bench --bench filter_benchmarks
cargo bench --bench transform_benchmarks
cargo bench --bench end_to_end_benchmark
```

Microbenchmarks isolate code paths. They do not include Kafka brokers, network,
consumer commits, scheduling, or destination acknowledgement and must not be
presented as end-to-end message throughput.

An end-to-end result record should include:

- source revision and clean/dirty worktree state;
- instance or host type, CPU architecture, core allocation, and memory;
- Kafka versions, broker topology, storage, and network placement;
- topic partitions, replication, and retention;
- payload distribution and total records;
- complete StreamForge configuration with secrets redacted;
- warm-up, run duration, repetitions, and aggregation method;
- source-produced count, source-consumed count, destination-acknowledged count,
  and independently observed destination count;
- latency percentiles, lag, errors, CPU, memory, and network;
- setup, runtime, and teardown cost;
- confirmation that no benchmark service was exposed publicly.

Reject a run if counters are inconsistent, the destination count is incomplete,
the comparison uses different delivery semantics, or any resource remains after
the teardown audit.

## Optimization priorities

The current JSON pipeline parses payloads into `serde_json::Value`, walks the
tree for filters and transforms, and serializes destination values. SIMD by
itself is unlikely to improve pointer-heavy tree traversal.

Profile before changing the representation. Candidate work should be evaluated
in this order:

1. preserve raw Kafka bytes for routes that do not need JSON;
2. parse lazily according to selected operations;
3. reduce array-element and destination serialization copies;
4. isolate vectorizable byte scanning, hashing, or numeric kernels;
5. measure the full pipeline again after each change.

## Production checklist

- Benchmark the exact delivery profile used in production.
- Verify destination records independently of application counters.
- Monitor lag, delivery errors, CPU, memory, and partition balance.
- Establish resource limits from measured use.
- Repeat the workload after any broker, instance, partition, filter, transform,
  fan-out, security, or version change.
- Publish numerical comparisons only after the result contract is satisfied.
