# Benchmark Records

Current benchmark instructions and comparison rules are maintained in
[`../PERFORMANCE_TESTING.md`](../PERFORMANCE_TESTING.md).

The `results/` directory contains historical artifacts. A historical report is
not a current baseline unless its commit, workload, configuration, environment,
and measurement method match the new run.

The canonical Kafka-backed runner is:

```bash
scripts/benchmarks/run_throughput_test.sh
```

Criterion microbenchmarks remain under `benches/`.

[`results/phase2-baseline-20260724.md`](results/phase2-baseline-20260724.md) is
a historical coupled-harness record. No schema-version-3 sustained result is a
public baseline yet; the first validated local run was intentionally withheld
because its worktree and saturation gates were not publication-eligible.
