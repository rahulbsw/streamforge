use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use streamforge::config::ErrorPolicy;
use streamforge::error::{
    DestinationFailureDisposition, DestinationStage, MirrorMakerError, Result,
};
use streamforge::filter::{EnvelopeTransform, Filter, HeaderSetTransform, Transform};
use streamforge::processor::{DestinationProcessor, SinkWriter};
use streamforge::MessageEnvelope;

#[derive(Default)]
struct RecordingSink {
    sent: Mutex<Vec<MessageEnvelope>>,
}

impl RecordingSink {
    fn messages(&self) -> Vec<MessageEnvelope> {
        self.sent.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl SinkWriter for RecordingSink {
    async fn send(&self, envelope: MessageEnvelope) -> Result<()> {
        self.sent.lock().unwrap().push(envelope);
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}

struct FailingFilter;

impl Filter for FailingFilter {
    fn evaluate_envelope(&self, _envelope: &MessageEnvelope) -> Result<bool> {
        Err(MirrorMakerError::Processing("filter failed".to_string()))
    }
}

struct FailingValueTransform;

impl Transform for FailingValueTransform {
    fn transform(&self, _value: Value) -> Result<Value> {
        Err(MirrorMakerError::Processing(
            "value transform failed".to_string(),
        ))
    }
}

struct FailingEnvelopeTransform;

impl EnvelopeTransform for FailingEnvelopeTransform {
    fn transform_envelope(&self, _envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        Err(MirrorMakerError::Processing(
            "envelope transform failed".to_string(),
        ))
    }
}

struct AddFinalPayloadField;

impl Transform for AddFinalPayloadField {
    fn transform(&self, mut value: Value) -> Result<Value> {
        value
            .as_object_mut()
            .ok_or_else(|| MirrorMakerError::Processing("expected object".to_string()))?
            .insert("final_payload".to_string(), Value::Bool(true));
        Ok(value)
    }
}

struct RequireFinalPayloadEnvelopeTransform;

impl EnvelopeTransform for RequireFinalPayloadEnvelopeTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        if envelope.value.get("final_payload") != Some(&Value::Bool(true)) {
            return Err(MirrorMakerError::Processing(
                "envelope transform did not observe final payload".to_string(),
            ));
        }
        Arc::make_mut(&mut envelope.headers)
            .insert("x-observed-final-payload".to_string(), b"true".to_vec());
        Ok(envelope)
    }
}

fn destination_with_filter(policy: ErrorPolicy, sink: Arc<RecordingSink>) -> DestinationProcessor {
    DestinationProcessor::new(
        sink,
        Some(Arc::new(FailingFilter)),
        vec![],
        None,
        "filter-target".to_string(),
        policy,
    )
}

#[tokio::test]
async fn filter_fail_preserves_typed_context() {
    let destination =
        destination_with_filter(ErrorPolicy::Fail, Arc::new(RecordingSink::default()));

    let error = destination
        .process(MessageEnvelope::new(json!({"id": 1})))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        MirrorMakerError::DestinationFailure { failure }
            if failure.destination == "filter-target"
                && failure.stage == DestinationStage::Filter
                && failure.disposition == DestinationFailureDisposition::FailFast
                && matches!(*failure.source, MirrorMakerError::Processing(_))
    ));
}

#[tokio::test]
async fn filter_dlq_preserves_typed_context() {
    let destination = destination_with_filter(ErrorPolicy::Dlq, Arc::new(RecordingSink::default()));

    let error = destination
        .process(MessageEnvelope::new(json!({"id": 1})))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        MirrorMakerError::DestinationFailure { failure }
            if failure.destination == "filter-target"
                && failure.stage == DestinationStage::Filter
                && failure.disposition == DestinationFailureDisposition::DeadLetter
                && matches!(*failure.source, MirrorMakerError::Processing(_))
    ));
}

#[tokio::test]
async fn filter_skip_and_log_skips_destination() {
    let sink = Arc::new(RecordingSink::default());
    let destination = destination_with_filter(ErrorPolicy::SkipAndLog, sink.clone());

    let processed = destination
        .process(MessageEnvelope::new(json!({"id": 1})))
        .await
        .unwrap();

    assert!(!processed);
    assert!(sink.messages().is_empty());
}

#[tokio::test]
async fn filter_continue_treats_error_as_pass() {
    let sink = Arc::new(RecordingSink::default());
    let destination = destination_with_filter(ErrorPolicy::Continue, sink.clone());

    assert!(destination
        .process(MessageEnvelope::new(json!({"id": 1})))
        .await
        .unwrap());
    assert_eq!(sink.messages().len(), 1);
}

#[tokio::test]
async fn value_transform_precedes_envelope_mutations() {
    let sink = Arc::new(RecordingSink::default());
    let destination = DestinationProcessor::new(
        sink.clone(),
        None,
        vec![Arc::new(RequireFinalPayloadEnvelopeTransform)],
        Some(Arc::new(AddFinalPayloadField)),
        "ordered-transforms".to_string(),
        ErrorPolicy::Fail,
    );

    assert!(destination
        .process(MessageEnvelope::new(json!({"id": 1})))
        .await
        .unwrap());

    let sent = sink.messages();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].value.get("final_payload"), Some(&Value::Bool(true)));
    assert_eq!(
        sent[0].headers.get("x-observed-final-payload"),
        Some(&b"true".to_vec())
    );
}

#[tokio::test]
async fn value_transform_continue_sends_original_envelope() {
    let sink = Arc::new(RecordingSink::default());
    let destination = DestinationProcessor::new(
        sink.clone(),
        None,
        vec![Arc::new(HeaderSetTransform::new("x-mutated", "true"))],
        Some(Arc::new(FailingValueTransform)),
        "continue-value-transform".to_string(),
        ErrorPolicy::Continue,
    );
    let original = MessageEnvelope::new(json!({"id": 1}))
        .key(json!("original-key"))
        .with_header_str("x-original".to_string(), "true")
        .timestamp(1234);

    assert!(destination.process(original.clone()).await.unwrap());

    let sent = sink.messages();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].key, original.key);
    assert_eq!(sent[0].value, original.value);
    assert_eq!(sent[0].headers, original.headers);
    assert_eq!(sent[0].timestamp, original.timestamp);
    assert!(!sent[0].has_header("x-mutated"));
}

#[tokio::test]
async fn envelope_transform_continue_sends_original_and_stops_transform_chain() {
    let sink = Arc::new(RecordingSink::default());
    let destination = DestinationProcessor::new(
        sink.clone(),
        None,
        vec![
            Arc::new(FailingEnvelopeTransform),
            Arc::new(HeaderSetTransform::new("x-must-not-run", "true")),
        ],
        None,
        "continue-envelope-transform".to_string(),
        ErrorPolicy::Continue,
    );
    let original = MessageEnvelope::new(json!({"id": 1}))
        .key(json!("original-key"))
        .with_header_str("x-original".to_string(), "true")
        .timestamp(1234);

    assert!(destination.process(original.clone()).await.unwrap());

    let sent = sink.messages();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].key, original.key);
    assert_eq!(sent[0].value, original.value);
    assert_eq!(sent[0].headers, original.headers);
    assert_eq!(sent[0].timestamp, original.timestamp);
    assert!(!sent[0].has_header("x-must-not-run"));
}
