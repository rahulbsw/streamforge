# StreamForge Troubleshooting Guide

**Version:** 1.0.0  
**Last Updated:** 2026-04-18

This guide covers common issues, symptoms, causes, and solutions for StreamForge operations.

---

## Table of Contents

1. [Quick Diagnosis](#quick-diagnosis)
2. [Startup Issues](#startup-issues)
3. [Performance Issues](#performance-issues)
4. [Data Issues](#data-issues)
5. [Connectivity Issues](#connectivity-issues)
6. [Resource Issues](#resource-issues)
7. [Configuration Issues](#configuration-issues)
8. [Kafka Issues](#kafka-issues)
9. [Debug Commands](#debug-commands)

---

## Quick Diagnosis

### Health Check Commands

```bash
# 1. Check pod status
kubectl get pods -n streamforge

# 2. Check logs
kubectl logs -f deployment/streamforge -n streamforge --tail=50

# 3. Check metrics
curl http://streamforge:8080/metrics | grep -E "(up|error|lag)"

# 4. Check consumer group
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group <appid>

# 5. Check resource usage
kubectl top pods -n streamforge
```

### Common Symptoms Quick Reference

| Symptom | Likely Cause | Quick Fix |
|---------|--------------|-----------|
| Pod stuck in `CrashLoopBackOff` | Config error or missing secret | Check logs, validate config |
| Consumer lag growing | Insufficient replicas or CPU | Scale up |
| High error rate | Bad data or config mismatch | Check DLQ headers |
| Zero throughput | Kafka connection failure | Check connectivity |
| High memory usage | Large messages or memory leak | Increase limits, restart |
| Slow processing | Complex filters or transforms | Optimize DSL, add threads |

---

## Startup Issues

### Issue: Pod stuck in `Pending`

**Symptoms:**
```
NAME                          READY   STATUS    RESTARTS   AGE
streamforge-7c8f9d4b6-abc12   0/1     Pending   0          5m
```

**Diagnosis:**
```bash
kubectl describe pod streamforge-7c8f9d4b6-abc12 -n streamforge
```

**Common causes:**

**1. Insufficient resources:**
```
Events:
  Warning  FailedScheduling  5m  default-scheduler  0/3 nodes are available: insufficient cpu.
```
**Solution:**
- Reduce resource requests
- Add more nodes to cluster
- Remove resource limits temporarily

```bash
kubectl patch deployment streamforge -n streamforge --patch '
spec:
  template:
    spec:
      containers:
      - name: streamforge
        resources:
          requests:
            cpu: 500m
            memory: 1Gi
'
```

**2. ImagePullBackOff:**
```
Events:
  Warning  Failed  5m  kubelet  Failed to pull image "streamforge:1.0.0": rpc error: code = Unknown
```
**Solution:**
- Check image exists: `docker pull streamforge:1.0.0`
- Check image pull secret: `kubectl get secret -n streamforge`
- Use correct image registry

**3. PVC not bound:**
```
Events:
  Warning  FailedMount  5m  kubelet  Unable to attach or mount volumes
```
**Solution:**
- Check PVC status: `kubectl get pvc -n streamforge`
- Create missing PV/PVC
- Remove volume if not needed

### Issue: Pod stuck in `Init:Error`

**Symptoms:**
```
NAME                          READY   STATUS       RESTARTS   AGE
streamforge-7c8f9d4b6-abc12   0/1     Init:Error   0          2m
```

**Diagnosis:**
```bash
kubectl logs streamforge-7c8f9d4b6-abc12 -c init-container -n streamforge
kubectl describe pod streamforge-7c8f9d4b6-abc12 -n streamforge
```

**Common causes:**

**Init container failed:**
- Check init container logs
- Verify dependencies (e.g., Kafka must be reachable)
- Fix init script

**Solution:**
Remove init container if not essential, or fix the init logic.

### Issue: Pod `CrashLoopBackOff`

**Symptoms:**
```
NAME                          READY   STATUS             RESTARTS   AGE
streamforge-7c8f9d4b6-abc12   0/1     CrashLoopBackOff   5          5m
```

**Diagnosis:**
```bash
# Check current logs
kubectl logs streamforge-7c8f9d4b6-abc12 -n streamforge

# Check previous crashed instance
kubectl logs streamforge-7c8f9d4b6-abc12 -n streamforge --previous
```

**Common causes:**

**1. Config parse error:**
```
ERROR Failed to parse config: invalid YAML at line 10
```
**Solution:**
```bash
# Validate config
streamforge-validate config.yaml

# Fix ConfigMap
kubectl edit configmap streamforge-config -n streamforge

# Restart
kubectl rollout restart deployment/streamforge -n streamforge
```

**2. Missing environment variable:**
```
ERROR Environment variable KAFKA_BOOTSTRAP not set
```
**Solution:**
```bash
kubectl set env deployment/streamforge KAFKA_BOOTSTRAP=kafka:9092 -n streamforge
```

**3. Kafka connection failure:**
```
ERROR Failed to connect to Kafka broker at kafka:9092: Connection refused
```
**Solution:**
- Check Kafka is running
- Verify bootstrap servers in config
- Check network policies
- Test connectivity: `kubectl exec -it streamforge-xxx -n streamforge -- ping kafka`

**4. OOMKilled (Out of Memory):**
```bash
kubectl describe pod streamforge-xxx -n streamforge
```
```
Last State:     Terminated
  Reason:       OOMKilled
  Exit Code:    137
```
**Solution:**
```bash
# Increase memory limits
kubectl patch deployment streamforge -n streamforge --patch '
spec:
  template:
    spec:
      containers:
      - name: streamforge
        resources:
          limits:
            memory: 4Gi
'
```

### Issue: Container exits immediately with code 1

**Diagnosis:**
```bash
kubectl logs streamforge-xxx -n streamforge --previous
```

**Common causes:**

**Invalid filter/transform syntax:**
```
ERROR Failed to parse filter: /status,==,active,extra-arg
```
**Solution:**
- Fix DSL syntax
- Use `streamforge-validate` to check config
- Review docs/DSL_SPEC.md for correct syntax

**Permission denied (TLS certs):**
```
ERROR Failed to read TLS certificate: Permission denied
```
**Solution:**
```bash
# Check file permissions in secret
kubectl describe secret kafka-tls -n streamforge

# Ensure securityContext allows reading
kubectl patch deployment streamforge -n streamforge --patch '
spec:
  template:
    spec:
      securityContext:
        fsGroup: 1000
'
```

---

## Performance Issues

### Issue: High Consumer Lag

**Symptoms:**
- Lag > 10000 messages
- Lag growing over time
- Alert: "StreamForgeHighLag"

**Diagnosis:**
```bash
# Check lag
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group <appid>

# Check throughput
curl http://streamforge:8080/metrics | grep messages_consumed_total

# Check CPU usage
kubectl top pods -n streamforge
```

**Common causes:**

**1. Insufficient parallelism (too few replicas):**
```
Partitions: 16
Replicas: 2
Result: 14 partitions idle, only 2 being consumed
```
**Solution:**
```bash
kubectl scale deployment streamforge --replicas=8 -n streamforge
```

**2. CPU saturation:**
```
CPU: 1900m/2000m (95%)
```
**Solution:**
```bash
# Increase CPU limits
kubectl patch deployment streamforge -n streamforge --patch '
spec:
  template:
    spec:
      containers:
      - name: streamforge
        resources:
          limits:
            cpu: 4000m
'
```

**3. Complex filters/transforms:**
```
Filter: REGEX:/email,.*@[a-z]+\.[a-z]{2,}$
Transform: CONSTRUCT with 20 fields
```
**Solution:**
- Simplify filters (use KEY_PREFIX instead of REGEX)
- Move complex logic upstream
- Increase threads:
```yaml
threads: 8  # increase from 4
```

**4. Kafka broker slow:**
```
Fetch wait time: 500ms (high)
```
**Solution:**
- Scale Kafka brokers
- Add partitions to topic
- Tune `fetch_max_wait_ms`:
```yaml
performance:
  fetch_max_wait_ms: 50  # reduce wait time
```

### Issue: High Processing Latency

**Symptoms:**
- p95 latency > 100ms (was 20ms)
- Slow end-to-end processing

**Diagnosis:**
```bash
curl http://streamforge:8080/metrics | grep processing_duration
```

**Common causes:**

**1. Large batch sizes:**
```yaml
performance:
  batch_size: 10000  # too large
```
**Solution:**
```yaml
performance:
  batch_size: 500  # smaller for lower latency
  linger_ms: 0     # send immediately
```

**2. Slow producer (destination Kafka slow):**
```
ERROR Producer timeout after 30000ms
```
**Solution:**
- Check destination Kafka health
- Increase producer timeout
- Use async producer (default)

**3. Commit overhead:**
```yaml
commit_strategy: "per-message"  # high overhead
```
**Solution:**
```yaml
commit_strategy: "time-based"
commit_interval_ms: 1000  # commit every 1 second
```

### Issue: Low Throughput

**Symptoms:**
- Throughput < 10K msg/s (expected 50K msg/s)
- CPU usage low (< 30%)

**Diagnosis:**
```bash
# Check threading
kubectl logs deployment/streamforge -n streamforge | grep "threads"

# Check batch sizes
kubectl logs deployment/streamforge -n streamforge | grep "batch"
```

**Common causes:**

**1. Too few threads:**
```yaml
threads: 1  # only using 1 CPU core
```
**Solution:**
```yaml
threads: 8  # match CPU cores
```

**2. Small batches:**
```yaml
performance:
  fetch_min_bytes: 1       # wait for 1 byte
  batch_size: 10           # small producer batches
```
**Solution:**
```yaml
performance:
  fetch_min_bytes: 10240   # 10 KB
  batch_size: 5000         # large batches
  linger_ms: 50            # allow batching
```

**3. Too many commits:**
```yaml
commit_strategy: "per-message"
```
**Solution:**
```yaml
commit_strategy: "manual"
commit_interval_ms: 5000  # commit every 5 seconds
```

**4. Compression overhead:**
```yaml
performance:
  compression: "gzip"  # slow
```
**Solution:**
```yaml
performance:
  compression: "zstd"  # faster
  # or
  compression: "none"  # no compression overhead
```

---

## Data Issues

### Issue: Messages going to DLQ

**Symptoms:**
- DLQ accumulating messages
- Error rate > 1/s
- Alert: "StreamForgeHighDLQRate"

**Diagnosis:**
```bash
# Sample DLQ messages
kafka-console-consumer --bootstrap-server kafka:9092 \
  --topic streamforge-dlq \
  --property print.headers=true \
  --max-messages 10

# Check error types
kubectl logs deployment/streamforge -n streamforge | grep "Sending to DLQ"
```

**Common error types:**

**1. FilterEvaluation error:**
```
Headers:
  x-streamforge-error-type: FilterEvaluation
  x-streamforge-filter: /status,==,active
  x-streamforge-source-topic: users
```

**Cause:** Message missing `/status` field or field is not a string.

**Solution:**
```yaml
# Make filter more lenient
filter: "OR:/status,==,active:/status,==,null"

# Or skip messages without field
filter: "AND:EXISTS:/status:/status,==,active"
```

**2. TransformError:**
```
Headers:
  x-streamforge-error-type: TransformError
  x-streamforge-transform: /user/nonexistent
```

**Cause:** Transform path does not exist in message.

**Solution:**
```yaml
# Use default value
transform: "EXTRACT:/user/id,user-id,default-value"

# Or use CONSTRUCT with fallback
transform: "CONSTRUCT:id=/user/id:name=/user/name:fallback=unknown"
```

**3. SerializationError:**
```
Headers:
  x-streamforge-error-type: SerializationError
```

**Cause:** Transform produced invalid JSON.

**Solution:**
- Review transform logic
- Validate transform output
- Use simpler transform (e.g., /data instead of CONSTRUCT)

**4. ProducerTimeout:**
```
Headers:
  x-streamforge-error-type: RetryExhausted
  x-streamforge-retry-attempts: 3
```

**Cause:** Destination Kafka slow or unavailable, retries exhausted.

**Solution:**
- Check destination Kafka health
- Increase retry attempts:
```yaml
retry:
  max_attempts: 5
  max_delay_ms: 60000
```

### Issue: Missing Messages (Data Loss)

**Symptoms:**
- Messages consumed but not produced
- No DLQ entries
- No errors logged

**Diagnosis:**
```bash
# Check filter logic
kubectl logs deployment/streamforge -n streamforge | grep "filtered out"

# Check transform logic
kubectl logs deployment/streamforge -n streamforge | grep "transform result: null"

# Compare consume vs produce counts
curl http://streamforge:8080/metrics | grep messages_consumed_total
curl http://streamforge:8080/metrics | grep messages_produced_total
```

**Common causes:**

**1. Overly restrictive filter:**
```yaml
filter: "/status,==,active"
# If most messages have status != "active", they're filtered out
```

**Solution:**
- Review filter logic
- Check sample messages to verify filter correctness
- Add logging to see filtered messages:
```yaml
# In dev/staging, enable debug logging
env:
- name: RUST_LOG
  value: "streamforge=debug"
```

**2. Transform returns null:**
```yaml
transform: "/user/optional-field"
# If field doesn't exist, transform returns null, message skipped
```

**Solution:**
```yaml
transform: "EXTRACT:/user/optional-field,field,default-value"
```

**3. Partition mismatch:**
```yaml
partitioning: "field:/user/region"
# If /user/region doesn't exist, message sent to partition -1 (error)
```

**Solution:**
- Use default partitioning
- Or ensure partition key field always exists

### Issue: Duplicate Messages

**Symptoms:**
- Same message ID appears multiple times in destination
- At-least-once delivery expected but duplicates excessive

**Diagnosis:**
```bash
# Check consumer group stability
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group <appid>

# Check for rebalances
kubectl logs deployment/streamforge -n streamforge | grep "rebalance"
```

**Common causes:**

**1. Consumer group rebalancing:**
- Pod restarts trigger rebalance
- New replicas trigger rebalance
- Partitions redistributed, some messages re-consumed

**Solution:**
- Reduce pod churn (avoid frequent restarts)
- Use stable replica count
- Commit more frequently:
```yaml
commit_strategy: "time-based"
commit_interval_ms: 1000  # commit every 1 second
```

**2. Producer retries:**
- Producer sends message
- Kafka acknowledges
- Acknowledgment lost (network blip)
- Producer retries, message duplicated

**Solution:**
- This is expected with at-least-once semantics
- Use idempotent producer (enabled by default in rdkafka)
- Implement deduplication downstream (use message ID)

**3. Manual offset reset:**
- Offsets reset to earlier position
- Messages re-consumed

**Solution:**
- Avoid manual offset resets
- If needed, reset to specific timestamp, not "earliest"

---

## Connectivity Issues

### Issue: Cannot connect to Kafka

**Symptoms:**
```
ERROR Failed to connect to Kafka broker: Connection refused
```

**Diagnosis:**
```bash
# Test connectivity from pod
kubectl exec -it streamforge-xxx -n streamforge -- nc -zv kafka 9092

# Check DNS resolution
kubectl exec -it streamforge-xxx -n streamforge -- nslookup kafka

# Check network policies
kubectl get networkpolicy -n streamforge
```

**Common causes:**

**1. Wrong bootstrap servers:**
```yaml
bootstrap: "kafka:9092"  # but Kafka is at kafka.kafka.svc:9092
```

**Solution:**
```yaml
bootstrap: "kafka.kafka.svc.cluster.local:9092"
```

**2. Network policy blocking traffic:**
```bash
kubectl describe networkpolicy -n streamforge
```

**Solution:**
- Add egress rule for Kafka:
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
  - Egress
  egress:
  - to:
    - namespaceSelector:
        matchLabels:
          name: kafka
    ports:
    - protocol: TCP
      port: 9092
```

**3. Kafka not running:**
```bash
kubectl get pods -n kafka
```

**Solution:**
- Start Kafka cluster
- Wait for Kafka to be ready

**4. TLS certificate error:**
```
ERROR SSL handshake failed: certificate verify failed
```

**Solution:**
- Check TLS config:
```yaml
kafka:
  ssl:
    ca_location: "/certs/ca.crt"  # must exist
```
- Verify secret mounted:
```bash
kubectl exec -it streamforge-xxx -n streamforge -- ls -la /certs
```
- Check certificate validity:
```bash
kubectl exec -it streamforge-xxx -n streamforge -- openssl x509 -in /certs/ca.crt -noout -dates
```

### Issue: SASL authentication failure

**Symptoms:**
```
ERROR SASL authentication failed: Invalid credentials
```

**Diagnosis:**
```bash
# Check SASL config
kubectl get configmap streamforge-config -n streamforge -o yaml

# Check credentials
kubectl get secret kafka-credentials -n streamforge -o yaml
```

**Common causes:**

**1. Wrong SASL mechanism:**
```yaml
kafka:
  security:
    sasl_mechanism: "PLAIN"  # but Kafka uses SCRAM-SHA-512
```

**Solution:**
```yaml
kafka:
  security:
    sasl_mechanism: "SCRAM-SHA-512"
```

**2. Incorrect username/password:**
```yaml
sasl_username: "${KAFKA_USER}"  # env var not set
```

**Solution:**
```bash
kubectl set env deployment/streamforge KAFKA_USER=myuser KAFKA_PASSWORD=mypass -n streamforge
```

**3. Secret not mounted:**
```bash
kubectl exec -it streamforge-xxx -n streamforge -- env | grep KAFKA
```

**Solution:**
```yaml
spec:
  containers:
  - name: streamforge
    envFrom:
    - secretRef:
        name: kafka-credentials
```

---

## Resource Issues

### Issue: Out of Memory (OOMKilled)

**Symptoms:**
```
Last State: Terminated
  Reason: OOMKilled
  Exit Code: 137
```

**Diagnosis:**
```bash
kubectl describe pod streamforge-xxx -n streamforge
kubectl top pod streamforge-xxx -n streamforge
```

**Common causes:**

**1. Memory limit too low:**
```yaml
resources:
  limits:
    memory: 512Mi  # too small
```

**Solution:**
```yaml
resources:
  limits:
    memory: 4Gi
```

**2. Large messages:**
```
Average message size: 10 MB
Batch size: 1000
Total: 10 GB in memory
```

**Solution:**
```yaml
performance:
  batch_size: 100  # reduce batch size
  fetch_max_bytes: 10485760  # 10 MB limit
```

**3. Memory leak (rare):**
- Memory usage grows over time
- Not correlated with load

**Solution:**
- Restart pods periodically
- Report issue to StreamForge GitHub

### Issue: CPU Throttling

**Symptoms:**
- CPU usage at limit (100%)
- Slow processing despite high CPU request

**Diagnosis:**
```bash
kubectl top pods -n streamforge

# Check throttling
kubectl exec -it streamforge-xxx -n streamforge -- cat /sys/fs/cgroup/cpu/cpu.stat
```

**Common causes:**

**1. CPU limit too low:**
```yaml
resources:
  limits:
    cpu: 1000m  # 1 core, but workload needs 4
```

**Solution:**
```yaml
resources:
  limits:
    cpu: 4000m
```

**2. Set requests == limits (guaranteed QoS):**
```yaml
resources:
  requests:
    cpu: 2000m
  limits:
    cpu: 4000m  # can throttle
```

**Solution:**
```yaml
resources:
  requests:
    cpu: 2000m
  limits:
    cpu: 2000m  # guaranteed, no throttling
```

### Issue: Disk Space Full

**Symptoms:**
```
ERROR Failed to write log: No space left on device
```

**Diagnosis:**
```bash
kubectl exec -it streamforge-xxx -n streamforge -- df -h
```

**Common causes:**

**1. Excessive logging:**
```yaml
env:
- name: RUST_LOG
  value: "debug"  # too verbose
```

**Solution:**
```yaml
env:
- name: RUST_LOG
  value: "info"
```

**2. DLQ messages accumulating locally (if local DLQ):**

**Solution:**
- Send DLQ to Kafka topic (default)
- Increase volume size

**3. Persistent volume full:**
```bash
kubectl get pvc -n streamforge
```

**Solution:**
- Increase PVC size (if storage class supports expansion)
- Clean up old data

---

## Configuration Issues

### Issue: Invalid DSL Syntax

**Symptoms:**
```
ERROR Failed to parse filter: unexpected token at position 10
```

**Diagnosis:**
```bash
streamforge-validate config.yaml
```

**Common syntax errors:**

**1. Missing colon:**
```yaml
filter: "AND/status,==,active/age,>,18"  # wrong
filter: "AND:/status,==,active:/age,>,18"  # correct
```

**2. Unescaped special characters:**
```yaml
filter: 'REGEX:/email,.*@.*\.com'  # wrong (. not escaped)
filter: 'REGEX:/email,.*@.*\\.com'  # correct
```

**3. Wrong operator:**
```yaml
filter: "/age,>=,18"  # wrong (>= not supported)
filter: "/age,>,17"   # correct (use > instead)
```

**4. Mismatched quotes:**
```yaml
filter: "REGEX:/name,^(John|Jane)"  # wrong (unclosed parenthesis)
filter: 'REGEX:/name,^(John|Jane)$'  # correct
```

**Solution:**
- Use `streamforge-validate` before deploying
- Review docs/DSL_SPEC.md for syntax
- Test config locally first

### Issue: Deprecated Syntax Warning

**Symptoms:**
```
WARNING Deprecated syntax: KEY_SUFFIX is deprecated, use KEY_MATCHES instead
```

**Diagnosis:**
```bash
streamforge-validate config.yaml
```

**Solution:**
```yaml
# Old (deprecated)
filter: "KEY_SUFFIX:-prod"

# New
filter: 'KEY_MATCHES:.*-prod$'
```

**Migration guide:** docs/DSL_SPEC.md (Backward Compatibility section)

### Issue: Config not reloading

**Symptoms:**
- Updated ConfigMap
- Pods still using old config

**Diagnosis:**
```bash
kubectl get configmap streamforge-config -n streamforge -o yaml
kubectl exec -it streamforge-xxx -n streamforge -- cat /app/config.yaml
```

**Causes:**

**1. ConfigMap not propagated:**
- Kubernetes propagates ConfigMap updates eventually (up to 60 seconds)

**Solution:**
```bash
# Force restart
kubectl rollout restart deployment/streamforge -n streamforge
```

**2. Hot-reload not enabled:**
- StreamForge requires restart for config changes

**Solution:**
- Always restart after ConfigMap update

---

## Kafka Issues

### Issue: Consumer group lag not decreasing

**Symptoms:**
- StreamForge running, no errors
- Lag stays at 10000, not decreasing

**Diagnosis:**
```bash
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group <appid>
```

**Common causes:**

**1. More replicas than partitions:**
```
Partitions: 4
Replicas: 8
Result: 4 replicas consume, 4 are idle
```

**Solution:**
- Scale replicas to match partitions: `kubectl scale deployment streamforge --replicas=4`
- Or add more partitions: `kafka-topics --alter --partitions 8`

**2. Consumer group rebalancing:**
```
Consumer rebalancing...
```
**Solution:**
- Wait for rebalance to complete (30-60 seconds)
- Reduce pod churn

**3. Kafka brokers overloaded:**
```
Fetch latency: 5000ms
```
**Solution:**
- Scale Kafka brokers
- Tune Kafka performance

### Issue: Topic does not exist

**Symptoms:**
```
ERROR Topic 'nonexistent-topic' does not exist
```

**Diagnosis:**
```bash
kafka-topics --bootstrap-server kafka:9092 --list
```

**Solution:**

**Option 1: Create topic**
```bash
kafka-topics --bootstrap-server kafka:9092 \
  --create --topic output-topic \
  --partitions 16 \
  --replication-factor 3
```

**Option 2: Enable auto-create**
```yaml
# Kafka broker config
auto.create.topics.enable=true
```

**Option 3: Fix topic name in config**
```yaml
routing:
  destinations:
    - output: "output-topic"  # ensure spelling is correct
```

### Issue: Partition count mismatch

**Symptoms:**
- Some partitions have high lag
- Others have zero lag
- Unbalanced consumption

**Diagnosis:**
```bash
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group <appid>
```

**Cause:**
- Producer uses key-based partitioning
- Keys are skewed (e.g., 80% have key "default")
- Most messages go to one partition

**Solution:**

**Option 1: Use random partitioning**
```yaml
partitioning: "random"
```

**Option 2: Use field-based partitioning with uniform distribution**
```yaml
partitioning: "field:/user/id"  # if user IDs are uniformly distributed
```

**Option 3: Add more partitions**
```bash
kafka-topics --bootstrap-server kafka:9092 \
  --alter --topic source-topic \
  --partitions 32
```

---

## Debug Commands

### Enable Debug Logging

**Temporarily (current pod):**
```bash
kubectl exec -it streamforge-xxx -n streamforge -- kill -USR1 1
# Toggles debug logging for duration of pod lifetime
```

**Permanently (all pods):**
```bash
kubectl set env deployment/streamforge RUST_LOG=streamforge=debug -n streamforge
```

**Restore info logging:**
```bash
kubectl set env deployment/streamforge RUST_LOG=streamforge=info -n streamforge
```

### Inspect Message Contents

**Sample source topic:**
```bash
kafka-console-consumer --bootstrap-server kafka:9092 \
  --topic source-topic \
  --property print.key=true \
  --property print.headers=true \
  --property print.timestamp=true \
  --max-messages 10
```

**Sample destination topic:**
```bash
kafka-console-consumer --bootstrap-server kafka:9092 \
  --topic dest-topic \
  --property print.key=true \
  --max-messages 10
```

**Sample DLQ:**
```bash
kafka-console-consumer --bootstrap-server kafka:9092 \
  --topic streamforge-dlq \
  --property print.headers=true \
  --max-messages 10
```

### Profile Performance

**CPU profiling:**
```bash
kubectl exec -it streamforge-xxx -n streamforge -- kill -SIGUSR2 1
# Outputs CPU profile to /tmp/cpu-profile.txt
kubectl cp streamforge-xxx:/tmp/cpu-profile.txt ./cpu-profile.txt -n streamforge
```

**Memory profiling:**
```bash
kubectl exec -it streamforge-xxx -n streamforge -- cat /proc/$(pgrep streamforge)/status
```

### Test Filters/Transforms Locally

**Test config:**
```bash
# Use dry-run mode (if available)
docker run --rm \
  -v $(pwd)/config.yaml:/app/config.yaml:ro \
  streamforge:1.0.0 \
  --config /app/config.yaml \
  --dry-run
```

**Validate config:**
```bash
streamforge-validate config.yaml --verbose
```

### Force Consumer Rebalance

**Restart single pod:**
```bash
kubectl delete pod streamforge-xxx -n streamforge
```

**Restart all pods:**
```bash
kubectl rollout restart deployment/streamforge -n streamforge
```

**Force rebalance by changing group ID:**
```yaml
appid: "streamforge-prod-v2"  # new group ID
offset: "latest"  # start from latest to avoid reprocessing
```

### Check Kafka Broker Health

**Broker API versions:**
```bash
kafka-broker-api-versions --bootstrap-server kafka:9092
```

**Topic metadata:**
```bash
kafka-topics --bootstrap-server kafka:9092 \
  --describe --topic source-topic
```

**Consumer group state:**
```bash
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group <appid> \
  --state
```

### Capture Metrics Snapshot

**Export all metrics:**
```bash
curl http://streamforge:8080/metrics > metrics-$(date +%s).txt
```

**Query specific metrics:**
```bash
curl -s http://streamforge:8080/metrics | grep -E "(lag|error|duration)"
```

---

## Getting Help

### Check Documentation

- [DSL Specification](DSL_SPEC.md) - Filter/transform syntax
- [Deployment Guide](DEPLOYMENT.md) - Deployment options
- [Operations Guide](OPERATIONS.md) - Day-to-day operations
- [Architecture](ARCHITECTURE.md) - System design

### Enable Verbose Logging

```yaml
env:
- name: RUST_LOG
  value: "streamforge=debug,rdkafka=info"
- name: RUST_BACKTRACE
  value: "full"
```

### Collect Diagnostic Bundle

```bash
#!/bin/bash
# collect-diagnostics.sh

mkdir -p diagnostics/$(date +%Y-%m-%d)
cd diagnostics/$(date +%Y-%m-%d)

# Pod status
kubectl get pods -n streamforge -o wide > pods.txt

# Logs
kubectl logs deployment/streamforge -n streamforge --tail=1000 > logs.txt

# Config
kubectl get configmap streamforge-config -n streamforge -o yaml > config.yaml

# Metrics
curl http://streamforge:8080/metrics > metrics.txt

# Consumer group
kafka-consumer-groups --bootstrap-server kafka:9092 \
  --describe --group <appid> > consumer-group.txt

# Events
kubectl get events -n streamforge --sort-by='.lastTimestamp' > events.txt

# Resource usage
kubectl top pods -n streamforge > resources.txt

echo "Diagnostics collected in diagnostics/$(date +%Y-%m-%d)/"
```

### Report Issues

**GitHub Issues:** https://github.com/rahulbsw/streamforge/issues

**Include:**
- StreamForge version
- Kubernetes version
- Kafka version
- Config file (redact sensitive data)
- Logs (last 100 lines)
- Error messages
- Steps to reproduce

---

## Issue Decision Tree

```
Is StreamForge running?
  ├─ No → Check startup issues
  │        └─ CrashLoopBackOff? → Check logs for config errors
  │        └─ Pending? → Check resource availability
  │        └─ ImagePullBackOff? → Check image registry
  │
  └─ Yes → Check metrics
           ├─ High lag? → Check performance issues
           │             └─ CPU high? → Scale up or add threads
           │             └─ CPU low? → Increase batch sizes
           │
           ├─ High errors? → Check DLQ headers
           │               └─ FilterEvaluation? → Fix filter logic
           │               └─ ProducerTimeout? → Check destination Kafka
           │
           ├─ Zero throughput? → Check connectivity
           │                    └─ Kafka connection error? → Check network
           │                    └─ SASL error? → Check credentials
           │
           └─ Duplicates? → Check commit strategy
                           └─ Frequent rebalances? → Reduce pod churn
                           └─ Manual offset reset? → Avoid resets
```

---

**Document Version:** 1.0.0  
**Last Updated:** 2026-04-18  
**Feedback:** https://github.com/rahulbsw/streamforge/issues
