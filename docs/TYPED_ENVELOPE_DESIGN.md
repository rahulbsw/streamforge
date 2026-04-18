# Typed Envelope Design

**Version:** 1.0.0-alpha.1  
**Status:** Design Approved (Phase 1)  
**Implementation:** Planned for Phase 3

---

## Executive Summary

StreamForge v1.0 will introduce a **type-safe envelope system** that makes deserialization costs explicit and prevents invalid operations at compile time.

**Key Innovation:** `Envelope<K, V>` where `K` (key) and `V` (value) can be `Bytes`, `String`, or `Json`.

**Benefits:**
- 🚀 **3-4x faster** for passthrough/header-only pipelines (skip JSON parsing)
- ✅ **Type safety** at compile time (can't apply JSON filter on bytes)
- 📊 **Performance transparency** (type signature shows deserialization cost)
- 🎯 **Clear semantics** (Envelope<Bytes, Json> = "key passthrough, value parsed")

---

## Problem Statement

### Current Design (v0.4.0)

```rust
pub struct MessageEnvelope {
    pub key: Option<Value>,  // always JSON if present
    pub value: Value,        // always JSON
    pub headers: HashMap<String, Vec<u8>>,
    // ...
}
```

**Problems:**
1. ❌ Always deserializes key+value as JSON, even for passthrough
2. ❌ No compile-time guarantee that JSON operations are valid
3. ❌ Wastes CPU on deserialization when only headers are needed
4. ❌ ~35K msg/s throughput ceiling due to mandatory JSON parsing

### Example: Header-Only Routing (Inefficient)

```yaml
# Only need headers, but still parses JSON!
filter: "HEADER:x-tenant,==,production"
```

**Current cost:** ~800ns deserialization + 50ns header check = **850ns/msg**  
**Wasted work:** JSON parsing never used

---

## Solution: Typed Envelope

### Generic Envelope Type

```rust
pub struct Envelope<K, V> {
    pub key: Option<K>,
    pub value: V,
    pub headers: HashMap<String, Vec<u8>>,
    pub timestamp: Option<i64>,
    // ...
}
```

Where `K` and `V` can be:
- **`Bytes`** - raw bytes (no deserialization)
- **`String`** - UTF-8 validated string
- **`Json`** - parsed JSON Value

### Type-Driven Operations

| Envelope Type | Key Type | Value Type | Deserialization Cost | Throughput |
|---------------|----------|------------|---------------------|------------|
| `Envelope<Bytes, Bytes>` | Raw bytes | Raw bytes | **0ns** | ~**100K+ msg/s** |
| `Envelope<String, Bytes>` | UTF-8 string | Raw bytes | ~100ns (key only) | ~80K msg/s |
| `Envelope<Json, Bytes>` | JSON | Raw bytes | ~500ns (key only) | ~60K msg/s |
| `Envelope<Bytes, Json>` | Raw bytes | JSON | ~800ns (value only) | ~35K msg/s |
| `Envelope<Json, Json>` | JSON | JSON | ~1,300ns (both) | ~25K msg/s |

---

## Five Envelope Types

### 1. `Envelope<Bytes, Bytes>` - Passthrough

**Use case:** High-performance forwarding with header-based routing

```yaml
routing_type: header
destinations:
  - filter: "HEADER:x-tenant,==,prod"
    output: prod-events
  - filter: "HEADER:x-tenant,==,staging"
    output: staging-events
```

**Valid operations:**
- ✅ Header filters: `HEADER:name,==,value`
- ✅ Header transforms: `FROM:constant`
- ✅ Timestamp control: `PRESERVE`, `CURRENT`

**Invalid operations:**
- ❌ JSON path filters (compile error)
- ❌ Value transforms (compile error)

**Performance:** ~100K+ msg/s (3x faster than current)

---

### 2. `Envelope<String, Bytes>` - Key Routing

**Use case:** Route by key pattern without parsing value

```yaml
routing_type: filter
destinations:
  - filter: "KEY_PREFIX:user-"
    output: user-events
  - filter: "KEY_PREFIX:admin-"
    output: admin-events
```

**Valid operations:**
- ✅ Key regex: `KEY_MATCHES:^user-.*`
- ✅ Key prefix/suffix: `KEY_PREFIX:`, `KEY_SUFFIX:`
- ✅ Header operations

**Invalid operations:**
- ❌ Value JSON filters

**Performance:** ~80K msg/s (key string matching)

---

### 3. `Envelope<Json, Bytes>` - Key JSON Routing

**Use case:** Route by key JSON fields (rare, for multi-tenant keys)

```yaml
routing_type: filter
destinations:
  - filter: "KEY:/tenantId,==,prod"
    output: prod-events
```

**Valid operations:**
- ✅ JSON path filters on key: `KEY:/field,==,value`
- ✅ Key transforms

**Invalid operations:**
- ❌ Value JSON filters

**Performance:** ~60K msg/s (key JSON parsing)

---

### 4. `Envelope<Bytes, Json>` - Standard Processing

**Use case:** Filter/transform message payload (MOST COMMON)

```yaml
routing_type: filter
destinations:
  - filter: "/status,==,active"
    transform: "EXTRACT:/user/email,userEmail"
    output: active-users
  - filter: "/priority,==,high"
    output: high-priority
```

**Valid operations:**
- ✅ All JSON filters on value: `/path,==,value`
- ✅ All transforms: `EXTRACT`, `CONSTRUCT`, `HASH`
- ✅ Header operations

**Invalid operations:**
- ❌ JSON operations on key (key is bytes)

**Performance:** ~35K msg/s (same as current)

---

### 5. `Envelope<Json, Json>` - Full Processing

**Use case:** Complex routing based on both key and value

```yaml
routing_type: filter
destinations:
  - filter: "AND:KEY:/tenantId,==,prod:/status,==,active"
    output: prod-active-events
```

**Valid operations:**
- ✅ All JSON filters on both key and value
- ✅ All transforms on both

**Invalid operations:**
- None (most powerful, most expensive)

**Performance:** ~25K msg/s (double JSON parsing)

---

## Type Transitions

Pipeline stages can transition between envelope types:

```rust
// 1. Consume raw bytes from Kafka
Envelope<Bytes, Bytes>

// 2. Deserialize value for filtering
→ .deserialize_value()
→ Envelope<Bytes, Json>

// 3. Apply JSON filter (same type)
→ .filter("/status,==,active")
→ Envelope<Bytes, Json>

// 4. Extract key from value field
→ .key_from_value("/userId")
→ Envelope<Json, Json>

// 5. Serialize key back to bytes
→ .serialize_key()
→ Envelope<Bytes, Json>

// 6. Produce to Kafka (serialize value)
→ .serialize_value()
→ Envelope<Bytes, Bytes>
→ produce()
```

---

## Type Safety Examples

### Compile Error: JSON Filter on Bytes

```rust
// ❌ COMPILE ERROR
let envelope: Envelope<Bytes, Bytes> = consume();
envelope.filter("/status,==,active");
//       ^^^^^^ error: cannot apply JSON filter on Envelope<_, Bytes>
//              hint: call .deserialize_value() first
```

### Correct: Explicit Deserialization

```rust
// ✅ CORRECT
let envelope: Envelope<Bytes, Bytes> = consume();
let envelope: Envelope<Bytes, Json> = envelope.deserialize_value()?;
envelope.filter("/status,==,active"); // OK: value is Json
```

---

## DSL Type Requirements

Each DSL operation declares its type requirements:

| DSL Operation | Requires Key | Requires Value | Type Constraint |
|---------------|--------------|----------------|-----------------|
| `HEADER:name,==,value` | - | - | `Envelope<_, _>` |
| `KEY_PREFIX:prefix` | `String` | - | `Envelope<String, _>` |
| `KEY_MATCHES:regex` | `String` | - | `Envelope<String, _>` |
| `KEY:/field,==,value` | `Json` | - | `Envelope<Json, _>` |
| `/field,==,value` | - | `Json` | `Envelope<_, Json>` |
| `EXTRACT:/path,out` | - | `Json` | `Envelope<_, Json>` |
| `key_transform: "/path"` | - | `Json` | `Envelope<_, Json>` → `Envelope<Json, _>` |

**Parser validates:** Operations match envelope type at parse time.

---

## Migration Strategy

### Phase 1 (v1.0-alpha.2): Documentation + Validation

**Goal:** Document type requirements, validate at runtime

```rust
// Current implementation stays: MessageEnvelope
// Add runtime validation:
if filter.requires_json_value() && !value.is_json() {
    return Err("Cannot apply JSON filter on non-JSON value");
}
```

**Deliverables:**
- Update DSL_SPEC.md with type requirements
- Add validation in filter_parser.rs
- Error messages mention type mismatches

---

### Phase 2 (v1.0-beta.1): Generic Envelope Implementation

**Goal:** Implement `Envelope<K, V>` generic type

```rust
// src/envelope.rs
pub struct Envelope<K, V> {
    pub key: Option<K>,
    pub value: V,
    // ...
}

impl Envelope<Bytes, Bytes> {
    pub fn deserialize_value(self) -> Result<Envelope<Bytes, Json>> { ... }
}

impl Envelope<Bytes, Json> {
    pub fn deserialize_key(self) -> Result<Envelope<Json, Json>> { ... }
}
```

**Deliverables:**
- Refactor src/envelope.rs
- Type transition functions
- Update processor.rs to use typed envelopes

---

### Phase 3 (v1.0): Type-Safe DSL + Optimizations

**Goal:** Full type safety with compile-time checks

```rust
// DSL operations constrained by trait bounds
trait Filter<K, V> {
    fn evaluate(&self, envelope: &Envelope<K, V>) -> Result<bool>;
}

// JSON filter only works on Envelope<_, Json>
impl Filter<K, Json> for JsonPathFilter {
    fn evaluate(&self, envelope: &Envelope<K, Json>) -> Result<bool> {
        // Can safely access envelope.value as JSON
    }
}
```

**Deliverables:**
- Type-safe DSL traits
- Performance benchmarks (compare all envelope types)
- Optimization: skip deserialization for Envelope<Bytes, Bytes>

---

## Performance Impact

### Benchmark Results (Projected)

| Scenario | Current (v0.4) | Typed Envelope | Speedup |
|----------|---------------|----------------|---------|
| Header-only routing | ~35K msg/s | ~**100K+ msg/s** | **3-4x** |
| Key prefix routing | ~35K msg/s | ~**80K msg/s** | **2-3x** |
| Value JSON filtering | ~35K msg/s | ~35K msg/s | 1x (same) |
| Key+Value JSON | ~25K msg/s | ~25K msg/s | 1x (same) |

**Savings:** 0-800ns per message depending on envelope type

---

## Breaking Changes

### v0.4.0 → v1.0 Migration

**Before (v0.4.0):**
```yaml
filter: "HEADER:x-tenant,==,prod"
```
Still parses JSON unnecessarily.

**After (v1.0):**
```yaml
envelope_type: passthrough  # NEW: explicit type hint
filter: "HEADER:x-tenant,==,prod"
```
Optimized: skips JSON parsing.

**Backward compatibility:** If `envelope_type` not specified, default to `Envelope<Bytes, Json>` (current behavior).

---

## Implementation Checklist

### Phase 2 Deliverables
- [ ] Implement `Envelope<K, V>` generic struct
- [ ] Type transition functions:
  - [ ] `deserialize_key()`
  - [ ] `deserialize_value()`
  - [ ] `serialize_key()`
  - [ ] `serialize_value()`
- [ ] Refactor processor pipeline to use typed envelopes
- [ ] Update filter/transform traits with type parameters

### Phase 3 Deliverables
- [ ] Type-safe DSL operations (trait bounds)
- [ ] Compiler enforces type requirements
- [ ] Performance benchmarks:
  - [ ] Measure deserialization overhead by type
  - [ ] Throughput comparison (all envelope types)
- [ ] Documentation:
  - [ ] Update ADVANCED_DSL_GUIDE.md
  - [ ] Add performance tuning section
  - [ ] Migration guide for v0.4 users

---

## FAQ

### Q: Why not always parse JSON for simplicity?

**A:** Performance. Parsing JSON for 100K msg/s pipelines that only check headers wastes ~800ns/msg × 100K = **80ms/sec CPU** unnecessarily. That's an entire CPU core wasted on unused work.

### Q: Can I mix envelope types in one pipeline?

**A:** Yes! Type transitions allow you to start with `Envelope<Bytes, Bytes>`, then deserialize only when needed:
```rust
Envelope<Bytes, Bytes>
  → filter headers (fast)
  → deserialize_value()  // only if header filter passed
  → filter JSON (slower)
```

### Q: What if I don't care about performance?

**A:** Use `Envelope<Json, Json>` everywhere. Same behavior as v0.4.0, just explicit about the cost.

### Q: How does this affect existing pipelines?

**A:** Zero impact if you use default `envelope_type`. Opt-in to optimized types when you need performance.

---

## References

- **PROJECT_SPEC.md §9:** Envelope and Transformation Contract
- **V1_PLAN.md Phase 3:** Envelope/enrichment/runtime maturity
- **ARCHITECTURE.md:** Current MessageEnvelope implementation

---

**Status:** Design approved, implementation planned for Phase 3  
**Target:** v1.0.0-beta.1 (Phase 2), fully optimized in v1.0.0 (Phase 3)
