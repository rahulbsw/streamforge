use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};

use crate::wasm::config::{DestinationUdfConfig, WasmConfig};

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

    /// Runtime batching and Kafka client performance tuning.
    #[serde(default)]
    pub performance: PerformanceConfig,

    /// Compression configuration
    #[serde(default)]
    pub compression: CompressionConfig,

    /// Multi-destination routing configuration
    pub routing: Option<RoutingConfig>,

    /// Value transform expression for single-destination mode.
    ///
    /// Supports the full transform DSL including `STRING:`, `CACHE_LOOKUP:`,
    /// `CACHE_PUT:`, `HASH:`, `CONSTRUCT:`, `ARITHMETIC:`, etc.
    /// Ignored when `routing` is set.
    #[serde(default)]
    pub transform: Option<String>,

    /// Consumer properties
    #[serde(default)]
    pub consumer_properties: HashMap<String, String>,

    /// Producer properties
    #[serde(default)]
    pub producer_properties: HashMap<String, String>,

    /// Security configuration
    #[serde(default)]
    pub security: Option<SecurityConfig>,

    /// Target Kafka security configuration. When omitted, `security` remains
    /// the backward-compatible source and target setting.
    #[serde(default)]
    pub target_security: Option<SecurityConfig>,

    /// Commit strategy configuration
    #[serde(default)]
    pub commit_strategy: CommitStrategyConfig,

    /// Cache configuration
    #[serde(default)]
    pub cache: Option<CacheBackendConfig>,

    /// Observability configuration (metrics, monitoring)
    #[serde(default)]
    pub observability: ObservabilityConfig,

    /// Retry configuration for transient failures
    #[serde(default)]
    pub retry: crate::retry::RetryConfig,

    /// Dead letter queue configuration for failed messages
    #[serde(default)]
    pub dlq: crate::dlq::DlqConfig,

    /// Optional registry of stateless WebAssembly UDF components.
    ///
    /// Omitting this field preserves the native-only execution path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm: Option<WasmConfig>,

    /// WebAssembly UDFs for single-destination mode.
    ///
    /// Ignored when `routing` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udfs: Option<DestinationUdfConfig>,
}

/// Runtime batching and Kafka client performance tuning.
///
/// The in-process batching fields preserve the historical hard-coded defaults.
/// Kafka fields are optional so omitting `performance` preserves librdkafka's
/// existing defaults. Explicit `consumer_properties` and `producer_properties`
/// take precedence over values generated from this section.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceConfig {
    /// Maximum messages collected before processing a batch.
    #[serde(default = "default_consumer_batch_size")]
    pub consumer_batch_size: usize,

    /// Maximum milliseconds to wait for a partially filled legacy batch. In
    /// partition-ordered mode this is also the idle queued-delivery flush delay.
    #[serde(default = "default_consumer_batch_timeout_ms")]
    pub consumer_batch_timeout_ms: u64,

    /// Concurrent processing multiplier applied to `threads`.
    #[serde(default = "default_parallelism_factor")]
    pub parallelism_factor: usize,

    /// In-process scheduling strategy.
    ///
    /// `legacy_batch` preserves the historical batch barrier. `partition_ordered`
    /// routes each source partition to one bounded FIFO worker lane.
    #[serde(default)]
    pub processing_mode: ProcessingMode,

    /// Per-worker input queue capacity in `partition_ordered` mode.
    #[serde(default = "default_worker_queue_capacity")]
    pub worker_queue_capacity: usize,

    /// Maps to librdkafka `fetch.min.bytes`.
    #[serde(default)]
    pub fetch_min_bytes: Option<u32>,

    /// Maps to librdkafka `fetch.wait.max.ms`.
    #[serde(default)]
    pub fetch_max_wait_ms: Option<u32>,

    /// Maps to librdkafka `linger.ms`.
    #[serde(default)]
    pub linger_ms: Option<u64>,

    /// Maximum messages per producer batch; maps to librdkafka
    /// `batch.num.messages`.
    #[serde(default)]
    pub batch_size: Option<usize>,

    /// Maps to librdkafka `queue.buffering.max.ms`.
    #[serde(default)]
    pub queue_buffering_max_ms: Option<u64>,

    /// Producer delivery completion behavior.
    ///
    /// `acknowledged` preserves the historical behavior by awaiting Kafka's
    /// delivery result for every record. `queued` returns after librdkafka
    /// accepts the record and tracks delivery completion in the background.
    #[serde(default)]
    pub producer_delivery_mode: ProducerDeliveryMode,

    /// Maximum queued producer deliveries awaiting acknowledgement.
    #[serde(default = "default_producer_max_in_flight")]
    pub producer_max_in_flight: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProducerDeliveryMode {
    #[default]
    Acknowledged,
    Queued,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingMode {
    #[default]
    LegacyBatch,
    PartitionOrdered,
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

    /// Path to a file containing the private-key password.
    #[serde(default)]
    pub key_password_file: Option<String>,

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

    /// Paths to files containing SASL credentials.
    #[serde(default)]
    pub username_file: Option<String>,
    #[serde(default)]
    pub password_file: Option<String>,

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregationConfig {
    pub group_by: Vec<AggregationGroupBy>,
    pub window: AggregationWindowConfig,
    pub metrics: Vec<AggregationMetricConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregationGroupBy {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregationWindowConfig {
    #[serde(rename = "type")]
    pub window_type: AggregationWindowType,
    pub size_seconds: u64,
    pub emit_interval_seconds: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AggregationWindowType {
    Tumbling,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregationMetricConfig {
    pub name: String,
    pub op: AggregationOp,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub percentiles: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AggregationOp {
    Count,
    Sum,
    Avg,
    ApproxDistinct,
    Quantiles,
}

/// Error handling policy for filter/transform failures
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorPolicy {
    /// Halt pipeline on any error (strictest)
    #[default]
    Fail,
    /// Send failed messages to dead letter queue (recommended)
    Dlq,
    /// Skip bad messages and log errors (permissive)
    SkipAndLog,
    /// Continue processing, log but don't DLQ (most permissive)
    Continue,
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

    /// Error handling policy for this destination
    ///
    /// Controls what happens when filter/transform evaluation fails:
    /// - "fail" - Halt pipeline on any error (default for backward compatibility)
    /// - "dlq" - Send failed messages to dead letter queue
    /// - "skip_and_log" - Skip bad messages and log errors
    /// - "continue" - Continue processing, log errors but don't DLQ
    ///
    /// Choose based on data criticality:
    /// - Financial/audit: use "fail" or "dlq"
    /// - Analytics: use "skip_and_log"
    /// - Enrichment: use "continue" with try() functions
    #[serde(default)]
    pub error_policy: ErrorPolicy,

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

    #[serde(default)]
    pub aggregation: Option<AggregationConfig>,

    /// Partition field JSON path
    pub partition: Option<String>,

    /// Broadcast flag for hybrid routing
    #[serde(default)]
    pub broadcast: bool,

    /// Description
    pub description: Option<String>,

    /// Optional stateless WebAssembly UDFs for this destination.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udfs: Option<DestinationUdfConfig>,
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

fn default_consumer_batch_size() -> usize {
    100
}

fn default_consumer_batch_timeout_ms() -> u64 {
    100
}

fn default_parallelism_factor() -> usize {
    10
}

fn default_worker_queue_capacity() -> usize {
    1_024
}

fn default_producer_max_in_flight() -> usize {
    10_000
}

fn config_error(message: impl Into<String>) -> crate::error::MirrorMakerError {
    crate::error::MirrorMakerError::Config(message.into())
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            consumer_batch_size: default_consumer_batch_size(),
            consumer_batch_timeout_ms: default_consumer_batch_timeout_ms(),
            parallelism_factor: default_parallelism_factor(),
            processing_mode: ProcessingMode::LegacyBatch,
            worker_queue_capacity: default_worker_queue_capacity(),
            fetch_min_bytes: None,
            fetch_max_wait_ms: None,
            linger_ms: None,
            batch_size: None,
            queue_buffering_max_ms: None,
            producer_delivery_mode: ProducerDeliveryMode::Acknowledged,
            producer_max_in_flight: default_producer_max_in_flight(),
        }
    }
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
        let config: Self = if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content).map_err(|e| {
                crate::error::MirrorMakerError::Config(format!("YAML parse error: {}", e))
            })?
        } else {
            // Default to JSON for backward compatibility
            serde_json::from_str(&content).map_err(|e| {
                crate::error::MirrorMakerError::Config(format!("JSON parse error: {}", e))
            })?
        };

        config.validate()?;

        Ok(config)
    }

    pub fn validate(&self) -> crate::Result<()> {
        if self.threads == 0 {
            return Err(config_error("threads must be > 0"));
        }
        if self.threads > i32::MAX as usize {
            return Err(config_error("threads must fit in a signed 32-bit integer"));
        }

        self.performance.validate()?;
        self.observability.validate()?;

        if let Some(wasm) = &self.wasm {
            wasm.validate().map_err(config_error)?;
        }

        if let Some(udfs) = &self.udfs {
            if self.routing.is_some() {
                return Err(config_error(
                    "top-level udfs is only valid in single-destination mode",
                ));
            }
            if udfs.is_empty() {
                return Err(config_error(
                    "top-level udfs must bind at least one WebAssembly stage",
                ));
            }
            let wasm = self
                .wasm
                .as_ref()
                .ok_or_else(|| config_error("top-level udfs requires a top-level wasm registry"))?;
            wasm.validate_refs("single destination", udfs)
                .map_err(config_error)?;
        }

        if self.performance.producer_delivery_mode == ProducerDeliveryMode::Queued {
            if self.commit_strategy.manual_commit {
                return Err(config_error(
                    "performance.producer_delivery_mode=queued requires \
                     commit_strategy.manual_commit=false because delivery errors are deferred",
                ));
            }

            if self.retry.max_attempts != 1 {
                return Err(config_error(
                    "performance.producer_delivery_mode=queued requires retry.max_attempts=1 \
                     because deferred delivery errors cannot retry the original envelope",
                ));
            }

            if self.dlq.enabled {
                return Err(config_error(
                    "performance.producer_delivery_mode=queued requires dlq.enabled=false \
                     because deferred delivery errors cannot retain the original envelope",
                ));
            }
        }

        if self.performance.processing_mode == ProcessingMode::PartitionOrdered
            && self.commit_strategy.manual_commit
        {
            return Err(config_error(
                "performance.processing_mode=partition_ordered currently requires \
                 commit_strategy.manual_commit=false; explicit rebalance-safe offset \
                 coordination is not yet implemented",
            ));
        }

        if let Some(routing) = &self.routing {
            for dest in &routing.destinations {
                if let Some(udfs) = &dest.udfs {
                    if udfs.is_empty() {
                        return Err(config_error(format!(
                            "destination {:?} udfs must bind at least one WebAssembly stage",
                            dest.output
                        )));
                    }
                    let wasm = self.wasm.as_ref().ok_or_else(|| {
                        config_error(format!(
                            "destination {:?} udfs requires a top-level wasm registry",
                            dest.output
                        ))
                    })?;
                    wasm.validate_refs(&format!("destination {:?}", dest.output), udfs)
                        .map_err(config_error)?;
                    if dest.aggregation.is_some() && udfs.envelope_transform.is_some() {
                        return Err(config_error(
                            "aggregation destinations cannot use a wasm envelope_transform",
                        ));
                    }
                }

                if let Some(aggregation) = &dest.aggregation {
                    if self.commit_strategy.manual_commit {
                        return Err(config_error(
                            "aggregation destinations do not support commit_strategy.manual_commit=true in v1",
                        ));
                    }

                    if dest.key_transform.is_some() {
                        return Err(config_error(
                            "aggregation destinations cannot use key_transform",
                        ));
                    }

                    if dest.headers.is_some()
                        || dest.header_transforms.is_some()
                        || dest.timestamp.is_some()
                    {
                        return Err(config_error(
                            "aggregation destinations cannot use header or timestamp transforms in v1",
                        ));
                    }

                    aggregation.validate()?;
                }
            }
        }

        Ok(())
    }

    /// Populate Kafka property maps from optional performance settings.
    ///
    /// Existing property-map entries always win. `linger.ms` and
    /// `queue.buffering.max.ms` are librdkafka aliases, so an explicit value for
    /// either suppresses generation of both.
    pub fn apply_performance_property_defaults(&mut self) {
        if let Some(value) = self.performance.fetch_min_bytes {
            self.consumer_properties
                .entry("fetch.min.bytes".to_string())
                .or_insert_with(|| value.to_string());
        }

        if let Some(value) = self.performance.fetch_max_wait_ms {
            self.consumer_properties
                .entry("fetch.wait.max.ms".to_string())
                .or_insert_with(|| value.to_string());
        }

        if let Some(value) = self.performance.batch_size {
            self.producer_properties
                .entry("batch.num.messages".to_string())
                .or_insert_with(|| value.to_string());
        }

        let buffering_time_is_explicit = self.producer_properties.contains_key("linger.ms")
            || self
                .producer_properties
                .contains_key("queue.buffering.max.ms");
        if !buffering_time_is_explicit {
            // `linger.ms` and `queue.buffering.max.ms` are aliases. Preserve
            // documented configs that set both by giving `linger_ms`
            // deterministic precedence.
            if let Some(value) = self.performance.linger_ms {
                self.producer_properties
                    .insert("linger.ms".to_string(), value.to_string());
            } else if let Some(value) = self.performance.queue_buffering_max_ms {
                self.producer_properties
                    .insert("queue.buffering.max.ms".to_string(), value.to_string());
            }
        }
    }

    pub fn get_target_broker(&self) -> String {
        self.target_broker
            .as_ref()
            .unwrap_or(&self.bootstrap)
            .clone()
    }

    /// Apply the backward-compatible source security configuration without
    /// resolving file-backed credentials.
    pub fn apply_security(&self, client_config: &mut rdkafka::ClientConfig) {
        if let Some(security) = &self.security {
            apply_security_config(security, client_config, false)
                .expect("inline security configuration does not perform I/O");
        }
    }

    /// Apply source security, resolving file-backed credentials when present.
    pub fn try_apply_source_security(
        &self,
        client_config: &mut rdkafka::ClientConfig,
    ) -> crate::Result<()> {
        if let Some(security) = &self.security {
            apply_security_config(security, client_config, true)?;
        }
        Ok(())
    }

    /// Apply target security. Omitting `target_security` preserves the v1
    /// behavior of sharing the source security configuration.
    pub fn try_apply_target_security(
        &self,
        client_config: &mut rdkafka::ClientConfig,
    ) -> crate::Result<()> {
        if let Some(security) = self.target_security.as_ref().or(self.security.as_ref()) {
            apply_security_config(security, client_config, true)?;
        }
        Ok(())
    }
}

fn apply_security_config(
    security: &SecurityConfig,
    client_config: &mut rdkafka::ClientConfig,
    resolve_files: bool,
) -> crate::Result<()> {
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
        } else if resolve_files {
            if let Some(path) = &ssl.key_password_file {
                client_config.set(
                    "ssl.key.password",
                    read_credential_file(path, "ssl.key_password_file")?,
                );
            }
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
        } else if resolve_files {
            if let Some(path) = &sasl.username_file {
                client_config.set(
                    "sasl.username",
                    read_credential_file(path, "sasl.username_file")?,
                );
            }
        }
        if let Some(password) = &sasl.password {
            client_config.set("sasl.password", password);
        } else if resolve_files {
            if let Some(path) = &sasl.password_file {
                client_config.set(
                    "sasl.password",
                    read_credential_file(path, "sasl.password_file")?,
                );
            }
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
    Ok(())
}

fn read_credential_file(path: &str, field: &str) -> crate::Result<String> {
    let value = std::fs::read_to_string(path).map_err(|error| {
        crate::error::MirrorMakerError::ConfigWithField {
            message: format!("cannot read credential file: {error}"),
            field: field.to_string(),
        }
    })?;
    let value = value.trim_end_matches(['\r', '\n']);
    if value.is_empty() {
        return Err(crate::error::MirrorMakerError::ConfigWithField {
            message: "credential file must not be empty".to_string(),
            field: field.to_string(),
        });
    }
    Ok(value.to_string())
}

impl PerformanceConfig {
    fn validate(&self) -> crate::Result<()> {
        if self.consumer_batch_size == 0 {
            return Err(config_error("performance.consumer_batch_size must be > 0"));
        }

        if self.consumer_batch_timeout_ms == 0 {
            return Err(config_error(
                "performance.consumer_batch_timeout_ms must be > 0",
            ));
        }

        if self.parallelism_factor == 0 {
            return Err(config_error("performance.parallelism_factor must be > 0"));
        }

        if self.worker_queue_capacity == 0 {
            return Err(config_error(
                "performance.worker_queue_capacity must be > 0",
            ));
        }

        if self.fetch_min_bytes == Some(0) {
            return Err(config_error("performance.fetch_min_bytes must be > 0"));
        }

        if self.batch_size == Some(0) {
            return Err(config_error("performance.batch_size must be > 0"));
        }

        if self.producer_max_in_flight == 0 {
            return Err(config_error(
                "performance.producer_max_in_flight must be > 0",
            ));
        }

        Ok(())
    }
}

impl AggregationConfig {
    fn validate(&self) -> crate::Result<()> {
        if self.group_by.is_empty() {
            return Err(config_error("group_by cannot be empty"));
        }

        if self.metrics.is_empty() {
            return Err(config_error("metrics cannot be empty"));
        }

        for group_by in &self.group_by {
            group_by.validate()?;
        }

        self.window.validate()?;

        for metric in &self.metrics {
            metric.validate()?;
        }

        Ok(())
    }
}

impl AggregationGroupBy {
    fn validate(&self) -> crate::Result<()> {
        if self.name.trim().is_empty() {
            return Err(config_error("group_by entries require non-empty name"));
        }

        if self.path.trim().is_empty() {
            return Err(config_error("group_by entries require non-empty path"));
        }

        Ok(())
    }
}

impl AggregationWindowConfig {
    fn validate(&self) -> crate::Result<()> {
        if self.size_seconds == 0 {
            return Err(config_error("window size_seconds must be > 0"));
        }

        if self.emit_interval_seconds == 0 || self.emit_interval_seconds > self.size_seconds {
            return Err(config_error(
                "window emit_interval_seconds must be > 0 and <= size_seconds",
            ));
        }

        Ok(())
    }
}

impl AggregationMetricConfig {
    fn validate(&self) -> crate::Result<()> {
        if self.op.requires_path() {
            match self.path.as_deref().map(str::trim) {
                Some(path) if !path.is_empty() => {}
                _ => {
                    return Err(config_error(format!(
                        "{} metrics require path",
                        self.op.as_str()
                    )));
                }
            }
        }

        if matches!(self.op, AggregationOp::Quantiles) {
            validate_quantile_percentiles(&self.name, self.percentiles.as_ref())?;
        }

        Ok(())
    }
}

fn validate_quantile_percentiles(name: &str, percentiles: Option<&Vec<f64>>) -> crate::Result<()> {
    let percentiles =
        percentiles.ok_or_else(|| config_error("quantiles metrics require percentiles"))?;
    if percentiles.is_empty() {
        return Err(config_error("quantiles metrics require percentiles"));
    }

    let mut keys = HashSet::new();
    for percentile in percentiles {
        if !percentile.is_finite() {
            return Err(config_error(format!(
                "quantiles metric '{}' has non-finite percentile",
                name
            )));
        }
        if !(0.0..=1.0).contains(percentile) {
            return Err(config_error(format!(
                "quantiles metric '{}' percentile must be in [0.0, 1.0], got {}",
                name, percentile
            )));
        }

        let key = quantile_percentile_key(*percentile);
        if !keys.insert(key.clone()) {
            return Err(config_error(format!(
                "quantiles metric '{}' contains duplicate percentile key '{}'",
                name, key
            )));
        }
    }

    Ok(())
}

fn quantile_percentile_key(percentile: f64) -> String {
    format!("p{}", percentile)
}

impl AggregationOp {
    fn requires_path(self) -> bool {
        matches!(
            self,
            Self::Sum | Self::Avg | Self::ApproxDistinct | Self::Quantiles
        )
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Sum => "sum",
            Self::Avg => "avg",
            Self::ApproxDistinct => "approx_distinct",
            Self::Quantiles => "quantiles",
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

    /// Address for the metrics HTTP server to bind.
    #[serde(default = "default_metrics_bind_address")]
    pub metrics_bind_address: IpAddr,

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
            metrics_bind_address: default_metrics_bind_address(),
            metrics_path: default_metrics_path(),
            lag_monitoring_enabled: default_lag_monitoring(),
            lag_monitoring_interval_secs: default_lag_interval(),
        }
    }
}

impl ObservabilityConfig {
    fn validate(&self) -> crate::Result<()> {
        if self.metrics_enabled && self.metrics_port == 0 {
            return Err(config_error(
                "observability.metrics_port must be greater than zero when metrics are enabled",
            ));
        }

        if self.lag_monitoring_enabled && self.lag_monitoring_interval_secs == 0 {
            return Err(config_error(
                "observability.lag_monitoring_interval_secs must be greater than zero when lag monitoring is enabled",
            ));
        }

        let path = self.metrics_path.as_str();
        if !path.starts_with('/')
            || path.len() == 1
            || path.chars().any(char::is_whitespace)
            || path.contains(['?', '#', '{', '}'])
        {
            return Err(config_error(
                "observability.metrics_path must be a static absolute HTTP path",
            ));
        }

        if matches!(path, "/health" | "/ready") {
            return Err(config_error(
                "observability.metrics_path cannot replace /health or /ready",
            ));
        }

        Ok(())
    }
}

fn default_metrics_enabled() -> bool {
    true
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_metrics_bind_address() -> IpAddr {
    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn minimal_config_yaml(extra: &str) -> String {
        format!(
            "appid: test\nbootstrap: localhost:9092\ninput: input-topic\n{}",
            extra
        )
    }

    fn temporary_credential_file(label: &str, contents: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "streamforge-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn file_backed_source_and_target_credentials_are_resolved_separately() {
        let source_user = temporary_credential_file("source-user", "source-user\n");
        let source_password = temporary_credential_file("source-password", "source-password\r\n");
        let target_user = temporary_credential_file("target-user", "target-user\n");
        let target_password = temporary_credential_file("target-password", "target-password\n");
        let yaml = minimal_config_yaml(&format!(
            r#"security:
  protocol: SASL_PLAINTEXT
  sasl:
    mechanism: SCRAM-SHA-512
    username:
    password:
    username_file: {}
    password_file: {}
    kerberos_service_name:
    kerberos_principal:
    kerberos_keytab:
    oauthbearer_token:
target_security:
  protocol: SASL_PLAINTEXT
  sasl:
    mechanism: SCRAM-SHA-512
    username:
    password:
    username_file: {}
    password_file: {}
    kerberos_service_name:
    kerberos_principal:
    kerberos_keytab:
    oauthbearer_token:
"#,
            source_user.display(),
            source_password.display(),
            target_user.display(),
            target_password.display()
        ));
        let config: MirrorMakerConfig = serde_yaml::from_str(&yaml).unwrap();
        let mut source = rdkafka::ClientConfig::new();
        let mut target = rdkafka::ClientConfig::new();

        config.try_apply_source_security(&mut source).unwrap();
        config.try_apply_target_security(&mut target).unwrap();

        assert_eq!(source.get("sasl.username"), Some("source-user"));
        assert_eq!(source.get("sasl.password"), Some("source-password"));
        assert_eq!(target.get("sasl.username"), Some("target-user"));
        assert_eq!(target.get("sasl.password"), Some("target-password"));

        for path in [source_user, source_password, target_user, target_password] {
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn credential_file_errors_are_field_specific_and_do_not_expose_values() {
        let empty = temporary_credential_file("empty-password", "\n");
        let yaml = minimal_config_yaml(&format!(
            r#"security:
  protocol: SASL_PLAINTEXT
  sasl:
    mechanism: PLAIN
    username:
    password:
    username_file: /path/that/does/not/exist
    password_file: {}
    kerberos_service_name:
    kerberos_principal:
    kerberos_keytab:
    oauthbearer_token:
"#,
            empty.display()
        ));
        let mut config: MirrorMakerConfig = serde_yaml::from_str(&yaml).unwrap();
        let error = config
            .try_apply_source_security(&mut rdkafka::ClientConfig::new())
            .unwrap_err()
            .to_string();
        assert!(error.contains("sasl.username_file"));
        assert!(!error.contains("password"));

        config
            .security
            .as_mut()
            .unwrap()
            .sasl
            .as_mut()
            .unwrap()
            .username_file = None;
        let error = config
            .try_apply_source_security(&mut rdkafka::ClientConfig::new())
            .unwrap_err()
            .to_string();
        assert!(error.contains("sasl.password_file"));
        std::fs::remove_file(empty).unwrap();
    }

    #[test]
    fn test_performance_config_defaults_preserve_existing_runtime_behavior() {
        let config: MirrorMakerConfig = serde_yaml::from_str(&minimal_config_yaml("")).unwrap();

        assert_eq!(config.performance.consumer_batch_size, 100);
        assert_eq!(config.performance.consumer_batch_timeout_ms, 100);
        assert_eq!(config.performance.parallelism_factor, 10);
        assert_eq!(
            config.performance.processing_mode,
            ProcessingMode::LegacyBatch
        );
        assert_eq!(config.performance.worker_queue_capacity, 1_024);
        assert_eq!(config.performance.fetch_min_bytes, None);
        assert_eq!(config.performance.fetch_max_wait_ms, None);
        assert_eq!(config.performance.linger_ms, None);
        assert_eq!(config.performance.batch_size, None);
        assert_eq!(config.performance.queue_buffering_max_ms, None);
        assert_eq!(
            config.performance.producer_delivery_mode,
            ProducerDeliveryMode::Acknowledged
        );
        assert_eq!(config.performance.producer_max_in_flight, 10_000);
    }

    #[test]
    fn test_observability_metrics_bind_address_defaults_and_deserializes() {
        let default_config: MirrorMakerConfig =
            serde_yaml::from_str(&minimal_config_yaml("")).unwrap();
        assert_eq!(
            default_config.observability.metrics_bind_address,
            IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        );

        let loopback_config: MirrorMakerConfig = serde_yaml::from_str(&minimal_config_yaml(
            "observability:\n  metrics_bind_address: 127.0.0.1\n",
        ))
        .unwrap();
        assert_eq!(
            loopback_config.observability.metrics_bind_address,
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        );
    }

    #[test]
    fn test_observability_metrics_path_is_validated() {
        let valid: MirrorMakerConfig = serde_yaml::from_str(&minimal_config_yaml(
            "observability:\n  metrics_path: /prom\n",
        ))
        .unwrap();
        valid.validate().unwrap();

        for path in ["metrics", "/", "/health", "/ready", "/metrics?format=text"] {
            let yaml = minimal_config_yaml(&format!("observability:\n  metrics_path: {path:?}\n"));
            let config: MirrorMakerConfig = serde_yaml::from_str(&yaml).unwrap();
            assert!(config.validate().is_err(), "{path} must be rejected");
        }
    }

    #[test]
    fn test_observability_runtime_values_are_validated() {
        for settings in [
            "metrics_enabled: true\n  metrics_port: 0",
            "lag_monitoring_enabled: true\n  lag_monitoring_interval_secs: 0",
        ] {
            let yaml = minimal_config_yaml(&format!("observability:\n  {settings}\n"));
            let config: MirrorMakerConfig = serde_yaml::from_str(&yaml).unwrap();
            assert!(config.validate().is_err(), "{settings} must be rejected");
        }
    }

    #[test]
    fn test_performance_config_deserializes_documented_names() {
        let yaml = minimal_config_yaml(
            r#"performance:
  consumer_batch_size: 2000
  consumer_batch_timeout_ms: 50
  parallelism_factor: 15
  processing_mode: partition_ordered
  worker_queue_capacity: 2048
  fetch_min_bytes: 131072
  fetch_max_wait_ms: 500
  linger_ms: 20
  batch_size: 1000
  queue_buffering_max_ms: 25
  producer_delivery_mode: queued
  producer_max_in_flight: 5000
retry:
  max_attempts: 1
dlq:
  enabled: false
"#,
        );
        let config: MirrorMakerConfig = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(config.performance.consumer_batch_size, 2000);
        assert_eq!(config.performance.consumer_batch_timeout_ms, 50);
        assert_eq!(config.performance.parallelism_factor, 15);
        assert_eq!(
            config.performance.processing_mode,
            ProcessingMode::PartitionOrdered
        );
        assert_eq!(config.performance.worker_queue_capacity, 2048);
        assert_eq!(config.performance.fetch_min_bytes, Some(131072));
        assert_eq!(config.performance.fetch_max_wait_ms, Some(500));
        assert_eq!(config.performance.linger_ms, Some(20));
        assert_eq!(config.performance.batch_size, Some(1000));
        assert_eq!(config.performance.queue_buffering_max_ms, Some(25));
        assert_eq!(
            config.performance.producer_delivery_mode,
            ProducerDeliveryMode::Queued
        );
        assert_eq!(config.performance.producer_max_in_flight, 5000);
    }

    #[test]
    fn test_performance_config_rejects_zero_runtime_limits() {
        for performance in [
            "consumer_batch_size: 0",
            "consumer_batch_timeout_ms: 0",
            "parallelism_factor: 0",
            "worker_queue_capacity: 0",
            "fetch_min_bytes: 0",
            "batch_size: 0",
            "producer_max_in_flight: 0",
        ] {
            let yaml = minimal_config_yaml(&format!("performance:\n  {}\n", performance));
            let config: MirrorMakerConfig = serde_yaml::from_str(&yaml).unwrap();
            assert!(
                config.validate().is_err(),
                "expected validation failure for {performance}"
            );
        }
    }

    #[test]
    fn test_queued_delivery_rejects_incompatible_reliability_settings() {
        let queued = r#"performance:
  producer_delivery_mode: queued
"#;

        let config: MirrorMakerConfig = serde_yaml::from_str(&minimal_config_yaml(queued)).unwrap();
        assert!(config.validate().is_err());

        let retry_safe: MirrorMakerConfig = serde_yaml::from_str(&minimal_config_yaml(&format!(
            "{queued}retry:\n  max_attempts: 1\n"
        )))
        .unwrap();
        assert!(retry_safe.validate().is_err());

        let valid: MirrorMakerConfig = serde_yaml::from_str(&minimal_config_yaml(&format!(
            "{queued}retry:\n  max_attempts: 1\ndlq:\n  enabled: false\n"
        )))
        .unwrap();
        assert!(valid.validate().is_ok());

        let manual: MirrorMakerConfig = serde_yaml::from_str(&minimal_config_yaml(&format!(
            "{queued}retry:\n  max_attempts: 1\ndlq:\n  enabled: false\ncommit_strategy:\n  manual_commit: true\n"
        )))
        .unwrap();
        assert!(manual.validate().is_err());
    }

    #[test]
    fn test_partition_ordered_rejects_manual_commit_until_offset_coordination_exists() {
        let config: MirrorMakerConfig = serde_yaml::from_str(&minimal_config_yaml(
            r#"performance:
  processing_mode: partition_ordered
commit_strategy:
  manual_commit: true
"#,
        ))
        .unwrap();

        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("partition_ordered"));
        assert!(error.contains("manual_commit=false"));
    }

    #[test]
    fn test_performance_properties_map_to_librdkafka_names() {
        let mut config: MirrorMakerConfig = serde_yaml::from_str(&minimal_config_yaml(
            r#"performance:
  fetch_min_bytes: 65536
  fetch_max_wait_ms: 500
  batch_size: 1000
  queue_buffering_max_ms: 25
"#,
        ))
        .unwrap();

        config.apply_performance_property_defaults();

        assert_eq!(
            config.consumer_properties.get("fetch.min.bytes"),
            Some(&"65536".to_string())
        );
        assert_eq!(
            config.consumer_properties.get("fetch.wait.max.ms"),
            Some(&"500".to_string())
        );
        assert_eq!(
            config.producer_properties.get("batch.num.messages"),
            Some(&"1000".to_string())
        );
        assert_eq!(
            config.producer_properties.get("queue.buffering.max.ms"),
            Some(&"25".to_string())
        );
    }

    #[test]
    fn test_explicit_kafka_properties_override_generated_settings() {
        let mut config: MirrorMakerConfig = serde_yaml::from_str(&minimal_config_yaml(
            r#"performance:
  fetch_min_bytes: 65536
  fetch_max_wait_ms: 500
  linger_ms: 20
  batch_size: 1000
consumer_properties:
  fetch.min.bytes: "1"
  fetch.wait.max.ms: "100"
producer_properties:
  batch.num.messages: "500"
  queue.buffering.max.ms: "5"
"#,
        ))
        .unwrap();

        config.apply_performance_property_defaults();

        assert_eq!(config.consumer_properties["fetch.min.bytes"], "1");
        assert_eq!(config.consumer_properties["fetch.wait.max.ms"], "100");
        assert_eq!(config.producer_properties["batch.num.messages"], "500");
        assert_eq!(config.producer_properties["queue.buffering.max.ms"], "5");
        assert!(!config.producer_properties.contains_key("linger.ms"));
    }

    #[test]
    fn test_linger_ms_wins_when_both_performance_aliases_are_configured() {
        let mut config: MirrorMakerConfig = serde_yaml::from_str(&minimal_config_yaml(
            r#"performance:
  linger_ms: 10
  queue_buffering_max_ms: 20
"#,
        ))
        .unwrap();

        config.validate().unwrap();
        config.apply_performance_property_defaults();

        assert_eq!(config.producer_properties["linger.ms"], "10");
        assert!(!config
            .producer_properties
            .contains_key("queue.buffering.max.ms"));
    }

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

    #[test]
    fn test_single_destination_wasm_bindings_validate_structurally() {
        let yaml = r#"
appid: test-app
bootstrap: localhost:9092
input: input-topic
output: output-topic
wasm:
  module_root: /opt/streamforge/udfs
  modules:
    - name: redact
      path: redact.wasm
      sha256: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      world: value_transform
      abi: v1
udfs:
  value_transform: redact
"#;

        let config: MirrorMakerConfig = serde_yaml::from_str(yaml).unwrap();
        config.validate().unwrap();
    }

    #[test]
    fn test_empty_and_wrong_world_wasm_bindings_are_rejected() {
        let empty: MirrorMakerConfig = serde_yaml::from_str(
            r#"
appid: test-app
bootstrap: localhost:9092
input: input-topic
output: output-topic
wasm:
  module_root: /opt/streamforge/udfs
  modules: []
udfs: {}
"#,
        )
        .unwrap();
        assert!(empty
            .validate()
            .unwrap_err()
            .to_string()
            .contains("between 1 and 32"));

        let wrong_world: MirrorMakerConfig = serde_yaml::from_str(
            r#"
appid: test-app
bootstrap: localhost:9092
input: input-topic
output: output-topic
wasm:
  module_root: /opt/streamforge/udfs
  modules:
    - name: filter-only
      path: filter.wasm
      sha256: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      world: filter
      abi: v1
udfs:
  value_transform: filter-only
"#,
        )
        .unwrap();
        assert!(wrong_world
            .validate()
            .unwrap_err()
            .to_string()
            .contains("requires world ValueTransform"));
    }
}
