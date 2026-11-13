# Implementation Status

Verified source status as of 2026-07-26. This document records current
capabilities and remaining release evidence. It does not convert source
implementation, unit tests, or historical measurements into a release claim.

## Focus tracks

| Track | Implemented in source | Outstanding release evidence |
| --- | --- | --- |
| `v1.1` deployment | Release packaging, synchronized exact-version images/chart, secure Helm defaults, Docker/Podman quickstart, corrected Kafka example, Podman-Minikube smoke harness | Published artifacts, clean-host quickstarts, multi-architecture smoke |
| `v1.2` observability | Readiness, stable metric/log fields, catalog, monitoring assets, probes | `promtool`, live Kafka recovery, Prometheus/dashboard/alert signal path |
| `v1.3` Kubernetes UI | Guided multi-destination flow, safe validation/dry-run, operations APIs/views, authorization, idempotent operator status, `Ready`, resource ownership | Playwright, authoritative UI integration |
| `v1.4` performance | Workload/policy catalog, normalized baseline record, fail-closed release evaluator | Profile-capable runner, p99 capture, dedicated workload/mode matrix, capacity guidance |

The releases are sequential and remain untagged while their live gates are
outstanding.

GitHub milestones `v1.1` through `v1.4` and the seven roadmap labels were read
back from the repository on 2026-07-26. Local issue and pull-request templates
require outcome, evidence, dependencies, acceptance tests, documentation,
cleanup, security impact, and rollback behavior.

## Installation and release pipeline

Implemented:

- `streamforge` and `streamforge-validate` release archives for supported Linux
  amd64/arm64 and macOS arm64 targets;
- archive checksums, SPDX SBOMs, and provenance;
- mandatory exact-version multi-architecture engine, operator, and UI image
  publication;
- OCI Helm chart publication with synchronized chart/application versions;
- digest-pinned release base images;
- non-root engine, operator, and pipeline workload identities;
- `scripts/quickstart.sh up|verify|down` with Docker/Podman selection, topic
  creation, real record verification, and cleanup;
- release preflight that validates component versions and container builds;
- side-effect-free `streamforge --help` and `--version`;
- a bounded Docker build context that excludes nested build/dependency state.

Verified locally:

- locked macOS release binaries build;
- the Cargo package contains only the intended 88 files and packages offline;
- Linux source-build and prebuilt-binary engine images build and their
  side-effect-free help/version smokes pass;
- operator release container builds and its `--help` smoke passes;
- Helm lint plus default, production UI, development UI, and monitoring
  representative renders pass;
- mocked Docker and Podman quickstart lifecycle and cleanup tests pass.

Still required:

- published archive/checksum/SBOM/provenance verification;
- published multi-architecture image execution;
- real clean-host Docker and Podman time-to-first-record below ten minutes;
- published OCI chart installation, rollout, and teardown in kind/minikube.

### Podman-backed Minikube evidence

An isolated local smoke test passed on 2026-07-26 with:

- Minikube `v1.38.1`, Kubernetes `v1.31.0`, the rootless Podman driver,
  containerd `2.2.1`, bridge CNI, and kube-proxy `masqueradeAll`;
- source-built arm64 `streamforge` and operator images containing version
  `1.1.0`;
- local Helm installation and an operator rollout with zero restarts;
- a two-replica `StreamforgePipeline` using Kafka `3.9.0`;
- one exact JSON input record observed on the output topic;
- `/health` returning `OK`, structured `/ready` reporting runtime and Kafka
  ready, `streamforge_build_info{version="1.1.0"} 1`, and
  `streamforge_ready 1`;
- UID `65532`, no privilege escalation, a read-only root filesystem, and no
  pipeline service-account token mount;
- a workload-backed `Ready=True` condition and `phase=Running`;
- no more than five reconciliations in the harness's 35-second observation
  window;
- controller owner references and successful garbage collection of the
  generated Deployment and ConfigMap after custom-resource deletion;
- Helm, namespace, Minikube profile, temporary archive, and uniquely tagged
  local-image cleanup.

The repeatable path is `scripts/tests/minikube_podman_smoke.sh`. It refuses an
existing profile/context, requires Minikube `v1.38.1` or newer through
`MINIKUBE_BIN`, forces command-scoped rootless Podman, waits for cluster network
components, applies kube-proxy masquerading only after a failed ClusterIP
probe, uses an absolute Kafka Service DNS name at runtime, and restores the
caller's prior Kubernetes context. Static tests prove existing-profile
preservation and cleanup after a partial Minikube start.

This resolves the earlier exploratory findings: the Kafka manifest now uses
the Apache 3.9 empty-host KRaft listener form, operator status writes are
idempotent, `Ready` is present, and generated resources are owned by the
pipeline.

The local source-built smoke does not satisfy the published-artifact,
multi-architecture, clean-host time-to-first-record, UI, or Prometheus gates.

## Metrics and logging

Implemented:

- backward-compatible `/health` literal `OK`;
- structured `/ready` with HTTP 200/503 from runtime and Kafka readiness;
- `streamforge_build_info` and `streamforge_ready`;
- idempotent registry setup and bounded label sources;
- `STREAMFORGE_LOG_FORMAT=text|json` while retaining `RUST_LOG`;
- structured operational fields that omit payloads, headers, message keys,
  credentials, and DSL expression contents;
- private metrics Service, ServiceMonitor, PrometheusRule, recording/alert
  rules, Grafana dashboard, and operator HTTP probes;
- one current catalog and label-cardinality policy in
  `OBSERVABILITY_QUICKSTART.md`.

Focused Rust, Helm, YAML, dashboard JSON, registration, readiness, and logging
tests pass. Every production registry metric name is present in the catalog;
the only catalog-only family is the documented derived histogram bucket series.
`promtool`, live Kafka failure/recovery, and an installed Prometheus signal path
remain release gates.

## Kubernetes configuration, operator, and UI

Implemented:

- six-step guided multi-destination onboarding with YAML import/export, current
  DSL, reliability/resources, validation, and review;
- TLS/SASL Kubernetes Secret references, retry/DLQ, and additive UDF CRD fields;
- one `streamforge-config-model` projection shared by operator and validator;
- distinct source/target engine security and protected credential-file loading;
- canonical Secret mount paths, unsafe-path rejection, inline pipeline
  credential rejection, and identical target-broker/security enforcement;
- structured validator JSON diagnostics and CRD input;
- bounded no-shell validation, Kubernetes server-side dry-run, and creation;
- pipeline summary, predefined metrics windows, events, and bounded log APIs;
- viewer/admin enforcement and explicit production UI authentication;
- a UI image definition that builds the matching validator architecture;
- idempotent status writes with stable transition timestamps, Deployment-owned
  reconciliation events, a workload-backed `Ready` condition, and controller
  owner references on generated Deployments and ConfigMaps;
- removal of the unsupported per-destination compression field, duplicate CRD
  mapping, obsolete onboarding form, and contradictory Kubernetes examples.

Verified locally:

- every supported `examples/pipelines/*.yaml` manifest passes the release
  validator in `pipeline-crd` mode;
- Rust projection/security/validator tests pass;
- operator formatting, warnings-denied Clippy, tests, and unused-dependency
  checks pass;
- UI tests, ESLint, TypeScript, unused-dependency analysis, and production build
  pass;
- UI release container builds and its bundled Linux validator smoke passes;
- production npm dependency audit reports zero vulnerabilities.

Still required:

- Playwright against kind/minikube;
- comparison of UI status, metrics, events, and logs with Kubernetes, Kafka,
  and Prometheus sources of truth.

## Core data plane

Implemented:

- Tokio/rust-rdkafka consume, process, and produce pipeline;
- single- and multi-destination routing;
- optional filters and value/envelope transforms;
- automatic and manual commit modes;
- bounded retry and dead-letter queue behavior;
- key, header, timestamp, and JSON value operations;
- keyed/default/field partitioning;
- global native Kafka compression;
- local and optional Redis cache backends;
- validated windowed aggregation;
- legacy-batch and bounded partition-ordered scheduling;
- acknowledged and bounded queued producer delivery.

Known boundaries:

- exactly-once Kafka transactions are not implemented;
- payload processing uses JSON values;
- Avro/Schema Registry and runtime configuration reload are not implemented;
- raw/generic typed-envelope work requires separate measured approval;
- queued delivery remains restricted to compatible auto-commit, retry, and DLQ
  settings.

## Stateless WebAssembly UDFs

Implemented:

- optional Wasmtime component runtime for filter, value-transform, and
  mutable-envelope worlds;
- canonical-root bounded loading, mandatory SHA-256 verification, empty host
  linker, world validation, and startup instantiation probes;
- pooling allocator with fresh stores/instances plus configured execution,
  artifact, input/output, memory, table, stack, and concurrency limits;
- native/UDF stage ordering and typed destination error policies;
- bounded UDF metrics;
- operator ConfigMap/PVC read-only artifact delivery and rollout digests;
- Rust guest SDK/examples, checked-in fixtures, correctness/security/
  concurrency tests, and Criterion coverage;
- patched Wasmtime `36.0.13`.

Known boundaries:

- guests are stateless;
- artifacts load only at startup from the configured local root;
- hot reload, network/OCI module download, WASI, and ambient host capabilities
  are not implemented;
- only the Rust guest SDK is maintained.

## Performance governance

Implemented:

- catalog entries for passthrough, transform-heavy, fan-out, and
  aggregation-heavy workloads at 1 KiB and 10 KiB;
- processing/delivery-mode matrix and unsafe-combination record;
- normalized schema-v3 record for the canonical 2026-07-25 AWS passthrough
  baseline;
- release evaluator for exact accounting, zero errors, repetitions,
  coefficient of variation, throughput, p99 latency, and peak RSS;
- explicit exception owner, reason, and removal-milestone fields;
- removal of the obsolete observability benchmark wrapper/guide and its
  unsupported fixed throughput targets;
- removal of six pre-v1.0 benchmark configs and validation of the three retained
  diagnostic profiles.

Repository cleanup also removed unindexed multi-document and obsolete-DSL
example suites that the current validator could not execute. Current examples
now use one supported configuration document per file.

The existing passthrough baseline lacks p99 and exact 1 KiB normalization and
cannot pass the new publication gate without an explicit exception. The Kafka
runner does not yet execute every catalog profile. Historical evidence remains
under `docs/benchmarks/results/` and must not be represented as current
capacity guidance.

## Current verification

Passed on 2026-07-26:

- `cargo fmt --check`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- `cargo test --all --all-features`;
- root and operator unused-dependency analysis;
- `cargo audit` with no vulnerability ignore;
- operator formatting, Clippy, tests, and release container smoke;
- UI unit tests, ESLint, TypeScript, Knip, and production build;
- production UI dependency audit;
- Helm lint and representative template rendering;
- workflow YAML, dashboard JSON, and benchmark workload/catalog parsing;
- performance release-evaluator unit tests;
- all retained pipeline CRD and engine configuration examples through
  `streamforge-validate`;
- the source-built Podman-backed Minikube smoke, including Kafka 3.9,
  replication, `Ready`, observability, security, bounded reconciliation,
  owner garbage collection, and complete isolated-resource cleanup;
- offline Cargo package creation;
- local Markdown link validation;
- `git diff --check`.

Known non-production exception:

- the full UI development dependency audit reports nine high-severity findings
  in the Next 15 ESLint/minimatch 3.x toolchain. They are not present in the
  production dependency audit. Owner: `track/ui`; removal milestone: `v1.4`.

Pending verification:

- live Kafka failure/recovery, Prometheus, UI, and browser release gates;
- published multi-architecture artifacts;
- dedicated v1.4 workload matrix, p99 evidence, and capacity guidance.

**Last updated:** 2026-07-26
