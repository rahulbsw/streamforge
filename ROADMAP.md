# StreamForge Roadmap

> **Current version: 0.5.0** · [Changelog](docs/CHANGELOG.md)

---

## Shipped (v0.1–v0.5)

Everything below is already in the current release.

**Pipeline engine**
- ✅ Kafka → Kafka pipeline with multi-destination routing
- ✅ Topic regex subscription (`input: "^prod\..*"`)
- ✅ Output topic templates (`output: "mirror.{source_topic}"`)
- ✅ At-least-once delivery with manual commit
- ✅ Dead-letter queue with exponential backoff retry
- ✅ Cross-cluster mirroring

**DSL (Rhai scripting engine)**
- ✅ Full scripting: if/else, switch, closures, string ops, array ops
- ✅ Filter arrays (all ANDed), transform arrays (sequential pipeline)
- ✅ `msg`, `key`, `headers`, `timestamp` in every expression scope
- ✅ Null/empty helpers: `is_null`, `is_empty`, `is_null_or_empty`, `??`
- ✅ `cache_lookup` / `cache_put` callable from scripts
- ✅ `now_ms()` for time-based filtering

**Envelope operations**
- ✅ Key extraction, templates, construction, hashing
- ✅ Header injection, copy, remove, FROM-field
- ✅ Timestamp: preserve, current, extract, offset

**Caching**
- ✅ In-process Moka cache (TTL + TTI)
- ✅ Redis backend
- ✅ Kafka compacted topic as cache (warmup on start)
- ✅ Multi-level L1 (local) + L2 (Redis)

**Observability**
- ✅ Prometheus `/metrics` with per-destination labels
- ✅ Consumer lag monitoring
- ✅ `/health` endpoint
- ✅ Grafana alert rules

**Security**
- ✅ SSL/TLS with mTLS
- ✅ SASL: PLAIN, SCRAM-SHA-256/512, GSSAPI/Kerberos, OAUTHBEARER
- ✅ Chainguard distroless base image (0 CVEs)

**Deployment**
- ✅ Single binary (linux-x86_64, linux-aarch64, macos)
- ✅ Docker / Docker Compose
- ✅ Kubernetes operator (CRD-based pipelines)
- ✅ Helm chart
- ✅ Web UI for operator pipeline management

---

## v0.6.0 — Schema & Validation

**Goal:** Production hardening and developer experience

- [ ] `streamforge validate` — dry-run config validation without connecting to Kafka
- [ ] `streamforge lint` — catch common mistakes in filter/transform scripts at startup
- [ ] Avro support (read/write with Schema Registry)
- [ ] Protobuf support
- [ ] Better startup error messages (which expression failed, at which line)

---

## v0.7.0 — WASM Transforms

**Goal:** Bring-your-own transform logic without modifying StreamForge

- [ ] WASM plugin system — compile a transform in any WASM-targeting language
- [ ] UDF registry — share transform modules across pipeline configs
- [ ] Hot-reload — update scripts without restarting (config watch mode)
- [ ] `streamforge bench-config` — measure throughput of a given config locally

---

## v1.0.0 — Production Hardening

**Goal:** Long-term stable API, exactly-once semantics, stateful transforms

- [ ] Exactly-once semantics (idempotent producer + transactional consumer)
- [ ] RocksDB state backend for stateful transforms (counters, windows)
- [ ] CDC (Change Data Capture) source connector
- [ ] Message replay / backfill — reprocess a time range from a topic
- [ ] Stable config API (no breaking changes after 1.0)

---

## Beyond v1.0

These are on the list but not scheduled:

- Stream-stream joins (two input topics merged by key)
- HTTP/gRPC source and sink connectors
- Pulsar and NATS support
- Multi-tenancy with isolated pipelines and per-tenant metrics
- Visual pipeline builder (drag-and-drop)

---

## Non-Goals

StreamForge is intentionally scoped. It will not become:

- A stream processor with windowing / aggregations (use Kafka Streams or Flink)
- A data lake or storage system
- A message broker replacement

---

## How to Influence the Roadmap

1. **Open an issue** with the `enhancement` label
2. **Vote** on existing issues with 👍
3. **Submit a PR** — the best way to get something built

Contact: [GitHub Issues](https://github.com/rahulbsw/streamforge/issues) · rahul.oracle.db@gmail.com
