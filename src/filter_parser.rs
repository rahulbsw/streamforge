use crate::error::{MirrorMakerError, Result};
use crate::filter::{
    AndFilter, ArithmeticOp, ArithmeticTransform, ArrayFilter, ArrayFilterMode, ArrayMapTransform,
    Filter, JsonPathFilter, JsonPathTransform, NotFilter, ObjectConstructTransform, OrFilter,
    RegexFilter, Transform,
};
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
pub fn parse_filter(expr: &str) -> Result<Arc<dyn Filter>> {
    let parts: Vec<&str> = expr.split(':').collect();

    if parts.is_empty() {
        return Err(MirrorMakerError::Config("Empty filter expression".to_string()));
    }

    match parts[0] {
        "AND" => Ok(Arc::from(parse_and_filter(&parts[1..])?)),
        "OR" => Ok(Arc::from(parse_or_filter(&parts[1..])?)),
        "NOT" => Ok(Arc::from(parse_not_filter(&parts[1..])?)),
        "REGEX" => Ok(Arc::from(parse_regex_filter(&parts[1..])?)),
        "ARRAY_ALL" => Ok(Arc::from(parse_array_filter(&parts[1..], ArrayFilterMode::All)?)),
        "ARRAY_ANY" => Ok(Arc::from(parse_array_filter(&parts[1..], ArrayFilterMode::Any)?)),
        _ => Ok(Arc::from(parse_simple_filter(expr)?)),
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
        return Err(MirrorMakerError::Config("AND filter requires at least one condition".to_string()));
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
            let or_filter = parse_or_filter(&conditions[i+1..or_end])?;
            filters.push(or_filter);
            i = or_end;
        } else if conditions[i] == "NOT" {
            if i + 1 >= conditions.len() {
                return Err(MirrorMakerError::Config("NOT requires a condition".to_string()));
            }
            let simple = parse_simple_filter(conditions[i+1])?;
            filters.push(Box::new(NotFilter::new(simple)));
            i += 2;
        } else {
            // Simple condition
            let simple = parse_simple_filter(conditions[i])?;
            filters.push(simple);
            i += 1;
        }
    }

    Ok(Box::new(AndFilter::new(filters)))
}

fn parse_or_filter(conditions: &[&str]) -> Result<Box<dyn Filter>> {
    if conditions.is_empty() {
        return Err(MirrorMakerError::Config("OR filter requires at least one condition".to_string()));
    }

    let mut filters: Vec<Box<dyn Filter>> = Vec::new();

    for cond in conditions {
        let simple = parse_simple_filter(cond)?;
        filters.push(simple);
    }

    Ok(Box::new(OrFilter::new(filters)))
}

fn parse_not_filter(conditions: &[&str]) -> Result<Box<dyn Filter>> {
    if conditions.len() != 1 {
        return Err(MirrorMakerError::Config("NOT filter requires exactly one condition".to_string()));
    }

    let simple = parse_simple_filter(conditions[0])?;
    Ok(Box::new(NotFilter::new(simple)))
}

fn parse_regex_filter(parts: &[&str]) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config("REGEX filter requires path and pattern".to_string()));
    }

    let combined = parts.join(":");
    let filter_parts: Vec<&str> = combined.split(',').collect();

    if filter_parts.len() != 2 {
        return Err(MirrorMakerError::Config(format!(
            "Invalid REGEX format: {}. Expected 'REGEX:/path,pattern'",
            combined
        )));
    }

    Ok(Box::new(RegexFilter::new(filter_parts[0], filter_parts[1])?))
}

fn parse_array_filter(parts: &[&str], mode: ArrayFilterMode) -> Result<Box<dyn Filter>> {
    if parts.is_empty() {
        return Err(MirrorMakerError::Config("ARRAY filter requires path and element filter".to_string()));
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
pub fn parse_transform(expr: &str) -> Result<Arc<dyn Transform>> {
    if expr.starts_with("CONSTRUCT:") {
        parse_construct_transform(&expr[10..])
    } else if expr.starts_with("ARRAY_MAP:") {
        parse_array_map_transform(&expr[10..])
    } else if expr.starts_with("ARITHMETIC:") {
        parse_arithmetic_transform(&expr[11..])
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
        _ => return Err(MirrorMakerError::Config(format!(
            "Unknown arithmetic operation: {}. Expected ADD, SUB, MUL, or DIV",
            parts[0]
        ))),
    };

    let left_path = parts[1];

    // Check if right operand is a constant or path
    if let Ok(constant) = parts[2].parse::<f64>() {
        Ok(Arc::new(ArithmeticTransform::new_with_constant(op, left_path, constant)?))
    } else {
        Ok(Arc::new(ArithmeticTransform::new_with_paths(op, left_path, parts[2])?))
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
        let transform = parse_transform("CONSTRUCT:id=/message/confId:site=/message/siteId").unwrap();

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
}
