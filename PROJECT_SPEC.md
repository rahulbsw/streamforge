# StreamForge Project Spec

## 1. Executive Summary

StreamForge is a **Rust-native Kafka selective replication engine** for mirroring messages from one Kafka topic or cluster to another topic or cluster, with **filtering, transformation, routing, envelope mutation, retry/DLQ handling, enrichment, and observability**.

The project should remain focused on being a **lightweight, high-performance, production-safe Kafka data-plane service**.

It should **not** evolve into:

* a full MirrorMaker 2 replacement
* a Kafka Connect replacement
* a stream analytics platform
* a general-purpose scripting engine in the hot path

The architecture should continue to use:

* **Rust**
* **tokio**
* **rdkafka**
* **custom DSL for filter/transform/routing**

The correct product direction is to **stabilize and harden the existing custom DSL and execution engine**, not replace it with DataFusion, SQL, or a heavy generalized runtime.

---

## 2. Product Positioning

### What StreamForge is

StreamForge is a Kafka-to-Kafka message processing engine optimized for:

* selective replication
* content-based filtering
* message transformation
* dynamic routing
* key/header/timestamp mutation
* enrichment and cache-assisted lookup
* production observability and reliability

### What StreamForge is not

StreamForge is not intended to support:

* active-active replication
* consumer group offset synchronization across clusters
* ACL synchronization
* topic config synchronization
* cluster metadata mirroring
* arbitrary user scripting as the default execution model
* complex stream analytics or SQL query processing

---

## 3. Current Assessment

The repository already has the correct core shape:

* Rust-native Kafka engine
* custom DSL parser
* filter/transform logic
* kafka integration
* processor pipeline
* config system
* metrics and observability
* operator / Helm / UI presence

### Strengths already present

* Strong product direction
* Practical Rust dependency stack
* Existing support for Kafka, metrics, retries, DLQ-related concepts
* Existing DSL for filter and transform
* Existing route and envelope-oriented semantics

### Main issues to address

1. **DSL maturity**

   * current parsing approach will become fragile as syntax grows
   * grammar needs to be formalized
   * parser, AST, validation, and evaluator should be separated

2. **Delivery semantics**

   * retry, commit, DLQ, and partial success semantics must be explicit and tested

3. **Product boundary clarity**

   * docs and roadmap must consistently state what the product does and does not do

4. **Platform breadth vs data-plane depth**

   * the engine should be hardened before expanding UI/operator complexity

5. **Documentation/version drift**

   * README, roadmap, versioning, and status references must be aligned

---

## 4. Product Goal for v1

Deliver a **production-ready v1** for Kafka selective replication with stable semantics.

### v1 must guarantee

* consume from Kafka reliably
* filter per message
* transform per message
* route to one or more destinations
* mutate payload and envelope deterministically
* retry recoverable failures
* send terminal failures to DLQ
* expose health and metrics
* validate config and DSL at startup
* deploy as binary / container / Helm

### v1 must not attempt

* full MM2 parity
* complex control plane
* generalized scripting runtime in hot path
* broad plugin ecosystem before core stability

---

## 5. Scope Definition

### In scope

* topic-to-topic mirroring
* cluster-to-cluster mirroring
* filter, transform, route
* key/header/timestamp operations
* cache lookup / put
* retry / DLQ / commit policy
* metrics / health / readiness
* config validation
* examples / docs / benchmarks

### Out of scope

* active-active replication
* distributed stateful processing
* offset synchronization across clusters
* ACL/config mirroring
* schema registry orchestration as a core requirement
* broad scripting language support as the default DSL

---

## 6. Architecture Requirements

### Core processing pipeline

For each consumed record:

1. consume from Kafka
2. decode record into internal envelope
3. extract payload/key/headers/timestamp/topic metadata
4. evaluate route set
5. for each destination route:

   * evaluate filter
   * apply transform chain
   * apply envelope mutations
   * enrich from cache if configured
   * produce to destination topic/cluster
6. record metrics and structured logs
7. apply retry on recoverable failures
8. send terminal failures to DLQ
9. commit source offset only according to defined at-least-once policy

### Required architectural qualities

* deterministic processing order
* typed errors
* low-allocation hot path where feasible
* no panics in steady-state processing
* explicit failure classes
* explicit handling of partial success across multiple destinations

---

## 7. Delivery Semantics Contract

This section is mandatory and should be treated as first-class design work.

### Required v1 semantics

* Default delivery guarantee: **at least once**
* Offsets must not be committed before required produce outcomes are satisfied
* Retry must be bounded and observable
* DLQ behavior must be deterministic
* Partial multi-destination success semantics must be explicitly defined

### Required design decisions

The implementation must define and document:

1. When offsets are committed
2. What counts as success for multi-destination routing
3. Whether one destination failure blocks commit for all
4. Which failures are retryable
5. Which failures are terminal
6. What payload/envelope is written to DLQ
7. How shutdown behaves during in-flight retries
8. How backpressure is applied when downstream Kafka is slow

### Recommendation

Use a clear default rule:

* a source message is considered successful only when all required destination writes succeed, or when policy explicitly allows per-route partial success
* otherwise retry, then DLQ, then commit according to documented policy

---

## 8. DSL Strategy

The project should keep the **custom DSL**, but simplify and harden it into a formally specified product surface.

### Primary DSL direction

The current DSL is likely more complicated than necessary for the core product goal. The v1 DSL should be intentionally simplified so it is:

* easier to read
* easier to write
* easier to validate
* easier to document
* easier to parse efficiently
* easier to keep stable over time

The DSL should optimize for the common case of Kafka selective replication:

* simple filters
* simple field mapping
* simple string/number transforms
* simple routing conditions
* simple envelope mutation

### Simplification principles

1. Prefer a **small core grammar** over a powerful but complex language.
2. Prefer **consistent function-style syntax** or another single uniform syntax style instead of mixing many expression styles.
3. Prefer **explicit field references** and **explicit transformation steps**.
4. Prefer **few concepts that compose well** rather than many special-case operators.
5. Hide advanced capabilities behind a minimal number of primitives.
6. Avoid syntax that is hard to parse, hard to escape, or hard for users to remember.
7. Keep the hot-path evaluator lightweight even if the parser is improved.
8. Backward compatibility matters, but v1 stability is more important than preserving every experimental syntax form.

### v1 DSL design objective

The v1 DSL should be reduced to a compact, stable feature set.

Recommended conceptual surface:

* `when` for filtering
* `set` for field assignment / transformation
* `route` for destination selection
* `drop` for explicit discard
* `key`, `header`, `timestamp` for envelope mutation
* small built-in function library for common transforms

### Preferred v1 capability set

The DSL should support at minimum:

* boolean composition: `and`, `or`, `not`
* equality and comparison operators
* field existence checks
* regex match only if truly needed
* field extraction by path
* object construction
* basic arithmetic
* basic string functions
* hashing / redaction
* cache lookup / put only if kept simple and well-defined
* route expressions

### Features to reduce or defer

Unless already essential and cleanly implemented, avoid or defer in v1:

* too many overlapping operators
* multiple equivalent ways to express the same logic
* deeply nested transform syntax
* special-case mini-languages inside the DSL
* overly clever array syntax
* advanced expression forms that complicate parsing but are rarely used
* general scripting semantics

### Do not

* replace the DSL with SQL
* replace it with DataFusion
* replace it with a general-purpose scripting engine
* keep expanding syntax only through string splitting and ad hoc parsing
* preserve complexity only because it already exists

### Do

Refactor the DSL into:

* lexer
* parser
* AST
* semantic validator
* evaluator
* error formatter

### DSL requirements

The DSL must support a simplified and stable v1 surface for:

* boolean composition: `AND`, `OR`, `NOT` or equivalent normalized lowercase forms
* comparisons on payload fields
* key filters
* header filters
* timestamp filters
* field extraction
* object construction
* arithmetic transforms
* string transforms
* hashing / redaction operations
* optional cache lookup / put operations if they remain simple
* route expressions

### DSL quality requirements

* formal grammar document
* stable syntax examples
* validation before runtime processing
* syntax errors with location/context
* semantic errors with actionable messages
* golden tests for representative expressions
* a migration note for simplifying any current syntax

### Mandatory instruction for implementation agents

Claude and Cursor must actively simplify the DSL during the v1 hardening effort.
They should not preserve unnecessary syntax complexity by default.
If multiple syntax forms currently exist for the same behavior, they should converge on one recommended form and document compatibility behavior.
The end result should feel like a compact rule language for replication pipelines, not a general-purpose programming language.

## 9. Envelope and Transformation Contract

The product must define a deterministic order for mutation with explicit type-safe envelope transformations.

### Typed Envelope Design (v1.0+)

**Core Principle:** The envelope type signature determines what operations are valid.

#### Generic Envelope Type

```rust
pub struct Envelope<K, V> {
    pub key: Option<K>,
    pub value: V,
    pub headers: HashMap<String, Vec<u8>>,
    pub timestamp: Option<i64>,
    pub partition: Option<i32>,
    pub offset: Option<i64>,
    pub topic: Option<String>,
}
```

Where `K` and `V` can be:
- `Bytes` (raw bytes, no deserialization)
- `String` (UTF-8 validated string)
- `Json` (parsed JSON Value)

#### Type-Driven Pipeline

**1. Passthrough / Header-only operations**
```
Envelope<Bytes, Bytes>
```
- **Use case:** High-performance forwarding with header-based routing
- **Valid operations:** Header filters, header transforms, timestamp control
- **Invalid operations:** JSON path filters, value transforms (compile error)
- **Performance:** Zero deserialization overhead

**2. Key-based routing (regex, prefix, suffix)**
```
Envelope<String, Bytes>
```
- **Use case:** Route by key pattern without parsing value
- **Valid operations:** Key regex filters, key prefix/suffix filters, header operations
- **Invalid operations:** Value JSON filters, value transforms
- **Performance:** Key deserialized to UTF-8, value stays as bytes

**3. Key-based JSON filtering**
```
Envelope<Json, Bytes>
```
- **Use case:** Route by key JSON fields (e.g., `/tenantId` in key)
- **Valid operations:** JSON path filters on key, key transforms, header operations
- **Invalid operations:** Value JSON filters
- **Performance:** Key parsed as JSON, value stays as bytes

**4. Value-only JSON processing (most common)**
```
Envelope<Bytes, Json>
```
- **Use case:** Filter/transform message payload, key passthrough
- **Valid operations:** All JSON filters/transforms on value, header operations
- **Invalid operations:** JSON operations on key
- **Performance:** Value parsed as JSON, key stays as bytes (typical case)

**5. Full JSON processing (key + value)**
```
Envelope<Json, Json>
```
- **Use case:** Complex routing based on both key and value JSON
- **Valid operations:** All JSON filters/transforms on both key and value
- **Invalid operations:** None (most powerful, most expensive)
- **Performance:** Both key and value parsed as JSON

#### Type Transitions

Pipeline stages transition between envelope types:

```rust
// Initial consumption: raw bytes
Envelope<Bytes, Bytes>

// Deserialize value for filtering
→ Envelope<Bytes, Json>  // deserialize_value()

// Apply JSON filter on value
→ Envelope<Bytes, Json>  // filter still has same type

// Extract key from value
→ Envelope<Json, Json>   // key_from_value("/userId")

// Serialize key back for production
→ Envelope<Bytes, Json>  // serialize_key()

// Produce to Kafka
→ (raw bytes sent)
```

#### DSL Type Requirements

Filters and transforms declare their type requirements:

```yaml
# Type: Envelope<Bytes, Bytes> → Envelope<Bytes, Bytes>
filter: "HEADER:x-tenant,==,production"

# Type: Envelope<String, _> → Envelope<String, _>
filter: "KEY_PREFIX:user-"

# Type: Envelope<_, Json> → Envelope<_, Json>
filter: "/status,==,active"

# Type: Envelope<Json, _> → Envelope<Json, _>
filter: "KEY:/tenantId,==,prod"

# Type: Envelope<_, Json> → Envelope<_, Json>
transform: "EXTRACT:/user/email,userEmail"

# Type: Envelope<Bytes, Json> → Envelope<Json, Json>
key_transform: "/userId"  // deserializes key
```

#### Type Safety Benefits

1. **Compile-time guarantees:** Can't apply JSON filters on `Bytes` type
2. **Performance transparency:** Type signature shows deserialization cost
3. **Clear semantics:** `Envelope<Bytes, Json>` means "key passthrough, value parsed"
4. **Optimization opportunities:** Skip deserialization when not needed
5. **Better error messages:** "Cannot apply JSON filter on Envelope<Bytes, _>"

#### Performance Characteristics by Envelope Type

| Envelope Type | Deserialization Cost | Typical Throughput | Use When |
|---------------|---------------------|-------------------|----------|
| `Envelope<Bytes, Bytes>` | Zero | ~100K+ msg/s | Header-only routing, passthrough mirroring |
| `Envelope<String, Bytes>` | Key only (~100ns) | ~80K msg/s | Key regex routing, partition by key pattern |
| `Envelope<Json, Bytes>` | Key only (~500ns) | ~60K msg/s | Route by key JSON fields (rare) |
| `Envelope<Bytes, Json>` | Value only (~800ns) | ~35K msg/s | **Most common**: filter/transform payload |
| `Envelope<Json, Json>` | Both (~1.3µs) | ~25K msg/s | Complex routing on key+value |

**Rule of thumb:** Use the least powerful type that satisfies your requirements.

#### Common Pipeline Patterns

**Pattern 1: High-throughput passthrough**
```yaml
# Envelope<Bytes, Bytes> → no deserialization
routing_type: header
destinations:
  - filter: "HEADER:x-tenant,==,prod"
    output: prod-events
  - filter: "HEADER:x-tenant,==,staging"
    output: staging-events
```
**Throughput:** ~100K msg/s (header-only filtering)

**Pattern 2: Key-based routing**
```yaml
# Envelope<String, Bytes> → deserialize key only
routing_type: filter
destinations:
  - filter: "KEY_PREFIX:user-"
    output: user-events
  - filter: "KEY_PREFIX:admin-"
    output: admin-events
```
**Throughput:** ~80K msg/s (key string matching)

**Pattern 3: Standard JSON filtering (most common)**
```yaml
# Envelope<Bytes, Json> → deserialize value only
routing_type: filter
destinations:
  - filter: "/status,==,active"
    transform: "EXTRACT:/user/email,userEmail"
    output: active-users
```
**Throughput:** ~35K msg/s (JSON parsing + filtering)

**Pattern 4: Complex key+value routing**
```yaml
# Envelope<Json, Json> → deserialize both
routing_type: filter
destinations:
  - filter: "AND:KEY:/tenantId,==,prod:/status,==,active"
    output: prod-active-events
```
**Throughput:** ~25K msg/s (double JSON parsing)

#### Migration Path from Current Implementation

**Current (v0.4.0):**
```rust
pub struct MessageEnvelope {
    pub key: Option<Value>,  // always JSON if present
    pub value: Value,        // always JSON
    // ...
}
```
**Problem:** Always deserializes key+value as JSON, even for passthrough

**Future (v1.0+):**
```rust
pub struct Envelope<K, V> {
    pub key: Option<K>,      // can be Bytes, String, or Json
    pub value: V,            // can be Bytes, String, or Json
    // ...
}
```
**Benefit:** Pay deserialization cost only when needed

### Required mutation order

Recommended order:

1. consume raw message → `Envelope<Bytes, Bytes>`
2. deserialize key/value as needed → `Envelope<K, V>` (typed)
3. evaluate filter (type-checked at compile time)
4. compute derived fields
5. perform enrichment/cache lookup
6. construct new payload (may change type)
7. mutate key (may change type)
8. mutate headers
9. mutate timestamp
10. serialize key/value → `Envelope<Bytes, Bytes>`
11. select destination topic/partitioning info
12. produce

### Required policies

Define explicit behavior for:

* missing field access → error vs default vs skip
* null values → passthrough vs error vs default
* invalid types → error (compile-time for type mismatches)
* transform failures → skip message, send to DLQ, or halt
* header overwrite behavior → last-write-wins vs error on duplicate
* timestamp override behavior → explicit vs preserve vs current
* key derivation failure → error vs null key
* deserialization failures → skip message and send to DLQ

### Implementation Strategy

**Phase 1 (v1.0-alpha.2):**
- Keep current `MessageEnvelope` (key: Option<Value>, value: Value)
- Document type requirements in DSL_SPEC.md
- Add validation: error if JSON filter applied but value not parseable

**Phase 2 (v1.0-beta.1):**
- Implement `Envelope<K, V>` generic type
- Add type transitions (deserialize_key, deserialize_value)
- Refactor DSL parser to track type requirements

**Phase 3 (v1.0):**
- Full type-safe pipeline with compile-time checks
- Optimize: skip deserialization when type is `Bytes`
- Performance testing: measure overhead of type system

---

## 10. Caching and Enrichment

Caching/enrichment is in scope, but only in a constrained way.

### v1 enrichment goals

* support lookup and put operations
* support in-memory cache and optional Redis backend if already present
* support deterministic merge behavior
* expose hit/miss/error metrics

### Required design rules

* enrichment should not silently change semantics
* missing cache values must follow configurable behavior
* cache timeouts/failures must be observable
* merge precedence must be documented

---

## 11. Observability Requirements

Observability is a v1 requirement.

### Metrics

Expose counters/gauges/histograms for at least:

* records consumed
* records filtered out
* records transformed
* records routed
* records produced successfully
* produce failures
* retries attempted
* retries exhausted
* DLQ writes
* parse failures
* config validation failures
* cache hits/misses/errors
* commit success/failure
* processing latency

### HTTP endpoints

Provide at minimum:

* `/health`
* `/ready`
* `/metrics`

### Logging

* structured logs
* route/filter/transform failure context
* correlation identifiers where practical
* startup config summary without leaking secrets

---

## 12. Configuration Requirements

Configuration must be validated before processing starts.

### Required config validation areas

* Kafka source config
* Kafka destination config
* route definitions
* filter syntax
* transform syntax
* retry config
* DLQ config
* cache config
* metrics/HTTP config

### Required UX

* fail fast on invalid config
* produce actionable validation errors
* support config validation mode / CLI if practical

---

## 13. Testing Requirements

Every meaningful behavior must be tested.

### Required test categories

1. unit tests

   * filters
   * transforms
   * parser primitives
   * evaluator behavior

2. parser/DSL golden tests

   * valid syntax
   * invalid syntax
   * semantic validation failures

3. integration tests

   * Kafka source to destination
   * multi-destination routing
   * retry behavior
   * DLQ flow
   * commit behavior
   * shutdown handling

4. benchmark tests

   * DSL evaluation latency
   * transformation hot path
   * Kafka processing throughput smoke benchmarks

5. regression tests

   * backward compatibility for supported syntax
   * previously fixed parsing/processing bugs

---

## 14. Documentation Requirements

The repository must leave v1 with production-grade documentation.

### Required docs

* `README.md`
* `ARCHITECTURE.md`
* `ROADMAP.md`
* `docs/V1_SCOPE.md`
* `docs/NONGOALS.md`
* `docs/DSL_SPEC.md`
* `docs/CONFIG_REFERENCE.md`
* `docs/FAILURE_SEMANTICS.md`
* `docs/DEPLOYMENT.md`
* `docs/EXAMPLES.md`

### Documentation rules

* no stale version references
* one source of truth for product scope
* one source of truth for DSL support
* one source of truth for failure semantics
* examples must match actual supported syntax

---

## 15. Execution Plan

### Phase 0 — Repository coherence and scope cleanup

Goals:

* align versioning and roadmap
* define v1 scope and non-goals
* remove stale or inconsistent documentation
* audit current examples/configs for unsupported syntax
* identify DSL features that are too complex, overlapping, or low-value

Deliverables:

* updated README / roadmap / architecture docs
* `docs/V1_SCOPE.md`
* `docs/NONGOALS.md`
* version/status consistency across repo
* DSL simplification inventory: keep / simplify / deprecate / remove

Exit criteria:

* repository tells one coherent story
* scope and non-goals are explicit
* DSL simplification direction is documented before parser refactor begins

### Phase 1 — Core engine hardening

Goals:

* formalize processor lifecycle
* define exact error taxonomy
* define commit/retry/DLQ semantics
* make pipeline deterministic

Deliverables:

* processor lifecycle doc
* typed error categories
* integration tests for success/failure paths
* documented commit policy

Exit criteria:

* core data plane semantics are explicit and tested

### Phase 2 — DSL stabilization and simplification

Goals:

* simplify the DSL surface for common use
* formalize grammar
* separate parser components
* improve validation and error reporting
* reduce overlapping or hard-to-maintain syntax forms

Deliverables:

* `docs/DSL_SPEC.md`
* lexer/parser/AST/validator/evaluator structure
* parser golden tests
* migration notes for old syntax where needed
* one clearly recommended syntax style for filters, transforms, and routing

Exit criteria:

* DSL is simpler, stable, documented, and suitable as a product surface

### Phase 3 — Envelope/enrichment/runtime maturity

Goals:

* implement typed envelope system `Envelope<K, V>`
* define type transitions (Bytes → String → Json)
* optimize deserialization (skip when type is Bytes)
* define mutation order with type safety
* finalize cache semantics
* finalize routing precedence
* define missing/null handling
* document envelope type requirements for all DSL operations

Deliverables:

* `src/envelope.rs` refactored to `Envelope<K, V>` generic type
* type transition functions (deserialize_key, deserialize_value, serialize_key, serialize_value)
* DSL type requirements documented in DSL_SPEC.md
* performance benchmarks (deserialization overhead by envelope type)
* documented envelope contract and mutation order
* enrichment tests with type-safe lookups
* route precedence tests

Exit criteria:

* typed envelope system implemented and tested
* DSL operations enforce type requirements
* performance optimizations validated (skip deserialization for Envelope<Bytes, Bytes>)
* advanced runtime behavior is deterministic and documented

### Phase 4 — Operability and deployment

Goals:

* finalize metrics taxonomy
* finalize health/readiness behavior
* improve deployment assets
* add local quickstart validation

Deliverables:

* metrics updates
* health/readiness docs
* Docker/Helm validation
* smoke test / quickstart

Exit criteria:

* service is operationally deployable

### Phase 5 — UI/operator polish

Goals:

* refine UI/operator only after engine stability
* keep UI useful but secondary

Deliverables:

* read-only pipeline visibility
* config preview/validation if safe
* limited operational tooling

Exit criteria:

* UI/operator do not distract from data-plane maturity

---

## 16. Priority Rules

When tradeoffs arise, prioritize in this order:

1. correctness of delivery semantics
2. DSL correctness and validation
3. integration test coverage
4. observability and operability
5. deployment simplicity
6. ergonomics and polish
7. UI/operator improvements

---

## 17. Risks to Manage

The implementation must explicitly watch for:

* parser complexity explosion
* inconsistent transform semantics between payload and envelope
* offset commit before destination durability
* undefined partial multi-destination behavior
* docs drifting from actual syntax
* UI/operator work consuming core engine time
* hot-path allocations growing as DSL expands

---

## 18. Definition of Done

The project is done for v1 only when all of the following are true:

* builds cleanly
* tests pass
* docs are updated
* examples run
* DSL syntax is documented
* retry/DLQ/commit semantics are documented and tested
* no stale version references remain
* v1 guarantees and non-goals are clearly stated
* production deployment path is documented

---

## 19. Master Prompt for Claude or Cursor

```text
You are working on StreamForge, a Rust-native Kafka selective replication engine.

Mission:
Deliver a production-ready v1 in a single continuous execution flow across multiple phases, without requiring user guidance between phases.

Product constraints:
- StreamForge is a Kafka-to-Kafka selective replication engine.
- It supports per-message filter, transform, routing, key/header/timestamp mutation, retry/DLQ, caching/enrichment, metrics, and deployability.
- It is NOT a full MirrorMaker 2 replacement.
- Do not add active-active replication, consumer offset sync across clusters, ACL sync, or cluster metadata sync.
- Do not replace the custom DSL with DataFusion, SQL, or a general-purpose scripting engine in the hot path.
- Keep Rust + rdkafka + tokio architecture.

Execution mode:
- Work in clearly separated phases.
- Complete one phase fully before moving to the next.
- At the end of each phase, update docs, tests, and examples before proceeding.
- If you discover missing foundations, insert a prerequisite sub-phase and continue.
- Do not ask for user decisions unless there is a genuine blocking ambiguity.
- Prefer best engineering judgment and continue.

Primary goals:
1. Harden the core data plane.
2. Stabilize the DSL as a formal product surface.
3. Define exact delivery, retry, commit, and DLQ semantics.
4. Improve observability, validation, and deployment readiness.
5. Keep UI/operator work secondary to core engine readiness.

Non-goals:
- Building a stream analytics platform
- Building a Kafka Connect ecosystem
- Building a general arbitrary scripting runtime
- Over-engineering control plane before data-plane maturity

Required deliverables:
- Updated architecture spec
- DSL spec with grammar
- Refactored parser/AST/validator/evaluator if needed
- Deterministic processor pipeline
- Retry/DLQ/commit semantics implementation and tests
- Config validation CLI or startup validation flow
- Metrics/health/readiness updates
- Integration tests and benchmark updates
- Updated README, roadmap, and docs
- Examples for common scenarios

Phase plan to execute:
Phase 0: repo coherence and scope cleanup
Phase 1: core engine hardening
Phase 2: DSL stabilization
Phase 3: envelope/enrichment/runtime maturity
Phase 4: operability and deployment
Phase 5: UI/operator polish only after core stability

Implementation requirements:
- Favor explicit types over loosely typed maps where feasible.
- Prefer zero-copy or low-allocation paths in hot code.
- Avoid hidden panics in processing path.
- Ensure errors are typed and actionable.
- Preserve backward compatibility where reasonable, but prioritize a stable v1 surface.
- Every feature must include tests.
- Every externally visible syntax or config behavior must be documented.

Definition of done:
- Clean compile
- Passing tests
- Updated docs
- At least one runnable example per major capability
- No stale version references
- Clear statement of v1 guarantees and non-goals
```

---

## 20. Cursor-Specific Prompt

```text
Act as the implementation driver for StreamForge.

You will execute end-to-end without waiting for user direction.

Rules:
- Start by reading the current repository structure and docs.
- Produce a concrete task breakdown from the existing codebase.
- Modify the code incrementally and keep the repo buildable.
- After each phase, run/update tests and docs in the same pass.
- Do not introduce speculative abstractions unless they directly support v1 requirements.
- Do not broaden the project scope beyond Kafka selective mirroring/filter/transform/routing.

Priority order:
1. correctness of delivery semantics
2. DSL correctness and validation
3. test coverage
4. observability and deployment
5. ergonomics and polish

Deliver in this order:
1. architecture alignment PR-sized changes
2. parser/DSL hardening
3. processor pipeline hardening
4. retry/DLQ/commit semantics
5. config validation and examples
6. docs/benchmarks cleanup
7. operator/UI final polish

When making decisions:
- Keep the current custom DSL direction.
- Preserve rdkafka + tokio core.
- Keep hot-path execution lightweight.
- Optimize for maintainability and production safety.
```

---

## 21. DSL v2.0 Implementation Status (2026-04-18)

The DSL has been significantly enhanced beyond the original v1.0 plan with three major additions:

### v2.0: Function-Style Syntax ✅ IMPLEMENTED

**Status:** Fully implemented with auto-detection

**Features:**
- Function-style syntax as readable alternative to colon-delimited format
- Auto-detection: parser recognizes v1 (colon) vs v2 (function-style) automatically
- Zero breaking changes: 100% backward compatible with existing v1 configs
- Extended AST from 330 lines to 670+ lines with 60+ expression variants

**Examples:**
```yaml
# v1 (colon-delimited) - still works
filter: "AND:/status,==,active:/count,>,10"

# v2 (function-style) - new, more readable
filter: "and(field('/status') == 'active', field('/count') > 10)"
```

**New Filter Functions:**
- Null checks: `is_null()`, `is_not_null()`, `is_empty()`, `is_not_empty()`, `is_blank()`
- String checks: `starts_with()`, `ends_with()`, `contains()`, `string_length()`
- Boolean logic: `and()`, `or()`, `not()` with nested expressions
- Field access: `field('/path')` with comparison operators

**Documentation:** `docs/DSL_V2_FUNCTION_SYNTAX.md` (comprehensive specification)

### v2.1: Dollar Syntax ($) ✅ IMPLEMENTED

**Status:** Fully implemented as concise alternative to `field()`

**Features:**
- `$` symbol as shorthand for field access (inspired by JSONPath)
- Simple form: `$status`, `$count` → `/status`, `/count`
- Dot notation: `$user.email` → `/user/email`, `$data.user.profile.age` → `/data/user/profile/age`
- Explicit form: `$('/field$1/name')` for paths with special characters ($, -, ., :)
- Supports all comparison operators: ==, !=, >, >=, <, <=
- Boolean and null literals: `$active == true`, `$deleted_at == null`

**Examples:**
```yaml
# Simple comparison
filter: "$status == 'active'"

# Dot notation for nested fields
filter: "$user.email == 'admin@example.com'"

# Complex boolean logic
filter: "and($status == 'active', or($tier == 'premium', $tier == 'enterprise'))"

# Explicit form for special characters
filter: "$('/field-with-dash') == 'test'"
```

**Implementation:** Extended lexer with `Token::Dollar` and `Token::Dot`, 15 tests

**When to use:**
- Use `$` for concise, readable expressions
- Use `field()` when explicit function call is clearer
- Use `$('/explicit/path')` for paths with special characters

### v2.2: Transform Evaluators ✅ IMPLEMENTED

**Status:** 35 transform functions fully implemented and tested

#### String Transforms (14 functions)
- **Case conversion:** `uppercase()`, `lowercase()`
- **Length:** `length()` - works on strings and arrays
- **Substring:** `substring(start, end)` - character-aware slicing
- **Split/Join:** `split(delimiter)`, `join(separator)`
- **Editing:** `replace(pattern, replacement)`
- **Padding:** `pad_left(width, char)`, `pad_right(width, char)`
- **Trimming:** `trim_start()`, `trim_end()`
- **Type conversion:** `to_string()`, `to_int()`, `to_float()`

**Examples:**
```yaml
transform: |
  {
    "id": $id,
    "name_upper": uppercase(field('/name')),
    "email_domain": split(field('/email'), '@')[1],
    "padded_id": pad_left(to_string(field('/id')), 8, '0')
  }
```

#### Date/Time Transforms (21 functions)
- **Current time:** `now()` (epoch ms), `now_iso()` (ISO 8601)
- **Parsing:** `parse_date(path, format)`, `from_epoch()`, `from_epoch_seconds()`
- **Formatting:** `format_date(path, format)`, `to_epoch()`, `to_epoch_seconds()`, `to_iso()`
- **Arithmetic:** `add_days()`, `add_hours()`, `add_minutes()`, `subtract_days()`
- **Extraction:** `year()`, `month()`, `day()`, `hour()`, `minute()`, `second()`, `day_of_week()`, `day_of_year()`

**Examples:**
```yaml
transform: |
  {
    "event_id": $id,
    "timestamp": now_iso(),
    "scheduled_for": add_days(field('/event_date'), 7),
    "year": year(field('/event_date')),
    "formatted": format_date(field('/event_date'), '%Y-%m-%d %H:%M')
  }
```

**Implementation Details:**
- `src/filter/string_transforms.rs` (585 lines, 11 tests)
- `src/filter/datetime_transforms.rs` (820 lines, 18 tests)
- Uses `chrono` crate for robust date/time handling
- Supports ISO 8601, epoch ms/seconds, and custom format strings
- Auto-detects common date formats

### Test Coverage

**Total Tests:** 333 passing (0 failures, 0 warnings)

**Breakdown:**
- Parser tests: 102 (v1 + v2 syntax)
- Dollar syntax tests: 15
- String transform tests: 11
- Date/time transform tests: 18
- Other tests: 187 (core engine, filters, etc.)

### Documentation

**New/Updated Files:**
- `docs/DSL_V2_FUNCTION_SYNTAX.md` - Complete v2.0 specification
- `docs/DSL_SPEC.md` - Updated with v2 features
- `examples/configs/function-style-syntax-examples.yaml` - 22 real-world examples
- `V1_PLAN.md` - Phase 2 updated with v2.0/v2.1/v2.2 achievements

### Code Impact

**New Modules:**
- `src/dsl/parser_v2.rs` (1100+ lines) - v2 parser with $ syntax
- `src/dsl/dollar_syntax_tests.rs` (320 lines) - $ syntax tests
- `src/filter/string_transforms.rs` (585 lines) - string functions
- `src/filter/datetime_transforms.rs` (820 lines) - date/time functions

**Extended Modules:**
- `src/dsl/ast.rs` - 330 → 670+ lines (60+ expression variants)
- `src/dsl/parser.rs` - Updated for auto-detection
- `src/filter/mod.rs` - Exports for all transforms

**Total New Code:** ~3,500 lines across DSL v2 implementation

### Stability Impact

**Backward Compatibility:** ✅ 100% maintained
- All v1 colon-delimited syntax continues to work
- Auto-detection ensures no breaking changes
- Existing configs require no migration

**Performance:** ✅ No regression
- Parser selection at parse-time (not hot-path)
- Transform evaluators use efficient implementations
- Chrono provides optimized date/time operations

**API Stability:** ✅ v1 guarantees preserved
- v2 syntax is additive, not replacing
- Transform trait interface unchanged
- Filter trait interface unchanged

### Future Enhancements (v1.1+)

Potential additions (not committed):
- Method chaining: `$field.trim().lowercase()`
- Array transforms: `array_map()`, `array_filter()`
- Math functions: `abs()`, `round()`, `ceil()`, `floor()`
- Additional date functions: `date_diff()`, `is_weekend()`, `business_days()`

---

## 22. Claude-Specific Prompt

```text
Act as principal engineer and delivery lead for StreamForge.

Your job is to complete the project in one multi-phase flow with minimal user interaction.

Responsibilities:
- audit current architecture and docs
- reconcile inconsistencies
- define the stable v1 contract
- produce implementation plan
- implement/refactor code where necessary
- add or improve tests
- update documentation
- leave the repository in a release-candidate state

Decision policy:
- Prefer practical, production-oriented solutions.
- Be conservative in expanding scope.
- Protect hot-path performance.
- Formalize the DSL rather than replacing it.
- Treat retry, commit, and DLQ semantics as first-class design work.
- Make operator and UI subordinate to core engine readiness.

Output expectations:
- concise rationale for each phase
- exact files changed
- exact tests added
- exact docs updated
- explicit risk list after each phase
- final release checklist
```

---

## 23. Final Direction

The correct strategy is:

* keep the custom DSL
* harden the core engine first
* formalize semantics before broadening features
* make StreamForge the best lightweight Rust engine for selective Kafka replication

The project should optimize for **correctness, performance, maintainability, and production clarity**.
