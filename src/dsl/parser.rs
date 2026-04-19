// DSL Parser - converts string input to AST

use super::ast::*;
use super::error::{ParseError, Position, Span};

/// Parser state
struct Parser<'a> {
    input: &'a str,
    position: usize,
    line: usize,
    column: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            position: 0,
            line: 1,
            column: 1,
        }
    }

    fn current_position(&self) -> Position {
        Position::new(self.line, self.column, self.position)
    }

    fn peek(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        if let Some(ch) = self.peek() {
            self.position += ch.len_utf8();
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            Some(ch)
        } else {
            None
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn peek_str(&self, s: &str) -> bool {
        self.input[self.position..].starts_with(s)
    }

    fn consume_str(&mut self, s: &str) -> bool {
        if self.peek_str(s) {
            for _ in 0..s.len() {
                self.advance();
            }
            true
        } else {
            false
        }
    }

    fn read_until(&mut self, delimiter: char) -> String {
        let mut result = String::new();
        while let Some(ch) = self.peek() {
            if ch == delimiter {
                break;
            }
            result.push(ch);
            self.advance();
        }
        result
    }

    fn read_until_any(&mut self, delimiters: &[char]) -> String {
        let mut result = String::new();
        while let Some(ch) = self.peek() {
            if delimiters.contains(&ch) {
                break;
            }
            result.push(ch);
            self.advance();
        }
        result
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        let pos = self.current_position();
        ParseError::new(
            message,
            Span::new(pos, pos),
            self.input,
        )
    }
}

/// Parse a filter expression
pub fn parse_filter_expr(input: &str) -> Result<Node<FilterExpr>, ParseError> {
    // Auto-detect syntax version
    let trimmed = input.trim();

    // Check for function-style syntax (v2)
    // Function-style starts with: and(, or(, not(, field(, exists(, is_null(, $(, $identifier, etc.
    if trimmed.starts_with("and(") ||
       trimmed.starts_with("or(") ||
       trimmed.starts_with("not(") ||
       trimmed.starts_with("field(") ||
       trimmed.starts_with("exists(") ||
       trimmed.starts_with("not_exists(") ||
       trimmed.starts_with("is_null(") ||
       trimmed.starts_with("is_not_null(") ||
       trimmed.starts_with("is_empty(") ||
       trimmed.starts_with("is_not_empty(") ||
       trimmed.starts_with("is_blank(") ||
       trimmed.starts_with("regex(") ||
       trimmed.starts_with("$(") ||
       (trimmed.starts_with('$') && trimmed.len() > 1 && trimmed.chars().nth(1).unwrap().is_alphabetic()) {
        // Use v2 parser (function-style)
        return super::parser_v2::parse_filter_expr_v2(input);
    }

    // Otherwise use v1 parser (colon-delimited)
    let mut parser = Parser::new(input);
    parse_filter_expr_inner(&mut parser)
}

fn parse_filter_expr_inner(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.skip_whitespace();

    // Check for composite filters (AND, OR, NOT)
    if parser.peek_str("AND:") {
        return parse_and_filter(parser);
    }
    if parser.peek_str("OR:") {
        return parse_or_filter(parser);
    }
    if parser.peek_str("NOT:") {
        return parse_not_filter(parser);
    }

    // Check for special filters
    if parser.peek_str("REGEX:") {
        return parse_regex_filter(parser);
    }
    if parser.peek_str("ARRAY_ANY:") {
        return parse_array_any_filter(parser);
    }
    if parser.peek_str("ARRAY_ALL:") {
        return parse_array_all_filter(parser);
    }
    if parser.peek_str("ARRAY_CONTAINS:") {
        return parse_array_contains_filter(parser);
    }
    if parser.peek_str("ARRAY_LENGTH:") {
        return parse_array_length_filter(parser);
    }
    if parser.peek_str("KEY_PREFIX:") {
        return parse_key_prefix_filter(parser);
    }
    if parser.peek_str("KEY_MATCHES:") {
        return parse_key_matches_filter(parser);
    }
    if parser.peek_str("KEY_SUFFIX:") {
        return parse_key_suffix_filter(parser);
    }
    if parser.peek_str("KEY_CONTAINS:") {
        return parse_key_contains_filter(parser);
    }
    if parser.peek_str("HEADER:") {
        return parse_header_filter(parser);
    }
    if parser.peek_str("TIMESTAMP_AGE:") {
        return parse_timestamp_age_filter(parser);
    }
    if parser.peek_str("EXISTS:") {
        return parse_exists_filter(parser);
    }
    if parser.peek_str("NOT_EXISTS:") {
        return parse_not_exists_filter(parser);
    }

    // Default: JSON path comparison
    parse_json_path_filter(parser, start)
}

fn parse_and_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("AND:");

    let mut exprs = Vec::new();

    loop {
        let expr_str = parser.read_until(':');
        if expr_str.is_empty() {
            break;
        }

        let mut sub_parser = Parser::new(&expr_str);
        let expr = parse_filter_expr_inner(&mut sub_parser)?;
        exprs.push(expr);

        if parser.peek() == Some(':') {
            parser.advance();
        } else {
            break;
        }
    }

    if exprs.is_empty() {
        return Err(parser.error("AND filter requires at least one condition"));
    }

    let end = parser.current_position();
    Ok(Node::new(FilterExpr::And(exprs), Span::new(start, end)))
}

fn parse_or_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("OR:");

    let mut exprs = Vec::new();

    loop {
        let expr_str = parser.read_until(':');
        if expr_str.is_empty() {
            break;
        }

        let mut sub_parser = Parser::new(&expr_str);
        let expr = parse_filter_expr_inner(&mut sub_parser)?;
        exprs.push(expr);

        if parser.peek() == Some(':') {
            parser.advance();
        } else {
            break;
        }
    }

    if exprs.is_empty() {
        return Err(parser.error("OR filter requires at least one condition"));
    }

    let end = parser.current_position();
    Ok(Node::new(FilterExpr::Or(exprs), Span::new(start, end)))
}

fn parse_not_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("NOT:");

    let expr_str = parser.read_until_any(&[]);
    let mut sub_parser = Parser::new(&expr_str);
    let expr = parse_filter_expr_inner(&mut sub_parser)?;

    let end = parser.current_position();
    Ok(Node::new(FilterExpr::Not(Box::new(expr)), Span::new(start, end)))
}

fn parse_regex_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("REGEX:");

    let parts: Vec<&str> = parser.input[parser.position..].split(',').collect();
    if parts.len() < 2 {
        return Err(parser.error("REGEX filter requires path and pattern"));
    }

    let path = parts[0].trim().to_string();
    let pattern = parts[1].trim().to_string();

    // Advance parser position
    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        FilterExpr::Regex { path, pattern },
        Span::new(start, end),
    ))
}

fn parse_array_any_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("ARRAY_ANY:");

    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.splitn(2, ',').collect();
    if parts.len() < 2 {
        return Err(parser.error("ARRAY_ANY requires array path and element filter"));
    }

    let array_path = parts[0].trim().to_string();
    let element_filter_str = parts[1].trim();

    let mut sub_parser = Parser::new(element_filter_str);
    let element_filter = parse_filter_expr_inner(&mut sub_parser)?;

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        FilterExpr::ArrayAny {
            array_path,
            element_filter: Box::new(element_filter),
        },
        Span::new(start, end),
    ))
}

fn parse_array_all_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("ARRAY_ALL:");

    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.splitn(2, ',').collect();
    if parts.len() < 2 {
        return Err(parser.error("ARRAY_ALL requires array path and element filter"));
    }

    let array_path = parts[0].trim().to_string();
    let element_filter_str = parts[1].trim();

    let mut sub_parser = Parser::new(element_filter_str);
    let element_filter = parse_filter_expr_inner(&mut sub_parser)?;

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        FilterExpr::ArrayAll {
            array_path,
            element_filter: Box::new(element_filter),
        },
        Span::new(start, end),
    ))
}

fn parse_array_contains_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("ARRAY_CONTAINS:");

    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.splitn(2, ',').collect();
    if parts.len() < 2 {
        return Err(parser.error("ARRAY_CONTAINS requires array path and value"));
    }

    let array_path = parts[0].trim().to_string();
    let value = parse_literal(parts[1].trim())?;

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        FilterExpr::ArrayContains { array_path, value },
        Span::new(start, end),
    ))
}

fn parse_array_length_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("ARRAY_LENGTH:");

    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.split(',').collect();
    if parts.len() < 3 {
        return Err(parser.error("ARRAY_LENGTH requires array path, operator, and length"));
    }

    let array_path = parts[0].trim().to_string();
    let op = ComparisonOp::from_str(parts[1].trim())
        .ok_or_else(|| parser.error(format!("Invalid operator: {}", parts[1])))?;
    let length = parts[2].trim().parse::<usize>()
        .map_err(|_| parser.error(format!("Invalid length: {}", parts[2])))?;

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        FilterExpr::ArrayLength { array_path, op, length },
        Span::new(start, end),
    ))
}

fn parse_key_prefix_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("KEY_PREFIX:");

    let prefix = parser.read_until_any(&[]).trim().to_string();
    let end = parser.current_position();

    Ok(Node::new(FilterExpr::KeyPrefix(prefix), Span::new(start, end)))
}

fn parse_key_matches_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("KEY_MATCHES:");

    let pattern = parser.read_until_any(&[]).trim().to_string();
    let end = parser.current_position();

    Ok(Node::new(FilterExpr::KeyMatches(pattern), Span::new(start, end)))
}

fn parse_key_suffix_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("KEY_SUFFIX:");

    let suffix = parser.read_until_any(&[]).trim().to_string();
    let end = parser.current_position();

    Ok(Node::new(FilterExpr::KeySuffix(suffix), Span::new(start, end)))
}

fn parse_key_contains_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("KEY_CONTAINS:");

    let substring = parser.read_until_any(&[]).trim().to_string();
    let end = parser.current_position();

    Ok(Node::new(FilterExpr::KeyContains(substring), Span::new(start, end)))
}

fn parse_header_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("HEADER:");

    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.split(',').collect();
    if parts.len() < 3 {
        return Err(parser.error("HEADER requires name, operator, and value"));
    }

    let name = parts[0].trim().to_string();
    let op = ComparisonOp::from_str(parts[1].trim())
        .ok_or_else(|| parser.error(format!("Invalid operator: {}", parts[1])))?;
    let value = parts[2].trim().to_string();

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        FilterExpr::Header { name, op, value },
        Span::new(start, end),
    ))
}

fn parse_timestamp_age_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("TIMESTAMP_AGE:");

    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.split(',').collect();
    if parts.len() < 2 {
        return Err(parser.error("TIMESTAMP_AGE requires operator and seconds"));
    }

    let op = ComparisonOp::from_str(parts[0].trim())
        .ok_or_else(|| parser.error(format!("Invalid operator: {}", parts[0])))?;
    let seconds = parts[1].trim().parse::<u64>()
        .map_err(|_| parser.error(format!("Invalid seconds: {}", parts[1])))?;

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        FilterExpr::TimestampAge { op, seconds },
        Span::new(start, end),
    ))
}

fn parse_exists_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("EXISTS:");

    let path = parser.read_until_any(&[]).trim().to_string();
    let end = parser.current_position();

    Ok(Node::new(FilterExpr::Exists(path), Span::new(start, end)))
}

fn parse_not_exists_filter(parser: &mut Parser) -> Result<Node<FilterExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("NOT_EXISTS:");

    let path = parser.read_until_any(&[]).trim().to_string();
    let end = parser.current_position();

    Ok(Node::new(FilterExpr::NotExists(path), Span::new(start, end)))
}

fn parse_json_path_filter(parser: &mut Parser, start: Position) -> Result<Node<FilterExpr>, ParseError> {
    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.split(',').collect();

    if parts.len() < 3 {
        return Err(parser.error("JSON path filter requires path, operator, and value"));
    }

    let path = parts[0].trim().to_string();
    let op = ComparisonOp::from_str(parts[1].trim())
        .ok_or_else(|| parser.error(format!("Invalid operator: {}", parts[1])))?;
    let value = parse_literal(parts[2].trim())?;

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        FilterExpr::JsonPath { path, op, value },
        Span::new(start, end),
    ))
}

fn parse_literal(s: &str) -> Result<Literal, ParseError> {
    let s = s.trim();

    // Try boolean
    if s == "true" {
        return Ok(Literal::Boolean(true));
    }
    if s == "false" {
        return Ok(Literal::Boolean(false));
    }
    if s == "null" {
        return Ok(Literal::Null);
    }

    // Try number
    if let Ok(n) = s.parse::<f64>() {
        return Ok(Literal::Number(n));
    }

    // Default to string
    Ok(Literal::String(s.to_string()))
}

/// Parse a transform expression
pub fn parse_transform_expr(input: &str) -> Result<Node<TransformExpr>, ParseError> {
    let mut parser = Parser::new(input);
    parse_transform_expr_inner(&mut parser)
}

fn parse_transform_expr_inner(parser: &mut Parser) -> Result<Node<TransformExpr>, ParseError> {
    let start = parser.current_position();
    parser.skip_whitespace();

    // Check for special transforms
    if parser.peek_str("EXTRACT:") {
        return parse_extract_transform(parser);
    }
    if parser.peek_str("CONSTRUCT:") {
        return parse_construct_transform(parser);
    }
    if parser.peek_str("HASH:") {
        return parse_hash_transform(parser);
    }
    if parser.peek_str("UPPERCASE:") || parser.peek_str("LOWERCASE:") || parser.peek_str("TRIM:") {
        return parse_string_transform(parser);
    }
    if parser.peek_str("ARRAY_MAP:") {
        return parse_array_map_transform(parser);
    }
    if parser.peek_str("ARRAY_FILTER:") {
        return parse_array_filter_transform(parser);
    }
    if parser.peek_str("ADD:") || parser.peek_str("SUB:") || parser.peek_str("MULTIPLY:") ||
       parser.peek_str("DIVIDE:") || parser.peek_str("ARITHMETIC:") {
        return parse_arithmetic_transform(parser);
    }
    if parser.peek_str("COALESCE:") {
        return parse_coalesce_transform(parser);
    }

    // Default: JSON path
    let path = parser.read_until_any(&[]).trim().to_string();
    let end = parser.current_position();

    Ok(Node::new(
        TransformExpr::JsonPath(path),
        Span::new(start, end),
    ))
}

fn parse_extract_transform(parser: &mut Parser) -> Result<Node<TransformExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("EXTRACT:");

    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.split(',').collect();

    if parts.is_empty() {
        return Err(parser.error("EXTRACT requires at least a path"));
    }

    let path = parts[0].trim().to_string();
    let target_field = if parts.len() > 1 {
        parts[1].trim().to_string()
    } else {
        path.trim_start_matches('/').replace('/', "_")
    };
    let default_value = if parts.len() > 2 {
        Some(parts[2].trim().to_string())
    } else {
        None
    };

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        TransformExpr::Extract { path, target_field, default_value },
        Span::new(start, end),
    ))
}

fn parse_construct_transform(parser: &mut Parser) -> Result<Node<TransformExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("CONSTRUCT:");

    let rest = &parser.input[parser.position..];
    let mut fields = Vec::new();

    for part in rest.split(':') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let kv: Vec<&str> = part.splitn(2, '=').collect();
        if kv.len() != 2 {
            return Err(parser.error(format!("Invalid CONSTRUCT field: {}", part)));
        }

        let field_name = kv[0].trim().to_string();
        let json_path = kv[1].trim().to_string();
        fields.push((field_name, json_path));
    }

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        TransformExpr::Construct(fields),
        Span::new(start, end),
    ))
}

fn parse_hash_transform(parser: &mut Parser) -> Result<Node<TransformExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("HASH:");

    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.split(',').collect();

    if parts.len() < 2 {
        return Err(parser.error("HASH requires algorithm and path"));
    }

    let algorithm = HashAlgorithm::from_str(parts[0].trim())
        .ok_or_else(|| parser.error(format!("Invalid hash algorithm: {}", parts[0])))?;
    let path = parts[1].trim().to_string();
    let target_field = if parts.len() > 2 {
        parts[2].trim().to_string()
    } else {
        format!("{}_hash", path.trim_start_matches('/').replace('/', "_"))
    };

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        TransformExpr::Hash { algorithm, path, target_field },
        Span::new(start, end),
    ))
}

fn parse_string_transform(parser: &mut Parser) -> Result<Node<TransformExpr>, ParseError> {
    let start = parser.current_position();

    let op = if parser.peek_str("UPPERCASE:") {
        parser.consume_str("UPPERCASE:");
        StringOp::Uppercase
    } else if parser.peek_str("LOWERCASE:") {
        parser.consume_str("LOWERCASE:");
        StringOp::Lowercase
    } else if parser.peek_str("TRIM:") {
        parser.consume_str("TRIM:");
        StringOp::Trim
    } else {
        return Err(parser.error("Unknown string operation"));
    };

    let path = parser.read_until_any(&[]).trim().to_string();
    let end = parser.current_position();

    Ok(Node::new(
        TransformExpr::String { op, path },
        Span::new(start, end),
    ))
}

fn parse_array_map_transform(parser: &mut Parser) -> Result<Node<TransformExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("ARRAY_MAP:");

    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.split(',').collect();

    if parts.len() < 2 {
        return Err(parser.error("ARRAY_MAP requires array path and element path"));
    }

    let array_path = parts[0].trim().to_string();
    let element_path = parts[1].trim().to_string();
    let target_field = if parts.len() > 2 {
        parts[2].trim().to_string()
    } else {
        format!("{}_mapped", array_path.trim_start_matches('/').replace('/', "_"))
    };

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        TransformExpr::ArrayMap { array_path, element_path, target_field },
        Span::new(start, end),
    ))
}

fn parse_array_filter_transform(parser: &mut Parser) -> Result<Node<TransformExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("ARRAY_FILTER:");

    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.splitn(2, ',').collect();

    if parts.len() < 2 {
        return Err(parser.error("ARRAY_FILTER requires array path and filter"));
    }

    let array_path = parts[0].trim().to_string();
    let filter_str = parts[1].trim();

    let mut sub_parser = Parser::new(filter_str);
    let filter = parse_filter_expr_inner(&mut sub_parser)?;

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        TransformExpr::ArrayFilter {
            array_path,
            filter: Box::new(filter),
        },
        Span::new(start, end),
    ))
}

fn parse_arithmetic_transform(parser: &mut Parser) -> Result<Node<TransformExpr>, ParseError> {
    let start = parser.current_position();

    // Determine operation
    let op = if parser.peek_str("ADD:") {
        parser.consume_str("ADD:");
        ArithmeticOp::Add
    } else if parser.peek_str("SUB:") || parser.peek_str("SUBTRACT:") {
        if parser.peek_str("SUB:") {
            parser.consume_str("SUB:");
        } else {
            parser.consume_str("SUBTRACT:");
        }
        ArithmeticOp::Sub
    } else if parser.peek_str("MUL:") || parser.peek_str("MULTIPLY:") {
        if parser.peek_str("MUL:") {
            parser.consume_str("MUL:");
        } else {
            parser.consume_str("MULTIPLY:");
        }
        ArithmeticOp::Mul
    } else if parser.peek_str("DIV:") || parser.peek_str("DIVIDE:") {
        if parser.peek_str("DIV:") {
            parser.consume_str("DIV:");
        } else {
            parser.consume_str("DIVIDE:");
        }
        ArithmeticOp::Div
    } else if parser.peek_str("ARITHMETIC:") {
        parser.consume_str("ARITHMETIC:");
        // Parse operation from next part
        let rest = &parser.input[parser.position..];
        let parts: Vec<&str> = rest.split(',').collect();
        if parts.is_empty() {
            return Err(parser.error("ARITHMETIC requires operation"));
        }
        ArithmeticOp::from_str(parts[0].trim())
            .ok_or_else(|| parser.error(format!("Invalid arithmetic operation: {}", parts[0])))?
    } else {
        return Err(parser.error("Unknown arithmetic operation"));
    };

    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.split(',').collect();

    let (left, right) = if op == ArithmeticOp::Add && parts[0].trim().starts_with("ARITHMETIC:") {
        // ARITHMETIC:ADD,/path,value format
        if parts.len() < 3 {
            return Err(parser.error("ARITHMETIC requires left and right operands"));
        }
        (
            parse_arithmetic_operand(parts[1].trim())?,
            parse_arithmetic_operand(parts[2].trim())?,
        )
    } else {
        // ADD:/path,value format
        if parts.len() < 2 {
            return Err(parser.error("Arithmetic requires left and right operands"));
        }
        (
            parse_arithmetic_operand(parts[0].trim())?,
            parse_arithmetic_operand(parts[1].trim())?,
        )
    };

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        TransformExpr::Arithmetic { op, left, right },
        Span::new(start, end),
    ))
}

fn parse_arithmetic_operand(s: &str) -> Result<ArithmeticOperand, ParseError> {
    if s.starts_with('/') {
        Ok(ArithmeticOperand::Path(s.to_string()))
    } else if let Ok(n) = s.parse::<f64>() {
        Ok(ArithmeticOperand::Constant(n))
    } else {
        Err(ParseError::new(
            format!("Invalid arithmetic operand: {}", s),
            Span::new(Position::zero(), Position::zero()),
            s,
        ))
    }
}

fn parse_coalesce_transform(parser: &mut Parser) -> Result<Node<TransformExpr>, ParseError> {
    let start = parser.current_position();
    parser.consume_str("COALESCE:");

    let rest = &parser.input[parser.position..];
    let parts: Vec<&str> = rest.split(',').collect();

    if parts.is_empty() {
        return Err(parser.error("COALESCE requires at least one path"));
    }

    let mut paths = Vec::new();
    let mut default_value = None;

    for (i, part) in parts.iter().enumerate() {
        let part = part.trim();
        if i == parts.len() - 1 && !part.starts_with('/') {
            // Last part might be default value
            default_value = Some(part.to_string());
        } else {
            paths.push(part.to_string());
        }
    }

    parser.position = parser.input.len();
    let end = parser.current_position();

    Ok(Node::new(
        TransformExpr::Coalesce { paths, default: default_value },
        Span::new(start, end),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_filter() {
        let result = parse_filter_expr("/status,==,active");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::JsonPath { path, op, value } => {
                assert_eq!(path, "/status");
                assert_eq!(*op, ComparisonOp::Eq);
                match value {
                    Literal::String(s) => assert_eq!(s, "active"),
                    _ => panic!("Expected string literal"),
                }
            }
            _ => panic!("Expected JsonPath filter"),
        }
    }

    #[test]
    fn test_parse_and_filter() {
        let result = parse_filter_expr("AND:/status,==,active:/age,>,18");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::And(exprs) => {
                assert_eq!(exprs.len(), 2);
            }
            _ => panic!("Expected AND filter"),
        }
    }

    #[test]
    fn test_parse_regex_filter() {
        let result = parse_filter_expr("REGEX:/email,.*@example\\.com");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::Regex { path, pattern } => {
                assert_eq!(path, "/email");
                assert!(pattern.contains("@example"));
            }
            _ => panic!("Expected Regex filter"),
        }
    }

    #[test]
    fn test_parse_simple_transform() {
        let result = parse_transform_expr("/data");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            TransformExpr::JsonPath(path) => {
                assert_eq!(path, "/data");
            }
            _ => panic!("Expected JsonPath transform"),
        }
    }

    #[test]
    fn test_parse_construct_transform() {
        let result = parse_transform_expr("CONSTRUCT:id=/user/id:name=/user/name");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            TransformExpr::Construct(fields) => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].0, "id");
                assert_eq!(fields[0].1, "/user/id");
            }
            _ => panic!("Expected Construct transform"),
        }
    }

    #[test]
    fn test_parse_error_with_position() {
        let result = parse_filter_expr("/status,INVALID,active");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.message.contains("Invalid operator"));
    }
}
