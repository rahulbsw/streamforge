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

The product must define a deterministic order for mutation.

### Required mutation order

Recommended order:

1. parse input payload
2. evaluate filter
3. compute derived fields
4. perform enrichment/cache lookup
5. construct new payload
6. mutate key
7. mutate headers
8. mutate timestamp
9. select destination topic/partitioning info
10. produce

### Required policies

Define explicit behavior for:

* missing field access
* null values
* invalid types
* transform failures
* header overwrite behavior
* timestamp override behavior
* key derivation failure

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

* define mutation order
* finalize cache semantics
* finalize routing precedence
* define missing/null handling

Deliverables:

* documented envelope contract
* enrichment tests
* route precedence tests

Exit criteria:

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

## 21. Claude-Specific Prompt

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

## 22. Final Direction

The correct strategy is:

* keep the custom DSL
* harden the core engine first
* formalize semantics before broadening features
* make StreamForge the best lightweight Rust engine for selective Kafka replication

The project should optimize for **correctness, performance, maintainability, and production clarity**.
