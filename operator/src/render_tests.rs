use crate::crd::StreamforgePipeline;
use crate::reconciler::Error;
use crate::render::{generate_config_yaml, validate_pipeline_spec};

fn pipeline_with_udfs() -> StreamforgePipeline {
    serde_json::from_value(serde_json::json!({
        "apiVersion": "streamforge.io/v1alpha1",
        "kind": "StreamforgePipeline",
        "metadata": {"name": "orders"},
        "spec": {
            "source": {
                "brokers": "source:9092",
                "topic": "orders",
                "groupId": "ignored-by-core"
            },
            "destinations": [
                {
                    "brokers": "target:9092",
                    "topic": "accepted",
                    "filter": "value.total > 0",
                    "udfs": {"filter": "allow-orders"}
                },
                {
                    "brokers": "target:9092",
                    "topic": "redacted",
                    "transform": "remove(value, \"ssn\")",
                    "partitioner": "field",
                    "partitionerField": "customer_id",
                    "udfs": {"valueTransform": "redact-orders"}
                }
            ],
            "udfs": {
                "runtime": {
                    "maxMemoryBytes": 67108864,
                    "maxExecutionMs": 10,
                    "maxConcurrentInstances": 8
                },
                "modules": [
                    {
                        "name": "redact-orders",
                        "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                        "world": "value_transform",
                        "artifact": {
                            "persistentVolumeClaim": {
                                "claimName": "udf-artifacts",
                                "path": "release/redact.wasm"
                            }
                        }
                    },
                    {
                        "name": "allow-orders",
                        "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "world": "filter",
                        "artifact": {
                            "configMap": {
                                "name": "order-udfs",
                                "key": "allow.wasm"
                            }
                        }
                    }
                ]
            }
        }
    }))
    .unwrap()
}

#[test]
fn generated_config_preserves_destinations_and_matches_core_udf_shape() {
    let config = generate_config_yaml(&pipeline_with_udfs()).unwrap();
    let rendered: serde_json::Value = serde_yaml::from_str(&config).unwrap();

    assert!(rendered.get("output").is_none());
    assert!(rendered.get("filter").is_none());
    assert!(rendered.get("group_id").is_none());
    assert_eq!(rendered["target_broker"], "target:9092");

    let destinations = rendered["routing"]["destinations"].as_array().unwrap();
    assert_eq!(destinations.len(), 2);
    assert_eq!(destinations[0]["output"], "accepted");
    assert_eq!(destinations[0]["filter"], "value.total > 0");
    assert_eq!(destinations[0]["udfs"]["filter"], "allow-orders");
    assert_eq!(destinations[1]["output"], "redacted");
    assert_eq!(destinations[1]["partition"], "customer_id");
    assert_eq!(destinations[1]["udfs"]["value_transform"], "redact-orders");

    let wasm = &rendered["wasm"];
    assert_eq!(wasm["module_root"], "/var/run/streamforge/udfs");
    assert_eq!(wasm["modules"][0]["name"], "allow-orders");
    assert_eq!(wasm["modules"][0]["path"], "allow-orders/module.wasm");
    assert_eq!(wasm["modules"][0]["world"], "filter");
    assert_eq!(wasm["modules"][0]["abi"], "v1");
    assert_eq!(wasm["modules"][1]["name"], "redact-orders");
    assert_eq!(
        wasm["modules"][1]["path"],
        "redact-orders/release/redact.wasm"
    );
    assert_eq!(wasm["runtime"]["max_memory_bytes"], 67_108_864);
    assert_eq!(wasm["runtime"]["max_execution_ms"], 10);
    assert_eq!(wasm["runtime"]["max_concurrent_instances"], 8);
    assert!(wasm["runtime"].get("maxMemoryBytes").is_none());
}

#[test]
fn rejects_destinations_on_different_brokers() {
    let mut pipeline = pipeline_with_udfs();
    pipeline.spec.destinations[1].brokers = "other-target:9092".to_string();

    let error = validate_pipeline_spec(&pipeline).unwrap_err();
    assert!(matches!(
        error,
        Error::InvalidSpec(message)
            if message.contains("all destinations must use 'target:9092'")
    ));
}

#[test]
fn rejects_unsafe_artifacts_and_invalid_bindings() {
    let mut pipeline = pipeline_with_udfs();
    pipeline.spec.udfs.as_mut().unwrap().modules[0].sha256 =
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string();
    assert!(validate_pipeline_spec(&pipeline)
        .unwrap_err()
        .to_string()
        .contains("64 lowercase hexadecimal"));

    let mut pipeline = pipeline_with_udfs();
    let crate::crd::UdfArtifactRef::PersistentVolumeClaim { path, .. } =
        &mut pipeline.spec.udfs.as_mut().unwrap().modules[0].artifact
    else {
        panic!("fixture must use a PVC");
    };
    *path = "../redact.wasm".to_string();
    assert!(validate_pipeline_spec(&pipeline)
        .unwrap_err()
        .to_string()
        .contains("safe relative path"));

    let mut pipeline = pipeline_with_udfs();
    pipeline.spec.destinations[0].udfs.as_mut().unwrap().filter = Some("redact-orders".to_string());
    assert!(validate_pipeline_spec(&pipeline)
        .unwrap_err()
        .to_string()
        .contains("expected 'filter'"));
}

#[test]
fn rejects_runtime_limits_the_core_would_reject() {
    let mut pipeline = pipeline_with_udfs();
    let runtime = &mut pipeline.spec.udfs.as_mut().unwrap().runtime;
    runtime.max_memory_bytes = Some(65_537);
    assert!(validate_pipeline_spec(&pipeline)
        .unwrap_err()
        .to_string()
        .contains("multiple of 65536"));

    let mut pipeline = pipeline_with_udfs();
    let runtime = &mut pipeline.spec.udfs.as_mut().unwrap().runtime;
    runtime.max_execution_ms = Some(5);
    runtime.epoch_tick_ms = Some(6);
    assert!(validate_pipeline_spec(&pipeline)
        .unwrap_err()
        .to_string()
        .contains("epochTickMs"));

    let mut pipeline = pipeline_with_udfs();
    pipeline
        .spec
        .udfs
        .as_mut()
        .unwrap()
        .runtime
        .max_concurrent_instances = Some(1_025);
    assert!(validate_pipeline_spec(&pipeline)
        .unwrap_err()
        .to_string()
        .contains("maxConcurrentInstances"));
}
