use crate::wasm::config::WasmRuntimeConfig;
use crate::wasm::error::{error, WasmError, WasmErrorKind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use wasmtime::{
    Config, Engine, InstanceAllocationStrategy, PoolingAllocationConfig, Store, StoreLimits,
    StoreLimitsBuilder,
};

const MAX_CORE_INSTANCES_PER_COMPONENT: u32 = 8;
const MAX_MEMORIES_PER_COMPONENT: u32 = 4;
const MAX_TABLES_PER_COMPONENT: u32 = 4;
const MAX_STORE_INSTANCES: usize = 16;

pub(crate) struct StoreState {
    limits: StoreLimits,
}

pub(crate) fn build_engine(runtime: &WasmRuntimeConfig) -> Result<Engine, WasmError> {
    let mut pooling = PoolingAllocationConfig::new();
    pooling
        .total_component_instances(runtime.max_concurrent_instances)
        .max_core_instances_per_component(MAX_CORE_INSTANCES_PER_COMPONENT)
        .max_memories_per_component(MAX_MEMORIES_PER_COMPONENT)
        .max_tables_per_component(MAX_TABLES_PER_COMPONENT)
        .total_core_instances(
            runtime
                .max_concurrent_instances
                .saturating_mul(MAX_CORE_INSTANCES_PER_COMPONENT),
        )
        .total_memories(
            runtime
                .max_concurrent_instances
                .saturating_mul(MAX_MEMORIES_PER_COMPONENT),
        )
        .total_tables(
            runtime
                .max_concurrent_instances
                .saturating_mul(MAX_TABLES_PER_COMPONENT),
        )
        .max_memories_per_module(MAX_MEMORIES_PER_COMPONENT)
        .max_tables_per_module(MAX_TABLES_PER_COMPONENT)
        .max_memory_size(runtime.max_memory_bytes)
        .table_elements(runtime.max_table_elements);

    let mut config = Config::new();
    config
        .epoch_interruption(true)
        .max_wasm_stack(runtime.max_wasm_stack_bytes)
        .allocation_strategy(InstanceAllocationStrategy::Pooling(pooling));
    Engine::new(&config).map_err(|failure| {
        error(
            WasmErrorKind::Configuration,
            None,
            format!("cannot initialize Wasmtime engine: {failure:#}"),
        )
    })
}

pub(crate) fn new_store(engine: &Engine, runtime: &WasmRuntimeConfig) -> Store<StoreState> {
    let limits = StoreLimitsBuilder::new()
        .memory_size(runtime.max_memory_bytes)
        .table_elements(runtime.max_table_elements)
        .instances(MAX_STORE_INSTANCES)
        .memories(MAX_MEMORIES_PER_COMPONENT as usize)
        .tables(MAX_TABLES_PER_COMPONENT as usize)
        .trap_on_grow_failure(true)
        .build();
    let mut store = Store::new(engine, StoreState { limits });
    store.limiter(|state| &mut state.limits);
    store.set_epoch_deadline(deadline_ticks(runtime));
    store
}

fn deadline_ticks(runtime: &WasmRuntimeConfig) -> u64 {
    runtime
        .max_execution_ms
        .saturating_add(runtime.epoch_tick_ms - 1)
        .checked_div(runtime.epoch_tick_ms)
        .unwrap_or(1)
        .max(1)
}

pub(crate) struct EpochTicker {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl EpochTicker {
    pub(crate) fn start(engine: Engine, tick_ms: u64) -> Result<Self, WasmError> {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let handle = thread::Builder::new()
            .name("streamforge-wasm-epoch".into())
            .spawn(move || {
                let interval = Duration::from_millis(tick_ms);
                while !thread_stop.load(Ordering::Relaxed) {
                    thread::sleep(interval);
                    engine.increment_epoch();
                }
            })
            .map_err(|failure| {
                error(
                    WasmErrorKind::Configuration,
                    None,
                    format!("cannot start Wasmtime epoch ticker: {failure}"),
                )
            })?;
        Ok(Self {
            stop,
            handle: Some(handle),
        })
    }
}

impl Drop for EpochTicker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadlines_round_up_to_at_least_one_tick() {
        let runtime = WasmRuntimeConfig {
            max_execution_ms: 5,
            epoch_tick_ms: 2,
            ..WasmRuntimeConfig::default()
        };
        assert_eq!(deadline_ticks(&runtime), 3);
    }
}
