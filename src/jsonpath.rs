/// Shared JSON path utilities
///
/// Consolidates JSON path parsing and value extraction to eliminate
/// code duplication across filters and transforms.
use serde_json::Value;

/// Pre-parsed JSON path segments for efficient traversal
///
/// Example: "/user/email" → ["user", "email"]
#[derive(Debug, Clone)]
pub struct JsonPath {
    /// Original path string (kept for error messages)
    pub path: String,
    /// Pre-parsed path segments
    pub segments: Vec<String>,
}

impl JsonPath {
    /// Create a new JsonPath from a string path
    ///
    /// # Arguments
    /// * `path` - JSON path string (e.g., "/user/email")
    ///
    /// # Examples
    /// ```
    /// use streamforge::jsonpath::JsonPath;
    ///
    /// let path = JsonPath::new("/user/email");
    /// ```
    pub fn new(path: &str) -> Self {
        let segments: Vec<String> = path
            .trim_matches('/')
            .split('/')
            .map(|s| s.to_string())
            .collect();

        Self {
            path: path.to_string(),
            segments,
        }
    }

    /// Extract a value reference from JSON using pre-parsed path segments
    ///
    /// Returns None if the path doesn't exist in the value.
    ///
    /// # Examples
    /// ```ignore
    /// use streamforge::jsonpath::JsonPath;
    /// use serde_json::json;
    ///
    /// let path = JsonPath::new("/user/email");
    /// let value = json!({"user": {"email": "test@example.com"}});
    ///
    /// let result = path.extract(&value);
    /// assert_eq!(result, Some(&json!("test@example.com")));
    /// ```
    pub fn extract<'a>(&self, value: &'a Value) -> Option<&'a Value> {
        let mut current = value;
        for part in &self.segments {
            current = current.get(part.as_str())?;
        }
        Some(current)
    }

    /// Extract a cloned value from JSON using pre-parsed path segments
    ///
    /// Returns None if the path doesn't exist in the value.
    pub fn extract_owned(&self, value: &Value) -> Option<Value> {
        self.extract(value).cloned()
    }

    /// Extract a string value from JSON path, with type coercion
    ///
    /// - Strings returned as-is
    /// - Numbers converted to strings
    /// - Booleans converted to strings
    /// - Other types return None
    pub fn extract_string(&self, value: &Value) -> Option<String> {
        let extracted = self.extract(value)?;
        match extracted {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    /// Extract a numeric (f64) value from JSON path
    ///
    /// Returns None if the path doesn't exist or value is not a number.
    pub fn extract_f64(&self, value: &Value) -> Option<f64> {
        self.extract(value)?.as_f64()
    }

    /// Extract an integer (i64) value from JSON path
    ///
    /// Returns None if the path doesn't exist or value is not an integer.
    pub fn extract_i64(&self, value: &Value) -> Option<i64> {
        self.extract(value)?.as_i64()
    }

    /// Extract a boolean value from JSON path
    ///
    /// Returns None if the path doesn't exist or value is not a boolean.
    pub fn extract_bool(&self, value: &Value) -> Option<bool> {
        self.extract(value)?.as_bool()
    }
}

/// Extract a value from JSON using segment slices (for backward compatibility)
///
/// This is used by code that already has pre-parsed segments.
pub fn extract_with_segments<'a>(value: &'a Value, segments: &[String]) -> Option<&'a Value> {
    let mut current = value;
    for part in segments {
        current = current.get(part.as_str())?;
    }
    Some(current)
}

/// Extract an owned value from JSON using segment slices
pub fn extract_owned_with_segments(value: &Value, segments: &[String]) -> Option<Value> {
    extract_with_segments(value, segments).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_jsonpath_new() {
        let path = JsonPath::new("/user/email");
        assert_eq!(path.path, "/user/email");
        assert_eq!(path.segments, vec!["user", "email"]);
    }

    #[test]
    fn test_extract_simple() {
        let path = JsonPath::new("/name");
        let value = json!({"name": "test"});
        let result = path.extract(&value);
        assert_eq!(result, Some(&json!("test")));
    }

    #[test]
    fn test_extract_nested() {
        let path = JsonPath::new("/user/email");
        let value = json!({"user": {"email": "test@example.com"}});
        let result = path.extract(&value);
        assert_eq!(result, Some(&json!("test@example.com")));
    }

    #[test]
    fn test_extract_missing() {
        let path = JsonPath::new("/user/missing");
        let value = json!({"user": {"email": "test@example.com"}});
        let result = path.extract(&value);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_string_coercion() {
        let path = JsonPath::new("/value");

        // String
        let value = json!({"value": "test"});
        assert_eq!(path.extract_string(&value), Some("test".to_string()));

        // Number
        let value = json!({"value": 123});
        assert_eq!(path.extract_string(&value), Some("123".to_string()));

        // Boolean
        let value = json!({"value": true});
        assert_eq!(path.extract_string(&value), Some("true".to_string()));

        // Object (not coercible)
        let value = json!({"value": {"nested": "obj"}});
        assert_eq!(path.extract_string(&value), None);
    }

    #[test]
    fn test_extract_numeric() {
        let path = JsonPath::new("/count");
        let value = json!({"count": 42});

        assert_eq!(path.extract_f64(&value), Some(42.0));
        assert_eq!(path.extract_i64(&value), Some(42));
    }

    #[test]
    fn test_extract_bool() {
        let path = JsonPath::new("/active");
        let value = json!({"active": true});

        assert_eq!(path.extract_bool(&value), Some(true));
    }

    #[test]
    fn test_extract_with_segments() {
        let value = json!({"user": {"email": "test@example.com"}});
        let segments = vec!["user".to_string(), "email".to_string()];

        let result = extract_with_segments(&value, &segments);
        assert_eq!(result, Some(&json!("test@example.com")));
    }
}
