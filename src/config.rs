use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A DSL expression — accepts either a single string or an array of strings.
///
/// When used as a **filter**, multiple strings are implicitly ANDed.
/// When used as a **transform**, strings are applied as a sequential pipeline
/// where the output of each step becomes the input `msg` of the next.
///
/// ```yaml
/// # Single expression (both forms are equivalent)
/// filter: 'msg["status"] == "active"'
/// filter:
///   - 'msg["status"] == "active"'
///
/// # AND filter — all conditions must pass
/// filter:
///   - 'msg["status"] == "active"'
///   - 'msg["score"] > 80'
///   - 'key.starts_with("user-")'
///
/// # Transform pipeline — applied in order, each output feeds the next
/// transform:
///   - 'msg + #{ email: msg["email"].to_lower() }'
///   - 'msg + #{ processed: true }'
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DslExpr {
    /// A single Rhai expression or script string.
    Single(String),
    /// Multiple expressions — ANDed for filters, piped for transforms.
    Multi(Vec<String>),
}

// Custom deserializer — serde_yaml's untagged enum support does not reliably
// fall through to Vec<String> when it encounters a YAML sequence inside a
// nested struct. This visitor handles both explicitly.
impl<'de> serde::Deserialize<'de> for DslExpr {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{self, SeqAccess, Visitor};
        use std::fmt;

        struct DslExprVisitor;

        impl<'de> Visitor<'de> for DslExprVisitor {
            type Value = DslExpr;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a Rhai expression string or an array of expression strings")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(DslExpr::Single(v.to_string()))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(DslExpr::Single(v))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut parts = Vec::new();
                while let Some(s) = seq.next_element::<String>()? {
                    parts.push(s);
                }
                if parts.len() == 1 {
                    Ok(DslExpr::Single(parts.remove(0)))
                } else {
                    Ok(DslExpr::Multi(parts))
                }
            }
        }

        d.deserialize_any(DslExprVisitor)
    }
}

impl DslExpr {
    /// Return all expressions as a flat Vec regardless of variant.
    pub fn into_parts(self) -> Vec<String> {
        match self {
            DslExpr::Single(s) => vec![s],
            DslExpr::Multi(v) => v,
        }
    }
}

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

    /// Transform expression (or pipeline) for single-destination mode.
    /// Ignored when `routing` is set. Accepts a single Rhai script or an array
    /// of scripts that are applied in sequence.
    #[serde(default)]
    pub transform: Option<DslExpr>,

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

    /// Performance tuning — consumer poll sizes, producer batching, acks.
    /// All settings have production-safe defaults; override to increase throughput.
    #[serde(default)]
    pub performance: PerformanceConfig,
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

    /// Filter expression — a Rhai boolean expression, or a list of expressions
    /// that are all ANDed together.
    ///
    /// Available scope: `msg` (payload), `key`, `headers`, `timestamp`.
    ///
    /// ```yaml
    /// filter: 'msg["status"] == "active" && msg["score"] > 80'
    /// # or as an array (ANDed automatically):
    /// filter:
    ///   - 'msg["status"] == "active"'
    ///   - 'msg["score"] > 80'
    ///   - 'key.starts_with("user-")'
    /// ```
    #[serde(default)]
    pub filter: Option<DslExpr>,

    /// Transform expression — a Rhai script that returns the new payload, or
    /// a list of scripts applied as a pipeline (each output is the next `msg`).
    ///
    /// ```yaml
    /// transform: '#{ id: msg["userId"], email: msg["email"].to_lower() }'
    /// # or as a pipeline:
    /// transform:
    ///   - 'msg + #{ email: msg["email"].to_lower() }'
    ///   - 'msg + #{ processed: true }'
    /// ```
    #[serde(default)]
    pub transform: Option<DslExpr>,

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
            serde_yaml::from_str(&content).map_err(|e| {
                crate::error::MirrorMakerError::Config(format!("YAML parse error: {}", e))
            })?
        } else {
            // Default to JSON for backward compatibility
            serde_json::from_str(&content).map_err(|e| {
                crate::error::MirrorMakerError::Config(format!("JSON parse error: {}", e))
            })?
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

// ============================================================================
// PerformanceConfig
// ============================================================================

/// Fine-grained throughput tuning for the consumer poll loop and Kafka producer.
///
/// All settings are optional — the defaults are safe for most workloads.
/// To maximize throughput you typically want to increase the batch sizes
/// and relax acks on the producer side.
///
/// ```yaml
/// performance:
///   # Consumer (showing tuning examples — see constants below for actual defaults)
///   consumer_batch_size: 1000        # messages per poll batch (default: 500)
///   consumer_batch_timeout_ms: 50    # max wait to fill batch (default: 50ms)
///   fetch_min_bytes: 131072          # 128KB for max throughput (default: 64KB)
///   fetch_max_wait_ms: 100           # 100ms for lower latency (default: 500ms)
///   queued_max_messages_kbytes: 524288  # rdkafka pre-fetch buffer = 512MB (default: 512MB)
///   parallelism_factor: 10           # concurrent produce futures = threads * factor (default: 10)
///
///   # Producer
///   producer_acks: "1"               # "all" (safest), "1" (faster), "0" (fastest, lossy)
///   producer_batch_size_bytes: 524288  # 512KB — librdkafka batch size
///   producer_linger_ms: 5            # batch accumulation window
///   producer_queue_max_messages: 1000000  # max messages in produce queue
///   producer_queue_max_kbytes: 2097152    # 2GB produce queue size
///   message_max_bytes: 1048576       # max message size (1MB)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    // ── Consumer poll loop ─────────────────────────────────────────────────
    /// Max messages collected per poll batch before processing.
    /// Higher = better throughput under load; lower = lower latency per message.
    /// Typical range: 200–2000.
    #[serde(default = "default_consumer_batch_size")]
    pub consumer_batch_size: usize,

    /// Max milliseconds to wait filling a batch before processing a partial one.
    /// Lower values reduce latency at low throughput; higher values reduce overhead.
    #[serde(default = "default_consumer_batch_timeout_ms")]
    pub consumer_batch_timeout_ms: u64,

    /// Multiplier on `threads` to set the max number of concurrent produce futures.
    /// threads=8, factor=10 → 80 produce I/Os in flight simultaneously.
    #[serde(default = "default_parallelism_factor")]
    pub parallelism_factor: usize,

    // ── librdkafka consumer tuning ─────────────────────────────────────────
    /// Minimum bytes the broker accumulates before responding to a fetch request.
    /// Default 64KB batches broker-side to reduce round-trips (3-5x throughput gain).
    /// Set to 1 for lowest latency (immediate response), or up to 1MB for max throughput.
    /// **Trade-off**: Higher values improve throughput but add up to `fetch_max_wait_ms`
    /// latency when load is low and broker has insufficient data to meet the threshold.
    /// Maps to librdkafka `fetch.min.bytes`.
    #[serde(default = "default_fetch_min_bytes")]
    pub fetch_min_bytes: u32,

    /// Max milliseconds the broker waits to reach `fetch_min_bytes`.
    /// Default 500ms allows time for broker-side batching at moderate load.
    /// Reduce to 100-200ms for lower latency, or increase to 1000ms for maximum batching.
    /// **Note**: This only affects latency when traffic is sparse — at high load,
    /// the broker accumulates `fetch_min_bytes` quickly and responds immediately.
    /// Maps to librdkafka `fetch.wait.max.ms`.
    #[serde(default = "default_fetch_max_wait_ms")]
    pub fetch_max_wait_ms: u32,

    /// Max bytes fetched per partition per fetch request.
    /// Maps to librdkafka `max.partition.fetch.bytes`.
    #[serde(default = "default_max_partition_fetch_bytes")]
    pub max_partition_fetch_bytes: u32,

    /// rdkafka internal consumer pre-fetch buffer size (KB).
    /// Higher = more messages buffered ahead of processing = smoother throughput.
    /// Maps to librdkafka `queued.max.messages.kbytes`.
    #[serde(default = "default_queued_max_messages_kbytes")]
    pub queued_max_messages_kbytes: u32,

    // ── librdkafka producer tuning ─────────────────────────────────────────
    /// Producer acknowledgement requirement.
    /// `"all"` — wait for all ISR replicas (safest, slowest).
    /// `"1"`   — wait for leader only (good balance).
    /// `"0"`   — fire-and-forget (highest throughput, messages may be lost on broker crash).
    /// Maps to librdkafka `request.required.acks`.
    #[serde(default = "default_producer_acks")]
    pub producer_acks: String,

    /// librdkafka internal batch accumulation size in bytes.
    /// Messages are coalesced into batches up to this size before sending.
    /// Higher = fewer network round-trips; default 16KB is conservative.
    /// Maps to librdkafka `batch.size`.
    #[serde(default = "default_producer_batch_size_bytes")]
    pub producer_batch_size_bytes: u32,

    /// Time in ms to wait for a batch to fill before sending it.
    /// `0` = send immediately (lowest latency, higher overhead).
    /// `5`–`50` = good throughput/latency balance.
    /// Maps to librdkafka `linger.ms` / `queue.buffering.max.ms`.
    #[serde(default = "default_producer_linger_ms")]
    pub producer_linger_ms: u32,

    /// Max number of messages in the librdkafka produce queue.
    /// Maps to librdkafka `queue.buffering.max.messages`.
    #[serde(default = "default_producer_queue_max_messages")]
    pub producer_queue_max_messages: u32,

    /// Max total bytes in the librdkafka produce queue.
    /// Maps to librdkafka `queue.buffering.max.kbytes`.
    #[serde(default = "default_producer_queue_max_kbytes")]
    pub producer_queue_max_kbytes: u32,
}

// Default functions for PerformanceConfig
//
// Byte size constants for Kafka consumer fetch tuning:
// - FETCH_MIN_BYTES_DEFAULT (64KB): Balances throughput and latency.
//   Below 64KB causes excessive round-trips; 64KB-1MB provides good throughput
//   with acceptable low-traffic latency (<500ms); above 1MB shows diminishing returns.
// - FETCH_MAX_WAIT_MS_DEFAULT (500ms): Conservative timeout for broker-side batching.
//   Allows time to accumulate 64KB at moderate rates while preventing excessive latency
//   during sparse traffic. At high load, broker fills buffer quickly and this timeout
//   is rarely hit (typically responds in <100ms).
const FETCH_MIN_BYTES_DEFAULT: u32 = 65536; // 64KB
const FETCH_MAX_WAIT_MS_DEFAULT: u32 = 500; // milliseconds

fn default_consumer_batch_size() -> usize {
    500
}
fn default_consumer_batch_timeout_ms() -> u64 {
    50
}
fn default_parallelism_factor() -> usize {
    10
}
fn default_fetch_min_bytes() -> u32 {
    FETCH_MIN_BYTES_DEFAULT
}
fn default_fetch_max_wait_ms() -> u32 {
    FETCH_MAX_WAIT_MS_DEFAULT
}
fn default_max_partition_fetch_bytes() -> u32 {
    1_048_576
} // 1MB per partition
fn default_queued_max_messages_kbytes() -> u32 {
    524_288
} // 512MB pre-fetch buffer
fn default_producer_acks() -> String {
    "all".to_string()
}
fn default_producer_batch_size_bytes() -> u32 {
    524_288
} // 512KB batch (librdkafka default is 16KB)
fn default_producer_linger_ms() -> u32 {
    5
}
fn default_producer_queue_max_messages() -> u32 {
    1_000_000
}
fn default_producer_queue_max_kbytes() -> u32 {
    2_097_152
} // 2GB queue

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            consumer_batch_size: default_consumer_batch_size(),
            consumer_batch_timeout_ms: default_consumer_batch_timeout_ms(),
            parallelism_factor: default_parallelism_factor(),
            fetch_min_bytes: default_fetch_min_bytes(),
            fetch_max_wait_ms: default_fetch_max_wait_ms(),
            max_partition_fetch_bytes: default_max_partition_fetch_bytes(),
            queued_max_messages_kbytes: default_queued_max_messages_kbytes(),
            producer_acks: default_producer_acks(),
            producer_batch_size_bytes: default_producer_batch_size_bytes(),
            producer_linger_ms: default_producer_linger_ms(),
            producer_queue_max_messages: default_producer_queue_max_messages(),
            producer_queue_max_kbytes: default_producer_queue_max_kbytes(),
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
        assert_eq!(
            config.filter,
            Some(crate::config::DslExpr::Single(
                "KEY_PREFIX:user-".to_string()
            ))
        );
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
            Some(crate::config::DslExpr::Single(
                "AND:KEY_PREFIX:premium-:/user/active,==,true".to_string()
            ))
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
