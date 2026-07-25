# Architecture

StreamForge is a Rust-native Kafka data-plane service for selective replication:
consume records, evaluate routing rules, optionally transform message envelopes,
and produce to one or more destinations.

This document describes the current architecture. Product boundaries and future
typed-envelope work are governed by `PROJECT_SPEC.md`.

## System context

```text
Source Kafka
    |
    v
rust-rdkafka StreamConsumer
    |
    v
selectable legacy batching or bounded source-partition worker lanes
    |
    v
MessageEnvelope (JSON value, optional key, headers, timestamp, source metadata)
    |
    +--> destination filter --> optional transform --> KafkaSink --> Target Kafka
    +--> destination filter --> optional transform --> KafkaSink --> Target Kafka
    |
    +--> metrics, retry, DLQ, and offset-commit handling
```

The current data path parses payloads into `serde_json::Value`. A raw-byte
passthrough envelope is planned but is not part of the current runtime.

## Major layers

### Configuration

`src/config.rs` parses YAML or JSON into typed configuration and validates
cross-field constraints.

Responsibilities include:

- source, destination, security, commit, retry, DLQ, cache, aggregation, and
  observability settings;
- backward-compatible runtime performance defaults;
- mapping selected performance fields to librdkafka properties;
- preserving explicit `consumer_properties` and `producer_properties` as the
  highest-precedence performance settings. StreamForge reapplies
  `enable.auto.commit` and `enable.auto.offset.store` after raw consumer
  properties because commit strategy is a validated reliability contract.

`src/main.rs` applies the validated configuration to the consumer and processing
loop.

### Consumer and processing loop

`src/main.rs` owns the `StreamConsumer`, subscription, processing-mode
selection, and offset-commit coordination. `src/partition_pipeline.rs` owns the
bounded partition-ordered worker implementation.

The compatibility mode retains the historical batch barrier:

```yaml
performance:
  processing_mode: legacy_batch
  consumer_batch_size: 100
  consumer_batch_timeout_ms: 100
  parallelism_factor: 10
```

Effective processing concurrency is the saturating product of `threads` and
`parallelism_factor`, with a minimum of one.

The opt-in partition-ordered mode detaches consumed records into owned messages
and routes every `(source topic, source partition)` to one of `threads` bounded
FIFO worker lanes:

```yaml
performance:
  processing_mode: partition_ordered
  worker_queue_capacity: 1024
```

Records from one source partition enter one lane in consumption order. Different
lanes execute concurrently. This mode currently supports auto commit only;
manual commit requires a rebalance-aware completed-offset coordinator and is
rejected during configuration validation.

### Envelope

`src/envelope.rs` defines `MessageEnvelope`. It carries:

- a JSON message value;
- an optional JSON key;
- headers;
- timestamp;
- source topic, partition, and offset metadata.

The JSON value is reference-counted for destination fan-out. Destinations without
a value transform retain the shared allocation. A destination with a transform
uses copy-on-write ownership: a uniquely owned value can be reused, while a
shared value is cloned only when mutation is required.

### Filter and transform DSL

`src/filter_parser.rs`, `src/dsl/`, and `src/filter/` implement the DSL.

Supported execution forms include:

- legacy colon-delimited filters and transforms;
- function-style filters parsed into an AST;
- value, key, header, timestamp, array, string, and cache-aware operations.

Function-style filter construction lowers the parsed expression into a compiled
evaluation tree. JSON path segments, regexes, and typed array literals are
prepared once at construction rather than on every message. Key-template
transforms similarly tokenize placeholders and paths once.

Function-style array `any` and `all` evaluation still clone each visited array
element into a temporary envelope. That boundary is intentionally left for a
later measured refactor.

### Destination processing

`src/processor.rs` builds a runtime for each configured destination.

Each destination can have:

- an optional filter;
- an optional value transform;
- key, header, and timestamp transforms;
- optional cache or aggregation behavior;
- an independent Kafka sink.

An absent value transform remains `None`; no identity transform or transform
metric is executed. Multi-destination routing shares the incoming value until a
destination requires mutation.

### Producer and partitioning

`src/kafka/sink.rs` wraps a rust-rdkafka `FutureProducer`, resolves output topic
templates, applies producer/security settings, serializes envelopes, and sends
records.

Producer delivery is selectable:

- `acknowledged` is the compatibility default and waits for every record's
  broker delivery result;
- `queued` uses librdkafka's nonblocking enqueue path, tracks delivery futures,
  applies a configured pending-delivery bound, faults on asynchronous failure,
  and drains on flush.

Queued mode deliberately supports only auto commit, with message retries
disabled and the DLQ disabled. A delayed delivery failure cannot be associated
with the original envelope, so enabling queued mode with manual commits or
envelope-level recovery is rejected rather than weakening those contracts
silently.

`src/partitioner.rs` supplies explicit partition choices when StreamForge owns
the routing decision:

- a present key uses deterministic keyed hashing;
- field-based partitioning hashes the configured JSON field;
- an absent key with default partitioning returns no explicit partition, so
  librdkafka selects the partition using its configured keyless behavior.

An explicit JSON `null` key is still a present key and follows keyed hashing.

### Reliability

The runtime supports manual and automatic commit modes, retry policies, and a
dead-letter queue. Commit and failure semantics are defined in
`docs/DELIVERY_GUARANTEES.md`.

Exactly-once Kafka transactions are not implemented.

### State and aggregation

`src/cache.rs` and `src/cache_backend.rs` provide local and Redis-backed caching.
`src/aggregation.rs` provides configured windowed aggregation subject to
validation constraints in `src/config.rs`.

Stateful behavior must not silently change delivery guarantees. Broader
fault-tolerant state recovery remains future work.

### Observability

`src/metrics.rs` and `src/observability/` provide processing metrics, Prometheus
exposure, HTTP observability endpoints, and consumer-lag monitoring.

Performance decisions should use completed-message rate, lag, error rate,
latency, CPU, and memory together. A microbenchmark result is not an end-to-end
Kafka service-level result.

## Phase 1 performance decisions

Phase 1 deliberately uses low-risk changes that preserve the JSON envelope and
DSL contracts:

1. Delegate keyless default partition selection to librdkafka.
2. Skip absent value transforms.
3. Use copy-on-write values for actual destination transforms.
4. Compile function-style paths and regexes at filter construction.
5. Compile key-template placeholders and paths at transform construction.
6. Expose batching, fill timeout, concurrency, and selected Kafka tuning fields.
7. Add regression tests and steady-state Criterion benchmarks.

No fixed throughput or latency is part of the architecture contract. See
`docs/PERFORMANCE.md` for the measurement method.

## Phase 2 delivery and scheduling decisions

The first dedicated profile showed that per-record delivery waiting and the
100-record batch barrier were stronger candidates than SIMD. The resulting
opt-in path:

1. replaces batch barriers with bounded partition-affine worker lanes;
2. makes `threads` the logical worker-lane count;
3. moves JSON parsing and envelope construction into those workers;
4. queues Kafka deliveries without awaiting each acknowledgement;
5. bounds and drains pending delivery futures;
6. exposes a delivery-acknowledgement metric separate from enqueue success;
7. keeps the legacy reliability behavior as the default.

The corrected Kafka harness warms the pipeline before timing and records input
publication, post-publication drain, and end-to-end completion independently.
This architecture is implemented but does not carry a throughput claim until a
new controlled benchmark is run.

## Why SIMD is not in Phase 1

The current JSON filter path walks a heterogeneous tree and performs
pointer-heavy, branch-heavy operations. SIMD does not automatically accelerate
that representation. A SIMD implementation is justified only when profiling
identifies a stable, uniform kernel such as byte scanning, hashing, or
homogeneous numeric processing.

The broader raw/lazy envelope design can avoid more work than vectorizing a
small part of the current parsed-JSON path. Because that design changes public
processing contracts, it remains in the later phase already defined by
`PROJECT_SPEC.md`.

## Scaling model

Vertical scaling is bounded by:

- source partition parallelism;
- configured processing concurrency;
- CPU cost of parsing, filters, transforms, aggregation, and serialization;
- destination producer queues and broker/network latency;
- memory retained by in-flight messages.

Horizontal scaling uses Kafka consumer-group partition assignment. Adding
instances beyond the number of useful source partitions does not add consumer
parallelism.

Ordering is preserved only within the constraints of Kafka partition ordering
and the configured processing/delivery behavior. Changing partitioning keys can
change ordering domains.

`partition_ordered` preserves source-partition processing/enqueue order during a
stable assignment. It does not force source and target partition identity, add
Kafka transactions, or fence work across a consumer-group rebalance.

## Module map

```text
src/
├── main.rs                     runtime setup, consumer loop, commits
├── lib.rs                      public exports
├── config.rs                   typed configuration and validation
├── envelope.rs                 current JSON message envelope
├── partition_pipeline.rs       bounded source-partition worker lanes
├── processor.rs                destination runtime and routing
├── filter_parser.rs            DSL construction and compiled evaluator
├── dsl/                        function-style parser and AST
├── filter/                     filters and transforms
├── kafka/sink.rs               producer wrapper and serialization
├── kafka/sink/delivery.rs      bounded asynchronous delivery tracking
├── partitioner.rs              keyed and field partition decisions
├── aggregation.rs              windowed aggregation
├── cache.rs                    cache interfaces
├── cache_backend.rs            cache implementations
├── retry.rs                    retry policy
├── dlq.rs                      dead-letter queue
├── metrics.rs                  processing metrics
└── observability/              HTTP metrics and lag monitoring
```

## Verification boundaries

Unit tests exercise configuration, DSL, transform, processor, and partitioning
semantics. Criterion targets cover isolated filter, transform, and end-to-end
code paths. Kafka integration results require a reproducible broker environment
and are not inferred from unit or microbenchmark success.

## Related documents

- `PROJECT_SPEC.md` — product scope and typed-envelope direction
- `ROADMAP.md` — planned work
- `docs/IMPLEMENTATION_STATUS.md` — verified capability status
- `docs/PERFORMANCE.md` — tuning and benchmark method
- `docs/DELIVERY_GUARANTEES.md` — commit and failure semantics

**Last updated:** 2026-07-24
