# StreamForge DSL Specification v1.0

**Version:** 1.0.0  
**Status:** Stable  
**Last Updated:** 2026-04-18

## Table of Contents

1. [Introduction](#introduction)
2. [Grammar (EBNF)](#grammar-ebnf)
3. [Filter Expressions](#filter-expressions)
4. [Transform Expressions](#transform-expressions)
5. [Envelope Operations](#envelope-operations)
6. [Type System](#type-system)
7. [Operator Precedence](#operator-precedence)
8. [Escaping and Quoting](#escaping-and-quoting)
9. [Error Handling](#error-handling)
10. [Examples](#examples)
11. [Migration from 0.x](#migration-from-0x)

---

## Introduction

The StreamForge DSL is a string-based domain-specific language for expressing message filters, transformations, and envelope manipulations in Kafka pipelines. It is designed for:

- **Expressiveness:** Support complex routing logic without code
- **Readability:** Human-readable syntax suitable for YAML configs
- **Type safety:** JSON Path validation at parse time
- **Performance:** Zero-copy paths where possible

### Design Principles

1. **Colon-delimited hierarchy:** Operations are separated by `:` for nesting
2. **Comma-separated parameters:** Within an operation, parameters use `,`
3. **JSON Path notation:** Field access uses `/` prefix (e.g., `/user/id`)
4. **Explicit over implicit:** Operation names are clear (e.g., `EXTRACT`, not just path)
5. **Composable:** Filters and transforms can be nested/chained

---

## Grammar (EBNF)

```ebnf
(* StreamForge DSL Grammar v1.0 *)

(* Top-level expressions *)
filter_expr     = simple_filter | composite_filter | envelope_filter ;
transform_expr  = path_extract | explicit_transform ;

(* Simple filters: path,op,value *)
simple_filter   = json_path "," comparison_op "," literal_value ;
comparison_op   = ">" | ">=" | "<" | "<=" | "==" | "!=" ;
literal_value   = string | number | boolean | "null" ;

(* Composite filters: boolean logic *)
composite_filter = and_filter | or_filter | not_filter | regex_filter | array_filter ;
and_filter       = "AND:" condition { ":" condition } ;
or_filter        = "OR:" condition { ":" condition } ;
not_filter       = "NOT:" condition ;
regex_filter     = "REGEX:" json_path "," regex_pattern ;
array_filter     = array_mode ":" json_path "," element_filter ;
array_mode       = "ARRAY_ALL" | "ARRAY_ANY" ;
element_filter   = simple_filter ;

(* Envelope filters: key, header, timestamp *)
envelope_filter  = key_filter | header_filter | timestamp_filter ;
key_filter       = key_prefix | key_matches | key_exists ;
key_prefix       = "KEY_PREFIX:" string_literal ;
key_matches      = "KEY_MATCHES:" regex_pattern ;
key_exists       = "KEY_EXISTS" ;
header_filter    = header_exists | header_cmp ;
header_exists    = "HEADER_EXISTS:" header_name ;
header_cmp       = "HEADER:" header_name "," comparison_op "," literal_value ;
timestamp_filter = timestamp_age | timestamp_after | timestamp_before ;
timestamp_age    = "TIMESTAMP_AGE:" comparison_op "," seconds ;
timestamp_after  = "TIMESTAMP_AFTER:" epoch_ms ;
timestamp_before = "TIMESTAMP_BEFORE:" epoch_ms ;

(* Path extraction - shorthand *)
path_extract     = json_path ;

(* Explicit transforms *)
explicit_transform = construct_transform
                   | array_map_transform
                   | arithmetic_transform
                   | hash_transform
                   | cache_transform
                   | string_transform ;

construct_transform = "CONSTRUCT:" field_mapping { ":" field_mapping } ;
field_mapping       = field_name "=" json_path ;

array_map_transform = "ARRAY_MAP:" json_path "," element_path "," output_field ;

arithmetic_transform = arithmetic_op ":" json_path "," json_path [ "," output_field ] ;
arithmetic_op        = "ADD" | "SUB" | "MUL" | "DIV" ;

hash_transform      = "HASH:" hash_algo "," json_path [ "," output_field ] ;
hash_algo           = "MD5" | "SHA256" | "SHA512" | "MURMUR64" | "MURMUR128" ;

cache_transform     = cache_lookup | cache_put ;
cache_lookup        = "CACHE_LOOKUP:" json_path "," store_name "," ( output_field | "MERGE" ) ;
cache_put           = "CACHE_PUT:" json_path "," store_name [ "," json_path ] ;

string_transform    = "STRING:" string_op "," string_params ;
string_op           = "UPPER" | "LOWER" | "TRIM" | "TRIM_START" | "TRIM_END" 
                    | "LENGTH" | "SUBSTRING" | "REPLACE" | "REPLACE_ALL" 
                    | "REGEX_REPLACE" | "SPLIT" | "CONCAT" ;
string_params       = json_path [ "," arg ] { "," arg } ;

(* Primitives *)
json_path       = "/" { identifier "/" } identifier ;
identifier      = letter { letter | digit | "_" } ;
string_literal  = ? any string without unescaped colons or commas ? ;
regex_pattern   = ? valid Rust regex ? ;
header_name     = identifier ;
store_name      = identifier ;
output_field    = identifier ;
field_name      = identifier ;
number          = [ "-" ] digit { digit } [ "." digit { digit } ] ;
boolean         = "true" | "false" ;
epoch_ms        = digit { digit } ;
seconds         = digit { digit } ;
condition       = filter_expr ;
arg             = string_literal | number ;

letter          = "a".."z" | "A".."Z" ;
digit           = "0".."9" ;
```

---

## Filter Expressions

### Simple Filters

**Syntax:** `<path>,<op>,<value>`

Compare a JSON field against a literal value.

**Comparison Operators:**
- `>`, `>=`, `<`, `<=` - Numeric comparison
- `==`, `!=` - Equality (works on strings, numbers, booleans, null)

**Examples:**
```yaml
# Numeric comparison
filter: "/user/age,>,18"

# String equality
filter: "/status,==,active"

# Null check
filter: "/optional_field,==,null"
```

### Boolean Logic

**AND:** All conditions must be true
```yaml
filter: "AND:/status,==,active:/user/age,>,18"
```

**OR:** At least one condition must be true
```yaml
filter: "OR:/tier,==,premium:/tier,==,enterprise"
```

**NOT:** Invert a condition
```yaml
filter: "NOT:/status,==,deleted"
```

**Nesting:** Combine boolean operators
```yaml
# (status == active) AND (age > 18 OR tier == premium)
filter: "AND:/status,==,active:OR:/user/age,>,18:/tier,==,premium"
```

### Regex Filters

Match a field against a regex pattern.

**Syntax:** `REGEX:<path>,<pattern>`

```yaml
# Email validation
filter: "REGEX:/user/email,^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$"

# Match phone format
filter: "REGEX:/phone,^\\+1-\\d{3}-\\d{3}-\\d{4}$"
```

**Note:** Backslashes must be doubled in YAML strings.

### Array Filters

Test conditions on array elements.

**ARRAY_ALL:** Every element must match
```yaml
# All prices must be positive
filter: "ARRAY_ALL:/items,/price,>,0"
```

**ARRAY_ANY:** At least one element must match
```yaml
# At least one item is a book
filter: "ARRAY_ANY:/items,/type,==,book"
```

### Envelope Filters

Operate on message metadata (key, headers, timestamp) rather than value.

#### Key Filters

```yaml
# Key starts with prefix
filter: "KEY_PREFIX:user-"

# Key matches regex
filter: "KEY_MATCHES:^(user|order)-[0-9]+"

# Key exists (not null)
filter: "KEY_EXISTS"
```

#### Header Filters

```yaml
# Header exists
filter: "HEADER_EXISTS:x-request-id"

# Header value comparison
filter: "HEADER:x-priority,==,high"
```

#### Timestamp Filters

```yaml
# Message age in seconds
filter: "TIMESTAMP_AGE:<,3600"  # Less than 1 hour old

# After specific time
filter: "TIMESTAMP_AFTER:1700000000000"  # After epoch ms

# Before specific time
filter: "TIMESTAMP_BEFORE:1700086400000"
```

---

## Transform Expressions

### Path Extraction (Shorthand)

Extract a field from the message.

**Syntax:** `<path>`

```yaml
# Extract nested field
transform: "/user/email"

# Extract top-level field
transform: "/status"
```

**Behavior:** Replaces entire message value with extracted field.

### Object Construction

Build a new JSON object from multiple fields.

**Syntax:** `CONSTRUCT:<field>=<path>:<field>=<path>:...`

```yaml
# Build user summary
transform: "CONSTRUCT:userId=/user/id:email=/user/email:tier=/subscription/tier"

# Output: {"userId": 123, "email": "user@example.com", "tier": "premium"}
```

### Array Mapping

Apply a transformation to each array element.

**Syntax:** `ARRAY_MAP:<array_path>,<element_path>,<output_field>`

```yaml
# Extract IDs from items
transform: "ARRAY_MAP:/items,/id,item_ids"

# Input:  {"items": [{"id": 1, "name": "A"}, {"id": 2, "name": "B"}]}
# Output: {"items": [...], "item_ids": [1, 2]}
```

### Arithmetic

Perform math operations on numeric fields.

**Syntax:** `<OP>:<left_path>,<right_path>[,<output_field>]`

**Operators:** `ADD`, `SUB`, `MUL`, `DIV`

```yaml
# Add tax to price
transform: "ADD:/price,/tax,total"

# Calculate discount
transform: "SUB:/original_price,/discount,final_price"
```

**Behavior:** 
- With `output_field`: Adds new field, preserves original
- Without `output_field`: Replaces message value with result

### Hashing

Hash a field for PII redaction or key derivation.

**Syntax:** `HASH:<algorithm>,<path>[,<output_field>]`

**Algorithms:** `MD5`, `SHA256`, `SHA512`, `MURMUR64`, `MURMUR128`

```yaml
# Hash email for privacy
transform: "HASH:SHA256,/user/email,hashed_email"

# Hash SSN
transform: "HASH:SHA512,/ssn,ssn_hash"
```

### Cache Operations

Enrich or cache messages using in-memory stores.

#### Cache Lookup (Enrichment)

**Syntax:** `CACHE_LOOKUP:<key_path>,<store_name>,<output_field|MERGE>`

```yaml
# Enrich with user profile
transform: "CACHE_LOOKUP:/user_id,user_profiles,user_profile"

# Merge cached object into message
transform: "CACHE_LOOKUP:/user_id,user_profiles,MERGE"
```

#### Cache Put (Caching)

**Syntax:** `CACHE_PUT:<key_path>,<store_name>[,<value_path>]`

```yaml
# Cache entire message
transform: "CACHE_PUT:/user_id,user_profiles"

# Cache specific field
transform: "CACHE_PUT:/user_id,user_profiles,/user/profile"
```

### String Operations

**Available operations:**
- `UPPER`, `LOWER` - Case conversion
- `TRIM`, `TRIM_START`, `TRIM_END` - Whitespace removal
- `LENGTH` - String length
- `SUBSTRING` - Extract substring
- `REPLACE`, `REPLACE_ALL` - String replacement
- `REGEX_REPLACE` - Regex-based replacement
- `SPLIT` - Split into array
- `CONCAT` - Concatenate strings

**Syntax:** `STRING:<op>,<path>[,<args>...][,<output_field>]`

```yaml
# Convert to uppercase
transform: "STRING:UPPER,/name,name_upper"

# Extract substring
transform: "STRING:SUBSTRING,/text,0,100,preview"

# Replace all occurrences
transform: "STRING:REPLACE_ALL,/description,OLD,NEW,updated_desc"

# Split by delimiter
transform: "STRING:SPLIT,/tags,comma,tags_array"

# Concatenate multiple parts
transform: "STRING:CONCAT,full_name,/first_name, ,/last_name"
```

---

## Envelope Operations

Envelope operations modify message metadata, not the value.

### Key Transformation

Set or transform the message key.

**Syntax:** Defined in `key_transform` config field

```yaml
# Extract field as key
key_transform: "/user/id"

# Template with placeholders
key_transform: "user-{/user/id}"

# Constant key
key_transform: "CONSTANT:my-key"

# Hash a field
key_transform: "HASH:SHA256,/user/email"

# Construct JSON key
key_transform: "CONSTRUCT:tenant=/tenant:user=/user/id"
```

### Header Manipulation

Add, copy, or remove headers.

**Static headers:**
```yaml
headers:
  x-processed-by: "streamforge"
  x-version: "1.0"
```

**Dynamic headers:**
```yaml
header_transforms:
  x-user-id: "FROM:/user/id"
  x-request-id: "COPY:request-id"
  x-remove-me: "REMOVE"
```

### Timestamp Control

Control message timestamp behavior.

```yaml
# Preserve original timestamp
timestamp_transform: "PRESERVE"

# Set to current time
timestamp_transform: "CURRENT"

# Extract from field (epoch ms)
timestamp_transform: "FROM:/event_time"

# Add seconds to current time
timestamp_transform: "ADD:3600"

# Subtract seconds from current time
timestamp_transform: "SUBTRACT:60"
```

---

## Type System

### JSON Path Validation

All JSON paths are validated at parse time to ensure:
- Start with `/`
- Contain valid identifiers (alphanumeric + underscore)
- No trailing slashes

**Valid:**
- `/user/id`
- `/subscription/tier`
- `/items/0/name` (array index access)

**Invalid:**
- `user/id` (missing leading `/`)
- `/user/` (trailing `/`)
- `/user-name` (hyphen not allowed in identifier)

### Value Type Inference

The DSL infers types from literal values:

- **Number:** `42`, `3.14`, `-10`
- **String:** `"active"`, `"user@example.com"`
- **Boolean:** `true`, `false`
- **Null:** `null`

**Type coercion rules:**
- Numeric operators (`>`, `>=`, etc.) require numeric values
- String operators work on string values
- `==` and `!=` work on any type with strict equality

### Array Element Access

Arrays can be accessed by index or filtered by element:

```yaml
# Access by index (in path)
transform: "/items/0/name"

# Filter array elements
filter: "ARRAY_ANY:/items,/price,>,100"
```

---

## Operator Precedence

### Boolean Logic

Precedence (highest to lowest):
1. `NOT`
2. `AND`
3. `OR`

**Example:**
```yaml
# Parsed as: A AND (B OR C)
filter: "AND:/a,==,1:OR:/b,==,2:/c,==,3"

# Use NOT for negation
filter: "NOT:AND:/a,==,1:/b,==,2"  # NOT (A AND B)
```

### Arithmetic

Standard math precedence:
1. `MUL`, `DIV` (left-to-right)
2. `ADD`, `SUB` (left-to-right)

**Note:** Only one arithmetic operation per transform. Chain multiple transforms for complex math.

---

## Escaping and Quoting

### Special Characters

- **Colon (`:`)** - Separates operation parts, cannot appear in values
- **Comma (`,`)** - Separates parameters, cannot appear in values
- **Slash (`/`)** - JSON path prefix

### YAML Escaping

In YAML configs, quote strings containing special characters:

```yaml
# Correct: quoted regex pattern
filter: 'REGEX:/email,^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$'

# Correct: quoted string with comma
filter: "/status,==,pending,review"  # Value is "pending,review" - INVALID
# Use: "/status,==,pending review" or store in variable

# Backslash escaping
filter: "REGEX:/path,\\d+"  # Regex: \d+
```

### Limitations

**Cannot use in values:**
- Colons (`:`) in filter/transform values
- Commas (`,`) in filter values

**Workaround:** Use constants or environment variables for complex values.

---

## Error Handling

### Parse-Time Errors

The DSL validator catches errors at parse time:

**Invalid Syntax:**
```yaml
filter: "/user/age,>,not_a_number"
# Error: Invalid numeric literal 'not_a_number'
```

**Missing Parameters:**
```yaml
transform: "HASH:SHA256"
# Error: HASH requires path parameter
```

**Invalid Path:**
```yaml
filter: "user/id,==,123"
# Error: JSON path must start with '/'
```

**Unknown Operation:**
```yaml
transform: "UNKNOWN_OP:/path"
# Error: Unknown transform operation 'UNKNOWN_OP'
```

### Runtime Errors

Errors during message processing:

**Path Not Found:**
```yaml
filter: "/nonexistent/field,==,value"
# Result: Filter returns false (field treated as null)
```

**Type Mismatch:**
```yaml
filter: "/string_field,>,10"
# Result: Filter returns false (cannot compare string > number)
```

**Regex Error:**
```yaml
filter: "REGEX:/email,[invalid(regex"
# Error: Invalid regex pattern (caught at parse time)
```

**Recovery Actions:**

Errors map to recovery actions (see docs/ERROR_HANDLING.md):
- **FilterEvaluation:** `SendToDlq` or `SkipAndLog`
- **TransformEvaluation:** `SendToDlq`
- **InvalidFilter/Transform:** `FailFast` (parse-time error)

---

## Examples

### Example 1: User Filtering and Enrichment

**Goal:** Route active premium users, enrich with profile

```yaml
routing:
  routing_type: filter
  destinations:
    - output: "premium-users"
      # Filter: active AND (age > 21 OR tier == premium)
      filter: "AND:/status,==,active:OR:/age,>,21:/tier,==,premium"
      
      # Extract user summary
      transform: "CONSTRUCT:id=/user/id:email=/email:tier=/tier"
      
      # Enrich with profile from cache
      # (Assuming profile cached from previous pipeline)
      cache_lookup: "/user/id,user_profiles,profile"
      
      # Set key to user ID
      key_transform: "/user/id"
      
      # Add processing header
      headers:
        x-pipeline: "premium-user-processor"
```

### Example 2: Event Age Filtering

**Goal:** Only process recent events (< 1 hour old)

```yaml
routing:
  routing_type: filter
  destinations:
    - output: "recent-events"
      filter: "TIMESTAMP_AGE:<,3600"  # < 3600 seconds (1 hour)
      transform: "/event"
```

### Example 3: PII Redaction

**Goal:** Hash sensitive fields before routing

```yaml
routing:
  routing_type: filter
  destinations:
    - output: "events-redacted"
      transform: "CONSTRUCT:user_id=/user/id:email_hash=HASH:SHA256,/user/email:ssn_hash=HASH:SHA512,/ssn"
```

**Note:** This syntax is pseudo-code. Real implementation would require chaining or using a custom processor.

### Example 4: Multi-Destination Routing

**Goal:** Route orders by priority

```yaml
routing:
  routing_type: filter
  destinations:
    - output: "orders-urgent"
      filter: "AND:/type,==,order:HEADER:x-priority,==,urgent"
      key_transform: "order-{/order_id}"
      
    - output: "orders-normal"
      filter: "/type,==,order"
      key_transform: "order-{/order_id}"
```

### Example 5: Array Processing

**Goal:** Extract product IDs from order items

```yaml
transform: "ARRAY_MAP:/items,/product_id,product_ids"

# Input:
# {
#   "order_id": "ORD-123",
#   "items": [
#     {"product_id": "P1", "quantity": 2},
#     {"product_id": "P2", "quantity": 1}
#   ]
# }
#
# Output:
# {
#   "order_id": "ORD-123",
#   "items": [...],
#   "product_ids": ["P1", "P2"]
# }
```

---

## Migration from 0.x

### Deprecated Features (v0.4.0 → v1.0)

#### 1. KEY_SUFFIX and KEY_CONTAINS

**Removed:** `KEY_SUFFIX:suffix`, `KEY_CONTAINS:substring`

**Reason:** Rarely used, regex covers all cases

**Migration:**

```yaml
# 0.x: KEY_SUFFIX
filter: "KEY_SUFFIX:-prod"

# 1.0: Use KEY_MATCHES with regex
filter: "KEY_MATCHES:.*-prod$"
```

```yaml
# 0.x: KEY_CONTAINS
filter: "KEY_CONTAINS:test"

# 1.0: Use KEY_MATCHES
filter: "KEY_MATCHES:.*test.*"
```

### Breaking Changes

**None.** All other 0.x syntax remains supported in v1.0.

### New Features in v1.0

1. **String operations:** `STRING:` prefix for UPPER, LOWER, TRIM, etc.
2. **Timestamp operations:** `TIMESTAMP_AGE`, `TIMESTAMP_AFTER`, `TIMESTAMP_BEFORE`
3. **Formal grammar:** EBNF specification for parser implementers
4. **Type validation:** JSON paths validated at parse time

### Compatibility

v1.0 parsers can read 0.x configs with these notes:
- `KEY_SUFFIX` and `KEY_CONTAINS` generate deprecation warnings
- All other syntax works unchanged
- Use `streamforge validate` CLI to check for deprecations

---

## Formal Guarantees (v1.0+)

### Stability Promise

**Stable:** Syntax documented in this spec will not change in minor versions (1.x)

**Backward Compatibility:** v1.x parsers will parse v1.0 configs

**Deprecation Policy:** 
- Features deprecated in v1.x remain functional until v2.0
- Deprecation warnings guide migration
- 6-month notice before removal

### Validation Guarantees

**Parse-Time Checks:**
- Syntax errors caught before pipeline starts
- JSON path validation
- Regex pattern validation
- Type compatibility checks (where possible)

**Runtime Behavior:**
- Filters return `false` on missing paths (not error)
- Transforms skip on missing paths (pass-through)
- Type mismatches log warnings but don't halt pipeline

---

## Appendix: Complete Operator Reference

### Filter Operators

| Operator | Syntax | Description |
|----------|--------|-------------|
| Simple | `/path,op,value` | Compare field to value |
| AND | `AND:c1:c2:...` | All conditions true |
| OR | `OR:c1:c2:...` | Any condition true |
| NOT | `NOT:condition` | Invert condition |
| REGEX | `REGEX:/path,pattern` | Regex match |
| ARRAY_ALL | `ARRAY_ALL:/arr,filter` | All elements match |
| ARRAY_ANY | `ARRAY_ANY:/arr,filter` | Any element matches |
| KEY_PREFIX | `KEY_PREFIX:str` | Key starts with |
| KEY_MATCHES | `KEY_MATCHES:regex` | Key matches regex |
| KEY_EXISTS | `KEY_EXISTS` | Key is not null |
| HEADER_EXISTS | `HEADER_EXISTS:name` | Header exists |
| HEADER | `HEADER:name,op,val` | Header comparison |
| TIMESTAMP_AGE | `TIMESTAMP_AGE:op,sec` | Message age |
| TIMESTAMP_AFTER | `TIMESTAMP_AFTER:ms` | After epoch |
| TIMESTAMP_BEFORE | `TIMESTAMP_BEFORE:ms` | Before epoch |

### Transform Operators

| Operator | Syntax | Description |
|----------|--------|-------------|
| Extract | `/path` | Extract field |
| CONSTRUCT | `CONSTRUCT:f=/p:...` | Build object |
| ARRAY_MAP | `ARRAY_MAP:/a,/e,out` | Map array |
| ADD | `ADD:/l,/r[,out]` | Add numbers |
| SUB | `SUB:/l,/r[,out]` | Subtract |
| MUL | `MUL:/l,/r[,out]` | Multiply |
| DIV | `DIV:/l,/r[,out]` | Divide |
| HASH | `HASH:alg,/p[,out]` | Hash field |
| CACHE_LOOKUP | `CACHE_LOOKUP:/k,s,out` | Enrich from cache |
| CACHE_PUT | `CACHE_PUT:/k,s[,/v]` | Store in cache |
| STRING:UPPER | `STRING:UPPER,/p[,out]` | Uppercase |
| STRING:LOWER | `STRING:LOWER,/p[,out]` | Lowercase |
| STRING:TRIM | `STRING:TRIM,/p[,out]` | Trim whitespace |
| STRING:LENGTH | `STRING:LENGTH,/p[,out]` | String length |
| STRING:SUBSTRING | `STRING:SUBSTRING,/p,s,l[,out]` | Extract substring |
| STRING:REPLACE | `STRING:REPLACE,/p,f,t[,out]` | Replace first |
| STRING:REPLACE_ALL | `STRING:REPLACE_ALL,/p,f,t[,out]` | Replace all |
| STRING:SPLIT | `STRING:SPLIT,/p,d[,out]` | Split to array |
| STRING:CONCAT | `STRING:CONCAT,out,p1,p2,...` | Concatenate |

### Envelope Operators

| Operator | Config Field | Description |
|----------|--------------|-------------|
| Key Transform | `key_transform` | Set message key |
| Static Headers | `headers` | Add constant headers |
| Dynamic Headers | `header_transforms` | Extract/copy headers |
| Timestamp | `timestamp_transform` | Set message timestamp |

---

**End of Specification**
