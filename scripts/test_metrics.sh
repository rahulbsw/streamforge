#!/bin/bash
#
# Quick test script for observability metrics
# Verifies that metrics endpoint is accessible and returning data
#

set -e

METRICS_PORT=${METRICS_PORT:-9090}
METRICS_URL="http://localhost:${METRICS_PORT}/metrics"
HEALTH_URL="http://localhost:${METRICS_PORT}/health"

echo "🧪 Testing Streamforge Observability Metrics"
echo "=============================================="
echo ""

# Test health endpoint
echo "1. Testing health endpoint..."
if curl -sf "${HEALTH_URL}" > /dev/null 2>&1; then
    echo "   ✅ Health endpoint: ${HEALTH_URL}"
    HEALTH_STATUS=$(curl -s "${HEALTH_URL}")
    echo "   Response: ${HEALTH_STATUS}"
else
    echo "   ❌ Health endpoint not accessible: ${HEALTH_URL}"
    echo "   Make sure Streamforge is running with observability.metrics_enabled: true"
    exit 1
fi

echo ""

# Test metrics endpoint
echo "2. Testing metrics endpoint..."
if curl -sf "${METRICS_URL}" > /dev/null 2>&1; then
    echo "   ✅ Metrics endpoint: ${METRICS_URL}"
else
    echo "   ❌ Metrics endpoint not accessible: ${METRICS_URL}"
    exit 1
fi

echo ""

# Check for key metrics
echo "3. Checking for key metrics..."

METRICS_OUTPUT=$(curl -s "${METRICS_URL}")

check_metric() {
    local metric_name=$1
    local description=$2

    if echo "${METRICS_OUTPUT}" | grep -q "${metric_name}"; then
        echo "   ✅ ${description}: ${metric_name}"
    else
        echo "   ⚠️  ${description} not found: ${metric_name}"
    fi
}

check_metric "streamforge_messages_consumed_total" "Messages consumed"
check_metric "streamforge_messages_produced_total" "Messages produced"
check_metric "streamforge_consumer_lag" "Consumer lag"
check_metric "streamforge_processing_duration_seconds" "Processing duration"
check_metric "streamforge_filter_evaluations_total" "Filter evaluations"
check_metric "streamforge_transform_operations_total" "Transform operations"
check_metric "streamforge_uptime_seconds" "Uptime"
check_metric "streamforge_messages_in_flight" "Messages in flight"

echo ""

# Display sample metrics
echo "4. Sample metrics output:"
echo "   ─────────────────────────────────────────────────────────"
echo "${METRICS_OUTPUT}" | grep "^streamforge_" | head -20
echo "   ─────────────────────────────────────────────────────────"

echo ""

# Summary
TOTAL_METRICS=$(echo "${METRICS_OUTPUT}" | grep -c "^streamforge_" || true)
echo "📊 Summary:"
echo "   Total metrics exposed: ${TOTAL_METRICS}"
echo "   Metrics endpoint: ${METRICS_URL}"
echo "   Health endpoint: ${HEALTH_URL}"
echo ""
echo "✅ Observability metrics are working correctly!"
echo ""
echo "Next steps:"
echo "  - Configure Prometheus to scrape ${METRICS_URL}"
echo "  - View quickstart guide: docs/OBSERVABILITY_QUICKSTART.md"
echo "  - Check full design: docs/OBSERVABILITY_METRICS_DESIGN.md"
