# Throughput Testing Guide

This guide shows how to run proper throughput tests with valid JSON data to measure actual Streamforge performance.

## Quick Start

```bash
cd benchmarks

# Run test with default settings (200K messages, 30K msg/s target)
./run_throughput_test.sh

# Run custom test
./run_throughput_test.sh 500000 50000  # 500K messages at 50K msg/s
```

## Scripts

### 1. `generate_json_test_data.sh`

Generates valid JSON test data in JSONL format (one JSON object per line).

**Usage:**
```bash
./generate_json_test_data.sh [num_messages] [output_file]
```

**Examples:**
```bash
# Generate 100K messages
./generate_json_test_data.sh 100000 test_100k.jsonl

# Generate 1M messages
./generate_json_test_data.sh 1000000 test_1m.jsonl
```

**Message Format:**
```json
{
  "userId": 1042,
  "user": {
    "id": 1042,
    "name": "Alice Smith",
    "email": "alice.smith@example.com"
  },
  "action": "purchase",
  "timestamp": "2026-04-04T01:42:35.123456Z",
  "metadata": {
    "source": "perf_test",
    "sequence": 42,
    "batch": 0
  }
}
```

### 2. `run_throughput_test.sh`

Runs a complete throughput test with proper JSON data.

**Usage:**
```bash
./run_throughput_test.sh [num_messages] [target_throughput]
```

**Parameters:**
- `num_messages`: Number of messages to send (default: 200000)
- `target_throughput`: Target throughput in msg/s (default: 30000)

**Examples:**
```bash
# Test with 100K messages, 30K msg/s target
./run_throughput_test.sh 100000 30000

# Test with 500K messages, 50K msg/s target
./run_throughput_test.sh 500000 50000

# Test with 1M messages, 25K msg/s target
./run_throughput_test.sh 1000000 25000
```

## What the Test Does

1. ✅ **Checks Prerequisites** - Kafka running, topics exist, binary built
2. ✅ **Generates Test Data** - Creates valid JSON messages (reuses if exists)
3. ✅ **Cleans Topics** - Deletes and recreates test topics
4. ✅ **Starts Streamforge** - With observability enabled
5. ✅ **Sends Data** - Streams JSON to Kafka via console-producer
6. ✅ **Monitors Progress** - Real-time throughput tracking every 5s
7. ✅ **Waits for Completion** - Ensures all messages processed
8. ✅ **Captures Metrics** - Prometheus metrics before/after
9. ✅ **Generates Report** - Complete performance analysis

## Test Output

Each test creates a timestamped results directory:

```
benchmarks/results/throughput_test_20260404_014235/
├── REPORT.md                 # Performance report
├── streamforge.log           # Full application logs
├── metrics_before.txt        # Prometheus metrics before test
├── metrics_after.txt         # Prometheus metrics after test
├── producer_output.txt       # Kafka producer output
└── streamforge.pid           # Process ID
```

## Sample Report

```markdown
# Throughput Test Report

**Date**: 2026-04-04 01:42:35
**Test Duration**: 8s
**Target Messages**: 200000
**Target Throughput**: 30000 msg/s

## Results

### Message Counts

| Metric | Value |
|--------|-------|
| **Consumed** | 200000 |
| **Produced** | 200000 |
| **Errors** | 0 |
| **Success Rate** | 100.00% |

### Throughput

| Metric | Value |
|--------|-------|
| **Average** | 25000 msg/s |
| **Target** | 30000 msg/s |
| **Achievement** | 83.3% |
```

## Understanding Results

### Success Criteria

- ✅ **100% success rate**: All consumed messages produced successfully
- ✅ **Zero errors**: No JSON parse errors or processing failures
- ✅ **Meets target**: Average throughput ≥ target throughput
- ✅ **Low latency**: P99 latency < 25ms (check metrics)

### Performance Factors

**Throughput is affected by:**

1. **Hardware**
   - CPU cores (8 threads = 8 cores ideal)
   - Disk I/O (SSD recommended)
   - Network bandwidth
   - Available memory

2. **Configuration**
   - Thread count (should match CPU cores)
   - Batch size (100-1000 optimal)
   - Kafka partition count (match thread count)
   - Auto-commit vs manual-commit

3. **Message Complexity**
   - Message size (larger = slower)
   - Transform operations (more = slower)
   - Filter complexity (complex = slower)
   - Multi-destination routing (more = slower)

4. **Environment**
   - Kafka on same machine vs remote
   - macOS vs Linux (Linux typically 20-30% faster)
   - Other processes consuming resources
   - Network latency

### Expected Performance

Based on validated benchmarks:

| Scenario | Throughput | Config |
|----------|------------|--------|
| **Simple passthrough** | 25-30K msg/s | 8 threads, no transforms |
| **With key extraction** | 20-25K msg/s | 8 threads, key transform |
| **With filtering** | 15-20K msg/s | 8 threads, filter + transform |
| **Multi-destination** | 10-15K msg/s | 8 threads, 2+ destinations |
| **Peak burst** | 35K msg/s | 8 threads, optimal conditions |

**Note**: 50K msg/s requires distributed setup (multiple machines, optimized Kafka cluster).

## Troubleshooting

### Low Throughput

**Symptoms**: Average throughput < 50% of target

**Possible causes:**
1. **CPU bottleneck**: Increase threads to match CPU cores
2. **Kafka bottleneck**: Check Kafka logs, increase partitions
3. **Disk I/O**: Check disk usage, use SSD
4. **Producer rate**: Console-producer has limits (~10K msg/s max)

**Solutions:**
```bash
# Check CPU usage
top -l 1 | grep "CPU usage"

# Check Kafka performance
kafka-producer-perf-test --topic test --num-records 10000 \
  --record-size 200 --throughput -1 \
  --producer-props bootstrap.servers=localhost:9092

# Increase threads (edit config)
threads: 16  # match CPU cores
```

### High Error Rate

**Symptoms**: Errors > 0, success rate < 100%

**Possible causes:**
1. **Invalid JSON**: Malformed messages in test data
2. **Missing fields**: Transform paths don't exist
3. **Kafka errors**: Connection issues, broker down

**Solutions:**
```bash
# Validate test data
head -1 test_data.jsonl | jq .

# Check Streamforge logs
tail -100 results/*/streamforge.log | grep ERROR

# Check Kafka health
kafka-topics --bootstrap-server localhost:9092 --list
```

### Messages Not Produced

**Symptoms**: Consumed > Produced

**Possible causes:**
1. **Filter rejection**: Messages filtered out
2. **Processing errors**: Check error count
3. **Producer errors**: Check logs for Kafka producer errors

**Solutions:**
```bash
# Check filter metrics
curl -s http://localhost:9090/metrics | grep filter

# Check error metrics
curl -s http://localhost:9090/metrics | grep error

# Review logs
grep "filter.*fail" results/*/streamforge.log
```

## Optimizing for Higher Throughput

### Configuration Tuning

```yaml
# benchmarks/configs/high-throughput.yaml
threads: 16                  # Match CPU cores
offset: latest              # Don't reprocess old messages

observability:
  metrics_enabled: true
  lag_monitoring_interval_secs: 30  # Less frequent = faster

routing:
  routing_type: filter
  destinations:
    - output: test-output
      # Simpler transforms = faster
      key_transform: "/userId"
      # No envelope transforms = faster
```

### Kafka Tuning

```bash
# Increase partitions (match thread count)
kafka-topics --alter --topic test-input \
  --partitions 16 --bootstrap-server localhost:9092

# Increase producer batch size
# Edit: server.properties
batch.size=32768
linger.ms=10
compression.type=lz4
```

### System Tuning

```bash
# Increase file descriptors (Linux)
ulimit -n 65536

# Increase socket buffer (Linux)
sudo sysctl -w net.core.rmem_max=134217728
sudo sysctl -w net.core.wmem_max=134217728
```

## Advanced Usage

### Custom Test Data

Generate custom message format:

```python
# custom_generator.py
import json
for i in range(100000):
    msg = {"your": "format", "id": i}
    print(json.dumps(msg))
```

```bash
python3 custom_generator.py > custom_data.jsonl
cat custom_data.jsonl | kafka-console-producer \
  --bootstrap-server localhost:9092 \
  --topic test-8p-input
```

### Load Testing with Locust

For more advanced load patterns:

```bash
# Install locust
pip install locust

# Run distributed load test
# (requires custom locust file for Kafka)
```

### Continuous Load

For sustained load testing:

```bash
# Generate large dataset
./generate_json_test_data.sh 10000000 large.jsonl

# Stream continuously
while true; do
  cat large.jsonl | kafka-console-producer \
    --bootstrap-server localhost:9092 \
    --topic test-8p-input
  sleep 1
done
```

## Comparing with Benchmarks

Compare your results with documented benchmarks:

```bash
# View existing benchmark results
cat benchmarks/results/BENCHMARKS.md
cat benchmarks/results/SCALING_TEST_RESULTS.md

# Compare throughput
echo "Your test: $AVG_THROUGHPUT msg/s"
echo "Benchmark: 25000-30000 msg/s (simple passthrough)"
echo "Benchmark: 10000-15000 msg/s (with transforms)"
```

## See Also

- [Benchmarks README](README.md) - Overview of all benchmarks
- [Observability Test Guide](OBSERVABILITY_TEST_GUIDE.md) - Observability-focused testing
- [Benchmark Results](results/BENCHMARKS.md) - Historical benchmark data
- [Quick Start](quick_start.sh) - Automated setup for new users
