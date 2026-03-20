# Hash Functions and Caching

This document describes the hash function and caching capabilities in StreamForge.

## Hash Functions

StreamForge supports multiple hash algorithms for different use cases.

### Supported Algorithms

| Algorithm | Output Size | Use Case | Performance |
|-----------|-------------|----------|-------------|
| **MD5** | 128-bit (32 hex) | Fast deduplication, non-cryptographic | ⚡⚡⚡⚡⚡ |
| **SHA256** | 256-bit (64 hex) | PII anonymization, moderate security | ⚡⚡⚡⚡ |
| **SHA512** | 512-bit (128 hex) | High security, compliance | ⚡⚡⚡ |
| **Murmur64** | 64-bit (16 hex) | Fast partitioning, load balancing | ⚡⚡⚡⚡⚡ |
| **Murmur128** | 128-bit (32 hex) | Fast partitioning, better distribution | ⚡⚡⚡⚡⚡ |

### Hash Transform Syntax

```yaml
# Replace field with hash
transform: "HASH:algorithm,/path"

# Add hash as new field (preserves original)
transform: "HASH:algorithm,/path,outputField"
```

### Examples

#### 1. Anonymize User ID

```yaml
# Input:  {"userId": "user123", "action": "login"}
# Output: {"userIdHash": "a665a...", "action": "login"}
transform: "HASH:SHA256,/userId,userIdHash"
```

#### 2. Fast Deduplication Key

```yaml
# Hash entire message for deduplication
# Input:  {"event": "click", "timestamp": 1234567890}
# Output: "d2d2d2d2..." (MD5 hash as string)
transform: "HASH:MD5,/."
```

#### 3. Consistent Partitioning

```yaml
# Use Murmur for deterministic partitioning
# Input:  {"customerId": "cust-456"}
# Output: "00000000000000000000000000001c8"
transform: "HASH:MURMUR128,/customerId"
```

#### 4. Multiple Hashes

```yaml
# Create object with multiple hashes
transform: "CONSTRUCT:userId=HASH:SHA256,/user:sessionId=HASH:MD5,/session:data=/."
```

## Performance Characteristics

### Hash Algorithm Benchmarks

Approximate performance on modern hardware:

```
MD5:         ~10 million ops/sec
Murmur64:    ~8 million ops/sec
Murmur128:   ~8 million ops/sec
SHA256:      ~2 million ops/sec
SHA512:      ~1 million ops/sec
```

### When to Use Each Algorithm

#### Murmur (64/128)
- **Best for:** Partitioning, sharding, load balancing
- **Pros:** Extremely fast, good distribution
- **Cons:** Not cryptographically secure
- **Use case:** `transform: "HASH:MURMUR128,/userId"`

#### MD5
- **Best for:** Fast deduplication, non-sensitive data
- **Pros:** Very fast, widely supported
- **Cons:** Cryptographically broken (don't use for security)
- **Use case:** `transform: "HASH:MD5,/messageId"`

#### SHA256
- **Best for:** PII anonymization, general security
- **Pros:** Good balance of speed and security
- **Cons:** Slower than MD5/Murmur
- **Use case:** `transform: "HASH:SHA256,/email,emailHash"`

#### SHA512
- **Best for:** High-security requirements, compliance
- **Pros:** Maximum security
- **Cons:** Slowest, larger output
- **Use case:** `transform: "HASH:SHA512,/ssn,ssnHash"`

## Caching

StreamForge provides a high-performance in-memory cache using [Moka](https://github.com/moka-rs/moka).

### Cache Features

- **Async-friendly:** Non-blocking operations
- **TTL support:** Automatic expiration after time-to-live
- **TTI support:** Expire after time-to-idle
- **Size-based eviction:** LRU/LFU policies
- **High performance:** 10-50 nanoseconds lookup time

### Cache Configuration

```rust
use streamforge::cache::{LookupCache, CacheConfig};

// Create cache with custom config
let config = CacheConfig {
    max_capacity: 10_000,      // Maximum entries
    ttl_seconds: Some(3600),   // 1 hour TTL
    tti_seconds: Some(600),    // 10 minutes idle timeout
};

let cache = LookupCache::new(config);
```

### Cache Usage Examples

#### Basic Operations

```rust
use streamforge::cache::{LookupCache, CacheConfig};
use serde_json::json;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let cache = Arc::new(LookupCache::new(CacheConfig::default()));

    // Put value in cache
    cache.put("user:123".to_string(), json!({
        "name": "John Doe",
        "tier": "premium"
    })).await;

    // Get value from cache
    if let Some(user) = cache.get("user:123").await {
        println!("User: {}", user);
    }

    // Get or load
    let value = cache.get_or_insert_with("user:456".to_string(), || async {
        // Fetch from database or API
        Ok(json!({"name": "Jane Doe"}))
    }).await?;
}
```

#### Cache Statistics

```rust
let stats = cache.stats();
println!("Entries: {}", stats.entry_count);
println!("Capacity: {}", stats.max_capacity);
println!("Utilization: {:.2}%", stats.utilization_percent());
```

#### Cache Manager (Multiple Caches)

```rust
use streamforge::cache::{CacheManager, CacheConfig};

let manager = CacheManager::new();

// Create multiple named caches
let user_cache = manager.get_or_create("users", CacheConfig {
    max_capacity: 10_000,
    ttl_seconds: Some(3600),
    tti_seconds: Some(600),
});

let product_cache = manager.get_or_create("products", CacheConfig {
    max_capacity: 50_000,
    ttl_seconds: Some(7200),
    tti_seconds: None,
});

// Get statistics for all caches
let all_stats = manager.all_stats();
for (name, stats) in all_stats {
    println!("{}: {} entries", name, stats.entry_count);
}
```

### Cache Lookup Transform

**Note:** Currently requires async context. This is a roadmap feature.

```rust
use streamforge::filter::{CacheLookupTransform, AsyncTransform};
use streamforge::cache::LookupCache;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let cache = Arc::new(LookupCache::new(CacheConfig::default()));

    // Pre-populate cache
    cache.put("user:123".to_string(), json!({"name": "John"})).await;

    // Create lookup transform
    let transform = CacheLookupTransform::new(
        cache,
        "/userId",              // Key path in message
        Some("user"),           // Cache key prefix
        Some("userProfile")     // Output field
    ).unwrap();

    let input = json!({"userId": "123", "action": "login"});
    let output = transform.transform_async(input).await?;
    // output: {"userId": "123", "action": "login", "userProfile": {"name": "John"}}
}
```

### Cache Patterns

#### Pattern 1: User Enrichment

```rust
// Cache user profiles
cache.put("user:123".to_string(), json!({
    "name": "John Doe",
    "email": "john@example.com",
    "tier": "premium"
})).await;

// Enrich message with user data
let transform = CacheLookupTransform::new(
    cache,
    "/userId",
    Some("user"),
    Some("user")
).unwrap();
```

#### Pattern 2: Product Lookup

```rust
// Cache product details
cache.put("product:ABC".to_string(), json!({
    "name": "Widget",
    "price": 99.99,
    "inStock": true
})).await;

// Add product details to order event
let transform = CacheLookupTransform::new(
    cache,
    "/productId",
    Some("product"),
    Some("productDetails")
).unwrap();
```

#### Pattern 3: Merge Cache Data

```rust
// Use new_with_merge to merge cache result into message
let transform = CacheLookupTransform::new_with_merge(
    cache,
    "/userId",
    Some("user")
).unwrap();

// Input:  {"userId": "123", "action": "purchase"}
// Cache:  {"name": "John", "tier": "premium"}
// Output: {"userId": "123", "action": "purchase", "name": "John", "tier": "premium"}
```

## Combining Hash and Cache

### Use Case: Hash-based Cache Key

```rust
use streamforge::hash::{hash_value, HashAlgorithm};

// Hash user email to create cache key
let email = json!("user@example.com");
let email_hash = hash_value(&email, HashAlgorithm::Sha256)?;

// Use hash as cache key
cache.put(format!("user:{}", email_hash), user_profile).await;
```

### Use Case: Deduplication with Cache

```rust
// Hash message content
let content_hash = hash_value(&message, HashAlgorithm::Md5)?;

// Check if we've seen this before
if cache.contains_key(&content_hash).await {
    // Duplicate - skip processing
    return Ok(());
}

// Mark as seen
cache.put(content_hash, json!({"seen": true})).await;
```

## Integration Examples

### Example 1: Hash User IDs and Cache Profiles

```yaml
appid: user-processing
bootstrap: kafka:9092
target_broker: kafka:9092
input: user-events
output: processed-events
offset: latest

# Step 1: Hash user ID
# Step 2: Enrich with cached profile (async)
transform: "HASH:SHA256,/userId,userIdHash"
```

### Example 2: Partition by Hash

```yaml
appid: partition-by-hash
bootstrap: kafka:9092
target_broker: kafka:9092
input: orders
output: orders-sharded
offset: latest

# Use custom partitioner with hash
partitioner_type: field
partitioner_field: "/customerHash"
transform: "HASH:MURMUR128,/customerId,customerHash"
```

### Example 3: Multi-Destination with Hash Routing

```yaml
appid: hash-routing
bootstrap: kafka:9092
input: events
offset: latest

destinations:
  # Route to shard 0 (hash < 0x8...)
  - brokers: kafka:9092
    topic: events-shard-0
    transform: "HASH:MURMUR64,/eventId,eventHash"
    filter: "REGEX:/eventHash,^[0-7]"

  # Route to shard 1 (hash >= 0x8...)
  - brokers: kafka:9092
    topic: events-shard-1
    transform: "HASH:MURMUR64,/eventId,eventHash"
    filter: "REGEX:/eventHash,^[8-9a-f]"
```

## Best Practices

### Hash Functions

1. **Choose the right algorithm:**
   - Murmur for partitioning/sharding
   - MD5 for fast deduplication
   - SHA256 for PII anonymization
   - SHA512 for high security

2. **Preserve original data when needed:**
   ```yaml
   # Add hash as new field
   transform: "HASH:SHA256,/email,emailHash"
   ```

3. **Filter before hashing:**
   ```yaml
   filter: "/important,==,true"
   transform: "HASH:SHA256,/data"
   ```

4. **Hash consistency:**
   - Same input always produces same hash
   - Use for deterministic routing/partitioning

### Caching

1. **Set appropriate TTL:**
   - Short TTL (5-15 min) for frequently changing data
   - Long TTL (1-24 hours) for stable reference data

2. **Monitor cache size:**
   ```rust
   let stats = cache.stats();
   if stats.utilization_percent() > 90.0 {
       // Consider increasing max_capacity
   }
   ```

3. **Use cache prefixes:**
   ```rust
   cache.put("user:123", user_data).await;
   cache.put("product:ABC", product_data).await;
   ```

4. **Handle cache misses gracefully:**
   ```rust
   match cache.get("key").await {
       Some(value) => process_with_cache(value),
       None => process_without_cache()
   }
   ```

## Roadmap

### Planned Features

- [ ] Redis cache backend integration
- [ ] Kafka-backed cache (compacted topics)
- [ ] Cache warmup on startup
- [ ] Cache metrics (hit rate, miss rate)
- [ ] Distributed cache coordination
- [ ] Cache invalidation patterns
- [ ] Async transform pipeline support

### Future Enhancements

- [ ] Custom hash functions (user-defined)
- [ ] Hash-based bloom filters
- [ ] Cache-aside pattern helpers
- [ ] Write-through cache support
- [ ] Multi-level caching (L1/L2)

## Troubleshooting

### Hash Issues

**Problem:** Hash output looks wrong
```rust
// Make sure you're using the right algorithm
let hash = hash_value(&value, HashAlgorithm::Sha256)?;
println!("Hash: {} (length: {})", hash, hash.len()); // Should be 64 for SHA256
```

**Problem:** Hashing performance is slow
```
Solution: Use Murmur for non-cryptographic hashing
transform: "HASH:MURMUR128,/field"  # Much faster than SHA256
```

### Cache Issues

**Problem:** Cache entries not found
```rust
// Make sure you're waiting for operations
cache.put("key".to_string(), value).await;
cache.cache.run_pending_tasks().await;  // Wait for completion
```

**Problem:** Cache growing too large
```rust
// Set appropriate max_capacity
let config = CacheConfig {
    max_capacity: 10_000,  // Adjust based on memory
    ttl_seconds: Some(3600),
    ..Default::default()
};
```

**Problem:** Stale data in cache
```rust
// Use TTL or TTI for automatic expiration
let config = CacheConfig {
    ttl_seconds: Some(300),   // Expire after 5 minutes
    tti_seconds: Some(60),    // Expire if idle for 1 minute
    ..Default::default()
};
```

## See Also

- [Examples](../examples/config.hash-and-cache.yaml)
- [API Documentation](https://docs.rs/streamforge)
- [Performance Guide](PERFORMANCE.md)
- [Advanced DSL Guide](ADVANCED_DSL_GUIDE.md)
