// DSL Evaluator - Convert validated AST to executable Filter/Transform trait objects

use super::ast::{FilterExpr, Literal, Node};
use crate::error::{MirrorMakerError, Result};
use crate::filter::{
    AndFilter, ArrayFilter, ArrayFilterMode, Filter, HeaderFilter, JsonPathFilter,
    KeyContainsFilter, KeyMatchesFilter, KeyPrefixFilter, KeySuffixFilter, NotFilter, OrFilter,
    RegexFilter, TimestampAgeFilter,
};
use std::sync::Arc;

/// Convert a validated FilterExpr AST node to an executable Filter trait object
pub fn eval_filter(node: &Node<FilterExpr>) -> Result<Arc<dyn Filter>> {
    Ok(Arc::from(eval_filter_as_box(node)?))
}

fn eval_filter_as_box(node: &Node<FilterExpr>) -> Result<Box<dyn Filter>> {
    match &node.value {
        FilterExpr::JsonPath { path, op, value } => {
            let op_str = op.as_str();
            let value_str = literal_to_string(value);
            Ok(Box::new(JsonPathFilter::new(path, op_str, &value_str)?))
        }

        FilterExpr::And(exprs) => {
            let mut filters: Vec<Box<dyn Filter>> = Vec::new();
            for expr in exprs {
                filters.push(eval_filter_as_box(expr)?);
            }
            Ok(Box::new(AndFilter::new(filters)))
        }

        FilterExpr::Or(exprs) => {
            let mut filters: Vec<Box<dyn Filter>> = Vec::new();
            for expr in exprs {
                filters.push(eval_filter_as_box(expr)?);
            }
            Ok(Box::new(OrFilter::new(filters)))
        }

        FilterExpr::Not(expr) => {
            let inner = eval_filter_as_box(expr)?;
            Ok(Box::new(NotFilter::new(inner)))
        }

        FilterExpr::Regex { path, pattern } => {
            Ok(Box::new(RegexFilter::new(path, pattern)?))
        }

        FilterExpr::ArrayAny {
            array_path,
            element_filter,
        } => {
            let element = eval_filter_as_box(element_filter)?;
            Ok(Box::new(ArrayFilter::new(
                array_path,
                element,
                ArrayFilterMode::Any,
            )?))
        }

        FilterExpr::ArrayAll {
            array_path,
            element_filter,
        } => {
            let element = eval_filter_as_box(element_filter)?;
            Ok(Box::new(ArrayFilter::new(
                array_path,
                element,
                ArrayFilterMode::All,
            )?))
        }

        FilterExpr::ArrayContains { array_path, value } => {
            // ArrayContains is implemented as ARRAY_ANY with equality filter
            let value_str = literal_to_string(value);
            let element_filter = Box::new(JsonPathFilter::new("", "==", &value_str)?);
            Ok(Box::new(ArrayFilter::new(
                array_path,
                element_filter,
                ArrayFilterMode::Any,
            )?))
        }

        FilterExpr::ArrayLength {
            array_path,
            op,
            length,
        } => {
            // ArrayLength is implemented as a JSON path filter on the array with length check
            let op_str = op.as_str();
            let length_str = length.to_string();
            let path = format!("{}[#]", array_path); // [#] is the length operator in some JSON path implementations
            Ok(Box::new(JsonPathFilter::new(&path, op_str, &length_str)?))
        }

        FilterExpr::KeyPrefix(prefix) => Ok(Box::new(KeyPrefixFilter::new(prefix))),

        FilterExpr::KeyMatches(pattern) => Ok(Box::new(KeyMatchesFilter::new(pattern)?)),

        FilterExpr::KeySuffix(suffix) => Ok(Box::new(KeySuffixFilter::new(suffix))),

        FilterExpr::KeyContains(substring) => Ok(Box::new(KeyContainsFilter::new(substring))),

        FilterExpr::Header { name, op, value } => {
            let op_str = op.as_str();
            Ok(Box::new(HeaderFilter::new(name, op_str, value)?))
        }

        FilterExpr::TimestampAge { op, seconds } => {
            let op_str = op.as_str();
            Ok(Box::new(TimestampAgeFilter::new(op_str, *seconds as i64)?))
        }

        FilterExpr::Exists(_path) => {
            // EXISTS is not yet implemented in the existing filter system
            // Return a placeholder error for now
            Err(MirrorMakerError::Config(
                "EXISTS filter not yet implemented in evaluator".to_string(),
            ))
        }

        FilterExpr::NotExists(_path) => {
            // NOT_EXISTS is not yet implemented in the existing filter system
            Err(MirrorMakerError::Config(
                "NOT_EXISTS filter not yet implemented in evaluator".to_string(),
            ))
        }
    }
}

fn literal_to_string(literal: &Literal) -> String {
    match literal {
        Literal::String(s) => s.clone(),
        Literal::Number(n) => n.to_string(),
        Literal::Boolean(b) => b.to_string(),
        Literal::Null => "null".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::ast::ComparisonOp;
    use crate::dsl::error::{Position, Span};

    fn test_span() -> Span {
        Span::new(Position::zero(), Position::new(1, 10, 10))
    }

    #[test]
    fn test_eval_simple_filter() {
        let node = Node::new(
            FilterExpr::JsonPath {
                path: "/status".to_string(),
                op: ComparisonOp::Eq,
                value: Literal::String("active".to_string()),
            },
            test_span(),
        );

        let filter = eval_filter(&node);
        assert!(filter.is_ok());
    }

    #[test]
    fn test_eval_and_filter() {
        let filter1 = Node::new(
            FilterExpr::JsonPath {
                path: "/status".to_string(),
                op: ComparisonOp::Eq,
                value: Literal::String("active".to_string()),
            },
            test_span(),
        );

        let filter2 = Node::new(
            FilterExpr::JsonPath {
                path: "/count".to_string(),
                op: ComparisonOp::Gt,
                value: Literal::Number(10.0),
            },
            test_span(),
        );

        let and_node = Node::new(FilterExpr::And(vec![filter1, filter2]), test_span());

        let filter = eval_filter(&and_node);
        assert!(filter.is_ok());
    }

    #[test]
    fn test_eval_regex_filter() {
        let node = Node::new(
            FilterExpr::Regex {
                path: "/email".to_string(),
                pattern: "^[a-z]+@example\\.com$".to_string(),
            },
            test_span(),
        );

        let filter = eval_filter(&node);
        assert!(filter.is_ok());
    }

    #[test]
    fn test_eval_key_prefix_filter() {
        let node = Node::new(FilterExpr::KeyPrefix("user-".to_string()), test_span());

        let filter = eval_filter(&node);
        assert!(filter.is_ok());
    }

    #[test]
    fn test_eval_not_filter() {
        let inner = Node::new(
            FilterExpr::JsonPath {
                path: "/deleted".to_string(),
                op: ComparisonOp::Eq,
                value: Literal::Boolean(true),
            },
            test_span(),
        );

        let not_node = Node::new(FilterExpr::Not(Box::new(inner)), test_span());

        let filter = eval_filter(&not_node);
        assert!(filter.is_ok());
    }
}
