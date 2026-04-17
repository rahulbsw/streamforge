# Advanced Filtering with Rhai

## Overview

StreamForge uses **Rhai** - a JavaScript-like scripting language - for message filtering. Filters are Rhai expressions that evaluate to `true` (pass) or `false` (filter out).

## Basic Filtering

### Simple Comparisons

```yaml
# Numeric comparisons
filter: 'msg["siteId"] > 10000'
filter: 'msg["age"] >= 18'
filter: 'msg["count"] < 100'

# String equality
filter: 'msg["status"] == "active"'
filter: 'msg["tier"] == "premium"'

# Boolean checks
filter: 'msg["verified"] == true'
filter: 'msg["deleted"]'  # shorthand for == true
```

**Sample Messages:**

```yaml
# PASSES: msg["siteId"] > 10000
{"siteId": 15000, "status": "active"}

# FAILS: msg["siteId"] > 10000  
{"siteId": 5000, "status": "active"}
```

## Boolean Logic

### AND Logic

All conditions must be true for the message to pass.

**Using `&&` operator:**
```yaml
filter: 'msg["siteId"] > 10000 && msg["status"] == "active"'
```

**Using list (all must be true):**
```yaml
filter:
  - 'msg["siteId"] > 10000'
  - 'msg["status"] == "active"'
  - '!msg["test"]'
```

**Config Example:**
```yaml
routing:
  destinations:
    - output: high-value-active
      filter:
        - 'msg["siteId"] > 10000'
        - 'msg["status"] == "active"'
```

**Sample Messages:**
```json
// PASSES - both conditions true
{"siteId": 15000, "status": "active"}

// FAILS - siteId too low
{"siteId": 5000, "status": "active"}

// FAILS - status not active
{"siteId": 15000, "status": "inactive"}
```

### OR Logic

At least one condition must be true for the message to pass.

**Using `||` operator:**
```yaml
filter: 'msg["siteId"] > 10000 || msg["priority"] == "high"'
```

**Config Example:**
```yaml
routing:
  destinations:
    - output: priority-queue
      filter: 'msg["priority"] == "high" || msg["urgent"] == true'
```

**Sample Messages:**
```json
// PASSES - siteId matches
{"siteId": 15000, "priority": "low"}

// PASSES - priority matches
{"siteId": 5000, "priority": "high"}

// FAILS - neither matches
{"siteId": 5000, "priority": "low"}
```

### NOT Logic

Inverts the result of a condition.

**Using `!` operator:**
```yaml
filter: '!msg["test"]'
filter: '!msg["deleted"]'
filter: 'msg["status"] != "inactive"'
```

**Config Example:**
```yaml
routing:
  destinations:
    - output: production-events
      filter: '!msg["test"]'
```

**Sample Messages:**
```json
// PASSES - test is false
{"test": false, "data": "..."}

// FAILS - test is true
{"test": true, "data": "..."}

// PASSES - test field missing (treated as false)
{"data": "..."}
```

### Nested Logic

Combine AND, OR, and NOT for complex conditions.

**Example 1: Premium OR High Value**
```yaml
filter: |
  (msg["tier"] == "premium" && msg["status"] == "active") ||
  (msg["amount"] > 1000 && msg["verified"] == true)
```

**Example 2: Multiple ORs with AND**
```yaml
filter: |
  msg["siteId"] > 10000 &&
  (msg["status"] == "active" || msg["status"] == "pending" || msg["priority"] == "high")
```

**Example 3: Exclusions**
```yaml
filter: |
  msg["status"] == "active" &&
  !msg["test"] &&
  !msg["deleted"]
```

## String Matching

### Exact Match
```yaml
filter: 'msg["status"] == "active"'
filter: 'msg["type"] == "user.created"'
```

### Prefix/Suffix/Contains
```yaml
# Starts with
filter: 'msg["topic"].starts_with("prod.")'
filter: 'msg["email"].starts_with("admin")'

# Ends with
filter: 'msg["filename"].ends_with(".json")'
filter: 'msg["email"].ends_with("@company.com")'

# Contains substring
filter: 'msg["description"].contains("urgent")'
filter: 'msg["tags"].contains("important")'
```

### Case-Insensitive Matching
```yaml
filter: 'msg["status"].to_lower() == "active"'
filter: 'msg["email"].to_lower().ends_with("@company.com")'
```

### Multiple String Options
```yaml
# Using OR
filter: |
  msg["status"] == "active" ||
  msg["status"] == "pending" ||
  msg["status"] == "approved"

# Using 'in' operator (if available in your Rhai version)
filter: 'msg["status"] in ["active", "pending", "approved"]'
```

## Regular Expressions

Match string fields against regex patterns using the `.matches()` method.

### Email Validation
```yaml
filter: 'msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$")'
```

**Config Example:**
```yaml
routing:
  destinations:
    - output: valid-emails
      filter: 'msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$")'
```

### URL Matching
```yaml
# HTTPS URLs only
filter: 'msg["url"].matches("^https://")'

# Specific domain
filter: 'msg["url"].matches("@example\\.com")'
```

### Status Patterns
```yaml
# Status starting with "active"
filter: 'msg["status"].matches("^active")'

# Status ending with "pending"
filter: 'msg["status"].matches("pending$")'

# Error, failure, or exception
filter: 'msg["type"].matches("^(error|failure|exception)")'
```

### Phone Numbers
```yaml
# US phone numbers
filter: 'msg["phone"].matches("^\\+1[0-9]{10}$")'

# International format
filter: 'msg["phone"].matches("^\\+[0-9]{1,3}[0-9]{10,14}$")'
```

### Version Numbers
```yaml
# Semantic versioning (1.2.3)
filter: 'msg["version"].matches("^[0-9]+\\.[0-9]+\\.[0-9]+$")'

# Major version 2.x
filter: 'msg["version"].matches("^2\\.")'
```

## Array Filtering

### Check All Elements (.all)

All elements in the array must match the condition.

```yaml
# All users must be active
filter: 'msg["users"].all(|u| u["status"] == "active")'

# All items have positive quantity
filter: 'msg["items"].all(|i| i["quantity"] > 0)'
```

**Config Example:**
```yaml
routing:
  destinations:
    - output: fully-active-groups
      filter: 'msg["users"].all(|u| u["status"] == "active")'
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

### Check Any Element (.any)

At least one element in the array must match the condition.

```yaml
# At least one task is high priority
filter: 'msg["tasks"].any(|t| t["priority"] == "high")'

# At least one item is out of stock
filter: 'msg["items"].any(|i| i["stock"] <= 0)'
```

**Config Example:**
```yaml
routing:
  destinations:
    - output: urgent-tasks
      filter: 'msg["tasks"].any(|t| t["priority"] == "high")'
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

### Array Length and Emptiness
```yaml
# Has items
filter: 'msg["items"].len() > 0'
filter: '!msg["items"].is_empty()'

# Minimum number of items
filter: 'msg["items"].len() >= 5'

# Maximum number of items
filter: 'msg["tags"].len() <= 10'
```

### Array Contains
```yaml
# Array contains specific value
filter: 'msg["roles"].contains("admin")'
filter: 'msg["tags"].contains("urgent")'
```

## Numeric Filtering

### Numeric Comparisons
```yaml
# Greater than
filter: 'msg["amount"] > 100'

# Greater than or equal
filter: 'msg["age"] >= 18'

# Less than
filter: 'msg["temperature"] < 100'

# Less than or equal
filter: 'msg["score"] <= 100'

# Equal
filter: 'msg["count"] == 0'

# Not equal
filter: 'msg["status_code"] != 200'
```

### Range Checks
```yaml
# Between (inclusive)
filter: 'msg["age"] >= 18 && msg["age"] <= 65'

# Outside range
filter: 'msg["temperature"] < 0 || msg["temperature"] > 100'
```

### Arithmetic in Filters
```yaml
# Calculate and compare
filter: '(msg["price"] * msg["quantity"]) > 1000'

# Percentage check
filter: '(msg["errors"] / msg["total"]) < 0.01'  # Error rate < 1%
```

## Null and Missing Field Handling

### Check for Null
```yaml
# Field is not null
filter: 'not_null(msg["field"])'

# Field is null or empty
filter: 'is_null_or_empty(msg["field"])'
```

### Safe Field Access
```yaml
# Check field exists before comparing
filter: 'not_null(msg["user"]) && msg["user"]["active"] == true'

# Use null coalescing
filter: '(msg["priority"] ?? "normal") == "high"'
```

### Missing vs Null vs Empty
```yaml
# Field exists and is not empty
filter: 'not_null(msg["name"]) && msg["name"] != ""'

# Field exists and array is not empty
filter: 'not_null(msg["items"]) && msg["items"].len() > 0'
```

## Time-Based Filtering

### Message Age
```yaml
# Messages from last 5 minutes (300 seconds)
filter: '(now_ms() - timestamp) / 1000 < 300'

# Messages older than 1 hour (3600 seconds)
filter: '(now_ms() - timestamp) / 1000 >= 3600'
```

### Timestamp in Payload
```yaml
# Events from last 10 minutes
filter: '(now_ms() - msg["event_time"]) / 1000 < 600'

# Future events (scheduled)
filter: 'msg["scheduled_time"] > now_ms()'
```

## Header-Based Filtering

Access Kafka message headers for filtering.

```yaml
# Check header exists
filter: 'not_null(headers["x-correlation-id"])'

# Header value match
filter: 'headers["x-tenant"] == "production"'

# Header prefix
filter: 'headers["x-request-id"].starts_with("req-")'
```

**Config Example:**
```yaml
routing:
  destinations:
    - output: prod-events
      filter: 'headers["x-environment"] == "production"'
    
    - output: staging-events
      filter: 'headers["x-environment"] == "staging"'
```

## Key-Based Filtering

Access Kafka message key for filtering.

```yaml
# Check if message has a key
filter: 'not_null(key)'

# Key prefix
filter: 'key.starts_with("premium-")'

# Key suffix
filter: 'key.ends_with("-test")'

# Key contains
filter: 'key.contains("vip")'

# Key pattern
filter: 'key.matches("^user-[0-9]+$")'
```

**Config Example:**
```yaml
routing:
  destinations:
    - output: premium-users
      filter: 'key.starts_with("premium-")'
    
    - output: test-users
      filter: 'key.ends_with("-test")'
```

## Complex Real-World Examples

### Example 1: E-commerce Order Processing

Process orders with multiple business rules.

```yaml
routing:
  destinations:
    - output: high-value-orders
      description: Premium customers with large orders
      filter: |
        msg["order"]["total"] > 500 &&
        msg["customer"]["tier"] == "premium" &&
        msg["order"]["status"] == "confirmed" &&
        !msg["order"]["test"]
```

### Example 2: User Activity with Multiple Conditions

Filter user events based on complex criteria.

```yaml
routing:
  destinations:
    - output: active-premium-users
      filter: |
        msg["user"]["verified"] == true &&
        msg["user"]["tier"] in ["premium", "enterprise"] &&
        msg["sessions"].any(|s| s["active"] == true) &&
        msg["sessions"].len() > 0 &&
        (now_ms() - msg["lastLogin"]) / 1000 < 86400
```

### Example 3: Email Validation Pipeline

Validate and route emails based on patterns and rules.

```yaml
routing:
  destinations:
    - output: corporate-emails
      filter: |
        msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$") &&
        msg["email"].to_lower().ends_with("@company.com") &&
        !msg["email"].starts_with("test") &&
        msg["verified"] == true
    
    - output: suspicious-emails
      filter: |
        msg["email"].matches("@(test|temp|fake|disposable)\\.") ||
        msg["bounced"] == true ||
        msg["spam_score"] > 0.7
```

### Example 4: Multi-Tenant Event Routing

Route events based on tenant and environment.

```yaml
routing:
  destinations:
    - output: prod-tenant-a
      filter: |
        headers["x-tenant"] == "tenant-a" &&
        headers["x-environment"] == "production" &&
        !msg["test"] &&
        (now_ms() - timestamp) / 1000 < 600
    
    - output: staging-tenant-a
      filter: |
        headers["x-tenant"] == "tenant-a" &&
        headers["x-environment"] == "staging"
```

### Example 5: Error and Quality Monitoring

Filter events for monitoring and alerting.

```yaml
routing:
  destinations:
    - output: critical-errors
      filter: |
        (msg["severity"] == "error" || msg["severity"] == "fatal") &&
        msg["environment"] == "production" &&
        !msg["acknowledged"]
    
    - output: quality-degradation
      filter: |
        msg["metrics"]["error_rate"] > 0.05 ||
        msg["metrics"]["p99_latency"] > 5000 ||
        msg["metrics"]["throughput"] < 100
```

## Performance Tips

1. **Simple filters are fastest** - Use direct comparisons when possible
2. **Early termination with &&** - Put most selective conditions first
   ```yaml
   # Good - rare condition first
   filter: 'msg["vip"] == true && msg["amount"] > 100'
   
   # Less efficient - common condition first  
   filter: 'msg["amount"] > 100 && msg["vip"] == true'
   ```

3. **Avoid regex in hot paths** - Use string methods when possible
   ```yaml
   # Faster
   filter: 'msg["email"].ends_with("@company.com")'
   
   # Slower
   filter: 'msg["email"].matches("@company\\.com$")'
   ```

4. **Short-circuit with ||** - First true condition stops evaluation
   ```yaml
   filter: 'msg["priority"] == "critical" || msg["amount"] > 10000 || msg["vip"]'
   ```

5. **Pre-check array lengths**
   ```yaml
   filter: 'msg["items"].len() > 0 && msg["items"].all(|i| i["valid"])'
   ```

## Debugging Filters

### Enable Debug Logging
```bash
RUST_LOG=debug CONFIG_FILE=config.yaml ./streamforge
```

### Add Debug Fields
```yaml
# Add calculated values to see intermediate results
transform: |
  let is_premium = msg["tier"] == "premium";
  let high_value = msg["amount"] > 1000;
  msg + #{
    debug_is_premium: is_premium,
    debug_high_value: high_value,
    debug_passes: is_premium && high_value
  }
```

### Test with Catchall
```yaml
routing:
  destinations:
    # Your filtered destinations
    - output: filtered-topic
      filter: '...'
    
    # Catchall to see what doesn't match
    - output: debug-unmatched
      # No filter = accepts all
```

## Common Pitfalls

### Missing Quotes Around Strings
```yaml
# Wrong - 'active' is treated as variable
filter: 'msg["status"] == active'

# Correct
filter: 'msg["status"] == "active"'
```

### Type Mismatches
```yaml
# Wrong - comparing number to string
filter: 'msg["amount"] == "100"'

# Correct
filter: 'msg["amount"] == 100'
```

### Null Pointer Errors
```yaml
# Unsafe - crashes if 'user' is null
filter: 'msg["user"]["active"] == true'

# Safe - checks null first
filter: 'not_null(msg["user"]) && msg["user"]["active"] == true'
```

## See Also

- [DSL_FEATURES.md](DSL_FEATURES.md) - Complete DSL feature overview
- [RHAI_QUICK_REFERENCE.md](RHAI_QUICK_REFERENCE.md) - Quick syntax reference
- [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) - Complete Rhai guide including transforms
- [Rhai Book](https://rhai.rs/book/) - Official Rhai documentation
