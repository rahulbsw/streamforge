# Benchmark Configurations

This directory contains StreamForge configurations optimized for different benchmarking scenarios.

## Quick Start

**1. Start Kafka:**
```bash
docker-compose -f ../../docker-compose.benchmark.yml up -d
```

**2. Create topics:**
```bash
# 8 partition topics (for throughput tests)
kafka-topics --create --topic test-8p-input --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092
kafka-topics --create --topic test-8p-output --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092

# Single partition topics (for latency tests)
kafka-topics --create --topic test-input --partitions 1 --replication-factor 1 --bootstrap-server localhost:9092
kafka-topics --create --topic test-output --partitions 1 --replication-factor 1 --bootstrap-server localhost:9092
```

**3. Run benchmark:**
```bash
cargo build --release
./target/release/streamforge --config examples/benchmarks/throughput-8thread.yaml
```

---

## Configurations

### throughput-8thread.yaml

**Purpose:** Maximum throughput validation  
**Target:** 30K+ msg/s sustained

**Configuration:**
- 8 threads on 8 partitions
- Large batches (5000 messages)
- Manual commit (5 second interval)
- zstd compression
- Passthrough routing (no filters)

**Use when:**
- Validating scaling performance
- Stress testing the system
- Measuring maximum capacity

**Expected results:**
- Throughput: 30-35K msg/s
- CPU: 100% (saturated)
- Latency: 50-100ms (high due to batching)

---

### latency-optimized.yaml

**Purpose:** Minimum latency validation  
**Target:** < 10ms p95 latency

**Configuration:**
- 2 threads (low contention)
- Small batches (100 messages)
- Per-message commit
- No compression
- Passthrough routing

**Use when:**
- Validating low-latency scenarios
- Testing real-time data pipelines
- SLA validation

**Expected results:**
- Latency p50: < 5ms
- Latency p95: < 10ms
- Latency p99: < 20ms
- Throughput: 5-10K msg/s (latency trade-off)

---

### filter-transform.yaml

**Purpose:** DSL performance validation  
**Target:** 10-20K msg/s with filtering

**Configuration:**
- 4 threads
- Moderate batches (2000 messages)
- Time-based commit (1 second)
- Multiple destinations with filters:
  - Simple JSON path filter
  - Complex AND filter with 3 conditions
  - Regex filter
  - Array filter
- CONSTRUCT transforms

**Use when:**
- Validating DSL performance
- Testing filter combinations
- Benchmarking transforms

**Expected results:**
- Simple filter: ~50K msg/s per thread
- Complex filter: ~20K msg/s per thread
- Regex filter: ~10K msg/s per thread
- With CONSTRUCT: ~15K msg/s per thread

---

## Legacy Configs (Pre-v1.0)

The following configs use the old format and are kept for reference:

- `at-least-once-config.yaml`
- `test-8thread-config.yaml`
- `test-8thread-fast-config.yaml`
- `test-critical-fixes-config.yaml`
- `test-simplify-config.yaml`
- `test-values.yaml`

**Note:** These configs use the pre-v1.0 format with `consumer_properties:` and `producer_properties:` blocks. They may not work with v1.0 without conversion.

To convert to v1.0 format:
1. Replace `consumer_properties:` and `producer_properties:` with `performance:` block
2. Add `retry:` and `dlq:` blocks
3. Update `routing:` to use `routing_type:` and `destinations:`
4. Change `commit_strategy:` from object to string

See the v1.0 configs above for examples.

---

## Customizing Configs

### For Higher Throughput

```yaml
threads: 16  # More parallelism
performance:
  batch_size: 10000  # Larger batches
  linger_ms: 100     # More batching
  compression: "zstd"
commit_interval_ms: 10000  # Less frequent commits
```

### For Lower Latency

```yaml
threads: 1  # No contention
performance:
  batch_size: 10  # Tiny batches
  linger_ms: 0    # Send immediately
  compression: "none"
commit_strategy: "per-message"
```

### For Testing Specific Filters

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "test-output"
      filter: "/your/json/path,==,value"
      transform: "CONSTRUCT:field1=/path1:field2=/path2"
```

---

## Monitoring

### Metrics Endpoint

```bash
# View all metrics
curl http://localhost:8080/metrics

# Throughput
curl -s http://localhost:8080/metrics | grep messages_consumed_total
curl -s http://localhost:8080/metrics | grep messages_produced_total

# Lag
curl -s http://localhost:8080/metrics | grep consumer_lag

# Errors
curl -s http://localhost:8080/metrics | grep errors_total
```

### Consumer Group

```bash
kafka-consumer-groups --bootstrap-server localhost:9092 \
  --describe --group benchmark-8thread
```

---

## Scripts

Automated benchmark scripts are in `../../scripts/benchmarks/`:

| Script | Purpose |
|--------|---------|
| `run_throughput_test.sh` | Automated throughput testing |
| `run_observability_test.sh` | Performance test with Prometheus monitoring |
| `generate_json_test_data.sh` | Generate test data |
| `quick_start.sh` | Quick start guide |

---

## See Also

- [BENCHMARKS.md](../../BENCHMARKS.md) - Complete benchmarking guide
- [DEPLOYMENT.md](../../docs/DEPLOYMENT.md#performance-tuning) - Performance tuning in production
- [OPERATIONS.md](../../docs/OPERATIONS.md#performance-optimization) - Operational performance optimization
- [Benchmark Results](../../docs/benchmarks/results/) - Historical benchmark data

---

**Version:** 1.0.0  
**Last Updated:** 2026-04-18
