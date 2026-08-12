# Streamforge Scripts

Utility scripts for testing and managing Streamforge.

## quickstart.sh

Runs the supported local Docker/Podman lifecycle:

```bash
scripts/quickstart.sh up
scripts/quickstart.sh verify
scripts/quickstart.sh down
```

## tests/minikube_podman_smoke.sh

Runs the isolated source-built Kubernetes integration test with rootless
Podman and Minikube `v1.38.1` or newer:

```bash
MINIKUBE_BIN=/path/to/minikube \
  scripts/tests/minikube_podman_smoke.sh
```

The harness refuses existing profiles, validates an exact replicated record,
`Ready`, health/readiness/metrics, security contexts, bounded reconciliation,
and owner garbage collection, and removes all isolated test resources.

## benchmarks/run_throughput_test.sh

Runs the maintained Kafka-backed sustained throughput harness:

```bash
podman compose -f docker-compose.benchmark.yml up -d
scripts/benchmarks/run_throughput_test.sh
podman compose -f docker-compose.benchmark.yml down -v
```

See [`../docs/PERFORMANCE_TESTING.md`](../docs/PERFORMANCE_TESTING.md) for the
measurement and publication contract.
