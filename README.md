# StreamForge

> Selective replication for Kafka. Filter, transform, redact, and route records
> between topics and clusters without deploying Kafka Connect.

[![Version](https://img.shields.io/badge/version-1.0.0-36d1c4.svg)](CHANGELOG.md)
[![CI](https://github.com/rahulbsw/streamforge/workflows/CI/badge.svg)](https://github.com/rahulbsw/streamforge/actions)
[![Docs](https://img.shields.io/badge/docs-GitHub%20Pages-38a3ff.svg)](https://rahulbsw.github.io/streamforge/)
[![License](https://img.shields.io/badge/license-Apache--2.0-d5dde4.svg)](LICENSE)

StreamForge moves only the records and fields that downstream systems need.
One source topic can feed analytics, lake, and lower-trust destinations with an
independent filter and transform for each route.

```text
Kafka source ──► filter ──► transform ──┬──► analytics topic
                                       └──► PII-safe topic
```

## Why StreamForge

- Route records by payload, key, headers, and timestamps.
- Reshape events and remove or hash sensitive fields before delivery.
- Fan out one source topic into destination-specific representations.
- Run opt-in, digest-pinned WebAssembly filters and transforms inside explicit
  resource and capability limits.
- Run as a standalone Rust binary or through the Kubernetes operator.
- Observe delivery, errors, lag, retries, and dead-letter records with
  Prometheus metrics.

## Run the local demo

Prerequisites: a Rust toolchain, Podman, and a Podman Compose provider.

```bash
podman compose -f examples/redpanda/docker-compose.yml up -d

cargo run --quiet --bin streamforge-validate -- \
  examples/redpanda/selective-replication.yaml

CONFIG_FILE=examples/redpanda/selective-replication.yaml \
  cargo run --release --bin streamforge
```

Keep StreamForge running, then follow the
[five-minute quickstart](docs/QUICKSTART.md) to create the topics, publish one
order, and inspect the two destination-specific outputs.

## Choose a path

| Goal | Start here |
| --- | --- |
| Understand the product boundary | [When to use StreamForge](docs/WHEN_TO_USE.md) |
| Build a selective replication pipeline | [Usage guide](docs/USAGE.md) |
| Learn the filter and transform language | [DSL reference](docs/ADVANCED_DSL_GUIDE.md) |
| Build a sandboxed custom filter or transform | [WebAssembly UDFs](docs/WASM_UDFS.md) |
| Deploy with Podman or Kubernetes | [Deployment guide](docs/DEPLOYMENT.md) |
| Configure TLS and SASL | [Security configuration](docs/SECURITY_CONFIGURATION.md) |
| Operate and troubleshoot a pipeline | [Operations](docs/OPERATIONS.md) |
| Browse the complete public documentation | [StreamForge documentation](https://rahulbsw.github.io/streamforge/) |

## Deployment modes

### Standalone

Use the binary or container when configuration is managed directly by your
deployment system. Start with [Podman](docs/DOCKER.md).

### Kubernetes

Use the operator and `StreamforgePipeline` custom resource when pipelines
should be managed declaratively. Start with
[Kubernetes](docs/KUBERNETES.md) or the
[Helm chart](helm/streamforge-operator/README.md).

## Compatibility and boundaries

StreamForge targets Kafka-compatible brokers. Kafka is the primary target in
the current documentation; Redpanda is covered for the selective-replication
workflows exercised by this repository.

StreamForge is not positioned as a replacement for MirrorMaker 2 active-active
replication and offset-sync workflows, or as a general-purpose stateful stream
processor. See [Compatibility](docs/COMPATIBILITY.md) for the tested scope.

## Performance policy

Performance depends on message shape, partitions, broker configuration,
delivery guarantees, and hardware. The public documentation therefore provides
[measurement and tuning guidance](docs/PERFORMANCE.md), not a universal
throughput claim. Results are published only after a reproducible, like-for-like
comparison passes record-count and delivery validation.

## Contributing

See the [contributing guide](docs/CONTRIBUTING.md) for development setup,
testing, and pull-request expectations. Use
[GitHub Discussions](https://github.com/rahulbsw/streamforge/discussions) for
questions and [GitHub Issues](https://github.com/rahulbsw/streamforge/issues)
for reproducible defects or feature proposals.

## License

StreamForge is licensed under the [Apache License 2.0](LICENSE).
