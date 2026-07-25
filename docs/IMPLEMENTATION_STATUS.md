# Implementation Status

Verified source status as of 2026-07-25. This document records implemented
capabilities and known boundaries; it does not assert production throughput
without a reproducible workload and benchmark result.

## Core data plane

Implemented:

- Kafka consume, process, and produce pipeline using Tokio and rust-rdkafka.
- Single- and multi-destination routing.
- Optional per-destination filters and transforms.
- Manual and automatic offset commit modes.
- Retry and dead-letter queue modules.
- Key, header, timestamp, and value-aware envelope operations.
- Default keyed partitioning and field-based partitioning.
- Native Kafka compression configuration.
- Local and Redis cache backends.
- Prometheus metrics, HTTP observability endpoints, and consumer-lag monitoring.
- Windowed aggregation with the constraints validated in configuration.

Known boundaries:

- Delivery is not exactly-once; transactional producer support is not
  implemented.
- Payload processing uses JSON values. Avro and Schema Registry integration are
  not implemented.
- Runtime configuration reload is not implemented.
- The generic raw/typed envelope described in `PROJECT_SPEC.md` remains planned.

## Filter and transform DSL

Implemented:

- Legacy colon-delimited filters and transforms.
- Function-style filters with parsed AST input.
- JSON path comparisons and boolean composition.
- Regex, array, key, header, timestamp, null/empty, and string predicates.
- JSON path extraction, object construction, array mapping, arithmetic, string,
  key, header, timestamp, hash, and cache transforms.
- Configuration-time compilation of function-style paths and regex patterns.
- Configuration-time tokenization of key-template placeholder paths.

Known hot-path boundary:

- Function-style array `any`/`all` evaluation still creates an envelope from a
  cloned array element. This is a candidate for a later measured optimization.

## Phase 1 performance hardening

Implemented in the current source:

- Keyless default partitioning delegates to librdkafka instead of forcing an
  explicit partition.
- Keyed default partitioning and field-based routing remain explicit.
- Destinations without transforms skip identity-transform execution and keep the
  existing shared value allocation.
- Actual transforms use copy-on-write value ownership.
- Function-style paths and regexes are compiled once.
- Key-template placeholder paths are compiled once.
- Runtime consumer batch size, batch fill timeout, and concurrency factor are
  configurable with backward-compatible defaults.
- Selected consumer and producer performance fields map to librdkafka
  properties, with explicit property maps taking precedence.
- Focused regression tests and steady-state Criterion benchmarks cover these
  paths.

Default runtime values remain:

| Setting | Default |
|---|---:|
| Consumer batch size | `100` messages |
| Consumer batch fill timeout | `100` ms |
| Parallelism factor | `10` times `threads` |

See `docs/PERFORMANCE.md` for the configuration and measurement contract.

## Phase 2 delivery and scheduling implementation

Implemented in the current source:

- Backward-compatible `legacy_batch` and opt-in `partition_ordered` processing
  modes.
- `partition_ordered` uses `threads` bounded FIFO worker lanes and stable source
  topic/partition routing, with JSON parsing and processing inside the workers.
- Backward-compatible per-record `acknowledged` delivery and opt-in bounded
  asynchronous `queued` delivery.
- Queued delivery tracks broker acknowledgements, applies configurable
  backpressure, persists the first delivery failure, and drains during flush.
- A separate `streamforge_messages_delivered_total` metric distinguishes broker
  acknowledgement from processor/enqueue completion.
- Validation rejects queued delivery with manual commits, retries, or DLQ, and
  rejects partition-ordered processing with manual commits until explicit,
  rebalance-aware completed-offset coordination is implemented.

These modes are implemented and unit-tested. The partition-ordered/queued
combination has completed a valid sustained local Kafka run; the remaining
mode comparison matrix and a new AWS run are still pending.

Default compatibility values remain:

| Setting | Default |
|---|---:|
| Processing mode | `legacy_batch` |
| Worker queue capacity | `1024` per worker |
| Producer delivery mode | `acknowledged` |
| Producer maximum pending deliveries | `10000` |

## Phase 2 benchmark and profiling foundation

Implemented in the current tree:

- A deterministic JSONL generator keyed by message count and seed.
- A Kafka-backed harness with isolated topics/groups; independent persistent
  ingress, timed metrics/resource, and post-window output-validation jobs; a
  shared monotonic barrier; exact counters and offsets; multiple repetitions;
  and schema-version-3 environment/results manifests.
- Per-repetition Kafka metadata deletion plus verified physical partition-file
  reclamation before the next repetition.
- A 24-case synthetic pipeline Criterion matrix covering JSON and envelope
  stages across 256 B, 4 KiB, and 64 KiB payloads.
- A manual GitHub Actions workflow that runs all Criterion targets and one
  Kafka-backed smoke repetition without imposing a noisy shared-runner gate.
- One canonical performance-testing contract and a dated baseline record
  containing local and dedicated AWS x86_64 evidence.
- Whole-process `perf` capture with release debug information, forced frame
  pointers, raw profile data, and zero-lost-sample verification.

The Kafka harness uses auto commit, partition-ordered workers, queued producer
delivery, retries disabled, and DLQ disabled for its optimized passthrough
workload. Exact delivered/output counts verify that a benchmark run completed
without loss, but do not change or prove general delivery semantics.

## Verification

Verified for the current source on 2026-07-25 UTC:

- `cargo test --all --no-fail-fast`: 474 passed, 0 failed, 30 ignored across
  unit, integration, and documentation tests.
- Partition-worker tests verify same-partition FIFO order, cross-lane
  concurrency, and bounded-queue backpressure.
- Queued-delivery tests verify successful acknowledgements, broker failures,
  canceled futures, configuration safety constraints, and flush forwarding.
- `cargo clippy --all-targets --offline -- -D warnings`: passed.
- `cargo fmt --all -- --check`, benchmark Bash/Python syntax checks,
  `git diff --check`, generated benchmark-config validation, and JSON schema
  parsing: passed.
- `cargo build --release --bin streamforge`: passed before the local benchmark
  preflight.
- Six sustained-harness unit tests, Python compilation, Bash syntax, compose
  rendering, and `git diff --check`: passed.
- Single-destination produced accounting now increments the exact
  destination-labelled counter and has focused regression tests.
- The loopback-only Podman harness passed three 120-second
  partition-ordered/queued repetitions after a one-million-record untimed
  warm-up. Each repetition reconciled exactly 24,000,000 timed records across
  input offsets, consumed, produced, broker-delivered, output offsets, and the
  independent output validator, with zero errors.
- Diagnostic local aggregate: median `199,604.900121 msg/s`, minimum
  `199,576.121938`, maximum `199,677.648263`, coefficient of variation
  `0.0214%`, median `1.721` StreamForge cores, and median peak RSS `128 MiB`.
  Environment: Apple M4 Pro host, Podman 5.7.1 ARM64 VM with 4 vCPUs and
  6,144 MiB RAM, Kafka image pinned by digest, 8 partitions, and 8 threads.
- That aggregate is not a public baseline: the worktree was dirty and two runs
  were ingress-limited. It is a sustained lower bound and is not directly
  comparable to the superseded coupled-harness numbers or an unmatched Java
  run.
- Kafka was published only on `127.0.0.1:9092`; the benchmark network was
  internal; ingress and output runners exposed no ports. Final topic and disk
  reclamation checks passed.
- No AWS rerun has been attempted after the valid local result. All resources
  from the previous AWS attempt remain verified deleted.

Previously verified on 2026-07-24:

- `cargo test --all`: 462 passed, 0 failed, 30 ignored across unit, integration,
  and documentation tests.
- All three Criterion targets completed and saved the `phase2-current`
  baseline; the synthetic pipeline ran all 24 cases.
- Kafka-backed passthrough baseline: 10,000 deterministic messages, 4
  partitions, 4 threads, 3 repetitions, median `1,495.709902 msg/s`, exact
  10,000 consumed/output records per repetition, and zero processing errors.
- `cargo clippy --all-targets --offline -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed.
- `docs/CONFIG_SCHEMA.json`: parsed successfully with `jq`.

Previously verified on dedicated AWS x86_64 compute on 2026-07-25:

- `cargo test --all --locked`: 462 passed, 0 failed, 30 ignored.
- All three Criterion targets completed with baseline name `aws-c7i-phase2`.
- Kafka-backed passthrough: 100,000 deterministic messages, 8 partitions, 8
  threads, 5 repetitions, median `6,254.848569 msg/s`, exact consumed/output
  counts, and zero processing errors.
- A 200,000-message profiling repetition completed at `7,094.333485 msg/s`;
  `perf` recorded 614 cycle samples with zero lost samples.
- Direct AWS service inventories confirmed both instances terminated, no live
  volumes or public IPs, and deletion of the private S3 bucket, scheduler, IAM
  objects, and isolated VPC/network resources.

Those local and AWS rates were measured by the superseded coupled harness. They
include source publication and polling overhead and are not StreamForge capacity
claims or results for the new modes. See
`docs/benchmarks/results/phase2-baseline-20260724.md` for the full environment
and method.

## Planned measured work

The next performance work starts from the dedicated whole-process profile, not
from a blanket SIMD rewrite:

1. Run the corrected live Kafka matrix for legacy/acknowledged,
   partition-ordered/acknowledged, and partition-ordered/queued.
2. Run a clean-worktree, matched Java/Rust comparison with identical payloads,
   partitions, acknowledgement semantics, warm-up, duration, and validation.
3. Provision the later AWS benchmark with cost-bounded Terraform and
   ECS-on-EC2 jobs only after the local matrix passes; keep all endpoints
   private or restricted to the user's IP.
4. Keep raw/lazy envelope work behind the existing 30% parse/serialization
   threshold; the AWS passthrough profile measured about 16.5% parsing and 3.2%
   serialization on overlapping inclusive stacks.
5. Profile transform-heavy and aggregation-heavy workloads separately.
6. Evaluate SIMD only if one of those profiles identifies a dominant
   vectorizable kernel.

Product boundaries and the typed-envelope direction remain governed by
`PROJECT_SPEC.md`.
