use crate::crd::StreamforgePipeline;
use crate::reconciler::Error;

pub(crate) const UDF_MODULE_ROOT: &str = "/var/run/streamforge/udfs";

fn engine_config_value(pipeline: &StreamforgePipeline) -> Result<serde_json::Value, Error> {
    let resource = serde_json::to_value(pipeline)?;
    streamforge_config_model::engine_config_value_from_pipeline_value(resource)
        .map_err(|error| Error::InvalidSpec(error.to_string()))
}

pub(crate) fn validate_pipeline_spec(pipeline: &StreamforgePipeline) -> Result<(), Error> {
    engine_config_value(pipeline).map(drop)
}

pub(crate) fn generate_config_yaml(pipeline: &StreamforgePipeline) -> Result<String, Error> {
    let config = engine_config_value(pipeline)?;
    serde_yaml::to_string(&config)
        .map_err(|error| Error::InvalidSpec(format!("Failed to serialize config: {error}")))
}
