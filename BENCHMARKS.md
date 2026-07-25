# StreamForge Benchmarks

The canonical benchmark instructions are in
[`docs/PERFORMANCE_TESTING.md`](docs/PERFORMANCE_TESTING.md).

StreamForge distinguishes:

- Criterion microbenchmarks for isolated code paths;
- synthetic pipeline benchmarks that do not include Kafka;
- Kafka-backed completion-throughput runs produced by
  `scripts/benchmarks/run_throughput_test.sh`.

Historical reports under `docs/benchmarks/results/` are retained as historical
artifacts. They are not current baselines unless the workload, environment,
configuration, and measurement method match.
