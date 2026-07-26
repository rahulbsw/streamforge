use crate::envelope::MessageEnvelope;
use crate::error::{MirrorMakerError, Result};
use crate::filter::{EnvelopeTransform, Filter, Transform};
use crate::wasm::config::WasmWorld;
use crate::wasm::{WasmError, WasmErrorKind, WasmRegistry};
use serde_json::Value;
use std::sync::Arc;

/// Adapter ordering contract for processor integration:
///
/// 1. native filter, then [`WasmFilter`];
/// 2. native value transform, then [`WasmValueTransform`];
/// 3. existing native envelope transforms, then [`WasmEnvelopeTransform`].
///
/// The current processor controls stage ordering. These adapters deliberately
/// represent one stage each and do not introduce work when they are absent.
pub const ADAPTER_COMPOSITION_ORDER: &str =
    "native_filter,wasm_filter,native_value,wasm_value,native_envelope,wasm_envelope";

/// Compose the optional native and WASM filters without dropping either one.
///
/// The native filter runs first and can short-circuit both on `false` and on
/// error. The returned trait object is `None` only when both inputs are absent.
pub fn compose_filter(
    native: Option<Arc<dyn Filter>>,
    wasm: Option<Arc<dyn Filter>>,
) -> Option<Arc<dyn Filter>> {
    match (native, wasm) {
        (Some(first), Some(second)) => Some(Arc::new(FilterChain { first, second })),
        (Some(filter), None) | (None, Some(filter)) => Some(filter),
        (None, None) => None,
    }
}

/// Compose the optional native and WASM value transforms without dropping
/// either one. The WASM transform receives the successful native output.
pub fn compose_value_transform(
    native: Option<Arc<dyn Transform>>,
    wasm: Option<Arc<dyn Transform>>,
) -> Option<Arc<dyn Transform>> {
    match (native, wasm) {
        (Some(first), Some(second)) => Some(Arc::new(TransformChain { first, second })),
        (Some(transform), None) | (None, Some(transform)) => Some(transform),
        (None, None) => None,
    }
}

struct FilterChain {
    first: Arc<dyn Filter>,
    second: Arc<dyn Filter>,
}

impl Filter for FilterChain {
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        if !self.first.evaluate_envelope(envelope)? {
            return Ok(false);
        }
        self.second.evaluate_envelope(envelope)
    }
}

struct TransformChain {
    first: Arc<dyn Transform>,
    second: Arc<dyn Transform>,
}

impl Transform for TransformChain {
    fn transform(&self, value: Value) -> Result<Value> {
        self.second.transform(self.first.transform(value)?)
    }
}

#[derive(Clone)]
pub struct WasmFilter {
    registry: Arc<WasmRegistry>,
    module: Arc<str>,
}

impl WasmFilter {
    pub fn new(
        registry: Arc<WasmRegistry>,
        module: impl Into<Arc<str>>,
    ) -> std::result::Result<Self, WasmError> {
        let module = module.into();
        validate_world(&registry, &module, WasmWorld::Filter)?;
        Ok(Self { registry, module })
    }

    pub fn module(&self) -> &str {
        &self.module
    }
}

impl Filter for WasmFilter {
    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        self.registry
            .invoke_filter(&self.module, envelope)
            .map_err(|failure| filter_error(&self.module, failure))
    }
}

#[derive(Clone)]
pub struct WasmValueTransform {
    registry: Arc<WasmRegistry>,
    module: Arc<str>,
}

impl WasmValueTransform {
    pub fn new(
        registry: Arc<WasmRegistry>,
        module: impl Into<Arc<str>>,
    ) -> std::result::Result<Self, WasmError> {
        let module = module.into();
        validate_world(&registry, &module, WasmWorld::ValueTransform)?;
        Ok(Self { registry, module })
    }

    pub fn module(&self) -> &str {
        &self.module
    }
}

impl Transform for WasmValueTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        self.registry
            .invoke_value_transform(&self.module, &value)
            .map_err(|failure| transform_error(&self.module, failure))
    }
}

#[derive(Clone)]
pub struct WasmEnvelopeTransform {
    registry: Arc<WasmRegistry>,
    module: Arc<str>,
}

impl WasmEnvelopeTransform {
    pub fn new(
        registry: Arc<WasmRegistry>,
        module: impl Into<Arc<str>>,
    ) -> std::result::Result<Self, WasmError> {
        let module = module.into();
        validate_world(&registry, &module, WasmWorld::EnvelopeTransform)?;
        Ok(Self { registry, module })
    }

    pub fn module(&self) -> &str {
        &self.module
    }
}

impl EnvelopeTransform for WasmEnvelopeTransform {
    fn transform_envelope(&self, mut envelope: MessageEnvelope) -> Result<MessageEnvelope> {
        let output = self
            .registry
            .invoke_envelope_transform(&self.module, &envelope)
            .map_err(|failure| transform_error(&self.module, failure))?;
        envelope.key = output.key;
        envelope.value = Arc::new(output.value);
        envelope.headers = Arc::new(output.headers);
        envelope.timestamp = output.timestamp;
        Ok(envelope)
    }
}

fn validate_world(
    registry: &WasmRegistry,
    module: &str,
    expected: WasmWorld,
) -> std::result::Result<(), WasmError> {
    match registry.world(module) {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(WasmError::new(
            WasmErrorKind::WorldMismatch,
            Some(module),
            format!("expected world {expected:?}, registry module uses {actual:?}"),
        )),
        None => Err(WasmError::new(
            WasmErrorKind::UnknownModule,
            Some(module),
            "module is not present in the loaded registry",
        )),
    }
}

fn filter_error(module: &str, failure: WasmError) -> MirrorMakerError {
    MirrorMakerError::FilterEvaluation {
        message: format_classified_error(&failure),
        filter: format!("wasm:{module}"),
        value: None,
    }
}

fn transform_error(module: &str, failure: WasmError) -> MirrorMakerError {
    MirrorMakerError::TransformEvaluation {
        message: format_classified_error(&failure),
        transform: format!("wasm:{module}"),
        value: None,
    }
}

fn format_classified_error(failure: &WasmError) -> String {
    format!("{:?}: {}", failure.kind(), failure.message())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn adapters_are_thread_safe() {
        assert_send_sync::<WasmFilter>();
        assert_send_sync::<WasmValueTransform>();
        assert_send_sync::<WasmEnvelopeTransform>();
    }

    #[test]
    fn mapped_errors_do_not_include_payloads() {
        let failure = WasmError::new(
            WasmErrorKind::Timeout,
            Some("filter"),
            "execution interrupted",
        );
        let mapped = filter_error("filter", failure);
        let text = mapped.to_string();
        assert!(text.contains("Timeout"));
        assert!(!text.contains("value"));
    }

    struct RecordingFilter {
        name: &'static str,
        result: Result<bool>,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    impl Filter for RecordingFilter {
        fn evaluate_envelope(&self, _envelope: &MessageEnvelope) -> Result<bool> {
            self.calls.lock().unwrap().push(self.name);
            self.result.clone()
        }
    }

    struct RecordingTransform {
        name: &'static str,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    impl Transform for RecordingTransform {
        fn transform(&self, mut value: Value) -> Result<Value> {
            self.calls.lock().unwrap().push(self.name);
            value
                .as_array_mut()
                .expect("test value is an array")
                .push(Value::String(self.name.into()));
            Ok(value)
        }
    }

    #[test]
    fn composed_filter_orders_and_short_circuits() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let chain = compose_filter(
            Some(Arc::new(RecordingFilter {
                name: "native",
                result: Ok(false),
                calls: calls.clone(),
            })),
            Some(Arc::new(RecordingFilter {
                name: "wasm",
                result: Ok(true),
                calls: calls.clone(),
            })),
        )
        .unwrap();
        assert!(!chain
            .evaluate_envelope(&MessageEnvelope::new(json!({})))
            .unwrap());
        assert_eq!(*calls.lock().unwrap(), vec!["native"]);
    }

    #[test]
    fn composed_value_transform_passes_native_output_to_wasm() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let chain = compose_value_transform(
            Some(Arc::new(RecordingTransform {
                name: "native",
                calls: calls.clone(),
            })),
            Some(Arc::new(RecordingTransform {
                name: "wasm",
                calls: calls.clone(),
            })),
        )
        .unwrap();
        assert_eq!(
            chain.transform(json!([])).unwrap(),
            json!(["native", "wasm"])
        );
        assert_eq!(*calls.lock().unwrap(), vec!["native", "wasm"]);
    }
}
