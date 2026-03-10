# Scaling Guide

Complete guide to scaling WAP MirrorMaker for high-throughput production deployments.

## Table of Contents

- [Scaling Fundamentals](#scaling-fundamentals)
- [Horizontal Scaling](#horizontal-scaling)
- [Vertical Scaling](#vertical-scaling)
- [Kafka Partition Strategy](#kafka-partition-strategy)
- [Consumer Group Coordination](#consumer-group-coordination)
- [Load Balancing](#load-balancing)
- [Scaling Patterns](#scaling-patterns)
- [Monitoring and Tuning](#monitoring-and-tuning)
- [Best Practices](#best-practices)

## Scaling Fundamentals

### Understanding the Architecture

```
┌────────────────────────────────────────────────────────────┐
│                    Source Kafka Cluster                     │
│  Topic: events (10 partitions, 100K msg/s)                 │
└─────────────────┬──────────────────────────────────────────┘
                  │
    ┌─────────────┼─────────────┐
    │             │             │
    ▼             ▼             ▼
┌─────────┐ ┌─────────┐ ┌─────────┐
│Instance1│ │Instance2│ │Instance3│  Consumer Group: "wap-mirrormaker"
│P0,P1,P2 │ │P3,P4,P5 │ │P6,P7,P8 │  Each gets partitions
└────┬────┘ └────┬────┘ └────┬────┘
     │           │           │
     └───────────┼───────────┘
                 │
                 ▼
┌────────────────────────────────────────────────────────────┐
│                   Target Kafka Cluster                      │
│  Multiple topics (filtered/transformed)                     │
└────────────────────────────────────────────────────────────┘
```

### Key Concepts

**Partition-Based Parallelism:**
- Kafka partitions are the unit of parallelism
- Each instance consumes specific partitions
- Cannot have more consumers than partitions

**Consumer Group:**
- All instances share the same `appid` (consumer group ID)
- Kafka automatically assigns partitions to instances
- Rebalancing happens when instances join/leave

**Throughput Formula:**
```
Total Throughput = (Partitions × Per-Partition Throughput) / Replication Factor
Instance Throughput = Total Throughput / Number of Instances
```

## Horizontal Scaling

### Adding More Instances

**When to Scale Horizontally:**
- Consumer lag increasing
- CPU usage > 80% across instances
- Want higher availability
- Input topic has many partitions

**Maximum Instances:**
```
Max Instances = Number of Source Topic Partitions
```

**Example: 10-partition topic**
- 1 instance: Consumes all 10 partitions
- 5 instances: Each consumes 2 partitions
- 10 instances: Each consumes 1 partition
- 11+ instances: Some instances idle (wasted resources)

### Configuration for Horizontal Scaling

**Same config for all instances:**

```json
{
  "appid": "wap-mirrormaker",
  "bootstrap": "kafka:9092",
  "input": "events",
  "output": "events-mirror",
  "threads": 4
}
```

**Key points:**
- ✅ Same `appid` (consumer group)
- ✅ Same topic configuration
- ✅ Same filter/transform logic
- ✅ Kafka handles partition assignment

### Deployment Strategies

#### Docker Compose

```yaml
version: '3.8'
services:
  mirrormaker:
    image: wap-mirrormaker-rust:latest
    deploy:
      replicas: 5  # Scale to 5 instances
    environment:
      - CONFIG_FILE=/app/config/config.json
    volumes:
      - ./config.json:/app/config/config.json:ro
```

Scale dynamically:
```bash
docker-compose up -d --scale mirrormaker=5
```

#### Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: wap-mirrormaker
spec:
  replicas: 5  # Number of instances
  selector:
    matchLabels:
      app: wap-mirrormaker
  template:
    metadata:
      labels:
        app: wap-mirrormaker
    spec:
      containers:
      - name: mirrormaker
        image: wap-mirrormaker-rust:latest
        resources:
          requests:
            memory: "256Mi"
            cpu: "1000m"
          limits:
            memory: "512Mi"
            cpu: "2000m"
        env:
        - name: CONFIG_FILE
          value: /app/config/config.json
        volumeMounts:
        - name: config
          mountPath: /app/config
          readOnly: true
      volumes:
      - name: config
        configMap:
          name: mirrormaker-config
```

Scale with kubectl:
```bash
kubectl scale deployment wap-mirrormaker --replicas=10
```

#### Horizontal Pod Autoscaler (HPA)

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: wap-mirrormaker-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: wap-mirrormaker
  minReplicas: 3
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
```

Auto-scales based on CPU/memory usage.

### Consumer Group Rebalancing

**What happens when scaling:**

1. **New instance joins:**
   ```
   Before: Instance1 (P0-P9)
   Add Instance2
   Rebalance...
   After: Instance1 (P0-P4), Instance2 (P5-P9)
   ```

2. **Instance removed:**
   ```
   Before: Inst1 (P0-P4), Inst2 (P5-P9)
   Remove Instance2
   Rebalance...
   After: Instance1 (P0-P9)
   ```

**Rebalance Settings:**

```json
{
  "consumer_properties": {
    "session.timeout.ms": "30000",
    "heartbeat.interval.ms": "3000",
    "max.poll.interval.ms": "300000"
  }
}
```

**During rebalancing:**
- Processing pauses briefly (1-5 seconds)
- Partitions reassigned
- Consumer lag may spike temporarily
- No message loss (Kafka handles offsets)

## Vertical Scaling

### Adding More Resources Per Instance

**When to Scale Vertically:**
- CPU bottleneck (>90% usage)
- Memory pressure
- Simple scaling without coordination
- Fewer than max partitions

### Thread Scaling

**Configuration:**

```json
{
  "threads": 8
}
```

**Guidelines:**
```
Threads = CPU Cores × 1-2

Examples:
- 2 cores → threads: 2-4
- 4 cores → threads: 4-8
- 8 cores → threads: 8-16
```

**Impact:**
- More threads = Higher throughput
- Too many threads = CPU thrashing
- Monitor CPU usage to find optimal

### Memory Scaling

**Configuration:**

```json
{
  "consumer_properties": {
    "fetch.max.bytes": "52428800",
    "max.poll.records": "1000"
  },
  "producer_properties": {
    "buffer.memory": "67108864",
    "batch.size": "131072"
  }
}
```

**Memory Sizing:**
```
Base Memory: ~50MB
Per Thread: +10-20MB
Buffer Memory: (producer.buffer.memory / 1MB)
Total ≈ 50 + (threads × 15) + (buffer / 1MB)

Example (8 threads, 64MB buffer):
Memory = 50 + (8 × 15) + 64 = 234MB
Allocate 512MB for safety
```

### CPU Allocation

**Docker:**
```bash
docker run \
  --cpus="4.0" \
  --memory="1g" \
  wap-mirrormaker-rust
```

**Kubernetes:**
```yaml
resources:
  requests:
    cpu: "2000m"      # 2 cores guaranteed
    memory: "512Mi"
  limits:
    cpu: "4000m"      # Can burst to 4 cores
    memory: "1Gi"
```

## Kafka Partition Strategy

### Optimal Partition Count

**Formula:**
```
Partitions = (Target Throughput × Replication) / Per-Partition Throughput

Example:
Target: 100K msg/s
Per-Partition: 10K msg/s
Replication: 3x
Partitions = (100K × 3) / 10K = 30 partitions
```

**Recommendations:**
- Minimum: 3 partitions (basic parallelism)
- Good: 10-20 partitions (room to scale)
- High throughput: 50-100 partitions
- Maximum: 1000s (diminishing returns)

### Partition Key Strategy

**Important for ordering:**

```json
{
  "partition": "/userId"
}
```

**Effects:**
- Same key → Same partition → Same consumer → Ordering preserved
- Different keys → Different partitions → Parallel processing

**Example:**
```
User123 messages → Partition 5 → Instance 2
User456 messages → Partition 8 → Instance 3
```

### Rebalancing Impact

**Adding partitions (requires restart):**

```bash
# Add partitions to topic
kafka-topics.sh --alter \
  --topic events \
  --partitions 20 \
  --bootstrap-server kafka:9092

# Restart consumers to pick up new partitions
kubectl rollout restart deployment wap-mirrormaker
```

**Note:** Cannot reduce partition count (Kafka limitation).

## Consumer Group Coordination

### Consumer Group Settings

```json
{
  "appid": "wap-mirrormaker",
  "consumer_properties": {
    "group.id": "wap-mirrormaker",
    "enable.auto.commit": "false",
    "auto.offset.reset": "latest",
    "session.timeout.ms": "30000",
    "heartbeat.interval.ms": "3000",
    "max.poll.interval.ms": "300000"
  }
}
```

### Multiple Consumer Groups

**Use Case: Different Processing Logic**

```
Group 1 (appid: "mirrormaker-archive")
  - All messages to archive

Group 2 (appid: "mirrormaker-realtime")
  - Filtered messages to real-time topics

Group 3 (appid: "mirrormaker-analytics")
  - Transformed messages to analytics
```

Each group processes independently with own offsets.

### Offset Management

**Automatic (Recommended):**
```json
{
  "consumer_properties": {
    "enable.auto.commit": "false"
  }
}
```

Application commits offsets after successful processing.

**Manual:**
Monitor lag:
```bash
kafka-consumer-groups.sh \
  --bootstrap-server kafka:9092 \
  --group wap-mirrormaker \
  --describe
```

## Load Balancing

### Kafka Native Load Balancing

Kafka automatically balances partitions:

```
10 partitions, 5 instances:
Instance 1: P0, P1
Instance 2: P2, P3
Instance 3: P4, P5
Instance 4: P6, P7
Instance 5: P8, P9
```

**Automatic rebalancing when:**
- Instance added
- Instance removed
- Instance fails
- Network partition

### Uneven Load Distribution

**Problem:**
```
Partition 0: 50K msg/s (hot partition)
Partition 1-9: 5K msg/s each
```

**Solution 1: Better Partition Keys**
```json
{
  "partition": "/userId"
}
```

Use high-cardinality fields to distribute load.

**Solution 2: Increase Partitions**
```bash
# More partitions = Better distribution
kafka-topics.sh --alter --topic events --partitions 20
```

**Solution 3: Custom Partitioning**
```rust
// Use hash of multiple fields
let partition_key = format!("{}-{}", user_id, timestamp % 1000);
```

## Scaling Patterns

### Pattern 1: Linear Scaling

**Start:** 1 instance, 10 partitions, 10K msg/s

**Scale:**
```
2 instances → 20K msg/s
5 instances → 50K msg/s
10 instances → 100K msg/s
```

**Configuration:**
- Same for all instances
- Let Kafka balance partitions
- Monitor throughput and CPU

### Pattern 2: Staged Scaling

**Progressive scaling with monitoring:**

```bash
# Stage 1: Start with 3 instances
kubectl scale deployment wap-mirrormaker --replicas=3

# Monitor for 30 minutes
# Check: CPU, memory, lag, throughput

# Stage 2: Scale to 5 instances
kubectl scale deployment wap-mirrormaker --replicas=5

# Monitor...

# Stage 3: Scale to 10 instances
kubectl scale deployment wap-mirrormaker --replicas=10
```

### Pattern 3: Auto-Scaling

**Based on consumer lag:**

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: mirrormaker-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: wap-mirrormaker
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: External
    external:
      metric:
        name: kafka_consumer_lag
        selector:
          matchLabels:
            topic: events
            group: wap-mirrormaker
      target:
        type: AverageValue
        averageValue: "1000"  # Scale when lag > 1000
```

### Pattern 4: Geographic Distribution

**Multi-region deployment:**

```
Region US-EAST:
  - 5 instances
  - Consume from local Kafka
  - Produce to central Kafka

Region US-WEST:
  - 5 instances
  - Consume from local Kafka
  - Produce to central Kafka

Region EU:
  - 5 instances
  - Consume from local Kafka
  - Produce to central Kafka
```

Each region processes independently, different consumer groups.

## Monitoring and Tuning

### Key Metrics

**Consumer Lag:**
```bash
kafka-consumer-groups.sh --describe --group wap-mirrormaker
```

Watch for:
- Lag increasing → Need more capacity
- Lag stable → Adequate capacity
- Lag decreasing → Catching up

**Application Metrics:**
```
Stats: processed=10000 (1000.0/s), filtered=100 (10.0/s),
       completed=9900 (990.0/s), errors=0 (0.0/s)
```

**Per Instance:**
- Throughput: 1000 msg/s → 10K msg/s typical range
- CPU: 50-80% optimal
- Memory: <80% of limit

### Scaling Triggers

**Scale UP when:**
- Consumer lag > 10K messages for 5+ minutes
- CPU > 80% sustained
- Memory > 80% sustained
- Throughput below target

**Scale DOWN when:**
- Consumer lag < 1K messages sustained
- CPU < 30% sustained
- Memory < 50% sustained
- Cost optimization needed

### Tuning After Scaling

**After adding instances:**

1. **Monitor rebalancing** (1-5 minutes)
2. **Verify partition distribution**
   ```bash
   kafka-consumer-groups.sh --describe --group wap-mirrormaker
   ```
3. **Check per-instance throughput**
4. **Adjust thread count if needed**

**Optimization loop:**
```
1. Scale horizontally (add instances)
2. Monitor for 30 minutes
3. Tune threads per instance
4. Monitor for 30 minutes
5. Adjust batch sizes if needed
6. Repeat until optimal
```

## Best Practices

### 1. Start Small, Scale Gradually

```bash
# Day 1: 2 instances
# Day 2: Monitor, maybe 3 instances
# Week 1: Stable at 5 instances
# Month 1: Auto-scaling with HPA
```

### 2. Match Partitions to Scale

```
Planning for 10 instances?
→ Create topic with 10-20 partitions

Planning for 100 instances?
→ Create topic with 100-200 partitions
```

### 3. Monitor Before Scaling

Don't scale blindly:
- Check consumer lag trend
- Verify actual bottleneck (CPU/memory/network)
- Review instance utilization
- Test with smaller scale first

### 4. Use Health Checks

```yaml
# Kubernetes
livenessProbe:
  exec:
    command: ["pgrep", "wap-mirrormaker-rust"]
  initialDelaySeconds: 10
  periodSeconds: 30

readinessProbe:
  exec:
    command: ["pgrep", "wap-mirrormaker-rust"]
  initialDelaySeconds: 5
  periodSeconds: 10
```

### 5. Plan for Failures

```
Total Capacity: 10 instances
Plan for: 2 instance failures
Deploy: 12 instances
Result: 83% utilization, 20% overhead
```

### 6. Use Resource Requests and Limits

```yaml
resources:
  requests:    # Guaranteed resources
    cpu: "1000m"
    memory: "256Mi"
  limits:      # Maximum resources
    cpu: "2000m"
    memory: "512Mi"
```

### 7. Network Optimization

**Same datacenter/region:**
- Lower latency
- Higher throughput
- Better reliability

**Cross-region:**
- Increase timeouts
- Enable compression
- Use dedicated network

### 8. Partition Strategy

**Good partition keys:**
- User ID (high cardinality)
- Order ID (unique)
- Device ID (distributed)

**Poor partition keys:**
- Country (low cardinality)
- Day of week (very low cardinality)
- Constant value (all to one partition)

## Scaling Examples

### Example 1: Small Deployment

**Scenario:**
- 1K msg/s throughput
- 5 partitions
- Single datacenter

**Configuration:**
```yaml
replicas: 2
resources:
  requests:
    cpu: "500m"
    memory: "256Mi"
  limits:
    cpu: "1000m"
    memory: "512Mi"
```

```json
{
  "threads": 2,
  "consumer_properties": {
    "fetch.min.bytes": "1048576"
  }
}
```

### Example 2: Medium Deployment

**Scenario:**
- 25K msg/s throughput
- 20 partitions
- Multi-az

**Configuration:**
```yaml
replicas: 5
resources:
  requests:
    cpu: "1000m"
    memory: "512Mi"
  limits:
    cpu: "2000m"
    memory: "1Gi"
```

```json
{
  "threads": 4,
  "consumer_properties": {
    "fetch.min.bytes": "1048576",
    "max.poll.records": "500"
  },
  "producer_properties": {
    "batch.size": "65536",
    "linger.ms": "10"
  }
}
```

### Example 3: Large Deployment

**Scenario:**
- 100K msg/s throughput
- 50 partitions
- Multi-region

**Configuration:**
```yaml
replicas: 20
resources:
  requests:
    cpu: "2000m"
    memory: "1Gi"
  limits:
    cpu: "4000m"
    memory: "2Gi"
```

```json
{
  "threads": 8,
  "consumer_properties": {
    "fetch.min.bytes": "2097152",
    "max.poll.records": "1000"
  },
  "producer_properties": {
    "batch.size": "131072",
    "linger.ms": "10",
    "compression.type": "snappy"
  }
}
```

## Troubleshooting Scaling Issues

### Issue: Instances Not Consuming

**Symptoms:**
- Some instances idle
- Uneven partition distribution

**Check:**
```bash
# Verify consumer group
kafka-consumer-groups.sh --describe --group wap-mirrormaker

# Check instance count vs partitions
kubectl get pods | grep mirrormaker | wc -l
```

**Solution:**
```bash
# Ensure instances < partitions
# Or add more partitions to topic
```

### Issue: Rebalancing Loops

**Symptoms:**
- Constant rebalancing
- High lag
- No progress

**Check:**
```bash
# Check logs for rebalance messages
kubectl logs -f deployment/wap-mirrormaker | grep rebalance
```

**Solution:**
```json
{
  "consumer_properties": {
    "session.timeout.ms": "45000",
    "max.poll.interval.ms": "600000"
  }
}
```

### Issue: Uneven Load

**Symptoms:**
- Some instances 100% CPU
- Other instances idle

**Solution:**
- Better partition keys
- Increase partition count
- Check for hot partitions

## Summary

### Quick Reference

| Throughput | Partitions | Instances | Threads | Memory |
|------------|------------|-----------|---------|--------|
| 1K msg/s   | 3-5        | 1-2       | 2       | 256Mi  |
| 10K msg/s  | 5-10       | 2-5       | 4       | 512Mi  |
| 25K msg/s  | 10-20      | 5-10      | 4       | 512Mi  |
| 50K msg/s  | 20-40      | 10-20     | 4-8     | 1Gi    |
| 100K msg/s | 50-100     | 20-50     | 8       | 1Gi    |

### Scaling Checklist

- [ ] Determine target throughput
- [ ] Calculate required partitions
- [ ] Start with 2-3 instances
- [ ] Monitor for 24 hours
- [ ] Scale gradually (2x at a time)
- [ ] Tune threads and batch sizes
- [ ] Set up auto-scaling
- [ ] Configure health checks
- [ ] Monitor consumer lag
- [ ] Plan for failures

### Next Steps

- [PERFORMANCE.md](PERFORMANCE.md) - Performance tuning
- [USAGE.md](USAGE.md) - Use cases and patterns
- [DOCKER.md](DOCKER.md) - Deployment options

---

**Remember:** Start small, monitor closely, scale gradually. It's easier to scale up than down!
