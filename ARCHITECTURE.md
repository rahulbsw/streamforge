# Architecture

StreamForge is a Rust-native Kafka-ingestion data-plane service for selective
replication: consume records, evaluate routing rules, optionally transform
message envelopes, and write to Kafka or Apache Fluss. Apache Iceberg is the
next planned sink adapter.

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
    +--> destination filter --> optional transform --> SinkWriter --> Kafka / Apache Fluss
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

`crates/streamforge-config-model` owns the additive
`StreamforgePipeline` v1alpha1-to-engine projection. The Kubernetes operator
serializes a CRD through this crate before writing its ConfigMap, and
`streamforge-validate --input-format pipeline-crd` uses the same projection
before typed engine validation. This prevents the UI dry-run and reconciler
from maintaining competing mappings.

### Destination sink extension

`SinkWriter` is the destination I/O boundary used by processors. The sink
factory registry selects an implementation from `sink.type`; an omitted type
defaults to Kafka for backward compatibility. The sink configuration carries
namespaced backend properties, and each factory validates and converts its
own block. Sink implementations must preserve the `send` acknowledgment and
`flush` barrier contract so the processing loop can safely coordinate source
offset commits. The Fluss adapter uses the native Rust client for append and
primary-key upsert. The Iceberg adapter is PR3 work and remains append-only
while the Rust client lacks row-level writes.

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

### Optional WebAssembly UDF runtime

`src/wasm/` implements the opt-in, stateless UDF extension point. It does not
replace the native DSL and is not initialized when the top-level `wasm`
registry is absent.

Startup canonicalizes the configured artifact root, reads each component once
through bounded I/O, verifies its pinned SHA-256 digest, compiles it with
Wasmtime, links it against an empty host linker, checks its declared WIT world,
and probes instantiation before any Kafka client is created. No WASI or other
ambient host interface is linked.

The versioned `streamforge:udf@1.0.0` WIT package defines separate filter,
JSON-value-transform, and mutable-envelope-transform worlds. Source
topic/partition/offset are input-only. Native and UDF stages compose in this
order:

1. native filter, then UDF filter;
2. native value transform, then UDF value transform;
3. native key/header/timestamp envelope mutations, then UDF envelope mutation.

This ordering implements the `PROJECT_SPEC.md` contract: envelope mutations
observe the final destination payload. It changes the earlier runtime behavior,
which applied native envelope mutations before the value transform. Pipelines
that derive envelope fields from values removed by their value transform must
retain those inputs in the transformed payload or update the envelope rule.

Each invocation uses a fresh store and component instance backed by Wasmtime's
pooling allocator. A dedicated epoch thread enforces execution deadlines.
Configured bounds cover artifact, input, output, linear memory, tables, stack,
and concurrent instances. Guest state is never a persistence contract.

UDF failures are deterministic destination-stage failures and are not retried.
The destination `error_policy` selects fail-fast, one contextual DLQ record,
destination skip, or unchanged-envelope continuation. See `docs/WASM_UDFS.md`
for the ABI, deployment, security, and performance contract.

### Destination processing

`src/processor.rs` builds a runtime for each configured destination.

Each destination can have:

- an optional filter;
- an optional value transform;
- key, header, and timestamp transforms;
- optional cache or aggregation behavior;
- an independent configured sink implementation.

An absent value transform remains `None`; no identity transform or transform
metric is executed. Multi-destination routing shares the incoming value until a
destination requires mutation.

### Producer and partitioning

`src/sink_factory.rs` resolves the configured `sink.type` and creates a
`SinkWriter`. Kafka is the default and is backed by `src/kafka/sink.rs`, which
wraps a rust-rdkafka `FutureProducer`, resolves output topic templates, applies
producer/security settings, serializes envelopes, and sends records. The
`src/fluss.rs` adapter resolves a pre-provisioned Fluss table and maps JSON rows
to its schema; it selects append or upsert from primary-key metadata.

The Iceberg factory loads an existing REST catalog table and appends JSON
object values as Parquet data files. It buffers records per destination writer
by event count or elapsed time, applies Iceberg partition transforms for
top-level primitive source fields, and commits one snapshot per batch. Each
send acknowledgment and the explicit `flush()` barrier wait for snapshot
commit. This keeps manual source-offset advancement behind durable destination
commits. The adapter does not create tables or perform row-level updates or
deletes; MoR and CoW are therefore unavailable.

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
dead-letter queue. In manual mode it awaits successful processing and calls
`MessageProcessor::flush()` before advancing source offsets. Buffered sinks use
that call as a durability/commit barrier. Commit and failure semantics are
defined in `docs/DELIVERY_GUARANTEES.md`.

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

The HTTP surface is intentionally bounded:

- `/health` is process liveness and returns the backward-compatible literal
  `OK`;
- `/ready` is structured readiness and changes between 200 and 503 based on
  runtime/Kafka state;
- the configured metrics path exposes the Prometheus registry.

`STREAMFORGE_LOG_FORMAT` selects `text` or `json`; `RUST_LOG` remains the level
filter. Operational fields are structured, but payloads, credentials, message
keys, and DSL expression contents are excluded.

The operator exposes the named metrics port and configures `/ready` and
`/health` probes. Helm can install a private metrics Service, ServiceMonitor,
PrometheusRule, alerts, recording rules, and Grafana dashboard.

Performance decisions should use completed-message rate, lag, error rate,
latency, CPU, and memory together. A microbenchmark result is not an end-to-end
Kafka service-level result.

### Kubernetes operator and UI

The operator remains the only pipeline-control component. It watches
`StreamforgePipeline` v1alpha1 resources and reconciles ConfigMaps, Deployments,
Secret mounts, and status. Kafka destinations share the target broker and
security settings. The canonical converter emits separate source and Kafka
target security blocks containing only mounted credential file paths. It
rejects common inline credential fields in sink properties; custom sinks must
use mounted file paths for credentials. Inline Kafka credentials and divergent
Kafka target security are rejected before workload creation.

Generated ConfigMaps and Deployments carry controller owner references to the
pipeline. The controller also watches owned Deployment changes so workload
availability updates status promptly. Status patches are skipped when the
desired status is unchanged; the `Ready` condition is derived from observed
Deployment generation, updated/available/ready replicas, and replica failures.
`lastTransitionTime` changes only when readiness changes. Deleting a pipeline
therefore garbage-collects its generated workload and configuration.

The Next.js UI talks to Kubernetes and predefined Prometheus queries. Creation
uses the bundled matching `streamforge-validate` binary without a shell,
enforces input/time/output bounds, and performs Kubernetes server-side dry-run
before applying a resource. Pipeline detail APIs expose bounded summaries,
metrics windows, events, and logs. Viewer sessions cannot mutate resources.
The UI does not run or supervise standalone local StreamForge processes.
The UI exposes only supported global/runtime controls; the former
per-destination compression field was removed because the engine implements
compression as a global producer policy.

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
├── sink_factory.rs             sink registry and destination writer construction
├── fluss.rs                    native Fluss append/upsert adapter
├── filter_parser.rs            DSL construction and compiled evaluator
├── dsl/                        function-style parser and AST
├── filter/                     filters and transforms
├── kafka/sink.rs               producer wrapper and serialization
├── kafka/sink/delivery.rs      bounded asynchronous delivery tracking
├── kubernetes.rs               typed wrapper around shared CRD projection
├── partitioner.rs              keyed and field partition decisions
├── aggregation.rs              windowed aggregation
├── cache.rs                    cache interfaces
├── cache_backend.rs            cache implementations
├── retry.rs                    retry policy
├── dlq.rs                      dead-letter queue
├── metrics.rs                  processing metrics
└── observability/              HTTP metrics and lag monitoring

crates/
└── streamforge-config-model/   shared CRD-to-engine JSON projection

operator/
├── src/crd.rs                  v1alpha1 resource/status schema
├── src/reconciler.rs           Kubernetes API reconciliation
├── src/resources.rs            generated ConfigMap/Deployment construction
├── src/status.rs               idempotent phase/Ready calculation
└── src/render.rs               shared-model engine configuration rendering

ui/
├── app/api/                    bounded Kubernetes/Prometheus APIs
├── app/pipelines/              onboarding and pipeline operations
└── lib/                        auth, Kubernetes, API, and pipeline contracts
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

**Last updated:** 2026-11-25
