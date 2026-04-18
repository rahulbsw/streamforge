# Architecture Overview

High-level architecture and design principles for Streamforge (formerly StreamForge).

---

## System Overview

Streamforge is a high-performance Kafka streaming toolkit that mirrors, filters, transforms, and routes messages between Kafka clusters with sub-microsecond latency.

```
┌─────────────────┐         ┌──────────────────┐         ┌─────────────────┐
│  Source Kafka   │────────>│   Streamforge    │────────>│  Target Kafka   │
│   Cluster(s)    │         │   Processing     │         │   Cluster(s)    │
└─────────────────┘         └──────────────────┘         └─────────────────┘
                                      │
                                      │ Optional:
                                      ├─ Filter (44-145ns)
                                      ├─ Transform (810-1,633ns)
                                      ├─ Hash for deduplication
                                      └─ Route to multiple destinations
```

---

## Core Architecture

### High-Level Design

```
┌────────────────────────────────────────────────────────────────┐
│                        Streamforge                             │
├────────────────────────────────────────────────────────────────┤
│  Configuration Layer                                           │
│  ├─ YAML/JSON Config Parser                                   │
│  └─ Security Configuration (SSL/TLS, SASL)                    │
├────────────────────────────────────────────────────────────────┤
│  Consumer Layer                                                │
│  ├─ Kafka StreamConsumer (rdkafka)                            │
│  ├─ Concurrent Batch Processing (80 parallel ops)             │
│  ├─ At-least-once / At-most-once semantics                    │
│  └─ Manual/Auto commit strategies                             │
├────────────────────────────────────────────────────────────────┤
│  Processing Layer                                              │
│  ├─ Message Processor (async trait)                           │
│  │   ├─ Single Destination Processor                          │
│  │   └─ Multi Destination Processor                           │
│  ├─ Filter Engine (custom DSL)                                │
│  │   ├─ Boolean Logic (AND/OR/NOT)                            │
│  │   ├─ Regex Matching                                        │
│  │   └─ Array Operations                                      │
│  ├─ Transform Engine (custom DSL)                             │
│  │   ├─ Field Mapping                                         │
│  │   ├─ Object Construction                                   │
│  │   └─ Arithmetic Operations                                 │
│  └─ Hashing & Caching (optional)                              │
│      ├─ SHA256 hashing for deduplication                      │
│      └─ LRU/Redis cache backends                              │
├────────────────────────────────────────────────────────────────┤
│  Producer Layer                                                │
│  ├─ Kafka Sink (FutureProducer)                               │
│  ├─ Custom Partitioning                                       │
│  ├─ Compression (gzip/snappy/zstd/lz4)                        │
│  └─ Async message delivery                                    │
├────────────────────────────────────────────────────────────────┤
│  Observability Layer                                           │
│  ├─ Metrics (Stats Reporter)                                  │
│  ├─ Tracing (tracing crate)                                   │
│  └─ Error Handling                                             │
└────────────────────────────────────────────────────────────────┘
```

---

## Key Design Decisions

### 1. Rust Language Choice

**Rationale:**
- **Memory Safety**: Zero-cost abstractions, no garbage collection
- **Performance**: Native compilation, minimal runtime overhead
- **Concurrency**: Fearless concurrency with ownership model
- **Ecosystem**: Rich async ecosystem (Tokio, rdkafka)

**Benefits Achieved:**
- 40x faster filters/transforms vs Java JSLT
- 10x less memory (~50MB vs ~500MB)
- 2.5x higher throughput (25K+ msg/s vs 10K msg/s)
- Zero CVEs with Chainguard base images

### 2. Custom DSL (No External Dependencies)

**Decision:** Build custom string-based filtering/transformation DSL instead of using JSLT/JavaScript/Rhai

**Rationale:**
- JSLT (Java) and JavaScript engines have significant overhead
- Rhai (Rust scripting) adds ~300KB binary size and runtime complexity
- Custom DSL optimized for message streaming patterns
- Sub-microsecond performance critical for throughput
- Explicit syntax matches Kafka streaming patterns

**Results:**
- Simple filters: 44-50ns (vs ~2,000ns for JSLT)
- Transforms: 810-1,633ns (vs ~40,000ns for JSLT)
- Zero external dependencies for core DSL
- Colon-delimited syntax (e.g., `/path,==,value`, `AND:cond1:cond2`)

**v1.0 Gaps:**
- ❌ No formal grammar (EBNF)
- ❌ Parser lacks validation layer (errors found at runtime)
- ❌ No AST representation
- ❌ Error messages lack context
- ⏭️ Planned: Separate parser/AST/validator/evaluator in Phase 2

### 3. Async/Await Architecture

**Decision:** Use Tokio async runtime for all I/O operations

**Rationale:**
- Non-blocking I/O maximizes CPU utilization
- Concurrent message processing without thread-per-message overhead
- Efficient resource usage for high-throughput scenarios

**Implementation:**
- `async fn process()` for message processing
- `buffer_unordered()` for concurrent batch processing
- 80 parallel operations (8 threads × 10 parallelism factor)

### 4. Concurrent Batch Processing

**Decision:** Process messages in batches with configurable concurrency

**Configuration:**
```rust
BATCH_SIZE = 100                // Messages per batch
BATCH_FILL_TIMEOUT_MS = 100     // Max wait for batch
PARALLELISM_FACTOR = 10         // threads × 10 = concurrency
```

**Rationale:**
- Balance throughput and latency
- Efficient commit strategies
- Resource pooling (connections, buffers)

**Results:**
- 132x improvement over sequential (83 → 11,000 msg/s)
- Perfect linear scaling (2.0x from 4 to 8 threads)
- Peak throughput: 34,517 msg/s sustained

### 5. Pluggable Delivery Semantics

**Decision:** Support both at-least-once and at-most-once delivery

**Configuration:**
```yaml
commit_strategy:
  manual_commit: true    # At-least-once
  commit_mode: sync      # Async or Sync

dead_letter_queue:
  enabled: true
  topic: streamforge-dlq
  max_retries: 3
```

**Rationale:**
- Different use cases have different requirements
- Trade-off between throughput and guarantees
- Flexibility for users to choose

**Performance:**
- At-least-once: 10,933 msg/s with full durability
- At-most-once: 11,200 msg/s (~3% overhead for guarantees)

**v1.0 Gaps:**
- ❌ Commit semantics not formally documented
- ❌ Retry backoff policy undefined
- ❌ DLQ message format unspecified
- ❌ No integration tests for failure scenarios
- ⏭️ Planned: docs/DELIVERY_GUARANTEES.md + tests in Phase 1

### 6. Multi-Destination Routing

**Decision:** Support content-based routing to multiple destinations

**Architecture:**
```rust
pub struct DestinationProcessor {
    sink: Arc<KafkaSink>,
    filter: Arc<dyn Filter>,      // Optional
    transform: Arc<dyn Transform>, // Optional
    name: String,
}

pub struct MultiDestinationProcessor {
    destinations: Vec<DestinationProcessor>,
    routing_path: Option<String>,
}
```

**Rationale:**
- Single pipeline can serve multiple use cases
- Filter and transform per destination
- Efficient: share consumer, process once

### 7. Cache-Based Deduplication

**Decision:** Optional hash-based deduplication with pluggable cache backends

**Supported Backends:**
- **LRU Cache** (in-memory, fast, bounded)
- **Redis** (distributed, persistent, shared)

**Rationale:**
- Handle duplicate messages from upstream
- Configurable cache backend based on scale
- Async cache operations don't block processing

---

## Data Flow

### Single Destination Flow

```
1. Consumer reads message batch (100 messages, 100ms timeout)
   ↓
2. Parse message key (permissive) and value (strict JSON)
   ↓
3. Process batch concurrently (80 parallel operations)
   ├─ Apply filter (if configured)
   ├─ Apply transform (if configured)
   └─ Check cache/hash (if configured)
   ↓
4. Send to Kafka sink (async)
   ↓
5. Commit offsets (if manual commit mode)
   ├─ Retry with exponential backoff (3 attempts)
   └─ Halt on persistent failure (prevent data loss)
```

### Multi-Destination Flow

```
1. Consumer reads message batch
   ↓
2. Parse message
   ↓
3. For each destination (in parallel):
   ├─ Evaluate destination-specific filter
   ├─ Apply destination-specific transform
   ├─ Check destination-specific cache
   └─ Send to destination sink
   ↓
4. Collect results
   ├─ If any destination failed → halt (data integrity)
   └─ If all succeeded → commit offsets
```

---

## Component Details

### Configuration Layer

**File:** `src/config.rs`

Responsibilities:
- Parse YAML/JSON configuration
- Validate configuration
- Apply security settings
- Provide defaults

### Consumer Layer

**File:** `src/main.rs`

Responsibilities:
- Create Kafka consumer
- Subscribe to topics
- Manage consumer groups
- Handle offset commits
- Implement commit retry logic

### Processing Layer

**Files:** `src/processor.rs`, `src/filter/`, `src/transform.rs`

Responsibilities:
- **MessageProcessor trait**: Define processing interface
- **SingleDestinationProcessor**: Single output processing
- **MultiDestinationProcessor**: Multi-output routing
- **Filter Engine**: Evaluate filter expressions (44-145ns)
- **Transform Engine**: Apply transformations (810-1,633ns)

### Producer Layer

**Files:** `src/kafka/sink.rs`, `src/kafka/partitioner.rs`

Responsibilities:
- Create Kafka producer (FutureProducer)
- Handle custom partitioning
- Apply compression
- Send messages asynchronously
- Handle producer errors

### Observability Layer

**Files:** `src/metrics.rs`

Responsibilities:
- Track processed messages
- Track completed messages
- Track errors
- Report statistics (every 10 seconds)
- Tracing integration

---

## Performance Architecture

### Throughput Optimization

1. **Concurrent Batch Processing**
   - Process 100 messages per batch
   - 80 concurrent operations (8 threads × 10)
   - Result: 132x throughput improvement

2. **Async I/O**
   - Non-blocking Kafka I/O
   - Tokio runtime for efficient scheduling
   - Result: Maximize CPU utilization

3. **Custom DSL**
   - Zero-overhead parsing (compile-time)
   - Sub-microsecond filter/transform
   - Result: 40x faster than JSLT

4. **Efficient Memory Usage**
   - ~50MB RAM footprint
   - Zero garbage collection
   - Result: 10x less memory than Java

### Latency Optimization

1. **Minimal Processing Overhead**
   - Filters: 44-145ns per message
   - Transforms: 810-1,633ns per message
   - Total: < 2µs per message

2. **Batch Timeout**
   - 100ms max wait for batch
   - Ensures low-latency during low traffic
   - Result: P99 latency < 150ms

---

## Scaling Architecture

### Vertical Scaling

**Single Instance:**
- 4 threads → 10,933 msg/s
- 8 threads → 25,000-30,000 msg/s (linear scaling)
- Scales with CPU cores

**Configuration:**
```yaml
threads: 8              # Number of consumer threads
```

### Horizontal Scaling

**Multiple Instances:**
- Kafka consumer groups
- Partitions distributed across instances
- Each instance processes subset of partitions

**Example:**
```
8 partitions, 2 instances:
- Instance 1: partitions 0-3
- Instance 2: partitions 4-7
```

### Kubernetes Scaling

**Horizontal Pod Autoscaler (HPA):**
```yaml
minReplicas: 2
maxReplicas: 10
targetCPUUtilizationPercentage: 70
```

**Scaling triggers:**
- CPU utilization
- Custom metrics (lag, throughput)
- Message queue depth

---

## Security Architecture

### Authentication

Supported mechanisms:
- **SASL/PLAIN** - Username/password (simple)
- **SASL/SCRAM-SHA-256** - Username/password (secure)
- **SASL/SCRAM-SHA-512** - Username/password (more secure)
- **SASL/GSSAPI** - Kerberos
- **Mutual TLS** - Certificate-based

### Encryption

- **SSL/TLS** - Transport encryption
- **TLS 1.2/1.3** - Modern protocols
- **Certificate validation** - Hostname verification

### Secrets Management

- **Environment variables** - For sensitive values
- **Kubernetes secrets** - For K8s deployments
- **File-based secrets** - Certificate files

---

## Reliability Architecture

### Error Handling

1. **Parse Errors**
   - Log with full context (topic, partition, offset, key)
   - Count as error in metrics
   - Handled per delivery semantics

2. **Processing Errors**
   - Propagate to batch level
   - Trigger commit failure handling

3. **Commit Errors**
   - Retry with exponential backoff (3 attempts)
   - Halt on persistent failure (prevent data loss)

### Delivery Guarantees

**At-least-once:**
- Manual commits after successful processing
- Retry logic prevents message loss
- Duplicates possible on failure recovery

**At-most-once:**
- Auto-commit mode
- Lower overhead (~3%)
- Message loss possible on failure

---

## Deployment Architecture

### Docker

```
streamforge:latest (20MB image)
├─ Chainguard base (minimal, zero CVEs)
├─ Static binary (no runtime dependencies)
└─ Config via volume mount or env vars
```

### Kubernetes

```
Deployment
├─ ConfigMap (configuration)
├─ Secret (credentials)
├─ Service (metrics endpoint)
└─ HPA (auto-scaling)
```

### Monitoring

- **Metrics**: Built-in stats reporter
- **Logs**: Structured logging via tracing
- **Traces**: OpenTelemetry compatible

---

## Module Organization (v1.0.0-alpha.1)

```
src/
├── main.rs                    # Entry point, tokio runtime setup
├── lib.rs                     # Public API exports
├── config.rs                  # YAML/JSON configuration parsing
├── error.rs                   # Error types (⚠️ currently string-based)
│
├── processor.rs               # Message processing traits (~500 lines)
├── filter_parser.rs           # DSL parser (~1800 lines)
│
├── filter/
│   ├── mod.rs                 # Filter and Transform traits
│   ├── envelope_filter.rs     # Envelope-aware filters
│   └── envelope_transform.rs  # Envelope transformations
│
├── kafka/
│   ├── mod.rs                 # Kafka client abstractions
│   └── sink.rs                # Producer wrapper (~300 lines)
│
├── envelope.rs                # MessageEnvelope struct
├── partitioner.rs             # Partitioning strategies
├── compression.rs             # Compression codec support
│
├── cache.rs                   # Cache trait
├── cache_backend.rs           # Cache implementations (~600 lines)
├── hash.rs                    # Hashing functions (MD5/SHA/Murmur)
│
└── observability/
    ├── mod.rs                 # Observability exports
    ├── metrics.rs             # Prometheus metric definitions
    ├── server.rs              # HTTP metrics endpoint
    └── lag_monitor.rs         # Consumer lag tracking
```

**Total:** ~15,638 lines of Rust code (as of v0.4.0)

### Key Module Dependencies

- **filter_parser.rs** → serde_json, regex (no AST layer yet)
- **processor.rs** → filter/, kafka/sink
- **kafka/sink.rs** → rdkafka, compression
- **observability/** → prometheus, axum
- **cache_backend.rs** → moka, redis (optional), dashmap

## v1.0 Roadmap and Known Gaps

### Phase 1: Core Engine Hardening (IN PROGRESS)

**Critical gaps blocking v1.0:**

1. **Error Type System** (`src/error.rs`)
   - Currently: String-based errors (`anyhow::Error`)
   - Needed: Typed error hierarchy with context
   - Deliverable: Refactored `src/error.rs` + `docs/ERROR_HANDLING.md`

2. **Delivery Semantics** (`src/processor.rs`)
   - Currently: At-least-once implicit, no tests
   - Needed: Explicit commit strategies, offset management tests
   - Deliverable: `docs/DELIVERY_GUARANTEES.md` + integration tests

3. **Retry and DLQ** (`src/retry.rs`, `src/dlq.rs`)
   - Currently: Basic implementation, semantics undefined
   - Needed: Retry policy (count, backoff), DLQ format
   - Deliverable: Modules + metrics + tests

4. **Integration Tests** (`tests/integration/`)
   - Currently: Only unit tests (92 passing)
   - Needed: End-to-end tests with Testcontainers
   - Deliverable: 10+ integration scenarios, failure injection

### Phase 2: DSL Stabilization

**DSL gaps:**

5. **Formal Grammar** (`docs/DSL_SPEC.md`)
   - Currently: Informal syntax examples
   - Needed: EBNF grammar, operator precedence, escaping rules
   - Deliverable: Complete DSL specification

6. **Parser Refactor** (`src/dsl/`)
   - Currently: `filter_parser.rs` monolith
   - Needed: Separate parser/AST/validator/evaluator
   - Deliverable: `src/dsl/ast.rs`, `src/dsl/parser.rs`, `src/dsl/validator.rs`

7. **Config Validation** (`src/bin/validate.rs`)
   - Currently: No pre-deploy validation
   - Needed: CLI tool to validate config files
   - Deliverable: `streamforge validate config.yaml` command

### Phase 3-6: See V1_PLAN.md

**Phases:**
- Phase 3: Envelope/Enrichment/Runtime Maturity
- Phase 4: Operability and Deployment
- Phase 5: UI/Operator Polish
- Phase 6: v1.0 Release Readiness

**Estimated total:** ~30 hours of autonomous execution

## Future Architecture Considerations (Post v1.0)

### Planned Improvements

1. **Exactly-Once Semantics**
   - Transactional producers (Kafka 3.3+)
   - Idempotent writes
   - EOS integration tests

2. **Dynamic Reconfiguration**
   - Reload config without restart
   - Add/remove destinations at runtime

3. **Advanced Routing**
   - Content-based routing with complex rules
   - Priority queues for message ordering

4. **Enhanced Observability**
   - Distributed tracing with trace IDs
   - OpenTelemetry integration
   - Grafana dashboard templates

5. **Advanced Caching**
   - Additional cache backends (Memcached, DynamoDB)
   - TTL-based expiration
   - Cache warming strategies

---

## References

### Documentation
- [Implementation Notes](docs/IMPLEMENTATION_NOTES.md) - Technical implementation details
- [Performance Guide](docs/PERFORMANCE.md) - Performance tuning
- [Scaling Guide](docs/SCALING.md) - Scaling strategies

### Benchmarks
- [Concurrent Processing Results](benchmarks/results/CONCURRENT_PROCESSING_RESULTS.md)
- [Scaling Test Results](benchmarks/results/SCALING_TEST_RESULTS.md)
- [Comprehensive Benchmarks](benchmarks/results/BENCHMARKS.md)

### External
- [Apache Kafka Documentation](https://kafka.apache.org/documentation/)
- [rdkafka-rust](https://github.com/fede1024/rust-rdkafka)
- [Tokio](https://tokio.rs/)

---

**Last Updated:** April 2026  
**Version:** 1.0.0-alpha.1
