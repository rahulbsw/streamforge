---
title: Kubernetes
nav_order: 7
parent: Deployment
---

# Streamforge on Kubernetes

Complete guide for deploying and managing Streamforge pipelines on Kubernetes using the Operator pattern.

## Table of Contents

- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [Helm Chart](#helm-chart)
- [CRD & Operator](#crd--operator)
- [UI Options](#ui-options)
- [Examples](#examples)
- [Best Practices](#best-practices)

---

## Architecture

### Traditional Deployment vs Operator Pattern

**❌ Traditional Approach (Limitations):**
```
User creates Deployment + ConfigMap manually
→ Hard to manage multiple pipelines
→ No dynamic updates
→ Requires manual scaling
→ No validation
```

**✅ Operator Pattern (Recommended):**
```
┌──────────────────────────────────────────────────────┐
│                  Kubernetes Cluster                   │
│                                                        │
│  ┌─────────────────────────────────────────────┐     │
│  │         Streamforge Operator                │     │
│  │  • Watches StreamforgePipeline CRDs         │     │
│  │  • Reconciles desired vs actual state       │     │
│  │  • Creates Deployment + ConfigMap          │     │
│  │  • Updates status                           │     │
│  │  • Self-healing                             │     │
│  └─────────────────────────────────────────────┘     │
│                         │                              │
│                         │ manages                      │
│                         ▼                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐           │
│  │Pipeline 1│  │Pipeline 2│  │Pipeline 3│           │
│  │          │  │          │  │          │           │
│  │Deployment│  │Deployment│  │Deployment│           │
│  │ConfigMap │  │ConfigMap │  │ConfigMap │           │
│  └──────────┘  └──────────┘  └──────────┘           │
└──────────────────────────────────────────────────────┘
```

### Key Benefits

✅ **Dynamic Management**: Add/update/delete pipelines without affecting others
✅ **Declarative**: Define pipelines as YAML resources
✅ **Self-Healing**: Operator reconciles failures automatically
✅ **Validation**: CRD validates specs before creation
✅ **Status Tracking**: Real-time pipeline status
✅ **GitOps Ready**: Perfect for ArgoCD, Flux

---

## Quick Start

### Prerequisites

- Kubernetes 1.19+
- Helm 3.0+
- kubectl configured

### 1. Install Operator

```bash
# Add Helm repository (when published)
helm repo add streamforge https://rahulbsw.github.io/streamforge
helm repo update

# Install operator
helm install streamforge-operator streamforge/streamforge-operator \
  --namespace streamforge-system \
  --create-namespace
```

### 2. Create Your First Pipeline

```bash
kubectl apply -f - <<EOF
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: my-pipeline
spec:
  appid: my-pipeline
  source:
    brokers: "kafka:9092"
    topic: "input"
    groupId: "streamforge"
  destinations:
    - brokers: "kafka:9092"
      topic: "output"
  replicas: 2
EOF
```

### 3. Check Status

```bash
# List pipelines
kubectl get sfp

# Details
kubectl describe sfp my-pipeline

# Logs
kubectl logs -l streamforge.io/pipeline=my-pipeline -f
```

### 4. Update Pipeline (Zero Downtime)

```bash
# Add a filter
kubectl patch sfp my-pipeline --type=merge -p '
spec:
  destinations:
    - brokers: "kafka:9092"
      topic: "output"
      filter: "/status,==,active"
'
```

### 5. Scale Pipeline

```bash
# Scale to 4 replicas
kubectl patch sfp my-pipeline -p '{"spec":{"replicas":4}}' --type=merge
```

---

## Helm Chart

### Installation Options

**Basic Installation:**
```bash
helm install streamforge-operator ./helm/streamforge-operator \
  --namespace streamforge-system \
  --create-namespace
```

**Custom Values:**
```bash
helm install streamforge-operator ./helm/streamforge-operator \
  --namespace streamforge-system \
  --create-namespace \
  --set operator.replicas=2 \
  --set defaults.image.tag=0.3.1 \
  --set monitoring.enabled=true
```

**Using values file:**
```yaml
# my-values.yaml
operator:
  replicas: 2
  resources:
    limits:
      memory: 512Mi

defaults:
  image:
    tag: "0.3.1"
  resources:
    requests:
      cpu: 200m
      memory: 256Mi

monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
```

```bash
helm install streamforge-operator ./helm/streamforge-operator \
  -f my-values.yaml \
  --namespace streamforge-system \
  --create-namespace
```

### Helm Values

See [helm/streamforge-operator/values.yaml](../helm/streamforge-operator/values.yaml) for all options.

Key configurations:

```yaml
operator:
  image:
    repository: ghcr.io/rahulbsw/streamforge-operator
    tag: "0.1.0"
  replicas: 1
  resources: {}

defaults:
  image:
    repository: ghcr.io/rahulbsw/streamforge
    tag: "0.3.0"
  resources: {}
  replicas: 1
  threads: 4

monitoring:
  enabled: false
  serviceMonitor:
    enabled: false
```

---

## CRD & Operator

### StreamforgePipeline CRD

The `StreamforgePipeline` CRD defines a Kafka streaming pipeline.

**Full Spec:**

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: example-pipeline
  namespace: default
spec:
  # Application ID
  appid: example-pipeline

  # Source Kafka
  source:
    brokers: "broker1:9092,broker2:9092"
    topic: "source-topic"
    groupId: "consumer-group"
    offset: "latest"  # or "earliest"
    security:
      protocol: "SASL_SSL"
      ssl:
        caLocation: "/certs/ca.crt"
      sasl:
        mechanism: "SCRAM-SHA-256"
        username: "user"
        password: "pass"

  # Destinations (multiple allowed)
  destinations:
    - brokers: "target:9092"
      topic: "target-topic"
      filter: "/status,==,active"
      transform: "EXTRACT:/user/email,userEmail"
      partitioner: "field"
      partitionerField: "/userId"
      compression: "snappy"

  # Resources
  resources:
    requests:
      cpu: "200m"
      memory: "256Mi"
    limits:
      cpu: "1000m"
      memory: "512Mi"

  # Scaling
  replicas: 2
  threads: 4

  # Logging
  logLevel: "info"

  # Pod configuration
  image:
    repository: ghcr.io/rahulbsw/streamforge
    tag: "0.3.0"
  serviceAccount: streamforge-pipeline
  nodeSelector:
    disktype: ssd
```

### Operator Behavior

The operator watches `StreamforgePipeline` resources and:

1. **Creates/Updates Deployment**: One per pipeline
2. **Creates/Updates ConfigMap**: Contains pipeline configuration
3. **Mounts Config**: ConfigMap mounted to pods at `/etc/streamforge/config.yaml`
4. **Updates Status**: Tracks phase (Pending/Running/Failed)
5. **Self-Heals**: Reconciles every 5 minutes or on changes

**Reconciliation Loop:**

```
Watch CRD Changes
     │
     ▼
Compare Desired vs Actual State
     │
     ├─► ConfigMap exists? → Create/Update
     │
     ├─► Deployment exists? → Create/Update
     │
     ├─► Pods ready? → Update status
     │
     └─► Requeue in 5 minutes
```

### Status Fields

The operator updates pipeline status:

```yaml
status:
  phase: Running        # Pending/Running/Failed/Stopped
  replicas: 2          # Number of ready pods
  conditions:
    - type: Ready
      status: "True"
      lastTransitionTime: "2026-03-10T08:00:00Z"
  lastUpdated: "2026-03-10T08:00:00Z"
```

Check status:
```bash
kubectl get sfp my-pipeline -o jsonpath='{.status.phase}'
```

---

## UI Options

### Option 1: Kubernetes Dashboard + CRDs

**Pros:**
- Native Kubernetes UI
- No additional components
- Secure (RBAC integrated)

**Cons:**
- Generic UI (not streamforge-specific)
- Limited validation
- Basic editing

**Setup:**

```bash
# Install Kubernetes Dashboard
kubectl apply -f https://raw.githubusercontent.com/kubernetes/dashboard/v2.7.0/aio/deploy/recommended.yaml

# Create service account
kubectl create serviceaccount streamforge-dashboard -n kubernetes-dashboard
kubectl create clusterrolebinding streamforge-dashboard \
  --clusterrole=cluster-admin \
  --serviceaccount=kubernetes-dashboard:streamforge-dashboard

# Get token
kubectl create token streamforge-dashboard -n kubernetes-dashboard
```

Access: http://localhost:8001/api/v1/namespaces/kubernetes-dashboard/services/https:kubernetes-dashboard:/proxy/

### Option 2: Lens (Desktop App) ⭐ Recommended

**Pros:**
- Best developer experience
- CRD support out-of-the-box
- Multi-cluster management
- Terminal, logs, port-forwarding built-in

**Cons:**
- Desktop app (not web-based)
- Free for open source, paid for teams

**Setup:**

1. Download: https://k8slens.dev
2. Connect your cluster
3. Navigate to Custom Resources → streamforgepipelines

Lens will show all pipelines with create/edit/delete options.

### Option 3: Headlamp (Web-based)

**Pros:**
- Web-based (self-hosted)
- Open source
- CRD support
- Modern UI

**Cons:**
- Requires deployment

**Setup:**

```bash
helm repo add headlamp https://headlamp-k8s.github.io/headlamp/
helm install headlamp headlamp/headlamp \
  --namespace headlamp \
  --create-namespace

# Access
kubectl port-forward -n headlamp svc/headlamp 8080:80
```

Access: http://localhost:8080

### Option 4: Custom Streamforge UI (Future)

**Roadmap v1.2:** Web UI specifically for Streamforge

**Planned Features:**
- Pipeline builder (drag-and-drop)
- Filter/transform DSL editor with syntax highlighting
- Live metrics and monitoring
- Pipeline templates
- Kafka cluster browser
- Testing playground

**Architecture:**

```
┌────────────────────────────────────────┐
│      Streamforge UI (React/Next.js)    │
│  • Pipeline editor                      │
│  • DSL syntax highlighting              │
│  • Live monitoring                      │
│  • Template library                     │
└────────────────────────────────────────┘
                  │
                  │ API calls
                  ▼
┌────────────────────────────────────────┐
│     Kubernetes API Server               │
│  • Authentication (RBAC)                │
│  • CRD operations                       │
└────────────────────────────────────────┘
                  │
                  │ watches
                  ▼
┌────────────────────────────────────────┐
│      Streamforge Operator               │
└────────────────────────────────────────┘
```

**Would you like this?** Vote on: https://github.com/rahulbsw/streamforge/discussions/new

### Option 5: kubectl Plugin

**Quick CLI management:**

```bash
# Install kubectl-streamforge plugin (planned v1.1)
kubectl krew install streamforge

# Usage
kubectl streamforge create my-pipeline \
  --source kafka:9092/input \
  --dest kafka:9092/output \
  --replicas 2

kubectl streamforge list
kubectl streamforge logs my-pipeline
kubectl streamforge scale my-pipeline --replicas=4
```

---

## Examples

### Simple Mirror

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: simple-mirror
spec:
  source:
    brokers: "kafka:9092"
    topic: "events"
  destinations:
    - brokers: "kafka:9092"
      topic: "events-backup"
  replicas: 2
```

### Filtered Multi-Destination

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: filtered-routing
spec:
  source:
    brokers: "kafka-source:9092"
    topic: "events"
  destinations:
    # Active events
    - brokers: "kafka-target:9092"
      topic: "active-events"
      filter: "/status,==,active"
    # High priority
    - brokers: "kafka-priority:9092"
      topic: "priority"
      filter: "AND:/priority,==,high:/status,==,active"
  replicas: 3
```

### With Transformation

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: transform-pipeline
spec:
  source:
    brokers: "kafka:9092"
    topic: "raw-events"
  destinations:
    - brokers: "kafka:9092"
      topic: "processed"
      transform: "CONSTRUCT:output,/user/id:userId,/event/type:eventType"
      compression: "zstd"
  replicas: 4
  threads: 8
```

### Secure with SSL/SASL

See [examples/pipelines/03-secure-transform.yaml](../examples/pipelines/03-secure-transform.yaml)

---

## Best Practices

### 1. Resource Management

**Set resource limits:**
```yaml
spec:
  resources:
    requests:
      cpu: "200m"
      memory: "256Mi"
    limits:
      cpu: "1000m"
      memory: "512Mi"
```

**Adjust based on load:**
- Light: 100m CPU, 128Mi memory
- Medium: 500m CPU, 512Mi memory
- Heavy: 2000m CPU, 2Gi memory

### 2. Scaling Strategy

**Replicas = Kafka Partitions**

If source topic has 10 partitions:
- Set replicas: 10 (one pod per partition)
- Or replicas: 5 (two partitions per pod)

**Horizontal scaling:**
```bash
kubectl patch sfp my-pipeline -p '{"spec":{"replicas":10}}' --type=merge
```

**Vertical scaling:**
```bash
kubectl patch sfp my-pipeline --type=merge -p '
spec:
  resources:
    limits:
      memory: "2Gi"
  threads: 8
'
```

### 3. Security

**Use Secrets for credentials:**

```bash
# Create secret
kubectl create secret generic kafka-creds \
  --from-literal=username=myuser \
  --from-literal=password=mypass

# Reference in pipeline
```

```yaml
spec:
  source:
    security:
      sasl:
        mechanism: SCRAM-SHA-256
        username: myuser
        password: mypass  # TODO: Support secret references in operator
```

### 4. Monitoring

**Enable Prometheus:**
```yaml
# values.yaml
monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
```

**Key metrics:**
- `streamforge_messages_consumed_total`
- `streamforge_messages_produced_total`
- `streamforge_lag_current`
- `streamforge_filter_duration_seconds`

### 5. Naming Conventions

```
<environment>-<purpose>-<source>-<dest>

Examples:
- prod-mirror-events-backup
- staging-filter-logs-analytics
- dev-transform-users-warehouse
```

### 6. GitOps

**Store pipelines in Git:**

```
pipelines/
├── prod/
│   ├── critical-mirror.yaml
│   └── analytics-pipeline.yaml
├── staging/
│   └── test-pipeline.yaml
└── dev/
    └── dev-pipeline.yaml
```

**Deploy with ArgoCD/Flux:**
```bash
# ArgoCD
argocd app create streamforge-pipelines \
  --repo https://github.com/myorg/pipelines \
  --path pipelines/prod \
  --dest-namespace default \
  --sync-policy automated
```

### 7. Testing

**Test pipeline before production:**

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: test-pipeline
  namespace: dev
spec:
  source:
    brokers: "kafka-dev:9092"
    topic: "test-input"
  destinations:
    - brokers: "kafka-dev:9092"
      topic: "test-output"
  replicas: 1
  logLevel: "debug"
```

---

## Troubleshooting

### Pipeline Not Starting

```bash
# Check events
kubectl get events --sort-by='.lastTimestamp' | grep my-pipeline

# Check operator logs
kubectl logs -n streamforge-system -l app.kubernetes.io/name=streamforge-operator

# Check pod status
kubectl describe pod -l streamforge.io/pipeline=my-pipeline
```

### High Memory Usage

```bash
# Reduce threads
kubectl patch sfp my-pipeline -p '{"spec":{"threads":2}}' --type=merge

# Increase memory limit
kubectl patch sfp my-pipeline --type=merge -p '
spec:
  resources:
    limits:
      memory: "1Gi"
'
```

### Lag Increasing

```bash
# Scale up
kubectl patch sfp my-pipeline -p '{"spec":{"replicas":6}}' --type=merge

# Check consumer group lag
kafka-consumer-groups.sh --bootstrap-server kafka:9092 \
  --group streamforge-my-pipeline --describe
```

---

## Next Steps

1. **Install Operator**: `helm install streamforge-operator`
2. **Create First Pipeline**: Apply example YAML
3. **Choose UI**: Set up Lens or Headlamp
4. **Monitor**: Enable Prometheus metrics
5. **Scale**: Test with production load

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md)

## License

Apache License 2.0
