#!/bin/bash
#
# High Throughput Test with JSON Data
#
# This script runs a proper throughput test with valid JSON messages
# to measure actual Streamforge performance.
#
# Usage:
#   ./run_throughput_test.sh [num_messages] [target_throughput]
#
# Examples:
#   ./run_throughput_test.sh 100000 30000    # 100K messages at 30K msg/s
#   ./run_throughput_test.sh 500000 50000    # 500K messages at 50K msg/s
#

set -e

NUM_MESSAGES=${1:-200000}
TARGET_THROUGHPUT=${2:-30000}

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

log_step() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Test configuration
TEST_DATA_FILE="$SCRIPT_DIR/test_data_${NUM_MESSAGES}.jsonl"
TEST_TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_DIR="$SCRIPT_DIR/results/throughput_test_$TEST_TIMESTAMP"
mkdir -p "$RESULTS_DIR"

log_step "Streamforge Throughput Test"
echo -e "${GREEN}Messages:${NC} $NUM_MESSAGES"
echo -e "${GREEN}Target Throughput:${NC} $TARGET_THROUGHPUT msg/s"
echo -e "${GREEN}Results:${NC} $RESULTS_DIR"

# Step 1: Check prerequisites
log_step "Step 1: Checking Prerequisites"

# Check Kafka
if ! nc -z localhost 9092 2>/dev/null; then
    log_error "Kafka is not running on localhost:9092"
    log_info "Start Kafka: docker-compose -f docker-compose.benchmark.yml up -d"
    exit 1
fi
log_info "✓ Kafka is running"

# Check topics
if ! kafka-topics --bootstrap-server localhost:9092 --list | grep -q "test-8p-input"; then
    log_error "Topic 'test-8p-input' does not exist"
    log_info "Create it: kafka-topics --create --topic test-8p-input --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092"
    exit 1
fi
log_info "✓ Topics exist"

# Check binary
if [[ ! -f "$PROJECT_ROOT/target/release/streamforge" ]]; then
    log_error "Streamforge binary not found"
    log_info "Build it: cargo build --release"
    exit 1
fi
log_info "✓ Streamforge binary ready"

# Step 2: Generate test data
log_step "Step 2: Generating Test Data"

if [[ -f "$TEST_DATA_FILE" ]]; then
    log_warn "Test data already exists: $TEST_DATA_FILE"
    log_info "Using existing file"
else
    log_info "Generating $NUM_MESSAGES JSON messages..."
    "$SCRIPT_DIR/generate_json_test_data.sh" "$NUM_MESSAGES" "$TEST_DATA_FILE"
fi

# Get file size
FILE_SIZE_MB=$(du -m "$TEST_DATA_FILE" | cut -f1)
log_info "✓ Test data ready ($FILE_SIZE_MB MB)"

# Step 3: Clean topics
log_step "Step 3: Cleaning Topics"

log_info "Deleting and recreating topics..."
kafka-topics --delete --topic test-8p-input --bootstrap-server localhost:9092 2>/dev/null || true
kafka-topics --delete --topic test-8p-output --bootstrap-server localhost:9092 2>/dev/null || true
sleep 2

kafka-topics --create --topic test-8p-input --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092
kafka-topics --create --topic test-8p-output --partitions 8 --replication-factor 1 --bootstrap-server localhost:9092
log_info "✓ Topics cleaned"

# Step 4: Start Streamforge
log_step "Step 4: Starting Streamforge"

CONFIG_FILE="$SCRIPT_DIR/configs/test-8thread-config.yaml"
LOG_FILE="$RESULTS_DIR/streamforge.log"

log_info "Starting Streamforge with config: $CONFIG_FILE"
"$PROJECT_ROOT/target/release/streamforge" --config "$CONFIG_FILE" > "$LOG_FILE" 2>&1 &
STREAMFORGE_PID=$!
echo $STREAMFORGE_PID > "$RESULTS_DIR/streamforge.pid"

log_info "Streamforge PID: $STREAMFORGE_PID"
log_info "Waiting for startup (5s)..."
sleep 5

# Check if still running
if ! kill -0 $STREAMFORGE_PID 2>/dev/null; then
    log_error "Streamforge failed to start"
    log_info "Check logs: cat $LOG_FILE"
    exit 1
fi
log_info "✓ Streamforge running"

# Step 5: Capture initial metrics
log_step "Step 5: Capturing Baseline Metrics"

sleep 2
curl -s http://localhost:9090/metrics > "$RESULTS_DIR/metrics_before.txt" 2>/dev/null || log_warn "Metrics endpoint not ready"

# Step 6: Send test data
log_step "Step 6: Sending Test Data"

log_info "Sending $NUM_MESSAGES messages to Kafka..."
log_info "Start time: $(date)"

SEND_START=$(date +%s)

cat "$TEST_DATA_FILE" | kafka-console-producer \
    --bootstrap-server localhost:9092 \
    --topic test-8p-input \
    --batch-size 1000 \
    2>&1 | tee "$RESULTS_DIR/producer_output.txt" &

PRODUCER_PID=$!
log_info "Producer PID: $PRODUCER_PID"

# Monitor progress
log_info ""
log_info "Monitoring progress (press Ctrl+C to stop monitoring)..."
echo ""

for i in {1..60}; do
    sleep 5

    # Check if producer is still running
    if ! kill -0 $PRODUCER_PID 2>/dev/null; then
        log_info "Producer completed"
        break
    fi

    # Get current metrics
    CONSUMED=$(curl -s http://localhost:9090/metrics 2>/dev/null | grep "streamforge_messages_consumed_total " | awk '{print $2}' || echo "0")
    PRODUCED=$(curl -s http://localhost:9090/metrics 2>/dev/null | grep "streamforge_messages_produced_total " | awk '{print $2}' || echo "0")
    ERRORS=$(curl -s http://localhost:9090/metrics 2>/dev/null | grep "streamforge_processing_errors_total " | awk '{print $2}' || echo "0")

    # Calculate throughput
    ELAPSED=$(($(date +%s) - SEND_START))
    if [[ $ELAPSED -gt 0 && "$CONSUMED" != "0" ]]; then
        THROUGHPUT=$((CONSUMED / ELAPSED))
        echo -e "  [$ELAPSED s] Consumed: ${GREEN}${CONSUMED}${NC} | Produced: ${GREEN}${PRODUCED}${NC} | Errors: ${RED}${ERRORS}${NC} | Rate: ${YELLOW}${THROUGHPUT} msg/s${NC}"
    fi
done

SEND_END=$(date +%s)
SEND_DURATION=$((SEND_END - SEND_START))

log_info ""
log_info "End time: $(date)"
log_info "Duration: ${SEND_DURATION}s"

# Step 7: Wait for processing to complete
log_step "Step 7: Waiting for Processing"

log_info "Waiting for Streamforge to catch up (max 60s)..."

for i in {1..12}; do
    sleep 5

    CONSUMED=$(curl -s http://localhost:9090/metrics 2>/dev/null | grep "streamforge_messages_consumed_total " | awk '{print $2}' || echo "0")
    PRODUCED=$(curl -s http://localhost:9090/metrics 2>/dev/null | grep "streamforge_messages_produced_total " | awk '{print $2}' || echo "0")

    echo "  Consumed: $CONSUMED | Produced: $PRODUCED | Target: $NUM_MESSAGES"

    if [[ "$CONSUMED" -ge "$NUM_MESSAGES" ]]; then
        log_info "✓ All messages consumed"
        break
    fi
done

sleep 5

# Step 8: Capture final metrics
log_step "Step 8: Capturing Final Metrics"

curl -s http://localhost:9090/metrics > "$RESULTS_DIR/metrics_after.txt"
log_info "✓ Final metrics saved"

# Step 9: Stop Streamforge
log_step "Step 9: Stopping Streamforge"

kill $STREAMFORGE_PID 2>/dev/null || true
wait $STREAMFORGE_PID 2>/dev/null || true
log_info "✓ Streamforge stopped"

# Step 10: Generate report
log_step "Step 10: Generating Report"

# Extract metrics
CONSUMED_TOTAL=$(grep "streamforge_messages_consumed_total " "$RESULTS_DIR/metrics_after.txt" | awk '{print $2}')
PRODUCED_TOTAL=$(grep "streamforge_messages_produced_total " "$RESULTS_DIR/metrics_after.txt" | awk '{print $2}')
ERRORS_TOTAL=$(grep "streamforge_processing_errors_total " "$RESULTS_DIR/metrics_after.txt" | awk '{print $2}')

# Calculate rates
AVG_THROUGHPUT=$((CONSUMED_TOTAL / SEND_DURATION))
SUCCESS_RATE=$(echo "scale=2; ($PRODUCED_TOTAL * 100) / $CONSUMED_TOTAL" | bc)

# Generate report
cat > "$RESULTS_DIR/REPORT.md" << REPORT
# Throughput Test Report

**Date**: $(date)
**Test Duration**: ${SEND_DURATION}s
**Target Messages**: $NUM_MESSAGES
**Target Throughput**: $TARGET_THROUGHPUT msg/s

---

## Results

### Message Counts

| Metric | Value |
|--------|-------|
| **Consumed** | $CONSUMED_TOTAL |
| **Produced** | $PRODUCED_TOTAL |
| **Errors** | $ERRORS_TOTAL |
| **Success Rate** | ${SUCCESS_RATE}% |

### Throughput

| Metric | Value |
|--------|-------|
| **Average** | $AVG_THROUGHPUT msg/s |
| **Target** | $TARGET_THROUGHPUT msg/s |
| **Achievement** | $(echo "scale=1; ($AVG_THROUGHPUT * 100) / $TARGET_THROUGHPUT" | bc)% |

---

## Test Files

- Config: \`$CONFIG_FILE\`
- Test Data: \`$TEST_DATA_FILE\` ($FILE_SIZE_MB MB)
- Streamforge Log: \`streamforge.log\`
- Metrics Before: \`metrics_before.txt\`
- Metrics After: \`metrics_after.txt\`

---

## Analysis

$(if [[ $AVG_THROUGHPUT -ge $TARGET_THROUGHPUT ]]; then
    echo "✅ **SUCCESS**: Achieved target throughput"
elif [[ $AVG_THROUGHPUT -ge $((TARGET_THROUGHPUT * 80 / 100)) ]]; then
    echo "⚠️ **PARTIAL**: Achieved 80%+ of target throughput"
else
    echo "❌ **BELOW TARGET**: Did not achieve target throughput"
fi)

### Bottleneck Analysis

$(if [[ $ERRORS_TOTAL -gt 0 ]]; then
    echo "- **Errors detected**: Check streamforge.log for details"
fi)

$(if [[ $PRODUCED_TOTAL -lt $CONSUMED_TOTAL ]]; then
    echo "- **Message loss**: $(($CONSUMED_TOTAL - $PRODUCED_TOTAL)) messages not produced"
fi)

---

**Generated**: $(date)
REPORT

log_info "✓ Report generated: $RESULTS_DIR/REPORT.md"

# Display summary
log_step "Test Summary"
echo ""
echo -e "${GREEN}Messages Consumed:${NC} $CONSUMED_TOTAL"
echo -e "${GREEN}Messages Produced:${NC} $PRODUCED_TOTAL"
echo -e "${RED}Errors:${NC} $ERRORS_TOTAL"
echo -e "${YELLOW}Average Throughput:${NC} $AVG_THROUGHPUT msg/s"
echo -e "${YELLOW}Success Rate:${NC} ${SUCCESS_RATE}%"
echo ""

if [[ $AVG_THROUGHPUT -ge $TARGET_THROUGHPUT ]]; then
    echo -e "${GREEN}✅ Test PASSED${NC} - Achieved target throughput"
else
    echo -e "${YELLOW}⚠️ Test PARTIAL${NC} - Below target throughput"
fi

echo ""
echo -e "${BLUE}Full report:${NC} $RESULTS_DIR/REPORT.md"
echo -e "${BLUE}View logs:${NC} cat $RESULTS_DIR/streamforge.log"
echo ""
