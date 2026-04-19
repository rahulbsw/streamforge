---
title: Project Summary
nav_order: 15
---

# StreamForge — Project Summary

## Overview

StreamForge is a high-performance Kafka streaming toolkit written in Rust. It provides cross-cluster message mirroring with a powerful filtering and transformation DSL, multiple cache backends, at-least-once delivery semantics, Prometheus observability, and a Kubernetes operator for cloud-native deployments.

## Current Status

| | |
|---|---|
| **Version** | 1.0.0 |
| **Status** | Stable v1.0 Production Release ✅ |
| **Last Updated** | 2026-04-18 |
| **License** | Apache 2.0 |
| **Repository** | [github.com/rahulbsw/streamforge](https://github.com/rahulbsw/streamforge) |

---

## Feature Matrix

| Feature | Status | Since |
|---------|--------|-------|
| Cross-cluster Kafka mirroring | ✅ Production | v0.1.0 |
| Multi-destination content-based routing | ✅ Production | v0.1.0 |
| JSON path filters (`>`, `<`, `==`, `!=`, etc.) | ✅ Production | v0.1.0 |
| Boolean logic (`AND`, `OR`, `NOT`) | ✅ Production | v0.1.0 |
| Object construction (`CONSTRUCT`) | ✅ Production | v0.1.0 |
| Native compression (Gzip, Snappy, Zstd) | ✅ Production | v0.1.0 |
| SSL/TLS, SASL, Kerberos security | ✅ Production | v0.1.0 |
| Docker + Chainguard distroless images | ✅ Production | v0.1.0 |
| YAML configuration (recommended) | ✅ Production | v0.2.0 |
| JSON configuration (backward compatible) | ✅ Production | v0.1.0 |
| Regular expressions (`REGEX`) | ✅ Production | v0.2.0 |
| Array operations (`ARRAY_ALL`, `ARRAY_ANY`, `ARRAY_MAP`) | ✅ Production | v0.2.0 |
| Arithmetic transforms (`ADD`, `SUB`, `MUL`, `DIV`) | ✅ Production | v0.2.0 |
| Concurrent multi-threaded processing | ✅ Production | v0.3.0 |
| Hash transforms (`MD5`, `SHA256`, `SHA512`, `MURMUR64/128`) | ✅ Production | v0.3.0 |
| Local cache (Moka, TTL/TTI) | ✅ Production | v0.3.0 |
| Redis cache backend | ✅ Production | v0.3.0 |
| Kafka-backed cache (compacted topic) | ✅ Production | v0.3.0 |
| Multi-level cache (L1 local + L2 Redis) | ✅ Production | v0.3.0 |
| At-least-once delivery (manual commit) | ✅ Production | v0.3.0 |
| Dead letter queue (DLQ) with retry backoff | ✅ Production | v0.3.0 |
| Kubernetes Operator (CRD-based pipelines) | ✅ Production | v0.3.0 |
| Helm chart for operator deployment | ✅ Production | v0.3.0 |
| Web UI for operator pipeline management | ✅ Production | v0.3.0 |
| Prometheus metrics (`/metrics` endpoint) | ✅ Production | v0.4.0 |
| Grafana alert rules | ✅ Production | v0.4.0 |
| Envelope transforms (wrap/unwrap/add_metadata) | ✅ Production | v0.4.0 |
| Multi-arch release binaries (x86_64 + aarch64) | ✅ Production | v0.4.0 |
| GHCR Docker image publishing | ✅ Production | v0.4.0 |
| Typed error system (14+ error types) | ✅ Production | v1.0.0 |
| Dead letter queue (DLQ) with error metadata | ✅ Production | v1.0.0 |
| Exponential backoff retry policy | ✅ Production | v1.0.0 |
| AST-based DSL parser with validation | ✅ Production | v1.0.0 |
| Function-style DSL (`and()`, `or()`, `field()`) | ✅ Production | v1.0.0 |
| Dollar shorthand ($status, $user.email) | ✅ Production | v1.0.0 |
| 35 transform evaluators (14 string + 21 date/time) | ✅ Production | v1.0.0 |
| Avro serialization | ⚠️ Planned v1.1.0 | — |
| Schema registry integration | ⚠️ Planned v1.1.0 | — |
| Exactly-once semantics | ⚠️ Planned v1.1.0 | — |

---

## Performance Benchmarks

### DSL Operation Performance (measured with `cargo bench`)

| Operation | Mean Time | Throughput |
|-----------|-----------|------------|
| Simple filter | 44–50ns | 20–23M ops/s |
| Boolean AND (2 conditions) | 97ns | 10.3M ops/s |
| Boolean AND (3 conditions) | 145ns | 6.9M ops/s |
| Regular expression | 47–59ns | 17–21M ops/s |
| Array ALL | 101ns | 9.9M ops/s |
| Array ANY | 57ns | 17.5M ops/s |
| Field extraction | 810–816ns | 1.23M ops/s |
| Object construction (4 fields) | 1,071ns | 933K ops/s |
| Arithmetic | 816–864ns | 1.16–1.23M ops/s |
| Hash (Murmur64/128) | ~125ns | ~8M ops/s |
| Hash (SHA256) | ~500ns | ~2M ops/s |
| Local cache lookup | 50ns (p50) | 20M ops/s |

### End-to-End System Performance

| Metric | Value | Context |
|--------|-------|---------|
| **Throughput** | 25,000–45,000 msg/s | Scales linearly with threads |
| **Latency p50** | 2.8ms | End-to-end |
| **Latency p99** | 12.4ms | End-to-end |
| **Memory** | 48–72MB | Including all buffers |
| **Startup time** | 0.1s | Cold start |
| **Container size** | ~20MB | Chainguard distroless |

*Measured on Apple M-series, 4 CPU cores, 1KB messages, 10 partitions*

📊 See [BENCHMARK_RESULTS.md](../benchmarks/results/BENCHMARKS.md) for full statistical analysis.

---

## Architecture

```
┌──────────────────────────────────────────────┐
│           Source Kafka Cluster               │
│         Topic (N partitions)                 │
└────────────────┬─────────────────────────────┘
                 │
         Consumer Group
         (Auto partition assignment)
                 │
     ┌───────────┼───────────┐
     │           │           │
     ▼           ▼           ▼
┌─────────┐ ┌─────────┐ ┌─────────┐
│Thread 1 │ │Thread 2 │ │Thread N │
│ Filter  │ │ Filter  │ │ Filter  │
│Transform│ │Transform│ │Transform│
│ Cache   │ │ Cache   │ │ Cache   │
│  Route  │ │  Route  │ │  Route  │
└────┬────┘ └────┬────┘ └────┬────┘
     │           │           │
     └───────────┼───────────┘
                 │
    ┌────────────┼────────────┐
    ▼            ▼            ▼
┌────────┐  ┌────────┐  ┌────────┐
│Topic A │  │Topic B │  │  DLQ   │
└────────┘  └────────┘  └────────┘
Target Kafka Cluster     (failures)

        Prometheus /metrics
        ↑ scrape every 15s
   ┌────────────┐
   │  Grafana   │
   └────────────┘
```

### Scaling

- **Horizontal**: Multiple instances share partitions via consumer group
- **Vertical**: `threads: N` for intra-instance parallelism (linear scaling to 8+ threads)
- **Auto-scaling**: Kubernetes HPA on CPU or consumer lag metric

---

## Technology Stack

| Category | Library | Purpose |
|----------|---------|---------|
| **Runtime** | Tokio 1.x | Async executor |
| **Kafka** | rdkafka 0.36 | Kafka client (librdkafka) |
| **Serialization** | serde_json, serde_yaml | Config + message parsing |
| **Regex** | regex 1.x | DSL pattern matching |
| **Hashing** | md-5, sha2, murmur3, hex | Hash transform algorithms |
| **Caching** | moka 0.12, dashmap 6.0 | Local concurrent cache |
| **Redis** | redis 0.24 (optional) | Distributed cache backend |
| **Observability** | prometheus 0.14 | Metrics exposition |
| **HTTP** | axum 0.7 | `/metrics` endpoint |
| **Retry** | tokio-retry 0.3 | Exponential backoff |
| **Error handling** | anyhow, thiserror | Error types |
| **Benchmarking** | criterion 0.5 | Performance tests |

---

## DSL Quick Reference

### Filtering

```yaml
# Comparison
filter: "/order/total,>,1000"

# Boolean logic
filter: "AND:/user/active,==,true:/user/tier,==,premium"

# Regular expression
filter: "REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"

# Array operations
filter: "ARRAY_ALL:/sessions,/active,==,true"
filter: "ARRAY_ANY:/tasks,/priority,==,high"
```

### Transformation

```yaml
# Field extraction
transform: "/message/confId"

# Object construction
transform: "CONSTRUCT:id=/user/id:email=/user/email"

# Arithmetic
transform: "ARITHMETIC:MUL,/price,1.08"

# Hash (anonymize PII)
transform: "HASH:SHA256,/email,emailHash"

# Envelope
transform: "ENVELOPE:wrap,payload"
transform: "ENVELOPE:add_metadata"

# Cache lookup
transform: "CACHE_LOOKUP:/userId,user-profiles,/userDetails"
```

---

## Deployment

### Docker Compose

```yaml
services:
  streamforge:
    image: ghcr.io/rahulbsw/streamforge:latest
    volumes:
      - ./config.yaml:/app/config/config.yaml:ro
    environment:
      - CONFIG_FILE=/app/config/config.yaml
      - RUST_LOG=info
```

### Kubernetes (with Operator)

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamForgePipeline
metadata:
  name: my-pipeline
spec:
  replicas: 3
  config:
    bootstrap: kafka:9092
    input: events
    output: processed
```

### Helm

```bash
helm install streamforge ./helm/streamforge-operator \
  --set image.tag=latest \
  --set replicas=3
```

---

## Documentation

### Essential Reading

| Guide | Time | Purpose |
|-------|------|---------|
| [README.md](../README.md) | 5 min | Overview |
| [QUICKSTART.md](QUICKSTART.md) | 10 min | Get started |
| [YAML_CONFIGURATION.md](YAML_CONFIGURATION.md) | 15 min | Config reference |
| [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) | 20 min | Full DSL reference |
| [OBSERVABILITY_QUICKSTART.md](OBSERVABILITY_QUICKSTART.md) | 10 min | Metrics & Grafana |
| [PERFORMANCE.md](PERFORMANCE.md) | 15 min | Tuning guide |

### All Documentation

See [DOCUMENTATION_INDEX.md](DOCUMENTATION_INDEX.md) for the complete list.

---

## Testing

```bash
# Unit + integration tests (333 tests)
cargo test

# Benchmarks
cargo bench

# Security audit
cargo audit
npm audit --prefix ui
```

**Test coverage**: 333 unit tests ✅ | 30+ benchmarks ✅ | 0 vulnerabilities ✅

---

## Roadmap

### Version 0.5.0
- [ ] Avro serialization + Confluent Schema Registry
- [ ] String operations (`UPPER`, `LOWER`, `TRIM`, `SUBSTRING`)
- [ ] Date/time transforms
- [ ] Conditional transforms (`IF:condition,then,else`)

### Version 1.0.0
- [ ] Exactly-once semantics (idempotent producer + transactional consumer)
- [ ] UDF support via WASM or Lua
- [ ] State management with RocksDB
- [ ] Production case studies

---

## Summary

StreamForge is a **production-ready** high-performance Kafka streaming toolkit:

✅ **Fast** — 25K–45K msg/s throughput, 50ns filter ops, 12ms p99 latency  
✅ **Efficient** — ~50MB memory, 20MB container, 0.1s startup  
✅ **Feature-rich** — DSL with filters, transforms, hashing, caching, envelopes  
✅ **Reliable** — at-least-once delivery, DLQ, exponential backoff retries  
✅ **Observable** — Prometheus metrics, Grafana alerts, structured logging  
✅ **Cloud-native** — Kubernetes operator, Helm chart, Web UI, multi-arch images  
✅ **Secure** — SSL/TLS, SASL, Kerberos, Chainguard distroless, 0 CVEs  
✅ **Tested** — 333 unit tests, 30+ benchmarks with real measured results  

---

For complete documentation, see [DOCUMENTATION_INDEX.md](DOCUMENTATION_INDEX.md)
