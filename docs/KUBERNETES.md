---
title: Kubernetes
nav_order: 2
parent: Deployment
---

# Run StreamForge on Kubernetes

The repository includes a `StreamforgePipeline` custom resource, a Rust
operator, and a Helm chart at `helm/streamforge-operator`. Install from a local
checkout or a reviewed internal artifact; this guide does not assume a public
chart repository or public image tag.

## Current operator scope

The operator currently:

- watches namespaced `StreamforgePipeline` resources;
- creates a `ConfigMap` and `Deployment` for each pipeline;
- reports ready replica count and a coarse phase;
- reconciles on changes and on a periodic interval;
- materializes only the first configured destination into the runtime
  configuration.

Important limitations:

- CR fields for additional destinations are not emitted to the generated
  runtime configuration.
- The CR `source.groupId` value is not used by the runtime; the generated
  `appid` remains the Kafka consumer group.
- CR security and compression fields are not currently emitted to the runtime
  configuration.
- Operator status is based on Deployment readiness, not verified Kafka
  delivery.
- The bundled chart creates cluster-scoped RBAC for the controller.

Review those limitations and the rendered RBAC before using the operator in a
shared cluster. Use a direct Kubernetes Deployment when they do not fit the
required security or routing model.

## Prerequisites

- `kubectl` access to the intended cluster
- Helm 3
- private registry images for the StreamForge runtime and operator
- Kafka reachable through private cluster egress
- pre-created topics and least-privilege Kafka ACLs
- a namespace dedicated to StreamForge workloads

## Render before install

Create a private values file with immutable image references:

```yaml
operator:
  image:
    repository: registry.internal/streamforge-operator
    tag: reviewed-release

defaults:
  image:
    repository: registry.internal/streamforge
    tag: reviewed-release

ui:
  enabled: false
```

Render and inspect the manifests:

```bash
helm template streamforge-operator ./helm/streamforge-operator \
  --namespace streamforge-system \
  --values private-values.yaml > rendered-streamforge.yaml
```

Review:

- `ClusterRole` permissions and cluster-wide watch scope;
- service accounts and image references;
- pod and container security contexts;
- resource requests and limits;
- whether any `Service`, `Ingress`, `NodePort`, or `LoadBalancer` would be
  created;
- all generated configuration for secrets or public endpoints.

The UI is disabled above. Do not enable it until its authentication, RBAC,
secret handling, and private access path have completed a production security
review.

## Install the local chart

```bash
kubectl create namespace streamforge-system

helm upgrade --install streamforge-operator ./helm/streamforge-operator \
  --namespace streamforge-system \
  --values private-values.yaml \
  --wait
```

Verify the controller:

```bash
kubectl get deployment,pods -n streamforge-system
kubectl logs -n streamforge-system \
  deployment/streamforge-operator \
  --tail=200
```

The generated Deployment name can include Helm release-name expansion. Use
`kubectl get deployment -n streamforge-system` if the exact name differs.

## Create a basic pipeline

The operator's generated runtime configuration currently supports one
destination:

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: orders-copy
  namespace: streamforge-system
spec:
  appid: orders-copy
  source:
    brokers: source-kafka.kafka.svc.cluster.local:9092
    topic: orders
    offset: earliest
  destinations:
    - brokers: destination-kafka.kafka.svc.cluster.local:9092
      topic: orders-copy
  replicas: 1
  threads: 4
  image:
    repository: registry.internal/streamforge
    tag: reviewed-release
    pullPolicy: IfNotPresent
  resources:
    requests:
      cpu: 250m
      memory: 256Mi
    limits:
      cpu: 1
      memory: 1Gi
```

Apply and inspect:

```bash
kubectl apply -f pipeline.yaml
kubectl get streamforgepipelines -n streamforge-system
kubectl describe streamforgepipeline orders-copy -n streamforge-system
kubectl get deployment,pods,configmap -n streamforge-system \
  -l streamforge.io/pipeline=orders-copy
```

The resource values demonstrate schema and Kubernetes syntax; establish
production sizing with the target workload.

## Secure Kafka connections

Because the operator does not currently emit CR security fields into the
runtime configuration, do not put Kafka usernames or passwords directly in the
CR and assume they will be applied.

For TLS/SASL pipelines, use a directly managed Deployment with a complete
StreamForge configuration supplied from a Kubernetes `Secret`, or update and
verify the operator before using it. See [Security](SECURITY_CONFIGURATION.md).

Never store a secret-bearing StreamForge configuration in a `ConfigMap`.

## Private metrics access

StreamForge listens on the configured metrics port on all pod interfaces. The
endpoint has no authentication or TLS.

If a metrics service is required, make it internal:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: streamforge-metrics
  namespace: streamforge-system
spec:
  type: ClusterIP
  selector:
    streamforge.io/pipeline: orders-copy
  ports:
    - name: metrics
      port: 9090
      targetPort: 9090
```

Add a `NetworkPolicy` that permits ingress only from the monitoring namespace
and egress only to Kafka, DNS, and other required private services. Do not use a
public `LoadBalancer`, `NodePort`, or internet-facing `Ingress` for metrics or
the UI.

For temporary local inspection:

```bash
kubectl port-forward -n streamforge-system \
  deployment/orders-copy 9090:9090
curl --fail http://127.0.0.1:9090/health
```

## Scaling

Useful consumer parallelism is bounded by source partitions. Increase replicas
only while partitions remain available for assignment and watch the rebalance.

```bash
kubectl patch streamforgepipeline orders-copy \
  --namespace streamforge-system \
  --type merge \
  --patch '{"spec":{"replicas":2}}'
```

Measure lag, broker-acknowledged deliveries, processing errors, CPU, memory, and
restart behavior after each change. More replicas do not divide a single hot
partition.

## Upgrade and rollback

Render the new chart and diff it before applying:

```bash
helm template streamforge-operator ./helm/streamforge-operator \
  --namespace streamforge-system \
  --values private-values.yaml > rendered-streamforge-next.yaml

kubectl diff --server-side -f rendered-streamforge-next.yaml
```

Then upgrade:

```bash
helm upgrade streamforge-operator ./helm/streamforge-operator \
  --namespace streamforge-system \
  --values private-values.yaml \
  --wait
```

Retain the previous image digests and values file. A Helm rollback restores
chart state, but it does not undo Kafka records, consumer offsets, topic
changes, or externally managed secrets.

## Removal

Before uninstalling, decide whether pipeline custom resources and their
consumer groups must be retained. The chart is configured to keep the CRD by
default.

```bash
helm uninstall streamforge-operator --namespace streamforge-system
```

Inventory remaining custom resources, deployments, config maps, service
accounts, cluster roles, cluster role bindings, services, secrets, and CRDs.
Deleting a namespace or custom resource is destructive and should follow an
approved data and offset-retention plan.

## Production checklist

- Images are private, immutable, scanned, and signed.
- Rendered cluster-scoped RBAC has been approved.
- UI remains disabled unless separately security-reviewed.
- No public service, ingress, listener, or security-group rule is created.
- Secret-bearing configuration is stored in a `Secret`, not a `ConfigMap`.
- Kafka TLS/SASL and ACLs are verified from the running pod.
- Metrics use `ClusterIP` plus restrictive network policy.
- Resource sizing comes from a representative workload.
- Delivery behavior is tested across a restart and rebalance.
- Rollback and teardown inventories have been rehearsed.

Continue with [Deployment](DEPLOYMENT.md), [Observability](OBSERVABILITY_QUICKSTART.md),
and [Troubleshooting](TROUBLESHOOTING.md).
