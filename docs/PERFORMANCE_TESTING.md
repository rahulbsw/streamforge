---
title: Performance Testing
nav_order: 10
parent: Deployment
---

# Performance Testing

StreamForge separates microbenchmarks from Kafka-backed throughput tests.
Results from one layer must not be presented as results from another.

| Layer | Target | Kafka | Purpose |
|---|---|---:|---|
| Criterion microbenchmarks | `filter_benchmarks`, `transform_benchmarks` | No | Isolate DSL construction and steady-state evaluation |
| Criterion synthetic pipeline | `end_to_end_benchmark` | No | Isolate parse, envelope, filter, transform, and serialization stages |
| Sustained Kafka harness | `run_throughput_test.sh` | Yes | Measure a warmed, fixed-duration pipeline with independent correctness checks |

## Criterion

```bash
cargo bench --bench filter_benchmarks -- --noplot
cargo bench --bench transform_benchmarks -- --noplot
cargo bench --bench end_to_end_benchmark -- --noplot
```

Save and compare the same target:

```bash
cargo bench --bench end_to_end_benchmark -- \
  --save-baseline current --noplot
cargo bench --bench end_to_end_benchmark -- \
  --baseline current --noplot
```

Criterion artifacts are written below `target/criterion/`. These tests do not
include broker, network, consumer, commit, or delivery-acknowledgement costs.

## Sustained Kafka harness

The supported local runtime is Podman. Build StreamForge, then start the private
benchmark environment:

```bash
cargo build --release --bin streamforge
podman compose -f docker-compose.benchmark.yml up -d
```

Kafka is published only on `127.0.0.1:9092`. The compose network is internal,
and the ingress and output runners publish no host ports.

Run the harness:

```bash
scripts/benchmarks/run_throughput_test.sh \
  [dataset_records] [partitions] [threads] [repetitions]
```

Defaults are 10,000 deterministic dataset records, 8 partitions, 8 threads,
and 3 repetitions. The dataset is replayed for the configured duration; it is
not the timed record count.

| Variable | Default | Meaning |
|---|---:|---|
| `BENCHMARK_DURATION_SECONDS` | `180` | Shared measured window per repetition |
| `BENCHMARK_WARMUP_MESSAGES` | `10000` | Untimed full-path warm-up count |
| `BENCHMARK_INGRESS_TARGET_RATE` | `0` | Open-loop ingress msg/s; `0` is unbounded |
| `BENCHMARK_STARTUP_TIMEOUT` | `120` | Readiness and physical cleanup timeout |
| `BENCHMARK_DRAIN_TIMEOUT` | `180` | Producer flush and post-window drain timeout |
| `BENCHMARK_POLL_INTERVAL_MS` | `500` | Metrics/resource sample interval |
| `BENCHMARK_METRICS_PORT` | `19090` | Loopback-only StreamForge metrics port |
| `BENCHMARK_RESULTS_ROOT` | `target/performance-results/throughput` | Artifact root |
| `BENCHMARK_PROCESSING_MODE` | `partition_ordered` | `legacy_batch` or `partition_ordered` |
| `BENCHMARK_DELIVERY_MODE` | `queued` | `acknowledged` or bounded `queued` |
| `BENCHMARK_MAX_IN_FLIGHT` | `10000` | Queued delivery bound |
| `CONTAINER_RUNTIME` | `podman` | Container CLI |

`legacy_batch` plus `queued` is rejected because it lacks a safe final delivery
drain. Queued delivery also retains the product configuration safety checks
documented in [Delivery guarantees](DELIVERY_GUARANTEES.md).

### Job model

Every repetition uses three independent jobs:

1. The ingress job starts one persistent Kafka producer, settles it, publishes
   warm-up records through that same process, then sends deterministic records
   for the shared duration. Producer startup is outside measurement.
2. The metrics validator samples destination-specific consumed, produced,
   broker-delivered, and error counters plus StreamForge CPU and RSS. It owns
   the timed output-delivery rate.
3. The output validator starts only after the measured window. It consumes the
   exact expected output count independently, so validation cannot reduce the
   timed throughput.

StreamForge and all three jobs must be ready before the controller releases one
monotonic-clock barrier. The harness requires two stable, exact warm-up samples
before releasing it.

### Measurement contract

The primary rate is:

```text
destination-specific broker-delivered delta
------------------------------------------------
actual monotonic metrics-sample window in seconds
```

The metrics sample immediately after the configured deadline defines the
actual window. Its observation delay is bounded by the polling interval and is
included in the denominator. Startup, broker readiness, consumer assignment,
warm-up, drain, output validation, and teardown are excluded.

Ingress is paced in batches when `BENCHMARK_INGRESS_TARGET_RATE` is non-zero.
Choose a rate high enough to establish the intended load but low enough to
avoid spending the run measuring an overloaded local broker. Treat an
ingress-limited result as a sustainable lower bound, not an engine ceiling.

CPU and RSS summaries use samples from the measured window only. Drain and the
post-window output validator do not contribute to those resource statistics.

### Pass criteria

A repetition passes only when all of these values exactly equal the ingress
job's timed record count:

- source-topic end-offset delta;
- StreamForge consumed delta;
- destination-labelled produced delta;
- destination-labelled broker-delivered delta;
- destination-topic end-offset delta;
- independently consumed output count.

The processing-error delta must be zero. Any missing metric, decreasing
counter, failed process, premature output validator, timeout, or count mismatch
fails the run.

After each repetition, the harness deletes its two topics and waits for both
Kafka metadata deletion and physical partition-directory reclamation before
starting the next repetition. This prevents retained benchmark data from
changing later runs or filling the broker disk.

### Artifacts

Each repetition contains:

- generated StreamForge configuration;
- ingress, metrics, and output-validator results;
- StreamForge and job logs;
- CSV metric/resource samples;
- an exact-accounting result.

The schema-version-3 aggregate records median, minimum, maximum, mean, median
absolute deviation, and coefficient of variation. It also records the dataset
SHA-256, source revision and dirty state, host CPU/OS/Rust version, Kafka image
digest, Podman version, VM CPU/memory allocation, and all run manifests.

The aggregate rejects public use when fewer than three repetitions were run,
the measured duration is below 120 seconds, the worktree is dirty, or one or
more runs are ingress-limited.

Stop and remove only the disposable benchmark environment:

```bash
podman compose -f docker-compose.benchmark.yml down -v
```

## Comparison and publication rules

Compare results only when all material inputs match:

- reviewed source revision and clean/dirty state;
- payload bytes, count, seed, and dataset hash;
- StreamForge configuration and delivery semantics;
- Kafka image, topology, storage, partitions, and replication;
- host architecture, Podman VM CPU/memory, and toolchain;
- warm-up, duration, repetitions, ingress rate, and aggregation method.

Use at least three 120-second repetitions on dedicated hardware. Shared CI is
smoke evidence only. Do not compare the sustained output-delivery rate with a
startup-inclusive completion rate, a Criterion operation time, or a benchmark
using different acknowledgement and correctness guarantees.

No numerical result should appear on the public performance page unless its
aggregate is publication-eligible and it improves the approved matched
baseline without correctness or resource regressions.

## Profiling and SIMD gates

Profile the whole process under the Kafka workload before changing the payload
representation or adding SIMD.

Consider a raw/lazy envelope only when parse plus serialization accounts for at
least 30% of sampled data-plane CPU. Consider SIMD only when profiling finds a
stable vectorizable kernel such as structural byte scanning or homogeneous
numeric processing. Require:

- at least 15% improvement in the confirmed kernel;
- no more than 3% regression for small-message workloads;
- correctness coverage on `aarch64` and `x86_64`;
- a scalar fallback and unchanged public behavior.

Pointer-heavy `serde_json::Value` traversal and configuration-time regex
compilation are not standalone SIMD targets.

## CI

`.github/workflows/performance-test.yml` runs Criterion and a Kafka smoke test
on manual dispatch. It intentionally has no regression threshold because
GitHub-hosted runners are shared and variable.

Historical records are retained under `docs/benchmarks/results/`. They are not
current baselines unless their workload, environment, configuration, delivery
semantics, and measurement contract match.
