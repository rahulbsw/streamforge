# Key and Header Filtering Guide

## Overview

StreamForge provides access to the **complete Kafka message envelope** in Rhai filters and transforms, not just the payload. This enables powerful routing patterns based on:

- **`msg`** - Message payload (JSON object)
- **`key`** - Kafka message key
- **`headers`** - Kafka headers (metadata)
- **`timestamp`** - Kafka message timestamp

## Available Variables in Rhai Scripts

Every filter and transform has access to these variables:

| Variable | Type | Description |
|----------|------|-------------|
| `msg` | Map | Message payload as JSON object |
| `key` | String | Message key (empty string `""` if absent) |
| `headers` | Map | Kafka headers: `header-name` → UTF-8 string value |
| `timestamp` | i64 | Message timestamp in milliseconds since Unix epoch |

## Key-Based Filtering

The Kafka message **key** is available as a `String` variable named `key`.

### Basic Key Checks

```yaml
# Check if key exists (not empty)
filter: 'key != ""'
filter: 'not_empty(key)'

# Check if key is missing/empty
filter: 'key == ""'
filter: 'is_null_or_empty(key)'
```

### Key Prefix Matching

```yaml
# Route premium users (key starts with "premium-")
filter: 'key.starts_with("premium-")'

# Route production events (key starts with "prod:")
filter: 'key.starts_with("prod:")'

# Multiple prefixes
filter: 'key.starts_with("user-") || key.starts_with("customer-")'
```

### Key Suffix Matching

```yaml
# Test users (key ends with "-test")
filter: 'key.ends_with("-test")'

# Specific region (key ends with "-us-east-1")
filter: 'key.ends_with("-us-east-1")'
```

### Key Contains

```yaml
# VIP users (key contains "vip")
filter: 'key.contains("vip")'

# Exclude test keys
filter: '!key.contains("test")'
```

### Key Regex Matching

```yaml
# User IDs in format "user-123"
filter: 'key.matches("^user-[0-9]+$")'

# Email addresses as keys
filter: 'key.matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$")'

# UUIDs
filter: 'key.matches("^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")'
```

### Complete Key Filtering Example

```yaml
appid: key-based-router
bootstrap: kafka:9092
input: events

routing:
  destinations:
    # Premium users
    - output: premium-events
      filter: 'key.starts_with("premium-")'
      description: Premium user events
    
    # Test users
    - output: test-events
      filter: 'key.ends_with("-test")'
      description: Test user events
    
    # VIP users
    - output: vip-events
      filter: 'key.contains("vip")'
      description: VIP user events
    
    # Messages without keys (default routing)
    - output: unkeyed-events
      filter: 'key == ""'
      description: Events without message keys
```

## Header-Based Filtering

Kafka **headers** are available as a `Map` (dictionary) named `headers`. Each header maps from `header-name` (string) to its value (UTF-8 string).

### Basic Header Checks

```yaml
# Check if header exists
filter: 'not_null(headers["x-correlation-id"])'
filter: 'headers["x-tenant"] != ()'  # () is null in Rhai

# Check if header is missing
filter: 'is_null(headers["x-correlation-id"])'
```

### Header Value Matching

```yaml
# Exact match
filter: 'headers["x-tenant"] == "production"'
filter: 'headers["x-environment"] == "prod"'

# Case-insensitive match
filter: 'headers["x-environment"].to_lower() == "production"'

# Multiple values
filter: 'headers["x-priority"] == "high" || headers["x-priority"] == "urgent"'
```

### Header String Operations

```yaml
# Starts with
filter: 'headers["x-request-id"].starts_with("req-")'

# Ends with
filter: 'headers["x-service"].ends_with("-api")'

# Contains
filter: 'headers["x-trace-id"].contains("prod")'
```

### Header Regex Matching

```yaml
# UUID format request IDs
filter: 'headers["x-request-id"].matches("^[0-9a-f]{8}-")'

# Version headers (e.g., "v1.2.3")
filter: 'headers["x-api-version"].matches("^v[0-9]+\\.[0-9]+\\.[0-9]+$")'
```

### Complete Header Filtering Example

```yaml
appid: multi-tenant-router
bootstrap: kafka:9092
input: raw-events

routing:
  destinations:
    # Production tenant
    - output: prod-tenant-events
      filter: 'headers["x-tenant"] == "production"'
      description: Production tenant only
    
    # Staging tenant
    - output: staging-tenant-events
      filter: 'headers["x-tenant"] == "staging"'
      description: Staging tenant only
    
    # High priority messages
    - output: priority-queue
      filter: |
        headers["x-priority"] == "high" ||
        headers["x-priority"] == "urgent"
      description: High priority messages
    
    # Messages with correlation ID (traceable)
    - output: traced-events
      filter: 'not_null(headers["x-correlation-id"])'
      description: Events with correlation tracking
    
    # Exclude test environment
    - output: production-events
      filter: 'headers["x-environment"] != "test"'
      description: Non-test environment events
```

## Combining Key and Header Filters

You can combine key and header filters with payload filters:

```yaml
routing:
  destinations:
    # Premium production users
    - output: premium-prod-users
      filter: |
        key.starts_with("premium-") &&
        headers["x-environment"] == "production" &&
        msg["active"] == true
    
    # High-value orders from specific tenant
    - output: tenant-a-high-value
      filter: |
        headers["x-tenant"] == "tenant-a" &&
        msg["order"]["total"] > 1000 &&
        msg["order"]["status"] == "confirmed"
    
    # VIP users with correlation tracking
    - output: vip-traced
      filter: |
        key.contains("vip") &&
        not_null(headers["x-correlation-id"]) &&
        !key.contains("test")
```

## Timestamp-Based Filtering

The message **timestamp** (in milliseconds) is available as an `i64` variable.

### Message Age Filtering

```yaml
# Messages from last 5 minutes (300 seconds)
filter: '(now_ms() - timestamp) / 1000 < 300'

# Messages older than 1 hour (3600 seconds)
filter: '(now_ms() - timestamp) / 1000 >= 3600'

# Messages from last 24 hours
filter: '(now_ms() - timestamp) / 1000 < 86400'
```

### Time Range Filtering

```yaml
# After specific date (January 1, 2026)
filter: 'timestamp >= 1735689600000'

# Between dates
filter: 'timestamp >= 1735689600000 && timestamp < 1738368000000'

# Future scheduled messages
filter: 'timestamp > now_ms()'
```

### Complete Timestamp Example

```yaml
routing:
  destinations:
    # Real-time processing (last 5 minutes)
    - output: realtime-events
      filter: '(now_ms() - timestamp) / 1000 < 300'
      description: Fresh events for real-time processing
    
    # Batch processing (older than 5 minutes)
    - output: batch-events
      filter: '(now_ms() - timestamp) / 1000 >= 300'
      description: Older events for batch processing
    
    # Late-arriving data (older than 1 hour)
    - output: late-arrivals
      filter: '(now_ms() - timestamp) / 1000 >= 3600'
      description: Late-arriving data for reconciliation
```

## Using Keys and Headers in Transforms

Keys and headers can also be used in transforms to enrich the output:

### Extract Key into Payload

```yaml
transform: |
  msg + #{
    originalKey: key,
    processedAt: now_ms()
  }
```

### Extract Headers into Payload

```yaml
transform: |
  msg + #{
    tenant: headers["x-tenant"],
    correlationId: headers["x-correlation-id"],
    environment: headers["x-environment"]
  }
```

### Conditional Transform Based on Key

```yaml
transform: |
  if key.starts_with("vip-") {
    msg + #{ tier: "premium", discount: 0.15 }
  } else if key.starts_with("premium-") {
    msg + #{ tier: "standard", discount: 0.10 }
  } else {
    msg + #{ tier: "basic", discount: 0.05 }
  }
```

### Complete Enrichment Example

```yaml
routing:
  destinations:
    - output: enriched-events
      transform: |
        #{
          # Original payload
          data: msg,
          
          # Envelope metadata
          metadata: #{
            key: key,
            tenant: headers["x-tenant"] ?? "unknown",
            correlationId: headers["x-correlation-id"] ?? "",
            requestId: headers["x-request-id"] ?? "",
            environment: headers["x-environment"] ?? "unknown",
            messageAge: (now_ms() - timestamp) / 1000,
            processedAt: now_ms()
          }
        }
```

## Multi-Tenant Routing Pattern

A common pattern is to route messages based on tenant headers and key prefixes:

```yaml
appid: multi-tenant-router
bootstrap: kafka:9092
input: shared-events

routing:
  destinations:
    # Tenant A - Production
    - output: tenant-a-prod
      filter: |
        headers["x-tenant"] == "tenant-a" &&
        headers["x-environment"] == "production" &&
        !key.ends_with("-test")
    
    # Tenant A - Staging
    - output: tenant-a-staging
      filter: |
        headers["x-tenant"] == "tenant-a" &&
        headers["x-environment"] == "staging"
    
    # Tenant B - Production
    - output: tenant-b-prod
      filter: |
        headers["x-tenant"] == "tenant-b" &&
        headers["x-environment"] == "production" &&
        !key.ends_with("-test")
    
    # Tenant B - Staging
    - output: tenant-b-staging
      filter: |
        headers["x-tenant"] == "tenant-b" &&
        headers["x-environment"] == "staging"
    
    # Test messages (any tenant)
    - output: test-events
      filter: |
        headers["x-environment"] == "test" ||
        key.ends_with("-test")
```

## Correlation and Tracing

Use headers for distributed tracing and correlation:

```yaml
routing:
  destinations:
    # Only process messages with full tracing context
    - output: traced-events
      filter: |
        not_null(headers["x-trace-id"]) &&
        not_null(headers["x-span-id"]) &&
        not_null(headers["x-correlation-id"])
      transform: |
        msg + #{
          tracing: #{
            traceId: headers["x-trace-id"],
            spanId: headers["x-span-id"],
            correlationId: headers["x-correlation-id"],
            parentSpanId: headers["x-parent-span-id"] ?? ""
          }
        }
```

## Security and Authorization

Filter messages based on security headers:

```yaml
routing:
  destinations:
    # Only authenticated requests
    - output: authenticated-events
      filter: 'not_null(headers["x-auth-token"])'
    
    # Admin-only events
    - output: admin-events
      filter: |
        headers["x-user-role"] == "admin" &&
        not_null(headers["x-auth-token"])
    
    # Remove sensitive headers before routing to external systems
    - output: public-events
      filter: '!key.contains("internal")'
      transform: |
        #{
          data: msg,
          # Headers NOT included - stripped for security
          processedAt: now_ms()
        }
```

## Performance Considerations

### Efficient Header Checks

```yaml
# Fast - direct map lookup
filter: 'headers["x-tenant"] == "production"'

# Slower - multiple checks
filter: |
  not_null(headers["x-tenant"]) &&
  headers["x-tenant"].to_lower() == "production"
```

### Early Termination

Put most selective conditions first:

```yaml
# Good - rare condition first
filter: |
  headers["x-priority"] == "critical" &&
  key.starts_with("vip-") &&
  msg["amount"] > 10000

# Less efficient - common condition first
filter: |
  msg["amount"] > 10000 &&
  key.starts_with("vip-") &&
  headers["x-priority"] == "critical"
```

## Debugging

Enable debug logging to see key and header values:

```bash
RUST_LOG=debug CONFIG_FILE=config.yaml ./streamforge
```

Add debug fields to transforms:

```yaml
transform: |
  msg + #{
    debug_key: key,
    debug_headers: headers,
    debug_timestamp: timestamp,
    debug_age_seconds: (now_ms() - timestamp) / 1000
  }
```

## Common Patterns

### Pattern 1: Environment-Based Routing
```yaml
filter: 'headers["x-environment"] == "production"'
```

### Pattern 2: Multi-Tenant Isolation
```yaml
filter: 'headers["x-tenant"] == "tenant-a"'
```

### Pattern 3: User Segmentation
```yaml
filter: 'key.starts_with("premium-") || key.starts_with("vip-")'
```

### Pattern 4: Correlation Tracking
```yaml
filter: 'not_null(headers["x-correlation-id"])'
```

### Pattern 5: Time-Based Processing
```yaml
filter: '(now_ms() - timestamp) / 1000 < 300'
```

### Pattern 6: Test Exclusion
```yaml
filter: '!key.ends_with("-test") && headers["x-environment"] != "test"'
```

## See Also

- [ADVANCED_FILTERS.md](ADVANCED_FILTERS.md) - Complete filtering guide
- [RHAI_QUICK_REFERENCE.md](RHAI_QUICK_REFERENCE.md) - Rhai syntax reference
- [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) - Complete DSL guide including key/header transforms
- [ENVELOPE_FEATURE_DESIGN.md](ENVELOPE_FEATURE_DESIGN.md) - Envelope operations design
