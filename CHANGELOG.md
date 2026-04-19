# Changelog

All notable changes to StreamForge will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0] - 2026-04-18

### Overview

StreamForge v1.0.0 is the first production-ready release. This release focuses on **stability, reliability, and operability** for production deployments.

**Key Milestones:**
- Typed error system with recovery actions
- Retry and dead letter queue (DLQ) implementation
- Comprehensive documentation (200+ pages)
- Production-ready deployment guides
- JSON schema for config validation
- 168 unit tests passing

---

### Added

#### Core Engine
- **Typed Error System** ([`src/error.rs`](src/error.rs))
  - 14+ error types with explicit recovery actions
  - Context propagation with `.with_context()` method
  - Recovery actions: RetryWithBackoff, SendToDlq, SkipAndLog, FailFast
  - Clone support for error types

- **Retry Policy** ([`src/retry.rs`](src/retry.rs))
  - Exponential backoff with jitter
  - Configurable max attempts, delays, and backoff factor
  - 7 unit tests covering retry scenarios

- **Dead Letter Queue (DLQ)** ([`src/dlq.rs`](src/dlq.rs))
  - Automatic DLQ routing for failed messages
  - Error metadata in message headers:
    - `x-streamforge-error-type`
    - `x-streamforge-source-topic`
    - `x-streamforge-source-partition`
    - `x-streamforge-source-offset`
    - `x-streamforge-filter` (for filter errors)
  - Configurable DLQ topic name
  - 3 unit tests for DLQ scenarios

- **Processor with Retry** ([`src/processor_with_retry.rs`](src/processor_with_retry.rs))
  - Wraps base processor with retry/DLQ logic
  - Integrated into main processing loop
  - Configurable via config.yaml

- **Performance Tuning Configuration** ([`src/config.rs`](src/config.rs))
  - Consumer tuning: `fetch_min_bytes`, `fetch_max_wait_ms`, `max_partition_fetch_bytes`
  - Producer tuning: `batch_size`, `linger_ms`, `queue_buffering_max_ms`, `compression`
  - Exposed via `performance:` config block

#### DSL (Domain-Specific Language)
- **Formal Grammar** ([`docs/DSL_SPEC.md`](docs/DSL_SPEC.md))
  - Complete EBNF grammar specification
  - 40+ operators documented with examples
  - Precedence, escaping, and quoting rules
  - Backward compatibility section

- **Config Validation CLI** ([`src/bin/validate.rs`](src/bin/validate.rs))
  - Pre-deployment config validation: `streamforge-validate config.yaml`
  - Deprecation warnings for `KEY_SUFFIX`, `KEY_CONTAINS`
  - Verbose mode for detailed output
  - Exit codes: 0 (success), 1 (error), 2 (warnings with --fail-on-warnings)

#### Documentation
- **Delivery Guarantees** ([`docs/DELIVERY_GUARANTEES.md`](docs/DELIVERY_GUARANTEES.md), 22 KB)
  - At-least-once semantics documented
  - 4 commit strategies: auto, manual, per-message, time-based
  - 7 failure scenarios with expected outcomes
  - Offset management and recovery procedures

- **Error Handling Guide** ([`docs/ERROR_HANDLING.md`](docs/ERROR_HANDLING.md), 23 KB)
  - Complete error taxonomy
  - Recovery actions for each error type
  - Error handling patterns and best practices

- **Deployment Guide** ([`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md), 45 KB)
  - Docker deployment (Dockerfile, docker-compose)
  - Kubernetes deployment (manifests, RBAC, HPA, PDB)
  - Helm chart configuration
  - Operator usage (StreamforgePipeline CRD)
  - Multi-cluster setup patterns (cross-region, active-passive, hub-and-spoke)
  - Security hardening (TLS, SASL, secrets management, network policies)
  - Production best practices (HA, resource management, performance tuning)

- **Operations Runbook** ([`docs/OPERATIONS.md`](docs/OPERATIONS.md), 40 KB)
  - Daily/weekly/monthly operational tasks
  - Monitoring and alerting (metrics, dashboards, alert rules)
  - Scaling operations (horizontal, vertical, thread scaling)
  - Incident response procedures (high lag, errors, crashes, DLQ overflow)
  - Capacity planning (formulas, growth planning, partition scaling)
  - Maintenance windows (upgrades, config updates, Kafka maintenance)
  - Backup and recovery procedures

- **Troubleshooting Guide** ([`docs/TROUBLESHOOTING.md`](docs/TROUBLESHOOTING.md), 42 KB)
  - 70+ common issues with symptoms, diagnosis, and solutions
  - Startup issues (CrashLoopBackOff, Pending, OOMKilled)
  - Performance issues (high lag, latency, low throughput)
  - Data issues (DLQ messages, missing messages, duplicates)
  - Connectivity issues (Kafka connection, TLS, SASL)
  - Resource issues (OOM, CPU throttling, disk space)
  - Configuration issues (invalid DSL, deprecated syntax)
  - Kafka issues (consumer lag, topic errors, partition mismatch)
  - Debug commands and diagnostic bundle script

- **Typed Envelope Design** ([`docs/TYPED_ENVELOPE_DESIGN.md`](docs/TYPED_ENVELOPE_DESIGN.md), 41 KB)
  - Generic `Envelope<K, V>` specification
  - Five envelope types: Bytes, String, Json, BytesBytes, BytesJson
  - Type transitions and zero-copy optimizations
  - DSL type requirements table
  - Implementation deferred to v1.1 (see Deferred section)

- **Phase 3 Pragmatic Approach** ([`docs/PHASE_3_PRAGMATIC_APPROACH.md`](docs/PHASE_3_PRAGMATIC_APPROACH.md))
  - Decision to defer generic envelope to v1.1
  - Runtime type awareness for v1.0
  - Migration path to v1.1

#### Configuration
- **JSON Schema** ([`docs/CONFIG_SCHEMA.json`](docs/CONFIG_SCHEMA.json))
  - JSON Schema Draft-07 for config.yaml validation
  - All fields documented with types, defaults, and examples
  - 3 complete example configurations in schema

- **Production Examples** ([`examples/production/`](examples/production/))
  - `user-filtering.yaml`: Multi-destination routing (~50K msg/s)
  - `cross-region-replication.yaml`: DR replication (~100K msg/s)
  - `cdc-to-datalake.yaml`: Database CDC streaming (~20K msg/s)
  - `multi-tenant-filtering.yaml`: Tenant routing (~30K msg/s)
  - `pii-redaction.yaml`: Data masking and PII redaction (~15K msg/s)
  - `README.md`: Production examples guide with tuning and deployment instructions

#### Testing
- **Integration Test Infrastructure** ([`tests/integration/`](tests/integration/))
  - Testcontainers setup with Redpanda
  - Common test utilities (TestKafka, message helpers, assertions)
  - Test scenarios: happy path, retry, DLQ, commit strategies, at-least-once delivery
  - Tests marked `#[ignore]` (require Docker to run)

---

### Changed

#### Configuration Format
- **Added `retry:` block** for retry policy configuration
  - `max_attempts`: Maximum retry attempts (default: 3)
  - `initial_delay_ms`: Initial retry delay (default: 100ms)
  - `max_delay_ms`: Maximum retry delay (default: 30s)
  - `jitter`: Add random jitter (default: true)

- **Added `dlq:` block** for dead letter queue configuration
  - `enabled`: Enable DLQ (default: true)
  - `topic`: DLQ topic name (default: "streamforge-dlq")
  - `include_error_headers`: Add error metadata (default: true)
  - `max_retries`: Max retries before DLQ (default: 3)

- **Added `performance:` block** for tuning
  - Consumer: `fetch_min_bytes`, `fetch_max_wait_ms`, `max_partition_fetch_bytes`
  - Producer: `batch_size`, `linger_ms`, `queue_buffering_max_ms`, `compression`

- **Added `commit_strategy` field** (string)
  - Options: "auto", "manual", "per-message", "time-based"
  - Default: "auto"

- **Added `commit_interval_ms` field** (integer)
  - Used with "manual" or "time-based" commit strategies
  - Default: 5000ms

#### Dependencies
- Updated `rdkafka` to 0.36 with SSL, GSSAPI, zstd features
- Added `tokio-retry` 0.3 for retry logic
- Added `testcontainers` 0.15 for integration tests
- Added `structopt` 0.3 for CLI parsing
- All dependencies pinned to stable versions

#### Logging
- Improved error messages with context
- Structured logging for retry attempts
- DLQ routing logged with error details

---

### Deprecated

#### DSL Syntax
The following filter operators are deprecated and will be removed in v2.0:

- **`KEY_SUFFIX:suffix`** → Use `KEY_MATCHES:.*suffix$` instead
- **`KEY_CONTAINS:substring`** → Use `KEY_MATCHES:.*substring.*` instead

**Migration:**
```yaml
# Old (deprecated)
filter: "KEY_SUFFIX:-prod"

# New
filter: 'KEY_MATCHES:.*-prod$'
```

The `streamforge-validate` CLI will warn about deprecated syntax.

---

### Deferred to v1.1

#### Typed Envelope System
- **Full generic `Envelope<K, V>` implementation** deferred to v1.1
  - Reason: 20-30 hours of work, high risk, touches entire codebase
  - v1.0: Documentation complete, runtime type awareness planned
  - v1.1: Full implementation for 3-4x performance gains
  - See: [`docs/TYPED_ENVELOPE_DESIGN.md`](docs/TYPED_ENVELOPE_DESIGN.md) and [`docs/PHASE_3_PRAGMATIC_APPROACH.md`](docs/PHASE_3_PRAGMATIC_APPROACH.md)

#### Parser Refactor
- **Full parser refactor with AST, validator, and improved error messages** deferred to post-v1.0
  - Reason: 13+ hours of work, "nice to have" for better errors
  - v1.0: Formal grammar (DSL_SPEC.md), validation CLI sufficient
  - Future: Lexer → Parser → AST → Validator → Evaluator architecture
  - See: [`docs/PARSER_REFACTOR_PLAN.md`](docs/PARSER_REFACTOR_PLAN.md)

---

### Fixed

#### Error Handling
- Fixed error types from strings to typed enums
- Fixed missing context in error messages
- Fixed transient errors causing permanent failures (now retries)

#### Configuration
- Fixed missing validation at startup
- Fixed env variable substitution in config
- Fixed secret mounting in Kubernetes examples

---

### Removed

#### Experimental Features
- Removed Rhai DSL experiment (rolled back to original string DSL)
  - Reason: Added 10+ dependencies, increased complexity, limited benefit
  - Original colon-delimited DSL is simpler and sufficient

---

## [0.4.0] - Previous Release (Baseline)

### Features in v0.4.0
- Basic Kafka-to-Kafka replication
- String-based filter/transform DSL (~1800 lines)
- 20+ filter types, 10+ transform types
- Multi-destination routing
- Compression support (gzip, snappy, zstd, lz4)
- Prometheus metrics
- Kubernetes CRD (StreamforgePipeline)
- Web UI (Next.js)

### Known Issues in v0.4.0
- No typed error system (errors are strings)
- No retry policy (transient errors fail permanently)
- No DLQ (failed messages are lost or crash pipeline)
- No formal DSL specification
- No config validation CLI
- Limited documentation
- No integration tests

---

## v1.0 Migration Guide

### Breaking Changes

**None.** v1.0 is backward compatible with v0.4.0 configurations.

### New Required Fields

The following fields are optional but recommended:

```yaml
# Retry policy (optional, defaults shown)
retry:
  max_attempts: 3
  initial_delay_ms: 100
  max_delay_ms: 30000
  jitter: true

# DLQ (optional, defaults shown)
dlq:
  enabled: true
  topic: "streamforge-dlq"
  include_error_headers: true
```

### Deprecated Syntax

Update the following deprecated filter syntax:

```yaml
# Old
filter: "KEY_SUFFIX:-prod"
filter: "KEY_CONTAINS:test"

# New
filter: 'KEY_MATCHES:.*-prod$'
filter: 'KEY_MATCHES:.*test.*'
```

Run `streamforge-validate config.yaml` to check for deprecations.

### Performance Tuning

Add `performance:` block for tuning (optional):

```yaml
performance:
  fetch_min_bytes: 5120
  fetch_max_wait_ms: 100
  batch_size: 2000
  linger_ms: 20
  compression: "zstd"
```

See [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md#performance-tuning) for guidance.

---

## Version History

- **v1.0.0** (2026-04-18): Production-ready release
- **v1.0.0-alpha.1** (2026-04-18): Alpha release for testing
- **v0.4.0**: Previous stable release (baseline)

---

## Upgrade Path

### From v0.4.0 to v1.0.0

1. **Backup current config:**
   ```bash
   cp config.yaml config-backup.yaml
   ```

2. **Validate config:**
   ```bash
   streamforge-validate config.yaml
   ```

3. **Update deprecated syntax** (if any warnings from step 2)

4. **Add retry and DLQ config** (recommended):
   ```yaml
   retry:
     max_attempts: 3
     initial_delay_ms: 100
     max_delay_ms: 30000
     jitter: true
   
   dlq:
     enabled: true
     topic: "my-pipeline-dlq"
     include_error_headers: true
   ```

5. **Test in dev/staging first**

6. **Deploy to production:**
   ```bash
   kubectl apply -f deployment.yaml
   kubectl rollout status deployment/streamforge
   ```

7. **Monitor metrics and DLQ:**
   ```bash
   curl http://streamforge:8080/metrics | grep error
   kafka-console-consumer --topic my-pipeline-dlq --max-messages 10
   ```

---

## Support

- **Documentation:** [docs/](docs/)
- **GitHub Issues:** https://github.com/rahulbsw/streamforge/issues
- **Deployment Guide:** [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)
- **Operations Runbook:** [docs/OPERATIONS.md](docs/OPERATIONS.md)
- **Troubleshooting:** [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md)

---

## What's Next (v1.1 Roadmap)

### Planned Features
1. **Typed Envelope System** - Generic `Envelope<K, V>` for 3-4x performance
2. **Parser Refactor** - Better error messages, AST-based validation
3. **Redis Cache Backend** - Distributed caching for enrichment
4. **Trace Correlation** - End-to-end message tracing with OpenTelemetry
5. **UI Improvements** - Better validation feedback, pipeline templates
6. **Operator Intelligence** - Auto-scaling recommendations, resource estimation

### Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

**Release Date:** 2026-04-18  
**Release Manager:** Rahul Jain  
**Contributors:** Rahul Jain, Claude Sonnet 4.5 (AI pair programmer)
