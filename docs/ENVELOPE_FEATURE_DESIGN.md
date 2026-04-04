# Kafka Message Envelope Support - Design Document

**Date**: April 3, 2026  
**Feature**: Full Kafka message envelope transformation (key, value, headers, timestamp)  
**Status**: 🚧 Design Phase

---

## Overview

Extend Streamforge to support filtering and transformation not just on message **values**, but on the complete Kafka message envelope:
- **Key** - Message key (routing, partitioning)
- **Value** - Message payload (current support)
- **Headers** - Metadata key-value pairs
- **Timestamp** - Message timestamp

---

## Current Limitations

### What Works Today ✅
- Filter on message **value** (JSON payload)
- Transform message **value**
- Key is passed through unchanged
- Headers are not accessible
- Timestamp is not accessible

### What Doesn't Work ❌
- Cannot filter based on message **key**
- Cannot transform message **key**
- Cannot access or filter on **headers**
- Cannot set/modify **headers**
- Cannot filter or modify **timestamp**
- Cannot set custom key/headers when writing to destination

### Example Use Cases That Fail Today

```yaml
# Use Case 1: Filter by key prefix
filter: "KEY_MATCHES:^user-.*"  # ❌ Not supported

# Use Case 2: Extract key field into value
transform: "SET:/userId,KEY"  # ❌ Not supported

# Use Case 3: Filter by header
filter: "HEADER:/x-tenant,==,prod"  # ❌ Not supported

# Use Case 4: Set custom key from value field
output_key: "/user/id"  # ❌ Not supported

# Use Case 5: Add headers to output
output_headers:
  x-processed-by: streamforge  # ❌ Not supported
```

---

## Proposed Design

### 1. Message Envelope Structure

```rust
/// Complete Kafka message envelope
pub struct MessageEnvelope {
    /// Message key (optional in Kafka)
    pub key: Option<Value>,
    
    /// Message value (payload)
    pub value: Value,
    
    /// Message headers
    pub headers: HashMap<String, Vec<u8>>,
    
    /// Message timestamp (milliseconds since epoch)
    pub timestamp: Option<i64>,
    
    /// Original partition (for routing reference)
    pub partition: Option<i32>,
    
    /// Original offset (for tracking)
    pub offset: Option<i64>,
}
```

### 2. Enhanced Filter Trait

```rust
pub trait Filter: Send + Sync {
    /// Evaluate filter on complete envelope
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool>;
    
    /// Legacy method (for backward compatibility)
    fn evaluate(&self, value: &Value) -> Result<bool> {
        let envelope = MessageEnvelope {
            key: None,
            value: value.clone(),
            headers: HashMap::new(),
            timestamp: None,
            partition: None,
            offset: None,
        };
        self.evaluate_envelope(&envelope)
    }
}
```

### 3. Enhanced Transform Trait

```rust
pub trait Transform: Send + Sync {
    /// Transform complete envelope
    fn transform_envelope(&self, envelope: MessageEnvelope) -> Result<MessageEnvelope>;
    
    /// Legacy method (for backward compatibility)
    fn transform(&self, value: Value) -> Result<Value> {
        let envelope = MessageEnvelope {
            key: None,
            value,
            headers: HashMap::new(),
            timestamp: None,
            partition: None,
            offset: None,
        };
        let result = self.transform_envelope(envelope)?;
        Ok(result.value)
    }
}
```

### 4. Configuration Schema

```yaml
routing:
  destinations:
    - output: user-events
      description: "User events with enriched keys"
      
      # Filter on any part of envelope
      filter: "AND(
        /user/active,==,true,
        KEY_MATCHES:^user-.*,
        HEADER:/x-tenant,==,production
      )"
      
      # Transform value (existing)
      transform: "CONSTRUCT:userId=/user/id:email=/user/email"
      
      # Transform key (NEW)
      key_transform: "CONSTRUCT:id=/user/id:type=user"
      # Or simple expression
      key_transform: "/user/id"  # Extract field as key
      # Or template
      key_transform: "TEMPLATE:user-{/user/id}"
      
      # Set/add headers (NEW)
      headers:
        x-processed-by: "streamforge"
        x-source-topic: "ORIGINAL_TOPIC"  # Magic variable
        x-user-id: "/user/id"  # Extract from value
        x-timestamp: "CURRENT_TIMESTAMP"  # Magic variable
      
      # Header transformations (NEW)
      header_transforms:
        - header: x-tenant
          transform: "UPPER"  # Uppercase value
        - header: x-correlation-id
          transform: "/request/correlationId"  # Extract from value
      
      # Timestamp handling (NEW)
      timestamp: "PRESERVE"  # PRESERVE | CURRENT | <field_path>
      # timestamp: "CURRENT"  # Use current time
      # timestamp: "/event/timestamp"  # Extract from value field
```

---

## DSL Extensions

### Key Operations

#### Key Filters
```
KEY_MATCHES:<regex>              # Key matches regex pattern
KEY_PREFIX:<prefix>              # Key starts with prefix
KEY_SUFFIX:<suffix>              # Key ends with suffix
KEY_CONTAINS:<substring>         # Key contains substring
KEY_LENGTH:<op>,<value>          # Key length comparison
KEY_EXISTS                       # Key is not null
KEY_IS_NULL                      # Key is null
KEY_EQUALS:<value>               # Key equals exact value
```

#### Key Transformations
```
KEY                              # Use current key as-is
KEY_FIELD:<json_path>            # Extract field from key (if JSON)
KEY_TEMPLATE:<template>          # Template with placeholders
KEY_FROM:<value_path>            # Extract from value field
KEY_CONSTANT:<value>             # Set constant key
KEY_CONCAT:<path1>,<path2>       # Concatenate fields
KEY_HASH:<algo>,<path>           # Hash of field
```

### Header Operations

#### Header Filters
```
HEADER:<name>,<op>,<value>       # Header comparison
HEADER_EXISTS:<name>             # Header exists
HEADER_MATCHES:<name>,<regex>    # Header matches regex
HEADER_PREFIX:<name>,<prefix>    # Header starts with
HEADER_IN:<name>,<v1>,<v2>       # Header value in list
```

#### Header Transformations
```
HEADER_SET:<name>,<value>        # Set header to constant
HEADER_FROM:<name>,<path>        # Extract from value
HEADER_COPY:<src>,<dst>          # Copy header
HEADER_REMOVE:<name>             # Remove header
HEADER_RENAME:<old>,<new>        # Rename header
HEADER_TEMPLATE:<name>,<tmpl>    # Template-based
```

### Timestamp Operations

#### Timestamp Filters
```
TIMESTAMP_AFTER:<epoch_ms>       # After timestamp
TIMESTAMP_BEFORE:<epoch_ms>      # Before timestamp
TIMESTAMP_BETWEEN:<start>,<end>  # In range
TIMESTAMP_AGE:<op>,<seconds>     # Age comparison
TIMESTAMP_EXISTS                 # Has timestamp
```

#### Timestamp Transformations
```
TIMESTAMP_PRESERVE               # Keep original
TIMESTAMP_CURRENT                # Use current time
TIMESTAMP_FROM:<path>            # Extract from value
TIMESTAMP_ADD:<seconds>          # Add offset
TIMESTAMP_SUBTRACT:<seconds>     # Subtract offset
```

---

## Configuration Examples

### Example 1: User Events with Key Routing

```yaml
appid: user-event-router
bootstrap: kafka-source:9092
target_broker: kafka-dest:9092
input: raw-events
threads: 4

routing:
  destinations:
    # Active users only, set key to user ID
    - output: active-users
      filter: "AND(
        /user/active,==,true,
        /user/tier,IN,premium,enterprise
      )"
      # Set key to user ID for partitioning
      key_transform: "/user/id"
      headers:
        x-event-type: "user-active"
        x-tier: "/user/tier"
    
    # All users with correlation tracking
    - output: all-users
      key_transform: "TEMPLATE:user-{/user/id}"
      headers:
        x-correlation-id: "HEADER:x-request-id"  # Copy from input
        x-processed-at: "CURRENT_TIMESTAMP"
```

### Example 2: Multi-Tenant Filtering by Header

```yaml
routing:
  destinations:
    # Production tenant events only
    - output: prod-events
      filter: "AND(
        HEADER_EXISTS:x-tenant,
        HEADER:/x-tenant,==,production
      )"
      key_transform: "CONSTRUCT:tenant=/x-tenant:id=/event/id"
      headers:
        x-environment: "production"
    
    # Test tenant events
    - output: test-events
      filter: "HEADER:/x-tenant,==,test"
      key_transform: "/event/id"
      headers:
        x-environment: "test"
```

### Example 3: Time-Based Filtering

```yaml
routing:
  destinations:
    # Recent events only (last 5 minutes)
    - output: recent-events
      filter: "TIMESTAMP_AGE:<,300"  # Less than 300 seconds old
      timestamp: "PRESERVE"
    
    # Historical events with current timestamp
    - output: historical-events
      filter: "TIMESTAMP_AGE:>=,300"
      timestamp: "CURRENT"  # Update to current time
```

### Example 4: Header Enrichment

```yaml
routing:
  destinations:
    - output: enriched-events
      # Add multiple headers
      headers:
        x-source-topic: "ORIGINAL_TOPIC"
        x-source-partition: "ORIGINAL_PARTITION"
        x-processed-by: "streamforge-v0.3"
        x-processing-timestamp: "CURRENT_TIMESTAMP"
        x-user-id: "/user/id"
        x-event-type: "/event/type"
      
      # Transform existing headers
      header_transforms:
        - header: x-correlation-id
          transform: "HEADER:x-request-id"  # Copy from input
        - header: x-tenant
          transform: "UPPER"  # Uppercase
```

### Example 5: Complex Key Construction

```yaml
routing:
  destinations:
    - output: events
      # Composite key from multiple fields
      key_transform: "CONSTRUCT:
        tenant=/tenant/id:
        user=/user/id:
        type=/event/type
      "
      # Results in key: {"tenant":"t1","user":"u1","type":"login"}
```

---

## Implementation Plan

### Phase 1: Core Infrastructure (Week 1)
1. ✅ Design document (this file)
2. ⏳ Create `MessageEnvelope` struct
3. ⏳ Update `Filter` trait with envelope support
4. ⏳ Update `Transform` trait with envelope support
5. ⏳ Add backward compatibility layer

### Phase 2: DSL Extensions (Week 1-2)
6. ⏳ Implement key filter expressions
7. ⏳ Implement header filter expressions
8. ⏳ Implement timestamp filter expressions
9. ⏳ Implement key transformations
10. ⏳ Implement header transformations
11. ⏳ Implement timestamp transformations

### Phase 3: Configuration (Week 2)
12. ⏳ Extend `DestinationConfig` with new fields
13. ⏳ Add configuration parsing
14. ⏳ Add validation
15. ⏳ Update configuration examples

### Phase 4: Integration (Week 2)
16. ⏳ Update `main.rs` to parse full envelope
17. ⏳ Update `processor.rs` to use envelope
18. ⏳ Update `sink.rs` to write key/headers
19. ⏳ Handle header serialization/deserialization

### Phase 5: Testing (Week 3)
20. ⏳ Unit tests for new filters
21. ⏳ Unit tests for new transforms
22. ⏳ Integration tests with Kafka
23. ⏳ Performance benchmarks
24. ⏳ Documentation and examples

---

## Backward Compatibility

### Existing Configurations Work As-Is

```yaml
# This still works (no changes needed)
routing:
  destinations:
    - output: events
      filter: "/user/active,==,true"
      transform: "CONSTRUCT:userId=/user/id"
```

### Migration Path

Old syntax continues to work:
- `filter` on value → Automatic envelope wrapper
- `transform` on value → Automatic envelope wrapper
- No key/headers → Passed through unchanged

New features are opt-in:
- Add `key_transform` when needed
- Add `headers` when needed
- Use envelope filters when needed

---

## Performance Considerations

### Memory Impact
- **Before:** Pass `Value` (8-24 bytes pointer)
- **After:** Pass `MessageEnvelope` (80-120 bytes with headers)
- **Mitigation:** Use `Arc<MessageEnvelope>` for sharing

### Processing Overhead
- **Header parsing:** ~100-500ns per message
- **Key transformation:** ~500-2000ns depending on operation
- **Overall impact:** < 5% throughput degradation
- **Mitigation:** Lazy header parsing, only when accessed

### Optimization Strategies
1. **Lazy evaluation:** Only parse headers if filter/transform uses them
2. **Arc sharing:** Share envelope across filters to avoid clones
3. **Header caching:** Cache parsed headers
4. **Fast path:** If no envelope operations, skip envelope creation

---

## Security Considerations

### Header Injection
- **Risk:** User-controlled data in headers could inject malicious content
- **Mitigation:** Validate header names (alphanumeric + hyphens only)
- **Mitigation:** Sanitize header values (escape special characters)

### Information Disclosure
- **Risk:** Headers might contain sensitive data (tokens, keys)
- **Mitigation:** Document security best practices
- **Mitigation:** Add header filtering/redaction support

### DoS via Headers
- **Risk:** Large number of headers could cause memory exhaustion
- **Mitigation:** Limit maximum headers per message (default: 100)
- **Mitigation:** Limit header value size (default: 10KB)

---

## Documentation Updates Needed

1. **DSL Guide** - Add envelope operations section
2. **Configuration Guide** - Add key/header/timestamp examples
3. **Usage Guide** - Add envelope use cases
4. **Migration Guide** - How to adopt new features
5. **Performance Guide** - Impact and optimization tips
6. **Security Guide** - Header security best practices

---

## Testing Strategy

### Unit Tests
- ✅ Envelope creation and manipulation
- ✅ Key filter expressions
- ✅ Header filter expressions
- ✅ Timestamp filter expressions
- ✅ Key transformations
- ✅ Header transformations
- ✅ Backward compatibility

### Integration Tests
- ✅ Full message flow with headers
- ✅ Multi-destination with different keys
- ✅ Header enrichment pipeline
- ✅ Time-based filtering

### Performance Tests
- ✅ Baseline (value-only processing)
- ✅ With key operations
- ✅ With header operations
- ✅ With full envelope operations
- ✅ Impact on throughput

---

## Success Criteria

1. ✅ All existing configurations work without changes
2. ✅ Can filter on key, headers, timestamp
3. ✅ Can transform key, headers, timestamp
4. ✅ Can set custom key per destination
5. ✅ Can set/add headers per destination
6. ✅ Performance degradation < 5%
7. ✅ Comprehensive documentation
8. ✅ 100% test coverage for new features

---

## Future Enhancements (Post-MVP)

### Advanced Key Operations
- `KEY_CRYPTO`: Encrypt/decrypt keys
- `KEY_MASK`: Partial masking for PII
- `KEY_LOOKUP`: Enrich from external store

### Advanced Header Operations
- `HEADER_DECODE_BASE64`: Decode base64 headers
- `HEADER_JSON_EXTRACT`: Parse JSON header values
- `HEADER_AGGREGATE`: Combine multiple headers

### Advanced Timestamp Operations
- `TIMESTAMP_ROUND`: Round to minute/hour/day
- `TIMESTAMP_TIMEZONE`: Convert timezones
- `TIMESTAMP_FORMAT`: Custom formatting

### Schema Registry Integration
- Validate keys against schemas
- Serialize keys with Avro/Protobuf
- Schema evolution support

---

## Questions & Decisions

### Q: Should we support complex key types (Avro, Protobuf)?
**Decision:** MVP supports JSON keys only. Add schema support in Phase 2.

### Q: How to handle binary header values?
**Decision:** Support both string and binary. Use base64 for binary in DSL.

### Q: Should timestamp be mutable?
**Decision:** Yes, allow setting custom timestamps (with caution in docs).

### Q: Performance vs features trade-off?
**Decision:** Optimize for common case (value-only), add fast path.

---

##  References

- [Kafka Message Format](https://kafka.apache.org/documentation/#messages)
- [rdkafka Headers API](https://docs.rs/rdkafka/latest/rdkafka/message/struct.OwnedHeaders.html)
- [Kafka Header Use Cases](https://www.confluent.io/blog/5-things-every-kafka-developer-should-know/)

---

**Status:** 📝 Design Complete  
**Next Step:** Begin implementation (Phase 1)  
**Estimated Completion:** 3 weeks  
**Priority:** HIGH - Frequently requested feature
