use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorMakerConfig {
    /// Application ID
    pub appid: String,

    /// Source Kafka bootstrap servers
    pub bootstrap: String,

    /// Input topic(s) - comma-separated
    pub input: String,

    /// Output topic (for single destination)
    pub output: Option<String>,

    /// Target broker (for cross-cluster mirroring)
    #[serde(default)]
    pub target_broker: Option<String>,

    /// Consumer offset reset strategy
    #[serde(default = "default_offset")]
    pub offset: String,

    /// Number of processing threads
    #[serde(default = "default_threads")]
    pub threads: usize,

    /// Compression configuration
    #[serde(default)]
    pub compression: CompressionConfig,

    /// Multi-destination routing configuration
    pub routing: Option<RoutingConfig>,

    /// Consumer properties
    #[serde(default)]
    pub consumer_properties: HashMap<String, String>,

    /// Producer properties
    #[serde(default)]
    pub producer_properties: HashMap<String, String>,

    /// Security configuration
    #[serde(default)]
    pub security: Option<SecurityConfig>,

    /// Commit strategy configuration
    #[serde(default)]
    pub commit_strategy: CommitStrategyConfig,

    /// Cache configuration
    #[serde(default)]
    pub cache: Option<CacheBackendConfig>,

    /// Observability configuration (metrics, monitoring)
    #[serde(default)]
    pub observability: ObservabilityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Security protocol: PLAINTEXT, SSL, SASL_PLAINTEXT, SASL_SSL
    pub protocol: SecurityProtocol,

    /// SSL/TLS configuration
    #[serde(default)]
    pub ssl: Option<SslConfig>,

    /// SASL authentication configuration
    #[serde(default)]
    pub sasl: Option<SaslConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SecurityProtocol {
    Plaintext,
    Ssl,
    SaslPlaintext,
    SaslSsl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslConfig {
    /// Path to CA certificate file for verifying broker's certificate
    pub ca_location: Option<String>,

    /// Path to client's certificate file (for mutual TLS)
    pub certificate_location: Option<String>,

    /// Path to client's private key file (for mutual TLS)
    pub key_location: Option<String>,

    /// Password for the private key file
    pub key_password: Option<String>,

    /// Endpoint identification algorithm (default: https)
    pub endpoint_identification_algorithm: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaslConfig {
    /// SASL mechanism: PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, GSSAPI, OAUTHBEARER
    pub mechanism: SaslMechanism,

    /// Username (for PLAIN and SCRAM mechanisms)
    pub username: Option<String>,

    /// Password (for PLAIN and SCRAM mechanisms)
    pub password: Option<String>,

    /// Kerberos service name (for GSSAPI)
    pub kerberos_service_name: Option<String>,

    /// Kerberos principal (for GSSAPI)
    pub kerberos_principal: Option<String>,

    /// Path to Kerberos keytab (for GSSAPI)
    pub kerberos_keytab: Option<String>,

    /// OAuth bearer token (for OAUTHBEARER)
    pub oauthbearer_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SaslMechanism {
    #[serde(rename = "PLAIN")]
    Plain,
    #[serde(rename = "SCRAM-SHA-256")]
    ScramSha256,
    #[serde(rename = "SCRAM-SHA-512")]
    ScramSha512,
    #[serde(rename = "GSSAPI")]
    Gssapi,
    #[serde(rename = "OAUTHBEARER")]
    Oauthbearer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    #[serde(default)]
    pub compression_type: CompressionType,

    #[serde(default)]
    pub compression_algo: CompressionAlgo,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CompressionType {
    #[default]
    None,
    /// Native Kafka compression (recommended)
    Raw,
    /// Enveloped compression (custom format)
    Enveloped,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompressionAlgo {
    #[default]
    Gzip,
    Snappy,
    Zstd,
    Lz4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    /// Routing type: content, filter, or hybrid
    pub routing_type: String,

    /// JSON path for content-based routing
    pub path: Option<String>,

    /// Destination configurations
    pub destinations: Vec<DestinationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestinationConfig {
    /// Destination topic name
    pub output: String,

    /// Match value for content-based routing
    pub match_value: Option<String>,

    /// Filter expression (simple or composite)
    /// Simple: "path,operator,value" e.g., "/message/siteId,>,10000"
    /// Composite JSON for AND/OR/NOT (parsed separately)
    ///
    /// New envelope filters:
    /// - KEY_PREFIX:prefix
    /// - KEY_MATCHES:regex
    /// - HEADER:name,op,value
    /// - TIMESTAMP_AGE:op,seconds
    pub filter: Option<String>,

    /// Transform expression for message value
    /// Simple path: "/message" or "/message/confId"
    /// Object construction JSON (parsed separately)
    pub transform: Option<String>,

    /// Key transformation expression (NEW)
    ///
    /// Sets the message key for this destination. Supported formats:
    /// - Simple path: "/user/id" - Extract field from value as key
    /// - Template: "user-{/user/id}" - Build key from template
    /// - Constant: "CONSTANT:my-key" - Set constant key
    /// - Hash: "HASH:SHA256,/user/email" - Hash a field
    /// - Construct: "CONSTRUCT:tenant=/tenant:user=/user/id" - Build JSON key
    ///
    /// If not specified, the original message key is preserved.
    #[serde(default)]
    pub key_transform: Option<String>,

    /// Headers to set on messages sent to this destination (NEW)
    ///
    /// Static headers with constant values. For dynamic headers from message
    /// values, use header_transforms instead.
    ///
    /// Example:
    /// ```yaml
    /// headers:
    ///   x-processed-by: "streamforge"
    ///   x-version: "1.0"
    /// ```
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,

    /// Dynamic header transformations (NEW)
    ///
    /// Extract headers from message values or copy from existing headers.
    ///
    /// Supported operations:
    /// - FROM:/path - Extract from value field
    /// - COPY:source-header - Copy from existing header
    /// - REMOVE - Remove a header
    ///
    /// Example:
    /// ```yaml
    /// header_transforms:
    ///   - header: x-user-id
    ///     operation: FROM:/user/id
    ///   - header: x-correlation-id
    ///     operation: COPY:x-request-id
    /// ```
    #[serde(default)]
    pub header_transforms: Option<Vec<HeaderTransformConfig>>,

    /// Timestamp handling for messages sent to this destination (NEW)
    ///
    /// Controls how message timestamps are set:
    /// - "PRESERVE" - Keep original timestamp (default)
    /// - "CURRENT" - Set to current time
    /// - "/path/to/field" - Extract from value field
    /// - "ADD:seconds" - Add seconds to original timestamp
    /// - "SUBTRACT:seconds" - Subtract seconds from original timestamp
    ///
    /// If not specified, the original timestamp is preserved.
    #[serde(default)]
    pub timestamp: Option<String>,

    /// Partition field JSON path
    pub partition: Option<String>,

    /// Broadcast flag for hybrid routing
    #[serde(default)]
    pub broadcast: bool,

    /// Description
    pub description: Option<String>,
}

/// Header transformation configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeaderTransformConfig {
    /// Header name to set/modify
    pub header: String,

    /// Transformation operation
    ///
    /// Formats:
    /// - "FROM:/path" - Extract from message value
    /// - "COPY:source-header" - Copy from another header
    /// - "REMOVE" - Remove the header
    /// - "constant-value" - Set to constant value
    pub operation: String,
}

fn default_offset() -> String {
    "latest".to_string()
}

fn default_threads() -> usize {
    4
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            compression_type: CompressionType::None,
            compression_algo: CompressionAlgo::Gzip,
        }
    }
}

/// Commit strategy configuration for at-least-once/at-most-once semantics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitStrategyConfig {
    /// Enable manual commits (at-least-once) vs auto-commit (at-most-once)
    /// Default: false (auto-commit for backward compatibility)
    #[serde(default)]
    pub manual_commit: bool,

    /// Commit mode: Async or Sync
    /// Async is faster but may lose commits on crash
    /// Sync is slower but guarantees commits
    #[serde(default)]
    pub commit_mode: CommitMode,

    /// Commit interval in milliseconds (for batching)
    /// Only applies when manual_commit is true
    /// Default: 5000 (5 seconds)
    #[serde(default = "default_commit_interval_ms")]
    pub commit_interval_ms: u64,

    /// Enable dead letter queue for failed messages
    #[serde(default)]
    pub enable_dlq: bool,

    /// Dead letter queue topic name
    pub dlq_topic: Option<String>,

    /// Maximum retries before sending to DLQ
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Retry backoff strategy
    #[serde(default)]
    pub retry_backoff: RetryBackoffConfig,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CommitMode {
    /// Async commit (faster, may lose on crash)
    #[default]
    Async,
    /// Sync commit (slower, guaranteed)
    Sync,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryBackoffConfig {
    /// Initial backoff in milliseconds
    #[serde(default = "default_initial_backoff_ms")]
    pub initial_backoff_ms: u64,

    /// Maximum backoff in milliseconds
    #[serde(default = "default_max_backoff_ms")]
    pub max_backoff_ms: u64,

    /// Backoff multiplier
    #[serde(default = "default_backoff_multiplier")]
    pub multiplier: f64,
}

/// Cache backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheBackendConfig {
    /// Cache backend type: local, redis, kafka
    pub backend_type: CacheBackendType,

    /// Local cache configuration
    pub local: Option<LocalCacheConfig>,

    /// Redis cache configuration
    pub redis: Option<RedisCacheConfig>,

    /// Kafka-backed cache configuration
    pub kafka: Option<KafkaCacheConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheBackendType {
    /// In-memory cache (Moka)
    Local,
    /// Redis cache
    Redis,
    /// Kafka compacted topic as cache
    Kafka,
    /// Multi-level: local + Redis
    Multi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalCacheConfig {
    /// Maximum number of cache entries
    #[serde(default = "default_cache_capacity")]
    pub max_capacity: u64,

    /// Time-to-live in seconds
    pub ttl_seconds: Option<u64>,

    /// Time-to-idle in seconds
    pub tti_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisCacheConfig {
    /// Redis connection URL
    /// Format: redis://[:password@]host[:port][/database]
    /// Example: redis://localhost:6379/0
    pub url: String,

    /// Connection pool size
    #[serde(default = "default_redis_pool_size")]
    pub pool_size: usize,

    /// Key prefix for all cache keys
    pub key_prefix: Option<String>,

    /// Default TTL in seconds for cache entries
    pub default_ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaCacheConfig {
    /// Kafka bootstrap servers for cache topic
    pub bootstrap: String,

    /// Compacted topic name to use as cache
    pub topic: String,

    /// Consumer group for cache consumer
    pub group_id: String,

    /// Key field in message (JSON path)
    pub key_field: String,

    /// Value field in message (JSON path, or "." for entire message)
    #[serde(default = "default_value_field")]
    pub value_field: String,

    /// Warm up cache on startup (consume entire topic)
    #[serde(default = "default_true")]
    pub warmup_on_start: bool,
}

fn default_commit_interval_ms() -> u64 {
    5000 // 5 seconds
}

fn default_max_retries() -> u32 {
    3
}

fn default_initial_backoff_ms() -> u64 {
    100
}

fn default_max_backoff_ms() -> u64 {
    30000 // 30 seconds
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

fn default_cache_capacity() -> u64 {
    10_000
}

fn default_redis_pool_size() -> usize {
    10
}

fn default_value_field() -> String {
    ".".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for CommitStrategyConfig {
    fn default() -> Self {
        Self {
            manual_commit: false, // Auto-commit by default for backward compatibility
            commit_mode: CommitMode::Async,
            commit_interval_ms: default_commit_interval_ms(),
            enable_dlq: false,
            dlq_topic: None,
            max_retries: default_max_retries(),
            retry_backoff: RetryBackoffConfig::default(),
        }
    }
}

impl Default for RetryBackoffConfig {
    fn default() -> Self {
        Self {
            initial_backoff_ms: default_initial_backoff_ms(),
            max_backoff_ms: default_max_backoff_ms(),
            multiplier: default_backoff_multiplier(),
        }
    }
}

impl MirrorMakerConfig {
    /// Load configuration from file.
    ///
    /// Automatically detects format based on file extension:
    /// - .json → JSON format
    /// - .yaml, .yml → YAML format
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use streamforge::config::MirrorMakerConfig;
    /// let config = MirrorMakerConfig::from_file("config.json").unwrap();
    /// let config = MirrorMakerConfig::from_file("config.yaml").unwrap();
    /// ```
    pub fn from_file(path: &str) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)?;

        // Detect format based on file extension
        let config = if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content)
                .map_err(|e| crate::error::MirrorMakerError::Config(format!("YAML parse error: {}", e)))?
        } else {
            // Default to JSON for backward compatibility
            serde_json::from_str(&content)
                .map_err(|e| crate::error::MirrorMakerError::Config(format!("JSON parse error: {}", e)))?
        };

        Ok(config)
    }

    pub fn get_target_broker(&self) -> String {
        self.target_broker
            .as_ref()
            .unwrap_or(&self.bootstrap)
            .clone()
    }

    /// Apply security configuration to a Kafka ClientConfig
    pub fn apply_security(&self, client_config: &mut rdkafka::ClientConfig) {
        if let Some(security) = &self.security {
            // Set security protocol
            let protocol = match security.protocol {
                SecurityProtocol::Plaintext => "plaintext",
                SecurityProtocol::Ssl => "ssl",
                SecurityProtocol::SaslPlaintext => "sasl_plaintext",
                SecurityProtocol::SaslSsl => "sasl_ssl",
            };
            client_config.set("security.protocol", protocol);

            // Apply SSL configuration
            if let Some(ssl) = &security.ssl {
                if let Some(ca_location) = &ssl.ca_location {
                    client_config.set("ssl.ca.location", ca_location);
                }
                if let Some(cert_location) = &ssl.certificate_location {
                    client_config.set("ssl.certificate.location", cert_location);
                }
                if let Some(key_location) = &ssl.key_location {
                    client_config.set("ssl.key.location", key_location);
                }
                if let Some(key_password) = &ssl.key_password {
                    client_config.set("ssl.key.password", key_password);
                }
                if let Some(endpoint_id) = &ssl.endpoint_identification_algorithm {
                    client_config.set("ssl.endpoint.identification.algorithm", endpoint_id);
                }
            }

            // Apply SASL configuration
            if let Some(sasl) = &security.sasl {
                let mechanism = match sasl.mechanism {
                    SaslMechanism::Plain => "PLAIN",
                    SaslMechanism::ScramSha256 => "SCRAM-SHA-256",
                    SaslMechanism::ScramSha512 => "SCRAM-SHA-512",
                    SaslMechanism::Gssapi => "GSSAPI",
                    SaslMechanism::Oauthbearer => "OAUTHBEARER",
                };
                client_config.set("sasl.mechanism", mechanism);

                if let Some(username) = &sasl.username {
                    client_config.set("sasl.username", username);
                }
                if let Some(password) = &sasl.password {
                    client_config.set("sasl.password", password);
                }
                if let Some(service_name) = &sasl.kerberos_service_name {
                    client_config.set("sasl.kerberos.service.name", service_name);
                }
                if let Some(principal) = &sasl.kerberos_principal {
                    client_config.set("sasl.kerberos.principal", principal);
                }
                if let Some(keytab) = &sasl.kerberos_keytab {
                    client_config.set("sasl.kerberos.keytab", keytab);
                }
                if let Some(token) = &sasl.oauthbearer_token {
                    client_config.set("sasl.oauthbearer.token", token);
                }
            }
        }
    }
}

/// Observability configuration for metrics and monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Enable Prometheus metrics endpoint
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,

    /// Port for metrics HTTP server
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,

    /// Path for metrics endpoint
    #[serde(default = "default_metrics_path")]
    pub metrics_path: String,

    /// Enable Kafka consumer lag monitoring
    #[serde(default = "default_lag_monitoring")]
    pub lag_monitoring_enabled: bool,

    /// Lag monitoring interval in seconds
    #[serde(default = "default_lag_interval")]
    pub lag_monitoring_interval_secs: u64,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: default_metrics_enabled(),
            metrics_port: default_metrics_port(),
            metrics_path: default_metrics_path(),
            lag_monitoring_enabled: default_lag_monitoring(),
            lag_monitoring_interval_secs: default_lag_interval(),
        }
    }
}

fn default_metrics_enabled() -> bool {
    true
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_metrics_path() -> String {
    "/metrics".to_string()
}

fn default_lag_monitoring() -> bool {
    true
}

fn default_lag_interval() -> u64 {
    30
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_destination_config_with_envelope_fields() {
        let yaml = r#"
output: test-topic
filter: "KEY_PREFIX:user-"
key_transform: "/user/id"
headers:
  x-processed-by: "streamforge"
  x-version: "1.0"
header_transforms:
  - header: x-user-id
    operation: "FROM:/user/id"
  - header: x-correlation-id
    operation: "COPY:x-request-id"
timestamp: "PRESERVE"
"#;

        let config: DestinationConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.output, "test-topic");
        assert_eq!(config.filter, Some("KEY_PREFIX:user-".to_string()));
        assert_eq!(config.key_transform, Some("/user/id".to_string()));

        // Check headers
        let headers = config.headers.unwrap();
        assert_eq!(headers.get("x-processed-by").unwrap(), "streamforge");
        assert_eq!(headers.get("x-version").unwrap(), "1.0");

        // Check header transforms
        let transforms = config.header_transforms.unwrap();
        assert_eq!(transforms.len(), 2);
        assert_eq!(transforms[0].header, "x-user-id");
        assert_eq!(transforms[0].operation, "FROM:/user/id");
        assert_eq!(transforms[1].header, "x-correlation-id");
        assert_eq!(transforms[1].operation, "COPY:x-request-id");

        // Check timestamp
        assert_eq!(config.timestamp, Some("PRESERVE".to_string()));
    }

    #[test]
    fn test_destination_config_minimal() {
        let yaml = r#"
output: minimal-topic
"#;

        let config: DestinationConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.output, "minimal-topic");
        assert_eq!(config.filter, None);
        assert_eq!(config.key_transform, None);
        assert_eq!(config.headers, None);
        assert_eq!(config.header_transforms, None);
        assert_eq!(config.timestamp, None);
    }

    #[test]
    fn test_destination_config_key_templates() {
        let configs = vec![
            (
                r#"
output: test
key_transform: "/user/id"
"#,
                "/user/id",
            ),
            (
                r#"
output: test
key_transform: "user-{/user/id}"
"#,
                "user-{/user/id}",
            ),
            (
                r#"
output: test
key_transform: "CONSTRUCT:tenant=/tenant:user=/user/id"
"#,
                "CONSTRUCT:tenant=/tenant:user=/user/id",
            ),
            (
                r#"
output: test
key_transform: "HASH:SHA256,/user/email"
"#,
                "HASH:SHA256,/user/email",
            ),
        ];

        for (yaml, expected) in configs {
            let config: DestinationConfig = serde_yaml::from_str(yaml).unwrap();
            assert_eq!(config.key_transform, Some(expected.to_string()));
        }
    }

    #[test]
    fn test_destination_config_timestamp_modes() {
        let configs = vec![
            ("PRESERVE", "PRESERVE"),
            ("CURRENT", "CURRENT"),
            ("/event/timestamp", "/event/timestamp"),
            ("ADD:3600", "ADD:3600"),
            ("SUBTRACT:300", "SUBTRACT:300"),
        ];

        for (input, expected) in configs {
            let yaml = format!(
                r#"
output: test
timestamp: "{}"
"#,
                input
            );

            let config: DestinationConfig = serde_yaml::from_str(&yaml).unwrap();
            assert_eq!(config.timestamp, Some(expected.to_string()));
        }
    }

    #[test]
    fn test_full_config_with_envelope_features() {
        let yaml = r#"
appid: test-app
bootstrap: localhost:9092
input: test-input
routing:
  routing_type: filter
  destinations:
    - output: premium-users
      filter: "AND:KEY_PREFIX:premium-:/user/active,==,true"
      key_transform: "/user/id"
      headers:
        x-tier: "premium"
      header_transforms:
        - header: x-user-id
          operation: "FROM:/user/id"
      timestamp: "PRESERVE"
    - output: all-users
      key_transform: "HASH:SHA256,/user/email"
      timestamp: "CURRENT"
"#;

        let config: MirrorMakerConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.appid, "test-app");

        let routing = config.routing.unwrap();
        assert_eq!(routing.destinations.len(), 2);

        // First destination
        let dest1 = &routing.destinations[0];
        assert_eq!(dest1.output, "premium-users");
        assert_eq!(
            dest1.filter,
            Some("AND:KEY_PREFIX:premium-:/user/active,==,true".to_string())
        );
        assert_eq!(dest1.key_transform, Some("/user/id".to_string()));
        assert_eq!(dest1.timestamp, Some("PRESERVE".to_string()));

        let headers1 = dest1.headers.as_ref().unwrap();
        assert_eq!(headers1.get("x-tier").unwrap(), "premium");

        let transforms1 = dest1.header_transforms.as_ref().unwrap();
        assert_eq!(transforms1.len(), 1);

        // Second destination
        let dest2 = &routing.destinations[1];
        assert_eq!(dest2.output, "all-users");
        assert_eq!(
            dest2.key_transform,
            Some("HASH:SHA256,/user/email".to_string())
        );
        assert_eq!(dest2.timestamp, Some("CURRENT".to_string()));
    }

    #[test]
    fn test_header_transform_config() {
        let yaml = r#"
header: x-user-id
operation: "FROM:/user/id"
"#;

        let config: HeaderTransformConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.header, "x-user-id");
        assert_eq!(config.operation, "FROM:/user/id");
    }
}
