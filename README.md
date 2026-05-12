# StreamForge

> Selective replication for Kafka and Redpanda. Filter, transform, redact, and route data between topics and clusters without Kafka Connect.

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-1.0.0-brightgreen.svg)](docs/CHANGELOG.md)
[![Kafka](https://img.shields.io/badge/broker-Kafka-black.svg)](#compatibility)
[![Redpanda](https://img.shields.io/badge/broker-Redpanda-red.svg)](#compatibility)
[![CI](https://github.com/rahulbsw/streamforge/workflows/CI/badge.svg)](https://github.com/rahulbsw/streamforge/actions)

---

StreamForge helps data teams move only the records and fields downstream systems actually need. Instead of mirroring whole topics, StreamForge lets you filter, reshape, redact, and route messages before they land in analytics, lake, or lower-trust environments.

**[5-Minute Demo](#5-minute-demo)** | **[When to Use StreamForge](#when-to-use-streamforge)** | **[Compatibility](#compatibility)** | **[Documentation Index](docs/DOCUMENTATION_INDEX.md)**

---

## Why Teams Use StreamForge

- Replicate only analytics-safe fields instead of whole topics
- Split one source topic into multiple downstream topics
- Hash or drop PII before data crosses trust boundaries
- Keep the deployment surface small with a single binary, operator, and Helm chart

## When to Use StreamForge

Use StreamForge when you need:
- selective replication to analytics or data lake pipelines
- PII-safe replication across environments
- topic fan-out with payload shaping
- a smaller operational footprint than Kafka Connect

Do not position StreamForge as:
- a full replacement for MirrorMaker 2 active-active or offset-sync workflows
- a general-purpose stateful stream processor

For concrete usage patterns and configs, see [docs/USAGE.md](docs/USAGE.md) and [examples/README.md](examples/README.md).

## 5-Minute Demo

1. Start a local Redpanda broker:
   ```bash
   docker run --rm -d --name redpanda \
     -p 9092:9092 -p 9644:9644 \
     docker.redpanda.com/redpandadata/redpanda:v24.1.10 \
     redpanda start --overprovisioned --smp 1 --memory 1G --reserve-memory 0M \
     --node-id 0 --check=false \
     --kafka-addr 0.0.0.0:9092 \
     --advertise-kafka-addr localhost:9092
   ```
2. Validate the selective replication config:
   ```bash
   cargo run --quiet --bin streamforge-validate -- examples/configs/config.filter-transform.example.json
   ```
3. Run StreamForge with the same config:
   ```bash
   CONFIG_FILE=examples/configs/config.filter-transform.example.json cargo run --release --bin streamforge
   ```
4. For the fuller local walkthrough and additional configs, see [docs/QUICKSTART.md](docs/QUICKSTART.md) and [examples/README.md](examples/README.md).

## Production Trust Signals

- At-least-once delivery with retry and DLQ support
- Native Prometheus metrics and lag monitoring
- Kubernetes operator, Helm chart, and web UI
- Kafka-first examples for standalone configs and Kubernetes pipelines

## Compatibility

StreamForge is built for Kafka-compatible brokers. Kafka is the primary target in current docs and examples, and the launch story positions Redpanda as a compatible selective-replication destination.

---

## Core Capabilities

- Content-based filtering across payload, key, headers, and timestamps
- Field extraction, reshaping, and PII hashing before downstream delivery
- Topic fan-out from one source topic to multiple destination topics
- At-least-once delivery with retry, DLQ handling, and observability hooks
- Standalone binary and Kubernetes operator deployment modes

## Example Pipelines

- [examples/configs/config.example.yaml](examples/configs/config.example.yaml) for a minimal standalone pipeline
- [examples/configs/config.multidest.yaml](examples/configs/config.multidest.yaml) for multi-destination routing
- [examples/production/pii-redaction.yaml](examples/production/pii-redaction.yaml) for analytics-safe redaction
- [examples/production/cdc-to-datalake.yaml](examples/production/cdc-to-datalake.yaml) for CDC-to-lake shaping
- [examples/pipelines/README.md](examples/pipelines/README.md) for operator-backed Kubernetes manifests

## Deploy and Operate

- [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for deployment patterns
- [docs/OPERATIONS.md](docs/OPERATIONS.md) for production runbooks
- [docs/OBSERVABILITY_QUICKSTART.md](docs/OBSERVABILITY_QUICKSTART.md) for Prometheus and lag monitoring
- [docs/SECURITY_CONFIGURATION.md](docs/SECURITY_CONFIGURATION.md) for TLS and SASL setup
- [helm/streamforge-operator/README.md](helm/streamforge-operator/README.md) for Helm-based installs

## Learn More

- [docs/QUICKSTART.md](docs/QUICKSTART.md) for the first local run
- [docs/USAGE.md](docs/USAGE.md) for deployment patterns and use cases
- [docs/YAML_CONFIGURATION.md](docs/YAML_CONFIGURATION.md) for config structure and format guidance
- [docs/ADVANCED_DSL_GUIDE.md](docs/ADVANCED_DSL_GUIDE.md) for the full filtering and transform DSL
- [docs/DOCUMENTATION_INDEX.md](docs/DOCUMENTATION_INDEX.md) for the broader doc set

## Contributing

Contribution and development setup are documented in [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).

## License

Apache License 2.0. See [LICENSE](LICENSE) for details.
