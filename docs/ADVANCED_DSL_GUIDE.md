# Advanced DSL Guide

This guide covers advanced filtering and transformation capabilities including array operations, regular expressions, and arithmetic operations.

## Table of Contents

- [Array Operations](#array-operations)
  - [Array Filters](#array-filters)
  - [Array Transforms](#array-transforms)
- [Regular Expressions](#regular-expressions)
- [Arithmetic Operations](#arithmetic-operations)
- [Complex Examples](#complex-examples)

## Array Operations

### Array Filters

Filter messages based on array element conditions. Two modes are available:

#### ARRAY_ALL

All elements in the array must match the condition.

**Syntax:** `ARRAY_ALL:/path,element_filter`

**Examples:**

```bash
# All users must be active
"ARRAY_ALL:/users,/status,==,active"

# All items must have quantity > 0
"ARRAY_ALL:/items,/quantity,>,0"
```

**Config Example:**
```json
{
  "destinations": [{
    "filter": "ARRAY_ALL:/users,/status,==,active",
    "topic": "active-users-only"
  }]
}
```

**Sample Messages:**

```json
// PASSES - all users are active
{
  "users": [
    {"id": 1, "status": "active"},
    {"id": 2, "status": "active"}
  ]
}

// FAILS - one user is inactive
{
  "users": [
    {"id": 1, "status": "active"},
    {"id": 2, "status": "inactive"}
  ]
}
```

#### ARRAY_ANY

At least one element in the array must match the condition.

**Syntax:** `ARRAY_ANY:/path,element_filter`

**Examples:**

```bash
# At least one task is high priority
"ARRAY_ANY:/tasks,/priority,==,high"

# At least one item is out of stock
"ARRAY_ANY:/items,/stock,<=,0"
```

**Config Example:**
```json
{
  "destinations": [{
    "filter": "ARRAY_ANY:/tasks,/priority,==,high",
    "topic": "urgent-tasks"
  }]
}
```

**Sample Messages:**

```json
// PASSES - at least one task is high priority
{
  "tasks": [
    {"id": 1, "priority": "low"},
    {"id": 2, "priority": "high"}
  ]
}

// FAILS - no high priority tasks
{
  "tasks": [
    {"id": 1, "priority": "low"},
    {"id": 2, "priority": "medium"}
  ]
}
```

### Array Transforms

Transform arrays by mapping over their elements.

#### ARRAY_MAP

Apply a transformation to each element in an array.

**Syntax:** `ARRAY_MAP:/path,element_transform`

**Examples:**

```bash
# Extract IDs from array of objects
"ARRAY_MAP:/users,/id"

# Extract nested values
"ARRAY_MAP:/orders,/customer/email"
```

**Config Example:**
```json
{
  "destinations": [{
    "transform": "ARRAY_MAP:/users,/id",
    "topic": "user-ids"
  }]
}
```

**Transformation Example:**

```json
// Input
{
  "users": [
    {"id": 1, "name": "Alice", "email": "alice@example.com"},
    {"id": 2, "name": "Bob", "email": "bob@example.com"},
    {"id": 3, "name": "Charlie", "email": "charlie@example.com"}
  ]
}

// Output (after ARRAY_MAP:/users,/id)
[1, 2, 3]
```

**Nested Field Example:**

```json
// Input
{
  "orders": [
    {"id": 100, "customer": {"email": "alice@example.com", "name": "Alice"}},
    {"id": 101, "customer": {"email": "bob@example.com", "name": "Bob"}}
  ]
}

// Output (after ARRAY_MAP:/orders,/customer/email)
["alice@example.com", "bob@example.com"]
```

## Regular Expressions

Match string fields against regex patterns.

**Syntax:** `REGEX:/path,pattern`

### Common Patterns

#### Email Validation

```bash
"REGEX:/message/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"
```

**Config Example:**
```json
{
  "destinations": [{
    "filter": "REGEX:/user/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$",
    "topic": "valid-emails"
  }]
}
```

#### URL Matching

```bash
# Match URLs starting with https
"REGEX:/message/url,^https://"

# Match specific domain
"REGEX:/message/url,@example\\.com"
```

#### Status Patterns

```bash
# Status starting with "active"
"REGEX:/message/status,^active"

# Status ending with "pending"
"REGEX:/message/status,pending$"

# Status containing "error"
"REGEX:/message/status,error"
```

#### Phone Numbers

```bash
# US phone numbers
"REGEX:/user/phone,^\\+1[0-9]{10}$"

# International format
"REGEX:/user/phone,^\\+[0-9]{1,3}[0-9]{10,14}$"
```

#### Version Numbers

```bash
# Semantic versioning (e.g., 1.2.3)
"REGEX:/app/version,^[0-9]+\\.[0-9]+\\.[0-9]+$"

# Major version 2.x
"REGEX:/app/version,^2\\."
```

### Complex Regex Examples

```json
{
  "destinations": [
    {
      "filter": "REGEX:/message/type,^(create|update|delete)$",
      "topic": "crud-operations"
    },
    {
      "filter": "REGEX:/message/ip,^192\\.168\\.",
      "topic": "local-network"
    },
    {
      "filter": "REGEX:/message/code,^[A-Z]{3}[0-9]{4}$",
      "topic": "valid-codes"
    }
  ]
}
```

### Regex Special Characters

Remember to escape special characters with `\\`:

- `.` → `\\.` (literal dot)
- `*` → `\\*` (literal asterisk)
- `+` → `\\+` (literal plus)
- `?` → `\\?` (literal question mark)
- `|` → `\\|` (literal pipe)
- `(` → `\\(` (literal parenthesis)
- `[` → `\\[` (literal bracket)
- `^` → `^` (start anchor, no escape)
- `$` → `$` (end anchor, no escape)

## Arithmetic Operations

Perform mathematical operations on numeric fields.

### Operations

- **ADD** - Addition
- **SUB** - Subtraction
- **MUL** - Multiplication
- **DIV** - Division

**Syntax:** `ARITHMETIC:op,operand1,operand2`

Operands can be:
- JSON path: `/path/to/field`
- Numeric constant: `123` or `1.5`

### Addition

```bash
# Add two fields: total = price + tax
"ARITHMETIC:ADD,/price,/tax"

# Add constant: adjusted = value + 100
"ARITHMETIC:ADD,/value,100"
```

**Config Example:**
```json
{
  "destinations": [{
    "transform": "ARITHMETIC:ADD,/order/subtotal,/order/shipping",
    "topic": "order-totals"
  }]
}
```

**Transformation Example:**

```json
// Input
{
  "order": {
    "subtotal": 100.50,
    "shipping": 9.99
  }
}

// Output (after ARITHMETIC:ADD,/order/subtotal,/order/shipping)
110.49
```

### Subtraction

```bash
# Subtract discount from total
"ARITHMETIC:SUB,/total,/discount"

# Subtract constant
"ARITHMETIC:SUB,/balance,50"
```

**Example:**

```json
// Input
{"total": 100.0, "discount": 20.0}

// Output (after ARITHMETIC:SUB,/total,/discount)
80.0
```

### Multiplication

```bash
# Calculate tax: tax = price * 0.08
"ARITHMETIC:MUL,/price,0.08"

# Apply markup: price = cost * 1.5
"ARITHMETIC:MUL,/cost,1.5"
```

**Example:**

```json
// Input
{"price": 100.0}

// Output (after ARITHMETIC:MUL,/price,1.2)
120.0
```

### Division

```bash
# Calculate average: avg = total / count
"ARITHMETIC:DIV,/total,/count"

# Convert to percentage: pct = value / 100
"ARITHMETIC:DIV,/value,100"
```

**Example:**

```json
// Input
{"total": 500.0, "count": 5.0}

// Output (after ARITHMETIC:DIV,/total,/count)
100.0
```

### Error Handling

**Division by zero:**
```json
// Input
{"value": 100.0, "divisor": 0.0}

// Error: Division by zero
// Message will fail to process
```

**Missing fields:**
```json
// Input
{"price": 100.0}
// Missing "tax" field

// Error: Right operand not found or not a number
// Message will fail to process
```

## Complex Examples

### Example 1: E-commerce Order Processing

Process orders with multiple conditions and transformations.

```json
{
  "destinations": [
    {
      "name": "high-value-orders",
      "filter": "AND:/order/items,>,0:/order/total,>,500",
      "transform": "CONSTRUCT:orderId=/order/id:total=/order/total:customer=/customer/email",
      "topic": "high-value"
    },
    {
      "name": "discount-applied",
      "filter": "/order/discount,>,0",
      "transform": "ARITHMETIC:SUB,/order/total,/order/discount",
      "topic": "final-prices"
    }
  ]
}
```

### Example 2: User Activity with Arrays

Filter and transform user activity data.

```json
{
  "destinations": [
    {
      "name": "active-sessions",
      "filter": "ARRAY_ALL:/sessions,/active,==,true",
      "transform": "ARRAY_MAP:/sessions,/id",
      "topic": "active-session-ids"
    },
    {
      "name": "problem-sessions",
      "filter": "ARRAY_ANY:/sessions,/error,==,true",
      "topic": "session-errors"
    }
  ]
}
```

### Example 3: Email Validation Pipeline

Validate and route emails based on patterns.

```json
{
  "destinations": [
    {
      "name": "corporate-emails",
      "filter": "REGEX:/email,@(company|enterprise)\\.com$",
      "topic": "corporate"
    },
    {
      "name": "valid-format",
      "filter": "REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$",
      "topic": "validated"
    },
    {
      "name": "suspicious-domains",
      "filter": "REGEX:/email,@(test|temp|fake|disposable)\\.",
      "topic": "suspicious"
    }
  ]
}
```

### Example 4: Financial Calculations

Calculate various financial metrics.

```json
{
  "destinations": [
    {
      "name": "sales-tax",
      "transform": "ARITHMETIC:MUL,/amount,0.08",
      "topic": "tax-amounts"
    },
    {
      "name": "total-with-tax",
      "transform": "ARITHMETIC:MUL,/amount,1.08",
      "topic": "final-amounts"
    },
    {
      "name": "profit-margin",
      "transform": "ARITHMETIC:SUB,/revenue,/cost",
      "topic": "profits"
    },
    {
      "name": "commission",
      "filter": "/sales/total,>,1000",
      "transform": "ARITHMETIC:MUL,/sales/total,0.05",
      "topic": "commissions"
    }
  ]
}
```

### Example 5: Mixed Operations

Combine multiple advanced features.

```json
{
  "destinations": [
    {
      "name": "valid-users-with-purchases",
      "filter": "AND:REGEX:/user/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$:ARRAY_ANY:/purchases,/amount,>,100",
      "transform": "CONSTRUCT:email=/user/email:totalPurchases=/stats/total:highValueCount=/stats/highValue",
      "topic": "premium-customers"
    },
    {
      "name": "bulk-order-discount",
      "filter": "AND:/items/quantity,>=,100:/items/price,>,10",
      "transform": "ARITHMETIC:MUL,/items/price,0.9",
      "topic": "discounted-prices"
    },
    {
      "name": "extract-admin-ids",
      "filter": "ARRAY_ANY:/users,/role,==,admin",
      "transform": "ARRAY_MAP:/users,/id",
      "topic": "admin-user-ids"
    }
  ]
}
```

## Performance Considerations

### Array Operations

- **ARRAY_ALL** with empty arrays returns `true` (all conditions vacuously satisfied)
- **ARRAY_ANY** with empty arrays returns `false` (no elements to match)
- Large arrays (>1000 elements) may impact processing time
- Consider filtering at the source if possible

### Regular Expressions

- Regex patterns are compiled once at startup for optimal performance
- Complex patterns (backreferences, lookaheads) are slower
- Use simple anchors (`^`, `$`) when possible
- Avoid catastrophic backtracking patterns

### Arithmetic Operations

- Division by zero returns an error and fails the message
- Operations on missing fields return errors
- All operations use 64-bit floating point (f64)
- Precision: ~15 decimal digits

## Error Handling

### Filter Errors

If a filter fails to evaluate (e.g., regex doesn't match, field missing):
- The filter returns `false`
- The message is not sent to that destination
- Processing continues for other destinations

### Transform Errors

If a transform fails (e.g., division by zero, missing field):
- An error is logged
- The message is **not sent** to that destination
- Processing continues for other destinations

## Best Practices

1. **Test patterns first**: Use regex testers before deploying
2. **Handle missing fields**: Use OR logic for optional fields
3. **Validate input data**: Ensure expected structure
4. **Monitor errors**: Check logs for failed transforms
5. **Use appropriate modes**: Choose ARRAY_ALL vs ARRAY_ANY carefully
6. **Escape regex properly**: Remember `\\` for special characters
7. **Check for division by zero**: Filter out zero values before DIV operations
8. **Performance test**: Benchmark with production-like data volumes

## Combining Features

You can combine these advanced features with existing boolean logic:

```json
{
  "destinations": [{
    "filter": "AND:REGEX:/type,^order:ARRAY_ANY:/items,/price,>,100:OR:/priority,==,high:/customer/vip,==,true",
    "transform": "CONSTRUCT:type=/type:total=/order/total:itemIds=ARRAY_MAP:/items,/id",
    "topic": "complex-routing"
  }]
}
```

**Note:** Nested transforms (like ARRAY_MAP inside CONSTRUCT) are not currently supported. Apply transforms sequentially if needed.

## Next Steps

- See [ADVANCED_FILTERS.md](ADVANCED_FILTERS.md) for boolean logic
- See [README.md](README.md) for basic usage
- See [QUICKSTART.md](QUICKSTART.md) for getting started
