// StreamForge DSL Module
//
// This module provides a structured DSL parser with:
// - AST representation for filters and transforms
// - Position tracking for better error messages
// - Validation before evaluation
// - Evaluation to executable trait objects

pub mod ast;
pub mod error;
pub mod evaluator;
pub mod parser;
pub mod parser_v2;
pub mod validator;

#[cfg(test)]
mod parser_comprehensive_tests;

#[cfg(test)]
mod parser_v2_integration_tests;

// Re-exports for convenience
pub use ast::{
    ArithmeticOp, ArithmeticOperand, ComparisonOp, DslExpr, FilterExpr, HashAlgorithm,
    Literal, Node, StringOp, StringOperand, TransformExpr,
};
pub use error::{ParseError, Position, Span, ValidationError, ValidationWarning};
pub use evaluator::eval_filter;
pub use parser::{parse_filter_expr, parse_transform_expr};
pub use validator::{validate_filter, validate_transform, ValidationResult};
