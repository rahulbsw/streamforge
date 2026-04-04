# Streamforge

**High-performance Kafka message mirroring and transformation toolkit built in Rust.**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)

---

## What Is Streamforge?

Streamforge is a Kafka-to-Kafka streaming service that mirrors, filters, transforms, and routes messages between clusters. It reads from one or more source topics, applies user-defined rules via a custom DSL, and writes the results to one or more destination topics -- potentially on a completely different Kafka cluster.

**Core capabilities:**

- **Cross-cluster mirroring** -- replicate messages between independent Kafka clusters.
- **Content-based filtering** -- evaluate JSON payloads, message keys, headers, and timestamps against filter expressions.
- **Message transformation** -- reshape payloads, extract fields, construct new objects, perform arithmetic, hash sensitive fields.
- **Multi-destination routing** -- route messages to different topics based on their content.
- **Envelope operations** -- filter and transform keys, headers, and timestamps, not just payloads.
- **Observability** -- Prometheus metrics endpoint with per-destination throughput, latency, error rates, and consumer lag.

## Use Cases

| Scenario | Description |
|---|---|
| **Cross-cluster replication** | Mirror production data to analytics or disaster-recovery clusters. |
| **Event routing** | Route events to topic-per-type (e.g., `meetings`, `calls`, `quality`) based on payload fields. |
| **Data redaction** | Hash or remove PII fields before forwarding to less-trusted environments. |
| **Header-based tenancy** | Filter messages by tenant header without parsing the payload. |
| **Schema slimming** | Extract only the fields downstream consumers need, reducing bandwidth. |
| **Time-window routing** | Route recent messages to real-time pipelines and older messages to batch pipelines. |
| **Key repartitioning** | Change message keys for different partitioning strategies per destination. |

## Quick Start

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# macOS dependencies
brew install cmake pkg-config openssl

# Linux (Debian/Ubuntu)
# apt-get install cmake pkg-config libssl-dev libsasl2-dev
```

### Build

```bash
cargo build --release
cargo test
```

### Minimal Configuration

Create `config.yaml`:

```yaml
appid: my-mirror
bootstrap: source-kafka:9092
target_broker: target-kafka:9092
input: source-topic
output: destination-topic
offset: latest
threads: 4
```

### Run

```bash
CONFIG_FILE=config.yaml ./target/release/streamforge
```

Format is auto-detected by file extension (`.yaml`, `.yml`, `.json`).

## Configuration

### Single Destination

```yaml
appid: streamforge
bootstrap: source-kafka:9092
target_broker: target-kafka:9092
input: events
output: events-copy
offset: latest
threads: 4
```

### Multi-Destination Routing

```yaml
appid: streamforge
bootstrap: source-kafka:9092
target_broker: target-kafka:9092
input: raw-events

routing:
  routing_type: filter
  destinations:
    - output: meetings
      filter: "/eventType,==,meeting.started"
      transform: "/data"
      description: "Meeting events"

    - output: quality-reports
      filter: "AND:/eventType,==,quality.report:/data/score,<,80"
      description: "Low-quality reports"

    - output: all-events
      description: "Catch-all"
```

### Envelope Operations

Filter and transform the full Kafka message envelope (key, headers, timestamp), not just the payload:

```yaml
routing:
  routing_type: filter
  destinations:
    - output: user-events
      filter: "KEY_PREFIX:user-"
      key_transform: "/user/id"
      headers:
        x-pipeline: "streamforge"
      header_transforms:
        - header: x-user-id
          operation: "FROM:/user/id"
      timestamp: "PRESERVE"

    - output: recent-events
      filter: "TIMESTAMP_AGE:<,300"
      timestamp: "CURRENT"
```

### Delivery Semantics

**At-least-once** (recommended):

```yaml
commit_strategy:
  manual_commit: true
  commit_mode: async
```

**At-most-once** (default, highest throughput):

```yaml
# No commit_strategy needed -- uses auto-commit
```

> Streamforge supports at-least-once and at-most-once delivery. Exactly-once is not currently supported.

### Observability

```yaml
observability:
  metrics_enabled: true
  metrics_port: 9090
  lag_monitoring_enabled: true
  lag_monitoring_interval_secs: 30
```

Exposes a Prometheus-compatible `/metrics` endpoint and a `/health` endpoint.

### Security

Full SSL/TLS, SASL (PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, GSSAPI/Kerberos, OAUTHBEARER) support:

```yaml
security:
  protocol: SASL_SSL
  ssl:
    ca_location: /path/to/ca.pem
  sasl:
    mechanism: SCRAM-SHA-256
    username: ${KAFKA_USER}
    password: ${KAFKA_PASS}
```

See [docs/SECURITY_CONFIGURATION.md](docs/SECURITY_CONFIGURATION.md) for full details.

## DSL Reference

### Filters

| Syntax | Description |
|---|---|
| `/path,op,value` | Compare JSON field (`>`, `>=`, `<`, `<=`, `==`, `!=`) |
| `AND:cond1:cond2` | All conditions must pass |
| `OR:cond1:cond2` | At least one condition must pass |
| `NOT:cond` | Invert a condition |
| `REGEX:/path,pattern` | Match field against regular expression |
| `ARRAY_ALL:/path,filter` | All array elements must match |
| `ARRAY_ANY:/path,filter` | At least one element must match |
| `KEY_PREFIX:prefix` | Message key starts with prefix |
| `KEY_SUFFIX:suffix` | Message key ends with suffix |
| `KEY_CONTAINS:sub` | Message key contains substring |
| `KEY_MATCHES:regex` | Message key matches regex |
| `KEY_EXISTS` | Message has a non-null key |
| `HEADER:name,op,value` | Compare header value (`==`, `!=`) |
| `HEADER_EXISTS:name` | Header exists |
| `TIMESTAMP_AGE:op,secs` | Message age in seconds (`<`, `<=`, `>`, `>=`) |
| `TIMESTAMP_AFTER:epoch_ms` | Timestamp after threshold |
| `TIMESTAMP_BEFORE:epoch_ms` | Timestamp before threshold |

### Transforms

| Syntax | Description |
|---|---|
| `/path` | Extract field or nested object |
| `CONSTRUCT:f1=/p1:f2=/p2` | Build new object from multiple paths |
| `ARRAY_MAP:/path,/element` | Map over array elements |
| `ARITHMETIC:op,left,right` | Arithmetic (`ADD`, `SUB`, `MUL`, `DIV`) |
| `HASH:algo,/path` | Hash field (`MD5`, `SHA256`, `SHA512`, `MURMUR64`, `MURMUR128`) |
| `HASH:algo,/path,out` | Hash field, store in `out`, preserve original |

### Key Transforms

| Syntax | Description |
|---|---|
| `/path` | Extract key from payload field |
| `CONSTRUCT:f1=/p1:f2=/p2` | Build JSON key from multiple fields |
| `HASH:algo,/path` | Hash a field as the key |
| `template-{/path}` | Template-based key construction |
| `CONSTANT:value` | Set a constant key |

### Header Transforms

| Syntax | Description |
|---|---|
| `FROM:/path` | Set header from payload field |
| `COPY:source-header` | Copy from another header |
| `REMOVE` | Remove the header |

### Timestamp Transforms

| Syntax | Description |
|---|---|
| `PRESERVE` | Keep original timestamp (default) |
| `CURRENT` | Set to current time |
| `FROM:/path` | Extract from payload field |
| `ADD:seconds` | Add seconds to original |
| `SUBTRACT:seconds` | Subtract seconds from original |

See [docs/ADVANCED_DSL_GUIDE.md](docs/ADVANCED_DSL_GUIDE.md) for comprehensive examples.

## Architecture

```
Source Kafka          Streamforge              Target Kafka
┌──────────┐    ┌──────────────────────┐    ┌──────────┐
│  topic-a  ├───►  Consumer            │    │ topic-x  │
│  topic-b  │    │    │                │    │ topic-y  │
└──────────┘    │    ▼                │    │ topic-z  │
                │  Processor           │    └──────────┘
                │  ├─ Filter (DSL)     │         ▲
                │  ├─ Envelope Ops     │         │
                │  └─ Transform (DSL)  │         │
                │    │                │         │
                │    ▼                │         │
                │  KafkaSink(s) ──────┼─────────┘
                └──────────────────────┘
```

- **Consumer**: Reads from source Kafka using `rdkafka` `StreamConsumer`. Supports batch collection with configurable size and timeout.
- **Processor**: Applies per-destination filter, envelope transforms, and value transforms. Each destination has its own `DestinationProcessor`.
- **KafkaSink**: Writes to target Kafka via `FutureProducer`. Supports custom partitioning, compression (Gzip, Snappy, Zstd), and full header/timestamp propagation.

## Performance

### DSL Micro-Benchmarks (Measured)

```bash
cargo bench
```

Results on Apple M-series (March 2026):

| Operation | Latency | Throughput |
|---|---|---|
| Simple filter | 43-50 ns | ~21M ops/s |
| Boolean logic (AND/OR) | 47-145 ns | 7-21M ops/s |
| Regex filter | 47-59 ns | 17-21M ops/s |
| Array operations | 57-101 ns | 10-18M ops/s |
| Object construction | 908-1,414 ns | 0.7-1.1M ops/s |
| Arithmetic | 816-864 ns | 1.2M ops/s |

See [benchmarks/results/BENCHMARK_RESULTS.md](benchmarks/results/BENCHMARK_RESULTS.md) for full data.

### End-to-End Throughput (Measured)

| Configuration | Sustained | Peak |
|---|---|---|
| 4 threads, at-least-once | ~11,000 msg/s | -- |
| 8 threads, at-least-once | 25,000-30,000 msg/s | 34,500 msg/s |

See [benchmarks/results/](benchmarks/results/) for detailed analysis.

### Compression Support

| Algorithm | Native Kafka | Enveloped | Status |
|---|---|---|---|
| Gzip | Yes | Yes | Implemented |
| Snappy | Yes | Yes | Implemented |
| Zstd | Yes | Yes | Implemented |
| LZ4 | Yes (via Kafka) | No | Native only |

> **Note**: LZ4 is supported as native Kafka producer compression (handled by librdkafka). Application-level (enveloped) LZ4 compression is not yet implemented.

## Metrics

### Console Stats

Reported every 10 seconds:

```
Stats: processed=10000 (1000.0/s), filtered=100 (10.0/s),
       completed=9900 (990.0/s), errors=0 (0.0/s)
```

### Prometheus Metrics

When `metrics_enabled: true`, available at `http://localhost:9090/metrics`:

```promql
rate(streamforge_messages_consumed_total[5m])
sum(rate(streamforge_messages_produced_total[5m])) by (destination)
streamforge_consumer_lag{topic="...", partition="..."}
histogram_quantile(0.99, rate(streamforge_processing_duration_seconds_bucket[5m]))
rate(streamforge_processing_errors_total[5m])
```

See [docs/OBSERVABILITY_QUICKSTART.md](docs/OBSERVABILITY_QUICKSTART.md) for Prometheus + Grafana setup.

## Documentation

| Document | Description |
|---|---|
| [QUICKSTART.md](docs/QUICKSTART.md) | Get running in 5 minutes |
| [USAGE.md](docs/USAGE.md) | 8 real-world use cases |
| [ADVANCED_DSL_GUIDE.md](docs/ADVANCED_DSL_GUIDE.md) | Complete DSL reference |
| [ADVANCED_FILTERS.md](docs/ADVANCED_FILTERS.md) | Boolean logic (AND/OR/NOT) |
| [ENVELOPE_MIGRATION_GUIDE.md](docs/ENVELOPE_MIGRATION_GUIDE.md) | Envelope features migration |
| [YAML_CONFIGURATION.md](docs/YAML_CONFIGURATION.md) | YAML vs JSON configuration |
| [SECURITY_CONFIGURATION.md](docs/SECURITY_CONFIGURATION.md) | SSL/TLS, SASL, Kerberos |
| [OBSERVABILITY_QUICKSTART.md](docs/OBSERVABILITY_QUICKSTART.md) | Metrics setup (Prometheus + Grafana) |
| [OBSERVABILITY_METRICS_DESIGN.md](docs/OBSERVABILITY_METRICS_DESIGN.md) | Complete metrics reference |
| [DOCKER.md](docs/DOCKER.md) | Docker and Kubernetes deployment |
| [PERFORMANCE.md](docs/PERFORMANCE.md) | Performance tuning |
| [SCALING.md](docs/SCALING.md) | Horizontal and vertical scaling |
| [CONTRIBUTING.md](docs/CONTRIBUTING.md) | Development setup and guidelines |
| [DOCUMENTATION_INDEX.md](docs/DOCUMENTATION_INDEX.md) | Full documentation index |

### Examples

See [examples/](examples/) for configuration files covering single-destination, multi-destination, envelope operations, observability, and Kubernetes deployment.

## Future Enhancements

- [ ] Avro serialization and Schema Registry integration
- [ ] Dead letter queue for failed messages
- [ ] Application-level LZ4 compression
- [ ] Nested transform composition

## Contributing

```bash
git clone https://github.com/rahulbsw/streamforge.git
cd streamforge
cargo build
cargo test
cargo bench
```

See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for guidelines.

## License

Apache License 2.0 -- see [LICENSE](LICENSE) for details.

Copyright 2025 Rahul Jain
