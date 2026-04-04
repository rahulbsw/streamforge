# Performance Testing Guide

## Overview

Streamforge includes multiple levels of performance testing:
1. **Microbenchmarks** (Criterion) - Fast, run in CI
2. **End-to-End Throughput Tests** - Manual, requires Kafka
3. **CI Performance Tests** - Manual trigger only

## 1. Microbenchmarks (Criterion)

### What They Test

- Filter evaluation performance
- JSON Path extraction speed
- Transform operation overhead
- Header manipulation cost

### Running Locally

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench filter_benchmarks
cargo bench --bench transform_benchmarks

# With verbose output
cargo bench -- --verbose
```

### Results Location

```
target/criterion/
├── filter_benchmarks/
│   └── report/index.html
└── transform_benchmarks/
    └── report/index.html
```

Open `target/criterion/report/index.html` in browser for detailed charts.

### CI Integration

**Status**: ✅ Builds benchmarks, ❌ Doesn't run them

```yaml
# .github/workflows/ci.yml
rust-benchmarks:
  name: Rust - Benchmarks
  runs-on: ubuntu-latest
  steps:
    - name: Build benchmarks
      run: cargo bench --no-run
```

**Why not run?**
- Takes 5-10 minutes
- Results would be noisy (shared CI runners)
- Not critical for PR validation

## 2. End-to-End Throughput Tests

### What They Test

- Real Kafka integration
- Message consumption rate
- Transformation throughput
- Producer performance
- Consumer lag behavior
- Observability overhead

### Running Locally

#### Quick Test (Automated)

```bash
cd benchmarks

# Run with 100K messages, 30K msg/s target
./run_throughput_test.sh 100000 30000

# Run with 500K messages, 50K msg/s target
./run_throughput_test.sh 500000 50000
```

#### Manual Test (Full Control)

```bash
# 1. Start Kafka
docker-compose -f docker-compose.benchmark.yml up -d

# 2. Create topics
kafka-topics --create --topic test-input --partitions 16 --replication-factor 1 --bootstrap-server localhost:9092
kafka-topics --create --topic test-output --partitions 16 --replication-factor 1 --bootstrap-server localhost:9092

# 3. Generate test data
cd benchmarks
./generate_json_test_data.sh 200000 test_data.jsonl

# 4. Configure Streamforge
cat > config.json << EOF
{
  "appid": "perf-test",
  "bootstrap": "localhost:9092",
  "input": "test-input",
  "output": "test-output",
  "threads": 16,
  "observability": {
    "metrics_enabled": true,
    "metrics_port": 9090,
    "lag_monitoring_enabled": true,
    "lag_monitoring_interval_secs": 10
  }
}
EOF

# 5. Start Streamforge
./target/release/streamforge

# 6. In another terminal, send messages
cat benchmarks/test_data.jsonl | kafka-console-producer \
  --bootstrap-server localhost:9092 \
  --topic test-input \
  --batch-size 2000

# 7. Monitor metrics
watch -n 2 'curl -s http://localhost:9090/metrics | grep consumed_total'
```

### Expected Performance

| Environment | Partitions | Threads | Throughput | Notes |
|-------------|------------|---------|------------|-------|
| macOS (laptop) | 8 | 8 | 6,700 msg/s | Validated |
| macOS (laptop) | 16 | 16 | 11,890 msg/s | Validated |
| Linux (server) | 16 | 16 | ~15,000 msg/s | Estimated |
| Linux (dedicated) | 32 | 32 | ~30,000 msg/s | Estimated |
| Production (clustered) | 64+ | 32+ | 50,000+ msg/s | With Kafka cluster |

### Results Location

```
benchmarks/results/throughput_test_<timestamp>/
├── REPORT.md              # Performance summary
├── streamforge.log        # Application logs
├── metrics_before.txt     # Prometheus metrics before
├── metrics_after.txt      # Prometheus metrics after
└── producer_output.txt    # Kafka producer output
```

## 3. CI Performance Tests (GitHub Actions)

### Manual Workflow Trigger

Performance tests are **NOT** run automatically on every commit. They must be triggered manually:

```bash
# Via GitHub UI:
# 1. Go to Actions tab
# 2. Select "Performance Tests" workflow
# 3. Click "Run workflow"
# 4. Configure parameters:
#    - Messages: 100000
#    - Partitions: 8
#    - Threads: 8

# Via GitHub CLI:
gh workflow run performance-test.yml \
  -f messages=100000 \
  -f partitions=16 \
  -f threads=16
```

### What It Tests

1. **Criterion Benchmarks** - Microbenchmarks
2. **Throughput Test** - End-to-end with Kafka
3. **Latency Test** - Latency distribution

### Limitations

**CI Environment:**
- Ubuntu-latest runners
- 2 CPU cores, 7GB RAM
- Shared VM (variable load)
- No dedicated resources

**Expected Results:**
- Lower than production
- High variance between runs
- **For comparison only**, not absolute benchmarks

**Why Not Run on Every PR?**
- Takes 15-30 minutes
- Results are not reliable for comparison
- CI runners not suitable for performance testing
- Would significantly slow down CI feedback loop

### When to Run

✅ **Run manually when:**
- Major performance optimization completed
- Investigating performance regression
- Before release (validation)
- After infrastructure changes

❌ **Don't run for:**
- Every commit
- Minor bug fixes
- Documentation changes
- Regular PRs

## 4. Continuous Performance Monitoring (Recommended)

For production-grade performance tracking, use a dedicated performance testing environment:

### Option A: Dedicated Perf Environment

```yaml
# Separate performance testing server
- Dedicated hardware (not shared)
- Linux (Ubuntu 22.04 LTS)
- 16+ CPU cores
- 32GB+ RAM
- SSD storage
- Kafka cluster (3+ brokers)
```

**Schedule:**
- Nightly performance tests
- Compare against baseline
- Alert on regressions > 10%

### Option B: Nightly GitHub Actions

```yaml
# .github/workflows/nightly-perf.yml
on:
  schedule:
    - cron: '0 2 * * *'  # 2 AM UTC daily
  workflow_dispatch:

jobs:
  performance-baseline:
    runs-on: ubuntu-latest
    # ... run throughput tests
    # ... compare with previous results
    # ... alert if regression detected
```

### Option C: External Service

Use services like:
- **Bencher.dev** - Continuous benchmarking
- **Conbench** - Benchmark tracking
- **Custom Grafana** - Historical tracking

## 5. Performance Regression Detection

### Tracking Baseline

```bash
# Run baseline test
./benchmarks/run_throughput_test.sh 100000 30000

# Save results
cp benchmarks/results/throughput_test_*/REPORT.md \
   benchmarks/baselines/baseline_$(date +%Y%m%d).md

# Compare with baseline
diff benchmarks/baselines/baseline_20260401.md \
     benchmarks/results/throughput_test_latest/REPORT.md
```

### Regression Criteria

**Alert if:**
- Throughput drops > 10%
- Latency P99 increases > 20%
- Error rate > 0.1%
- Consumer lag > 1000 messages

### Git Bisect for Regressions

```bash
# Find commit that caused regression
git bisect start
git bisect bad HEAD
git bisect good v1.0.0

# For each commit:
cargo build --release
./benchmarks/run_throughput_test.sh 100000 30000
# Mark good/bad based on results

git bisect reset
```

## 6. Profiling and Optimization

### CPU Profiling

```bash
# Install flamegraph
cargo install flamegraph

# Profile Streamforge
cargo flamegraph --bin streamforge

# Open flamegraph.svg in browser
```

### Memory Profiling

```bash
# Install valgrind
sudo apt-get install valgrind

# Profile memory
valgrind --tool=massif ./target/release/streamforge

# Analyze
ms_print massif.out.*
```

### Async Profiling

```bash
# Install tokio-console
cargo install tokio-console

# Run with tokio-console enabled
RUSTFLAGS="--cfg tokio_unstable" cargo build --release
./target/release/streamforge

# In another terminal
tokio-console
```

## 7. Best Practices

### DO:
✅ Run benchmarks before and after optimization  
✅ Use consistent test environment  
✅ Warm up before measuring  
✅ Run multiple iterations  
✅ Document test conditions  
✅ Compare against baseline  
✅ Profile before optimizing  

### DON'T:
❌ Run performance tests on laptop during other work  
❌ Compare results from different machines  
❌ Optimize without measuring first  
❌ Trust single-run results  
❌ Run performance tests in CI for every commit  
❌ Mix load testing with other benchmarks  

## 8. Troubleshooting

### Low Throughput

**Symptoms**: Throughput < 5,000 msg/s

**Check:**
1. Partition count matches thread count
2. Kafka is on dedicated machine (not localhost)
3. Producer tool (use kafka-producer-perf-test, not console-producer)
4. Consumer fetch settings (increase fetch.min.bytes)
5. CPU usage (should be 70-90%)
6. Network bandwidth

### High Latency

**Symptoms**: P99 > 100ms

**Check:**
1. System load (other processes)
2. Garbage collection (though Rust doesn't have GC)
3. Disk I/O (check with iostat)
4. Network latency (ping Kafka broker)
5. Transform complexity
6. Logging level (debug logs add overhead)

### Consumer Lag

**Symptoms**: Lag keeps increasing

**Check:**
1. Throughput < production rate
2. Error rate (errors slow processing)
3. Partition rebalancing
4. Kafka broker health
5. Thread count (increase if CPU available)

## 9. Performance Testing Checklist

Before running performance tests:

- [ ] Clean Kafka topics (no old data)
- [ ] Restart Kafka (clean state)
- [ ] Build release mode (`cargo build --release`)
- [ ] Close other applications
- [ ] Consistent network conditions
- [ ] Document test configuration
- [ ] Monitor system resources during test
- [ ] Save results with timestamp
- [ ] Compare with baseline
- [ ] Document any anomalies

## 10. Interpreting Results

### Good Results

✅ Throughput within 10% of target  
✅ P99 latency < 25ms  
✅ Zero errors  
✅ Consumer lag returns to 0  
✅ CPU usage 70-90%  
✅ Memory stable  

### Investigate If

⚠️ Throughput < 80% of expected  
⚠️ P99 latency > 50ms  
⚠️ Error rate > 0%  
⚠️ Consumer lag keeps growing  
⚠️ CPU usage < 50% or > 95%  
⚠️ Memory keeps growing  

## Summary

| Test Type | When to Run | Where | Purpose |
|-----------|-------------|-------|---------|
| **Microbenchmarks** | During development | Local | Validate optimization |
| **Throughput Tests** | Before PR / Release | Local | Validate end-to-end performance |
| **CI Perf Tests** | Manual trigger | GitHub Actions | Regression detection |
| **Nightly Tests** | Scheduled | Dedicated server | Continuous monitoring |
| **Load Tests** | Pre-production | Staging environment | Production validation |

---

**For more details:**
- [Throughput Testing Guide](../benchmarks/THROUGHPUT_TESTING.md)
- [Observability Test Guide](../benchmarks/OBSERVABILITY_TEST_GUIDE.md)
- [Benchmark Results](../benchmarks/results/BENCHMARKS.md)
