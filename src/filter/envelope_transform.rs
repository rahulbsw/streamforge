use crate::envelope::MessageEnvelope;
use crate::error::{MirrorMakerError, Result};
use crate::hash::{hash_value, HashAlgorithm};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

// ============================================================================
// KEY TRANSFORMS
// ============================================================================

/// Transform that sets the message key from a field in the value
///
/// Example:
/// ```ignore
/// use streamforge::filter::KeyFromTransform;
///
/// // Extract user ID as key
/// let transform = KeyFromTransform::new("/user/id").unwrap();
/// ```
pub struct KeyFromTransform {
    value_path: String,
}

impl KeyFromTransform {
    pub fn new(value_path: &str) -> Result<Self> {
        Ok(Self {
            value_path: value_path.to_string(),
        })
    }

    fn extract_from_value(&self, value: &Value) -> Option<Value> {
        let parts: Vec<&str> = self.value_path.trim_matches('/').split('/').collect();
        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }
        Some(current.clone())
    }
}

impl EnvelopeTransform for KeyFromTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        if let Some(extracted) = self.extract_from_value(&envelope.value) {
            envelope.key = Some(extracted);
            Ok(envelope)
        } else {
            Err(MirrorMakerError::Processing(format!(
                "Key path not found in value: {}",
                self.value_path
            )))
        }
    }
}

/// Transform that sets a constant key
///
/// Example:
/// ```ignore
/// use streamforge::filter::KeyConstantTransform;
///
/// // Set all messages to same key
/// let transform = KeyConstantTransform::new("constant-key");
/// ```
pub struct KeyConstantTransform {
    constant_value: Value,
}

impl KeyConstantTransform {
    pub fn new(constant: &str) -> Self {
        Self {
            constant_value: json!(constant),
        }
    }

    pub fn new_json(value: Value) -> Self {
        Self {
            constant_value: value,
        }
    }
}

impl EnvelopeTransform for KeyConstantTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        envelope.key = Some(self.constant_value.clone());
        Ok(envelope)
    }
}

/// Transform that builds a key using a template
///
/// Example:
/// ```ignore
/// use streamforge::filter::KeyTemplateTransform;
///
/// // Build key: "user-{userId}"
/// let transform = KeyTemplateTransform::new("user-{/user/id}").unwrap();
/// ```
pub struct KeyTemplateTransform {
    template: String,
}

impl KeyTemplateTransform {
    pub fn new(template: &str) -> Result<Self> {
        Ok(Self {
            template: template.to_string(),
        })
    }

    fn apply_template(&self, value: &Value) -> Result<String> {
        let mut result = self.template.clone();

        // Find all {/path} placeholders
        let re = regex::Regex::new(r"\{(/[^}]+)\}").unwrap();

        for cap in re.captures_iter(&self.template.clone()) {
            let placeholder = &cap[0];
            let path = &cap[1];

            // Extract value from path
            let extracted = self.extract_from_path(value, path)?;
            let value_str = match &extracted {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => serde_json::to_string(&extracted).unwrap_or_default(),
            };

            result = result.replace(placeholder, &value_str);
        }

        Ok(result)
    }

    fn extract_from_path(&self, value: &Value, path: &str) -> Result<Value> {
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        let mut current = value;
        for part in parts {
            current = current
                .get(part)
                .ok_or_else(|| MirrorMakerError::Processing(format!("Path not found: {}", path)))?;
        }
        Ok(current.clone())
    }
}

impl EnvelopeTransform for KeyTemplateTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        let key_str = self.apply_template(&envelope.value)?;
        envelope.key = Some(json!(key_str));
        Ok(envelope)
    }
}

/// Transform that hashes a value field and uses it as key
///
/// Example:
/// ```ignore
/// use streamforge::filter::KeyHashTransform;
/// use streamforge::hash::HashAlgorithm;
///
/// // Hash user email as key
/// let transform = KeyHashTransform::new("/user/email", HashAlgorithm::Sha256).unwrap();
/// ```
pub struct KeyHashTransform {
    value_path: String,
    algorithm: HashAlgorithm,
}

impl KeyHashTransform {
    pub fn new(value_path: &str, algorithm: HashAlgorithm) -> Result<Self> {
        Ok(Self {
            value_path: value_path.to_string(),
            algorithm,
        })
    }

    fn extract_from_value(&self, value: &Value) -> Option<Value> {
        let parts: Vec<&str> = self.value_path.trim_matches('/').split('/').collect();
        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }
        Some(current.clone())
    }
}

impl EnvelopeTransform for KeyHashTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        if let Some(extracted) = self.extract_from_value(&envelope.value) {
            let hash = hash_value(&extracted, self.algorithm)?;
            envelope.key = Some(json!(hash));
            Ok(envelope)
        } else {
            Err(MirrorMakerError::Processing(format!(
                "Key path not found in value: {}",
                self.value_path
            )))
        }
    }
}

/// Transform that constructs a JSON key from multiple fields
///
/// Example:
/// ```ignore
/// use streamforge::filter::KeyConstructTransform;
/// use std::collections::HashMap;
///
/// let mut fields = HashMap::new();
/// fields.insert("tenant".to_string(), "/tenant/id".to_string());
/// fields.insert("user".to_string(), "/user/id".to_string());
/// let transform = KeyConstructTransform::new(fields).unwrap();
/// // Results in key: {"tenant":"t1","user":"u123"}
/// ```
pub struct KeyConstructTransform {
    fields: HashMap<String, String>,
}

impl KeyConstructTransform {
    pub fn new(fields: HashMap<String, String>) -> Result<Self> {
        Ok(Self { fields })
    }

    fn extract_from_path(&self, value: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }
        Some(current.clone())
    }
}

impl EnvelopeTransform for KeyConstructTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        let mut result = Map::new();

        for (output_name, input_path) in &self.fields {
            if let Some(extracted) = self.extract_from_path(&envelope.value, input_path) {
                result.insert(output_name.clone(), extracted);
            }
        }

        envelope.key = Some(Value::Object(result));
        Ok(envelope)
    }
}

// ============================================================================
// HEADER TRANSFORMS
// ============================================================================

/// Transform that sets a header to a constant value
///
/// Example:
/// ```ignore
/// use streamforge::filter::HeaderSetTransform;
///
/// let transform = HeaderSetTransform::new("x-processed-by", "streamforge");
/// ```
pub struct HeaderSetTransform {
    header_name: String,
    value: String,
}

impl HeaderSetTransform {
    pub fn new(header_name: &str, value: &str) -> Self {
        Self {
            header_name: header_name.to_string(),
            value: value.to_string(),
        }
    }
}

impl EnvelopeTransform for HeaderSetTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        envelope
            .headers
            .insert(self.header_name.clone(), self.value.as_bytes().to_vec());
        Ok(envelope)
    }
}

/// Transform that sets a header from a value field
///
/// Example:
/// ```ignore
/// use streamforge::filter::HeaderFromTransform;
///
/// // Set x-user-id header from value field
/// let transform = HeaderFromTransform::new("x-user-id", "/user/id").unwrap();
/// ```
pub struct HeaderFromTransform {
    header_name: String,
    value_path: String,
}

impl HeaderFromTransform {
    pub fn new(header_name: &str, value_path: &str) -> Result<Self> {
        Ok(Self {
            header_name: header_name.to_string(),
            value_path: value_path.to_string(),
        })
    }

    fn extract_from_value(&self, value: &Value) -> Option<String> {
        let parts: Vec<&str> = self.value_path.trim_matches('/').split('/').collect();
        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        match current {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }
}

impl EnvelopeTransform for HeaderFromTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        if let Some(value_str) = self.extract_from_value(&envelope.value) {
            envelope
                .headers
                .insert(self.header_name.clone(), value_str.as_bytes().to_vec());
            Ok(envelope)
        } else {
            Err(MirrorMakerError::Processing(format!(
                "Header source path not found: {}",
                self.value_path
            )))
        }
    }
}

/// Transform that copies a header to a new name
///
/// Example:
/// ```ignore
/// use streamforge::filter::HeaderCopyTransform;
///
/// // Copy x-request-id to x-correlation-id
/// let transform = HeaderCopyTransform::new("x-request-id", "x-correlation-id");
/// ```
pub struct HeaderCopyTransform {
    source_header: String,
    dest_header: String,
}

impl HeaderCopyTransform {
    pub fn new(source: &str, dest: &str) -> Self {
        Self {
            source_header: source.to_string(),
            dest_header: dest.to_string(),
        }
    }
}

impl EnvelopeTransform for HeaderCopyTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        if let Some(value) = envelope.headers.get(&self.source_header).cloned() {
            envelope.headers.insert(self.dest_header.clone(), value);
            Ok(envelope)
        } else {
            // Source header doesn't exist - just return envelope unchanged
            Ok(envelope)
        }
    }
}

/// Transform that removes a header
///
/// Example:
/// ```ignore
/// use streamforge::filter::HeaderRemoveTransform;
///
/// let transform = HeaderRemoveTransform::new("x-internal-token");
/// ```
pub struct HeaderRemoveTransform {
    header_name: String,
}

impl HeaderRemoveTransform {
    pub fn new(header_name: &str) -> Self {
        Self {
            header_name: header_name.to_string(),
        }
    }
}

impl EnvelopeTransform for HeaderRemoveTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        envelope.headers.remove(&self.header_name);
        Ok(envelope)
    }
}

// ============================================================================
// TIMESTAMP TRANSFORMS
// ============================================================================

/// Transform that preserves the original timestamp
pub struct TimestampPreserveTransform;

impl EnvelopeTransform for TimestampPreserveTransform {
    fn transform_envelope(&self, envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        // No-op: timestamp already preserved
        Ok(envelope)
    }
}

/// Transform that sets timestamp to current time
pub struct TimestampCurrentTransform;

impl EnvelopeTransform for TimestampCurrentTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        envelope.timestamp = Some(now);
        Ok(envelope)
    }
}

/// Transform that sets timestamp from a value field
///
/// Example:
/// ```ignore
/// use streamforge::filter::TimestampFromTransform;
///
/// // Use event timestamp from value
/// let transform = TimestampFromTransform::new("/event/timestamp").unwrap();
/// ```
pub struct TimestampFromTransform {
    value_path: String,
}

impl TimestampFromTransform {
    pub fn new(value_path: &str) -> Result<Self> {
        Ok(Self {
            value_path: value_path.to_string(),
        })
    }

    fn extract_timestamp(&self, value: &Value) -> Option<i64> {
        let parts: Vec<&str> = self.value_path.trim_matches('/').split('/').collect();
        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        current.as_i64()
    }
}

impl EnvelopeTransform for TimestampFromTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        if let Some(ts) = self.extract_timestamp(&envelope.value) {
            envelope.timestamp = Some(ts);
            Ok(envelope)
        } else {
            Err(MirrorMakerError::Processing(format!(
                "Timestamp path not found or not a number: {}",
                self.value_path
            )))
        }
    }
}

/// Transform that adds seconds to the timestamp
///
/// Example:
/// ```ignore
/// use streamforge::filter::TimestampAddTransform;
///
/// // Add 1 hour (3600 seconds)
/// let transform = TimestampAddTransform::new(3600);
/// ```
pub struct TimestampAddTransform {
    seconds: i64,
}

impl TimestampAddTransform {
    pub fn new(seconds: i64) -> Self {
        Self { seconds }
    }
}

impl EnvelopeTransform for TimestampAddTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        if let Some(ts) = envelope.timestamp {
            envelope.timestamp = Some(ts + (self.seconds * 1000));
            Ok(envelope)
        } else {
            Err(MirrorMakerError::Processing(
                "Cannot add to timestamp: message has no timestamp".to_string(),
            ))
        }
    }
}

/// Transform that subtracts seconds from the timestamp
///
/// Example:
/// ```ignore
/// use streamforge::filter::TimestampSubtractTransform;
///
/// // Subtract 5 minutes (300 seconds)
/// let transform = TimestampSubtractTransform::new(300);
/// ```
pub struct TimestampSubtractTransform {
    seconds: i64,
}

impl TimestampSubtractTransform {
    pub fn new(seconds: i64) -> Self {
        Self { seconds }
    }
}

impl EnvelopeTransform for TimestampSubtractTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        if let Some(ts) = envelope.timestamp {
            envelope.timestamp = Some(ts - (self.seconds * 1000));
            Ok(envelope)
        } else {
            Err(MirrorMakerError::Processing(
                "Cannot subtract from timestamp: message has no timestamp".to_string(),
            ))
        }
    }
}

// ============================================================================
// ENVELOPE TRANSFORM TRAIT
// ============================================================================

/// Trait for transformations that operate on the complete message envelope
pub trait EnvelopeTransform: Send + Sync {
    fn transform_envelope(&self, envelope: MessageEnvelope) -> Result<MessageEnvelope>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_from_transform() {
        let transform = KeyFromTransform::new("/user/id").unwrap();
        let envelope = MessageEnvelope::new(json!({"user": {"id": "user-123"}}));

        let result = transform.transform_envelope(envelope).unwrap();
        assert_eq!(result.key, Some(json!("user-123")));
    }

    #[test]
    fn test_key_constant_transform() {
        let transform = KeyConstantTransform::new("constant-key");
        let envelope = MessageEnvelope::new(json!({}));

        let result = transform.transform_envelope(envelope).unwrap();
        assert_eq!(result.key, Some(json!("constant-key")));
    }

    #[test]
    fn test_key_template_transform() {
        let transform = KeyTemplateTransform::new("user-{/user/id}").unwrap();
        let envelope = MessageEnvelope::new(json!({"user": {"id": "123"}}));

        let result = transform.transform_envelope(envelope).unwrap();
        assert_eq!(result.key, Some(json!("user-123")));
    }

    #[test]
    fn test_key_hash_transform() {
        let transform = KeyHashTransform::new("/user/email", HashAlgorithm::Md5).unwrap();
        let envelope = MessageEnvelope::new(json!({"user": {"email": "test@example.com"}}));

        let result = transform.transform_envelope(envelope).unwrap();
        assert!(result.key.is_some());
        assert!(result.key.unwrap().is_string());
    }

    #[test]
    fn test_key_construct_transform() {
        let mut fields = HashMap::new();
        fields.insert("tenant".to_string(), "/tenant".to_string());
        fields.insert("user".to_string(), "/user/id".to_string());

        let transform = KeyConstructTransform::new(fields).unwrap();
        let envelope = MessageEnvelope::new(json!({
            "tenant": "t1",
            "user": {"id": "u123"}
        }));

        let result = transform.transform_envelope(envelope).unwrap();
        let key = result.key.unwrap();
        assert_eq!(key.get("tenant").unwrap(), &json!("t1"));
        assert_eq!(key.get("user").unwrap(), &json!("u123"));
    }

    #[test]
    fn test_header_set_transform() {
        let transform = HeaderSetTransform::new("x-processed-by", "streamforge");
        let envelope = MessageEnvelope::new(json!({}));

        let result = transform.transform_envelope(envelope).unwrap();
        assert_eq!(
            result.header_str("x-processed-by"),
            Some("streamforge".to_string())
        );
    }

    #[test]
    fn test_header_from_transform() {
        let transform = HeaderFromTransform::new("x-user-id", "/user/id").unwrap();
        let envelope = MessageEnvelope::new(json!({"user": {"id": "user-123"}}));

        let result = transform.transform_envelope(envelope).unwrap();
        assert_eq!(result.header_str("x-user-id"), Some("user-123".to_string()));
    }

    #[test]
    fn test_header_copy_transform() {
        let transform = HeaderCopyTransform::new("x-request-id", "x-correlation-id");
        let envelope =
            MessageEnvelope::new(json!({})).with_header_str("x-request-id".to_string(), "req-123");

        let result = transform.transform_envelope(envelope).unwrap();
        assert_eq!(
            result.header_str("x-correlation-id"),
            Some("req-123".to_string())
        );
    }

    #[test]
    fn test_header_remove_transform() {
        let transform = HeaderRemoveTransform::new("x-internal-token");
        let envelope = MessageEnvelope::new(json!({}))
            .with_header_str("x-internal-token".to_string(), "secret");

        let result = transform.transform_envelope(envelope).unwrap();
        assert!(!result.has_header("x-internal-token"));
    }

    #[test]
    fn test_timestamp_current_transform() {
        let transform = TimestampCurrentTransform;
        let envelope = MessageEnvelope::new(json!({}));

        let result = transform.transform_envelope(envelope).unwrap();
        assert!(result.timestamp.is_some());
    }

    #[test]
    fn test_timestamp_from_transform() {
        let transform = TimestampFromTransform::new("/event/timestamp").unwrap();
        let envelope = MessageEnvelope::new(json!({"event": {"timestamp": 1234567890}}));

        let result = transform.transform_envelope(envelope).unwrap();
        assert_eq!(result.timestamp, Some(1234567890));
    }

    #[test]
    fn test_timestamp_add_transform() {
        let transform = TimestampAddTransform::new(3600); // Add 1 hour
        let envelope = MessageEnvelope::new(json!({})).timestamp(1000000);

        let result = transform.transform_envelope(envelope).unwrap();
        assert_eq!(result.timestamp, Some(1000000 + 3600000)); // +3600 seconds in ms
    }

    #[test]
    fn test_timestamp_subtract_transform() {
        let transform = TimestampSubtractTransform::new(300); // Subtract 5 minutes
        let envelope = MessageEnvelope::new(json!({})).timestamp(1000000);

        let result = transform.transform_envelope(envelope).unwrap();
        assert_eq!(result.timestamp, Some(1000000 - 300000)); // -300 seconds in ms
    }
}
