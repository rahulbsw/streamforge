use crate::filter::{EnvelopeTransform, Filter, IdentityTransform, PassThroughFilter, Transform};
use crate::kafka::sink::KafkaSink;
use crate::observability::{labels, METRICS};
use crate::{MessageEnvelope, MirrorMakerError, Result};
use std::sync::Arc;
use tracing::{debug, error};

/// Message processor trait
#[async_trait::async_trait]
pub trait MessageProcessor: Send + Sync {
    async fn process(&self, envelope: MessageEnvelope) -> Result<()>;
}

// ============================================================================
// SingleDestinationProcessor
// ============================================================================

/// Forwards every message to a single sink, applying an optional transform.
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
            let new_value = t.transform(&envelope)?;
            MessageEnvelope {
                value: new_value,
                ..envelope
            }
        } else {
            envelope
        };
        self.sink.send(envelope).await
    }
}

// ============================================================================
// DestinationProcessor
// ============================================================================

/// Applies a filter, optional envelope transforms, and a payload transform
/// before forwarding to a sink.
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
        let timer = METRICS
            .processing_duration
            .with_label_values(&[self.name.as_str()])
            .start_timer();

        // 1. Evaluate filter against the full envelope (msg, key, headers, timestamp)
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

        // 2. Apply declarative envelope transforms (key_transform, header_transforms, timestamp)
        let mut envelope = envelope;
        for et in &self.envelope_transforms {
            METRICS
                .transform_operations
                .with_label_values(&[self.name.as_str(), labels::TRANSFORM_TYPE_ENVELOPE])
                .inc();
            envelope = et.transform_envelope(envelope)?;
        }

        // 3. Apply payload transform — receives full envelope so it can read
        //    key/headers/timestamp for conditional logic
        METRICS
            .transform_operations
            .with_label_values(&[self.name.as_str(), labels::TRANSFORM_TYPE_VALUE])
            .inc();
        let new_value = self.transform.transform(&envelope)?;
        envelope.value = new_value;

        // 4. Send to sink
        self.sink.send(envelope).await?;

        METRICS
            .messages_produced
            .with_label_values(&[self.name.as_str()])
            .inc();

        timer.observe_duration();
        Ok(true)
    }
}

// ============================================================================
// MultiDestinationProcessor
// ============================================================================

pub struct MultiDestinationProcessor {
    destinations: Vec<DestinationProcessor>,
    #[allow(dead_code)]
    routing_path: Option<String>,
}

impl MultiDestinationProcessor {
    pub fn new(destinations: Vec<DestinationProcessor>, routing_path: Option<String>) -> Self {
        Self {
            destinations,
            routing_path,
        }
    }
}

#[async_trait::async_trait]
impl MessageProcessor for MultiDestinationProcessor {
    async fn process(&self, envelope: MessageEnvelope) -> Result<()> {
        let mut processed = false;
        let mut errors = Vec::new();
        let dest_count = self.destinations.len();

        for dest in self.destinations.iter().take(dest_count.saturating_sub(1)) {
            match dest.process(envelope.clone()).await {
                Ok(true) => processed = true,
                Ok(false) => {}
                Err(e) => {
                    error!("Error in destination {}: {}", dest.name, e);
                    errors.push(format!("{}: {}", dest.name, e));
                }
            }
        }

        if let Some(last) = self.destinations.last() {
            match last.process(envelope).await {
                Ok(true) => processed = true,
                Ok(false) => {}
                Err(e) => {
                    error!("Error in destination {}: {}", last.name, e);
                    errors.push(format!("{}: {}", last.name, e));
                }
            }
        }

        if !errors.is_empty() {
            return Err(MirrorMakerError::Processing(format!(
                "Failed {} destination(s): {}",
                errors.len(),
                errors.join("; ")
            )));
        }

        if !processed {
            debug!("Message filtered out by all destinations");
        }
        Ok(())
    }
}
