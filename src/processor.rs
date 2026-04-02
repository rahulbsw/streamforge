use crate::filter::{Filter, Transform, PassThroughFilter, IdentityTransform};
use crate::kafka::sink::KafkaSink;
use crate::{MirrorMakerError, Result};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, error, warn};

/// Message processor trait
#[async_trait::async_trait]
pub trait MessageProcessor: Send + Sync {
    /// Process a message
    async fn process(&self, key: Value, value: Value) -> Result<()>;
}

/// Single-destination processor
pub struct SingleDestinationProcessor {
    sink: Arc<KafkaSink>,
}

impl SingleDestinationProcessor {
    pub fn new(sink: Arc<KafkaSink>) -> Self {
        Self { sink }
    }
}

#[async_trait::async_trait]
impl MessageProcessor for SingleDestinationProcessor {
    async fn process(&self, key: Value, value: Value) -> Result<()> {
        self.sink.send(key, value).await
    }
}

/// Destination with filter and transform
pub struct DestinationProcessor {
    sink: Arc<KafkaSink>,
    filter: Arc<dyn Filter>,
    transform: Arc<dyn Transform>,
    name: String,
}

impl DestinationProcessor {
    pub fn new(
        sink: Arc<KafkaSink>,
        filter: Option<Arc<dyn Filter>>,
        transform: Option<Arc<dyn Transform>>,
        name: String,
    ) -> Self {
        Self {
            sink,
            filter: filter.unwrap_or_else(|| Arc::new(PassThroughFilter)),
            transform: transform.unwrap_or_else(|| Arc::new(IdentityTransform)),
            name,
        }
    }

    pub async fn process(&self, key: Value, value: Value) -> Result<bool> {
        // Apply filter
        if !self.filter.evaluate(&value)? {
            debug!("Message filtered out by destination: {}", self.name);
            return Ok(false);
        }

        // Apply transform
        let transformed = self.transform.transform(value)?;

        // Send to sink
        self.sink.send(key, transformed).await?;
        Ok(true)
    }
}

/// Multi-destination router processor
pub struct MultiDestinationProcessor {
    destinations: Vec<DestinationProcessor>,
    routing_path: Option<String>,
}

impl MultiDestinationProcessor {
    pub fn new(
        destinations: Vec<DestinationProcessor>,
        routing_path: Option<String>,
    ) -> Self {
        Self {
            destinations,
            routing_path,
        }
    }

    /// Extract routing value from JSON path
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
    async fn process(&self, key: Value, value: Value) -> Result<()> {
        let mut processed = false;
        let mut errors = Vec::new();

        // Process through each destination
        for dest in &self.destinations {
            match dest.process(key.clone(), value.clone()).await {
                Ok(true) => processed = true,
                Ok(false) => {} // Filtered out, that's ok
                Err(e) => {
                    error!("Error processing destination {}: {}", dest.name, e);
                    errors.push(format!("{}: {}", dest.name, e));
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
