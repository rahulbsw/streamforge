use crate::envelope::MessageEnvelope;
use crate::wasm::abi::{
    decode_envelope_output, decode_value_output, envelope_transform_input, filter_input,
    value_input, WasmEnvelopeOutput,
};
use crate::wasm::bindings::{envelope_transform, filter, value_transform};
use crate::wasm::config::{WasmConfig, WasmModuleConfig, WasmRuntimeConfig, WasmWorld};
use crate::wasm::engine::{build_engine, new_store, EpochTicker, StoreState};
use crate::wasm::error::{error, WasmError, WasmErrorKind};
use crate::wasm::loader::{canonical_module_root, read_verified_module};
use crate::wasm::metrics::{observe_compilation, ModuleMetrics};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Trap};

/// Immutable, concurrently shareable registry of digest-pinned UDF components.
///
/// Each invocation creates a fresh `Store` and component instance. Wasmtime's
/// pooling allocator recycles the underlying VM resources after the store is
/// dropped without leaking guest memory or globals between records.
#[derive(Clone)]
pub struct WasmRegistry {
    inner: Arc<RegistryInner>,
}

struct RegistryInner {
    engine: Engine,
    runtime: WasmRuntimeConfig,
    modules: HashMap<String, CompiledModule>,
    _ticker: EpochTicker,
}

struct CompiledModule {
    world: WasmWorld,
    pre: CompiledPre,
    metrics: ModuleMetrics,
}

enum CompiledPre {
    Filter(filter::FilterV1Pre<StoreState>),
    ValueTransform(value_transform::ValueTransformV1Pre<StoreState>),
    EnvelopeTransform(envelope_transform::EnvelopeTransformV1Pre<StoreState>),
}

impl WasmRegistry {
    /// Load, verify, compile, type-check, and probe every configured module.
    ///
    /// This is intentionally an eager startup operation: no file access,
    /// digest calculation, compilation, or export lookup occurs on a record
    /// processing path.
    pub fn load(config: &WasmConfig) -> Result<Self, WasmError> {
        config
            .validate()
            .map_err(|message| error(WasmErrorKind::Configuration, None, message))?;

        let root = canonical_module_root(&config.module_root)?;

        let engine = build_engine(&config.runtime)?;
        let ticker = EpochTicker::start(engine.clone(), config.runtime.epoch_tick_ms)?;
        let mut modules = HashMap::with_capacity(config.modules.len());
        let mut components = HashMap::<String, Component>::with_capacity(config.modules.len());

        for module in &config.modules {
            let digest = module.sha256.to_ascii_lowercase();
            let component = if let Some(component) = components.get(&digest) {
                component.clone()
            } else {
                let started = Instant::now();
                let compiled = compile_component(&root, &engine, &config.runtime, module);
                observe_compilation(
                    &module.name,
                    started.elapsed(),
                    &compiled.as_ref().map(|_| ()).map_err(Clone::clone),
                );
                let component = compiled?;
                components.insert(digest, component.clone());
                component
            };
            let compiled = bind_module(&engine, &config.runtime, module, &component)?;
            modules.insert(module.name.clone(), compiled);
        }

        Ok(Self {
            inner: Arc::new(RegistryInner {
                engine,
                runtime: config.runtime.clone(),
                modules,
                _ticker: ticker,
            }),
        })
    }

    pub fn contains(&self, name: &str) -> bool {
        self.inner.modules.contains_key(name)
    }

    pub fn world(&self, name: &str) -> Option<WasmWorld> {
        self.inner.modules.get(name).map(|module| module.world)
    }

    pub fn module_names(&self) -> impl Iterator<Item = &str> {
        self.inner.modules.keys().map(String::as_str)
    }

    pub fn invoke_filter(&self, name: &str, envelope: &MessageEnvelope) -> Result<bool, WasmError> {
        let module = self.module(name, WasmWorld::Filter)?;
        let CompiledPre::Filter(pre) = &module.pre else {
            return Err(world_mismatch(name, WasmWorld::Filter, module.world));
        };
        let guard = module.metrics.begin();
        let result = (|| {
            let (input, input_bytes) =
                filter_input(name, envelope, self.inner.runtime.max_input_bytes)?;
            guard.observe_input(input_bytes);
            let mut store = new_store(&self.inner.engine, &self.inner.runtime);
            let bindings = pre
                .instantiate(&mut store)
                .map_err(|failure| invocation_error(name, failure))?;
            bindings
                .streamforge_udf_filter_api()
                .call_filter(&mut store, &input)
                .map_err(|failure| invocation_error(name, failure))?
                .map_err(|guest| guest_error(name, guest.code, guest.message))
        })();
        guard.finish(&result, result.as_ref().ok().map(|_| 1));
        result
    }

    pub fn invoke_value_transform(&self, name: &str, value: &Value) -> Result<Value, WasmError> {
        let module = self.module(name, WasmWorld::ValueTransform)?;
        let CompiledPre::ValueTransform(pre) = &module.pre else {
            return Err(world_mismatch(
                name,
                WasmWorld::ValueTransform,
                module.world,
            ));
        };
        let guard = module.metrics.begin();
        let result = (|| {
            let input = value_input(name, value, self.inner.runtime.max_input_bytes)?;
            guard.observe_input(input.len());
            let mut store = new_store(&self.inner.engine, &self.inner.runtime);
            let bindings = pre
                .instantiate(&mut store)
                .map_err(|failure| invocation_error(name, failure))?;
            let bytes = bindings
                .streamforge_udf_value_transform_api()
                .call_transform(&mut store, &input)
                .map_err(|failure| invocation_error(name, failure))?
                .map_err(|guest| guest_error(name, guest.code, guest.message))?;
            let output_bytes = bytes.len();
            decode_value_output(name, bytes, self.inner.runtime.max_output_bytes)
                .map(|value| (value, output_bytes))
        })();
        guard.finish(&result, result.as_ref().ok().map(|(_, bytes)| *bytes));
        result.map(|(value, _)| value)
    }

    pub fn invoke_envelope_transform(
        &self,
        name: &str,
        envelope: &MessageEnvelope,
    ) -> Result<WasmEnvelopeOutput, WasmError> {
        let module = self.module(name, WasmWorld::EnvelopeTransform)?;
        let CompiledPre::EnvelopeTransform(pre) = &module.pre else {
            return Err(world_mismatch(
                name,
                WasmWorld::EnvelopeTransform,
                module.world,
            ));
        };
        let guard = module.metrics.begin();
        let result = (|| {
            let (input, input_bytes) =
                envelope_transform_input(name, envelope, self.inner.runtime.max_input_bytes)?;
            guard.observe_input(input_bytes);
            let mut store = new_store(&self.inner.engine, &self.inner.runtime);
            let bindings = pre
                .instantiate(&mut store)
                .map_err(|failure| invocation_error(name, failure))?;
            let output = bindings
                .streamforge_udf_envelope_transform_api()
                .call_transform(&mut store, &input)
                .map_err(|failure| invocation_error(name, failure))?
                .map_err(|guest| guest_error(name, guest.code, guest.message))?;
            decode_envelope_output(name, output, self.inner.runtime.max_output_bytes)
        })();
        guard.finish(&result, result.as_ref().ok().map(|(_, bytes)| *bytes));
        result.map(|(output, _)| output)
    }

    fn module(&self, name: &str, expected: WasmWorld) -> Result<&CompiledModule, WasmError> {
        let module = self.inner.modules.get(name).ok_or_else(|| {
            error(
                WasmErrorKind::UnknownModule,
                Some(name),
                "module is not present in the loaded registry",
            )
        })?;
        if module.world != expected {
            return Err(world_mismatch(name, expected, module.world));
        }
        Ok(module)
    }
}

fn compile_component(
    root: &std::path::Path,
    engine: &Engine,
    runtime: &WasmRuntimeConfig,
    module: &WasmModuleConfig,
) -> Result<Component, WasmError> {
    let bytes = read_verified_module(root, module, runtime.max_module_bytes)?;
    Component::from_binary(engine, &bytes).map_err(|failure| {
        let kind = if classify_runtime_error(&failure) == WasmErrorKind::ResourceLimit {
            WasmErrorKind::ResourceLimit
        } else {
            WasmErrorKind::Compilation
        };
        error(
            kind,
            Some(&module.name),
            format!("component compilation failed: {failure:#}"),
        )
    })
}

fn bind_module(
    engine: &Engine,
    runtime: &WasmRuntimeConfig,
    module: &WasmModuleConfig,
    component: &Component,
) -> Result<CompiledModule, WasmError> {
    let linker = Linker::<StoreState>::new(engine);
    let instance_pre = linker.instantiate_pre(component).map_err(|failure| {
        error(
            WasmErrorKind::Abi,
            Some(&module.name),
            format!("component imports are not permitted or could not be linked: {failure:#}"),
        )
    })?;
    let pre = match module.world {
        WasmWorld::Filter => {
            let pre = filter::FilterV1Pre::new(instance_pre)
                .map_err(|failure| abi_error(module, failure))?;
            probe_filter(engine, runtime, module, &pre)?;
            CompiledPre::Filter(pre)
        }
        WasmWorld::ValueTransform => {
            let pre = value_transform::ValueTransformV1Pre::new(instance_pre)
                .map_err(|failure| abi_error(module, failure))?;
            probe_value_transform(engine, runtime, module, &pre)?;
            CompiledPre::ValueTransform(pre)
        }
        WasmWorld::EnvelopeTransform => {
            let pre = envelope_transform::EnvelopeTransformV1Pre::new(instance_pre)
                .map_err(|failure| abi_error(module, failure))?;
            probe_envelope_transform(engine, runtime, module, &pre)?;
            CompiledPre::EnvelopeTransform(pre)
        }
    };
    Ok(CompiledModule {
        world: module.world,
        pre,
        metrics: ModuleMetrics::new(&module.name, module.world),
    })
}

fn probe_filter(
    engine: &Engine,
    runtime: &WasmRuntimeConfig,
    module: &WasmModuleConfig,
    pre: &filter::FilterV1Pre<StoreState>,
) -> Result<(), WasmError> {
    pre.instantiate(&mut new_store(engine, runtime))
        .map(|_| ())
        .map_err(|failure| probe_error(module, failure))
}

fn probe_value_transform(
    engine: &Engine,
    runtime: &WasmRuntimeConfig,
    module: &WasmModuleConfig,
    pre: &value_transform::ValueTransformV1Pre<StoreState>,
) -> Result<(), WasmError> {
    pre.instantiate(&mut new_store(engine, runtime))
        .map(|_| ())
        .map_err(|failure| probe_error(module, failure))
}

fn probe_envelope_transform(
    engine: &Engine,
    runtime: &WasmRuntimeConfig,
    module: &WasmModuleConfig,
    pre: &envelope_transform::EnvelopeTransformV1Pre<StoreState>,
) -> Result<(), WasmError> {
    pre.instantiate(&mut new_store(engine, runtime))
        .map(|_| ())
        .map_err(|failure| probe_error(module, failure))
}

fn abi_error(module: &WasmModuleConfig, failure: wasmtime::Error) -> WasmError {
    error(
        WasmErrorKind::Abi,
        Some(&module.name),
        format!(
            "component does not implement {:?}/{:?}: {failure:#}",
            module.abi, module.world
        ),
    )
}

fn probe_error(module: &WasmModuleConfig, failure: wasmtime::Error) -> WasmError {
    let kind = classify_runtime_error(&failure);
    error(
        kind,
        Some(&module.name),
        format!("component startup probe failed: {failure:#}"),
    )
}

fn invocation_error(module: &str, failure: wasmtime::Error) -> WasmError {
    error(
        classify_runtime_error(&failure),
        Some(module),
        format!("component invocation trapped: {failure:#}"),
    )
}

fn guest_error(module: &str, code: impl std::fmt::Debug, _message: String) -> WasmError {
    error(
        WasmErrorKind::Guest,
        Some(module),
        format!("guest returned {code:?}"),
    )
}

fn classify_runtime_error(failure: &wasmtime::Error) -> WasmErrorKind {
    let trap = failure
        .chain()
        .find_map(|cause| cause.downcast_ref::<Trap>());
    if trap == Some(&Trap::Interrupt) {
        return WasmErrorKind::Timeout;
    }
    if trap == Some(&Trap::StackOverflow) {
        return WasmErrorKind::ResourceLimit;
    }
    if trap.is_some() {
        return WasmErrorKind::Trap;
    }
    let message = failure.to_string().to_ascii_lowercase();
    if message.contains("resource limit")
        || message.contains("maximum concurrent")
        || message.contains("pooling allocator")
        || message.contains("failed to grow")
        || message.contains("out of memory")
    {
        WasmErrorKind::ResourceLimit
    } else {
        WasmErrorKind::Trap
    }
}

fn world_mismatch(module: &str, expected: WasmWorld, actual: WasmWorld) -> WasmError {
    error(
        WasmErrorKind::WorldMismatch,
        Some(module),
        format!("expected world {expected:?}, registry module uses {actual:?}"),
    )
}
