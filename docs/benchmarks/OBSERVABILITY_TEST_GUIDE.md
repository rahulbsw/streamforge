# Observability Performance Test Guide

Complete guide for running performance tests with Prometheus metrics monitoring.

## Quick Start (5 minutes)

```bash
# 1. Start Kafka
docker-compose -f docker-compose.benchmark.yml up -d

# 2. Create topics (8 partitions for parallel processing)
kafka-topics --create --topic test-8p-input --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092 || echo "Topic exists"
kafka-topics --create --topic test-8p-output --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092 || echo "Topic exists"

# 3. Build Streamforge
cargo build --release

# 4. Run observability test
cd benchmarks
./run_observability_test.sh
```

## Test Overview

The observability test:
1. ✅ Starts Streamforge with metrics enabled
2. ✅ Monitors Prometheus metrics in real-time
3. ✅ Collects time-series data every 2 seconds
4. ✅ Displays live dashboard in terminal
5. ✅ Generates performance report

## Prerequisites

### 1. Docker & Docker Compose
```bash
# Check if installed
docker --version
docker-compose --version

# If not installed:
# macOS: brew install docker docker-compose
# Linux: apt-get install docker.io docker-compose
```

### 2. Kafka Command-Line Tools
```bash
# Check if available
which kafka-topics

# If not found, download Kafka:
# https://kafka.apache.org/downloads
# Add bin/ directory to PATH
```

### 3. Build Streamforge
```bash
cargo build --release
```

## Running the Test

### Option 1: Interactive Mode (Recommended)

```bash
cd benchmarks
./run_observability_test.sh
```

When prompted, choose **Option 1** for live metrics dashboard:

```
═══════════════════════════════════════════════════════════════
  Streamforge Observability Test - Live Metrics
═══════════════════════════════════════════════════════════════

📥 MESSAGE PROCESSING
   Consumed:      125000 messages (2083.33 msg/s)
   Produced:      125000 messages (2083.33 msg/s)
   Filtered:      0 messages
   Errors:        0 (success rate: 100.00%)

📊 FILTER PERFORMANCE
   Pass:          125000
   Fail:          0
   Pass Rate:     100.00%

⏱️  SYSTEM STATUS
   Uptime:        60s
   In-flight:     250 messages
   Consumer Lag:  0 messages

───────────────────────────────────────────────────────────────
Press Ctrl+C to stop monitoring
```

### Option 2: Background Mode

Choose **Option 2** to run for a fixed duration (default: 120 seconds):

```bash
cd benchmarks
TEST_DURATION=300 ./run_observability_test.sh
# Runs for 5 minutes (300 seconds)
```

## Generating Load

While the test is running, generate load in another terminal:

### Using kafka-console-producer (Simple)

```bash
# Terminal 2: Generate test messages
for i in {1..10000}; do
  echo "{\"user\":{\"id\":$((RANDOM % 1000)),\"name\":\"user$i\"},\"event\":\"test\",\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"}"
done | kafka-console-producer --broker-list localhost:9092 --topic test-8p-input
```

### Using kafka-producer-perf-test (High throughput)

```bash
# Terminal 2: Generate 10,000 messages at 1000 msg/s
kafka-producer-perf-test \
  --topic test-8p-input \
  --num-records 10000 \
  --record-size 512 \
  --throughput 1000 \
  --producer-props bootstrap.servers=localhost:9092
```

### Stepped Load Test

```bash
# Step 1: Low load (100 msg/s for 60s)
kafka-producer-perf-test --topic test-8p-input --num-records 6000 \
  --record-size 512 --throughput 100 --producer-props bootstrap.servers=localhost:9092

# Wait 10 seconds
sleep 10

# Step 2: Medium load (1000 msg/s for 60s)
kafka-producer-perf-test --topic test-8p-input --num-records 60000 \
  --record-size 512 --throughput 1000 --producer-props bootstrap.servers=localhost:9092

# Wait 10 seconds
sleep 10

# Step 3: High load (5000 msg/s for 60s)
kafka-producer-perf-test --topic test-8p-input --num-records 300000 \
  --record-size 512 --throughput 5000 --producer-props bootstrap.servers=localhost:9092
```

## Monitoring Metrics

### Live Terminal Dashboard

The test automatically displays a live dashboard with:
- Message throughput (consumed/produced/s)
- Filter performance (pass/fail rates)
- Consumer lag per partition
- System status (uptime, in-flight messages)
- Error rates and success percentage

### Prometheus Metrics Endpoint

Access raw metrics while test is running:

```bash
# View all metrics
curl http://localhost:9090/metrics

# Filter specific metrics
curl -s http://localhost:9090/metrics | grep streamforge_messages_consumed

# Extract consumer lag
curl -s http://localhost:9090/metrics | grep streamforge_consumer_lag

# Get processing latency histogram
curl -s http://localhost:9090/metrics | grep streamforge_processing_duration_seconds
```

### Health Check

```bash
curl http://localhost:9090/health
# Should return: OK
```

### Key Metrics to Monitor

#### Throughput
```bash
# Messages consumed per second
curl -s http://localhost:9090/metrics | grep "streamforge_messages_consumed_total"

# Messages produced per destination
curl -s http://localhost:9090/metrics | grep "streamforge_messages_produced_total"
```

#### Latency
```bash
# Processing duration histogram
curl -s http://localhost:9090/metrics | grep "streamforge_processing_duration_seconds_bucket"

# Calculate P99: Use Prometheus query
# histogram_quantile(0.99, rate(streamforge_processing_duration_seconds_bucket[5m]))
```

#### Consumer Lag (Critical!)
```bash
# Lag per partition
curl -s http://localhost:9090/metrics | grep "streamforge_consumer_lag"

# Total lag across all partitions
curl -s http://localhost:9090/metrics | grep "streamforge_consumer_lag" | awk '{sum+=$2} END {print sum}'
```

#### Errors
```bash
# Processing errors by type
curl -s http://localhost:9090/metrics | grep "streamforge_processing_errors_total"
```

## Results Analysis

After the test completes, find results in:

```
benchmarks/results/observability_YYYYMMDD_HHMMSS/
├── test_config.yaml           # Test configuration used
├── streamforge.log            # Application logs
├── metrics.csv                # Time-series metrics data
└── OBSERVABILITY_TEST_REPORT.md  # Performance report
```

### Metrics CSV Format

```csv
timestamp,messages_consumed,messages_produced,processing_errors,consumer_lag,messages_in_flight,filter_pass,filter_fail
1706889600,1000,1000,0,0,50,1000,0
1706889602,2500,2500,0,0,75,2500,0
1706889604,4200,4200,0,0,80,4200,0
...
```

### Analyzing Metrics Data

```bash
# Calculate average throughput
awk -F',' 'NR>1 {consumed+=$2; count++} END {print consumed/count " messages consumed"}' metrics.csv

# Find peak lag
awk -F',' 'NR>1 {if($5>max)max=$5} END {print "Peak lag: " max}' metrics.csv

# Calculate error rate
awk -F',' 'NR>1 {errors+=$4; consumed+=$2} END {print "Error rate: " (errors/consumed)*100 "%"}' metrics.csv
```

## Troubleshooting

### Kafka not starting

```bash
# Check if Kafka container is running
docker ps | grep kafka

# Check Kafka logs
docker-compose -f docker-compose.benchmark.yml logs kafka

# Restart Kafka
docker-compose -f docker-compose.benchmark.yml restart kafka
```

### Topics not created

```bash
# List existing topics
kafka-topics --list --bootstrap-server localhost:9092

# Create topics manually
kafka-topics --create --topic test-8p-input --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092
kafka-topics --create --topic test-8p-output --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092
```

### Metrics endpoint not accessible

```bash
# Check if Streamforge is running
ps aux | grep streamforge

# Check logs
tail -f benchmarks/results/observability_*/streamforge.log

# Verify metrics port
netstat -an | grep 9090
```

### No messages being consumed

```bash
# Check consumer group
kafka-consumer-groups --bootstrap-server localhost:9092 --describe --group streamforge-observability-test

# Check topic has data
kafka-console-consumer --bootstrap-server localhost:9092 --topic test-8p-input --from-beginning --max-messages 10

# Check partition assignment
kafka-consumer-groups --bootstrap-server localhost:9092 --describe --group streamforge-observability-test
```

## Advanced Usage

### Custom Test Duration

```bash
TEST_DURATION=600 ./run_observability_test.sh
# Runs for 10 minutes
```

### Custom Metrics Port

```bash
METRICS_PORT=9091 ./run_observability_test.sh
```

### Different Kafka Broker

```bash
KAFKA_BROKER=remote-kafka:9092 ./run_observability_test.sh
```

### Run with Prometheus

If you want to scrape metrics with actual Prometheus:

```bash
# 1. Start Prometheus with provided config
docker run -d \
  -p 9091:9090 \
  -v $(pwd)/../examples/prometheus.yml:/etc/prometheus/prometheus.yml \
  prom/prometheus

# 2. Run test
./run_observability_test.sh

# 3. Access Prometheus UI
open http://localhost:9091
```

## Performance Targets

Based on previous benchmarks:

| Threads | Expected Throughput | Consumer Lag | Success Rate |
|---------|---------------------|--------------|--------------|
| 4       | 10,000-11,000 msg/s | < 100        | > 99.9%      |
| 8       | 25,000-30,000 msg/s | < 500        | > 99.9%      |

## Next Steps

1. **Run baseline test**: `./run_observability_test.sh`
2. **Generate load**: Use kafka-producer-perf-test
3. **Monitor metrics**: Watch live dashboard or curl metrics endpoint
4. **Analyze results**: Review metrics.csv and report
5. **Tune configuration**: Adjust threads, batch sizes based on results
6. **Compare with benchmarks**: See `results/BENCHMARK_RESULTS.md`

## See Also

- [BENCHMARK_RESULTS.md](results/BENCHMARK_RESULTS.md) - Historical benchmark results
- [../docs/OBSERVABILITY_QUICKSTART.md](../docs/OBSERVABILITY_QUICKSTART.md) - Observability setup guide
- [../docs/PERFORMANCE.md](../docs/PERFORMANCE.md) - Performance tuning guide
- [../docs/SCALING.md](../docs/SCALING.md) - Scaling architecture
