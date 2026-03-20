# Streamforge Kubernetes Operator

Kubernetes operator for managing Streamforge pipelines using the StreamforgePipeline Custom Resource Definition (CRD).

## Overview

The Streamforge Operator watches for StreamforgePipeline custom resources and automatically creates and manages the underlying Kubernetes resources (Deployments, ConfigMaps, Services) needed to run Kafka streaming pipelines.

## Features

- 🎯 **Declarative Pipelines** - Define pipelines as Kubernetes resources
- 🔄 **Automatic Reconciliation** - Continuously ensures desired state matches actual state
- 📊 **Status Reporting** - Reports pipeline health, phase, and replica count
- ⚙️ **Dynamic Configuration** - Generates ConfigMaps from StreamforgePipeline specs
- 🚀 **Auto-scaling** - Supports horizontal scaling via replica count
- 🔧 **Flexible Defaults** - Cluster-wide defaults configurable via Helm values

## Architecture

```
┌─────────────────────────────────────────────┐
│         StreamforgePipeline CRD              │
│  (User defines desired pipeline state)       │
└──────────────────┬──────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────┐
│      Streamforge Operator (Controller)       │
│   - Watches StreamforgePipeline resources    │
│   - Reconciles to desired state              │
│   - Updates status                           │
└──────────────────┬──────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────┐
│         Kubernetes Resources                 │
│  ├─ Deployment (Streamforge pods)           │
│  ├─ ConfigMap (Pipeline configuration)      │
│  └─ Service (Optional metrics endpoint)     │
└─────────────────────────────────────────────┘
```

## Prerequisites

- Kubernetes cluster 1.24+
- kubectl configured with cluster access
- Helm 3.8+

## Installation

### Using Helm

```bash
# Install from local chart
helm install streamforge ../helm/streamforge-operator \
  --namespace streamforge-system \
  --create-namespace

# Or install with custom values
helm install streamforge ../helm/streamforge-operator \
  --namespace streamforge-system \
  --create-namespace \
  --values custom-values.yaml
```

### Verifying Installation

```bash
# Check operator is running
kubectl get pods -n streamforge-system

# Check CRD is installed
kubectl get crd streamforgepipelines.streamforge.io

# Check operator logs
kubectl logs -n streamforge-system deployment/streamforge-operator
```

## Development

### Prerequisites

- Rust 1.75+
- Docker
- kind or minikube (for local testing)

### Building

```bash
# Build the operator binary
cargo build --release

# Build Docker image
docker build -f operator/Dockerfile -t streamforge-operator:dev .
```

### Running Locally

```bash
# Install CRD
kubectl apply -f helm/streamforge-operator/crds/streamforge.io_streamforgepipelines.yaml

# Run operator outside cluster (uses local kubeconfig)
RUST_LOG=info cargo run
```

### Running in Cluster

```bash
# Build and load image into kind
docker build -f operator/Dockerfile -t streamforge-operator:dev .
kind load docker-image streamforge-operator:dev

# Install with dev image
helm install streamforge ../helm/streamforge-operator \
  --set operator.image.repository=streamforge-operator \
  --set operator.image.tag=dev \
  --set operator.image.pullPolicy=Never \
  --namespace streamforge-system \
  --create-namespace
```

## Configuration

### Operator Configuration

Configured via Helm values (`helm/streamforge-operator/values.yaml`):

```yaml
operator:
  replicas: 1
  image:
    repository: ghcr.io/rahulbsw/streamforge-operator
    tag: latest

  # Resource limits for operator pod
  resources:
    limits:
      cpu: 500m
      memory: 256Mi
    requests:
      cpu: 100m
      memory: 128Mi
```

### Pipeline Defaults

Default settings for all pipelines (unless overridden in individual pipeline specs):

```yaml
defaults:
  image:
    repository: ghcr.io/rahulbsw/streamforge
    tag: 0.3.0

  resources:
    limits:
      cpu: 1000m
      memory: 512Mi
    requests:
      cpu: 100m
      memory: 128Mi

  serviceAccount:
    create: true
    name: streamforge-pipeline
```

## Creating Pipelines

### Example: Simple Mirror Pipeline

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: my-pipeline
  namespace: streamforge-system
spec:
  # Source Kafka cluster
  source:
    brokers: "kafka.kafka.svc.cluster.local:9092"
    topic: "input-topic"
    offset: "latest"
    groupId: "my-pipeline-group"

  # Destination Kafka cluster(s)
  destinations:
    - brokers: "kafka.kafka.svc.cluster.local:9092"
      topic: "output-topic"

  # Scaling configuration
  replicas: 2
  threads: 4

  # Application identifier
  appid: "my-pipeline"
```

### Apply Pipeline

```bash
kubectl apply -f my-pipeline.yaml

# Check pipeline status
kubectl get streamforgepipeline my-pipeline -n streamforge-system

# Watch pipeline pods come up
kubectl get pods -n streamforge-system -l streamforge.io/pipeline=my-pipeline -w
```

## Operator Behavior

### Reconciliation Loop

The operator continuously reconciles StreamforgePipeline resources:

1. **Watch** - Monitors StreamforgePipeline resources for changes
2. **Reconcile** - Compares desired state (spec) with actual state
3. **Generate Config** - Creates ConfigMap with streamforge application config
4. **Create/Update Deployment** - Ensures Deployment matches spec
5. **Update Status** - Updates pipeline status with current phase and replicas

### Status Phases

| Phase | Description |
|-------|-------------|
| `Pending` | Pipeline created, resources being provisioned |
| `Running` | All replicas running and healthy |
| `Failed` | One or more pods failed or deployment error |
| `Unknown` | Unable to determine status |

### Generated Resources

For each StreamforgePipeline, the operator creates:

#### ConfigMap
- **Name**: `{pipeline-name}-config`
- **Purpose**: Contains streamforge application configuration
- **Contents**: Generated from StreamforgePipeline spec in YAML format

Example generated config:
```yaml
appid: "my-pipeline"
bootstrap: "kafka.kafka.svc.cluster.local:9092"
target_broker: "kafka.kafka.svc.cluster.local:9092"
input: "input-topic"
output: "output-topic"
offset: "latest"
group_id: "my-pipeline-group"
threads: 4
```

#### Deployment
- **Name**: `{pipeline-name}`
- **Replicas**: From `spec.replicas`
- **Image**: From defaults or pipeline spec override
- **Labels**:
  - `app.kubernetes.io/name: streamforge`
  - `app.kubernetes.io/instance: {pipeline-name}`
  - `streamforge.io/pipeline: {pipeline-name}`
- **Volume Mounts**:
  - ConfigMap mounted at `/etc/streamforge/config.yaml`
  - Secrets mounted at `/etc/streamforge/secrets/{role}/{secret-name}/`
    - `role` = `source`, `destination-0`, `destination-1`, etc.
    - Organized by cluster to avoid conflicts
    - Read-only with 0400 permissions

## Advanced Features

### Multiple Destinations

Fan-out to multiple Kafka clusters or topics:

```yaml
spec:
  source:
    brokers: "source-kafka:9092"
    topic: "input"

  destinations:
    - brokers: "dest-kafka-1:9092"
      topic: "output-1"
    - brokers: "dest-kafka-2:9092"
      topic: "output-2"
```

### Security Configuration

#### SASL/SCRAM (Inline - Not Recommended)

```yaml
spec:
  source:
    brokers: "kafka:9092"
    topic: "input"
    security:
      protocol: "SASL_SSL"
      sasl:
        mechanism: "SCRAM-SHA-512"
        username: "user"    # ❌ Not secure - credentials in Git
        password: "pass"    # ❌ Not secure - credentials in Git
```

#### SASL with Kubernetes Secrets (Recommended)

```yaml
spec:
  source:
    brokers: "kafka:9093"
    topic: "input"
    security:
      protocol: "SASL_SSL"
      ssl:
        caSecret:
          name: kafka-ca-cert
          key: ca.crt
      sasl:
        mechanism: "SCRAM-SHA-512"
        usernameSecret:      # ✅ Secure - from K8s secret
          name: kafka-credentials
          key: username
        passwordSecret:      # ✅ Secure - from K8s secret
          name: kafka-credentials
          key: password
```

**Create the secret:**
```bash
kubectl create secret generic kafka-credentials \
  --from-literal=username=myuser \
  --from-literal=password=mypassword \
  -n streamforge-system
```

**Secrets are automatically mounted at:**
```
/etc/streamforge/secrets/source/kafka-credentials/
├── username
└── password
```

#### Kerberos with Secrets

```yaml
spec:
  source:
    brokers: "kafka:9092"
    topic: "input"
    security:
      protocol: "SASL_SSL"
      sasl:
        mechanism: "GSSAPI"
        kerberosServiceName: "kafka"
        keytabSecret:
          name: kafka-kerberos
          key: krb5.keytab
```

**Create the keytab secret:**
```bash
kubectl create secret generic kafka-kerberos \
  --from-file=krb5.keytab=/path/to/keytab \
  -n streamforge-system
```

#### Multi-Cluster with Different Credentials

When mirroring between different Kafka clusters with independent authentication:

```yaml
spec:
  source:
    brokers: "prod-kafka:9093"
    topic: "events"
    security:
      protocol: "SASL_SSL"
      ssl:
        caSecret:
          name: prod-ca
          key: ca.crt
      sasl:
        mechanism: "SCRAM-SHA-512"
        usernameSecret:
          name: prod-credentials
          key: username
        passwordSecret:
          name: prod-credentials
          key: password

  destinations:
    - brokers: "analytics-kafka:9093"
      topic: "analytics-events"
      security:
        protocol: "SASL_SSL"
        ssl:
          caSecret:
            name: analytics-ca
            key: ca.crt
        sasl:
          mechanism: "SCRAM-SHA-256"
          usernameSecret:
            name: analytics-credentials
            key: username
          passwordSecret:
            name: analytics-credentials
            key: password
```

**Secrets are mounted with role prefixes to avoid conflicts:**
```
/etc/streamforge/secrets/
├── source/
│   ├── prod-ca/
│   └── prod-credentials/
└── destination-0/
    ├── analytics-ca/
    └── analytics-credentials/
```

📖 **See [examples/pipelines/SECRETS.md](../../examples/pipelines/SECRETS.md) for complete secret management guide**

### Resource Overrides

Override default resources per pipeline:

```yaml
spec:
  resources:
    limits:
      cpu: "2000m"
      memory: "1Gi"
    requests:
      cpu: "500m"
      memory: "512Mi"
```

## Monitoring

### Operator Logs

```bash
# View operator logs
kubectl logs -n streamforge-system deployment/streamforge-operator -f

# Increase log verbosity (set via Helm)
--set operator.logLevel=debug
```

### Pipeline Status

```bash
# Get pipeline status
kubectl get streamforgepipeline -A

# Describe pipeline for details
kubectl describe streamforgepipeline my-pipeline -n streamforge-system

# Check pipeline pods
kubectl get pods -n streamforge-system -l streamforge.io/pipeline=my-pipeline
```

### Metrics

The operator exposes metrics on port 8080:
- `/metrics` - Prometheus format metrics
- `/health` - Health check endpoint

## Troubleshooting

### Pipeline Stuck in Pending

**Possible Causes:**
- Image pull failures
- Insufficient cluster resources
- ConfigMap generation error

**Debug:**
```bash
kubectl describe streamforgepipeline <name> -n streamforge-system
kubectl get events -n streamforge-system --field-selector involvedObject.name=<name>
kubectl logs -n streamforge-system deployment/streamforge-operator
```

### Pipeline Pods Crash Loop

**Possible Causes:**
- Invalid Kafka broker addresses
- Authentication failures
- Configuration errors

**Debug:**
```bash
kubectl logs -n streamforge-system <pod-name>
kubectl describe pod -n streamforge-system <pod-name>
kubectl get configmap <pipeline-name>-config -n streamforge-system -o yaml
```

### Operator Not Reconciling

**Possible Causes:**
- Operator pod not running
- RBAC permissions missing
- CRD not installed

**Debug:**
```bash
kubectl get pods -n streamforge-system
kubectl logs -n streamforge-system deployment/streamforge-operator
kubectl get crd streamforgepipelines.streamforge.io
kubectl auth can-i --list --as=system:serviceaccount:streamforge-system:streamforge-operator
```

## Development Guide

### Project Structure

```
operator/
├── src/
│   ├── main.rs           # Entry point
│   ├── crd.rs            # StreamforgePipeline CRD definition
│   ├── reconciler.rs     # Main reconciliation logic
│   └── config.rs         # Configuration generation
├── Cargo.toml            # Dependencies
└── Dockerfile            # Container image
```

### Key Dependencies

- `kube` - Kubernetes client and runtime
- `k8s-openapi` - Kubernetes API types
- `tokio` - Async runtime
- `serde` - Serialization/deserialization
- `tracing` - Structured logging

### Testing

```bash
# Run unit tests
cargo test

# Run clippy
cargo clippy --all-targets

# Check formatting
cargo fmt -- --check
```

### Adding New Features

1. Update CRD in `src/crd.rs`
2. Update reconciliation logic in `src/reconciler.rs`
3. Update Helm CRD in `helm/streamforge-operator/crds/`
4. Add tests
5. Update documentation

## CI/CD

GitHub Actions workflows:
- `.github/workflows/ci.yml` - Build, test, lint on PR/push
- `.github/workflows/release.yml` - Build and push Docker images on release

## Contributing

See [../CONTRIBUTING.md](../CONTRIBUTING.md)

## License

Apache License 2.0
