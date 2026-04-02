# Quick Reference: Hash Functions & Caching

## Hash Functions - Quick Start

### Basic Usage

```yaml
# Replace field with hash
transform: "HASH:SHA256,/userId"

# Add hash as new field (preserves original)
transform: "HASH:SHA256,/userId,userIdHash"
```

### All Algorithms

| Syntax | Speed | Use Case |
|--------|-------|----------|
| `HASH:MD5,/field` | ⚡⚡⚡⚡⚡ | Fast deduplication |
| `HASH:SHA256,/field` | ⚡⚡⚡⚡ | PII anonymization |
| `HASH:SHA512,/field` | ⚡⚡⚡ | High security |
| `HASH:MURMUR64,/field` | ⚡⚡⚡⚡⚡ | Partitioning (64-bit) |
| `HASH:MURMUR128,/field` | ⚡⚡⚡⚡⚡ | Partitioning (128-bit) |

### Common Patterns

```yaml
# Anonymize email
transform: "HASH:SHA256,/email,emailHash"

# Fast deduplication key
transform: "HASH:MD5,/.,messageHash"

# Partition by customer
transform: "HASH:MURMUR128,/customerId,customerHash"

# Multiple hashes
transform: "CONSTRUCT:id=HASH:SHA256,/userId:key=HASH:MD5,/sessionId"
```

## Caching - Quick Start

### Create Cache

```rust
use streamforge::cache::{LookupCache, CacheConfig};
use std::sync::Arc;

let cache = Arc::new(LookupCache::new(CacheConfig {
    max_capacity: 10_000,
    ttl_seconds: Some(3600),  // 1 hour
    tti_seconds: Some(600),   // 10 min idle
}));
```

### Basic Operations

```rust
// Put
cache.put("key".to_string(), json!({"data": "value"})).await;

// Get
if let Some(value) = cache.get("key").await {
    println!("{}", value);
}

// Get or load
let value = cache.get_or_insert_with("key".to_string(), || async {
    Ok(json!({"loaded": true}))
}).await?;

// Stats
let stats = cache.stats();
println!("Entries: {}/{}", stats.entry_count, stats.max_capacity);
```

### Cache Lookup Transform

```rust
use streamforge::filter::{CacheLookupTransform, AsyncTransform};

// Add cache result as new field
let transform = CacheLookupTransform::new(
    cache,
    "/userId",           // Key path
    Some("user"),        // Prefix (creates "user:123")
    Some("userProfile")  // Output field name
).unwrap();

let input = json!({"userId": "123", "action": "login"});
let output = transform.transform_async(input).await?;
// {"userId": "123", "action": "login", "userProfile": {...}}
```

### Cache Manager

```rust
use streamforge::cache::CacheManager;

let manager = CacheManager::new();

let user_cache = manager.get_or_create("users", CacheConfig {
    max_capacity: 10_000,
    ttl_seconds: Some(3600),
    tti_seconds: None,
});

let product_cache = manager.get_or_create("products", CacheConfig {
    max_capacity: 50_000,
    ttl_seconds: Some(7200),
    tti_seconds: None,
});
```

## Real-World Examples

### Example 1: PII Anonymization Pipeline

```yaml
appid: anonymize-pii
bootstrap: kafka:9092
target_broker: kafka:9092
input: user-events
output: anonymized-events

# Hash sensitive fields
transform: "CONSTRUCT:emailHash=HASH:SHA256,/email:phoneHash=HASH:SHA256,/phone:userId=/userId"
```

### Example 2: Fast Deduplication

```yaml
appid: deduplicate
bootstrap: kafka:9092
target_broker: kafka:9092
input: raw-events
output: unique-events

# Create deduplication key from entire message
transform: "HASH:MD5,/.,dedupKey"
```

### Example 3: Sharding by Hash

```yaml
appid: shard-by-customer
bootstrap: kafka:9092
target_broker: kafka:9092
input: orders
output: orders-sharded

# Hash for consistent partitioning
partitioner_type: field
partitioner_field: "/customerHash"
transform: "HASH:MURMUR128,/customerId,customerHash"
```

### Example 4: User Enrichment

```rust
#[tokio::main]
async fn main() {
    let cache = Arc::new(LookupCache::new(CacheConfig::default()));

    // Pre-populate cache (from DB, API, etc.)
    cache.put("user:123".to_string(), json!({
        "name": "John Doe",
        "tier": "premium",
        "email": "john@example.com"
    })).await;

    // Create enrichment transform
    let transform = CacheLookupTransform::new(
        cache,
        "/userId",
        Some("user"),
        Some("user")
    ).unwrap();

    // Process messages
    let input = json!({"userId": "123", "action": "purchase", "amount": 99.99});
    let enriched = transform.transform_async(input).await?;
    // {"userId": "123", "action": "purchase", "amount": 99.99, "user": {...}}
}
```

## Performance Tips

### Hash Functions

1. **Use Murmur for partitioning** - 4x faster than SHA256
2. **Use MD5 for deduplication** - Fast and sufficient for non-security
3. **Filter before hashing** - Reduce CPU usage
   ```yaml
   filter: "/important,==,true"
   transform: "HASH:SHA256,/data"
   ```

### Caching

1. **Set appropriate TTL** - Balance freshness vs load
   ```rust
   ttl_seconds: Some(3600)  // 1 hour for stable data
   ttl_seconds: Some(300)   // 5 min for changing data
   ```

2. **Monitor utilization** - Prevent eviction churn
   ```rust
   if cache.stats().utilization_percent() > 90.0 {
       // Increase max_capacity
   }
   ```

3. **Use cache prefixes** - Organize keys
   ```rust
   cache.put("user:123", user_data).await;
   cache.put("product:ABC", product_data).await;
   ```

## Troubleshooting

### Hash Issues

**Q: My hash looks wrong**
```rust
// Check hash length
let hash = hash_value(&value, HashAlgorithm::Sha256)?;
assert_eq!(hash.len(), 64); // SHA256 = 64 hex chars
```

**Q: Performance is slow**
```yaml
# Switch to Murmur for non-crypto use
transform: "HASH:MURMUR128,/field"  # Much faster
```

### Cache Issues

**Q: Cache entries disappear**
```rust
// Increase TTL or capacity
let config = CacheConfig {
    max_capacity: 20_000,      // Double capacity
    ttl_seconds: Some(7200),   // Longer TTL
    tti_seconds: None,         // No idle timeout
};
```

**Q: Cache misses are high**
```rust
// Pre-populate cache at startup
for user in load_users() {
    cache.put(format!("user:{}", user.id), user.data).await;
}
```

## API Reference

### Hash Functions

```rust
use streamforge::hash::{hash_bytes, hash_value, HashAlgorithm};

// Hash bytes
let hash = hash_bytes(b"data", HashAlgorithm::Sha256)?;

// Hash JSON value
let hash = hash_value(&json!("value"), HashAlgorithm::Md5)?;

// Parse algorithm
let algo = HashAlgorithm::from_str("SHA256")?;
```

### Cache

```rust
use streamforge::cache::{LookupCache, CacheConfig, CacheManager};

// Create cache
let cache = LookupCache::new(CacheConfig::default());

// Operations
cache.put(key, value).await;
let value = cache.get(&key).await;
cache.remove(&key).await;
cache.clear().await;

// Stats
let stats = cache.stats();
```

## See Also

- [Full Documentation](docs/HASH_AND_CACHE.md)
- [Examples](examples/config.hash-and-cache.yaml)
- [Implementation Summary](IMPLEMENTATION_SUMMARY.md)
