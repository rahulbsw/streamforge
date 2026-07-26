use crate::observability::{labels, METRICS};
use crate::wasm::config::WasmWorld;
use crate::wasm::{WasmError, WasmErrorKind};
use prometheus::{Counter, Gauge, Histogram};
use std::time::{Duration, Instant};

const COMPILE_STATUS_ERROR: &str = "error";

pub(crate) struct ModuleMetrics {
    active: Gauge,
    duration: Histogram,
    input_bytes: Histogram,
    output_bytes: Histogram,
    ok: Counter,
    guest_error: Counter,
    trap: Counter,
    timeout: Counter,
    resource_limit: Counter,
    invalid_output: Counter,
}

impl ModuleMetrics {
    pub(crate) fn new(module: &str, world: WasmWorld) -> Self {
        let kind = match world {
            WasmWorld::Filter => labels::WASM_KIND_FILTER,
            WasmWorld::ValueTransform => labels::WASM_KIND_VALUE_TRANSFORM,
            WasmWorld::EnvelopeTransform => labels::WASM_KIND_ENVELOPE_TRANSFORM,
        };
        let status = |value| {
            METRICS
                .wasm_invocations
                .with_label_values(&[module, kind, value])
        };
        Self {
            active: METRICS
                .wasm_active_invocations
                .with_label_values(&[module, kind]),
            duration: METRICS.wasm_duration.with_label_values(&[module, kind]),
            input_bytes: METRICS.wasm_input_bytes.with_label_values(&[module, kind]),
            output_bytes: METRICS.wasm_output_bytes.with_label_values(&[module, kind]),
            ok: status(labels::WASM_STATUS_OK),
            guest_error: status(labels::WASM_STATUS_GUEST_ERROR),
            trap: status(labels::WASM_STATUS_TRAP),
            timeout: status(labels::WASM_STATUS_TIMEOUT),
            resource_limit: status(labels::WASM_STATUS_RESOURCE_LIMIT),
            invalid_output: status(labels::WASM_STATUS_INVALID_OUTPUT),
        }
    }

    pub(crate) fn begin(&self) -> InvocationGuard<'_> {
        self.active.inc();
        InvocationGuard {
            metrics: self,
            started: Instant::now(),
        }
    }
}

pub(crate) struct InvocationGuard<'a> {
    metrics: &'a ModuleMetrics,
    started: Instant,
}

impl InvocationGuard<'_> {
    pub(crate) fn observe_input(&self, input_bytes: usize) {
        self.metrics.input_bytes.observe(input_bytes as f64);
    }

    pub(crate) fn finish<T>(self, result: &Result<T, WasmError>, output_bytes: Option<usize>) {
        if let Some(bytes) = output_bytes {
            self.metrics.output_bytes.observe(bytes as f64);
        }
        match result {
            Ok(_) => self.metrics.ok.inc(),
            Err(failure) => match failure.kind() {
                WasmErrorKind::Guest => self.metrics.guest_error.inc(),
                WasmErrorKind::Timeout => self.metrics.timeout.inc(),
                WasmErrorKind::ResourceLimit
                | WasmErrorKind::InputLimit
                | WasmErrorKind::OutputLimit => self.metrics.resource_limit.inc(),
                WasmErrorKind::InvalidOutput => self.metrics.invalid_output.inc(),
                _ => self.metrics.trap.inc(),
            },
        }
        self.metrics
            .duration
            .observe(self.started.elapsed().as_secs_f64());
    }
}

impl Drop for InvocationGuard<'_> {
    fn drop(&mut self) {
        self.metrics.active.dec();
    }
}

pub(crate) fn observe_compilation(
    module: &str,
    duration: Duration,
    result: &Result<(), WasmError>,
) {
    METRICS
        .wasm_compilation_duration
        .with_label_values(&[module])
        .observe(duration.as_secs_f64());
    METRICS
        .wasm_compilations
        .with_label_values(&[
            module,
            if result.is_ok() {
                labels::WASM_STATUS_OK
            } else {
                COMPILE_STATUS_ERROR
            },
        ])
        .inc();
}
