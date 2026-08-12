---
title: Kubernetes
nav_order: 2
parent: Deployment
---

# Run StreamForge on Kubernetes

StreamForge 1.1.0 ships a versioned OCI Helm chart, operator image, engine
image, and optional UI image. Pin one exact version across all components.

## Prerequisites

- Kubernetes 1.28 or newer
- Helm 3
- `kubectl` access to the target cluster
- Kafka topics and least-privilege ACLs created in advance
- network access from pipeline pods to Kafka

Prometheus Operator and Grafana are optional external dependencies. The chart
does not install them.

## Install the published chart

```bash
export STREAMFORGE_VERSION=1.1.0

helm upgrade --install streamforge \
  oci://ghcr.io/rahulbsw/charts/streamforge-operator \
  --version "${STREAMFORGE_VERSION}" \
  --namespace streamforge-system \
  --create-namespace \
  --wait
```

The default image tags come from `Chart.appVersion`. Override repositories when
using a reviewed internal mirror:

```yaml
operator:
  image:
    repository: registry.internal/streamforge-operator
defaults:
  image:
    repository: registry.internal/streamforge
ui:
  image:
    repository: registry.internal/streamforge-ui
```

Render and inspect local changes before installation:

```bash
helm template streamforge ./helm/streamforge-operator \
  --namespace streamforge-system \
  --values private-values.yaml > rendered-streamforge.yaml

kubectl apply --server-side --dry-run=server \
  -f rendered-streamforge.yaml
```

Review cluster-scoped RBAC, image references, pod security contexts, resource
limits, and all exposed services.

## Create a multi-destination pipeline

All destinations in one `v1alpha1` pipeline share one target Kafka broker set
and target security configuration.

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: orders
  namespace: streamforge-system
spec:
  appid: orders-replication
  source:
    brokers: source-kafka.kafka.svc.cluster.local:9092
    topic: orders
    offset: earliest
  destinations:
    - brokers: target-kafka.kafka.svc.cluster.local:9092
      topic: orders-analytics
      filter: "and($region == 'us', $amount >= 100)"
    - brokers: target-kafka.kafka.svc.cluster.local:9092
      topic: orders-redacted
      transform: "construct(order_id=$order_id, region=$region)"
  replicas: 1
  threads: 4
```

`spec.appid` is the Kafka consumer identity and defaults to `metadata.name`.
The compatibility field `spec.source.groupId` remains accepted but is ignored
by the runtime.

Validate locally and against the API server:

```bash
streamforge-validate \
  --input-format pipeline-crd \
  --output json \
  pipeline.yaml

kubectl apply --server-side --dry-run=server -f pipeline.yaml
kubectl apply -f pipeline.yaml
kubectl wait \
  --namespace streamforge-system \
  --for=condition=Ready \
  streamforgepipeline/orders \
  --timeout=120s
```

The operator derives `Ready` from the owned Deployment's observed generation
and available replicas. Unchanged status is not rewritten, and generated
Deployments and ConfigMaps carry controller owner references.

## Local Podman-backed Minikube smoke

The source-built integration harness requires rootless Podman and Minikube
`v1.38.1` or newer. Point `MINIKUBE_BIN` at a reviewed compatible binary:

```bash
MINIKUBE_BIN=/path/to/minikube \
  scripts/tests/minikube_podman_smoke.sh
```

The harness refuses existing profiles and contexts, creates an isolated
profile, verifies Kafka replication, status, observability, security,
reconciliation behavior, and owner cleanup, then removes its profile and
uniquely tagged local images. It does not replace the installed Minikube
binary.

## TLS and SASL Secret references

Do not put credential values in a custom resource. Reference Kubernetes
Secrets; the operator mounts them read-only and the shared converter emits only
file paths in the pipeline `ConfigMap`.

```yaml
spec:
  source:
    brokers: source-kafka:9093
    topic: orders
    security:
      protocol: SASL_SSL
      ssl:
        caSecret:
          name: source-kafka-ca
          key: ca.crt
      sasl:
        mechanism: SCRAM-SHA-512
        usernameSecret:
          name: source-kafka-auth
          key: username
        passwordSecret:
          name: source-kafka-auth
          key: password
  destinations:
    - brokers: target-kafka:9093
      topic: orders-copy
      security:
        protocol: SASL_SSL
        ssl:
          caSecret:
            name: target-kafka-ca
            key: ca.crt
        sasl:
          mechanism: SCRAM-SHA-512
          usernameSecret:
            name: target-kafka-auth
            key: username
          passwordSecret:
            name: target-kafka-auth
            key: password
```

For multiple destinations, repeat the identical `security` block on each
destination. Different target broker or authentication settings require
separate pipeline resources.

Inline `ssl.keyPassword`, `sasl.username`, and `sasl.password` values are
rejected for pipeline resources. Supported references include CA,
certificate, private key, private-key password, SASL username/password, and
Kerberos keytab Secret keys.

## WebAssembly UDF artifacts

Declare UDF modules once under `spec.udfs` and bind them per destination.
ConfigMap keys and PVC paths are mounted read-only under
`/var/run/streamforge/udfs`. The validator checks module names, SHA-256
digests, ABI, world compatibility, limits, and safe artifact paths before the
operator creates a workload.

See [WebAssembly UDFs](WASM_UDFS.md) for the complete schema and lifecycle.

## Optional UI

The UI is disabled by default. Production mode requires an existing Secret with
a JWT signing key and a JSON user list:

```yaml
ui:
  enabled: true
  auth:
    existingSecret: streamforge-ui-auth
    secretKey: jwt-secret
    usersKey: users.json
```

The generated credentials mode is limited to disposable local clusters:

```yaml
ui:
  enabled: true
  auth:
    developmentMode: true
```

Production mode contains no default credentials. Viewer accounts are
read-only; admin accounts can validate and mutate pipelines. Keep the UI on a
private `ClusterIP` or approved authenticated ingress.

## Optional monitoring

Enable the metrics Service and alert rules:

```yaml
monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
  grafanaDashboard:
    enabled: true
    namespace: monitoring
```

`monitoring.enabled` is the parent gate. Prometheus Operator and Grafana must
already be installed. Keep `/metrics`, `/health`, and `/ready` on a private
network and restrict access with `NetworkPolicy`.

## Operations

Inspect a pipeline:

```bash
kubectl get streamforgepipelines -n streamforge-system
kubectl describe streamforgepipeline orders -n streamforge-system
kubectl get deployment,pods,configmap -n streamforge-system \
  -l streamforge.io/pipeline=orders
```

Useful consumer parallelism is bounded by source partitions. After changing
replicas or threads, measure lag, broker-acknowledged delivery, errors, CPU,
memory, restarts, and rebalance behavior.

## Upgrade and rollback

Render and diff the next exact chart version before upgrading:

```bash
helm template streamforge \
  oci://ghcr.io/rahulbsw/charts/streamforge-operator \
  --version 1.1.0 \
  --namespace streamforge-system > rendered-streamforge-next.yaml

kubectl diff --server-side -f rendered-streamforge-next.yaml
helm upgrade streamforge \
  oci://ghcr.io/rahulbsw/charts/streamforge-operator \
  --version 1.1.0 \
  --namespace streamforge-system \
  --wait
```

Use `helm rollback streamforge REVISION --namespace streamforge-system` for
chart state. Rollback does not undo Kafka records, consumer offsets, topic
changes, or external Secret rotation.

## Removal

```bash
helm uninstall streamforge --namespace streamforge-system
```

The CRD is retained by default. Inventory remaining custom resources,
deployments, ConfigMaps, Secrets, RBAC, services, consumer groups, and topics
before deleting anything else. Deleting a `StreamforgePipeline` garbage-collects
the Deployment and ConfigMap controlled by that resource; externally managed
Secrets, Kafka topics, consumer groups, and UDF artifact sources are retained.

Continue with [Security](SECURITY_CONFIGURATION.md),
[Observability](OBSERVABILITY_QUICKSTART.md), and
[Troubleshooting](TROUBLESHOOTING.md).
