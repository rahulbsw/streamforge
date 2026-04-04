use serde_json::Value;
use std::collections::HashMap;

/// Complete Kafka message envelope containing key, value, headers, and metadata.
///
/// This structure represents the full Kafka message, enabling filtering and
/// transformation on all message components, not just the payload.
///
/// # Examples
///
/// ```ignore
/// use streamforge::envelope::MessageEnvelope;
/// use serde_json::json;
///
/// let envelope = MessageEnvelope {
///     key: Some(json!({"userId": "user-123"})),
///     value: json!({"event": "login", "timestamp": 1234567890}),
///     headers: {
///         let mut h = HashMap::new();
///         h.insert("x-tenant".to_string(), b"production".to_vec());
///         h
///     },
///     timestamp: Some(1234567890000),
///     partition: Some(0),
///     offset: Some(12345),
///     topic: Some("events".to_string()),
///  };
/// ```
#[derive(Debug, Clone)]
pub struct MessageEnvelope {
    /// Message key (optional in Kafka)
    ///
    /// The key is used for:
    /// - Message partitioning (determining which partition receives the message)
    /// - Compaction (in compacted topics, latest value per key is retained)
    /// - Message routing and filtering
    ///
    /// If None, message is sent to partition via round-robin or custom logic.
    pub key: Option<Value>,

    /// Message value (payload)
    ///
    /// The main message content. In Streamforge, this is always JSON.
    pub value: Value,

    /// Message headers (metadata key-value pairs)
    ///
    /// Headers are used for:
    /// - Routing and filtering without deserializing value
    /// - Tracing and correlation (x-correlation-id, x-request-id)
    /// - Security (authentication tokens, tenant IDs)
    /// - Metadata (content-type, schema-version)
    ///
    /// Values are raw bytes to support both string and binary data.
    pub headers: HashMap<String, Vec<u8>>,

    /// Message timestamp (milliseconds since Unix epoch)
    ///
    /// Kafka supports two timestamp types:
    /// - CreateTime: When producer created the message
    /// - LogAppendTime: When broker received the message
    ///
    /// If None, Kafka uses current time when message is produced.
    pub timestamp: Option<i64>,

    /// Source partition (for reference/logging)
    ///
    /// The partition this message was consumed from.
    /// Used for logging, debugging, and tracking message origin.
    pub partition: Option<i32>,

    /// Source offset (for reference/logging)
    ///
    /// The offset within the partition where this message was located.
    /// Used for logging, debugging, and tracking message origin.
    pub offset: Option<i64>,

    /// Source topic (for reference/logging)
    ///
    /// The topic this message was consumed from.
    /// Useful when routing from multiple input topics.
    pub topic: Option<String>,
}

impl MessageEnvelope {
    /// Create a new envelope with minimal required fields
    pub fn new(value: Value) -> Self {
        Self {
            key: None,
            value,
            headers: HashMap::new(),
            timestamp: None,
            partition: None,
            offset: None,
            topic: None,
        }
    }

    /// Create envelope from key and value
    pub fn with_key(key: Option<Value>, value: Value) -> Self {
        Self {
            key,
            value,
            headers: HashMap::new(),
            timestamp: None,
            partition: None,
            offset: None,
            topic: None,
        }
    }

    /// Builder: Set key
    pub fn key(mut self, key: Value) -> Self {
        self.key = Some(key);
        self
    }

    /// Builder: Set timestamp
    pub fn timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Builder: Add header
    pub fn header(mut self, name: String, value: Vec<u8>) -> Self {
        self.headers.insert(name, value);
        self
    }

    /// Builder: Add string header
    pub fn with_header_str(mut self, name: String, value: &str) -> Self {
        self.headers.insert(name, value.as_bytes().to_vec());
        self
    }

    /// Builder: Set source metadata
    pub fn source(mut self, topic: String, partition: i32, offset: i64) -> Self {
        self.topic = Some(topic);
        self.partition = Some(partition);
        self.offset = Some(offset);
        self
    }

    /// Get header as string (if valid UTF-8)
    pub fn header_str(&self, name: &str) -> Option<String> {
        self.headers
            .get(name)
            .and_then(|v| String::from_utf8(v.clone()).ok())
    }

    /// Check if header exists
    pub fn has_header(&self, name: &str) -> bool {
        self.headers.contains_key(name)
    }

    /// Get all header names
    pub fn header_names(&self) -> Vec<&String> {
        self.headers.keys().collect()
    }

    /// Get number of headers
    pub fn header_count(&self) -> usize {
        self.headers.len()
    }

    /// Remove header
    pub fn remove_header(&mut self, name: &str) -> Option<Vec<u8>> {
        self.headers.remove(name)
    }

    /// Clear all headers
    pub fn clear_headers(&mut self) {
        self.headers.clear();
    }

    /// Get message age in seconds (if timestamp present)
    pub fn age_seconds(&self) -> Option<i64> {
        self.timestamp.map(|ts| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            (now - ts) / 1000
        })
    }

    /// Check if message is older than specified seconds
    pub fn is_older_than(&self, seconds: i64) -> bool {
        self.age_seconds().map_or(false, |age| age > seconds)
    }

    /// Check if message is newer than specified seconds
    pub fn is_newer_than(&self, seconds: i64) -> bool {
        self.age_seconds().map_or(false, |age| age < seconds)
    }
}

// Note: rdkafka Headers conversion will be implemented in main.rs
// where we have access to the concrete BorrowedHeaders type

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_envelope_new() {
        let envelope = MessageEnvelope::new(json!({"test": "value"}));
        assert!(envelope.key.is_none());
        assert_eq!(envelope.value, json!({"test": "value"}));
        assert!(envelope.headers.is_empty());
        assert!(envelope.timestamp.is_none());
    }

    #[test]
    fn test_envelope_with_key() {
        let envelope = MessageEnvelope::with_key(
            Some(json!({"id": 123})),
            json!({"test": "value"}),
        );
        assert_eq!(envelope.key, Some(json!({"id": 123})));
        assert_eq!(envelope.value, json!({"test": "value"}));
    }

    #[test]
    fn test_envelope_builder() {
        let envelope = MessageEnvelope::new(json!({"test": "value"}))
            .key(json!({"id": 123}))
            .timestamp(1234567890000)
            .with_header_str("x-tenant".to_string(), "production")
            .source("input-topic".to_string(), 0, 12345);

        assert_eq!(envelope.key, Some(json!({"id": 123})));
        assert_eq!(envelope.timestamp, Some(1234567890000));
        assert_eq!(envelope.header_str("x-tenant"), Some("production".to_string()));
        assert_eq!(envelope.topic, Some("input-topic".to_string()));
        assert_eq!(envelope.partition, Some(0));
        assert_eq!(envelope.offset, Some(12345));
    }

    #[test]
    fn test_header_operations() {
        let mut envelope = MessageEnvelope::new(json!({}));

        // Add headers
        envelope.headers.insert("x-tenant".to_string(), b"prod".to_vec());
        envelope.headers.insert("x-user".to_string(), b"user-123".to_vec());

        // Check existence
        assert!(envelope.has_header("x-tenant"));
        assert!(!envelope.has_header("x-missing"));

        // Get as string
        assert_eq!(envelope.header_str("x-tenant"), Some("prod".to_string()));

        // Count
        assert_eq!(envelope.header_count(), 2);

        // Remove
        envelope.remove_header("x-tenant");
        assert_eq!(envelope.header_count(), 1);

        // Clear
        envelope.clear_headers();
        assert_eq!(envelope.header_count(), 0);
    }

    #[test]
    fn test_age_calculation() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Message from 100 seconds ago
        let old_envelope = MessageEnvelope::new(json!({}))
            .timestamp(now - 100_000);

        assert!(old_envelope.is_older_than(50));
        assert!(!old_envelope.is_older_than(150));
        assert!(!old_envelope.is_newer_than(50));
        assert!(old_envelope.is_newer_than(150));
    }

    #[test]
    fn test_header_names() {
        let envelope = MessageEnvelope::new(json!({}))
            .with_header_str("x-tenant".to_string(), "prod")
            .with_header_str("x-user".to_string(), "user-123");

        let names = envelope.header_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&&"x-tenant".to_string()));
        assert!(names.contains(&&"x-user".to_string()));
    }
}
