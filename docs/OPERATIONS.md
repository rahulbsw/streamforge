# StreamForge Operations Runbook

**Version:** 1.0.0  
**Last Updated:** 2026-04-18

This runbook provides operational procedures for running StreamForge in production.

---

## Table of Contents

1. [Daily Operations](#daily-operations)
2. [Monitoring and Alerting](#monitoring-and-alerting)
3. [Scaling Operations](#scaling-operations)
4. [Incident Response](#incident-response)
5. [Capacity Planning](#capacity-planning)
6. [Maintenance Windows](#maintenance-windows)
7. [Backup and Recovery](#backup-and-recovery)
8. [Performance Optimization](#performance-optimization)
9. [Common Operational Tasks](#common-operational-tasks)

---

## Daily Operations

### Morning Health Check

**1. Check pod status:**
```bash
kubectl get pods -n streamforge
kubectl get hpa -n streamforge
```

Expected output:
```
NAME                          READY   STATUS    RESTARTS   AGE
streamforge-7c8f9d4b6-abc12   1/1     Running   0          2d
streamforge-7c8f9d4b6-def34   1/1     Running   0          2d
streamforge-7c8f9d4b6-ghi56   1/1     Running   0          1d
```

**2. Check consumer lag:**
```bash
# Via metrics endpoint
curl http://streamforge.streamforge.svc:8080/metrics | grep consumer_lag

# Or via Kafka directly
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group <appid>
```

Acceptable lag:
- **< 1000 messages:** Healthy
- **1000-10000 messages:** Monitor
- **> 10000 messages:** Investigate (scale up or tune)

**3. Check error rates:**
```promql
rate(streamforge_errors_total[5m])
```

Acceptable error rate:
- **< 1/s:** Normal (transient errors)
- **1-10/s:** Monitor
- **> 10/s:** Investigate (check DLQ and logs)

**4. Check DLQ:**
```bash
kafka-console-consumer --bootstrap-server kafka:9092 \
  --topic streamforge-dlq \
  --property print.headers=true \
  --max-messages 10
```

Review recent DLQ messages:
- Parse error headers (`x-streamforge-error-type`)
- Identify patterns (bad data format, config issues)
- Update filters/transforms if needed

### Weekly Tasks

**1. Review metrics trends:**
- Throughput (consumed/produced per second)
- Processing latency (p50, p95, p99)
- Error rates over time
- Resource utilization (CPU, memory)

**2. Check for updates:**
```bash
helm repo update
helm search repo streamforge
```

**3. Review DLQ accumulation:**
```bash
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group streamforge-dlq-consumer
```

If DLQ is growing:
- Investigate root cause (bad data, config error)
- Fix issue in pipeline config
- Reprocess DLQ messages if needed

**4. Capacity planning check:**
- Review growth trends
- Predict when scaling will be needed
- Plan for capacity additions

### Monthly Tasks

**1. Upgrade StreamForge:**
```bash
helm upgrade streamforge streamforge/streamforge \
  --namespace streamforge \
  --values values.yaml \
  --version 1.0.1
```

**2. Security audit:**
- Review access logs
- Rotate credentials (Kafka passwords, TLS certs)
- Update TLS certificates before expiration

**3. Performance benchmarking:**
- Run load tests
- Compare with baseline metrics
- Identify performance degradation

**4. Disaster recovery test:**
- Verify backups are working
- Test failover procedures
- Update runbooks based on findings

---

## Monitoring and Alerting

### Key Metrics

#### Throughput Metrics

**Messages consumed per second:**
```promql
rate(streamforge_messages_consumed_total[5m])
```

**Messages produced per second:**
```promql
rate(streamforge_messages_produced_total[5m])
```

**Baseline:** Establish during initial deployment (e.g., 50K msg/s)

#### Lag Metrics

**Consumer lag:**
```promql
streamforge_consumer_lag
```

**Lag increase rate:**
```promql
deriv(streamforge_consumer_lag[10m])
```

**Alert thresholds:**
- Warning: lag > 10000
- Critical: lag > 100000 or lag growing for 10 minutes

#### Error Metrics

**Error rate:**
```promql
rate(streamforge_errors_total[5m])
```

**Error by type:**
```promql
rate(streamforge_errors_total{error_type="FilterEvaluation"}[5m])
rate(streamforge_errors_total{error_type="ProducerTimeout"}[5m])
```

**Alert thresholds:**
- Warning: error rate > 1/s
- Critical: error rate > 10/s

#### DLQ Metrics

**DLQ message rate:**
```promql
rate(streamforge_dlq_messages_total[5m])
```

**DLQ accumulation:**
```bash
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group streamforge | grep dlq
```

**Alert thresholds:**
- Warning: DLQ rate > 1/s
- Critical: DLQ rate > 10/s or DLQ lag > 1000

#### Latency Metrics

**Processing duration (p95):**
```promql
histogram_quantile(0.95, rate(streamforge_processing_duration_seconds_bucket[5m]))
```

**Processing duration (p99):**
```promql
histogram_quantile(0.99, rate(streamforge_processing_duration_seconds_bucket[5m]))
```

**Alert thresholds:**
- Warning: p95 > 100ms
- Critical: p95 > 500ms

#### Resource Metrics

**CPU usage:**
```promql
rate(container_cpu_usage_seconds_total{pod=~"streamforge.*"}[5m])
```

**Memory usage:**
```promql
container_memory_usage_bytes{pod=~"streamforge.*"}
```

**Alert thresholds:**
- Warning: CPU > 70% or Memory > 80%
- Critical: CPU > 90% or Memory > 95%

### Alert Rules

**Critical Alerts (page immediately):**

1. **Pipeline Down:**
   ```promql
   up{job="streamforge"} == 0
   ```
   **Action:** Check pod status, review logs, restart if needed

2. **High Error Rate:**
   ```promql
   rate(streamforge_errors_total[5m]) > 10
   ```
   **Action:** Check logs for error patterns, review recent config changes

3. **Consumer Lag Critical:**
   ```promql
   streamforge_consumer_lag > 100000
   ```
   **Action:** Scale up replicas, increase threads, check Kafka performance

4. **Memory Exhaustion:**
   ```promql
   container_memory_usage_bytes / container_spec_memory_limit_bytes > 0.95
   ```
   **Action:** Increase memory limits, check for memory leak

**Warning Alerts (investigate during business hours):**

1. **Consumer Lag Warning:**
   ```promql
   streamforge_consumer_lag > 10000
   ```
   **Action:** Monitor lag trend, prepare to scale if increasing

2. **DLQ Accumulation:**
   ```promql
   rate(streamforge_dlq_messages_total[5m]) > 1
   ```
   **Action:** Review DLQ messages, identify data quality issues

3. **High Latency:**
   ```promql
   histogram_quantile(0.95, rate(streamforge_processing_duration_seconds_bucket[5m])) > 0.1
   ```
   **Action:** Check filter/transform complexity, review performance

4. **Pod Restarts:**
   ```promql
   rate(kube_pod_container_status_restarts_total{pod=~"streamforge.*"}[1h]) > 0
   ```
   **Action:** Review pod logs, check for OOMKilled or CrashLoopBackOff

### Dashboard Layout

**Overview Dashboard:**
- Throughput (consumed/produced)
- Consumer lag
- Error rate
- DLQ rate
- Pod count and health

**Performance Dashboard:**
- Processing latency (p50, p95, p99)
- CPU usage by pod
- Memory usage by pod
- Network I/O

**Error Analysis Dashboard:**
- Errors by type
- Error rate over time
- DLQ messages by error type
- Retry attempts histogram

---

## Scaling Operations

### Horizontal Scaling (Add Replicas)

**When to scale up:**
- Consumer lag > 10000 and growing
- CPU usage > 70% sustained
- Throughput needs to increase

**Scale up:**
```bash
# Manual
kubectl scale deployment streamforge --replicas=5 -n streamforge

# Or update HPA
kubectl edit hpa streamforge -n streamforge
```

**Verify:**
```bash
kubectl get pods -n streamforge
kubectl get hpa streamforge -n streamforge

# Check consumer group rebalancing
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group <appid>
```

**Expected behavior:**
- Pods start and become Ready
- Consumer group rebalances
- Partitions redistributed across consumers
- Lag decreases over 5-10 minutes

**When to scale down:**
- Lag < 100 sustained
- CPU usage < 30% sustained
- Traffic decreased

**Scale down:**
```bash
kubectl scale deployment streamforge --replicas=2 -n streamforge
```

**Caution:** Gradual scale-down to avoid lag spikes during rebalancing.

### Vertical Scaling (Increase Resources)

**When to scale up:**
- Memory usage > 80% sustained
- CPU limits hit frequently
- OOMKilled events

**Update resources:**
```bash
kubectl patch deployment streamforge -n streamforge --patch '
spec:
  template:
    spec:
      containers:
      - name: streamforge
        resources:
          requests:
            cpu: "2000m"
            memory: "4Gi"
          limits:
            cpu: "4000m"
            memory: "8Gi"
'
```

**Or via Helm:**
```bash
helm upgrade streamforge streamforge/streamforge \
  --namespace streamforge \
  --reuse-values \
  --set resources.requests.cpu=2000m \
  --set resources.requests.memory=4Gi \
  --set resources.limits.cpu=4000m \
  --set resources.limits.memory=8Gi
```

**Verify:**
```bash
kubectl get pods -n streamforge
kubectl describe pod streamforge-<pod-id> -n streamforge | grep -A5 Requests
```

### Thread Scaling (Increase Parallelism)

**When to increase threads:**
- CPU usage < 50% but lag is high
- Many CPU cores available
- Filter/transform logic is CPU-bound

**Update config:**
```yaml
threads: 8  # increase from 4
```

**Apply:**
```bash
kubectl edit configmap streamforge-config -n streamforge
kubectl rollout restart deployment/streamforge -n streamforge
```

**Verify:**
```bash
kubectl logs -f deployment/streamforge -n streamforge | grep "threads"
```

**Rule of thumb:**
- 1 thread per CPU core
- Max 16 threads (diminishing returns)
- Monitor CPU usage after change

### Autoscaling Configuration

**HPA based on CPU:**
```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: streamforge
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
```

**HPA based on custom metric (consumer lag):**
```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: streamforge
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: streamforge
  minReplicas: 2
  maxReplicas: 10
  metrics:
  - type: Pods
    pods:
      metric:
        name: streamforge_consumer_lag
      target:
        type: AverageValue
        averageValue: "5000"
```

**Scale-up behavior:**
- Stabilization: 60 seconds
- Max scale rate: 50% per minute

**Scale-down behavior:**
- Stabilization: 300 seconds (5 minutes)
- Max scale rate: 25% per minute

---

## Incident Response

### High Consumer Lag

**Symptoms:**
- Lag > 100000 messages
- Lag growing steadily
- Alert fired: "StreamForgeHighLag"

**Investigation:**

1. **Check throughput:**
   ```bash
   curl http://streamforge:8080/metrics | grep messages_consumed_total
   curl http://streamforge:8080/metrics | grep messages_produced_total
   ```

2. **Check replicas:**
   ```bash
   kubectl get deployment streamforge -n streamforge
   ```

3. **Check CPU/memory:**
   ```bash
   kubectl top pods -n streamforge
   ```

4. **Check errors:**
   ```bash
   kubectl logs deployment/streamforge -n streamforge | grep ERROR
   ```

**Resolution:**

**If CPU saturated (> 80%):**
- Scale up replicas: `kubectl scale deployment streamforge --replicas=N`
- Increase threads in config
- Optimize filters/transforms

**If memory saturated (> 90%):**
- Increase memory limits
- Reduce batch size
- Check for memory leaks (restart pods)

**If Kafka is slow:**
- Check Kafka broker health
- Increase `fetch_max_wait_ms`
- Increase `fetch_min_bytes`

**If throughput is limited:**
- Increase producer batch size
- Reduce `linger_ms`
- Enable compression

### High Error Rate

**Symptoms:**
- Error rate > 10/s
- Alert fired: "StreamForgeHighErrorRate"
- DLQ accumulating rapidly

**Investigation:**

1. **Check error types:**
   ```bash
   kubectl logs deployment/streamforge -n streamforge | grep ERROR | tail -50
   ```

2. **Check DLQ headers:**
   ```bash
   kafka-console-consumer --bootstrap-server kafka:9092 \
     --topic streamforge-dlq \
     --property print.headers=true \
     --max-messages 5
   ```

3. **Check recent config changes:**
   ```bash
   kubectl describe configmap streamforge-config -n streamforge
   ```

**Common error types and resolutions:**

**FilterEvaluation errors:**
- **Cause:** Bad data format (missing fields, wrong types)
- **Fix:** Update filter to handle missing fields (e.g., add default value)
- **Example:** Change `/status,==,active` to `OR:/status,==,active:/status,==,null`

**ProducerTimeout errors:**
- **Cause:** Kafka producer timeout (slow brokers, network issues)
- **Fix:** Increase retry attempts, check Kafka health
- **Config:** Increase `max_delay_ms` in retry config

**SerializationError:**
- **Cause:** Invalid JSON in transform output
- **Fix:** Review transform logic, add validation
- **Example:** Ensure CONSTRUCT generates valid JSON

**ConnectionError:**
- **Cause:** Kafka broker unreachable
- **Fix:** Check network, DNS, Kafka broker status
- **Recovery:** Will auto-retry with exponential backoff

### Pod Crashes (CrashLoopBackOff)

**Symptoms:**
- Pods restarting frequently
- Status: CrashLoopBackOff
- Alert fired: "StreamForgePodDown"

**Investigation:**

1. **Check pod status:**
   ```bash
   kubectl get pods -n streamforge
   kubectl describe pod streamforge-<pod-id> -n streamforge
   ```

2. **Check logs:**
   ```bash
   kubectl logs streamforge-<pod-id> -n streamforge --previous
   ```

3. **Check events:**
   ```bash
   kubectl get events -n streamforge --sort-by='.lastTimestamp'
   ```

**Common causes:**

**OOMKilled (Out of Memory):**
- **Symptom:** Last State: Terminated, Reason: OOMKilled
- **Fix:** Increase memory limits
- **Config:**
  ```yaml
  resources:
    limits:
      memory: 8Gi
  ```

**Config error:**
- **Symptom:** Logs show "invalid config" or "parse error"
- **Fix:** Validate config with `streamforge-validate`
- **Check:** Run `kubectl logs` to see exact error

**Kafka connection failure:**
- **Symptom:** Logs show "Failed to connect to Kafka"
- **Fix:** Check bootstrap servers, TLS certs, SASL credentials
- **Test:** Use `kafka-console-consumer` to verify connectivity

**Missing secret:**
- **Symptom:** Logs show "secret not found" or volume mount error
- **Fix:** Create missing secret
- **Check:** `kubectl get secret <name> -n streamforge`

### DLQ Overflow

**Symptoms:**
- DLQ lag > 1000
- DLQ rate > 10/s sustained
- Disk usage increasing

**Investigation:**

1. **Count DLQ messages:**
   ```bash
   kafka-run-class kafka.tools.GetOffsetShell \
     --broker-list kafka:9092 \
     --topic streamforge-dlq
   ```

2. **Sample DLQ messages:**
   ```bash
   kafka-console-consumer --bootstrap-server kafka:9092 \
     --topic streamforge-dlq \
     --property print.headers=true \
     --max-messages 20
   ```

3. **Identify error patterns:**
   - Group by `x-streamforge-error-type` header
   - Identify common source topics
   - Check for data quality issues

**Resolution:**

**If error is in config:**
- Fix filter/transform logic
- Deploy updated config
- Reprocess DLQ messages

**If error is in data:**
- Fix upstream data producer
- Add data validation at source
- Optionally skip bad messages (update filter)

**Reprocess DLQ:**
```yaml
# Create DLQ reprocessing pipeline
appid: "dlq-reprocessor"
input: "streamforge-dlq"
offset: "earliest"
threads: 1
routing:
  destinations:
    - output: "original-topic"
      filter: "/error-type,!=,permanent"  # Skip permanent failures
      transform: "/original-data"  # Extract original message
```

**Purge DLQ (if data is bad and not recoverable):**
```bash
kafka-delete-records --bootstrap-server kafka:9092 \
  --offset-json-file delete-dlq.json
```

delete-dlq.json:
```json
{
  "partitions": [
    {"topic": "streamforge-dlq", "partition": 0, "offset": 1000}
  ]
}
```

### Performance Degradation

**Symptoms:**
- Processing latency increased (p95 > 200ms, was 50ms)
- Throughput decreased (20K msg/s, was 50K msg/s)
- No obvious errors

**Investigation:**

1. **Check metrics history:**
   - Compare current vs baseline (1 week ago)
   - Identify when degradation started

2. **Check resource usage:**
   ```bash
   kubectl top pods -n streamforge
   ```

3. **Check Kafka performance:**
   ```bash
   kafka-broker-api-versions --bootstrap-server kafka:9092
   # Check broker response time
   ```

4. **Check for config changes:**
   ```bash
   kubectl get configmap streamforge-config -n streamforge -o yaml
   ```

**Common causes:**

**Increased message size:**
- **Symptom:** Same throughput (msg/s) but higher latency
- **Fix:** Increase `fetch_min_bytes`, tune compression

**Complex filters/transforms added:**
- **Symptom:** CPU usage increased
- **Fix:** Optimize DSL expressions, increase threads

**Kafka broker issues:**
- **Symptom:** High fetch latency
- **Fix:** Scale Kafka brokers, add partitions

**Network congestion:**
- **Symptom:** High network I/O wait
- **Fix:** Increase network bandwidth, enable compression

---

## Capacity Planning

### Throughput Estimation

**Formula:**
```
Max throughput (msg/s) = (CPU cores × threads per core × single-thread throughput) / message size factor
```

**Baseline single-thread throughput:**
- Passthrough (no filters): ~100K msg/s
- Simple filters (JSON path): ~50K msg/s
- Complex transforms (CONSTRUCT): ~20K msg/s
- Regex filters: ~10K msg/s

**Message size factor:**
- Small (< 1 KB): 1.0x
- Medium (1-10 KB): 0.8x
- Large (10-100 KB): 0.5x
- Very large (> 100 KB): 0.2x

**Example:**
- 4 CPU cores
- 4 threads per core = 16 threads total
- Complex transforms (~20K msg/s per thread)
- Medium messages (1-10 KB): 0.8x factor

Max throughput = 4 × 4 × 20000 × 0.8 = 256K msg/s

### Resource Requirements

**Per replica:**

| Throughput | CPU Request | CPU Limit | Memory Request | Memory Limit |
|------------|-------------|-----------|----------------|--------------|
| 10K msg/s  | 500m        | 1000m     | 1 Gi           | 2 Gi         |
| 50K msg/s  | 1000m       | 2000m     | 2 Gi           | 4 Gi         |
| 100K msg/s | 2000m       | 4000m     | 4 Gi           | 8 Gi         |
| 200K msg/s | 4000m       | 8000m     | 8 Gi           | 16 Gi        |

**Partitions:**
- One consumer per partition (max)
- If replicas > partitions, some replicas will be idle
- Recommended: partitions ≥ replicas × 2

**Example:**
- Target: 100K msg/s
- Partitions: 16
- Replicas: 4 (leaves headroom for scaling to 16)
- Resources per replica: 2 CPU / 4 Gi

### Growth Planning

**Monthly review:**

1. **Measure current usage:**
   ```promql
   avg_over_time(rate(streamforge_messages_consumed_total[1d])[30d])
   ```

2. **Calculate growth rate:**
   ```
   Growth rate = (Current - Last month) / Last month
   ```

3. **Project future needs:**
   ```
   Projected throughput (3 months) = Current × (1 + growth_rate)^3
   ```

4. **Plan capacity additions:**
   - If projected > 80% of max capacity: add replicas
   - If projected > 200% of max capacity: add partitions

**Example:**
- Current: 50K msg/s
- Last month: 40K msg/s
- Growth rate: (50K - 40K) / 40K = 25% per month
- Projected (3 months): 50K × 1.25^3 = 97.7K msg/s
- Current max: 100K msg/s (80% threshold = 80K)
- **Action:** Plan to add 2 replicas in 2 months

### Kafka Partition Scaling

**When to add partitions:**
- Consumer lag high even with max replicas
- Throughput > (partitions × per-partition throughput)
- Need more parallelism

**Add partitions:**
```bash
kafka-topics --bootstrap-server kafka:9092 \
  --alter --topic source-topic \
  --partitions 32
```

**Considerations:**
- **Cannot decrease partitions** (Kafka limitation)
- Rebalancing will occur (temporary lag spike)
- Keyed messages may redistribute (breaks ordering)
- DLQ topic should match partition count

**Best practice:**
- Start with 16 partitions
- Double when needed (16 → 32 → 64)
- Max 128 partitions per topic (broker limits)

---

## Maintenance Windows

### Planned Upgrades

**Pre-upgrade checklist:**
1. Review changelog for breaking changes
2. Backup current config
3. Test in dev/staging environment
4. Schedule during low-traffic window
5. Prepare rollback plan

**Upgrade procedure:**

```bash
# 1. Backup config
kubectl get configmap streamforge-config -n streamforge -o yaml > config-backup.yaml

# 2. Update Helm chart
helm repo update
helm upgrade streamforge streamforge/streamforge \
  --namespace streamforge \
  --values values.yaml \
  --version 1.1.0

# 3. Monitor rollout
kubectl rollout status deployment/streamforge -n streamforge

# 4. Verify health
kubectl get pods -n streamforge
curl http://streamforge:8080/metrics | grep up

# 5. Check lag
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group <appid>
```

**Rollback procedure (if upgrade fails):**
```bash
# 1. Rollback Helm release
helm rollback streamforge -n streamforge

# 2. Verify rollback
kubectl rollout status deployment/streamforge -n streamforge

# 3. Restore config if needed
kubectl apply -f config-backup.yaml
```

### Config Updates

**Zero-downtime config update:**

```bash
# 1. Edit ConfigMap
kubectl edit configmap streamforge-config -n streamforge

# 2. Rolling restart
kubectl rollout restart deployment/streamforge -n streamforge

# 3. Monitor rollout (one pod at a time)
kubectl rollout status deployment/streamforge -n streamforge

# 4. Check logs for errors
kubectl logs -f deployment/streamforge -n streamforge
```

**High-risk config changes:**
- Changing consumer group ID (will re-consume from offset)
- Changing partition routing (breaks keyed ordering)
- Changing DLQ topic (old DLQ orphaned)

**For high-risk changes:**
1. Deploy as new pipeline with new appid
2. Run in parallel with old pipeline
3. Verify correctness
4. Switch traffic to new pipeline
5. Retire old pipeline

### Kafka Cluster Maintenance

**Broker rolling restart:**

StreamForge will auto-reconnect to Kafka brokers:
- Retry connection errors
- Consumer rebalances automatically
- Producer retries failed sends

**Monitor during Kafka maintenance:**
```bash
watch kubectl logs deployment/streamforge -n streamforge --tail=20
```

**Expected behavior:**
- Connection errors logged (normal during restart)
- Retry attempts visible in logs
- Consumer lag may spike temporarily (catchup after restart)

**Kafka version upgrade:**
- Test StreamForge with new Kafka version in dev first
- Check rdkafka compatibility matrix
- Update bootstrap servers if endpoints changed

---

## Backup and Recovery

### Configuration Backup

**Backup all resources:**
```bash
kubectl get configmap,secret,deployment,service,hpa -n streamforge -o yaml > streamforge-backup.yaml
```

**Backup to Git:**
```bash
# Export to Git repo
mkdir -p backups/$(date +%Y-%m-%d)
kubectl get configmap streamforge-config -n streamforge -o yaml > backups/$(date +%Y-%m-%d)/config.yaml
git add backups/
git commit -m "Backup StreamForge config"
git push
```

**Automated backup (CronJob):**
```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: streamforge-backup
  namespace: streamforge
spec:
  schedule: "0 2 * * *"  # Daily at 2 AM
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: backup
            image: bitnami/kubectl:latest
            command:
            - /bin/sh
            - -c
            - |
              kubectl get configmap,secret,deployment -n streamforge -o yaml > /backup/streamforge-$(date +%Y-%m-%d).yaml
              # Upload to S3 or Git
            volumeMounts:
            - name: backup
              mountPath: /backup
          restartPolicy: OnFailure
          volumes:
          - name: backup
            persistentVolumeClaim:
              claimName: backup-pvc
```

### Offset Backup

**Current offsets are managed by Kafka** (consumer group state).

**View current offsets:**
```bash
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group <appid> > offsets-backup.txt
```

**Reset offsets (disaster recovery):**
```bash
# Reset to earliest
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --group <appid> \
  --reset-offsets --to-earliest --topic source-topic \
  --execute

# Reset to specific offset
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --group <appid> \
  --reset-offsets --to-offset 1000 --topic source-topic:0 \
  --execute

# Reset to timestamp
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --group <appid> \
  --reset-offsets --to-datetime 2026-04-18T00:00:00.000 --topic source-topic \
  --execute
```

**Note:** Stop all consumers before resetting offsets.

### Disaster Recovery

**Scenario 1: Namespace deleted**

```bash
# 1. Recreate namespace
kubectl create namespace streamforge

# 2. Restore resources
kubectl apply -f streamforge-backup.yaml

# 3. Verify
kubectl get pods -n streamforge
```

**Scenario 2: Config lost**

```bash
# 1. Restore from backup
kubectl apply -f backups/2026-04-18/config.yaml

# 2. Restart pods
kubectl rollout restart deployment/streamforge -n streamforge
```

**Scenario 3: Kafka data loss (topic deleted)**

- **Source topic deleted:** StreamForge will error (topic not found), fix Kafka
- **Destination topic deleted:** Recreate topic, StreamForge will auto-recover
- **DLQ topic deleted:** Recreate, but messages lost (not recoverable)

**Best practice:** Enable Kafka topic auto-create or pre-create topics.

---

## Performance Optimization

### Tuning for Throughput

**Goal:** Maximize messages per second

**Config changes:**
```yaml
threads: 8  # Match CPU cores

performance:
  fetch_min_bytes: 10240      # Larger batches
  fetch_max_wait_ms: 100      # Don't wait long
  batch_size: 5000            # Large producer batches
  linger_ms: 50               # Allow batching
  queue_buffering_max_ms: 100
  compression: "zstd"         # Fast compression

# Manual commit for throughput
commit_strategy: "manual"
commit_interval_ms: 5000  # Commit every 5 seconds
```

**Resource allocation:**
```yaml
resources:
  requests:
    cpu: 4000m
    memory: 8Gi
  limits:
    cpu: 4000m
    memory: 8Gi
```

**Scale replicas:**
```bash
kubectl scale deployment streamforge --replicas=8 -n streamforge
```

### Tuning for Latency

**Goal:** Minimize end-to-end latency

**Config changes:**
```yaml
threads: 2  # Fewer threads, less contention

performance:
  fetch_min_bytes: 1          # Don't wait for data
  fetch_max_wait_ms: 10       # Short wait
  batch_size: 100             # Small batches
  linger_ms: 0                # Send immediately
  queue_buffering_max_ms: 1

# Per-message commit for low latency
commit_strategy: "per-message"
```

**Resource allocation:**
```yaml
resources:
  limits:
    cpu: 2000m
    memory: 2Gi
```

**Trade-off:** Lower throughput (10-20K msg/s) for lower latency (< 10ms p95).

### Tuning for Efficiency

**Goal:** Minimize resource usage (cost optimization)

**Config changes:**
```yaml
threads: 4  # Moderate threading

performance:
  fetch_min_bytes: 5120       # Medium batches
  fetch_max_wait_ms: 500      # Wait for batches
  batch_size: 2000
  linger_ms: 100              # Batch aggressively
  compression: "zstd"

commit_strategy: "manual"
commit_interval_ms: 10000  # Infrequent commits
```

**Resource allocation:**
```yaml
resources:
  requests:
    cpu: 500m
    memory: 1Gi
  limits:
    cpu: 1000m
    memory: 2Gi
```

**Scale down aggressively:**
```yaml
autoscaling:
  minReplicas: 1
  maxReplicas: 5
  targetCPU: 80  # Allow higher utilization
```

---

## Common Operational Tasks

### View Logs

**Tail logs:**
```bash
kubectl logs -f deployment/streamforge -n streamforge
```

**Logs from specific pod:**
```bash
kubectl logs streamforge-7c8f9d4b6-abc12 -n streamforge
```

**Logs from previous crashed pod:**
```bash
kubectl logs streamforge-7c8f9d4b6-abc12 -n streamforge --previous
```

**Search logs for errors:**
```bash
kubectl logs deployment/streamforge -n streamforge | grep ERROR
```

**Export logs:**
```bash
kubectl logs deployment/streamforge -n streamforge --since=1h > logs.txt
```

### Restart Pods

**Rolling restart (zero downtime):**
```bash
kubectl rollout restart deployment/streamforge -n streamforge
```

**Force restart single pod:**
```bash
kubectl delete pod streamforge-7c8f9d4b6-abc12 -n streamforge
```

**Restart all pods:**
```bash
kubectl delete pods -l app=streamforge -n streamforge
```

### Update Config

**Edit ConfigMap:**
```bash
kubectl edit configmap streamforge-config -n streamforge
```

**Or apply from file:**
```bash
kubectl apply -f config.yaml
```

**Reload config (if hot-reload enabled):**
```bash
curl -X POST http://streamforge:8080/reload
```

**Or restart pods:**
```bash
kubectl rollout restart deployment/streamforge -n streamforge
```

### Check Consumer Group

**Describe group:**
```bash
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group <appid>
```

Output:
```
GROUP    TOPIC        PARTITION  CURRENT-OFFSET  LOG-END-OFFSET  LAG   CONSUMER-ID
appid    source-topic 0          1000            1050            50    consumer-1
appid    source-topic 1          1200            1200            0     consumer-2
```

### Reset Consumer Offsets

**Reset to latest:**
```bash
kubectl scale deployment streamforge --replicas=0 -n streamforge

kafka-consumer-groups --bootstrap-server kafka:9092 \
  --group <appid> \
  --reset-offsets --to-latest --topic source-topic \
  --execute

kubectl scale deployment streamforge --replicas=3 -n streamforge
```

**Reset to specific timestamp:**
```bash
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --group <appid> \
  --reset-offsets --to-datetime 2026-04-18T12:00:00.000 --topic source-topic \
  --execute
```

### Test Config Locally

**Validate config:**
```bash
streamforge-validate config.yaml
```

**Run locally:**
```bash
docker run --rm \
  -v $(pwd)/config.yaml:/app/config.yaml:ro \
  streamforge:1.0.0 \
  --config /app/config.yaml
```

### Export Metrics

**Scrape metrics:**
```bash
curl http://streamforge:8080/metrics
```

**Export to file:**
```bash
curl http://streamforge:8080/metrics > metrics.txt
```

**Query specific metric:**
```bash
curl -s http://streamforge:8080/metrics | grep consumer_lag
```

---

## Contact and Escalation

**On-call rotation:** See PagerDuty schedule

**Escalation path:**
1. On-call engineer (initial response)
2. Platform team lead (if unresolved in 30 minutes)
3. SRE manager (if critical and unresolved in 1 hour)

**Documentation:**
- [Troubleshooting Guide](TROUBLESHOOTING.md)
- [Architecture](ARCHITECTURE.md)
- [Performance Tuning](PERFORMANCE_TUNING_RESULTS.md)

**Support channels:**
- Slack: #streamforge-support
- Email: streamforge-oncall@example.com
- GitHub Issues: https://github.com/rahulbsw/streamforge/issues

---

**Document Version:** 1.0.0  
**Last Updated:** 2026-04-18
