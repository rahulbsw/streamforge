# At-Least-Once Delivery & Cache Backends

This guide covers:
1. **At-least-once delivery** with configurable manual commits
2. **Redis cache backend** for distributed caching
3. **Kafka-backed cache** using compacted topics
4. **Multi-level caching** (local + Redis)

## Table of Contents

- [At-Least-Once Delivery](#at-least-once-delivery)
- [Cache Backends](#cache-backends)
- [How to Build/Populate Moka Cache](#how-to-buildpopulate-moka-cache)
- [Configuration Examples](#configuration-examples)
- [Best Practices](#best-practices)

---

## At-Least-Once Delivery

### Overview

StreamForge supports two delivery semantics:

| Mode | Commits | Guarantees | Use Case |
|------|---------|------------|----------|
| **At-most-once** | Auto-commit | May lose messages on crash | High throughput, losses acceptable |
| **At-least-once** | Manual commit | No losses, may duplicate | Critical data, idempotent processing |

### Configuration

```yaml
appid: at-least-once-example
bootstrap: kafka:9092
target_broker: kafka:9092
input: important-events
output: processed-events

# Commit strategy configuration
commit_strategy:
  # Enable manual commits for at-least-once
  manual_commit: true

  # Commit mode: async (faster) or sync (safer)
  commit_mode: async

  # Commit interval in milliseconds (batch commits)
  commit_interval_ms: 5000  # 5 seconds

  # Dead Letter Queue configuration
  enable_dlq: true
  dlq_topic: "failed-events-dlq"
  max_retries: 3

  # Retry backoff configuration
  retry_backoff:
    initial_backoff_ms: 100
    max_backoff_ms: 30000
    multiplier: 2.0
```

### Commit Modes

#### Async Commit (Default)

```yaml
commit_strategy:
  manual_commit: true
  commit_mode: async  # Fast, but may lose commits on crash
```

**Pros:**
- High throughput
- Low latency

**Cons:**
- May lose commits on crash
- Eventual consistency

**Use case:** High-volume pipelines where occasional duplicates are acceptable

#### Sync Commit

```yaml
commit_strategy:
  manual_commit: true
  commit_mode: sync  # Slower, but guaranteed
```

**Pros:**
- Guaranteed commits
- Strong consistency

**Cons:**
- Higher latency
- Lower throughput

**Use case:** Financial transactions, audit logs, critical data

### Dead Letter Queue (DLQ)

Failed messages (after retries) are sent to a DLQ:

```yaml
commit_strategy:
  enable_dlq: true
  dlq_topic: "my-pipeline-dlq"
  max_retries: 3
```

DLQ message format:
```json
{
  "original_topic": "input-topic",
  "original_partition": 0,
  "original_offset": 12345,
  "error": "Processing failed: invalid JSON",
  "retry_count": 3,
  "timestamp": 1234567890,
  "original_message": { ... }
}
```

### Retry Backoff

Exponential backoff between retries:

```yaml
retry_backoff:
  initial_backoff_ms: 100    # Start with 100ms
  max_backoff_ms: 30000      # Cap at 30 seconds
  multiplier: 2.0            # Double each retry
```

Retry delays:
- Retry 1: 100ms
- Retry 2: 200ms
- Retry 3: 400ms
- Retry 4: 800ms
- ...
- Max: 30,000ms

### Backward Compatibility

**Default behavior is unchanged** (auto-commit for backward compatibility):

```yaml
# This config continues to work as before
appid: my-pipeline
bootstrap: kafka:9092
input: events
output: processed

# No commit_strategy = auto-commit (at-most-once)
```

To enable at-least-once, explicitly set `manual_commit: true`.

---

## Cache Backends

StreamForge supports multiple cache backends:

| Backend | Use Case | Latency | Capacity | Shared |
|---------|----------|---------|----------|--------|
| **Local (Moka)** | Fast, single-node | 10-50ns | Limited by RAM | No |
| **Redis** | Distributed, persistent | 1-2ms | Large | Yes |
| **Kafka** | Event-sourced, durable | 5-10ms | Very large | Yes |
| **Multi-level** | Best of both | 10ns-2ms | Large | Yes |

### 1. Local Cache (Moka)

In-memory cache using Moka (default):

```yaml
cache:
  backend_type: local
  local:
    max_capacity: 10000
    ttl_seconds: 3600    # 1 hour
    tti_seconds: 600     # 10 minutes idle
```

**Pros:**
- Extremely fast (10-50ns)
- No external dependencies
- Simple setup

**Cons:**
- Not shared across instances
- Limited by RAM
- Lost on restart

**Use case:** Reference data, low cardinality lookups

### 2. Redis Cache

Distributed cache using Redis:

```yaml
cache:
  backend_type: redis
  redis:
    url: "redis://localhost:6379/0"
    pool_size: 10
    key_prefix: "streamforge"
    default_ttl_seconds: 3600
```

**Pros:**
- Shared across all instances
- Persistent (with AOF/RDB)
- Large capacity
- Pub/sub for invalidation

**Cons:**
- Network latency (1-2ms)
- Requires Redis server
- Additional ops complexity

**Use case:** High-cardinality data, multi-instance deployments

### 3. Kafka-Backed Cache

Use Kafka compacted topic as cache:

```yaml
cache:
  backend_type: kafka
  kafka:
    bootstrap: "kafka:9092"
    topic: "user-profiles-compacted"
    group_id: "streamforge-cache"
    key_field: "/userId"
    value_field: "."  # Entire message
    warmup_on_start: true
```

**Pros:**
- Event-sourced (full history)
- Extremely durable
- Scales horizontally
- Free with existing Kafka

**Cons:**
- Higher latency (5-10ms)
- Warmup time on start
- Requires compacted topic

**Use case:** Slowly changing dimension data, audit requirements

### 4. Multi-Level Cache

L1 (local) + L2 (Redis):

```yaml
cache:
  backend_type: multi
  local:
    max_capacity: 5000
    ttl_seconds: 300
  redis:
    url: "redis://localhost:6379/0"
    key_prefix: "streamforge"
    default_ttl_seconds: 3600
```

**Behavior:**
1. Check L1 (local) first - 10-50ns
2. On L1 miss, check L2 (Redis) - 1-2ms
3. On L2 hit, promote to L1
4. On L2 miss, cache miss

**Benefits:**
- Fast common-case (L1 hit)
- Large capacity (L2)
- Shared across instances
- Automatic promotion

---

## How to Build/Populate Moka Cache

### Method 1: Pre-populate on Startup

```rust
use streamforge::cache::{LookupCache, CacheConfig};
use serde_json::json;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let cache = Arc::new(LookupCache::new(CacheConfig::default()));

    // Load from database
    let users = load_users_from_db().await;
    for user in users {
        cache.put(
            format!("user:{}", user.id),
            json!({
                "name": user.name,
                "email": user.email,
                "tier": user.tier
            })
        ).await;
    }

    println!("Cache populated with {} users", cache.stats().entry_count);
}
```

### Method 2: Lazy Load (Get-or-Insert)

```rust
async fn get_user_profile(
    cache: &LookupCache,
    user_id: &str,
) -> Result<Value> {
    cache.get_or_insert_with(
        format!("user:{}", user_id),
        || async {
            // Fetch from DB/API on cache miss
            let profile = fetch_from_database(user_id).await?;
            Ok(json!(profile))
        }
    ).await
}
```

### Method 3: Kafka Topic Consumer

Continuously populate from Kafka topic:

```rust
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::Message;

async fn sync_cache_from_kafka(
    cache: Arc<LookupCache>,
    bootstrap: &str,
    topic: &str,
) {
    let consumer: StreamConsumer = create_consumer(bootstrap, topic);

    tokio::spawn(async move {
        while let Ok(msg) = consumer.recv().await {
            if let Some(payload) = msg.payload() {
                let value: Value = serde_json::from_slice(payload)?;
                let user_id = value["userId"].as_str().unwrap();

                cache.put(
                    format!("user:{}", user_id),
                    value
                ).await;
            }
        }
    });
}
```

### Method 4: Redis Sync

Sync from Redis to Moka:

```rust
async fn sync_from_redis_to_moka(
    redis_conn: &mut redis::aio::Connection,
    moka_cache: &LookupCache,
    pattern: &str,
) -> Result<()> {
    use redis::AsyncCommands;

    // Get all keys matching pattern
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(pattern)
        .query_async(redis_conn)
        .await?;

    // Load into Moka
    for key in keys {
        let value: String = redis_conn.get(&key).await?;
        let json_value: Value = serde_json::from_str(&value)?;
        moka_cache.put(key, json_value).await;
    }

    Ok(())
}
```

### Method 5: Scheduled Refresh

Refresh cache periodically:

```rust
async fn start_cache_refresh_task(
    cache: Arc<LookupCache>,
    refresh_interval: Duration,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(refresh_interval);

        loop {
            interval.tick().await;

            // Reload data
            match load_fresh_data().await {
                Ok(data) => {
                    cache.clear().await;
                    for (key, value) in data {
                        cache.put(key, value).await;
                    }
                    println!("Cache refreshed");
                }
                Err(e) => eprintln!("Cache refresh failed: {}", e),
            }
        }
    });
}
```

---

## Configuration Examples

### Example 1: At-Least-Once with Local Cache

```yaml
appid: critical-pipeline
bootstrap: kafka:9092
target_broker: kafka:9092
input: orders
output: processed-orders
offset: earliest

# At-least-once delivery
commit_strategy:
  manual_commit: true
  commit_mode: sync  # Guaranteed commits
  enable_dlq: true
  dlq_topic: "orders-dlq"
  max_retries: 5

# Local cache for product lookups
cache:
  backend_type: local
  local:
    max_capacity: 50000
    ttl_seconds: 7200
```

### Example 2: High-Volume with Redis Cache

```yaml
appid: high-volume-enrichment
bootstrap: kafka:9092
target_broker: kafka:9092
input: events
output: enriched-events

# Auto-commit for high throughput
commit_strategy:
  manual_commit: false

# Redis for shared cache across replicas
cache:
  backend_type: redis
  redis:
    url: "redis://redis-cluster:6379/0"
    pool_size: 20
    key_prefix: "enrichment"
    default_ttl_seconds: 3600
```

### Example 3: Multi-Level Cache

```yaml
appid: smart-caching
bootstrap: kafka:9092
target_broker: kafka:9092
input: user-activities
output: enriched-activities

commit_strategy:
  manual_commit: true
  commit_mode: async
  commit_interval_ms: 10000

# Multi-level: Fast local + shared Redis
cache:
  backend_type: multi
  local:
    max_capacity: 10000
    ttl_seconds: 300   # 5 min in L1
    tti_seconds: 60
  redis:
    url: "redis://localhost:6379/0"
    key_prefix: "user"
    default_ttl_seconds: 3600  # 1 hour in L2
```

### Example 4: Kafka-Backed Cache

```yaml
appid: event-sourced-enrichment
bootstrap: kafka:9092
target_broker: kafka:9092
input: transactions
output: enriched-transactions

commit_strategy:
  manual_commit: true
  commit_mode: async

# Kafka compacted topic as cache
cache:
  backend_type: kafka
  kafka:
    bootstrap: "kafka:9092"
    topic: "customer-profiles"  # Must be compacted
    group_id: "streamforge-enrichment-cache"
    key_field: "/customerId"
    value_field: "/profile"
    warmup_on_start: true
```

### Example 5: Complete Production Config

```yaml
appid: production-pipeline
bootstrap: kafka-prod:9092
target_broker: kafka-prod:9092
input: user-events
output: processed-events
offset: earliest
threads: 8

# Production-grade commit strategy
commit_strategy:
  manual_commit: true
  commit_mode: async
  commit_interval_ms: 5000
  enable_dlq: true
  dlq_topic: "events-dlq"
  max_retries: 3
  retry_backoff:
    initial_backoff_ms: 100
    max_backoff_ms: 30000
    multiplier: 2.0

# Multi-level cache for best performance
cache:
  backend_type: multi
  local:
    max_capacity: 100000
    ttl_seconds: 600
    tti_seconds: 300
  redis:
    url: "redis://redis-prod:6379/0"
    pool_size: 50
    key_prefix: "streamforge:prod"
    default_ttl_seconds: 3600

# Production Kafka settings
consumer_properties:
  fetch.min.bytes: "1048576"
  fetch.wait.max.ms: "500"
  session.timeout.ms: "30000"
  heartbeat.interval.ms: "3000"

producer_properties:
  batch.size: "65536"
  linger.ms: "10"
  compression.type: "snappy"
  acks: "all"
  enable.idempotence: "true"
```

---

## Best Practices

### At-Least-Once Delivery

1. **Enable for critical data:**
   ```yaml
   commit_strategy:
     manual_commit: true  # Only for important pipelines
   ```

2. **Use idempotent processing:**
   - Design processors to handle duplicates
   - Use deduplication keys
   - Enable producer idempotence

3. **Configure appropriate retries:**
   ```yaml
   max_retries: 3  # Balance reliability vs latency
   ```

4. **Monitor DLQ:**
   - Set up alerts on DLQ topic
   - Investigate and replay failed messages

5. **Tune commit interval:**
   ```yaml
   commit_interval_ms: 5000  # Higher = better throughput, more duplicates on crash
   ```

### Cache Backends

#### Local Cache

1. **Size appropriately:**
   ```yaml
   max_capacity: 10000  # Based on available RAM
   ```

2. **Set TTL for changing data:**
   ```yaml
   ttl_seconds: 3600  # Refresh hourly
   ```

3. **Use TTI for sparse access:**
   ```yaml
   tti_seconds: 600  # Evict idle entries
   ```

#### Redis Cache

1. **Use connection pooling:**
   ```yaml
   pool_size: 20  # Match expected concurrency
   ```

2. **Always use key prefix:**
   ```yaml
   key_prefix: "streamforge:app1"  # Avoid key collisions
   ```

3. **Set default TTL:**
   ```yaml
   default_ttl_seconds: 3600  # Prevent unbounded growth
   ```

4. **Monitor Redis memory:**
   - Use Redis `INFO` command
   - Set `maxmemory` policy
   - Monitor evictions

#### Kafka Cache

1. **Use compacted topics:**
   ```bash
   kafka-topics.sh --create --topic user-profiles \
     --config cleanup.policy=compact \
     --config min.cleanable.dirty.ratio=0.1
   ```

2. **Warmup on start:**
   ```yaml
   warmup_on_start: true  # Load full cache before processing
   ```

3. **Monitor lag:**
   - Ensure cache consumer keeps up
   - Scale if lagging

#### Multi-Level Cache

1. **Small L1, large L2:**
   ```yaml
   local:
     max_capacity: 5000   # Hot set only
   redis:
     # No limit (use Redis maxmemory)
   ```

2. **Short L1 TTL:**
   ```yaml
   local:
     ttl_seconds: 300  # 5 min
   redis:
     default_ttl_seconds: 3600  # 1 hour
   ```

3. **Match access patterns:**
   - Frequently accessed → L1
   - Infrequently accessed → L2 only

---

## Performance Comparison

### Delivery Semantics

| Mode | Throughput | Latency | Duplicates | Losses |
|------|------------|---------|------------|--------|
| At-most-once (auto) | 100K msg/s | 1ms | None | Possible |
| At-least-once (async) | 80K msg/s | 2ms | Possible | None |
| At-least-once (sync) | 25K msg/s | 10ms | Possible | None |

### Cache Backends

| Backend | Latency (p50) | Latency (p99) | Throughput |
|---------|---------------|---------------|------------|
| Local | 50ns | 100ns | 20M ops/s |
| Redis | 1ms | 3ms | 100K ops/s |
| Kafka | 5ms | 20ms | 50K ops/s |
| Multi (L1 hit) | 50ns | 100ns | 20M ops/s |
| Multi (L2 hit) | 1ms | 3ms | 100K ops/s |

---

## Troubleshooting

### At-Least-Once Issues

**Problem: High duplicate rate**
```yaml
# Solution: Increase commit interval
commit_interval_ms: 10000  # From 5000
```

**Problem: DLQ filling up**
```yaml
# Solution: Increase retries and backoff
max_retries: 5
retry_backoff:
  max_backoff_ms: 60000
```

**Problem: Slow processing**
```yaml
# Solution: Use async commit
commit_mode: async  # From sync
```

### Cache Issues

**Problem: Low cache hit rate (local)**
```yaml
# Solution: Increase capacity or TTL
local:
  max_capacity: 50000  # From 10000
  ttl_seconds: 7200    # From 3600
```

**Problem: Redis connection errors**
```yaml
# Solution: Increase pool size
redis:
  pool_size: 50  # From 10
```

**Problem: Kafka cache lag**
```yaml
# Solution: Disable warmup or increase resources
kafka:
  warmup_on_start: false  # Process async
```

---

## See Also

- [Hash Functions & Caching](HASH_AND_CACHE.md)
- [Performance Guide](PERFORMANCE.md)
- [Security Configuration](SECURITY_CONFIGURATION.md)
