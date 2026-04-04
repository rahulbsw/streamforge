use crate::envelope::MessageEnvelope;
use crate::error::{MirrorMakerError, Result};
use crate::filter::Filter;
use regex::Regex;
use serde_json::Value;

// ============================================================================
// KEY FILTERS
// ============================================================================

/// Filter that checks if key matches a regex pattern
///
/// Example:
/// ```ignore
/// use streamforge::filter::KeyMatchesFilter;
///
/// // Match keys starting with "user-"
/// let filter = KeyMatchesFilter::new(r"^user-.*").unwrap();
/// ```
pub struct KeyMatchesFilter {
    regex: Regex,
}

impl KeyMatchesFilter {
    pub fn new(pattern: &str) -> Result<Self> {
        let regex = Regex::new(pattern)
            .map_err(|e| MirrorMakerError::Config(format!("Invalid regex pattern: {}", e)))?;
        Ok(Self { regex })
    }
}

impl Filter for KeyMatchesFilter {
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        if let Some(key) = &envelope.key {
            // Convert key to string for matching
            let key_str = match key {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => serde_json::to_string(key).unwrap_or_default(),
            };
            Ok(self.regex.is_match(&key_str))
        } else {
            Ok(false) // No key = no match
        }
    }
}

/// Filter that checks if key starts with a prefix
///
/// Example:
/// ```ignore
/// use streamforge::filter::KeyPrefixFilter;
///
/// // Match keys starting with "premium-"
/// let filter = KeyPrefixFilter::new("premium-");
/// ```
pub struct KeyPrefixFilter {
    prefix: String,
}

impl KeyPrefixFilter {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }
}

impl Filter for KeyPrefixFilter {
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        if let Some(key) = &envelope.key {
            let key_str = match key {
                Value::String(s) => s.clone(),
                _ => serde_json::to_string(key).unwrap_or_default(),
            };
            Ok(key_str.starts_with(&self.prefix))
        } else {
            Ok(false)
        }
    }
}

/// Filter that checks if key ends with a suffix
///
/// Example:
/// ```ignore
/// use streamforge::filter::KeySuffixFilter;
///
/// // Match keys ending with "-prod"
/// let filter = KeySuffixFilter::new("-prod");
/// ```
pub struct KeySuffixFilter {
    suffix: String,
}

impl KeySuffixFilter {
    pub fn new(suffix: &str) -> Self {
        Self {
            suffix: suffix.to_string(),
        }
    }
}

impl Filter for KeySuffixFilter {
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        if let Some(key) = &envelope.key {
            let key_str = match key {
                Value::String(s) => s.clone(),
                _ => serde_json::to_string(key).unwrap_or_default(),
            };
            Ok(key_str.ends_with(&self.suffix))
        } else {
            Ok(false)
        }
    }
}

/// Filter that checks if key contains a substring
///
/// Example:
/// ```ignore
/// use streamforge::filter::KeyContainsFilter;
///
/// // Match keys containing "test"
/// let filter = KeyContainsFilter::new("test");
/// ```
pub struct KeyContainsFilter {
    substring: String,
}

impl KeyContainsFilter {
    pub fn new(substring: &str) -> Self {
        Self {
            substring: substring.to_string(),
        }
    }
}

impl Filter for KeyContainsFilter {
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        if let Some(key) = &envelope.key {
            let key_str = match key {
                Value::String(s) => s.clone(),
                _ => serde_json::to_string(key).unwrap_or_default(),
            };
            Ok(key_str.contains(&self.substring))
        } else {
            Ok(false)
        }
    }
}

/// Filter that checks if key exists (is not null)
///
/// Example:
/// ```ignore
/// use streamforge::filter::KeyExistsFilter;
///
/// let filter = KeyExistsFilter;
/// ```
pub struct KeyExistsFilter;

impl Filter for KeyExistsFilter {
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        Ok(envelope.key.is_some())
    }
}

// ============================================================================
// HEADER FILTERS
// ============================================================================

/// Filter that checks if a header exists
///
/// Example:
/// ```ignore
/// use streamforge::filter::HeaderExistsFilter;
///
/// // Check if x-tenant header exists
/// let filter = HeaderExistsFilter::new("x-tenant");
/// ```
pub struct HeaderExistsFilter {
    header_name: String,
}

impl HeaderExistsFilter {
    pub fn new(header_name: &str) -> Self {
        Self {
            header_name: header_name.to_string(),
        }
    }
}

impl Filter for HeaderExistsFilter {
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        Ok(envelope.has_header(&self.header_name))
    }
}

/// Filter that checks if a header matches a value
///
/// Example:
/// ```ignore
/// use streamforge::filter::HeaderFilter;
///
/// // Check if x-tenant header equals "production"
/// let filter = HeaderFilter::new("x-tenant", "==", "production").unwrap();
/// ```
pub struct HeaderFilter {
    header_name: String,
    operator: ComparisonOp,
    expected_value: String,
}

#[derive(Debug, Clone)]
enum ComparisonOp {
    Eq,
    Ne,
}

impl HeaderFilter {
    pub fn new(header_name: &str, operator: &str, expected_value: &str) -> Result<Self> {
        let op = match operator {
            "==" => ComparisonOp::Eq,
            "!=" => ComparisonOp::Ne,
            _ => {
                return Err(MirrorMakerError::Config(format!(
                    "Unknown header operator: {}. Expected == or !=",
                    operator
                )))
            }
        };

        Ok(Self {
            header_name: header_name.to_string(),
            operator: op,
            expected_value: expected_value.to_string(),
        })
    }
}

impl Filter for HeaderFilter {
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        if let Some(actual_value) = envelope.header_str(&self.header_name) {
            match self.operator {
                ComparisonOp::Eq => Ok(actual_value == self.expected_value),
                ComparisonOp::Ne => Ok(actual_value != self.expected_value),
            }
        } else {
            // Header doesn't exist
            match self.operator {
                ComparisonOp::Eq => Ok(false), // Missing != expected = false
                ComparisonOp::Ne => Ok(true),  // Missing != expected = true
            }
        }
    }
}

// ============================================================================
// TIMESTAMP FILTERS
// ============================================================================

/// Filter that checks message age in seconds
///
/// Example:
/// ```ignore
/// use streamforge::filter::TimestampAgeFilter;
///
/// // Match messages older than 300 seconds (5 minutes)
/// let filter = TimestampAgeFilter::new(">", 300).unwrap();
/// ```
pub struct TimestampAgeFilter {
    operator: AgeOp,
    threshold_seconds: i64,
}

#[derive(Debug, Clone)]
enum AgeOp {
    Lt,  // <  (younger than)
    Lte, // <= (younger than or equal)
    Gt,  // >  (older than)
    Gte, // >= (older than or equal)
}

impl TimestampAgeFilter {
    pub fn new(operator: &str, threshold_seconds: i64) -> Result<Self> {
        let op = match operator {
            "<" => AgeOp::Lt,
            "<=" => AgeOp::Lte,
            ">" => AgeOp::Gt,
            ">=" => AgeOp::Gte,
            _ => {
                return Err(MirrorMakerError::Config(format!(
                    "Unknown age operator: {}. Expected <, <=, >, or >=",
                    operator
                )))
            }
        };

        Ok(Self {
            operator: op,
            threshold_seconds,
        })
    }
}

impl Filter for TimestampAgeFilter {
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        if let Some(age) = envelope.age_seconds() {
            match self.operator {
                AgeOp::Lt => Ok(age < self.threshold_seconds),
                AgeOp::Lte => Ok(age <= self.threshold_seconds),
                AgeOp::Gt => Ok(age > self.threshold_seconds),
                AgeOp::Gte => Ok(age >= self.threshold_seconds),
            }
        } else {
            Ok(false) // No timestamp = no match
        }
    }
}

/// Filter that checks if timestamp is after a specific time
///
/// Example:
/// ```ignore
/// use streamforge::filter::TimestampAfterFilter;
///
/// // Match messages after 2024-01-01
/// let filter = TimestampAfterFilter::new(1704067200000); // epoch ms
/// ```
pub struct TimestampAfterFilter {
    threshold_ms: i64,
}

impl TimestampAfterFilter {
    pub fn new(threshold_ms: i64) -> Self {
        Self { threshold_ms }
    }
}

impl Filter for TimestampAfterFilter {
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        if let Some(ts) = envelope.timestamp {
            Ok(ts > self.threshold_ms)
        } else {
            Ok(false)
        }
    }
}

/// Filter that checks if timestamp is before a specific time
///
/// Example:
/// ```ignore
/// use streamforge::filter::TimestampBeforeFilter;
///
/// // Match messages before 2024-01-01
/// let filter = TimestampBeforeFilter::new(1704067200000); // epoch ms
/// ```
pub struct TimestampBeforeFilter {
    threshold_ms: i64,
}

impl TimestampBeforeFilter {
    pub fn new(threshold_ms: i64) -> Self {
        Self { threshold_ms }
    }
}

impl Filter for TimestampBeforeFilter {
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        if let Some(ts) = envelope.timestamp {
            Ok(ts < self.threshold_ms)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_key_matches_filter() {
        let filter = KeyMatchesFilter::new(r"^user-\d+$").unwrap();

        let mut envelope1 = MessageEnvelope::new(json!({}));
        envelope1.key = Some(json!("user-123"));
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let mut envelope2 = MessageEnvelope::new(json!({}));
        envelope2.key = Some(json!("admin-456"));
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());

        let envelope3 = MessageEnvelope::new(json!({}));
        assert!(!filter.evaluate_envelope(&envelope3).unwrap());
    }

    #[test]
    fn test_key_prefix_filter() {
        let filter = KeyPrefixFilter::new("premium-");

        let mut envelope1 = MessageEnvelope::new(json!({}));
        envelope1.key = Some(json!("premium-user"));
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let mut envelope2 = MessageEnvelope::new(json!({}));
        envelope2.key = Some(json!("basic-user"));
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());
    }

    #[test]
    fn test_key_suffix_filter() {
        let filter = KeySuffixFilter::new("-prod");

        let mut envelope1 = MessageEnvelope::new(json!({}));
        envelope1.key = Some(json!("service-prod"));
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let mut envelope2 = MessageEnvelope::new(json!({}));
        envelope2.key = Some(json!("service-test"));
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());
    }

    #[test]
    fn test_key_contains_filter() {
        let filter = KeyContainsFilter::new("test");

        let mut envelope1 = MessageEnvelope::new(json!({}));
        envelope1.key = Some(json!("my-test-key"));
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let mut envelope2 = MessageEnvelope::new(json!({}));
        envelope2.key = Some(json!("my-prod-key"));
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());
    }

    #[test]
    fn test_key_exists_filter() {
        let filter = KeyExistsFilter;

        let mut envelope1 = MessageEnvelope::new(json!({}));
        envelope1.key = Some(json!("any-key"));
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let envelope2 = MessageEnvelope::new(json!({}));
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());
    }

    #[test]
    fn test_header_exists_filter() {
        let filter = HeaderExistsFilter::new("x-tenant");

        let envelope1 =
            MessageEnvelope::new(json!({})).with_header_str("x-tenant".to_string(), "production");
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let envelope2 = MessageEnvelope::new(json!({}));
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());
    }

    #[test]
    fn test_header_filter_equals() {
        let filter = HeaderFilter::new("x-tenant", "==", "production").unwrap();

        let envelope1 =
            MessageEnvelope::new(json!({})).with_header_str("x-tenant".to_string(), "production");
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let envelope2 =
            MessageEnvelope::new(json!({})).with_header_str("x-tenant".to_string(), "test");
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());

        let envelope3 = MessageEnvelope::new(json!({}));
        assert!(!filter.evaluate_envelope(&envelope3).unwrap());
    }

    #[test]
    fn test_header_filter_not_equals() {
        let filter = HeaderFilter::new("x-tenant", "!=", "production").unwrap();

        let envelope1 =
            MessageEnvelope::new(json!({})).with_header_str("x-tenant".to_string(), "test");
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let envelope2 =
            MessageEnvelope::new(json!({})).with_header_str("x-tenant".to_string(), "production");
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());

        // Missing header != "production" should be true
        let envelope3 = MessageEnvelope::new(json!({}));
        assert!(filter.evaluate_envelope(&envelope3).unwrap());
    }

    #[test]
    fn test_timestamp_age_filter() {
        let filter = TimestampAgeFilter::new(">", 100).unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Old message (200 seconds ago)
        let envelope1 = MessageEnvelope::new(json!({})).timestamp(now - 200_000);
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        // Recent message (50 seconds ago)
        let envelope2 = MessageEnvelope::new(json!({})).timestamp(now - 50_000);
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());

        // No timestamp
        let envelope3 = MessageEnvelope::new(json!({}));
        assert!(!filter.evaluate_envelope(&envelope3).unwrap());
    }

    #[test]
    fn test_timestamp_after_filter() {
        let threshold = 1704067200000i64; // 2024-01-01 00:00:00 UTC

        let filter = TimestampAfterFilter::new(threshold);

        let envelope1 = MessageEnvelope::new(json!({})).timestamp(threshold + 1000);
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let envelope2 = MessageEnvelope::new(json!({})).timestamp(threshold - 1000);
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());
    }

    #[test]
    fn test_timestamp_before_filter() {
        let threshold = 1704067200000i64; // 2024-01-01 00:00:00 UTC

        let filter = TimestampBeforeFilter::new(threshold);

        let envelope1 = MessageEnvelope::new(json!({})).timestamp(threshold - 1000);
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let envelope2 = MessageEnvelope::new(json!({})).timestamp(threshold + 1000);
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());
    }
}
