# StreamforgePipeline Examples

Example Kubernetes manifests for StreamforgePipeline custom resources.

## Examples

### simple-mirror.yaml
Basic topic mirroring with no transformations. Good starting point for understanding pipeline configuration.

**Features:**
- Simple source → destination mirroring
- 2 replicas for high availability
- Latest offset (doesn't replay historical messages)
- Consumer group for offset tracking

**Usage:**
```bash
kubectl apply -f simple-mirror.yaml
kubectl get streamforgepipeline -n streamforge-system
kubectl get pods -n streamforge-system -l streamforge.io/pipeline=simple-mirror
```

### secure-sasl-pipeline.yaml
Pipeline with SASL authentication using Kubernetes secrets.

**Features:**
- SASL/SCRAM authentication
- Credentials stored in Kubernetes secrets (not inline)
- TLS encryption with CA certificate
- Production-ready security

**Usage:**
```bash
# Create secrets first
kubectl create secret generic kafka-sasl-credentials \
  --from-literal=username=myuser \
  --from-literal=password=mypassword \
  -n streamforge-system

kubectl create secret generic kafka-ca-cert \
  --from-file=ca.crt=/path/to/ca-cert.pem \
  -n streamforge-system

# Deploy pipeline
kubectl apply -f secure-sasl-pipeline.yaml
```

### secure-tls-pipeline.yaml
Pipeline with mutual TLS (mTLS) authentication.

**Features:**
- Client certificate authentication
- All certificates stored in secrets
- Encrypted communication
- Highest security level

**Usage:**
```bash
# Create TLS secret
kubectl create secret generic kafka-tls-certs \
  --from-file=ca.crt=/path/to/ca-cert.pem \
  --from-file=client.crt=/path/to/client-cert.pem \
  --from-file=client.key=/path/to/client-key.pem \
  --from-literal=key.password=myKeyPassword \
  -n streamforge-system

# Deploy pipeline
kubectl apply -f secure-tls-pipeline.yaml
```

### multi-cluster-secure-pipeline.yaml
Advanced example: Mirror between multiple Kafka clusters with different credentials.

**Features:**
- Source and 2 destinations, each with different authentication
- Production → Analytics (SASL) + Backup (mTLS)
- Demonstrates secret path organization (source/, destination-0/, destination-1/)
- No credential conflicts between clusters

**Usage:**
```bash
# See the file for complete secret creation commands
kubectl apply -f multi-cluster-secure-pipeline.yaml

# Verify secret mounting paths
kubectl exec -n streamforge-system deployment/multi-cluster-secure-pipeline -- \
  ls -R /etc/streamforge/secrets/
```

**📖 For detailed secret management documentation, see [SECRETS.md](./SECRETS.md)**

## Pipeline Specification

A StreamforgePipeline has the following main components:

### Source Configuration
```yaml
source:
  brokers: "kafka-broker:9092"    # Kafka broker addresses
  topic: "source-topic"           # Source topic name
  offset: "latest"                # Start offset: latest, earliest, or specific
  groupId: "consumer-group-id"    # Consumer group for offset management
```

### Destination Configuration
```yaml
destinations:
  - brokers: "kafka-broker:9092"  # Can be same or different cluster
    topic: "dest-topic"           # Destination topic name
```

### Resource Configuration
```yaml
replicas: 2                       # Number of pod replicas
threads: 4                        # Worker threads per replica
appid: "my-pipeline"              # Application identifier
```

## Advanced Features

For more advanced configurations, see the [operator documentation](../../helm/streamforge-operator/README.md) which covers:

- Multiple destinations (fan-out)
- Message filtering with JSONPath
- Message transformations
- Security configuration (SASL, SSL, Kerberos)
- Custom resource limits
- Monitoring and metrics

## Viewing Pipeline Status

```bash
# List all pipelines
kubectl get streamforgepipelines -A

# Get detailed pipeline info
kubectl describe streamforgepipeline simple-mirror -n streamforge-system

# Check pipeline pods
kubectl get pods -n streamforge-system -l streamforge.io/pipeline=simple-mirror

# View pipeline logs
kubectl logs -n streamforge-system -l streamforge.io/pipeline=simple-mirror --tail=100
```

## Troubleshooting

### Pipeline shows "Unknown" status
- Check operator logs: `kubectl logs -n streamforge-system deployment/streamforge-operator`
- Verify Kafka connectivity from the cluster
- Check if source/destination topics exist

### Pipeline pods crash loop
- Check pod logs: `kubectl logs -n streamforge-system <pod-name>`
- Verify Kafka broker addresses are correct
- Check if authentication credentials are valid (if configured)

### No messages flowing
- Verify source topic has messages
- Check consumer group offset: use `kafka-consumer-groups.sh`
- Verify network connectivity between Kubernetes and Kafka
- Check for filtering rules that might exclude messages

## Cleanup

```bash
kubectl delete streamforgepipeline simple-mirror -n streamforge-system
```
