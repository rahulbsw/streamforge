use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info, warn};
use tracing_subscriber;
use futures::stream::StreamExt;
use streamforge::filter::{Filter, Transform};
use streamforge::filter_parser::{parse_filter, parse_transform};
use streamforge::kafka::KafkaSink;
use streamforge::metrics::{Stats, StatsReporter};
use streamforge::processor::{MessageProcessor, MultiDestinationProcessor, SingleDestinationProcessor, DestinationProcessor};
use streamforge::{MirrorMakerConfig, MirrorMakerError, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("Starting Streamforge - High-performance Kafka streaming toolkit");

    // Load configuration (from file or environment)
    let config = load_config()?;
    info!("Loaded configuration: appid={}", config.appid);

    // Create statistics
    let stats = Arc::new(Stats::new());

    // Build processor based on configuration
    let processor: Arc<dyn MessageProcessor> = if let Some(routing) = &config.routing {
        info!("Multi-destination routing enabled");
        build_multi_destination_processor(&config, routing, stats.clone()).await?
    } else {
        info!("Single-destination mode");
        build_single_destination_processor(&config, stats.clone()).await?
    };

    // Create Kafka consumer
    let consumer = create_consumer(&config)?;

    // Subscribe to input topics
    let topics: Vec<&str> = config.input.split(',').collect();
    consumer.subscribe(&topics)?;
    info!("Subscribed to topics: {:?}", topics);

    // Start statistics reporter
    let stats_clone = stats.clone();
    tokio::spawn(async move {
        let mut reporter = StatsReporter::new(stats_clone);
        let mut ticker = interval(Duration::from_secs(10));
        loop {
            ticker.tick().await;
            reporter.report();
        }
    });

    // Main processing loop with concurrent message processing
    let parallelism = config.threads * 10;
    let batch_size = 100; // Process 100 messages per batch
    let manual_commit = config.commit_strategy.manual_commit;

    info!("Starting concurrent message processing (parallelism: {}, batch_size: {})", parallelism, batch_size);

    if manual_commit {
        info!("Using batch-level commits for at-least-once delivery");
    }

    let mut message_stream = consumer.stream();

    loop {
        // Collect batch of messages
        let mut batch = Vec::new();

        for _ in 0..batch_size {
            match tokio::time::timeout(Duration::from_millis(100), message_stream.next()).await {
                Ok(Some(msg_result)) => batch.push(msg_result),
                Ok(None) => break, // Stream ended
                Err(_) => break,   // Timeout - process what we have
            }
        }

        if batch.is_empty() {
            continue;
        }

        // Process batch concurrently
        let results = futures::stream::iter(batch.into_iter())
            .map(|msg_result| {
                let processor = processor.clone();
                let stats = stats.clone();

                async move {
                    match msg_result {
                        Ok(msg) => {
                            stats.processed();

                            // Extract key and value
                            let key = match msg.key() {
                                Some(k) => match serde_json::from_slice::<Value>(k) {
                                    Ok(v) => v,
                                    Err(_) => {
                                        Value::String(String::from_utf8_lossy(k).to_string())
                                    }
                                },
                                None => Value::Null,
                            };

                            let value = match msg.payload() {
                                Some(v) => match serde_json::from_slice::<Value>(v) {
                                    Ok(json) => json,
                                    Err(e) => {
                                        warn!("Failed to parse message as JSON: {}", e);
                                        stats.error();
                                        return Err(MirrorMakerError::Processing(format!("Invalid JSON: {}", e)));
                                    }
                                },
                                None => {
                                    warn!("Empty message payload");
                                    return Err(MirrorMakerError::Processing("Empty payload".to_string()));
                                }
                            };

                            // Process message
                            match processor.process(key, value).await {
                                Ok(_) => {
                                    stats.completed();
                                    Ok(())
                                }
                                Err(e) => {
                                    error!("Failed to process message: {}", e);
                                    stats.error();
                                    Err(e)
                                }
                            }
                        }
                        Err(e) => {
                            error!("Kafka consumer error: {}", e);
                            stats.error();
                            Err(MirrorMakerError::Kafka(e))
                        }
                    }
                }
            })
            .buffer_unordered(parallelism)
            .collect::<Vec<_>>()
            .await;

        // Commit batch if using manual commits
        if manual_commit {
            let all_success = results.iter().all(|r| r.is_ok());

            if all_success {
                // Commit the batch
                if let Err(e) = consumer.commit_consumer_state(
                    rdkafka::consumer::CommitMode::Async
                ) {
                    error!("Failed to commit offsets: {}", e);
                    stats.error();
                }
            } else {
                warn!("Batch had errors, skipping commit (messages will be reprocessed)");
            }
        }
    }
}

fn load_config() -> Result<MirrorMakerConfig> {
    // Check for config file path in environment or use default
    let config_path = std::env::var("CONFIG_FILE").unwrap_or_else(|_| "config.json".to_string());

    if std::path::Path::new(&config_path).exists() {
        info!("Loading configuration from: {}", config_path);
        MirrorMakerConfig::from_file(&config_path)
    } else {
        // Create default config for testing
        warn!("Config file not found, using default configuration");
        Ok(create_default_config())
    }
}

fn create_default_config() -> MirrorMakerConfig {
    MirrorMakerConfig {
        appid: "streamforge".to_string(),
        bootstrap: "localhost:9092".to_string(),
        input: "input-topic".to_string(),
        output: Some("output-topic".to_string()),
        target_broker: None,
        offset: "latest".to_string(),
        threads: 4,
        compression: Default::default(),
        routing: None,
        consumer_properties: Default::default(),
        producer_properties: Default::default(),
        security: None,
        commit_strategy: Default::default(),
        cache: None,
    }
}

fn create_consumer(config: &MirrorMakerConfig) -> Result<StreamConsumer> {
    let mut consumer_config = ClientConfig::new();
    consumer_config
        .set("bootstrap.servers", &config.bootstrap)
        .set("group.id", &config.appid)
        .set("auto.offset.reset", &config.offset);

    // Configure commit strategy based on config
    let auto_commit = !config.commit_strategy.manual_commit;
    consumer_config.set("enable.auto.commit", auto_commit.to_string());

    if !auto_commit {
        info!("Manual commit enabled - at-least-once semantics");
        info!("Commit mode: {:?}", config.commit_strategy.commit_mode);
    } else {
        warn!("Auto-commit enabled - at-most-once semantics (messages may be lost on failure)");
    }

    // Apply security configuration
    config.apply_security(&mut consumer_config);

    // Apply user-provided consumer properties (can override security settings if needed)
    for (key, value) in &config.consumer_properties {
        consumer_config.set(key, value);
    }

    let consumer: StreamConsumer = consumer_config.create()?;
    Ok(consumer)
}

async fn build_single_destination_processor(
    config: &MirrorMakerConfig,
    _stats: Arc<Stats>,
) -> Result<Arc<dyn MessageProcessor>> {
    let output_topic = config
        .output
        .clone()
        .ok_or_else(|| MirrorMakerError::Config("Output topic not specified".to_string()))?;

    let sink = KafkaSink::new(config, output_topic, None).await?;
    Ok(Arc::new(SingleDestinationProcessor::new(Arc::new(sink))))
}

async fn build_multi_destination_processor(
    config: &MirrorMakerConfig,
    routing: &streamforge::RoutingConfig,
    _stats: Arc<Stats>,
) -> Result<Arc<dyn MessageProcessor>> {
    let mut destinations = Vec::new();

    for dest in &routing.destinations {
        info!("Setting up destination: {}", dest.output);

        // Create sink
        let sink = KafkaSink::new(config, dest.output.clone(), dest.partition.clone()).await?;

        // Create filter if specified
        let filter: Option<Arc<dyn Filter>> = if let Some(ref filter_expr) = dest.filter {
            info!("  Filter: {}", filter_expr);
            Some(parse_filter(filter_expr)?)
        } else {
            None
        };

        // Create transform if specified
        let transform: Option<Arc<dyn Transform>> = if let Some(ref transform_expr) = dest.transform {
            info!("  Transform: {}", transform_expr);
            Some(parse_transform(transform_expr)?)
        } else {
            None
        };

        // Create destination processor
        let dest_processor = DestinationProcessor::new(
            Arc::new(sink),
            filter,
            transform,
            dest.output.clone(),
        );

        destinations.push(dest_processor);
    }

    Ok(Arc::new(MultiDestinationProcessor::new(
        destinations,
        routing.path.clone(),
    )))
}
