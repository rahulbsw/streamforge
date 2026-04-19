// Date/time transformation functions for DSL v2.0

use super::Transform;
use crate::error::{MirrorMakerError, Result};
use chrono::{DateTime, Datelike, NaiveDateTime, TimeZone, Timelike, Utc};
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

/// Helper to parse ISO 8601 string to DateTime
fn parse_iso_string(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            // Try parsing as naive datetime and assume UTC
            NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
                .map(|ndt| Utc.from_utc_datetime(&ndt))
        })
        .map_err(|e| MirrorMakerError::Config(format!("Cannot parse datetime '{}': {}", s, e)))
}

/// Helper to parse datetime from various formats
fn parse_datetime_value(value: &Value, format: Option<&str>) -> Result<DateTime<Utc>> {
    match value {
        Value::String(s) => {
            if let Some(fmt) = format {
                // Parse with custom format
                NaiveDateTime::parse_from_str(s, fmt)
                    .map(|ndt| Utc.from_utc_datetime(&ndt))
                    .map_err(|e| {
                        MirrorMakerError::Config(format!(
                            "Cannot parse datetime '{}' with format '{}': {}",
                            s, fmt, e
                        ))
                    })
            } else {
                // Auto-detect ISO 8601
                parse_iso_string(s)
            }
        }
        Value::Number(n) => {
            // Assume epoch milliseconds
            let ms = n.as_i64().ok_or_else(|| {
                MirrorMakerError::Config("Cannot convert number to i64".to_string())
            })?;
            Utc.timestamp_millis_opt(ms).single().ok_or_else(|| {
                MirrorMakerError::Config(format!("Invalid timestamp: {}", ms))
            })
        }
        _ => Err(MirrorMakerError::Config(format!(
            "Cannot parse datetime from {:?}",
            value
        ))),
    }
}

/// Current timestamp (epoch milliseconds)
pub struct NowTransform;

impl NowTransform {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl Transform for NowTransform {
    fn transform(&self, _value: Value) -> Result<Value> {
        let now = Utc::now().timestamp_millis();
        Ok(Value::Number(now.into()))
    }
}

/// Current timestamp (ISO 8601)
pub struct NowIsoTransform;

impl NowIsoTransform {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl Transform for NowIsoTransform {
    fn transform(&self, _value: Value) -> Result<Value> {
        let now = Utc::now().to_rfc3339();
        Ok(Value::String(now))
    }
}

/// Parse date from string
pub struct ParseDateTransform {
    path: String,
    format: Option<String>,
}

impl ParseDateTransform {
    pub fn new(path: impl Into<String>, format: Option<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            format,
        })
    }
}

impl Transform for ParseDateTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, self.format.as_deref())?;
        Ok(Value::String(dt.to_rfc3339()))
    }
}

/// Convert epoch milliseconds to ISO 8601
pub struct FromEpochTransform {
    path: String,
}

impl FromEpochTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for FromEpochTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        match extracted {
            Value::Number(n) => {
                let ms = n.as_i64().ok_or_else(|| {
                    MirrorMakerError::Config("Cannot convert number to i64".to_string())
                })?;
                let dt = Utc.timestamp_millis_opt(ms).single().ok_or_else(|| {
                    MirrorMakerError::Config(format!("Invalid timestamp: {}", ms))
                })?;
                Ok(Value::String(dt.to_rfc3339()))
            }
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot convert non-number to datetime at path '{}'",
                self.path
            ))),
        }
    }
}

/// Convert epoch seconds to ISO 8601
pub struct FromEpochSecondsTransform {
    path: String,
}

impl FromEpochSecondsTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for FromEpochSecondsTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        match extracted {
            Value::Number(n) => {
                let secs = n.as_i64().ok_or_else(|| {
                    MirrorMakerError::Config("Cannot convert number to i64".to_string())
                })?;
                let dt = Utc.timestamp_opt(secs, 0).single().ok_or_else(|| {
                    MirrorMakerError::Config(format!("Invalid timestamp: {}", secs))
                })?;
                Ok(Value::String(dt.to_rfc3339()))
            }
            _ => Err(MirrorMakerError::Config(format!(
                "Cannot convert non-number to datetime at path '{}'",
                self.path
            ))),
        }
    }
}

/// Format date with custom format
pub struct FormatDateTransform {
    path: String,
    format: String,
}

impl FormatDateTransform {
    pub fn new(path: impl Into<String>, format: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            format: format.into(),
        })
    }
}

impl Transform for FormatDateTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        let formatted = dt.format(&self.format).to_string();
        Ok(Value::String(formatted))
    }
}

/// Convert datetime to epoch milliseconds
pub struct ToEpochTransform {
    path: String,
}

impl ToEpochTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for ToEpochTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        Ok(Value::Number(dt.timestamp_millis().into()))
    }
}

/// Convert datetime to epoch seconds
pub struct ToEpochSecondsTransform {
    path: String,
}

impl ToEpochSecondsTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for ToEpochSecondsTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        Ok(Value::Number(dt.timestamp().into()))
    }
}

/// Convert datetime to ISO 8601
pub struct ToIsoTransform {
    path: String,
}

impl ToIsoTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for ToIsoTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        Ok(Value::String(dt.to_rfc3339()))
    }
}

/// Add days to datetime
pub struct AddDaysTransform {
    path: String,
    days: i64,
}

impl AddDaysTransform {
    pub fn new(path: impl Into<String>, days: i64) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            days,
        })
    }
}

impl Transform for AddDaysTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        let new_dt = dt + chrono::Duration::days(self.days);
        Ok(Value::String(new_dt.to_rfc3339()))
    }
}

/// Add hours to datetime
pub struct AddHoursTransform {
    path: String,
    hours: i64,
}

impl AddHoursTransform {
    pub fn new(path: impl Into<String>, hours: i64) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            hours,
        })
    }
}

impl Transform for AddHoursTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        let new_dt = dt + chrono::Duration::hours(self.hours);
        Ok(Value::String(new_dt.to_rfc3339()))
    }
}

/// Add minutes to datetime
pub struct AddMinutesTransform {
    path: String,
    minutes: i64,
}

impl AddMinutesTransform {
    pub fn new(path: impl Into<String>, minutes: i64) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            minutes,
        })
    }
}

impl Transform for AddMinutesTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        let new_dt = dt + chrono::Duration::minutes(self.minutes);
        Ok(Value::String(new_dt.to_rfc3339()))
    }
}

/// Subtract days from datetime
pub struct SubtractDaysTransform {
    path: String,
    days: i64,
}

impl SubtractDaysTransform {
    pub fn new(path: impl Into<String>, days: i64) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            days,
        })
    }
}

impl Transform for SubtractDaysTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        let new_dt = dt - chrono::Duration::days(self.days);
        Ok(Value::String(new_dt.to_rfc3339()))
    }
}

/// Extract year from datetime
pub struct YearTransform {
    path: String,
}

impl YearTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for YearTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        Ok(Value::Number(dt.year().into()))
    }
}

/// Extract month from datetime (1-12)
pub struct MonthTransform {
    path: String,
}

impl MonthTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for MonthTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        Ok(Value::Number(dt.month().into()))
    }
}

/// Extract day from datetime (1-31)
pub struct DayTransform {
    path: String,
}

impl DayTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for DayTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        Ok(Value::Number(dt.day().into()))
    }
}

/// Extract hour from datetime (0-23)
pub struct HourTransform {
    path: String,
}

impl HourTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for HourTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        Ok(Value::Number(dt.hour().into()))
    }
}

/// Extract minute from datetime (0-59)
pub struct MinuteTransform {
    path: String,
}

impl MinuteTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for MinuteTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        Ok(Value::Number(dt.minute().into()))
    }
}

/// Extract second from datetime (0-59)
pub struct SecondTransform {
    path: String,
}

impl SecondTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for SecondTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        Ok(Value::Number(dt.second().into()))
    }
}

/// Extract day of week from datetime (0-6, Sunday=0)
pub struct DayOfWeekTransform {
    path: String,
}

impl DayOfWeekTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for DayOfWeekTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        let weekday = dt.weekday().num_days_from_sunday();
        Ok(Value::Number(weekday.into()))
    }
}

/// Extract day of year from datetime (1-366)
pub struct DayOfYearTransform {
    path: String,
}

impl DayOfYearTransform {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
        })
    }
}

impl Transform for DayOfYearTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        let extracted = extract_path(&value, &self.path).ok_or_else(|| {
            MirrorMakerError::JsonPathNotFound {
                path: self.path.clone(),
                value: None,
            }
        })?;

        let dt = parse_datetime_value(&extracted, None)?;
        Ok(Value::Number(dt.ordinal().into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_now() {
        let transform = NowTransform::new().unwrap();
        let input = json!({});
        let result = transform.transform(input).unwrap();
        assert!(result.is_number());
        // Should be recent timestamp
        let ts = result.as_i64().unwrap();
        assert!(ts > 1700000000000); // After 2023
    }

    #[test]
    fn test_now_iso() {
        let transform = NowIsoTransform::new().unwrap();
        let input = json!({});
        let result = transform.transform(input).unwrap();
        assert!(result.is_string());
        let s = result.as_str().unwrap();
        assert!(s.contains("T")); // ISO 8601 format
        assert!(s.contains("Z") || s.contains("+")); // Timezone
    }

    #[test]
    fn test_parse_date_iso() {
        let transform = ParseDateTransform::new("/date", None).unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!("2024-03-15T10:30:00+00:00"));
    }

    #[test]
    fn test_from_epoch() {
        let transform = FromEpochTransform::new("/timestamp").unwrap();
        let input = json!({"timestamp": 1710498600000i64}); // 2024-03-15T10:30:00Z
        let result = transform.transform(input).unwrap();
        assert!(result.as_str().unwrap().starts_with("2024-03-15"));
    }

    #[test]
    fn test_from_epoch_seconds() {
        let transform = FromEpochSecondsTransform::new("/timestamp").unwrap();
        let input = json!({"timestamp": 1710498600}); // 2024-03-15T10:30:00Z
        let result = transform.transform(input).unwrap();
        assert!(result.as_str().unwrap().starts_with("2024-03-15"));
    }

    #[test]
    fn test_format_date() {
        let transform = FormatDateTransform::new("/date", "%Y-%m-%d").unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!("2024-03-15"));
    }

    #[test]
    fn test_to_epoch() {
        let transform = ToEpochTransform::new("/date").unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert!(result.is_number());
        let ts = result.as_i64().unwrap();
        assert_eq!(ts, 1710498600000i64);
    }

    #[test]
    fn test_to_epoch_seconds() {
        let transform = ToEpochSecondsTransform::new("/date").unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert!(result.is_number());
        let ts = result.as_i64().unwrap();
        assert_eq!(ts, 1710498600);
    }

    #[test]
    fn test_add_days() {
        let transform = AddDaysTransform::new("/date", 7).unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert!(result.as_str().unwrap().starts_with("2024-03-22"));
    }

    #[test]
    fn test_add_hours() {
        let transform = AddHoursTransform::new("/date", 24).unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert!(result.as_str().unwrap().starts_with("2024-03-16"));
    }

    #[test]
    fn test_subtract_days() {
        let transform = SubtractDaysTransform::new("/date", 1).unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert!(result.as_str().unwrap().starts_with("2024-03-14"));
    }

    #[test]
    fn test_year() {
        let transform = YearTransform::new("/date").unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(2024));
    }

    #[test]
    fn test_month() {
        let transform = MonthTransform::new("/date").unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn test_day() {
        let transform = DayTransform::new("/date").unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(15));
    }

    #[test]
    fn test_hour() {
        let transform = HourTransform::new("/date").unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(10));
    }

    #[test]
    fn test_minute() {
        let transform = MinuteTransform::new("/date").unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(30));
    }

    #[test]
    fn test_day_of_week() {
        let transform = DayOfWeekTransform::new("/date").unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"}); // Friday
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(5)); // Friday = 5 (Sunday = 0)
    }

    #[test]
    fn test_day_of_year() {
        let transform = DayOfYearTransform::new("/date").unwrap();
        let input = json!({"date": "2024-03-15T10:30:00Z"});
        let result = transform.transform(input).unwrap();
        assert_eq!(result, json!(75)); // 75th day of 2024 (leap year)
    }
}
