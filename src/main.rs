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
    const BATCH_SIZE: usize = 100;
    const BATCH_FILL_TIMEOUT_MS: u64 = 100;
    const PARALLELISM_FACTOR: usize = 10;

    let parallelism = (config.threads * PARALLELISM_FACTOR).max(1);
    let manual_commit = config.commit_strategy.manual_commit;
    let commit_mode = match config.commit_strategy.commit_mode {
        streamforge::config::CommitMode::Async => rdkafka::consumer::CommitMode::Async,
        streamforge::config::CommitMode::Sync => rdkafka::consumer::CommitMode::Sync,
    };

    info!("Starting concurrent message processing (parallelism: {}, batch_size: {})", parallelism, BATCH_SIZE);

    if manual_commit {
        info!("Using batch-level commits for at-least-once delivery (mode: {:?})", commit_mode);
    }

    let mut message_stream = consumer.stream();

    loop {
        // Collect batch of messages with single deadline
        let mut batch = Vec::with_capacity(BATCH_SIZE);
        let deadline = tokio::time::Instant::now() + Duration::from_millis(BATCH_FILL_TIMEOUT_MS);
        let mut stream_ended = false;

        for _ in 0..BATCH_SIZE {
            match tokio::time::timeout_at(deadline, message_stream.next()).await {
                Ok(Some(msg_result)) => batch.push(msg_result),
                Ok(None) => {
                    stream_ended = true;
                    break;
                }
                Err(_) => break, // Timeout - process what we have
            }
        }

        if batch.is_empty() {
            if stream_ended {
                info!("Consumer stream ended, shutting down");
                break;
            }
            // Backoff to avoid spin loop when idle
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        }

        // Process batch concurrently
        let stream = futures::stream::iter(batch.into_iter())
            .map(|msg_result| {
                let processor = processor.clone();
                let stats = stats.clone();

                async move {
                    match msg_result {
                        Ok(msg) => {
                            stats.processed();

                            let key = parse_message_key(msg.key());
                            let value = match parse_message_value(msg.payload()) {
                                Ok(v) => v,
                                Err(e) => {
                                    error!(
                                        "Failed to parse message: {} (topic={}, partition={}, offset={}, key={:?})",
                                        e,
                                        msg.topic(),
                                        msg.partition(),
                                        msg.offset(),
                                        msg.key().map(|k| String::from_utf8_lossy(k).to_string())
                                    );
                                    stats.error();
                                    return Err(e);
                                }
                            };

                            // Process message
                            match processor.process(key, value).await {
                                Ok(_) => {
                                    stats.completed();
                                    Ok(())
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to process message: {} (topic={}, partition={}, offset={})",
                                        e, msg.topic(), msg.partition(), msg.offset()
                                    );
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
            .buffer_unordered(parallelism);

        // Different handling based on commit mode
        if manual_commit {
            // Collect results to check success before committing
            let results: Vec<_> = stream.collect().await;
            let error_count = results.iter().filter(|r| r.is_err()).count();

            if error_count == 0 {
                // All messages processed successfully - commit with retry
                const MAX_COMMIT_RETRIES: u32 = 3;
                let mut retry_count = 0;

                loop {
                    match consumer.commit_consumer_state(commit_mode) {
                        Ok(_) => {
                            if retry_count > 0 {
                                info!("Successfully committed batch after {} retries", retry_count);
                            }
                            break;
                        }
                        Err(e) => {
                            error!("Failed to commit offsets (attempt {}/{}): {}",
                                   retry_count + 1, MAX_COMMIT_RETRIES, e);
                            stats.error();

                            retry_count += 1;
                            if retry_count >= MAX_COMMIT_RETRIES {
                                error!("CRITICAL: Unable to commit offsets after {} attempts. \
                                        Halting to prevent data loss. Manual intervention required.",
                                        MAX_COMMIT_RETRIES);
                                return Err(MirrorMakerError::Kafka(e));
                            }

                            // Exponential backoff
                            let backoff_ms = 100 * 2_u64.pow(retry_count);
                            warn!("Retrying commit in {}ms...", backoff_ms);
                            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        }
                    }
                }
            } else {
                // Batch has errors - halt processing to prevent skipping failed messages
                error!("CRITICAL: Batch processing failed with {} errors out of {} messages. \
                        Halting to prevent data loss.", error_count, results.len());
                error!("Failed messages will be reprocessed on restart. \
                        Note: Successfully processed messages in this batch may create duplicates.");
                return Err(MirrorMakerError::Processing(
                    format!("Batch processing failed: {} errors", error_count)
                ));
            }
        } else {
            // Auto-commit mode: collect results to count errors
            let results: Vec<_> = stream.collect().await;
            let error_count = results.iter().filter(|r| r.is_err()).count();

            // Log each error
            for result in results.iter() {
                if let Err(e) = result {
                    error!("Message processing failed in auto-commit mode (data loss): {}", e);
                }
            }

            if error_count > 0 {
                warn!("Batch completed with {} errors in auto-commit mode. \
                       Failed messages will NOT be reprocessed (data loss). \
                       Consider enabling manual_commit for at-least-once delivery guarantees.",
                       error_count);
            }
        }
    }

    Ok(())
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

/// Parse Kafka message key into a JSON Value
fn parse_message_key(raw: Option<&[u8]>) -> Value {
    match raw {
        Some(k) => match serde_json::from_slice::<Value>(k) {
            Ok(v) => v,
            Err(_) => Value::String(String::from_utf8_lossy(k).to_string()),
        },
        None => Value::Null,
    }
}

/// Parse Kafka message payload into a JSON Value
fn parse_message_value(raw: Option<&[u8]>) -> Result<Value> {
    match raw {
        Some(v) => serde_json::from_slice::<Value>(v)
            .map_err(|e| MirrorMakerError::Processing(format!("Invalid JSON: {}", e))),
        None => Err(MirrorMakerError::Processing("Empty payload".to_string())),
    }
}
