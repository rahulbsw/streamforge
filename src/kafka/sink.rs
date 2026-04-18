use crate::compression::Compressor;
use crate::config::{CompressionAlgo, CompressionType, MirrorMakerConfig};
use crate::envelope::MessageEnvelope;
use crate::error::MirrorMakerError;
use crate::partitioner::{DefaultPartitioner, FieldPartitioner, Partitioner};
use crate::Result;
use rdkafka::config::ClientConfig;
use rdkafka::message::OwnedHeaders;
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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
/// - Output topic templates: `"mirror.{source_topic}"` resolves at send time
pub struct KafkaSink {
    producer: Arc<FutureProducer>,
    /// Output topic name or template. Templates contain `{source_topic}` which
    /// is replaced with the source topic name from the message envelope.
    output_template: String,
    /// True when `output_template` contains `{source_topic}`.
    is_template: bool,
    partitioner: Arc<dyn Partitioner>,
    compressor: Compressor,
    /// Per-topic partition count cache. Fixed topics are populated at construction;
    /// template-resolved topics are populated lazily on first send.
    partition_cache: Mutex<HashMap<String, i32>>,
}

impl KafkaSink {
    /// Create a new KafkaSink.
    ///
    /// `output_template` may be a plain topic name (`"events-copy"`) or a template
    /// containing `{source_topic}` (`"mirror.{source_topic}"`). Templates are resolved
    /// at send time using the source topic stored in the message envelope.
    pub async fn new(
        config: &MirrorMakerConfig,
        output_template: String,
        partition_field: Option<String>,
    ) -> Result<Self> {
        let target_broker = config.get_target_broker();
        let compression_type = config.compression.compression_type;
        let compression_algo = config.compression.compression_algo;
        let is_template = output_template.contains("{source_topic}");

        info!(
            "Creating KafkaSink: broker={}, output={}, template={}, compression={:?}",
            target_broker, output_template, is_template, compression_type
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

        // Apply user-provided producer properties
        for (key, value) in &config.producer_properties {
            producer_config.set(key, value);
        }

        // Create producer
        let producer: FutureProducer = producer_config.create()?;
        let producer = Arc::new(producer);

        // Pre-fetch partition count for fixed (non-template) topics.
        // Template topics are resolved lazily at send time.
        let mut initial_cache = HashMap::new();
        if !is_template {
            let metadata = tokio::task::block_in_place(|| {
                producer.client().fetch_metadata(
                    Some(&output_template),
                    Timeout::After(Duration::from_secs(10)),
                )
            })?;
            let num_partitions = metadata
                .topics()
                .iter()
                .find(|t| t.name() == output_template)
                .map(|t| t.partitions().len() as i32)
                .unwrap_or_else(|| {
                    warn!(
                        "Topic '{}' not found in broker metadata — defaulting to 1 partition. \
                         Verify the topic exists on the target cluster.",
                        output_template
                    );
                    1
                });
            info!(
                "Target topic '{}' has {} partitions",
                output_template, num_partitions
            );
            initial_cache.insert(output_template.clone(), num_partitions);
        } else {
            info!(
                "Output template '{}' — partition count fetched on first send per resolved topic",
                output_template
            );
        }

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
            output_template,
            is_template,
            partitioner,
            compressor,
            partition_cache: Mutex::new(initial_cache),
        })
    }

    /// Resolve the output topic name for a message.
    ///
    /// For fixed topics returns a clone of the stored template string.
    /// For templates substitutes `{source_topic}` with the envelope's source topic.
    /// Returns an error if the output is a template and the envelope carries no source topic.
    fn resolve_topic(&self, source_topic: Option<&str>) -> Result<String> {
        if self.is_template {
            let src = source_topic.ok_or_else(|| {
                MirrorMakerError::Processing(
                    "Template topic resolution failed: message envelope has no source topic. \
                     Ensure the consumer sets envelope.topic before routing to a template sink."
                        .to_string(),
                )
            })?;
            Ok(self.output_template.replace("{source_topic}", src))
        } else {
            Ok(self.output_template.clone())
        }
    }

    /// Return the partition count for `topic`, fetching from the broker on first encounter.
    ///
    /// The broker fetch is a blocking call wrapped in `block_in_place` so the
    /// Tokio scheduler can compensate for the thread being held during the
    /// network round-trip. Mutex lock/unlock is brief on both sides of the
    /// fetch, so no await point crosses a held lock.
    fn get_or_fetch_partitions(&self, topic: &str) -> Result<i32> {
        // Fast path: already cached — lock is held only for a map lookup.
        {
            let cache = self
                .partition_cache
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(&n) = cache.get(topic) {
                return Ok(n);
            }
        }
        // Slow path: blocking broker metadata fetch.
        // `block_in_place` signals Tokio to move other tasks off this thread
        // while the blocking call runs, preventing scheduler starvation.
        let metadata = tokio::task::block_in_place(|| {
            self.producer
                .client()
                .fetch_metadata(Some(topic), Timeout::After(Duration::from_secs(10)))
        })?;
        let num_partitions = metadata
            .topics()
            .iter()
            .find(|t| t.name() == topic)
            .map(|t| t.partitions().len() as i32)
            .unwrap_or_else(|| {
                warn!(
                    "Topic '{}' not found in broker metadata — defaulting to 1 partition. \
                     Verify the topic exists on the target cluster.",
                    topic
                );
                1
            });
        info!(
            "Discovered topic '{}' with {} partitions (via template resolution)",
            topic, num_partitions
        );
        // Recover from mutex poison: a previous panic between the two lock sites
        // should not permanently break all future sends.
        self.partition_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(topic.to_string(), num_partitions);
        Ok(num_partitions)
    }

    /// Send a message envelope to the target Kafka topic.
    ///
    /// If the output is a template, the target topic is resolved from the
    /// envelope's source topic field before sending.
    pub async fn send(&self, envelope: MessageEnvelope) -> Result<()> {
        let target_topic = self.resolve_topic(envelope.topic.as_deref())?;
        let num_partitions = self.get_or_fetch_partitions(&target_topic)?;

        // Determine partition
        let key_for_partition = envelope.key.as_ref().unwrap_or(&Value::Null);
        let partition = self.partitioner.partition(
            &target_topic,
            key_for_partition,
            &envelope.value,
            num_partitions,
        );

        debug!(
            "Sending to topic '{}' partition {}/{}",
            target_topic, partition, num_partitions
        );

        // Serialize key (if present)
        let key_bytes = envelope.key.as_ref().map(serde_json::to_vec).transpose()?;

        // Serialize value
        let mut value_bytes = serde_json::to_vec(&envelope.value)?;

        // Apply enveloped compression if configured
        if matches!(self.compressor.compression_type, CompressionType::Enveloped) {
            value_bytes = self.compressor.compress(&value_bytes)?;
        }

        // Build headers from envelope
        let mut headers = OwnedHeaders::new();
        for (name, value) in &envelope.headers {
            headers = headers.insert(rdkafka::message::Header {
                key: name,
                value: Some(value),
            });
        }

        // Create record with all envelope components
        let mut record = FutureRecord::to(&target_topic)
            .partition(partition)
            .payload(&value_bytes)
            .headers(headers);

        // Add key if present
        if let Some(ref kb) = key_bytes {
            record = record.key(kb);
        }

        // Add timestamp if present
        if let Some(ts) = envelope.timestamp {
            record = record.timestamp(ts);
        }

        // Send record
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

    /// Flush all pending messages, propagating any producer error.
    pub async fn flush(&self) -> Result<()> {
        self.producer
            .flush(Timeout::After(Duration::from_secs(30)))
            .map_err(|e| {
                error!(
                    "Failed to flush producer for '{}': {}",
                    self.output_template, e
                );
                MirrorMakerError::Kafka(e.to_string())
            })
    }
}

/// Multi-destination sink manager
pub struct MultiSink {
    sinks: HashMap<String, Arc<KafkaSink>>,
}

impl Default for MultiSink {
    fn default() -> Self {
        Self::new()
    }
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

    pub async fn send_to(&self, topic: &str, envelope: MessageEnvelope) -> Result<()> {
        if let Some(sink) = self.sinks.get(topic) {
            sink.send(envelope).await
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
    use crate::config::{CommitStrategyConfig, CompressionConfig, MirrorMakerConfig};

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
            transform: None,
            consumer_properties: HashMap::new(),
            producer_properties: HashMap::new(),
            security: None,
            commit_strategy: CommitStrategyConfig::default(),
            cache: None,
            observability: Default::default(),
            retry: Default::default(),
            dlq: Default::default(),
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
        let multi = MultiSink::new();
        assert_eq!(multi.sinks.len(), 0);
    }

    mod topic_template_tests {
        use crate::envelope::MessageEnvelope;
        use serde_json::json;

        /// Mirror `KafkaSink::resolve_topic` logic for unit tests without a real broker.
        fn resolve(
            template: &str,
            is_template: bool,
            source: Option<&str>,
        ) -> Result<String, String> {
            if is_template {
                source
                    .ok_or_else(|| "no source topic".to_string())
                    .map(|src| template.replace("{source_topic}", src))
            } else {
                Ok(template.to_string())
            }
        }

        #[test]
        fn test_fixed_topic_unchanged() {
            assert_eq!(
                resolve("events-copy", false, Some("events")).unwrap(),
                "events-copy"
            );
        }

        #[test]
        fn test_template_replaced_with_source_topic() {
            assert_eq!(
                resolve("mirror.{source_topic}", true, Some("payments")).unwrap(),
                "mirror.payments"
            );
        }

        #[test]
        fn test_template_with_prefix_and_suffix() {
            assert_eq!(
                resolve("prod.{source_topic}.v2", true, Some("orders")).unwrap(),
                "prod.orders.v2"
            );
        }

        #[test]
        fn test_template_no_source_is_error() {
            // A missing source topic must now produce an error — not silently
            // route to "mirror.unknown".
            assert!(resolve("copy.{source_topic}", true, None).is_err());
        }

        #[test]
        fn test_is_template_detection() {
            assert!("mirror.{source_topic}".contains("{source_topic}"));
            assert!("{source_topic}-copy".contains("{source_topic}"));
            assert!(!"fixed-topic".contains("{source_topic}"));
        }

        #[test]
        fn test_envelope_source_topic_used() {
            let mut envelope = MessageEnvelope::new(json!({"event": "login"}));
            envelope.topic = Some("auth-events".to_string());

            let resolved = resolve("processed.{source_topic}", true, envelope.topic.as_deref());
            assert_eq!(resolved.unwrap(), "processed.auth-events");
        }
    }
}
