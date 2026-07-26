use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// StreamforgePipeline CRD
#[derive(CustomResource, Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[kube(
    group = "streamforge.io",
    version = "v1alpha1",
    kind = "StreamforgePipeline",
    plural = "streamforgepipelines",
    shortname = "sfp",
    namespaced,
    status = "PipelineStatus",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Replicas", "type":"integer", "jsonPath":".status.replicas"}"#,
    printcolumn = r#"{"name":"Source", "type":"string", "jsonPath":".spec.source.topic"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct StreamforgePipelineSpec {
    /// Application ID
    pub appid: Option<String>,

    /// Source Kafka configuration
    pub source: SourceConfig,

    /// Destination configurations
    pub destinations: Vec<DestinationConfig>,

    /// WebAssembly UDF modules available to destination bindings.
    #[serde(default)]
    pub udfs: Option<UdfConfig>,

    /// Resource requirements
    #[serde(default)]
    pub resources: ResourceRequirements,

    /// Number of replicas
    #[serde(default = "default_replicas")]
    pub replicas: i32,

    /// Number of consumer threads
    #[serde(default = "default_threads")]
    pub threads: i32,

    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Image configuration
    #[serde(default)]
    pub image: ImageConfig,

    /// Service account
    pub service_account: Option<String>,

    /// Node selector
    #[serde(default)]
    pub node_selector: BTreeMap<String, String>,

    /// Tolerations
    #[serde(default)]
    pub tolerations: Vec<serde_json::Value>,

    /// Affinity
    pub affinity: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceConfig {
    pub brokers: String,
    pub topic: String,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default = "default_offset")]
    pub offset: String,
    pub security: Option<SecurityConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DestinationConfig {
    pub brokers: String,
    pub topic: String,
    pub filter: Option<String>,
    pub transform: Option<String>,
    /// Optional WebAssembly UDF bindings for this destination.
    #[serde(default)]
    pub udfs: Option<UdfBindings>,
    #[serde(default)]
    pub partitioner: Option<String>,
    pub partitioner_field: Option<String>,
    #[serde(default = "default_compression")]
    pub compression: String,
    pub security: Option<SecurityConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UdfConfig {
    /// Runtime limits. Omitted fields use StreamForge's production defaults.
    #[serde(default)]
    pub runtime: UdfRuntimeConfig,
    /// Module registry. Module names must be unique within the pipeline.
    pub modules: Vec<UdfModule>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UdfRuntimeConfig {
    pub max_module_bytes: Option<u64>,
    pub max_input_bytes: Option<u64>,
    pub max_output_bytes: Option<u64>,
    pub max_memory_bytes: Option<u64>,
    pub max_table_elements: Option<u64>,
    pub max_execution_ms: Option<u64>,
    pub epoch_tick_ms: Option<u64>,
    pub max_wasm_stack_bytes: Option<u64>,
    pub max_concurrent_instances: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UdfModule {
    /// Stable module name referenced by destination bindings.
    pub name: String,
    /// Lowercase SHA-256 digest, exactly 64 hexadecimal characters.
    pub sha256: String,
    /// Exported world implemented by the module.
    pub world: UdfWorld,
    /// StreamForge UDF ABI version.
    #[serde(default)]
    pub abi: UdfAbi,
    /// Kubernetes artifact containing the component binary.
    pub artifact: UdfArtifactRef,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UdfWorld {
    Filter,
    ValueTransform,
    EnvelopeTransform,
}

impl UdfWorld {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Filter => "filter",
            Self::ValueTransform => "value_transform",
            Self::EnvelopeTransform => "envelope_transform",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UdfAbi {
    #[default]
    V1,
}

impl UdfAbi {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V1 => "v1",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum UdfArtifactRef {
    /// A single ConfigMap key containing the component binary.
    ConfigMap { name: String, key: String },
    /// A component at a safe relative path inside a read-only PVC.
    PersistentVolumeClaim {
        #[serde(rename = "claimName")]
        claim_name: String,
        path: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UdfBindings {
    pub filter: Option<String>,
    pub value_transform: Option<String>,
    pub envelope_transform: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecurityConfig {
    #[serde(default = "default_protocol")]
    pub protocol: String,
    pub ssl: Option<SslConfig>,
    pub sasl: Option<SaslConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretReference {
    /// Name of the secret
    pub name: String,
    /// Key within the secret
    pub key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SslConfig {
    pub ca_location: Option<String>,
    pub certificate_location: Option<String>,
    pub key_location: Option<String>,
    pub key_password: Option<String>,
    /// Secret containing CA certificate
    pub ca_secret: Option<SecretReference>,
    /// Secret containing client certificate
    pub certificate_secret: Option<SecretReference>,
    /// Secret containing client key
    pub key_secret: Option<SecretReference>,
    /// Secret containing key password
    pub key_password_secret: Option<SecretReference>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SaslConfig {
    pub mechanism: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub kerberos_service_name: Option<String>,
    /// Secret containing SASL username
    pub username_secret: Option<SecretReference>,
    /// Secret containing SASL password
    pub password_secret: Option<SecretReference>,
    /// Secret containing Kerberos keytab
    pub keytab_secret: Option<SecretReference>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, JsonSchema)]
pub struct ResourceRequirements {
    pub requests: Option<BTreeMap<String, String>>,
    pub limits: Option<BTreeMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageConfig {
    #[serde(default = "default_image_repository")]
    pub repository: String,
    #[serde(default = "default_image_tag")]
    pub tag: String,
    #[serde(default = "default_image_pull_policy")]
    pub pull_policy: String,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            repository: default_image_repository(),
            tag: default_image_tag(),
            pull_policy: default_image_pull_policy(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct PipelineStatus {
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub replicas: i32,
    #[serde(default)]
    pub conditions: Vec<PipelineCondition>,
    pub last_updated: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineCondition {
    pub r#type: String,
    pub status: String,
    pub last_transition_time: Option<String>,
    pub reason: Option<String>,
    pub message: Option<String>,
}

// Default functions
fn default_replicas() -> i32 {
    1
}
fn default_threads() -> i32 {
    4
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_offset() -> String {
    "latest".to_string()
}
fn default_compression() -> String {
    "none".to_string()
}
fn default_protocol() -> String {
    "PLAINTEXT".to_string()
}
fn default_image_repository() -> String {
    std::env::var("DEFAULT_IMAGE_REPOSITORY")
        .unwrap_or_else(|_| "ghcr.io/rahulbsw/streamforge".to_string())
}
fn default_image_tag() -> String {
    std::env::var("DEFAULT_IMAGE_TAG").unwrap_or_else(|_| "0.3.0".to_string())
}
fn default_image_pull_policy() -> String {
    "IfNotPresent".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udf_spec_round_trips_with_typed_artifact_references() {
        let source = serde_json::json!({
            "apiVersion": "streamforge.io/v1alpha1",
            "kind": "StreamforgePipeline",
            "metadata": {"name": "orders"},
            "spec": {
                "source": {
                    "brokers": "kafka:9092",
                    "topic": "orders"
                },
                "destinations": [{
                    "brokers": "target:9092",
                    "topic": "filtered-orders",
                    "udfs": {
                        "filter": "allow-orders",
                        "valueTransform": "redact-orders"
                    }
                }],
                "udfs": {
                    "runtime": {
                        "maxMemoryBytes": 67108864,
                        "maxExecutionMs": 10
                    },
                    "modules": [
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
                        },
                        {
                            "name": "redact-orders",
                            "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                            "world": "value_transform",
                            "abi": "v1",
                            "artifact": {
                                "persistentVolumeClaim": {
                                    "claimName": "udf-artifacts",
                                    "path": "release/redact.wasm"
                                }
                            }
                        }
                    ]
                }
            }
        });

        let pipeline: StreamforgePipeline = serde_json::from_value(source).unwrap();
        let serialized = serde_json::to_value(&pipeline).unwrap();
        let reparsed: StreamforgePipeline = serde_json::from_value(serialized).unwrap();
        let udfs = reparsed.spec.udfs.unwrap();

        assert_eq!(udfs.modules.len(), 2);
        assert!(matches!(
            udfs.modules[0].artifact,
            UdfArtifactRef::ConfigMap { ref name, ref key }
                if name == "order-udfs" && key == "allow.wasm"
        ));
        assert!(matches!(
            udfs.modules[1].artifact,
            UdfArtifactRef::PersistentVolumeClaim {
                ref claim_name,
                ref path
            } if claim_name == "udf-artifacts" && path == "release/redact.wasm"
        ));
        assert_eq!(udfs.runtime.max_memory_bytes, Some(67_108_864));
        assert_eq!(
            reparsed.spec.destinations[0]
                .udfs
                .as_ref()
                .unwrap()
                .value_transform
                .as_deref(),
            Some("redact-orders")
        );
    }
}
