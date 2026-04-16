use crate::cache::SyncCacheManager;
use crate::error::{MirrorMakerError, Result};
use crate::filter::{
    AndFilter, ArithmeticOp, ArithmeticTransform, ArrayFilter, ArrayFilterMode, ArrayMapTransform,
    CacheLookupTransform, CachePutTransform, ConcatPart, ConcatTransform, EnvelopeTransform,
    Filter, HashTransform, HeaderCopyTransform, HeaderExistsFilter, HeaderFilter,
    HeaderFromTransform, HeaderRemoveTransform, HeaderSetTransform, JsonPathFilter,
    JsonPathTransform, KeyConstantTransform, KeyConstructTransform, KeyContainsFilter,
    KeyExistsFilter, KeyFromTransform, KeyHashTransform, KeyMatchesFilter, KeyPrefixFilter,
    KeySuffixFilter, KeyTemplateTransform, NotFilter, ObjectConstructTransform, OrFilter,
    RegexFilter, StringOp, StringTransform, TimestampAddTransform, TimestampAfterFilter,
    TimestampAgeFilter, TimestampBeforeFilter, TimestampCurrentTransform, TimestampFromTransform,
    TimestampPreserveTransform, TimestampSubtractTransform, Transform,
};
use regex::Regex;
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

/// Parse transform expression from string.
///
/// Formats:
/// - Simple path: "/message" or "/message/confId"
/// - Object construction: "CONSTRUCT:field1=/path1:field2=/path2"
/// - Array map: "ARRAY_MAP:/path,element_transform"
/// - Arithmetic: "ARITHMETIC:op,operand1,operand2"
/// - Hash: "HASH:algorithm,/path[,output_field]"
///
/// For cache transforms use [`parse_transform_with_cache`].
pub fn parse_transform(expr: &str) -> Result<Arc<dyn Transform>> {
    parse_transform_with_cache(expr, None)
}

/// Parse a transform expression, with optional access to named cache stores.
///
/// All formats supported by [`parse_transform`] are accepted, plus:
///
/// - `CACHE_LOOKUP:/keyPath,store-name,outputField`
///   Looks up `message[keyPath]` in `store-name`. On hit, adds the cached
///   value as a new field named `outputField`. On miss, passes through unchanged.
///
/// - `CACHE_LOOKUP:/keyPath,store-name,MERGE`
///   Same as above but merges the cached object into the message instead of
///   adding a named field (both message and cached value must be objects).
///
/// - `CACHE_PUT:/keyPath,store-name`
///   Stores the entire message in `store-name` under the key extracted from
///   `message[keyPath]`. The message passes through unchanged.
///
/// - `CACHE_PUT:/keyPath,store-name,/valuePath`
///   Stores `message[valuePath]` instead of the whole message.
///
/// Named cache stores are created on first use (10 000 entries, 1 h TTL).
/// If `cache_manager` is `None`, `CACHE_LOOKUP` / `CACHE_PUT` return an error.
pub fn parse_transform_with_cache(
    expr: &str,
    cache_manager: Option<Arc<SyncCacheManager>>,
) -> Result<Arc<dyn Transform>> {
    if let Some(rest) = expr.strip_prefix("CONSTRUCT:") {
        parse_construct_transform(rest)
    } else if let Some(rest) = expr.strip_prefix("ARRAY_MAP:") {
        parse_array_map_transform(rest)
    } else if let Some(rest) = expr.strip_prefix("ARITHMETIC:") {
        parse_arithmetic_transform(rest)
    } else if let Some(rest) = expr.strip_prefix("HASH:") {
        parse_hash_transform(rest)
    } else if let Some(rest) = expr.strip_prefix("CACHE_LOOKUP:") {
        parse_cache_lookup_transform(rest, cache_manager)
    } else if let Some(rest) = expr.strip_prefix("CACHE_PUT:") {
        parse_cache_put_transform(rest, cache_manager)
    } else if let Some(rest) = expr.strip_prefix("STRING:") {
        parse_string_transform(rest)
    } else {
        Ok(Arc::new(JsonPathTransform::new(expr)?))
    }
}

// ============================================================================
// STRING TRANSFORM PARSERS
// ============================================================================

/// Parse `STRING:<op>,/path[,arg...]` into a `StringTransform` or `ConcatTransform`.
///
/// All single-field operations accept an optional trailing `,outputField` argument.
/// When present the result is added as a new top-level field; the source is unchanged.
///
/// Operations:
/// - `STRING:UPPER,/path[,outputField]`
/// - `STRING:LOWER,/path[,outputField]`
/// - `STRING:TRIM,/path[,outputField]`
/// - `STRING:TRIM_START,/path[,outputField]`
/// - `STRING:TRIM_END,/path[,outputField]`
/// - `STRING:SUBSTRING,/path,start[,length][,outputField]`
/// - `STRING:REPLACE,/path,from,to[,outputField]`
/// - `STRING:REPLACE_ALL,/path,from,to[,outputField]`
/// - `STRING:REGEX_REPLACE,/path,pattern,replacement[,outputField]`
/// - `STRING:SPLIT,/path,delimiter[,outputField]`
/// - `STRING:LENGTH,/path[,outputField]`
/// - `STRING:CONCAT,outputField,part1,part2,...`
fn parse_string_transform(expr: &str) -> Result<Arc<dyn Transform>> {
    // Split op from the rest on the first comma
    let (op, rest) = match expr.find(',') {
        Some(idx) => (&expr[..idx], &expr[idx + 1..]),
        None => (expr, ""),
    };

    match op {
        "UPPER" => parse_string_simple(rest, StringOp::Upper),
        "LOWER" => parse_string_simple(rest, StringOp::Lower),
        "TRIM" => parse_string_simple(rest, StringOp::Trim),
        "TRIM_START" => parse_string_simple(rest, StringOp::TrimStart),
        "TRIM_END" => parse_string_simple(rest, StringOp::TrimEnd),
        "LENGTH" => parse_string_simple(rest, StringOp::Length),
        "SUBSTRING" => parse_string_substring(rest),
        "REPLACE" => parse_string_replace(rest, false),
        "REPLACE_ALL" => parse_string_replace(rest, true),
        "REGEX_REPLACE" => parse_string_regex_replace(rest),
        "SPLIT" => parse_string_split(rest),
        "CONCAT" => parse_string_concat(rest),
        _ => Err(MirrorMakerError::Config(format!(
            "Unknown STRING operation '{}'. Supported: UPPER, LOWER, TRIM, TRIM_START, \
             TRIM_END, LENGTH, SUBSTRING, REPLACE, REPLACE_ALL, REGEX_REPLACE, SPLIT, CONCAT",
            op
        ))),
    }
}

/// Parse `/path[,outputField]` for single-field ops (UPPER, LOWER, TRIM, LENGTH).
fn parse_string_simple(rest: &str, op: StringOp) -> Result<Arc<dyn Transform>> {
    let parts: Vec<&str> = rest.splitn(2, ',').collect();
    if parts.is_empty() || parts[0].is_empty() {
        return Err(MirrorMakerError::Config(
            "STRING: missing /path argument".to_string(),
        ));
    }
    let path = parts[0];
    let output_field = parts.get(1).copied().filter(|s| !s.is_empty());
    Ok(Arc::new(StringTransform::new(path, op, output_field)?))
}

/// Parse `/path,start[,length][,outputField]` for SUBSTRING.
fn parse_string_substring(rest: &str) -> Result<Arc<dyn Transform>> {
    let parts: Vec<&str> = rest.splitn(4, ',').collect();
    if parts.len() < 2 {
        return Err(MirrorMakerError::Config(format!(
            "STRING:SUBSTRING requires at least /path and start index. Got: '{}'",
            rest
        )));
    }
    let path = parts[0];
    let start = parts[1].parse::<usize>().map_err(|_| {
        MirrorMakerError::Config(format!("STRING:SUBSTRING: invalid start index '{}'", parts[1]))
    })?;

    // parts[2] is either a length (number) or an outputField (non-numeric)
    let (length, output_field) = match parts.get(2) {
        None => (None, None),
        Some(s) => match s.parse::<usize>() {
            Ok(n) => (Some(n), parts.get(3).copied().filter(|f| !f.is_empty())),
            Err(_) => (None, Some(*s)),
        },
    };

    Ok(Arc::new(StringTransform::new(
        path,
        StringOp::Substring { start, length },
        output_field,
    )?))
}

/// Parse `/path,from,to[,outputField]` for REPLACE / REPLACE_ALL.
fn parse_string_replace(rest: &str, all: bool) -> Result<Arc<dyn Transform>> {
    let parts: Vec<&str> = rest.splitn(4, ',').collect();
    if parts.len() < 3 {
        return Err(MirrorMakerError::Config(format!(
            "STRING:REPLACE requires /path,from,to. Got: '{}'",
            rest
        )));
    }
    let path = parts[0];
    let from = parts[1].to_string();
    if from.is_empty() {
        return Err(MirrorMakerError::Config(format!(
            "STRING:REPLACE: 'from' string must not be empty. \
             Replacing an empty string inserts the replacement between every character \
             (e.g. \"abc\".replace(\"\", \"X\") → \"XaXbXcX\"). \
             Expression: '{}'",
            rest
        )));
    }
    let to = parts[2].to_string();
    let output_field = parts.get(3).copied().filter(|s| !s.is_empty());
    Ok(Arc::new(StringTransform::new(
        path,
        StringOp::Replace { from, to, all },
        output_field,
    )?))
}

/// Parse `/path,pattern,replacement[,outputField]` for REGEX_REPLACE.
fn parse_string_regex_replace(rest: &str) -> Result<Arc<dyn Transform>> {
    let parts: Vec<&str> = rest.splitn(4, ',').collect();
    if parts.len() < 3 {
        return Err(MirrorMakerError::Config(format!(
            "STRING:REGEX_REPLACE requires /path,pattern,replacement. Got: '{}'",
            rest
        )));
    }
    let path = parts[0];
    let pattern = Regex::new(parts[1]).map_err(|e| {
        MirrorMakerError::Config(format!("STRING:REGEX_REPLACE: invalid pattern '{}': {}", parts[1], e))
    })?;
    let replacement = parts[2].to_string();
    let output_field = parts.get(3).copied().filter(|s| !s.is_empty());
    Ok(Arc::new(StringTransform::new(
        path,
        StringOp::RegexReplace { pattern, replacement },
        output_field,
    )?))
}

/// Parse `/path,delimiter[,outputField]` for SPLIT.
fn parse_string_split(rest: &str) -> Result<Arc<dyn Transform>> {
    let parts: Vec<&str> = rest.splitn(3, ',').collect();
    if parts.len() < 2 {
        return Err(MirrorMakerError::Config(format!(
            "STRING:SPLIT requires /path,delimiter. Got: '{}'",
            rest
        )));
    }
    let path = parts[0];
    let delimiter = parts[1].to_string();
    let output_field = parts.get(2).copied().filter(|s| !s.is_empty());
    Ok(Arc::new(StringTransform::new(
        path,
        StringOp::Split { delimiter },
        output_field,
    )?))
}

/// Parse `outputField,part1,part2,...` for CONCAT.
/// Parts starting with `/` are JSON path extractions; all others are literals.
fn parse_string_concat(rest: &str) -> Result<Arc<dyn Transform>> {
    let parts: Vec<&str> = rest.split(',').collect();
    if parts.len() < 2 {
        return Err(MirrorMakerError::Config(format!(
            "STRING:CONCAT requires outputField and at least one part. Got: '{}'",
            rest
        )));
    }
    let output_field = parts[0];
    let concat_parts: Vec<ConcatPart> = parts[1..]
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            if p.is_empty() {
                Err(MirrorMakerError::Config(format!(
                    "STRING:CONCAT: part {} is empty (check for trailing or double comma in: '{}'). \
                     An empty literal produces invisible whitespace in the output.",
                    i + 1,
                    rest
                )))
            } else if p.starts_with('/') {
                Ok(ConcatPart::Path(p.to_string()))
            } else {
                Ok(ConcatPart::Literal(p.to_string()))
            }
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Arc::new(ConcatTransform::new(output_field, concat_parts)))
}

fn parse_cache_lookup_transform(
    expr: &str,
    cache_manager: Option<Arc<SyncCacheManager>>,
) -> Result<Arc<dyn Transform>> {
    let mgr = cache_manager.ok_or_else(|| {
        MirrorMakerError::Config(
            "CACHE_LOOKUP requires a cache manager — ensure cache is configured".to_string(),
        )
    })?;

    // Format: /keyPath,store-name,outputField  OR  /keyPath,store-name,MERGE
    let parts: Vec<&str> = expr.splitn(3, ',').collect();
    if parts.len() != 3 {
        return Err(MirrorMakerError::Config(format!(
            "Invalid CACHE_LOOKUP format: '{}'. \
             Expected 'CACHE_LOOKUP:/keyPath,store-name,outputField' \
             or 'CACHE_LOOKUP:/keyPath,store-name,MERGE'",
            expr
        )));
    }

    let key_path = parts[0];
    let store_name = parts[1];
    let output = parts[2];

    let cache = mgr.get_or_create(store_name);

    if output == "MERGE" {
        Ok(Arc::new(CacheLookupTransform::new_merge(cache, key_path)?))
    } else {
        Ok(Arc::new(CacheLookupTransform::new(cache, key_path, output)?))
    }
}

fn parse_cache_put_transform(
    expr: &str,
    cache_manager: Option<Arc<SyncCacheManager>>,
) -> Result<Arc<dyn Transform>> {
    let mgr = cache_manager.ok_or_else(|| {
        MirrorMakerError::Config(
            "CACHE_PUT requires a cache manager — ensure cache is configured".to_string(),
        )
    })?;

    // Format: /keyPath,store-name  OR  /keyPath,store-name,/valuePath
    let parts: Vec<&str> = expr.splitn(3, ',').collect();
    if parts.len() < 2 {
        return Err(MirrorMakerError::Config(format!(
            "Invalid CACHE_PUT format: '{}'. \
             Expected 'CACHE_PUT:/keyPath,store-name' \
             or 'CACHE_PUT:/keyPath,store-name,/valuePath'",
            expr
        )));
    }

    let key_path = parts[0];
    let store_name = parts[1];
    let value_path = parts.get(2).copied();

    let cache = mgr.get_or_create(store_name);

    Ok(Arc::new(CachePutTransform::new(cache, key_path, value_path)?))
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
    // STRING TRANSFORM TESTS
    // ========================================================================

    #[test]
    fn test_string_upper() {
        let t = parse_transform("STRING:UPPER,/email").unwrap();
        let result = t.transform(json!({"email": "user@EXAMPLE.com"})).unwrap();
        assert_eq!(result["email"], json!("USER@EXAMPLE.COM"));
    }

    #[test]
    fn test_string_lower() {
        let t = parse_transform("STRING:LOWER,/status").unwrap();
        let result = t.transform(json!({"status": "ACTIVE"})).unwrap();
        assert_eq!(result["status"], json!("active"));
    }

    #[test]
    fn test_string_trim() {
        let t = parse_transform("STRING:TRIM,/name").unwrap();
        let result = t.transform(json!({"name": "  Alice  "})).unwrap();
        assert_eq!(result["name"], json!("Alice"));
    }

    #[test]
    fn test_string_trim_start() {
        let t = parse_transform("STRING:TRIM_START,/name").unwrap();
        let result = t.transform(json!({"name": "  Alice  "})).unwrap();
        assert_eq!(result["name"], json!("Alice  "));
    }

    #[test]
    fn test_string_trim_end() {
        let t = parse_transform("STRING:TRIM_END,/name").unwrap();
        let result = t.transform(json!({"name": "  Alice  "})).unwrap();
        assert_eq!(result["name"], json!("  Alice"));
    }

    #[test]
    fn test_string_substring_with_length() {
        let t = parse_transform("STRING:SUBSTRING,/description,0,10").unwrap();
        let result = t.transform(json!({"description": "Hello, World!"})).unwrap();
        assert_eq!(result["description"], json!("Hello, Wor"));
    }

    #[test]
    fn test_string_substring_without_length() {
        let t = parse_transform("STRING:SUBSTRING,/text,7").unwrap();
        let result = t.transform(json!({"text": "Hello, World!"})).unwrap();
        assert_eq!(result["text"], json!("World!"));
    }

    #[test]
    fn test_string_substring_beyond_end_clamps() {
        let t = parse_transform("STRING:SUBSTRING,/text,0,100").unwrap();
        let result = t.transform(json!({"text": "short"})).unwrap();
        assert_eq!(result["text"], json!("short"));
    }

    #[test]
    fn test_string_replace_first() {
        let t = parse_transform("STRING:REPLACE,/msg,foo,bar").unwrap();
        let result = t.transform(json!({"msg": "foo and foo"})).unwrap();
        assert_eq!(result["msg"], json!("bar and foo"));
    }

    #[test]
    fn test_string_replace_all() {
        let t = parse_transform("STRING:REPLACE_ALL,/msg,foo,bar").unwrap();
        let result = t.transform(json!({"msg": "foo and foo"})).unwrap();
        assert_eq!(result["msg"], json!("bar and bar"));
    }

    #[test]
    fn test_string_regex_replace() {
        let t = parse_transform(r"STRING:REGEX_REPLACE,/email,@.*,@example.com").unwrap();
        let result = t.transform(json!({"email": "user@oldomain.io"})).unwrap();
        assert_eq!(result["email"], json!("user@example.com"));
    }

    #[test]
    fn test_string_split() {
        let t = parse_transform("STRING:SPLIT,/tags,|").unwrap();
        let result = t.transform(json!({"tags": "rust|kafka|streaming"})).unwrap();
        assert_eq!(result["tags"], json!(["rust", "kafka", "streaming"]));
    }

    #[test]
    fn test_string_length() {
        let t = parse_transform("STRING:LENGTH,/description").unwrap();
        let result = t.transform(json!({"description": "hello"})).unwrap();
        assert_eq!(result["description"], json!(5));
    }

    #[test]
    fn test_string_output_field_preserves_original() {
        let t = parse_transform("STRING:UPPER,/email,emailUpper").unwrap();
        let result = t.transform(json!({"email": "user@example.com"})).unwrap();
        assert_eq!(result["email"], json!("user@example.com"), "original must be kept");
        assert_eq!(result["emailUpper"], json!("USER@EXAMPLE.COM"));
    }

    #[test]
    fn test_string_concat_paths_and_literals() {
        let t = parse_transform("STRING:CONCAT,fullName,/firstName, ,/lastName").unwrap();
        let result = t.transform(json!({"firstName": "Jane", "lastName": "Doe"})).unwrap();
        assert_eq!(result["fullName"], json!("Jane Doe"));
    }

    #[test]
    fn test_string_concat_literal_only() {
        let t = parse_transform("STRING:CONCAT,greeting,Hello , World").unwrap();
        let result = t.transform(json!({"x": 1})).unwrap();
        assert_eq!(result["greeting"], json!("Hello  World"));
    }

    #[test]
    fn test_string_nested_path() {
        let t = parse_transform("STRING:UPPER,/user/email").unwrap();
        let result = t.transform(json!({"user": {"email": "me@example.com"}})).unwrap();
        assert_eq!(result["user"]["email"], json!("ME@EXAMPLE.COM"));
    }

    #[test]
    fn test_string_length_with_output_field() {
        let t = parse_transform("STRING:LENGTH,/bio,bioLength").unwrap();
        let result = t.transform(json!({"bio": "I write Rust"})).unwrap();
        assert_eq!(result["bio"], json!("I write Rust"), "original kept");
        assert_eq!(result["bioLength"], json!(12));
    }

    #[test]
    fn test_string_unknown_op_returns_error() {
        let result = parse_transform("STRING:CAPITALIZE,/name");
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("Unknown STRING operation"));
    }

    // ========================================================================
    // ========================================================================
    // NULL / MISSING FIELD — PASS-THROUGH TESTS
    // ========================================================================
    // All transforms must pass through the message unchanged when a referenced
    // field is absent, null, or the wrong type for the operation.

    #[test]
    fn test_json_path_transform_missing_field_passes_through() {
        let t = parse_transform("/nonexistent").unwrap();
        let msg = json!({"other": "value"});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "missing path must pass through");
    }

    #[test]
    fn test_arithmetic_missing_left_operand_passes_through() {
        let t = parse_transform("ARITHMETIC:MUL,/missing,2.0").unwrap();
        let msg = json!({"price": 10.0});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "missing left operand must pass through");
    }

    #[test]
    fn test_arithmetic_missing_right_operand_passes_through() {
        let t = parse_transform("ARITHMETIC:ADD,/price,/missing").unwrap();
        let msg = json!({"price": 10.0});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "missing right operand must pass through");
    }

    #[test]
    fn test_arithmetic_division_by_zero_passes_through() {
        let t = parse_transform("ARITHMETIC:DIV,/price,/zero").unwrap();
        let msg = json!({"price": 10.0, "zero": 0.0});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "division by zero must pass through");
    }

    #[test]
    fn test_hash_missing_field_passes_through() {
        let t = parse_transform("HASH:SHA256,/nonexistent").unwrap();
        let msg = json!({"other": "value"});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "missing hash path must pass through");
    }

    #[test]
    fn test_array_map_missing_path_passes_through() {
        let t = parse_transform("ARRAY_MAP:/missing,/id").unwrap();
        let msg = json!({"other": "value"});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "missing array path must pass through");
    }

    #[test]
    fn test_array_map_non_array_passes_through() {
        let t = parse_transform("ARRAY_MAP:/name,/id").unwrap();
        let msg = json!({"name": "Alice"});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "non-array at path must pass through");
    }

    #[test]
    fn test_string_upper_missing_field_passes_through() {
        let t = parse_transform("STRING:UPPER,/missing").unwrap();
        let msg = json!({"other": "value"});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "missing STRING path must pass through");
    }

    #[test]
    fn test_string_upper_null_field_passes_through() {
        let t = parse_transform("STRING:UPPER,/name").unwrap();
        let msg = json!({"name": null});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "null STRING field must pass through");
    }

    #[test]
    fn test_string_upper_object_field_passes_through() {
        let t = parse_transform("STRING:UPPER,/nested").unwrap();
        let msg = json!({"nested": {"key": "value"}});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "object at STRING path must pass through");
    }

    #[test]
    fn test_string_concat_missing_path_uses_empty_string() {
        // Missing path part → treated as empty string, concat continues
        let t = parse_transform("STRING:CONCAT,full,/first, ,/missing").unwrap();
        let msg = json!({"first": "Jane"});
        let result = t.transform(msg).unwrap();
        assert_eq!(result["full"], json!("Jane "), "missing concat path produces empty contribution");
    }

    #[test]
    fn test_cache_lookup_missing_key_path_passes_through() {
        use crate::cache::SyncCacheManager;
        let mgr = Arc::new(SyncCacheManager::new());
        let t = parse_transform_with_cache("CACHE_LOOKUP:/userId,store,profile", Some(mgr)).unwrap();
        let msg = json!({"event": "login"});  // no /userId
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "missing key path must pass through");
    }

    #[test]
    fn test_cache_lookup_merge_non_object_cached_passes_through() {
        use crate::cache::SyncCacheManager;
        let mgr = Arc::new(SyncCacheManager::new());
        mgr.get_or_create("store").put("k1".to_string(), json!("a-scalar"));
        let t = parse_transform_with_cache("CACHE_LOOKUP:/id,store,MERGE", Some(mgr)).unwrap();
        let msg = json!({"id": "k1", "event": "click"});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "MERGE with non-object cached value must pass through");
    }

    #[test]
    fn test_cache_put_missing_key_path_passes_through() {
        use crate::cache::SyncCacheManager;
        let mgr = Arc::new(SyncCacheManager::new());
        let t = parse_transform_with_cache("CACHE_PUT:/id,store", Some(mgr.clone())).unwrap();
        let msg = json!({"name": "Alice"});  // no /id
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "missing key path must pass through without caching");
        // Store is created by parse but must remain empty — key was never stored
        let store = mgr.get("store").unwrap();
        assert!(store.get("unknown-key").is_none(), "no entry should exist when key path was missing");
    }

    #[test]
    fn test_cache_put_missing_value_path_passes_through() {
        use crate::cache::SyncCacheManager;
        let mgr = Arc::new(SyncCacheManager::new());
        let t = parse_transform_with_cache("CACHE_PUT:/id,store,/missing", Some(mgr.clone())).unwrap();
        let msg = json!({"id": "u1", "other": "data"});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "missing value path must pass through without caching");
        // The store was auto-created by get_or_create in parse, but nothing stored
        let store = mgr.get("store").unwrap();
        assert!(store.get("u1").is_none(), "nothing should be stored when value path is missing");
    }

    // HIGH-ISSUE FIX TESTS
    // ========================================================================

    // PARSER-3: REPLACE with empty `from` must error at parse time
    #[test]
    fn test_string_replace_empty_from_is_error() {
        let result = parse_transform("STRING:REPLACE,/msg,,replacement");
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("'from' string must not be empty"), "unexpected: {}", msg);
    }

    #[test]
    fn test_string_replace_all_empty_from_is_error() {
        let result = parse_transform("STRING:REPLACE_ALL,/msg,,replacement");
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("'from' string must not be empty"), "unexpected: {}", msg);
    }

    // PARSER-2: CONCAT with empty parts must error at parse time
    #[test]
    fn test_string_concat_trailing_comma_is_error() {
        // Trailing comma produces an empty part
        let result = parse_transform("STRING:CONCAT,out,/first,");
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("empty"), "unexpected: {}", msg);
    }

    #[test]
    fn test_string_concat_double_comma_is_error() {
        // Double comma produces an empty part between two valid parts
        let result = parse_transform("STRING:CONCAT,out,/first,,/last");
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("empty"), "unexpected: {}", msg);
    }

    #[test]
    fn test_string_concat_valid_does_not_error() {
        // Verify that the new validation does not break valid CONCAT
        let t = parse_transform("STRING:CONCAT,fullName,/first, ,/last").unwrap();
        let result = t.transform(json!({"first": "Jane", "last": "Doe"})).unwrap();
        assert_eq!(result["fullName"], json!("Jane Doe"));
    }

    // ========================================================================
    // CACHE TRANSFORM TESTS
    // ========================================================================

    #[test]
    fn test_cache_put_stores_message() {
        use crate::cache::SyncCacheManager;
        let mgr = Arc::new(SyncCacheManager::new());

        // CACHE_PUT stores whole message, passes it through
        let t = parse_transform_with_cache("CACHE_PUT:/id,users", Some(mgr.clone())).unwrap();

        let msg = json!({"id": "u1", "name": "Alice"});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "message must pass through unchanged");

        // Verify the value was actually stored
        let store = mgr.get("users").unwrap();
        assert_eq!(store.get("u1"), Some(msg));
    }

    #[test]
    fn test_cache_put_stores_field() {
        use crate::cache::SyncCacheManager;
        let mgr = Arc::new(SyncCacheManager::new());

        let t = parse_transform_with_cache(
            "CACHE_PUT:/id,users,/profile",
            Some(mgr.clone()),
        )
        .unwrap();

        let msg = json!({"id": "u2", "profile": {"tier": "premium"}, "noise": true});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "message must pass through unchanged");

        let store = mgr.get("users").unwrap();
        assert_eq!(store.get("u2"), Some(json!({"tier": "premium"})));
    }

    #[test]
    fn test_cache_lookup_adds_field() {
        use crate::cache::SyncCacheManager;
        let mgr = Arc::new(SyncCacheManager::new());

        // Pre-populate the cache
        let store = mgr.get_or_create("users");
        store.put("u1".to_string(), json!({"tier": "gold", "country": "US"}));

        let t = parse_transform_with_cache(
            "CACHE_LOOKUP:/userId,users,userProfile",
            Some(mgr.clone()),
        )
        .unwrap();

        let msg = json!({"userId": "u1", "event": "login"});
        let result = t.transform(msg).unwrap();

        assert_eq!(result["event"], json!("login"));
        assert_eq!(result["userProfile"], json!({"tier": "gold", "country": "US"}));
    }

    #[test]
    fn test_cache_lookup_merge() {
        use crate::cache::SyncCacheManager;
        let mgr = Arc::new(SyncCacheManager::new());

        let store = mgr.get_or_create("users");
        store.put("u2".to_string(), json!({"tier": "silver"}));

        let t = parse_transform_with_cache(
            "CACHE_LOOKUP:/userId,users,MERGE",
            Some(mgr.clone()),
        )
        .unwrap();

        let msg = json!({"userId": "u2", "event": "purchase"});
        let result = t.transform(msg).unwrap();

        assert_eq!(result["event"], json!("purchase"));
        assert_eq!(result["tier"], json!("silver"));
        assert_eq!(result["userId"], json!("u2"));
    }

    #[test]
    fn test_cache_lookup_miss_passthrough() {
        use crate::cache::SyncCacheManager;
        let mgr = Arc::new(SyncCacheManager::new());

        let t = parse_transform_with_cache(
            "CACHE_LOOKUP:/userId,users,profile",
            Some(mgr),
        )
        .unwrap();

        let msg = json!({"userId": "unknown", "event": "login"});
        let result = t.transform(msg.clone()).unwrap();
        assert_eq!(result, msg, "on cache miss message must be returned unchanged");
    }

    #[test]
    fn test_cache_put_then_lookup_pipeline() {
        use crate::cache::SyncCacheManager;
        let mgr = Arc::new(SyncCacheManager::new());

        // Step 1: first message populates the cache
        let put = parse_transform_with_cache(
            "CACHE_PUT:/userId,profiles,/userData",
            Some(mgr.clone()),
        )
        .unwrap();

        let first = json!({"userId": "u3", "userData": {"plan": "pro", "active": true}});
        put.transform(first).unwrap();

        // Step 2: second message enriches from the cache
        let lookup = parse_transform_with_cache(
            "CACHE_LOOKUP:/userId,profiles,userData",
            Some(mgr.clone()),
        )
        .unwrap();

        let second = json!({"userId": "u3", "event": "checkout"});
        let enriched = lookup.transform(second).unwrap();

        assert_eq!(enriched["event"], json!("checkout"));
        assert_eq!(enriched["userData"], json!({"plan": "pro", "active": true}));
    }

    #[test]
    fn test_cache_without_manager_returns_error() {
        let result = parse_transform_with_cache("CACHE_LOOKUP:/id,store,field", None);
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("cache manager"), "unexpected error: {}", err_msg);

        let result2 = parse_transform_with_cache("CACHE_PUT:/id,store", None);
        assert!(result2.is_err());
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
