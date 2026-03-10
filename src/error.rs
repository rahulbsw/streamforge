use thiserror::Error;

#[derive(Error, Debug)]
pub enum MirrorMakerError {
    #[error("Kafka error: {0}")]
    Kafka(#[from] rdkafka::error::KafkaError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Compression error: {0}")]
    Compression(String),

    #[error("Processing error: {0}")]
    Processing(String),
}

pub type Result<T> = std::result::Result<T, MirrorMakerError>;
