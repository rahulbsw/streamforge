#!/bin/bash
#
# Observability-Enabled Performance Test
#
# Runs performance test with Prometheus metrics monitoring
# Uses existing benchmark infrastructure with metrics collection
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Configuration
KAFKA_BROKER=${KAFKA_BROKER:-"localhost:9092"}
METRICS_PORT=${METRICS_PORT:-9090}
TEST_DURATION=${TEST_DURATION:-120}  # 2 minutes default
RESULTS_DIR="$SCRIPT_DIR/results/observability_$(date +%Y%m%d_%H%M%S)"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

cleanup() {
    log_info "Cleaning up..."
    if [[ -n "$STREAMFORGE_PID" ]] && kill -0 $STREAMFORGE_PID 2>/dev/null; then
        kill $STREAMFORGE_PID 2>/dev/null || true
        wait $STREAMFORGE_PID 2>/dev/null || true
    fi
    if [[ -n "$METRICS_PID" ]] && kill -0 $METRICS_PID 2>/dev/null; then
        kill $METRICS_PID 2>/dev/null || true
    fi
}

trap cleanup EXIT INT TERM

# Check prerequisites
check_prerequisites() {
    log_step "Checking Prerequisites"

    if [[ ! -f "$PROJECT_ROOT/target/release/streamforge" ]]; then
        log_error "Binary not found. Building..."
        cd "$PROJECT_ROOT"
        cargo build --release
    fi
    log_info "✓ Streamforge binary ready"

    if ! timeout 2 bash -c "echo > /dev/tcp/localhost/9092" 2>/dev/null; then
        log_error "Kafka not running on localhost:9092"
        log_info "Start with: docker-compose up -d"
        exit 1
    fi
    log_info "✓ Kafka reachable"

    mkdir -p "$RESULTS_DIR"
    log_info "✓ Results directory: $RESULTS_DIR"
}

# Create test config with observability
create_config() {
    log_step "Creating Test Configuration"

    cat > "$RESULTS_DIR/test_config.yaml" << EOF
appid: streamforge-observability-test
bootstrap: $KAFKA_BROKER
target_broker: $KAFKA_BROKER
input: test-8p-input
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
    - output: test-8p-output
      filter: "EXISTS:/user"
      key_transform: "/user/id"
      headers:
        x-processed: "true"
EOF

    log_info "✓ Config created: $RESULTS_DIR/test_config.yaml"
}

# Start Streamforge with observability
start_streamforge() {
    log_step "Starting Streamforge"

    cd "$PROJECT_ROOT"
    CONFIG_FILE="$RESULTS_DIR/test_config.yaml" \
        RUST_LOG=info \
        ./target/release/streamforge \
        > "$RESULTS_DIR/streamforge.log" 2>&1 &

    STREAMFORGE_PID=$!
    log_info "Streamforge started (PID: $STREAMFORGE_PID)"

    # Wait for metrics endpoint
    log_info "Waiting for metrics endpoint..."
    for i in {1..30}; do
        if curl -sf http://localhost:$METRICS_PORT/health > /dev/null 2>&1; then
            log_info "✓ Metrics endpoint ready"
            sleep 2
            return 0
        fi
        sleep 1
    done

    log_error "Metrics endpoint not ready"
    return 1
}

# Monitor metrics in real-time
monitor_metrics() {
    log_step "Starting Metrics Monitoring"

    local output_file="$RESULTS_DIR/metrics.csv"

    (
        echo "timestamp,messages_consumed,messages_produced,processing_errors,consumer_lag,messages_in_flight,filter_pass,filter_fail" > "$output_file"

        while true; do
            timestamp=$(date +%s)
            metrics=$(curl -s http://localhost:$METRICS_PORT/metrics 2>/dev/null || echo "")

            if [[ -z "$metrics" ]]; then
                sleep 2
                continue
            fi

            consumed=$(echo "$metrics" | grep "^streamforge_messages_consumed_total " | awk '{print $2}' || echo "0")
            produced=$(echo "$metrics" | grep "^streamforge_messages_produced_total{" | awk '{sum+=$2} END {print sum}' || echo "0")
            errors=$(echo "$metrics" | grep "^streamforge_processing_errors_total{" | awk '{sum+=$2} END {print sum}' || echo "0")
            lag=$(echo "$metrics" | grep "^streamforge_consumer_lag{" | awk '{sum+=$2} END {print sum}' || echo "0")
            in_flight=$(echo "$metrics" | grep "^streamforge_messages_in_flight " | awk '{print $2}' || echo "0")
            filter_pass=$(echo "$metrics" | grep 'streamforge_filter_evaluations_total.*result="pass"' | awk '{sum+=$2} END {print sum}' || echo "0")
            filter_fail=$(echo "$metrics" | grep 'streamforge_filter_evaluations_total.*result="fail"' | awk '{sum+=$2} END {print sum}' || echo "0")

            echo "$timestamp,$consumed,$produced,$errors,$lag,$in_flight,$filter_pass,$filter_fail" >> "$output_file"

            # Display current metrics
            echo -ne "\r\033[K"  # Clear line
            echo -ne "📊 Consumed: $consumed | Produced: $produced | Lag: $lag | Errors: $errors | In-flight: $in_flight"

            sleep 2
        done
    ) &

    METRICS_PID=$!
    log_info "Metrics collector started (PID: $METRICS_PID)"
}

# Display real-time metrics summary
display_metrics() {
    log_step "Live Metrics (refresh every 5s, Ctrl+C to stop)"

    echo ""
    echo "Metrics endpoint: http://localhost:$METRICS_PORT/metrics"
    echo "Streamforge logs: $RESULTS_DIR/streamforge.log"
    echo ""

    while true; do
        clear
        echo "═══════════════════════════════════════════════════════════════"
        echo "  Streamforge Observability Test - Live Metrics"
        echo "═══════════════════════════════════════════════════════════════"
        echo ""

        metrics=$(curl -s http://localhost:$METRICS_PORT/metrics 2>/dev/null || echo "")

        if [[ -z "$metrics" ]]; then
            echo "⚠️  Metrics endpoint not available"
            sleep 5
            continue
        fi

        # Extract metrics
        consumed=$(echo "$metrics" | grep "^streamforge_messages_consumed_total " | awk '{print $2}' || echo "0")
        produced=$(echo "$metrics" | grep "^streamforge_messages_produced_total{" | awk '{sum+=$2} END {print sum}' || echo "0")
        filtered=$(echo "$metrics" | grep "^streamforge_messages_filtered_total{" | awk '{sum+=$2} END {print sum}' || echo "0")
        errors=$(echo "$metrics" | grep "^streamforge_processing_errors_total{" | awk '{sum+=$2} END {print sum}' || echo "0")
        lag=$(echo "$metrics" | grep "^streamforge_consumer_lag{" | awk '{sum+=$2} END {print sum}' || echo "0")
        in_flight=$(echo "$metrics" | grep "^streamforge_messages_in_flight " | awk '{print $2}' || echo "0")
        uptime=$(echo "$metrics" | grep "^streamforge_uptime_seconds " | awk '{print $2}' || echo "0")

        filter_pass=$(echo "$metrics" | grep 'streamforge_filter_evaluations_total.*result="pass"' | awk '{sum+=$2} END {print sum}' || echo "0")
        filter_fail=$(echo "$metrics" | grep 'streamforge_filter_evaluations_total.*result="fail"' | awk '{sum+=$2} END {print sum}' || echo "0")

        # Calculate rates
        if [[ $uptime =~ ^[0-9]+(\.[0-9]+)?$ ]] && (( $(echo "$uptime > 0" | bc -l) )); then
            consume_rate=$(echo "scale=2; $consumed / $uptime" | bc)
            produce_rate=$(echo "scale=2; $produced / $uptime" | bc)
        else
            consume_rate="0"
            produce_rate="0"
        fi

        # Calculate success rate
        if [[ $consumed -gt 0 ]]; then
            success_rate=$(echo "scale=2; 100 * (1 - $errors / $consumed)" | bc)
        else
            success_rate="100.00"
        fi

        # Display
        echo "📥 MESSAGE PROCESSING"
        echo "   Consumed:      $consumed messages ($consume_rate msg/s)"
        echo "   Produced:      $produced messages ($produce_rate msg/s)"
        echo "   Filtered:      $filtered messages"
        echo "   Errors:        $errors (success rate: ${success_rate}%)"
        echo ""

        echo "📊 FILTER PERFORMANCE"
        echo "   Pass:          $filter_pass"
        echo "   Fail:          $filter_fail"
        if [[ $((filter_pass + filter_fail)) -gt 0 ]]; then
            pass_rate=$(echo "scale=2; 100 * $filter_pass / ($filter_pass + $filter_fail)" | bc)
            echo "   Pass Rate:     ${pass_rate}%"
        fi
        echo ""

        echo "⏱️  SYSTEM STATUS"
        echo "   Uptime:        ${uptime}s"
        echo "   In-flight:     $in_flight messages"
        echo "   Consumer Lag:  $lag messages"
        echo ""

        # Lag warning
        if [[ $(echo "$lag > 1000" | bc) -eq 1 ]]; then
            echo -e "${YELLOW}⚠️  WARNING: High consumer lag detected${NC}"
            echo ""
        fi

        echo "───────────────────────────────────────────────────────────────"
        echo "Press Ctrl+C to stop monitoring"
        echo ""

        sleep 5
    done
}

# Generate report
generate_report() {
    log_step "Generating Report"

    local report="$RESULTS_DIR/OBSERVABILITY_TEST_REPORT.md"

    # Get final metrics
    metrics=$(curl -s http://localhost:$METRICS_PORT/metrics 2>/dev/null || echo "")

    consumed=$(echo "$metrics" | grep "^streamforge_messages_consumed_total " | awk '{print $2}' || echo "0")
    produced=$(echo "$metrics" | grep "^streamforge_messages_produced_total{" | awk '{sum+=$2} END {print sum}' || echo "0")
    errors=$(echo "$metrics" | grep "^streamforge_processing_errors_total{" | awk '{sum+=$2} END {print sum}' || echo "0")
    uptime=$(echo "$metrics" | grep "^streamforge_uptime_seconds " | awk '{print $2}' || echo "0")

    cat > "$report" << EOF
# Observability Performance Test Report

**Date**: $(date)
**Duration**: ${uptime}s
**Configuration**: 8 threads, 8 partitions

---

## Summary

- **Messages Consumed**: $consumed
- **Messages Produced**: $produced
- **Errors**: $errors
- **Average Throughput**: $(echo "scale=2; $consumed / $uptime" | bc) msg/s

---

## Metrics Collected

All metrics saved to: \`metrics.csv\`

### Available Prometheus Metrics

\`\`\`
http://localhost:$METRICS_PORT/metrics
\`\`\`

Key metrics:
- Message throughput (consumed, produced, filtered)
- Processing latency (P50, P95, P99)
- Consumer lag per partition
- Filter performance (pass/fail rates)
- Transform operations
- Error rates by type

---

## Files Generated

- \`test_config.yaml\` - Test configuration
- \`streamforge.log\` - Streamforge application logs
- \`metrics.csv\` - Time-series metrics data
- \`OBSERVABILITY_TEST_REPORT.md\` - This report

---

## Next Steps

1. Analyze metrics.csv for performance trends
2. Check streamforge.log for warnings/errors
3. Query specific metrics from Prometheus endpoint
4. Compare with previous benchmark results

EOF

    log_info "✓ Report generated: $report"
    cat "$report"
}

# Main execution
main() {
    log_step "Streamforge Observability Performance Test"

    check_prerequisites
    create_config
    start_streamforge
    monitor_metrics

    log_info ""
    log_info "Test running. Choose an option:"
    log_info "  1. Watch live metrics (recommended)"
    log_info "  2. Run in background for $TEST_DURATION seconds"
    echo ""
    read -p "Choice (1/2): " -n 1 -r choice
    echo ""

    if [[ $choice == "1" ]]; then
        display_metrics
    else
        log_info "Running test for $TEST_DURATION seconds..."
        log_info "Metrics being collected in background"
        log_info "Tail logs: tail -f $RESULTS_DIR/streamforge.log"
        log_info "View metrics: curl http://localhost:$METRICS_PORT/metrics"

        sleep $TEST_DURATION

        generate_report

        log_step "Test Complete!"
        log_info "Results: $RESULTS_DIR"
    fi
}

main "$@"
