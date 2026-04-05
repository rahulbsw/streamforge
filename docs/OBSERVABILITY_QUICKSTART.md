---
title: Observability
nav_order: 11
parent: Deployment
---

# Observability Quickstart

Get Prometheus metrics and Kafka lag monitoring running in 5 minutes.

## Quick Start

### 1. Enable Metrics in Config

Add to your `config.yaml`:

```yaml
observability:
  metrics_enabled: true
  metrics_port: 9090
  lag_monitoring_enabled: true
  lag_monitoring_interval_secs: 30
```

### 2. Start Streamforge

```bash
CONFIG_FILE=config.yaml ./streamforge
```

You'll see:
```
✅ Metrics registered successfully
🔍 Metrics server listening on http://0.0.0.0:9090
   Metrics endpoint: http://localhost:9090/metrics
   Health endpoint:  http://localhost:9090/health
✅ Consumer lag monitoring started (interval: 30s)
```

### 3. View Metrics

**Browser:**
```
http://localhost:9090/metrics
```

**curl:**
```bash
curl http://localhost:9090/metrics
```

**Sample Output:**
```prometheus
# HELP streamforge_messages_consumed_total Total messages consumed from source Kafka
# TYPE streamforge_messages_consumed_total counter
streamforge_messages_consumed_total 125000

# HELP streamforge_messages_produced_total Messages successfully produced to destinations
# TYPE streamforge_messages_produced_total counter
streamforge_messages_produced_total{destination="premium-events"} 45000
streamforge_messages_produced_total{destination="standard-events"} 80000

# HELP streamforge_consumer_lag Consumer lag per partition
# TYPE streamforge_consumer_lag gauge
streamforge_consumer_lag{topic="input-topic",partition="0"} 1250
streamforge_consumer_lag{topic="input-topic",partition="1"} 890

# HELP streamforge_processing_duration_seconds End-to-end processing latency per destination
# TYPE streamforge_processing_duration_seconds histogram
streamforge_processing_duration_seconds_bucket{destination="premium-events",le="0.001"} 35000
streamforge_processing_duration_seconds_bucket{destination="premium-events",le="0.005"} 43000
streamforge_processing_duration_seconds_bucket{destination="premium-events",le="0.01"} 44500
streamforge_processing_duration_seconds_bucket{destination="premium-events",le="+Inf"} 45000
streamforge_processing_duration_seconds_sum{destination="premium-events"} 67.5
streamforge_processing_duration_seconds_count{destination="premium-events"} 45000
```

## Prometheus Setup

### Add Scrape Config

Edit `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'streamforge'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
    scrape_timeout: 10s
```

### Start Prometheus

```bash
docker run -d \
  -p 9091:9090 \
  -v $(pwd)/prometheus.yml:/etc/prometheus/prometheus.yml \
  prom/prometheus
```

Access Prometheus UI: `http://localhost:9091`

## Quick Queries

### Message Throughput
```promql
# Messages per second
rate(streamforge_messages_consumed_total[5m])

# Per destination
sum(rate(streamforge_messages_produced_total[5m])) by (destination)
```

### Error Rate
```promql
# Errors per second
rate(streamforge_processing_errors_total[5m])

# Error percentage
rate(streamforge_processing_errors_total[5m]) / 
rate(streamforge_messages_consumed_total[5m]) * 100
```

### Consumer Lag
```promql
# Total lag
sum(streamforge_consumer_lag)

# Per partition
streamforge_consumer_lag

# Lag increasing (alert!)
delta(streamforge_consumer_lag[5m]) > 1000
```

### Processing Latency
```promql
# P99 latency
histogram_quantile(0.99, 
  rate(streamforge_processing_duration_seconds_bucket[5m])
)

# Average latency
rate(streamforge_processing_duration_seconds_sum[5m]) /
rate(streamforge_processing_duration_seconds_count[5m])
```

### Filter Effectiveness
```promql
# Pass rate percentage
rate(streamforge_filter_evaluations_total{result="pass"}[5m]) /
rate(streamforge_filter_evaluations_total[5m]) * 100

# Messages filtered out per destination
rate(streamforge_messages_filtered_total[5m])
```

## Grafana Dashboard

### Quick Dashboard JSON

Create a dashboard with these panels:

**Panel 1: Message Throughput**
```json
{
  "title": "Message Throughput",
  "targets": [{
    "expr": "rate(streamforge_messages_consumed_total[5m])",
    "legendFormat": "Consumed"
  }, {
    "expr": "sum(rate(streamforge_messages_produced_total[5m]))",
    "legendFormat": "Produced"
  }]
}
```

**Panel 2: Consumer Lag**
```json
{
  "title": "Consumer Lag by Partition",
  "targets": [{
    "expr": "streamforge_consumer_lag",
    "legendFormat": "{{topic}}-{{partition}}"
  }]
}
```

**Panel 3: Error Rate**
```json
{
  "title": "Error Rate",
  "targets": [{
    "expr": "rate(streamforge_processing_errors_total[5m])",
    "legendFormat": "{{type}}"
  }]
}
```

**Panel 4: Processing Latency**
```json
{
  "title": "Processing Latency (P50, P95, P99)",
  "targets": [
    {
      "expr": "histogram_quantile(0.50, rate(streamforge_processing_duration_seconds_bucket[5m]))",
      "legendFormat": "P50"
    },
    {
      "expr": "histogram_quantile(0.95, rate(streamforge_processing_duration_seconds_bucket[5m]))",
      "legendFormat": "P95"
    },
    {
      "expr": "histogram_quantile(0.99, rate(streamforge_processing_duration_seconds_bucket[5m]))",
      "legendFormat": "P99"
    }
  ]
}
```

Import to Grafana:
```bash
# Coming soon: Pre-built dashboard JSON
# Check examples/grafana-dashboard.json
```

## Alerting Rules

### prometheus-alerts.yml

```yaml
groups:
  - name: streamforge_alerts
    interval: 30s
    rules:
      # High error rate
      - alert: StreamforgeHighErrorRate
        expr: rate(streamforge_processing_errors_total[5m]) > 10
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "High error rate in Streamforge"
          description: "Error rate is {{ $value }} errors/sec"

      # Consumer lag increasing
      - alert: StreamforgeConsumerLagIncreasing
        expr: delta(streamforge_consumer_lag[5m]) > 10000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Consumer lag increasing"
          description: "Lag increased by {{ $value }} in 5 minutes"

      # High latency
      - alert: StreamforgeHighLatency
        expr: |
          histogram_quantile(0.99,
            rate(streamforge_processing_duration_seconds_bucket[5m])
          ) > 1.0
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "P99 latency above 1 second"

      # Service down
      - alert: StreamforgeDown
        expr: up{job="streamforge"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Streamforge service is down"
```

## Testing Locally

### 1. Generate Load

```bash
# Terminal 1: Start Streamforge
CONFIG_FILE=examples/config.with-observability.yaml ./streamforge

# Terminal 2: Produce test messages
kafka-console-producer.sh --topic input-topic --bootstrap-server localhost:9092
```

### 2. Watch Metrics

```bash
# Watch metrics update
watch -n 2 'curl -s http://localhost:9090/metrics | grep streamforge_messages'

# Check specific metric
curl -s http://localhost:9090/metrics | grep streamforge_consumer_lag
```

### 3. Verify Lag Monitoring

```bash
# Check lag metrics are updating
curl -s http://localhost:9090/metrics | grep consumer_lag

# Example output:
# streamforge_consumer_lag{topic="input-topic",partition="0"} 0
# streamforge_consumer_lag{topic="input-topic",partition="1"} 0
```

## Troubleshooting

### Metrics endpoint not accessible

**Check if server started:**
```bash
netstat -an | grep 9090
# Should show: tcp4  0  0  *.9090  *.*  LISTEN
```

**Check logs:**
```
2026-04-03T10:00:00Z INFO streamforge: Metrics server listening on http://0.0.0.0:9090
```

### No lag metrics

**Possible causes:**
1. No partitions assigned yet (consumer just started)
2. Lag monitoring disabled in config
3. Consumer group has no committed offsets

**Check:**
```bash
# Wait 30 seconds for first lag check
sleep 30

# Check metrics
curl http://localhost:9090/metrics | grep consumer_lag
```

### Metrics not updating

**Verify:**
1. Messages are being consumed (check logs)
2. Metrics are being incremented (check counter values)
3. Prometheus is scraping (check Prometheus UI → Targets)

## Next Steps

- [Full Design Document](OBSERVABILITY_METRICS_DESIGN.md) - Complete metrics reference
- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Dashboard Tutorial](https://grafana.com/docs/grafana/latest/dashboards/)
- See `examples/config.with-observability.yaml` for full config example

## Summary

You now have:
- ✅ Prometheus metrics exposed on `:9090/metrics`
- ✅ Kafka consumer lag monitoring
- ✅ Per-destination metrics (throughput, errors, latency)
- ✅ Filter and transform operation tracking
- ✅ Health check endpoint

**Total setup time:** < 5 minutes 🚀
