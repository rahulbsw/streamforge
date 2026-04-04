use serde_json::Value;

/// Partitioning strategy for Kafka messages
pub trait Partitioner: Send + Sync {
    /// Determine partition for a message
    fn partition(&self, topic: &str, key: &Value, value: &Value, num_partitions: i32) -> i32;
}

/// Default partitioner - uses hash of key
pub struct DefaultPartitioner;

impl Partitioner for DefaultPartitioner {
    fn partition(&self, _topic: &str, key: &Value, _value: &Value, num_partitions: i32) -> i32 {
        if num_partitions <= 0 {
            return 0;
        }

        let hash = if let Some(key_str) = key.as_str() {
            Self::hash_string(key_str)
        } else {
            Self::hash_string(&key.to_string())
        };

        (hash % num_partitions as u64) as i32
    }
}

impl DefaultPartitioner {
    fn hash_string(s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }
}

/// Field-based partitioner - extracts value from JSON path
pub struct FieldPartitioner {
    field_path: String,
}

impl FieldPartitioner {
    pub fn new(field_path: String) -> Self {
        Self { field_path }
    }

    /// Extract value from JSON using simple path (e.g., "/message/confId")
    fn extract_value(&self, value: &Value) -> Option<i64> {
        let parts: Vec<&str> = self.field_path.trim_matches('/').split('/').collect();

        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        // Try to convert to number
        current.as_i64().or_else(|| current.as_str()?.parse().ok())
    }
}

impl Partitioner for FieldPartitioner {
    fn partition(&self, _topic: &str, _key: &Value, value: &Value, num_partitions: i32) -> i32 {
        if num_partitions <= 0 {
            return 0;
        }

        if let Some(field_value) = self.extract_value(value) {
            (field_value.abs() % num_partitions as i64) as i32
        } else {
            0 // Default partition if extraction fails
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_default_partitioner() {
        let partitioner = DefaultPartitioner;
        let key = json!("test-key");
        let value = json!({"message": "test"});

        let partition = partitioner.partition("test-topic", &key, &value, 10);
        assert!((0..10).contains(&partition));
    }

    #[test]
    fn test_field_partitioner() {
        let partitioner = FieldPartitioner::new("/message/confId".to_string());
        let key = json!("test-key");
        let value = json!({"message": {"confId": 12345}});

        let partition = partitioner.partition("test-topic", &key, &value, 10);
        assert_eq!(partition, 5); // 12345 % 10 = 5
    }

    #[test]
    fn test_field_partitioner_nested() {
        let partitioner = FieldPartitioner::new("/data/user/id".to_string());
        let value = json!({"data": {"user": {"id": 789}}});

        let partition = partitioner.partition("test", &json!(null), &value, 10);
        assert_eq!(partition, 9); // 789 % 10 = 9
    }
}
