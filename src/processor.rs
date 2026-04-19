use crate::filter::{EnvelopeTransform, Filter, IdentityTransform, PassThroughFilter, Transform};
use crate::kafka::sink::KafkaSink;
use crate::observability::{labels, METRICS};
use crate::{MessageEnvelope, MirrorMakerError, Result};
use prometheus::{Counter, Histogram};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, error};

/// Message processor trait
#[async_trait::async_trait]
pub trait MessageProcessor: Send + Sync {
    /// Process a message envelope
    async fn process(&self, envelope: MessageEnvelope) -> Result<()>;
}

/// Single-destination processor — optionally applies a value transform before sending.
pub struct SingleDestinationProcessor {
    sink: Arc<KafkaSink>,
    transform: Option<Arc<dyn Transform>>,
}

impl SingleDestinationProcessor {
    pub fn new(sink: Arc<KafkaSink>) -> Self {
        Self {
            sink,
            transform: None,
        }
    }

    pub fn with_transform(sink: Arc<KafkaSink>, transform: Arc<dyn Transform>) -> Self {
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
}

/// Destination with filter and transform
pub struct DestinationProcessor {
    sink: Arc<KafkaSink>,
    filter: Arc<dyn Filter>,
    envelope_transforms: Vec<Arc<dyn EnvelopeTransform>>,
    transform: Arc<dyn Transform>,
    name: String,
    error_policy: crate::config::ErrorPolicy,
    // Pre-resolved metrics (to avoid HashMap lookups on hot path)
    processing_duration: Histogram,
    filter_pass_counter: Counter,
    filter_fail_counter: Counter,
    messages_filtered_counter: Counter,
    messages_filtered_error_counter: Counter,
    transform_envelope_counter: Counter,
    transform_value_counter: Counter,
    messages_produced_counter: Counter,
}

impl DestinationProcessor {
    pub fn new(
        sink: Arc<KafkaSink>,
        filter: Option<Arc<dyn Filter>>,
        envelope_transforms: Vec<Arc<dyn EnvelopeTransform>>,
        transform: Option<Arc<dyn Transform>>,
        name: String,
        error_policy: crate::config::ErrorPolicy,
    ) -> Self {
        // Pre-resolve metrics with labels to avoid HashMap lookups on hot path
        let processing_duration = METRICS
            .processing_duration
            .with_label_values(&[name.as_str()]);

        let filter_pass_counter = METRICS
            .filter_evaluations
            .with_label_values(&[name.as_str(), labels::FILTER_RESULT_PASS]);

        let filter_fail_counter = METRICS
            .filter_evaluations
            .with_label_values(&[name.as_str(), labels::FILTER_RESULT_FAIL]);

        let messages_filtered_counter = METRICS
            .messages_filtered
            .with_label_values(&[name.as_str(), labels::FILTER_REASON_FAILED]);

        let messages_filtered_error_counter = METRICS
            .messages_filtered
            .with_label_values(&[name.as_str(), labels::FILTER_REASON_ERROR]);

        let transform_envelope_counter = METRICS
            .transform_operations
            .with_label_values(&[name.as_str(), labels::TRANSFORM_TYPE_ENVELOPE]);

        let transform_value_counter = METRICS
            .transform_operations
            .with_label_values(&[name.as_str(), labels::TRANSFORM_TYPE_VALUE]);

        let messages_produced_counter = METRICS
            .messages_produced
            .with_label_values(&[name.as_str()]);

        Self {
            sink,
            filter: filter.unwrap_or_else(|| Arc::new(PassThroughFilter)),
            envelope_transforms,
            transform: transform.unwrap_or_else(|| Arc::new(IdentityTransform)),
            name,
            error_policy,
            processing_duration,
            filter_pass_counter,
            filter_fail_counter,
            messages_filtered_counter,
            messages_filtered_error_counter,
            transform_envelope_counter,
            transform_value_counter,
            messages_produced_counter,
        }
    }

    pub async fn process(&self, envelope: MessageEnvelope) -> Result<bool> {
        // Track processing duration (pre-resolved metric)
        let timer = self.processing_duration.start_timer();

        // Apply envelope filter (works for both value-only and envelope-aware filters)
        let filter_passed = match self.filter.evaluate_envelope(&envelope) {
            Ok(passed) => passed,
            Err(e) => {
                return self.handle_error(e, "filter evaluation");
            }
        };

        // Use pre-resolved counters (no HashMap lookup)
        if filter_passed {
            self.filter_pass_counter.inc();
        } else {
            self.filter_fail_counter.inc();
        }

        if !filter_passed {
            debug!("Message filtered out by destination: {}", self.name);
            self.messages_filtered_counter.inc();
            return Ok(false);
        }

        // Apply envelope transforms (key, headers, timestamp)
        let mut envelope = envelope;
        for transform in &self.envelope_transforms {
            self.transform_envelope_counter.inc();

            envelope = match transform.transform_envelope(envelope) {
                Ok(env) => env,
                Err(e) => {
                    return self.handle_error(e, "envelope transform");
                }
            };
        }

        // Apply value transform (always done, backward compatible)
        self.transform_value_counter.inc();

        // Unwrap Arc to get owned Value for transform (cheap if no other references)
        let value_owned = Arc::try_unwrap(envelope.value).unwrap_or_else(|arc| (*arc).clone());

        let transformed_value = match self.transform.transform(value_owned) {
            Ok(val) => val,
            Err(e) => {
                return self.handle_error(e, "value transform");
            }
        };
        envelope.value = Arc::new(transformed_value);

        // Send to sink
        self.sink.send(envelope).await?;

        // Track successful message production (pre-resolved counter)
        self.messages_produced_counter.inc();

        timer.observe_duration();
        Ok(true)
    }

    /// Handle errors according to the error policy
    fn handle_error(&self, error: MirrorMakerError, operation: &str) -> Result<bool> {
        use crate::config::ErrorPolicy;
        use tracing::warn;

        match self.error_policy {
            ErrorPolicy::Fail => {
                // Fail fast - propagate error to halt pipeline
                error!(
                    destination = %self.name,
                    operation = %operation,
                    error = %error,
                    "Pipeline halted due to error (error_policy: fail)"
                );
                Err(error)
            }
            ErrorPolicy::Dlq => {
                // Send to DLQ - propagate error with DLQ recovery action
                warn!(
                    destination = %self.name,
                    operation = %operation,
                    error = %error,
                    "Error will be sent to DLQ (error_policy: dlq)"
                );
                // Ensure error has SendToDlq recovery action
                Err(error)
            }
            ErrorPolicy::SkipAndLog => {
                // Skip message and continue - log and return Ok(false)
                warn!(
                    destination = %self.name,
                    operation = %operation,
                    error = %error,
                    "Skipping message due to error (error_policy: skip_and_log)"
                );
                self.messages_filtered_error_counter.inc();
                Ok(false) // Skipped, but not an error
            }
            ErrorPolicy::Continue => {
                // Continue processing - log and return Ok(false)
                warn!(
                    destination = %self.name,
                    operation = %operation,
                    error = %error,
                    "Continuing despite error (error_policy: continue)"
                );
                self.messages_filtered_error_counter.inc();
                Ok(false) // Skipped this destination, continue others
            }
        }
    }
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
        // Process all destinations concurrently for better throughput
        // Cloning envelope is cheap now (Arc-wrapped value and headers from Task #6)
        let futures: Vec<_> = self
            .destinations
            .iter()
            .map(|dest| {
                let env = envelope.clone();
                async move { (dest.name.clone(), dest.process(env).await) }
            })
            .collect();

        // Wait for all destinations to complete
        let results = futures::future::join_all(futures).await;

        // Collect results
        let mut processed = false;
        let mut errors = Vec::new();

        for (dest_name, result) in results {
            match result {
                Ok(true) => processed = true,
                Ok(false) => {} // Filtered out, that's ok
                Err(e) => {
                    error!("Error processing destination {}: {}", dest_name, e);
                    errors.push(format!("{}: {}", dest_name, e));
                }
            }
        }

        // Fail if any destination had errors (fail-fast for data integrity)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
