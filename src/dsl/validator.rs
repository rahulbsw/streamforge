// DSL Validator - Semantic validation of parsed AST

use super::ast::{ArithmeticOperand, ComparisonOp, FilterExpr, Literal, Node, TransformExpr};
use super::error::{Span, ValidationError, ValidationWarning};

/// Validation result with warnings
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate a filter expression
pub fn validate_filter(filter: &Node<FilterExpr>) -> ValidationResult {
    let mut result = ValidationResult::new();
    validate_filter_node(filter, &mut result);
    result
}

/// Validate a transform expression
pub fn validate_transform(transform: &Node<TransformExpr>) -> ValidationResult {
    let mut result = ValidationResult::new();
    validate_transform_node(transform, &mut result);
    result
}

fn validate_filter_node(node: &Node<FilterExpr>, result: &mut ValidationResult) {
    match &node.value {
        FilterExpr::JsonPath { path, op, value } => {
            validate_json_path(path, node.span, result);
            validate_comparison(op, value, node.span, result);
        }

        FilterExpr::And(exprs) | FilterExpr::Or(exprs) => {
            if exprs.is_empty() {
                result.add_error(ValidationError::InvalidArgumentCount {
                    operator: if matches!(node.value, FilterExpr::And(_)) {
                        "AND"
                    } else {
                        "OR"
                    }
                    .to_string(),
                    expected: 1,
                    actual: 0,
                    span: node.span,
                });
            }
            for expr in exprs {
                validate_filter_node(expr, result);
            }
        }

        FilterExpr::Not(expr) => {
            validate_filter_node(expr, result);
        }

        FilterExpr::Regex { path, pattern } => {
            validate_json_path(path, node.span, result);
            validate_regex_pattern(pattern, node.span, result);
        }

        FilterExpr::ArrayAny {
            array_path,
            element_filter,
        }
        | FilterExpr::ArrayAll {
            array_path,
            element_filter,
        } => {
            validate_json_path(array_path, node.span, result);
            validate_filter_node(element_filter, result);
        }

        FilterExpr::ArrayContains { array_path, value } => {
            validate_json_path(array_path, node.span, result);
            validate_literal(value, node.span, result);
        }

        FilterExpr::ArrayLength {
            array_path,
            op,
            length: _,
        } => {
            validate_json_path(array_path, node.span, result);
            validate_numeric_comparison(op, node.span, result);
        }

        FilterExpr::KeyPrefix(_) | FilterExpr::KeyMatches(_) => {
            // Always valid
        }

        FilterExpr::KeySuffix(suffix) => {
            result.add_warning(ValidationWarning::DeprecatedSyntax {
                old: format!("KEY_SUFFIX:{}", suffix),
                new: format!("KEY_MATCHES:.*{}$", regex::escape(suffix)),
                span: node.span,
            });
        }

        FilterExpr::KeyContains(substring) => {
            result.add_warning(ValidationWarning::DeprecatedSyntax {
                old: format!("KEY_CONTAINS:{}", substring),
                new: format!("KEY_MATCHES:.*{}.*", regex::escape(substring)),
                span: node.span,
            });
        }

        FilterExpr::Header { name, op, value: _ } => {
            if name.is_empty() {
                result.add_error(ValidationError::InvalidPath {
                    path: name.clone(),
                    reason: "Header name cannot be empty".to_string(),
                    span: node.span,
                });
            }
            validate_string_comparison(op, node.span, result);
        }

        FilterExpr::TimestampAge { op, seconds: _ } => {
            validate_numeric_comparison(op, node.span, result);
        }

        FilterExpr::Exists(path) | FilterExpr::NotExists(path) => {
            validate_json_path(path, node.span, result);
        }

        FilterExpr::IsNull(path)
        | FilterExpr::IsNotNull(path)
        | FilterExpr::IsEmpty(path)
        | FilterExpr::IsNotEmpty(path)
        | FilterExpr::IsBlank(path) => {
            validate_json_path(path, node.span, result);
        }

        FilterExpr::StartsWith { path, prefix: _ } => {
            validate_json_path(path, node.span, result);
        }

        FilterExpr::EndsWith { path, suffix: _ } => {
            validate_json_path(path, node.span, result);
        }

        FilterExpr::Contains { path, substring: _ } => {
            validate_json_path(path, node.span, result);
        }

        FilterExpr::StringLength {
            path,
            op,
            length: _,
        } => {
            validate_json_path(path, node.span, result);
            validate_numeric_comparison(op, node.span, result);
        }
    }
}

fn validate_transform_node(node: &Node<TransformExpr>, result: &mut ValidationResult) {
    match &node.value {
        TransformExpr::JsonPath(path) => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::Extract {
            path,
            target_field,
            default_value: _,
        } => {
            validate_json_path(path, node.span, result);
            validate_field_name(target_field, node.span, result);
        }

        TransformExpr::Construct(fields) => {
            if fields.is_empty() {
                result.add_error(ValidationError::InvalidArgumentCount {
                    operator: "CONSTRUCT".to_string(),
                    expected: 1,
                    actual: 0,
                    span: node.span,
                });
            }

            for (field_name, json_path) in fields {
                validate_field_name(field_name, node.span, result);
                validate_json_path(json_path, node.span, result);
            }

            // Check for duplicate field names
            let mut seen = std::collections::HashSet::new();
            for (field_name, _) in fields {
                if !seen.insert(field_name) {
                    result.add_warning(ValidationWarning::UnusedValue {
                        value: format!("Duplicate field name '{}'", field_name),
                        span: node.span,
                    });
                }
            }
        }

        TransformExpr::Hash {
            algorithm: _,
            path,
            target_field,
        } => {
            validate_json_path(path, node.span, result);
            validate_field_name(target_field, node.span, result);
        }

        TransformExpr::String { op: _, path } => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::ArrayMap {
            array_path,
            element_path,
            target_field,
        } => {
            validate_json_path(array_path, node.span, result);
            validate_json_path(element_path, node.span, result);
            validate_field_name(target_field, node.span, result);
        }

        TransformExpr::ArrayFilter { array_path, filter } => {
            validate_json_path(array_path, node.span, result);
            validate_filter_node(filter, result);
        }

        TransformExpr::Arithmetic { op: _, left, right } => {
            validate_arithmetic_operand(left, node.span, result);
            validate_arithmetic_operand(right, node.span, result);
        }

        TransformExpr::Coalesce { paths, default: _ } => {
            if paths.is_empty() {
                result.add_error(ValidationError::InvalidArgumentCount {
                    operator: "COALESCE".to_string(),
                    expected: 1,
                    actual: 0,
                    span: node.span,
                });
            }

            for path in paths {
                validate_json_path(path, node.span, result);
            }
        }

        // String operations (new in v2)
        TransformExpr::StringLength(path) => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::Substring {
            path,
            start: _,
            end: _,
        } => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::Split { path, delimiter: _ } => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::Join { path, separator: _ } => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::Concat(operands) => {
            for operand in operands {
                if let super::ast::StringOperand::Path(path) = operand {
                    validate_json_path(path, node.span, result);
                }
            }
        }

        TransformExpr::Replace {
            path,
            pattern: _,
            replacement: _,
        } => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::PadLeft {
            path,
            width: _,
            pad_char: _,
        } => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::PadRight {
            path,
            width: _,
            pad_char: _,
        } => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::ToString(path)
        | TransformExpr::ToInt(path)
        | TransformExpr::ToFloat(path) => {
            validate_json_path(path, node.span, result);
        }

        // Date/time operations (new in v2)
        TransformExpr::Now | TransformExpr::NowIso => {
            // No validation needed
        }

        TransformExpr::ParseDate { path, format: _ } => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::FromEpoch(path) | TransformExpr::FromEpochSeconds(path) => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::FormatDate { path, format: _ } => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::ToEpoch(path)
        | TransformExpr::ToEpochSeconds(path)
        | TransformExpr::ToIso(path) => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::AddDays { path, days: _ }
        | TransformExpr::AddHours { path, hours: _ }
        | TransformExpr::AddMinutes { path, minutes: _ }
        | TransformExpr::SubtractDays { path, days: _ } => {
            validate_json_path(path, node.span, result);
        }

        TransformExpr::Year(path)
        | TransformExpr::Month(path)
        | TransformExpr::Day(path)
        | TransformExpr::Hour(path)
        | TransformExpr::Minute(path)
        | TransformExpr::Second(path)
        | TransformExpr::DayOfWeek(path)
        | TransformExpr::DayOfYear(path) => {
            validate_json_path(path, node.span, result);
        }
    }
}

// Validation helper functions

fn validate_json_path(path: &str, span: Span, result: &mut ValidationResult) {
    if path.is_empty() {
        result.add_error(ValidationError::InvalidPath {
            path: path.to_string(),
            reason: "Path cannot be empty".to_string(),
            span,
        });
        return;
    }

    if !path.starts_with('/') {
        result.add_error(ValidationError::InvalidPath {
            path: path.to_string(),
            reason: "Path must start with '/'".to_string(),
            span,
        });
    }

    // Check for invalid characters
    if path.contains("//") {
        result.add_error(ValidationError::InvalidPath {
            path: path.to_string(),
            reason: "Path cannot contain empty segments (//)".to_string(),
            span,
        });
    }
}

fn validate_field_name(name: &str, span: Span, result: &mut ValidationResult) {
    if name.is_empty() {
        result.add_error(ValidationError::InvalidPath {
            path: name.to_string(),
            reason: "Field name cannot be empty".to_string(),
            span,
        });
        return;
    }

    // Check for invalid characters in field names
    if name.contains('/') || name.contains(':') || name.contains(',') {
        result.add_error(ValidationError::InvalidPath {
            path: name.to_string(),
            reason: "Field name cannot contain '/', ':', or ','".to_string(),
            span,
        });
    }
}

fn validate_comparison(
    op: &ComparisonOp,
    value: &Literal,
    span: Span,
    result: &mut ValidationResult,
) {
    // Type checking for operators
    match (op, value) {
        (
            ComparisonOp::Gt | ComparisonOp::Ge | ComparisonOp::Lt | ComparisonOp::Le,
            Literal::Number(_),
        ) => {
            // Valid: numeric comparison with number
        }
        (ComparisonOp::Gt | ComparisonOp::Ge | ComparisonOp::Lt | ComparisonOp::Le, _) => {
            result.add_warning(ValidationWarning::PerformanceHint {
                message: format!(
                    "Operator '{}' typically used with numbers, but got {}",
                    op.as_str(),
                    literal_type_name(value)
                ),
                span,
            });
        }
        (ComparisonOp::Eq | ComparisonOp::Ne, _) => {
            // Equality works with all types
        }
    }
}

fn validate_numeric_comparison(op: &ComparisonOp, span: Span, result: &mut ValidationResult) {
    // For operations that only make sense with numbers
    match op {
        ComparisonOp::Eq | ComparisonOp::Ne => {
            result.add_warning(ValidationWarning::PerformanceHint {
                message: format!(
                    "Consider using '>' or '<' for numeric comparisons instead of '{}'",
                    op.as_str()
                ),
                span,
            });
        }
        _ => {} // Gt, Ge, Lt, Le are fine
    }
}

fn validate_string_comparison(op: &ComparisonOp, span: Span, result: &mut ValidationResult) {
    match op {
        ComparisonOp::Gt | ComparisonOp::Ge | ComparisonOp::Lt | ComparisonOp::Le => {
            result.add_warning(ValidationWarning::PerformanceHint {
                message: format!(
                    "Operator '{}' performs lexicographic comparison for strings",
                    op.as_str()
                ),
                span,
            });
        }
        _ => {} // Eq, Ne are fine
    }
}

fn validate_regex_pattern(pattern: &str, span: Span, result: &mut ValidationResult) {
    if pattern.is_empty() {
        result.add_error(ValidationError::InvalidPath {
            path: pattern.to_string(),
            reason: "Regex pattern cannot be empty".to_string(),
            span,
        });
        return;
    }

    // Try to compile the regex to validate syntax
    if let Err(e) = regex::Regex::new(pattern) {
        result.add_error(ValidationError::InvalidPath {
            path: pattern.to_string(),
            reason: format!("Invalid regex pattern: {}", e),
            span,
        });
    }
}

fn validate_literal(_value: &Literal, _span: Span, _result: &mut ValidationResult) {
    // All literals are valid by construction
}

fn validate_arithmetic_operand(
    operand: &ArithmeticOperand,
    span: Span,
    result: &mut ValidationResult,
) {
    match operand {
        ArithmeticOperand::Path(path) => {
            validate_json_path(path, span, result);
        }
        ArithmeticOperand::Constant(_) => {
            // Constants are always valid
        }
    }
}

fn literal_type_name(literal: &Literal) -> &'static str {
    match literal {
        Literal::String(_) => "string",
        Literal::Number(_) => "number",
        Literal::Boolean(_) => "boolean",
        Literal::Null => "null",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::error::Position;

    fn test_span() -> Span {
        Span::new(Position::zero(), Position::new(1, 10, 10))
    }

    #[test]
    fn test_valid_simple_filter() {
        let filter = Node::new(
            FilterExpr::JsonPath {
                path: "/status".to_string(),
                op: ComparisonOp::Eq,
                value: Literal::String("active".to_string()),
            },
            test_span(),
        );

        let result = validate_filter(&filter);
        assert!(result.is_valid());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_invalid_json_path() {
        let filter = Node::new(
            FilterExpr::JsonPath {
                path: "invalid".to_string(), // Missing leading /
                op: ComparisonOp::Eq,
                value: Literal::String("test".to_string()),
            },
            test_span(),
        );

        let result = validate_filter(&filter);
        assert!(!result.is_valid());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            result.errors[0],
            ValidationError::InvalidPath { .. }
        ));
    }

    #[test]
    fn test_empty_and_filter() {
        let filter = Node::new(FilterExpr::And(vec![]), test_span());

        let result = validate_filter(&filter);
        assert!(!result.is_valid());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            result.errors[0],
            ValidationError::InvalidArgumentCount { .. }
        ));
    }

    #[test]
    fn test_deprecated_key_suffix() {
        let filter = Node::new(FilterExpr::KeySuffix("test".to_string()), test_span());

        let result = validate_filter(&filter);
        assert!(result.is_valid()); // Valid but with warning
        assert_eq!(result.warnings.len(), 1);
        assert!(matches!(
            result.warnings[0],
            ValidationWarning::DeprecatedSyntax { .. }
        ));
    }

    #[test]
    fn test_deprecated_key_contains() {
        let filter = Node::new(
            FilterExpr::KeyContains("substring".to_string()),
            test_span(),
        );

        let result = validate_filter(&filter);
        assert!(result.is_valid());
        assert_eq!(result.warnings.len(), 1);
        assert!(matches!(
            result.warnings[0],
            ValidationWarning::DeprecatedSyntax { .. }
        ));
    }

    #[test]
    fn test_invalid_regex() {
        let filter = Node::new(
            FilterExpr::Regex {
                path: "/field".to_string(),
                pattern: "[unclosed".to_string(), // Invalid regex
            },
            test_span(),
        );

        let result = validate_filter(&filter);
        assert!(!result.is_valid());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            result.errors[0],
            ValidationError::InvalidPath { .. }
        ));
    }

    #[test]
    fn test_valid_construct() {
        let transform = Node::new(
            TransformExpr::Construct(vec![
                ("id".to_string(), "/user/id".to_string()),
                ("name".to_string(), "/user/name".to_string()),
            ]),
            test_span(),
        );

        let result = validate_transform(&transform);
        assert!(result.is_valid());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_empty_construct() {
        let transform = Node::new(TransformExpr::Construct(vec![]), test_span());

        let result = validate_transform(&transform);
        assert!(!result.is_valid());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            result.errors[0],
            ValidationError::InvalidArgumentCount { .. }
        ));
    }

    #[test]
    fn test_duplicate_construct_fields() {
        let transform = Node::new(
            TransformExpr::Construct(vec![
                ("id".to_string(), "/user/id".to_string()),
                ("id".to_string(), "/account/id".to_string()), // Duplicate field
            ]),
            test_span(),
        );

        let result = validate_transform(&transform);
        assert!(result.is_valid()); // Valid but with warning
        assert_eq!(result.warnings.len(), 1);
        assert!(matches!(
            result.warnings[0],
            ValidationWarning::UnusedValue { .. }
        ));
    }

    #[test]
    fn test_invalid_field_name() {
        let transform = Node::new(
            TransformExpr::Extract {
                path: "/data".to_string(),
                target_field: "invalid/field".to_string(), // Contains /
                default_value: None,
            },
            test_span(),
        );

        let result = validate_transform(&transform);
        assert!(!result.is_valid());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            result.errors[0],
            ValidationError::InvalidPath { .. }
        ));
    }

    #[test]
    fn test_empty_coalesce() {
        let transform = Node::new(
            TransformExpr::Coalesce {
                paths: vec![],
                default: None,
            },
            test_span(),
        );

        let result = validate_transform(&transform);
        assert!(!result.is_valid());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            result.errors[0],
            ValidationError::InvalidArgumentCount { .. }
        ));
    }

    #[test]
    fn test_nested_and_or() {
        let inner_filter1 = Node::new(
            FilterExpr::JsonPath {
                path: "/status".to_string(),
                op: ComparisonOp::Eq,
                value: Literal::String("active".to_string()),
            },
            test_span(),
        );

        let inner_filter2 = Node::new(
            FilterExpr::JsonPath {
                path: "/count".to_string(),
                op: ComparisonOp::Gt,
                value: Literal::Number(10.0),
            },
            test_span(),
        );

        let or_filter = Node::new(
            FilterExpr::Or(vec![inner_filter1, inner_filter2]),
            test_span(),
        );

        let and_filter = Node::new(
            FilterExpr::And(vec![or_filter.clone(), or_filter]),
            test_span(),
        );

        let result = validate_filter(&and_filter);
        assert!(result.is_valid());
        assert!(result.errors.is_empty());
    }
}
