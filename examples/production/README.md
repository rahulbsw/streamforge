# Production Configuration Examples

This directory contains production-ready StreamForge configurations for common use cases.

## Overview

Each configuration demonstrates best practices for specific scenarios:

| Config | Use Case | Primary trade-off | Complexity |
|--------|----------|-------------------|------------|
| [user-filtering.yaml](user-filtering.yaml) | User event filtering and routing | More destination-specific work | Medium |
| [cross-region-replication.yaml](cross-region-replication.yaml) | Multi-region Kafka replication | WAN efficiency and recovery point | Low |
| [cdc-to-datalake.yaml](cdc-to-datalake.yaml) | Database CDC to data lake | Freshness versus batching | Medium |
| [multi-tenant-filtering.yaml](multi-tenant-filtering.yaml) | Multi-tenant event routing | Isolation versus pipeline count | Medium |
| [pii-redaction.yaml](pii-redaction.yaml) | PII minimization and pseudonymization | Stronger transforms cost more CPU | High |

---

## Configuration Patterns

### user-filtering.yaml

**Scenario:** Filter and route user events based on status, tier, and region.

**Key features:**
- Multi-destination routing with independent filters
- Region-based partitioning for locality
- Timestamp-based filtering for time windows
- CONSTRUCT transforms for field extraction

**When to use:**
- User activity streams
- Event-driven architectures
- Regional data processing
- Multi-tier systems

**Tuning:**
- Increase `threads` for higher throughput
- Adjust `batch_size` for latency vs throughput trade-off
- Use `fetch_min_bytes` to control batch sizes

---

### cross-region-replication.yaml

**Scenario:** Replicate events from one Kafka cluster to another for DR or multi-region.

**Key features:**
- Batching-oriented configuration for WAN efficiency
- Compression for WAN transfer
- TLS/SASL for secure cross-region
- Offset management with `earliest` for DR

**When to use:**
- Disaster recovery
- Multi-region deployments
- Data center migration
- Hub-and-spoke architectures

**Tuning:**
- Large `batch_size` and `linger_ms` for WAN efficiency
- High `fetch_min_bytes` to reduce round trips
- Aggressive `retry` settings for reliability
- Consider `commit_interval_ms` vs RPO requirements

**Security:**
- Use separate credentials for source/dest clusters
- Encrypt credentials with Kubernetes secrets
- Enable TLS for both source and destination
- Use SASL_SSL protocol for authentication

---

### cdc-to-datalake.yaml

**Scenario:** Stream database changes (CDC) to data lake via Kafka.

**Key features:**
- Debezium CDC format parsing
- Operation-based routing (INSERT/UPDATE/DELETE)
- Schema change handling
- Field extraction from CDC envelope

**When to use:**
- Database replication
- Data warehouse ETL
- Real-time analytics
- Audit logging

**CDC operations:**
- `c` (create): Extract `/payload/after` (new row)
- `u` (update): Extract `/payload/after` (updated row)
- `d` (delete): Extract `/payload/before` (deleted row)
- `s` (schema change): Extract `/payload` (DDL)

**Tuning:**
- Moderate `batch_size` for balanced throughput
- Higher `linger_ms` for data lake batching
- Consider `commit_interval_ms` vs data freshness

**Integration:**
- Works with Debezium, Maxwell, or similar CDC tools
- Output topics can be consumed by:
  - Kafka Connect S3 sink
  - Apache Flink
  - Spark Streaming
  - Custom data lake consumers

---

### multi-tenant-filtering.yaml

**Scenario:** Route events from shared topic to tenant-specific topics.

**Key features:**
- Tier-based routing (enterprise/professional/free)
- Tenant-specific destinations
- Active/inactive filtering
- Dedicated topics for high-value customers

**When to use:**
- SaaS platforms
- Multi-tenant applications
- Customer-specific SLAs
- Usage-based billing

**Tuning:**
- Cache tenant metadata for enrichment
- Use `partitioning: hash` for even load distribution
- Consider separate pipelines for enterprise tier (lower latency)

**Scaling:**
- Scale replicas based on total tenant count
- Monitor lag per destination topic
- Use separate consumer groups for tenant tiers

---

### pii-redaction.yaml

**Scenario:** Redact or mask PII before sending to analytics/third-party systems.

**Key features:**
- Field hashing (SHA256, MD5)
- Selective field extraction with CONSTRUCT
- Consent-based routing
- Full data retention for compliance

**When to use:**
- GDPR/CCPA compliance
- Third-party integrations
- Analytics pipelines
- Data minimization

**PII handling:**
- **Hash:** Use SHA256 for irreversible anonymization
- **Redact:** Remove fields entirely with CONSTRUCT
- **Mask:** Replace with placeholder (not yet supported, use CONSTRUCT)
- **Encrypt:** Use encrypted Kafka topics + TLS

**Compliance considerations:**
- Audit all PII access (enable Kafka audit logs)
- Implement data retention policies
- Document data lineage
- Encrypt at rest and in transit
- Use field-level encryption for sensitive data

---

## Deployment

### Validate Config

```bash
# Validate syntax
streamforge-validate examples/production/user-filtering.yaml

# Check for deprecations
streamforge-validate examples/production/user-filtering.yaml --fail-on-warnings
```

### Deploy to Kubernetes

**Using kubectl:**
```bash
# Create ConfigMap from file
kubectl create configmap streamforge-config \
  --from-file=config.yaml=examples/production/user-filtering.yaml \
  -n streamforge

# Apply deployment
kubectl apply -f k8s/deployment.yaml -n streamforge
```

**Using Helm:**
```bash
# Install with custom config
helm install streamforge streamforge/streamforge \
  --namespace streamforge \
  --create-namespace \
  --values examples/production/user-filtering.yaml
```

**Using Operator:**
```bash
# Convert YAML to StreamforgePipeline CRD
kubectl apply -f - <<EOF
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: user-filtering
  namespace: streamforge
spec:
  image: streamforge:1.0.0
  replicas: 3
  config:
    $(cat examples/production/user-filtering.yaml | sed 's/^/    /')
EOF
```

### Test Locally

**Podman:**
```bash
podman run --rm \
  -v $(pwd)/examples/production/user-filtering.yaml:/app/config.yaml:ro \
  --network host \
  streamforge:1.0.0 \
  --config /app/config.yaml
```

**Podman Compose:**
```yaml
version: '3.8'
services:
  streamforge:
    image: streamforge:1.0.0
    volumes:
      - ./examples/production/user-filtering.yaml:/app/config.yaml:ro
    environment:
      RUST_LOG: info
    networks:
      - kafka-network
```

---

## Performance Tuning

The following profiles illustrate configuration trade-offs; they are not
capacity or latency promises. Measure them with representative messages,
partitions, brokers, delivery guarantees, and hardware.

### Batching-oriented

```yaml
threads: 8
performance:
  fetch_min_bytes: 10240
  batch_size: 5000
  linger_ms: 50
  compression: "zstd"
commit_strategy: "manual"
commit_interval_ms: 10000
```

### Latency-oriented

```yaml
threads: 2
performance:
  fetch_min_bytes: 1
  fetch_max_wait_ms: 10
  batch_size: 100
  linger_ms: 0
commit_strategy: "per-message"
```

### Balanced starting point

```yaml
threads: 4
performance:
  fetch_min_bytes: 5120
  batch_size: 2000
  linger_ms: 20
  compression: "zstd"
commit_strategy: "time-based"
commit_interval_ms: 1000
```

### Resource Efficiency (low cost)

```yaml
threads: 4
performance:
  fetch_min_bytes: 5120
  fetch_max_wait_ms: 500
  batch_size: 2000
  linger_ms: 100
  compression: "zstd"
commit_strategy: "manual"
commit_interval_ms: 10000
resources:
  requests:
    cpu: 500m
    memory: 1Gi
```

---

## Monitoring

### Key Metrics

**Throughput:**
```promql
rate(streamforge_messages_consumed_total[5m])
rate(streamforge_messages_produced_total[5m])
```

**Lag:**
```promql
streamforge_consumer_lag
```

**Error Rate:**
```promql
rate(streamforge_errors_total[5m])
```

**DLQ Rate:**
```promql
rate(streamforge_dlq_messages_total[5m])
```

**Latency (p95):**
```promql
histogram_quantile(0.95, rate(streamforge_processing_duration_seconds_bucket[5m]))
```

### Alerts

**Critical:**
- Consumer lag is growing without recovery
- Error rate breaches the workload-specific error budget
- Pod down
- Memory exhaustion

**Warning:**
- Consumer lag remains above the workload-specific threshold
- Error rate is elevated
- DLQ accumulation
- Processing latency breaches the workload-specific objective

---

## Troubleshooting

### High Lag

1. Check CPU usage: `kubectl top pods -n streamforge`
2. Scale up replicas: `kubectl scale deployment streamforge --replicas=N`
3. Increase threads in config
4. Optimize filters (use KEY_PREFIX instead of REGEX)

### High Error Rate

1. Check DLQ messages: `kafka-console-consumer --topic streamforge-dlq`
2. Review error headers for patterns
3. Fix filter/transform logic
4. Update config and redeploy

### Low Throughput

1. Check threading: Increase `threads` to match CPU cores
2. Check batching: Increase `batch_size` and `linger_ms`
3. Check commit overhead: Use `manual` or `time-based` strategy
4. Enable compression: Use `zstd` for best performance

### Memory Issues

1. Check message sizes
2. Reduce `batch_size`
3. Increase memory limits
4. Check for memory leaks (restart pods)

---

## Security

### Encryption

**TLS for Kafka:**
```yaml
kafka:
  security:
    protocol: "SSL"
  ssl:
    ca_location: "/certs/ca.crt"
    certificate_location: "/certs/client.crt"
    key_location: "/certs/client.key"
```

**SASL Authentication:**
```yaml
kafka:
  security:
    protocol: "SASL_SSL"
    sasl_mechanism: "SCRAM-SHA-512"
    sasl_username: "${KAFKA_USER}"
    sasl_password: "${KAFKA_PASSWORD}"
```

### Secrets Management

**Kubernetes Secrets:**
```bash
kubectl create secret generic kafka-credentials \
  --from-literal=username=myuser \
  --from-literal=password=mypassword \
  -n streamforge
```

**Environment Variables:**
```yaml
env:
- name: KAFKA_USER
  valueFrom:
    secretKeyRef:
      name: kafka-credentials
      key: username
- name: KAFKA_PASSWORD
  valueFrom:
    secretKeyRef:
      name: kafka-credentials
      key: password
```

---

## Additional Resources

- [Deployment Guide](../../docs/DEPLOYMENT.md)
- [Operations Runbook](../../docs/OPERATIONS.md)
- [Troubleshooting Guide](../../docs/TROUBLESHOOTING.md)
- [DSL Reference](../../docs/ADVANCED_DSL_GUIDE.md)
- [Performance Tuning](../../docs/PERFORMANCE.md)

---

**Questions or Issues?**
- GitHub: https://github.com/rahulbsw/streamforge/issues
- Documentation: https://github.datasierra.com/streamforge/
