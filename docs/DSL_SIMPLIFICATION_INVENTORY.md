# DSL Simplification Inventory (Phase 0)

**Version:** 1.0.0-alpha.1  
**Created:** 2026-04-18  
**Purpose:** Phase 0 inventory of DSL features to keep/simplify/deprecate/remove for v1.0

---

## Executive Summary

The StreamForge DSL is a custom string-based colon-delimited syntax for filters and transforms. This inventory analyzes the current DSL implementation (~1800 lines in `src/filter_parser.rs`) to identify simplification opportunities for v1.0 stabilization.

**Key findings:**
- ✅ **Keep:** 90% of current DSL features are essential
- 🔨 **Simplify:** 5-10% can be simplified or unified
- ⚠️ **Deprecate:** <1% should be deprecated with migration path
- ❌ **Remove:** None identified (all features in use)

---

## Current DSL Surface (v0.4.0)

### Filter Operations (20+ types)

| Operator | Syntax | Status | Notes |
|----------|--------|--------|-------|
| **Comparison** | `/path,op,value` | ✅ Keep | Core feature |
| **Boolean AND** | `AND:cond1:cond2` | ✅ Keep | Essential |
| **Boolean OR** | `OR:cond1:cond2` | ✅ Keep | Essential |
| **Boolean NOT** | `NOT:cond` | ✅ Keep | Essential |
| **Regex** | `REGEX:/path,pattern` | ✅ Keep | Common use case |
| **Array All** | `ARRAY_ALL:/path,filter` | ✅ Keep | Critical for arrays |
| **Array Any** | `ARRAY_ANY:/path,filter` | ✅ Keep | Critical for arrays |
| **Key Prefix** | `KEY_PREFIX:prefix` | ✅ Keep | Envelope feature |
| **Key Suffix** | `KEY_SUFFIX:suffix` | 🔨 Simplify | Rarely used, consider removing |
| **Key Contains** | `KEY_CONTAINS:substring` | 🔨 Simplify | Can use REGEX instead |
| **Key Matches** | `KEY_MATCHES:regex` | ✅ Keep | Essential for key filtering |
| **Key Exists** | `KEY_EXISTS` | ✅ Keep | Useful for null checks |
| **Header** | `HEADER:name,op,value` | ✅ Keep | Envelope feature |
| **Header Exists** | `HEADER_EXISTS:name` | ✅ Keep | Common use case |
| **Timestamp Age** | `TIMESTAMP_AGE:op,seconds` | ✅ Keep | Time-based routing |

**Comparison operators:** `>`, `>=`, `<`, `<=`, `==`, `!=`

### Transform Operations (10+ types)

| Operator | Syntax | Status | Notes |
|----------|--------|--------|-------|
| **Extract** | `/path` or `EXTRACT:/path,output` | ✅ Keep | Most common |
| **Construct** | `CONSTRUCT:f1=/p1:f2=/p2` | ✅ Keep | Object building |
| **Array Map** | `ARRAY_MAP:/array,/element,output` | ✅ Keep | Array operations |
| **Arithmetic** | `ADD:/left,/right,output` | ✅ Keep | Math operations |
| **Hash** | `HASH:algo,/path,output` | ✅ Keep | PII redaction |
| **Cache Lookup** | `CACHE_LOOKUP:/key,store,/dest` | ✅ Keep | Enrichment |
| **Cache Put** | `CACHE_PUT:/key,store,/value` | ✅ Keep | Caching |
| **String Upper** | `UPPER:/path,output` | ⚠️ Plan | Not yet implemented |
| **String Lower** | `LOWER:/path,output` | ⚠️ Plan | Not yet implemented |
| **Substring** | `SUBSTRING:/path,start,end,output` | ⚠️ Plan | Not yet implemented |

**Arithmetic operators:** `ADD`, `SUB`, `MUL`, `DIV`  
**Hash algorithms:** `MD5`, `SHA256`, `SHA512`, `MURMUR64`, `MURMUR128`

### Envelope Operations

| Operation | Syntax | Status | Notes |
|-----------|--------|--------|-------|
| **Key Transform** | `key_transform: "/path"` | ✅ Keep | Envelope feature |
| **Header Add** | `headers: {name: value}` | ✅ Keep | Envelope feature |
| **Header Transform** | `FROM:/path` | ✅ Keep | Envelope feature |
| **Timestamp Control** | `PRESERVE`, `CURRENT` | ✅ Keep | Envelope feature |

---

## Simplification Opportunities

### 1. Key Filter Unification

**Current:**
```yaml
# Multiple ways to filter keys
KEY_PREFIX:user-
KEY_SUFFIX:-prod
KEY_CONTAINS:prod
KEY_MATCHES:^user-.*-prod$
```

**Proposed v1.0:**
```yaml
# Simplify to two operators
KEY_PREFIX:user-         # Keep (common)
KEY_MATCHES:^user-.*-prod$  # Keep (flexible)
# Remove: KEY_SUFFIX, KEY_CONTAINS (use REGEX)
```

**Rationale:**
- `KEY_SUFFIX` and `KEY_CONTAINS` are rare
- `KEY_MATCHES` (regex) can cover all cases
- Reduces parser complexity
- Migration: `KEY_CONTAINS:foo` → `KEY_MATCHES:.*foo.*`

**Impact:** ~100 lines removed from parser

### 2. Extract Syntax Normalization

**Current:**
```yaml
# Two syntaxes for same operation
transform: "/user/email"           # Shorthand
transform: "EXTRACT:/user/email"   # Explicit
```

**Proposed v1.0:**
```yaml
# Keep both, document when to use each
"/path"              # For simple extraction (recommended)
"EXTRACT:/path,out"  # For renaming or multi-extract
```

**Rationale:**
- Both have clear use cases
- Shorthand is more readable
- Explicit is clearer for complex pipelines
- No change needed, just document

**Impact:** None (documentation only)

### 3. Arithmetic Operator Consistency

**Current:**
```yaml
ADD:/price,/tax,total
SUB:/amount,/discount,final
MUL:/quantity,/price,cost
DIV:/total,/count,average
```

**Analysis:** ✅ Keep as-is

**Rationale:**
- Consistent syntax across all operators
- Clear intent
- No simpler alternative

**Impact:** None

---

## Features to Keep (No Changes)

### Core Filters
- Comparison operators (`>`, `>=`, `<`, `<=`, `==`, `!=`)
- Boolean logic (`AND`, `OR`, `NOT`)
- Regex (`REGEX:/path,pattern`)
- Array operations (`ARRAY_ALL`, `ARRAY_ANY`)

**Why:** These are fundamental to the DSL value proposition.

### Core Transforms
- Field extraction (`/path`)
- Object construction (`CONSTRUCT`)
- Array mapping (`ARRAY_MAP`)
- Hashing (`HASH`)
- Cache operations (`CACHE_LOOKUP`, `CACHE_PUT`)

**Why:** Common use cases validated by user feedback and examples.

### Envelope Operations
- Key/header/timestamp manipulation
- Critical for Kafka-native operations
- No simpler alternative

**Why:** Unique StreamForge features not available in other tools.

---

## Features to Simplify (v1.0 Phase 2)

### 1. Remove `KEY_SUFFIX` and `KEY_CONTAINS`

**Migration guide:**
```yaml
# Before (v0.4.0)
filter: "KEY_SUFFIX:-prod"
filter: "KEY_CONTAINS:staging"

# After (v1.0.0)
filter: "KEY_MATCHES:.*-prod$"
filter: "KEY_MATCHES:.*staging.*"
```

**Timeline:**
- v1.0.0-alpha.2: Deprecation warning
- v1.0.0-beta.1: Remove from parser
- v1.0.0: Fully removed

### 2. Formalize EXTRACT Shorthand

**Documentation:**
```yaml
# Recommended for simple extraction
transform: "/user/email"

# Use when renaming
transform: "EXTRACT:/user/email,userEmail"

# NOT recommended (verbose)
transform: "EXTRACT:/user/email"  # Same as "/user/email"
```

**Timeline:**
- v1.0.0: Document in DSL_SPEC.md

---

## Features to Deprecate (Future)

### None identified for v1.0

All current features are in active use. Future deprecations will follow a 3-release cycle:
1. Deprecation warning
2. Removal from docs
3. Removal from code

---

## Features NOT to Add (Scope Control)

### ❌ General-Purpose Scripting
- No: `if/else`, `for` loops, variables
- Why: Use Rhai/Lua/WASM for complex logic (future plugin system)

### ❌ External Service Calls
- No: HTTP requests, database queries
- Why: Use cache backends instead (Redis, Kafka-backed)

### ❌ Complex Type System
- No: Type annotations, generics
- Why: JSON path-based, runtime typing

### ❌ Custom Functions
- No: User-defined functions (UDF)
- Why: Planned for v1.1+ via WASM plugins

---

## Parser Complexity Analysis

### Current Implementation (v0.4.0)

**File:** `src/filter_parser.rs` (~1800 lines)

**Breakdown:**
- Filter parsing: ~800 lines
- Transform parsing: ~700 lines
- Helper functions: ~200 lines
- Tests: ~100 lines

**Complexity drivers:**
1. String parsing with colon delimiters
2. Nested boolean logic (`AND`, `OR`, `NOT`)
3. Recursive array operations
4. Arithmetic expression parsing

### v1.0 Parser Refactor (Phase 2)

**Planned structure:**
```
src/dsl/
├── mod.rs           # Public API
├── ast.rs           # AST types (~300 lines)
├── parser.rs        # String → AST (~600 lines)
├── validator.rs     # AST validation (~400 lines)
└── evaluator.rs     # AST → closure (~500 lines)
```

**Benefits:**
- Separate parsing from evaluation
- Type-checked validation pass
- Better error messages (with AST context)
- Testable at each layer

**Timeline:** Phase 2 (after Phase 1 core hardening)

---

## Grammar Formalization Plan

### Current (Informal)

Documentation uses examples:
```yaml
# Example: filter active users
filter: "/status,==,active"
```

### v1.0 Target (EBNF Grammar)

**Planned file:** `docs/DSL_SPEC.md`

**Sample grammar:**
```ebnf
filter     ::= simple | boolean | array | envelope
simple     ::= PATH "," OP "," VALUE
boolean    ::= "AND:" filter ":" filter
            |  "OR:" filter ":" filter
            |  "NOT:" filter
OP         ::= ">" | ">=" | "<" | "<=" | "==" | "!="
PATH       ::= "/" IDENTIFIER ("/" IDENTIFIER)*
```

**Timeline:** Phase 2

---

## Backward Compatibility Promise (v1.0)

### What's Stable

✅ **Filter operators:** `/`, `AND:`, `OR:`, `NOT:`, `REGEX:`, `ARRAY_ALL:`, `ARRAY_ANY:`  
✅ **Transform operators:** `/`, `EXTRACT:`, `CONSTRUCT:`, `ARRAY_MAP:`, `HASH:`, `CACHE_LOOKUP:`  
✅ **Comparison operators:** `>`, `>=`, `<`, `<=`, `==`, `!=`  
✅ **Envelope operations:** `key_transform`, `headers`, `header_transforms`, `timestamp`

**Promise:** No breaking changes to these operators in v1.x.

### What May Change

⚠️ **Deprecated operators:** `KEY_SUFFIX:`, `KEY_CONTAINS:` (removed in v1.0)  
⚠️ **Error messages:** Format and content (improved in Phase 2)  
⚠️ **Parser internals:** AST representation (transparent to users)

**Promise:** 3-release deprecation cycle with migration guide.

---

## Testing Strategy for DSL Changes

### Current Test Coverage (v0.4.0)

- Unit tests: ~100 test cases in `filter_parser.rs`
- Benchmark tests: 30+ benchmarks in `benches/`
- Integration tests: None (Phase 1 gap)

### v1.0 Test Requirements

**Phase 1: Add Integration Tests**
- End-to-end DSL evaluation
- Error handling scenarios
- Edge cases (empty strings, unicode, special chars)

**Phase 2: Golden Tests**
- Input DSL → AST snapshots
- AST → Evaluation snapshots
- Prevents unintended changes

**Phase 3: Fuzzing**
- Random DSL generation
- Parser crash detection
- Validate all inputs handled gracefully

---

## Summary: DSL Simplification Actions

### Immediate (Phase 0) ✅
- [x] Document current DSL surface
- [x] Identify simplification opportunities
- [x] Create migration plan for deprecations

### Phase 2 (DSL Stabilization)
- [ ] Write EBNF grammar (`docs/DSL_SPEC.md`)
- [ ] Refactor parser to separate AST layer
- [ ] Add validation pass with type checking
- [ ] Implement CLI validation tool
- [ ] Remove `KEY_SUFFIX` and `KEY_CONTAINS`
- [ ] Write migration guide

### Phase 6 (v1.0 Release)
- [ ] Freeze stable operator set
- [ ] Document backward compatibility promise
- [ ] Deprecation policy for future changes

---

## References

- **V1_PLAN.md**: Phase 2 (DSL Stabilization) details
- **PROJECT_SPEC.md**: DSL philosophy and scope
- **src/filter_parser.rs**: Current implementation
- **docs/ADVANCED_DSL_GUIDE.md**: User-facing DSL documentation

---

**Status:** Phase 0 complete  
**Next:** Phase 1 (Core Engine Hardening)
