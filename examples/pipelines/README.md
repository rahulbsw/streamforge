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
