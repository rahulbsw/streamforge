use kube::ResourceExt;
use std::collections::BTreeMap;
use std::path::{Component, Path};

use crate::crd::{
    DestinationConfig, StreamforgePipeline, UdfArtifactRef, UdfBindings, UdfConfig, UdfModule,
    UdfRuntimeConfig, UdfWorld,
};
use crate::reconciler::Error;

pub(crate) const UDF_MODULE_ROOT: &str = "/var/run/streamforge/udfs";
const MAX_UDF_MODULES: usize = 32;
const MAX_UDF_NAME_LEN: usize = 59;

pub(crate) fn validate_pipeline_spec(pipeline: &StreamforgePipeline) -> Result<(), Error> {
    let spec = &pipeline.spec;

    if spec.destinations.is_empty() {
        return Err(Error::InvalidSpec("No destinations specified".to_string()));
    }

    let target_broker = &spec.destinations[0].brokers;
    if let Some((index, destination)) = spec
        .destinations
        .iter()
        .enumerate()
        .find(|(_, destination)| destination.brokers.as_str() != target_broker.as_str())
    {
        return Err(Error::InvalidSpec(format!(
            "destination {} uses broker '{}', but all destinations must use '{}' until per-destination brokers are supported",
            index, destination.brokers, target_broker
        )));
    }

    let Some(udfs) = &spec.udfs else {
        for (index, destination) in spec.destinations.iter().enumerate() {
            if has_udf_bindings(destination.udfs.as_ref()) {
                return Err(Error::InvalidSpec(format!(
                    "destination {} references UDFs but spec.udfs is not configured",
                    index
                )));
            }
        }
        return Ok(());
    };

    if udfs.modules.is_empty() || udfs.modules.len() > MAX_UDF_MODULES {
        return Err(Error::InvalidSpec(
            "spec.udfs.modules must contain between 1 and 32 modules".to_string(),
        ));
    }

    validate_udf_runtime(udfs)?;
    let mut modules = BTreeMap::new();
    for module in &udfs.modules {
        validate_udf_name(&module.name)?;
        if module.sha256.len() != 64
            || !module
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::InvalidSpec(format!(
                "UDF '{}' sha256 must be exactly 64 lowercase hexadecimal characters",
                module.name
            )));
        }
        if modules
            .insert(module.name.as_str(), &module.world)
            .is_some()
        {
            return Err(Error::InvalidSpec(format!(
                "duplicate UDF module name '{}'",
                module.name
            )));
        }

        match &module.artifact {
            UdfArtifactRef::ConfigMap { name, key } => {
                validate_kubernetes_resource_name(name, "ConfigMap")?;
                if key.is_empty()
                    || key.len() > 253
                    || key == "."
                    || key == ".."
                    || !key.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b'.'
                    })
                {
                    return Err(Error::InvalidSpec(format!(
                        "UDF '{}' ConfigMap key must be a safe file key",
                        module.name
                    )));
                }
            }
            UdfArtifactRef::PersistentVolumeClaim { claim_name, path } => {
                validate_kubernetes_resource_name(claim_name, "PersistentVolumeClaim")?;
                validate_relative_artifact_path(path, &module.name)?;
            }
        }
    }

    for (index, destination) in spec.destinations.iter().enumerate() {
        let Some(bindings) = destination.udfs.as_ref() else {
            continue;
        };
        validate_binding(
            index,
            "filter",
            bindings.filter.as_deref(),
            UdfWorld::Filter,
            &modules,
        )?;
        validate_binding(
            index,
            "valueTransform",
            bindings.value_transform.as_deref(),
            UdfWorld::ValueTransform,
            &modules,
        )?;
        validate_binding(
            index,
            "envelopeTransform",
            bindings.envelope_transform.as_deref(),
            UdfWorld::EnvelopeTransform,
            &modules,
        )?;
    }

    Ok(())
}

fn validate_udf_runtime(udfs: &UdfConfig) -> Result<(), Error> {
    let runtime = &udfs.runtime;
    validate_optional_limit("maxModuleBytes", runtime.max_module_bytes, 1, 268_435_456)?;
    validate_optional_limit("maxInputBytes", runtime.max_input_bytes, 1, 67_108_864)?;
    validate_optional_limit("maxOutputBytes", runtime.max_output_bytes, 1, 67_108_864)?;
    validate_optional_limit(
        "maxMemoryBytes",
        runtime.max_memory_bytes,
        65_536,
        536_870_912,
    )?;
    if runtime
        .max_memory_bytes
        .is_some_and(|bytes| bytes % 65_536 != 0)
    {
        return Err(Error::InvalidSpec(
            "spec.udfs.runtime.maxMemoryBytes must be a multiple of 65536".to_string(),
        ));
    }
    validate_optional_limit("maxTableElements", runtime.max_table_elements, 1, 1_000_000)?;
    validate_optional_limit("maxExecutionMs", runtime.max_execution_ms, 1, 60_000)?;
    validate_optional_limit("epochTickMs", runtime.epoch_tick_ms, 1, 1_000)?;
    let execution_ms = runtime.max_execution_ms.unwrap_or(5);
    let epoch_tick_ms = runtime.epoch_tick_ms.unwrap_or(1);
    if epoch_tick_ms > execution_ms {
        return Err(Error::InvalidSpec(
            "spec.udfs.runtime.epochTickMs must be <= maxExecutionMs".to_string(),
        ));
    }
    validate_optional_limit(
        "maxWasmStackBytes",
        runtime.max_wasm_stack_bytes,
        65_536,
        16_777_216,
    )?;
    validate_optional_limit(
        "maxConcurrentInstances",
        runtime.max_concurrent_instances.map(u64::from),
        1,
        1_024,
    )?;
    Ok(())
}

fn validate_optional_limit(
    field: &str,
    value: Option<u64>,
    minimum: u64,
    maximum: u64,
) -> Result<(), Error> {
    if value.is_some_and(|value| value < minimum || value > maximum) {
        return Err(Error::InvalidSpec(format!(
            "spec.udfs.runtime.{field} must be between {minimum} and {maximum}"
        )));
    }
    Ok(())
}

fn validate_binding<'a>(
    destination_index: usize,
    binding_name: &str,
    module_name: Option<&str>,
    expected_world: UdfWorld,
    modules: &BTreeMap<&'a str, &'a UdfWorld>,
) -> Result<(), Error> {
    let Some(module_name) = module_name else {
        return Ok(());
    };
    let actual_world = modules.get(module_name).ok_or_else(|| {
        Error::InvalidSpec(format!(
            "destination {} {} UDF references unknown module '{}'",
            destination_index, binding_name, module_name
        ))
    })?;
    if actual_world.as_str() != expected_world.as_str() {
        return Err(Error::InvalidSpec(format!(
            "destination {} {} UDF '{}' has world '{}', expected '{}'",
            destination_index,
            binding_name,
            module_name,
            actual_world.as_str(),
            expected_world.as_str()
        )));
    }
    Ok(())
}

fn has_udf_bindings(bindings: Option<&UdfBindings>) -> bool {
    bindings.is_some_and(|bindings| {
        bindings.filter.is_some()
            || bindings.value_transform.is_some()
            || bindings.envelope_transform.is_some()
    })
}

fn validate_udf_name(name: &str) -> Result<(), Error> {
    if name.is_empty()
        || name.len() > MAX_UDF_NAME_LEN
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || !name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !name
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
    {
        return Err(Error::InvalidSpec(format!(
            "UDF module name '{}' must be a lowercase DNS label no longer than {} characters",
            name, MAX_UDF_NAME_LEN
        )));
    }
    Ok(())
}

fn validate_kubernetes_resource_name(name: &str, kind: &str) -> Result<(), Error> {
    if name.is_empty()
        || name.len() > 253
        || !name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
        })
    {
        return Err(Error::InvalidSpec(format!(
            "{} name '{}' is not a safe Kubernetes DNS subdomain",
            kind, name
        )));
    }
    Ok(())
}

fn validate_relative_artifact_path(path: &str, module_name: &str) -> Result<(), Error> {
    if path.is_empty()
        || path.len() > 1024
        || path.contains('\\')
        || path
            .bytes()
            .any(|byte| byte == b'\0' || byte.is_ascii_control())
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(Error::InvalidSpec(format!(
            "UDF '{}' PVC path must be a safe relative path",
            module_name
        )));
    }
    Ok(())
}

pub(crate) fn generate_config_yaml(pipeline: &StreamforgePipeline) -> Result<String, Error> {
    validate_pipeline_spec(pipeline)?;
    let spec = &pipeline.spec;
    let target_broker = &spec.destinations[0].brokers;
    let destinations = spec
        .destinations
        .iter()
        .map(render_destination)
        .collect::<Vec<_>>();

    let mut config = serde_json::json!({
        "appid": spec.appid.clone().unwrap_or_else(|| pipeline.name_any()),
        "bootstrap": spec.source.brokers.clone(),
        "target_broker": target_broker,
        "input": spec.source.topic.clone(),
        "offset": spec.source.offset.clone(),
        "threads": spec.threads,
        "routing": {
            "routing_type": "filter",
            "destinations": destinations,
        },
    });

    if let Some(udfs) = &spec.udfs {
        config["wasm"] = render_wasm_config(udfs);
    }

    serde_yaml::to_string(&config)
        .map_err(|error| Error::InvalidSpec(format!("Failed to serialize config: {error}")))
}

fn render_destination(destination: &DestinationConfig) -> serde_json::Value {
    let mut rendered = serde_json::json!({
        "output": destination.topic,
    });
    if let Some(filter) = &destination.filter {
        rendered["filter"] = serde_json::json!(filter);
    }
    if let Some(transform) = &destination.transform {
        rendered["transform"] = serde_json::json!(transform);
    }
    if destination.partitioner.as_deref() == Some("field") {
        if let Some(field) = &destination.partitioner_field {
            rendered["partition"] = serde_json::json!(field);
        }
    }
    if let Some(bindings) = &destination.udfs {
        let mut rendered_bindings = serde_json::Map::new();
        if let Some(module) = &bindings.filter {
            rendered_bindings.insert("filter".to_string(), serde_json::json!(module));
        }
        if let Some(module) = &bindings.value_transform {
            rendered_bindings.insert("value_transform".to_string(), serde_json::json!(module));
        }
        if let Some(module) = &bindings.envelope_transform {
            rendered_bindings.insert("envelope_transform".to_string(), serde_json::json!(module));
        }
        if !rendered_bindings.is_empty() {
            rendered["udfs"] = serde_json::Value::Object(rendered_bindings);
        }
    }
    rendered
}

fn render_wasm_config(udfs: &UdfConfig) -> serde_json::Value {
    let mut modules = udfs.modules.iter().collect::<Vec<_>>();
    modules.sort_by(|left, right| left.name.cmp(&right.name));
    let modules = modules
        .into_iter()
        .map(|module| {
            serde_json::json!({
                "name": module.name,
                "path": module_relative_path(module),
                "sha256": module.sha256,
                "world": module.world.as_str(),
                "abi": module.abi.as_str(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "module_root": UDF_MODULE_ROOT,
        "runtime": render_runtime(&udfs.runtime),
        "modules": modules,
    })
}

fn render_runtime(runtime: &UdfRuntimeConfig) -> serde_json::Value {
    let mut rendered = serde_json::Map::new();
    macro_rules! insert_limit {
        ($field:ident) => {
            if let Some(value) = runtime.$field {
                rendered.insert(stringify!($field).to_string(), serde_json::json!(value));
            }
        };
    }
    insert_limit!(max_module_bytes);
    insert_limit!(max_input_bytes);
    insert_limit!(max_output_bytes);
    insert_limit!(max_memory_bytes);
    insert_limit!(max_table_elements);
    insert_limit!(max_execution_ms);
    insert_limit!(epoch_tick_ms);
    insert_limit!(max_wasm_stack_bytes);
    insert_limit!(max_concurrent_instances);
    serde_json::Value::Object(rendered)
}

fn module_relative_path(module: &UdfModule) -> String {
    match &module.artifact {
        UdfArtifactRef::ConfigMap { .. } => format!("{}/module.wasm", module.name),
        UdfArtifactRef::PersistentVolumeClaim { path, .. } => {
            format!("{}/{}", module.name, path)
        }
    }
}
