# Kafka Test Clusters

This directory contains Kubernetes manifests for deploying test Kafka clusters for development and testing with Streamforge.

## Files

### kafka-standalone.yaml
A complete single-node Kafka cluster in KRaft mode (no Zookeeper) suitable for development and testing.

**Features:**
- Apache Kafka 3.9.0 in KRaft mode
- Single broker + controller
- Deployed to `kafka` namespace
- Exposed via ClusterIP service on port 9092
- Service name: `kafka.kafka.svc.cluster.local:9092`

**Usage:**
```bash
# Deploy
kubectl apply -f kafka-standalone.yaml

# Verify
kubectl get pods -n kafka
kubectl get svc -n kafka

# Test connection
kubectl run kafka-test -n kafka --rm -it --image=apache/kafka:3.9.0 -- \
  /opt/kafka/bin/kafka-topics.sh --list \
  --bootstrap-server kafka.kafka.svc.cluster.local:9092
```

### kafka-simple.yaml
A minimal Kafka deployment (older version used in initial testing).

### kafka-test-cluster.yaml
Additional test cluster configuration.

## Using with Streamforge Pipelines

When creating a StreamforgePipeline, use the Kafka service endpoint:

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: test-pipeline
  namespace: streamforge-system
spec:
  source:
    brokers: "kafka.kafka.svc.cluster.local:9092"
    topic: "input-topic"
  destinations:
    - brokers: "kafka.kafka.svc.cluster.local:9092"
      topic: "output-topic"
  replicas: 2
```

## Testing End-to-End

1. Deploy Kafka:
```bash
kubectl apply -f kafka-standalone.yaml
```

2. Create topics:
```bash
kubectl exec -n kafka deployment/kafka -- \
  /opt/kafka/bin/kafka-topics.sh --create \
  --bootstrap-server localhost:9092 \
  --topic input-topic --partitions 3 --replication-factor 1

kubectl exec -n kafka deployment/kafka -- \
  /opt/kafka/bin/kafka-topics.sh --create \
  --bootstrap-server localhost:9092 \
  --topic output-topic --partitions 3 --replication-factor 1
```

3. Deploy Streamforge operator and pipeline:
```bash
helm install streamforge helm/streamforge-operator
kubectl apply -f your-pipeline.yaml
```

4. Send test messages:
```bash
kubectl exec -n kafka deployment/kafka -- \
  /opt/kafka/bin/kafka-console-producer.sh \
  --bootstrap-server localhost:9092 \
  --topic input-topic
```

5. Verify output:
```bash
kubectl exec -n kafka deployment/kafka -- \
  /opt/kafka/bin/kafka-console-consumer.sh \
  --bootstrap-server localhost:9092 \
  --topic output-topic --from-beginning
```

## Notes

- These are **test clusters only** - not production ready
- No persistence (data lost on pod restart)
- No authentication/authorization configured
- Single replica - no high availability
- For production, use a proper Kafka operator like Strimzi

## Cleanup

```bash
kubectl delete -f kafka-standalone.yaml
kubectl delete namespace kafka
```
