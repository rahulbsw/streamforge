//! Extensible destination sink construction.

use crate::config::{MirrorMakerConfig, SinkConfig};
use crate::kafka::sink::KafkaSink;
use crate::processor::SinkWriter;
use crate::{MirrorMakerError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

/// Implement this trait to add a sink backend without changing processor code.
#[async_trait]
pub trait SinkFactory: Send + Sync {
    /// Stable sink type used in `sink.type`.
    fn sink_type(&self) -> &'static str;

    /// Create a writer for one destination.
    async fn create(
        &self,
        config: &MirrorMakerConfig,
        sink: &SinkConfig,
        output: &str,
        partition: Option<&str>,
    ) -> Result<Arc<dyn SinkWriter>>;
}

fn factories() -> &'static RwLock<HashMap<String, Arc<dyn SinkFactory>>> {
    static FACTORIES: OnceLock<RwLock<HashMap<String, Arc<dyn SinkFactory>>>> = OnceLock::new();
    FACTORIES.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Register a custom sink factory. Duplicate names are rejected so startup
/// cannot silently choose a different implementation.
pub fn register_sink_factory(factory: Arc<dyn SinkFactory>) -> Result<()> {
    let name = factory.sink_type().trim().to_ascii_lowercase();
    if name.is_empty() || name == "kafka" {
        return Err(MirrorMakerError::Config(
            "custom sink type must be non-empty and cannot replace the built-in kafka sink"
                .to_string(),
        ));
    }
    let mut registered = factories()
        .write()
        .map_err(|_| MirrorMakerError::Config("sink factory registry is unavailable".into()))?;
    if registered.contains_key(&name) {
        return Err(MirrorMakerError::Config(format!(
            "sink factory '{name}' is already registered"
        )));
    }
    registered.insert(name, factory);
    Ok(())
}

/// Construct a destination sink from its configured type.
pub async fn create_sink(
    config: &MirrorMakerConfig,
    sink: &SinkConfig,
    output: &str,
    partition: Option<&str>,
) -> Result<Arc<dyn SinkWriter>> {
    let name = sink.kind.trim().to_ascii_lowercase();
    if name == "kafka" {
        return Ok(Arc::new(
            KafkaSink::new(config, output.to_string(), partition.map(str::to_string)).await?,
        ));
    }
    let factory = factories()
        .read()
        .map_err(|_| MirrorMakerError::Config("sink factory registry is unavailable".into()))?
        .get(&name)
        .cloned()
        .ok_or_else(|| MirrorMakerError::Config(format!("unknown sink type '{name}'")))?;
    factory.create(config, sink, output, partition).await
}
