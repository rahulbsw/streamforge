# StreamForge Kubernetes Operator

The Rust operator reconciles `streamforge.io/v1alpha1`
`StreamforgePipeline` resources into one configuration `ConfigMap` and one
engine `Deployment` per pipeline.

User-facing installation, values, authentication, monitoring, and rollback
instructions live in the canonical
[Helm chart guide](../helm/streamforge-operator/README.md). Kubernetes pipeline
examples live in [the Kubernetes guide](../docs/KUBERNETES.md).

## Supported projection

The operator and `streamforge-validate` share the canonical CRD-to-engine
converter in `crates/streamforge-config-model`.

The projection supports:

- multiple destinations on one target broker set;
- filter, transform, field partitioning, retry, and DLQ settings;
- source and target TLS/SASL Secret references;
- WebAssembly UDF ConfigMap/PVC artifacts and per-destination bindings;
- Helm-provided defaults for image, pull policy, service account, resources,
  replicas, threads, and log level.

`spec.appid` is the engine and Kafka consumer identity, defaulting to
`metadata.name`. `spec.source.groupId` remains accepted only for v1alpha1
compatibility and is ignored.

All destinations share one target Kafka client. Their broker and security
settings must be identical. Use separate pipeline resources for different
target clusters or credentials.

## Security behavior

Pipeline resources reject inline `ssl.keyPassword`, `sasl.username`, and
`sasl.password` values. Secret references are mounted read-only below:

- `/etc/streamforge/secrets/source`
- `/etc/streamforge/secrets/destination-0`

The generated ConfigMap contains only mounted file paths. Engine pods run as
UID/GID `65532`, drop all capabilities, use a read-only root filesystem, and
disable service-account token automounting by default.

## Local development

From the repository root:

```bash
cargo fmt --manifest-path operator/Cargo.toml -- --check
cargo clippy \
  --manifest-path operator/Cargo.toml \
  --locked \
  --all-targets \
  --all-features \
  -- -D warnings
cargo test \
  --manifest-path operator/Cargo.toml \
  --locked \
  --all-features
cargo machete operator
```

Build the image from the repository root because the operator depends on the
shared converter:

```bash
docker build \
  --file operator/Dockerfile \
  --tag streamforge-operator:1.1.0 \
  .
```

## Reconciliation

For each valid resource, the controller:

1. validates the complete CRD with the shared converter;
2. renders deterministic engine YAML;
3. creates or patches the pipeline ConfigMap;
4. creates or patches the hardened Deployment;
5. records configuration and UDF digests in pod-template annotations;
6. reports deployment readiness and conditions in resource status.

Invalid specifications fail before workload creation with a field-specific
diagnostic. Operator status is based on Kubernetes Deployment state; Kafka
delivery remains authoritative in StreamForge metrics and logs.

## Generated CRD

`helm/streamforge-operator/crds/streamforgepipeline.yaml` is the checked-in
release schema. When the typed CRD changes, regenerate and review the schema,
then run Helm lint and representative template rendering before release.

## Production boundary

The operator is the Kubernetes control plane. It does not add a standalone UI
daemon, manage Kafka topics or ACLs, install Prometheus/Grafana, or verify
end-to-end record delivery. Those remain explicit platform responsibilities.
