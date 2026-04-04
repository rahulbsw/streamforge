use crate::error::{MirrorMakerError, Result};
use crate::filter::{
    AndFilter, ArithmeticOp, ArithmeticTransform, ArrayFilter, ArrayFilterMode, ArrayMapTransform,
    EnvelopeTransform, Filter, HashTransform, HeaderCopyTransform, HeaderExistsFilter,
    HeaderFilter, HeaderFromTransform, HeaderRemoveTransform, HeaderSetTransform, JsonPathFilter,
    JsonPathTransform, KeyConstantTransform, KeyConstructTransform, KeyContainsFilter,
    KeyExistsFilter, KeyFromTransform, KeyHashTransform, KeyMatchesFilter, KeyPrefixFilter,
    KeySuffixFilter, KeyTemplateTransform, NotFilter, ObjectConstructTransform, OrFilter,
    RegexFilter, TimestampAddTransform, TimestampAfterFilter, TimestampAgeFilter,
    TimestampBeforeFilter, TimestampCurrentTransform, TimestampFromTransform,
    TimestampPreserveTransform, TimestampSubtractTransform, Transform,
};
use crate::hash::HashAlgorithm;
use std::collections::HashMap;
use std::sync::Arc;

/// Parse filter expression from string
///
/// Formats:
/// - Simple: "path,op,value"
/// - AND: "AND:cond1:cond2:cond3"
/// - OR: "OR:cond1:cond2:cond3"
/// - NOT: "NOT:cond"
/// - REGEX: "REGEX:/path,pattern"
/// - ARRAY_ALL: "ARRAY_ALL:/path,element_filter"
/// - ARRAY_ANY: "ARRAY_ANY:/path,element_filter"
/// - KEY_MATCHES: "KEY_MATCHES:pattern"
/// - KEY_PREFIX: "KEY_PREFIX:prefix"
/// - KEY_SUFFIX: "KEY_SUFFIX:suffix"
/// - KEY_CONTAINS: "KEY_CONTAINS:substring"
/// - KEY_EXISTS: "KEY_EXISTS"
/// - HEADER_EXISTS: "HEADER_EXISTS:name"
/// - HEADER: "HEADER:name,op,value"
/// - TIMESTAMP_AGE: "TIMESTAMP_AGE:op,seconds"
/// - TIMESTAMP_AFTER: "TIMESTAMP_AFTER:epoch_ms"
/// - TIMESTAMP_BEFORE: "TIMESTAMP_BEFORE:epoch_ms"
pub fn parse_filter(expr: &str) -> Result<Arc<dyn Filter>> {
    Ok(Arc::from(parse_filter_as_box(expr)?))
}

/// Internal helper that returns Box instead of Arc
fn parse_filter_as_box(expr: &str) -> Result<Box<dyn Filter>> {
    let parts: Vec<&str> = expr.split(':').collect();

    if parts.is_empty() {
        return Err(MirrorMakerError::Config(
            "Empty filter expression".to_string(),
        ));
    }

    match parts[0] {
        "AND" => parse_and_filter(&parts[1..]),
        "OR" => parse_or_filter(&parts[1..]),
        "NOT" => parse_not_filter(&parts[1..]),
        "REGEX" => parse_regex_filter(&parts[1..]),
        "ARRAY_ALL" => parse_array_filter(&parts[1..], ArrayFilterMode::All),
        "ARRAY_ANY" => parse_array_filter(&parts[1..], ArrayFilterMode::Any),
        // Key filters
        "KEY_MATCHES" => parse_key_matches_filter(&parts[1..]),
        "KEY_PREFIX" => parse_key_prefix_filter(&parts[1..]),
        "KEY_SUFFIX" => parse_key_suffix_filter(&parts[1..]),
        "KEY_CONTAINS" => parse_key_contains_filter(&parts[1..]),
        "KEY_EXISTS" => Ok(Box::new(KeyExistsFilter)),
        // Header filters
        "HEADER_EXISTS" => parse_header_exists_filter(&parts[1..]),
        "HEADER" => parse_header_filter(&parts[1..]),
        // Timestamp filters
        "TIMESTAMP_AGE" => parse_timestamp_age_filter(&parts[1..]),
        "TIMESTAMP_AFTER" => parse_timestamp_after_filter(&parts[1..]),
        "TIMESTAMP_BEFORE" => parse_timestamp_before_filter(&parts[1..]),
        _ => parse_simple_filter(expr),
    }
}

/// Reconstructs a filter expression from split parts
///
/// Returns (filter_expr, num_parts_consumed)
///
/// Handles envelope filters that use ":" in their syntax
fn reconstruct_filter_expr(parts: &[&str]) -> Result<(String, usize)> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config("Empty condition".to_string()));
    }

    match parts[0] {
        // Single-part filters
        "KEY_EXISTS" => Ok((parts[0].to_string(), 1)),

        // Two-part filters (keyword:value)
        "KEY_PREFIX" | "KEY_SUFFIX" | "KEY_CONTAINS" | "KEY_MATCHES" | "HEADER_EXISTS" => {
            if parts.len() < 2 {
                return Err(MirrorMakerError::Config(format!(
                    "{} filter requires additional parameters",
                    parts[0]
                )));
            }
            Ok((format!("{}:{}", parts[0], parts[1]), 2))
        }

        // Two-part filters (keyword:number)
        "TIMESTAMP_AFTER" | "TIMESTAMP_BEFORE" => {
            if parts.len() < 2 {
                return Err(MirrorMakerError::Config(format!(
                    "{} filter requires epoch_ms parameter",
                    parts[0]
                )));
            }
            Ok((format!("{}:{}", parts[0], parts[1]), 2))
        }

        // Three-part filters with comma-separated values
        // These need special handling as they contain commas
        "HEADER" | "TIMESTAMP_AGE" => {
            // Reconstruct until we find a complete expression
            // HEADER:name,op,value or TIMESTAMP_AGE:op,seconds
            if parts.len() < 2 {
                return Err(MirrorMakerError::Config(format!(
                    "{} filter requires additional parameters",
                    parts[0]
                )));
            }
            // Join the second part onwards until we have a valid expression
            let rest = parts[1..].join(":");
            Ok((format!("{}:{}", parts[0], rest), parts.len()))
        }

        // Default: simple filter (path,op,value) or other filter type
        _ => {
            // If it contains commas, it's likely a simple filter - return as-is
            if parts[0].contains(',') {
                Ok((parts[0].to_string(), 1))
            } else {
                // Otherwise, it might be a path-based filter, return as-is
                Ok((parts[0].to_string(), 1))
            }
        }
    }
}

fn parse_simple_filter(expr: &str) -> Result<Box<dyn Filter>> {
    let parts: Vec<&str> = expr.split(',').collect();
    if parts.len() != 3 {
        return Err(MirrorMakerError::Config(format!(
            "Invalid filter format: {}. Expected 'path,operator,value'",
            expr
        )));
    }

    Ok(Box::new(JsonPathFilter::new(parts[0], parts[1], parts[2])?))
}

fn parse_and_filter(conditions: &[&str]) -> Result<Box<dyn Filter>> {
    if conditions.is_empty() {
        return Err(MirrorMakerError::Config(
            "AND filter requires at least one condition".to_string(),
        ));
    }

    let mut filters: Vec<Box<dyn Filter>> = Vec::new();

    let mut i = 0;
    while i < conditions.len() {
        // Check if this is a nested OR or NOT
        if conditions[i] == "OR" {
            // Find the extent of the OR (until next AND/OR/NOT or end)
            let mut or_end = i + 1;
            while or_end < conditions.len() && !matches!(conditions[or_end], "AND" | "OR" | "NOT") {
                or_end += 1;
            }
            let or_filter = parse_or_filter(&conditions[i + 1..or_end])?;
            filters.push(or_filter);
            i = or_end;
        } else if conditions[i] == "NOT" {
            if i + 1 >= conditions.len() {
                return Err(MirrorMakerError::Config(
                    "NOT requires a condition".to_string(),
                ));
            }
            // Check if it's an envelope filter that needs multiple parts
            let (filter_str, consumed) = reconstruct_filter_expr(&conditions[i + 1..])?;
            let parsed = parse_filter_as_box(&filter_str)?;
            filters.push(Box::new(NotFilter::new(parsed)));
            i += 1 + consumed;
        } else {
            // Could be any type of filter - may need multiple parts
            let (filter_str, consumed) = reconstruct_filter_expr(&conditions[i..])?;
            let parsed = parse_filter_as_box(&filter_str)?;
            filters.push(parsed);
            i += consumed;
        }
    }

    Ok(Box::new(AndFilter::new(filters)))
}

fn parse_or_filter(conditions: &[&str]) -> Result<Box<dyn Filter>> {
    if conditions.is_empty() {
        return Err(MirrorMakerError::Config(
            "OR filter requires at least one condition".to_string(),
        ));
    }

    let mut filters: Vec<Box<dyn Filter>> = Vec::new();

    let mut i = 0;
    while i < conditions.len() {
        // May need multiple parts for envelope filters
        let (filter_str, consumed) = reconstruct_filter_expr(&conditions[i..])?;
        let parsed = parse_filter_as_box(&filter_str)?;
        filters.push(parsed);
        i += consumed;
    }

    Ok(Box::new(OrFilter::new(filters)))
}

fn parse_not_filter(conditions: &[&str]) -> Result<Box<dyn Filter>> {
    if conditions.len() != 1 {
        return Err(MirrorMakerError::Config(
            "NOT filter requires exactly one condition".to_string(),
        ));
    }

    // Recursively parse the condition (could be envelope or value filter)
    let parsed = parse_filter_as_box(conditions[0])?;
    Ok(Box::new(NotFilter::new(parsed)))
}

fn parse_regex_filter(parts: &[&str]) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config(
            "REGEX filter requires path and pattern".to_string(),
        ));
    }

    let combined = parts.join(":");
    let filter_parts: Vec<&str> = combined.split(',').collect();

    if filter_parts.len() != 2 {
        return Err(MirrorMakerError::Config(format!(
            "Invalid REGEX format: {}. Expected 'REGEX:/path,pattern'",
            combined
        )));
    }

    Ok(Box::new(RegexFilter::new(
        filter_parts[0],
        filter_parts[1],
    )?))
}

fn parse_array_filter(parts: &[&str], mode: ArrayFilterMode) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config(
            "ARRAY filter requires path and element filter".to_string(),
        ));
    }

    let combined = parts.join(":");
    let filter_parts: Vec<&str> = combined.split(',').collect();

    if filter_parts.len() < 2 {
        return Err(MirrorMakerError::Config(format!(
            "Invalid ARRAY filter format: {}. Expected 'ARRAY_*:/path,element_filter'",
            combined
        )));
    }

    let path = filter_parts[0];
    let element_filter_expr = filter_parts[1..].join(",");
    let element_filter = parse_simple_filter(&element_filter_expr)?;

    Ok(Box::new(ArrayFilter::new(path, element_filter, mode)?))
}

/// Parse transform expression from string
///
/// Formats:
/// - Simple path: "/message" or "/message/confId"
/// - Object construction: "CONSTRUCT:field1=/path1:field2=/path2"
/// - Array map: "ARRAY_MAP:/path,element_transform"
/// - Arithmetic: "ARITHMETIC:op,operand1,operand2"
///   - op: ADD, SUB, MUL, DIV
///   - operands: /path or numeric constant
/// - Hash: "HASH:algorithm,/path" or "HASH:algorithm,/path,output_field"
///   - algorithm: MD5, SHA256, SHA512, MURMUR64, MURMUR128
pub fn parse_transform(expr: &str) -> Result<Arc<dyn Transform>> {
    if let Some(rest) = expr.strip_prefix("CONSTRUCT:") {
        parse_construct_transform(rest)
    } else if let Some(rest) = expr.strip_prefix("ARRAY_MAP:") {
        parse_array_map_transform(rest)
    } else if let Some(rest) = expr.strip_prefix("ARITHMETIC:") {
        parse_arithmetic_transform(rest)
    } else if let Some(rest) = expr.strip_prefix("HASH:") {
        parse_hash_transform(rest)
    } else {
        Ok(Arc::new(JsonPathTransform::new(expr)?))
    }
}

fn parse_construct_transform(expr: &str) -> Result<Arc<dyn Transform>> {
    let parts: Vec<&str> = expr.split(':').collect();
    let mut fields = HashMap::new();

    for part in parts {
        let field_parts: Vec<&str> = part.split('=').collect();
        if field_parts.len() != 2 {
            return Err(MirrorMakerError::Config(format!(
                "Invalid CONSTRUCT format: {}. Expected 'field=path'",
                part
            )));
        }
        fields.insert(field_parts[0].to_string(), field_parts[1].to_string());
    }

    Ok(Arc::new(ObjectConstructTransform::new(fields)?))
}

fn parse_array_map_transform(expr: &str) -> Result<Arc<dyn Transform>> {
    let parts: Vec<&str> = expr.split(',').collect();

    if parts.len() < 2 {
        return Err(MirrorMakerError::Config(format!(
            "Invalid ARRAY_MAP format: {}. Expected 'ARRAY_MAP:/path,element_transform'",
            expr
        )));
    }

    let path = parts[0];
    let element_transform_expr = parts[1..].join(",");

    // For now, only support simple path transforms as element transforms
    let element_transform = Box::new(JsonPathTransform::new(&element_transform_expr)?);

    Ok(Arc::new(ArrayMapTransform::new(path, element_transform)?))
}

fn parse_arithmetic_transform(expr: &str) -> Result<Arc<dyn Transform>> {
    let parts: Vec<&str> = expr.split(',').collect();

    if parts.len() != 3 {
        return Err(MirrorMakerError::Config(format!(
            "Invalid ARITHMETIC format: {}. Expected 'ARITHMETIC:op,operand1,operand2'",
            expr
        )));
    }

    let op = match parts[0] {
        "ADD" => ArithmeticOp::Add,
        "SUB" => ArithmeticOp::Sub,
        "MUL" => ArithmeticOp::Mul,
        "DIV" => ArithmeticOp::Div,
        _ => {
            return Err(MirrorMakerError::Config(format!(
                "Unknown arithmetic operation: {}. Expected ADD, SUB, MUL, or DIV",
                parts[0]
            )))
        }
    };

    let left_path = parts[1];

    // Check if right operand is a constant or path
    if let Ok(constant) = parts[2].parse::<f64>() {
        Ok(Arc::new(ArithmeticTransform::new_with_constant(
            op, left_path, constant,
        )?))
    } else {
        Ok(Arc::new(ArithmeticTransform::new_with_paths(
            op, left_path, parts[2],
        )?))
    }
}

fn parse_hash_transform(expr: &str) -> Result<Arc<dyn Transform>> {
    let parts: Vec<&str> = expr.split(',').collect();

    if parts.len() < 2 {
        return Err(MirrorMakerError::Config(format!(
            "Invalid HASH format: {}. Expected 'HASH:algorithm,/path' or 'HASH:algorithm,/path,output_field'",
            expr
        )));
    }

    let algorithm = HashAlgorithm::parse(parts[0])?;
    let path = parts[1];

    if parts.len() >= 3 {
        // With output field - preserves original value
        let output_field = parts[2];
        Ok(Arc::new(HashTransform::new_with_output(
            path,
            algorithm,
            output_field,
        )?))
    } else {
        // Without output field - replaces with hash
        Ok(Arc::new(HashTransform::new(path, algorithm)?))
    }
}

// ============================================================================
// ENVELOPE FILTER PARSERS
// ============================================================================

fn parse_key_matches_filter(parts: &[&str]) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config(
            "KEY_MATCHES filter requires a pattern".to_string(),
        ));
    }

    let pattern = parts.join(":");
    Ok(Box::new(KeyMatchesFilter::new(&pattern)?))
}

fn parse_key_prefix_filter(parts: &[&str]) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config(
            "KEY_PREFIX filter requires a prefix".to_string(),
        ));
    }

    let prefix = parts.join(":");
    Ok(Box::new(KeyPrefixFilter::new(&prefix)))
}

fn parse_key_suffix_filter(parts: &[&str]) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config(
            "KEY_SUFFIX filter requires a suffix".to_string(),
        ));
    }

    let suffix = parts.join(":");
    Ok(Box::new(KeySuffixFilter::new(&suffix)))
}

fn parse_key_contains_filter(parts: &[&str]) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config(
            "KEY_CONTAINS filter requires a substring".to_string(),
        ));
    }

    let substring = parts.join(":");
    Ok(Box::new(KeyContainsFilter::new(&substring)))
}

fn parse_header_exists_filter(parts: &[&str]) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config(
            "HEADER_EXISTS filter requires a header name".to_string(),
        ));
    }

    let header_name = parts.join(":");
    Ok(Box::new(HeaderExistsFilter::new(&header_name)))
}

fn parse_header_filter(parts: &[&str]) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config(
            "HEADER filter requires name,operator,value".to_string(),
        ));
    }

    let combined = parts.join(":");
    let filter_parts: Vec<&str> = combined.split(',').collect();

    if filter_parts.len() != 3 {
        return Err(MirrorMakerError::Config(format!(
            "Invalid HEADER format: {}. Expected 'HEADER:name,op,value'",
            combined
        )));
    }

    Ok(Box::new(HeaderFilter::new(
        filter_parts[0],
        filter_parts[1],
        filter_parts[2],
    )?))
}

fn parse_timestamp_age_filter(parts: &[&str]) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config(
            "TIMESTAMP_AGE filter requires operator,seconds".to_string(),
        ));
    }

    let combined = parts.join(":");
    let filter_parts: Vec<&str> = combined.split(',').collect();

    if filter_parts.len() != 2 {
        return Err(MirrorMakerError::Config(format!(
            "Invalid TIMESTAMP_AGE format: {}. Expected 'TIMESTAMP_AGE:op,seconds'",
            combined
        )));
    }

    let operator = filter_parts[0];
    let seconds = filter_parts[1].parse::<i64>().map_err(|_| {
        MirrorMakerError::Config(format!("Invalid seconds value: {}", filter_parts[1]))
    })?;

    Ok(Box::new(TimestampAgeFilter::new(operator, seconds)?))
}

fn parse_timestamp_after_filter(parts: &[&str]) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config(
            "TIMESTAMP_AFTER filter requires epoch_ms".to_string(),
        ));
    }

    let epoch_str = parts.join(":");
    let epoch_ms = epoch_str
        .parse::<i64>()
        .map_err(|_| MirrorMakerError::Config(format!("Invalid epoch_ms value: {}", epoch_str)))?;

    Ok(Box::new(TimestampAfterFilter::new(epoch_ms)))
}

fn parse_timestamp_before_filter(parts: &[&str]) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config(
            "TIMESTAMP_BEFORE filter requires epoch_ms".to_string(),
        ));
    }

    let epoch_str = parts.join(":");
    let epoch_ms = epoch_str
        .parse::<i64>()
        .map_err(|_| MirrorMakerError::Config(format!("Invalid epoch_ms value: {}", epoch_str)))?;

    Ok(Box::new(TimestampBeforeFilter::new(epoch_ms)))
}

// ============================================================================
// Envelope Transform Parsers
// ============================================================================

/// Parse key transform expression
///
/// Formats:
/// - "/path" - Extract key from value JSON path
/// - "CONSTRUCT:field1=/path1:field2=/path2" - Construct key from multiple fields
/// - "{template-with-{/placeholders}}" - Template-based key construction
/// - "HASH:algorithm,/path" - Hash a value field
/// - Other strings - Constant key value
pub fn parse_key_transform(expr: &str) -> Result<Arc<dyn EnvelopeTransform>> {
    if let Some(rest) = expr.strip_prefix("CONSTRUCT:") {
        parse_key_construct_transform(rest)
    } else if let Some(rest) = expr.strip_prefix("HASH:") {
        parse_key_hash_transform(rest)
    } else if expr.starts_with('/') {
        // JSON path extraction
        Ok(Arc::new(KeyFromTransform::new(expr)?))
    } else if expr.contains("{/") {
        // Template-based key construction
        Ok(Arc::new(KeyTemplateTransform::new(expr)?))
    } else {
        // Constant key
        Ok(Arc::new(KeyConstantTransform::new(expr)))
    }
}

fn parse_key_construct_transform(expr: &str) -> Result<Arc<dyn EnvelopeTransform>> {
    let parts: Vec<&str> = expr.split(':').collect();
    let mut fields = HashMap::new();

    for part in parts {
        let field_parts: Vec<&str> = part.split('=').collect();
        if field_parts.len() != 2 {
            return Err(MirrorMakerError::Config(format!(
                "Invalid CONSTRUCT format: {}. Expected 'field=path'",
                part
            )));
        }
        fields.insert(field_parts[0].to_string(), field_parts[1].to_string());
    }

    Ok(Arc::new(KeyConstructTransform::new(fields)?))
}

fn parse_key_hash_transform(expr: &str) -> Result<Arc<dyn EnvelopeTransform>> {
    let parts: Vec<&str> = expr.split(',').collect();

    if parts.len() != 2 {
        return Err(MirrorMakerError::Config(format!(
            "Invalid key HASH format: {}. Expected 'HASH:algorithm,/path'",
            expr
        )));
    }

    let algorithm = HashAlgorithm::parse(parts[0])?;
    let path = parts[1];

    Ok(Arc::new(KeyHashTransform::new(path, algorithm)?))
}

/// Parse header transform operation
///
/// Operations:
/// - "FROM:/path" - Extract from value JSON path
/// - "COPY:source_header" - Copy from another header
/// - "REMOVE" - Remove header
pub fn parse_header_transform(
    header_name: &str,
    operation: &str,
) -> Result<Arc<dyn EnvelopeTransform>> {
    if let Some(path) = operation.strip_prefix("FROM:") {
        Ok(Arc::new(HeaderFromTransform::new(header_name, path)?))
    } else if let Some(source_header) = operation.strip_prefix("COPY:") {
        Ok(Arc::new(HeaderCopyTransform::new(
            source_header,
            header_name,
        )))
    } else if operation == "REMOVE" {
        Ok(Arc::new(HeaderRemoveTransform::new(header_name)))
    } else {
        Err(MirrorMakerError::Config(format!(
            "Unknown header operation: {}. Expected FROM:, COPY:, or REMOVE",
            operation
        )))
    }
}

/// Parse static header set operations from HashMap
pub fn parse_static_headers(headers: &HashMap<String, String>) -> Vec<Arc<dyn EnvelopeTransform>> {
    headers
        .iter()
        .map(|(name, value)| {
            Arc::new(HeaderSetTransform::new(name, value)) as Arc<dyn EnvelopeTransform>
        })
        .collect()
}

/// Parse timestamp transform expression
///
/// Formats:
/// - "PRESERVE" - Keep original timestamp
/// - "CURRENT" - Set to current time
/// - "FROM:/path" - Extract from value JSON path
/// - "ADD:seconds" - Add seconds to timestamp
/// - "SUBTRACT:seconds" - Subtract seconds from timestamp
pub fn parse_timestamp_transform(expr: &str) -> Result<Arc<dyn EnvelopeTransform>> {
    match expr {
        "PRESERVE" => Ok(Arc::new(TimestampPreserveTransform)),
        "CURRENT" => Ok(Arc::new(TimestampCurrentTransform)),
        _ if expr.starts_with("FROM:") => {
            let path = &expr[5..];
            Ok(Arc::new(TimestampFromTransform::new(path)?))
        }
        _ if expr.starts_with("ADD:") => {
            let seconds = expr[4..].parse::<i64>()
                .map_err(|_| MirrorMakerError::Config(format!(
                    "Invalid ADD seconds value: {}",
                    &expr[4..]
                )))?;
            Ok(Arc::new(TimestampAddTransform::new(seconds)))
        }
        _ if expr.starts_with("SUBTRACT:") => {
            let seconds = expr[9..].parse::<i64>()
                .map_err(|_| MirrorMakerError::Config(format!(
                    "Invalid SUBTRACT seconds value: {}",
                    &expr[9..]
                )))?;
            Ok(Arc::new(TimestampSubtractTransform::new(seconds)))
        }
        _ => Err(MirrorMakerError::Config(format!(
            "Unknown timestamp operation: {}. Expected PRESERVE, CURRENT, FROM:, ADD:, or SUBTRACT:",
            expr
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_simple_filter() {
        let filter = parse_filter("/message/siteId,>,10000").unwrap();
        let msg = json!({"message": {"siteId": 15000}});
        assert!(filter.evaluate(&msg).unwrap());
    }

    #[test]
    fn test_parse_and_filter() {
        let filter = parse_filter("AND:/message/siteId,>,10000:/message/status,==,active").unwrap();

        let msg1 = json!({"message": {"siteId": 15000, "status": "active"}});
        assert!(filter.evaluate(&msg1).unwrap());

        let msg2 = json!({"message": {"siteId": 15000, "status": "inactive"}});
        assert!(!filter.evaluate(&msg2).unwrap());
    }

    #[test]
    fn test_parse_or_filter() {
        let filter = parse_filter("OR:/message/siteId,>,10000:/message/priority,==,high").unwrap();

        let msg1 = json!({"message": {"siteId": 15000, "priority": "low"}});
        assert!(filter.evaluate(&msg1).unwrap());

        let msg2 = json!({"message": {"siteId": 5000, "priority": "high"}});
        assert!(filter.evaluate(&msg2).unwrap());

        let msg3 = json!({"message": {"siteId": 5000, "priority": "low"}});
        assert!(!filter.evaluate(&msg3).unwrap());
    }

    #[test]
    fn test_parse_not_filter() {
        let filter = parse_filter("NOT:/message/test,==,true").unwrap();

        let msg1 = json!({"message": {"test": false}});
        assert!(filter.evaluate(&msg1).unwrap());

        let msg2 = json!({"message": {"test": true}});
        assert!(!filter.evaluate(&msg2).unwrap());
    }

    #[test]
    fn test_parse_simple_transform() {
        let transform = parse_transform("/message").unwrap();

        let input = json!({
            "message": {"confId": 123, "siteId": 456},
            "metadata": {"ts": 789}
        });

        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!({"confId": 123, "siteId": 456}));
    }

    #[test]
    fn test_parse_construct_transform() {
        let transform =
            parse_transform("CONSTRUCT:id=/message/confId:site=/message/siteId").unwrap();

        let input = json!({
            "message": {"confId": 123, "siteId": 456, "other": "ignored"}
        });

        let result = transform.transform(input).unwrap();
        assert_eq!(result.get("id").unwrap(), &json!(123));
        assert_eq!(result.get("site").unwrap(), &json!(456));
        assert!(result.get("other").is_none());
    }

    #[test]
    fn test_parse_regex_filter() {
        let filter = parse_filter("REGEX:/message/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$").unwrap();

        let msg1 = json!({"message": {"email": "user@example.com"}});
        assert!(filter.evaluate(&msg1).unwrap());

        let msg2 = json!({"message": {"email": "invalid"}});
        assert!(!filter.evaluate(&msg2).unwrap());
    }

    #[test]
    fn test_parse_array_all_filter() {
        let filter = parse_filter("ARRAY_ALL:/users,/status,==,active").unwrap();

        let msg1 = json!({
            "users": [
                {"status": "active"},
                {"status": "active"}
            ]
        });
        assert!(filter.evaluate(&msg1).unwrap());

        let msg2 = json!({
            "users": [
                {"status": "active"},
                {"status": "inactive"}
            ]
        });
        assert!(!filter.evaluate(&msg2).unwrap());
    }

    #[test]
    fn test_parse_array_any_filter() {
        let filter = parse_filter("ARRAY_ANY:/tasks,/priority,==,high").unwrap();

        let msg1 = json!({
            "tasks": [
                {"priority": "low"},
                {"priority": "high"}
            ]
        });
        assert!(filter.evaluate(&msg1).unwrap());

        let msg2 = json!({
            "tasks": [
                {"priority": "low"},
                {"priority": "low"}
            ]
        });
        assert!(!filter.evaluate(&msg2).unwrap());
    }

    #[test]
    fn test_parse_array_map_transform() {
        let transform = parse_transform("ARRAY_MAP:/users,/id").unwrap();

        let input = json!({
            "users": [
                {"id": 1, "name": "Alice"},
                {"id": 2, "name": "Bob"}
            ]
        });

        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!([1, 2]));
    }

    #[test]
    fn test_parse_arithmetic_add_paths() {
        let transform = parse_transform("ARITHMETIC:ADD,/price,/tax").unwrap();

        let input = json!({"price": 100.0, "tax": 15.0});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(115.0));
    }

    #[test]
    fn test_parse_arithmetic_multiply_constant() {
        let transform = parse_transform("ARITHMETIC:MUL,/price,1.2").unwrap();

        let input = json!({"price": 100.0});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(120.0));
    }

    #[test]
    fn test_parse_arithmetic_subtract() {
        let transform = parse_transform("ARITHMETIC:SUB,/total,/discount").unwrap();

        let input = json!({"total": 100.0, "discount": 20.0});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(80.0));
    }

    #[test]
    fn test_parse_arithmetic_divide() {
        let transform = parse_transform("ARITHMETIC:DIV,/value,2.0").unwrap();

        let input = json!({"value": 50.0});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(25.0));
    }

    #[test]
    fn test_parse_hash_transform_md5() {
        let transform = parse_transform("HASH:MD5,/userId").unwrap();

        let input = json!({"userId": "user123"});
        let result = transform.transform(input).unwrap();

        assert!(result.is_string());
        let hash = result.as_str().unwrap();
        assert_eq!(hash.len(), 32); // MD5 is 32 hex chars
    }

    #[test]
    fn test_parse_hash_transform_sha256() {
        let transform = parse_transform("HASH:SHA256,/message/email").unwrap();

        let input = json!({"message": {"email": "test@example.com"}});
        let result = transform.transform(input).unwrap();

        assert!(result.is_string());
        let hash = result.as_str().unwrap();
        assert_eq!(hash.len(), 64); // SHA256 is 64 hex chars
    }

    #[test]
    fn test_parse_hash_transform_with_output_field() {
        let transform = parse_transform("HASH:MD5,/userId,userIdHash").unwrap();

        let input = json!({"userId": "user123", "name": "Test"});
        let result = transform.transform(input).unwrap();

        // Should preserve original fields and add hash
        assert_eq!(result.get("userId").unwrap(), &json!("user123"));
        assert_eq!(result.get("name").unwrap(), &json!("Test"));
        assert!(result.get("userIdHash").unwrap().is_string());
    }

    #[test]
    fn test_parse_hash_transform_murmur128() {
        let transform = parse_transform("HASH:MURMUR128,/key").unwrap();

        let input = json!({"key": "partition-key"});
        let result = transform.transform(input).unwrap();

        assert!(result.is_string());
        let hash = result.as_str().unwrap();
        assert_eq!(hash.len(), 32); // Murmur128 is 32 hex chars
    }

    #[test]
    fn test_parse_hash_transform_consistency() {
        let transform = parse_transform("HASH:SHA256,/value").unwrap();
        let input = json!({"value": "test"});

        let result1 = transform.transform(input.clone()).unwrap();
        let result2 = transform.transform(input).unwrap();

        // Same input should produce same hash
        assert_eq!(result1, result2);
    }

    // ========================================================================
    // ENVELOPE FILTER PARSER TESTS
    // ========================================================================

    #[test]
    fn test_parse_key_matches_filter() {
        use crate::envelope::MessageEnvelope;

        let filter = parse_filter("KEY_MATCHES:^user-\\d+$").unwrap();

        let mut envelope = MessageEnvelope::new(json!({}));
        envelope.key = Some(json!("user-123"));
        assert!(filter.evaluate_envelope(&envelope).unwrap());

        envelope.key = Some(json!("admin-456"));
        assert!(!filter.evaluate_envelope(&envelope).unwrap());
    }

    #[test]
    fn test_parse_key_prefix_filter() {
        use crate::envelope::MessageEnvelope;

        let filter = parse_filter("KEY_PREFIX:premium-").unwrap();

        let mut envelope = MessageEnvelope::new(json!({}));
        envelope.key = Some(json!("premium-user"));
        assert!(filter.evaluate_envelope(&envelope).unwrap());

        envelope.key = Some(json!("basic-user"));
        assert!(!filter.evaluate_envelope(&envelope).unwrap());
    }

    #[test]
    fn test_parse_key_suffix_filter() {
        use crate::envelope::MessageEnvelope;

        let filter = parse_filter("KEY_SUFFIX:-prod").unwrap();

        let mut envelope = MessageEnvelope::new(json!({}));
        envelope.key = Some(json!("service-prod"));
        assert!(filter.evaluate_envelope(&envelope).unwrap());

        envelope.key = Some(json!("service-test"));
        assert!(!filter.evaluate_envelope(&envelope).unwrap());
    }

    #[test]
    fn test_parse_key_contains_filter() {
        use crate::envelope::MessageEnvelope;

        let filter = parse_filter("KEY_CONTAINS:test").unwrap();

        let mut envelope = MessageEnvelope::new(json!({}));
        envelope.key = Some(json!("my-test-key"));
        assert!(filter.evaluate_envelope(&envelope).unwrap());

        envelope.key = Some(json!("my-prod-key"));
        assert!(!filter.evaluate_envelope(&envelope).unwrap());
    }

    #[test]
    fn test_parse_key_exists_filter() {
        use crate::envelope::MessageEnvelope;

        let filter = parse_filter("KEY_EXISTS").unwrap();

        let mut envelope = MessageEnvelope::new(json!({}));
        envelope.key = Some(json!("any-key"));
        assert!(filter.evaluate_envelope(&envelope).unwrap());

        envelope.key = None;
        assert!(!filter.evaluate_envelope(&envelope).unwrap());
    }

    #[test]
    fn test_parse_header_exists_filter() {
        use crate::envelope::MessageEnvelope;

        let filter = parse_filter("HEADER_EXISTS:x-tenant").unwrap();

        let envelope1 =
            MessageEnvelope::new(json!({})).with_header_str("x-tenant".to_string(), "production");
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let envelope2 = MessageEnvelope::new(json!({}));
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());
    }

    #[test]
    fn test_parse_header_filter() {
        use crate::envelope::MessageEnvelope;

        let filter = parse_filter("HEADER:x-tenant,==,production").unwrap();

        let envelope1 =
            MessageEnvelope::new(json!({})).with_header_str("x-tenant".to_string(), "production");
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let envelope2 =
            MessageEnvelope::new(json!({})).with_header_str("x-tenant".to_string(), "test");
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());
    }

    #[test]
    fn test_parse_timestamp_age_filter() {
        use crate::envelope::MessageEnvelope;

        let filter = parse_filter("TIMESTAMP_AGE:>,100").unwrap();

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
    }

    #[test]
    fn test_parse_timestamp_after_filter() {
        use crate::envelope::MessageEnvelope;

        let threshold = 1704067200000i64; // 2024-01-01 00:00:00 UTC
        let filter = parse_filter(&format!("TIMESTAMP_AFTER:{}", threshold)).unwrap();

        let envelope1 = MessageEnvelope::new(json!({})).timestamp(threshold + 1000);
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let envelope2 = MessageEnvelope::new(json!({})).timestamp(threshold - 1000);
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());
    }

    #[test]
    fn test_parse_timestamp_before_filter() {
        use crate::envelope::MessageEnvelope;

        let threshold = 1704067200000i64; // 2024-01-01 00:00:00 UTC
        let filter = parse_filter(&format!("TIMESTAMP_BEFORE:{}", threshold)).unwrap();

        let envelope1 = MessageEnvelope::new(json!({})).timestamp(threshold - 1000);
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        let envelope2 = MessageEnvelope::new(json!({})).timestamp(threshold + 1000);
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());
    }

    #[test]
    fn test_parse_combined_envelope_and_value_filters() {
        use crate::envelope::MessageEnvelope;

        // Test AND with both envelope and value filters
        let filter = parse_filter("AND:KEY_PREFIX:user-:/user/active,==,true").unwrap();

        let mut envelope1 = MessageEnvelope::new(json!({"user": {"active": true}}));
        envelope1.key = Some(json!("user-123"));
        assert!(filter.evaluate_envelope(&envelope1).unwrap());

        // Wrong key
        let mut envelope2 = MessageEnvelope::new(json!({"user": {"active": true}}));
        envelope2.key = Some(json!("admin-456"));
        assert!(!filter.evaluate_envelope(&envelope2).unwrap());

        // Wrong value
        let mut envelope3 = MessageEnvelope::new(json!({"user": {"active": false}}));
        envelope3.key = Some(json!("user-123"));
        assert!(!filter.evaluate_envelope(&envelope3).unwrap());
    }
}
