use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use streamforge::config::ErrorPolicy;
use streamforge::dlq::DlqWriter;
use streamforge::error::{
    DestinationFailure, DestinationFailureDisposition, DestinationStage, MirrorMakerError,
    RecoveryAction, Result,
};
use streamforge::filter::Filter;
use streamforge::processor::{
    DestinationProcessor, MessageProcessor, MultiDestinationProcessor, SinkWriter,
};
use streamforge::processor_with_retry::ProcessorWithRetry;
use streamforge::{DlqMessage, MessageEnvelope, RetryConfig, RetryPolicy};

#[derive(Default)]
struct RecordingDlqWriter {
    messages: Mutex<Vec<DlqMessage>>,
}

impl RecordingDlqWriter {
    fn messages(&self) -> Vec<DlqMessage> {
        self.messages.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl DlqWriter for RecordingDlqWriter {
    async fn send(&self, message: DlqMessage) -> Result<()> {
        self.messages.lock().unwrap().push(message);
        Ok(())
    }
}

struct AlwaysErrorProcessor {
    calls: Arc<AtomicUsize>,
    error: MirrorMakerError,
}

#[async_trait::async_trait]
impl MessageProcessor for AlwaysErrorProcessor {
    async fn process(&self, _envelope: MessageEnvelope) -> Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(self.error.clone())
    }
}

#[derive(Default)]
struct CountingSink {
    sends: AtomicUsize,
}

#[async_trait::async_trait]
impl SinkWriter for CountingSink {
    async fn send(&self, _envelope: MessageEnvelope) -> Result<()> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}

struct AlwaysFailFilter {
    calls: Arc<AtomicUsize>,
}

impl Filter for AlwaysFailFilter {
    fn evaluate_envelope(&self, _envelope: &MessageEnvelope) -> Result<bool> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(MirrorMakerError::Processing(
            "native filter failure".to_string(),
        ))
    }
}

fn retry_policy() -> RetryPolicy {
    RetryPolicy::new(RetryConfig {
        max_attempts: 3,
        initial_delay_ms: 1,
        jitter: 0.0,
        ..Default::default()
    })
}

fn destination_error(
    destination: &str,
    stage: DestinationStage,
    disposition: DestinationFailureDisposition,
) -> MirrorMakerError {
    MirrorMakerError::DestinationFailures {
        failures: vec![DestinationFailure::new(
            destination,
            stage,
            disposition,
            MirrorMakerError::Processing("native evaluation failed".to_string()),
        )],
    }
}

#[test]
fn destination_failures_are_never_retryable() {
    let dead_letter = destination_error(
        "dlq-target",
        DestinationStage::Filter,
        DestinationFailureDisposition::DeadLetter,
    );
    let mixed = MirrorMakerError::DestinationFailures {
        failures: vec![
            DestinationFailure::new(
                "must-halt",
                DestinationStage::Filter,
                DestinationFailureDisposition::FailFast,
                MirrorMakerError::Processing("fatal".to_string()),
            ),
            DestinationFailure::new(
                "would-dlq",
                DestinationStage::ValueTransform,
                DestinationFailureDisposition::DeadLetter,
                MirrorMakerError::Processing("dead-letter".to_string()),
            ),
        ],
    };

    assert!(!dead_letter.is_recoverable());
    assert_eq!(dead_letter.recovery_action(), RecoveryAction::SendToDlq);
    assert!(!mixed.is_recoverable());
    assert_eq!(mixed.recovery_action(), RecoveryAction::FailFast);
}

#[tokio::test]
async fn dead_letter_failure_reaches_writer_once_with_original_context() {
    let calls = Arc::new(AtomicUsize::new(0));
    let processor = Arc::new(AlwaysErrorProcessor {
        calls: calls.clone(),
        error: MirrorMakerError::DestinationFailures {
            failures: vec![DestinationFailure::new(
                "target-topic",
                DestinationStage::ValueTransform,
                DestinationFailureDisposition::DeadLetter,
                MirrorMakerError::TransformEvaluation {
                    message: "Guest: rejected".to_string(),
                    transform: "wasm:redact".to_string(),
                    value: None,
                },
            )],
        },
    });
    let writer = Arc::new(RecordingDlqWriter::default());
    let wrapper = ProcessorWithRetry::new_with_dlq_writer(
        processor,
        RetryPolicy::new(RetryConfig {
            max_attempts: 1,
            initial_delay_ms: 1,
            jitter: 0.0,
            ..Default::default()
        }),
        Some(writer.clone()),
        "test-pipeline".to_string(),
    );
    let envelope = MessageEnvelope::new(json!({"id": 7})).source("source-topic".to_string(), 2, 41);

    assert!(wrapper.process(envelope.clone()).await.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let messages = writer.messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].destination.as_deref(), Some("target-topic"));
    assert_eq!(messages[0].stage, Some(DestinationStage::ValueTransform));
    assert_eq!(messages[0].transform.as_deref(), Some("wasm:redact"));
    assert!(messages[0].filter.is_none());
    assert_eq!(messages[0].envelope.value, envelope.value);
    assert_eq!(messages[0].envelope.topic, envelope.topic);
    assert_eq!(messages[0].envelope.partition, envelope.partition);
    assert_eq!(messages[0].envelope.offset, envelope.offset);
    assert!(matches!(
        &messages[0].error,
        MirrorMakerError::DestinationFailure { .. }
    ));
}

#[tokio::test]
async fn fail_fast_failure_never_reaches_dlq() {
    let calls = Arc::new(AtomicUsize::new(0));
    let processor = Arc::new(AlwaysErrorProcessor {
        calls: calls.clone(),
        error: destination_error(
            "target-topic",
            DestinationStage::Filter,
            DestinationFailureDisposition::FailFast,
        ),
    });
    let writer = Arc::new(RecordingDlqWriter::default());
    let wrapper = ProcessorWithRetry::new_with_dlq_writer(
        processor,
        retry_policy(),
        Some(writer.clone()),
        "test-pipeline".to_string(),
    );

    let error = wrapper
        .process(MessageEnvelope::new(json!({"id": 7})))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        MirrorMakerError::DestinationFailures { failures }
            if failures.len() == 1
                && failures[0].disposition == DestinationFailureDisposition::FailFast
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(writer.messages().is_empty());
}

#[tokio::test]
async fn mixed_fail_and_dlq_failures_dead_letter_then_return_only_fail_fast() {
    let calls = Arc::new(AtomicUsize::new(0));
    let processor = Arc::new(AlwaysErrorProcessor {
        calls: calls.clone(),
        error: MirrorMakerError::DestinationFailures {
            failures: vec![
                DestinationFailure::new(
                    "must-halt",
                    DestinationStage::Filter,
                    DestinationFailureDisposition::FailFast,
                    MirrorMakerError::Processing("fatal".to_string()),
                ),
                DestinationFailure::new(
                    "would-dlq",
                    DestinationStage::EnvelopeTransform,
                    DestinationFailureDisposition::DeadLetter,
                    MirrorMakerError::Processing("dead-letter".to_string()),
                ),
            ],
        },
    });
    let writer = Arc::new(RecordingDlqWriter::default());
    let wrapper = ProcessorWithRetry::new_with_dlq_writer(
        processor,
        retry_policy(),
        Some(writer.clone()),
        "test-pipeline".to_string(),
    );

    let error = wrapper
        .process(MessageEnvelope::new(json!({"id": 7})))
        .await
        .unwrap_err();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let messages = writer.messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].destination.as_deref(), Some("would-dlq"));
    assert_eq!(messages[0].stage, Some(DestinationStage::EnvelopeTransform));
    assert!(matches!(
        error,
        MirrorMakerError::DestinationFailures { failures }
            if failures.len() == 1
                && failures[0].destination == "must-halt"
                && failures[0].disposition == DestinationFailureDisposition::FailFast
    ));
}

#[tokio::test]
async fn dlq_failure_does_not_rerun_successful_destination() {
    let successful_sink = Arc::new(CountingSink::default());
    let failing_sink = Arc::new(CountingSink::default());
    let failing_filter_calls = Arc::new(AtomicUsize::new(0));
    let multi = Arc::new(MultiDestinationProcessor::new(
        vec![
            DestinationProcessor::new(
                successful_sink.clone(),
                None,
                vec![],
                None,
                "successful-topic".to_string(),
                ErrorPolicy::Fail,
            ),
            DestinationProcessor::new(
                failing_sink.clone(),
                Some(Arc::new(AlwaysFailFilter {
                    calls: failing_filter_calls.clone(),
                })),
                vec![],
                None,
                "failed-topic".to_string(),
                ErrorPolicy::Dlq,
            ),
        ],
        None,
    ));
    let writer = Arc::new(RecordingDlqWriter::default());
    let wrapper = ProcessorWithRetry::new_with_dlq_writer(
        multi,
        retry_policy(),
        Some(writer.clone()),
        "test-pipeline".to_string(),
    );

    assert!(wrapper
        .process(MessageEnvelope::new(json!({"id": 7})))
        .await
        .is_ok());
    assert_eq!(successful_sink.sends.load(Ordering::SeqCst), 1);
    assert_eq!(failing_sink.sends.load(Ordering::SeqCst), 0);
    assert_eq!(failing_filter_calls.load(Ordering::SeqCst), 1);
    let messages = writer.messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].destination.as_deref(), Some("failed-topic"));
    assert_eq!(messages[0].stage, Some(DestinationStage::Filter));
}
