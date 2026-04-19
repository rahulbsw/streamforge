// String transformation functions for DSL v2.0

use super::Transform;
use crate::error::{MirrorMakerError, Result};
use serde_json::Value;

/// Helper function to extract value from JSON path
fn extract_path(value: &Value, path: &str) -> Option<Value> {
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();

    let mut current = value;
    for part in parts {
        current = current.get(part)?;
    }

    Some(current.clone())
}

/// String length transform
pub struct StringLengthTransform {
    path: String,
}

impl StringLengthTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self { path: path.into() })
    }
}

impl Transform for StringLengthTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        match extracted {
            Value::String(s) => Ok(Value::Number(s.len().into())),
            Value::Array(arr) => Ok(Value::Number(arr.len().into())),
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot get length of non-string/array at path '{}'",
                self.path
            ))),
        }
    }
}

/// Substring transform
pub struct SubstringTransform {
    path: String,
    start: usize,
    end: Option<usize>,
}

impl SubstringTransform {
    pub fn new(path: impl Into<String>, start: usize, end: Option<usize>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            start,
            end,
        })
    }
}

impl Transform for SubstringTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        match extracted {
            Value::String(s) => {
                let chars: Vec<char> = s.chars().collect();
                let start = self.start.min(chars.len());
                let end = self.end.unwrap_or(chars.len()).min(chars.len());

                if start >= end {
                    Ok(Value::String(String::new()))
                } else {
                    let substring: String = chars[start..end].iter().collect();
                    Ok(Value::String(substring))
                }
            }
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot substring non-string at path '{}'",
                self.path
            ))),
        }
    }
}

/// Split string transform
pub struct SplitTransform {
    path: String,
    delimiter: String,
}

impl SplitTransform {
    pub fn new(path: impl Into<String>, delimiter: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            delimiter: delimiter.into(),
        })
    }
}

impl Transform for SplitTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        match extracted {
            Value::String(s) => {
                let parts: Vec<Value> = s
                    .split(&self.delimiter)
                    .map(|part| Value::String(part.to_string()))
                    .collect();
                Ok(Value::Array(parts))
            }
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot split non-string at path '{}'",
                self.path
            ))),
        }
    }
}

/// Join array transform
pub struct JoinTransform {
    path: String,
    separator: String,
}

impl JoinTransform {
    pub fn new(path: impl Into<String>, separator: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            separator: separator.into(),
        })
    }
}

impl Transform for JoinTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        match extracted {
            Value::Array(arr) => {
                let mut strings = Vec::new();
                for v in arr.iter() {
                    let s = match v {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        Value::Null => "null".to_string(),
                        _ => serde_json::to_string(v).map_err(|e| {
                            MirrorMakerError::Config(format!(
                                "Cannot serialize array element to string during join at path '{}': {}",
                                self.path, e
                            ))
                        })?,
                    };
                    strings.push(s);
                }
                Ok(Value::String(strings.join(&self.separator)))
            }
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot join non-array at path '{}'",
                self.path
            ))),
        }
    }
}

/// Replace pattern transform
pub struct ReplaceTransform {
    path: String,
    pattern: String,
    replacement: String,
}

impl ReplaceTransform {
    pub fn new(
        path: impl Into<String>,
        pattern: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            pattern: pattern.into(),
            replacement: replacement.into(),
        })
    }
}

impl Transform for ReplaceTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        match extracted {
            Value::String(s) => {
                let result = s.replace(&self.pattern, &self.replacement);
                Ok(Value::String(result))
            }
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot replace in non-string at path '{}'",
                self.path
            ))),
        }
    }
}

/// Pad left transform
pub struct PadLeftTransform {
    path: String,
    width: usize,
    pad_char: char,
}

impl PadLeftTransform {
    pub fn new(path: impl Into<String>, width: usize, pad_char: char) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            width,
            pad_char,
        })
    }
}

impl Transform for PadLeftTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        match extracted {
            Value::String(s) => {
                let len = s.chars().count();
                if len >= self.width {
                    Ok(Value::String(s.clone()))
                } else {
                    let padding = self.pad_char.to_string().repeat(self.width - len);
                    Ok(Value::String(format!("{}{}", padding, s)))
                }
            }
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot pad non-string at path '{}'",
                self.path
            ))),
        }
    }
}

/// Pad right transform
pub struct PadRightTransform {
    path: String,
    width: usize,
    pad_char: char,
}

impl PadRightTransform {
    pub fn new(path: impl Into<String>, width: usize, pad_char: char) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            width,
            pad_char,
        })
    }
}

impl Transform for PadRightTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        match extracted {
            Value::String(s) => {
                let len = s.chars().count();
                if len >= self.width {
                    Ok(Value::String(s.clone()))
                } else {
                    let padding = self.pad_char.to_string().repeat(self.width - len);
                    Ok(Value::String(format!("{}{}", s, padding)))
                }
            }
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot pad non-string at path '{}'",
                self.path
            ))),
        }
    }
}

/// To string transform
pub struct ToStringTransform {
    path: String,
}

impl ToStringTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self { path: path.into() })
    }
}

impl Transform for ToStringTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        let result = match extracted {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            _ => serde_json::to_string(&extracted).map_err(|e| {
                MirrorMakerError::Config(format!("Cannot convert to string: {}", e))
            })?,
        };

        Ok(Value::String(result))
    }
}

/// To int transform
pub struct ToIntTransform {
    path: String,
}

impl ToIntTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self { path: path.into() })
    }
}

impl Transform for ToIntTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        let result = match extracted {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    i
                } else if let Some(f) = n.as_f64() {
                    f as i64
                } else {
                    return Err(MirrorMakerError::Config(
                        "Cannot convert number to int".to_string(),
                    ));
                }
            }
            Value::String(s) => s.parse::<i64>().map_err(|e| {
                MirrorMakerError::Config(format!("Cannot parse string to int: {}", e))
            })?,
            Value::Bool(b) => {
                if b {
                    1
                } else {
                    0
                }
            }
            _ => {
                return Err(MirrorMakerError::Config(format!(
                    "Cannot convert {:?} to int",
                    extracted
                )));
            }
        };

        Ok(Value::Number(result.into()))
    }
}

/// To float transform
pub struct ToFloatTransform {
    path: String,
}

impl ToFloatTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self { path: path.into() })
    }
}

impl Transform for ToFloatTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        let result = match extracted {
            Value::Number(n) => n.as_f64().ok_or_else(|| {
                MirrorMakerError::Config("Cannot convert number to float".to_string())
            })?,
            Value::String(s) => s.parse::<f64>().map_err(|e| {
                MirrorMakerError::Config(format!("Cannot parse string to float: {}", e))
            })?,
            Value::Bool(b) => {
                if b {
                    1.0
                } else {
                    0.0
                }
            }
            _ => {
                return Err(MirrorMakerError::Config(format!(
                    "Cannot convert {:?} to float",
                    extracted
                )));
            }
        };

        serde_json::Number::from_f64(result)
            .map(Value::Number)
            .ok_or_else(|| MirrorMakerError::Config("Invalid float value".to_string()))
    }
}

/// Uppercase transform
pub struct UppercaseTransform {
    path: String,
}

impl UppercaseTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self { path: path.into() })
    }
}

impl Transform for UppercaseTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        match extracted {
            Value::String(s) => Ok(Value::String(s.to_uppercase())),
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot uppercase non-string at path '{}'",
                self.path
            ))),
        }
    }
}

/// Lowercase transform
pub struct LowercaseTransform {
    path: String,
}

impl LowercaseTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self { path: path.into() })
    }
}

impl Transform for LowercaseTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        match extracted {
            Value::String(s) => Ok(Value::String(s.to_lowercase())),
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot lowercase non-string at path '{}'",
                self.path
            ))),
        }
    }
}

/// Trim start transform
pub struct TrimStartTransform {
    path: String,
}

impl TrimStartTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self { path: path.into() })
    }
}

impl Transform for TrimStartTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        match extracted {
            Value::String(s) => Ok(Value::String(s.trim_start().to_string())),
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot trim non-string at path '{}'",
                self.path
            ))),
        }
    }
}

/// Trim end transform
pub struct TrimEndTransform {
    path: String,
}

impl TrimEndTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self { path: path.into() })
    }
}

impl Transform for TrimEndTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted =
            extract_path(&value, &self.path).ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            })?;

        match extracted {
            Value::String(s) => Ok(Value::String(s.trim_end().to_string())),
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot trim non-string at path '{}'",
                self.path
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_string_length() {
        let transform = StringLengthTransform::new("/name").unwrap();
        let input = json!({"name": "hello"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn test_substring() {
        let transform = SubstringTransform::new("/text", 0, Some(5)).unwrap();
        let input = json!({"text": "hello world"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn test_split() {
        let transform = SplitTransform::new("/csv", ",").unwrap();
        let input = json!({"csv": "a,b,c"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(["a", "b", "c"]));
    }

    #[test]
    fn test_join() {
        let transform = JoinTransform::new("/array", ", ").unwrap();
        let input = json!({"array": ["a", "b", "c"]});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!("a, b, c"));
    }

    #[test]
    fn test_replace() {
        let transform = ReplaceTransform::new("/text", "old", "new").unwrap();
        let input = json!({"text": "hello old world"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!("hello new world"));
    }

    #[test]
    fn test_pad_left() {
        let transform = PadLeftTransform::new("/id", 8, '0').unwrap();
        let input = json!({"id": "123"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!("00000123"));
    }

    #[test]
    fn test_to_string() {
        let transform = ToStringTransform::new("/number").unwrap();
        let input = json!({"number": 42});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!("42"));
    }

    #[test]
    fn test_to_int() {
        let transform = ToIntTransform::new("/string").unwrap();
        let input = json!({"string": "42"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn test_to_float() {
        let transform = ToFloatTransform::new("/string").unwrap();
        let input = json!({"string": "3.15"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(3.15));
    }

    #[test]
    fn test_uppercase() {
        let transform = UppercaseTransform::new("/name").unwrap();
        let input = json!({"name": "hello world"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!("HELLO WORLD"));
    }

    #[test]
    fn test_lowercase() {
        let transform = LowercaseTransform::new("/name").unwrap();
        let input = json!({"name": "HELLO WORLD"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!("hello world"));
    }
}
