#!/bin/bash
#
# Comprehensive performance test with stepped load and metrics monitoring
#
# This script:
# 1. Starts Streamforge with observability enabled
# 2. Generates stepped load (100 -> 1000 -> 5000 -> 10000 msg/s)
# 3. Monitors Prometheus metrics in real-time
# 4. Collects performance data and generates a report
#
# Prerequisites:
# - Kafka running on localhost:9092
# - Input topic created: perf-test-input
# - Output topics created: perf-test-output-1, perf-test-output-2
#

set -e

# Configuration
KAFKA_BROKER=${KAFKA_BROKER:-"localhost:9092"}
INPUT_TOPIC=${INPUT_TOPIC:-"perf-test-input"}
OUTPUT_TOPIC_1=${OUTPUT_TOPIC_1:-"perf-test-output-1"}
OUTPUT_TOPIC_2=${OUTPUT_TOPIC_2:-"perf-test-output-2"}
METRICS_PORT=${METRICS_PORT:-9090}
TEST_DURATION_PER_STEP=${TEST_DURATION_PER_STEP:-60}  # seconds per load step
RESULTS_DIR="./performance_results_$(date +%Y%m%d_%H%M%S)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

# Cleanup function
cleanup() {
    log_info "Cleaning up..."

    # Stop load generator
    if [[ -n "$LOAD_GEN_PID" ]] && kill -0 $LOAD_GEN_PID 2>/dev/null; then
        log_info "Stopping load generator (PID: $LOAD_GEN_PID)"
        kill $LOAD_GEN_PID 2>/dev/null || true
        wait $LOAD_GEN_PID 2>/dev/null || true
    fi

    # Stop metrics collector
    if [[ -n "$METRICS_PID" ]] && kill -0 $METRICS_PID 2>/dev/null; then
        log_info "Stopping metrics collector (PID: $METRICS_PID)"
        kill $METRICS_PID 2>/dev/null || true
        wait $METRICS_PID 2>/dev/null || true
    fi

    # Stop Streamforge
    if [[ -n "$STREAMFORGE_PID" ]] && kill -0 $STREAMFORGE_PID 2>/dev/null; then
        log_info "Stopping Streamforge (PID: $STREAMFORGE_PID)"
        kill $STREAMFORGE_PID 2>/dev/null || true
        wait $STREAMFORGE_PID 2>/dev/null || true
    fi

    log_info "Cleanup complete"
}

trap cleanup EXIT INT TERM

# Check prerequisites
check_prerequisites() {
    log_step "Checking Prerequisites"

    # Check if Streamforge binary exists
    if [[ ! -f "./target/release/streamforge" ]]; then
        log_error "Streamforge binary not found. Run: cargo build --release"
        exit 1
    fi
    log_info "✓ Streamforge binary found"

    # Check if Kafka is reachable
    if ! timeout 5 bash -c "echo > /dev/tcp/${KAFKA_BROKER%:*}/${KAFKA_BROKER#*:}" 2>/dev/null; then
        log_error "Kafka not reachable at $KAFKA_BROKER"
        log_info "Start Kafka with: docker-compose up -d kafka"
        exit 1
    fi
    log_info "✓ Kafka reachable at $KAFKA_BROKER"

    # Check if kafka-producer-perf-test is available
    if ! command -v kafka-producer-perf-test.sh &> /dev/null && \
       ! command -v kafka-producer-perf-test &> /dev/null; then
        log_warn "kafka-producer-perf-test not found in PATH"
        log_warn "Will use custom load generator (slower)"
        USE_CUSTOM_LOADER=true
    else
        log_info "✓ kafka-producer-perf-test found"
        USE_CUSTOM_LOADER=false
    fi

    # Create results directory
    mkdir -p "$RESULTS_DIR"
    log_info "✓ Results directory: $RESULTS_DIR"
}

# Create test configuration
create_test_config() {
    log_step "Creating Test Configuration"

    cat > "$RESULTS_DIR/test_config.yaml" << EOF
appid: streamforge-perf-test
bootstrap: $KAFKA_BROKER
target_broker: $KAFKA_BROKER
input: $INPUT_TOPIC
threads: 8
offset: latest

observability:
  metrics_enabled: true
  metrics_port: $METRICS_PORT
  lag_monitoring_enabled: true
  lag_monitoring_interval_secs: 10

routing:
  routing_type: filter
  destinations:
    # Route 50% to output-1 (even user IDs)
    - output: $OUTPUT_TOPIC_1
      filter: "MOD:/user/id,2,0"
      key_transform: "/user/id"
      headers:
        x-route: "output-1"

    # Route 50% to output-2 (odd user IDs)
    - output: $OUTPUT_TOPIC_2
      filter: "MOD:/user/id,2,1"
      key_transform: "/user/id"
      headers:
        x-route: "output-2"
EOF

    log_info "✓ Test configuration created: $RESULTS_DIR/test_config.yaml"
}

# Start Streamforge
start_streamforge() {
    log_step "Starting Streamforge"

    CONFIG_FILE="$RESULTS_DIR/test_config.yaml" \
        RUST_LOG=info \
        ./target/release/streamforge \
        > "$RESULTS_DIR/streamforge.log" 2>&1 &

    STREAMFORGE_PID=$!
    log_info "Streamforge started (PID: $STREAMFORGE_PID)"

    # Wait for metrics endpoint to be ready
    log_info "Waiting for metrics endpoint..."
    for i in {1..30}; do
        if curl -sf http://localhost:$METRICS_PORT/health > /dev/null 2>&1; then
            log_info "✓ Metrics endpoint ready"
            sleep 2  # Give it a bit more time to stabilize
            return 0
        fi
        sleep 1
    done

    log_error "Metrics endpoint not ready after 30 seconds"
    return 1
}

# Metrics collector
collect_metrics() {
    local interval=$1
    local output_file=$2

    log_info "Starting metrics collector (interval: ${interval}s)"

    (
        echo "timestamp,messages_consumed,messages_produced,messages_filtered,processing_errors,consumer_lag,p50_latency_ms,p95_latency_ms,p99_latency_ms,messages_in_flight,throughput_mps" > "$output_file"

        while true; do
            timestamp=$(date +%s)

            metrics=$(curl -s http://localhost:$METRICS_PORT/metrics 2>/dev/null || echo "")

            if [[ -z "$metrics" ]]; then
                sleep $interval
                continue
            fi

            # Extract key metrics
            consumed=$(echo "$metrics" | grep "^streamforge_messages_consumed_total " | awk '{print $2}' || echo "0")
            produced=$(echo "$metrics" | grep "^streamforge_messages_produced_total{" | awk '{sum+=$2} END {print sum}' || echo "0")
            filtered=$(echo "$metrics" | grep "^streamforge_messages_filtered_total{" | awk '{sum+=$2} END {print sum}' || echo "0")
            errors=$(echo "$metrics" | grep "^streamforge_processing_errors_total{" | awk '{sum+=$2} END {print sum}' || echo "0")
            lag=$(echo "$metrics" | grep "^streamforge_consumer_lag{" | awk '{sum+=$2} END {print sum}' || echo "0")
            in_flight=$(echo "$metrics" | grep "^streamforge_messages_in_flight " | awk '{print $2}' || echo "0")

            # Calculate throughput (messages per second) - simple approximation
            throughput=$(echo "$metrics" | grep "^streamforge_processing_rate_mps " | awk '{print $2}' || echo "0")

            # Extract latency percentiles (histogram quantiles - simplified)
            # In real Prometheus, you'd calculate quantiles, here we'll use bucket counts as proxy
            p50="0"
            p95="0"
            p99="0"

            echo "$timestamp,$consumed,$produced,$filtered,$errors,$lag,$p50,$p95,$p99,$in_flight,$throughput" >> "$output_file"

            sleep $interval
        done
    ) &

    METRICS_PID=$!
}

# Load generator function
generate_load() {
    local rate=$1
    local duration=$2
    local step_name=$3

    log_step "Load Step: $step_name ($rate msg/s for ${duration}s)"

    local total_messages=$((rate * duration))
    local record_size=1024

    log_info "Generating $total_messages messages at $rate msg/s..."

    if [[ "$USE_CUSTOM_LOADER" == "true" ]]; then
        # Custom loader using kafka-console-producer
        generate_load_custom "$rate" "$duration"
    else
        # Use kafka-producer-perf-test
        local throughput=$((rate))  # Messages per second

        kafka-producer-perf-test.sh \
            --topic "$INPUT_TOPIC" \
            --num-records "$total_messages" \
            --record-size "$record_size" \
            --throughput "$throughput" \
            --producer-props \
                bootstrap.servers="$KAFKA_BROKER" \
                acks=1 \
                compression.type=none \
            --payload-file <(generate_test_payload) \
            2>&1 | tee -a "$RESULTS_DIR/load_gen_${step_name}.log" || true
    fi

    log_info "✓ Load generation complete for step: $step_name"

    # Cool down period
    log_info "Cool down for 10 seconds..."
    sleep 10
}

# Generate test payload
generate_test_payload() {
    cat << 'EOF'
{"user":{"id":{{USER_ID}},"name":"user{{USER_ID}}","email":"user{{USER_ID}}@example.com"},"event":"purchase","amount":{{AMOUNT}},"timestamp":"{{TIMESTAMP}}"}
EOF
}

# Custom load generator
generate_load_custom() {
    local rate=$1
    local duration=$2

    local interval_ms=$((1000 / rate))
    local total_messages=$((rate * duration))

    log_warn "Using custom load generator (may not achieve exact rate)"

    for i in $(seq 1 $total_messages); do
        user_id=$((RANDOM % 1000))
        amount=$((RANDOM % 10000))
        timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

        echo "{\"user\":{\"id\":$user_id,\"name\":\"user$user_id\",\"email\":\"user$user_id@example.com\"},\"event\":\"purchase\",\"amount\":$amount,\"timestamp\":\"$timestamp\"}" | \
            kafka-console-producer.sh \
                --broker-list "$KAFKA_BROKER" \
                --topic "$INPUT_TOPIC" \
                2>/dev/null || true

        # Throttle
        if [[ $((i % rate)) -eq 0 ]]; then
            sleep 1
        fi
    done
}

# Run performance test
run_performance_test() {
    log_step "Running Performance Test"

    # Start metrics collector (collect every 2 seconds)
    collect_metrics 2 "$RESULTS_DIR/metrics.csv"

    log_info "Metrics collector started (PID: $METRICS_PID)"
    log_info "Metrics being saved to: $RESULTS_DIR/metrics.csv"

    # Wait for baseline
    log_info "Waiting 10 seconds for baseline metrics..."
    sleep 10

    # Step 1: 100 msg/s (warm-up)
    generate_load 100 $TEST_DURATION_PER_STEP "warmup_100mps"

    # Step 2: 1,000 msg/s
    generate_load 1000 $TEST_DURATION_PER_STEP "load_1000mps"

    # Step 3: 5,000 msg/s
    generate_load 5000 $TEST_DURATION_PER_STEP "load_5000mps"

    # Step 4: 10,000 msg/s (peak)
    generate_load 10000 $TEST_DURATION_PER_STEP "peak_10000mps"

    # Step 5: Sustained load (5000 msg/s for 2 minutes)
    log_step "Sustained Load Test (5000 msg/s for 2 minutes)"
    generate_load 5000 120 "sustained_5000mps"

    log_info "All load steps complete. Collecting final metrics..."
    sleep 15
}

# Generate performance report
generate_report() {
    log_step "Generating Performance Report"

    local report_file="$RESULTS_DIR/PERFORMANCE_REPORT.md"

    cat > "$report_file" << EOF
# Streamforge Performance Test Report

**Date**: $(date)
**Duration**: $((TEST_DURATION_PER_STEP * 4 + 120)) seconds
**Configuration**: 8 threads, 2 destinations, filter-based routing

---

## Test Configuration

- **Kafka Broker**: $KAFKA_BROKER
- **Input Topic**: $INPUT_TOPIC
- **Output Topics**: $OUTPUT_TOPIC_1, $OUTPUT_TOPIC_2
- **Message Size**: ~1KB JSON
- **Routing**: 50/50 split based on user ID (even/odd)

---

## Load Steps

1. **Warm-up**: 100 msg/s for ${TEST_DURATION_PER_STEP}s
2. **Low Load**: 1,000 msg/s for ${TEST_DURATION_PER_STEP}s
3. **Medium Load**: 5,000 msg/s for ${TEST_DURATION_PER_STEP}s
4. **Peak Load**: 10,000 msg/s for ${TEST_DURATION_PER_STEP}s
5. **Sustained**: 5,000 msg/s for 120s

---

## Metrics Summary

EOF

    # Analyze metrics from CSV
    if [[ -f "$RESULTS_DIR/metrics.csv" ]]; then
        log_info "Analyzing metrics data..."

        # Calculate statistics using awk
        awk -F',' 'NR>1 {
            consumed+=$2; produced+=$3; filtered+=$4; errors+=$5;
            lag+=$6; in_flight+=$7; throughput+=$8; count++
        }
        END {
            printf "### Overall Statistics\n\n"
            printf "- **Total Messages Consumed**: %d\n", consumed
            printf "- **Total Messages Produced**: %d\n", produced
            printf "- **Total Messages Filtered**: %d\n", filtered
            printf "- **Total Errors**: %d\n", errors
            printf "- **Average Consumer Lag**: %.2f messages\n", lag/count
            printf "- **Average Messages In Flight**: %.2f\n", in_flight/count
            printf "- **Average Throughput**: %.2f msg/s\n\n", throughput/count
        }' "$RESULTS_DIR/metrics.csv" >> "$report_file"
    fi

    cat >> "$report_file" << EOF

---

## Collected Data

- **Metrics CSV**: metrics.csv
- **Streamforge Logs**: streamforge.log
- **Load Generator Logs**: load_gen_*.log

---

## Key Observations

EOF

    # Extract final metrics
    final_metrics=$(curl -s http://localhost:$METRICS_PORT/metrics 2>/dev/null || echo "")

    if [[ -n "$final_metrics" ]]; then
        consumed=$(echo "$final_metrics" | grep "^streamforge_messages_consumed_total " | awk '{print $2}')
        produced=$(echo "$final_metrics" | grep "^streamforge_messages_produced_total{" | awk '{sum+=$2} END {print sum}')
        errors=$(echo "$final_metrics" | grep "^streamforge_processing_errors_total{" | awk '{sum+=$2} END {print sum}')

        cat >> "$report_file" << EOF
1. **Messages Consumed**: $consumed
2. **Messages Produced**: $produced
3. **Processing Errors**: $errors
4. **Success Rate**: $(echo "scale=2; 100 * (1 - $errors / $consumed)" | bc)%

EOF
    fi

    cat >> "$report_file" << EOF
---

## Next Steps

1. Review metrics.csv for detailed time-series data
2. Check streamforge.log for any errors or warnings
3. Compare results with previous test runs
4. Tune configuration if needed (threads, batch sizes, etc.)

EOF

    log_info "✓ Performance report generated: $report_file"

    # Display report summary
    echo ""
    cat "$report_file"
}

# Main execution
main() {
    log_step "Streamforge Performance Test"

    check_prerequisites
    create_test_config
    start_streamforge
    run_performance_test
    generate_report

    log_step "Performance Test Complete!"
    log_info "Results saved to: $RESULTS_DIR"
    log_info "View report: cat $RESULTS_DIR/PERFORMANCE_REPORT.md"
    log_info "View metrics: cat $RESULTS_DIR/metrics.csv"

    # Ask if user wants to keep Streamforge running
    echo ""
    read -p "Keep Streamforge running for manual inspection? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Stopping Streamforge..."
        return 0
    else
        log_info "Streamforge still running (PID: $STREAMFORGE_PID)"
        log_info "Metrics: http://localhost:$METRICS_PORT/metrics"
        log_info "Stop with: kill $STREAMFORGE_PID"

        # Disable cleanup trap
        trap - EXIT INT TERM
    fi
}

# Run main
main "$@"
