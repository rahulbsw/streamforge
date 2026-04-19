// DSL-specific error types with position tracking

use std::fmt;

/// Position in the input string (line, column)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl Position {
    pub fn new(line: usize, column: usize, offset: usize) -> Self {
        Self { line, column, offset }
    }

    pub fn zero() -> Self {
        Self { line: 1, column: 1, offset: 0 }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}", self.line, self.column)
    }
}

/// Span in the input (start and end positions)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn from_positions(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start.line == self.end.line {
            write!(f, "line {}, columns {}-{}",
                   self.start.line, self.start.column, self.end.column)
        } else {
            write!(f, "lines {}-{}", self.start.line, self.end.line)
        }
    }
}

/// DSL parse error with position information
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub input: String,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span, input: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span,
            input: input.into(),
        }
    }

    /// Format error with context (shows the problematic line)
    pub fn format_with_context(&self) -> String {
        let lines: Vec<&str> = self.input.lines().collect();
        let mut output = String::new();

        output.push_str(&format!("Error at {}\n", self.span));
        output.push_str(&format!("  {}\n", self.message));

        // Show the line with the error
        if self.span.start.line > 0 && self.span.start.line <= lines.len() {
            let line_idx = self.span.start.line - 1;
            let line = lines[line_idx];

            output.push('\n');
            output.push_str(&format!("{:4} | {}\n", self.span.start.line, line));

            // Add caret indicator
            let indent = " ".repeat(7 + self.span.start.column - 1);
            let underline_len = if self.span.start.line == self.span.end.line {
                (self.span.end.column - self.span.start.column).max(1)
            } else {
                line.len() - self.span.start.column + 1
            };
            let underline = "^".repeat(underline_len);
            output.push_str(&format!("     | {}{}\n", indent, underline));
        }

        output
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_with_context())
    }
}

impl std::error::Error for ParseError {}

/// DSL validation error
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Type mismatch (expected type, actual value, position)
    TypeMismatch {
        expected: String,
        actual: String,
        span: Span,
    },

    /// Invalid JSON path
    InvalidPath {
        path: String,
        reason: String,
        span: Span,
    },

    /// Unknown operator
    UnknownOperator {
        operator: String,
        span: Span,
    },

    /// Invalid argument count
    InvalidArgumentCount {
        operator: String,
        expected: usize,
        actual: usize,
        span: Span,
    },

    /// Undefined variable or function
    Undefined {
        name: String,
        span: Span,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypeMismatch { expected, actual, span } => {
                write!(f, "Type mismatch at {}: expected {}, got {}",
                       span, expected, actual)
            }
            Self::InvalidPath { path, reason, span } => {
                write!(f, "Invalid path '{}' at {}: {}", path, span, reason)
            }
            Self::UnknownOperator { operator, span } => {
                write!(f, "Unknown operator '{}' at {}", operator, span)
            }
            Self::InvalidArgumentCount { operator, expected, actual, span } => {
                write!(f, "Invalid argument count for '{}' at {}: expected {}, got {}",
                       operator, span, expected, actual)
            }
            Self::Undefined { name, span } => {
                write!(f, "Undefined '{}' at {}", name, span)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// DSL warning (non-fatal)
#[derive(Debug, Clone)]
pub enum ValidationWarning {
    /// Deprecated syntax
    DeprecatedSyntax {
        old: String,
        new: String,
        span: Span,
    },

    /// Unused value
    UnusedValue {
        value: String,
        span: Span,
    },

    /// Performance hint
    PerformanceHint {
        message: String,
        span: Span,
    },
}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeprecatedSyntax { old, new, span } => {
                write!(f, "Warning at {}: '{}' is deprecated, use '{}' instead",
                       span, old, new)
            }
            Self::UnusedValue { value, span } => {
                write!(f, "Warning at {}: unused value '{}'", span, value)
            }
            Self::PerformanceHint { message, span } => {
                write!(f, "Performance hint at {}: {}", span, message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_display() {
        let pos = Position::new(5, 10, 50);
        assert_eq!(format!("{}", pos), "line 5, column 10");
    }

    #[test]
    fn test_span_display_same_line() {
        let span = Span::new(
            Position::new(5, 10, 50),
            Position::new(5, 20, 60),
        );
        assert_eq!(format!("{}", span), "line 5, columns 10-20");
    }

    #[test]
    fn test_span_display_different_lines() {
        let span = Span::new(
            Position::new(5, 10, 50),
            Position::new(7, 5, 80),
        );
        assert_eq!(format!("{}", span), "lines 5-7");
    }

    #[test]
    fn test_parse_error_format() {
        let input = "/status,==,active";
        let span = Span::new(
            Position::new(1, 8, 7),
            Position::new(1, 10, 9),
        );
        let error = ParseError::new("Invalid operator", span, input);

        let formatted = error.format_with_context();
        assert!(formatted.contains("Error at"));
        assert!(formatted.contains("Invalid operator"));
        assert!(formatted.contains("/status,==,active"));
        assert!(formatted.contains("^"));
    }
}
