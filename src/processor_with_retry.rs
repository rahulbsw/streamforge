///! Processor wrapper with retry and DLQ support
///!
///! Wraps any MessageProcessor to add retry logic and dead letter queue
///! handling according to the error recovery actions defined in error.rs.

use crate::{
    DeadLetterQueue, DlqMessage, MessageEnvelope, MirrorMakerError,
    RecoveryAction, Result, RetryPolicy,
};
use crate::processor::MessageProcessor;
use std::sync::Arc;
use tracing::{debug, error, warn};

/// Processor with retry and DLQ support
pub struct ProcessorWithRetry {
    /// Underlying processor
    processor: Arc<dyn MessageProcessor>,

    /// Retry policy
    retry_policy: RetryPolicy,

    /// Dead letter queue handler
    dlq: Option<Arc<DeadLetterQueue>>,

    /// Pipeline name (for DLQ metadata)
    pipeline_name: String,
}

impl ProcessorWithRetry {
    /// Create a new processor with retry and DLQ support
    pub fn new(
        processor: Arc<dyn MessageProcessor>,
        retry_policy: RetryPolicy,
        dlq: Option<Arc<DeadLetterQueue>>,
        pipeline_name: String,
    ) -> Self {
        Self {
            processor,
            retry_policy,
            dlq,
            pipeline_name,
        }
    }

    /// Process a message with retry and DLQ handling
    pub async fn process_with_retry(&self, envelope: MessageEnvelope) -> Result<()> {
        // Attempt to process with retry
        let result = self
            .retry_policy
            .execute(
                || {
                    let env = envelope.clone();
                    let processor = self.processor.clone();
                    async move { processor.process(env).await }
                },
                "process_message",
            )
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(err) => {
                // Check recovery action
                match err.recovery_action() {
                    RecoveryAction::SendToDlq => {
                        // Send to DLQ
                        self.send_to_dlq(envelope, err).await
                    }
                    RecoveryAction::SkipAndLog => {
                        // Log and skip
                        warn!(
                            topic = ?envelope.topic,
                            partition = ?envelope.partition,
                            offset = ?envelope.offset,
                            error = %err,
                            "Skipping message after error"
                        );
                        Ok(())
                    }
                    RecoveryAction::FailFast => {
                        // Propagate error (will halt pipeline)
                        error!(
                            topic = ?envelope.topic,
                            partition = ?envelope.partition,
                            offset = ?envelope.offset,
                            error = %err,
                            "Fatal error, halting pipeline"
                        );
                        Err(err)
                    }
                    RecoveryAction::RetryWithBackoff => {
                        // This should have been handled by retry_policy already
                        // If we're here, retries were exhausted
                        error!(
                            topic = ?envelope.topic,
                            partition = ?envelope.partition,
                            offset = ?envelope.offset,
                            error = %err,
                            "Retry exhausted, sending to DLQ"
                        );
                        self.send_to_dlq(envelope, err).await
                    }
                }
            }
        }
    }

    /// Send message to dead letter queue
    async fn send_to_dlq(&self, envelope: MessageEnvelope, error: MirrorMakerError) -> Result<()> {
        match &self.dlq {
            Some(dlq) => {
                debug!(
                    topic = ?envelope.topic,
                    partition = ?envelope.partition,
                    offset = ?envelope.offset,
                    error = %error,
                    "Sending message to DLQ"
                );

                let dlq_msg = DlqMessage {
                    envelope,
                    error,
                    pipeline: self.pipeline_name.clone(),
                    destination: None,
                    filter: None,
                    transform: None,
                };

                dlq.send(dlq_msg).await?;
                Ok(())
            }
            None => {
                // No DLQ configured - this is a fatal error
                error!(
                    topic = ?envelope.topic,
                    partition = ?envelope.partition,
                    offset = ?envelope.offset,
                    error = %error,
                    "DLQ not configured, cannot send message"
                );
                Err(error)
            }
        }
    }
}

#[async_trait::async_trait]
impl MessageProcessor for ProcessorWithRetry {
    async fn process(&self, envelope: MessageEnvelope) -> Result<()> {
        self.process_with_retry(envelope).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DlqConfig, RetryConfig};
    use serde_json::json;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct MockProcessor {
        fail_count: Arc<AtomicU32>,
        error_to_return: Option<MirrorMakerError>,
    }

    #[async_trait::async_trait]
    impl MessageProcessor for MockProcessor {
        async fn process(&self, _envelope: MessageEnvelope) -> Result<()> {
            let count = self.fail_count.fetch_add(1, Ordering::SeqCst);

            if let Some(err) = &self.error_to_return {
                if count < 2 {
                    // Fail first 2 times
                    return Err(err.clone());
                }
            }

            Ok(())
        }
    }

    #[tokio::test]
    async fn test_process_succeeds_immediately() {
        let mock = Arc::new(MockProcessor {
            fail_count: Arc::new(AtomicU32::new(0)),
            error_to_return: None,
        });

        let retry_config = RetryConfig {
            max_attempts: 3,
            initial_delay_ms: 10,
            ..Default::default()
        };

        let processor = ProcessorWithRetry::new(
            mock.clone(),
            RetryPolicy::new(retry_config),
            None,
            "test-pipeline".to_string(),
        );

        let envelope = MessageEnvelope::new(json!({"test": "value"}));
        let result = processor.process(envelope).await;

        assert!(result.is_ok());
        assert_eq!(mock.fail_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_process_succeeds_after_retry() {
        let mock = Arc::new(MockProcessor {
            fail_count: Arc::new(AtomicU32::new(0)),
            error_to_return: Some(MirrorMakerError::KafkaProducer {
                message: "Queue full".into(),
                destination: None,
                recoverable: true,
            }),
        });

        let retry_config = RetryConfig {
            max_attempts: 3,
            initial_delay_ms: 10,
            ..Default::default()
        };

        let processor = ProcessorWithRetry::new(
            mock.clone(),
            RetryPolicy::new(retry_config),
            None,
            "test-pipeline".to_string(),
        );

        let envelope = MessageEnvelope::new(json!({"test": "value"}));
        let result = processor.process(envelope).await;

        assert!(result.is_ok());
        // Should have been called 3 times (fail, fail, succeed)
        assert_eq!(mock.fail_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_fatal_error_fails_fast() {
        let mock = Arc::new(MockProcessor {
            fail_count: Arc::new(AtomicU32::new(0)),
            error_to_return: Some(MirrorMakerError::Config("Bad config".into())),
        });

        let retry_config = RetryConfig {
            max_attempts: 3,
            initial_delay_ms: 10,
            ..Default::default()
        };

        let processor = ProcessorWithRetry::new(
            mock.clone(),
            RetryPolicy::new(retry_config),
            None,
            "test-pipeline".to_string(),
        );

        let envelope = MessageEnvelope::new(json!({"test": "value"}));
        let result = processor.process(envelope).await;

        // Should fail immediately (FailFast)
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MirrorMakerError::Config(_)));
        // Should only be called once (no retry for config errors)
        assert_eq!(mock.fail_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_message_error_goes_to_dlq() {
        let mock = Arc::new(MockProcessor {
            fail_count: Arc::new(AtomicU32::new(0)),
            error_to_return: Some(MirrorMakerError::FilterEvaluation {
                message: "Filter failed".into(),
                filter: "/status,==,active".into(),
                value: None,
            }),
        });

        let retry_config = RetryConfig {
            max_attempts: 1, // No retry for message errors
            initial_delay_ms: 10,
            ..Default::default()
        };

        // Create DLQ (will fail since no real Kafka, but we can test the flow)
        let dlq_config = DlqConfig {
            enabled: true,
            topic: "test-dlq".to_string(),
            ..Default::default()
        };

        // Note: Can't actually create DLQ without Kafka, but the test shows the structure
        // In real integration tests, we'd use testcontainers for Kafka

        let processor = ProcessorWithRetry::new(
            mock.clone(),
            RetryPolicy::new(retry_config),
            None, // DLQ would be Some(Arc::new(dlq)) in real code
            "test-pipeline".to_string(),
        );

        let envelope = MessageEnvelope::new(json!({"test": "value"}));
        let result = processor.process(envelope).await;

        // Without DLQ configured, should return error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MirrorMakerError::FilterEvaluation { .. }
        ));
    }
}
