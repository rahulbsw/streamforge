//! Dead Letter Queue (DLQ) handling
//!
//! Sends messages that cannot be processed to a dedicated DLQ topic
//! with full error context for debugging and manual recovery.

use crate::{MessageEnvelope, MirrorMakerError, Result};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use std::cell::RefCell;
use std::time::Duration;
use tracing::{debug, error, warn};

// Thread-local buffer for JSON serialization to reduce allocations
thread_local! {
    static SERIALIZE_BUFFER: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(4096));
}

/// Serialize a value to JSON bytes using thread-local buffer (reduces allocations)
fn serialize_to_vec<T: serde::Serialize>(value: &T) -> Result<Vec<u8>> {
    SERIALIZE_BUFFER.with(|buf_cell| {
        let mut buf = buf_cell.borrow_mut();
        buf.clear();
        serde_json::to_writer(&mut *buf, value)
            .map_err(|e| MirrorMakerError::Serialization(e.to_string()))?;
        Ok(buf.clone())
    })
}

/// DLQ configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DlqConfig {
    /// Enable DLQ (if false, errors will cause pipeline to halt)
    pub enabled: bool,

    /// DLQ topic name
    pub topic: String,

    /// Kafka brokers for DLQ (can be different from main brokers)
    pub brokers: Option<String>,

    /// Include original message headers in DLQ message
    pub include_original_headers: bool,

    /// Include stack trace in error header
    pub include_stack_trace: bool,

    /// Maximum retries to send to DLQ (if DLQ fails, halt pipeline)
    pub max_dlq_retries: u32,

    /// Compression type for DLQ messages
    pub compression: Option<String>,
}

impl Default for DlqConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            topic: "streamforge-dlq".to_string(),
            brokers: None,
            include_original_headers: true,
            include_stack_trace: false,
            max_dlq_retries: 3,
            compression: None,
        }
    }
}

/// DLQ message with error metadata
#[derive(Debug, Clone)]
pub struct DlqMessage {
    /// Original message envelope
    pub envelope: MessageEnvelope,

    /// Error that caused DLQ
    pub error: MirrorMakerError,

    /// Pipeline name (for tracking)
    pub pipeline: String,

    /// Destination that failed (if applicable)
    pub destination: Option<String>,

    /// Filter expression (if applicable)
    pub filter: Option<String>,

    /// Transform expression (if applicable)
    pub transform: Option<String>,
}

/// Dead Letter Queue handler
pub struct DeadLetterQueue {
    config: DlqConfig,
    producer: FutureProducer,
}

impl DeadLetterQueue {
    /// Create a new DLQ handler
    pub fn new(config: DlqConfig, main_brokers: &str) -> Result<Self> {
        if !config.enabled {
            // Create a dummy producer (won't be used)
            let producer = ClientConfig::new()
                .set("bootstrap.servers", main_brokers)
                .create()
                .map_err(|e| MirrorMakerError::Config(
                    format!("Failed to create DLQ producer: {}", e)
                ))?;

            return Ok(Self { config, producer });
        }

        let default_brokers = main_brokers.to_string();
        let brokers = config.brokers.as_ref().unwrap_or(&default_brokers);

        let mut client_config = ClientConfig::new();
        client_config.set("bootstrap.servers", brokers);

        // DLQ producer settings (reliability over performance)
        client_config.set("acks", "all");  // Wait for all replicas
        client_config.set("retries", "3");
        client_config.set("max.in.flight.requests.per.connection", "1");  // Ordering
        client_config.set("request.timeout.ms", "30000");

        if let Some(compression) = &config.compression {
            client_config.set("compression.type", compression);
        }

        let producer = client_config
            .create()
            .map_err(|e| MirrorMakerError::Config(
                format!("Failed to create DLQ producer: {}", e)
            ))?;

        debug!(
            topic = %config.topic,
            brokers = %brokers,
            "DLQ handler initialized"
        );

        Ok(Self { config, producer })
    }

    /// Send a message to the DLQ
    pub async fn send(&self, dlq_msg: DlqMessage) -> Result<()> {
        if !self.config.enabled {
            warn!(
                error = %dlq_msg.error,
                "DLQ disabled, message would be sent to DLQ but is being dropped"
            );
            return Ok(());
        }

        // Build DLQ message with error headers
        let mut headers = if self.config.include_original_headers {
            (*dlq_msg.envelope.headers).clone()
        } else {
            std::collections::HashMap::new()
        };

        // Add error metadata headers
        self.add_error_headers(&mut headers, &dlq_msg);

        // Serialize key and value using thread-local buffer
        let key_bytes = if let Some(key) = &dlq_msg.envelope.key {
            Some(serialize_to_vec(key)?)
        } else {
            None
        };

        let value_bytes = serialize_to_vec(&*dlq_msg.envelope.value)?;

        // Send with retry (rebuild record on each attempt since it doesn't implement Clone)
        let mut last_error = None;
        for attempt in 0..self.config.max_dlq_retries {
            // Create Kafka record
            let mut record = FutureRecord::to(&self.config.topic)
                .payload(&value_bytes);

            if let Some(key) = &key_bytes {
                record = record.key(key);
            }

            // Add headers to record
            let mut kafka_headers = rdkafka::message::OwnedHeaders::new();
            for (name, value) in &headers {
                kafka_headers = kafka_headers.insert(rdkafka::message::Header {
                    key: name,
                    value: Some(value),
                });
            }
            record = record.headers(kafka_headers);

            match self.producer
                .send(record, Duration::from_secs(30))
                .await
            {
                Ok(_) => {
                    debug!(
                        topic = %self.config.topic,
                        source_topic = ?dlq_msg.envelope.topic,
                        source_partition = ?dlq_msg.envelope.partition,
                        source_offset = ?dlq_msg.envelope.offset,
                        error_type = %error_type_name(&dlq_msg.error),
                        "Message sent to DLQ"
                    );
                    return Ok(());
                }
                Err((e, _)) => {
                    last_error = Some(e);
                    if attempt < self.config.max_dlq_retries - 1 {
                        warn!(
                            attempt = attempt + 1,
                            max_attempts = self.config.max_dlq_retries,
                            error = ?last_error,
                            "Failed to send to DLQ, retrying"
                        );
                        tokio::time::sleep(Duration::from_millis(100 * (2_u64.pow(attempt)))).await;
                    }
                }
            }
        }

        // DLQ send exhausted - this is critical
        error!(
            topic = %self.config.topic,
            source_topic = ?dlq_msg.envelope.topic,
            source_offset = ?dlq_msg.envelope.offset,
            error = ?last_error,
            "CRITICAL: Failed to send message to DLQ after {} attempts, cannot continue",
            self.config.max_dlq_retries
        );

        Err(MirrorMakerError::DeadLetterQueue {
            message: format!(
                "Failed to send to DLQ after {} attempts: {:?}",
                self.config.max_dlq_retries, last_error
            ),
            dlq_topic: self.config.topic.clone(),
        })
    }

    /// Add error metadata headers to DLQ message
    fn add_error_headers(
        &self,
        headers: &mut std::collections::HashMap<String, Vec<u8>>,
        dlq_msg: &DlqMessage,
    ) {
        // Error information
        headers.insert(
            "x-streamforge-error".to_string(),
            dlq_msg.error.to_string().into_bytes(),
        );

        headers.insert(
            "x-streamforge-error-type".to_string(),
            error_type_name(&dlq_msg.error).as_bytes().to_vec(),
        );

        // Source information
        if let Some(topic) = &dlq_msg.envelope.topic {
            headers.insert(
                "x-streamforge-source-topic".to_string(),
                topic.as_bytes().to_vec(),
            );
        }

        if let Some(partition) = dlq_msg.envelope.partition {
            headers.insert(
                "x-streamforge-source-partition".to_string(),
                partition.to_string().into_bytes(),
            );
        }

        if let Some(offset) = dlq_msg.envelope.offset {
            headers.insert(
                "x-streamforge-source-offset".to_string(),
                offset.to_string().into_bytes(),
            );
        }

        // Timestamp
        let timestamp = chrono::Utc::now().to_rfc3339();
        headers.insert(
            "x-streamforge-timestamp".to_string(),
            timestamp.into_bytes(),
        );

        // Pipeline information
        headers.insert(
            "x-streamforge-pipeline".to_string(),
            dlq_msg.pipeline.as_bytes().to_vec(),
        );

        if let Some(destination) = &dlq_msg.destination {
            headers.insert(
                "x-streamforge-destination".to_string(),
                destination.as_bytes().to_vec(),
            );
        }

        if let Some(filter) = &dlq_msg.filter {
            headers.insert(
                "x-streamforge-filter".to_string(),
                filter.as_bytes().to_vec(),
            );
        }

        if let Some(transform) = &dlq_msg.transform {
            headers.insert(
                "x-streamforge-transform".to_string(),
                transform.as_bytes().to_vec(),
            );
        }

        // Stack trace (optional)
        if self.config.include_stack_trace {
            headers.insert(
                "x-streamforge-stack-trace".to_string(),
                format!("{:?}", dlq_msg.error).into_bytes(),
            );
        }
    }
}

/// Get error type name (for x-streamforge-error-type header)
fn error_type_name(error: &MirrorMakerError) -> &'static str {
    match error {
        MirrorMakerError::Kafka(_) => "Kafka",
        MirrorMakerError::KafkaProducer { .. } => "KafkaProducer",
        MirrorMakerError::KafkaConsumer { .. } => "KafkaConsumer",
        MirrorMakerError::OffsetCommit { .. } => "OffsetCommit",
        MirrorMakerError::Serialization { .. } => "Serialization",
        MirrorMakerError::MessageDeserialization { .. } => "MessageDeserialization",
        MirrorMakerError::Config(_) => "Config",
        MirrorMakerError::ConfigWithField { .. } => "ConfigWithField",
        MirrorMakerError::ConfigMissing { .. } => "ConfigMissing",
        MirrorMakerError::ConfigInvalid { .. } => "ConfigInvalid",
        MirrorMakerError::DslParse { .. } => "DslParse",
        MirrorMakerError::InvalidFilter { .. } => "InvalidFilter",
        MirrorMakerError::InvalidTransform { .. } => "InvalidTransform",
        MirrorMakerError::Processing(_) => "Processing",
        MirrorMakerError::ProcessingWithContext { .. } => "ProcessingWithContext",
        MirrorMakerError::FilterEvaluation { .. } => "FilterEvaluation",
        MirrorMakerError::TransformEvaluation { .. } => "TransformEvaluation",
        MirrorMakerError::JsonPathNotFound { .. } => "JsonPathNotFound",
        MirrorMakerError::Compression(_) => "Compression",
        MirrorMakerError::CompressionWithCodec { .. } => "CompressionWithCodec",
        MirrorMakerError::Decompression { .. } => "Decompression",
        MirrorMakerError::Cache { .. } => "Cache",
        MirrorMakerError::Redis { .. } => "Redis",
        MirrorMakerError::Io { .. } => "Io",
        MirrorMakerError::RetryExhausted { .. } => "RetryExhausted",
        MirrorMakerError::DeadLetterQueue { .. } => "DeadLetterQueue",
        MirrorMakerError::Generic(_) => "Generic",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_default_dlq_config() {
        let config = DlqConfig::default();
        assert!(config.enabled);
        assert_eq!(config.topic, "streamforge-dlq");
        assert!(config.include_original_headers);
        assert_eq!(config.max_dlq_retries, 3);
    }

    #[test]
    fn test_error_type_name() {
        let error = MirrorMakerError::MessageDeserialization {
            message: "Bad JSON".into(),
            topic: "test".into(),
            partition: 0,
            offset: 100,
            key: None,
        };

        assert_eq!(error_type_name(&error), "MessageDeserialization");
    }

    #[test]
    fn test_dlq_message_creation() {
        let envelope = MessageEnvelope::new(json!({"test": "value"}))
            .source("test-topic".into(), 0, 100);

        let error = MirrorMakerError::FilterEvaluation {
            message: "Filter failed".into(),
            filter: "/status,==,active".into(),
            value: Some(json!({"status": "inactive"}).to_string()),
        };

        let dlq_msg = DlqMessage {
            envelope,
            error,
            pipeline: "test-pipeline".into(),
            destination: Some("output-topic".into()),
            filter: Some("/status,==,active".into()),
            transform: None,
        };

        assert_eq!(dlq_msg.pipeline, "test-pipeline");
        assert_eq!(dlq_msg.destination, Some("output-topic".into()));
    }
}
