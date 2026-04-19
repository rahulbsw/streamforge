# DSL v2.0: Function-Style Syntax Proposal

**Status:** 📋 Proposal (for v1.1 or v2.0)  
**Created:** 2026-04-18  
**Rationale:** Improve readability for complex expressions while maintaining backward compatibility

---

## Problem Statement

The current colon-delimited DSL is compact but hard to read for complex expressions:

```yaml
# Current - hard to parse visually
filter: "OR:AND:/status,==,active:/tier,==,premium:AND:/status,==,trial:/days_left,>,7"

# Desired - function-style
filter: "or(
  and(field('/status') == 'active', field('/tier') == 'premium'),
  and(field('/status') == 'trial', field('/days_left') > 7)
)"
```

**User feedback:** "Can the DSL be like simple function or pseudo code instead of colon or comma?"

---

## Proposed Syntax

### Filters

```javascript
// Boolean logic
and(expr1, expr2, ...)
or(expr1, expr2, ...)
not(expr)

// Field comparisons
field('/path') == 'value'
field('/path') != 'value'
field('/count') > 100
field('/count') >= 100
field('/count') < 50
field('/count') <= 50

// Regex
regex(field('/email'), '^[a-z]+@example\\.com$')
field('/email').matches('^[a-z]+@')

// Array operations
array_any('/items', item => item.price > 100)
array_all('/tags', tag => tag.active == true)
array_contains('/roles', 'admin')
array_length('/items') > 5

// Key operations
key_prefix('user-')
key_matches('order-[0-9]+')

// Header operations
header('Content-Type') == 'application/json'

// Timestamp operations
timestamp_age() > 3600  // seconds
timestamp_after(1640000000000)  // epoch ms
timestamp_before(1640000000000)

// Existence checks
exists('/optional_field')
not_exists('/deleted_at')

// Null/empty checks
is_null(field('/value'))
is_not_null(field('/value'))
is_empty(field('/string'))        // empty string or empty array
is_not_empty(field('/string'))
is_blank(field('/text'))           // null, empty, or whitespace-only

// String functions (in filters)
length(field('/name')) > 10
field('/name').length() > 10
starts_with(field('/email'), 'admin')
ends_with(field('/file'), '.json')
contains(field('/text'), 'error')
```

### Transforms

```javascript
// Simple extraction
field('/user/id')

// Extract with target
extract(path: '/user/name', target: 'username', default: 'unknown')

// Object construction
construct(
  id: field('/user/id'),
  name: field('/user/name'),
  email: field('/user/email')
)

// Hashing
hash(algo: 'SHA256', path: '/user/email', target: 'email_hash')
field('/user/email').hash('MD5')

// String operations
uppercase(field('/name'))
lowercase(field('/email'))
trim(field('/text'))
trim_start(field('/text'))
trim_end(field('/text'))

// Or method-style
field('/name').uppercase()
field('/email').lowercase()
field('/text').trim()

// String manipulation
length(field('/name'))
field('/name').length()

substring(field('/text'), start: 0, end: 10)
field('/text').substring(0, 10)

split(field('/csv'), delimiter: ',')
field('/csv').split(',')

join(field('/array'), separator: ', ')
field('/array').join(', ')

concat(field('/first'), ' ', field('/last'))
field('/first') + ' ' + field('/last')

replace(field('/text'), pattern: 'old', replacement: 'new')
field('/text').replace('old', 'new')

pad_left(field('/id'), width: 8, char: '0')   // "00000123"
pad_right(field('/name'), width: 20, char: ' ')

// String checks/conversions
to_string(field('/number'))
to_int(field('/string'))
to_float(field('/string'))

// Array operations
array_map('/items', item => item.id, target: 'item_ids')
array_filter('/items', item => item.active == true)

// Arithmetic
field('/count') + 1
field('/value') - 10
field('/price') * 1.2
field('/total') / 2

// Or function-style
add(field('/count'), 1)
subtract(field('/value'), 10)
multiply(field('/price'), 1.2)
divide(field('/total'), 2)

// Coalesce
coalesce(field('/primary'), field('/secondary'), 'default')
field('/primary') ?? field('/secondary') ?? 'default'

// Date/Time operations
now()                                          // Current timestamp (epoch ms)
now_iso()                                      // Current timestamp (ISO 8601)

// Parse date strings
parse_date(field('/date'), format: '%Y-%m-%d')
parse_date(field('/iso_date'))                 // Auto-detect ISO 8601
from_epoch(field('/timestamp'))                // Epoch ms to ISO 8601
from_epoch_seconds(field('/timestamp'))        // Epoch seconds to ISO 8601

// Format dates
format_date(field('/date'), format: '%Y-%m-%d %H:%M:%S')
to_epoch(field('/iso_date'))                   // ISO 8601 to epoch ms
to_epoch_seconds(field('/iso_date'))           // ISO 8601 to epoch seconds
to_iso(field('/date'))                         // Any date to ISO 8601

// Date arithmetic
add_days(field('/date'), 7)
add_hours(field('/date'), 24)
add_minutes(field('/date'), 30)
subtract_days(field('/date'), 1)

// Date extraction
year(field('/date'))
month(field('/date'))
day(field('/date'))
hour(field('/date'))
minute(field('/date'))
second(field('/date'))
day_of_week(field('/date'))                    // 0-6 (Sunday=0)
day_of_year(field('/date'))                    // 1-366

// Date comparison (in filters)
date_diff(field('/start'), field('/end'), unit: 'days')
is_before(field('/date'), '2024-01-01')
is_after(field('/date'), '2023-01-01')
is_between(field('/date'), start: '2023-01-01', end: '2024-01-01')
```

---

## Implementation Options

### Option 1: Full Expression Parser (Recommended)

**Approach:**
- Build lexer + tokenizer
- Recursive descent parser with operator precedence
- Reuse existing AST (FilterExpr, TransformExpr)
- New entry point: `parse_filter_expr_v2(input: &str)`

**Pros:**
- Most flexible
- Natural syntax for users
- Full control over error messages
- Can add features incrementally

**Cons:**
- ~500-800 lines of new parser code
- More complex than current parser
- Estimated effort: 6-8 hours

**Libraries:**
- `nom` - parser combinator library
- `pest` - PEG parser generator
- Hand-written recursive descent (simplest)

### Option 2: Embed Scripting Engine

**Approach:**
- Use Rhai scripting engine (Rust-native)
- Register StreamForge functions (field, array_any, etc.)
- Let Rhai handle parsing

**Pros:**
- Full scripting capability (loops, variables, functions)
- Battle-tested parser
- ~200 lines of integration code
- Users can define custom functions

**Cons:**
- Heavy dependency (~200KB compiled)
- Overkill for simple filters
- Harder to sandbox
- More attack surface

**Example:**
```rust
// Cargo.toml
rhai = "1.15"

// Integration
let engine = Engine::new();
engine.register_fn("field", |path: &str| { /* ... */ });
engine.register_fn("and", |a: bool, b: bool| a && b);
let result: bool = engine.eval(filter_expr)?;
```

### Option 3: Hybrid Approach

**Approach:**
- Support both syntaxes
- Auto-detect based on first character
- Colon-delimited: `AND:...` (starts with operator keyword)
- Function-style: `and(...)` (starts with `a-z` + `(`)

**Pros:**
- Backward compatible
- Users can choose
- Gradual migration

**Cons:**
- Two parsers to maintain
- More complex codebase

---

## Migration Strategy

### Phase 1: Add Function-Style Parser (v1.1)

1. Implement `src/dsl/parser_v2.rs`
2. Add auto-detection in `parse_filter_expr()`
3. 100+ test cases for new syntax
4. Update docs with examples

**Estimated effort:** 8-10 hours

### Phase 2: Deprecate Old Syntax (v2.0)

1. Emit warnings for colon-delimited syntax
2. Provide migration tool: `streamforge migrate-config old.yaml > new.yaml`
3. Update all examples to new syntax

**Estimated effort:** 4-6 hours

### Phase 3: Remove Old Parser (v3.0)

1. Delete old parser code
2. Simplify codebase

---

## Example Migration

### Before (v1.0 - Colon-delimited)

```yaml
appid: "user-filtering"
bootstrap: "kafka:9092"
input: "user-events"

routing:
  routing_type: "filter"
  destinations:
    - output: "active-users"
      filter: "AND:/status,==,active:/profile/complete,==,true"
      transform: "CONSTRUCT:id=/user/id:name=/user/name"
      
    - output: "premium-users"
      filter: "ARRAY_ANY:/subscriptions,/tier,==,premium"
      transform: "EXTRACT:/user/email,email"
```

### After (v2.0 - Function-style)

```yaml
appid: "user-filtering"
bootstrap: "kafka:9092"
input: "user-events"

routing:
  routing_type: "filter"
  destinations:
    - output: "active-users"
      filter: "and(field('/status') == 'active', field('/profile/complete') == true)"
      transform: "construct(id: field('/user/id'), name: field('/user/name'))"
      
    - output: "premium-users"
      filter: "array_any('/subscriptions', sub => sub.tier == 'premium')"
      transform: "extract(path: '/user/email', target: 'email')"
```

---

## Grammar (EBNF)

```ebnf
filter_expr = logical_expr | comparison_expr | function_call ;

logical_expr = "and" "(" filter_list ")"
             | "or" "(" filter_list ")"
             | "not" "(" filter_expr ")" ;

filter_list = filter_expr { "," filter_expr } ;

comparison_expr = field_access operator literal ;

operator = "==" | "!=" | ">" | ">=" | "<" | "<=" ;

function_call = identifier "(" argument_list? ")" ;

field_access = "field" "(" string_literal ")"
             | identifier "." identifier ;

argument_list = argument { "," argument } ;

argument = named_argument | positional_argument ;

named_argument = identifier ":" expression ;

positional_argument = expression ;

expression = filter_expr | transform_expr | literal | field_access ;

literal = string_literal | number_literal | boolean_literal ;

string_literal = "'" { character } "'" | '"' { character } '"' ;

number_literal = [ "-" ] digit { digit } [ "." digit { digit } ] ;

boolean_literal = "true" | "false" ;

identifier = letter { letter | digit | "_" } ;
```

---

## Parser Implementation Sketch

```rust
// src/dsl/parser_v2.rs

use logos::Logos;  // Lexer generator

#[derive(Logos, Debug, PartialEq)]
enum Token {
    #[token("and")]
    And,
    
    #[token("or")]
    Or,
    
    #[token("field")]
    Field,
    
    #[token("(")]
    LParen,
    
    #[token(")")]
    RParen,
    
    #[token(",")]
    Comma,
    
    #[token("==")]
    Eq,
    
    #[regex(r"'[^']*'", |lex| lex.slice()[1..lex.slice().len()-1].to_string())]
    String(String),
    
    #[regex(r"-?[0-9]+(\.[0-9]+)?", |lex| lex.slice().parse())]
    Number(f64),
    
    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn parse_filter(&mut self) -> Result<FilterExpr> {
        match self.current()? {
            Token::And => self.parse_and_filter(),
            Token::Or => self.parse_or_filter(),
            Token::Field => self.parse_comparison(),
            _ => Err(ParseError::unexpected_token(self.pos)),
        }
    }
    
    fn parse_and_filter(&mut self) -> Result<FilterExpr> {
        self.expect(Token::And)?;
        self.expect(Token::LParen)?;
        
        let mut exprs = vec![];
        loop {
            exprs.push(self.parse_filter()?);
            
            if self.current()? == &Token::RParen {
                break;
            }
            self.expect(Token::Comma)?;
        }
        
        self.expect(Token::RParen)?;
        Ok(FilterExpr::And(exprs))
    }
    
    // ... more parsing methods
}

pub fn parse_filter_expr_v2(input: &str) -> Result<Node<FilterExpr>> {
    let lexer = Token::lexer(input);
    let tokens: Vec<Token> = lexer.collect();
    
    let mut parser = Parser { tokens, pos: 0 };
    parser.parse_filter()
}
```

---

## Auto-Detection Logic

```rust
// src/dsl/parser.rs

pub fn parse_filter_expr(input: &str) -> Result<Node<FilterExpr>> {
    // Auto-detect syntax version
    let trimmed = input.trim();
    
    if trimmed.starts_with("and(") || 
       trimmed.starts_with("or(") ||
       trimmed.starts_with("not(") ||
       trimmed.starts_with("field(") {
        // Function-style syntax (v2)
        parser_v2::parse_filter_expr_v2(input)
    } else {
        // Colon-delimited syntax (v1)
        parser_v1::parse_filter_expr_v1(input)
    }
}
```

---

## Testing Strategy

```rust
#[test]
fn test_function_style_and() {
    let input = "and(field('/status') == 'active', field('/count') > 10)";
    let result = parse_filter_expr(input);
    assert!(result.is_ok());
    
    match &result.unwrap().value {
        FilterExpr::And(exprs) => assert_eq!(exprs.len(), 2),
        _ => panic!("Expected AND filter"),
    }
}

#[test]
fn test_backward_compat_colon_delimited() {
    let input = "AND:/status,==,active:/count,>,10";
    let result = parse_filter_expr(input);
    assert!(result.is_ok());
}

#[test]
fn test_method_chaining() {
    let input = "field('/email').lowercase().matches('^[a-z]+@')";
    let result = parse_filter_expr(input);
    assert!(result.is_ok());
}
```

---

## Performance Impact

**Parsing time (estimated):**
- Current syntax: ~100ns per expression
- Function-style: ~500ns-1µs per expression

**Impact:** Negligible. Parsing only happens at config load (once at startup).

For 1000 filters:
- Current: 100µs
- Function-style: 500µs-1ms

Both are unnoticeable during startup.

---

## Risks & Mitigation

**Risk 1: Breaking changes**  
**Mitigation:** Support both syntaxes, auto-detect, gradual deprecation

**Risk 2: Parser bugs**  
**Mitigation:** 100+ test cases, fuzzing, extensive validation

**Risk 3: User confusion (two syntaxes)**  
**Mitigation:** Clear docs, examples, migration guide, lint warnings

**Risk 4: Increased maintenance burden**  
**Mitigation:** Eventually deprecate old syntax in v3.0

---

## Decision

**Recommendation:** **Option 1 (Full Expression Parser)** for v1.1

**Rationale:**
1. Best user experience (readable, familiar)
2. Full control over syntax and error messages
3. No heavy dependencies
4. Natural evolution of current AST-based parser
5. Can be implemented incrementally

**Timeline:**
- v1.1 (Q3 2026): Add function-style parser with auto-detection
- v2.0 (Q1 2027): Deprecate colon-delimited syntax
- v3.0 (Q3 2027): Remove old syntax

---

## Appendix: Real-World Example

### Current Syntax (v1.0)

```yaml
# PII redaction config
routing:
  destinations:
    - output: "analytics"
      filter: "AND:EXISTS:/user/id:/consent/analytics,==,true"
      transform: "CONSTRUCT:user_hash=HASH:SHA256,/user/id:event=/event_type:region=/region"
      
    - output: "marketing"
      filter: "AND:EXISTS:/email:/consent/marketing,==,true:NOT:/email,==,null"
      transform: "CONSTRUCT:email_hash=HASH:MD5,/email:campaign=/campaign_id"
```

### Function-Style Syntax (v2.0)

```yaml
# PII redaction config
routing:
  destinations:
    - output: "analytics"
      filter: |
        and(
          exists('/user/id'),
          field('/consent/analytics') == true
        )
      transform: |
        construct(
          user_hash: hash('SHA256', '/user/id'),
          event: field('/event_type'),
          region: field('/region')
        )
      
    - output: "marketing"
      filter: |
        and(
          exists('/email'),
          field('/consent/marketing') == true,
          not(field('/email') == null)
        )
      transform: |
        construct(
          email_hash: hash('MD5', '/email'),
          campaign: field('/campaign_id')
        )
```

**Improvement:** Much easier to understand intent at a glance.

---

## Next Steps

1. Get user feedback on proposed syntax
2. Prototype lexer + parser (~4 hours)
3. Implement full parser with tests (~6 hours)
4. Update documentation with examples (~2 hours)
5. Add migration tool (~2 hours)

**Total effort:** ~14-16 hours for full implementation in v1.1

---

**Author:** StreamForge Team  
**Date:** 2026-04-18  
**Version:** 1.0 (Proposal)
