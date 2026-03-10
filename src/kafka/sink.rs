use crate::compression::Compressor;
use crate::config::{CompressionAlgo, CompressionType, MirrorMakerConfig};
use crate::partitioner::{DefaultPartitioner, FieldPartitioner, Partitioner};
use crate::Result;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// CustomKafkaSink - Rust equivalent of Java's CustomKafkaSink
///
/// Enables cross-cluster Kafka mirroring by creating a separate KafkaProducer
/// for the target cluster. Supports:
/// - Cross-cluster mirroring (different bootstrap servers)
/// - Custom partitioning strategies
/// - Native Kafka compression (gzip, snappy, zstd, lz4)
/// - Multi-destination routing
pub struct KafkaSink {
    producer: Arc<FutureProducer>,
    target_topic: String,
    partitioner: Arc<dyn Partitioner>,
    compressor: Compressor,
    num_partitions: i32,
}

impl KafkaSink {
    /// Create a new KafkaSink
    pub async fn new(
        config: &MirrorMakerConfig,
        target_topic: String,
        partition_field: Option<String>,
    ) -> Result<Self> {
        let target_broker = config.get_target_broker();
        let compression_type = config.compression.compression_type;
        let compression_algo = config.compression.compression_algo;

        info!(
            "Creating KafkaSink: broker={}, topic={}, compression={:?}",
            target_broker, target_topic, compression_type
        );

        // Build producer configuration
        let mut producer_config = ClientConfig::new();
        producer_config
            .set("bootstrap.servers", &target_broker)
            .set("acks", "all")
            .set("message.timeout.ms", "60000");

        // Configure native Kafka compression
        match compression_type {
            CompressionType::Raw => {
                let kafka_compression = match compression_algo {
                    CompressionAlgo::Gzip => "gzip",
                    CompressionAlgo::Snappy => "snappy",
                    CompressionAlgo::Zstd => "zstd",
                    CompressionAlgo::Lz4 => "lz4",
                };
                producer_config.set("compression.type", kafka_compression);
                info!("Using native Kafka compression: {}", kafka_compression);
            }
            CompressionType::None | CompressionType::Enveloped => {
                producer_config.set("compression.type", "none");
            }
        }

        // Apply security configuration
        config.apply_security(&mut producer_config);

        // Apply user-provided producer properties (can override security settings if needed)
        for (key, value) in &config.producer_properties {
            producer_config.set(key, value);
        }

        // Create producer
        let producer: FutureProducer = producer_config.create()?;
        let producer = Arc::new(producer);

        // Get partition count
        let metadata = producer
            .client()
            .fetch_metadata(Some(&target_topic), Timeout::After(Duration::from_secs(10)))?;

        let num_partitions = metadata
            .topics()
            .iter()
            .find(|t| t.name() == target_topic)
            .and_then(|t| Some(t.partitions().len() as i32))
            .unwrap_or(1);

        info!(
            "Target topic '{}' has {} partitions",
            target_topic, num_partitions
        );

        // Create partitioner
        let partitioner: Arc<dyn Partitioner> = if let Some(field) = partition_field {
            info!("Using field-based partitioner: {}", field);
            Arc::new(FieldPartitioner::new(field))
        } else {
            info!("Using default hash-based partitioner");
            Arc::new(DefaultPartitioner)
        };

        // Create compressor (for Enveloped compression type)
        let compressor = Compressor::new(compression_type, compression_algo);

        Ok(Self {
            producer,
            target_topic,
            partitioner,
            compressor,
            num_partitions,
        })
    }

    /// Send a message to the target Kafka topic
    pub async fn send(&self, key: Value, value: Value) -> Result<()> {
        // Determine partition
        let partition = self
            .partitioner
            .partition(&self.target_topic, &key, &value, self.num_partitions);

        debug!("Routing to partition {}/{}", partition, self.num_partitions);

        // Serialize key and value
        let key_bytes = serde_json::to_vec(&key)?;
        let mut value_bytes = serde_json::to_vec(&value)?;

        // Apply enveloped compression if configured
        if matches!(
            self.compressor.compression_type,
            CompressionType::Enveloped
        ) {
            value_bytes = self.compressor.compress(&value_bytes)?;
        }

        // Create and send record
        let record = FutureRecord::to(&self.target_topic)
            .partition(partition)
            .key(&key_bytes)
            .payload(&value_bytes);

        match self
            .producer
            .send(record, Timeout::After(Duration::from_secs(10)))
            .await
        {
            Ok((partition, offset)) => {
                debug!("Message sent: partition={}, offset={}", partition, offset);
                Ok(())
            }
            Err((err, _record)) => {
                error!("Failed to send message: {:?}", err);
                Err(err.into())
            }
        }
    }

    /// Flush all pending messages
    pub async fn flush(&self) -> Result<()> {
        use rdkafka::producer::Producer;
        self.producer.flush(Timeout::After(Duration::from_secs(30)));
        Ok(())
    }
}

/// Multi-destination sink manager
pub struct MultiSink {
    sinks: HashMap<String, Arc<KafkaSink>>,
}

impl MultiSink {
    pub fn new() -> Self {
        Self {
            sinks: HashMap::new(),
        }
    }

    pub async fn add_sink(&mut self, topic: String, sink: KafkaSink) {
        self.sinks.insert(topic.clone(), Arc::new(sink));
        info!("Added sink for topic: {}", topic);
    }

    pub async fn send_to(&self, topic: &str, key: Value, value: Value) -> Result<()> {
        if let Some(sink) = self.sinks.get(topic) {
            sink.send(key, value).await
        } else {
            warn!("No sink configured for topic: {}", topic);
            Ok(())
        }
    }

    pub async fn flush_all(&self) -> Result<()> {
        for (topic, sink) in &self.sinks {
            info!("Flushing sink for topic: {}", topic);
            sink.flush().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CompressionConfig, MirrorMakerConfig};
    use serde_json::json;

    fn create_test_config() -> MirrorMakerConfig {
        MirrorMakerConfig {
            appid: "test-app".to_string(),
            bootstrap: "localhost:9092".to_string(),
            input: "test-input".to_string(),
            output: Some("test-output".to_string()),
            target_broker: None,
            offset: "latest".to_string(),
            threads: 4,
            compression: CompressionConfig::default(),
            routing: None,
            consumer_properties: HashMap::new(),
            producer_properties: HashMap::new(),
            security: None,
        }
    }

    #[tokio::test]
    #[ignore] // Requires running Kafka
    async fn test_kafka_sink_creation() {
        let config = create_test_config();
        let result = KafkaSink::new(&config, "test-topic".to_string(), None).await;

        // This will fail without a running Kafka, but tests the API
        assert!(result.is_err());
    }

    #[test]
    fn test_multi_sink() {
        let mut multi = MultiSink::new();
        assert_eq!(multi.sinks.len(), 0);
    }
}
