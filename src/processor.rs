use crate::filter::{EnvelopeTransform, Filter, IdentityTransform, PassThroughFilter, Transform};
use crate::kafka::sink::KafkaSink;
use crate::observability::{labels, METRICS};
use crate::{
    AggregateEmission, AggregationConfig, AggregationEngine, MessageEnvelope, MirrorMakerError,
    Result,
};
use prometheus::{Counter, Histogram};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error};

#[async_trait::async_trait]
pub trait SinkWriter: Send + Sync {
    async fn send(&self, envelope: MessageEnvelope) -> Result<()>;
    async fn flush(&self) -> Result<()>;
}

#[async_trait::async_trait]
impl SinkWriter for KafkaSink {
    async fn send(&self, envelope: MessageEnvelope) -> Result<()> {
        KafkaSink::send(self, envelope).await
    }

    async fn flush(&self) -> Result<()> {
        KafkaSink::flush(self).await
    }
}

/// Message processor trait
#[async_trait::async_trait]
pub trait MessageProcessor: Send + Sync {
    /// Process a message envelope
    async fn process(&self, envelope: MessageEnvelope) -> Result<()>;

    /// Flush buffered state or pending output.
    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}

/// Single-destination processor — optionally applies a value transform before sending.
pub struct SingleDestinationProcessor {
    sink: Arc<dyn SinkWriter>,
    transform: Option<Arc<dyn Transform>>,
}

impl SingleDestinationProcessor {
    pub fn new(sink: Arc<dyn SinkWriter>) -> Self {
        Self {
            sink,
            transform: None,
        }
    }

    pub fn with_transform(sink: Arc<dyn SinkWriter>, transform: Arc<dyn Transform>) -> Self {
        Self {
            sink,
            transform: Some(transform),
        }
    }
}

#[async_trait::async_trait]
impl MessageProcessor for SingleDestinationProcessor {
    async fn process(&self, envelope: MessageEnvelope) -> Result<()> {
        let envelope = if let Some(t) = &self.transform {
            // Unwrap Arc to get owned Value for transform (cheap if no other references)
            let value_owned = Arc::try_unwrap(envelope.value).unwrap_or_else(|arc| (*arc).clone());
            let transformed = t.transform(value_owned)?;
            MessageEnvelope {
                value: Arc::new(transformed),
                ..envelope
            }
        } else {
            envelope
        };
        self.sink.send(envelope).await
    }

    async fn flush(&self) -> Result<()> {
        self.sink.flush().await
    }
}

struct DestinationMetrics {
    processing_duration: Histogram,
    filter_pass_counter: Counter,
    filter_fail_counter: Counter,
    messages_filtered_counter: Counter,
    messages_filtered_error_counter: Counter,
    transform_envelope_counter: Counter,
    transform_value_counter: Counter,
    messages_produced_counter: Counter,
}

impl DestinationMetrics {
    fn new(name: &str) -> Self {
        Self {
            processing_duration: METRICS.processing_duration.with_label_values(&[name]),
            filter_pass_counter: METRICS
                .filter_evaluations
                .with_label_values(&[name, labels::FILTER_RESULT_PASS]),
            filter_fail_counter: METRICS
                .filter_evaluations
                .with_label_values(&[name, labels::FILTER_RESULT_FAIL]),
            messages_filtered_counter: METRICS
                .messages_filtered
                .with_label_values(&[name, labels::FILTER_REASON_FAILED]),
            messages_filtered_error_counter: METRICS
                .messages_filtered
                .with_label_values(&[name, labels::FILTER_REASON_ERROR]),
            transform_envelope_counter: METRICS
                .transform_operations
                .with_label_values(&[name, labels::TRANSFORM_TYPE_ENVELOPE]),
            transform_value_counter: METRICS
                .transform_operations
                .with_label_values(&[name, labels::TRANSFORM_TYPE_VALUE]),
            messages_produced_counter: METRICS.messages_produced.with_label_values(&[name]),
        }
    }
}

struct DestinationRuntime {
    filter: Arc<dyn Filter>,
    transform: Arc<dyn Transform>,
    name: String,
    error_policy: crate::config::ErrorPolicy,
    metrics: DestinationMetrics,
}

impl DestinationRuntime {
    fn new(
        filter: Option<Arc<dyn Filter>>,
        transform: Option<Arc<dyn Transform>>,
        name: String,
        error_policy: crate::config::ErrorPolicy,
    ) -> Self {
        let metrics = DestinationMetrics::new(name.as_str());

        Self {
            filter: filter.unwrap_or_else(|| Arc::new(PassThroughFilter)),
            transform: transform.unwrap_or_else(|| Arc::new(IdentityTransform)),
            name,
            error_policy,
            metrics,
        }
    }

    fn evaluate_filter(&self, envelope: &MessageEnvelope) -> Result<bool> {
        let filter_passed = match self.filter.evaluate_envelope(envelope) {
            Ok(passed) => passed,
            Err(e) => {
                return self.handle_error(e, "filter evaluation");
            }
        };

        if filter_passed {
            self.metrics.filter_pass_counter.inc();
        } else {
            self.metrics.filter_fail_counter.inc();
            self.metrics.messages_filtered_counter.inc();
            debug!("Message filtered out by destination: {}", self.name);
        }

        Ok(filter_passed)
    }

    fn apply_value_transform(&self, envelope: &mut MessageEnvelope) -> Result<bool> {
        self.metrics.transform_value_counter.inc();

        let value_owned =
            Arc::try_unwrap(Arc::clone(&envelope.value)).unwrap_or_else(|arc| (*arc).clone());

        let transformed_value = match self.transform.transform(value_owned) {
            Ok(val) => val,
            Err(e) => {
                return self.handle_error(e, "value transform");
            }
        };

        envelope.value = Arc::new(transformed_value);
        Ok(true)
    }

    fn handle_error(&self, error: MirrorMakerError, operation: &str) -> Result<bool> {
        use crate::config::ErrorPolicy;
        use tracing::warn;

        match self.error_policy {
            ErrorPolicy::Fail => {
                error!(
                    destination = %self.name,
                    operation = %operation,
                    error = %error,
                    "Pipeline halted due to error (error_policy: fail)"
                );
                Err(error)
            }
            ErrorPolicy::Dlq => {
                warn!(
                    destination = %self.name,
                    operation = %operation,
                    error = %error,
                    "Error will be sent to DLQ (error_policy: dlq)"
                );
                Err(error)
            }
            ErrorPolicy::SkipAndLog => {
                warn!(
                    destination = %self.name,
                    operation = %operation,
                    error = %error,
                    "Skipping message due to error (error_policy: skip_and_log)"
                );
                self.metrics.messages_filtered_error_counter.inc();
                Ok(false)
            }
            ErrorPolicy::Continue => {
                warn!(
                    destination = %self.name,
                    operation = %operation,
                    error = %error,
                    "Continuing despite error (error_policy: continue)"
                );
                self.metrics.messages_filtered_error_counter.inc();
                Ok(false)
            }
        }
    }
}

struct ImmediateDestinationProcessor {
    sink: Arc<dyn SinkWriter>,
    envelope_transforms: Vec<Arc<dyn EnvelopeTransform>>,
    runtime: DestinationRuntime,
}

impl ImmediateDestinationProcessor {
    fn new(
        sink: Arc<dyn SinkWriter>,
        filter: Option<Arc<dyn Filter>>,
        envelope_transforms: Vec<Arc<dyn EnvelopeTransform>>,
        transform: Option<Arc<dyn Transform>>,
        name: String,
        error_policy: crate::config::ErrorPolicy,
    ) -> Self {
        Self {
            sink,
            envelope_transforms,
            runtime: DestinationRuntime::new(filter, transform, name, error_policy),
        }
    }

    async fn process(&self, envelope: MessageEnvelope) -> Result<bool> {
        let timer = self.runtime.metrics.processing_duration.start_timer();

        if !self.runtime.evaluate_filter(&envelope)? {
            return Ok(false);
        }

        let mut envelope = envelope;
        for transform in &self.envelope_transforms {
            self.runtime.metrics.transform_envelope_counter.inc();

            envelope = match transform.transform_envelope(envelope) {
                Ok(env) => env,
                Err(e) => {
                    return self.runtime.handle_error(e, "envelope transform");
                }
            };
        }

        if !self.runtime.apply_value_transform(&mut envelope)? {
            return Ok(false);
        }

        self.sink.send(envelope).await?;
        self.runtime.metrics.messages_produced_counter.inc();

        timer.observe_duration();
        Ok(true)
    }

    async fn flush(&self) -> Result<()> {
        self.sink.flush().await
    }
}

struct AggregatingDestinationProcessor {
    sink: Arc<dyn SinkWriter>,
    aggregation: Mutex<AggregationEngine>,
    runtime: DestinationRuntime,
}

impl AggregatingDestinationProcessor {
    fn new(
        sink: Arc<dyn SinkWriter>,
        filter: Option<Arc<dyn Filter>>,
        transform: Option<Arc<dyn Transform>>,
        aggregation: AggregationConfig,
        name: String,
        error_policy: crate::config::ErrorPolicy,
    ) -> Result<Self> {
        let engine = AggregationEngine::new(aggregation, name.clone())?;

        Ok(Self {
            sink,
            aggregation: Mutex::new(engine),
            runtime: DestinationRuntime::new(filter, transform, name, error_policy),
        })
    }

    async fn process(&self, envelope: MessageEnvelope) -> Result<bool> {
        let timer = self.runtime.metrics.processing_duration.start_timer();

        if !self.runtime.evaluate_filter(&envelope)? {
            return Ok(false);
        }

        let mut envelope = envelope;
        if !self.runtime.apply_value_transform(&mut envelope)? {
            return Ok(false);
        }

        let timestamp_ms = observation_timestamp_ms(&envelope)?;
        let observe_result = self
            .aggregation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .observe(&envelope.value, timestamp_ms);

        match observe_result {
            Ok(()) => {
                timer.observe_duration();
                Ok(true)
            }
            Err(e) => self.runtime.handle_error(e, "aggregation observe"),
        }
    }

    async fn flush(&self) -> Result<()> {
        let emitted = self
            .aggregation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .prepare_flush_expired(current_time_millis()?)?;

        for emission in emitted {
            self.sink
                .send(aggregate_emission_to_envelope(emission))
                .await?;
            self.runtime.metrics.messages_produced_counter.inc();
        }

        self.sink.flush().await?;
        self.aggregation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .commit_flush();
        Ok(())
    }
}

/// Destination with either immediate send or aggregation buffering.
pub struct DestinationProcessor {
    kind: DestinationProcessorKind,
}

enum DestinationProcessorKind {
    Immediate(ImmediateDestinationProcessor),
    Aggregating(AggregatingDestinationProcessor),
}

impl DestinationProcessor {
    pub fn new(
        sink: Arc<dyn SinkWriter>,
        filter: Option<Arc<dyn Filter>>,
        envelope_transforms: Vec<Arc<dyn EnvelopeTransform>>,
        transform: Option<Arc<dyn Transform>>,
        name: String,
        error_policy: crate::config::ErrorPolicy,
    ) -> Self {
        Self {
            kind: DestinationProcessorKind::Immediate(ImmediateDestinationProcessor::new(
                sink,
                filter,
                envelope_transforms,
                transform,
                name,
                error_policy,
            )),
        }
    }

    pub fn with_aggregation(
        sink: Arc<dyn SinkWriter>,
        filter: Option<Arc<dyn Filter>>,
        transform: Option<Arc<dyn Transform>>,
        aggregation: AggregationConfig,
        name: String,
        error_policy: crate::config::ErrorPolicy,
    ) -> Result<Self> {
        Ok(Self {
            kind: DestinationProcessorKind::Aggregating(AggregatingDestinationProcessor::new(
                sink,
                filter,
                transform,
                aggregation,
                name,
                error_policy,
            )?),
        })
    }

    pub async fn process(&self, envelope: MessageEnvelope) -> Result<bool> {
        match &self.kind {
            DestinationProcessorKind::Immediate(processor) => processor.process(envelope).await,
            DestinationProcessorKind::Aggregating(processor) => processor.process(envelope).await,
        }
    }

    pub async fn flush(&self) -> Result<()> {
        match &self.kind {
            DestinationProcessorKind::Immediate(processor) => processor.flush().await,
            DestinationProcessorKind::Aggregating(processor) => processor.flush().await,
        }
    }

    fn name(&self) -> &str {
        match &self.kind {
            DestinationProcessorKind::Immediate(processor) => processor.runtime.name.as_str(),
            DestinationProcessorKind::Aggregating(processor) => processor.runtime.name.as_str(),
        }
    }
}

fn current_time_millis() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| MirrorMakerError::Processing(format!("system clock error: {}", e)))?
        .as_millis() as u64)
}

fn observation_timestamp_ms(envelope: &MessageEnvelope) -> Result<u64> {
    match envelope.timestamp {
        Some(timestamp) => u64::try_from(timestamp).map_err(|_| {
            MirrorMakerError::Processing(format!(
                "message timestamp must be non-negative for aggregation: {}",
                timestamp
            ))
        }),
        None => current_time_millis(),
    }
}

fn aggregate_emission_to_envelope(emission: AggregateEmission) -> MessageEnvelope {
    let mut envelope = MessageEnvelope::new(emission.value)
        .key(Value::String(emission.group_key.as_str().to_string()));
    envelope.topic = Some(emission.output_topic);
    envelope
}

/// Multi-destination router processor
pub struct MultiDestinationProcessor {
    destinations: Vec<DestinationProcessor>,
    #[allow(dead_code)] // Reserved for future content-based routing
    routing_path: Option<String>,
}

impl MultiDestinationProcessor {
    pub fn new(destinations: Vec<DestinationProcessor>, routing_path: Option<String>) -> Self {
        Self {
            destinations,
            routing_path,
        }
    }

    /// Extract routing value from JSON path
    #[allow(dead_code)] // Reserved for future content-based routing
    fn extract_routing_value(&self, value: &Value) -> Option<String> {
        let path = self.routing_path.as_ref()?;
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();

        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }

        Some(current.as_str()?.to_string())
    }
}

#[async_trait::async_trait]
impl MessageProcessor for MultiDestinationProcessor {
    async fn process(&self, envelope: MessageEnvelope) -> Result<()> {
        let futures: Vec<_> = self
            .destinations
            .iter()
            .map(|dest| {
                let env = envelope.clone();
                async move { (dest.name().to_string(), dest.process(env).await) }
            })
            .collect();

        let results = futures::future::join_all(futures).await;

        let mut processed = false;
        let mut errors = Vec::new();

        for (dest_name, result) in results {
            match result {
                Ok(true) => processed = true,
                Ok(false) => {}
                Err(e) => {
                    error!("Error processing destination {}: {}", dest_name, e);
                    errors.push(format!("{}: {}", dest_name, e));
                }
            }
        }

        if !errors.is_empty() {
            return Err(MirrorMakerError::Processing(format!(
                "Failed to process {} destination(s): {}",
                errors.len(),
                errors.join("; ")
            )));
        }

        if !processed {
            debug!("Message not processed by any destination (filtered out by all)");
        }

        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        let futures: Vec<_> = self
            .destinations
            .iter()
            .map(|dest| async move { (dest.name().to_string(), dest.flush().await) })
            .collect();

        let results = futures::future::join_all(futures).await;
        let mut errors = Vec::new();

        for (dest_name, result) in results {
            if let Err(e) = result {
                error!("Error flushing destination {}: {}", dest_name, e);
                errors.push(format!("{}: {}", dest_name, e));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(MirrorMakerError::Processing(format!(
                "Failed to flush {} destination(s): {}",
                errors.len(),
                errors.join("; ")
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AggregationGroupBy, AggregationMetricConfig, AggregationOp, AggregationWindowConfig,
        ErrorPolicy,
    };
    use crate::GroupKey;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct RecordingSink {
        sent: Mutex<Vec<MessageEnvelope>>,
        flushes: AtomicUsize,
        send_failures_remaining: AtomicUsize,
        flush_failures_remaining: AtomicUsize,
    }

    impl RecordingSink {
        fn new() -> Self {
            Self {
                sent: Mutex::new(Vec::new()),
                flushes: AtomicUsize::new(0),
                send_failures_remaining: AtomicUsize::new(0),
                flush_failures_remaining: AtomicUsize::new(0),
            }
        }

        fn with_send_failures(send_failures: usize) -> Self {
            Self {
                sent: Mutex::new(Vec::new()),
                flushes: AtomicUsize::new(0),
                send_failures_remaining: AtomicUsize::new(send_failures),
                flush_failures_remaining: AtomicUsize::new(0),
            }
        }

        fn with_flush_failures(flush_failures: usize) -> Self {
            Self {
                sent: Mutex::new(Vec::new()),
                flushes: AtomicUsize::new(0),
                send_failures_remaining: AtomicUsize::new(0),
                flush_failures_remaining: AtomicUsize::new(flush_failures),
            }
        }

        fn sent_messages(&self) -> Vec<MessageEnvelope> {
            self.sent.lock().unwrap().clone()
        }

        fn flush_count(&self) -> usize {
            self.flushes.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl SinkWriter for RecordingSink {
        async fn send(&self, envelope: MessageEnvelope) -> Result<()> {
            if self.send_failures_remaining.load(Ordering::SeqCst) > 0 {
                self.send_failures_remaining.fetch_sub(1, Ordering::SeqCst);
                return Err(MirrorMakerError::Kafka("send failed".to_string()));
            }
            self.sent.lock().unwrap().push(envelope);
            Ok(())
        }

        async fn flush(&self) -> Result<()> {
            if self.flush_failures_remaining.load(Ordering::SeqCst) > 0 {
                self.flush_failures_remaining.fetch_sub(1, Ordering::SeqCst);
                return Err(MirrorMakerError::Kafka("flush failed".to_string()));
            }
            self.flushes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct FailingTransform;

    impl Transform for FailingTransform {
        fn transform(&self, _value: Value) -> Result<Value> {
            Err(MirrorMakerError::Processing("transform failed".to_string()))
        }
    }

    fn aggregation_config(emit_interval_seconds: u64) -> AggregationConfig {
        AggregationConfig {
            group_by: vec![AggregationGroupBy {
                name: "tenant".to_string(),
                path: "/tenant".to_string(),
            }],
            window: AggregationWindowConfig {
                window_type: crate::AggregationWindowType::Tumbling,
                size_seconds: 1,
                emit_interval_seconds,
            },
            metrics: vec![AggregationMetricConfig {
                name: "count".to_string(),
                op: AggregationOp::Count,
                path: None,
                percentiles: None,
            }],
        }
    }

    #[test]
    fn test_extract_routing_value() {
        let processor = MultiDestinationProcessor {
            destinations: vec![],
            routing_path: Some("/eventType".to_string()),
        };

        let value = json!({
            "eventType": "meeting.started",
            "data": {"confId": 123}
        });

        let routing_value = processor.extract_routing_value(&value);
        assert_eq!(routing_value, Some("meeting.started".to_string()));
    }

    #[test]
    fn test_extract_nested_routing_value() {
        let processor = MultiDestinationProcessor {
            destinations: vec![],
            routing_path: Some("/message/type".to_string()),
        };

        let value = json!({
            "message": {
                "type": "quality.report",
                "siteId": 456
            }
        });

        let routing_value = processor.extract_routing_value(&value);
        assert_eq!(routing_value, Some("quality.report".to_string()));
    }

    #[test]
    fn test_aggregate_emission_to_envelope_uses_group_key_and_topic_metadata() {
        let emission = AggregateEmission {
            output_topic: "aggregates.topic".to_string(),
            group_key: GroupKey::new(vec![("tenant".to_string(), json!("tenant-a"))]).unwrap(),
            value: json!({
                "window": {
                    "start_ms": 1_000,
                    "end_ms": 2_000,
                    "type": "tumbling",
                    "size_seconds": 1
                },
                "group": {
                    "tenant": "tenant-a"
                },
                "metrics": {
                    "count": 2
                }
            }),
        };

        let envelope = aggregate_emission_to_envelope(emission);

        assert_eq!(envelope.topic.as_deref(), Some("aggregates.topic"));
        assert_eq!(
            envelope.key,
            Some(Value::String(
                r#"[{"name":"tenant","value":"tenant-a"}]"#.to_string()
            ))
        );
        assert_eq!(
            *envelope.value,
            json!({
                "window": {
                    "start_ms": 1_000,
                    "end_ms": 2_000,
                    "type": "tumbling",
                    "size_seconds": 1
                },
                "group": {
                    "tenant": "tenant-a"
                },
                "metrics": {
                    "count": 2
                }
            })
        );
    }

    #[tokio::test]
    async fn test_aggregating_destination_flush_emits_completed_window() {
        let sink = Arc::new(RecordingSink::new());
        let destination = DestinationProcessor::with_aggregation(
            sink.clone(),
            None,
            None,
            aggregation_config(1),
            "aggregates.topic".to_string(),
            ErrorPolicy::Fail,
        )
        .unwrap();

        let envelope = MessageEnvelope::new(json!({"tenant": "tenant-a"})).timestamp(0);
        let processed = destination.process(envelope).await.unwrap();
        let flushed = destination.flush().await;

        assert!(processed);
        assert!(flushed.is_ok());
        assert_eq!(sink.flush_count(), 1);
        assert_eq!(sink.sent_messages().len(), 1);
    }

    #[tokio::test]
    async fn test_multi_destination_flush_fans_out_to_immediate_and_aggregation_destinations() {
        let immediate_sink = Arc::new(RecordingSink::new());
        let aggregation_sink = Arc::new(RecordingSink::new());
        let processor = MultiDestinationProcessor::new(
            vec![
                DestinationProcessor::new(
                    immediate_sink.clone(),
                    None,
                    vec![],
                    None,
                    "immediate.topic".to_string(),
                    ErrorPolicy::Fail,
                ),
                DestinationProcessor::with_aggregation(
                    aggregation_sink.clone(),
                    None,
                    None,
                    aggregation_config(1),
                    "aggregates.topic".to_string(),
                    ErrorPolicy::Fail,
                )
                .unwrap(),
            ],
            None,
        );

        processor
            .process(MessageEnvelope::new(json!({"tenant": "tenant-a"})).timestamp(0))
            .await
            .unwrap();
        processor.flush().await.unwrap();

        assert_eq!(immediate_sink.flush_count(), 1);
        assert_eq!(aggregation_sink.flush_count(), 1);
        assert_eq!(aggregation_sink.sent_messages().len(), 1);
    }

    #[tokio::test]
    async fn test_transform_error_with_skip_policy_stops_immediate_delivery() {
        let sink = Arc::new(RecordingSink::new());
        let destination = DestinationProcessor::new(
            sink.clone(),
            None,
            vec![],
            Some(Arc::new(FailingTransform)),
            "immediate.topic".to_string(),
            ErrorPolicy::SkipAndLog,
        );

        let processed = destination
            .process(MessageEnvelope::new(json!({"tenant": "tenant-a"})))
            .await
            .unwrap();

        assert!(!processed);
        assert!(sink.sent_messages().is_empty());
    }

    #[tokio::test]
    async fn test_transform_error_with_skip_policy_stops_aggregation_observe() {
        let sink = Arc::new(RecordingSink::new());
        let destination = DestinationProcessor::with_aggregation(
            sink.clone(),
            None,
            Some(Arc::new(FailingTransform)),
            aggregation_config(1),
            "aggregates.topic".to_string(),
            ErrorPolicy::SkipAndLog,
        )
        .unwrap();

        let processed = destination
            .process(MessageEnvelope::new(json!({"tenant": "tenant-a"})).timestamp(0))
            .await
            .unwrap();
        destination.flush().await.unwrap();

        assert!(!processed);
        assert!(sink.sent_messages().is_empty());
    }

    #[tokio::test]
    async fn test_failed_aggregate_send_does_not_lose_pending_window() {
        let sink = Arc::new(RecordingSink::with_send_failures(1));
        let destination = DestinationProcessor::with_aggregation(
            sink.clone(),
            None,
            None,
            aggregation_config(1),
            "aggregates.topic".to_string(),
            ErrorPolicy::Fail,
        )
        .unwrap();

        destination
            .process(MessageEnvelope::new(json!({"tenant": "tenant-a"})).timestamp(0))
            .await
            .unwrap();

        let first_flush = destination.flush().await;
        let second_flush = destination.flush().await;

        assert!(first_flush.is_err());
        assert!(second_flush.is_ok());
        assert_eq!(sink.sent_messages().len(), 1);
    }

    #[tokio::test]
    async fn test_failed_aggregate_sink_flush_does_not_lose_pending_window() {
        let sink = Arc::new(RecordingSink::with_flush_failures(1));
        let destination = DestinationProcessor::with_aggregation(
            sink.clone(),
            None,
            None,
            aggregation_config(1),
            "aggregates.topic".to_string(),
            ErrorPolicy::Fail,
        )
        .unwrap();

        destination
            .process(MessageEnvelope::new(json!({"tenant": "tenant-a"})).timestamp(0))
            .await
            .unwrap();

        let first_flush = destination.flush().await;
        let second_flush = destination.flush().await;

        assert!(first_flush.is_err());
        assert!(second_flush.is_ok());
        assert_eq!(sink.sent_messages().len(), 2);
    }
}
