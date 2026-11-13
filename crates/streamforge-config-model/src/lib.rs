//! Shared `StreamforgePipeline` to engine configuration projection.

use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

const UDF_MODULE_ROOT: &str = "/var/run/streamforge/udfs";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineConversionError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for PipelineConversionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for PipelineConversionError {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PipelineDocument {
    api_version: String,
    kind: String,
    metadata: Metadata,
    spec: PipelineSpec,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PipelineSpec {
    appid: Option<String>,
    source: Source,
    destinations: Vec<Destination>,
    #[serde(default)]
    udfs: Option<UdfConfig>,
    #[serde(default = "default_threads")]
    threads: usize,
    #[serde(default)]
    retry: Retry,
    #[serde(default)]
    dlq: Dlq,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    brokers: String,
    topic: String,
    #[serde(default = "default_offset")]
    offset: String,
    #[serde(default)]
    security: Option<SecurityConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Destination {
    brokers: String,
    topic: String,
    filter: Option<String>,
    transform: Option<String>,
    partitioner: Option<String>,
    partitioner_field: Option<String>,
    #[serde(default)]
    udfs: Option<UdfBindings>,
    #[serde(default)]
    security: Option<SecurityConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecurityConfig {
    #[serde(default = "default_security_protocol")]
    protocol: String,
    #[serde(default)]
    ssl: Option<SslConfig>,
    #[serde(default)]
    sasl: Option<SaslConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecretReference {
    name: String,
    key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SslConfig {
    ca_location: Option<String>,
    certificate_location: Option<String>,
    key_location: Option<String>,
    key_password: Option<String>,
    ca_secret: Option<SecretReference>,
    certificate_secret: Option<SecretReference>,
    key_secret: Option<SecretReference>,
    key_password_secret: Option<SecretReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaslConfig {
    mechanism: String,
    username: Option<String>,
    password: Option<String>,
    kerberos_service_name: Option<String>,
    username_secret: Option<SecretReference>,
    password_secret: Option<SecretReference>,
    keytab_secret: Option<SecretReference>,
}

#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Retry {
    max_attempts: u32,
    initial_delay_ms: u64,
    max_delay_ms: u64,
    multiplier: f64,
}

impl Default for Retry {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 30_000,
            multiplier: 2.0,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Dlq {
    enabled: bool,
    topic: String,
    include_headers: bool,
    max_dlq_retries: u32,
}

impl Default for Dlq {
    fn default() -> Self {
        Self {
            enabled: false,
            topic: "streamforge-dlq".to_string(),
            include_headers: true,
            max_dlq_retries: 3,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UdfConfig {
    #[serde(default)]
    runtime: UdfRuntime,
    modules: Vec<UdfModule>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UdfRuntime {
    max_module_bytes: Option<u64>,
    max_input_bytes: Option<u64>,
    max_output_bytes: Option<u64>,
    max_memory_bytes: Option<u64>,
    max_table_elements: Option<u64>,
    max_execution_ms: Option<u64>,
    epoch_tick_ms: Option<u64>,
    max_wasm_stack_bytes: Option<u64>,
    max_concurrent_instances: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UdfModule {
    name: String,
    sha256: String,
    world: String,
    #[serde(default = "default_udf_abi")]
    abi: String,
    artifact: UdfArtifact,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum UdfArtifact {
    ConfigMap {
        name: String,
        key: String,
    },
    PersistentVolumeClaim {
        #[serde(rename = "claimName")]
        claim_name: String,
        path: String,
    },
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UdfBindings {
    filter: Option<String>,
    value_transform: Option<String>,
    envelope_transform: Option<String>,
}

fn default_threads() -> usize {
    4
}

fn default_offset() -> String {
    "latest".to_string()
}

fn default_udf_abi() -> String {
    "v1".to_string()
}

fn default_security_protocol() -> String {
    "PLAINTEXT".to_string()
}

fn invalid(field: impl Into<String>, message: impl Into<String>) -> PipelineConversionError {
    PipelineConversionError {
        field: field.into(),
        message: message.into(),
    }
}

pub fn engine_config_value_from_pipeline_value(
    value: Value,
) -> Result<Value, PipelineConversionError> {
    let pipeline: PipelineDocument = serde_json::from_value(value)
        .map_err(|error| invalid("$", format!("invalid StreamforgePipeline: {error}")))?;

    if pipeline.api_version != "streamforge.io/v1alpha1" {
        return Err(invalid("apiVersion", "must be streamforge.io/v1alpha1"));
    }
    if pipeline.kind != "StreamforgePipeline" {
        return Err(invalid("kind", "must be StreamforgePipeline"));
    }
    if pipeline.spec.threads == 0 {
        return Err(invalid("spec.threads", "must be greater than zero"));
    }
    if pipeline.spec.destinations.is_empty() {
        return Err(invalid(
            "spec.destinations",
            "must contain at least one destination",
        ));
    }
    if pipeline.spec.source.brokers.trim().is_empty() {
        return Err(invalid("spec.source.brokers", "must not be empty"));
    }
    if pipeline.spec.source.topic.trim().is_empty() {
        return Err(invalid("spec.source.topic", "must not be empty"));
    }

    let target_broker = pipeline.spec.destinations[0].brokers.trim();
    if target_broker.is_empty() {
        return Err(invalid("spec.destinations[0].brokers", "must not be empty"));
    }
    for (index, destination) in pipeline.spec.destinations.iter().enumerate() {
        if destination.brokers != target_broker {
            return Err(invalid(
                format!("spec.destinations[{index}].brokers"),
                format!(
                    "destination {index} uses broker '{}', but all destinations must use '{target_broker}' until per-destination brokers are supported",
                    destination.brokers
                ),
            ));
        }
        if destination.topic.trim().is_empty() {
            return Err(invalid(
                format!("spec.destinations[{index}].topic"),
                "must not be empty",
            ));
        }
    }

    if pipeline.spec.retry.max_attempts == 0 {
        return Err(invalid(
            "spec.retry.maxAttempts",
            "must be greater than zero",
        ));
    }
    if pipeline.spec.retry.multiplier < 1.0 {
        return Err(invalid("spec.retry.multiplier", "must be at least 1.0"));
    }
    if pipeline.spec.retry.initial_delay_ms > pipeline.spec.retry.max_delay_ms {
        return Err(invalid(
            "spec.retry.maxDelayMs",
            "must be greater than or equal to initialDelayMs",
        ));
    }
    if pipeline.spec.dlq.enabled && pipeline.spec.dlq.topic.trim().is_empty() {
        return Err(invalid(
            "spec.dlq.topic",
            "must not be empty when DLQ is enabled",
        ));
    }
    if pipeline.spec.dlq.enabled && pipeline.spec.dlq.max_dlq_retries == 0 {
        return Err(invalid(
            "spec.dlq.maxDlqRetries",
            "must be greater than zero when DLQ is enabled",
        ));
    }

    let (rendered_wasm, udf_worlds) = match pipeline.spec.udfs.as_ref() {
        Some(udfs) => {
            let (wasm, worlds) = validate_and_render_udfs(udfs)?;
            (Some(wasm), worlds)
        }
        None => (None, BTreeMap::new()),
    };
    if rendered_wasm.is_none()
        && pipeline
            .spec
            .destinations
            .iter()
            .any(|destination| has_udf_bindings(destination.udfs.as_ref()))
    {
        return Err(invalid(
            "spec.destinations[].udfs",
            "requires spec.udfs to define the referenced modules",
        ));
    }
    validate_udf_bindings(&pipeline.spec.destinations, &udf_worlds)?;

    let source_security = pipeline
        .spec
        .source
        .security
        .as_ref()
        .map(|security| render_security(security, "spec.source.security", "source"))
        .transpose()?;
    let target_security = render_target_security(&pipeline.spec.destinations)?;

    let appid = pipeline
        .spec
        .appid
        .or(pipeline.metadata.name)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| invalid("metadata.name", "is required when spec.appid is omitted"))?;
    let target_broker = target_broker.to_string();
    let destinations: Vec<Value> = pipeline
        .spec
        .destinations
        .iter()
        .map(|destination| {
            let mut rendered = serde_json::Map::new();
            rendered.insert("output".to_string(), json!(destination.topic));
            if let Some(filter) = &destination.filter {
                rendered.insert("filter".to_string(), json!(filter));
            }
            if let Some(transform) = &destination.transform {
                rendered.insert("transform".to_string(), json!(transform));
            }
            if destination.partitioner.as_deref() == Some("field") {
                if let Some(field) = &destination.partitioner_field {
                    rendered.insert("partition".to_string(), json!(field));
                }
            }
            if let Some(bindings) = render_udf_bindings(destination.udfs.as_ref()) {
                rendered.insert("udfs".to_string(), bindings);
            }
            Value::Object(rendered)
        })
        .collect();

    let mut config = json!({
        "appid": appid,
        "bootstrap": pipeline.spec.source.brokers,
        "input": pipeline.spec.source.topic,
        "target_broker": target_broker,
        "offset": pipeline.spec.source.offset,
        "threads": pipeline.spec.threads,
        "routing": {
            "routing_type": "filter",
            "path": Value::Null,
            "destinations": destinations
        },
        "retry": {
            "max_attempts": pipeline.spec.retry.max_attempts,
            "initial_delay_ms": pipeline.spec.retry.initial_delay_ms,
            "max_delay_ms": pipeline.spec.retry.max_delay_ms,
            "multiplier": pipeline.spec.retry.multiplier,
            "jitter": 0.1
        },
        "dlq": {
            "enabled": pipeline.spec.dlq.enabled,
            "topic": pipeline.spec.dlq.topic,
            "brokers": Value::Null,
            "include_original_headers": pipeline.spec.dlq.include_headers,
            "include_stack_trace": false,
            "max_dlq_retries": pipeline.spec.dlq.max_dlq_retries,
            "compression": Value::Null
        }
    });
    if let Some(wasm) = rendered_wasm {
        config["wasm"] = wasm;
    }
    if let Some(security) = source_security {
        config["security"] = security;
    }
    if let Some(security) = target_security {
        config["target_security"] = security;
    }
    Ok(config)
}

fn render_target_security(
    destinations: &[Destination],
) -> Result<Option<Value>, PipelineConversionError> {
    let expected = destinations[0].security.as_ref();
    for (index, destination) in destinations.iter().enumerate().skip(1) {
        if destination.security.as_ref() != expected {
            return Err(invalid(
                format!("spec.destinations[{index}].security"),
                "must match spec.destinations[0].security because all destinations share one target Kafka client",
            ));
        }
    }
    expected
        .map(|security| render_security(security, "spec.destinations[0].security", "destination-0"))
        .transpose()
}

fn render_security(
    security: &SecurityConfig,
    field: &str,
    mount_role: &str,
) -> Result<Value, PipelineConversionError> {
    if !matches!(
        security.protocol.as_str(),
        "PLAINTEXT" | "SSL" | "SASL_PLAINTEXT" | "SASL_SSL"
    ) {
        return Err(invalid(
            format!("{field}.protocol"),
            "must be PLAINTEXT, SSL, SASL_PLAINTEXT, or SASL_SSL",
        ));
    }
    if security.protocol == "PLAINTEXT" && (security.ssl.is_some() || security.sasl.is_some()) {
        return Err(invalid(
            field,
            "PLAINTEXT must not include SSL or SASL settings",
        ));
    }
    if matches!(security.protocol.as_str(), "SSL" | "SASL_SSL") && security.ssl.is_none() {
        return Err(invalid(
            format!("{field}.ssl"),
            "is required for SSL protocols",
        ));
    }
    if matches!(security.protocol.as_str(), "SASL_PLAINTEXT" | "SASL_SSL")
        && security.sasl.is_none()
    {
        return Err(invalid(
            format!("{field}.sasl"),
            "is required for SASL protocols",
        ));
    }

    let mut rendered = serde_json::Map::new();
    rendered.insert("protocol".to_string(), json!(security.protocol));
    if let Some(ssl) = security.ssl.as_ref() {
        if ssl.key_password.is_some() {
            return Err(invalid(
                format!("{field}.ssl.keyPassword"),
                "inline credentials are not allowed; use keyPasswordSecret",
            ));
        }
        let mut value = serde_json::Map::new();
        insert_optional_file(
            &mut value,
            "ca_location",
            ssl.ca_location.as_deref(),
            ssl.ca_secret.as_ref(),
            &format!("{field}.ssl.caSecret"),
            mount_role,
        )?;
        insert_optional_file(
            &mut value,
            "certificate_location",
            ssl.certificate_location.as_deref(),
            ssl.certificate_secret.as_ref(),
            &format!("{field}.ssl.certificateSecret"),
            mount_role,
        )?;
        insert_optional_file(
            &mut value,
            "key_location",
            ssl.key_location.as_deref(),
            ssl.key_secret.as_ref(),
            &format!("{field}.ssl.keySecret"),
            mount_role,
        )?;
        if let Some(secret) = ssl.key_password_secret.as_ref() {
            value.insert(
                "key_password_file".to_string(),
                json!(secret_file_path(
                    secret,
                    &format!("{field}.ssl.keyPasswordSecret"),
                    mount_role
                )?),
            );
        }
        rendered.insert("ssl".to_string(), Value::Object(value));
    }
    if let Some(sasl) = security.sasl.as_ref() {
        if sasl.username.is_some() {
            return Err(invalid(
                format!("{field}.sasl.username"),
                "inline credentials are not allowed; use usernameSecret",
            ));
        }
        if sasl.password.is_some() {
            return Err(invalid(
                format!("{field}.sasl.password"),
                "inline credentials are not allowed; use passwordSecret",
            ));
        }
        if !matches!(
            sasl.mechanism.as_str(),
            "PLAIN" | "SCRAM-SHA-256" | "SCRAM-SHA-512" | "GSSAPI"
        ) {
            return Err(invalid(
                format!("{field}.sasl.mechanism"),
                "must be PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, or GSSAPI",
            ));
        }
        if matches!(
            sasl.mechanism.as_str(),
            "PLAIN" | "SCRAM-SHA-256" | "SCRAM-SHA-512"
        ) {
            if sasl.username_secret.is_none() {
                return Err(invalid(
                    format!("{field}.sasl.usernameSecret"),
                    "is required for PLAIN and SCRAM mechanisms",
                ));
            }
            if sasl.password_secret.is_none() {
                return Err(invalid(
                    format!("{field}.sasl.passwordSecret"),
                    "is required for PLAIN and SCRAM mechanisms",
                ));
            }
        }
        if sasl.mechanism == "GSSAPI" {
            if sasl.kerberos_service_name.is_none() {
                return Err(invalid(
                    format!("{field}.sasl.kerberosServiceName"),
                    "is required for GSSAPI",
                ));
            }
            if sasl.keytab_secret.is_none() {
                return Err(invalid(
                    format!("{field}.sasl.keytabSecret"),
                    "is required for GSSAPI",
                ));
            }
        }
        let mut value = serde_json::Map::new();
        value.insert("mechanism".to_string(), json!(sasl.mechanism));
        if let Some(secret) = sasl.username_secret.as_ref() {
            value.insert(
                "username_file".to_string(),
                json!(secret_file_path(
                    secret,
                    &format!("{field}.sasl.usernameSecret"),
                    mount_role
                )?),
            );
        }
        if let Some(secret) = sasl.password_secret.as_ref() {
            value.insert(
                "password_file".to_string(),
                json!(secret_file_path(
                    secret,
                    &format!("{field}.sasl.passwordSecret"),
                    mount_role
                )?),
            );
        }
        if let Some(service_name) = sasl.kerberos_service_name.as_ref() {
            value.insert("kerberos_service_name".to_string(), json!(service_name));
        }
        if let Some(secret) = sasl.keytab_secret.as_ref() {
            value.insert(
                "kerberos_keytab".to_string(),
                json!(secret_file_path(
                    secret,
                    &format!("{field}.sasl.keytabSecret"),
                    mount_role
                )?),
            );
        }
        rendered.insert("sasl".to_string(), Value::Object(value));
    }
    Ok(Value::Object(rendered))
}

fn insert_optional_file(
    rendered: &mut serde_json::Map<String, Value>,
    output_field: &str,
    location: Option<&str>,
    secret: Option<&SecretReference>,
    field: &str,
    mount_role: &str,
) -> Result<(), PipelineConversionError> {
    if location.is_some() && secret.is_some() {
        return Err(invalid(
            field,
            "cannot be combined with the corresponding location field",
        ));
    }
    if let Some(location) = location {
        rendered.insert(output_field.to_string(), json!(location));
    } else if let Some(secret) = secret {
        rendered.insert(
            output_field.to_string(),
            json!(secret_file_path(secret, field, mount_role)?),
        );
    }
    Ok(())
}

fn secret_file_path(
    secret: &SecretReference,
    field: &str,
    mount_role: &str,
) -> Result<String, PipelineConversionError> {
    if !is_kubernetes_resource_name(&secret.name) {
        return Err(invalid(
            format!("{field}.name"),
            "must be a valid Kubernetes DNS subdomain",
        ));
    }
    if !is_safe_secret_key(&secret.key) {
        return Err(invalid(
            format!("{field}.key"),
            "must be a safe Kubernetes Secret key",
        ));
    }
    Ok(format!(
        "/etc/streamforge/secrets/{mount_role}/{}/{}",
        secret.name, secret.key
    ))
}

fn is_safe_secret_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 253
        && key != "."
        && key != ".."
        && key.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b'.'
        })
}

fn has_udf_bindings(bindings: Option<&UdfBindings>) -> bool {
    bindings.is_some_and(|bindings| {
        bindings.filter.is_some()
            || bindings.value_transform.is_some()
            || bindings.envelope_transform.is_some()
    })
}

fn render_udf_bindings(bindings: Option<&UdfBindings>) -> Option<Value> {
    let bindings = bindings?;
    let mut rendered = serde_json::Map::new();
    if let Some(module) = &bindings.filter {
        rendered.insert("filter".to_string(), json!(module));
    }
    if let Some(module) = &bindings.value_transform {
        rendered.insert("value_transform".to_string(), json!(module));
    }
    if let Some(module) = &bindings.envelope_transform {
        rendered.insert("envelope_transform".to_string(), json!(module));
    }
    (!rendered.is_empty()).then_some(Value::Object(rendered))
}

fn validate_and_render_udfs(
    udfs: &UdfConfig,
) -> Result<(Value, BTreeMap<String, String>), PipelineConversionError> {
    if udfs.modules.is_empty() || udfs.modules.len() > 32 {
        return Err(invalid(
            "spec.udfs.modules",
            "must contain between 1 and 32 modules",
        ));
    }
    let mut names = BTreeSet::new();
    let mut digests = BTreeSet::new();
    let mut worlds = BTreeMap::new();
    let mut modules = Vec::with_capacity(udfs.modules.len());
    for (index, module) in udfs.modules.iter().enumerate() {
        if module.name.is_empty()
            || module.name.len() > 59
            || !module
                .name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            || !module
                .name
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            || !module
                .name
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
        {
            return Err(invalid(
                format!("spec.udfs.modules[{index}].name"),
                "must be a lowercase DNS label no longer than 59 characters",
            ));
        }
        if !names.insert(module.name.as_str()) {
            return Err(invalid(
                format!("spec.udfs.modules[{index}].name"),
                "must be unique",
            ));
        }
        if module.sha256.len() != 64
            || !module
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid(
                format!("spec.udfs.modules[{index}].sha256"),
                "must be exactly 64 lowercase hexadecimal characters",
            ));
        }
        if !digests.insert(module.sha256.as_str()) {
            return Err(invalid(
                format!("spec.udfs.modules[{index}].sha256"),
                "must be unique because Kubernetes module paths are name-scoped",
            ));
        }
        if !matches!(
            module.world.as_str(),
            "filter" | "value_transform" | "envelope_transform"
        ) {
            return Err(invalid(
                format!("spec.udfs.modules[{index}].world"),
                "must be filter, value_transform, or envelope_transform",
            ));
        }
        if module.abi != "v1" {
            return Err(invalid(
                format!("spec.udfs.modules[{index}].abi"),
                "must be v1",
            ));
        }
        worlds.insert(module.name.clone(), module.world.clone());
        let relative_path = match &module.artifact {
            UdfArtifact::ConfigMap { name, key } => {
                if !is_kubernetes_resource_name(name) {
                    return Err(invalid(
                        format!("spec.udfs.modules[{index}].artifact.configMap.name"),
                        "must be a valid Kubernetes DNS subdomain",
                    ));
                }
                if key.is_empty()
                    || key.len() > 253
                    || key == "."
                    || key == ".."
                    || !key.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b'.'
                    })
                {
                    return Err(invalid(
                        format!("spec.udfs.modules[{index}].artifact.configMap.key"),
                        "must be a safe ConfigMap file key",
                    ));
                }
                format!("{}/module.wasm", module.name)
            }
            UdfArtifact::PersistentVolumeClaim { claim_name, path } => {
                if !is_kubernetes_resource_name(claim_name) {
                    return Err(invalid(
                        format!(
                            "spec.udfs.modules[{index}].artifact.persistentVolumeClaim.claimName"
                        ),
                        "must be a valid Kubernetes DNS subdomain",
                    ));
                }
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
                    return Err(invalid(
                        format!("spec.udfs.modules[{index}].artifact.persistentVolumeClaim.path"),
                        "must be a safe relative path",
                    ));
                }
                format!("{}/{}", module.name, path)
            }
        };
        modules.push(json!({
            "name": module.name,
            "path": relative_path,
            "sha256": module.sha256,
            "world": module.world,
            "abi": module.abi
        }));
    }
    modules.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));

    let runtime = &udfs.runtime;
    validate_optional_runtime_limit("maxModuleBytes", runtime.max_module_bytes, 1, 268_435_456)?;
    validate_optional_runtime_limit("maxInputBytes", runtime.max_input_bytes, 1, 67_108_864)?;
    validate_optional_runtime_limit("maxOutputBytes", runtime.max_output_bytes, 1, 67_108_864)?;
    validate_optional_runtime_limit(
        "maxMemoryBytes",
        runtime.max_memory_bytes,
        65_536,
        536_870_912,
    )?;
    if runtime
        .max_memory_bytes
        .is_some_and(|bytes| bytes % 65_536 != 0)
    {
        return Err(invalid(
            "spec.udfs.runtime.maxMemoryBytes",
            "must be a multiple of 65536",
        ));
    }
    validate_optional_runtime_limit("maxTableElements", runtime.max_table_elements, 1, 1_000_000)?;
    validate_optional_runtime_limit("maxExecutionMs", runtime.max_execution_ms, 1, 60_000)?;
    validate_optional_runtime_limit("epochTickMs", runtime.epoch_tick_ms, 1, 1_000)?;
    if runtime.epoch_tick_ms.unwrap_or(1) > runtime.max_execution_ms.unwrap_or(5) {
        return Err(invalid(
            "spec.udfs.runtime.epochTickMs",
            "must be less than or equal to maxExecutionMs",
        ));
    }
    validate_optional_runtime_limit(
        "maxWasmStackBytes",
        runtime.max_wasm_stack_bytes,
        65_536,
        16_777_216,
    )?;
    validate_optional_runtime_limit(
        "maxConcurrentInstances",
        runtime.max_concurrent_instances.map(u64::from),
        1,
        1_024,
    )?;

    let mut rendered_runtime = serde_json::Map::new();
    macro_rules! insert_runtime {
        ($field:ident) => {
            if let Some(value) = runtime.$field {
                rendered_runtime.insert(stringify!($field).to_string(), json!(value));
            }
        };
    }
    insert_runtime!(max_module_bytes);
    insert_runtime!(max_input_bytes);
    insert_runtime!(max_output_bytes);
    insert_runtime!(max_memory_bytes);
    insert_runtime!(max_table_elements);
    insert_runtime!(max_execution_ms);
    insert_runtime!(epoch_tick_ms);
    insert_runtime!(max_wasm_stack_bytes);
    insert_runtime!(max_concurrent_instances);

    Ok((
        json!({
            "module_root": UDF_MODULE_ROOT,
            "runtime": rendered_runtime,
            "modules": modules
        }),
        worlds,
    ))
}

fn validate_optional_runtime_limit(
    field: &str,
    value: Option<u64>,
    minimum: u64,
    maximum: u64,
) -> Result<(), PipelineConversionError> {
    if value.is_some_and(|value| value < minimum || value > maximum) {
        return Err(invalid(
            format!("spec.udfs.runtime.{field}"),
            format!("must be between {minimum} and {maximum}"),
        ));
    }
    Ok(())
}

fn is_kubernetes_resource_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|label| {
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
}

fn validate_udf_bindings(
    destinations: &[Destination],
    worlds: &BTreeMap<String, String>,
) -> Result<(), PipelineConversionError> {
    for (index, destination) in destinations.iter().enumerate() {
        let Some(bindings) = destination.udfs.as_ref() else {
            continue;
        };
        for (field, module_name, expected_world) in [
            ("filter", bindings.filter.as_deref(), "filter"),
            (
                "valueTransform",
                bindings.value_transform.as_deref(),
                "value_transform",
            ),
            (
                "envelopeTransform",
                bindings.envelope_transform.as_deref(),
                "envelope_transform",
            ),
        ] {
            let Some(module_name) = module_name else {
                continue;
            };
            let actual_world = worlds.get(module_name).ok_or_else(|| {
                invalid(
                    format!("spec.destinations[{index}].udfs.{field}"),
                    format!("references unknown module '{module_name}'"),
                )
            })?;
            if actual_world != expected_world {
                return Err(invalid(
                    format!("spec.destinations[{index}].udfs.{field}"),
                    format!(
                        "module '{module_name}' has world '{actual_world}', expected '{expected_world}'"
                    ),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pipeline_with_udfs() -> Value {
        json!({
            "apiVersion": "streamforge.io/v1alpha1",
            "kind": "StreamforgePipeline",
            "metadata": {"name": "orders"},
            "spec": {
                "source": {"brokers": "source:9092", "topic": "orders"},
                "destinations": [{
                    "brokers": "target:9092",
                    "topic": "accepted",
                    "udfs": {"filter": "allow-orders"}
                }],
                "udfs": {
                    "modules": [{
                        "name": "allow-orders",
                        "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "world": "filter",
                        "artifact": {
                            "configMap": {"name": "order-udfs", "key": "allow.wasm"}
                        }
                    }]
                }
            }
        })
    }

    #[test]
    fn rejects_name_scoped_duplicate_digests() {
        let mut pipeline = pipeline_with_udfs();
        pipeline["spec"]["udfs"]["modules"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "name": "redact-orders",
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "world": "value_transform",
                "artifact": {
                    "configMap": {"name": "order-udfs", "key": "redact.wasm"}
                }
            }));

        let error = engine_config_value_from_pipeline_value(pipeline).unwrap_err();
        assert_eq!(error.field, "spec.udfs.modules[1].sha256");
        assert!(error.message.contains("must be unique"));
    }

    #[test]
    fn rejects_unsafe_kubernetes_artifact_names() {
        let mut pipeline = pipeline_with_udfs();
        pipeline["spec"]["udfs"]["modules"][0]["artifact"]["configMap"]["name"] =
            json!("../order-udfs");

        let error = engine_config_value_from_pipeline_value(pipeline).unwrap_err();
        assert_eq!(error.field, "spec.udfs.modules[0].artifact.configMap.name");
    }

    #[test]
    fn rejects_binding_world_mismatches_with_field_diagnostics() {
        let mut pipeline = pipeline_with_udfs();
        pipeline["spec"]["udfs"]["modules"][0]["world"] = json!("value_transform");

        let error = engine_config_value_from_pipeline_value(pipeline).unwrap_err();
        assert_eq!(error.field, "spec.destinations[0].udfs.filter");
        assert!(error.message.contains("expected 'filter'"));
    }

    #[test]
    fn renders_source_and_target_secret_references_as_mounted_files() {
        let mut pipeline = pipeline_with_udfs();
        pipeline["spec"]["source"]["security"] = json!({
            "protocol": "SASL_SSL",
            "ssl": {
                "caSecret": {"name": "source-ca", "key": "ca.crt"}
            },
            "sasl": {
                "mechanism": "SCRAM-SHA-512",
                "usernameSecret": {"name": "source-auth", "key": "username"},
                "passwordSecret": {"name": "source-auth", "key": "password"}
            }
        });
        pipeline["spec"]["destinations"][0]["security"] = json!({
            "protocol": "SSL",
            "ssl": {
                "caSecret": {"name": "target-ca", "key": "ca.crt"},
                "certificateSecret": {"name": "target-mtls", "key": "tls.crt"},
                "keySecret": {"name": "target-mtls", "key": "tls.key"}
            }
        });

        let config = engine_config_value_from_pipeline_value(pipeline).unwrap();
        assert_eq!(
            config["security"]["ssl"]["ca_location"],
            "/etc/streamforge/secrets/source/source-ca/ca.crt"
        );
        assert_eq!(
            config["security"]["sasl"]["password_file"],
            "/etc/streamforge/secrets/source/source-auth/password"
        );
        assert_eq!(
            config["target_security"]["ssl"]["key_location"],
            "/etc/streamforge/secrets/destination-0/target-mtls/tls.key"
        );
        assert!(!config.to_string().contains("\"password\":"));
    }

    #[test]
    fn rejects_inline_credentials_and_unsafe_secret_keys() {
        let mut pipeline = pipeline_with_udfs();
        pipeline["spec"]["source"]["security"] = json!({
            "protocol": "SASL_PLAINTEXT",
            "sasl": {
                "mechanism": "PLAIN",
                "username": "inline-user"
            }
        });
        let error = engine_config_value_from_pipeline_value(pipeline).unwrap_err();
        assert_eq!(error.field, "spec.source.security.sasl.username");

        let mut pipeline = pipeline_with_udfs();
        pipeline["spec"]["source"]["security"] = json!({
            "protocol": "SSL",
            "ssl": {
                "caSecret": {"name": "source-ca", "key": "../ca.crt"}
            }
        });
        let error = engine_config_value_from_pipeline_value(pipeline).unwrap_err();
        assert_eq!(error.field, "spec.source.security.ssl.caSecret.key");
    }

    #[test]
    fn rejects_different_destination_security_settings() {
        let mut pipeline = pipeline_with_udfs();
        let destination = pipeline["spec"]["destinations"][0].clone();
        pipeline["spec"]["destinations"]
            .as_array_mut()
            .unwrap()
            .push(destination);
        pipeline["spec"]["destinations"][0]["security"] = json!({
            "protocol": "SSL",
            "ssl": {}
        });

        let error = engine_config_value_from_pipeline_value(pipeline).unwrap_err();
        assert_eq!(error.field, "spec.destinations[1].security");
    }
}
