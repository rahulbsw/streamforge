# DSL Parser Refactoring Plan

**Version:** 1.0.0  
**Phase:** 2 - DSL Stabilization  
**Status:** Planning  
**Created:** 2026-04-18

## Executive Summary

Refactor the monolithic `filter_parser.rs` (~1912 lines) into a modular, maintainable parser architecture with clear separation of concerns:

- **Lexer:** Tokenization
- **Parser:** AST construction from tokens
- **AST:** Abstract Syntax Tree representation
- **Validator:** Semantic validation (type checking, path validation)
- **Evaluator:** AST execution (existing filter/transform traits)

## Current State

**File:** `src/filter_parser.rs` (1912 lines)

**Structure:**
- Mix of parsing and validation logic
- String splitting approach (`:` and `,` delimiters)
- Direct construction of filter/transform trait objects
- No intermediate AST representation
- Validation happens during parsing
- Error messages show raw input strings

**Problems:**
1. **Poor error messages:** "Invalid format" without position/context
2. **No look-ahead:** Can't validate before execution
3. **Hard to test:** Parsing and validation intertwined
4. **Hard to extend:** Adding new operators requires touching multiple functions
5. **No formal grammar validation:** Grammar is implicit in code

## Target Architecture

```
Input String
    ↓
┌─────────┐
│ Lexer   │ → Tokens
└─────────┘
    ↓
┌─────────┐
│ Parser  │ → AST
└─────────┘
    ↓
┌─────────┐
│Validator│ → Validated AST + Errors/Warnings
└─────────┘
    ↓
┌─────────┐
│Evaluator│ → Filter/Transform Trait Objects
└─────────┘
```

### Module Structure

```
src/dsl/
├── mod.rs              # Public API
├── token.rs            # Token types and lexer
├── ast.rs              # AST node definitions
├── parser.rs           # Token → AST
├── validator.rs        # AST validation
├── evaluator.rs        # AST → trait objects
└── error.rs            # DSL-specific errors
```

## Phase 2.1: Token Design

### Token Types

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Path(String),           // "/user/id"
    String(String),         // "active"
    Number(f64),            // 42, 3.14
    Boolean(bool),          // true, false
    Null,                   // null
    
    // Operators
    Colon,                  // :
    Comma,                  // ,
    
    // Comparison operators
    Eq,                     // ==
    Ne,                     // !=
    Gt,                     // >
    Ge,                     // >=
    Lt,                     // <
    Le,                     // <=
    
    // Keywords (filters)
    And,                    // AND
    Or,                     // OR
    Not,                    // NOT
    Regex,                  // REGEX
    ArrayAll,               // ARRAY_ALL
    ArrayAny,               // ARRAY_ANY
    KeyPrefix,              // KEY_PREFIX
    KeyMatches,             // KEY_MATCHES
    KeyExists,              // KEY_EXISTS
    HeaderExists,           // HEADER_EXISTS
    Header,                 // HEADER
    TimestampAge,           // TIMESTAMP_AGE
    TimestampAfter,         // TIMESTAMP_AFTER
    TimestampBefore,        // TIMESTAMP_BEFORE
    
    // Keywords (transforms)
    Construct,              // CONSTRUCT
    ArrayMap,               // ARRAY_MAP
    Add,                    // ADD
    Sub,                    // SUB
    Mul,                    // MUL
    Div,                    // DIV
    Hash,                   // HASH
    CacheLookup,            // CACHE_LOOKUP
    CachePut,               // CACHE_PUT
    StringOp(StringOpKind), // STRING:UPPER, etc.
    
    // Special
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StringOpKind {
    Upper, Lower, Trim, TrimStart, TrimEnd,
    Length, Substring, Replace, ReplaceAll,
    RegexReplace, Split, Concat,
}
```

### Lexer Implementation

```rust
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0, line: 1, column: 1 }
    }
    
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        while let Some(token) = self.next_token()? {
            tokens.push(token);
        }
        tokens.push(Token::Eof);
        Ok(tokens)
    }
    
    fn next_token(&mut self) -> Result<Option<Token>, LexError> {
        self.skip_whitespace();
        
        match self.peek() {
            None => Ok(None),
            Some('/') => self.read_path(),
            Some(':') => { self.advance(); Ok(Some(Token::Colon)) },
            Some(',') => { self.advance(); Ok(Some(Token::Comma)) },
            Some(c) if c.is_alphabetic() => self.read_keyword_or_string(),
            Some(c) if c.is_numeric() || c == '-' => self.read_number(),
            Some('"') => self.read_quoted_string(),
            Some(c) => Err(LexError::unexpected_char(c, self.line, self.column)),
        }
    }
    
    fn read_keyword_or_string(&mut self) -> Result<Option<Token>, LexError> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        
        let word = &self.input[start..self.pos];
        let token = match word {
            "AND" => Token::And,
            "OR" => Token::Or,
            "NOT" => Token::Not,
            "true" => Token::Boolean(true),
            "false" => Token::Boolean(false),
            "null" => Token::Null,
            // ... all other keywords
            _ => Token::String(word.to_string()),
        };
        
        Ok(Some(token))
    }
}
```

## Phase 2.2: AST Design

### AST Node Types

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum FilterAst {
    Simple {
        path: String,
        op: ComparisonOp,
        value: Literal,
        span: Span,
    },
    And {
        conditions: Vec<FilterAst>,
        span: Span,
    },
    Or {
        conditions: Vec<FilterAst>,
        span: Span,
    },
    Not {
        condition: Box<FilterAst>,
        span: Span,
    },
    Regex {
        path: String,
        pattern: String,
        span: Span,
    },
    ArrayAll {
        path: String,
        element_filter: Box<FilterAst>,
        span: Span,
    },
    ArrayAny {
        path: String,
        element_filter: Box<FilterAst>,
        span: Span,
    },
    KeyPrefix {
        prefix: String,
        span: Span,
    },
    KeyMatches {
        pattern: String,
        span: Span,
    },
    KeyExists {
        span: Span,
    },
    HeaderExists {
        name: String,
        span: Span,
    },
    Header {
        name: String,
        op: ComparisonOp,
        value: Literal,
        span: Span,
    },
    TimestampAge {
        op: ComparisonOp,
        seconds: i64,
        span: Span,
    },
    TimestampAfter {
        epoch_ms: i64,
        span: Span,
    },
    TimestampBefore {
        epoch_ms: i64,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransformAst {
    Extract {
        path: String,
        span: Span,
    },
    Construct {
        fields: Vec<FieldMapping>,
        span: Span,
    },
    ArrayMap {
        array_path: String,
        element_path: String,
        output_field: String,
        span: Span,
    },
    Arithmetic {
        op: ArithmeticOp,
        left: String,
        right: String,
        output_field: Option<String>,
        span: Span,
    },
    Hash {
        algorithm: HashAlgorithm,
        path: String,
        output_field: Option<String>,
        span: Span,
    },
    CacheLookup {
        key_path: String,
        store_name: String,
        output_mode: OutputMode,
        span: Span,
    },
    CachePut {
        key_path: String,
        store_name: String,
        value_path: Option<String>,
        span: Span,
    },
    StringOp {
        op: StringOperation,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOp {
    Eq, Ne, Gt, Ge, Lt, Le,
}

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}
```

### Parser Implementation

```rust
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }
    
    pub fn parse_filter(&mut self) -> Result<FilterAst, ParseError> {
        match self.peek() {
            Token::And => self.parse_and_filter(),
            Token::Or => self.parse_or_filter(),
            Token::Not => self.parse_not_filter(),
            Token::Regex => self.parse_regex_filter(),
            Token::Path(_) => self.parse_simple_filter(),
            // ... all other filter types
            token => Err(ParseError::unexpected_token(token, self.span())),
        }
    }
    
    fn parse_simple_filter(&mut self) -> Result<FilterAst, ParseError> {
        let start = self.span();
        let path = self.expect_path()?;
        self.expect(Token::Comma)?;
        let op = self.parse_comparison_op()?;
        self.expect(Token::Comma)?;
        let value = self.parse_literal()?;
        
        Ok(FilterAst::Simple {
            path,
            op,
            value,
            span: start.to(self.span()),
        })
    }
    
    fn parse_and_filter(&mut self) -> Result<FilterAst, ParseError> {
        let start = self.span();
        self.expect(Token::And)?;
        
        let mut conditions = Vec::new();
        loop {
            self.expect(Token::Colon)?;
            conditions.push(self.parse_filter()?);
            
            if !matches!(self.peek(), Token::Colon) {
                break;
            }
        }
        
        Ok(FilterAst::And {
            conditions,
            span: start.to(self.span()),
        })
    }
}
```

## Phase 2.3: Validator Design

### Validation Passes

```rust
pub struct Validator {
    errors: Vec<ValidationError>,
    warnings: Vec<ValidationWarning>,
}

impl Validator {
    pub fn validate_filter(&mut self, ast: &FilterAst) -> ValidationResult {
        // Pass 1: Validate JSON paths
        self.validate_paths(ast);
        
        // Pass 2: Check type compatibility
        self.validate_types(ast);
        
        // Pass 3: Check regex patterns
        self.validate_regexes(ast);
        
        // Pass 4: Deprecation warnings
        self.check_deprecations(ast);
        
        if self.errors.is_empty() {
            ValidationResult::Ok { warnings: self.warnings.clone() }
        } else {
            ValidationResult::Err {
                errors: self.errors.clone(),
                warnings: self.warnings.clone(),
            }
        }
    }
    
    fn validate_paths(&mut self, ast: &FilterAst) {
        match ast {
            FilterAst::Simple { path, span, .. } => {
                if !path.starts_with('/') {
                    self.errors.push(ValidationError {
                        message: format!("JSON path must start with '/', got: {}", path),
                        span: *span,
                        help: Some("Add '/' prefix to path".to_string()),
                    });
                }
                
                // Check for invalid characters
                if path.contains("//") {
                    self.errors.push(ValidationError {
                        message: "Path contains empty segment: '//'".to_string(),
                        span: *span,
                        help: Some("Remove extra '/' character".to_string()),
                    });
                }
            }
            FilterAst::And { conditions, .. } => {
                for cond in conditions {
                    self.validate_paths(cond);
                }
            }
            // ... other cases
        }
    }
    
    fn validate_types(&mut self, ast: &FilterAst) {
        match ast {
            FilterAst::Simple { op, value, span, .. } => {
                // Numeric operators require numeric values
                if matches!(op, ComparisonOp::Gt | ComparisonOp::Ge | ComparisonOp::Lt | ComparisonOp::Le) {
                    if !matches!(value, Literal::Number(_)) {
                        self.errors.push(ValidationError {
                            message: format!("Operator {:?} requires numeric value", op),
                            span: *span,
                            help: Some("Use a number, not a string".to_string()),
                        });
                    }
                }
            }
            // ... other cases
        }
    }
    
    fn validate_regexes(&mut self, ast: &FilterAst) {
        match ast {
            FilterAst::Regex { pattern, span, .. } => {
                if let Err(e) = Regex::new(pattern) {
                    self.errors.push(ValidationError {
                        message: format!("Invalid regex pattern: {}", e),
                        span: *span,
                        help: Some("Check regex syntax".to_string()),
                    });
                }
            }
            // ... other cases
        }
    }
}
```

### Error Types

```rust
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ValidationResult {
    Ok { warnings: Vec<ValidationWarning> },
    Err {
        errors: Vec<ValidationError>,
        warnings: Vec<ValidationWarning>,
    },
}
```

## Phase 2.4: Evaluator Design

### AST → Trait Objects

```rust
pub struct Evaluator {
    cache_manager: Option<Arc<SyncCacheManager>>,
}

impl Evaluator {
    pub fn eval_filter(&self, ast: &FilterAst) -> Result<Arc<dyn Filter>> {
        match ast {
            FilterAst::Simple { path, op, value, .. } => {
                Ok(Arc::new(JsonPathFilter::new(
                    path,
                    self.convert_op(*op),
                    self.convert_literal(value),
                )?))
            }
            FilterAst::And { conditions, .. } => {
                let filters: Result<Vec<_>> = conditions
                    .iter()
                    .map(|c| self.eval_filter(c))
                    .collect();
                Ok(Arc::new(AndFilter::new(filters?)))
            }
            // ... all other cases
        }
    }
    
    pub fn eval_transform(&self, ast: &TransformAst) -> Result<Arc<dyn Transform>> {
        match ast {
            TransformAst::Extract { path, .. } => {
                Ok(Arc::new(JsonPathTransform::new(path)?))
            }
            TransformAst::Hash { algorithm, path, output_field, .. } => {
                Ok(Arc::new(HashTransform::new(
                    *algorithm,
                    path,
                    output_field.as_deref(),
                )?))
            }
            // ... all other cases
        }
    }
}
```

## Phase 2.5: Public API

### Module Root (`src/dsl/mod.rs`)

```rust
mod token;
mod ast;
mod parser;
mod validator;
mod evaluator;
mod error;

pub use token::{Token, Lexer};
pub use ast::{FilterAst, TransformAst, Literal, ComparisonOp};
pub use parser::Parser;
pub use validator::{Validator, ValidationResult, ValidationError, ValidationWarning};
pub use evaluator::Evaluator;
pub use error::DslError;

/// Parse and validate a filter expression
pub fn parse_filter(expr: &str) -> Result<Arc<dyn Filter>, DslError> {
    let mut lexer = Lexer::new(expr);
    let tokens = lexer.tokenize()?;
    
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_filter()?;
    
    let mut validator = Validator::new();
    let validation = validator.validate_filter(&ast)?;
    
    // Log warnings
    for warning in validation.warnings() {
        tracing::warn!("{}", warning);
    }
    
    let evaluator = Evaluator::new(None);
    evaluator.eval_filter(&ast)
}

/// Parse and validate a transform expression
pub fn parse_transform(expr: &str) -> Result<Arc<dyn Transform>, DslError> {
    let mut lexer = Lexer::new(expr);
    let tokens = lexer.tokenize()?;
    
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_transform()?;
    
    let mut validator = Validator::new();
    let validation = validator.validate_transform(&ast)?;
    
    for warning in validation.warnings() {
        tracing::warn!("{}", warning);
    }
    
    let evaluator = Evaluator::new(None);
    evaluator.eval_transform(&ast)
}
```

## Implementation Plan

### Step 1: Create Module Structure (1 hour)

```bash
mkdir -p src/dsl
touch src/dsl/mod.rs
touch src/dsl/token.rs
touch src/dsl/ast.rs
touch src/dsl/parser.rs
touch src/dsl/validator.rs
touch src/dsl/evaluator.rs
touch src/dsl/error.rs
```

### Step 2: Implement Token & Lexer (2 hours)

- Define `Token` enum with all DSL keywords
- Implement `Lexer` with position tracking
- Add tests for tokenization
- **Tests:** 50+ tokenization test cases

### Step 3: Implement AST (1 hour)

- Define `FilterAst` and `TransformAst` enums
- Add `Span` for error reporting
- Add helper methods (e.g., `ast.span()`)
- **Tests:** AST construction tests

### Step 4: Implement Parser (3 hours)

- Token stream → AST
- Recursive descent parser
- Error recovery (skip to next `:` on error)
- **Tests:** 100+ parser test cases (golden tests)

### Step 5: Implement Validator (2 hours)

- Path validation
- Type checking
- Regex validation
- Deprecation warnings
- **Tests:** 30+ validation test cases

### Step 6: Implement Evaluator (1 hour)

- AST → trait objects (reuse existing filters/transforms)
- Cache manager integration
- **Tests:** Integration tests with filter/transform execution

### Step 7: Integration & Migration (2 hours)

- Update `filter_parser.rs` to use new DSL module
- Add backward compatibility shims
- Update all call sites
- **Tests:** Run full test suite

### Step 8: CLI Tool (1 hour)

- Create `src/bin/validate.rs`
- Read config file
- Parse all filter/transform expressions
- Report errors and warnings
- Exit with non-zero on errors

**Total:** ~13 hours

## Benefits

### Improved Error Messages

**Before:**
```
Error: Invalid filter format: /user/age,,18
```

**After:**
```
Error: Invalid filter expression
  --> config.yaml:12:18
   |
12 |   filter: "/user/age,,18"
   |                      ^ unexpected comma
   |
   = help: comparison filters expect: <path>,<op>,<value>
```

### Better Testing

**Before:** Integration tests only (parse + execute)

**After:**
- Unit tests for lexer
- Unit tests for parser
- Unit tests for validator
- Golden tests for parser (input → AST snapshots)
- Integration tests for evaluator

### Extensibility

Adding a new operator:
1. Add token to `Token` enum
2. Add AST node to `FilterAst`/`TransformAst`
3. Add parser case
4. Add validator rules
5. Add evaluator case

**Lines changed:** ~50 (vs ~200 in current implementation)

### Performance

**Parse-time validation:** Catch errors before pipeline starts

**Lazy evaluation:** Parse once, execute many times (cache AST)

**Zero overhead:** Evaluator produces same trait objects as before

---

## Success Criteria

- [ ] All 168 existing unit tests pass
- [ ] 100+ new parser tests (golden tests)
- [ ] Error messages include line/column position
- [ ] `streamforge validate` CLI works
- [ ] Zero performance regression
- [ ] Backward compatible with 0.x configs
- [ ] Documentation updated (DSL_SPEC.md)

---

**Next Steps:** Implement Step 1-8 sequentially
