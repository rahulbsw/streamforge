# Benchmark configurations

These three engine configurations are diagnostic inputs for the supported
benchmark tooling. They do not define capacity targets or publication-eligible
results.

## Profiles

- `throughput-8thread.yaml` — eight-thread passthrough with acknowledged
  delivery, manual commit, and zstd compression.
- `latency-optimized.yaml` — low-batching passthrough with auto commit.
- `filter-transform.yaml` — four filtered/transformed destinations with manual
  commit, retry, and DLQ.

Validate each profile before use:

```bash
streamforge-validate examples/benchmarks/throughput-8thread.yaml
streamforge-validate examples/benchmarks/latency-optimized.yaml
streamforge-validate examples/benchmarks/filter-transform.yaml
```

Run an engine config with the stable entry point:

```bash
CONFIG_FILE=examples/benchmarks/throughput-8thread.yaml streamforge
```

Create isolated topics and consumer groups for every run. Record the exact
commit, image digest, configuration, payload distribution, partitions,
acknowledgement/commit modes, warm-up, timed window, exact ingress/consumed/
produced/delivered/output counts, errors, latency percentiles, CPU, peak RSS,
and teardown evidence.

The publication workflow and workload/mode catalog are under
`scripts/benchmarks/`. Release results must pass
`performance_release_gate.py`; ad hoc runs remain diagnostic.

See [Performance](../../docs/PERFORMANCE.md) and the canonical
[benchmark guide](../../docs/benchmarks/README.md).
