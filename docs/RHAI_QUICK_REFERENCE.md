# Rhai DSL Quick Reference

Quick reference for writing filters and transforms using the Rhai scripting language.

## Message Access

```yaml
# Access top-level fields
msg["status"]
msg["userId"]
msg["timestamp"]

# Access nested fields
msg["user"]["email"]
msg["order"]["customer"]["name"]

# Access array elements
msg["items"][0]["price"]
msg["tags"].last()
```

## Filter Syntax

Filters are Rhai expressions that return `true` or `false`:

```yaml
# Simple comparisons
filter: 'msg["status"] == "active"'
filter: 'msg["amount"] > 100'
filter: 'msg["age"] >= 18'

# Boolean logic
filter: 'msg["active"] && msg["verified"]'
filter: 'msg["status"] == "pending" || msg["status"] == "approved"'
filter: '!msg["deleted"]'

# Null checks
filter: 'not_null(msg["field"])'
filter: 'is_null_or_empty(msg["field"])'

# Multiple conditions (all must be true)
filter:
  - 'msg["status"] == "active"'
  - 'msg["amount"] > 100'
  - '!msg["test"]'
```

## Transform Syntax

Transforms are Rhai expressions that return a new message value:

```yaml
# Extract single field
transform: 'msg["userId"]'

# Build new object
transform: |
  #{
    id: msg["userId"],
    email: msg["email"].to_lower(),
    timestamp: now_ms()
  }

# Merge with original
transform: 'msg + #{ processedAt: now_ms(), version: 2 }'

# Array of values
transform: '[msg["id"], msg["name"], msg["email"]]'
```

## String Operations

```yaml
# Case conversion
'msg["email"].to_lower()'
'msg["name"].to_upper()'

# Pattern matching
'msg["email"].starts_with("admin")'
'msg["topic"].ends_with(".json")'
'msg["text"].contains("urgent")'

# Cleaning
'msg["name"].trim()'
'msg["text"].replace("old", "new")'

# Splitting
'msg["email"].split("@")[1]'  # domain

# Regular expressions
'msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$")'
```

## Array Operations

```yaml
# Length and emptiness
'msg["items"].len()'
'msg["tags"].is_empty()'

# Contains
'msg["roles"].contains("admin")'

# All elements match
'msg["users"].all(|u| u["active"] == true)'

# Any element matches
'msg["tasks"].any(|t| t["priority"] == "high")'

# Filter array
'msg["items"].filter(|i| i["price"] > 100)'

# Map array
'msg["users"].map(|u| u["id"])'
'msg["orders"].map(|o| o["total"] * 1.08)'
```

## Arithmetic

```yaml
# Basic math
'msg["price"] + msg["tax"]'
'msg["total"] - msg["discount"]'
'msg["price"] * 1.08'
'msg["total"] / msg["quantity"]'
'msg["value"] % 10'  # modulo

# Compound expressions
'(msg["price"] * msg["quantity"]) * 1.08'
```

## Control Flow

### If/Else

```yaml
transform: |
  if msg["amount"] > 1000 {
    msg + #{ tier: "premium", discount: 0.1 }
  } else {
    msg + #{ tier: "standard", discount: 0 }
  }
```

### Switch

```yaml
filter: |
  switch msg["status"] {
    "active" | "pending" => true,
    "inactive" | "deleted" => false,
    _ => false
  }
```

### Functions

```yaml
transform: |
  fn calculate_total(items) {
    let sum = 0;
    for item in items {
      sum += item["price"] * item["quantity"];
    }
    sum
  }
  
  #{
    items: msg["items"],
    subtotal: calculate_total(msg["items"]),
    tax: calculate_total(msg["items"]) * 0.08
  }
```

## Built-in StreamForge Functions

```yaml
# Null/empty checks
'not_null(msg["field"])'
'is_null_or_empty(msg["field"])'

# Timestamps
'now_ms()'                                    # Current time in milliseconds
'(now_ms() - timestamp) / 1000'               # Age in seconds

# Cache lookups
'cache_lookup("cache_name", msg["key"])'

# Hashing (for PII)
'hash_sha256(msg["email"].to_lower())'

# Envelope access
'headers["x-correlation-id"]'                 # Access Kafka headers
'key'                                         # Access Kafka message key
'timestamp'                                   # Access Kafka timestamp
```

## Null Handling

```yaml
# Null coalescing
'msg["value"] ?? 0'
'msg["name"] ?? "unknown"'

# Safe chaining
'msg["user"]?["email"] ?? "no-email"'

# Check before access
filter: 'not_null(msg["user"]) && msg["user"]["active"] == true'
```

## Common Patterns

### Email Validation
```yaml
filter: |
  not_null(msg["email"]) &&
  msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$")
```

### Age-based Filtering
```yaml
filter: '(now_ms() - msg["timestamp"]) / 1000 < 300'  # Last 5 minutes
```

### PII Masking
```yaml
transform: |
  #{
    userId: msg["userId"],
    emailHash: hash_sha256(msg["email"].to_lower()),
    phoneHash: hash_sha256(msg["phone"]),
    # email and phone not included - dropped
  }
```

### Cache Enrichment
```yaml
transform: |
  let profile = cache_lookup("profiles", msg["userId"]);
  msg + #{
    tier: profile["tier"] ?? "standard",
    plan: profile["plan"] ?? "free"
  }
```

### Multi-condition Routing
```yaml
filter: |
  (msg["tier"] == "premium" && msg["active"]) ||
  (msg["amount"] > 1000 && msg["verified"])
```

## Envelope Operations (String Format)

While filters and transforms use Rhai, envelope operations still use string format:

### Key Transform
```yaml
key_transform: '/userId'                      # Extract field
key_transform: 'HASH:SHA256,/email'           # Hash field
key_transform: 'CONSTRUCT:tenant=/tenant:user=/user'  # Composite
```

### Header Operations
```yaml
headers:
  x-pipeline: "streamforge"
header_transforms:
  - header: x-user-id
    operation: 'FROM:/user/id'
  - header: x-correlation-id
    operation: 'COPY:x-request-id'
  - header: x-sensitive
    operation: 'REMOVE'
```

### Timestamp Operations
```yaml
timestamp: 'PRESERVE'                         # Keep original
timestamp: 'CURRENT'                          # Set to now
timestamp: 'FROM:/event/timestamp'            # Extract from payload
```

## Performance Tips

1. **Simple filters are fastest** - Use simple comparisons when possible
2. **Early termination** - Put most selective conditions first with `&&`
3. **Avoid regex in hot paths** - Use string methods when possible
4. **Cache compiled regexes** - They're compiled at startup
5. **Minimize array operations** - `.any()` is faster than `.all()` on large arrays
6. **Pre-check lengths** - `msg["items"].len() > 0` before iterating

## Debugging

```yaml
# Add debug fields to see intermediate values
transform: |
  let calculated = msg["price"] * 1.08;
  msg + #{
    debug_calculated: calculated,
    debug_timestamp: now_ms()
  }
```

## Error Handling

```yaml
# Check types before operations
filter: 'not_null(msg["amount"]) && msg["amount"] > 0'

# Provide defaults
transform: 'msg["value"] ?? 0'

# Safe array access
transform: 'if msg["items"].len() > 0 { msg["items"][0] } else { () }'
```

## See Also

- [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) - Complete Rhai DSL documentation
- [DSL_FEATURES.md](DSL_FEATURES.md) - Feature overview
- [Rhai Language Documentation](https://rhai.rs/book/) - Official Rhai docs
- [README.md](../README.md) - Recipes and examples
