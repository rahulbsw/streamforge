# Advanced DSL Guide

This guide covers advanced filtering and transformation capabilities including array operations, regular expressions, and arithmetic operations.

## Table of Contents

- [Array Operations](#array-operations)
  - [Array Filters](#array-filters)
  - [Array Transforms](#array-transforms)
- [Regular Expressions](#regular-expressions)
- [Arithmetic Operations](#arithmetic-operations)
- [Envelope Operations](#envelope-operations)
  - [Key Filters](#key-filters)
  - [Key Transforms](#key-transforms)
  - [Header Filters](#header-filters)
  - [Header Transforms](#header-transforms)
  - [Timestamp Filters](#timestamp-filters)
  - [Timestamp Transforms](#timestamp-transforms)
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

## Envelope Operations

Streamforge supports filtering and transforming the complete Kafka message envelope, not just the payload. The envelope includes:
- **Key**: Message routing key
- **Value**: Message payload (the main content)
- **Headers**: Metadata key-value pairs
- **Timestamp**: Message timestamp in milliseconds

This enables advanced routing scenarios like multi-tenant filtering, correlation tracking, and time-based routing.

### Key Filters

Filter messages based on the Kafka message key (not the value/payload).

#### KEY_EXISTS

Check if the message has a key (non-null).

**Syntax:** `KEY_EXISTS`

**Example:**
```yaml
- output: keyed-messages
  filter: "KEY_EXISTS"
```

#### KEY_PREFIX

Match keys that start with a specific prefix.

**Syntax:** `KEY_PREFIX:prefix`

**Examples:**
```yaml
# Premium users
- output: premium-events
  filter: "KEY_PREFIX:premium-"

# Production environment
- output: prod-events
  filter: "KEY_PREFIX:prod:"
```

#### KEY_SUFFIX

Match keys that end with a specific suffix.

**Syntax:** `KEY_SUFFIX:suffix`

**Examples:**
```yaml
# Test users
- output: test-events
  filter: "KEY_SUFFIX:-test"
```

#### KEY_CONTAINS

Match keys that contain a substring.

**Syntax:** `KEY_CONTAINS:substring`

**Examples:**
```yaml
# VIP users
- output: vip-events
  filter: "KEY_CONTAINS:vip"
```

#### KEY_MATCHES

Match keys against a regular expression pattern.

**Syntax:** `KEY_MATCHES:pattern`

**Examples:**
```yaml
# User IDs (user-123)
- output: user-events
  filter: "KEY_MATCHES:^user-[0-9]+$"

# Email addresses
- output: email-events
  filter: "KEY_MATCHES:^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"
```

**Example: Combining key and value filters**
```yaml
- output: premium-active-users
  filter: "AND:KEY_PREFIX:premium-:/user/active,==,true"
  # Must have premium key AND be active in payload
```

### Key Transforms

Transform the message key per destination. Keys are used for:
- **Partitioning**: Determining which partition receives the message
- **Compaction**: In compacted topics, latest value per key is retained
- **Routing**: Downstream systems use keys for lookups and joins

#### Extract Key from Value

Extract a field from the message payload and use it as the key.

**Syntax:** `key_transform: "/path/to/field"`

**Examples:**
```yaml
# Use user ID as key
- output: user-events
  key_transform: "/user/id"

# Use nested field
- output: order-events
  key_transform: "/order/customerId"
```

**Transformation:**
```json
// Input (key=null, value={...})
{
  "user": {
    "id": "user-123",
    "name": "Alice"
  }
}

// Output (key="user-123", value={...})
```

#### Construct Composite Key

Build a JSON key from multiple fields.

**Syntax:** `key_transform: "CONSTRUCT:field1=/path1:field2=/path2:..."`

**Examples:**
```yaml
# Multi-tenant key
- output: tenant-events
  key_transform: "CONSTRUCT:tenant=/tenant/id:user=/user/id"

# Composite routing key
- output: routed-events
  key_transform: "CONSTRUCT:region=/region:customerId=/customer/id:timestamp=/event/ts"
```

**Transformation:**
```json
// Input
{
  "tenant": {"id": "acme"},
  "user": {"id": "user-123"}
}

// Output key
{
  "tenant": "acme",
  "user": "user-123"
}
```

#### Template-Based Key

Build a string key using template placeholders.

**Syntax:** `key_transform: "template-{/path1}-{/path2}"`

**Examples:**
```yaml
# Simple template
- output: formatted-events
  key_transform: "user-{/user/id}"

# Multi-field template
- output: tenant-partitioned
  key_transform: "{/tenant}-{/region}-{/user/id}"
```

**Transformation:**
```json
// Input
{
  "tenant": "acme",
  "user": {"id": "123"}
}

// Output key (string)
"acme-123"
```

#### Hash Key for Privacy

Hash a field from the payload for privacy/anonymization.

**Syntax:** `key_transform: "HASH:algorithm,/path"`

**Algorithms:** `MD5`, `SHA256`, `SHA512`, `MURMUR64`, `MURMUR128`

**Examples:**
```yaml
# Hash email for privacy
- output: anonymized-events
  key_transform: "HASH:SHA256,/user/email"
  headers:
    x-anonymized: "true"

# Fast hashing for partitioning
- output: partitioned-events
  key_transform: "HASH:MURMUR128,/user/id"
```

#### Constant Key

Set a static key for all messages to a destination.

**Syntax:** `key_transform: "constant-value"`

**Examples:**
```yaml
# All messages get same key (useful for compacted topics with single value)
- output: config-topic
  key_transform: "app-config"
```

### Header Filters

Filter messages based on Kafka headers (metadata).

#### HEADER_EXISTS

Check if a specific header exists.

**Syntax:** `HEADER_EXISTS:header-name`

**Examples:**
```yaml
# Messages with correlation ID
- output: correlated-events
  filter: "HEADER_EXISTS:x-correlation-id"

# Messages with tenant ID
- output: tenant-events
  filter: "HEADER_EXISTS:x-tenant-id"
```

#### HEADER

Filter by header value with comparison.

**Syntax:** `HEADER:header-name,operator,value`

**Operators:** `==`, `!=`, `>`, `>=`, `<`, `<=`

**Examples:**
```yaml
# Production tenant only
- output: prod-events
  filter: "HEADER:x-tenant,==,production"

# High priority messages
- output: priority-events
  filter: "HEADER:x-priority,>=,8"

# Exclude test environment
- output: non-test-events
  filter: "NOT:HEADER:x-environment,==,test"
```

**Multi-tenant routing example:**
```yaml
routing:
  routing_type: filter
  destinations:
    - output: prod-tenant
      filter: "HEADER:x-tenant,==,production"
    
    - output: staging-tenant
      filter: "HEADER:x-tenant,==,staging"
    
    - output: test-tenant
      filter: "HEADER:x-tenant,==,test"
```

### Header Transforms

Add, modify, or remove message headers per destination.

#### Static Headers

Add fixed headers to all messages.

**Syntax:**
```yaml
headers:
  header-name: "value"
  another-header: "another-value"
```

**Examples:**
```yaml
- output: tracked-events
  headers:
    x-processing-pipeline: "streamforge"
    x-version: "1.0"
    x-environment: "production"
```

#### Dynamic Header from Value

Extract a field from the payload and set it as a header.

**Syntax:** `operation: "FROM:/path/to/field"`

**Examples:**
```yaml
- output: enriched-events
  header_transforms:
    # Extract user ID to header
    - header: x-user-id
      operation: "FROM:/user/id"
    
    # Extract tenant ID to header
    - header: x-tenant-id
      operation: "FROM:/tenant/id"
    
    # Extract nested field
    - header: x-trace-id
      operation: "FROM:/metadata/tracing/traceId"
```

**Transformation:**
```json
// Input (no headers)
{
  "user": {"id": "user-123"},
  "event": "login"
}

// Output (headers added)
Headers: {
  "x-user-id": "user-123"
}
```

#### Copy Header

Copy value from one header to another.

**Syntax:** `operation: "COPY:source-header"`

**Examples:**
```yaml
- output: correlated-events
  header_transforms:
    # Copy request ID to correlation ID
    - header: x-correlation-id
      operation: "COPY:x-request-id"
    
    # Duplicate trace ID
    - header: x-parent-trace-id
      operation: "COPY:x-trace-id"
```

**Useful for:**
- Correlation tracking between systems
- Header normalization (different systems use different header names)
- Maintaining tracing context

#### Remove Header

Remove sensitive or unnecessary headers.

**Syntax:** `operation: "REMOVE"`

**Examples:**
```yaml
- output: public-events
  header_transforms:
    # Remove internal authentication token
    - header: x-internal-token
      operation: "REMOVE"
    
    # Remove sensitive user info
    - header: x-user-email
      operation: "REMOVE"
```

**Full example with multiple header operations:**
```yaml
- output: enriched-and-cleaned-events
  # Add static headers
  headers:
    x-pipeline: "streamforge"
    x-version: "2.0"
  
  # Dynamic header operations
  header_transforms:
    # Extract from payload
    - header: x-user-id
      operation: "FROM:/user/id"
    
    # Copy for correlation
    - header: x-correlation-id
      operation: "COPY:x-request-id"
    
    # Remove sensitive data
    - header: x-internal-auth
      operation: "REMOVE"
```

### Timestamp Filters

Filter messages based on Kafka message timestamp.

#### TIMESTAMP_AGE

Filter by message age in seconds.

**Syntax:** `TIMESTAMP_AGE:operator,seconds`

**Operators:** `>`, `>=`, `<`, `<=`, `==`, `!=`

**Examples:**
```yaml
# Recent messages (last 5 minutes)
- output: recent-events
  filter: "TIMESTAMP_AGE:<,300"

# Old messages (older than 1 hour)
- output: historical-events
  filter: "TIMESTAMP_AGE:>=,3600"

# Messages exactly 10 seconds old (rarely useful)
- output: exactly-timed-events
  filter: "TIMESTAMP_AGE:==,10"
```

**Use cases:**
- Real-time vs batch processing routing
- Late-arriving data handling
- Time-sensitive event routing

#### TIMESTAMP_AFTER

Filter messages after a specific timestamp (epoch milliseconds).

**Syntax:** `TIMESTAMP_AFTER:epoch_ms`

**Examples:**
```yaml
# After January 1, 2024
- output: new-events
  filter: "TIMESTAMP_AFTER:1704067200000"

# Use for reprocessing from specific point
- output: reprocess-events
  filter: "TIMESTAMP_AFTER:1720000000000"
```

#### TIMESTAMP_BEFORE

Filter messages before a specific timestamp.

**Syntax:** `TIMESTAMP_BEFORE:epoch_ms`

**Examples:**
```yaml
# Before January 1, 2024
- output: old-events
  filter: "TIMESTAMP_BEFORE:1704067200000"
```

**Example: Time-based routing**
```yaml
destinations:
  # Real-time processing (last 5 minutes)
  - output: realtime-topic
    filter: "TIMESTAMP_AGE:<,300"
  
  # Batch processing (older than 5 minutes)
  - output: batch-topic
    filter: "TIMESTAMP_AGE:>=,300"
```

### Timestamp Transforms

Modify message timestamps per destination.

#### PRESERVE

Keep the original message timestamp (default behavior if not specified).

**Syntax:** `timestamp: "PRESERVE"`

**Example:**
```yaml
- output: preserved-timestamp-events
  timestamp: "PRESERVE"
```

#### CURRENT

Set timestamp to current time when producing to destination.

**Syntax:** `timestamp: "CURRENT"`

**Examples:**
```yaml
# Reset timestamp for reprocessed data
- output: reprocessed-events
  timestamp: "CURRENT"

# Update timestamp for test events
- output: test-events
  filter: "HEADER:x-environment,==,test"
  timestamp: "CURRENT"
```

**Use cases:**
- Reprocessing old data (reset event age)
- Test message generation
- Timestamp normalization

#### FROM

Extract timestamp from a field in the payload.

**Syntax:** `timestamp: "FROM:/path/to/timestamp"`

**Examples:**
```yaml
# Use event timestamp from payload
- output: event-time-routed
  timestamp: "FROM:/event/timestamp"

# Use custom timestamp field
- output: custom-timestamp-events
  timestamp: "FROM:/metadata/processedAt"
```

**Input:**
```json
{
  "event": {
    "timestamp": 1704067200000,
    "type": "order"
  }
}
```

**Result:** Message written with timestamp `1704067200000` (from payload)

#### ADD

Add seconds to the current timestamp.

**Syntax:** `timestamp: "ADD:seconds"`

**Examples:**
```yaml
# Add 1 hour (for timezone adjustment)
- output: timezone-adjusted-events
  timestamp: "ADD:3600"

# Add 1 day
- output: future-scheduled-events
  timestamp: "ADD:86400"
```

#### SUBTRACT

Subtract seconds from the current timestamp.

**Syntax:** `timestamp: "SUBTRACT:seconds"`

**Examples:**
```yaml
# Subtract 5 minutes
- output: backdated-events
  timestamp: "SUBTRACT:300"

# Subtract 1 hour
- output: past-events
  timestamp: "SUBTRACT:3600"
```

**Full timestamp manipulation example:**
```yaml
destinations:
  # Preserve original for audit trail
  - output: audit-events
    timestamp: "PRESERVE"
  
  # Reset for reprocessing
  - output: reprocess-events
    timestamp: "CURRENT"
  
  # Use event time from payload
  - output: event-time-events
    timestamp: "FROM:/event/occurredAt"
  
  # Timezone adjustment (UTC+1)
  - output: europe-events
    timestamp: "ADD:3600"
```

### Complete Envelope Example

**Multi-tenant event routing with full envelope features:**

```yaml
appid: multi-tenant-router
bootstrap: localhost:9092
target_broker: localhost:9093
input: raw-events
threads: 4

routing:
  routing_type: filter
  destinations:
    # Production tenant with enrichment
    - output: prod-tenant-events
      description: "Production tenant events with tracking"
      
      # Filter: Production tenant AND recent (last 10 min)
      filter: "AND:HEADER:x-tenant,==,production:TIMESTAMP_AGE:<,600"
      
      # Key: Composite key for partitioning
      key_transform: "CONSTRUCT:tenant=/tenant/id:user=/user/id"
      
      # Static headers: Add tracking metadata
      headers:
        x-environment: "production"
        x-pipeline: "streamforge"
        x-version: "2.0"
      
      # Dynamic headers: Extract from payload
      header_transforms:
        - header: x-user-id
          operation: "FROM:/user/id"
        - header: x-correlation-id
          operation: "COPY:x-request-id"
      
      # Timestamp: Preserve original
      timestamp: "PRESERVE"
    
    # Test tenant with anonymization
    - output: test-tenant-events
      filter: "HEADER:x-tenant,==,test"
      
      # Key: Hash email for privacy
      key_transform: "HASH:SHA256,/user/email"
      
      headers:
        x-environment: "test"
        x-anonymized: "true"
      
      # Timestamp: Reset to current
      timestamp: "CURRENT"
    
    # Premium users with age-based routing
    - output: premium-recent-events
      filter: "AND:KEY_PREFIX:premium-:TIMESTAMP_AGE:<,300"
      
      key_transform: "/user/id"
      
      headers:
        x-tier: "premium"
        x-freshness: "recent"
      
      timestamp: "PRESERVE"
    
    # Historical data (older than 1 hour)
    - output: historical-events
      filter: "TIMESTAMP_AGE:>=,3600"
      
      key_transform: "/event/id"
      
      headers:
        x-processing-mode: "batch"
        x-freshness: "historical"
      
      # Update timestamp to current (reset age)
      timestamp: "CURRENT"
```

### Envelope Best Practices

1. **Key Design**
   - Use composite keys for multi-tenant systems: `CONSTRUCT:tenant=/tenant:user=/user`
   - Hash sensitive fields: `HASH:SHA256,/user/email`
   - Keep keys small (< 100 bytes) for performance

2. **Header Management**
   - Remove sensitive headers before external routing
   - Use correlation IDs for tracing: `COPY:x-request-id`
   - Add pipeline metadata for debugging

3. **Timestamp Handling**
   - Use `PRESERVE` for audit trails
   - Use `CURRENT` for reprocessing/replays
   - Use `FROM:/field` for event-time processing

4. **Filtering Strategy**
   - Combine envelope and value filters: `AND:HEADER:x-tenant,==,prod:/user/active,==,true`
   - Filter on headers first (faster than payload parsing)
   - Use timestamp filters for time-based routing

5. **Performance**
   - Envelope operations add ~5-10% overhead vs value-only
   - Header operations are very fast (no JSON parsing)
   - Key hashing is fast (< 100μs per message)

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
