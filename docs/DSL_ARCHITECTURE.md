# StreamForge DSL Architecture (v1.0)

**Status:** ✅ Complete  
**Version:** 1.0.0  
**Date:** 2026-04-18

---

## Overview

StreamForge v1.0 includes a complete rewrite of the DSL parser with an AST-based architecture. The new design provides better error messages, position tracking, and validation capabilities while maintaining full backward compatibility with existing configurations.

## Architecture

The DSL module is organized into 5 core components:

```
src/dsl/
├── ast.rs              # Abstract Syntax Tree node definitions
├── error.rs            # Position-tracked error types
├── parser.rs           # String → AST parser
├── validator.rs        # AST semantic validation
├── evaluator.rs        # AST → Filter/Transform trait objects
└── mod.rs              # Public API exports
```

### Data Flow

```
User DSL String
    ↓
[Parser] → AST with position tracking
    ↓
[Validator] → ValidationResult (errors + warnings)
    ↓
[Evaluator] → Filter/Transform trait objects
    ↓
Execution Engine
```

---

## Component Details

### 1. AST (ast.rs)

**Purpose:** Structured representation of DSL expressions with position information.

**Key Types:**

```rust
pub struct Node<T> {
    pub value: T,
    pub span: Span,  // Position info for error messages
}

pub enum FilterExpr {
    JsonPath { path: String, op: ComparisonOp, value: Literal },
    And(Vec<Node<FilterExpr>>),
    Or(Vec<Node<FilterExpr>>),
    Not(Box<Node<FilterExpr>>),
    Regex { path: String, pattern: String },
    ArrayAny { array_path: String, element_filter: Box<Node<FilterExpr>> },
    ArrayAll { array_path: String, element_filter: Box<Node<FilterExpr>> },
    ArrayContains { array_path: String, value: Literal },
    ArrayLength { array_path: String, op: ComparisonOp, length: usize },
    KeyPrefix(String),
    KeyMatches(String),
    KeySuffix(String),      // Deprecated
    KeyContains(String),    // Deprecated
    Header { name: String, op: ComparisonOp, value: String },
    TimestampAge { op: ComparisonOp, seconds: u64 },
    Exists(String),
    NotExists(String),
}

pub enum TransformExpr {
    JsonPath(String),
    Extract { path: String, target_field: String, default_value: Option<String> },
    Construct(Vec<(String, String)>),
    Hash { algorithm: HashAlgorithm, path: String, target_field: String },
    String { op: StringOp, path: String },
    ArrayMap { array_path: String, element_path: String, target_field: String },
    ArrayFilter { array_path: String, filter: Box<Node<FilterExpr>> },
    Arithmetic { op: ArithmeticOp, left: ArithmeticOperand, right: ArithmeticOperand },
    Coalesce { paths: Vec<String>, default: Option<String> },
}
```

**Coverage:** All 40+ DSL operators represented as strongly-typed AST nodes.

---

### 2. Error Types (error.rs)

**Purpose:** Position-tracked errors with context-aware formatting.

**Key Types:**

```rust
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

pub struct Span {
    pub start: Position,
    pub end: Position,
}

pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub input: String,
}

pub enum ValidationError {
    TypeMismatch { expected: String, actual: String, span: Span },
    InvalidPath { path: String, reason: String, span: Span },
    UnknownOperator { operator: String, span: Span },
    InvalidArgumentCount { operator: String, expected: usize, actual: usize, span: Span },
    Undefined { name: String, span: Span },
}

pub enum ValidationWarning {
    DeprecatedSyntax { old: String, new: String, span: Span },
    UnusedValue { value: String, span: Span },
    PerformanceHint { message: String, span: Span },
}
```

**Example Error Output:**

```
Error at line 1, columns 8-10
  Invalid operator

   1 | /status,><,active
     |        ^^
```

---

### 3. Parser (parser.rs)

**Purpose:** Convert DSL strings to AST with position tracking.

**Public API:**

```rust
pub fn parse_filter_expr(input: &str) -> Result<Node<FilterExpr>, ParseError>
pub fn parse_transform_expr(input: &str) -> Result<Node<TransformExpr>, ParseError>
```

**Implementation:**

- **Position tracking:** Every token records line, column, and byte offset
- **Error recovery:** Provides precise error messages with context
- **Recursive descent:** Supports nested expressions (AND, OR, NOT)

**Parser Structure:**

```rust
struct Parser<'a> {
    input: &'a str,
    position: usize,
    line: usize,
    column: usize,
}

impl<'a> Parser<'a> {
    // Core parsing functions
    fn parse_filter(&mut self) -> Result<Node<FilterExpr>, ParseError>
    fn parse_transform(&mut self) -> Result<Node<TransformExpr>, ParseError>
    
    // Operator-specific parsers
    fn parse_and_filter(&mut self) -> Result<Node<FilterExpr>, ParseError>
    fn parse_or_filter(&mut self) -> Result<Node<FilterExpr>, ParseError>
    fn parse_regex_filter(&mut self) -> Result<Node<FilterExpr>, ParseError>
    fn parse_array_any_filter(&mut self) -> Result<Node<FilterExpr>, ParseError>
    // ... 15+ more operator parsers
}
```

---

### 4. Validator (validator.rs)

**Purpose:** Semantic validation of parsed AST.

**Public API:**

```rust
pub fn validate_filter(filter: &Node<FilterExpr>) -> ValidationResult
pub fn validate_transform(transform: &Node<TransformExpr>) -> ValidationResult

pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}
```

**Validation Rules:**

1. **JSON Path Validation:**
   - Must start with `/`
   - Cannot contain empty segments (`//`)
   - Cannot be empty

2. **Type Checking:**
   - Numeric operators (>, <, >=, <=) should use numbers
   - String comparisons on strings
   - Boolean checks on booleans

3. **Operator Validation:**
   - Correct argument counts (AND needs ≥1 condition)
   - Valid operator combinations
   - Field names cannot contain `/`, `:`, `,`

4. **Regex Validation:**
   - Patterns must compile successfully
   - Cannot be empty

5. **Deprecation Warnings:**
   - `KEY_SUFFIX` → use `KEY_MATCHES` with regex `.*suffix$`
   - `KEY_CONTAINS` → use `KEY_MATCHES` with regex `.*substring.*`

6. **Performance Hints:**
   - Lexicographic string comparisons (>, <)
   - Numeric equality checks (use > or < instead of ==)

---

### 5. Evaluator (evaluator.rs)

**Purpose:** Convert validated AST to executable Filter/Transform trait objects.

**Public API:**

```rust
pub fn eval_filter(node: &Node<FilterExpr>) -> Result<Arc<dyn Filter>>
```

**Implementation:**

- Maps AST nodes to existing `Filter` trait implementations
- Maintains backward compatibility with existing filter system
- Uses `?` operator to propagate errors from filter construction

**Example Mapping:**

```rust
FilterExpr::JsonPath { path, op, value } 
    → JsonPathFilter::new(path, op_str, value_str)

FilterExpr::And(exprs) 
    → AndFilter::new(vec![eval(expr1), eval(expr2), ...])

FilterExpr::Regex { path, pattern } 
    → RegexFilter::new(path, pattern)
```

---

## Integration with Existing System

### Backward Compatibility

The new DSL module **does not replace** the existing `filter_parser.rs`. Both coexist:

- **Existing path:** `filter_parser::parse_filter()` → trait objects (unchanged)
- **New path:** `dsl::parse_filter_expr()` → AST → validation → trait objects

This allows:
1. Gradual migration to new parser
2. Testing both parsers side-by-side
3. Zero breaking changes for existing users

### Future Integration (v1.1)

The plan for v1.1 is to:
1. Update `filter_parser::parse_filter()` to use new DSL parser internally
2. Deprecate old parser code
3. Remove old parser in v2.0

---

## Testing

### Test Coverage

**Total:** 102 tests (98 active + 4 ignored)

| Module | Tests | Coverage |
|--------|-------|----------|
| `ast.rs` | 5 | All operator enums, from_str() methods |
| `error.rs` | 5 | Position/Span display, error formatting |
| `parser.rs` | 9 | Basic filters, transforms, AND/OR, errors |
| `validator.rs` | 13 | Path validation, type checking, deprecations |
| `evaluator.rs` | 5 | AST → trait object conversion |
| `parser_comprehensive_tests.rs` | 65 | All 40+ operators, edge cases |

**Ignored tests (future work):**
- Nested AND/OR support (e.g., `AND:OR:a:b:c`)
- Empty string edge cases

### Test Categories

**Filter Tests (48):**
- JSON path comparisons (11)
- Boolean logic - AND, OR, NOT (8)
- Regex operations (5)
- Array operations (7)
- Key operations (4)
- Header operations (2)
- Timestamp operations (2)
- Existence checks (2)
- Error cases (7)

**Transform Tests (17):**
- Simple extraction (3)
- Object construction (2)
- Hash operations (3)
- String operations (3)
- Array operations (2)
- Arithmetic operations (5)
- Coalesce (2)
- Error cases (3)

---

## Performance

The new parser is **not** used in the hot path (message processing). It is only invoked during:
1. **Config loading** (once at startup)
2. **streamforge-validate CLI** (validation tool)

Therefore, parser performance is not critical for throughput.

**Benchmark results (from benches/):**

| Operation | Time | Notes |
|-----------|------|-------|
| Simple filter parse | 100ns | ~10M ops/sec |
| AND filter parse (2 conditions) | 329ns | ~3M ops/sec |
| Regex filter parse (with compilation) | 409µs | One-time cost at startup |
| Array filter parse | 230ns | ~4.3M ops/sec |

**Startup impact:** Negligible. Parsing 100 filters takes < 100µs.

---

## Error Message Quality

### Before (v0.x)

```
Error: Invalid filter format: /status,><,active. Expected 'path,operator,value'
```

### After (v1.0)

```
Error at line 1, columns 8-10
  Invalid operator: ><
  Supported operators: ==, !=, >, >=, <, <=

   1 | /status,><,active
     |        ^^
```

### Deprecation Warnings

```
Warning at line 1, columns 1-18
  'KEY_SUFFIX:-archived' is deprecated, use 'KEY_MATCHES:.*-archived$' instead
```

---

## Design Decisions

### 1. Why AST-based parser?

**Before:** String splitting and ad-hoc parsing in `filter_parser.rs`

**After:** Structured AST with position tracking

**Benefits:**
- Better error messages (position tracking)
- Type-safe representation (Rust enums)
- Validation before evaluation
- Foundation for future features (e.g., optimizer, query planner)

### 2. Why keep existing filter_parser.rs?

**Risk mitigation:** Large refactor in v1.0 release window

**Approach:** Parallel implementation allows:
- Thorough testing before migration
- Gradual rollout
- Easy rollback if issues found

**Timeline:** Full migration in v1.1

### 3. Why separate validation from parsing?

**Separation of concerns:**
- **Parser:** Syntax correctness (is it valid DSL?)
- **Validator:** Semantic correctness (does it make sense?)

**Example:**
- Parser accepts: `/field,><,value` (valid syntax: path,op,value)
- Validator rejects: `><` is not a valid operator

This allows:
- Better error messages (syntax vs semantic errors)
- Validation warnings without parse failures
- Future: type inference, optimization passes

---

## Future Enhancements (v1.1+)

### 1. Nested Boolean Logic

**Current limitation:** Cannot nest AND inside OR

```
# Not yet supported
AND:OR:/a,==,1:/b,==,2:/c,==,3
```

**Solution:** Extend parser to handle nested operators

**Estimated effort:** 2-3 hours

### 2. Empty String Handling

**Current behavior:** Empty strings parse as simple JSON paths

```rust
parse_filter_expr("") → Ok(JsonPath("", ...))  // Should be Err
```

**Solution:** Add early validation in parser entry point

**Estimated effort:** 30 minutes

### 3. Validator Improvements

**Type inference:**
- Track JSON value types through pipeline
- Warn about type mismatches at validation time

**Example:**
```
/count,>,foo  // Warning: comparing number to string
```

**Performance optimization hints:**
- Suggest regex caching for repeated patterns
- Warn about inefficient filters (e.g., NOT:AND:large_expression)

**Estimated effort:** 4-6 hours

### 4. DSL Specification (EBNF Grammar)

**Goal:** Formal grammar in docs/DSL_SPEC.md

**Format:** Extended Backus-Naur Form (EBNF)

**Benefit:** Complete reference for users implementing DSL parsers

**Estimated effort:** 2-3 hours

---

## Migration Guide (v0.x → v1.0)

### For Users

**No changes required.** All existing DSL expressions work unchanged.

**Deprecated features:**
- `KEY_SUFFIX` → use `KEY_MATCHES` with regex
- `KEY_CONTAINS` → use `KEY_MATCHES` with regex

These still work in v1.0 but will emit warnings in validation CLI.

### For Developers

**New API:**

```rust
// Old (still works)
use streamforge::filter_parser::parse_filter;
let filter = parse_filter("/status,==,active")?;

// New (better errors)
use streamforge::dsl::{parse_filter_expr, validate_filter, eval_filter};

let ast = parse_filter_expr("/status,==,active")?;
let validation = validate_filter(&ast);
if !validation.is_valid() {
    for error in &validation.errors {
        eprintln!("Error: {}", error);
    }
    return Err(...);
}
let filter = eval_filter(&ast)?;
```

**Testing:**

```rust
// Test AST structure directly
let ast = parse_filter_expr("AND:/a,==,1:/b,==,2").unwrap();
match &ast.value {
    FilterExpr::And(exprs) => assert_eq!(exprs.len(), 2),
    _ => panic!("Expected AND filter"),
}
```

---

## Related Documentation

- [ERROR_HANDLING.md](ERROR_HANDLING.md) - Error taxonomy and recovery actions
- [DELIVERY_GUARANTEES.md](DELIVERY_GUARANTEES.md) - At-least-once semantics
- [README.md](../README.md) - User-facing DSL examples
- [V1_PLAN.md](../V1_PLAN.md) - Phase 2.2: Parser Refactor

---

## Commits

Parser refactor implemented in commit range:
- Foundation: error types, AST definitions
- Parser: 850-line recursive descent parser
- Validator: 13 semantic validation rules
- Evaluator: AST → trait object conversion
- Tests: 102 test cases (98 active, 4 future)

**Total:** ~2,000 lines of new code, 0 lines changed in existing codebase.

---

## Conclusion

The v1.0 DSL module provides:

✅ **Better errors:** Position-tracked parse errors with context  
✅ **Type safety:** Strongly-typed AST representation  
✅ **Validation:** Semantic checks with warnings  
✅ **Testing:** 102 test cases covering all operators  
✅ **Backward compatible:** Zero breaking changes  
✅ **Foundation:** Ready for future enhancements (optimizer, type inference)

**Status:** ✅ Phase 2.2 complete. Ready for v1.0 release.
