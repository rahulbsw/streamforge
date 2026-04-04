use crate::error::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Cache backend trait - allows different implementations (local, Redis, Kafka)
#[async_trait]
pub trait CacheBackend: Send + Sync {
    /// Get a value from cache
    async fn get(&self, key: &str) -> Result<Option<Value>>;

    /// Put a value into cache
    async fn put(&self, key: String, value: Value) -> Result<()>;

    /// Check if key exists in cache
    async fn contains_key(&self, key: &str) -> Result<bool>;

    /// Remove a key from cache
    async fn remove(&self, key: &str) -> Result<()>;

    /// Clear all entries from cache
    async fn clear(&self) -> Result<()>;

    /// Get backend name
    fn backend_name(&self) -> &str;
}

/// Local (Moka) cache backend
pub mod local {
    use super::*;
    use crate::cache::LookupCache;
    use std::sync::Arc;

    pub struct LocalCacheBackend {
        cache: Arc<LookupCache>,
    }

    impl LocalCacheBackend {
        pub fn new(cache: Arc<LookupCache>) -> Self {
            Self { cache }
        }
    }

    #[async_trait]
    impl CacheBackend for LocalCacheBackend {
        async fn get(&self, key: &str) -> Result<Option<Value>> {
            Ok(self.cache.get(key).await)
        }

        async fn put(&self, key: String, value: Value) -> Result<()> {
            self.cache.put(key, value).await;
            Ok(())
        }

        async fn contains_key(&self, key: &str) -> Result<bool> {
            Ok(self.cache.contains_key(key).await)
        }

        async fn remove(&self, key: &str) -> Result<()> {
            self.cache.remove(key).await;
            Ok(())
        }

        async fn clear(&self) -> Result<()> {
            self.cache.clear().await;
            Ok(())
        }

        fn backend_name(&self) -> &str {
            "local"
        }
    }
}

/// Redis cache backend
#[cfg(feature = "redis-cache")]
pub mod redis_backend {
    use super::*;
    use crate::error::MirrorMakerError;
    use redis::aio::ConnectionManager;
    use redis::{AsyncCommands, RedisError};
    use tracing::{debug, error, info};

    pub struct RedisCacheBackend {
        connection: ConnectionManager,
        key_prefix: Option<String>,
        default_ttl: Option<u64>,
    }

    impl RedisCacheBackend {
        pub async fn new(
            url: &str,
            key_prefix: Option<String>,
            default_ttl: Option<u64>,
        ) -> Result<Self> {
            let client = redis::Client::open(url)
                .map_err(|e| MirrorMakerError::Config(format!("Redis client error: {}", e)))?;

            let connection = client.get_connection_manager().await.map_err(|e| {
                MirrorMakerError::Processing(format!("Redis connection error: {}", e))
            })?;

            info!(
                "Connected to Redis: prefix={:?}, ttl={:?}s",
                key_prefix, default_ttl
            );

            Ok(Self {
                connection,
                key_prefix,
                default_ttl,
            })
        }

        fn build_key(&self, key: &str) -> String {
            if let Some(prefix) = &self.key_prefix {
                format!("{}:{}", prefix, key)
            } else {
                key.to_string()
            }
        }

        fn handle_redis_error(&self, e: RedisError) -> MirrorMakerError {
            error!("Redis error: {}", e);
            MirrorMakerError::Processing(format!("Redis operation failed: {}", e))
        }
    }

    #[async_trait]
    impl CacheBackend for RedisCacheBackend {
        async fn get(&self, key: &str) -> Result<Option<Value>> {
            let redis_key = self.build_key(key);
            let mut conn = self.connection.clone();

            match conn.get::<_, Option<String>>(&redis_key).await {
                Ok(Some(json_str)) => {
                    debug!("Redis cache hit: {}", key);
                    serde_json::from_str(&json_str).map(Some).map_err(|e| {
                        MirrorMakerError::Processing(format!("JSON parse error: {}", e))
                    })
                }
                Ok(None) => {
                    debug!("Redis cache miss: {}", key);
                    Ok(None)
                }
                Err(e) => Err(self.handle_redis_error(e)),
            }
        }

        async fn put(&self, key: String, value: Value) -> Result<()> {
            let redis_key = self.build_key(&key);
            let mut conn = self.connection.clone();

            let json_str = serde_json::to_string(&value).map_err(|e| {
                MirrorMakerError::Processing(format!("JSON serialize error: {}", e))
            })?;

            if let Some(ttl) = self.default_ttl {
                // Set with TTL
                conn.set_ex::<_, _, ()>(&redis_key, json_str, ttl)
                    .await
                    .map_err(|e| self.handle_redis_error(e))?;
            } else {
                // Set without TTL
                conn.set::<_, _, ()>(&redis_key, json_str)
                    .await
                    .map_err(|e| self.handle_redis_error(e))?;
            }

            debug!("Redis cache set: {}", key);
            Ok(())
        }

        async fn contains_key(&self, key: &str) -> Result<bool> {
            let redis_key = self.build_key(key);
            let mut conn = self.connection.clone();

            conn.exists(&redis_key)
                .await
                .map_err(|e| self.handle_redis_error(e))
        }

        async fn remove(&self, key: &str) -> Result<()> {
            let redis_key = self.build_key(key);
            let mut conn = self.connection.clone();

            conn.del::<_, ()>(&redis_key)
                .await
                .map_err(|e| self.handle_redis_error(e))?;

            debug!("Redis cache delete: {}", key);
            Ok(())
        }

        async fn clear(&self) -> Result<()> {
            let mut conn = self.connection.clone();

            if let Some(prefix) = &self.key_prefix {
                // Delete all keys with prefix
                let pattern = format!("{}:*", prefix);
                let keys: Vec<String> = redis::cmd("KEYS")
                    .arg(&pattern)
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| self.handle_redis_error(e))?;

                if !keys.is_empty() {
                    conn.del::<_, ()>(&keys)
                        .await
                        .map_err(|e| self.handle_redis_error(e))?;
                    info!("Redis cache cleared: {} keys deleted", keys.len());
                }
            } else {
                // Warning: this will flush the entire database
                error!("Cannot clear Redis without key_prefix - would flush entire database!");
                return Err(MirrorMakerError::Config(
                    "Redis clear without key_prefix is not allowed".to_string(),
                ));
            }

            Ok(())
        }

        fn backend_name(&self) -> &str {
            "redis"
        }
    }
}

/// Kafka-backed cache (compacted topic)
pub mod kafka_backend {
    use super::*;
    use crate::cache::{CacheConfig, LookupCache};
    use crate::error::MirrorMakerError;
    use rdkafka::consumer::{Consumer, StreamConsumer};
    use rdkafka::{ClientConfig, Message};
    use std::sync::Arc;
    use std::time::Duration;
    use tracing::{debug, error, info, warn};

    pub struct KafkaCacheBackend {
        cache: Arc<LookupCache>,
        consumer: Arc<StreamConsumer>,
        topic: String,
        key_field: String,
        value_field: String,
    }

    impl KafkaCacheBackend {
        pub async fn new(
            bootstrap: &str,
            topic: &str,
            group_id: &str,
            key_field: &str,
            value_field: &str,
            warmup_on_start: bool,
        ) -> Result<Self> {
            // Create consumer
            let consumer: StreamConsumer = ClientConfig::new()
                .set("bootstrap.servers", bootstrap)
                .set("group.id", group_id)
                .set("auto.offset.reset", "earliest")
                .set("enable.auto.commit", "true")
                .create()
                .map_err(|e| {
                    MirrorMakerError::Config(format!("Kafka consumer creation failed: {}", e))
                })?;

            consumer
                .subscribe(&[topic])
                .map_err(|e| MirrorMakerError::Config(format!("Kafka subscribe failed: {}", e)))?;

            info!("Kafka cache consumer created for topic: {}", topic);

            // Create local cache
            let cache = Arc::new(LookupCache::new(CacheConfig::default()));

            let backend = Self {
                cache,
                consumer: Arc::new(consumer),
                topic: topic.to_string(),
                key_field: key_field.to_string(),
                value_field: value_field.to_string(),
            };

            // Warm up cache if requested
            if warmup_on_start {
                backend.warmup_cache().await?;
            }

            Ok(backend)
        }

        /// Warm up cache by consuming entire topic
        async fn warmup_cache(&self) -> Result<()> {
            info!("Warming up Kafka cache from topic: {}", self.topic);

            let mut message_count = 0;
            let timeout = Duration::from_secs(10);

            // Consume until no more messages
            loop {
                match tokio::time::timeout(timeout, self.consumer.recv()).await {
                    Ok(Ok(msg)) => {
                        if let Some(payload) = msg.payload() {
                            match self.process_message(payload).await {
                                Ok(_) => message_count += 1,
                                Err(e) => warn!("Failed to process cache warmup message: {}", e),
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        error!("Kafka consumer error during warmup: {}", e);
                        break;
                    }
                    Err(_) => {
                        // Timeout - assume we've consumed all messages
                        break;
                    }
                }
            }

            info!(
                "Kafka cache warmed up: {} messages loaded from topic '{}'",
                message_count, self.topic
            );
            Ok(())
        }

        /// Process a Kafka message and update cache
        async fn process_message(&self, payload: &[u8]) -> Result<()> {
            let value: Value = serde_json::from_slice(payload)
                .map_err(|e| MirrorMakerError::Processing(format!("JSON parse error: {}", e)))?;

            // Extract key from message
            let key = self.extract_field(&value, &self.key_field)?;

            // Extract value from message
            let cache_value = if self.value_field == "." {
                value
            } else {
                self.extract_field(&value, &self.value_field)?
            };

            // Update local cache
            let key_str = if let Some(s) = key.as_str() {
                s.to_string()
            } else if let Some(n) = key.as_i64() {
                n.to_string()
            } else {
                return Err(MirrorMakerError::Processing(
                    "Cache key must be string or number".to_string(),
                ));
            };

            self.cache.put(key_str, cache_value).await;
            Ok(())
        }

        /// Extract field from JSON using path
        fn extract_field(&self, value: &Value, path: &str) -> Result<Value> {
            if path == "." {
                return Ok(value.clone());
            }

            let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
            let mut current = value;

            for part in parts {
                current = current.get(part).ok_or_else(|| {
                    MirrorMakerError::Processing(format!("Field not found: {}", path))
                })?;
            }

            Ok(current.clone())
        }

        /// Start background task to keep cache updated
        pub async fn start_sync_task(self: Arc<Self>) {
            tokio::spawn(async move {
                info!("Starting Kafka cache sync task");

                loop {
                    match self.consumer.recv().await {
                        Ok(msg) => {
                            if let Some(payload) = msg.payload() {
                                if let Err(e) = self.process_message(payload).await {
                                    warn!("Failed to process cache update: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Kafka consumer error: {}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            });
        }
    }

    #[async_trait]
    impl CacheBackend for KafkaCacheBackend {
        async fn get(&self, key: &str) -> Result<Option<Value>> {
            Ok(self.cache.get(key).await)
        }

        async fn put(&self, key: String, value: Value) -> Result<()> {
            // Note: For Kafka-backed cache, writes go to the local cache only
            // To write to Kafka, you should produce to the topic directly
            debug!("Kafka cache put (local only): {}", key);
            self.cache.put(key, value).await;
            Ok(())
        }

        async fn contains_key(&self, key: &str) -> Result<bool> {
            Ok(self.cache.contains_key(key).await)
        }

        async fn remove(&self, key: &str) -> Result<()> {
            // For Kafka, removal means producing a tombstone (null value)
            // For now, just remove from local cache
            debug!("Kafka cache remove (local only): {}", key);
            self.cache.remove(key).await;
            Ok(())
        }

        async fn clear(&self) -> Result<()> {
            self.cache.clear().await;
            Ok(())
        }

        fn backend_name(&self) -> &str {
            "kafka"
        }
    }
}

/// Multi-level cache (L1 = local, L2 = Redis)
#[cfg(feature = "redis-cache")]
pub mod multi {
    use super::*;
    use tracing::debug;

    pub struct MultiLevelCache {
        l1: Box<dyn CacheBackend>,
        l2: Box<dyn CacheBackend>,
    }

    impl MultiLevelCache {
        pub fn new(l1: Box<dyn CacheBackend>, l2: Box<dyn CacheBackend>) -> Self {
            Self { l1, l2 }
        }
    }

    #[async_trait]
    impl CacheBackend for MultiLevelCache {
        async fn get(&self, key: &str) -> Result<Option<Value>> {
            // Check L1 first
            if let Some(value) = self.l1.get(key).await? {
                debug!("L1 cache hit: {}", key);
                return Ok(Some(value));
            }

            // Check L2
            if let Some(value) = self.l2.get(key).await? {
                debug!("L2 cache hit: {}, promoting to L1", key);
                // Promote to L1
                self.l1.put(key.to_string(), value.clone()).await?;
                return Ok(Some(value));
            }

            debug!("Cache miss (all levels): {}", key);
            Ok(None)
        }

        async fn put(&self, key: String, value: Value) -> Result<()> {
            // Write to both levels
            self.l1.put(key.clone(), value.clone()).await?;
            self.l2.put(key, value).await?;
            Ok(())
        }

        async fn contains_key(&self, key: &str) -> Result<bool> {
            if self.l1.contains_key(key).await? {
                return Ok(true);
            }
            self.l2.contains_key(key).await
        }

        async fn remove(&self, key: &str) -> Result<()> {
            self.l1.remove(key).await?;
            self.l2.remove(key).await?;
            Ok(())
        }

        async fn clear(&self) -> Result<()> {
            self.l1.clear().await?;
            self.l2.clear().await?;
            Ok(())
        }

        fn backend_name(&self) -> &str {
            "multi-level"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{CacheConfig, LookupCache};
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_local_backend() {
        let cache = Arc::new(LookupCache::new(CacheConfig::default()));
        let backend = local::LocalCacheBackend::new(cache);

        // Test put and get
        backend
            .put("key1".to_string(), json!("value1"))
            .await
            .unwrap();
        let value = backend.get("key1").await.unwrap();
        assert_eq!(value, Some(json!("value1")));

        // Test contains_key
        assert!(backend.contains_key("key1").await.unwrap());
        assert!(!backend.contains_key("key2").await.unwrap());

        // Test remove
        backend.remove("key1").await.unwrap();
        assert!(!backend.contains_key("key1").await.unwrap());

        assert_eq!(backend.backend_name(), "local");
    }
}
