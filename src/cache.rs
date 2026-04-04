use crate::error::Result;
use moka::future::Cache;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries in cache
    pub max_capacity: u64,
    /// Time-to-live for cache entries (in seconds)
    pub ttl_seconds: Option<u64>,
    /// Time-to-idle for cache entries (in seconds)
    pub tti_seconds: Option<u64>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_capacity: 10_000,
            ttl_seconds: Some(3600), // 1 hour default TTL
            tti_seconds: Some(600),  // 10 minutes idle timeout
        }
    }
}

/// Lookup cache for key-value pairs
pub struct LookupCache {
    cache: Cache<String, Value>,
    config: CacheConfig,
}

impl LookupCache {
    /// Create a new lookup cache with the given configuration
    pub fn new(config: CacheConfig) -> Self {
        let mut builder = Cache::builder()
            .max_capacity(config.max_capacity)
            .name("lookup-cache");

        if let Some(ttl) = config.ttl_seconds {
            builder = builder.time_to_live(Duration::from_secs(ttl));
            debug!("Cache TTL set to {} seconds", ttl);
        }

        if let Some(tti) = config.tti_seconds {
            builder = builder.time_to_idle(Duration::from_secs(tti));
            debug!("Cache TTI set to {} seconds", tti);
        }

        let cache = builder.build();

        info!(
            "Created lookup cache: max_capacity={}, ttl={:?}s, tti={:?}s",
            config.max_capacity, config.ttl_seconds, config.tti_seconds
        );

        Self { cache, config }
    }

    /// Get a value from cache
    pub async fn get(&self, key: &str) -> Option<Value> {
        self.cache.get(key).await
    }

    /// Put a value into cache
    pub async fn put(&self, key: String, value: Value) {
        self.cache.insert(key, value).await;
    }

    /// Check if key exists in cache
    pub async fn contains_key(&self, key: &str) -> bool {
        self.cache.contains_key(key)
    }

    /// Remove a key from cache
    pub async fn remove(&self, key: &str) {
        self.cache.invalidate(key).await;
    }

    /// Clear all entries from cache
    pub async fn clear(&self) {
        self.cache.invalidate_all();
        // Wait for invalidation to complete
        self.cache.run_pending_tasks().await;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.cache.entry_count(),
            max_capacity: self.config.max_capacity,
            weighted_size: self.cache.weighted_size(),
        }
    }

    /// Get or insert a value using a loader function
    pub async fn get_or_insert_with<F, Fut>(&self, key: String, loader: F) -> Result<Value>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Value>>,
    {
        if let Some(value) = self.cache.get(&key).await {
            debug!("Cache hit for key: {}", key);
            return Ok(value);
        }

        debug!("Cache miss for key: {}", key);
        let value = loader().await?;
        self.cache.insert(key, value.clone()).await;
        Ok(value)
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entry_count: u64,
    pub max_capacity: u64,
    pub weighted_size: u64,
}

impl CacheStats {
    /// Calculate cache utilization percentage
    pub fn utilization_percent(&self) -> f64 {
        if self.max_capacity == 0 {
            0.0
        } else {
            (self.entry_count as f64 / self.max_capacity as f64) * 100.0
        }
    }
}

/// Shared cache manager for multiple caches
pub struct CacheManager {
    caches: Arc<dashmap::DashMap<String, Arc<LookupCache>>>,
}

impl CacheManager {
    /// Create a new cache manager
    pub fn new() -> Self {
        Self {
            caches: Arc::new(dashmap::DashMap::new()),
        }
    }

    /// Get or create a named cache
    pub fn get_or_create(&self, name: &str, config: CacheConfig) -> Arc<LookupCache> {
        self.caches
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(LookupCache::new(config)))
            .clone()
    }

    /// Get a named cache if it exists
    pub fn get(&self, name: &str) -> Option<Arc<LookupCache>> {
        self.caches.get(name).map(|entry| entry.clone())
    }

    /// Remove a named cache
    pub fn remove(&self, name: &str) {
        self.caches.remove(name);
    }

    /// Get all cache names
    pub fn cache_names(&self) -> Vec<String> {
        self.caches
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get statistics for all caches
    pub fn all_stats(&self) -> Vec<(String, CacheStats)> {
        self.caches
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().stats()))
            .collect()
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let config = CacheConfig {
            max_capacity: 100,
            ttl_seconds: None,
            tti_seconds: None,
        };
        let cache = LookupCache::new(config);

        // Put and get
        cache.put("key1".to_string(), json!("value1")).await;
        let value = cache.get("key1").await;
        assert_eq!(value, Some(json!("value1")));

        // Contains key
        assert!(cache.contains_key("key1").await);
        assert!(!cache.contains_key("key2").await);

        // Remove
        cache.remove("key1").await;
        assert!(!cache.contains_key("key1").await);
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let config = CacheConfig {
            max_capacity: 100,
            ttl_seconds: None,
            tti_seconds: None,
        };
        let cache = LookupCache::new(config);

        cache.put("key1".to_string(), json!("value1")).await;
        cache.put("key2".to_string(), json!("value2")).await;

        // Wait for cache operations to complete
        cache.cache.run_pending_tasks().await;

        let stats = cache.stats();
        assert_eq!(stats.entry_count, 2);
        assert_eq!(stats.max_capacity, 100);
        assert_eq!(stats.utilization_percent(), 2.0);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let config = CacheConfig::default();
        let cache = LookupCache::new(config);

        cache.put("key1".to_string(), json!("value1")).await;
        cache.put("key2".to_string(), json!("value2")).await;

        // Wait for cache operations to complete
        cache.cache.run_pending_tasks().await;
        assert_eq!(cache.stats().entry_count, 2);

        cache.clear().await;
        assert_eq!(cache.stats().entry_count, 0);
    }

    #[tokio::test]
    async fn test_get_or_insert_with() {
        let config = CacheConfig::default();
        let cache = LookupCache::new(config);

        // First call should load the value
        let value = cache
            .get_or_insert_with("key1".to_string(), || async { Ok(json!({"loaded": true})) })
            .await
            .unwrap();
        assert_eq!(value, json!({"loaded": true}));

        // Second call should get from cache
        let value2 = cache
            .get_or_insert_with("key1".to_string(), || async {
                Ok(json!({"should_not_be_called": true}))
            })
            .await
            .unwrap();
        assert_eq!(value2, json!({"loaded": true}));
    }

    #[tokio::test]
    async fn test_cache_manager() {
        let manager = CacheManager::new();

        let config1 = CacheConfig {
            max_capacity: 100,
            ttl_seconds: None,
            tti_seconds: None,
        };
        let cache1 = manager.get_or_create("cache1", config1);
        cache1.put("key1".to_string(), json!("value1")).await;

        let config2 = CacheConfig {
            max_capacity: 200,
            ttl_seconds: None,
            tti_seconds: None,
        };
        let cache2 = manager.get_or_create("cache2", config2);
        cache2.put("key2".to_string(), json!("value2")).await;

        // Get existing cache
        let cache1_again = manager.get("cache1").unwrap();
        assert!(cache1_again.contains_key("key1").await);

        // Check cache names
        let names = manager.cache_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"cache1".to_string()));
        assert!(names.contains(&"cache2".to_string()));

        // Get all stats
        let stats = manager.all_stats();
        assert_eq!(stats.len(), 2);
    }

    #[tokio::test]
    async fn test_cache_max_capacity() {
        let config = CacheConfig {
            max_capacity: 2,
            ttl_seconds: None,
            tti_seconds: None,
        };
        let cache = LookupCache::new(config);

        cache.put("key1".to_string(), json!("value1")).await;
        cache.put("key2".to_string(), json!("value2")).await;
        cache.put("key3".to_string(), json!("value3")).await;

        // Cache should evict oldest entries
        let stats = cache.stats();
        assert!(stats.entry_count <= 2);
    }

    #[tokio::test]
    async fn test_cache_with_complex_values() {
        let config = CacheConfig::default();
        let cache = LookupCache::new(config);

        let complex_value = json!({
            "user": {
                "id": 123,
                "name": "Test User",
                "tags": ["admin", "developer"]
            },
            "metadata": {
                "timestamp": 1234567890,
                "version": "1.0"
            }
        });

        cache
            .put("user:123".to_string(), complex_value.clone())
            .await;

        let retrieved = cache.get("user:123").await.unwrap();
        assert_eq!(retrieved, complex_value);
    }
}
