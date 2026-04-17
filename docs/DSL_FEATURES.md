# DSL Feature Summary

## Overview

StreamForge uses the **Rhai scripting language** for filtering and transforming Kafka messages. Rhai is a lightweight, JavaScript-like language that compiles at startup and executes in ~500ns per message.

**Performance**: 40x faster than Java JSLT through pre-compiled scripts and direct JSON manipulation.

## Complete Feature List

### 1. Field Access

Access message fields using JavaScript-like syntax.

**Syntax**: `msg["field"]` or `msg["nested"]["field"]`

```yaml
filter: 'msg["status"] == "active"'
transform: 'msg["user"]["email"]'
```

**Nested access**:
```yaml
filter: 'msg["order"]["total"] > 100'
transform: 'msg["customer"]["profile"]["tier"]'
```

### 2. Comparison Operators

Compare field values with expected values.

**Operators**: `>`, `>=`, `<`, `<=`, `==`, `!=`

```yaml
filter: 'msg["siteId"] > 10000'
filter: 'msg["status"] == "active"'
filter: 'msg["price"] <= 99.99'
```

**Supported Types**:
- Numeric (integers, floats)
- String
- Boolean
- Null checks

### 3. Boolean Logic

Combine multiple conditions with standard logical operators.

**Operators**: `&&` (AND), `||` (OR), `!` (NOT)

```yaml
filter: 'msg["siteId"] > 10000 && msg["status"] == "active"'
filter: 'msg["priority"] == "high" || msg["urgent"] == true'
filter: '!msg["test"]'
```

**Multiple conditions**:
```yaml
filter:
  - 'msg["siteId"] > 10000'
  - 'msg["status"] == "active"'
  - '!msg["test"]'
```

### 4. String Operations

Rich string manipulation built into Rhai.

**Methods**: `starts_with`, `ends_with`, `contains`, `to_upper`, `to_lower`, `trim`, `split`

```yaml
filter: 'msg["email"].contains("@company.com")'
filter: 'msg["type"].starts_with("payment.")'
filter: 'msg["status"].to_lower() == "active"'

transform: |
  #{
    email: msg["email"].to_lower(),
    name: msg["name"].trim(),
    domain: msg["email"].split("@")[1]
  }
```

**Use Cases**:
- Email domain validation
- Prefix/suffix routing
- Case-insensitive matching
- String cleaning and normalization

### 5. Regular Expressions

Match string fields against regex patterns using the `matches` method.

**Syntax**: `field.matches("pattern")`

```yaml
filter: 'msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$")'
filter: 'msg["phone"].matches("^\\+1[0-9]{10}$")'
filter: 'msg["version"].matches("^2\\.[0-9]+\\.[0-9]+$")'
```

**Use Cases**:
- Email validation
- URL pattern matching
- Version number checking
- Phone number validation

### 6. Array Operations

Rhai provides rich array manipulation.

**Array filtering with closures**:
```yaml
# Check if all users are active
filter: 'msg["users"].all(|u| u["status"] == "active")'

# Check if any user is admin
filter: 'msg["users"].any(|u| u["role"] == "admin")'

# Filter array to active users
transform: |
  #{
    activeUsers: msg["users"].filter(|u| u["status"] == "active")
  }
```

**Array mapping**:
```yaml
# Extract IDs from array of objects
transform: 'msg["users"].map(|u| u["id"])'

# Extract nested values
transform: 'msg["orders"].map(|o| o["customer"]["email"])'
```

**Array methods**: `len`, `is_empty`, `contains`, `filter`, `map`, `all`, `any`, `find`

### 7. Arithmetic Operations

Standard mathematical operators.

**Operators**: `+`, `-`, `*`, `/`, `%` (modulo)

```yaml
# Add two fields
transform: '#{  total: msg["price"] + msg["tax"] }'

# Multiply by constant (tax calculation)
transform: '#{ taxAmount: msg["price"] * 0.08 }'

# Calculate average
transform: '#{ average: msg["total"] / msg["count"] }'

# Discount calculation
transform: '#{ discounted: msg["price"] * 0.9 }'
```

**Compound operations**:
```yaml
transform: |
  #{
    subtotal: msg["price"] * msg["quantity"],
    tax: (msg["price"] * msg["quantity"]) * 0.08,
    total: (msg["price"] * msg["quantity"]) * 1.08
  }
```

### 8. Object Construction

Create new JSON objects using Rhai object literals.

**Syntax**: `#{ field1: value1, field2: value2 }`

```yaml
transform: |
  #{
    id: msg["confId"],
    site: msg["siteId"],
    timestamp: msg["ts"]
  }
```

**Merging objects**:
```yaml
transform: 'msg + #{ processedAt: now_ms(), version: 2 }'
```

## Configuration Examples

### Example 1: Email Validation with Multiple Conditions

```yaml
routing:
  destinations:
    - output: corporate-users
      name: valid-corporate-emails
      filter:
        - 'msg["user"]["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$")'
        - 'msg["user"]["email"].contains("@company.com")'
      transform: |
        #{
          email: msg["user"]["email"],
          name: msg["user"]["name"]
        }
```

### Example 2: Array Processing with Arithmetic

```yaml
routing:
  destinations:
    - output: purchase-amounts
      name: high-value-active-users
      filter:
        - 'msg["purchases"].any(|p| p["amount"] > 100)'
        - 'msg["user"]["active"] == true'
      transform: |
        msg["purchases"].map(|p| p["amount"])
    
    - output: bulk-discount-prices
      name: discounted-prices
      filter: 'msg["order"]["items"] > 5'
      transform: |
        msg["order"]["total"] * 0.9
```

### Example 3: Complex Boolean Logic

```yaml
routing:
  destinations:
    - output: priority-orders
      name: premium-or-urgent
      filter: |
        (msg["user"]["tier"] == "premium" && msg["user"]["status"] == "active") ||
        (msg["order"]["priority"] == "urgent" && msg["order"]["total"] > 500)
      transform: |
        #{
          userId: msg["user"]["id"],
          orderTotal: msg["order"]["total"],
          priority: msg["order"]["priority"]
        }
```

### Example 4: Regular Expression Routing

```yaml
routing:
  destinations:
    - output: errors
      name: error-events
      filter: 'msg["message"]["type"].matches("^(error|failure|exception)")'
    
    - output: success
      name: success-events
      filter: 'msg["message"]["type"].matches("^(success|complete|done)")'
```

## Performance Characteristics

### Filter Performance

- **Simple comparison**: ~100-200ns per evaluation
- **Boolean logic (&&/||/!)**: ~200-400ns depending on complexity
- **String operations**: ~100-500ns (method dependent)
- **Regular expressions**: ~500ns-2µs (compiled at startup, pattern complexity dependent)
- **Array operations**: ~1-10µs (array size dependent)

### Transform Performance

- **Field access**: ~50-100ns
- **Object construction**: ~300-600ns
- **Array mapping**: ~1-10µs (array size dependent)
- **Arithmetic**: ~50-100ns
- **String manipulation**: ~100-300ns

### Comparison with Java JSLT

| Operation | Java JSLT | Rhai DSL | Speedup |
|-----------|-----------|----------|---------|
| Simple filter | 4µs | 200ns | 20x |
| Boolean logic | 10µs | 400ns | 25x |
| Object construction | 8µs | 600ns | 13x |
| Array mapping | 50µs | 5µs | 10x |
| String operations | 5µs | 300ns | 17x |

## Error Handling

### Filter Errors

When a filter fails to evaluate:
- Returns `false` (message is not sent to that destination)
- Processing continues for other destinations
- Error is logged with stack trace

### Transform Errors

When a transform fails:
- Error is logged with Rhai stack trace
- Message is **not sent** to that destination
- Processing continues for other destinations
- DLQ (dead letter queue) can be configured to capture failed messages

**Common errors**:
- Division by zero: `msg["value"] / 0`
- Null field access: `msg["missing"]["nested"]`
- Type mismatch: `msg["string"] + 123`
- Array out of bounds: `msg["items"][999]`

**Error prevention**:
```yaml
# Check for null before access
filter: 'not_null(msg["user"]) && msg["user"]["active"] == true'

# Use null-coalescing operator
transform: 'msg["value"] ?? 0'

# Safe array access
filter: 'msg["items"].len() > 0'
```

## Best Practices

1. **Use simple filters when possible** - Faster and easier to debug
2. **Test scripts separately** - Use Rhai playground or unit tests
3. **Handle null values** - Use `not_null()` or `??` operator
4. **Validate array access** - Check `.len()` before indexing
5. **Use meaningful variable names** - In complex transforms with `let`
6. **Break complex logic into multiple steps** - Use multiple destinations if needed
7. **Monitor transform errors** - Check logs for failed transformations
8. **Profile before optimizing** - Measure actual performance impact

## Rhai Language Features

Rhai provides a rich scripting environment:

### Control Flow
```yaml
transform: |
  if msg["amount"] > 1000 {
    msg + #{ tier: "premium", discount: 0.1 }
  } else {
    msg + #{ tier: "standard", discount: 0 }
  }
```

### Switch Statements
```yaml
filter: |
  switch msg["status"] {
    "active" | "pending" => true,
    "inactive" | "deleted" => false,
    _ => false
  }
```

### Loops (use sparingly in filters)
```yaml
transform: |
  let total = 0;
  for item in msg["items"] {
    total += item["price"];
  }
  msg + #{ totalPrice: total }
```

### Functions
```yaml
transform: |
  fn calculate_tax(amount) {
    amount * 0.08
  }
  #{
    subtotal: msg["amount"],
    tax: calculate_tax(msg["amount"]),
    total: msg["amount"] * 1.08
  }
```

## Future Enhancements

Rhai already provides:
- ✅ Full control flow (if/else, switch, loops)
- ✅ String manipulation (concat, substring, split, etc.)
- ✅ Array operations (map, filter, reduce, etc.)
- ✅ Custom functions
- ✅ Closures

Planned StreamForge additions:
- Date/time parsing and formatting functions
- JSON schema validation
- External API calls (async functions)
- Stateful transforms (windowing, aggregation)
- Custom Rust function plugins

## Built-in Functions

StreamForge provides additional functions beyond standard Rhai:

### Null/Empty Checks
```yaml
filter: 'not_null(msg["field"])'
filter: 'is_null_or_empty(msg["field"])'
```

### Timestamp Functions
```yaml
transform: 'msg + #{ processedAt: now_ms() }'  # Current timestamp in milliseconds
transform: 'msg + #{ age: (now_ms() - timestamp) / 1000 }'  # Age in seconds
```

### Cache Lookup
```yaml
transform: |
  let profile = cache_lookup("profiles", msg["userId"]);
  msg + #{ tier: profile["tier"] ?? "standard" }
```

### Hashing (for PII)
```yaml
transform: |
  #{
    emailHash: hash_sha256(msg["email"].to_lower()),
    phoneHash: hash_sha256(msg["phone"])
  }
```

### Header/Key/Timestamp Access
```yaml
filter: 'headers["x-tenant"] == "production"'
filter: 'key.starts_with("premium-")'
filter: '(now_ms() - timestamp) / 1000 < 300'  # Messages newer than 5 minutes
```

## Envelope Operations (Key, Headers, Timestamp)

While filter and transform expressions use Rhai, **envelope operations still use string format**:

### Key Transform
```yaml
- output: partitioned-events
  key_transform: '/tenantId'                  # Extract field as key
  # or
  key_transform: 'HASH:SHA256,/userId'        # Hash for privacy
  # or
  key_transform: 'CONSTRUCT:tenant=/tenant:user=/user'  # Composite key
```

### Header Transforms
```yaml
- output: enriched-events
  headers:
    x-pipeline: "streamforge"
    x-version: "2.0"
  header_transforms:
    - header: x-user-id
      operation: 'FROM:/user/id'              # Extract from payload
    - header: x-correlation-id
      operation: 'COPY:x-request-id'          # Copy existing header
    - header: x-sensitive-token
      operation: 'REMOVE'                     # Remove header
```

### Timestamp Transform
```yaml
- output: timestamped-events
  timestamp: 'PRESERVE'                       # Keep original (default)
  # or
  timestamp: 'CURRENT'                        # Set to current time
  # or
  timestamp: 'FROM:/event/timestamp'          # Extract from payload
```

See [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md#envelope-operations) for complete envelope operation documentation.

## Documentation

- [README.md](../README.md) - Quick start and recipes
- [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) - Complete Rhai DSL reference
- [QUICKSTART.md](QUICKSTART.md) - Getting started
- [USAGE.md](USAGE.md) - Real-world use cases
- [Rhai Language Documentation](https://rhai.rs/book/) - Official Rhai docs

## Contributing

To add new built-in functions:

1. Define the function in `src/rhai_dsl.rs`
2. Register it in the Rhai engine: `engine.register_fn("my_func", my_func)`
3. Add unit tests
4. Update documentation
5. Benchmark performance impact

See existing implementations in `src/rhai_dsl.rs` for examples.
