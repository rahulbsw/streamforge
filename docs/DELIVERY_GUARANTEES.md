# Delivery Guarantees

**Version:** 1.0.0-alpha.1  
**Status:** Specification Complete (Phase 1)  
**Last Updated:** 2026-04-18

---

## Executive Summary

StreamForge v1.0 provides **at-least-once delivery** by default with configurable commit strategies, retry policies, and dead letter queue handling.

**Key Guarantees:**
- ✅ **At-least-once:** Every message processed at least once (duplicates possible on failure)
- ⏱️ **Ordered processing:** Messages within a partition processed in order
- 🔄 **Retry with backoff:** Transient failures retried automatically
- 💀 **DLQ for permanent failures:** Bad messages don't halt the pipeline
- 📊 **Observable:** Metrics track every stage of delivery

**NOT Guaranteed:**
- ❌ Exactly-once semantics (planned v1.1+)
- ❌ Cross-partition ordering
- ❌ Zero duplicates on failure recovery

---

## Table of Contents

- [Delivery Semantics](#delivery-semantics)
- [Commit Strategies](#commit-strategies)
- [Retry Policy](#retry-policy)
- [Dead Letter Queue](#dead-letter-queue)
- [Failure Scenarios](#failure-scenarios)
- [Configuration](#configuration)
- [Observability](#observability)

---

## Delivery Semantics

### At-Least-Once (Default)

**Definition:** Every message is processed and delivered at least once. Duplicates may occur on failure recovery.

**How it works:**
1. Consume message from Kafka
2. Process message (filter, transform, produce)
3. **Commit offset only after successful produce**
4. On failure: retry or send to DLQ, then retry step 3

**Duplicate scenarios:**
- Process succeeds, produce succeeds, **commit fails** → message reprocessed on restart
- Process succeeds, commit succeeds, **crash before persist** → message reprocessed on restart

**Trade-off:** Guarantees no data loss, but allows duplicates.

---

### At-Most-Once (Optional)

**Definition:** Every message is processed at most once. Data loss possible on failure.

**How it works:**
1. Consume message from Kafka
2. **Commit offset immediately** (before processing)
3. Process message (filter, transform, produce)
4. On failure: skip message (already committed)

**Data loss scenarios:**
- Commit succeeds, **process fails** → message lost
- Commit succeeds, **produce fails** → message lost

**Trade-off:** No duplicates, but data loss possible.

**⚠️ Not recommended for production** unless data loss is acceptable.

---

### Exactly-Once (Planned v1.1+)

**Definition:** Every message is processed exactly once. No duplicates, no data loss.

**Requirements:**
- Kafka 3.3+ with transactional producer
- Idempotent producer enabled
- Read-process-write transactions

**Status:** Not implemented in v1.0. Use at-least-once with idempotency keys for now.

---

## Commit Strategies

### 1. Manual Commit After Batch (Default, Recommended)

**Configuration:**
```yaml
commit_strategy:
  manual_commit: true
  commit_mode: async
  commit_interval: 100  # Commit every 100 messages
  commit_timeout_ms: 5000
```

**Behavior:**
- Batch consume 100 messages
- Process all messages in batch (with retries)
- Produce all messages to destination(s)
- **Commit offset of last message in batch**
- If any message fails permanently → send to DLQ, then commit

**Guarantees:**
- ✅ At-least-once per batch
- ✅ High throughput (batch commits)
- ⚠️ Duplicates on batch-level failure

**When offset is committed:**
```rust
// Pseudocode
for batch in consumer.consume_batch(100) {
    let mut dlq_messages = Vec::new();
    
    for msg in batch {
        match process(msg) {
            Ok(_) => {}, // Success
            Err(e) if e.is_recoverable() => retry_with_backoff(msg)?,
            Err(e) => dlq_messages.push((msg, e)),  // DLQ
        }
    }
    
    // Send all DLQ messages
    for (msg, err) in dlq_messages {
        dlq.send(msg, err)?;
    }
    
    // Commit offset after batch processed
    consumer.commit()?;  // ← COMMIT HAPPENS HERE
}
```

**Failure scenarios:**
1. **Process fails before produce:**
   - Retry up to 3 times
   - If exhausted → DLQ
   - Offset NOT committed yet → message not lost

2. **Produce succeeds, commit fails:**
   - Offset NOT committed → batch reprocessed
   - Messages produced again → **duplicates**

3. **Commit succeeds, crash before persist:**
   - Offset committed but not persisted to Kafka
   - On restart: batch reprocessed → **duplicates**

---

### 2. Manual Commit Per Message (Low Latency)

**Configuration:**
```yaml
commit_strategy:
  manual_commit: true
  commit_mode: sync
  commit_interval: 1  # Commit every message
```

**Behavior:**
- Consume 1 message
- Process message (with retries)
- Produce message
- **Commit offset immediately**
- Next message

**Guarantees:**
- ✅ At-least-once per message
- ✅ Low latency (no batching)
- ⚠️ Lower throughput (commit overhead)

**Trade-off:** Commits are expensive (~5ms), so throughput drops to ~200 msg/s per partition.

**Use when:**
- Latency critical (real-time processing)
- Small message rate (<1K msg/s)
- Need fine-grained recovery

---

### 3. Auto Commit (Kafka Default, Not Recommended)

**Configuration:**
```yaml
commit_strategy:
  manual_commit: false  # Use Kafka auto-commit
  auto_commit_interval_ms: 5000
```

**Behavior:**
- Kafka commits offset every 5 seconds automatically
- **Independent of processing success**
- Messages processed may not be committed yet
- Committed messages may not be processed yet

**Guarantees:**
- ⚠️ No guarantees (between at-most-once and at-least-once)
- ⚠️ Data loss possible (commit before process)
- ⚠️ Duplicates possible (process before commit)

**Problems:**
```
Timeline:
0s:    Consume msg offset 100
1s:    Process msg 100 (takes 6 seconds)
5s:    Auto-commit offset 100 ← Message not processed yet!
6s:    Process completes, produce succeeds
7s:    Crash
Restart: Offset 100 already committed → message lost
```

**⚠️ DO NOT USE for anything that requires delivery guarantees.**

---

### 4. Time-Based Commit (Alternative)

**Configuration:**
```yaml
commit_strategy:
  manual_commit: true
  commit_mode: async
  commit_interval_ms: 30000  # Commit every 30 seconds
```

**Behavior:**
- Process messages continuously
- Commit offset every 30 seconds (last successfully processed message)

**Guarantees:**
- ✅ At-least-once
- ⚠️ Up to 30 seconds of duplicates on failure

**Use when:**
- High throughput (minimize commit overhead)
- Duplicates acceptable (have idempotency elsewhere)

---

## Commit Strategy Comparison

| Strategy | Throughput | Latency | Duplicates on Failure | Data Loss | Recommended |
|----------|------------|---------|----------------------|-----------|-------------|
| Manual (batch) | ~35K msg/s | ~100ms | ~100 messages | Never | ✅ **Yes** |
| Manual (per-msg) | ~200 msg/s | ~5ms | 1 message | Never | Low rate only |
| Auto commit | ~35K msg/s | Variable | Variable | Possible | ❌ **Never** |
| Time-based | ~40K msg/s | Variable | ~30s worth | Never | High throughput |

---

## Retry Policy

### Exponential Backoff

**Default configuration:**
```yaml
retry:
  max_attempts: 3
  initial_delay_ms: 100
  max_delay_ms: 30000
  multiplier: 2.0
  jitter: 0.1  # 10% random jitter
```

**Retry schedule:**
- Attempt 1: Immediate (0ms)
- Attempt 2: 100ms + jitter (90-110ms)
- Attempt 3: 200ms + jitter (180-220ms)
- Attempt 4: 400ms + jitter (360-440ms)
- Failed: Send to DLQ

**Which errors are retried:**
- ✅ `KafkaProducer { recoverable: true }`
- ✅ `KafkaConsumer { recoverable: true }`
- ✅ `OffsetCommit` (always)
- ✅ `Redis` (cache failures)
- ✅ `Io` (network failures)
- ❌ `MessageDeserialization` (bad data, not transient)
- ❌ `FilterEvaluation` (logic error, not transient)
- ❌ `Config` (cannot retry, needs restart)

**Retry logic:**
```rust
async fn process_with_retry(msg: Message) -> Result<()> {
    let mut delay = config.retry.initial_delay_ms;
    
    for attempt in 1..=config.retry.max_attempts {
        match process_message(msg).await {
            Ok(_) => return Ok(()),
            Err(e) if e.is_recoverable() && attempt < config.retry.max_attempts => {
                warn!("Attempt {} failed: {}, retrying in {}ms", attempt, e, delay);
                sleep(Duration::from_millis(delay)).await;
                delay = (delay * config.retry.multiplier as u64).min(config.retry.max_delay_ms);
            }
            Err(e) => return Err(e),  // Not recoverable or exhausted
        }
    }
    
    Err(MirrorMakerError::RetryExhausted {
        message: "Max retry attempts reached".into(),
        attempts: config.retry.max_attempts,
        last_error: "...".into(),
    })
}
```

---

## Dead Letter Queue

### Purpose

Messages that **cannot be processed** (permanent failures) are sent to a DLQ to:
1. Prevent pipeline halt (skip bad messages)
2. Enable manual inspection and replay
3. Maintain observability (what's failing and why)

### DLQ Message Format

**Headers added:**
```
x-streamforge-error: "JSON path not found: /user/email"
x-streamforge-error-type: "JsonPathNotFound"
x-streamforge-source-topic: "input-topic"
x-streamforge-source-partition: "3"
x-streamforge-source-offset: "12345"
x-streamforge-timestamp: "2026-04-18T10:30:00Z"
x-streamforge-pipeline: "my-pipeline"
x-streamforge-destination: "output-topic"
x-streamforge-filter: "/status,==,active"
x-streamforge-transform: "EXTRACT:/user/email,userEmail"
```

**Key and value:** Original message unchanged

**Example DLQ message:**
```json
{
  "headers": {
    "x-streamforge-error": "JSON path not found: /user/email",
    "x-streamforge-error-type": "JsonPathNotFound",
    "x-streamforge-source-topic": "events",
    "x-streamforge-source-partition": "3",
    "x-streamforge-source-offset": "12345",
    "x-streamforge-timestamp": "2026-04-18T10:30:00.123Z"
  },
  "key": {"userId": "user-123"},
  "value": {
    "event": "login",
    "timestamp": 1234567890,
    "user": {"id": "user-123"}  // Note: no "email" field
  }
}
```

### DLQ Configuration

```yaml
dead_letter_queue:
  enabled: true
  topic: "streamforge-dlq"
  
  # Include original headers
  include_original_headers: true
  
  # Include stack trace in error header
  include_stack_trace: false
  
  # DLQ producer settings (can be different from main)
  brokers: "kafka-dlq:9092"
  compression: "none"
  
  # Max retries to send to DLQ (if DLQ fails, halt pipeline)
  max_dlq_retries: 3
```

### Which Errors Go to DLQ

✅ **Sent to DLQ:**
- `MessageDeserialization` (bad JSON)
- `FilterEvaluation` (filter threw exception)
- `TransformEvaluation` (transform threw exception)
- `JsonPathNotFound` (missing field)
- `Compression` / `Decompression` (corrupt data)
- `RetryExhausted` (after max retries)

❌ **NOT sent to DLQ (halt instead):**
- `Config` (fix and restart)
- `DslParse` (fix and restart)
- `DeadLetterQueue` (cannot lose data if DLQ fails)

### DLQ Failure Handling

**What if DLQ produce fails?**

```rust
match send_to_dlq(msg, error) {
    Ok(_) => {
        // DLQ succeeded, continue processing
        metrics.dlq_messages_total.inc();
    }
    Err(dlq_error) => {
        // DLQ FAILED - this is critical
        error!(
            "CRITICAL: Failed to send message to DLQ: {}\n\
             Original error: {}\n\
             Message: {:?}",
            dlq_error, error, msg
        );
        
        // Retry DLQ send (up to 3 attempts)
        for attempt in 1..=3 {
            match retry_dlq_send(msg, error) {
                Ok(_) => break,
                Err(e) if attempt == 3 => {
                    // DLQ exhausted - HALT PIPELINE
                    // Cannot lose data
                    return Err(MirrorMakerError::DeadLetterQueue {
                        message: "DLQ send exhausted".into(),
                        dlq_topic: config.dlq.topic.clone(),
                    });
                }
                _ => sleep(Duration::from_secs(1)),
            }
        }
    }
}
```

**Why halt on DLQ failure?**
- Cannot lose data
- DLQ failure indicates serious problem (DLQ topic missing, broker down)
- Better to halt and alert than silently drop messages

---

## Failure Scenarios

### Scenario 1: Produce Failure (Transient)

**Sequence:**
1. Consume message offset 100
2. Process succeeds
3. Produce fails (queue full)
4. **Retry:** Wait 100ms, retry produce
5. Produce succeeds
6. Commit offset 100
7. Continue

**Outcome:** ✅ Message delivered once, no duplicates

---

### Scenario 2: Produce Failure (Exhausted)

**Sequence:**
1. Consume message offset 100
2. Process succeeds
3. Produce fails (queue full)
4. Retry 1: Fails (still full)
5. Retry 2: Fails (still full)
6. Retry 3: Fails (still full)
7. **Send to DLQ** with error metadata
8. Commit offset 100
9. Continue with offset 101

**Outcome:** ✅ Bad message in DLQ, pipeline continues

---

### Scenario 3: Commit Failure (Transient)

**Sequence:**
1. Consume messages offset 100-199 (batch of 100)
2. Process all messages
3. Produce all messages
4. Commit offset 199 fails (coordinator unavailable)
5. **Retry commit:** Wait 100ms, retry
6. Commit succeeds
7. Continue with offset 200

**Outcome:** ✅ Batch committed, no duplicates

---

### Scenario 4: Commit Failure (Exhausted)

**Sequence:**
1. Consume messages offset 100-199
2. Process and produce all messages successfully
3. Commit offset 199 fails
4. Retry 1: Fails
5. Retry 2: Fails
6. Retry 3: Fails (exhausted)
7. **HALT PIPELINE** (cannot continue without committing)
8. On restart: Re-consume from offset 100

**Outcome:** ⚠️ Duplicates (messages 100-199 produced twice)

**Why halt:** If we continue without committing, on restart we'd reprocess from offset 0, creating many more duplicates.

---

### Scenario 5: Crash After Produce, Before Commit

**Sequence:**
1. Consume messages offset 100-199
2. Process and produce all successfully
3. About to commit offset 199
4. **CRASH** (pod killed, OOM, etc.)
5. On restart: Consumer resumes from last committed offset (99)
6. Re-consume and reprocess messages 100-199
7. **Duplicates produced**

**Outcome:** ⚠️ Duplicates (messages 100-199 produced twice)

**Why:** Kafka doesn't know about uncommitted work. This is inherent to at-least-once.

**Mitigation:** Use idempotency keys in messages, or implement exactly-once (v1.1+).

---

### Scenario 6: DLQ Failure

**Sequence:**
1. Consume message offset 100
2. Deserialization fails (bad JSON)
3. Send to DLQ → DLQ broker unavailable
4. Retry DLQ send: Attempt 1 fails
5. Retry DLQ send: Attempt 2 fails
6. Retry DLQ send: Attempt 3 fails
7. **HALT PIPELINE**

**Outcome:** ❌ Pipeline stopped, message not lost (offset not committed)

**Manual recovery:**
1. Fix DLQ topic/broker
2. Restart StreamForge
3. Message 100 reprocessed and sent to DLQ

---

### Scenario 7: Rebalance During Processing

**Sequence:**
1. Consume message offset 100 from partition 3
2. Processing (takes 5 seconds)
3. **Rebalance:** Another consumer joins group
4. Partition 3 revoked from this consumer
5. Processing completes, but **commit fails** (no longer own partition)
6. New consumer for partition 3 starts from last committed offset (99)
7. Message 100 reprocessed

**Outcome:** ⚠️ Duplicate (message 100 processed twice)

**Why:** In-flight messages during rebalance are not committed.

**Mitigation:** Process quickly (<5s) to minimize rebalance window.

---

## Configuration

### Recommended Production Config

```yaml
# At-least-once with batch commit (high throughput)
commit_strategy:
  manual_commit: true
  commit_mode: async
  commit_interval: 100  # Batch size
  commit_timeout_ms: 5000

# Retry with exponential backoff
retry:
  max_attempts: 3
  initial_delay_ms: 100
  max_delay_ms: 30000
  multiplier: 2.0
  jitter: 0.1

# Dead letter queue
dead_letter_queue:
  enabled: true
  topic: "streamforge-dlq"
  brokers: "kafka:9092"
  include_original_headers: true
  max_dlq_retries: 3

# Error handling
error_handling:
  missing_field_behavior: "error"  # Send to DLQ
  null_value_behavior: "passthrough"
  cache_failure_behavior: "skip"  # Don't halt on cache failures
```

### Low-Latency Config

```yaml
# Per-message commit (low latency)
commit_strategy:
  manual_commit: true
  commit_mode: sync  # Wait for commit to complete
  commit_interval: 1

# Aggressive retry
retry:
  max_attempts: 5
  initial_delay_ms: 10
  max_delay_ms: 1000

# Same DLQ config...
```

### High-Throughput Config

```yaml
# Large batch commit (maximize throughput)
commit_strategy:
  manual_commit: true
  commit_mode: async
  commit_interval: 1000  # Large batch

# Same retry config...
```

---

## Observability

### Metrics

```promql
# Messages consumed
rate(streamforge_messages_consumed_total[5m])

# Messages produced
rate(streamforge_messages_produced_total[5m])

# Commit successes
rate(streamforge_commits_total{result="success"}[5m])

# Commit failures (should be near zero)
rate(streamforge_commits_total{result="failure"}[5m])

# Retry attempts
rate(streamforge_retries_total[5m])

# DLQ messages (should be low)
rate(streamforge_dlq_messages_total[5m])

# Processing lag (commit lag)
streamforge_consumer_lag{partition="3"}
```

### Alerts

```yaml
# High commit failure rate
- alert: HighCommitFailureRate
  expr: rate(streamforge_commits_total{result="failure"}[5m]) > 0.01
  for: 5m
  annotations:
    summary: "High commit failure rate"

# DLQ growing rapidly
- alert: DLQBacklog
  expr: rate(streamforge_dlq_messages_total[1h]) > 100
  for: 10m
  annotations:
    summary: "DLQ receiving >100 msg/hour"

# Lag increasing
- alert: ConsumerLagIncreasing
  expr: streamforge_consumer_lag > 10000
  for: 5m
  annotations:
    summary: "Consumer lag > 10K messages"
```

---

## Testing Delivery Guarantees

### Integration Tests

```rust
#[tokio::test]
async fn test_at_least_once_with_produce_retry() {
    // Setup: Producer that fails first 2 attempts
    let producer = MockProducer::new()
        .fail_times(2)
        .then_succeed();
    
    // Act: Process message
    process_message(msg, producer).await.unwrap();
    
    // Assert: Message produced exactly once
    assert_eq!(producer.send_count(), 1);
    assert_eq!(producer.attempt_count(), 3);
}

#[tokio::test]
async fn test_bad_message_goes_to_dlq() {
    let dlq = MockDlq::new();
    let invalid_json = b"not valid json";
    
    process_message(invalid_json, dlq).await.unwrap();
    
    // Assert: Original message in DLQ
    assert_eq!(dlq.message_count(), 1);
    assert_eq!(dlq.messages()[0].value, invalid_json);
    assert!(dlq.messages()[0].headers.contains_key("x-streamforge-error"));
}

#[tokio::test]
async fn test_commit_failure_causes_duplicate() {
    let consumer = MockConsumer::new()
        .with_message(100, "test")
        .commit_fails_once();
    
    // First attempt: produce succeeds, commit fails
    process_batch(&consumer).await.unwrap_err();
    
    // Restart: reprocess from last committed offset
    let consumer = MockConsumer::new()
        .from_offset(100)  // Offset not committed
        .with_message(100, "test");
    
    process_batch(&consumer).await.unwrap();
    
    // Assert: Message produced twice
    assert_eq!(producer.send_count(), 2);
}
```

---

## References

- **ERROR_HANDLING.md**: Error types and recovery actions
- **src/error.rs**: Error type implementation
- **PROJECT_SPEC.md §5**: Delivery guarantees requirements
- **V1_PLAN.md Phase 1**: Core engine hardening

---

**Status:** Specification complete, implementation in progress  
**Version:** 1.0.0-alpha.1  
**Phase:** 1 (Core Engine Hardening)
