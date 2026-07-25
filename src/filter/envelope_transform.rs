use crate::envelope::MessageEnvelope;
use crate::error::{MirrorMakerError, Result};
use crate::hash::{hash_value, HashAlgorithm};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

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
    value_path_segments: Vec<String>,
}

impl KeyFromTransform {
    pub fn new(value_path: &str) -> Result<Self> {
        // Pre-parse path segments to avoid allocation on every message
        let value_path_segments: Vec<String> = value_path
            .trim_matches('/')
            .split('/')
            .map(|s| s.to_string())
            .collect();

        Ok(Self {
            value_path: value_path.to_string(),
            value_path_segments,
        })
    }

    fn extract_from_value(&self, value: &Value) -> Option<Value> {
        let mut current = value;
        for part in &self.value_path_segments {
            current = current.get(part.as_str())?;
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
    tokens: Vec<KeyTemplateToken>,
}

#[derive(Debug)]
enum KeyTemplateToken {
    Literal(String),
    Placeholder {
        placeholder: String,
        path: String,
        path_segments: Box<[String]>,
    },
}

impl KeyTemplateTransform {
    pub fn new(template: &str) -> Result<Self> {
        static PLACEHOLDER_REGEX: OnceLock<regex::Regex> = OnceLock::new();
        let placeholder_regex = PLACEHOLDER_REGEX.get_or_init(|| {
            regex::Regex::new(r"\{(/[^}]+)\}")
                .expect("the static key-template placeholder regex must be valid")
        });

        let mut tokens = Vec::new();
        let mut cursor = 0;
        for captures in placeholder_regex.captures_iter(template) {
            let placeholder = captures
                .get(0)
                .expect("capture group zero always contains the full match");
            let path = captures
                .get(1)
                .expect("the key-template regex always captures the placeholder path")
                .as_str();

            if placeholder.start() > cursor {
                tokens.push(KeyTemplateToken::Literal(
                    template[cursor..placeholder.start()].to_string(),
                ));
            }
            tokens.push(KeyTemplateToken::Placeholder {
                placeholder: placeholder.as_str().to_string(),
                path: path.to_string(),
                path_segments: path
                    .trim_matches('/')
                    .split('/')
                    .map(str::to_string)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            });
            cursor = placeholder.end();
        }

        if cursor < template.len() || tokens.is_empty() {
            tokens.push(KeyTemplateToken::Literal(template[cursor..].to_string()));
        }

        Ok(Self {
            template: template.to_string(),
            tokens,
        })
    }

    fn apply_template(&self, value: &Value) -> Result<String> {
        let mut rendered_values = Vec::new();
        for token in &self.tokens {
            if let KeyTemplateToken::Placeholder {
                path,
                path_segments,
                ..
            } = token
            {
                let extracted = Self::extract_from_path(value, path, path_segments)?;
                let value_str = match extracted {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => serde_json::to_string(extracted).unwrap_or_default(),
                };
                rendered_values.push(value_str);
            }
        }

        // The previous implementation applied replacements sequentially. Preserve
        // that edge-case behavior when a value itself contains a placeholder.
        let requires_sequential_rewrite = rendered_values.iter().any(|rendered| {
            self.tokens.iter().any(|token| match token {
                KeyTemplateToken::Placeholder { placeholder, .. } => rendered.contains(placeholder),
                KeyTemplateToken::Literal(_) => false,
            })
        });
        if requires_sequential_rewrite {
            let mut result = self.template.clone();
            let mut rendered = rendered_values.iter();
            for token in &self.tokens {
                if let KeyTemplateToken::Placeholder { placeholder, .. } = token {
                    let value = rendered
                        .next()
                        .expect("each placeholder has one pre-rendered value");
                    result = result.replace(placeholder, value);
                }
            }
            return Ok(result);
        }

        let mut result = String::with_capacity(self.template.len());
        let mut rendered = rendered_values.iter();
        for token in &self.tokens {
            match token {
                KeyTemplateToken::Literal(literal) => result.push_str(literal),
                KeyTemplateToken::Placeholder { .. } => {
                    result.push_str(
                        rendered
                            .next()
                            .expect("each placeholder has one pre-rendered value"),
                    );
                }
            }
        }
        Ok(result)
    }

    fn extract_from_path<'a>(
        value: &'a Value,
        path: &str,
        path_segments: &[String],
    ) -> Result<&'a Value> {
        let mut current = value;
        for part in path_segments {
            current = current
                .get(part)
                .ok_or_else(|| MirrorMakerError::Processing(format!("Path not found: {}", path)))?;
        }
        Ok(current)
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
    value_path_segments: Vec<String>,
    algorithm: HashAlgorithm,
}

impl KeyHashTransform {
    pub fn new(value_path: &str, algorithm: HashAlgorithm) -> Result<Self> {
        // Pre-parse path segments to avoid allocation on every message
        let value_path_segments: Vec<String> = value_path
            .trim_matches('/')
            .split('/')
            .map(|s| s.to_string())
            .collect();

        Ok(Self {
            value_path: value_path.to_string(),
            value_path_segments,
            algorithm,
        })
    }

    fn extract_from_value(&self, value: &Value) -> Option<Value> {
        let mut current = value;
        for part in &self.value_path_segments {
            current = current.get(part.as_str())?;
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
    fields: HashMap<String, (String, Vec<String>)>, // (path_string, pre-parsed_segments)
}

impl KeyConstructTransform {
    pub fn new(fields: HashMap<String, String>) -> Result<Self> {
        // Pre-parse all paths to avoid allocation on every message
        let fields = fields
            .into_iter()
            .map(|(key, path)| {
                let segments: Vec<String> = path
                    .trim_matches('/')
                    .split('/')
                    .map(|s| s.to_string())
                    .collect();
                (key, (path, segments))
            })
            .collect();

        Ok(Self { fields })
    }

    fn extract_from_path(&self, value: &Value, segments: &[String]) -> Option<Value> {
        let mut current = value;
        for part in segments {
            current = current.get(part.as_str())?;
        }
        Some(current.clone())
    }
}

impl EnvelopeTransform for KeyConstructTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        let mut result = Map::new();

        for (output_name, (_path, segments)) in &self.fields {
            if let Some(extracted) = self.extract_from_path(&envelope.value, segments) {
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
        Arc::make_mut(&mut envelope.headers)
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
    value_path_segments: Vec<String>,
}

impl HeaderFromTransform {
    pub fn new(header_name: &str, value_path: &str) -> Result<Self> {
        // Pre-parse path segments to avoid allocation on every message
        let value_path_segments: Vec<String> = value_path
            .trim_matches('/')
            .split('/')
            .map(|s| s.to_string())
            .collect();

        Ok(Self {
            header_name: header_name.to_string(),
            value_path: value_path.to_string(),
            value_path_segments,
        })
    }

    fn extract_from_value(&self, value: &Value) -> Option<String> {
        let mut current = value;
        for part in &self.value_path_segments {
            current = current.get(part.as_str())?;
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
            Arc::make_mut(&mut envelope.headers)
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
            Arc::make_mut(&mut envelope.headers).insert(self.dest_header.clone(), value);
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
        Arc::make_mut(&mut envelope.headers).remove(&self.header_name);
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
    value_path_segments: Vec<String>,
}

impl TimestampFromTransform {
    pub fn new(value_path: &str) -> Result<Self> {
        // Pre-parse path segments to avoid allocation on every message
        let value_path_segments: Vec<String> = value_path
            .trim_matches('/')
            .split('/')
            .map(|s| s.to_string())
            .collect();

        Ok(Self {
            value_path: value_path.to_string(),
            value_path_segments,
        })
    }

    fn extract_timestamp(&self, value: &Value) -> Option<i64> {
        let mut current = value;
        for part in &self.value_path_segments {
            current = current.get(part.as_str())?;
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
    fn test_key_template_transform_tokenizes_paths_at_startup() {
        let transform = KeyTemplateTransform::new("tenant-{/tenant}/user-{/user/id}").unwrap();
        assert_eq!(transform.tokens.len(), 4);

        let KeyTemplateToken::Placeholder {
            path,
            path_segments,
            ..
        } = &transform.tokens[3]
        else {
            panic!("expected the final token to be a placeholder");
        };
        assert_eq!(path, "/user/id");
        assert_eq!(
            path_segments.as_ref(),
            &["user".to_string(), "id".to_string()]
        );

        let envelope = MessageEnvelope::new(json!({
            "tenant": "acme",
            "user": {"id": 42}
        }));
        let result = transform.transform_envelope(envelope).unwrap();
        assert_eq!(result.key, Some(json!("tenant-acme/user-42")));
    }

    #[test]
    fn test_key_template_transform_preserves_sequential_replacement_semantics() {
        let transform = KeyTemplateTransform::new("{/first}-{/second}").unwrap();
        let envelope = MessageEnvelope::new(json!({
            "first": "{/second}",
            "second": "resolved"
        }));

        let result = transform.transform_envelope(envelope).unwrap();
        assert_eq!(result.key, Some(json!("resolved-resolved")));
    }

    #[test]
    fn test_key_template_transform_preserves_unmatched_placeholder_literal() {
        let transform = KeyTemplateTransform::new("literal-{/}").unwrap();
        let result = transform
            .transform_envelope(MessageEnvelope::new(json!({})))
            .unwrap();
        assert_eq!(result.key, Some(json!("literal-{/}")));
    }

    #[test]
    fn test_key_template_transform_preserves_missing_path_error() {
        let transform = KeyTemplateTransform::new("user-{/user/id}").unwrap();
        let error = transform
            .transform_envelope(MessageEnvelope::new(json!({})))
            .expect_err("missing template path must fail");
        assert_eq!(
            error.to_string(),
            "Processing error: Path not found: /user/id"
        );
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
