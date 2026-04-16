mod envelope_filter;
mod envelope_transform;

pub use envelope_filter::{
    HeaderExistsFilter, HeaderFilter, KeyContainsFilter, KeyExistsFilter, KeyMatchesFilter,
    KeyPrefixFilter, KeySuffixFilter, TimestampAfterFilter, TimestampAgeFilter,
    TimestampBeforeFilter,
};

pub use envelope_transform::{
    EnvelopeTransform, HeaderCopyTransform, HeaderFromTransform, HeaderRemoveTransform,
    HeaderSetTransform, KeyConstantTransform, KeyConstructTransform, KeyFromTransform,
    KeyHashTransform, KeyTemplateTransform, TimestampAddTransform, TimestampCurrentTransform,
    TimestampFromTransform, TimestampPreserveTransform, TimestampSubtractTransform,
};

use crate::cache::SyncLookupCache;
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
            _ => {
                return Err(MirrorMakerError::Config(format!(
                    "Unknown operator: {}",
                    operator
                )))
            }
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
                actual.as_f64().is_some_and(|v| v > *expected)
            }
            (ComparisonValue::Number(expected), ComparisonOp::Gte) => {
                actual.as_f64().is_some_and(|v| v >= *expected)
            }
            (ComparisonValue::Number(expected), ComparisonOp::Lt) => {
                actual.as_f64().is_some_and(|v| v < *expected)
            }
            (ComparisonValue::Number(expected), ComparisonOp::Lte) => {
                actual.as_f64().is_some_and(|v| v <= *expected)
            }
            (ComparisonValue::Number(expected), ComparisonOp::Eq) => actual
                .as_f64()
                .is_some_and(|v| (v - *expected).abs() < f64::EPSILON),
            (ComparisonValue::Number(expected), ComparisonOp::Ne) => actual
                .as_f64()
                .is_none_or(|v| (v - *expected).abs() >= f64::EPSILON),
            (ComparisonValue::String(expected), ComparisonOp::Eq) => {
                actual.as_str().is_some_and(|v| v == expected)
            }
            (ComparisonValue::String(expected), ComparisonOp::Ne) => {
                actual.as_str().is_none_or(|v| v != expected)
            }
            (ComparisonValue::Bool(expected), ComparisonOp::Eq) => {
                actual.as_bool() == Some(*expected)
            }
            (ComparisonValue::Bool(expected), ComparisonOp::Ne) => {
                actual.as_bool() != Some(*expected)
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
        match self.extract_value(&value) {
            Some(extracted) => Ok(extracted),
            None => {
                tracing::debug!("JsonPathTransform: path '{}' not found, passing through", self.path);
                Ok(value)
            }
        }
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
    All, // All elements must match
    Any, // At least one element must match
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
        let Some(extracted) = self.extract_value(&value) else {
            tracing::debug!("ARRAY_MAP: path '{}' not found, passing through", self.path);
            return Ok(value);
        };
        let Some(arr) = extracted.as_array() else {
            tracing::debug!("ARRAY_MAP: path '{}' is not an array, passing through", self.path);
            return Ok(value);
        };
        let mut result = Vec::new();
        for element in arr {
            let transformed = self.element_transform.transform(element.clone())?;
            result.push(transformed);
        }
        Ok(Value::Array(result))
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
        let Some(left) = self.extract_value(&value, &self.left_path) else {
            tracing::debug!("ARITHMETIC: left operand '{}' not found or not a number, passing through", self.left_path);
            return Ok(value);
        };

        let right = match &self.right {
            ArithmeticOperand::Path(path) => {
                let Some(r) = self.extract_value(&value, path) else {
                    tracing::debug!("ARITHMETIC: right operand '{}' not found or not a number, passing through", path);
                    return Ok(value);
                };
                r
            }
            ArithmeticOperand::Constant(c) => *c,
        };

        match self.calculate(left, right) {
            Ok(result) => Ok(json!(result)),
            Err(e) => {
                tracing::debug!("ARITHMETIC: calculation failed ({}), passing through", e);
                Ok(value)
            }
        }
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
    pub fn new_with_output(
        path: &str,
        algorithm: HashAlgorithm,
        output_field: &str,
    ) -> Result<Self> {
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
        let Some(extracted) = self.extract_value(&value) else {
            tracing::debug!("HASH: path '{}' not found, passing through", self.path);
            return Ok(value);
        };

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

/// Cache lookup transform — reads from a named `SyncLookupCache` and enriches the message.
///
/// On a cache hit the looked-up value is added to (or merged into) the message.
/// On a cache miss the message is returned unchanged.
///
/// DSL syntax (via `parse_transform_with_cache`):
/// - `CACHE_LOOKUP:/keyPath,store-name,outputField` — add result as a new field
/// - `CACHE_LOOKUP:/keyPath,store-name,MERGE`       — merge result into the message object
pub struct CacheLookupTransform {
    cache: Arc<SyncLookupCache>,
    key_path: String,
    /// When `Some(field)` the lookup result is added under that field name.
    /// When `None` the lookup result is merged directly into the message object.
    output_field: Option<String>,
}

impl CacheLookupTransform {
    /// Add the cached value as a new field in the message.
    pub fn new(cache: Arc<SyncLookupCache>, key_path: &str, output_field: &str) -> Result<Self> {
        Ok(Self {
            cache,
            key_path: key_path.to_string(),
            output_field: Some(output_field.to_string()),
        })
    }

    /// Merge the cached object directly into the message object.
    pub fn new_merge(cache: Arc<SyncLookupCache>, key_path: &str) -> Result<Self> {
        Ok(Self {
            cache,
            key_path: key_path.to_string(),
            output_field: None,
        })
    }

    fn extract_key(&self, value: &Value) -> Option<String> {
        let parts: Vec<&str> = self.key_path.trim_matches('/').split('/').collect();
        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }
        match current {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }
}

impl Transform for CacheLookupTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let Some(key) = self.extract_key(&value) else {
            tracing::debug!("CACHE_LOOKUP: key path '{}' not found or not a string/number, passing through", self.key_path);
            return Ok(value);
        };

        let Some(cached) = self.cache.get(&key) else {
            tracing::debug!("CACHE_LOOKUP miss for key '{}'", key);
            return Ok(value);
        };

        match &self.output_field {
            Some(field) => {
                // Add lookup result as a new named field
                if let Value::Object(mut obj) = value {
                    obj.insert(field.clone(), cached);
                    Ok(Value::Object(obj))
                } else {
                    let mut result = Map::new();
                    result.insert("original".to_string(), value);
                    result.insert(field.clone(), cached);
                    Ok(Value::Object(result))
                }
            }
            None => {
                // Merge the cached object into the message object
                match (value, cached) {
                    (Value::Object(mut msg), Value::Object(cached_obj)) => {
                        for (k, v) in cached_obj {
                            msg.insert(k, v);
                        }
                        Ok(Value::Object(msg))
                    }
                    (original, _) => {
                        tracing::debug!("CACHE_LOOKUP MERGE: message or cached value is not a JSON object, passing through");
                        Ok(original)
                    }
                }
            }
        }
    }
}

/// Cache put transform — writes a value to a named `SyncLookupCache` and passes
/// the message through unchanged.
///
/// DSL syntax (via `parse_transform_with_cache`):
/// - `CACHE_PUT:/keyPath,store-name`             — store the entire message at the key
/// - `CACHE_PUT:/keyPath,store-name,/valuePath`  — store a field extracted from the message
pub struct CachePutTransform {
    cache: Arc<SyncLookupCache>,
    key_path: String,
    /// When `Some(path)` only that field is stored. When `None` the whole message is stored.
    value_path: Option<String>,
}

impl CachePutTransform {
    pub fn new(cache: Arc<SyncLookupCache>, key_path: &str, value_path: Option<&str>) -> Result<Self> {
        Ok(Self {
            cache,
            key_path: key_path.to_string(),
            value_path: value_path.map(|s| s.to_string()),
        })
    }

    fn extract_path(&self, value: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }
        Some(current.clone())
    }

    fn extract_key(&self, value: &Value) -> Option<String> {
        let extracted = self.extract_path(value, &self.key_path)?;
        match extracted {
            Value::String(s) => Some(s),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }
}

impl Transform for CachePutTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let Some(key) = self.extract_key(&value) else {
            tracing::debug!("CACHE_PUT: key path '{}' not found or not a string/number, skipping cache write", self.key_path);
            return Ok(value);
        };

        let to_store = match &self.value_path {
            Some(path) => match self.extract_path(&value, path) {
                Some(v) => v,
                None => {
                    tracing::debug!("CACHE_PUT: value path '{}' not found, skipping cache write", path);
                    return Ok(value);
                }
            },
            None => value.clone(),
        };

        self.cache.put(key.clone(), to_store);
        tracing::debug!("CACHE_PUT stored key '{}'", key);

        Ok(value) // pass message through unchanged
    }
}

// ============================================================================
// String transforms
// ============================================================================

/// Extract a string value from a JSON path. Numbers and booleans are coerced.
/// Returns `None` when the path is absent, null, or a non-scalar type (object/array).
fn get_string_at_path(value: &Value, path: &str) -> Option<String> {
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    let mut current = value;
    for part in &parts {
        current = current.get(part)?;
    }
    match current {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        // Null, Object, Array are not coercible to a string for transform purposes
        _ => None,
    }
}

/// Write `result` into the message. If `output_field` is set adds a new top-level
/// field and keeps the original. Otherwise overwrites the field at `path`.
fn write_string_result(
    value: Value,
    path: &str,
    result: Value,
    output_field: Option<&str>,
) -> Result<Value> {
    match output_field {
        Some(field) => {
            // Add result as a new top-level field, keep original
            if let Value::Object(mut obj) = value {
                obj.insert(field.to_string(), result);
                Ok(Value::Object(obj))
            } else {
                Err(MirrorMakerError::Processing(
                    "STRING: output_field requires the message to be a JSON object".to_string(),
                ))
            }
        }
        None => set_value_at_path(value, path, result),
    }
}

/// Overwrite the value at `path` inside `root` with `new_val`.
fn set_value_at_path(root: Value, path: &str, new_val: Value) -> Result<Value> {
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
        return Ok(new_val);
    }
    let mut root = root;
    set_nested(&mut root, &parts, new_val)?;
    Ok(root)
}

fn set_nested(node: &mut Value, parts: &[&str], new_val: Value) -> Result<()> {
    if parts.is_empty() {
        return Ok(());
    }
    let obj = node.as_object_mut().ok_or_else(|| {
        MirrorMakerError::Processing("STRING: cannot descend into a non-object".to_string())
    })?;
    if parts.len() == 1 {
        obj.insert(parts[0].to_string(), new_val);
    } else {
        let child = obj.get_mut(parts[0]).ok_or_else(|| {
            MirrorMakerError::Processing(format!("STRING: intermediate path '{}' not found", parts[0]))
        })?;
        set_nested(child, &parts[1..], new_val)?;
    }
    Ok(())
}

/// String operations supported by [`StringTransform`].
#[derive(Clone)]
pub enum StringOp {
    Upper,
    Lower,
    Trim,
    TrimStart,
    TrimEnd,
    Substring { start: usize, length: Option<usize> },
    Replace { from: String, to: String, all: bool },
    RegexReplace { pattern: Regex, replacement: String },
    Split { delimiter: String },
    Length,
}

/// Apply a string operation to a JSON field, optionally writing to a separate output field.
///
/// DSL syntax (parsed via `filter_parser::parse_transform`):
///
/// | Expression | Description |
/// |---|---|
/// | `STRING:UPPER,/path` | Uppercase the field |
/// | `STRING:LOWER,/path` | Lowercase the field |
/// | `STRING:TRIM,/path` | Trim leading and trailing whitespace |
/// | `STRING:TRIM_START,/path` | Trim leading whitespace |
/// | `STRING:TRIM_END,/path` | Trim trailing whitespace |
/// | `STRING:SUBSTRING,/path,start,length` | Extract substring (length optional) |
/// | `STRING:REPLACE,/path,from,to` | Replace first occurrence of `from` with `to` |
/// | `STRING:REPLACE_ALL,/path,from,to` | Replace all occurrences |
/// | `STRING:REGEX_REPLACE,/path,pattern,replacement` | Regex replace all matches |
/// | `STRING:SPLIT,/path,delimiter` | Split into a JSON array |
/// | `STRING:LENGTH,/path` | Replace field with its character length |
///
/// All operations accept an optional extra `,outputField` argument.  When
/// present the result is stored in a new top-level field and the original is
/// preserved.  Without it the source field is overwritten.
pub struct StringTransform {
    pub path: String,
    pub op: StringOp,
    pub output_field: Option<String>,
}

impl StringTransform {
    pub fn new(path: &str, op: StringOp, output_field: Option<&str>) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            op,
            output_field: output_field.map(|s| s.to_string()),
        })
    }

    fn apply(&self, s: &str) -> Result<Value> {
        match &self.op {
            StringOp::Upper => Ok(Value::String(s.to_uppercase())),
            StringOp::Lower => Ok(Value::String(s.to_lowercase())),
            StringOp::Trim => Ok(Value::String(s.trim().to_string())),
            StringOp::TrimStart => Ok(Value::String(s.trim_start().to_string())),
            StringOp::TrimEnd => Ok(Value::String(s.trim_end().to_string())),
            StringOp::Length => Ok(json!(s.chars().count())),
            StringOp::Substring { start, length } => {
                let chars: Vec<char> = s.chars().collect();
                let start = (*start).min(chars.len());
                let slice = match length {
                    Some(len) => &chars[start..(start + len).min(chars.len())],
                    None => &chars[start..],
                };
                Ok(Value::String(slice.iter().collect()))
            }
            StringOp::Replace { from, to, all } => {
                let result = if *all {
                    s.replace(from.as_str(), to.as_str())
                } else {
                    s.replacen(from.as_str(), to.as_str(), 1)
                };
                Ok(Value::String(result))
            }
            StringOp::RegexReplace { pattern, replacement } => {
                let result = pattern.replace_all(s, replacement.as_str()).to_string();
                Ok(Value::String(result))
            }
            StringOp::Split { delimiter } => {
                let parts: Vec<Value> = s
                    .split(delimiter.as_str())
                    .map(|p| Value::String(p.to_string()))
                    .collect();
                Ok(Value::Array(parts))
            }
        }
    }
}

impl Transform for StringTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let Some(s) = get_string_at_path(&value, &self.path) else {
            tracing::debug!("STRING: path '{}' not found or not a string/scalar, passing through", self.path);
            return Ok(value);
        };
        let result = self.apply(&s)?;
        write_string_result(value, &self.path, result, self.output_field.as_deref())
    }
}

/// Concatenate multiple fields and/or literal strings into a single output field.
///
/// DSL syntax:
/// ```text
/// STRING:CONCAT,outputField,/firstName, ,/lastName
/// ```
/// Parts starting with `/` are JSON path extractions; all other parts are literals.
///
/// Numeric and boolean fields are coerced to their string representation.
pub struct ConcatTransform {
    pub parts: Vec<ConcatPart>,
    pub output_field: String,
}

/// A single segment in a [`ConcatTransform`].
#[derive(Debug, Clone)]
pub enum ConcatPart {
    Literal(String),
    Path(String),
}

impl ConcatTransform {
    pub fn new(output_field: &str, parts: Vec<ConcatPart>) -> Self {
        Self {
            parts,
            output_field: output_field.to_string(),
        }
    }
}

impl Transform for ConcatTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let mut buf = String::new();
        for part in &self.parts {
            match part {
                ConcatPart::Literal(s) => buf.push_str(s),
                ConcatPart::Path(path) => match get_string_at_path(&value, path) {
                    Some(s) => buf.push_str(&s),
                    None => {
                        tracing::debug!("CONCAT: path '{}' not found or not a string, using empty string", path);
                    }
                },
            }
        }

        if let Value::Object(mut obj) = value {
            obj.insert(self.output_field.clone(), Value::String(buf));
            Ok(Value::Object(obj))
        } else {
            tracing::debug!("CONCAT: message is not a JSON object, passing through");
            Ok(value)
        }
    }
}
