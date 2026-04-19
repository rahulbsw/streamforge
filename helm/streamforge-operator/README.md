# Streamforge Operator Helm Chart

Kubernetes Operator for managing Streamforge pipelines using Custom Resource Definitions (CRDs).

## Architecture

The Streamforge Operator follows the Kubernetes Operator pattern:

1. **CRD (Custom Resource Definition)**: Defines `StreamforgePipeline` resource
2. **Operator**: Watches CRD changes and reconciles state
3. **Dynamic Pipelines**: Each pipeline gets its own Deployment + ConfigMap
4. **Independent Lifecycle**: Adding/updating/deleting pipelines doesn't affect others

```
┌─────────────────────────────────────────────────────────────┐
│                    Kubernetes Cluster                        │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │             Streamforge Operator                      │   │
│  │  - Watches StreamforgePipeline CRDs                  │   │
│  │  - Reconciles desired vs actual state                │   │
│  │  - Creates/Updates/Deletes pipeline resources        │   │
│  └──────────────────────────────────────────────────────┘   │
│                          │                                    │
│                          │ manages                            │
│                          ▼                                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │  Pipeline 1  │  │  Pipeline 2  │  │  Pipeline 3  │         │
│  │              │  │              │  │              │         │
│  │ Deployment   │  │ Deployment   │  │ Deployment   │         │
│  │ ConfigMap    │  │ ConfigMap    │  │ ConfigMap    │         │
│  │ (independent)│  │ (independent)│  │ (independent)│         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

## Features

✅ **Dynamic Pipeline Management**: Add/update/delete pipelines without affecting others
✅ **Declarative Configuration**: Define pipelines as Kubernetes resources
✅ **Auto-scaling**: Scales with Kubernetes HPA
✅ **Self-healing**: Operator reconciles on failures
✅ **ConfigMap Management**: Automatic config generation and mounting
✅ **Secret Integration**: Secure credential management
✅ **Resource Limits**: Per-pipeline resource controls
✅ **Multi-destination**: Route to multiple Kafka clusters
✅ **Security**: Full SSL/TLS and SASL support

## Installation

### Prerequisites

- Kubernetes 1.19+
- Helm 3.0+
- kubectl configured

### Install CRDs and Operator

```bash
# Add Helm repository (when published)
helm repo add streamforge https://rahulbsw.github.io/streamforge
helm repo update

# Install operator with CRDs
helm install streamforge-operator streamforge/streamforge-operator \
  --namespace streamforge-system \
  --create-namespace
```

### Install from Source

```bash
# Clone repository
git clone https://github.com/rahulbsw/streamforge
cd streamforge/helm/streamforge-operator

# Install
helm install streamforge-operator . \
  --namespace streamforge-system \
  --create-namespace
```

## Quick Start

### 1. Create a Simple Pipeline

```bash
kubectl apply -f - <<EOF
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: my-first-pipeline
spec:
  appid: my-first-pipeline
  source:
    brokers: "kafka:9092"
    topic: "source-topic"
    groupId: "streamforge"
  destinations:
    - brokers: "kafka:9092"
      topic: "target-topic"
  replicas: 2
EOF
```

### 2. Check Pipeline Status

```bash
# List pipelines
kubectl get streamforgepipeline
# or short form
kubectl get sfp

# Get details
kubectl describe sfp my-first-pipeline

# Check pods
kubectl get pods -l streamforge.io/pipeline=my-first-pipeline
```

### 3. View Logs

```bash
# Get logs from all pipeline pods
kubectl logs -l streamforge.io/pipeline=my-first-pipeline -f

# Or specific pod
kubectl logs my-first-pipeline-5f7b9c8d4-xk2m9 -f
```

### 4. Scale Pipeline

```bash
# Update replicas
kubectl patch sfp my-first-pipeline -p '{"spec":{"replicas":4}}' --type=merge

# Or edit directly
kubectl edit sfp my-first-pipeline
```

### 5. Delete Pipeline

```bash
kubectl delete sfp my-first-pipeline
```

## Configuration

### Operator Values

```yaml
operator:
  image:
    repository: ghcr.io/rahulbsw/streamforge-operator
    tag: "0.1.0"
  replicas: 1
  resources:
    requests:
      cpu: 100m
      memory: 128Mi
    limits:
      cpu: 500m
      memory: 256Mi

defaults:
  image:
    repository: ghcr.io/rahulbsw/streamforge
    tag: "0.3.0"
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
```

### Custom Values

```bash
helm install streamforge-operator . \
  --namespace streamforge-system \
  --set operator.replicas=2 \
  --set defaults.image.tag=0.3.1 \
  --set monitoring.enabled=true \
  --set ui.enabled=true
```

### UI Configuration

Enable the web UI for managing pipelines:

```yaml
ui:
  enabled: true  # Enable UI deployment
  
  image:
    repository: ghcr.io/rahulbsw/streamforge-ui
    tag: "latest"
  
  replicas: 1
  
  service:
    type: NodePort  # or LoadBalancer, ClusterIP
    port: 3001
    nodePort: 30001
  
  # JWT secret for authentication (change in production!)
  jwtSecret: "your-secure-random-secret-here"
  
  # Ingress configuration
  ingress:
    enabled: false
    className: nginx
    hosts:
      - host: streamforge.example.com
        paths:
          - path: /
            pathType: Prefix
```

**Install with UI:**
```bash
helm install streamforge-operator . \
  --namespace streamforge-system \
  --create-namespace \
  --set ui.enabled=true
```

**Access UI:**
```bash
# Minikube
minikube service streamforge-operator-ui -n streamforge-system

# Port-forward
kubectl port-forward -n streamforge-system svc/streamforge-operator-ui 3001:3001
```

**Default credentials:**
- Username: `admin`
- Password: `admin`

⚠️ **Change in production!**

## Pipeline Examples

See [examples/k8s/pipelines/](../../examples/k8s/pipelines/) for complete examples:

- **01-simple-mirror.yaml**: Basic topic-to-topic mirroring
- **02-filtered-routing.yaml**: Multi-destination with filters
- **03-secure-transform.yaml**: SSL/SASL with transformations

### Apply Examples

```bash
kubectl apply -f examples/k8s/pipelines/
```

## Pipeline Specification

### Full CRD Spec

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: pipeline-name
spec:
  appid: unique-app-id

  # Source configuration
  source:
    brokers: "broker1:9092,broker2:9092"
    topic: "source-topic"
    groupId: "consumer-group"
    offset: "latest"  # or "earliest"
    security:
      protocol: "SASL_SSL"  # PLAINTEXT, SSL, SASL_PLAINTEXT, SASL_SSL
      ssl:
        caLocation: "/path/to/ca.crt"
        certificateLocation: "/path/to/client.crt"
        keyLocation: "/path/to/client.key"
      sasl:
        mechanism: "SCRAM-SHA-256"
        username: "user"
        password: "pass"

  # Destinations (multiple allowed)
  destinations:
    - brokers: "target:9092"
      topic: "target-topic"
      filter: "/field,==,value"  # Optional
      transform: "EXTRACT:/path,field"  # Optional
      partitioner: "field"  # default, random, hash, field
      partitionerField: "/userId"  # Required if partitioner=field
      compression: "snappy"  # none, gzip, snappy, lz4, zstd
      security:
        protocol: "SSL"
        ssl:
          caLocation: "/path/to/ca.crt"

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
  logLevel: "info"  # trace, debug, info, warn, error

  # Image override (optional)
  image:
    repository: ghcr.io/rahulbsw/streamforge
    tag: "0.3.0"
    pullPolicy: IfNotPresent

  # Pod scheduling (optional)
  serviceAccount: streamforge-pipeline
  nodeSelector:
    disktype: ssd
  tolerations:
    - key: "key1"
      operator: "Equal"
      value: "value1"
      effect: "NoSchedule"
  affinity:
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
              - key: topology.kubernetes.io/zone
                operator: In
                values:
                  - us-west-1a
```

## Filter DSL Syntax

```yaml
# Simple comparison
filter: "/status,==,active"

# Boolean logic
filter: "AND:/amount,>,100:/country,==,US"
filter: "OR:/priority,==,high:/priority,==,urgent"
filter: "NOT:/status,==,inactive"

# Regular expressions
filter: "REGEX:/email,^[a-z0-9._%+-]+@[a-z0-9.-]+\\.[a-z]{2,}$"

# Array operations
filter: "ARRAY_ALL:/orders,/status,==,completed"
filter: "ARRAY_ANY:/tags,/name,==,important"
```

## Transform DSL Syntax

```yaml
# Extract fields
transform: "EXTRACT:/user/email,userEmail"

# Construct object
transform: "CONSTRUCT:output,/id:userId,/name:userName,/email:userEmail"

# Array map
transform: "ARRAY_MAP:/items,/price,itemPrices"

# Arithmetic
transform: "ADD:/price,/tax,totalPrice"
transform: "MUL:/quantity,/price,totalCost"
```

## Monitoring

### Prometheus Metrics

Enable ServiceMonitor for Prometheus Operator:

```yaml
monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 30s
```

### Grafana Dashboard

```bash
helm install streamforge-operator . \
  --set monitoring.grafanaDashboard.enabled=true \
  --set monitoring.grafanaDashboard.namespace=monitoring
```

## Troubleshooting

### Check Operator Logs

```bash
kubectl logs -n streamforge-system deployment/streamforge-operator -f
```

### Check Pipeline Status

```bash
kubectl describe sfp pipeline-name
```

### Common Issues

**Pipeline not starting:**
```bash
# Check events
kubectl get events --sort-by='.lastTimestamp' | grep pipeline-name

# Check operator logs
kubectl logs -n streamforge-system -l app.kubernetes.io/name=streamforge-operator
```

**Connection errors:**
- Verify Kafka broker addresses
- Check security credentials in secrets
- Verify network policies allow pod-to-Kafka communication

**High memory usage:**
- Reduce `threads` value
- Lower `resources.limits.memory`
- Check for large messages

## Upgrading

### Upgrade Operator

```bash
helm upgrade streamforge-operator . \
  --namespace streamforge-system \
  --reuse-values
```

### Upgrade Pipeline Images

```bash
# Update all pipelines to new image
kubectl get sfp -o name | xargs -I {} kubectl patch {} \
  -p '{"spec":{"image":{"tag":"0.3.1"}}}' --type=merge
```

## Uninstallation

```bash
# Delete all pipelines first
kubectl delete sfp --all

# Uninstall operator
helm uninstall streamforge-operator -n streamforge-system

# Delete CRDs (if desired)
kubectl delete crd streamforgepipelines.streamforge.io
```

## Development

### Build Operator

```bash
cd operator
cargo build --release
docker build -t streamforge-operator:dev .
```

### Testing

```bash
# Install in test cluster
kind create cluster --name streamforge-test
helm install streamforge-operator . --namespace streamforge-system --create-namespace

# Apply test pipeline
kubectl apply -f examples/k8s/pipelines/01-simple-mirror.yaml

# Cleanup
kind delete cluster --name streamforge-test
```

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for development guidelines.

## License

Apache License 2.0 - see [LICENSE](../../LICENSE)

## Links

- **GitHub**: https://github.com/rahulbsw/streamforge
- **crates.io**: https://crates.io/crates/streamforge
- **Documentation**: http://github.rahuljain.info/streamforge/
- **Issues**: https://github.com/rahulbsw/streamforge/issues
