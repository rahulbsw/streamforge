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
    pub const FILTER_REASON_ERROR: &str = "error";

    pub const TRANSFORM_TYPE_ENVELOPE: &str = "envelope";
    pub const TRANSFORM_TYPE_VALUE: &str = "value";

    pub const WASM_KIND_FILTER: &str = "filter";
    pub const WASM_KIND_VALUE_TRANSFORM: &str = "value_transform";
    pub const WASM_KIND_ENVELOPE_TRANSFORM: &str = "envelope_transform";

    pub const WASM_STATUS_OK: &str = "ok";
    pub const WASM_STATUS_GUEST_ERROR: &str = "guest_error";
    pub const WASM_STATUS_TRAP: &str = "trap";
    pub const WASM_STATUS_TIMEOUT: &str = "timeout";
    pub const WASM_STATUS_RESOURCE_LIMIT: &str = "resource_limit";
    pub const WASM_STATUS_INVALID_OUTPUT: &str = "invalid_output";

    pub const AGGREGATION_UPDATE_STATUS_ACCEPTED: &str = "accepted";
    pub const AGGREGATION_UPDATE_STATUS_REJECTED: &str = "rejected";

    pub const AGGREGATION_FLUSH_STATUS_EMITTED: &str = "emitted";
    pub const AGGREGATION_FLUSH_STATUS_EMPTY: &str = "empty";
    pub const AGGREGATION_FLUSH_STATUS_FAILED: &str = "failed";
}

lazy_static! {
    pub static ref METRICS: Metrics = Metrics::new();
    pub static ref REGISTRY: Registry = {
        let registry = Registry::new();
        register_metrics_in(&registry, &METRICS)
            .expect("StreamForge metrics must have unique names and valid descriptors");
        registry
    };
}

/// Central metrics structure
pub struct Metrics {
    // Message processing counters
    pub messages_consumed: Counter,
    pub messages_produced: CounterVec,
    pub messages_delivered: CounterVec,
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

    // WebAssembly UDF metrics. Module and kind labels are configuration-time
    // values; status is restricted to the constants above.
    pub wasm_invocations: CounterVec,
    pub wasm_duration: HistogramVec,
    pub wasm_input_bytes: HistogramVec,
    pub wasm_output_bytes: HistogramVec,
    pub wasm_active_invocations: GaugeVec,
    pub wasm_compilations: CounterVec,
    pub wasm_compilation_duration: HistogramVec,

    // Envelope operation metrics
    pub key_transforms: CounterVec,
    pub header_operations: CounterVec,
    pub timestamp_operations: CounterVec,

    // Kafka consumer lag metrics
    pub consumer_lag: GaugeVec,
    pub consumer_offset: GaugeVec,
    pub consumer_high_watermark: GaugeVec,
    pub time_since_last_commit: Gauge,

    // Aggregation metrics
    pub aggregation_updates: CounterVec,
    pub aggregation_windows_open: GaugeVec,
    pub aggregation_flushes: CounterVec,
    pub aggregation_records_emitted: CounterVec,

    // System health
    pub build_info: GaugeVec,
    pub ready: Gauge,
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

            messages_delivered: CounterVec::new(
                Opts::new(
                    "streamforge_messages_delivered_total",
                    "Messages acknowledged by destination Kafka",
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

            wasm_invocations: CounterVec::new(
                Opts::new(
                    "streamforge_wasm_invocations_total",
                    "WebAssembly UDF invocations by configured module, kind, and bounded status",
                ),
                &["module", "kind", "status"],
            )
            .unwrap(),

            wasm_duration: HistogramVec::new(
                HistogramOpts::new(
                    "streamforge_wasm_duration_seconds",
                    "WebAssembly UDF invocation duration",
                )
                .buckets(vec![
                    0.000_001, 0.000_005, 0.000_01, 0.000_05, 0.000_1, 0.000_5, 0.001, 0.0025,
                    0.005, 0.01, 0.025, 0.05,
                ]),
                &["module", "kind"],
            )
            .unwrap(),

            wasm_input_bytes: HistogramVec::new(
                HistogramOpts::new(
                    "streamforge_wasm_input_bytes",
                    "Serialized input bytes passed to WebAssembly UDFs",
                )
                .buckets(vec![
                    256.0,
                    1_024.0,
                    4_096.0,
                    16_384.0,
                    65_536.0,
                    262_144.0,
                    1_048_576.0,
                ]),
                &["module", "kind"],
            )
            .unwrap(),

            wasm_output_bytes: HistogramVec::new(
                HistogramOpts::new(
                    "streamforge_wasm_output_bytes",
                    "Serialized output bytes returned by WebAssembly UDFs",
                )
                .buckets(vec![
                    256.0,
                    1_024.0,
                    4_096.0,
                    16_384.0,
                    65_536.0,
                    262_144.0,
                    1_048_576.0,
                ]),
                &["module", "kind"],
            )
            .unwrap(),

            wasm_active_invocations: GaugeVec::new(
                Opts::new(
                    "streamforge_wasm_active_invocations",
                    "WebAssembly UDF invocations currently executing",
                ),
                &["module", "kind"],
            )
            .unwrap(),

            wasm_compilations: CounterVec::new(
                Opts::new(
                    "streamforge_wasm_compilations_total",
                    "WebAssembly component startup compilation attempts",
                ),
                &["module", "status"],
            )
            .unwrap(),

            wasm_compilation_duration: HistogramVec::new(
                HistogramOpts::new(
                    "streamforge_wasm_compilation_duration_seconds",
                    "WebAssembly component verification and compilation duration at startup",
                )
                .buckets(vec![
                    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
                ]),
                &["module"],
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

            aggregation_updates: CounterVec::new(
                Opts::new(
                    "streamforge_aggregation_updates_total",
                    "Aggregation update attempts by destination, metric, and status",
                ),
                &["destination", "metric", "status"],
            )
            .unwrap(),

            aggregation_windows_open: GaugeVec::new(
                Opts::new(
                    "streamforge_aggregation_windows_open",
                    "Currently open aggregation windows per destination",
                ),
                &["destination"],
            )
            .unwrap(),

            aggregation_flushes: CounterVec::new(
                Opts::new(
                    "streamforge_aggregation_flushes_total",
                    "Aggregation flush attempts by destination and status",
                ),
                &["destination", "status"],
            )
            .unwrap(),

            aggregation_records_emitted: CounterVec::new(
                Opts::new(
                    "streamforge_aggregation_records_emitted_total",
                    "Aggregation records emitted to destination topics",
                ),
                &["destination"],
            )
            .unwrap(),

            build_info: {
                let metric = GaugeVec::new(
                    Opts::new(
                        "streamforge_build_info",
                        "StreamForge build information; the value is always 1",
                    ),
                    &["version"],
                )
                .unwrap();
                metric
                    .with_label_values(&[env!("CARGO_PKG_VERSION")])
                    .set(1.0);
                metric
            },

            ready: Gauge::new(
                "streamforge_ready",
                "Whether StreamForge is ready to process Kafka records (1 ready, 0 not ready)",
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

fn register_metrics_in(registry: &Registry, metrics: &Metrics) -> prometheus::Result<()> {
    registry.register(Box::new(metrics.messages_consumed.clone()))?;
    registry.register(Box::new(metrics.messages_produced.clone()))?;
    registry.register(Box::new(metrics.messages_delivered.clone()))?;
    registry.register(Box::new(metrics.messages_filtered.clone()))?;
    registry.register(Box::new(metrics.processing_errors.clone()))?;
    registry.register(Box::new(metrics.processing_duration.clone()))?;
    registry.register(Box::new(metrics.batch_processing_duration.clone()))?;
    registry.register(Box::new(metrics.processing_rate.clone()))?;
    registry.register(Box::new(metrics.messages_in_flight.clone()))?;
    registry.register(Box::new(metrics.filter_evaluations.clone()))?;
    registry.register(Box::new(metrics.filter_duration.clone()))?;
    registry.register(Box::new(metrics.filter_errors.clone()))?;
    registry.register(Box::new(metrics.transform_operations.clone()))?;
    registry.register(Box::new(metrics.transform_duration.clone()))?;
    registry.register(Box::new(metrics.transform_errors.clone()))?;
    registry.register(Box::new(metrics.wasm_invocations.clone()))?;
    registry.register(Box::new(metrics.wasm_duration.clone()))?;
    registry.register(Box::new(metrics.wasm_input_bytes.clone()))?;
    registry.register(Box::new(metrics.wasm_output_bytes.clone()))?;
    registry.register(Box::new(metrics.wasm_active_invocations.clone()))?;
    registry.register(Box::new(metrics.wasm_compilations.clone()))?;
    registry.register(Box::new(metrics.wasm_compilation_duration.clone()))?;
    registry.register(Box::new(metrics.key_transforms.clone()))?;
    registry.register(Box::new(metrics.header_operations.clone()))?;
    registry.register(Box::new(metrics.timestamp_operations.clone()))?;
    registry.register(Box::new(metrics.consumer_lag.clone()))?;
    registry.register(Box::new(metrics.consumer_offset.clone()))?;
    registry.register(Box::new(metrics.consumer_high_watermark.clone()))?;
    registry.register(Box::new(metrics.time_since_last_commit.clone()))?;
    registry.register(Box::new(metrics.aggregation_updates.clone()))?;
    registry.register(Box::new(metrics.aggregation_windows_open.clone()))?;
    registry.register(Box::new(metrics.aggregation_flushes.clone()))?;
    registry.register(Box::new(metrics.aggregation_records_emitted.clone()))?;
    registry.register(Box::new(metrics.build_info.clone()))?;
    registry.register(Box::new(metrics.ready.clone()))?;
    registry.register(Box::new(metrics.uptime_seconds.clone()))?;
    registry.register(Box::new(metrics.kafka_connections.clone()))?;

    Ok(())
}

/// Initialize the process-wide registry.
///
/// Registration happens once when the lazy registry is created, so repeated
/// calls are safe for embedders and tests.
pub fn register_metrics() -> Result<(), Box<dyn std::error::Error>> {
    lazy_static::initialize(&REGISTRY);
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

    #[test]
    fn test_aggregation_metrics_with_labels() {
        let metrics = Metrics::new();
        metrics
            .aggregation_updates
            .with_label_values(&[
                "orders-metrics-1m",
                "order_count",
                labels::AGGREGATION_UPDATE_STATUS_ACCEPTED,
            ])
            .inc();
        metrics
            .aggregation_windows_open
            .with_label_values(&["orders-metrics-1m"])
            .set(2.0);
        metrics
            .aggregation_flushes
            .with_label_values(&[
                "orders-metrics-1m",
                labels::AGGREGATION_FLUSH_STATUS_EMITTED,
            ])
            .inc();
        metrics
            .aggregation_records_emitted
            .with_label_values(&["orders-metrics-1m"])
            .inc_by(3.0);
    }

    #[test]
    fn test_wasm_metrics_use_bounded_configuration_labels() {
        let metrics = Metrics::new();
        metrics
            .wasm_invocations
            .with_label_values(&[
                "redact",
                labels::WASM_KIND_VALUE_TRANSFORM,
                labels::WASM_STATUS_OK,
            ])
            .inc();
        metrics
            .wasm_duration
            .with_label_values(&["redact", labels::WASM_KIND_VALUE_TRANSFORM])
            .observe(0.000_1);
        metrics
            .wasm_active_invocations
            .with_label_values(&["redact", labels::WASM_KIND_VALUE_TRANSFORM])
            .set(1.0);
    }

    #[test]
    fn registry_contains_build_and_readiness_metrics() {
        let registry = Registry::new();
        let metrics = Metrics::new();
        register_metrics_in(&registry, &metrics).unwrap();

        let names: Vec<_> = registry
            .gather()
            .into_iter()
            .map(|family| family.name().to_string())
            .collect();

        assert!(names.contains(&"streamforge_build_info".to_string()));
        assert!(names.contains(&"streamforge_ready".to_string()));
    }

    #[test]
    fn duplicate_registration_is_rejected_without_partial_global_state() {
        let registry = Registry::new();
        let metrics = Metrics::new();
        register_metrics_in(&registry, &metrics).unwrap();

        assert!(register_metrics_in(&registry, &metrics).is_err());
    }

    #[test]
    fn process_registry_initialization_is_idempotent() {
        register_metrics().unwrap();
        register_metrics().unwrap();
    }
}
