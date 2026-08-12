# StreamForge Product Specification

StreamForge is a Rust-native Kafka selective-replication engine. It consumes
Kafka records, applies deterministic per-destination filtering and
transformation, and produces to one or more Kafka destinations.

This file is the canonical product-boundary document. Its decisions are locked
unless the project owner explicitly changes them. Current implementation
evidence belongs in `docs/IMPLEMENTATION_STATUS.md`; sequenced work belongs in
`ROADMAP.md`; architectural detail belongs in `ARCHITECTURE.md`.

## Product goal

StreamForge should be a focused, lightweight, high-performance, production-safe
Kafka data-plane service for:

- topic-to-topic and cluster-to-cluster selective replication;
- content, key, header, and timestamp filtering;
- payload and envelope transformation;
- multi-destination routing and fan-out;
- bounded retry, dead-letter queue, and offset-commit behavior;
- constrained cache-assisted lookup and aggregation;
- operational metrics, structured logs, liveness, and readiness;
- binary, container, Helm, Kubernetes operator, and Kubernetes UI workflows.

Correctness of delivery semantics takes priority over throughput. Performance
claims require reproducible release evidence and are not part of this product
contract.

## Non-goals

StreamForge is not:

- a full MirrorMaker 2 replacement;
- a Kafka Connect ecosystem;
- an active-active replication or cluster-metadata synchronization system;
- a consumer-group offset, ACL, or topic-configuration synchronization tool;
- a SQL or general stream-analytics platform;
- a general-purpose scripting environment;
- a distributed stateful-processing or state-recovery platform;
- a standalone UI control daemon.

The project must not replace its Rust, Tokio, rust-rdkafka, and custom DSL core
with DataFusion, SQL, or a heavyweight generalized runtime.

## Required data-plane behavior

For each consumed record, StreamForge:

1. captures the value and Kafka envelope metadata;
2. evaluates each configured destination in declaration order;
3. applies native and optional WebAssembly filters;
4. applies value transformations;
5. applies key, header, timestamp, and full-envelope transformations;
6. produces to the selected destination topic;
7. records bounded telemetry;
8. applies configured failure, retry, DLQ, and offset-commit policies.

The current runtime uses a JSON value envelope. A raw or generic typed-envelope
architecture may be evaluated only as a measured, separately approved change;
it is not an existing v1.x capability.

### Delivery contract

- At-least-once is the production reliability target for acknowledged delivery
  with manual commit.
- An offset must not be committed before every required destination outcome is
  satisfied by the configured policy.
- Retry is bounded and observable.
- Terminal failures follow the configured deterministic destination error and
  DLQ policy.
- Exactly-once Kafka transactions are not implemented.
- Queued producer delivery is an explicit performance mode. It is restricted to
  compatible auto-commit, retry, and DLQ settings rather than silently
  weakening stronger contracts.
- Ordering is bounded by Kafka partition ordering and the configured scheduling
  and partitioning behavior.

The detailed compatibility behavior is defined in
`docs/DELIVERY_GUARANTEES.md`.

## DSL contract

The custom filter/transform DSL remains the primary native extension surface.
It must stay deterministic, validated before processing, and lightweight in the
hot path.

The supported contract includes:

- legacy colon-delimited syntax retained under the documented v1.x
  compatibility policy;
- recommended function-style and dollar-path syntax;
- boolean composition, comparisons, existence/null checks, and regex;
- value, key, header, timestamp, string, array, arithmetic, hash, and cache
  operations;
- object construction and field extraction;
- actionable configuration-time diagnostics.

New syntax must not be added merely to provide another spelling for existing
behavior. Parser, AST, validation, evaluation, and error formatting should
remain separated where practical. `docs/DSL_SPEC.md` is the syntax source of
truth.

## WebAssembly UDF contract

The merged stateless WebAssembly component ABI is a supported extension. It may
provide destination filters, JSON value transforms, and bounded mutable
envelope transforms.

Locked constraints:

- every module is named, versioned, and SHA-256 digest-pinned;
- modules are resolved below one configured local artifact root and verified
  before Kafka clients start;
- no WASI, filesystem, network, clock, random, environment, process, or other
  ambient host capability is linked;
- execution, memory, tables, stack, input, output, artifact size, and
  concurrency are bounded;
- guest state is not a persistence or recovery contract;
- UDF failures follow explicit per-destination policy;
- the native DSL remains supported;
- no second UDF runtime or broad plugin ecosystem is introduced without a new
  product decision;
- no model inference runs in the Kafka data path, including through a UDF.

The ABI and operating contract are defined in `docs/WASM_UDFS.md`.

## Configuration and security

Configuration must be typed and validated before processing. Validation covers
source and target Kafka settings, destinations, DSL, security, reliability,
cache/aggregation, observability, performance modes, and UDFs.

Security rules:

- source and target Kafka clients may use distinct TLS/SASL identities;
- Kubernetes custom resources and ConfigMaps contain Secret references or
  mounted file paths, never credential values;
- inline credentials in pipeline CRDs are rejected;
- logs and diagnostics exclude payloads, headers, keys, credentials, and DSL
  expression contents by default;
- no secret value is committed, printed, or written to handoff documentation;
- observability endpoints remain private and are not an authentication layer.

`CONFIG_FILE` remains the engine configuration entry point during v1.x.
`docs/SECURITY_CONFIGURATION.md` defines the supported secret-delivery
patterns.

## Observability contract

The production service exposes:

- backward-compatible `/health` process liveness;
- structured `/ready` runtime and Kafka readiness;
- a documented Prometheus metrics catalog with bounded labels;
- text or line-delimited JSON logs selected by
  `STREAMFORGE_LOG_FORMAT=text|json`;
- `RUST_LOG` level filtering.

Every production metric and structured field must be documented. Dashboards and
alerts must use the same catalog and be validated in the release environment.

## Kubernetes operator and UI

The operator is the Kubernetes control plane. It reconciles
`StreamforgePipeline` resources into engine configuration and workloads. The UI
uses Kubernetes/operator APIs; it does not supervise local engine processes.

Required constraints:

- CRD `streamforge.io/v1alpha1` remains compatible during v1.x unless formally
  deprecated;
- operator and validator share one canonical CRD-to-engine projection;
- Kubernetes server-side dry-run precedes creation;
- validation is bounded and never invokes a shell;
- viewer accounts cannot mutate pipeline resources;
- production UI authentication requires externally managed secrets;
- generated credentials are allowed only in explicit development mode;
- Prometheus access uses predefined server-side queries and enumerated windows;
- no arbitrary command or query execution path is exposed.

## AI discovery

AI assistance remains research-only. Discovery may analyze synthetic or
sanitized onboarding, validation, support, and capacity-planning cases and
produce a go/no-go recommendation.

Locked constraints:

- no production AI dependency or API without explicit approval;
- no model inference in the Kafka data path;
- no payload, log, credential, secret, or sensitive topology transmission by
  default;
- no mandatory external model provider;
- disposable experiments use only synthetic or sanitized fixtures and leave no
  production code or dependency behind.

## Compatibility policy

During v1.x:

- preserve `CONFIG_FILE`, CRD `v1alpha1`, existing supported metrics, UDF ABI
  v1, and existing UI APIs unless formally deprecated;
- make new fields, endpoints, flags, and Helm values additive by default;
- retain deprecated behavior only with a warning, regression test, migration
  path, and stated removal version;
- do not add major SQL, a second UDF runtime, broad state management, or
  unrelated DSL expansion.

Locked decisions stay locked. A conflicting request must be surfaced before it
is implemented.

## Release and quality contract

Releases are gate-driven and sequential as defined in `ROADMAP.md`. A release is
not complete until affected implementation, tests, examples, documentation,
dependencies, security review, and obsolete-code cleanup finish together.

Required qualities include:

- typed public interfaces and input validation at boundaries;
- no hidden steady-state panics;
- deterministic error and recovery behavior;
- bounded resource use and label cardinality;
- formatting, linting, type checking, unused-dependency checks, security
  scanning, unit/integration tests, and release builds;
- current examples that validate and execute using published artifacts;
- one concise source of truth per subject;
- removal of replaced code, documentation, configuration, tests, and generated
  runtime files once compatibility requirements are satisfied.

Live Kafka, Kubernetes, Prometheus, browser, multi-architecture, and dedicated
performance gates must be reported as pending when their required environments
or published artifacts have not been exercised. Source implementation alone is
not evidence that a release is complete.

**Last updated:** 2026-07-26
