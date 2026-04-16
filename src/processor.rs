use crate::filter::{EnvelopeTransform, Filter, IdentityTransform, PassThroughFilter, Transform};
use crate::kafka::sink::KafkaSink;
use crate::observability::{labels, METRICS};
use crate::{MessageEnvelope, MirrorMakerError, Result};
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
            let transformed = t.transform(envelope.value)?;
            MessageEnvelope {
                value: transformed,
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
}

impl DestinationProcessor {
    pub fn new(
        sink: Arc<KafkaSink>,
        filter: Option<Arc<dyn Filter>>,
        envelope_transforms: Vec<Arc<dyn EnvelopeTransform>>,
        transform: Option<Arc<dyn Transform>>,
        name: String,
    ) -> Self {
        Self {
            sink,
            filter: filter.unwrap_or_else(|| Arc::new(PassThroughFilter)),
            envelope_transforms,
            transform: transform.unwrap_or_else(|| Arc::new(IdentityTransform)),
            name,
        }
    }

    pub async fn process(&self, envelope: MessageEnvelope) -> Result<bool> {
        // Track processing duration
        let timer = METRICS
            .processing_duration
            .with_label_values(&[self.name.as_str()])
            .start_timer();

        // Apply envelope filter (works for both value-only and envelope-aware filters)
        let filter_passed = self.filter.evaluate_envelope(&envelope)?;

        METRICS
            .filter_evaluations
            .with_label_values(&[
                self.name.as_str(),
                if filter_passed {
                    labels::FILTER_RESULT_PASS
                } else {
                    labels::FILTER_RESULT_FAIL
                },
            ])
            .inc();

        if !filter_passed {
            debug!("Message filtered out by destination: {}", self.name);
            METRICS
                .messages_filtered
                .with_label_values(&[self.name.as_str(), labels::FILTER_REASON_FAILED])
                .inc();
            return Ok(false);
        }

        // Apply envelope transforms (key, headers, timestamp)
        let mut envelope = envelope;
        for transform in &self.envelope_transforms {
            METRICS
                .transform_operations
                .with_label_values(&[self.name.as_str(), labels::TRANSFORM_TYPE_ENVELOPE])
                .inc();

            envelope = transform.transform_envelope(envelope)?;
        }

        // Apply value transform (always done, backward compatible)
        METRICS
            .transform_operations
            .with_label_values(&[self.name.as_str(), labels::TRANSFORM_TYPE_VALUE])
            .inc();

        let transformed_value = self.transform.transform(envelope.value)?;
        envelope.value = transformed_value;

        // Send to sink
        self.sink.send(envelope).await?;

        // Track successful message production
        METRICS
            .messages_produced
            .with_label_values(&[self.name.as_str()])
            .inc();

        timer.observe_duration();
        Ok(true)
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
        let mut processed = false;
        let mut errors = Vec::new();

        // Process through each destination
        // Optimize: clone for all but the last destination to reduce allocations
        let dest_count = self.destinations.len();

        // Process all destinations except the last (with cloning)
        for dest in self.destinations.iter().take(dest_count.saturating_sub(1)) {
            match dest.process(envelope.clone()).await {
                Ok(true) => processed = true,
                Ok(false) => {} // Filtered out, that's ok
                Err(e) => {
                    error!("Error processing destination {}: {}", dest.name, e);
                    errors.push(format!("{}: {}", dest.name, e));
                }
            }
        }

        // Process the last destination (no clone, move envelope)
        if let Some(last_dest) = self.destinations.last() {
            match last_dest.process(envelope).await {
                Ok(true) => processed = true,
                Ok(false) => {} // Filtered out, that's ok
                Err(e) => {
                    error!("Error processing destination {}: {}", last_dest.name, e);
                    errors.push(format!("{}: {}", last_dest.name, e));
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
