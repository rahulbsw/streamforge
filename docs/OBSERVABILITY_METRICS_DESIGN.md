# Observability Metrics Design

**Status**: Design Proposal  
**Date**: April 3, 2026  
**Author**: Based on user request for enhanced observability

---

## Problem Statement

Current metrics implementation is limited:
- Only logs periodic stats to console
- No per-destination metrics
- No filter/transform success/failure rates
- No Kafka lag visibility
- No standard metrics format (Prometheus/OpenTelemetry)
- Limited operational visibility

**Production observability gaps:**
- Cannot alert on specific destination failures
- Cannot track filter effectiveness
- Cannot detect processing bottlenecks
- Cannot monitor Kafka consumer lag
- Cannot integrate with standard monitoring stacks (Prometheus, Grafana, Datadog)

---

## Goals

1. **Expose Prometheus metrics** on HTTP endpoint (e.g., `/metrics`)
2. **Track comprehensive pipeline metrics** (consumption, filtering, routing, errors)
3. **Per-destination metrics** (throughput, latency, errors)
4. **Kafka consumer lag** monitoring
5. **Envelope operation metrics** (key/header/timestamp transforms)
6. **Maintain backward compatibility** (existing log-based metrics still work)
7. **Low overhead** (< 2% performance impact)

---

## Proposed Metrics

### 1. Message Processing Metrics

#### Counters

```prometheus
# Total messages consumed from source Kafka
streamforge_messages_consumed_total{topic="input-topic"}

# Messages successfully produced to destinations
streamforge_messages_produced_total{destination="output-topic"}

# Messages filtered out (did not pass filter)
streamforge_messages_filtered_total{destination="output-topic", reason="filter_failed"}

# Transformation errors
streamforge_transform_errors_total{destination="output-topic", type="value|key|header|timestamp"}

# Processing errors
streamforge_processing_errors_total{type="parse_error|sink_error|filter_error"}
```

#### Gauges

```prometheus
# Current processing rate (messages/second)
streamforge_processing_rate_mps

# Messages in flight (being processed)
streamforge_messages_in_flight
```

#### Histograms

```prometheus
# End-to-end processing latency (consume → produce)
streamforge_processing_duration_seconds{destination="output-topic"}

# Batch processing duration
streamforge_batch_processing_duration_seconds
```

### 2. Per-Destination Metrics

```prometheus
# Messages routed to each destination
streamforge_destination_messages_total{destination="output-topic"}

# Destination processing errors
streamforge_destination_errors_total{destination="output-topic", error_type="filter|transform|sink"}

# Messages filtered out per destination
streamforge_destination_filtered_total{destination="output-topic"}

# Destination processing latency
streamforge_destination_latency_seconds{destination="output-topic", quantile="0.5|0.9|0.99"}
```

### 3. Filter Metrics

```prometheus
# Filter evaluations
streamforge_filter_evaluations_total{destination="output-topic", result="pass|fail"}

# Filter evaluation duration
streamforge_filter_duration_seconds{filter_type="value|key|header|timestamp"}

# Filter errors
streamforge_filter_errors_total{destination="output-topic"}
```

### 4. Transform Metrics

```prometheus
# Transform operations
streamforge_transform_operations_total{destination="output-topic", transform_type="value|key|header|timestamp"}

# Transform duration
streamforge_transform_duration_seconds{transform_type="value|key|header|timestamp"}

# Transform errors by type
streamforge_transform_errors_by_type_total{destination="output-topic", transform_type="value|key|header|timestamp"}
```

### 5. Envelope Operation Metrics

```prometheus
# Key transformations
streamforge_key_transforms_total{destination="output-topic", operation="extract|construct|hash|template"}

# Header operations
streamforge_header_operations_total{destination="output-topic", operation="set|copy|remove|from"}

# Timestamp operations
streamforge_timestamp_operations_total{destination="output-topic", operation="preserve|current|from|add|subtract"}
```

### 6. Kafka Consumer Lag Metrics

```prometheus
# Consumer lag per partition
streamforge_consumer_lag{topic="input-topic", partition="0"}

# Consumer offset
streamforge_consumer_offset{topic="input-topic", partition="0"}

# High water mark
streamforge_consumer_high_watermark{topic="input-topic", partition="0"}

# Time since last commit
streamforge_consumer_time_since_last_commit_seconds
```

### 7. System Health Metrics

```prometheus
# Service uptime
streamforge_uptime_seconds

# Active consumer connections
streamforge_kafka_connections{type="consumer|producer"}

# Thread pool utilization
streamforge_thread_pool_active_threads
streamforge_thread_pool_total_threads
```

---

## Implementation Plan

### Phase 1: Prometheus Integration (Core)

#### 1.1 Add Dependencies

```toml
# Cargo.toml additions
[dependencies]
# Metrics
prometheus = { version = "0.13", features = ["process"] }
lazy_static = "1.4"

# HTTP server for /metrics endpoint
axum = "0.7"
tower = "0.4"
```

#### 1.2 Create Metrics Module

**File: `src/observability/mod.rs`**

```rust
use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec,
    Registry, TextEncoder, Encoder, Opts, HistogramOpts,
};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    
    // Message processing counters
    pub static ref MESSAGES_CONSUMED: Counter = Counter::new(
        "streamforge_messages_consumed_total",
        "Total messages consumed from source Kafka"
    ).unwrap();
    
    pub static ref MESSAGES_PRODUCED: CounterVec = CounterVec::new(
        Opts::new(
            "streamforge_messages_produced_total",
            "Messages successfully produced to destinations"
        ),
        &["destination"]
    ).unwrap();
    
    pub static ref MESSAGES_FILTERED: CounterVec = CounterVec::new(
        Opts::new(
            "streamforge_messages_filtered_total",
            "Messages filtered out per destination"
        ),
        &["destination", "reason"]
    ).unwrap();
    
    pub static ref PROCESSING_ERRORS: CounterVec = CounterVec::new(
        Opts::new(
            "streamforge_processing_errors_total",
            "Processing errors by type"
        ),
        &["type"]
    ).unwrap();
    
    // Processing latency histogram
    pub static ref PROCESSING_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "streamforge_processing_duration_seconds",
            "End-to-end processing latency"
        )
        .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
        &["destination"]
    ).unwrap();
    
    // Filter metrics
    pub static ref FILTER_EVALUATIONS: CounterVec = CounterVec::new(
        Opts::new(
            "streamforge_filter_evaluations_total",
            "Filter evaluations by result"
        ),
        &["destination", "result"]
    ).unwrap();
    
    // Transform metrics
    pub static ref TRANSFORM_OPERATIONS: CounterVec = CounterVec::new(
        Opts::new(
            "streamforge_transform_operations_total",
            "Transform operations by type"
        ),
        &["destination", "transform_type"]
    ).unwrap();
    
    // Kafka consumer lag
    pub static ref CONSUMER_LAG: GaugeVec = GaugeVec::new(
        Opts::new(
            "streamforge_consumer_lag",
            "Consumer lag per partition"
        ),
        &["topic", "partition"]
    ).unwrap();
    
    pub static ref CONSUMER_OFFSET: GaugeVec = GaugeVec::new(
        Opts::new(
            "streamforge_consumer_offset",
            "Current consumer offset"
        ),
        &["topic", "partition"]
    ).unwrap();
}

pub fn register_metrics() -> Result<(), Box<dyn std::error::Error>> {
    REGISTRY.register(Box::new(MESSAGES_CONSUMED.clone()))?;
    REGISTRY.register(Box::new(MESSAGES_PRODUCED.clone()))?;
    REGISTRY.register(Box::new(MESSAGES_FILTERED.clone()))?;
    REGISTRY.register(Box::new(PROCESSING_ERRORS.clone()))?;
    REGISTRY.register(Box::new(PROCESSING_DURATION.clone()))?;
    REGISTRY.register(Box::new(FILTER_EVALUATIONS.clone()))?;
    REGISTRY.register(Box::new(TRANSFORM_OPERATIONS.clone()))?;
    REGISTRY.register(Box::new(CONSUMER_LAG.clone()))?;
    REGISTRY.register(Box::new(CONSUMER_OFFSET.clone()))?;
    
    Ok(())
}

pub fn metrics_text() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
```

#### 1.3 HTTP Metrics Endpoint

**File: `src/observability/server.rs`**

```rust
use axum::{routing::get, Router};
use std::net::SocketAddr;

pub async fn start_metrics_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler));
    
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Metrics server listening on http://{}", addr);
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}

async fn metrics_handler() -> String {
    super::metrics_text()
}

async fn health_handler() -> &'static str {
    "OK"
}
```

#### 1.4 Instrument Code

**Update `src/main.rs`:**

```rust
use streamforge::observability::{register_metrics, start_metrics_server};
use streamforge::observability::{MESSAGES_CONSUMED, MESSAGES_PRODUCED};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize metrics
    register_metrics()?;
    
    // Start metrics server
    let metrics_port = 9090; // Configurable
    tokio::spawn(async move {
        if let Err(e) = start_metrics_server(metrics_port).await {
            error!("Metrics server error: {}", e);
        }
    });
    
    // ... existing code ...
    
    // In message processing loop:
    MESSAGES_CONSUMED.inc();
    
    // After successful processing:
    MESSAGES_PRODUCED.with_label_values(&[destination]).inc();
}
```

**Update `src/processor.rs`:**

```rust
use streamforge::observability::{
    FILTER_EVALUATIONS, TRANSFORM_OPERATIONS, PROCESSING_DURATION
};

impl DestinationProcessor {
    pub async fn process(&self, envelope: MessageEnvelope) -> Result<bool> {
        let timer = PROCESSING_DURATION
            .with_label_values(&[&self.name])
            .start_timer();
        
        // Filter evaluation
        let filter_passed = self.filter.evaluate_envelope(&envelope)?;
        
        FILTER_EVALUATIONS
            .with_label_values(&[&self.name, if filter_passed { "pass" } else { "fail" }])
            .inc();
        
        if !filter_passed {
            return Ok(false);
        }
        
        // Envelope transforms
        for transform in &self.envelope_transforms {
            TRANSFORM_OPERATIONS
                .with_label_values(&[&self.name, "envelope"])
                .inc();
            
            envelope = transform.transform_envelope(envelope)?;
        }
        
        // Value transform
        TRANSFORM_OPERATIONS
            .with_label_values(&[&self.name, "value"])
            .inc();
        
        // ... existing code ...
        
        timer.observe_duration();
        Ok(true)
    }
}
```

### Phase 2: Kafka Lag Monitoring

**File: `src/observability/lag_monitor.rs`**

```rust
use rdkafka::consumer::Consumer;
use rdkafka::TopicPartitionList;
use std::time::Duration;
use tokio::time::interval;

pub async fn monitor_consumer_lag(
    consumer: Arc<StreamConsumer>,
    topic: String,
) {
    let mut ticker = interval(Duration::from_secs(30));
    
    loop {
        ticker.tick().await;
        
        // Get current assignment
        let assignment = consumer.assignment().unwrap();
        
        for partition in assignment.elements() {
            let topic = partition.topic();
            let partition_id = partition.partition();
            
            // Get committed offset
            let committed = consumer
                .committed_offsets(TopicPartitionList::new(), Duration::from_secs(5))
                .unwrap();
            
            // Get high watermark
            let (low, high) = consumer
                .fetch_watermarks(topic, partition_id, Duration::from_secs(5))
                .unwrap();
            
            let current_offset = committed.find_partition(topic, partition_id)
                .and_then(|p| p.offset().to_raw())
                .unwrap_or(0);
            
            let lag = high - current_offset;
            
            CONSUMER_LAG
                .with_label_values(&[topic, &partition_id.to_string()])
                .set(lag as f64);
            
            CONSUMER_OFFSET
                .with_label_values(&[topic, &partition_id.to_string()])
                .set(current_offset as f64);
        }
    }
}
```

### Phase 3: Configuration

**Add to `config.rs`:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,
    
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
    
    #[serde(default)]
    pub metrics_path: String, // Default: "/metrics"
    
    #[serde(default = "default_lag_monitoring")]
    pub lag_monitoring_enabled: bool,
    
    #[serde(default = "default_lag_interval")]
    pub lag_monitoring_interval_secs: u64, // Default: 30
}

fn default_metrics_enabled() -> bool { true }
fn default_metrics_port() -> u16 { 9090 }
fn default_lag_monitoring() -> bool { true }
fn default_lag_interval() -> u64 { 30 }
```

**Example YAML config:**

```yaml
appid: streamforge
bootstrap: localhost:9092
input: input-topic
threads: 4

# Observability configuration
observability:
  metrics_enabled: true
  metrics_port: 9090
  metrics_path: "/metrics"
  lag_monitoring_enabled: true
  lag_monitoring_interval_secs: 30

routing:
  routing_type: filter
  destinations:
    - output: destination-1
      # ... filters and transforms ...
```

---

## Prometheus Queries

### Dashboard Queries

#### Message Throughput
```promql
# Messages consumed per second
rate(streamforge_messages_consumed_total[5m])

# Messages produced per destination
rate(streamforge_messages_produced_total[5m])

# Total throughput
sum(rate(streamforge_messages_produced_total[5m]))
```

#### Error Rate
```promql
# Error rate per second
rate(streamforge_processing_errors_total[5m])

# Error percentage
rate(streamforge_processing_errors_total[5m]) / rate(streamforge_messages_consumed_total[5m]) * 100
```

#### Filter Effectiveness
```promql
# Filter pass rate
rate(streamforge_filter_evaluations_total{result="pass"}[5m]) /
rate(streamforge_filter_evaluations_total[5m]) * 100

# Messages filtered out per destination
rate(streamforge_messages_filtered_total[5m])
```

#### Processing Latency
```promql
# P99 latency per destination
histogram_quantile(0.99, rate(streamforge_processing_duration_seconds_bucket[5m]))

# Average latency
rate(streamforge_processing_duration_seconds_sum[5m]) /
rate(streamforge_processing_duration_seconds_count[5m])
```

#### Consumer Lag
```promql
# Total lag across all partitions
sum(streamforge_consumer_lag)

# Max lag per partition
max(streamforge_consumer_lag) by (partition)

# Lag increasing (alert condition)
delta(streamforge_consumer_lag[5m]) > 1000
```

### Alerting Rules

```yaml
groups:
  - name: streamforge_alerts
    rules:
      - alert: HighErrorRate
        expr: rate(streamforge_processing_errors_total[5m]) > 10
        for: 2m
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value }} errors/sec"
      
      - alert: ConsumerLagIncreasing
        expr: delta(streamforge_consumer_lag[5m]) > 10000
        for: 5m
        annotations:
          summary: "Consumer lag increasing"
          description: "Lag increased by {{ $value }} in 5 minutes"
      
      - alert: DestinationDown
        expr: rate(streamforge_destination_errors_total[5m]) > 100
        for: 2m
        annotations:
          summary: "Destination {{ $labels.destination }} has high errors"
      
      - alert: HighLatency
        expr: histogram_quantile(0.99, rate(streamforge_processing_duration_seconds_bucket[5m])) > 1.0
        for: 5m
        annotations:
          summary: "P99 latency above 1 second"
```

---

## Grafana Dashboard

### Dashboard JSON (example panels)

```json
{
  "dashboard": {
    "title": "Streamforge Observability",
    "panels": [
      {
        "title": "Message Throughput",
        "targets": [{
          "expr": "rate(streamforge_messages_consumed_total[5m])",
          "legendFormat": "Consumed"
        }, {
          "expr": "sum(rate(streamforge_messages_produced_total[5m]))",
          "legendFormat": "Produced"
        }]
      },
      {
        "title": "Error Rate",
        "targets": [{
          "expr": "rate(streamforge_processing_errors_total[5m])",
          "legendFormat": "{{type}}"
        }]
      },
      {
        "title": "Consumer Lag",
        "targets": [{
          "expr": "streamforge_consumer_lag",
          "legendFormat": "{{topic}}-{{partition}}"
        }]
      },
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
    ]
  }
}
```

---

## OpenTelemetry Alternative

If OpenTelemetry is preferred over Prometheus:

### Dependencies

```toml
[dependencies]
opentelemetry = "0.21"
opentelemetry-otlp = "0.14"
opentelemetry-prometheus = "0.14"
opentelemetry_sdk = { version = "0.21", features = ["rt-tokio"] }
```

### OTEL Setup

```rust
use opentelemetry::global;
use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;

pub fn init_otel_metrics() -> Result<SdkMeterProvider> {
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint("http://localhost:4317");
    
    let provider = SdkMeterProvider::builder()
        .with_reader(
            opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
                .with_interval(Duration::from_secs(30))
                .build()
        )
        .build();
    
    global::set_meter_provider(provider.clone());
    Ok(provider)
}
```

**Pros of OTEL:**
- Vendor-neutral
- Unified traces + metrics + logs
- Growing ecosystem

**Cons of OTEL:**
- More complex setup
- Heavier dependencies
- Less mature Rust support

**Recommendation:** Start with Prometheus (simpler), add OTEL later if needed.

---

## Performance Impact

### Estimated Overhead

| Operation | Overhead | Mitigation |
|-----------|----------|------------|
| Counter increment | ~10 ns | Negligible |
| Histogram observation | ~200 ns | Use sampling for high-frequency |
| Label lookups | ~50 ns | Cache label vectors |
| HTTP /metrics scrape | N/A | Separate thread, non-blocking |
| Lag monitoring | ~10ms / 30s | Low frequency poll |

**Total estimated overhead: < 2%** with reasonable sampling

### Optimization Strategies

1. **Sample histograms** - Only record 10% of operations
2. **Batch updates** - Update gauges in batches
3. **Lazy label creation** - Cache frequently used label combinations
4. **Separate thread** - All metrics updates async

---

## Testing Plan

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metrics_increment() {
        MESSAGES_CONSUMED.inc();
        assert_eq!(MESSAGES_CONSUMED.get(), 1);
    }
    
    #[test]
    fn test_destination_metrics() {
        MESSAGES_PRODUCED.with_label_values(&["test-dest"]).inc();
        // Verify metric exists
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_metrics_endpoint() {
    start_metrics_server(9091).await;
    
    let response = reqwest::get("http://localhost:9091/metrics")
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let body = response.text().await.unwrap();
    assert!(body.contains("streamforge_messages_consumed_total"));
}
```

---

## Rollout Plan

### Phase 1: Core Metrics (Week 1)
- [ ] Add Prometheus dependency
- [ ] Implement basic counters (consumed, produced, errors)
- [ ] Add /metrics HTTP endpoint
- [ ] Update main processing loop

### Phase 2: Per-Destination Metrics (Week 1-2)
- [ ] Add destination-level metrics
- [ ] Instrument processor with labels
- [ ] Add filter/transform counters

### Phase 3: Lag Monitoring (Week 2)
- [ ] Implement lag monitoring background task
- [ ] Add lag gauges
- [ ] Test with multiple partitions

### Phase 4: Documentation & Dashboards (Week 2-3)
- [ ] Write configuration guide
- [ ] Create Grafana dashboard JSON
- [ ] Document Prometheus queries
- [ ] Add alerting examples

### Phase 5: Optional Enhancements (Week 3+)
- [ ] OpenTelemetry support
- [ ] Trace correlation
- [ ] Custom metric plugins

---

## Summary

This design provides:

✅ **Comprehensive observability** - All key metrics covered  
✅ **Standard format** - Prometheus-compatible  
✅ **Low overhead** - < 2% performance impact  
✅ **Production-ready** - Alerting and dashboards included  
✅ **Backward compatible** - Existing log metrics still work  
✅ **Extensible** - Can add OTEL later  

**Next Steps:**
1. Review and approve design
2. Create feature branch
3. Implement Phase 1 (core metrics)
4. Test with sample workload
5. Iterate on additional metrics
