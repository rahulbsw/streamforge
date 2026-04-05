---
title: Configuration
nav_order: 4
---

# YAML Configuration Guide

StreamForge now supports both JSON and YAML configuration formats. YAML is recommended for complex configurations with multiple filters and transformations due to its superior readability.

## Table of Contents

- [Why YAML?](#why-yaml)
- [Format Detection](#format-detection)
- [Basic Examples](#basic-examples)
- [Readability Comparison](#readability-comparison)
- [YAML Features](#yaml-features)
- [Migration from JSON](#migration-from-json)
- [Best Practices](#best-practices)

## Why YAML?

### Advantages of YAML

✅ **More Readable**: No brackets, cleaner syntax
✅ **Comments**: Add descriptions inline
✅ **Multi-line Strings**: Complex filters more readable
✅ **Less Noise**: No quotes on keys, fewer commas
✅ **Better for Complex Configs**: Easier to maintain

### When to Use YAML

- Multiple destinations (3+)
- Complex filters with boolean logic
- Long transformation expressions
- Team collaboration (easier code reviews)
- Configuration as documentation

### When to Use JSON

- Simple single-destination configs
- Programmatic generation
- API responses
- Strict typing requirements

## Format Detection

The configuration parser automatically detects the format based on file extension:

```bash
# YAML format
CONFIG_FILE=config.yaml ./streamforge

# JSON format (backward compatible)
CONFIG_FILE=config.json ./streamforge
```

**Supported extensions:**
- `.yaml` → YAML format
- `.yml` → YAML format
- `.json` → JSON format

## Basic Examples

### Simple Configuration

**YAML** (config.yaml):
```yaml
appid: streamforge
bootstrap: kafka:9092
input: source-topic
output: destination-topic
offset: latest
threads: 4

compression:
  compression_type: raw
  compression_algo: gzip
```

**JSON** (config.json):
```json
{
  "appid": "streamforge",
  "bootstrap": "kafka:9092",
  "input": "source-topic",
  "output": "destination-topic",
  "offset": "latest",
  "threads": 4,
  "compression": {
    "compression_type": "raw",
    "compression_algo": "gzip"
  }
}
```

### With Consumer Properties

**YAML**:
```yaml
consumer_properties:
  fetch.min.bytes: "1048576"
  fetch.wait.max.ms: "500"
  max.poll.records: "500"
```

**JSON**:
```json
{
  "consumer_properties": {
    "fetch.min.bytes": "1048576",
    "fetch.wait.max.ms": "500",
    "max.poll.records": "500"
  }
}
```

## Readability Comparison

### Example: Multi-Destination Routing

**YAML** (Much More Readable!):
```yaml
routing:
  routing_type: content
  destinations:
    # Validated users only
    - output: validated-users
      description: Users with valid email format
      filter: "REGEX:/user/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$"
      transform: "CONSTRUCT:email=/user/email:name=/user/name:id=/user/id"
      partition: /user/id

    # High-value orders
    - output: premium-orders
      description: Orders over $500 that are confirmed
      filter: "AND:/order/total,>,500:/order/status,==,confirmed"
      transform: |
        CONSTRUCT:orderId=/order/id:total=/order/total:customer=/customer/email
      partition: /order/id

    # Bulk discount calculation
    - output: discounted-prices
      description: Apply 10% discount for bulk orders
      filter: "AND:/order/items,>=,10:/order/total,>,100"
      transform: "ARITHMETIC:MUL,/order/total,0.9"
      partition: /order/id
```

**JSON** (Harder to Read):
```json
{
  "routing": {
    "routing_type": "content",
    "destinations": [
      {
        "output": "validated-users",
        "description": "Users with valid email format",
        "filter": "REGEX:/user/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$",
        "transform": "CONSTRUCT:email=/user/email:name=/user/name:id=/user/id",
        "partition": "/user/id"
      },
      {
        "output": "premium-orders",
        "description": "Orders over $500 that are confirmed",
        "filter": "AND:/order/total,>,500:/order/status,==,confirmed",
        "transform": "CONSTRUCT:orderId=/order/id:total=/order/total:customer=/customer/email",
        "partition": "/order/id"
      },
      {
        "output": "discounted-prices",
        "description": "Apply 10% discount for bulk orders",
        "filter": "AND:/order/items,>=,10:/order/total,>,100",
        "transform": "ARITHMETIC:MUL,/order/total,0.9",
        "partition": "/order/id"
      }
    ]
  }
}
```

### Example: Complex Boolean Logic

**YAML**:
```yaml
- output: premium-or-bulk
  description: Premium users OR bulk orders
  filter: |
    OR:AND:/user/tier,==,premium:/user/status,==,active:AND:/order/items,>=,10:/order/total,>,500
  transform: |
    CONSTRUCT:userId=/user/id:tier=/user/tier:orderTotal=/order/total:itemCount=/order/items
```

**JSON**:
```json
{
  "output": "premium-or-bulk",
  "description": "Premium users OR bulk orders",
  "filter": "OR:AND:/user/tier,==,premium:/user/status,==,active:AND:/order/items,>=,10:/order/total,>,500",
  "transform": "CONSTRUCT:userId=/user/id:tier=/user/tier:orderTotal=/order/total:itemCount=/order/items"
}
```

Notice how YAML:
- ✅ No quotes around field names
- ✅ Comments inline with `#`
- ✅ Multi-line strings with `|`
- ✅ Clear visual separation
- ✅ Less punctuation noise

## YAML Features

### 1. Comments

```yaml
routing:
  destinations:
    # Email validation pipeline
    - output: validated-users
      # Uses regex to validate email format
      filter: "REGEX:/user/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$"
      # Extract only essential fields
      transform: "CONSTRUCT:email=/user/email:name=/user/name"
```

### 2. Multi-line Strings

**With `|` (preserve newlines):**
```yaml
filter: |
  AND:REGEX:/event/type,^(create|update|delete)$:OR:/event/source,==,api:/event/source,==,web:NOT:/event/test,==,true
```

**With `>` (fold newlines):**
```yaml
description: >
  This destination processes all premium users
  who have active subscriptions and have made
  purchases in the last 30 days.
```

### 3. Descriptive Structure

```yaml
destinations:
  # ============================================
  # USER PROCESSING
  # ============================================

  - output: active-users
    description: Active users only
    filter: "/user/active,==,true"

  - output: premium-users
    description: Premium tier users
    filter: "/user/tier,==,premium"

  # ============================================
  # ORDER PROCESSING
  # ============================================

  - output: high-value-orders
    description: Orders over $1000
    filter: "/order/total,>,1000"
```

### 4. Anchors and Aliases (Advanced)

Reuse common configurations:

```yaml
# Define reusable configs
x-common-consumer-settings: &common-consumer
  fetch.min.bytes: "1048576"
  fetch.wait.max.ms: "500"

x-common-producer-settings: &common-producer
  batch.size: "65536"
  linger.ms: "10"

# Use them
consumer_properties:
  <<: *common-consumer
  max.poll.records: "500"

producer_properties:
  <<: *common-producer
  compression.type: "gzip"
```

### 5. Optional Values

```yaml
# Optional fields can be omitted
- output: simple-destination
  filter: "/field,==,value"
  # No transform, no partition - that's OK!
```

## Migration from JSON

### Step 1: Convert Format

**Automatic conversion tools:**

```bash
# Using yq (install: brew install yq)
cat config.json | yq -P > config.yaml

# Using python
python3 -c "import json, yaml, sys; yaml.dump(json.load(sys.stdin), sys.stdout, default_flow_style=False)" < config.json > config.yaml
```

### Step 2: Add Comments

```yaml
# Add descriptive comments
- output: validated-users
  description: Users with valid email format  # This shows in logs
  filter: "REGEX:/user/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$"
```

### Step 3: Use Multi-line for Complex Filters

**Before:**
```yaml
filter: "AND:REGEX:/event/type,^(create|update|delete)$:OR:/event/source,==,api:/event/source,==,web:NOT:/event/test,==,true"
```

**After:**
```yaml
filter: |
  AND:REGEX:/event/type,^(create|update|delete)$:OR:/event/source,==,api:/event/source,==,web:NOT:/event/test,==,true
```

Even better with comments:
```yaml
# Match CRUD operations from API or web sources, excluding tests
filter: |
  AND:REGEX:/event/type,^(create|update|delete)$:OR:/event/source,==,api:/event/source,==,web:NOT:/event/test,==,true
```

### Step 4: Organize with Sections

```yaml
routing:
  destinations:
    # ============================================
    # VALIDATION PIPELINE
    # ============================================
    - output: validated-users
      # ... config ...

    # ============================================
    # ANALYTICS PIPELINE
    # ============================================
    - output: analytics-events
      # ... config ...
```

## Best Practices

### 1. Use Comments Liberally

```yaml
- output: premium-orders
  # Business rule: Orders over $500 require manual review
  filter: "AND:/order/total,>,500:/order/status,==,confirmed"
  # Extract fields needed for review dashboard
  transform: "CONSTRUCT:orderId=/order/id:total=/order/total:customer=/customer/email"
```

### 2. Group Related Destinations

```yaml
destinations:
  # User validation and routing
  - output: validated-users
    # ... config ...
  - output: corporate-users
    # ... config ...

  # Order processing
  - output: premium-orders
    # ... config ...
  - output: bulk-orders
    # ... config ...
```

### 3. Use Descriptive Names

```yaml
# Good
- output: high-value-confirmed-orders
  description: Orders over $500 that are confirmed

# Less clear
- output: orders1
  description: special orders
```

### 4. Document Complex Filters

```yaml
- output: complex-routing
  # This filter routes:
  # 1. CRUD operations (create/update/delete)
  # 2. From API or web sources
  # 3. Excluding test events
  filter: |
    AND:REGEX:/event/type,^(create|update|delete)$:OR:/event/source,==,api:/event/source,==,web:NOT:/event/test,==,true
```

### 5. Consistent Indentation

Use 2 spaces (YAML standard):

```yaml
routing:
  destinations:
    - output: topic1
      filter: "..."
      transform: "..."
```

### 6. Multi-line for Long Expressions

**Hard to read:**
```yaml
transform: "CONSTRUCT:userId=/user/id:userName=/user/name:userEmail=/user/email:userTier=/user/tier:orderTotal=/order/total:orderStatus=/order/status"
```

**Better:**
```yaml
transform: |
  CONSTRUCT:userId=/user/id:userName=/user/name:userEmail=/user/email:userTier=/user/tier:orderTotal=/order/total:orderStatus=/order/status
```

**Even better with structure:**
```yaml
# Extract user and order summary
transform: |
  CONSTRUCT:userId=/user/id:userName=/user/name:userEmail=/user/email:userTier=/user/tier:orderTotal=/order/total:orderStatus=/order/status
```

## Example Configurations

### Example 1: Development Config

```yaml
# Development environment configuration
appid: streamforge-dev
bootstrap: localhost:9092
input: dev-events
offset: earliest  # Start from beginning for dev

# Low resource usage for local dev
threads: 2

routing:
  destinations:
    # Just copy everything for testing
    - output: dev-mirror
      description: Simple mirror for development
```

### Example 2: Production Config

```yaml
# Production environment configuration
appid: streamforge-prod
bootstrap: prod-kafka-1:9092,prod-kafka-2:9092,prod-kafka-3:9092
target_broker: prod-kafka-target-1:9092,prod-kafka-target-2:9092
input: production-events
offset: latest

# Production-grade settings
threads: 8

compression:
  compression_type: raw
  compression_algo: snappy  # Fast compression for production

routing:
  routing_type: content
  destinations:
    # Production destinations with business logic
    - output: validated-users
      description: Production user validation pipeline
      filter: |
        AND:REGEX:/user/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$:NOT:REGEX:/user/email,@(test|temp)\\.
      transform: "CONSTRUCT:id=/user/id:email=/user/email"
      partition: /user/id

    # Audit trail for compliance
    - output: audit-trail
      description: Complete audit trail for compliance
      transform: /

# Production Kafka settings
consumer_properties:
  fetch.min.bytes: "1048576"
  fetch.wait.max.ms: "500"
  max.poll.records: "1000"
  session.timeout.ms: "30000"

producer_properties:
  batch.size: "131072"
  linger.ms: "10"
  compression.type: "snappy"
  acks: "1"
```

## Validation

### YAML Syntax Validation

```bash
# Check syntax with yamllint (install: pip install yamllint)
yamllint config.yaml

# Check with yq
yq eval config.yaml

# Test loading
./streamforge --help  # If no error, config is valid
```

### Common YAML Errors

**Wrong indentation:**
```yaml
# Wrong
routing:
destinations:  # Should be indented
  - output: topic1
```

**Missing quotes for special characters:**
```yaml
# Wrong - colons need quotes
filter: /path:value

# Right
filter: "/path:value"
```

**Mixing tabs and spaces:**
```yaml
# Use spaces only, never tabs
```

## Comparison Summary

| Feature | JSON | YAML |
|---------|------|------|
| Readability | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Comments | ❌ | ✅ |
| Multi-line strings | ❌ | ✅ |
| Less punctuation | ❌ | ✅ |
| Visual structure | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Complex configs | ⭐⭐ | ⭐⭐⭐⭐⭐ |
| Code reviews | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Programmatic | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| Strict typing | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |

## Recommendation

**Use YAML for:**
- ✅ Production configurations
- ✅ Multi-destination routing (3+ destinations)
- ✅ Complex filters and transforms
- ✅ Team collaboration
- ✅ Configuration documentation

**Use JSON for:**
- ✅ Simple single-destination configs
- ✅ Programmatically generated configs
- ✅ CI/CD templates
- ✅ Strict schema validation

## See Also

- [config.example.yaml](../examples/configs/config.example.yaml) - Simple YAML example
- [config.multidest.yaml](../examples/configs/config.multidest.yaml) - Multi-destination YAML
- [config.advanced.yaml](../examples/configs/config.advanced.yaml) - Advanced YAML with all features
- [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) - Filter and transform syntax
- [USAGE.md](USAGE.md) - Complete use cases

---

**Try it now!** Copy one of the YAML examples and run:
```bash
CONFIG_FILE=config.yaml ./streamforge
```
