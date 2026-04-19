# Error Handling

**Version:** 1.0.0-alpha.1  
**Status:** Implemented (Phase 1)  
**Last Updated:** 2026-04-18

---

## Executive Summary

StreamForge v1.0 implements a **typed error system** with explicit recovery actions. Every error category maps to a specific recovery strategy, making error handling deterministic and observable.

**Key Features:**
- 🎯 **14+ typed error categories** (not generic strings)
- 🔄 **4 recovery actions:** RetryWithBackoff, SendToDlq, SkipAndLog, FailFast
- 📊 **Context propagation:** errors carry topic/partition/offset
- ✅ **Backward compatible:** existing code still works

---

## Table of Contents

- [Error Categories](#error-categories)
- [Recovery Actions](#recovery-actions)
- [Error Flow](#error-flow)
- [Configuration](#configuration)
- [Observability](#observability)
- [Best Practices](#best-practices)

---

## Error Categories

### 1. Kafka Errors

#### `MirrorMakerError::Kafka(KafkaError)`
- **Source:** rdkafka library
- **Examples:** Connection timeout, authentication failure, broker unavailable
- **Recoverable:** Depends on underlying error (checked via `is_recoverable()`)
- **Recovery:** RetryWithBackoff for transient errors, FailFast for auth failures

#### `MirrorMakerError::KafkaProducer`
```rust
KafkaProducer {
    message: String,
    destination: Option<String>,
    recoverable: bool,
}
```
- **Source:** Producer send failures
- **Examples:** Queue full, message too large, timeout
- **Recoverable:** Based on `recoverable` field
- **Recovery:** RetryWithBackoff if recoverable, else SendToDlq

#### `MirrorMakerError::KafkaConsumer`
```rust
KafkaConsumer {
    message: String,
    topic: Option<String>,
    partition: Option<i32>,
    recoverable: bool,
}
```
- **Source:** Consumer poll/subscription failures
- **Examples:** Rebalance in progress, offset out of range
- **Recoverable:** Based on `recoverable` field
- **Recovery:** RetryWithBackoff if recoverable, else FailFast

#### `MirrorMakerError::OffsetCommit`
```rust
OffsetCommit {
    message: String,
    topic: String,
    partition: i32,
    offset: i64,
    retry_count: u32,
}
```
- **Source:** Manual offset commit failures
- **Examples:** Coordinator not available, rebalance in progress
- **Recoverable:** Always (until max retries)
- **Recovery:** RetryWithBackoff (up to 3 attempts)

---

### 2. Configuration Errors

#### `MirrorMakerError::Config(String)`
- **Source:** Invalid configuration values
- **Examples:** Missing required field, invalid broker address
- **Recoverable:** No (fix config and restart)
- **Recovery:** FailFast

#### `MirrorMakerError::ConfigMissing`
```rust
ConfigMissing { field: String }
```
- **Source:** Required field not provided
- **Examples:** No bootstrap servers, no input topic
- **Recoverable:** No
- **Recovery:** FailFast

#### `MirrorMakerError::ConfigInvalid`
```rust
ConfigInvalid {
    field: String,
    value: String,
    reason: String,
}
```
- **Source:** Field present but invalid
- **Examples:** Negative thread count, invalid compression type
- **Recoverable:** No
- **Recovery:** FailFast

---

### 3. DSL Parsing Errors

#### `MirrorMakerError::DslParse`
```rust
DslParse {
    message: String,
    dsl_string: String,
    position: Option<usize>,
}
```
- **Source:** Invalid filter/transform syntax
- **Examples:** Malformed expression, unknown operator
- **Recoverable:** No (fix DSL and restart)
- **Recovery:** FailFast

#### `MirrorMakerError::InvalidFilter`
```rust
InvalidFilter {
    expression: String,
    reason: String,
}
```
- **Source:** Semantically invalid filter
- **Examples:** Invalid comparison operator, missing operand
- **Recoverable:** No
- **Recovery:** FailFast

#### `MirrorMakerError::InvalidTransform`
```rust
InvalidTransform {
    expression: String,
    reason: String,
}
```
- **Source:** Semantically invalid transform
- **Examples:** Invalid field path, type mismatch
- **Recoverable:** No
- **Recovery:** FailFast

---

### 4. Message Processing Errors

#### `MirrorMakerError::Processing(String)`
- **Source:** Generic processing failure
- **Examples:** Unexpected runtime error
- **Recoverable:** No (message-specific)
- **Recovery:** SendToDlq

#### `MirrorMakerError::ProcessingWithContext`
```rust
ProcessingWithContext {
    message: String,
    topic: String,
    partition: i32,
    offset: i64,
}
```
- **Source:** Processing failure with full context
- **Examples:** Transform failed on specific message
- **Recoverable:** No (message-specific)
- **Recovery:** SendToDlq

#### `MirrorMakerError::MessageDeserialization`
```rust
MessageDeserialization {
    message: String,
    topic: String,
    partition: i32,
    offset: i64,
    key: Option<Vec<u8>>,
}
```
- **Source:** Invalid message format (not valid JSON)
- **Examples:** Corrupt data, wrong schema
- **Recoverable:** No (message-level)
- **Recovery:** SendToDlq

#### `MirrorMakerError::FilterEvaluation`
```rust
FilterEvaluation {
    message: String,
    filter: String,
    value: Option<String>,
}
```
- **Source:** Filter threw exception
- **Examples:** Division by zero, null pointer
- **Recoverable:** No (message-level)
- **Recovery:** SendToDlq

#### `MirrorMakerError::TransformEvaluation`
```rust
TransformEvaluation {
    message: String,
    transform: String,
    value: Option<String>,
}
```
- **Source:** Transform threw exception
- **Examples:** Invalid regex, missing field
- **Recoverable:** No (message-level)
- **Recovery:** SendToDlq

#### `MirrorMakerError::JsonPathNotFound`
```rust
JsonPathNotFound {
    path: String,
    value: Option<String>,
}
```
- **Source:** JSON path does not exist
- **Examples:** `/user/email` but no `email` field
- **Recoverable:** No (message-level)
- **Recovery:** SendToDlq (or skip based on config)

---

### 5. Serialization Errors

#### `MirrorMakerError::Serialization(serde_json::Error)`
- **Source:** JSON serialization failure
- **Examples:** Infinite recursion, invalid UTF-8
- **Recoverable:** No (message-level)
- **Recovery:** SendToDlq

---

### 6. Compression Errors

#### `MirrorMakerError::Compression(String)`
- **Source:** Compression failure
- **Examples:** Invalid input data, encoder error
- **Recoverable:** No (message-level)
- **Recovery:** SendToDlq

#### `MirrorMakerError::Decompression`
```rust
Decompression {
    message: String,
    codec: String,
}
```
- **Source:** Decompression failure
- **Examples:** Corrupt compressed data, wrong codec
- **Recoverable:** No (message-level)
- **Recovery:** SendToDlq

---

### 7. Cache Errors

#### `MirrorMakerError::Cache`
```rust
Cache {
    message: String,
    backend: String,
    key: Option<String>,
}
```
- **Source:** Cache lookup/put failure
- **Examples:** Cache unavailable, serialization error
- **Recoverable:** Yes (cache failures should not halt processing)
- **Recovery:** SkipAndLog (continue without cache)

#### `MirrorMakerError::Redis`
```rust
Redis {
    message: String,
    operation: String,
}
```
- **Source:** Redis connection/operation failure
- **Examples:** Connection refused, timeout
- **Recoverable:** Yes (transient)
- **Recovery:** RetryWithBackoff, then SkipAndLog

---

### 8. Retry and DLQ Errors

#### `MirrorMakerError::RetryExhausted`
```rust
RetryExhausted {
    message: String,
    attempts: u32,
    last_error: String,
}
```
- **Source:** Max retries reached
- **Examples:** 3 failed attempts to produce
- **Recoverable:** No
- **Recovery:** SendToDlq

#### `MirrorMakerError::DeadLetterQueue`
```rust
DeadLetterQueue {
    message: String,
    dlq_topic: String,
}
```
- **Source:** DLQ produce failure
- **Examples:** DLQ topic doesn't exist, DLQ broker down
- **Recoverable:** No
- **Recovery:** FailFast (cannot lose data)

---

### 9. I/O Errors

#### `MirrorMakerError::Io(std::io::Error)`
- **Source:** File system, network I/O
- **Examples:** Config file not found, disk full
- **Recoverable:** Depends (check underlying error)
- **Recovery:** RetryWithBackoff for network, FailFast for config

---

### 10. Generic Error

#### `MirrorMakerError::Generic(String)`
- **Source:** Fallback for unexpected errors
- **Examples:** Should be avoided, use specific types
- **Recoverable:** No
- **Recovery:** SkipAndLog

---

## Recovery Actions

### RecoveryAction Enum

```rust
pub enum RecoveryAction {
    RetryWithBackoff,
    SendToDlq,
    SkipAndLog,
    FailFast,
}
```

### 1. RetryWithBackoff

**When:** Transient failures that may resolve (network, rebalance, queue full)

**Behavior:**
- Retry with exponential backoff
- Initial delay: 100ms
- Max delay: 30 seconds
- Multiplier: 2.0
- Max attempts: 3 (configurable)

**Example:**
```rust
match error {
    MirrorMakerError::KafkaProducer { recoverable: true, .. } => {
        // Retry 3 times with backoff
        for attempt in 1..=3 {
            sleep(100ms * 2^attempt);
            retry_produce()?;
        }
    }
}
```

**Errors with this action:**
- `KafkaProducer { recoverable: true }`
- `KafkaConsumer { recoverable: true }`
- `OffsetCommit`
- `Io` (network)
- `Redis`
- `Cache`

---

### 2. SendToDlq

**When:** Permanent message-level errors (bad data, transform failures)

**Behavior:**
- Send original message to dead letter queue
- Add error metadata to headers:
  - `x-streamforge-error`: Error message
  - `x-streamforge-error-type`: Error type name
  - `x-streamforge-source-topic`: Original topic
  - `x-streamforge-source-partition`: Original partition
  - `x-streamforge-source-offset`: Original offset
  - `x-streamforge-timestamp`: Error timestamp
- Continue processing next message

**Example:**
```rust
match error {
    MirrorMakerError::FilterEvaluation { .. } => {
        dlq_producer.send(DlqMessage {
            original: envelope,
            error: error.to_string(),
            headers: error_headers(),
        });
    }
}
```

**Errors with this action:**
- `MessageDeserialization`
- `FilterEvaluation`
- `TransformEvaluation`
- `JsonPathNotFound`
- `Processing`
- `ProcessingWithContext`
- `Serialization`
- `Compression`
- `Decompression`
- `RetryExhausted`

**DLQ message format:**
```json
{
  "headers": {
    "x-streamforge-error": "JSON path not found: /user/email",
    "x-streamforge-error-type": "JsonPathNotFound",
    "x-streamforge-source-topic": "input-topic",
    "x-streamforge-source-partition": "3",
    "x-streamforge-source-offset": "12345",
    "x-streamforge-timestamp": "2026-04-18T10:30:00Z"
  },
  "key": "<original key>",
  "value": "<original value>"
}
```

---

### 3. SkipAndLog

**When:** Non-critical failures that should not halt processing

**Behavior:**
- Log error at WARN level
- Increment `streamforge_errors_total{action="skip"}` metric
- Continue processing next message

**Example:**
```rust
match error {
    MirrorMakerError::Cache { .. } => {
        warn!("Cache lookup failed: {}, continuing without cache", error);
        metrics.errors_total.with_label_values(&["skip"]).inc();
        // Continue processing
    }
}
```

**Errors with this action:**
- `Cache` (after retry)
- `Generic` (fallback)

---

### 4. FailFast

**When:** Fatal errors that prevent safe operation (config, DSL, DLQ failures)

**Behavior:**
- Log error at ERROR level
- Shutdown gracefully:
  1. Stop consuming new messages
  2. Flush pending messages
  3. Commit offsets
  4. Exit with error code
- Kubernetes will restart pod

**Example:**
```rust
match error {
    MirrorMakerError::Config { .. } => {
        error!("Configuration error: {}, cannot continue", error);
        shutdown_gracefully();
        std::process::exit(1);
    }
}
```

**Errors with this action:**
- `Config`
- `ConfigMissing`
- `ConfigInvalid`
- `DslParse`
- `InvalidFilter`
- `InvalidTransform`
- `DeadLetterQueue` (cannot lose data)

---

## Error Flow

### Processing Pipeline with Error Handling

```
┌─────────────────────────────────────────────────────────────┐
│  1. Consume Message                                          │
│     ↓                                                        │
│     Error: KafkaConsumer                                     │
│     → RecoveryAction::RetryWithBackoff                       │
│     → If exhausted: FailFast                                 │
└─────────────────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────────┐
│  2. Deserialize (JSON)                                       │
│     ↓                                                        │
│     Error: MessageDeserialization                            │
│     → RecoveryAction::SendToDlq                              │
│     → Continue with next message                             │
└─────────────────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────────┐
│  3. Evaluate Filter                                          │
│     ↓                                                        │
│     Error: FilterEvaluation                                  │
│     → RecoveryAction::SendToDlq                              │
│     → Continue with next message                             │
└─────────────────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────────┐
│  4. Apply Transform                                          │
│     ↓                                                        │
│     Error: TransformEvaluation                               │
│     → RecoveryAction::SendToDlq                              │
│     → Continue with next message                             │
└─────────────────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────────┐
│  5. Cache Lookup (optional)                                  │
│     ↓                                                        │
│     Error: Cache / Redis                                     │
│     → RecoveryAction::SkipAndLog                             │
│     → Continue without cache                                 │
└─────────────────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────────┐
│  6. Produce to Destination                                   │
│     ↓                                                        │
│     Error: KafkaProducer                                     │
│     → RecoveryAction::RetryWithBackoff                       │
│     → If exhausted: SendToDlq                                │
└─────────────────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────────┐
│  7. Commit Offset                                            │
│     ↓                                                        │
│     Error: OffsetCommit                                      │
│     → RecoveryAction::RetryWithBackoff (3 attempts)          │
│     → If exhausted: FailFast                                 │
└─────────────────────────────────────────────────────────────┘
```

---

## Configuration

### Error Handling Configuration

```yaml
# config.yaml
error_handling:
  # Retry configuration
  retry:
    max_attempts: 3
    initial_delay_ms: 100
    max_delay_ms: 30000
    multiplier: 2.0

  # Dead letter queue
  dead_letter_queue:
    enabled: true
    topic: "streamforge-dlq"
    # Include original message headers
    include_headers: true
    # Include error stack trace
    include_stack_trace: true

  # Missing field behavior
  missing_field_behavior: "error"  # or "null", "skip"

  # Null value behavior
  null_value_behavior: "passthrough"  # or "error", "default"

  # Cache failure behavior
  cache_failure_behavior: "skip"  # or "error"
```

---

## Observability

### Metrics

```promql
# Total errors by type
streamforge_errors_total{error_type="MessageDeserialization"}

# Errors by recovery action
streamforge_errors_total{action="retry"}
streamforge_errors_total{action="dlq"}
streamforge_errors_total{action="skip"}
streamforge_errors_total{action="fail_fast"}

# Retry attempts
streamforge_retries_total{error_type="KafkaProducer"}

# DLQ messages
streamforge_dlq_messages_total
```

### Logging

```rust
// Error logs include full context
error!(
    topic = %envelope.topic,
    partition = envelope.partition,
    offset = envelope.offset,
    error_type = "FilterEvaluation",
    filter = %filter_expr,
    "Filter evaluation failed, sending to DLQ"
);
```

**Log format:**
```
ERROR streamforge: Filter evaluation failed, sending to DLQ
  topic: input-topic
  partition: 3
  offset: 12345
  error_type: FilterEvaluation
  filter: /status,==,active
  error: JSON path not found: /status
```

---

## Best Practices

### 1. Use Specific Error Types

❌ **Bad:**
```rust
return Err(MirrorMakerError::Generic("something went wrong".into()));
```

✅ **Good:**
```rust
return Err(MirrorMakerError::FilterEvaluation {
    message: "Division by zero in arithmetic filter".into(),
    filter: filter_expr.clone(),
    value: Some(value.to_string()),
});
```

### 2. Add Context to Errors

❌ **Bad:**
```rust
transform.apply(envelope)?;  // Error has no context
```

✅ **Good:**
```rust
transform.apply(envelope)
    .map_err(|e| e.with_context(format!(
        "Failed to apply transform on topic={}, partition={}, offset={}",
        envelope.topic, envelope.partition, envelope.offset
    )))?;
```

### 3. Check Recoverability

```rust
if error.is_recoverable() {
    // Retry with backoff
    retry_with_backoff(operation, 3)?;
} else {
    // Send to DLQ or fail fast
    match error.recovery_action() {
        RecoveryAction::SendToDlq => send_to_dlq(envelope, error)?,
        RecoveryAction::FailFast => {
            error!("Fatal error: {}", error);
            shutdown_gracefully();
        }
        _ => {}
    }
}
```

### 4. Monitor Error Rates

Set up alerts:
```yaml
# Alertmanager
- alert: HighErrorRate
  expr: rate(streamforge_errors_total[5m]) > 10
  for: 5m
  labels:
    severity: warning

- alert: DLQBacklog
  expr: streamforge_dlq_messages_total - streamforge_dlq_messages_total offset 1h > 1000
  for: 10m
  labels:
    severity: critical
```

### 5. Test Error Paths

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_invalid_json_sends_to_dlq() {
        let invalid_json = b"not valid json";
        let result = process_message(invalid_json);
        
        assert!(matches!(
            result.unwrap_err(),
            MirrorMakerError::MessageDeserialization { .. }
        ));
        assert_eq!(dlq_count(), 1);
    }
}
```

---

## Error Decision Tree

```
Error Occurred
    │
    ├─ Is it a config/DSL error?
    │   └─ YES → FailFast (fix config and restart)
    │
    ├─ Can it be retried?
    │   ├─ YES → RetryWithBackoff
    │   │   └─ Exhausted? → SendToDlq (if message-level) or FailFast (if system-level)
    │   └─ NO → Continue below
    │
    ├─ Is it message-specific?
    │   ├─ YES → SendToDlq (bad data, transform failure)
    │   └─ NO → Continue below
    │
    ├─ Is it critical?
    │   ├─ YES → FailFast (DLQ failure, offset commit exhausted)
    │   └─ NO → SkipAndLog (cache failure, non-critical)
```

---

## References

- **src/error.rs**: Error type definitions
- **PROJECT_SPEC.md §6**: Error handling requirements
- **V1_PLAN.md Phase 1**: Core engine hardening
- **DELIVERY_GUARANTEES.md**: Commit and retry semantics

---

**Status:** Implemented  
**Version:** 1.0.0-alpha.1  
**Phase:** 1 (Core Engine Hardening)
