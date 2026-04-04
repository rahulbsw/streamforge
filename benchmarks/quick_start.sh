#!/bin/bash
#
# Quick Start - Observability Performance Test
#
# This script automates the entire setup:
# 1. Start Kafka
# 2. Create topics
# 3. Build Streamforge
# 4. Run observability test
#

set -e

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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

log_step "Streamforge Observability Test - Quick Start"

# Step 1: Check Docker
log_step "Step 1: Checking Docker"
if ! docker ps > /dev/null 2>&1; then
    log_error "Docker is not running. Please start Docker and try again."
    exit 1
fi
log_info "✓ Docker is running"

# Step 2: Start Kafka
log_step "Step 2: Starting Kafka"
cd "$PROJECT_ROOT"

if docker ps | grep -q kafka; then
    log_warn "Kafka already running, skipping start"
else
    log_info "Starting Kafka with docker-compose..."
    docker-compose -f docker-compose.benchmark.yml up -d

    log_info "Waiting for Kafka to be ready (30s)..."
    sleep 30

    if timeout 5 bash -c "echo > /dev/tcp/localhost/9092" 2>/dev/null; then
        log_info "✓ Kafka is ready"
    else
        log_error "Kafka failed to start"
        log_info "Check logs: docker-compose -f docker-compose.benchmark.yml logs kafka"
        exit 1
    fi
fi

# Step 3: Create topics
log_step "Step 3: Creating Kafka Topics"

# Function to create topic if it doesn't exist
create_topic() {
    local topic=$1
    local partitions=$2

    if kafka-topics --bootstrap-server localhost:9092 --list | grep -q "^${topic}$"; then
        log_warn "Topic $topic already exists"
    else
        log_info "Creating topic: $topic (partitions: $partitions)"
        kafka-topics --create \
            --topic "$topic" \
            --partitions "$partitions" \
            --replication-factor 1 \
            --bootstrap-server localhost:9092 || {
                log_error "Failed to create topic: $topic"
                exit 1
            }
        log_info "✓ Topic created: $topic"
    fi
}

create_topic "test-8p-input" 8
create_topic "test-8p-output" 8

# Step 4: Build Streamforge
log_step "Step 4: Building Streamforge"

if [[ -f "$PROJECT_ROOT/target/release/streamforge" ]]; then
    log_info "Binary already exists, checking if rebuild needed..."

    # Check if source files are newer than binary
    if find "$PROJECT_ROOT/src" -newer "$PROJECT_ROOT/target/release/streamforge" | grep -q .; then
        log_info "Source files changed, rebuilding..."
        cd "$PROJECT_ROOT"
        cargo build --release
    else
        log_info "✓ Using existing binary (up to date)"
    fi
else
    log_info "Building Streamforge (this may take a few minutes)..."
    cd "$PROJECT_ROOT"
    cargo build --release
    log_info "✓ Build complete"
fi

# Step 5: Run test
log_step "Step 5: Running Observability Test"

cd "$SCRIPT_DIR"
log_info ""
log_info "Starting observability test..."
log_info ""
log_info "The test will:"
log_info "  1. Start Streamforge with metrics enabled"
log_info "  2. Show a live metrics dashboard"
log_info "  3. Collect performance data"
log_info ""
log_info "To generate load, open another terminal and run:"
log_info "  kafka-producer-perf-test --topic test-8p-input --num-records 10000 \\"
log_info "    --record-size 512 --throughput 1000 \\"
log_info "    --producer-props bootstrap.servers=localhost:9092"
log_info ""
log_info "Press Enter to continue..."
read

./run_observability_test.sh
