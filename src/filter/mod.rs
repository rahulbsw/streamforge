mod envelope_transform;

pub use envelope_transform::{
    EnvelopeTransform, HeaderCopyTransform, HeaderFromTransform, HeaderRemoveTransform,
    HeaderSetTransform, KeyConstantTransform, KeyConstructTransform, KeyFromTransform,
    KeyHashTransform, KeyTemplateTransform, TimestampAddTransform, TimestampCurrentTransform,
    TimestampFromTransform, TimestampPreserveTransform, TimestampSubtractTransform,
};

use crate::envelope::MessageEnvelope;
use crate::error::Result;
use serde_json::Value;

/// Evaluate whether a message should be forwarded to a destination.
///
/// The default `evaluate` implementation wraps the value in a minimal envelope
/// and calls `evaluate_envelope`, so implementations only need to override
/// whichever matches their access pattern.
pub trait Filter: Send + Sync {
    /// Evaluate against the message payload only.
    fn evaluate(&self, value: &Value) -> Result<bool> {
        let envelope = MessageEnvelope::new(value.clone());
        self.evaluate_envelope(&envelope)
    }

    /// Evaluate against the full Kafka envelope (payload, key, headers, timestamp).
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        self.evaluate(&envelope.value)
    }
}

/// Transform the message payload.
///
/// The full envelope is provided so transforms can read `key`, `headers`, and
/// `timestamp` for conditional logic. The returned `Value` replaces
/// `envelope.value`. Envelope fields other than the payload are NOT changed
/// by this trait; use `key_transform`, `header_transforms`, and `timestamp`
/// destination config fields for that.
pub trait Transform: Send + Sync {
    fn transform(&self, envelope: &MessageEnvelope) -> Result<Value>;
}

/// Always passes every message (used as the no-filter default).
pub struct PassThroughFilter;

impl Filter for PassThroughFilter {
    fn evaluate(&self, _value: &Value) -> Result<bool> {
        Ok(true)
    }
}

/// Returns the payload unchanged (used as the no-transform default).
pub struct IdentityTransform;

impl Transform for IdentityTransform {
    fn transform(&self, envelope: &MessageEnvelope) -> Result<Value> {
        Ok(envelope.value.clone())
    }
}
