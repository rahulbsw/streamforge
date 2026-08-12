//! Canonical Kubernetes `StreamforgePipeline` to engine configuration mapping.

use crate::config::MirrorMakerConfig;
use crate::config_model;
pub use crate::config_model::PipelineConversionError;
use serde_json::Value;

pub fn config_from_pipeline_value(
    value: Value,
) -> Result<MirrorMakerConfig, PipelineConversionError> {
    let engine_value = config_model::engine_config_value_from_pipeline_value(value)?;
    let config: MirrorMakerConfig =
        serde_json::from_value(engine_value).map_err(|error| PipelineConversionError {
            field: "spec".to_string(),
            message: format!("cannot build engine config: {error}"),
        })?;
    config.validate().map_err(|error| PipelineConversionError {
        field: "spec".to_string(),
        message: error.to_string(),
    })?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pipeline() -> Value {
        json!({
            "apiVersion": "streamforge.io/v1alpha1",
            "kind": "StreamforgePipeline",
            "metadata": {"name": "orders"},
            "spec": {
                "source": {"brokers": "source:9092", "topic": "orders"},
                "destinations": [
                    {"brokers": "target:9092", "topic": "us", "filter": "/region,==,us"},
                    {"brokers": "target:9092", "topic": "eu", "transform": "/payload"}
                ],
                "retry": {"maxAttempts": 4},
                "dlq": {"enabled": true, "topic": "orders-dlq"}
            }
        })
    }

    #[test]
    fn converts_multiple_destinations_and_reliability_fields() {
        let config = config_from_pipeline_value(pipeline()).unwrap();
        assert_eq!(config.appid, "orders");
        assert_eq!(config.routing.unwrap().destinations.len(), 2);
        assert_eq!(config.retry.max_attempts, 4);
        assert!(config.dlq.enabled);
    }

    #[test]
    fn rejects_mixed_destination_brokers() {
        let mut value = pipeline();
        value["spec"]["destinations"][1]["brokers"] = json!("other:9092");
        let error = config_from_pipeline_value(value).unwrap_err();
        assert_eq!(error.field, "spec.destinations[1].brokers");
    }

    #[test]
    fn projects_crd_udf_artifacts_and_bindings_without_loading_them() {
        let mut value = pipeline();
        value["spec"]["udfs"] = json!({
            "modules": [{
                "name": "allow-orders",
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "world": "filter",
                "artifact": {
                    "configMap": {
                        "name": "order-udfs",
                        "key": "allow.wasm"
                    }
                }
            }]
        });
        value["spec"]["destinations"][0]["udfs"] = json!({"filter": "allow-orders"});

        let config = config_from_pipeline_value(value).unwrap();
        let wasm = config.wasm.expect("CRD UDF registry should be projected");
        assert_eq!(
            wasm.module_root.to_string_lossy(),
            "/var/run/streamforge/udfs"
        );
        assert_eq!(
            wasm.modules[0].path.to_string_lossy(),
            "allow-orders/module.wasm"
        );
        let routing = config.routing.expect("routing should be projected");
        assert_eq!(
            routing.destinations[0]
                .udfs
                .as_ref()
                .and_then(|bindings| bindings.filter.as_deref()),
            Some("allow-orders")
        );
    }
}
