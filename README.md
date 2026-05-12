# StreamForge

> Selective replication for Kafka and Redpanda. Filter, transform, redact, and route data between topics and clusters without Kafka Connect.

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-1.0.0-brightgreen.svg)](CHANGELOG.md)
[![Kafka](https://img.shields.io/badge/broker-Kafka-black.svg)](#compatibility)
[![Redpanda](https://img.shields.io/badge/broker-Redpanda-red.svg)](#compatibility)
[![CI](https://github.com/rahulbsw/streamforge/workflows/CI/badge.svg)](https://github.com/rahulbsw/streamforge/actions)

---

StreamForge helps data teams move only the records and fields downstream systems actually need. Instead of mirroring whole topics, StreamForge lets you filter, reshape, redact, and route messages before they land in analytics, lake, or lower-trust environments.

**[5-Minute Demo](docs/QUICKSTART.md)** | **[When to Use StreamForge](docs/WHEN_TO_USE.md)** | **[Compatibility](docs/COMPATIBILITY.md)** | **[Full Documentation](https://github.datasierra.com/streamforge)**

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

For a detailed decision guide, see [docs/WHEN_TO_USE.md](docs/WHEN_TO_USE.md).

## 5-Minute Demo

1. Start a local Redpanda broker with [`examples/redpanda/docker-compose.yml`](examples/redpanda/docker-compose.yml).
2. Validate the demo config:
   ```bash
   cargo run --quiet --bin streamforge-validate -- examples/redpanda/selective-replication.yaml
   ```
3. Run StreamForge with [`examples/redpanda/selective-replication.yaml`](examples/redpanda/selective-replication.yaml).
4. Produce sample records to `raw-orders` and watch `analytics-orders` and `pii-safe-orders`.

## Production Trust Signals

- At-least-once delivery with retry and DLQ support
- Native Prometheus metrics and lag monitoring
- Kubernetes operator, Helm chart, and web UI
- Explicit comparison guidance against MirrorMaker 2 and Arroyo
- Kafka-first with Redpanda-tested examples

---

## What It Does

StreamForge reads from one or more source Kafka topics, applies user-defined rules via a declarative DSL, and writes results to one or more destination topics — optionally on a completely different cluster.

**Core capabilities:**

- **Cross-cluster mirroring** — replicate messages between independent Kafka clusters
- **Content-based filtering** — evaluate JSON payloads, keys, headers, and timestamps
- **Message transformation** — reshape payloads, extract fields, build new objects, hash PII
- **Multi-destination routing** — fan-out to different topics based on message content
- **Envelope operations** — transform keys, headers, and timestamps, not just payloads
- **At-least-once delivery** — manual commit with configurable retry and dead-letter queue
- **Caching** — local (Moka), Redis, Kafka-backed, or multi-level L1/L2 lookups
- **Observability** — Prometheus `/metrics`, Grafana alert rules, structured logging
- **Cloud-native** — Kubernetes operator (CRD), Helm chart, Web UI, multi-arch images

---

## Quick Start

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# macOS
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

```yaml
# config.yaml
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

YAML, JSON, and `.yml` are all auto-detected by extension.

For a step-by-step walkthrough, see the **[Quickstart Guide](https://github.datasierra.com/streamforge/QUICKSTART)**.

---

## Use Cases

| Scenario | How |
|----------|-----|
| Cross-cluster replication | Mirror production data to analytics or DR clusters |
| Event fan-out | Route a single topic to per-type topics (`meetings`, `calls`, `quality`) |
| Data redaction | Hash or drop PII fields before forwarding to less-trusted environments |
| Header-based tenancy | Filter by tenant header without parsing the payload |
| Schema slimming | Extract only the fields downstream consumers need |
| Time-window routing | Route recent messages to real-time pipelines, older to batch |
| Key repartitioning | Rewrite message keys for different partitioning strategies |
| Dead letter routing | Separate invalid messages for error handling |

Full examples with YAML configs: **[docs/USAGE.md](docs/USAGE.md)**

---

## Configuration Examples

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

    - output: quality-alerts
      filter: "AND:/eventType,==,quality.report:/data/score,<,80"

    - output: all-events   # catch-all
```

### PII Redaction

```yaml
destinations:
  - output: safe-events
    transform: "HASH:SHA256,/user/email,emailHash"
```

### Envelope Operations (key, headers, timestamp)

```yaml
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
```

### At-Least-Once Delivery with DLQ

```yaml
commit_strategy:
  manual_commit: true
  commit_mode: async

dead_letter_queue:
  enabled: true
  topic: streamforge-dlq
  max_retries: 3
```

---

## DSL Reference

### Filters

| Syntax | Description |
|--------|-------------|
| `/path,op,value` | Compare JSON field (`>`, `>=`, `<`, `<=`, `==`, `!=`) |
| `AND:cond1:cond2` | All conditions must pass |
| `OR:cond1:cond2` | Any condition must pass |
| `NOT:cond` | Invert a condition |
| `REGEX:/path,pattern` | Match field against regular expression |
| `ARRAY_ALL:/path,filter` | All array elements must match |
| `ARRAY_ANY:/path,filter` | At least one element must match |
| `KEY_PREFIX:prefix` | Message key starts with prefix |
| `KEY_MATCHES:regex` | Message key matches regex |
| `HEADER:name,op,value` | Compare header value |
| `TIMESTAMP_AGE:op,secs` | Message age in seconds |

### Transforms

| Syntax | Description |
|--------|-------------|
| `/path` | Extract field or nested object |
| `CONSTRUCT:f1=/p1:f2=/p2` | Build new object from multiple paths |
| `ARRAY_MAP:/path,/element` | Map over array elements |
| `ARITHMETIC:op,left,right` | Arithmetic (`ADD`, `SUB`, `MUL`, `DIV`) |
| `HASH:algo,/path` | Hash field (`MD5`, `SHA256`, `SHA512`, `MURMUR64`, `MURMUR128`) |
| `HASH:algo,/path,out` | Hash field, store in `out`, keep original |
| `CACHE_LOOKUP:/key,store,/dest` | Look up value from cache backend |

Full DSL reference: **[docs/ADVANCED_DSL_GUIDE.md](docs/ADVANCED_DSL_GUIDE.md)**

---

## Performance

Benchmarks run with `cargo bench` on Apple M-series, 4 cores, 1KB messages, 10 partitions.

### DSL Operations

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Simple filter | 44–50 ns | 20–23M ops/s |
| Boolean AND (2 conds) | 97 ns | 10M ops/s |
| Regex filter | 47–59 ns | 17–21M ops/s |
| Array ALL/ANY | 57–101 ns | 10–18M ops/s |
| Object construction | 908–1,414 ns | 0.7–1.1M ops/s |
| Arithmetic | 816–864 ns | 1.2M ops/s |
| Hash (Murmur64) | ~125 ns | ~8M ops/s |

### End-to-End Throughput

| Threads | Delivery | Sustained | Peak |
|---------|----------|-----------|------|
| 4 | at-least-once | ~11,000 msg/s | — |
| 8 | at-least-once | 25,000–30,000 msg/s | 34,500 msg/s |

Full results: **[benchmarks/results/](benchmarks/results/)**

---

## Observability

```yaml
observability:
  metrics_enabled: true
  metrics_port: 9090
  lag_monitoring_enabled: true
```

Exposes a Prometheus-compatible `/metrics` endpoint and `/health`.

Useful queries:

```promql
rate(streamforge_messages_consumed_total[5m])
sum(rate(streamforge_messages_produced_total[5m])) by (destination)
streamforge_consumer_lag{topic="...", partition="..."}
histogram_quantile(0.99, rate(streamforge_processing_duration_seconds_bucket[5m]))
```

Prometheus + Grafana setup: **[docs/OBSERVABILITY_QUICKSTART.md](docs/OBSERVABILITY_QUICKSTART.md)**

---

## Security

Full SSL/TLS and SASL support (PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, GSSAPI/Kerberos, OAUTHBEARER):

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

Details: **[docs/SECURITY_CONFIGURATION.md](docs/SECURITY_CONFIGURATION.md)**

---

## Deployment

### Docker

```bash
docker run -d \
  -v $(pwd)/config.yaml:/app/config/config.yaml:ro \
  -e CONFIG_FILE=/app/config/config.yaml \
  ghcr.io/rahulbsw/streamforge:latest
```

### Kubernetes Operator

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamForgePipeline
metadata:
  name: my-pipeline
spec:
  replicas: 3
  config:
    bootstrap: kafka:9092
    input: raw-events
    output: processed-events
```

### Helm

```bash
helm install streamforge ./helm/streamforge-operator \
  --set image.tag=latest \
  --set replicas=3
```

Docker + Kubernetes guide: **[docs/DOCKER.md](docs/DOCKER.md)**

---

## Documentation

Full documentation is published at **[github.datasierra.com/streamforge](https://github.datasierra.com/streamforge)**.

| Guide | Description |
|-------|-------------|
| [QUICKSTART.md](docs/QUICKSTART.md) | Get running in 5 minutes |
| [USAGE.md](docs/USAGE.md) | 8 real-world use cases |
| [YAML_CONFIGURATION.md](docs/YAML_CONFIGURATION.md) | Full config reference |
| [ADVANCED_DSL_GUIDE.md](docs/ADVANCED_DSL_GUIDE.md) | Complete DSL reference |
| [ADVANCED_FILTERS.md](docs/ADVANCED_FILTERS.md) | Boolean logic and complex filters |
| [SECURITY_CONFIGURATION.md](docs/SECURITY_CONFIGURATION.md) | SSL/TLS, SASL, Kerberos |
| [OBSERVABILITY_QUICKSTART.md](docs/OBSERVABILITY_QUICKSTART.md) | Prometheus + Grafana setup |
| [PERFORMANCE.md](docs/PERFORMANCE.md) | Performance tuning guide |
| [SCALING.md](docs/SCALING.md) | Horizontal and vertical scaling |
| [DOCKER.md](docs/DOCKER.md) | Docker and Kubernetes deployment |
| [CONTRIBUTING.md](docs/CONTRIBUTING.md) | Development setup and guidelines |
| [CHANGELOG.md](docs/CHANGELOG.md) | Version history |
| [DOCUMENTATION_INDEX.md](docs/DOCUMENTATION_INDEX.md) | Full index |

---

## Contributing

```bash
git clone https://github.com/rahulbsw/streamforge.git
cd streamforge
cargo build
cargo test        # 92 unit tests
cargo bench       # 30+ benchmarks
cargo clippy
```

See **[docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)** for guidelines.

---

## Roadmap

**v0.5.0**
- [ ] Avro serialization + Confluent Schema Registry
- [ ] String operations (`UPPER`, `LOWER`, `TRIM`, `SUBSTRING`)
- [ ] Date/time transforms
- [ ] Conditional transforms (`IF:condition,then,else`)

**v1.0.0**
- [ ] Exactly-once semantics
- [ ] UDF support via WASM or Lua
- [ ] State management with RocksDB

---

## License

Apache License 2.0 — see [LICENSE](LICENSE) for details.

Copyright 2025 Rahul Jain
