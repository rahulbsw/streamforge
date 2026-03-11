# Streamforge Examples

This directory contains examples and reference configurations for using Streamforge.

## Directory Structure

```
examples/
├── configs/            # Application config files (YAML/JSON)
├── kubernetes/
│   └── kafka/          # Test Kafka cluster deployments
└── pipelines/          # Example StreamforgePipeline K8s resources
```

## Quick Start

### 1. Deploy Test Kafka Cluster

```bash
kubectl apply -f kubernetes/kafka/kafka-standalone.yaml
```

### 2. Install Streamforge Operator

```bash
helm install streamforge ../helm/streamforge-operator \
  --namespace streamforge-system \
  --create-namespace
```

### 3. Create a Pipeline

```bash
kubectl apply -f pipelines/simple-mirror.yaml
```

## More Information

- [Application Configs](configs/) - YAML/JSON configs for running streamforge binary directly
- [Kafka Test Clusters](kubernetes/kafka/README.md) - Development Kafka setups
- [Pipeline Examples](pipelines/README.md) - StreamforgePipeline Kubernetes resources

## Deployment Modes

Streamforge can run in two modes:

### 1. Standalone Binary
Use config files from `configs/` directory to run the streamforge binary directly:
```bash
streamforge --config examples/configs/config.example.yaml
```

### 2. Kubernetes Operator
Use pipeline manifests from `pipelines/` directory with the Kubernetes operator:
```bash
kubectl apply -f examples/pipelines/simple-mirror.yaml
```

## Production Deployment

For production use:
1. Use a production-grade Kafka cluster (Confluent, Strimzi, MSK, etc.)
2. Configure authentication and encryption
3. Set resource limits and requests
4. Enable monitoring and observability
5. Configure backup and disaster recovery

See the main project README for production deployment guidance.
