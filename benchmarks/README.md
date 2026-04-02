# Benchmarks

This directory contains benchmark results, test configurations, and performance analysis for Streamforge.

## Directory Structure

### `configs/`
Test and benchmark configuration files:
- `at-least-once-config.yaml` - At-least-once delivery semantics test config
- `test-8thread-config.yaml` - 8 thread scaling test configuration
- `test-8thread-fast-config.yaml` - Optimized 8 thread configuration
- `test-critical-fixes-config.yaml` - Configuration for testing critical fixes
- `test-simplify-config.yaml` - Simplified test configuration
- `test-values.yaml` - Test values configuration

### `results/`
Benchmark results and performance analysis:
- `BENCHMARK_RESULTS.md` - Initial benchmark results
- `BENCHMARKS.md` - Comprehensive benchmark analysis
- `CONCURRENT_PROCESSING_RESULTS.md` - Concurrent processing performance results (132x improvement)
- `SCALING_TEST_RESULTS.md` - Linear scaling validation (8 threads, 8 partitions)
- `DELIVERY_SEMANTICS_IMPLEMENTATION.md` - At-least-once vs at-most-once performance comparison

## Key Results

### Throughput Improvements
- **Sequential:** 83 msg/s (baseline)
- **Optimized sequential:** 3,000 msg/s (36x improvement)
- **Concurrent (4 threads):** 10,933 msg/s (132x improvement)
- **Concurrent (8 threads):** 25,000-30,000 msg/s sustained, 34,517 msg/s peak

### Linear Scaling
- 4 threads → 8 threads: **2.0x improvement** (perfect linear scaling)
- Validates architecture scales with CPU cores

### Delivery Semantics
- **At-least-once (manual commit):** 10,933 msg/s with full data integrity
- **At-most-once (auto-commit):** 11,200 msg/s with <3% overhead

## Running Benchmarks

### Prerequisites
```bash
# Start Kafka (Docker Compose)
docker-compose up -d

# Create test topics
kafka-topics --create --topic test-8p-input --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092
kafka-topics --create --topic test-8p-output --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092
```

### Run with Config
```bash
# Build optimized binary
cargo build --release

# Run with specific config
./target/release/streamforge --config benchmarks/configs/test-8thread-config.yaml
```

### Generate Test Data
```bash
# Use included test data generator
cd scripts
./generate-test-data.sh test-8p-input 100000
```

## Benchmark Methodology

All benchmarks use:
- **8 partition topics** for parallel processing validation
- **Kafka in Docker** with standard configuration
- **Release builds** (`--release` flag)
- **10-30 second sustained runs** for throughput measurement
- **Manual commit mode** for at-least-once guarantees (unless testing auto-commit)

## See Also

- `/benches/` - Criterion micro-benchmarks for filters and transforms
- `/docs/PERFORMANCE.md` - Performance tuning guide
- `/docs/SCALING.md` - Scaling architecture documentation
