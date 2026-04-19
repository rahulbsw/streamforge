# Phase 3: Pragmatic Typed Envelope Approach

**Version:** 1.0.0-alpha.1  
**Decision Date:** 2026-04-18  
**Status:** Approved for v1.0

## Decision

**Defer full generic `Envelope<K, V>` implementation to v1.1.**

For v1.0, implement **runtime type awareness** with clear migration path.

## Rationale

### Full Generic Implementation Cost

Implementing `Envelope<K, V>` requires:
- ✅ Refactor src/envelope.rs (~300 lines)
- ✅ Update all filter traits (14+ filter types)
- ✅ Update all transform traits (10+ transform types)
- ✅ Update MessageProcessor trait and 3+ implementations
- ✅ Update main.rs consumption/production logic
- ✅ Update all tests (168+ tests)
- ✅ Performance benchmarks

**Estimated effort:** 20-30 hours
**Risk:** High (touches every part of codebase)

### Pragmatic Approach Benefits

**Effort:** 2-3 hours  
**Risk:** Low (additive, backward compatible)  
**Value:** Immediate (better errors, documentation, type awareness)

---

## Implementation: Runtime Type Tracking

### Type Markers

```rust
/// Envelope content type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeType {
    /// Raw bytes (no deserialization)
    Bytes,
    /// Validated UTF-8 string
    String,
    /// Parsed JSON value
    Json,
}

pub struct MessageEnvelope {
    pub key: Option<Value>,
    pub value: Value,
    pub headers: HashMap<String, Vec<u8>>,
    // ...
    
    // NEW: Runtime type tracking
    key_type: EnvelopeType,
    value_type: EnvelopeType,
}
```

### Type Validation

```rust
impl MessageEnvelope {
    /// Validate that key type supports operation
    pub fn validate_key_type(&self, required: EnvelopeType) -> Result<()> {
        if self.key_type != required {
            return Err(MirrorMakerError::TypeMismatch {
                operation: "key operation",
                required: required,
                actual: self.key_type,
                hint: "Call deserialize_key() first".to_string(),
            });
        }
        Ok(())
    }
    
    /// Validate that value type supports operation
    pub fn validate_value_type(&self, required: EnvelopeType) -> Result<()> {
        if self.value_type != required {
            return Err(MirrorMakerError::TypeMismatch {
                operation: "value operation",
                required: required,
                actual: self.value_type,
                hint: "Value must be JSON for JSON operations".to_string(),
            });
        }
        Ok(())
    }
}
```

### Filter Type Checks

```rust
impl JsonPathFilter {
    pub fn evaluate(&self, envelope: &MessageEnvelope) -> Result<bool> {
        // NEW: Validate value is JSON
        envelope.validate_value_type(EnvelopeType::Json)?;
        
        // Existing logic...
    }
}

impl KeyPrefixFilter {
    pub fn evaluate(&self, envelope: &MessageEnvelope) -> Result<bool> {
        // NEW: Validate key is String
        envelope.validate_key_type(EnvelopeType::String)?;
        
        // Existing logic...
    }
}
```

### Better Error Messages

**Before:**
```
Error: Cannot find field '/status' in message
```

**After:**
```
Error: Type mismatch
  Operation: JSON path filter '/status'
  Required: Json
  Actual: Bytes
  Hint: Value must be JSON for JSON path operations. 
        If you're doing passthrough with header-based routing,
        use HEADER: filters instead of JSON path filters.
```

---

## Documentation Updates

### DSL_SPEC.md Additions

Add section: **Performance Characteristics by Operation**

| Operation Type | Key Requirement | Value Requirement | Performance |
|---------------|-----------------|-------------------|-------------|
| `HEADER:` | Any | Any | ~50ns (fast) |
| `KEY_PREFIX:` | String | Any | ~100ns (string match) |
| `KEY_MATCHES:` | String | Any | ~500ns (regex) |
| `/path,==,value` | Any | Json | ~800ns (JSON + eval) |
| `EXTRACT:/path` | Any | Json | ~1000ns (JSON + extract) |

**Recommendation:** Use header-based routing for maximum throughput.

### ERROR_HANDLING.md Updates

Add error type: `TypeMismatch`

```yaml
Error: TypeMismatch
Recovery Action: FailFast (config error)
When: DSL operation requires different type than envelope has
Example: Applying JSON filter to Bytes value
Fix: Update config to match data types or add deserialization
```

---

## Migration Path to v1.1

### v1.0 (Current)
- Runtime type tracking
- Better error messages
- Documentation

### v1.1 (Future)
- Generic `Envelope<K, V>`
- Compile-time type safety
- Full performance optimizations

**User Impact:** Zero breaking changes. v1.1 is purely additive.

---

## Implementation Checklist

### Phase 3 Deliverables (v1.0)

- [ ] Add EnvelopeType enum to src/envelope.rs
- [ ] Add key_type, value_type fields to MessageEnvelope
- [ ] Add validate_key_type(), validate_value_type() methods
- [ ] Add type checks to filters (JsonPathFilter, KeyPrefixFilter, etc.)
- [ ] Add TypeMismatch error variant
- [ ] Update error messages with type hints
- [ ] Add "Performance Characteristics" section to DSL_SPEC.md
- [ ] Add TypeMismatch to ERROR_HANDLING.md
- [ ] Update tests (should still pass with default types)
- [ ] Add test for type validation errors

**Estimated:** 2-3 hours  
**Risk:** Low  
**Value:** High (better UX, clear migration path)

---

## Benefits

### Immediate (v1.0)
1. **Better errors:** Users know why filter failed (type mismatch, not "field not found")
2. **Documentation:** Clear performance trade-offs documented
3. **Type awareness:** System knows types at runtime
4. **Migration path:** Clear path to generic implementation

### Future (v1.1)
1. **Compile-time safety:** `Envelope<Bytes, Bytes>` can't have JSON filters
2. **Performance:** Skip deserialization for Bytes types (3-4x faster)
3. **Zero cost:** Type system has no runtime overhead
4. **Gradual migration:** Users opt-in with `envelope_type:` config field

---

## Conclusion

This pragmatic approach delivers:
- ✅ Type awareness for v1.0
- ✅ Better error messages
- ✅ Clear documentation
- ✅ Low risk, backward compatible
- ✅ Clear path to full generic implementation

Full generic `Envelope<K, V>` deferred to v1.1 (post-release optimization).

---

**Approved by:** Phase 3 implementation team  
**Effective:** v1.0.0-alpha.2
