# StreamForge Deployment Guide

**Version:** 1.0.0  
**Last Updated:** 2026-04-18

This guide covers deploying StreamForge in production environments using Docker, Kubernetes, Helm, and the Kubernetes Operator.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Docker Deployment](#docker-deployment)
3. [Kubernetes Deployment](#kubernetes-deployment)
4. [Helm Chart Deployment](#helm-chart-deployment)
5. [Operator Deployment](#operator-deployment)
6. [Multi-Cluster Setup](#multi-cluster-setup)
7. [Production Best Practices](#production-best-practices)
8. [Security Hardening](#security-hardening)
9. [Monitoring and Observability](#monitoring-and-observability)
10. [Configuration Management](#configuration-management)

---

## Prerequisites

### Required
- Kafka cluster (2.8+) with accessible bootstrap servers
- Docker (20.10+) or Kubernetes (1.21+)
- Network connectivity between StreamForge and Kafka brokers
- TLS certificates (if using SSL/SASL)

### Recommended
- Prometheus for metrics collection
- Grafana for dashboards
- Persistent volume for DLQ messages
- Redis for distributed caching (optional)

### Resource Requirements

**Minimum (Development):**
- CPU: 1 core
- Memory: 512 MB
- Disk: 1 GB

**Production (per pipeline):**
- CPU: 2-4 cores
- Memory: 2-4 GB
- Disk: 10 GB (for logs, DLQ)
- Network: 1 Gbps

**Scaling:**
- 1 core per ~20K msg/s throughput
- 1 GB memory per 100K msg/s for JSON processing
- Increase threads parameter for CPU-bound workloads

---

## Docker Deployment

### 1. Build Docker Image

**Dockerfile:**
```dockerfile
FROM rust:1.75-slim as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY benches ./benches

# Build release binary
RUN cargo build --release --bin streamforge

# Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsasl2-2 \
    libzstd1 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary
COPY --from=builder /app/target/release/streamforge /usr/local/bin/

# Create non-root user
RUN useradd -m -u 1000 streamforge
USER streamforge

WORKDIR /app

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/streamforge"]
CMD ["--config", "/app/config.yaml"]
```

**Build:**
```bash
docker build -t streamforge:1.0.0 .
docker tag streamforge:1.0.0 streamforge:latest
```

### 2. Run with Docker

**Simple run:**
```bash
docker run -d \
  --name streamforge \
  -v $(pwd)/config.yaml:/app/config.yaml:ro \
  -p 8080:8080 \
  streamforge:1.0.0
```

**With environment variables:**
```bash
docker run -d \
  --name streamforge \
  -e KAFKA_BOOTSTRAP=kafka.example.com:9092 \
  -e RUST_LOG=info \
  -e RUST_BACKTRACE=1 \
  -v $(pwd)/config.yaml:/app/config.yaml:ro \
  -v $(pwd)/certs:/app/certs:ro \
  -p 8080:8080 \
  streamforge:1.0.0
```

### 3. Docker Compose

**docker-compose.yml:**
```yaml
version: '3.8'

services:
  streamforge:
    image: streamforge:1.0.0
    container_name: streamforge
    restart: unless-stopped
    ports:
      - "8080:8080"
    volumes:
      - ./config.yaml:/app/config.yaml:ro
      - ./certs:/app/certs:ro
      - streamforge-data:/app/data
    environment:
      RUST_LOG: info
      RUST_BACKTRACE: 1
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s
    networks:
      - kafka-network
    depends_on:
      - kafka

  # Optional: Local Kafka for testing
  kafka:
    image: docker.redpanda.com/redpandadata/redpanda:latest
    command:
      - redpanda
      - start
      - --smp
      - '1'
      - --reserve-memory
      - 0M
      - --overprovisioned
      - --node-id
      - '0'
      - --kafka-addr
      - PLAINTEXT://0.0.0.0:29092,OUTSIDE://0.0.0.0:9092
      - --advertise-kafka-addr
      - PLAINTEXT://kafka:29092,OUTSIDE://localhost:9092
    ports:
      - "9092:9092"
      - "29092:29092"
    networks:
      - kafka-network

  # Optional: Prometheus
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
    networks:
      - kafka-network

  # Optional: Grafana
  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    volumes:
      - grafana-data:/var/lib/grafana
      - ./grafana/dashboards:/etc/grafana/provisioning/dashboards:ro
      - ./grafana/datasources:/etc/grafana/provisioning/datasources:ro
    environment:
      GF_SECURITY_ADMIN_PASSWORD: admin
    networks:
      - kafka-network

volumes:
  streamforge-data:
  prometheus-data:
  grafana-data:

networks:
  kafka-network:
    driver: bridge
```

**prometheus.yml:**
```yaml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'streamforge'
    static_configs:
      - targets: ['streamforge:8080']
```

**Start:**
```bash
docker-compose up -d
docker-compose logs -f streamforge
```

---

## Kubernetes Deployment

### 1. Namespace

**namespace.yaml:**
```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: streamforge
  labels:
    name: streamforge
```

### 2. ConfigMap

**configmap.yaml:**
```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: streamforge-config
  namespace: streamforge
data:
  config.yaml: |
    appid: "streamforge-prod"
    bootstrap: "kafka.kafka.svc.cluster.local:9092"
    input: "source-topic"
    offset: "latest"
    threads: 4
    
    # Performance tuning
    performance:
      fetch_min_bytes: 1024
      fetch_max_wait_ms: 100
      queue_buffering_max_ms: 5
      batch_size: 1000
      linger_ms: 10
    
    # Retry and DLQ
    retry:
      max_attempts: 3
      initial_delay_ms: 100
      max_delay_ms: 30000
      jitter: true
    
    dlq:
      enabled: true
      topic: "streamforge-dlq"
      include_error_headers: true
    
    # Routing
    routing:
      routing_type: "filter"
      destinations:
        - output: "filtered-topic"
          filter: "/status,==,active"
          transform: "/data"
          key_transform: "/user/id"
          headers:
            x-pipeline: "streamforge"
    
    # Observability
    metrics:
      enabled: true
      port: 8080
      path: "/metrics"
```

### 3. Secret (for TLS/SASL)

**secret.yaml:**
```yaml
apiVersion: v1
kind: Secret
metadata:
  name: streamforge-kafka-certs
  namespace: streamforge
type: Opaque
data:
  ca.crt: <base64-encoded-ca-cert>
  client.crt: <base64-encoded-client-cert>
  client.key: <base64-encoded-client-key>
  sasl-password: <base64-encoded-password>
```

**Create from files:**
```bash
kubectl create secret generic streamforge-kafka-certs \
  --from-file=ca.crt=./certs/ca.crt \
  --from-file=client.crt=./certs/client.crt \
  --from-file=client.key=./certs/client.key \
  --from-literal=sasl-password="your-password" \
  -n streamforge
```

### 4. Deployment

**deployment.yaml:**
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: streamforge
  namespace: streamforge
  labels:
    app: streamforge
    version: "1.0.0"
spec:
  replicas: 2
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  selector:
    matchLabels:
      app: streamforge
  template:
    metadata:
      labels:
        app: streamforge
        version: "1.0.0"
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "8080"
        prometheus.io/path: "/metrics"
    spec:
      serviceAccountName: streamforge
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
      
      containers:
      - name: streamforge
        image: streamforge:1.0.0
        imagePullPolicy: IfNotPresent
        
        args:
          - "--config"
          - "/app/config.yaml"
        
        ports:
        - name: metrics
          containerPort: 8080
          protocol: TCP
        
        env:
        - name: RUST_LOG
          value: "info"
        - name: RUST_BACKTRACE
          value: "1"
        - name: POD_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: POD_NAMESPACE
          valueFrom:
            fieldRef:
              fieldPath: metadata.namespace
        
        volumeMounts:
        - name: config
          mountPath: /app/config.yaml
          subPath: config.yaml
          readOnly: true
        - name: certs
          mountPath: /app/certs
          readOnly: true
        - name: data
          mountPath: /app/data
        
        resources:
          requests:
            cpu: 1000m
            memory: 2Gi
          limits:
            cpu: 2000m
            memory: 4Gi
        
        livenessProbe:
          httpGet:
            path: /health
            port: metrics
          initialDelaySeconds: 30
          periodSeconds: 30
          timeoutSeconds: 5
          failureThreshold: 3
        
        readinessProbe:
          httpGet:
            path: /health
            port: metrics
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 2
      
      volumes:
      - name: config
        configMap:
          name: streamforge-config
      - name: certs
        secret:
          secretName: streamforge-kafka-certs
      - name: data
        emptyDir: {}
      
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchLabels:
                  app: streamforge
              topologyKey: kubernetes.io/hostname
```

### 5. Service

**service.yaml:**
```yaml
apiVersion: v1
kind: Service
metadata:
  name: streamforge
  namespace: streamforge
  labels:
    app: streamforge
spec:
  type: ClusterIP
  ports:
  - name: metrics
    port: 8080
    targetPort: metrics
    protocol: TCP
  selector:
    app: streamforge
```

### 6. ServiceAccount and RBAC

**rbac.yaml:**
```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: streamforge
  namespace: streamforge
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: streamforge
  namespace: streamforge
rules:
- apiGroups: [""]
  resources: ["configmaps", "secrets"]
  verbs: ["get", "list", "watch"]
- apiGroups: [""]
  resources: ["pods"]
  verbs: ["get", "list"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: streamforge
  namespace: streamforge
subjects:
- kind: ServiceAccount
  name: streamforge
  namespace: streamforge
roleRef:
  kind: Role
  name: streamforge
  apiGroup: rbac.authorization.k8s.io
```

### 7. HorizontalPodAutoscaler

**hpa.yaml:**
```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: streamforge
  namespace: streamforge
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: streamforge
  minReplicas: 2
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Percent
        value: 50
        periodSeconds: 60
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 25
        periodSeconds: 60
```

### 8. Deploy to Kubernetes

```bash
# Create namespace
kubectl apply -f namespace.yaml

# Create secrets and config
kubectl apply -f secret.yaml
kubectl apply -f configmap.yaml

# Create RBAC
kubectl apply -f rbac.yaml

# Deploy application
kubectl apply -f deployment.yaml
kubectl apply -f service.yaml
kubectl apply -f hpa.yaml

# Verify deployment
kubectl get pods -n streamforge
kubectl logs -f deployment/streamforge -n streamforge
kubectl get svc -n streamforge
```

---

## Helm Chart Deployment

### 1. Install Helm Chart

**Add repository:**
```bash
helm repo add streamforge https://streamforge.io/helm-charts
helm repo update
```

**Install:**
```bash
helm install streamforge streamforge/streamforge \
  --namespace streamforge \
  --create-namespace \
  --values values.yaml
```

### 2. Custom Values

**values.yaml:**
```yaml
# Image configuration
image:
  repository: streamforge
  tag: "1.0.0"
  pullPolicy: IfNotPresent

# Replica count
replicaCount: 2

# Resources
resources:
  requests:
    cpu: 1000m
    memory: 2Gi
  limits:
    cpu: 2000m
    memory: 4Gi

# Autoscaling
autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 10
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80

# Service
service:
  type: ClusterIP
  port: 8080

# Ingress (if needed)
ingress:
  enabled: false
  className: nginx
  annotations: {}
  hosts:
    - host: streamforge.example.com
      paths:
        - path: /
          pathType: Prefix
  tls: []

# StreamForge configuration
config:
  appid: "streamforge-prod"
  bootstrap: "kafka.kafka.svc.cluster.local:9092"
  input: "source-topic"
  offset: "latest"
  threads: 4
  
  performance:
    fetch_min_bytes: 1024
    fetch_max_wait_ms: 100
    queue_buffering_max_ms: 5
    batch_size: 1000
    linger_ms: 10
  
  retry:
    max_attempts: 3
    initial_delay_ms: 100
    max_delay_ms: 30000
    jitter: true
  
  dlq:
    enabled: true
    topic: "streamforge-dlq"
    include_error_headers: true
  
  routing:
    routing_type: "filter"
    destinations:
      - output: "filtered-topic"
        filter: "/status,==,active"
        transform: "/data"
        key_transform: "/user/id"
  
  metrics:
    enabled: true
    port: 8080

# Kafka TLS/SASL
kafka:
  tls:
    enabled: false
    ca: ""
    cert: ""
    key: ""
  sasl:
    enabled: false
    mechanism: "PLAIN"
    username: ""
    password: ""

# Monitoring
monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 30s

# Security
securityContext:
  runAsNonRoot: true
  runAsUser: 1000
  fsGroup: 1000

podSecurityContext:
  runAsNonRoot: true
  runAsUser: 1000

# Affinity
affinity:
  podAntiAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
    - weight: 100
      podAffinityTerm:
        labelSelector:
          matchLabels:
            app: streamforge
        topologyKey: kubernetes.io/hostname
```

### 3. Upgrade

```bash
helm upgrade streamforge streamforge/streamforge \
  --namespace streamforge \
  --values values.yaml \
  --wait
```

### 4. Rollback

```bash
helm rollback streamforge -n streamforge
```

---

## Operator Deployment

### 1. Install Operator

```bash
kubectl apply -f https://streamforge.io/operator/install.yaml
```

**Or with Helm:**
```bash
helm install streamforge-operator streamforge/operator \
  --namespace streamforge-system \
  --create-namespace
```

### 2. Create StreamforgePipeline CRD

**pipeline.yaml:**
```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: user-filtering
  namespace: streamforge
spec:
  image: streamforge:1.0.0
  replicas: 2
  
  resources:
    requests:
      cpu: 1000m
      memory: 2Gi
    limits:
      cpu: 2000m
      memory: 4Gi
  
  autoscaling:
    enabled: true
    minReplicas: 2
    maxReplicas: 10
    targetCPU: 70
    targetMemory: 80
  
  kafka:
    bootstrap: "kafka.kafka.svc.cluster.local:9092"
    tls:
      enabled: false
    sasl:
      enabled: false
  
  pipeline:
    appid: "user-filtering"
    input: "users"
    offset: "latest"
    threads: 4
    
    performance:
      fetch_min_bytes: 1024
      batch_size: 1000
      linger_ms: 10
    
    retry:
      maxAttempts: 3
      initialDelay: 100ms
      maxDelay: 30s
      jitter: true
    
    dlq:
      enabled: true
      topic: "user-filtering-dlq"
    
    routing:
      type: filter
      destinations:
        - output: "active-users"
          filter: "/status,==,active"
          transform: "/data"
          keyTransform: "/user/id"
          headers:
            x-pipeline: "user-filtering"
        
        - output: "premium-users"
          filter: "/tier,==,premium"
          transform: "/data"
  
  monitoring:
    enabled: true
    serviceMonitor: true
```

### 3. Apply Pipeline

```bash
kubectl apply -f pipeline.yaml

# Check status
kubectl get streamforgepipeline -n streamforge
kubectl describe streamforgepipeline user-filtering -n streamforge

# View generated resources
kubectl get deploy,svc,hpa -n streamforge -l pipeline=user-filtering
```

### 4. Update Pipeline

```bash
# Edit in-place
kubectl edit streamforgepipeline user-filtering -n streamforge

# Or apply updated YAML
kubectl apply -f pipeline.yaml
```

### 5. Delete Pipeline

```bash
kubectl delete streamforgepipeline user-filtering -n streamforge
```

---

## Multi-Cluster Setup

### Architecture

```
┌──────────────────┐         ┌──────────────────┐
│   Cluster A      │         │   Cluster B      │
│  (us-east-1)     │         │  (us-west-2)     │
│                  │         │                  │
│  ┌────────────┐  │         │  ┌────────────┐  │
│  │  Kafka A   │  │         │  │  Kafka B   │  │
│  │  (source)  │  │         │  │  (target)  │  │
│  └────────────┘  │         │  └────────────┘  │
│        │         │         │        │         │
│        ▼         │         │        ▼         │
│  ┌────────────┐  │  WAN    │  ┌────────────┐  │
│  │StreamForge │──┼────────►│  │  Consumer  │  │
│  │            │  │         │  │    Apps    │  │
│  └────────────┘  │         │  └────────────┘  │
│                  │         │                  │
└──────────────────┘         └──────────────────┘
```

### 1. Cross-Cluster Replication

**Scenario:** Replicate from Kafka A (us-east-1) to Kafka B (us-west-2)

**config-us-east-1.yaml:**
```yaml
appid: "cross-region-replication"
bootstrap: "kafka-a.us-east-1.internal:9092"
input: "events"
offset: "earliest"  # or "latest" for new messages only
threads: 8

# Source cluster TLS/SASL
kafka:
  security:
    protocol: "SASL_SSL"
    sasl_mechanism: "PLAIN"
    sasl_username: "${KAFKA_A_USER}"
    sasl_password: "${KAFKA_A_PASSWORD}"
  ssl:
    ca_location: "/certs/kafka-a-ca.crt"

# Performance for cross-region
performance:
  fetch_min_bytes: 10240  # 10 KB - larger batches
  fetch_max_wait_ms: 500
  batch_size: 5000
  linger_ms: 50
  compression: "zstd"  # compress for WAN

retry:
  max_attempts: 5
  initial_delay_ms: 500
  max_delay_ms: 60000
  jitter: true

dlq:
  enabled: true
  topic: "cross-region-dlq"

routing:
  routing_type: "passthrough"
  destinations:
    - output: "events"
      # Target cluster (Kafka B)
      bootstrap: "kafka-b.us-west-2.internal:9092"
      security:
        protocol: "SASL_SSL"
        sasl_mechanism: "PLAIN"
        sasl_username: "${KAFKA_B_USER}"
        sasl_password: "${KAFKA_B_PASSWORD}"
      ssl:
        ca_location: "/certs/kafka-b-ca.crt"
      
      # Optional: Filter for regional data
      filter: "/region,==,us-east"
      
      # Preserve original keys and timestamps
      partitioning: "default"
      preserve_timestamp: true
```

**Deploy:**
```bash
# Deploy in source cluster (us-east-1)
kubectl apply -f deployment-cross-region.yaml -n streamforge

# Monitor lag
kubectl exec -it deployment/streamforge -n streamforge -- \
  kafka-consumer-groups --bootstrap-server kafka-a:9092 \
  --describe --group cross-region-replication
```

### 2. Active-Passive Failover

**Primary (Active):**
```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: primary-pipeline
  namespace: streamforge
spec:
  replicas: 3
  kafka:
    bootstrap: "kafka-primary.internal:9092"
  pipeline:
    input: "orders"
    offset: "earliest"
    routing:
      destinations:
        - output: "processed-orders"
```

**Secondary (Standby):**
```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: secondary-pipeline
  namespace: streamforge
spec:
  replicas: 1  # standby mode
  kafka:
    bootstrap: "kafka-secondary.internal:9092"
  pipeline:
    input: "orders"
    offset: "latest"  # don't reprocess on failover
    routing:
      destinations:
        - output: "processed-orders"
```

**Failover process:**
```bash
# 1. Detect primary failure
kubectl get pods -n streamforge | grep primary-pipeline

# 2. Scale up secondary
kubectl scale deployment secondary-pipeline --replicas=3 -n streamforge

# 3. Update DNS/load balancer to point to secondary Kafka

# 4. Monitor lag
kubectl logs -f deployment/secondary-pipeline -n streamforge
```

### 3. Hub-and-Spoke Pattern

**Hub cluster:** Aggregates from multiple sources

```yaml
# Pipeline 1: us-east → hub
appid: "us-east-to-hub"
bootstrap: "kafka-us-east.internal:9092"
input: "regional-events"
routing:
  destinations:
    - output: "global-events"
      bootstrap: "kafka-hub.internal:9092"
      transform: "CONSTRUCT:region=us-east:data=/data"

# Pipeline 2: eu-west → hub
appid: "eu-west-to-hub"
bootstrap: "kafka-eu-west.internal:9092"
input: "regional-events"
routing:
  destinations:
    - output: "global-events"
      bootstrap: "kafka-hub.internal:9092"
      transform: "CONSTRUCT:region=eu-west:data=/data"
```

---

## Production Best Practices

### 1. High Availability

**Multiple replicas:**
```yaml
spec:
  replicas: 3  # minimum for HA
  
  affinity:
    podAntiAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
      - labelSelector:
          matchLabels:
            app: streamforge
        topologyKey: kubernetes.io/hostname
```

**Multiple availability zones:**
```yaml
spec:
  affinity:
    podAntiAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
      - labelSelector:
          matchLabels:
            app: streamforge
        topologyKey: topology.kubernetes.io/zone
```

**PodDisruptionBudget:**
```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: streamforge-pdb
  namespace: streamforge
spec:
  minAvailable: 2
  selector:
    matchLabels:
      app: streamforge
```

### 2. Resource Management

**Set requests == limits for guaranteed QoS:**
```yaml
resources:
  requests:
    cpu: 2000m
    memory: 4Gi
  limits:
    cpu: 2000m
    memory: 4Gi
```

**Use ResourceQuotas:**
```yaml
apiVersion: v1
kind: ResourceQuota
metadata:
  name: streamforge-quota
  namespace: streamforge
spec:
  hard:
    requests.cpu: "20"
    requests.memory: "40Gi"
    limits.cpu: "20"
    limits.memory: "40Gi"
    pods: "20"
```

### 3. Performance Tuning

**Consumer tuning:**
```yaml
performance:
  fetch_min_bytes: 10240      # Wait for 10 KB
  fetch_max_wait_ms: 100      # Or 100 ms
  max_partition_fetch_bytes: 1048576  # 1 MB per partition
```

**Producer tuning:**
```yaml
performance:
  batch_size: 5000            # Batch up to 5000 messages
  linger_ms: 50               # Wait 50 ms for batching
  queue_buffering_max_ms: 100
  compression: "zstd"         # Use zstd compression
```

**Threading:**
```yaml
threads: 8  # Match available CPU cores
```

### 4. Commit Strategy

**For low latency (< 100 ms):**
```yaml
commit_strategy: "per-message"
```

**For high throughput (> 50K msg/s):**
```yaml
commit_strategy: "manual"
commit_interval_ms: 5000  # Commit every 5 seconds
```

**For balanced:**
```yaml
commit_strategy: "time-based"
commit_interval_ms: 1000  # Commit every 1 second
```

### 5. DLQ Management

**Enable DLQ:**
```yaml
dlq:
  enabled: true
  topic: "streamforge-dlq"
  include_error_headers: true
  max_retries: 3
```

**Monitor DLQ:**
```bash
# Count DLQ messages
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group streamforge-dlq-monitor

# Inspect DLQ message
kafka-console-consumer --bootstrap-server kafka:9092 \
  --topic streamforge-dlq \
  --from-beginning \
  --property print.headers=true \
  --max-messages 1
```

**Reprocess DLQ:**
```yaml
# Create reprocessing pipeline
appid: "dlq-reprocessor"
input: "streamforge-dlq"
offset: "earliest"
routing:
  destinations:
    - output: "original-topic"
      # Fix the issue that caused DLQ
      filter: "/error-type,!=,permanent"
```

### 6. Graceful Shutdown

**Kubernetes termination:**
```yaml
spec:
  containers:
  - name: streamforge
    lifecycle:
      preStop:
        exec:
          command: ["/bin/sh", "-c", "sleep 15"]
  
  terminationGracePeriodSeconds: 30
```

**Signal handling:**
- StreamForge handles SIGTERM gracefully
- Stops consuming new messages
- Finishes processing in-flight messages
- Commits offsets
- Closes producers/consumers

### 7. Logging

**Structured logging:**
```yaml
env:
- name: RUST_LOG
  value: "streamforge=info,rdkafka=warn"
- name: RUST_LOG_FORMAT
  value: "json"  # for log aggregation
```

**Log aggregation:**
```yaml
# Sidecar for log shipping
- name: fluentd
  image: fluent/fluentd:latest
  volumeMounts:
  - name: logs
    mountPath: /var/log/streamforge
```

---

## Security Hardening

### 1. TLS Configuration

**Enable TLS:**
```yaml
kafka:
  security:
    protocol: "SSL"
  ssl:
    ca_location: "/certs/ca.crt"
    certificate_location: "/certs/client.crt"
    key_location: "/certs/client.key"
    key_password: "${SSL_KEY_PASSWORD}"
```

**Kubernetes secret:**
```yaml
apiVersion: v1
kind: Secret
metadata:
  name: kafka-tls
  namespace: streamforge
type: kubernetes.io/tls
data:
  ca.crt: <base64>
  tls.crt: <base64>
  tls.key: <base64>
```

### 2. SASL Authentication

**PLAIN:**
```yaml
kafka:
  security:
    protocol: "SASL_SSL"
    sasl_mechanism: "PLAIN"
    sasl_username: "${KAFKA_USER}"
    sasl_password: "${KAFKA_PASSWORD}"
```

**SCRAM-SHA-512:**
```yaml
kafka:
  security:
    protocol: "SASL_SSL"
    sasl_mechanism: "SCRAM-SHA-512"
    sasl_username: "${KAFKA_USER}"
    sasl_password: "${KAFKA_PASSWORD}"
```

**OAuth:**
```yaml
kafka:
  security:
    protocol: "SASL_SSL"
    sasl_mechanism: "OAUTHBEARER"
    sasl_oauthbearer_config: |
      client_id=${OAUTH_CLIENT_ID}
      client_secret=${OAUTH_CLIENT_SECRET}
      token_endpoint_url=${OAUTH_TOKEN_URL}
```

### 3. Secrets Management

**Use External Secrets Operator:**
```yaml
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: streamforge-kafka-credentials
  namespace: streamforge
spec:
  secretStoreRef:
    name: aws-secrets-manager
    kind: SecretStore
  target:
    name: kafka-credentials
  data:
  - secretKey: username
    remoteRef:
      key: streamforge/kafka/username
  - secretKey: password
    remoteRef:
      key: streamforge/kafka/password
```

**Or use HashiCorp Vault:**
```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: streamforge
  namespace: streamforge
  annotations:
    vault.hashicorp.com/agent-inject: "true"
    vault.hashicorp.com/role: "streamforge"
    vault.hashicorp.com/agent-inject-secret-kafka: "secret/data/streamforge/kafka"
```

### 4. Network Policies

**Restrict traffic:**
```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: streamforge-netpol
  namespace: streamforge
spec:
  podSelector:
    matchLabels:
      app: streamforge
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: monitoring
    ports:
    - protocol: TCP
      port: 8080
  egress:
  - to:
    - namespaceSelector:
        matchLabels:
          name: kafka
    ports:
    - protocol: TCP
      port: 9092
    - protocol: TCP
      port: 9093
  - to:
    - podSelector:
        matchLabels:
          k8s-app: kube-dns
    ports:
    - protocol: UDP
      port: 53
```

### 5. Pod Security

**PodSecurityPolicy (deprecated) or Pod Security Standards:**
```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: streamforge
  labels:
    pod-security.kubernetes.io/enforce: restricted
    pod-security.kubernetes.io/audit: restricted
    pod-security.kubernetes.io/warn: restricted
```

**SecurityContext:**
```yaml
securityContext:
  runAsNonRoot: true
  runAsUser: 1000
  runAsGroup: 1000
  fsGroup: 1000
  allowPrivilegeEscalation: false
  capabilities:
    drop:
    - ALL
  readOnlyRootFilesystem: true
  seccompProfile:
    type: RuntimeDefault
```

### 6. Image Security

**Use distroless or minimal images:**
```dockerfile
FROM gcr.io/distroless/cc-debian11
COPY --from=builder /app/target/release/streamforge /
USER nonroot:nonroot
ENTRYPOINT ["/streamforge"]
```

**Scan images:**
```bash
# Trivy
trivy image streamforge:1.0.0

# Grype
grype streamforge:1.0.0
```

---

## Monitoring and Observability

### 1. Prometheus Metrics

**ServiceMonitor:**
```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: streamforge
  namespace: streamforge
spec:
  selector:
    matchLabels:
      app: streamforge
  endpoints:
  - port: metrics
    interval: 30s
    path: /metrics
```

**Key metrics to monitor:**
```promql
# Throughput
rate(streamforge_messages_consumed_total[5m])
rate(streamforge_messages_produced_total[5m])

# Lag
streamforge_consumer_lag

# Error rate
rate(streamforge_errors_total[5m])

# DLQ rate
rate(streamforge_dlq_messages_total[5m])

# Processing latency (p95)
histogram_quantile(0.95, rate(streamforge_processing_duration_seconds_bucket[5m]))

# Retry rate
rate(streamforge_retries_total[5m])
```

### 2. Grafana Dashboards

**Dashboard JSON:**
```json
{
  "dashboard": {
    "title": "StreamForge Pipeline",
    "panels": [
      {
        "title": "Message Throughput",
        "targets": [{
          "expr": "rate(streamforge_messages_consumed_total[5m])"
        }]
      },
      {
        "title": "Consumer Lag",
        "targets": [{
          "expr": "streamforge_consumer_lag"
        }]
      },
      {
        "title": "Error Rate",
        "targets": [{
          "expr": "rate(streamforge_errors_total[5m])"
        }]
      }
    ]
  }
}
```

### 3. Alerting Rules

**prometheus-rules.yaml:**
```yaml
apiVersion: monitoring.coreos.com/v1
kind: PrometheusRule
metadata:
  name: streamforge-alerts
  namespace: streamforge
spec:
  groups:
  - name: streamforge
    interval: 30s
    rules:
    - alert: StreamForgeHighLag
      expr: streamforge_consumer_lag > 100000
      for: 5m
      labels:
        severity: warning
      annotations:
        summary: "High consumer lag"
        description: "Lag is {{ $value }} messages"
    
    - alert: StreamForgeHighErrorRate
      expr: rate(streamforge_errors_total[5m]) > 10
      for: 2m
      labels:
        severity: critical
      annotations:
        summary: "High error rate"
        description: "Error rate is {{ $value }}/s"
    
    - alert: StreamForgePodDown
      expr: up{job="streamforge"} == 0
      for: 1m
      labels:
        severity: critical
      annotations:
        summary: "StreamForge pod is down"
    
    - alert: StreamForgeHighDLQRate
      expr: rate(streamforge_dlq_messages_total[5m]) > 5
      for: 5m
      labels:
        severity: warning
      annotations:
        summary: "High DLQ rate"
        description: "DLQ rate is {{ $value }}/s"
```

### 4. Distributed Tracing

**Jaeger integration (future):**
```yaml
env:
- name: OTEL_EXPORTER_JAEGER_ENDPOINT
  value: "http://jaeger-collector:14268/api/traces"
- name: OTEL_SERVICE_NAME
  value: "streamforge"
```

---

## Configuration Management

### 1. Environment-Specific Configs

**Directory structure:**
```
configs/
├── base/
│   ├── config.yaml
│   └── kustomization.yaml
├── dev/
│   ├── config-patch.yaml
│   └── kustomization.yaml
├── staging/
│   ├── config-patch.yaml
│   └── kustomization.yaml
└── prod/
    ├── config-patch.yaml
    └── kustomization.yaml
```

**base/kustomization.yaml:**
```yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
- namespace.yaml
- deployment.yaml
- service.yaml
configMapGenerator:
- name: streamforge-config
  files:
  - config.yaml
```

**prod/kustomization.yaml:**
```yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
bases:
- ../base
patchesStrategicMerge:
- config-patch.yaml
replicas:
- name: streamforge
  count: 5
```

**Deploy:**
```bash
kubectl apply -k configs/prod/
```

### 2. ConfigMap Hot Reload

StreamForge supports config hot reload (without restart):

**Watch ConfigMap changes:**
```yaml
spec:
  containers:
  - name: config-watcher
    image: jimmidyson/configmap-reload:latest
    args:
    - --volume-dir=/config
    - --webhook-url=http://localhost:8080/reload
    volumeMounts:
    - name: config
      mountPath: /config
```

### 3. Validation Before Deploy

```bash
# Validate config locally
streamforge-validate configs/prod/config.yaml

# Validate in CI/CD
docker run --rm -v $(pwd)/configs:/configs streamforge:1.0.0 \
  streamforge-validate /configs/prod/config.yaml --fail-on-warnings
```

---

## Next Steps

- [Operations Guide](OPERATIONS.md) - Day-to-day operations
- [Troubleshooting](TROUBLESHOOTING.md) - Common issues and solutions
- [Performance Tuning](PERFORMANCE_TUNING_RESULTS.md) - Optimization guide
- [Monitoring](docs/monitoring/) - Dashboards and alerts

---

**Document Version:** 1.0.0  
**Last Updated:** 2026-04-18  
**Feedback:** https://github.com/rahulbsw/streamforge/issues
