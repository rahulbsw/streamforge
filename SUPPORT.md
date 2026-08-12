# Support

## Start here

- [Quick Start](docs/QUICKSTART.md): run the local selective-replication demo.
- [Usage](docs/USAGE.md): supported use cases and examples.
- [Configuration](docs/YAML_CONFIGURATION.md): YAML reference.
- [Troubleshooting](docs/TROUBLESHOOTING.md): diagnosis and recovery.
- [Documentation site](https://rahulbsw.github.io/streamforge/): curated user
  and operator documentation.

Search [existing issues](https://github.com/rahulbsw/streamforge/issues) before
opening a new report.

## Choose a channel

- [GitHub Discussions](https://github.com/rahulbsw/streamforge/discussions):
  questions, ideas, and community support.
- [GitHub Issues](https://github.com/rahulbsw/streamforge/issues/new):
  reproducible bugs and feature proposals.
- `rahul.oracle.db@gmail.com`: private or commercial inquiries. Use the subject
  `Commercial Support Inquiry - StreamForge` for deployment, configuration,
  custom development, performance, training, workshops, or SLA-backed support.
- [Security Policy](SECURITY.md): private vulnerability reporting. Never put a
  vulnerability or credential in a public issue.

All interactions follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Include enough evidence

Provide:

- StreamForge version and the exact commit or image tag;
- operating system, architecture, deployment mode, and Rust version when built
  from source;
- relevant configuration with credentials and private endpoints redacted;
- complete errors and stack traces with secrets redacted;
- minimal reproduction steps; and
- expected and actual behavior.

For performance reports, also include the workload, message size, partitions,
broker topology, delivery settings, hardware, throughput, latency, resource
use, and record-count validation. See [Performance](docs/PERFORMANCE.md).

## Topic-specific help

- Filters and transforms: [DSL reference](docs/ADVANCED_DSL_GUIDE.md)
- Performance and operations: [Performance](docs/PERFORMANCE.md) and
  [Operations](docs/OPERATIONS.md)
- TLS, SASL, and secrets: [Security configuration](docs/SECURITY_CONFIGURATION.md)
- Containers: [Podman guide](docs/DOCKER.md)
- Kubernetes: [Kubernetes guide](docs/KUBERNETES.md)
- Contributing: [Contributing guide](docs/CONTRIBUTING.md)

## Response targets

| Channel | Target |
| --- | --- |
| Security reports | 24–48 hours |
| Bug reports | 2–5 business days |
| Feature proposals | 1–2 weeks |
| Questions | 2–7 days |
| Pull requests | 1–2 weeks |

These are estimates and vary with complexity and maintainer availability. For
other direct inquiries, the target is 48 hours.

Project links: [repository](https://github.com/rahulbsw/streamforge),
[changelog](CHANGELOG.md), [Apache Kafka](https://kafka.apache.org/), and
[rdkafka-rust](https://github.com/fede1024/rust-rdkafka).
