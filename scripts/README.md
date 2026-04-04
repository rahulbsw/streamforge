# Streamforge Scripts

Utility scripts for testing and managing Streamforge.

## test_metrics.sh

Quick verification script for observability metrics.

**Usage:**
```bash
# Start Streamforge with observability enabled
CONFIG_FILE=examples/config.with-observability.yaml ./target/release/streamforge

# In another terminal, run the test script
./scripts/test_metrics.sh
```

**What it checks:**
- ✅ Health endpoint is accessible
- ✅ Metrics endpoint is accessible  
- ✅ Key metrics are being exposed
- ✅ Displays sample metrics output

**Environment variables:**
- `METRICS_PORT` - Override metrics port (default: 9090)

**Example output:**
```
🧪 Testing Streamforge Observability Metrics
==============================================

1. Testing health endpoint...
   ✅ Health endpoint: http://localhost:9090/health
   Response: OK

2. Testing metrics endpoint...
   ✅ Metrics endpoint: http://localhost:9090/metrics

3. Checking for key metrics...
   ✅ Messages consumed: streamforge_messages_consumed_total
   ✅ Messages produced: streamforge_messages_produced_total
   ✅ Consumer lag: streamforge_consumer_lag
   ✅ Processing duration: streamforge_processing_duration_seconds
   ...

📊 Summary:
   Total metrics exposed: 45
   Metrics endpoint: http://localhost:9090/metrics
   
✅ Observability metrics are working correctly!
```
