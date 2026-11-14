# WASM UDF Local Benchmark Record — 2026-07-25

This record contains the pre-change reference and post-change local evidence
for the `feature/wasm-udf` branch. It is development evidence, not a production
throughput claim or a cross-machine release gate.

## Pre-change provenance

- Source commit: `6896e971931d9ca62b1725beb5e7336998c0ce61`
- Host: Apple arm64, Darwin 25.5.0
- Rust: `rustc 1.89.0 (29483883e 2025-08-04)`
- Commands:
  - `cargo test --all --locked`
  - `cargo bench --bench transform_benchmarks -- --noplot`
  - `cargo bench --bench end_to_end_benchmark -- --noplot`

The full test baseline passed:

- library: 399 passed, 5 ignored;
- main binary: 39 passed;
- integration tests: 23 passed;
- documentation tests: 13 passed, 25 ignored.

## Native transform reference

Criterion median estimates:

| Benchmark | Median |
|---|---:|
| Extract field | 833.71 ns |
| Construct medium object | 1.0142 µs |
| Array map, simple | 1.5976 µs |
| Combined filter and transform | 844.17 ns |
| Complex filter and construct | 956.52 ns |
| Simple transform, 10,000 elements | 1.2092 million elements/s |
| Construct transform, 10,000 elements | 1.0268 million elements/s |

## Synthetic pipeline reference

Criterion median estimates:

| Payload | Parse | Serialize | Parse + serialize | One native transform |
|---|---:|---:|---:|---:|
| 256 B | 508.40 ns | 187.47 ns | 693.49 ns | 201.01 ns |
| 4 KiB | 864.11 ns | 1.3386 µs | 2.2914 µs | 263.26 ns |
| 64 KiB | 6.3496 µs | 18.283 µs | 25.422 µs | 237.58 ns |

The UDF implementation must compare no-UDF results against this same benchmark
shape, but final regression decisions require same-revision A/B runs on the
controlled benchmark host. Shared local-run timing is retained only to detect
large implementation mistakes while developing.

## Post-change method

The post-change worktree was dirty and included the concurrent WASM UDF feature
work. Measurements ran on the same local host with:

- `cargo bench --bench wasm_udf_benchmarks --locked --offline -- --noplot`
- `cargo bench --bench transform_benchmarks --locked --offline -- --noplot`
- `cargo bench --bench end_to_end_benchmark --locked --offline -- --noplot`

The checked-in component fixtures were verified against
`tests/fixtures/wasm/generated/SHA256SUMS` before use. Successful UDF calls used
a 100 ms execution ceiling so host scheduler stalls did not masquerade as guest
timeouts. The timeout benchmark used a separate 1 ms ceiling. All fan-out calls
were sequential. Concurrency remains integration-test evidence rather than a
Criterion microbenchmark.

After the security suite added its final stack-exhaustion and malicious
guest-message paths, the complete WASM target was rerun against filter fixture
SHA-256
`a7816141d01935a8aa5f65ab6f13d4ed7c12a5cee9dfba061efa3af1b11e7517`.
The final merge-readiness run used patched Wasmtime `36.0.10`; the results
below are from that final-fixture, patched-runtime run.

The native no-UDF budget is evaluated on
`one_destination_passthrough_preparation`, which parses the same synthetic JSON
bytes and creates one destination envelope without loading or invoking a UDF.
The local Criterion cache comparison reported:

| Payload | Post-change median | Estimated change | 95% change interval | Result |
|---|---:|---:|---:|---|
| 256 B | 540.74 ns | +0.86% | -1.29% to +2.87% | No change |
| 4 KiB | 896.44 ns | +0.78% | -0.66% to +2.26% | No change |
| 64 KiB | 6.5154 µs | +0.48% | -0.62% to +1.44% | No change |

Every interval remained below the +3% native no-UDF budget. This is a local
development pass only; the release decision still requires fresh matched A/B
runs on controlled native hardware.

The final-fixture full run measured 547.37 ns, 916.83 ns, and 6.7883 µs for
256 B, 4 KiB, and 64 KiB. Its 64 KiB cached comparison was temporarily above
3% while the full suite was running. An immediate isolated rerun of the same
native-only group measured 538.80 ns, 922.37 ns, and 6.5698 µs; Criterion
classified every comparison as no change or within the configured noise
threshold. This variability is why these local results remain diagnostic and
the controlled matched A/B requirement is not waived.

After upgrading Wasmtime from `36.0.6` to the security-patched `36.0.10`, an
isolated no-UDF rerun measured 547.61 ns, 902.95 ns, and 6.3934 µs. The 256 B
median was 1.27% above the recorded 540.74 ns feature baseline; the 4 KiB case
was statistically unchanged and the 64 KiB case remained within noise. No
patched-runtime control exceeded the 3% local diagnostic budget against the
recorded feature baseline.

## Post-change WASM UDF results — Wasmtime 36.0.10

Criterion median estimates:

| Operation | 256 B | 4 KiB | 64 KiB |
|---|---:|---:|---:|
| Filter, one destination | 15.513 µs | 78.339 µs | 1.0415 ms |
| Filter, four destinations | 63.723 µs | 302.63 µs | 4.1406 ms |
| Value transform, one destination | 13.851 µs | 35.358 µs | 386.34 µs |
| Value transform, four destinations | 54.235 µs | 151.59 µs | 1.5457 ms |
| Envelope transform, one destination | 13.277 µs | 14.704 µs | 44.655 µs |
| Envelope transform, four destinations | 51.131 µs | 59.747 µs | 183.65 µs |

Startup `load + verify + compile + link + probe + shutdown` measured
19.017 ms. Failure-path medians were 11.144 µs for a filter guest error,
12.083 µs for a value-transform guest error, 12.186 µs for an output-limit
violation, and 1.2531 ms for the deliberate infinite-loop timeout with a 1 ms
budget. A noisy isolated rerun of the four-destination 4 KiB value transform
was statistically unchanged; controlled matched A/B hardware remains
authoritative for a release decision.

## Wasmtime 36.0.13 security patch check — 2026-08-12

Wasmtime was upgraded from `36.0.10` to `36.0.13` for
`RUSTSEC-2026-0222`. A same-session Apple arm64 Criterion comparison ran the
complete `wasm_udf_benchmarks` suite under `caffeinate`, with `36.0.13` saved
first and `36.0.10` compared against it before the worktree was restored to
`36.0.13`.

Criterion detected no performance change in startup or any of the 256 B,
4 KiB, and 64 KiB native no-UDF controls. The older `36.0.10` runtime was
statistically slower in multiple filter, value-transform, and
envelope-transform invocation cases; failure paths were mixed or unchanged.
This is favorable local diagnostic evidence, not a release claim. The
controlled-hardware matched A/B requirement remains unchanged, and Criterion
runtime output was not added to version control.

## Post-change native reference comparison

The unchanged native controls were rerun after the feature work. Median deltas
below are against the pre-change values above.

| Benchmark | Pre-change | Post-change | Median delta |
|---|---:|---:|---:|
| Extract field | 833.71 ns | 852.33 ns | +2.23% |
| Construct medium object | 1.0142 µs | 1.0159 µs | +0.17% |
| Array map, simple | 1.5976 µs | 1.5887 µs | -0.56% |
| Combined filter and transform | 844.17 ns | 857.89 ns | +1.63% |
| Complex filter and construct | 956.52 ns | 977.27 ns | +2.17% |
| Simple transform, 10,000 elements | 1.2092 M elem/s | 1.2057 M elem/s | -0.29% |
| Construct transform, 10,000 elements | 1.0268 M elem/s | 1.0176 M elem/s | -0.90% |

| Payload | Operation | Pre-change | Post-change | Median delta |
|---|---|---:|---:|---:|
| 256 B | Parse | 508.40 ns | 509.10 ns | +0.14% |
| 256 B | Serialize | 187.47 ns | 193.98 ns | +3.47% |
| 256 B | Parse + serialize | 693.49 ns | 711.94 ns | +2.66% |
| 256 B | One native transform | 201.01 ns | 205.37 ns | +2.17% |
| 4 KiB | Parse | 864.11 ns | 875.14 ns | +1.28% |
| 4 KiB | Serialize | 1.3386 µs | 1.4524 µs | +8.50% |
| 4 KiB | Parse + serialize | 2.2914 µs | 2.2711 µs | -0.89% |
| 4 KiB | One native transform | 263.26 ns | 286.51 ns | +8.83% |
| 64 KiB | Parse | 6.3496 µs | 6.4613 µs | +1.76% |
| 64 KiB | Serialize | 18.283 µs | 17.880 µs | -2.20% |
| 64 KiB | Parse + serialize | 25.422 µs | 25.801 µs | +1.49% |
| 64 KiB | One native transform | 237.58 ns | 240.58 ns | +1.26% |

The 256 B serialization, 4 KiB serialization, and 4 KiB native-transform
point estimates exceeded 3%. Criterion classified all three as either
within-noise or no-change in the same-host cached comparison; the latter two
also had broad intervals. These are retained as noisy diagnostic outliers and
must be resolved by controlled A/B evidence before treating the broader native
microbenchmark set as a release gate.
