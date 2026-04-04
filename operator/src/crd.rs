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
    #[serde(default)]
    pub partitioner: Option<String>,
    pub partitioner_field: Option<String>,
    #[serde(default = "default_compression")]
    pub compression: String,
    pub security: Option<SecurityConfig>,
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
