# StreamForge v1.0 Completion Plan

## Executive Summary

**Current Version:** 1.0.0-alpha.1  
**Target Version:** 1.0.0  
**Branch:** `improvement`  
**Execution Mode:** Continuous autonomous flow through defined phases

## Product Definition

**StreamForge** is a Rust-native Kafka-to-Kafka selective replication engine focused on:
- Per-message filtering, transformation, and routing
- Key/header/timestamp manipulation
- Retry, DLQ, and delivery guarantees
- Caching and enrichment
- Production-ready observability
- Cloud-native deployment (Helm, K8s operator)

**NOT in scope:**
- Full MirrorMaker 2 replacement (no offset sync, ACL sync, metadata sync)
- Active-active replication
- Stream analytics platform
- General-purpose scripting runtime
- Kafka Connect ecosystem

## Current State Analysis

### ✅ What Exists (v0.4.0)

**Core Engine:**
- Rust + rdkafka + tokio async runtime
- At-least-once delivery semantics
- Configurable threading model
- Consumer/producer tuning knobs

**DSL Implementation:**
- String-based filter/transform parser (~1800 lines in filter_parser.rs)
- 20+ filter types (AND/OR/NOT, REGEX, ARRAY operations, key/header filters)
- 10+ transform types (EXTRACT, CONSTRUCT, ARITHMETIC, HASH, STRING ops)
- Envelope access (msg, key, headers, timestamp)
- Cache operations (CACHE_LOOKUP, CACHE_PUT)

**Data Path:**
- Multi-destination routing
- Compression support (gzip, snappy, zstd, lz4)
- Partitioning strategies (default, random, hash, field)
- Key transformation pipeline
- Header manipulation
- Timestamp control

**Observability:**
- Prometheus metrics
- HTTP metrics endpoint
- Lag monitoring
- Structured logging (tracing)

**Deployment:**
- Docker images (multi-arch)
- Helm chart for operator
- Kubernetes CRD (StreamforgePipeline)
- Web UI (Next.js, JWT auth)

### ❌ Gaps for v1.0

**Critical (Blockers):**
1. **No formal DSL specification** - grammar, syntax rules, semantics
2. **Parser lacks validation layer** - errors found at runtime, not parse time
3. **Undefined retry/DLQ semantics** - when? how many times? what state?
4. **No config validation CLI** - users can't validate before deploy
5. **Unclear delivery guarantees** - at-least-once is assumed but not documented/tested
6. **Missing integration test suite** - only unit tests and benchmarks
7. **No clear error taxonomy** - errors are strings, not actionable types

**Important (Quality):**
8. **Inconsistent docs** - mix of old/new DSL examples
9. **No v1 compatibility promise** - what's stable? what can change?
10. **Missing common scenario examples** - e.g., CDC, data lake, multi-tenant
11. **Performance not characterized** - throughput limits, bottlenecks unknown
12. **No operational runbook** - troubleshooting, tuning, capacity planning

**Nice-to-Have (Polish):**
13. **UI is secondary** - works but not integrated into main workflow
14. **Operator is pass-through** - could be smarter about validation
15. **No trace correlation** - can't follow message end-to-end

## Phase Execution Plan

### Phase 0: Repository Coherence (COMPLETE ✅)
**Goal:** Clean scope, establish baseline, create tracking

- [x] Analyze current codebase structure
- [x] Document current state and gaps
- [x] Clean up merged branches (feat/observability, feat/v0.5-mm2)
- [x] Create V1_PLAN.md (this file)
- [x] Update Cargo.toml to 1.0.0-alpha.1
- [x] Update ARCHITECTURE.md with v1.0 context
- [x] Audit and fix doc version inconsistencies
- [x] Create DSL simplification inventory

**Deliverables:**
- ✅ V1_PLAN.md (comprehensive 6-phase roadmap)
- ✅ ARCHITECTURE.md (updated with v1.0 gaps and module breakdown)
- ✅ Clean branch state (improvement branch active)
- ✅ Version bumped to 1.0.0-alpha.1
- ✅ docs/DSL_SIMPLIFICATION_INVENTORY.md
- ✅ docs/CHANGELOG.md updated
- ✅ docs/PROJECT_SUMMARY.md updated

### Phase 1: Core Engine Hardening (IN PROGRESS)
**Goal:** Deterministic, testable, documented data plane

**1.1 Error Type System** ✅ COMPLETE
- ✅ Created typed error hierarchy (14+ error types)
- ✅ Mapped errors to recovery actions (RetryWithBackoff, SendToDlq, SkipAndLog, FailFast)
- ✅ Added context propagation (with_context method)
- ✅ Backward compatible with string-based errors
- ✅ All 154 unit tests passing

**1.2 Delivery Semantics** 🚧 IN PROGRESS
- Document at-least-once guarantees
- Implement commit strategies (per-batch, time-based, count-based)
- Add offset management tests
- Define failure scenarios and recovery

**1.3 Retry and DLQ**
- Define retry policy (count, backoff, conditions)
- Implement DLQ routing (dead letter topic)
- Add retry metrics
- Test failure paths

**1.4 Integration Tests**
- Testcontainers setup
- End-to-end pipeline tests
- Failure injection tests
- Performance regression tests

**Deliverables:**
- ✅ src/error.rs refactored with typed errors (14+ types, recovery actions)
- [ ] src/processor.rs with explicit commit logic
- [ ] src/retry.rs + src/dlq.rs modules
- [ ] tests/integration/ directory with 10+ scenarios
- [ ] docs/DELIVERY_GUARANTEES.md
- [ ] docs/ERROR_HANDLING.md

**Progress:** 1/6 deliverables complete (error type system)

### Phase 2: DSL Stabilization
**Goal:** Formal grammar, validation, stable API

**2.1 DSL Specification**
- Write EBNF grammar
- Document all operators with examples
- Specify precedence, escaping, quoting rules
- Define backward compatibility rules

**2.2 Parser Refactor**
- Separate parsing from validation
- Add AST representation
- Implement validation pass (type checking, path validation)
- Better error messages with line/column

**2.3 Config Validation**
- CLI tool: `streamforge validate config.yaml`
- Startup validation with fast-fail
- Warn on deprecated syntax
- Test suite for parser edge cases

**Deliverables:**
- docs/DSL_SPEC.md with EBNF grammar
- src/dsl/ast.rs, src/dsl/parser.rs, src/dsl/validator.rs
- src/bin/validate.rs CLI
- 100+ parser test cases
- docs/DSL_MIGRATION.md (0.x -> 1.0)

### Phase 3: Envelope and Runtime Maturity
**Goal:** Type-safe envelope system, zero-copy paths, deterministic behavior

**3.1 Typed Envelope System** 🎯 NEW
- Design: `Envelope<K, V>` where K/V can be Bytes, String, or Json
- Specification complete in PROJECT_SPEC.md and TYPED_ENVELOPE_DESIGN.md
- Five envelope types with clear use cases:
  * Envelope<Bytes, Bytes> - passthrough (~100K msg/s)
  * Envelope<String, Bytes> - key routing (~80K msg/s)
  * Envelope<Json, Bytes> - key JSON routing (~60K msg/s)
  * Envelope<Bytes, Json> - value filtering (~35K msg/s, most common)
  * Envelope<Json, Json> - full JSON (~25K msg/s)
- Type transitions: deserialize_key(), deserialize_value(), serialize_*()
- DSL type requirements: each operation declares type constraints
- Performance benefits: 3-4x faster for header-only pipelines

**3.2 Message Envelope Implementation**
- Implement generic Envelope<K, V> struct
- Add type transition functions
- Refactor processor pipeline to use typed envelopes
- Optimize allocation patterns
- Document envelope transform semantics
- Test all envelope operations

**3.3 Cache and Enrichment**
- Document cache semantics (consistency, TTL, eviction)
- Test cache miss/hit paths
- Add cache metrics
- Examples for lookup/enrichment patterns

**3.3 Performance Profiling**
- Benchmark hot paths
- Identify allocation hotspots
- Document throughput limits
- Tuning guide

**Deliverables:**
- ✅ docs/TYPED_ENVELOPE_DESIGN.md (complete specification)
- ✅ PROJECT_SPEC.md updated with typed envelope design
- [ ] src/envelope.rs refactored to Envelope<K, V>
- [ ] Type transition functions (deserialize/serialize)
- [ ] DSL parser updated with type requirements
- [ ] Performance benchmarks (all envelope types)
- [ ] docs/CACHING.md
- [ ] benchmarks/ with baseline results
- [ ] docs/PERFORMANCE_TUNING.md (expanded)

**Progress:** Design complete, implementation pending

### Phase 4: Operability and Deployment
**Goal:** Production-ready deployment and ops

**4.1 Configuration Validation**
- Startup validation with clear errors
- Config file schema (JSON schema)
- Environment variable support
- Secrets management guide

**4.2 Observability**
- Standard metric labels
- Health/readiness endpoints
- Structured logging best practices
- Trace IDs

**4.3 Deployment Guides**
- Docker deployment
- Kubernetes deployment
- Helm chart configuration
- Operator usage
- Multi-cluster setup

**4.4 Operational Runbook**
- Troubleshooting guide
- Common failure modes
- Scaling guide
- Upgrade path

**Deliverables:**
- docs/CONFIG_SCHEMA.json
- docs/DEPLOYMENT.md
- docs/OPERATIONS.md
- docs/TROUBLESHOOTING.md
- examples/production/ with realistic configs

### Phase 5: UI and Operator Polish
**Goal:** Enhanced user experience (secondary to core)

**5.1 UI Integration**
- Fix UI to use correct DSL syntax
- Add validation feedback
- Improve error display
- Pipeline templates

**5.2 Operator Intelligence**
- Validate CRDs before apply
- Better status reporting
- Auto-scaling recommendations
- Resource estimation

**5.3 Developer Experience**
- VS Code snippets for DSL
- Config examples
- Interactive tutorial

**Deliverables:**
- ui/ updates
- operator/ updates
- examples/templates/
- docs/GETTING_STARTED.md

### Phase 6: v1.0 Release Readiness
**Goal:** Polish, docs, announce

**6.1 Documentation Audit**
- Verify all docs are consistent
- Update version references
- Complete all TODOs
- Review examples

**6.2 Release Artifacts**
- CHANGELOG.md with full history
- MIGRATION_GUIDE.md (0.x -> 1.0)
- Release notes
- Compatibility matrix

**6.3 Testing**
- Full regression suite
- Example validation
- Performance benchmarks
- Security audit

**6.4 v1.0 Guarantees**
- Document stable APIs
- Backward compatibility promise
- Deprecation policy
- Support plan

**Deliverables:**
- docs/V1_GUARANTEES.md
- CHANGELOG.md
- All tests passing
- Release branch and tags

## Definition of Done (v1.0)

- [ ] All phases completed
- [ ] Version = 1.0.0
- [ ] Zero compiler warnings
- [ ] All tests passing (unit + integration + bench)
- [ ] Documentation complete and consistent
- [ ] Examples run successfully
- [ ] ARCHITECTURE.md reflects reality
- [ ] DSL_SPEC.md is formal and complete
- [ ] Error handling is typed and actionable
- [ ] Retry/DLQ implemented and tested
- [ ] Performance characterized and documented
- [ ] Deployment guides tested
- [ ] Operational runbook complete
- [ ] v1.0 compatibility promise documented
- [ ] Clean git history (squashed, tagged)

## Current Phase Status

**Phase 0: Repository Coherence**  
Status: ✅ 100% complete (2026-04-18)  

**Phase 1: Core Engine Hardening**  
Status: 🚧 Starting next  
Focus: Error types, delivery semantics, retry/DLQ, integration tests

## Execution Principles

1. **Complete one phase before next** - no partial work
2. **Update tests and docs in-phase** - not as backlog
3. **No user decisions needed** - use best judgment
4. **Favor explicit over implicit** - types, errors, docs
5. **Zero-copy when possible** - but correctness first
6. **Every feature has tests** - no exceptions
7. **Every API has docs** - no exceptions
8. **Backward compat when reasonable** - but v1 surface stability prioritized

## Timeline Estimate

Assuming continuous execution:
- Phase 0: 2 hours (current)
- Phase 1: 8 hours (core hardening)
- Phase 2: 6 hours (DSL stabilization)
- Phase 3: 4 hours (runtime maturity)
- Phase 4: 4 hours (operability)
- Phase 5: 3 hours (UI polish)
- Phase 6: 3 hours (release prep)

**Total: ~30 hours of execution time**

## Notes

- Working on `improvement` branch
- Will merge to main when v1.0 complete
- Rhai DSL already rolled back - using original string DSL
- UI exists but is secondary to core
- All changes will be committed incrementally
- Each phase ends with commit + doc update

---

**Started:** 2026-04-18  
**Status:** In Progress - Phase 1 (1/6 deliverables)  
**Last Updated:** 2026-04-18  
**Phase 0 Completed:** 2026-04-18 (2 commits)  
**Phase 1 Progress:** Error type system complete (1 commit)  
**Phase 3 Design:** Typed envelope specification complete (1 commit)  
**Total Commits:** 4
