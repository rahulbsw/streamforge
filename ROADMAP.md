# StreamForge Roadmap

StreamForge is a focused Kafka selective-replication engine. The v1.x roadmap
prioritizes secure extensions, installation, observability, Kubernetes
operations, and reproducible performance evidence. It does not expand
StreamForge into a general-purpose stream processor.

## Current implemented extension: sandboxed WebAssembly UDFs

The merged UDF PR is part of the current codebase. It provides digest-pinned
Wasmtime component filters, value transforms, and envelope transforms with:

- a versioned WIT ABI and Rust SDK;
- startup verification, digest checks, bounded execution, memory/input/output
  limits, concurrency limits, and forbidden-import validation;
- explicit per-destination bindings and error policies;
- operator ConfigMap/PVC artifact mounting with read-only paths;
- metrics, benchmarks, fixtures, CI checks, and focused documentation.

This changes the former “no UDF work” boundary. The locked replacement is:
the merged bounded WebAssembly ABI is supported; no second UDF runtime, ambient
host capability, in-data-path model inference, or unrelated UDF/DSL expansion
is added without a separate product decision.

See [`docs/WASM_UDFS.md`](docs/WASM_UDFS.md).

## Release sequence

| Release | Focus | Working-tree status | Release status |
| --- | --- | --- | --- |
| `v1.1` | Installation and deployment simplicity | Implemented | Live artifact smoke gates pending |
| `v1.2` | Metrics and logging foundation | Implemented | Live Kafka/Prometheus smoke gates pending |
| `v1.3` | Kubernetes UI onboarding and pipeline operations | Implemented | Browser and kind/minikube gates pending |
| `v1.4` | Performance productization | Gate/catalog foundation implemented | New dedicated-hardware profiles pending |

Releases are sequential. A later release cannot be tagged to bypass an
incomplete earlier release gate.

## Gates shared by every release

A release is not complete until all affected implementation, tests, examples,
documentation, dependencies, and obsolete-code cleanup are complete in the
same release.

Before tagging:

- update `README.md`, `ROADMAP.md`, `CHANGELOG.md`, and
  `docs/IMPLEMENTATION_STATUS.md`;
- update affected architecture, configuration, operations, troubleshooting,
  deployment, UDF, and focused-area guides;
- update documentation indexes and cross-references;
- execute and validate supported commands, examples, image tags, metrics, UDF
  artifacts, and configuration fields;
- archive or remove superseded guidance;
- search for stale versions, binary names, deprecated commands, contradictory
  claims, unresolved work markers, dead compatibility paths, and tracked runtime
  artifacts;
- remove replaced code, dependencies, flags, scripts, UI/API paths, Helm
  values/templates, examples, and configuration fields after tracing callers;
- keep a v1.x deprecation only with a warning, test, migration path, and stated
  removal version;
- publish release notes with additions, behavior changes, migration, known
  limitations, rollback, exceptions, owners, and removal milestones.

Required checks:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all --all-features
```

Also required: Rust and UI unused-dependency analysis, UI lint/type-check/build,
Helm lint/rendering, UDF artifact/digest verification, secret scanning,
dependency vulnerability review, release builds, and a clean intentional
worktree.

## v1.1 — Installation and deployment simplicity

Outcome: users can verify StreamForge locally or install it on Kubernetes
without building release artifacts manually.

Implemented:

- release jobs build `streamforge` and `streamforge-validate` for supported
  Linux and macOS targets;
- release archives receive checksums, SPDX SBOMs, and provenance attestations;
- engine, operator, and UI images use exact versions and multi-architecture
  publication;
- release base images are digest-pinned and engine/operator-managed workloads
  run as non-root identities;
- the Helm chart publishes as a versioned OCI artifact;
- `scripts/quickstart.sh up|verify|down` supports Docker and Podman and verifies
  a real source-to-destination record;
- release validation includes bounded WebAssembly UDF startup;
- moving release tags, non-blocking publication jobs, insecure production UI
  credentials, and divergent chart versions were removed.

Remaining release evidence:

- execute real Docker/Podman and downloaded-binary journeys from clean hosts;
- install the published OCI chart in kind/minikube and verify cleanup;
- record time-to-first-record below ten minutes;
- prove engine, validator, operator, UI, and chart version alignment.

A source-built arm64 operator and engine smoke test passed on 2026-07-26 in an
isolated rootless Podman-backed Minikube cluster. Helm installation, operator
rollout, two-replica pipeline rollout, one-record replication, health,
readiness, metrics, bounded reconciliation, `Ready` status, owner-reference
garbage collection, and namespace/profile/image cleanup passed. The corrected
Kafka 3.9 KRaft example and repeatable
`scripts/tests/minikube_podman_smoke.sh` harness are checked in. This is local
source-built integration evidence, not the still-required published-artifact
gate.

## v1.2 — Metrics and logging foundation

Outcome: StreamForge exposes stable, documented telemetry for dashboards,
alerts, UI consumption, and bounded UDF execution.

Implemented:

- backward-compatible `/health` and structured `/ready` with Kafka-backed
  readiness transitions;
- build/readiness metrics plus existing pipeline and UDF metrics;
- idempotent Prometheus registration and bounded label constants;
- `STREAMFORGE_LOG_FORMAT=text|json` while preserving `RUST_LOG`;
- structured operational fields without payload, credential, key, filter, or
  transform contents;
- ServiceMonitor, PrometheusRule, recording rules, alerts, Grafana dashboard,
  operator metrics port, and HTTP probes.

Remaining release evidence:

- validate PromQL with `promtool`;
- exercise Kafka failure/recovery and UDF failure metrics live;
- scrape the installed chart and exercise alerts/dashboards.

## v1.3 — Kubernetes UI onboarding and pipeline operations

Outcome: platform engineers can create, validate, deploy, observe, and
troubleshoot Kubernetes pipelines without manually writing YAML.

Implemented:

- guided source, multi-destination, processing, reliability/resources,
  validation, and review flow;
- current DSL, TLS/SASL Secret references, retry/DLQ, YAML import/export, and
  additive CRD fields;
- shared CRD-to-engine projection used by operator and validator while
  preserving validated UDF rendering/bindings;
- separate source and target file-backed security projection with inline
  pipeline credentials rejected before workload creation;
- structured validator diagnostics and digest-pinned UDF startup validation;
- bounded validator execution, server-side dry-run, and pipeline summary,
  metrics, events, and bounded logs;
- viewer/admin enforcement, explicit production authentication, and a UI image
  containing the matching validator binary;
- idempotent operator status writes, Deployment-owned readiness events, a
  workload-backed `Ready` condition, and controller owner references for
  generated Deployments and ConfigMaps.

Remaining release evidence:

- run Playwright and all routes in kind/minikube;
- verify status/metrics/events/logs and UDF-bound resources against
  authoritative systems;
- verify the multi-architecture UI image executes its matching validator.

Compatibility notes:

- `StreamforgePipeline` remains `streamforge.io/v1alpha1`;
- retry, DLQ, and UDF fields are additive;
- destinations in one pipeline currently share one target broker set;
- destinations in one pipeline also share one target security configuration;
- the unsupported per-destination compression field was removed from the CRD
  and UI; direct engine configuration retains the supported global policy;
- the UI remains Kubernetes/operator-backed; no standalone control daemon.

## v1.4 — Performance productization

Outcome: performance claims become reproducible release evidence and capacity
guidance.

Implemented:

- publication-grade schema-v3 sustained passthrough harness and canonical AWS
  baseline;
- workload catalog for passthrough, transform-heavy, fan-out, and
  aggregation-heavy profiles at 1 KiB and 10 KiB;
- processing/delivery comparison combinations;
- fail-closed evaluator for publication eligibility, exact accounting, zero
  errors, repetitions, CV, throughput, p99 latency, and RSS;
- named, owned, milestone-bound exceptions;
- local WebAssembly UDF Criterion/functional baseline evidence.

Remaining release evidence:

- extend the Kafka runner to execute every catalog profile and payload size,
  including native and UDF transform-heavy cases;
- capture p99 latency in schema-v3;
- run the supported processing/delivery matrix;
- collect at least three dedicated-hardware repetitions per publication
  workload;
- publish one canonical manifest and capacity recommendation per workload;
- archive historical reports that could be mistaken for current results.

The 2026-07-25 passthrough baseline lacks p99 latency and exact 1 KiB
normalization, so it cannot pass the new gate without an explicit exception.

## AI discovery track

AI remains research-only. Discovery may analyze sanitized support issues,
onboarding failures, configuration errors, troubleshooting patterns, and
capacity questions and produce a go/no-go recommendation.

Locked constraints:

- no model inference in the Kafka data path, including through UDFs;
- no payload, log, credential, topology-secret, or secret transmission by
  default;
- no mandatory external model provider;
- synthetic or sanitized fixtures only;
- no production AI dependency/API/prototype without explicit approval;
- existing AI-ready event streams remain a data-engineering use case.

## Tracking

Milestones: `v1.1`, `v1.2`, `v1.3`, and `v1.4`.

Labels:

- `track/deployment`
- `track/observability`
- `track/ui`
- `track/performance`
- `track/ai-discovery`
- `cleanup/dead-code`
- `docs/drift`

Every issue must state outcome, evidence, dependencies, acceptance tests,
documentation changes, cleanup, security impact, and rollback behavior.
The four milestones and seven labels above were verified in the GitHub
repository on 2026-07-26. Local bug/feature templates require the issue fields
listed above.

## Compatibility defaults

- preserve `CONFIG_FILE`, CRD `v1alpha1`, existing metrics, UDF ABI v1, and
  existing UI APIs during v1.x unless formally deprecated;
- make new fields/endpoints/flags/Helm values additive;
- do not add a standalone UI control daemon;
- do not add major SQL, a second UDF runtime, broad state-management, or
  unrelated DSL expansion without a separate locked decision;
- do not mark a release complete while documentation is stale or superseded
  production code remains without an approved compatibility reason.

**Last updated:** 2026-07-26
