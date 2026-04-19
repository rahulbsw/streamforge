///! Integration test infrastructure
///!
///! Shared utilities for end-to-end pipeline tests using testcontainers.

use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{Headers, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use streamforge::config::{CommitMode, CommitStrategyConfig};
use streamforge::{DlqConfig, MirrorMakerConfig, RetryConfig};
use testcontainers::clients::Cli;
use testcontainers::core::WaitFor;
use testcontainers::{Container, Image};
use tokio::time::timeout;

/// Redpanda container image (Kafka-compatible, faster startup)
#[derive(Debug)]
pub struct Redpanda {
    _priv: (),
}

impl Default for Redpanda {
    fn default() -> Self {
        Self { _priv: () }
    }
}

impl Image for Redpanda {
    type Args = ();

    fn name(&self) -> String {
        "docker.redpanda.com/redpandadata/redpanda".to_string()
    }

    fn tag(&self) -> String {
        "v23.3.3".to_string()
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("Successfully started Redpanda")]
    }

    fn env_vars(&self) -> Box<dyn Iterator<Item = (&String, &String)> + '_> {
        Box::new(std::iter::empty())
    }

    fn volumes(&self) -> Box<dyn Iterator<Item = (&String, &String)> + '_> {
        Box::new(std::iter::empty())
    }

    fn expose_ports(&self) -> Vec<u16> {
        vec![9092]
    }

    fn entrypoint(&self) -> Option<String> {
        Some("redpanda".to_string())
    }

    fn args(&self) -> Box<dyn Iterator<Item = String> + '_> {
        Box::new(
            vec![
                "start".to_string(),
                "--smp".to_string(),
                "1".to_string(),
                "--memory".to_string(),
                "1G".to_string(),
                "--overprovisioned".to_string(),
                "--node-id".to_string(),
                "0".to_string(),
                "--check=false".to_string(),
                "--kafka-addr".to_string(),
                "PLAINTEXT://0.0.0.0:9092".to_string(),
                "--advertise-kafka-addr".to_string(),
                "PLAINTEXT://127.0.0.1:9092".to_string(),
            ]
            .into_iter(),
        )
    }
}

/// Test Kafka environment using testcontainers
pub struct TestKafka<'a> {
    pub container: Container<'a, Redpanda>,
    pub bootstrap_servers: String,
    pub admin_client: AdminClient<DefaultClientContext>,
}

impl<'a> TestKafka<'a> {
    /// Start a test Kafka container
    pub async fn start(docker: &'a Cli) -> Self {
        let container = docker.run(Redpanda::default());
        let port = container.get_host_port_ipv4(9092);
        let bootstrap_servers = format!("127.0.0.1:{}", port);

        // Wait for Kafka to be ready
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Create admin client
        let admin_client: AdminClient<DefaultClientContext> = ClientConfig::new()
            .set("bootstrap.servers", &bootstrap_servers)
            .create()
            .expect("Failed to create admin client");

        Self {
            container,
            bootstrap_servers,
            admin_client,
        }
    }

    /// Create a topic with specified partitions
    pub async fn create_topic(&self, topic: &str, partitions: i32) -> Result<(), String> {
        let topics = vec![NewTopic::new(topic, partitions, TopicReplication::Fixed(1))];
        let options = AdminOptions::new().request_timeout(Some(Duration::from_secs(10)));

        self.admin_client
            .create_topics(&topics, &options)
            .await
            .map_err(|e| format!("Failed to create topic {}: {:?}", topic, e))?;

        // Wait for topic to be ready
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    /// Create multiple topics
    pub async fn create_topics(&self, topics: &[(&str, i32)]) -> Result<(), String> {
        for (topic, partitions) in topics {
            self.create_topic(topic, *partitions).await?;
        }
        Ok(())
    }

    /// Get bootstrap servers for producer/consumer
    pub fn bootstrap(&self) -> &str {
        &self.bootstrap_servers
    }
}

/// Create a basic test config for integration tests
pub fn test_config_base(bootstrap: &str, input: &str, output: &str) -> MirrorMakerConfig {
    MirrorMakerConfig {
        appid: format!("test-{}", uuid::Uuid::new_v4()),
        bootstrap: bootstrap.to_string(),
        input: input.to_string(),
        output: Some(output.to_string()),
        target_broker: None,
        offset: "earliest".to_string(),
        threads: 2,
        compression: Default::default(),
        routing: None,
        transform: None,
        consumer_properties: HashMap::new(),
        producer_properties: HashMap::new(),
        security: None,
        commit_strategy: CommitStrategyConfig {
            manual_commit: true,
            commit_mode: CommitMode::Async,
            commit_interval_ms: 5000,
            enable_dlq: false,
            dlq_topic: None,
            max_retries: 3,
            retry_backoff: Default::default(),
        },
        cache: None,
        observability: Default::default(),
        retry: RetryConfig::default(),
        dlq: DlqConfig {
            enabled: false,
            ..Default::default()
        },
    }
}

/// Add retry configuration to a test config
pub fn with_retry(mut config: MirrorMakerConfig, max_attempts: u32) -> MirrorMakerConfig {
    config.retry = RetryConfig {
        max_attempts,
        initial_delay_ms: 50,
        max_delay_ms: 1000,
        multiplier: 2.0,
        jitter: 0.1,
    };
    config
}

/// Add DLQ configuration to a test config
pub fn with_dlq(mut config: MirrorMakerConfig, dlq_topic: &str) -> MirrorMakerConfig {
    config.dlq = DlqConfig {
        enabled: true,
        topic: dlq_topic.to_string(),
        brokers: None,
        include_original_headers: true,
        include_stack_trace: false,
        max_dlq_retries: 3,
        compression: None,
    };
    config
}

/// Create a test producer
pub fn create_producer(bootstrap: &str) -> FutureProducer {
    ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .set("message.timeout.ms", "10000")
        .create()
        .expect("Failed to create test producer")
}

/// Create a test consumer
pub fn create_consumer(bootstrap: &str, group_id: &str, topic: &str) -> StreamConsumer {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .set("group.id", group_id)
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false")
        .create()
        .expect("Failed to create test consumer");

    consumer
        .subscribe(&[topic])
        .expect("Failed to subscribe to topic");

    consumer
}

/// Send a test message to Kafka
pub async fn send_message(
    producer: &FutureProducer,
    topic: &str,
    key: Option<&str>,
    value: Value,
) -> Result<(), String> {
    let value_bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;

    let mut record = FutureRecord::to(topic).payload(&value_bytes);

    if let Some(k) = key {
        record = record.key(k);
    }

    producer
        .send(record, Duration::from_secs(5))
        .await
        .map_err(|(e, _)| format!("Failed to send message: {:?}", e))?;

    Ok(())
}

/// Consume messages from a topic with timeout
pub async fn consume_messages(
    consumer: &StreamConsumer,
    count: usize,
    timeout_secs: u64,
) -> Result<Vec<Value>, String> {
    let mut messages = Vec::new();
    let deadline = timeout(Duration::from_secs(timeout_secs), async {
        use futures::stream::StreamExt;
        let mut stream = consumer.stream();

        while messages.len() < count {
            if let Some(result) = stream.next().await {
                match result {
                    Ok(msg) => {
                        let payload = msg.payload().ok_or("No payload")?;
                        let value: Value =
                            serde_json::from_slice(payload).map_err(|e| e.to_string())?;
                        messages.push(value);
                    }
                    Err(e) => return Err(format!("Consumer error: {:?}", e)),
                }
            }
        }
        Ok(messages)
    });

    deadline
        .await
        .map_err(|_| format!("Timeout waiting for {} messages", count))?
}

/// Assert DLQ message has expected headers
pub fn assert_dlq_headers(
    msg: &impl Message,
    expected_error_type: &str,
    expected_source_topic: Option<&str>,
) {
    let headers = msg.headers().expect("No headers in DLQ message");

    let mut error_type = None;
    let mut source_topic = None;

    for header in headers.iter() {
        match header.key {
            "x-streamforge-error-type" => {
                error_type = header.value.map(|v| String::from_utf8_lossy(v).to_string());
            }
            "x-streamforge-source-topic" => {
                source_topic = header.value.map(|v| String::from_utf8_lossy(v).to_string());
            }
            _ => {}
        }
    }

    assert_eq!(
        error_type.as_deref(),
        Some(expected_error_type),
        "Expected error type '{}', got {:?}",
        expected_error_type,
        error_type
    );

    if let Some(expected) = expected_source_topic {
        assert_eq!(
            source_topic.as_deref(),
            Some(expected),
            "Expected source topic '{}', got {:?}",
            expected,
            source_topic
        );
    }
}

/// Wait for messages to appear in a topic
pub async fn wait_for_messages(
    bootstrap: &str,
    topic: &str,
    expected_count: usize,
    timeout_secs: u64,
) -> Result<Vec<Value>, String> {
    let group_id = format!("test-wait-{}", uuid::Uuid::new_v4());
    let consumer = create_consumer(bootstrap, &group_id, topic);
    consume_messages(&consumer, expected_count, timeout_secs).await
}
