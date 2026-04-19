# StreamForge Benchmarks

**Version:** 1.0.0  
**Last Updated:** 2026-04-18

This document describes the benchmark infrastructure for StreamForge, including micro-benchmarks, end-to-end performance tests, and results analysis.

---

## Table of Contents

1. [Overview](#overview)
2. [Micro-Benchmarks (Criterion)](#micro-benchmarks-criterion)
3. [End-to-End Performance Tests](#end-to-end-performance-tests)
4. [Running Benchmarks](#running-benchmarks)
5. [Benchmark Configurations](#benchmark-configurations)
6. [Results and Analysis](#results-and-analysis)
7. [Performance Targets](#performance-targets)

---

## Overview

StreamForge uses two types of benchmarks:

### 1. Micro-Benchmarks (`benches/`)

**Purpose:** Measure individual component performance (filters, transforms, parsers)  
**Tool:** [Criterion.rs](https://github.com/bheisler/criterion.rs)  
**Location:** `benches/*.rs`  
**Run time:** Seconds  
**Output:** Statistical analysis (mean, median, std dev)

**Use when:**
- Optimizing a specific filter or transform
- Validating performance regression in DSL parser
- Comparing alternative implementations

### 2. End-to-End Performance Tests (`examples/benchmarks/`, `scripts/benchmarks/`)

**Purpose:** Measure full pipeline throughput, latency, and scaling  
**Tool:** Custom scripts + Kafka + Prometheus  
**Location:** `examples/benchmarks/*.yaml`, `scripts/benchmarks/*.sh`  
**Run time:** Minutes to hours  
**Output:** Throughput (msg/s), latency (p50/p95/p99), resource usage

**Use when:**
- Validating production performance
- Testing scaling behavior (threads, partitions)
- Measuring end-to-end latency
- Load testing with realistic data

---

## Micro-Benchmarks (Criterion)

### Available Benchmarks

Located in `benches/`:

| File | Benchmarks | What It Measures |
|------|-----------|------------------|
| `filter_benchmarks.rs` | Simple filters, boolean logic, regex, arrays, parser | Filter evaluation performance |
| `transform_benchmarks.rs` | JSON extraction, CONSTRUCT, arrays, arithmetic, parser | Transform performance |

### Running Micro-Benchmarks

**Run all benchmarks:**
```bash
cargo bench
```

**Run specific benchmark:**
```bash
cargo bench filter/simple_numeric_gt
cargo bench transform/extract_field
```

**Run with output:**
```bash
cargo bench -- --verbose
```

**Compare against baseline:**
```bash
# Save baseline
cargo bench -- --save-baseline main

# Make changes...

# Compare
cargo bench -- --baseline main
```

### Example Output

```
filter/simple_numeric_gt
                        time:   [145.23 ns 146.89 ns 148.72 ns]
filter/and_two_conditions
                        time:   [312.45 ns 315.67 ns 319.12 ns]
filter/regex_email
                        time:   [1.2341 µs 1.2456 µs 1.2598 µs]
```

### Interpreting Results

- **Simple filters:** ~150ns (JSON path + comparison)
- **Boolean AND/OR:** ~300-400ns (multiple filters)
- **Regex:** ~1-2µs (regex compilation cached)
- **Array filters:** ~500-800ns (depends on array size)
- **Parser:** ~200-500ns (depends on complexity)

**Throughput estimation:**
- Single thread, simple filter: ~6.6M operations/sec (1/150ns)
- Single thread, complex filter: ~2-3M operations/sec
- With JSON parsing overhead: ~100K-1M msg/sec per thread

---

## End-to-End Performance Tests

### Test Types

#### 1. Throughput Tests

**Goal:** Maximum messages per second  
**Config:** `examples/benchmarks/throughput-8thread.yaml`  
**Script:** `scripts/benchmarks/run_throughput_test.sh`

**Configuration:**
- 8 threads on 8 partitions
- Large batches (5000 messages)
- Manual commit (5 second interval)
- zstd compression
- Passthrough (no filters)

**Expected results:**
- **Target:** 30K+ msg/s sustained
- **Peak:** 35K+ msg/s

#### 2. Latency Tests

**Goal:** Minimum end-to-end latency  
**Config:** `examples/benchmarks/latency-optimized.yaml`  
**Script:** Custom timing measurement

**Configuration:**
- 2 threads (low contention)
- Small batches (100 messages)
- Per-message commit
- No compression
- Passthrough

**Expected results:**
- **p50:** < 5ms
- **p95:** < 10ms
- **p99:** < 20ms

#### 3. Filter/Transform Performance

**Goal:** DSL performance under load  
**Config:** `examples/benchmarks/filter-transform.yaml`  
**Script:** `scripts/benchmarks/run_throughput_test.sh`

**Configuration:**
- 4 threads
- Various filters (simple, complex, regex, array)
- CONSTRUCT transforms
- Time-based commit (1 second)

**Expected results:**
- **Simple filter:** ~50K msg/s
- **Complex filter:** ~20K msg/s
- **Regex filter:** ~10K msg/s
- **With CONSTRUCT:** ~15K msg/s

---

## Running Benchmarks

### Prerequisites

**1. Start Kafka:**
```bash
docker-compose -f docker-compose.benchmark.yml up -d
```

**2. Create topics:**
```bash
# 8 partition topics for throughput tests
kafka-topics --create --topic test-8p-input --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092

kafka-topics --create --topic test-8p-output --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092

# Single partition topics for latency tests
kafka-topics --create --topic test-input --partitions 1 --replication-factor 1 --bootstrap-server localhost:9092

kafka-topics --create --topic test-output --partitions 1 --replication-factor 1 --bootstrap-server localhost:9092
```

### Run Micro-Benchmarks

```bash
# All benchmarks
cargo bench

# Filter benchmarks only
cargo bench filter

# Transform benchmarks only
cargo bench transform

# Save baseline
cargo bench -- --save-baseline v1.0.0

# Generate HTML report
cargo bench -- --plotting-backend plotters
open target/criterion/report/index.html
```

### Run Throughput Tests

**Quick test (200K messages):**
```bash
cd scripts/benchmarks
./run_throughput_test.sh
```

**Custom test (1M messages at 50K msg/s target):**
```bash
./run_throughput_test.sh 1000000 50000
```

**With specific config:**
```bash
cd ../..
cargo build --release
./target/release/streamforge --config examples/benchmarks/throughput-8thread.yaml
```

### Run Observability Tests

**With Prometheus monitoring:**
```bash
cd scripts/benchmarks
./run_observability_test.sh
```

**Manual monitoring:**
```bash
# Terminal 1: Run StreamForge
cargo run --release -- --config examples/benchmarks/throughput-8thread.yaml

# Terminal 2: Watch metrics
watch -n 1 'curl -s localhost:8080/metrics | grep -E "(messages_consumed|messages_produced|consumer_lag)"'

# Terminal 3: Generate load
./scripts/benchmarks/generate_json_test_data.sh test-8p-input 100000
```

---

## Benchmark Configurations

All benchmark configs are in `examples/benchmarks/`:

| Config | Threads | Batching | Commit | Use Case |
|--------|---------|----------|--------|----------|
| `throughput-8thread.yaml` | 8 | Large (5000) | Manual (5s) | Max throughput |
| `latency-optimized.yaml` | 2 | Small (100) | Per-message | Min latency |
| `filter-transform.yaml` | 4 | Medium (2000) | Time-based (1s) | DSL performance |

### Customizing Configs

**For higher throughput:**
```yaml
threads: 16  # More parallelism
performance:
  batch_size: 10000  # Larger batches
  linger_ms: 100     # More batching
commit_interval_ms: 10000  # Less frequent commits
```

**For lower latency:**
```yaml
threads: 1  # No contention
performance:
  batch_size: 10  # Tiny batches
  linger_ms: 0    # Send immediately
commit_strategy: "per-message"  # Commit every message
```

**For testing filters:**
```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "filtered"
      filter: "YOUR_FILTER_HERE"
      transform: "YOUR_TRANSFORM_HERE"
```

---

## Results and Analysis

### Historical Results

Benchmark results and analysis are in `docs/benchmarks/results/`:

| Document | Content |
|----------|---------|
| `BENCHMARK_RESULTS.md` | Initial benchmark results |
| `BENCHMARKS.md` | Comprehensive benchmark analysis |
| `CONCURRENT_PROCESSING_RESULTS.md` | 132x improvement from concurrent processing |
| `SCALING_TEST_RESULTS.md` | Linear scaling validation (8 threads) |
| `DELIVERY_SEMANTICS_IMPLEMENTATION.md` | At-least-once vs at-most-once comparison |

### Key Historical Results

**Throughput improvements (0.x → 1.0):**
- **Sequential baseline:** 83 msg/s
- **Optimized sequential:** 3,000 msg/s (36x)
- **Concurrent (4 threads):** 10,933 msg/s (132x)
- **Concurrent (8 threads):** 25,000-30,000 msg/s sustained
- **Peak:** 34,517 msg/s

**Scaling:**
- 4 threads → 8 threads: **2.0x improvement** (perfect linear scaling)
- Validates architecture scales with CPU cores

**Delivery semantics:**
- **At-least-once (manual commit):** 10,933 msg/s
- **At-most-once (auto-commit):** 11,200 msg/s (<3% overhead)

### Analyzing Your Results

**1. Check throughput:**
```bash
# Messages consumed per second
curl -s localhost:8080/metrics | grep messages_consumed_total

# Calculate rate
# (current - previous) / time_elapsed
```

**2. Check latency:**
```bash
# Processing duration histogram
curl -s localhost:8080/metrics | grep processing_duration_seconds

# p95 latency
histogram_quantile(0.95, rate(streamforge_processing_duration_seconds_bucket[5m]))
```

**3. Check lag:**
```bash
curl -s localhost:8080/metrics | grep consumer_lag

# Or via Kafka
kafka-consumer-groups --bootstrap-server localhost:9092 \
  --describe --group benchmark-8thread
```

**4. Check errors:**
```bash
curl -s localhost:8080/metrics | grep errors_total

# Error rate
rate(streamforge_errors_total[5m])
```

---

## Performance Targets

### v1.0 Targets

| Metric | Target | Config |
|--------|--------|--------|
| **Throughput (passthrough)** | 30K msg/s | 8 threads, 8 partitions, no filters |
| **Throughput (simple filter)** | 20K msg/s | 4 threads, JSON path filter |
| **Throughput (complex filter)** | 10K msg/s | 4 threads, AND + CONSTRUCT |
| **Latency (p95)** | < 10ms | Per-message commit, no batching |
| **Latency (p99)** | < 20ms | Per-message commit |
| **Filter evaluation** | < 200ns | Simple JSON path comparison |
| **Transform evaluation** | < 500ns | Simple extraction |
| **Parser** | < 300ns | Simple filter parse |

### Scaling Targets

| CPUs | Threads | Expected Throughput |
|------|---------|---------------------|
| 2 | 2 | ~7.5K msg/s |
| 4 | 4 | ~15K msg/s |
| 8 | 8 | ~30K msg/s |
| 16 | 16 | ~60K msg/s |

**Assumptions:**
- Linear scaling with CPU cores
- 8+ partitions (no partition bottleneck)
- Simple filters or passthrough
- Adequate Kafka broker performance

---

## Continuous Benchmarking

### CI/CD Integration

**GitHub Actions example:**
```yaml
name: Benchmark

on:
  push:
    branches: [main]
  pull_request:

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Run micro-benchmarks
        run: cargo bench -- --save-baseline ${{ github.sha }}
      
      - name: Compare with main
        if: github.event_name == 'pull_request'
        run: |
          cargo bench -- --baseline main
          # Fail if > 10% regression
```

### Regression Detection

**Compare baselines:**
```bash
# Save current as baseline
cargo bench -- --save-baseline current

# Make changes...

# Compare
cargo bench -- --baseline current

# Look for regressions
# "Performance has regressed" = slower than baseline
# "Performance has improved" = faster than baseline
```

**Automatic regression check:**
```bash
#!/bin/bash
# benchmark-check.sh

cargo bench -- --baseline main > bench-results.txt

if grep -q "Performance has regressed" bench-results.txt; then
  echo "❌ Performance regression detected!"
  exit 1
else
  echo "✅ No performance regression"
  exit 0
fi
```

---

## Troubleshooting

### Low Throughput

**Check:**
1. CPU usage: `htop` or `top`
2. Consumer lag: `kafka-consumer-groups --describe`
3. Thread count: Match CPU cores
4. Batch sizes: Increase for throughput
5. Commit interval: Less frequent commits

**Fix:**
```yaml
threads: 8  # Match CPU cores
performance:
  batch_size: 5000
  linger_ms: 50
commit_interval_ms: 10000
```

### High Latency

**Check:**
1. Batch sizes: Too large
2. Linger time: Too long
3. Commit strategy: Per-message vs batched
4. Filter complexity: Regex is slow

**Fix:**
```yaml
threads: 2  # Reduce contention
performance:
  batch_size: 10
  linger_ms: 0
commit_strategy: "per-message"
```

### Inconsistent Results

**Causes:**
- Background processes (close Chrome, Slack, etc.)
- CPU throttling (run on AC power)
- Insufficient warm-up (criterion does auto-warmup)
- Network latency (use local Kafka)

**Fix:**
```bash
# Minimal system load
systemctl stop unnecessary-services

# Fixed CPU frequency
sudo cpupower frequency-set --governor performance

# Longer benchmark run
cargo bench -- --measurement-time 30
```

---

## Next Steps

- **Baseline:** Run `cargo bench` to establish v1.0 baseline
- **Monitor:** Run end-to-end tests weekly
- **Optimize:** Focus on regressions > 10%
- **Document:** Add results to `docs/benchmarks/results/`

---

**Version:** 1.0.0  
**Last Updated:** 2026-04-18  
**See Also:**
- [Performance Tuning Guide](docs/PERFORMANCE_TUNING_RESULTS.md)
- [Deployment Guide](docs/DEPLOYMENT.md#performance-tuning)
- [Operations Runbook](docs/OPERATIONS.md#performance-optimization)
