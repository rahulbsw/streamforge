# Performance Testing CI/CD Guide

## Overview

The project has a comprehensive GitHub Actions workflow for performance testing at `.github/workflows/performance-test.yml`.

## What It Tests

### 1. DSL Micro-benchmarks (Rhai Engine)
- Filter operations (simple_eq, numeric_gt, and/or/not logic)
- Transform operations (field extraction, object construction, string manipulation)
- Combined operations (filter + transform)
- **Duration**: ~10-15 minutes
- **No Kafka required**

### 2. End-to-End Throughput (3 Scenarios)
- **Scenario A**: Passthrough baseline (no filter/transform)
- **Scenario B**: Rhai filter + transform (realistic workload)
- **Scenario C**: Multi-destination + cache enrichment

**Metrics collected**:
- Throughput (msg/s)
- p99 latency
- Routing distribution
- Consumer lag

### 3. Latency Profile
- Sustained load test with 20K messages
- Detailed latency histogram (p50, p75, p90, p95, p99, p999)
- Measures Rhai evaluation time only

### 4. Partition Scaling Analysis
Tests scaling with different partition/thread configurations:

**View A - Matched (partitions = threads)**:
- 4P / 4T
- 8P / 8T
- 16P / 16T

**View B - Fixed threads (threads constant)**:
- 4P / 8T
- 8P / 8T
- 16P / 8T

Shows:
- Speedup vs baseline
- Scaling efficiency
- Optimal threads-to-partitions ratio

### 5. Consolidated Report
- Aggregates all results
- Downloads artifacts
- Generates final summary

---

## How to Run

### Method 1: GitHub UI (Easiest)

1. Navigate to your repository on GitHub
2. Click **Actions** tab
3. Select **"Performance Tests"** workflow from the left sidebar
4. Click **"Run workflow"** button (top right)
5. Configure parameters (or use defaults):
   - **messages**: Number of messages per scenario (default: 100,000)
   - **threads**: Worker threads (default: 8)
   - **partitions**: Topic partitions (default: 8)
6. Click **"Run workflow"**

### Method 2: GitHub CLI

```bash
# Install GitHub CLI if needed
brew install gh

# Authenticate
gh auth login

# Run with default parameters (100K msgs, 8 threads, 8 partitions)
gh workflow run performance-test.yml

# Run with custom parameters
gh workflow run performance-test.yml \
  -f messages=200000 \
  -f threads=16 \
  -f partitions=16

# Check workflow status
gh run list --workflow=performance-test.yml

# View logs of latest run
gh run view --log
```

### Method 3: Using the Helper Script

```bash
# Create and run helper script
./scripts/trigger-perf-test.sh  # (if you have it)

# Or use the one generated:
bash /tmp/run-perf-test.sh
```

---

## Expected Results (With New Optimizations)

After the `fetch_min_bytes=64KB` optimization:

### Scenario A - Passthrough
- **Before**: ~3,000-5,000 msg/s (with fetch_min_bytes=1)
- **After**: ~15,000-20,000 msg/s (with fetch_min_bytes=64KB)
- **Improvement**: 3-4x

### Scenario B - Rhai Filter + Transform
- **Before**: ~2,000-4,000 msg/s
- **After**: ~10,000-15,000 msg/s
- **Improvement**: 3-5x

### Scenario C - Multi-destination + Cache
- **Before**: ~1,500-3,000 msg/s
- **After**: ~8,000-12,000 msg/s
- **Improvement**: 3-4x

### Latency Profile
- **p99**: <500µs (Rhai evaluation only)
- **p50**: <100µs
- **Distribution**: Most messages under 100µs

### Partition Scaling
- **4P/4T → 8P/8T**: ~2x speedup
- **8P/8T → 16P/16T**: ~1.8x speedup
- **Efficiency**: 80-95% of linear scaling

---

## Interpreting Results

### GitHub Step Summary

Each job writes detailed results to the **GitHub Step Summary**:

1. **DSL Benchmarks**: Criterion output with per-operation timings
2. **Throughput Test**: Table comparing all 3 scenarios
3. **Latency Profile**: ASCII histogram of latency distribution
4. **Partition Scaling**: Two tables showing matched and fixed-thread views

### Artifacts

All runs save artifacts (30-day retention):
- `criterion-results/` - DSL benchmark data
- `throughput-results-<sha>/` - Scenario metrics and logs
- `latency-results-<sha>/` - Latency histogram data
- `partition-scaling-<sha>/` - Scaling test results

### Key Metrics to Watch

#### Throughput (msg/s)
- **Good**: >10,000 msg/s for Scenario B
- **Excellent**: >15,000 msg/s for Scenario B
- **Outstanding**: >20,000 msg/s for Scenario B

#### p99 Latency
- **Good**: <1ms (1,000µs)
- **Excellent**: <500µs
- **Outstanding**: <200µs

#### Scaling Efficiency
- **Good**: 70-80% of linear
- **Excellent**: 80-90% of linear
- **Outstanding**: >90% of linear

---

## Comparing Before/After

To compare performance before and after the optimization:

### 1. Create a baseline (BEFORE optimization)

```bash
# Checkout before optimization
git checkout <commit-before-optimization>

# Rebuild
cargo build --release

# Run performance test
gh workflow run performance-test.yml \
  -f messages=100000 \
  -f threads=8 \
  -f partitions=8
```

### 2. Test with optimization (AFTER)

```bash
# Checkout after optimization
git checkout main

# Rebuild
cargo build --release

# Run performance test again
gh workflow run performance-test.yml \
  -f messages=100000 \
  -f threads=8 \
  -f partitions=8
```

### 3. Compare Results

Download both artifacts and compare:
```bash
# Download artifacts for both runs
gh run download <run-id-before> -d results-before/
gh run download <run-id-after> -d results-after/

# Compare throughput
diff results-before/throughput-*/results.json results-after/throughput-*/results.json
```

---

## CI Integration

### Run on Pull Requests

To automatically run performance tests on PRs, add to `.github/workflows/pr-checks.yml`:

```yaml
performance-gate:
  name: Performance Regression Check
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Run quick performance test
      uses: ./.github/workflows/performance-test.yml
      with:
        messages: 50000
        threads: 4
        partitions: 4
    
    - name: Check for regression
      run: |
        # Compare with baseline from main branch
        # Fail if throughput drops >10%
        ./scripts/check-performance-regression.sh
```

### Scheduled Runs

Add to the workflow triggers:

```yaml
on:
  workflow_dispatch:  # Keep manual trigger
  schedule:
    - cron: '0 2 * * 1'  # Every Monday at 2 AM
```

---

## Troubleshooting

### Workflow fails at "Wait for Kafka to be ready"

**Cause**: Kafka takes longer than 60s to start on GitHub runners

**Solution**: The workflow already has retry logic. If it still fails, increase timeout:

```yaml
# In .github/workflows/performance-test.yml
for i in $(seq 1 30); do  # Increase to 60
  ...
done
```

### Inconsistent results

**Cause**: GitHub-hosted runners have shared resources

**Solutions**:
1. Run multiple times and average
2. Use self-hosted runners with dedicated resources
3. Compare trends across multiple runs

### Out of memory errors

**Cause**: Too many messages for runner memory (7GB)

**Solution**: Reduce message count:
```bash
gh workflow run performance-test.yml \
  -f messages=50000  # Instead of 100000
```

---

## Best Practices

### 1. Establish Baselines
Run tests on main branch regularly to establish performance baselines.

### 2. Test Before Major Changes
Always run performance tests before:
- Merging large refactors
- Upgrading dependencies (rdkafka, tokio, etc.)
- Changing core algorithms

### 3. Monitor Trends
Track metrics over time:
- Throughput trends
- Latency trends
- Scaling efficiency

### 4. Document Regressions
If performance drops:
1. Identify the commit that caused it
2. Document the cause
3. Decide if the trade-off is acceptable

### 5. Celebrate Wins
When performance improves (like our 3-5x gain from fetch tuning):
1. Document the change
2. Share results with team
3. Update performance documentation

---

## Current Performance Baseline (Post-Optimization)

**Date**: 2026-04-16  
**Commit**: (after fetch_min_bytes=64KB optimization)

### Local Test Results (Apple Silicon M1/M2)
- **Throughput**: ~11,020 msg/s
- **Batch Duration**: 90.74ms average
- **p99 Latency**: <100ms (88% of batches)
- **Error Rate**: 0%

### Expected CI Results (GitHub Runners)
- **Throughput**: ~8,000-12,000 msg/s (lower due to shared resources)
- **Latency**: Higher variance than local
- **Scaling**: Similar efficiency ratios

---

## Related Documentation

- [PERFORMANCE_TUNING_RESULTS.md](../PERFORMANCE_TUNING_RESULTS.md) - Detailed fetch tuning analysis
- [ARCHITECTURE.md](../ARCHITECTURE.md) - System architecture
- [OBSERVABILITY_METRICS_DESIGN.md](../docs/OBSERVABILITY_METRICS_DESIGN.md) - Metrics design

---

## Questions?

If you need help with performance testing:
1. Check workflow logs in GitHub Actions
2. Review artifacts for detailed metrics
3. Compare with baseline results
4. Check Kafka/Rust configuration
