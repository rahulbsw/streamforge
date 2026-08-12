# StreamForge Operator Helm Chart

This chart installs the StreamForge Kubernetes operator, its
`streamforge.io/v1alpha1` custom resource, RBAC, optional control UI, and
optional Prometheus/Grafana assets.

Chart and component versions are released together. Pin an exact version in
production; moving tags are not supported release artifacts.

## Prerequisites

- Kubernetes 1.28 or newer
- Helm 3
- access to the versioned engine, operator, and optional UI images
- Kafka-compatible source and destination brokers

Prometheus Operator and Grafana are optional external dependencies. The chart
does not install them.

## Install the published OCI chart

```bash
export STREAMFORGE_VERSION=1.1.0

helm upgrade --install streamforge \
  oci://ghcr.io/rahulbsw/charts/streamforge-operator \
  --version "${STREAMFORGE_VERSION}" \
  --namespace streamforge-system \
  --create-namespace \
  --wait
```

The default engine, operator, and UI tags resolve from `Chart.appVersion`.
Override repositories for mirrors, private registries, or local kind/minikube
images:

```bash
helm upgrade --install streamforge ./helm/streamforge-operator \
  --namespace streamforge-system \
  --create-namespace \
  --set operator.image.repository=registry.example.com/streamforge-operator \
  --set defaults.image.repository=registry.example.com/streamforge \
  --set ui.image.repository=registry.example.com/streamforge-ui
```

## Pipeline defaults

The chart passes the following defaults to the operator. They apply only when a
`StreamforgePipeline` omits the corresponding field:

```yaml
defaults:
  image:
    repository: ghcr.io/rahulbsw/streamforge
    tag: "" # Chart.appVersion
    pullPolicy: IfNotPresent
  resources:
    requests:
      cpu: 100m
      memory: 128Mi
    limits:
      cpu: 1000m
      memory: 512Mi
  replicas: 1
  threads: 4
  logLevel: info
  serviceAccount:
    create: true
    name: streamforge-pipeline
```

An explicit CR value always wins. Objects created by older CRD revisions may
already contain API-server defaults; remove or update those stored fields if
you intend to adopt new Helm defaults.

Each pipeline becomes one configuration `ConfigMap` and one `Deployment`.
Kafka credentials remain in referenced Kubernetes `Secret` objects and are
mounted read-only. Engine containers run as UID/GID `65532`, drop Linux
capabilities, use a read-only root filesystem, and do not mount a service
account token by default.

## Create a pipeline

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: orders
  namespace: streamforge-system
spec:
  appid: orders-replication
  source:
    brokers: source-kafka:9092
    topic: orders
    offset: latest
  destinations:
    - brokers: target-kafka:9092
      topic: orders-analytics
      filter: "and($region == 'us', $amount >= 100)"
    - brokers: target-kafka:9092
      topic: orders-redacted
      transform: "construct(order_id=$order_id, region=$region)"
  replicas: 1
  threads: 4
```

All destinations in one v1alpha1 resource currently use one target broker set.
`spec.appid` controls the Kafka consumer identity and defaults to
`metadata.name`. The compatibility field `spec.source.groupId` remains accepted
but is not used by the engine.

Validate and apply:

```bash
streamforge-validate \
  --input-format pipeline-crd \
  --output json \
  pipeline.yaml

kubectl apply --server-side --dry-run=server -f pipeline.yaml
kubectl apply -f pipeline.yaml
kubectl wait \
  --namespace streamforge-system \
  --for=condition=Available \
  deployment/orders \
  --timeout=120s
```

See [`../../docs/KUBERNETES.md`](../../docs/KUBERNETES.md) for TLS/SASL,
WebAssembly UDF artifact mounts, operations, and troubleshooting.

## Production UI authentication

The UI is disabled by default. A production deployment requires an existing
Secret containing:

- `jwt-secret`: a JWT signing key of at least 32 characters;
- `users.json`: a JSON array with `username`, bcrypt `passwordHash`, and
  `admin` or `viewer` role.

Create the values in an approved secret store, materialize temporary local
files with restrictive permissions, and create the Kubernetes Secret without
checking either value into Git:

```bash
kubectl create secret generic streamforge-ui-auth \
  --namespace streamforge-system \
  --from-file=jwt-secret=/secure/path/jwt-secret \
  --from-file=users.json=/secure/path/users.json

helm upgrade --install streamforge ./helm/streamforge-operator \
  --namespace streamforge-system \
  --set ui.enabled=true \
  --set ui.auth.existingSecret=streamforge-ui-auth
```

Custom key names are supported:

```yaml
ui:
  auth:
    existingSecret: streamforge-ui-auth
    secretKey: jwt-secret
    usersKey: users.json
```

The chart does not contain production usernames, passwords, or signing keys.

### Explicit development mode

For a disposable local cluster only:

```bash
helm upgrade --install streamforge ./helm/streamforge-operator \
  --namespace streamforge-system \
  --create-namespace \
  --set ui.enabled=true \
  --set ui.auth.developmentMode=true
```

This mode creates and retains random JWT, admin-password, and viewer-password
values. It sets `NODE_ENV=development` and enables only the two demo roles.
Do not combine it with `ui.auth.existingSecret`, and do not use it in
production.

## Monitoring

Enable the metrics Service and PrometheusRule:

```bash
helm upgrade --install streamforge ./helm/streamforge-operator \
  --namespace streamforge-system \
  --set monitoring.enabled=true
```

Enable optional integrations when their controllers are installed:

```yaml
monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 30s
    scrapeTimeout: 10s
    labels:
      release: kube-prometheus-stack
  prometheusRule:
    labels:
      release: kube-prometheus-stack
  grafanaDashboard:
    enabled: true
    namespace: monitoring
```

`monitoring.enabled` is the parent gate. The ServiceMonitor and Grafana
dashboard require both the parent and their own `enabled` value.

See [`../../docs/OBSERVABILITY_QUICKSTART.md`](../../docs/OBSERVABILITY_QUICKSTART.md)
for the metric catalog and signal-path release checks.

## Verify, upgrade, and roll back

```bash
helm lint helm/streamforge-operator
helm template streamforge helm/streamforge-operator
kubectl get streamforgepipelines,deployments,pods \
  --namespace streamforge-system

helm history streamforge --namespace streamforge-system
helm rollback streamforge <revision> --namespace streamforge-system --wait
```

Roll back the chart, operator, engine, and UI versions together. Preserve
production authentication Secrets and pipeline CRs during rollback.

## Uninstall

```bash
helm uninstall streamforge --namespace streamforge-system
kubectl delete namespace streamforge-system
```

Helm installs files under `crds/` before templates and does not automatically
delete CRDs. After confirming that no pipeline CRs or rollback requirements
remain:

```bash
kubectl delete crd streamforgepipelines.streamforge.io
```

Deleting the CRD deletes the corresponding custom resources. Treat that as a
separate destructive operation.

## Release checks

Before publishing the chart:

```bash
helm lint helm/streamforge-operator
helm template streamforge helm/streamforge-operator
helm template streamforge helm/streamforge-operator \
  --set ui.enabled=true \
  --set ui.auth.existingSecret=streamforge-ui-auth
helm template streamforge helm/streamforge-operator \
  --set ui.enabled=true \
  --set ui.auth.developmentMode=true
helm template streamforge helm/streamforge-operator \
  --set monitoring.enabled=true \
  --set monitoring.serviceMonitor.enabled=true
```

The release workflow also verifies matching component versions, builds every
image before publication, loads the published operator image into kind, and
checks rollout and cleanup.
