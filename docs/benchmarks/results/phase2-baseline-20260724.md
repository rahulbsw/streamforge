# Phase 2 Performance Baselines — 2026-07-24 and 2026-07-25

This record captures the local and dedicated-AWS baselines produced by the
current deterministic Phase 2 benchmark contract. They are evidence for future
comparisons, not portable throughput or production-capacity claims.

## Local baseline environment

| Field | Value |
|---|---|
| Git commit | `27019982bfd35569ba04fee18b3fa657245b0dd7` |
| Worktree | Dirty; includes the uncommitted Phase 1 and Phase 2 changes under test |
| OS | Darwin 25.5.0 arm64 |
| CPU | Apple M4 Pro |
| Rust | rustc 1.89.0 (29483883e 2025-08-04) |
| Kafka | `confluentinc/cp-kafka:7.5.0` |
| Criterion baseline name | `phase2-current` |

## Synthetic pipeline

Criterion point estimates from `end_to_end_benchmark`; all values are mean time
per iteration. These are in-memory stage measurements and exclude Kafka.

| Stage | 256 B | 4 KiB | 64 KiB |
|---|---:|---:|---:|
| Parse bytes | 564.72 ns | 893.55 ns | 6.440 µs |
| Serialize value | 206.73 ns | 1.297 µs | 17.483 µs |
| Parse + serialize | 806.06 ns | 2.283 µs | 24.425 µs |
| One-destination preparation | 711.46 ns | 1.030 µs | 6.406 µs |
| Four-destination `Arc` fan-out | 118.34 ns | 115.74 ns | 120.19 ns |
| One path filter | 13.17 ns | 12.05 ns | 11.74 ns |
| One compiled regex filter | 26.48 ns | 27.74 ns | 26.39 ns |
| One value transform | 192.01 ns | 245.74 ns | 216.06 ns |

All 24 synthetic cases completed. The filter and transform suites were also
saved under `phase2-current`; their full machine-readable estimates remain in
`target/criterion/`.

Interpretation is limited: parse/serialization costs grow with this fixture's
payload size, while `Arc` fan-out and field-local DSL operations are effectively
size-independent. This local run alone did not identify the next production
optimization; the dedicated whole-process profile later in this record provides
that evidence.

## Kafka-backed passthrough

Command:

```bash
scripts/benchmarks/run_throughput_test.sh 10000 4 4 3
```

Dataset:

- seed: `0`
- records: `10,000`
- bytes: `2,171,890`
- SHA-256: `cac31952a83f74541f4870d390c6663bef71ac9428678f72d2c72aa355cc337b`

| Repetition | Duration | Completion throughput | Consumed | Output records | Errors |
|---:|---:|---:|---:|---:|---:|
| 1 | 6.855745458 s | 1,458.630584 msg/s | 10,000 | 10,000 | 0 |
| 2 | 6.666970666 s | 1,499.931603 msg/s | 10,000 | 10,000 | 0 |
| 3 | 6.685788458 s | 1,495.709902 msg/s | 10,000 | 10,000 | 0 |

Summary:

- median: `1,495.709902 msg/s`
- minimum: `1,458.630584 msg/s`
- maximum: `1,499.931603 msg/s`
- all repetitions passed exact consumed/output counts with zero processing
  errors

The raw structured result is retained locally at
`target/performance-results/throughput/baseline-20260724T231740Z-81698/result.json`.
Because `target/` is not versioned, this document preserves the reviewable
record; rerun the harness for machine-readable comparison evidence.

## Dedicated AWS x86_64 validation

The same dirty source snapshot was packaged as a Git bundle plus a binary patch
and run on dedicated On-Demand compute on 2026-07-25. Inputs were verified by
SHA-256 before compilation.

| Field | Value |
|---|---|
| Run ID | `sf-perf-20260725T011032Z` |
| Git commit | `27019982bfd35569ba04fee18b3fa657245b0dd7` |
| Worktree | Dirty; includes the uncommitted Phase 1 and Phase 2 changes under test |
| Bundle SHA-256 | `f1461c4a3fcb0d60f82aa73083d4a87f2863dff95ad7755295b24442513b7203` |
| Patch SHA-256 | `ffd800a80d95280b919caae7c1af34453c27972b1e4d0a463d5ac61e8c829803` |
| Region / AZ | `us-west-2` / `us-west-2a` |
| Instance | `c7i.2xlarge` On-Demand; 8 vCPU |
| CPU | Intel Xeon Platinum 8488C; 4 cores / 8 threads |
| AMI | `ami-0f6de954b71901fb8`; Amazon Linux 2023, kernel `6.1.176-221.367.amzn2023.x86_64` |
| Rust | rustc 1.89.0; release debug info and forced frame pointers enabled |
| Kafka image | `confluentinc/cp-kafka@sha256:fbbb6fa11b258a88b83f54d4f0bddfcffbf2279f99d66a843486e3da7bdfbf41` |
| ZooKeeper image | `confluentinc/cp-zookeeper@sha256:02f6c042bb9a7844382fc4cedc513a44585d8a5acae873fb9e510e3ca9dcabc6` |
| Criterion baseline name | `aws-c7i-phase2` |

`cargo test --all --locked` passed with 462 tests, 0 failures, and 30 ignored
tests. All three Criterion targets completed.

### Synthetic pipeline on c7i

Representative 64 KiB point estimates:

| Stage | Estimate | Throughput where reported |
|---|---:|---:|
| Parse bytes | 11.152 µs | 5.473 GiB/s |
| Serialize value | 35.246 µs | 1.732 GiB/s |
| Parse + serialize | 47.853 µs | 1.276 GiB/s |
| One-destination preparation | 11.153 µs | 5.472 GiB/s |
| Four-destination `Arc` fan-out | 196.89 ns | 20.316 Melem/s |
| One path filter | 15.276 ns | 65.462 Melem/s |
| One compiled regex filter | 37.971 ns | 26.336 Melem/s |
| One value transform | 553.45 ns | 1.807 Melem/s |

Standalone filter throughput remained nearly flat from 100 through 10,000
elements: about 40.7–41.2 Melem/s for the simple case and 12.39 Melem/s for the
complex case. Transform throughput remained about 0.85 Melem/s for the simple
case, 0.68 Melem/s for object construction, and 0.84 Melem/s for arithmetic.

These measurements must not be compared directly with the Apple M4 Pro values
as an architecture ranking: the CPU, operating system, container runtime, and
host conditions differ.

### Kafka-backed passthrough on c7i

Command contract:

```bash
scripts/benchmarks/run_throughput_test.sh 100000 8 8 5
```

Dataset:

- seed: `0`
- records: `100,000`
- bytes: `21,908,890`
- SHA-256: `197c16d749df0961f06913c836a6ed944c5ee30ed5857842cffe2daf6bb8131f`

| Repetition | Duration | Completion throughput |
|---:|---:|---:|
| 1 | 17.704800390 s | 5,648.185678 msg/s |
| 2 | 16.177066434 s | 6,181.590488 msg/s |
| 3 | 15.955118479 s | 6,267.581161 msg/s |
| 4 | 15.927486592 s | 6,278.454508 msg/s |
| 5 | 15.987597286 s | 6,254.848569 msg/s |

Summary:

- median: `6,254.848569 msg/s`
- minimum: `5,648.185678 msg/s`
- maximum: `6,278.454508 msg/s`
- exact consumed/output counts and zero processing errors in every repetition

A separate 200,000-message, 8-partition, 8-thread profiling run completed at
`7,094.333485 msg/s`. Its single repetition is profile evidence, not a
replacement for the five-run throughput summary.

### Whole-process CPU profile

`perf` recorded 614 cycle samples with zero lost samples while attached to the
StreamForge process. Inclusive samples identify these relevant paths:

- async collection and `buffer_unordered` polling: `59.69%` and `54.86%`
  respectively, with overlapping call stacks;
- Kafka sink send path: `17.87%`;
- JSON parsing into `serde_json::Value`: `16.49%`;
- libc allocation: `16.09%`, with `_int_malloc` at `14.59%`;
- envelope drop: `6.72%`;
- JSON serialization in the sink path: `3.17%`.

Inclusive percentages overlap and must not be summed. Parsing plus serialization
does not meet the roadmap's 30% threshold for prioritizing a raw/lazy envelope
rewrite. The next measured work should focus on allocation/envelope lifecycle,
Kafka send/batching behavior, and async scheduling. No dominant vectorizable
kernel appears in this profile, so a general SIMD rewrite is not supported by
the evidence.

### Isolation, cost, and cleanup

- The temporary security group had no inbound rules and only TCP/443 egress.
- No SSH key, IPv6 address, NAT gateway, Elastic IP, load balancer, or public S3
  access was used. Kafka was published only on instance loopback.
- IMDSv2 was required; the 40 GiB gp3 root volume was encrypted and configured
  for deletion on termination.
- Attempt 1 ran from 01:17:46 to 01:20:20 UTC and stopped after a missing build
  package was detected. Attempt 2 ran from 01:24:51 to 01:41:08 UTC and passed.
- At the observed On-Demand rate of `$0.357/hour`, estimated EC2 compute was
  `$0.1122`. Estimated public IPv4 and prorated gp3 charges add about `$0.0030`;
  short-lived S3 storage and requests were below `$0.001`. Rounded total
  estimated run cost: `$0.12`.
- All 60 successful-run payloads passed the downloaded checksum manifest before
  deletion. Both instances terminated, both volumes and public IPs were
  released, and the S3 bucket, scheduler, IAM objects, and isolated VPC/network
  resources were deleted.

The verified successful-run artifacts were downloaded to
`/private/tmp/streamforge-aws-perf.QP2hDE/attempt2/` during the run. This dated
record is the durable summary; temporary-directory retention is not guaranteed.
