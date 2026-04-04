mod envelope_filter;
mod envelope_transform;

pub use envelope_filter::{
    HeaderExistsFilter, HeaderFilter, KeyExistsFilter, KeyPrefixFilter, KeySuffixFilter,
    KeyContainsFilter, KeyMatchesFilter, TimestampAgeFilter, TimestampAfterFilter,
    TimestampBeforeFilter,
};

pub use envelope_transform::{
    EnvelopeTransform, HeaderCopyTransform, HeaderFromTransform, HeaderRemoveTransform,
    HeaderSetTransform, KeyConstantTransform, KeyConstructTransform, KeyFromTransform,
    KeyHashTransform, KeyTemplateTransform, TimestampAddTransform, TimestampCurrentTransform,
    TimestampFromTransform, TimestampPreserveTransform, TimestampSubtractTransform,
};

use crate::cache::LookupCache;
use crate::envelope::MessageEnvelope;
use crate::error::{MirrorMakerError, Result};
use crate::hash::{hash_value, HashAlgorithm};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::sync::Arc;

/// Filter trait for evaluating whether a message should be processed
pub trait Filter: Send + Sync {
    /// Evaluate filter on message value (legacy, backward compatible)
    fn evaluate(&self, value: &Value) -> Result<bool> {
        // Default implementation: create envelope with just value
        let envelope = MessageEnvelope::new(value.clone());
        self.evaluate_envelope(&envelope)
    }

    /// Evaluate filter on complete envelope (new method)
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        // Default implementation: evaluate on value only (for backward compatibility)
        self.evaluate(&envelope.value)
    }
}

/// Transform trait for modifying messages
pub trait Transform: Send + Sync {
    fn transform(&self, value: Value) -> Result<Value>;
}

/// JSON Path-based filter with comparison
///
/// Supports simple JSON path navigation and comparison operations.
///
/// Path format: "/field/nested/value"
/// Operators: >, >=, <, <=, ==, !=
///
/// Examples:
/// - path="/message/siteId", op=">", value="10000"
/// - path="/message/status", op="==", value="active"
pub struct JsonPathFilter {
    path: String,
    operator: ComparisonOp,
    expected: ComparisonValue,
}

#[derive(Debug, Clone)]
enum ComparisonOp {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Ne,
}

#[derive(Debug, Clone)]
enum ComparisonValue {
    Number(f64),
    String(String),
    Bool(bool),
}

impl JsonPathFilter {
    /// Create a new JSON path filter
    ///
    /// # Examples
    ///
    /// ```
    /// # use streamforge::filter::JsonPathFilter;
    /// // Numeric comparison
    /// let filter = JsonPathFilter::new("/message/siteId", ">", "10000").unwrap();
    ///
    /// // String comparison
    /// let filter = JsonPathFilter::new("/message/status", "==", "active").unwrap();
    ///
    /// // Boolean comparison
    /// let filter = JsonPathFilter::new("/message/enabled", "==", "true").unwrap();
    /// ```
    pub fn new(path: &str, operator: &str, value: &str) -> Result<Self> {
        let op = match operator {
            ">" => ComparisonOp::Gt,
            ">=" => ComparisonOp::Gte,
            "<" => ComparisonOp::Lt,
            "<=" => ComparisonOp::Lte,
            "==" => ComparisonOp::Eq,
            "!=" => ComparisonOp::Ne,
            _ => return Err(MirrorMakerError::Config(format!("Unknown operator: {}", operator))),
        };

        // Parse value - try number first, then boolean, then string
        let expected = if let Ok(num) = value.parse::<f64>() {
            ComparisonValue::Number(num)
        } else if let Ok(b) = value.parse::<bool>() {
            ComparisonValue::Bool(b)
        } else {
            ComparisonValue::String(value.to_string())
        };

        Ok(Self {
            path: path.to_string(),
            operator: op,
            expected,
        })
    }

    /// Extract value from JSON using path
    fn extract_value<'a>(&self, value: &'a Value) -> Option<&'a Value> {
        let parts: Vec<&str> = self.path.trim_matches('/').split('/').collect();

        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        Some(current)
    }

    /// Compare values based on operator
    fn compare(&self, actual: &Value) -> bool {
        match (&self.expected, &self.operator) {
            (ComparisonValue::Number(expected), ComparisonOp::Gt) => {
                actual.as_f64().map_or(false, |v| v > *expected)
            }
            (ComparisonValue::Number(expected), ComparisonOp::Gte) => {
                actual.as_f64().map_or(false, |v| v >= *expected)
            }
            (ComparisonValue::Number(expected), ComparisonOp::Lt) => {
                actual.as_f64().map_or(false, |v| v < *expected)
            }
            (ComparisonValue::Number(expected), ComparisonOp::Lte) => {
                actual.as_f64().map_or(false, |v| v <= *expected)
            }
            (ComparisonValue::Number(expected), ComparisonOp::Eq) => {
                actual.as_f64().map_or(false, |v| (v - *expected).abs() < f64::EPSILON)
            }
            (ComparisonValue::Number(expected), ComparisonOp::Ne) => {
                actual.as_f64().map_or(true, |v| (v - *expected).abs() >= f64::EPSILON)
            }
            (ComparisonValue::String(expected), ComparisonOp::Eq) => {
                actual.as_str().map_or(false, |v| v == expected)
            }
            (ComparisonValue::String(expected), ComparisonOp::Ne) => {
                actual.as_str().map_or(true, |v| v != expected)
            }
            (ComparisonValue::Bool(expected), ComparisonOp::Eq) => {
                actual.as_bool().map_or(false, |v| v == *expected)
            }
            (ComparisonValue::Bool(expected), ComparisonOp::Ne) => {
                actual.as_bool().map_or(true, |v| v != *expected)
            }
            _ => false,
        }
    }
}

impl Filter for JsonPathFilter {
    fn evaluate(&self, value: &Value) -> Result<bool> {
        if let Some(extracted) = self.extract_value(value) {
            Ok(self.compare(extracted))
        } else {
            Ok(false) // Field not found = filter fails
        }
    }
}

/// Composite filter: AND
///
/// All sub-filters must pass for the message to pass.
///
/// Example:
/// ```
/// # use streamforge::filter::{JsonPathFilter, AndFilter};
/// let filter1 = Box::new(JsonPathFilter::new("/message/siteId", ">", "10000").unwrap());
/// let filter2 = Box::new(JsonPathFilter::new("/message/status", "==", "active").unwrap());
/// let and_filter = AndFilter::new(vec![filter1, filter2]);
/// ```
pub struct AndFilter {
    filters: Vec<Box<dyn Filter>>,
}

impl AndFilter {
    pub fn new(filters: Vec<Box<dyn Filter>>) -> Self {
        Self { filters }
    }
}

impl Filter for AndFilter {
    fn evaluate(&self, value: &Value) -> Result<bool> {
        for filter in &self.filters {
            if !filter.evaluate(value)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        for filter in &self.filters {
            if !filter.evaluate_envelope(envelope)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Composite filter: OR
///
/// At least one sub-filter must pass for the message to pass.
///
/// Example:
/// ```
/// # use streamforge::filter::{JsonPathFilter, OrFilter};
/// let filter1 = Box::new(JsonPathFilter::new("/message/siteId", ">", "10000").unwrap());
/// let filter2 = Box::new(JsonPathFilter::new("/message/priority", "==", "high").unwrap());
/// let or_filter = OrFilter::new(vec![filter1, filter2]);
/// ```
pub struct OrFilter {
    filters: Vec<Box<dyn Filter>>,
}

impl OrFilter {
    pub fn new(filters: Vec<Box<dyn Filter>>) -> Self {
        Self { filters }
    }
}

impl Filter for OrFilter {
    fn evaluate(&self, value: &Value) -> Result<bool> {
        for filter in &self.filters {
            if filter.evaluate(value)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        for filter in &self.filters {
            if filter.evaluate_envelope(envelope)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Composite filter: NOT
///
/// Inverts the result of a sub-filter.
///
/// Example:
/// ```
/// # use streamforge::filter::{JsonPathFilter, NotFilter};
/// let filter = Box::new(JsonPathFilter::new("/message/test", "==", "true").unwrap());
/// let not_filter = NotFilter::new(filter);
/// ```
pub struct NotFilter {
    filter: Box<dyn Filter>,
}

impl NotFilter {
    pub fn new(filter: Box<dyn Filter>) -> Self {
        Self { filter }
    }
}

impl Filter for NotFilter {
    fn evaluate(&self, value: &Value) -> Result<bool> {
        Ok(!self.filter.evaluate(value)?)
    }

    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        Ok(!self.filter.evaluate_envelope(envelope)?)
    }
}

/// JSON Path-based transform (field extraction)
///
/// Extracts a nested field or object from JSON.
///
/// Path format: "/field/nested/value"
///
/// Examples:
/// - "/message" - Extract entire message object
/// - "/message/confId" - Extract specific field
pub struct JsonPathTransform {
    path: String,
}

impl JsonPathTransform {
    /// Create a new JSON path transform
    ///
    /// # Examples
    ///
    /// ```
    /// # use streamforge::filter::JsonPathTransform;
    /// // Extract nested object
    /// let transform = JsonPathTransform::new("/message").unwrap();
    ///
    /// // Extract specific field
    /// let transform = JsonPathTransform::new("/message/confId").unwrap();
    /// ```
    pub fn new(path: &str) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
        })
    }

    /// Extract value from JSON using path
    fn extract_value(&self, value: &Value) -> Option<Value> {
        let parts: Vec<&str> = self.path.trim_matches('/').split('/').collect();

        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        Some(current.clone())
    }
}

impl Transform for JsonPathTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        self.extract_value(&value)
            .ok_or_else(|| MirrorMakerError::Processing(format!("Path not found: {}", self.path)))
    }
}

/// Object construction transform
///
/// Creates a new JSON object by extracting multiple fields.
///
/// Example:
/// ```
/// # use streamforge::filter::ObjectConstructTransform;
/// # use std::collections::HashMap;
/// let mut fields = HashMap::new();
/// fields.insert("id".to_string(), "/message/confId".to_string());
/// fields.insert("site".to_string(), "/message/siteId".to_string());
/// fields.insert("timestamp".to_string(), "/message/ts".to_string());
///
/// let transform = ObjectConstructTransform::new(fields).unwrap();
/// ```
pub struct ObjectConstructTransform {
    fields: Vec<(String, String)>, // (output_field_name, input_json_path)
}

impl ObjectConstructTransform {
    /// Create a new object construction transform
    ///
    /// Fields map output field names to input JSON paths.
    pub fn new(fields: std::collections::HashMap<String, String>) -> Result<Self> {
        let fields_vec: Vec<(String, String)> = fields.into_iter().collect();
        Ok(Self { fields: fields_vec })
    }

    /// Extract value from JSON using path
    fn extract_value<'a>(&self, value: &'a Value, path: &str) -> Option<&'a Value> {
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();

        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        Some(current)
    }
}

impl Transform for ObjectConstructTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let mut result = Map::new();

        for (output_name, input_path) in &self.fields {
            if let Some(extracted) = self.extract_value(&value, input_path) {
                result.insert(output_name.clone(), extracted.clone());
            }
            // If field doesn't exist, just skip it (don't include in output)
        }

        Ok(Value::Object(result))
    }
}

/// Always-pass filter (no filtering)
pub struct PassThroughFilter;

impl Filter for PassThroughFilter {
    fn evaluate(&self, _value: &Value) -> Result<bool> {
        Ok(true)
    }
}

/// Identity transform (no transformation)
pub struct IdentityTransform;

impl Transform for IdentityTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        Ok(value)
    }
}

/// Regular expression filter
///
/// Matches string fields against a regex pattern.
///
/// Example:
/// ```
/// # use streamforge::filter::RegexFilter;
/// // Match email addresses
/// let filter = RegexFilter::new("/message/email", r"^[\w\.-]+@[\w\.-]+\.\w+$").unwrap();
///
/// // Match status starting with "active"
/// let filter = RegexFilter::new("/message/status", r"^active").unwrap();
/// ```
pub struct RegexFilter {
    path: String,
    regex: Regex,
}

impl RegexFilter {
    /// Create a new regex filter
    ///
    /// # Arguments
    /// * `path` - JSON path to the string field
    /// * `pattern` - Regular expression pattern
    pub fn new(path: &str, pattern: &str) -> Result<Self> {
        let regex = Regex::new(pattern)
            .map_err(|e| MirrorMakerError::Config(format!("Invalid regex pattern: {}", e)))?;

        Ok(Self {
            path: path.to_string(),
            regex,
        })
    }

    /// Extract value from JSON using path
    fn extract_value<'a>(&self, value: &'a Value) -> Option<&'a Value> {
        let parts: Vec<&str> = self.path.trim_matches('/').split('/').collect();

        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        Some(current)
    }
}

impl Filter for RegexFilter {
    fn evaluate(&self, value: &Value) -> Result<bool> {
        if let Some(extracted) = self.extract_value(value) {
            if let Some(s) = extracted.as_str() {
                Ok(self.regex.is_match(s))
            } else {
                Ok(false) // Not a string
            }
        } else {
            Ok(false) // Field not found
        }
    }
}

/// Array filter
///
/// Filters based on array element conditions.
///
/// Modes:
/// - ALL: All elements must match the filter
/// - ANY: At least one element must match the filter
///
/// Example:
/// ```
/// # use streamforge::filter::{ArrayFilter, JsonPathFilter, ArrayFilterMode};
/// // Check if all array elements have status "active"
/// let element_filter = Box::new(JsonPathFilter::new("/status", "==", "active").unwrap());
/// let filter = ArrayFilter::new("/users", element_filter, ArrayFilterMode::All).unwrap();
/// ```
pub struct ArrayFilter {
    path: String,
    element_filter: Box<dyn Filter>,
    mode: ArrayFilterMode,
}

#[derive(Debug, Clone, Copy)]
pub enum ArrayFilterMode {
    All,  // All elements must match
    Any,  // At least one element must match
}

impl ArrayFilter {
    /// Create a new array filter
    ///
    /// # Arguments
    /// * `path` - JSON path to the array field
    /// * `element_filter` - Filter to apply to each element
    /// * `mode` - ALL or ANY matching mode
    pub fn new(path: &str, element_filter: Box<dyn Filter>, mode: ArrayFilterMode) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            element_filter,
            mode,
        })
    }

    /// Extract value from JSON using path
    fn extract_value<'a>(&self, value: &'a Value) -> Option<&'a Value> {
        let parts: Vec<&str> = self.path.trim_matches('/').split('/').collect();

        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        Some(current)
    }
}

impl Filter for ArrayFilter {
    fn evaluate(&self, value: &Value) -> Result<bool> {
        if let Some(extracted) = self.extract_value(value) {
            if let Some(arr) = extracted.as_array() {
                match self.mode {
                    ArrayFilterMode::All => {
                        for element in arr {
                            if !self.element_filter.evaluate(element)? {
                                return Ok(false);
                            }
                        }
                        Ok(true)
                    }
                    ArrayFilterMode::Any => {
                        for element in arr {
                            if self.element_filter.evaluate(element)? {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    }
                }
            } else {
                Ok(false) // Not an array
            }
        } else {
            Ok(false) // Field not found
        }
    }
}

/// Array map transform
///
/// Transforms each element in an array using a transform function.
///
/// Example:
/// ```
/// # use streamforge::filter::{ArrayMapTransform, JsonPathTransform};
/// // Extract "id" field from each element in the array
/// let element_transform = Box::new(JsonPathTransform::new("/id").unwrap());
/// let transform = ArrayMapTransform::new("/users", element_transform).unwrap();
/// ```
pub struct ArrayMapTransform {
    path: String,
    element_transform: Box<dyn Transform>,
}

impl ArrayMapTransform {
    /// Create a new array map transform
    ///
    /// # Arguments
    /// * `path` - JSON path to the array field
    /// * `element_transform` - Transform to apply to each element
    pub fn new(path: &str, element_transform: Box<dyn Transform>) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            element_transform,
        })
    }

    /// Extract value from JSON using path
    fn extract_value(&self, value: &Value) -> Option<Value> {
        let parts: Vec<&str> = self.path.trim_matches('/').split('/').collect();

        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        Some(current.clone())
    }
}

impl Transform for ArrayMapTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        if let Some(extracted) = self.extract_value(&value) {
            if let Some(arr) = extracted.as_array() {
                let mut result = Vec::new();
                for element in arr {
                    let transformed = self.element_transform.transform(element.clone())?;
                    result.push(transformed);
                }
                Ok(Value::Array(result))
            } else {
                Err(MirrorMakerError::Processing(format!(
                    "Path {} is not an array",
                    self.path
                )))
            }
        } else {
            Err(MirrorMakerError::Processing(format!(
                "Path not found: {}",
                self.path
            )))
        }
    }
}

/// Arithmetic transform
///
/// Performs arithmetic operations on numeric fields.
///
/// Operations: ADD, SUB(tract), MUL(tiply), DIV(ide)
///
/// Can operate on:
/// - Two JSON paths: result = path1 op path2
/// - One JSON path and a constant: result = path op constant
///
/// Example:
/// ```
/// # use streamforge::filter::{ArithmeticTransform, ArithmeticOp};
/// // Add two fields: total = price + tax
/// let transform = ArithmeticTransform::new_with_paths(
///     ArithmeticOp::Add,
///     "/price",
///     "/tax"
/// ).unwrap();
///
/// // Multiply by constant: total = price * 1.2
/// let transform = ArithmeticTransform::new_with_constant(
///     ArithmeticOp::Mul,
///     "/price",
///     1.2
/// ).unwrap();
/// ```
pub struct ArithmeticTransform {
    op: ArithmeticOp,
    left_path: String,
    right: ArithmeticOperand,
}

#[derive(Debug, Clone, Copy)]
pub enum ArithmeticOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone)]
enum ArithmeticOperand {
    Path(String),
    Constant(f64),
}

impl ArithmeticTransform {
    /// Create arithmetic transform with two paths
    pub fn new_with_paths(op: ArithmeticOp, left_path: &str, right_path: &str) -> Result<Self> {
        Ok(Self {
            op,
            left_path: left_path.to_string(),
            right: ArithmeticOperand::Path(right_path.to_string()),
        })
    }

    /// Create arithmetic transform with path and constant
    pub fn new_with_constant(op: ArithmeticOp, left_path: &str, constant: f64) -> Result<Self> {
        Ok(Self {
            op,
            left_path: left_path.to_string(),
            right: ArithmeticOperand::Constant(constant),
        })
    }

    /// Extract value from JSON using path
    fn extract_value(&self, value: &Value, path: &str) -> Option<f64> {
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();

        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        current.as_f64()
    }

    /// Perform the arithmetic operation
    fn calculate(&self, left: f64, right: f64) -> Result<f64> {
        match self.op {
            ArithmeticOp::Add => Ok(left + right),
            ArithmeticOp::Sub => Ok(left - right),
            ArithmeticOp::Mul => Ok(left * right),
            ArithmeticOp::Div => {
                if right.abs() < f64::EPSILON {
                    Err(MirrorMakerError::Processing("Division by zero".to_string()))
                } else {
                    Ok(left / right)
                }
            }
        }
    }
}

impl Transform for ArithmeticTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let left = self.extract_value(&value, &self.left_path)
            .ok_or_else(|| {
                MirrorMakerError::Processing(format!("Left operand not found or not a number: {}", self.left_path))
            })?;

        let right = match &self.right {
            ArithmeticOperand::Path(path) => self.extract_value(&value, path)
                .ok_or_else(|| {
                    MirrorMakerError::Processing(format!("Right operand not found or not a number: {}", path))
                })?,
            ArithmeticOperand::Constant(c) => *c,
        };

        let result = self.calculate(left, right)?;
        Ok(json!(result))
    }
}

/// Hash transform
///
/// Hashes a field value using the specified algorithm.
///
/// Algorithms: MD5, SHA256, SHA512, Murmur64, Murmur128
///
/// Example:
/// ```
/// # use streamforge::filter::HashTransform;
/// # use streamforge::hash::HashAlgorithm;
/// // Hash a field with SHA256
/// let transform = HashTransform::new("/message/userId", HashAlgorithm::Sha256).unwrap();
///
/// // Hash with MD5 for fast partitioning
/// let transform = HashTransform::new("/message/id", HashAlgorithm::Md5).unwrap();
///
/// // Hash with Murmur for deterministic distribution
/// let transform = HashTransform::new("/message/key", HashAlgorithm::Murmur128).unwrap();
/// ```
pub struct HashTransform {
    path: String,
    algorithm: HashAlgorithm,
    output_field: Option<String>,
}

impl HashTransform {
    /// Create a new hash transform
    ///
    /// # Arguments
    /// * `path` - JSON path to the field to hash
    /// * `algorithm` - Hash algorithm to use
    pub fn new(path: &str, algorithm: HashAlgorithm) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            algorithm,
            output_field: None,
        })
    }

    /// Create a new hash transform with output field name
    ///
    /// # Arguments
    /// * `path` - JSON path to the field to hash
    /// * `algorithm` - Hash algorithm to use
    /// * `output_field` - Name of the field to store hash in (preserves original)
    pub fn new_with_output(path: &str, algorithm: HashAlgorithm, output_field: &str) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            algorithm,
            output_field: Some(output_field.to_string()),
        })
    }

    /// Extract value from JSON using path
    fn extract_value(&self, value: &Value) -> Option<Value> {
        let parts: Vec<&str> = self.path.trim_matches('/').split('/').collect();

        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        Some(current.clone())
    }
}

impl Transform for HashTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = self
            .extract_value(&value)
            .ok_or_else(|| MirrorMakerError::Processing(format!("Path not found: {}", self.path)))?;

        let hash = hash_value(&extracted, self.algorithm)?;

        // If output_field is specified, merge hash with original value
        if let Some(output_field) = &self.output_field {
            if let Value::Object(mut obj) = value {
                obj.insert(output_field.clone(), Value::String(hash));
                Ok(Value::Object(obj))
            } else {
                // If not an object, create a new object with both
                let mut result = Map::new();
                result.insert("original".to_string(), value);
                result.insert(output_field.clone(), Value::String(hash));
                Ok(Value::Object(result))
            }
        } else {
            // Replace with hash value only
            Ok(Value::String(hash))
        }
    }
}

/// Cache lookup transform
///
/// Looks up a value in cache and enriches the message with the result.
///
/// Example:
/// ```ignore
/// # use streamforge::filter::CacheLookupTransform;
/// # use streamforge::cache::{LookupCache, CacheConfig};
/// # use std::sync::Arc;
/// // Create cache
/// let cache = Arc::new(LookupCache::new(CacheConfig::default()));
///
/// // Lookup user by ID and add to message
/// let transform = CacheLookupTransform::new(
///     cache,
///     "/userId",           // Key path in message
///     "user",              // Cache key prefix
///     Some("userProfile")  // Output field name
/// ).unwrap();
/// ```
pub struct CacheLookupTransform {
    cache: Arc<LookupCache>,
    key_path: String,
    cache_key_prefix: Option<String>,
    output_field: String,
    merge_result: bool,
}

impl CacheLookupTransform {
    /// Create a new cache lookup transform
    ///
    /// # Arguments
    /// * `cache` - Shared cache instance
    /// * `key_path` - JSON path to extract lookup key from
    /// * `cache_key_prefix` - Optional prefix for cache keys (e.g., "user:")
    /// * `output_field` - Field name to store lookup result
    pub fn new(
        cache: Arc<LookupCache>,
        key_path: &str,
        cache_key_prefix: Option<&str>,
        output_field: Option<&str>,
    ) -> Result<Self> {
        Ok(Self {
            cache,
            key_path: key_path.to_string(),
            cache_key_prefix: cache_key_prefix.map(|s| s.to_string()),
            output_field: output_field.unwrap_or("lookup_result").to_string(),
            merge_result: false,
        })
    }

    /// Create a cache lookup transform that merges results
    ///
    /// Instead of adding a new field, merge the lookup result into the message
    pub fn new_with_merge(
        cache: Arc<LookupCache>,
        key_path: &str,
        cache_key_prefix: Option<&str>,
    ) -> Result<Self> {
        Ok(Self {
            cache,
            key_path: key_path.to_string(),
            cache_key_prefix: cache_key_prefix.map(|s| s.to_string()),
            output_field: String::new(),
            merge_result: true,
        })
    }

    /// Extract lookup key from JSON using path
    fn extract_key(&self, value: &Value) -> Option<String> {
        let parts: Vec<&str> = self.key_path.trim_matches('/').split('/').collect();

        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        // Convert to string
        match current {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }

    /// Build cache key with optional prefix
    fn build_cache_key(&self, key: &str) -> String {
        if let Some(prefix) = &self.cache_key_prefix {
            format!("{}:{}", prefix, key)
        } else {
            key.to_string()
        }
    }
}

#[async_trait::async_trait]
impl Transform for CacheLookupTransform {
    fn transform(&self, _value: Value) -> Result<Value> {
        // Note: We can't use async in the sync Transform trait
        // This is a limitation - in practice, you'd want to use an async transform trait
        // For now, this serves as a placeholder structure
        Err(MirrorMakerError::Processing(
            "CacheLookupTransform requires async context - use AsyncTransform instead".to_string()
        ))
    }
}

/// Async transform trait for cache lookups
#[async_trait::async_trait]
pub trait AsyncTransform: Send + Sync {
    async fn transform_async(&self, value: Value) -> Result<Value>;
}

#[async_trait::async_trait]
impl AsyncTransform for CacheLookupTransform {
    async fn transform_async(&self, value: Value) -> Result<Value> {
        let key = self
            .extract_key(&value)
            .ok_or_else(|| MirrorMakerError::Processing(format!("Key path not found: {}", self.key_path)))?;

        let cache_key = self.build_cache_key(&key);

        // Lookup in cache
        if let Some(lookup_result) = self.cache.get(&cache_key).await {
            if self.merge_result {
                // Merge lookup result into original value
                if let (Value::Object(mut orig_obj), Value::Object(lookup_obj)) = (value, lookup_result) {
                    for (k, v) in lookup_obj {
                        orig_obj.insert(k, v);
                    }
                    Ok(Value::Object(orig_obj))
                } else {
                    Err(MirrorMakerError::Processing(
                        "Cannot merge: both values must be objects".to_string()
                    ))
                }
            } else {
                // Add lookup result as new field
                if let Value::Object(mut obj) = value {
                    obj.insert(self.output_field.clone(), lookup_result);
                    Ok(Value::Object(obj))
                } else {
                    // If not an object, create a new object
                    let mut result = Map::new();
                    result.insert("original".to_string(), value);
                    result.insert(self.output_field.clone(), lookup_result);
                    Ok(Value::Object(result))
                }
            }
        } else {
            // Cache miss - return original value unchanged
            tracing::debug!("Cache miss for key: {}", cache_key);
            Ok(value)
        }
    }
}
