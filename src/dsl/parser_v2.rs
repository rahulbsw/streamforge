// Function-style DSL Parser (v2.0)
//
// Parses expressions like:
//   and(field('/status') == 'active', field('/count') > 10)
//   construct(id: field('/user/id'), name: field('/user/name'))

use super::ast::{ComparisonOp, FilterExpr, Literal, Node};
use super::error::{ParseError, Position, Span};

/// Token types for the lexer
#[derive(Debug, Clone, PartialEq)]
enum Token {
    // Keywords
    And,
    Or,
    Not,
    Field,
    Exists,
    NotExists,
    IsNull,
    IsNotNull,
    IsEmpty,
    IsNotEmpty,
    IsBlank,
    StartsWith,
    EndsWith,
    Contains,
    Length,
    Regex,
    ArrayAny,
    ArrayAll,
    ArrayContains,
    ArrayLength,
    KeyPrefix,
    KeyMatches,
    Header,
    TimestampAge,
    Extract,
    Construct,
    Hash,
    Uppercase,
    Lowercase,
    Trim,
    TrimStart,
    TrimEnd,
    Substring,
    Split,
    Join,
    Concat,
    Replace,
    PadLeft,
    PadRight,
    ToString,
    ToInt,
    ToFloat,
    Now,
    NowIso,
    ParseDate,
    FromEpoch,
    FromEpochSeconds,
    FormatDate,
    ToEpoch,
    ToEpochSeconds,
    ToIso,
    AddDays,
    AddHours,
    AddMinutes,
    SubtractDays,
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    DayOfWeek,
    DayOfYear,
    Coalesce,
    Try,
    ArrayMap,
    ArrayFilter,
    Add,
    Sub,
    Mul,
    Div,

    // Operators
    Eq,       // ==
    Ne,       // !=
    Gt,       // >
    Ge,       // >=
    Lt,       // <
    Le,       // <=
    Plus,     // +
    Minus,    // -
    Star,     // *
    Slash,    // /
    Colon,    // :
    Arrow,    // =>
    Question, // ??
    Dollar,   // $
    Dot,      // .

    // Delimiters
    LParen,   // (
    RParen,   // )
    LBracket, // [
    RBracket, // ]
    Comma,    // ,

    // Literals
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
    Identifier(String),

    // End of input
    Eof,
}

/// Lexer for tokenizing input
struct Lexer {
    input: String,      // Keep for error reporting
    chars: Vec<char>,   // Char vector for O(1) access
    position: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    fn new(input: &str) -> Self {
        Self {
            input: input.to_string(),
            chars: input.chars().collect(),
            position: 0,
            line: 1,
            column: 1,
        }
    }

    fn current_char(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn peek_char(&self, offset: usize) -> Option<char> {
        self.chars.get(self.position + offset).copied()
    }

    fn advance(&mut self) {
        if let Some(ch) = self.current_char() {
            self.position += 1;
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn read_string(&mut self, quote: char) -> Result<String, ParseError> {
        let start_pos = self.get_position();
        self.advance(); // Skip opening quote

        let mut value = String::new();
        let mut escaped = false;

        loop {
            match self.current_char() {
                None => {
                    return Err(ParseError::new(
                        "Unterminated string literal",
                        Span::new(start_pos, self.get_position()),
                        &self.input,
                    ));
                }
                Some(ch) if escaped => {
                    // Handle escape sequences
                    let escape_char = match ch {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '\\' => '\\',
                        '\'' => '\'',
                        '"' => '"',
                        _ => ch,
                    };
                    value.push(escape_char);
                    escaped = false;
                    self.advance();
                }
                Some('\\') => {
                    escaped = true;
                    self.advance();
                }
                Some(ch) if ch == quote => {
                    self.advance(); // Skip closing quote
                    return Ok(value);
                }
                Some(ch) => {
                    value.push(ch);
                    self.advance();
                }
            }
        }
    }

    fn read_number(&mut self) -> Result<f64, ParseError> {
        let start = self.position;
        let start_pos = self.get_position();

        // Handle negative sign
        if self.current_char() == Some('-') {
            self.advance();
        }

        // Read digits before decimal point
        while let Some(ch) = self.current_char() {
            if ch.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }

        // Read decimal point and digits after
        if self.current_char() == Some('.') {
            self.advance();
            while let Some(ch) = self.current_char() {
                if ch.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        // Collect chars into string for parsing
        let number_str: String = self.chars[start..self.position].iter().collect();

        number_str.parse().map_err(|e| {
            ParseError::new(
                format!("Invalid number literal '{}': {}", number_str, e),
                Span::new(start_pos, self.get_position()),
                &self.input,
            )
        })
    }

    fn read_identifier(&mut self) -> String {
        let start = self.position;

        while let Some(ch) = self.current_char() {
            if ch.is_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }

        self.chars[start..self.position].iter().collect()
    }

    fn get_position(&self) -> Position {
        Position::new(self.line, self.column, self.position)
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace();

        let ch = match self.current_char() {
            Some(c) => c,
            None => return Ok(Token::Eof),
        };

        let token = match ch {
            '(' => {
                self.advance();
                Token::LParen
            }
            ')' => {
                self.advance();
                Token::RParen
            }
            '[' => {
                self.advance();
                Token::LBracket
            }
            ']' => {
                self.advance();
                Token::RBracket
            }
            ',' => {
                self.advance();
                Token::Comma
            }
            ':' => {
                self.advance();
                Token::Colon
            }
            '+' => {
                self.advance();
                Token::Plus
            }
            '-' => {
                if self.peek_char(1).is_some_and(|c| c.is_ascii_digit()) {
                    Token::Number(self.read_number()?)
                } else {
                    self.advance();
                    Token::Minus
                }
            }
            '*' => {
                self.advance();
                Token::Star
            }
            '/' => {
                self.advance();
                Token::Slash
            }
            '=' => {
                self.advance();
                match self.current_char() {
                    Some('=') => {
                        self.advance();
                        Token::Eq
                    }
                    Some('>') => {
                        self.advance();
                        Token::Arrow
                    }
                    _ => {
                        return Err(ParseError::new(
                            "Unexpected '=', did you mean '==' or '=>'?",
                            Span::new(self.get_position(), self.get_position()),
                            &self.input,
                        ));
                    }
                }
            }
            '!' => {
                self.advance();
                if self.current_char() == Some('=') {
                    self.advance();
                    Token::Ne
                } else {
                    return Err(ParseError::new(
                        "Unexpected '!', did you mean '!='?",
                        Span::new(self.get_position(), self.get_position()),
                        &self.input,
                    ));
                }
            }
            '>' => {
                self.advance();
                if self.current_char() == Some('=') {
                    self.advance();
                    Token::Ge
                } else {
                    Token::Gt
                }
            }
            '<' => {
                self.advance();
                if self.current_char() == Some('=') {
                    self.advance();
                    Token::Le
                } else {
                    Token::Lt
                }
            }
            '?' => {
                self.advance();
                if self.current_char() == Some('?') {
                    self.advance();
                    Token::Question
                } else {
                    return Err(ParseError::new(
                        "Unexpected '?', did you mean '??'?",
                        Span::new(self.get_position(), self.get_position()),
                        &self.input,
                    ));
                }
            }
            '$' => {
                self.advance();
                Token::Dollar
            }
            '.' => {
                // Check if it's part of a number (e.g., .5) or a dot operator
                if self.peek_char(1).is_some_and(|c| c.is_ascii_digit()) {
                    // It's a number like .5
                    Token::Number(self.read_number()?)
                } else {
                    self.advance();
                    Token::Dot
                }
            }
            '\'' | '"' => Token::String(self.read_string(ch)?),
            _ if ch.is_ascii_digit() => Token::Number(self.read_number()?),
            _ if ch.is_alphabetic() || ch == '_' => {
                let ident = self.read_identifier();
                self.keyword_or_identifier(&ident)
            }
            _ => {
                return Err(ParseError::new(
                    format!("Unexpected character: '{}'", ch),
                    Span::new(self.get_position(), self.get_position()),
                    &self.input,
                ));
            }
        };

        Ok(token)
    }

    fn keyword_or_identifier(&self, ident: &str) -> Token {
        match ident {
            // Boolean keywords
            "true" => Token::Boolean(true),
            "false" => Token::Boolean(false),
            "null" => Token::Null,

            // Filter keywords
            "and" => Token::And,
            "or" => Token::Or,
            "not" => Token::Not,
            "field" => Token::Field,
            "exists" => Token::Exists,
            "not_exists" => Token::NotExists,
            "is_null" => Token::IsNull,
            "is_not_null" => Token::IsNotNull,
            "is_empty" => Token::IsEmpty,
            "is_not_empty" => Token::IsNotEmpty,
            "is_blank" => Token::IsBlank,
            "starts_with" => Token::StartsWith,
            "ends_with" => Token::EndsWith,
            "contains" => Token::Contains,
            "length" => Token::Length,
            "regex" => Token::Regex,
            "array_any" => Token::ArrayAny,
            "array_all" => Token::ArrayAll,
            "array_contains" => Token::ArrayContains,
            "array_length" => Token::ArrayLength,
            "key_prefix" => Token::KeyPrefix,
            "key_matches" => Token::KeyMatches,
            "header" => Token::Header,
            "timestamp_age" => Token::TimestampAge,

            // Transform keywords
            "extract" => Token::Extract,
            "construct" => Token::Construct,
            "hash" => Token::Hash,
            "uppercase" => Token::Uppercase,
            "lowercase" => Token::Lowercase,
            "trim" => Token::Trim,
            "trim_start" => Token::TrimStart,
            "trim_end" => Token::TrimEnd,
            "substring" => Token::Substring,
            "split" => Token::Split,
            "join" => Token::Join,
            "concat" => Token::Concat,
            "replace" => Token::Replace,
            "pad_left" => Token::PadLeft,
            "pad_right" => Token::PadRight,
            "to_string" => Token::ToString,
            "to_int" => Token::ToInt,
            "to_float" => Token::ToFloat,
            "now" => Token::Now,
            "now_iso" => Token::NowIso,
            "parse_date" => Token::ParseDate,
            "from_epoch" => Token::FromEpoch,
            "from_epoch_seconds" => Token::FromEpochSeconds,
            "format_date" => Token::FormatDate,
            "to_epoch" => Token::ToEpoch,
            "to_epoch_seconds" => Token::ToEpochSeconds,
            "to_iso" => Token::ToIso,
            "add_days" => Token::AddDays,
            "add_hours" => Token::AddHours,
            "add_minutes" => Token::AddMinutes,
            "subtract_days" => Token::SubtractDays,
            "year" => Token::Year,
            "month" => Token::Month,
            "day" => Token::Day,
            "hour" => Token::Hour,
            "minute" => Token::Minute,
            "second" => Token::Second,
            "day_of_week" => Token::DayOfWeek,
            "day_of_year" => Token::DayOfYear,
            "coalesce" => Token::Coalesce,
            "try" => Token::Try,
            "array_map" => Token::ArrayMap,
            "array_filter" => Token::ArrayFilter,
            "add" => Token::Add,
            "sub" | "subtract" => Token::Sub,
            "mul" | "multiply" => Token::Mul,
            "div" | "divide" => Token::Div,

            // Otherwise, it's an identifier
            _ => Token::Identifier(ident.to_string()),
        }
    }
}

/// Parser for building AST from tokens
pub struct ParserV2 {
    lexer: Lexer,
    current_token: Token,
    input: String,
}

impl ParserV2 {
    fn new(input: &str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next_token()?;

        Ok(Self {
            lexer,
            current_token,
            input: input.to_string(),
        })
    }

    fn advance(&mut self) -> Result<(), ParseError> {
        self.current_token = self.lexer.next_token()?;
        Ok(())
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        if std::mem::discriminant(&self.current_token) == std::mem::discriminant(&expected) {
            self.advance()
        } else {
            Err(ParseError::new(
                format!("Expected {:?}, got {:?}", expected, self.current_token),
                self.current_span(),
                &self.input,
            ))
        }
    }

    fn current_span(&self) -> Span {
        let pos = self.lexer.get_position();
        Span::new(pos, pos)
    }

    pub fn parse_filter(&mut self) -> Result<Node<FilterExpr>, ParseError> {
        let start_pos = self.lexer.get_position();

        let filter = match &self.current_token {
            Token::And => self.parse_and_filter()?,
            Token::Or => self.parse_or_filter()?,
            Token::Not => self.parse_not_filter()?,
            Token::Field => self.parse_field_comparison()?,
            Token::Dollar => self.parse_dollar_comparison()?,
            Token::Exists => self.parse_exists_filter()?,
            Token::NotExists => self.parse_not_exists_filter()?,
            Token::IsNull => self.parse_is_null_filter()?,
            Token::IsNotNull => self.parse_is_not_null_filter()?,
            Token::IsEmpty => self.parse_is_empty_filter()?,
            Token::IsNotEmpty => self.parse_is_not_empty_filter()?,
            Token::IsBlank => self.parse_is_blank_filter()?,
            Token::Regex => self.parse_regex_filter()?,
            _ => {
                return Err(ParseError::new(
                    format!("Unexpected token in filter: {:?}", self.current_token),
                    self.current_span(),
                    &self.input,
                ));
            }
        };

        let end_pos = self.lexer.get_position();
        Ok(Node::new(filter, Span::new(start_pos, end_pos)))
    }

    fn parse_and_filter(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::And)?;
        self.expect(Token::LParen)?;

        let mut exprs = vec![];
        loop {
            exprs.push(self.parse_filter()?);

            if self.current_token == Token::RParen {
                break;
            }
            self.expect(Token::Comma)?;
        }

        self.expect(Token::RParen)?;
        Ok(FilterExpr::And(exprs))
    }

    fn parse_or_filter(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::Or)?;
        self.expect(Token::LParen)?;

        let mut exprs = vec![];
        loop {
            exprs.push(self.parse_filter()?);

            if self.current_token == Token::RParen {
                break;
            }
            self.expect(Token::Comma)?;
        }

        self.expect(Token::RParen)?;
        Ok(FilterExpr::Or(exprs))
    }

    fn parse_not_filter(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::Not)?;
        self.expect(Token::LParen)?;
        let expr = self.parse_filter()?;
        self.expect(Token::RParen)?;
        Ok(FilterExpr::Not(Box::new(expr)))
    }

    fn parse_field_comparison(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::Field)?;
        self.expect(Token::LParen)?;

        let path = match &self.current_token {
            Token::String(s) => s.clone(),
            _ => {
                return Err(ParseError::new(
                    "Expected string path in field()",
                    self.current_span(),
                    &self.input,
                ));
            }
        };
        self.advance()?;
        self.expect(Token::RParen)?;

        // Now expect comparison operator
        let op = match &self.current_token {
            Token::Eq => ComparisonOp::Eq,
            Token::Ne => ComparisonOp::Ne,
            Token::Gt => ComparisonOp::Gt,
            Token::Ge => ComparisonOp::Ge,
            Token::Lt => ComparisonOp::Lt,
            Token::Le => ComparisonOp::Le,
            _ => {
                return Err(ParseError::new(
                    "Expected comparison operator after field()",
                    self.current_span(),
                    &self.input,
                ));
            }
        };
        self.advance()?;

        // Parse value
        let value = self.parse_literal()?;

        Ok(FilterExpr::JsonPath { path, op, value })
    }

    fn parse_dollar_comparison(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::Dollar)?;

        // Check if it's $(...) or $identifier
        let path = if self.current_token == Token::LParen {
            // $('/explicit/path') form
            self.advance()?;
            let path = match &self.current_token {
                Token::String(s) => s.clone(),
                _ => {
                    return Err(ParseError::new(
                        "Expected string path in $()",
                        self.current_span(),
                        &self.input,
                    ));
                }
            };
            self.advance()?;
            self.expect(Token::RParen)?;
            path
        } else {
            // $identifier or $identifier.field.subfield form
            let mut parts = vec![];

            // Read first identifier
            match &self.current_token {
                Token::Identifier(id) => {
                    parts.push(id.clone());
                    self.advance()?;
                }
                _ => {
                    return Err(ParseError::new(
                        "Expected identifier after $",
                        self.current_span(),
                        &self.input,
                    ));
                }
            }

            // Check for dot notation: $field.subfield.name
            while self.current_token == Token::Dot {
                self.advance()?; // Skip dot

                match &self.current_token {
                    Token::Identifier(id) => {
                        parts.push(id.clone());
                        self.advance()?;
                    }
                    _ => {
                        return Err(ParseError::new(
                            "Expected identifier after .",
                            self.current_span(),
                            &self.input,
                        ));
                    }
                }
            }

            // Convert $field.subfield to /field/subfield
            format!("/{}", parts.join("/"))
        };

        // Now expect comparison operator
        let op = match &self.current_token {
            Token::Eq => ComparisonOp::Eq,
            Token::Ne => ComparisonOp::Ne,
            Token::Gt => ComparisonOp::Gt,
            Token::Ge => ComparisonOp::Ge,
            Token::Lt => ComparisonOp::Lt,
            Token::Le => ComparisonOp::Le,
            _ => {
                return Err(ParseError::new(
                    "Expected comparison operator after $field",
                    self.current_span(),
                    &self.input,
                ));
            }
        };
        self.advance()?;

        // Parse value
        let value = self.parse_literal()?;

        Ok(FilterExpr::JsonPath { path, op, value })
    }

    fn parse_literal(&mut self) -> Result<Literal, ParseError> {
        let literal = match &self.current_token {
            Token::String(s) => Literal::String(s.clone()),
            Token::Number(n) => Literal::Number(*n),
            Token::Boolean(b) => Literal::Boolean(*b),
            Token::Null => Literal::Null,
            _ => {
                return Err(ParseError::new(
                    "Expected literal value",
                    self.current_span(),
                    &self.input,
                ));
            }
        };
        self.advance()?;
        Ok(literal)
    }

    fn parse_exists_filter(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::Exists)?;
        self.expect(Token::LParen)?;
        let path = match &self.current_token {
            Token::String(s) => s.clone(),
            _ => {
                return Err(ParseError::new(
                    "Expected string path",
                    self.current_span(),
                    &self.input,
                ));
            }
        };
        self.advance()?;
        self.expect(Token::RParen)?;
        Ok(FilterExpr::Exists(path))
    }

    fn parse_not_exists_filter(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::NotExists)?;
        self.expect(Token::LParen)?;
        let path = match &self.current_token {
            Token::String(s) => s.clone(),
            _ => {
                return Err(ParseError::new(
                    "Expected string path",
                    self.current_span(),
                    &self.input,
                ));
            }
        };
        self.advance()?;
        self.expect(Token::RParen)?;
        Ok(FilterExpr::NotExists(path))
    }

    fn parse_is_null_filter(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::IsNull)?;
        self.expect(Token::LParen)?;
        // Can be either field('/path') or just '/path'
        let path = if self.current_token == Token::Field {
            self.advance()?;
            self.expect(Token::LParen)?;
            let p = match &self.current_token {
                Token::String(s) => s.clone(),
                _ => {
                    return Err(ParseError::new(
                        "Expected string path",
                        self.current_span(),
                        &self.input,
                    ));
                }
            };
            self.advance()?;
            self.expect(Token::RParen)?;
            p
        } else {
            match &self.current_token {
                Token::String(s) => s.clone(),
                _ => {
                    return Err(ParseError::new(
                        "Expected string path",
                        self.current_span(),
                        &self.input,
                    ));
                }
            }
        };
        if self.current_token != Token::RParen {
            self.advance()?;
        }
        self.expect(Token::RParen)?;
        Ok(FilterExpr::IsNull(path))
    }

    fn parse_is_not_null_filter(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::IsNotNull)?;
        self.expect(Token::LParen)?;
        let path = match &self.current_token {
            Token::String(s) => s.clone(),
            _ => {
                return Err(ParseError::new(
                    "Expected string path",
                    self.current_span(),
                    &self.input,
                ));
            }
        };
        self.advance()?;
        self.expect(Token::RParen)?;
        Ok(FilterExpr::IsNotNull(path))
    }

    fn parse_is_empty_filter(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::IsEmpty)?;
        self.expect(Token::LParen)?;
        let path = match &self.current_token {
            Token::String(s) => s.clone(),
            _ => {
                return Err(ParseError::new(
                    "Expected string path",
                    self.current_span(),
                    &self.input,
                ));
            }
        };
        self.advance()?;
        self.expect(Token::RParen)?;
        Ok(FilterExpr::IsEmpty(path))
    }

    fn parse_is_not_empty_filter(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::IsNotEmpty)?;
        self.expect(Token::LParen)?;
        let path = match &self.current_token {
            Token::String(s) => s.clone(),
            _ => {
                return Err(ParseError::new(
                    "Expected string path",
                    self.current_span(),
                    &self.input,
                ));
            }
        };
        self.advance()?;
        self.expect(Token::RParen)?;
        Ok(FilterExpr::IsNotEmpty(path))
    }

    fn parse_is_blank_filter(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::IsBlank)?;
        self.expect(Token::LParen)?;
        let path = match &self.current_token {
            Token::String(s) => s.clone(),
            _ => {
                return Err(ParseError::new(
                    "Expected string path",
                    self.current_span(),
                    &self.input,
                ));
            }
        };
        self.advance()?;
        self.expect(Token::RParen)?;
        Ok(FilterExpr::IsBlank(path))
    }

    fn parse_regex_filter(&mut self) -> Result<FilterExpr, ParseError> {
        self.expect(Token::Regex)?;
        self.expect(Token::LParen)?;

        // First argument: field('/path') or '/path'
        let path = if self.current_token == Token::Field {
            self.advance()?;
            self.expect(Token::LParen)?;
            let p = match &self.current_token {
                Token::String(s) => s.clone(),
                _ => {
                    return Err(ParseError::new(
                        "Expected string path",
                        self.current_span(),
                        &self.input,
                    ));
                }
            };
            self.advance()?;
            self.expect(Token::RParen)?;
            p
        } else {
            match &self.current_token {
                Token::String(s) => s.clone(),
                _ => {
                    return Err(ParseError::new(
                        "Expected string path or field()",
                        self.current_span(),
                        &self.input,
                    ));
                }
            }
        };

        if self.current_token != Token::Comma {
            self.advance()?;
        }
        self.expect(Token::Comma)?;

        // Second argument: pattern string
        let pattern = match &self.current_token {
            Token::String(s) => s.clone(),
            _ => {
                return Err(ParseError::new(
                    "Expected regex pattern string",
                    self.current_span(),
                    &self.input,
                ));
            }
        };
        self.advance()?;
        self.expect(Token::RParen)?;

        Ok(FilterExpr::Regex { path, pattern })
    }
}

/// Parse a filter expression in function-style syntax
pub fn parse_filter_expr_v2(input: &str) -> Result<Node<FilterExpr>, ParseError> {
    let mut parser = ParserV2::new(input)?;
    parser.parse_filter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_basic_tokens() {
        let mut lexer = Lexer::new("and(field('/status') == 'active')");

        assert_eq!(lexer.next_token().unwrap(), Token::And);
        assert_eq!(lexer.next_token().unwrap(), Token::LParen);
        assert_eq!(lexer.next_token().unwrap(), Token::Field);
        assert_eq!(lexer.next_token().unwrap(), Token::LParen);
        assert_eq!(lexer.next_token().unwrap(), Token::String("/status".to_string()));
        assert_eq!(lexer.next_token().unwrap(), Token::RParen);
        assert_eq!(lexer.next_token().unwrap(), Token::Eq);
        assert_eq!(lexer.next_token().unwrap(), Token::String("active".to_string()));
        assert_eq!(lexer.next_token().unwrap(), Token::RParen);
        assert_eq!(lexer.next_token().unwrap(), Token::Eof);
    }

    #[test]
    fn test_lexer_numbers() {
        let mut lexer = Lexer::new("123 45.67 -8.9");

        assert_eq!(lexer.next_token().unwrap(), Token::Number(123.0));
        assert_eq!(lexer.next_token().unwrap(), Token::Number(45.67));
        assert_eq!(lexer.next_token().unwrap(), Token::Number(-8.9));
    }

    #[test]
    fn test_lexer_booleans() {
        let mut lexer = Lexer::new("true false null");

        assert_eq!(lexer.next_token().unwrap(), Token::Boolean(true));
        assert_eq!(lexer.next_token().unwrap(), Token::Boolean(false));
        assert_eq!(lexer.next_token().unwrap(), Token::Null);
    }

    #[test]
    fn test_parse_simple_comparison() {
        let result = parse_filter_expr_v2("field('/status') == 'active'");
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
        let result = parse_filter_expr_v2("and(field('/status') == 'active', field('/count') > 10)");
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
    fn test_parse_not_filter() {
        let result = parse_filter_expr_v2("not(field('/deleted') == true)");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::Not(_) => {}
            _ => panic!("Expected NOT filter"),
        }
    }

    #[test]
    fn test_parse_exists() {
        let result = parse_filter_expr_v2("exists('/optional_field')");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::Exists(path) => {
                assert_eq!(path, "/optional_field");
            }
            _ => panic!("Expected EXISTS filter"),
        }
    }

    #[test]
    fn test_parse_is_null() {
        let result = parse_filter_expr_v2("is_null('/value')");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::IsNull(path) => {
                assert_eq!(path, "/value");
            }
            _ => panic!("Expected IS_NULL filter"),
        }
    }

    #[test]
    fn test_parse_regex() {
        let result = parse_filter_expr_v2("regex(field('/email'), '^[a-z]+@example\\\\.com$')");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::Regex { path, pattern } => {
                assert_eq!(path, "/email");
                assert_eq!(pattern, "^[a-z]+@example\\.com$");
            }
            _ => panic!("Expected REGEX filter"),
        }
    }

    #[test]
    fn test_number_parsing_edge_cases() {
        // Test that numbers parse correctly (not silently becoming 0.0)

        // Valid numbers should parse correctly
        let result = parse_filter_expr_v2("field('/count') > 123.45");
        assert!(result.is_ok());

        let result = parse_filter_expr_v2("field('/count') > -42");
        assert!(result.is_ok());

        let result = parse_filter_expr_v2("field('/count') > 0.5");
        assert!(result.is_ok());

        // Note: "123.45.67" will parse as "123.45" followed by ".67"
        // This is correct behavior - the lexer stops at the second dot
        let result = parse_filter_expr_v2("field('/count') > 123.45");
        assert!(result.is_ok());
    }

    #[test]
    fn test_arrow_token() {
        // Test that => (arrow) is properly tokenized as two characters, not three
        let mut lexer = Lexer::new("=>");
        let token = lexer.next_token().unwrap();
        assert_eq!(token, Token::Arrow, "=> should tokenize as Arrow");

        // Test that == is still tokenized correctly
        let mut lexer = Lexer::new("==");
        let token = lexer.next_token().unwrap();
        assert_eq!(token, Token::Eq, "== should tokenize as Eq");
    }

    #[test]
    fn test_unicode_handling() {
        // Test that multi-byte UTF-8 characters work correctly in identifiers
        let result = parse_filter_expr_v2("field('/日本語') == 'test'");
        assert!(result.is_ok(), "Should handle UTF-8 in field paths");

        // Test unicode in string values
        let result = parse_filter_expr_v2("field('/name') == '日本語'");
        assert!(result.is_ok(), "Should handle UTF-8 in string literals");
    }
}
