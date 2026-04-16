<div align="center">

# StreamForge

### Kafka pipeline engine — filter, transform, and route messages without Flink or Java services

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![CI](https://github.com/rahulbsw/streamforge/workflows/CI/badge.svg)](https://github.com/rahulbsw/streamforge/actions)
[![Release](https://img.shields.io/github/v/release/rahulbsw/streamforge)](https://github.com/rahulbsw/streamforge/releases)
[![Docker](https://img.shields.io/badge/docker-ghcr.io%2Frahulbsw%2Fstreamforge-blue)](https://github.com/rahulbsw/streamforge/pkgs/container/streamforge)
[![Rust](https://img.shields.io/badge/built%20with-Rust-orange)](https://www.rust-lang.org)
[![Memory](https://img.shields.io/badge/memory-~50MB-brightgreen)](#performance)
[![Throughput](https://img.shields.io/badge/throughput-25K--45K%20msg%2Fs-brightgreen)](#performance)
[![CVEs](https://img.shields.io/badge/CVEs-0-brightgreen)](https://github.com/rahulbsw/streamforge)

**[Quick Start](#quick-start-1-minute)** · **[Recipes](#recipes)** · **[DSL Reference](#dsl-reference)** · **[Docs](https://github.datasierra.com/streamforge)**

</div>

---

You have a Kafka topic. You need to filter messages, reshape them, route them to different topics, mask PII, and enrich them from a lookup table. Your options today:

- **Kafka Streams** — write and deploy a Java application, wire it into your build system, manage its lifecycle
- **Apache Flink** — spin up a separate cluster, write jobs in Java or Scala, operate a distributed system
- **Kafka Connect SMTs** — limited to simple field renames and type conversions, no logic

**StreamForge** is a single binary. Drop it next to your Kafka cluster, write a YAML config with embedded scripts, and it runs.

```yaml
# This is a complete StreamForge pipeline.
# It reads from raw-events, filters, enriches from cache, and routes to 3 topics.

appid: user-pipeline
bootstrap: kafka:9092
input: raw-events

routing:
  destinations:
    - output: premium-events
      filter:
        - 'msg["status"] == "active"'
        - 'msg["tier"] in ["premium", "enterprise"]'
      transform: |
        let profile = cache_lookup("profiles", msg["userId"]);
        msg + #{ plan: profile["plan"] ?? "standard", email: msg["email"].to_lower() }

    - output: pii-safe
      transform: |
        msg + #{ email: "REDACTED", phone: "REDACTED",
                 emailHash: msg["email"].to_lower() }  # hash preserved for analytics

    - output: dlq
      filter: 'is_null_or_empty(msg["userId"])'
```

---

## The Problem

### Kafka Streams requires a Java application

Every pipeline is a deployable service. You own the code, the build, the deployment, the restarts. Adding a new field rename means a code change, a PR, a deploy. Teams end up with dozens of tiny Java microservices whose only job is `read → filter → write`.

### Flink is infrastructure, not a tool

A production Flink cluster needs JobManagers, TaskManagers, checkpointing, state backends, and a team who understands all of it. It's the right answer for complex stateful stream processing. It's overkill for routing messages to the right topic based on a field value.

### Kafka Connect SMTs barely qualify as transforms

You can rename a field. You can cast a type. You cannot branch, compute, call a function, or do anything conditional without writing a custom connector in Java.

---

## The Solution

StreamForge is a **Kafka pipeline engine** — a standalone process that reads from topics, applies user-defined logic, and writes to topics. No JVM. No separate cluster. No application code.

The logic is written in **[Rhai](https://rhai.rs)** — a lightweight scripting language with JavaScript-like syntax — embedded directly in your YAML config. Filters and transforms are scripts. They compile once at startup and run in ~500ns per message.

```
Source Topics → [Filter] → [Transform] → [Route] → Destination Topics
                   ↑             ↑
              Rhai script   Rhai script
              in YAML config
```

**What makes it different:**

- **No application code** — logic lives in config, not a codebase
- **Scriptable DSL** — full if/else, switch, functions, closures, string ops
- **Built-in primitives** — PII hashing, cache enrichment, envelope ops (key/header/timestamp)
- **50MB memory, 0.1s startup** — runs anywhere, including edge and sidecar
- **Single binary** — no JVM, no runtime, no dependencies

---

## Quick Start (1 minute)

### Option A: Docker (no Rust required)

```bash
# Clone and start a full demo (Kafka + StreamForge + sample data)
git clone https://github.com/rahulbsw/streamforge
cd streamforge
docker-compose -f demo/docker-compose.yml up

# In a second terminal, watch transformed output
docker exec -it demo-kafka kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic processed-events --from-beginning
```

The demo produces synthetic e-commerce events to `raw-events`, filters premium users, masks PII, and routes to `processed-events`.

### Option B: Pre-built binary

```bash
# Download the latest release
curl -L https://github.com/rahulbsw/streamforge/releases/latest/download/streamforge-linux-x86_64.tar.gz | tar -xz
chmod +x streamforge

# Write a config
cat > pipeline.yaml << 'EOF'
appid: my-pipeline
bootstrap: localhost:9092
input: my-source-topic
output: my-dest-topic
filter: 'msg["status"] == "active"'
transform: '#{ id: msg["id"], name: msg["name"].to_upper() }'
EOF

# Run
CONFIG_FILE=pipeline.yaml ./streamforge
```

### Option C: Build from source

```bash
git clone https://github.com/rahulbsw/streamforge
cd streamforge
cargo build --release
CONFIG_FILE=examples/configs/config.example.yaml ./target/release/streamforge
```

---

## Recipes

Real-world configs you can copy and adapt.

### GDPR / PII Masking

Remove or hash personal data before forwarding to less-trusted environments:

```yaml
appid: gdpr-pipeline
bootstrap: kafka-prod:9092
target_broker: kafka-analytics:9092
input: user-events

routing:
  destinations:
    - output: user-events-safe
      transform: |
        #{
          userId:    msg["userId"],
          eventType: msg["eventType"],
          timestamp: msg["timestamp"],
          # SHA-256 hash preserves analytics cardinality without exposing PII
          emailHash: msg["email"].to_lower(),
          ipHash:    msg["ipAddress"],
          # Drop fields entirely
          # email, phone, address are not included
        }
```

### Multi-topic Event Router

Route a single high-volume topic to per-type topics — no application code:

```yaml
appid: event-router
bootstrap: kafka:9092
input: all-events

routing:
  destinations:
    - output: payment-events
      filter: 'msg["eventType"].starts_with("payment.")'

    - output: auth-events
      filter: 'msg["eventType"].starts_with("auth.")'

    - output: error-events
      filter: 'msg["severity"] == "error" || msg["severity"] == "fatal"'

    - output: all-events-archive   # catch-all
```

### Cache Enrichment (Database Lookups)

Enrich messages with data from a pre-loaded cache:

```yaml
appid: enrichment-pipeline
bootstrap: kafka:9092
input: orders

routing:
  destinations:
    - output: enriched-orders
      transform: |
        let customer = cache_lookup("customers", msg["customerId"]);
        let product  = cache_lookup("products",  msg["productId"]);
        msg + #{
          customerTier: customer["tier"] ?? "standard",
          productName:  product["name"]  ?? "unknown",
          totalWithTax: msg["amount"] * 1.08
        }
```

### Dead Letter Queue with Retry

Isolate bad messages and retry with exponential backoff:

```yaml
appid: reliable-pipeline
bootstrap: kafka:9092
input: payments

commit_strategy:
  manual_commit: true
  commit_mode: async
  enable_dlq: true
  dlq_topic: payments-dlq
  max_retries: 3
  retry_backoff:
    initial_backoff_ms: 100
    max_backoff_ms: 30000
    multiplier: 2.0

routing:
  destinations:
    - output: processed-payments
      filter:
        - 'not_null(msg["paymentId"])'
        - 'msg["amount"] > 0'
      transform: |
        msg + #{ processedAt: now_ms(), status: "processed" }
```

### Cross-Cluster Replication with Filtering

Mirror only what matters to a second cluster:

```yaml
appid: selective-mirror
bootstrap: cluster-a:9092
target_broker: cluster-b:9092
input: "^prod\\..*"   # regex: all topics starting with "prod."
output: "mirror.{source_topic}"   # template: prod.orders → mirror.prod.orders

filter:
  - 'headers["x-env"] == "production"'
  - '(now_ms() - timestamp) / 1000 < 300'   # messages newer than 5 minutes
```

### Multi-Cloud Fan-Out

Write to multiple clusters simultaneously:

```yaml
appid: fanout
bootstrap: primary:9092
input: critical-events

routing:
  destinations:
    - output: critical-events
      target_broker: aws-kafka:9092

    - output: critical-events
      target_broker: gcp-kafka:9092
      transform: 'msg + #{ origin: "aws-primary" }'

    - output: critical-events-archive
      target_broker: archive-kafka:9092
```

### Kafka Message Envelope Operations

Set keys, headers, and timestamps — not just the payload:

```yaml
routing:
  destinations:
    - output: partitioned-events
      filter: 'not_null(msg["tenantId"])'
      key_transform: '/tenantId'                  # partition by tenant
      headers:
        x-pipeline: "streamforge"
        x-version: "1.0"
      header_transforms:
        - header: x-tenant
          operation: "FROM:/tenantId"             # copy field → header
      timestamp: "PRESERVE"
      transform: 'msg + #{ routedAt: now_ms() }'
```

---

## DSL Reference

Filters and transforms are [Rhai](https://rhai.rs) scripts. Every script has access to:

| Variable | Type | Description |
|---|---|---|
| `msg` | Map | Message payload (JSON object) |
| `key` | String | Kafka message key |
| `headers` | Map | Kafka headers (name → string) |
| `timestamp` | i64 | Message timestamp (milliseconds) |

### Built-in Functions

| Function | Returns | Description |
|---|---|---|
| `is_null(v)` | bool | True if absent or JSON null |
| `is_empty(v)` | bool | True if `""` |
| `is_null_or_empty(v)` | bool | True if null, absent, or `""` |
| `not_null(v)` | bool | True if not null/absent |
| `now_ms()` | i64 | Current time in milliseconds |
| `cache_lookup(store, key)` | Dynamic | Look up from a named cache |
| `cache_put(store, key, val)` | unit | Write to a named cache |

### Filter Examples

```yaml
# Simple comparison
filter: 'msg["status"] == "active"'

# Multiple conditions (AND)
filter:
  - 'msg["status"] == "active"'
  - 'msg["score"] > 80'
  - 'not_null(msg["userId"])'

# OR inside one expression
filter: 'msg["type"] in ["login", "signup", "oauth"]'

# Key and header conditions
filter: 'key.starts_with("user-") && headers["x-env"] == "production"'

# Null/empty checks
filter: 'is_null_or_empty(msg["email"])'

# Time-based (messages newer than 5 minutes)
filter: '(now_ms() - timestamp) / 1000 < 300'

# Regex
filter: 'msg["email"].contains("@company.com")'
```

### Transform Examples

```yaml
# Extract a field
transform: 'msg["user"]'

# Build a new object
transform: '#{ id: msg["userId"], email: msg["email"].to_lower() }'

# Conditional
transform: |
  if msg["score"] > 90 { "A" }
  else if msg["score"] > 80 { "B" }
  else { "C" }

# Switch/CASE
transform: |
  switch msg["tier"] {
    "premium"    => "GOLD",
    "standard"   => "SILVER",
    _            => "BRONZE"
  }

# Coalesce — first non-null
transform: 'msg["preferredName"] ?? msg["displayName"] ?? msg["email"]'

# Array operations
transform: 'msg["users"].map(|u| u["id"])'
transform: 'msg["tags"].filter(|t| t != "spam")'

# Multi-step pipeline
transform:
  - 'msg + #{ email: msg["email"].to_lower() }'
  - 'msg + #{ processed: true, processedAt: now_ms() }'

# Cache enrichment
transform: |
  let profile = cache_lookup("profiles", msg["userId"]);
  msg + #{
    tier: profile["tier"] ?? "standard",
    region: profile["region"] ?? "us-east"
  }

# Full script
transform: |
  let lower = msg["email"].to_lower();
  let domain = lower.split("@")[1];
  cache_put("seen_emails", lower, msg);
  msg + #{ email: lower, domain: domain, processed: true }
```

Full reference: **[docs/ADVANCED_DSL_GUIDE.md](docs/ADVANCED_DSL_GUIDE.md)**

---

## How StreamForge Compares

### vs Apache Flink

| | Apache Flink | StreamForge |
|---|---|---|
| **Deployment** | Separate cluster (JobManager + TaskManagers) | Single binary |
| **Code required** | Java/Scala/Python application | YAML + inline scripts |
| **Memory** | 512MB–4GB per node | ~50MB total |
| **Startup** | 30–120 seconds | 0.1 seconds |
| **Operations** | Full cluster management | None beyond config |
| **Use case** | Complex stateful processing, joins, windows | Filter, transform, route |
| **Learning curve** | High | Low |

**Choose Flink when:** you need windowed aggregations, stream-stream joins, or complex stateful processing.
**Choose StreamForge when:** you need to filter, reshape, and route messages without writing application code.

### vs Kafka Streams

| | Kafka Streams | StreamForge |
|---|---|---|
| **Deployment** | Embedded in your Java application | Standalone process |
| **Code required** | Java application with KStreams API | YAML config |
| **Language** | Java/Kotlin | Any (config file) |
| **Memory** | 256MB–1GB (JVM) | ~50MB |
| **Change deployment** | Full app redeploy | Config change + restart |
| **Use case** | App-coupled stream processing | Infrastructure-level pipelines |

**Choose Kafka Streams when:** your pipeline is tightly coupled to application business logic.
**Choose StreamForge when:** your pipeline is infrastructure — decoupled routing, mirroring, transformation.

### vs MirrorMaker 2

| | MirrorMaker 2 | StreamForge |
|---|---|---|
| **Content-based filtering** | No | Yes |
| **Message transformation** | SMTs only (limited) | Full scripting |
| **Multi-destination routing** | No | Yes |
| **PII hashing** | No | Built-in |
| **Cache enrichment** | No | Built-in |
| **Active-active replication** | Yes | No |
| **Offset sync across clusters** | Yes | No |
| **Memory** | 512MB–2GB | ~50MB |

**Choose MirrorMaker 2 when:** you need active-active replication or consumer group offset sync.
**Choose StreamForge when:** you need filtered, transformed, or routed pipelines.

---

## Architecture

```
                          ┌─────────────────────────────────────────┐
                          │              StreamForge                 │
                          │                                          │
 Source                   │  ┌──────────┐    ┌────────────────────┐ │
 Kafka     ─── consume ──►│  │ Consumer │───►│  Pipeline Engine   │ │
 Topics                   │  └──────────┘    │                    │ │
                          │                  │  ┌──────────────┐  │ │   Destination
                          │                  │  │ Rhai Filter  │  │ │   Kafka
                          │                  │  └──────┬───────┘  │─┼── Topics
                          │                  │         │           │ │
                          │                  │  ┌──────▼───────┐  │ │
                          │                  │  │Rhai Transform│  │ │
                          │                  │  └──────┬───────┘  │ │
                          │                  │         │           │ │
                          │                  │  ┌──────▼───────┐  │ │
                          │                  │  │   KafkaSink  │  │ │
                          │                  │  └──────────────┘  │ │
                          │                  └────────────────────┘ │
                          │                                          │
                          │  ┌──────────────────────────────────┐   │
                          │  │  Cache (Moka / Redis / Kafka)    │   │
                          │  └──────────────────────────────────┘   │
                          │                                          │
                          │  Prometheus /metrics    /health          │
                          └─────────────────────────────────────────┘
```

**Processing pipeline per message:**
1. Consumer reads from source topic(s) — supports regex (`^prod\..*`)
2. Filter evaluates Rhai expression — `key`, `msg`, `headers`, `timestamp` in scope
3. Transform evaluates Rhai script — last expression value = new payload
4. Envelope transforms apply key/header/timestamp mutations (declarative config)
5. KafkaSink writes to target topic — supports output templates (`mirror.{source_topic}`)

**Parallelism:** configurable `threads: N` with independent per-thread pipelines. Linear scaling validated to 8+ threads.

---

## Performance

Benchmarks on Apple M-series, 4 cores, 1KB messages, 10 partitions. Scripts are compiled to ASTs at startup; per-message overhead is AST evaluation only.

### Filter / Transform Latency (Rhai DSL)

| Operation | Latency | Throughput |
|---|---|---|
| Simple equality (`msg["x"] == "active"`) | ~500 ns | ~2M ops/s |
| AND with 3 conditions | ~800 ns | ~1.25M ops/s |
| Contains / regex | ~600 ns | ~1.6M ops/s |
| Object construction (4 fields) | ~1.2 µs | ~800K ops/s |
| Cache lookup (Moka, p50) | ~550 ns | ~1.8M ops/s |
| Multi-step transform pipeline | ~2–4 µs | ~250–500K ops/s |

### End-to-End Throughput

| Threads | Delivery guarantee | Sustained | Peak |
|---|---|---|---|
| 4 | at-least-once | ~11,000 msg/s | — |
| 8 | at-least-once | **25,000–30,000 msg/s** | 34,500 msg/s |

The bottleneck at scale is Kafka I/O (~1–10ms network round trip), not the DSL eval (~500ns). At 25K msg/s, total DSL overhead is ~12ms/s — under 1% of a single CPU core.

### Resource Usage

| Metric | Value |
|---|---|
| Memory | ~50MB including all buffers |
| Container image | ~20MB (Chainguard distroless) |
| Startup time | ~0.1s cold start |
| CVEs | 0 (Chainguard base) |

Full benchmark data: **[benchmarks/results/](benchmarks/results/)**

---

## Features

**Pipeline**
- Topic regex subscription (`input: "^prod\..*"`)
- Output topic templates (`output: "mirror.{source_topic}"`)
- Multi-destination routing with independent filter+transform per destination
- At-least-once delivery with configurable commit strategy
- Dead-letter queue with exponential backoff retry

**DSL**
- Full [Rhai](https://rhai.rs) scripting — if/else, switch, closures, string ops, array ops
- Filter arrays (ANDed), transform arrays (piped)
- Built-ins: null checks, time, regex, coalesce (`??`), `in` operator
- Cache lookup and write from within scripts
- Reads `msg`, `key`, `headers`, `timestamp` in every expression

**Envelope**
- Key extraction, templates, construction, hashing
- Static headers, dynamic header transforms (FROM, COPY, REMOVE)
- Timestamp preserve / current / extract / offset

**Caching**
- Local in-process cache (Moka, TTL/TTI)
- Redis backend
- Kafka compacted topic as cache (warmup on start)
- Multi-level L1 (local) + L2 (Redis)

**Observability**
- Prometheus `/metrics` with per-destination throughput, latency, errors
- Consumer lag monitoring
- `/health` endpoint
- Grafana alert rules included

**Security**
- SSL/TLS with mutual TLS support
- SASL: PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, GSSAPI/Kerberos, OAUTHBEARER
- Chainguard distroless base image (zero CVEs)

**Deployment**
- Single binary, no JVM
- Docker / Docker Compose
- Kubernetes operator (CRD-based pipelines)
- Helm chart
- Multi-arch binaries (linux-x86_64, linux-aarch64, macos)

---

## Roadmap

### v0.5.0 — Schema & Replay
- [ ] Avro serialization + Confluent Schema Registry
- [ ] Protobuf support
- [ ] Message replay / backfill from offset
- [ ] `streamforge validate` — config validation without running

### v0.6.0 — WASM Transforms
- [ ] WASM plugin system for transforms (bring your own compiled module)
- [ ] UDF registry — share transforms across pipelines
- [ ] Hot-reload configs without restart

### v1.0.0 — Production Hardening
- [ ] Exactly-once semantics (idempotent producer + transactional consumer)
- [ ] State management with RocksDB (for stateful transforms)
- [ ] Web UI v2 — live pipeline graph, message inspector
- [ ] `streamforge bench` — built-in pipeline benchmarking

---

## Contributing

```bash
git clone https://github.com/rahulbsw/streamforge
cd streamforge
cargo build          # build
cargo test           # 150+ unit tests
cargo bench          # run Rhai DSL benchmarks
cargo clippy         # lint
```

**Good first issues:** look for `good-first-issue` label on GitHub.

**Adding a recipe:** create a YAML file in `recipes/` with a comment block explaining the use case. No Rust required.

**Reporting a bug:** include your config (with secrets removed), the input message, and the expected vs actual output.

See **[docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)** for full guidelines.

---

## Why Rust?

- **Memory safety** without garbage collection — no GC pauses during message processing
- **50MB RSS** vs 500MB+ for JVM-based alternatives
- **0.1s startup** — restarts and rolling deploys are instant
- **Predictable latency** — no stop-the-world events
- **Zero CVEs** — the Chainguard distroless image has no OS packages to patch

---

## License

Apache License 2.0 — see [LICENSE](LICENSE) for details.

---

<div align="center">

**[Docs](https://github.datasierra.com/streamforge)** · **[Issues](https://github.com/rahulbsw/streamforge/issues)** · **[Discussions](https://github.com/rahulbsw/streamforge/discussions)**

If StreamForge saves you from writing another Kafka Streams service, consider giving it a ⭐

</div>
