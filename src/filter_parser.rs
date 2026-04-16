//! Parsers for the declarative envelope-mutation config fields:
//! `key_transform`, `headers`, `header_transforms`, and `timestamp`.
//!
//! Filter and transform *expressions* are now Rhai scripts compiled through
//! [`crate::rhai_dsl::RhaiEngine`] — these parsers handle only the structured
//! config fields that mutate the Kafka envelope (key, headers, timestamp).

use crate::error::{MirrorMakerError, Result};
use crate::filter::{
    EnvelopeTransform, HeaderCopyTransform, HeaderFromTransform, HeaderRemoveTransform,
    HeaderSetTransform, KeyConstantTransform, KeyConstructTransform, KeyFromTransform,
    KeyHashTransform, KeyTemplateTransform, TimestampAddTransform, TimestampCurrentTransform,
    TimestampFromTransform, TimestampPreserveTransform, TimestampSubtractTransform,
};
use crate::hash::HashAlgorithm;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Key transform parser
// ============================================================================

/// Parse a `key_transform` expression into a compiled [`EnvelopeTransform`].
///
/// | Format | Description |
/// |---|---|
/// | `/path` | Extract value field as key |
/// | `{template-with-{/path}}` | Template with embedded path placeholders |
/// | `CONSTRUCT:f1=/p1:f2=/p2` | Build JSON key from multiple fields |
/// | `HASH:algorithm,/path` | Hash a field value as the key |
/// | anything else | Constant string key |
pub fn parse_key_transform(expr: &str) -> Result<Arc<dyn EnvelopeTransform>> {
    if let Some(rest) = expr.strip_prefix("CONSTRUCT:") {
        parse_key_construct_transform(rest)
    } else if let Some(rest) = expr.strip_prefix("HASH:") {
        parse_key_hash_transform(rest)
    } else if expr.starts_with('/') {
        Ok(Arc::new(KeyFromTransform::new(expr)?))
    } else if expr.contains("{/") {
        Ok(Arc::new(KeyTemplateTransform::new(expr)?))
    } else {
        Ok(Arc::new(KeyConstantTransform::new(expr)))
    }
}

fn parse_key_construct_transform(expr: &str) -> Result<Arc<dyn EnvelopeTransform>> {
    let mut fields = HashMap::new();
    for part in expr.split(':') {
        let (name, path) = part.split_once('=').ok_or_else(|| {
            MirrorMakerError::Config(format!(
                "CONSTRUCT field must be 'name=path'. Got: '{part}'"
            ))
        })?;
        fields.insert(name.to_string(), path.to_string());
    }
    Ok(Arc::new(KeyConstructTransform::new(fields)?))
}

fn parse_key_hash_transform(expr: &str) -> Result<Arc<dyn EnvelopeTransform>> {
    let (algo_str, path) = expr.split_once(',').ok_or_else(|| {
        MirrorMakerError::Config(format!(
            "HASH key transform requires 'algorithm,/path'. Got: '{expr}'"
        ))
    })?;
    let algorithm = HashAlgorithm::parse(algo_str)?;
    Ok(Arc::new(KeyHashTransform::new(path, algorithm)?))
}

// ============================================================================
// Static header parser
// ============================================================================

/// Build [`EnvelopeTransform`]s from a `headers:` map of constant values.
pub fn parse_static_headers(headers: &HashMap<String, String>) -> Vec<Arc<dyn EnvelopeTransform>> {
    headers
        .iter()
        .map(|(name, value)| {
            Arc::new(HeaderSetTransform::new(name, value)) as Arc<dyn EnvelopeTransform>
        })
        .collect()
}

// ============================================================================
// Header transform parser
// ============================================================================

/// Parse a single `header_transforms` operation into an [`EnvelopeTransform`].
///
/// | Operation | Description |
/// |---|---|
/// | `FROM:/path` | Extract header value from message payload field |
/// | `COPY:source-header` | Copy from another header |
/// | `REMOVE` | Remove the header |
pub fn parse_header_transform(
    header_name: &str,
    operation: &str,
) -> Result<Arc<dyn EnvelopeTransform>> {
    if let Some(path) = operation.strip_prefix("FROM:") {
        Ok(Arc::new(HeaderFromTransform::new(header_name, path)?))
    } else if let Some(source) = operation.strip_prefix("COPY:") {
        Ok(Arc::new(HeaderCopyTransform::new(source, header_name)))
    } else if operation == "REMOVE" {
        Ok(Arc::new(HeaderRemoveTransform::new(header_name)))
    } else {
        Err(MirrorMakerError::Config(format!(
            "Unknown header operation '{operation}'. Expected FROM:, COPY:, or REMOVE"
        )))
    }
}

// ============================================================================
// Timestamp transform parser
// ============================================================================

/// Parse a `timestamp:` expression into an [`EnvelopeTransform`].
///
/// | Value | Description |
/// |---|---|
/// | `PRESERVE` | Keep original timestamp (default) |
/// | `CURRENT` | Set to current wall-clock time |
/// | `FROM:/path` | Extract from a payload field |
/// | `ADD:seconds` | Add seconds to original timestamp |
/// | `SUBTRACT:seconds` | Subtract seconds from original timestamp |
pub fn parse_timestamp_transform(expr: &str) -> Result<Arc<dyn EnvelopeTransform>> {
    match expr {
        "PRESERVE" => Ok(Arc::new(TimestampPreserveTransform)),
        "CURRENT" => Ok(Arc::new(TimestampCurrentTransform)),
        _ if expr.starts_with("FROM:") => Ok(Arc::new(TimestampFromTransform::new(&expr[5..])?)),
        _ if expr.starts_with("ADD:") => {
            let s = expr[4..].parse::<i64>().map_err(|_| {
                MirrorMakerError::Config(format!("ADD: invalid seconds value '{}'", &expr[4..]))
            })?;
            Ok(Arc::new(TimestampAddTransform::new(s)))
        }
        _ if expr.starts_with("SUBTRACT:") => {
            let s = expr[9..].parse::<i64>().map_err(|_| {
                MirrorMakerError::Config(format!(
                    "SUBTRACT: invalid seconds value '{}'",
                    &expr[9..]
                ))
            })?;
            Ok(Arc::new(TimestampSubtractTransform::new(s)))
        }
        _ => Err(MirrorMakerError::Config(format!(
            "Unknown timestamp operation '{expr}'. \
             Expected PRESERVE, CURRENT, FROM:, ADD:, or SUBTRACT:"
        ))),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::MessageEnvelope;
    use serde_json::json;

    #[test]
    fn test_key_transform_path() {
        let t = parse_key_transform("/user/id").unwrap();
        let env = MessageEnvelope::new(json!({"user": {"id": "u1"}}));
        let out = t.transform_envelope(env).unwrap();
        assert_eq!(out.key, Some(json!("u1")));
    }

    #[test]
    fn test_key_transform_constant() {
        let t = parse_key_transform("my-constant-key").unwrap();
        let env = MessageEnvelope::new(json!({}));
        let out = t.transform_envelope(env).unwrap();
        assert_eq!(out.key, Some(json!("my-constant-key")));
    }

    #[test]
    fn test_key_transform_template() {
        let t = parse_key_transform("user-{/userId}").unwrap();
        let env = MessageEnvelope::new(json!({"userId": "42"}));
        let out = t.transform_envelope(env).unwrap();
        assert_eq!(out.key, Some(json!("user-42")));
    }

    #[test]
    fn test_key_transform_hash() {
        let t = parse_key_transform("HASH:MD5,/userId").unwrap();
        let env = MessageEnvelope::new(json!({"userId": "u1"}));
        let out = t.transform_envelope(env).unwrap();
        assert!(out.key.is_some());
    }

    #[test]
    fn test_static_headers() {
        let mut headers = HashMap::new();
        headers.insert("x-pipeline".to_string(), "streamforge".to_string());
        let transforms = parse_static_headers(&headers);
        assert_eq!(transforms.len(), 1);

        let env = MessageEnvelope::new(json!({}));
        let out = transforms[0].transform_envelope(env).unwrap();
        assert_eq!(
            out.header_str("x-pipeline"),
            Some("streamforge".to_string())
        );
    }

    #[test]
    fn test_header_from_transform() {
        let t = parse_header_transform("x-user-id", "FROM:/user/id").unwrap();
        let env = MessageEnvelope::new(json!({"user": {"id": "u1"}}));
        let out = t.transform_envelope(env).unwrap();
        assert_eq!(out.header_str("x-user-id"), Some("u1".to_string()));
    }

    #[test]
    fn test_header_copy_transform() {
        let t = parse_header_transform("x-copy", "COPY:x-source").unwrap();
        let env = MessageEnvelope::new(json!({})).with_header_str("x-source".to_string(), "val");
        let out = t.transform_envelope(env).unwrap();
        assert_eq!(out.header_str("x-copy"), Some("val".to_string()));
    }

    #[test]
    fn test_header_remove_transform() {
        let t = parse_header_transform("x-remove", "REMOVE").unwrap();
        let env = MessageEnvelope::new(json!({})).with_header_str("x-remove".to_string(), "v");
        assert!(env.has_header("x-remove"));
        let out = t.transform_envelope(env).unwrap();
        assert!(!out.has_header("x-remove"));
    }

    #[test]
    fn test_timestamp_preserve() {
        let t = parse_timestamp_transform("PRESERVE").unwrap();
        let mut env = MessageEnvelope::new(json!({}));
        env.timestamp = Some(12345);
        let out = t.transform_envelope(env).unwrap();
        assert_eq!(out.timestamp, Some(12345));
    }

    #[test]
    fn test_timestamp_current() {
        let t = parse_timestamp_transform("CURRENT").unwrap();
        let env = MessageEnvelope::new(json!({}));
        let out = t.transform_envelope(env).unwrap();
        assert!(out.timestamp.is_some());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        assert!((out.timestamp.unwrap() - now).abs() < 1000);
    }

    #[test]
    fn test_timestamp_add() {
        let t = parse_timestamp_transform("ADD:60").unwrap();
        let mut env = MessageEnvelope::new(json!({}));
        env.timestamp = Some(1_000_000);
        let out = t.transform_envelope(env).unwrap();
        assert_eq!(out.timestamp, Some(1_060_000));
    }

    #[test]
    fn test_timestamp_subtract() {
        let t = parse_timestamp_transform("SUBTRACT:30").unwrap();
        let mut env = MessageEnvelope::new(json!({}));
        env.timestamp = Some(1_000_000);
        let out = t.transform_envelope(env).unwrap();
        assert_eq!(out.timestamp, Some(970_000));
    }
}
