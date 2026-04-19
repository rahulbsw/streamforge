use thiserror::Error;

/// StreamForge error types with context and recovery actions
#[derive(Error, Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum MirrorMakerError {
    // ========== Kafka Errors ==========
    #[error("Kafka error: {0}")]
    Kafka(String), // Store as String for Clone

    #[error("Kafka producer error: {message}")]
    KafkaProducer {
        message: String,
        destination: Option<String>,
        recoverable: bool,
    },

    #[error("Kafka consumer error: {message}")]
    KafkaConsumer {
        message: String,
        topic: Option<String>,
        partition: Option<i32>,
        recoverable: bool,
    },

    #[error("Offset commit failed: {message}")]
    OffsetCommit {
        message: String,
        topic: String,
        partition: i32,
        offset: i64,
        retry_count: u32,
    },

    // ========== Serialization Errors ==========
    #[error("JSON serialization error: {0}")]
    Serialization(String), // Store as String for Clone

    #[error("Message deserialization failed: {message}")]
    MessageDeserialization {
        message: String,
        topic: String,
        partition: i32,
        offset: i64,
        key: Option<Vec<u8>>,
    },

    // ========== Configuration Errors ==========
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Configuration error: {message} (field: {field})")]
    ConfigWithField { message: String, field: String },

    #[error("Missing required configuration: {field}")]
    ConfigMissing { field: String },

    #[error("Invalid configuration value: {field} = {value}")]
    ConfigInvalid {
        field: String,
        value: String,
        reason: String,
    },

    // ========== DSL Parsing Errors ==========
    #[error("DSL parse error: {message}")]
    DslParse {
        message: String,
        dsl_string: String,
        position: Option<usize>,
    },

    #[error("Invalid filter expression: {expression}")]
    InvalidFilter { expression: String, reason: String },

    #[error("Invalid transform expression: {expression}")]
    InvalidTransform { expression: String, reason: String },

    // ========== Processing Errors ==========
    #[error("Processing error: {0}")]
    Processing(String),

    #[error("Processing error in {topic}[{partition}]@{offset}: {message}")]
    ProcessingWithContext {
        message: String,
        topic: String,
        partition: i32,
        offset: i64,
    },

    #[error("Filter evaluation failed: {message}")]
    FilterEvaluation {
        message: String,
        filter: String,
        value: Option<String>,
    },

    #[error("Transform evaluation failed: {message}")]
    TransformEvaluation {
        message: String,
        transform: String,
        value: Option<String>,
    },

    #[error("JSON path not found: {path}")]
    JsonPathNotFound { path: String, value: Option<String> },

    // ========== Compression Errors ==========
    #[error("Compression error: {0}")]
    Compression(String),

    #[error("{codec} compression error: {message}")]
    CompressionWithCodec { message: String, codec: String },

    #[error("Decompression error: {message}")]
    Decompression { message: String, codec: String },

    // ========== Cache Errors ==========
    #[error("Cache error: {message}")]
    Cache {
        message: String,
        backend: String,
        key: Option<String>,
    },

    #[error("Redis error: {message}")]
    Redis { message: String, operation: String },

    // ========== I/O Errors ==========
    #[error("IO error: {0}")]
    Io(String), // Store as String for Clone

    // ========== Retry and DLQ Errors ==========
    #[error("Retry exhausted: {message}")]
    RetryExhausted {
        message: String,
        attempts: u32,
        last_error: String,
    },

    #[error("Dead letter queue error: {message}")]
    DeadLetterQueue { message: String, dlq_topic: String },

    // ========== Generic Error ==========
    #[error("{0}")]
    Generic(String),
}

// From implementations for error types converted to String for Clone
impl From<rdkafka::error::KafkaError> for MirrorMakerError {
    fn from(e: rdkafka::error::KafkaError) -> Self {
        MirrorMakerError::Kafka(e.to_string())
    }
}

impl From<serde_json::Error> for MirrorMakerError {
    fn from(e: serde_json::Error) -> Self {
        MirrorMakerError::Serialization(e.to_string())
    }
}

impl From<std::io::Error> for MirrorMakerError {
    fn from(e: std::io::Error) -> Self {
        MirrorMakerError::Io(e.to_string())
    }
}

impl MirrorMakerError {
    /// Returns true if the error is likely transient and can be retried
    pub fn is_recoverable(&self) -> bool {
        match self {
            // Transient Kafka errors
            MirrorMakerError::KafkaProducer { recoverable, .. } => *recoverable,
            MirrorMakerError::KafkaConsumer { recoverable, .. } => *recoverable,
            MirrorMakerError::OffsetCommit { .. } => true, // Always retry commits

            // Network/IO errors are often transient
            MirrorMakerError::Io(_) => true,
            MirrorMakerError::Redis { .. } => true,
            MirrorMakerError::Cache { .. } => true,

            // Permanent errors
            MirrorMakerError::Config(_) => false,
            MirrorMakerError::ConfigWithField { .. } => false,
            MirrorMakerError::ConfigMissing { .. } => false,
            MirrorMakerError::ConfigInvalid { .. } => false,
            MirrorMakerError::DslParse { .. } => false,
            MirrorMakerError::InvalidFilter { .. } => false,
            MirrorMakerError::InvalidTransform { .. } => false,
            MirrorMakerError::RetryExhausted { .. } => false,

            // Message-level errors (skip message, don't halt)
            MirrorMakerError::Processing(_) => false,
            MirrorMakerError::ProcessingWithContext { .. } => false,
            MirrorMakerError::MessageDeserialization { .. } => false,
            MirrorMakerError::FilterEvaluation { .. } => false,
            MirrorMakerError::TransformEvaluation { .. } => false,
            MirrorMakerError::JsonPathNotFound { .. } => false,

            _ => false,
        }
    }

    /// Returns suggested recovery action
    pub fn recovery_action(&self) -> RecoveryAction {
        match self {
            // Retry with backoff
            e if e.is_recoverable() => RecoveryAction::RetryWithBackoff,

            // Configuration errors: fail fast
            MirrorMakerError::Config(_)
            | MirrorMakerError::ConfigWithField { .. }
            | MirrorMakerError::ConfigMissing { .. }
            | MirrorMakerError::ConfigInvalid { .. } => RecoveryAction::FailFast,

            // DSL errors: fail fast (startup validation)
            MirrorMakerError::DslParse { .. }
            | MirrorMakerError::InvalidFilter { .. }
            | MirrorMakerError::InvalidTransform { .. } => RecoveryAction::FailFast,

            // Message-level errors: send to DLQ
            MirrorMakerError::Processing(_)
            | MirrorMakerError::ProcessingWithContext { .. }
            | MirrorMakerError::MessageDeserialization { .. }
            | MirrorMakerError::FilterEvaluation { .. }
            | MirrorMakerError::TransformEvaluation { .. }
            | MirrorMakerError::JsonPathNotFound { .. } => RecoveryAction::SendToDlq,

            // Retry exhausted: send to DLQ
            MirrorMakerError::RetryExhausted { .. } => RecoveryAction::SendToDlq,

            // Default: skip and log
            _ => RecoveryAction::SkipAndLog,
        }
    }

    /// Add context to the error
    pub fn with_context(self, context: impl Into<String>) -> Self {
        match self {
            MirrorMakerError::Processing(message) => {
                MirrorMakerError::Processing(format!("{}: {}", context.into(), message))
            }
            MirrorMakerError::ProcessingWithContext {
                message,
                topic,
                partition,
                offset,
            } => MirrorMakerError::ProcessingWithContext {
                message: format!("{}: {}", context.into(), message),
                topic,
                partition,
                offset,
            },
            _ => self,
        }
    }
}

/// Recovery action for errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Retry with exponential backoff
    RetryWithBackoff,
    /// Send message to dead letter queue
    SendToDlq,
    /// Skip message and log error
    SkipAndLog,
    /// Fail immediately and halt processing
    FailFast,
}

pub type Result<T> = std::result::Result<T, MirrorMakerError>;
