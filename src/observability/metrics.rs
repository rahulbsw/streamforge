use lazy_static::lazy_static;
use prometheus::{
    Counter, CounterVec, Encoder, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, Opts,
    Registry, TextEncoder,
};

/// Metric label value constants to avoid stringly-typed code
pub mod labels {
    pub const CONNECTION_TYPE_CONSUMER: &str = "consumer";
    pub const CONNECTION_TYPE_PRODUCER: &str = "producer";

    pub const ERROR_TYPE_PARSE: &str = "parse_error";
    pub const ERROR_TYPE_PROCESSING: &str = "processing_error";
    pub const ERROR_TYPE_KAFKA: &str = "kafka_error";

    pub const FILTER_RESULT_PASS: &str = "pass";
    pub const FILTER_RESULT_FAIL: &str = "fail";
    pub const FILTER_REASON_FAILED: &str = "filter_failed";

    pub const TRANSFORM_TYPE_ENVELOPE: &str = "envelope";
    pub const TRANSFORM_TYPE_VALUE: &str = "value";
}

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    pub static ref METRICS: Metrics = Metrics::new();
}

/// Central metrics structure
pub struct Metrics {
    // Message processing counters
    pub messages_consumed: Counter,
    pub messages_produced: CounterVec,
    pub messages_filtered: CounterVec,
    pub processing_errors: CounterVec,

    // Processing latency
    pub processing_duration: HistogramVec,
    pub batch_processing_duration: Histogram,

    // Processing rate and in-flight
    pub processing_rate: Gauge,
    pub messages_in_flight: Gauge,

    // Filter metrics
    pub filter_evaluations: CounterVec,
    pub filter_duration: HistogramVec,
    pub filter_errors: CounterVec,

    // Transform metrics
    pub transform_operations: CounterVec,
    pub transform_duration: HistogramVec,
    pub transform_errors: CounterVec,

    // Envelope operation metrics
    pub key_transforms: CounterVec,
    pub header_operations: CounterVec,
    pub timestamp_operations: CounterVec,

    // Kafka consumer lag metrics
    pub consumer_lag: GaugeVec,
    pub consumer_offset: GaugeVec,
    pub consumer_high_watermark: GaugeVec,
    pub time_since_last_commit: Gauge,

    // System health
    pub uptime_seconds: Gauge,
    pub kafka_connections: GaugeVec,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            messages_consumed: Counter::new(
                "streamforge_messages_consumed_total",
                "Total messages consumed from source Kafka",
            )
            .unwrap(),

            messages_produced: CounterVec::new(
                Opts::new(
                    "streamforge_messages_produced_total",
                    "Messages successfully produced to destinations",
                ),
                &["destination"],
            )
            .unwrap(),

            messages_filtered: CounterVec::new(
                Opts::new(
                    "streamforge_messages_filtered_total",
                    "Messages filtered out per destination",
                ),
                &["destination", "reason"],
            )
            .unwrap(),

            processing_errors: CounterVec::new(
                Opts::new(
                    "streamforge_processing_errors_total",
                    "Processing errors by type",
                ),
                &["type"],
            )
            .unwrap(),

            processing_duration: HistogramVec::new(
                HistogramOpts::new(
                    "streamforge_processing_duration_seconds",
                    "End-to-end processing latency per destination",
                )
                .buckets(vec![
                    0.0001, 0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
                    2.5,
                ]),
                &["destination"],
            )
            .unwrap(),

            batch_processing_duration: Histogram::with_opts(
                HistogramOpts::new(
                    "streamforge_batch_processing_duration_seconds",
                    "Batch processing duration",
                )
                .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0]),
            )
            .unwrap(),

            processing_rate: Gauge::new(
                "streamforge_processing_rate_mps",
                "Current processing rate (messages per second)",
            )
            .unwrap(),

            messages_in_flight: Gauge::new(
                "streamforge_messages_in_flight",
                "Messages currently being processed",
            )
            .unwrap(),

            filter_evaluations: CounterVec::new(
                Opts::new(
                    "streamforge_filter_evaluations_total",
                    "Filter evaluations by result",
                ),
                &["destination", "result"],
            )
            .unwrap(),

            filter_duration: HistogramVec::new(
                HistogramOpts::new(
                    "streamforge_filter_duration_seconds",
                    "Filter evaluation duration",
                )
                .buckets(vec![
                    0.00001, 0.00005, 0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05,
                ]),
                &["filter_type"],
            )
            .unwrap(),

            filter_errors: CounterVec::new(
                Opts::new(
                    "streamforge_filter_errors_total",
                    "Filter evaluation errors",
                ),
                &["destination"],
            )
            .unwrap(),

            transform_operations: CounterVec::new(
                Opts::new(
                    "streamforge_transform_operations_total",
                    "Transform operations by type",
                ),
                &["destination", "transform_type"],
            )
            .unwrap(),

            transform_duration: HistogramVec::new(
                HistogramOpts::new(
                    "streamforge_transform_duration_seconds",
                    "Transform operation duration",
                )
                .buckets(vec![
                    0.00001, 0.00005, 0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05,
                ]),
                &["transform_type"],
            )
            .unwrap(),

            transform_errors: CounterVec::new(
                Opts::new(
                    "streamforge_transform_errors_total",
                    "Transform operation errors",
                ),
                &["destination", "transform_type"],
            )
            .unwrap(),

            key_transforms: CounterVec::new(
                Opts::new(
                    "streamforge_key_transforms_total",
                    "Key transformation operations",
                ),
                &["destination", "operation"],
            )
            .unwrap(),

            header_operations: CounterVec::new(
                Opts::new(
                    "streamforge_header_operations_total",
                    "Header operations (set/copy/remove/from)",
                ),
                &["destination", "operation"],
            )
            .unwrap(),

            timestamp_operations: CounterVec::new(
                Opts::new(
                    "streamforge_timestamp_operations_total",
                    "Timestamp operations",
                ),
                &["destination", "operation"],
            )
            .unwrap(),

            consumer_lag: GaugeVec::new(
                Opts::new("streamforge_consumer_lag", "Consumer lag per partition"),
                &["topic", "partition"],
            )
            .unwrap(),

            consumer_offset: GaugeVec::new(
                Opts::new(
                    "streamforge_consumer_offset",
                    "Current consumer offset per partition",
                ),
                &["topic", "partition"],
            )
            .unwrap(),

            consumer_high_watermark: GaugeVec::new(
                Opts::new(
                    "streamforge_consumer_high_watermark",
                    "High watermark per partition",
                ),
                &["topic", "partition"],
            )
            .unwrap(),

            time_since_last_commit: Gauge::new(
                "streamforge_time_since_last_commit_seconds",
                "Time since last offset commit",
            )
            .unwrap(),

            uptime_seconds: Gauge::new("streamforge_uptime_seconds", "Service uptime in seconds")
                .unwrap(),

            kafka_connections: GaugeVec::new(
                Opts::new("streamforge_kafka_connections", "Active Kafka connections"),
                &["type"],
            )
            .unwrap(),
        }
    }
}

/// Register all metrics with the registry
pub fn register_metrics() -> Result<(), Box<dyn std::error::Error>> {
    REGISTRY.register(Box::new(METRICS.messages_consumed.clone()))?;
    REGISTRY.register(Box::new(METRICS.messages_produced.clone()))?;
    REGISTRY.register(Box::new(METRICS.messages_filtered.clone()))?;
    REGISTRY.register(Box::new(METRICS.processing_errors.clone()))?;
    REGISTRY.register(Box::new(METRICS.processing_duration.clone()))?;
    REGISTRY.register(Box::new(METRICS.batch_processing_duration.clone()))?;
    REGISTRY.register(Box::new(METRICS.processing_rate.clone()))?;
    REGISTRY.register(Box::new(METRICS.messages_in_flight.clone()))?;
    REGISTRY.register(Box::new(METRICS.filter_evaluations.clone()))?;
    REGISTRY.register(Box::new(METRICS.filter_duration.clone()))?;
    REGISTRY.register(Box::new(METRICS.filter_errors.clone()))?;
    REGISTRY.register(Box::new(METRICS.transform_operations.clone()))?;
    REGISTRY.register(Box::new(METRICS.transform_duration.clone()))?;
    REGISTRY.register(Box::new(METRICS.transform_errors.clone()))?;
    REGISTRY.register(Box::new(METRICS.key_transforms.clone()))?;
    REGISTRY.register(Box::new(METRICS.header_operations.clone()))?;
    REGISTRY.register(Box::new(METRICS.timestamp_operations.clone()))?;
    REGISTRY.register(Box::new(METRICS.consumer_lag.clone()))?;
    REGISTRY.register(Box::new(METRICS.consumer_offset.clone()))?;
    REGISTRY.register(Box::new(METRICS.consumer_high_watermark.clone()))?;
    REGISTRY.register(Box::new(METRICS.time_since_last_commit.clone()))?;
    REGISTRY.register(Box::new(METRICS.uptime_seconds.clone()))?;
    REGISTRY.register(Box::new(METRICS.kafka_connections.clone()))?;

    Ok(())
}

/// Get metrics in Prometheus text format
pub fn metrics_text() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .expect("Prometheus text encoding failed");
    String::from_utf8(buffer).expect("Prometheus output contained invalid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new();
        metrics.messages_consumed.inc();
        assert_eq!(metrics.messages_consumed.get(), 1.0);
    }

    #[test]
    fn test_metrics_with_labels() {
        let metrics = Metrics::new();
        metrics
            .messages_produced
            .with_label_values(&["test-destination"])
            .inc();
        // Verify metric exists (Prometheus doesn't expose easy way to check value with labels)
    }

    #[test]
    fn test_histogram_observe() {
        let metrics = Metrics::new();
        metrics
            .processing_duration
            .with_label_values(&["test-dest"])
            .observe(0.025);
        // Should not panic
    }
}
