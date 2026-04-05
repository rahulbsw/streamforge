---
title: Changelog
nav_order: 13
---

# Changelog

All notable changes to StreamForge are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [0.4.0] - 2026-04-03 - Observability, Envelopes & Release Pipeline

### Added

#### Prometheus Observability
- Prometheus metrics exporter on `/metrics` HTTP endpoint
- Grafana dashboard templates (`examples/streamforge_alerts.yml`)
- Per-destination throughput, latency, and error counters
- JVM-style process metrics (CPU, memory, file descriptors)
- `docs/OBSERVABILITY_QUICKSTART.md` and `docs/OBSERVABILITY_METRICS_DESIGN.md`

#### Envelope Transforms
- `ENVELOPE:wrap` — wraps the full message payload into a named field
- `ENVELOPE:unwrap` — extracts an inner field as the new root
- `ENVELOPE:add_metadata` — injects topic, partition, offset, timestamp headers
- Migration guide: `docs/ENVELOPE_MIGRATION_GUIDE.md`
- Design reference: `docs/ENVELOPE_FEATURE_DESIGN.md`
- Example configs: `examples/config.envelope-simple.yaml`, `examples/config.envelope-features.yaml`

#### Multi-Architecture Release Pipeline
- Automated GitHub Actions release workflow (`release.yml`)
- Pre-built binaries for `linux-x86_64`, `linux-aarch64`, `macos-x86_64`, `macos-aarch64`
- Docker images published to GHCR (`ghcr.io/rahulbsw/streamforge`)
- Chainguard distroless base images for minimal attack surface
- Native ARM64 runner eliminates QEMU cross-compilation overhead

### Fixed
- All clippy warnings resolved across main crate and operator
- Operator reconciler unused import removed
- 17 npm security vulnerabilities in UI dependencies resolved

---

## [0.3.0] - 2026-04-01 - Concurrent Processing, Hash & Cache

### Added

#### Concurrent Processing
- Multi-threaded pipeline with configurable thread count (`threads: N`)
- Lock-free work queue for message dispatch
- Linear scaling validated: 8 threads → 25,000–34,500 msg/s
- Concurrent consumer/producer architecture with Tokio

#### Hash Transforms
- 5 hash algorithms: `MD5`, `SHA256`, `SHA512`, `MURMUR64`, `MURMUR128`
- DSL syntax: `HASH:algorithm,/path[,outputField]`
- Use cases: PII anonymization, deduplication, consistent partitioning
- Throughput: up to 10M ops/s (Murmur), 2M ops/s (SHA256)

#### Cache Backends
- **Local cache** (Moka): TTL/TTI eviction, 50ns lookup, async-first
- **Redis cache**: connection pooling, key prefixes, auto-expiration
- **Kafka-backed cache**: compacted topic as distributed cache, warmup on start
- **Multi-level cache**: L1 (local) + L2 (Redis), automatic promotion
- DSL syntax: `CACHE_LOOKUP:/keyPath,cacheName,/outputField`
- Feature flags: `local-cache` (default), `redis-cache`, `all-caches`

#### At-Least-Once Delivery
- Manual commit mode with async or sync commit options
- Configurable commit interval (batching)
- Dead Letter Queue (DLQ) with configurable topic
- Exponential backoff retry (initial 100ms → max 30s, multiplier 2.0)
- Backward compatible — auto-commit remains the default

#### Kubernetes & Helm
- Kubernetes Operator (`operator/`) for CRD-based pipeline management
- Helm chart (`helm/streamforge-operator/`) for operator deployment
- Kubernetes secret support for secure Kafka connections
- Web UI for operator pipeline management (`ui/`)

### Performance
- 25,000–34,500 msg/s at 8 threads (linear scaling)
- Memory: ~50MB (vs ~500MB Java MirrorMaker)
- Hash operations: 50ns–1µs depending on algorithm
- Local cache lookup: 50ns p50, 100ns p99

---

## [0.2.0] - 2026-03-10 - Advanced DSL & YAML Support

### Added

#### YAML Configuration Support
- ✅ YAML format support (`.yaml`, `.yml` extensions)
- ✅ Automatic format detection based on file extension
- ✅ Backward compatible with JSON
- ✅ Multi-line strings for complex filters
- ✅ Inline comments for documentation
- ✅ Much more readable for complex configurations

**Examples:**
```yaml
routing:
  destinations:
    # Users with valid email
    - output: validated-users
      description: Email validation pipeline
      filter: "REGEX:/user/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"
```

**Files:**
- `config.example.yaml` - Simple YAML example
- `config.multidest.yaml` - Multi-destination YAML
- `config.advanced.yaml` - Advanced YAML with all features
- `YAML_CONFIGURATION.md` - Complete YAML guide

#### Array Operations
- ✅ `ARRAY_ALL` filter - Check if all elements match a condition
- ✅ `ARRAY_ANY` filter - Check if any element matches a condition
- ✅ `ARRAY_MAP` transform - Map over array elements
- ✅ Support for nested array element filtering
- ✅ Empty array handling

**Examples:**
```json
"filter": "ARRAY_ALL:/users,/status,==,active"
"filter": "ARRAY_ANY:/tasks,/priority,==,high"
"transform": "ARRAY_MAP:/users,/id"
```

#### Regular Expressions
- ✅ `REGEX` filter for pattern matching
- ✅ Full regex syntax support
- ✅ Compiled patterns for optimal performance
- ✅ Case-sensitive matching

**Examples:**
```json
"filter": "REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"
"filter": "REGEX:/version,^2\\."
"filter": "REGEX:/status,^(active|pending)$"
```

#### Arithmetic Operations
- ✅ `ARITHMETIC:ADD` - Addition
- ✅ `ARITHMETIC:SUB` - Subtraction
- ✅ `ARITHMETIC:MUL` - Multiplication
- ✅ `ARITHMETIC:DIV` - Division
- ✅ Support for path-to-path operations
- ✅ Support for path-to-constant operations
- ✅ Division by zero error handling

**Examples:**
```json
"transform": "ARITHMETIC:ADD,/price,/tax"
"transform": "ARITHMETIC:MUL,/price,1.2"
"transform": "ARITHMETIC:SUB,/total,/discount"
"transform": "ARITHMETIC:DIV,/total,/count"
```

#### Documentation
- ✅ ADVANCED_DSL_GUIDE.md - Comprehensive DSL reference
- ✅ DSL_FEATURES.md - Feature summary and comparison
- ✅ config.advanced.example.json - Example configurations

#### Tests
- ✅ 19 new test cases for array operations
- ✅ 8 new test cases for regular expressions
- ✅ 14 new test cases for arithmetic operations
- ✅ Parser tests for all new features
- ✅ 100% test pass rate (56 tests passing)

### Changed
- Updated README.md with DSL capabilities section
- Updated IMPLEMENTATION_STATUS.md to reflect completed features
- Updated comparison table to show Rust advantages
- Removed JSLT/JavaScript from "Future Enhancements"

### Performance
- Array operations: ~1-10µs (size dependent)
- Regular expressions: ~500ns-1µs (complexity dependent)
- Arithmetic operations: ~50ns
- Overall: 40x faster than Java JSLT

---

## [0.1.0] - 2026-03-10 - Initial Release

### Added

#### Core Features
- ✅ Cross-cluster Kafka mirroring
- ✅ Async/await with Tokio runtime
- ✅ Custom partitioning (hash-based, field-based)
- ✅ Multi-destination routing
- ✅ Native Kafka compression (Gzip, Snappy, Zstd)
- ✅ Lock-free metrics with atomic operations

#### Filtering & Transformation
- ✅ JSON Path filters with comparison operators
  - Numeric: `>`, `>=`, `<`, `<=`, `==`, `!=`
  - String: `==`, `!=`
  - Boolean: `==`, `!=`
- ✅ Boolean logic (AND/OR/NOT)
- ✅ JSON Path transforms (field extraction)
- ✅ Object construction (CONSTRUCT)
- ✅ Per-destination filters and transforms

#### Docker Support
- ✅ Multi-stage Dockerfile with Chainguard base images
- ✅ Static binary variant (Dockerfile.static)
- ✅ Docker Compose configuration
- ✅ ~20-30MB dynamic image size
- ✅ ~10-15MB static image size
- ✅ Non-root user execution
- ✅ Health checks included

#### Configuration
- ✅ JSON-based configuration
- ✅ Single-destination mode
- ✅ Multi-destination routing mode
- ✅ Environment variable config path
- ✅ Consumer/producer property override

#### Metrics
- ✅ Processed messages counter
- ✅ Filtered messages counter
- ✅ Completed messages counter
- ✅ Error counter
- ✅ Rate calculation
- ✅ Periodic reporting (10s interval)

#### Documentation
- ✅ README.md - Project overview
- ✅ QUICKSTART.md - Getting started guide
- ✅ IMPLEMENTATION_NOTES.md - Architecture details
- ✅ ADVANCED_FILTERS.md - Boolean logic guide
- ✅ DOCKER.md - Docker deployment guide
- ✅ IMPLEMENTATION_STATUS.md - Feature tracking

### Performance
- Memory usage: ~50MB (vs ~500MB Java)
- CPU efficiency: 2-3x better than Java
- Throughput: ~25K msg/s (vs ~10K Java)
- Latency p99: ~15ms (vs ~50ms Java)
- Filter evaluation: ~100ns per filter

---

## Roadmap

### Version 0.5.0 (Planned)
- [ ] Avro serialization support
- [ ] Schema registry integration
- [ ] Schema evolution handling
- [ ] String manipulation operations (`UPPER`, `LOWER`, `TRIM`, `SUBSTRING`)
- [ ] Date/time operations and format transforms
- [ ] Conditional transforms (`IF:condition,thenTransform,elseTransform`)

### Version 1.0.0 (Planned)
- [ ] Exactly-once semantics (idempotent producer + transactional consumer)
- [ ] UDF support via WASM or Lua
- [ ] State management with RocksDB
- [ ] Production hardening and SLA documentation
- [ ] Comprehensive production case studies
