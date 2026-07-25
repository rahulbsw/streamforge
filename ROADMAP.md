# StreamForge Roadmap

Vision and planned features for StreamForge.

## Vision

StreamForge aims to be the **fastest, most reliable, and easiest-to-use Kafka selective replication engine**. We focus on:

1. **Performance** - Measured Rust-native efficiency on representative workloads
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
- Configurable threading and in-flight processing model
- Consumer/producer tuning knobs (exposed via `performance:` config block)
- Typed error system with recovery actions
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
- **v2.2 Transform evaluators**
  - String transforms: uppercase, lowercase, length, substring, split, join, replace, pad, trim, and type conversions
  - Date/time transforms: now, parse, format, arithmetic, and component extraction
- AST-based parser with position-tracked errors
- Semantic validation pass before execution
- Complete EBNF grammar specification (docs/DSL_SPEC.md)

✅ **Data Plane:**
- Multi-destination routing with filter-based selection
- Filters for boolean logic, regex, arrays, keys, headers, timestamps, and null/empty checks
- Transforms for extraction, construction, arithmetic, hashing, strings, and date/time values
- Envelope access (msg value, key, headers, timestamp, partition, offset, topic)
- Compression support (gzip, snappy, zstd, lz4)
- Default keyed/keyless partitioning and field-based partitioning
- Key transformation pipeline
- Header manipulation
- Timestamp control

✅ **Observability:**
- Prometheus metrics with per-destination tracking
- Kafka consumer lag monitoring
- Filter/transform operation tracking
- HTTP metrics endpoint
- Structured logging (tracing with span IDs)
- Grafana dashboard templates with alert rules

✅ **Deployment:**
- Docker images (multi-arch: x86_64, aarch64)
- Helm chart for Kubernetes Operator
- Kubernetes CRD (StreamforgePipeline v1alpha1)
- Web UI (Next.js with JWT auth)
- Chainguard distroless container images

✅ **Performance:**
- Configurable consumer batching, fill timeout, and processing concurrency
- Configuration-time compilation of function-style paths, regexes, and key templates
- Copy-on-write values for transformed destinations and shared values for passthrough destinations
- Keyless default partitioning delegated to librdkafka
- Criterion filter, transform, and end-to-end benchmark targets

✅ **Testing:**
- Unit coverage for parser, filter, transform, routing, partitioning, configuration, and core modules
- Integration test infrastructure (testcontainers-based)
- Criterion benchmarks for filter, transform, and end-to-end paths

✅ **Documentation:**
- Complete DSL reference (docs/ADVANCED_DSL_GUIDE.md, docs/DSL_SPEC.md)
- Function-style DSL guide (docs/DSL_V2_FUNCTION_SYNTAX.md)
- Production deployment guides (docs/DEPLOYMENT.md, docs/DOCKER.md, docs/KUBERNETES.md)
- Operations runbook (docs/OPERATIONS.md)
- Troubleshooting guide (docs/TROUBLESHOOTING.md)
- Real-world example configurations
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

- [x] **Phase 1 hot-path hardening**
  - Delegate keyless default partitioning to librdkafka
  - Skip absent transforms and use copy-on-write for actual transforms
  - Precompile function-style paths/regexes and key-template paths
  - Expose runtime batching and concurrency controls
  - Add focused regression tests and steady-state benchmarks
- [x] Establish the deterministic synthetic and Kafka-backed baseline framework
  - Add stage-level JSON/envelope Criterion measurements
  - Add isolated Kafka repetitions with structured environment/result manifests
  - Record the initial local Phase 2 baseline
- [x] Capture a whole-process CPU profile on representative dedicated hardware
  - AWS c7i.2xlarge passthrough profile captured 614 cycle samples with zero
    lost samples
  - Parsing was about 16.5% inclusive and serialization about 3.2%; the result
    does not trigger the 30% raw/lazy-envelope threshold
- [x] Implement bounded queued delivery and source-partition worker lanes
  - Keep legacy batching and acknowledged delivery as compatibility defaults
  - Reject unsafe queued/manual-commit/retry/DLQ combinations
  - Expose broker-delivery completion separately from enqueue completion
- [x] Correct the sustained Kafka harness measurement contract
  - Start StreamForge and persistent ingress before the timed barrier
  - Separate ingress, timed metrics, and post-window output-validation jobs
  - Exclude startup, warm-up, drain, validation, and teardown
  - Require exact counters/offsets and physical topic-file reclamation
- [x] Add single-destination produced accounting and focused regression tests
- [x] Pass the loopback-only local Podman sustained validation with exact
  consumed, produced, delivered, output, and error counts
- [x] Replace ad hoc AWS host provisioning with a cost-bounded private
  Terraform and ECS-on-EC2 benchmark environment
  - Keep the task and host in a private subnet with no public IP, internet
    gateway, NAT gateway, load balancer, SSH access, or public ingress
  - Gate billable runtime behind an explicit flag and hard expiry
  - Validate a publication-eligible three-repetition sustained baseline and
    destroy all provisioned resources
- [ ] Run the corrected legacy/partition-ordered and
  acknowledged/queued live Kafka comparison matrix
- [ ] Produce a clean-worktree, matched Java/Rust comparison before publishing
  a comparative throughput claim
- [ ] Implement rebalance-aware completed-offset coordination before supporting
  partition-ordered manual commits
- [ ] Profile transform-heavy and aggregation-heavy workloads
- [ ] Implement raw/lazy envelope paths where profiling confirms parse or
  serialization cost
- [ ] Remove array-element cloning from function-style `any`/`all` evaluation
- [ ] Evaluate SIMD only for a profiled vectorizable kernel; the c7i
  passthrough profile did not identify one
- [ ] Measure and tune aggregation data structures and timers

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
- Load testing at production-representative scale

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

**Last Updated:** 2026-07-25

**Maintained By:** StreamForge Core Team
