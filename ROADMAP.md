# StreamForge Roadmap

Vision and planned features for StreamForge.

## Vision

StreamForge aims to be the **fastest, most reliable, and easiest-to-use Kafka selective replication engine**. We focus on:

1. **Performance** - Rust-native speed (25K-45K msg/s sustained throughput)
2. **Reliability** - Production-grade stability with typed errors, retry, and DLQ
3. **Usability** - Simple DSL, great documentation, validation CLI
4. **Security** - Enterprise-ready security features (SSL/TLS, SASL, Kerberos)
5. **Community** - Welcoming, collaborative ecosystem

---

## Current Version: 1.0.0 ✅ STABLE

**Released:** 2026-04-18  
**Status:** Production-ready stable release

### What's Included (v1.0.0)

✅ **Core Engine:**
- Rust + rdkafka + tokio async runtime
- At-least-once delivery semantics (documented and tested)
- Configurable threading model (linear scaling to 8+ threads)
- Consumer/producer tuning knobs (exposed via `performance:` config block)
- Typed error system (14+ error types with recovery actions)
- Dead letter queue (DLQ) with error metadata headers
- Exponential backoff retry policy (configurable max attempts, delays, jitter)

✅ **DSL (v2.0/v2.1/v2.2):**
- **v1.x Colon-delimited syntax** (fully supported, backward compatible)
  - Example: `"AND:/status,==,active:/tier,==,premium"`
- **v2.0 Function-style syntax** (production-ready, auto-detected)
  - Example: `"and(field('/status') == 'active', field('/tier') == 'premium')"`
- **v2.1 Dollar shorthand** (concise field access)
  - Example: `"and($status == 'active', $tier == 'premium')"`
  - Dot notation: `$user.email`, `$data.nested.path`
- **v2.2 Transform evaluators** (35 functions)
  - 14 string transforms: uppercase, lowercase, length, substring, split, join, replace, pad, trim, type conversions
  - 21 date/time transforms: now, parse_date, format_date, add_days, year, month, day, hour, etc.
- AST-based parser with position-tracked errors
- Semantic validation pass before execution
- Complete EBNF grammar specification (docs/DSL_SPEC.md)

✅ **Data Plane:**
- Multi-destination routing with filter-based selection
- 40+ filter types (AND/OR/NOT, regex, array ops, key/header/timestamp filters, null/empty checks)
- 30+ transform types (extract, construct, arithmetic, hash, string ops, date/time ops)
- Envelope access (msg value, key, headers, timestamp, partition, offset, topic)
- Compression support (gzip, snappy, zstd, lz4)
- Partitioning strategies (default, random, hash, field-based)
- Key transformation pipeline
- Header manipulation
- Timestamp control

✅ **Observability:**
- 60+ Prometheus metrics with per-destination tracking
- Kafka consumer lag monitoring
- Filter/transform operation tracking
- HTTP metrics endpoint (< 2% overhead)
- Structured logging (tracing with span IDs)
- Grafana dashboard templates with alert rules

✅ **Deployment:**
- Docker images (multi-arch: x86_64, aarch64)
- Helm chart for Kubernetes Operator
- Kubernetes CRD (StreamforgePipeline v1alpha1)
- Web UI (Next.js with JWT auth)
- Chainguard distroless container images (~20MB)

✅ **Performance:**
- 25,000–45,000 msg/s sustained throughput (JSON processing)
- 12ms p99 latency end-to-end
- 44–50ns simple filter latency
- ~50MB memory footprint

✅ **Testing:**
- **333 unit tests passing** (0 failures, 0 warnings)
  - 102 parser tests (v1 + v2 syntax)
  - 15 dollar syntax tests
  - 11 string transform tests
  - 18 date/time transform tests
  - 187 other tests (filters, transforms, core engine)
- Integration test infrastructure (testcontainers-based)
- Comprehensive benchmarks (filter, transform, end-to-end)

✅ **Documentation:**
- **10,000+ lines across 42 documentation files**
- Complete DSL reference (docs/ADVANCED_DSL_GUIDE.md, docs/DSL_SPEC.md)
- Function-style DSL guide (docs/DSL_V2_FUNCTION_SYNTAX.md)
- Production deployment guides (docs/DEPLOYMENT.md, docs/DOCKER.md, docs/KUBERNETES.md)
- Operations runbook (docs/OPERATIONS.md, 40 KB)
- Troubleshooting guide (docs/TROUBLESHOOTING.md, 70+ issues covered)
- 40+ real-world example configurations
- Delivery guarantees specification (docs/DELIVERY_GUARANTEES.md)
- Error handling taxonomy (docs/ERROR_HANDLING.md)

---

## Version 1.1 (Planned - Q3 2026)

**Goal:** Advanced features and performance enhancements

### Planned Features

- [ ] **Avro Support**
  - Avro serialization/deserialization
  - Confluent Schema Registry integration
  - Schema evolution handling
  - Fast Avro encoding/decoding

- [ ] **Exactly-Once Semantics**
  - Transactional producer support
  - Idempotent consumers
  - End-to-end exactly-once guarantees (EOS)

- [ ] **Generic Envelope<K, V> Refactor**
  - Type-safe envelope system
  - `Envelope<Bytes, Bytes>` for passthrough (zero-copy)
  - `Envelope<Json, Json>` for full processing
  - `Envelope<String, Bytes>` for key-based routing
  - Zero deserialization overhead for passthrough pipelines

- [ ] **User-Defined Functions (UDF)**
  - WASM-based UDF runtime (lightweight, sandboxed)
  - Or Lua scripting (Rhai engine considered)
  - Custom filter/transform logic without recompiling

- [ ] **State Management**
  - RocksDB-backed state store
  - Stateful transformations (aggregations, windows)
  - Fault-tolerant state recovery

- [ ] **Lambda Expressions in DSL**
  - Inline lambda for complex transforms
  - Example: `array_map('/items', item => item.price * 1.2)`
  - Method chaining: `$field.trim().lowercase().split(',')`

### Performance Enhancements

- [ ] Zero-copy optimizations for Envelope<Bytes, Bytes>
- [ ] SIMD operations for bulk filtering
- [ ] Parallel message processing within partition
- [ ] Target: 60K+ messages/second (with zero-copy)

### Developer Experience

- [ ] VS Code extension for DSL syntax highlighting
- [ ] Interactive DSL playground (REPL)
- [ ] Config validation as pre-commit hook
- [ ] Better error messages with suggestions

---

## Version 2.0 (Planned - Q1 2027)

**Goal:** Modernize DSL, deprecate legacy syntax

### Breaking Changes

- [ ] **Deprecate colon-delimited v1 syntax**
  - v1 syntax will still work but emit deprecation warnings
  - Automatic migration tool: `streamforge migrate config.yaml`
  - Full removal in v3.0 (Q4 2027)

- [ ] **Stabilize CRD to v1**
  - StreamforgePipeline moves from v1alpha1 → v1
  - Schema changes finalized

- [ ] **Break 0.x compatibility**
  - Remove deprecated operators (KEY_SUFFIX, KEY_CONTAINS)
  - Remove legacy config formats

### New Features

- [ ] **SQL-like query syntax** (optional alternative to DSL)
  - Example: `SELECT * FROM input WHERE status = 'active' AND tier IN ('premium', 'enterprise')`
  - Transpiled to AST (same execution path as DSL)

- [ ] **Advanced Routing**
  - Topic-to-topic routing matrix
  - Dynamic topic creation
  - Conditional multi-destination routing

- [ ] **Enhanced Observability**
  - OpenTelemetry integration
  - Distributed tracing with Jaeger/Zipkin
  - End-to-end message correlation

### Documentation

- [ ] Complete API reference (auto-generated from code)
- [ ] Interactive tutorials
- [ ] Video walkthroughs
- [ ] Localization (i18n support)

---

## Version 3.0+ (Future)

**Long-term Vision**

- [ ] **Multi-Cloud Support**
  - Amazon MSK
  - Azure Event Hubs (Kafka-compatible)
  - Confluent Cloud optimizations

- [ ] **Advanced Analytics**
  - Real-time aggregations
  - Windowing operations (tumbling, sliding, session)
  - Time-series downsampling

- [ ] **Governance & Compliance**
  - Built-in PII detection and redaction
  - Audit logging (immutable)
  - Role-based access control (RBAC)

- [ ] **Stream Joins**
  - Inner/outer/left joins across topics
  - Temporal joins (time-based windowing)
  - KTable equivalents

---

## Release Schedule

| Version | Target Date | Status |
|---------|-------------|--------|
| v1.0.0  | 2026-04-18  | ✅ Released |
| v1.0.1  | 2026-05-15  | Patch release (bugfixes) |
| v1.1.0  | 2026-09-01  | Feature release |
| v1.2.0  | 2026-12-01  | Feature release |
| v2.0.0  | 2027-03-01  | Breaking changes |
| v3.0.0  | 2028-01-01  | Major evolution |

---

## Contribution Opportunities

Want to contribute? Here are high-impact areas:

### Code Contributions
- Implement Avro support (Issue #123)
- Add exactly-once semantics (Issue #145)
- Build WASM UDF runtime (Issue #178)
- Performance benchmarking suite (Issue #201)

### Documentation Contributions
- Write migration guide v1 → v2 DSL syntax
- Create video tutorials for common use cases
- Translate docs to other languages (Spanish, Mandarin, Japanese)
- Expand examples directory with real-world scenarios

### Testing Contributions
- Add integration tests for complex scenarios
- Performance regression testing
- Chaos engineering (failure injection)
- Load testing at scale (100K+ msg/s)

### Community Contributions
- Answer questions on GitHub Discussions
- Review pull requests
- Maintain Helm chart
- Improve Kubernetes Operator

---

## Feedback & Suggestions

Have ideas for future versions? Open a discussion at:
- **GitHub Discussions:** https://github.com/rahulbsw/streamforge/discussions
- **Feature Requests:** https://github.com/rahulbsw/streamforge/issues/new?template=feature_request.md
- **Slack Community:** https://streamforge.slack.com

We value community input and prioritize features based on user demand!

---

## Version Compatibility Matrix

| Feature | v1.0 | v1.1 | v2.0 | v3.0 |
|---------|------|------|------|------|
| Colon DSL syntax | ✅ | ✅ | ⚠️ Deprecated | ❌ |
| Function-style DSL | ✅ | ✅ | ✅ | ✅ |
| Dollar syntax | ✅ | ✅ | ✅ | ✅ |
| At-least-once | ✅ | ✅ | ✅ | ✅ |
| Exactly-once | ❌ | ✅ | ✅ | ✅ |
| Avro | ❌ | ✅ | ✅ | ✅ |
| UDF (WASM) | ❌ | ✅ | ✅ | ✅ |
| Envelope<K,V> | ❌ | ✅ | ✅ | ✅ |
| SQL syntax | ❌ | ❌ | ✅ | ✅ |
| Stream joins | ❌ | ❌ | ❌ | ✅ |

---

**Last Updated:** 2026-04-18  
**Maintained By:** StreamForge Core Team
