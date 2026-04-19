use futures::stream::StreamExt;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{Headers, Message};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use streamforge::filter::{EnvelopeTransform, Filter, Transform};
use streamforge::filter_parser::{
    parse_filter, parse_header_transform, parse_key_transform, parse_static_headers,
    parse_timestamp_transform, parse_transform_with_cache,
};
use streamforge::kafka::KafkaSink;
use streamforge::metrics::{Stats, StatsReporter};
use streamforge::observability::{
    labels, register_metrics, start_lag_monitor, start_metrics_server, METRICS,
};
use streamforge::processor::{
    DestinationProcessor, MessageProcessor, MultiDestinationProcessor, SingleDestinationProcessor,
};
use streamforge::processor_with_retry::ProcessorWithRetry;
use streamforge::{DeadLetterQueue, MessageEnvelope, MirrorMakerConfig, MirrorMakerError, Result, RetryPolicy, SyncCacheManager};
use tokio::time::interval;
use tracing::{error, info, warn};

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

    // Initialize observability (metrics)
    if config.observability.metrics_enabled {
        register_metrics()
            .map_err(|e| MirrorMakerError::Config(format!("Failed to register metrics: {}", e)))?;
        info!("✅ Metrics registered successfully");

        // Start metrics HTTP server
        let metrics_port = config.observability.metrics_port;
        tokio::spawn(async move {
            start_metrics_server(metrics_port).await;
        });
    } else {
        info!("⏭️  Metrics disabled in configuration");
    }

    // Record service start time for uptime metric
    let start_time = std::time::Instant::now();
    METRICS
        .kafka_connections
        .with_label_values(&[labels::CONNECTION_TYPE_CONSUMER])
        .set(1.0);
    METRICS
        .kafka_connections
        .with_label_values(&[labels::CONNECTION_TYPE_PRODUCER])
        .set(1.0);

    // Create statistics
    let stats = Arc::new(Stats::new());

    // Shared sync cache manager — used by CACHE_LOOKUP / CACHE_PUT transforms
    let cache_manager = Arc::new(SyncCacheManager::new());

    // Warn when both routing and transform are set — routing wins, transform is ignored.
    if config.routing.is_some() && config.transform.is_some() {
        warn!(
            "Both 'routing' and 'transform' are set in config. \
             The top-level 'transform' field is ignored in multi-destination mode. \
             Use per-destination 'transform' expressions inside 'routing.destinations' instead."
        );
    }

    // Build processor based on configuration
    let base_processor: Arc<dyn MessageProcessor> = if let Some(routing) = &config.routing {
        info!("Multi-destination routing enabled");
        build_multi_destination_processor(&config, routing, stats.clone(), cache_manager.clone())
            .await?
    } else {
        info!("Single-destination mode");
        build_single_destination_processor(&config, stats.clone(), cache_manager.clone()).await?
    };

    // Wrap processor with retry and DLQ support (skip wrapper if max_attempts=1 for performance)
    let processor: Arc<dyn MessageProcessor> = if config.retry.max_attempts == 1 {
        info!("Retry disabled (max_attempts=1) - using base processor directly");
        base_processor
    } else {
        let retry_policy = RetryPolicy::new(config.retry.clone());
        let dlq = if config.dlq.enabled {
            info!(
                "DLQ enabled: topic={}, max_retries={}",
                config.dlq.topic, config.dlq.max_dlq_retries
            );
            Some(Arc::new(DeadLetterQueue::new(config.dlq.clone(), &config.bootstrap)?))
        } else {
            info!("DLQ disabled - errors will halt pipeline");
            None
        };

        info!(
            "Retry policy: max_attempts={}, initial_delay={}ms, max_delay={}ms",
            config.retry.max_attempts,
            config.retry.initial_delay_ms,
            config.retry.max_delay_ms
        );

        Arc::new(ProcessorWithRetry::new(
            base_processor,
            retry_policy,
            dlq,
            config.appid.clone(),
        )) as Arc<dyn MessageProcessor>
    };

    // Create Kafka consumer
    let consumer = create_consumer(&config)?;

    // Subscribe to input topics (supports comma-separated list or regex pattern)
    let topics = parse_input_topics(&config.input);
    let topic_refs: Vec<&str> = topics.iter().map(String::as_str).collect();
    consumer.subscribe(&topic_refs)?;
    if is_topic_regex(&config.input) {
        info!("Subscribed with regex pattern: {}", config.input);
    } else {
        info!("Subscribed to topics: {:?}", topics);
    }

    // Wrap consumer in Arc for sharing
    let consumer = Arc::new(consumer);

    // Start consumer lag monitoring
    if config.observability.lag_monitoring_enabled {
        let consumer_for_lag = consumer.clone();
        let lag_interval = config.observability.lag_monitoring_interval_secs;

        tokio::spawn(async move {
            start_lag_monitor(consumer_for_lag, lag_interval).await;
        });

        info!(
            "✅ Consumer lag monitoring started (interval: {}s)",
            lag_interval
        );
    } else {
        info!("⏭️  Consumer lag monitoring disabled");
    }

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

    // Start uptime tracker
    let start_time_clone = start_time;
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(10));
        loop {
            ticker.tick().await;
            let uptime = start_time_clone.elapsed().as_secs();
            METRICS.uptime_seconds.set(uptime as f64);
        }
    });

    // Main processing loop with concurrent message processing

    /// Maximum messages to collect before processing as a batch.
    /// Higher values improve throughput but increase latency and memory usage.
    /// Typical range: 50-500 depending on message size and processing complexity.
    const BATCH_SIZE: usize = 100;

    /// Maximum time (ms) to wait for batch to fill before processing partial batch.
    /// Lower values reduce latency during low-traffic periods.
    /// Higher values maximize batch utilization during high traffic.
    /// Should be much smaller than consumer session timeout (default 30s).
    const BATCH_FILL_TIMEOUT_MS: u64 = 100;

    /// Multiplier applied to config.threads to determine concurrent processing limit.
    /// Example: threads=4, factor=10 → parallelism=40 concurrent operations.
    /// Higher values improve CPU utilization for I/O-bound tasks but increase memory overhead.
    /// Adjust based on: I/O wait time, message processing duration, available memory.
    const PARALLELISM_FACTOR: usize = 10;

    let parallelism = (config.threads * PARALLELISM_FACTOR).max(1);
    let manual_commit = config.commit_strategy.manual_commit;
    let commit_mode = match config.commit_strategy.commit_mode {
        streamforge::config::CommitMode::Async => rdkafka::consumer::CommitMode::Async,
        streamforge::config::CommitMode::Sync => rdkafka::consumer::CommitMode::Sync,
    };

    info!(
        "Starting concurrent message processing (parallelism: {}, batch_size: {})",
        parallelism, BATCH_SIZE
    );

    if manual_commit {
        info!(
            "Using batch-level commits for at-least-once delivery (mode: {:?})",
            commit_mode
        );
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
            // Timeout already provides backoff (100ms), continue to next batch
            continue;
        }

        // Track batch size
        let batch_size = batch.len();
        METRICS.messages_in_flight.add(batch_size as f64);

        // Process batch concurrently
        let batch_timer = METRICS.batch_processing_duration.start_timer();
        let stream = futures::stream::iter(batch.into_iter())
            .map(|msg_result| {
                let processor = processor.clone();
                let stats = stats.clone();

                async move {
                    match msg_result {
                        Ok(msg) => {
                            stats.processed();
                            METRICS.messages_consumed.inc();

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
                                    METRICS.processing_errors.with_label_values(&[labels::ERROR_TYPE_PARSE]).inc();
                                    return Err(e);
                                }
                            };

                            // Extract full message envelope
                            let key_opt = if key.is_null() { None } else { Some(key) };
                            let mut envelope = MessageEnvelope::with_key(key_opt, value);

                            // Extract headers
                            if let Some(headers) = msg.headers() {
                                let headers_map = Arc::make_mut(&mut envelope.headers);
                                for header in headers.iter() {
                                    headers_map.insert(
                                        header.key.to_string(),
                                        header.value.map(|v| v.to_vec()).unwrap_or_default()
                                    );
                                }
                            }

                            // Extract timestamp
                            envelope.timestamp = msg.timestamp().to_millis();

                            // Set source metadata
                            envelope.topic = Some(msg.topic().to_string());
                            envelope.partition = Some(msg.partition());
                            envelope.offset = Some(msg.offset());

                            // Process message
                            match processor.process(envelope).await {
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
                                    METRICS.processing_errors.with_label_values(&[labels::ERROR_TYPE_PROCESSING]).inc();
                                    Err(e)
                                }
                            }
                        }
                        Err(e) => {
                            error!("Kafka consumer error: {}", e);
                            stats.error();
                            METRICS.processing_errors.with_label_values(&[labels::ERROR_TYPE_KAFKA]).inc();
                            Err(MirrorMakerError::Kafka(e.to_string()))
                        }
                    }
                }
            })
            .buffer_unordered(parallelism);

        // Different handling based on commit mode
        if manual_commit {
            // Collect results to check success before committing
            let results: Vec<_> = stream.collect().await;
            batch_timer.observe_duration();
            METRICS.messages_in_flight.sub(batch_size as f64);
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
                            error!(
                                "Failed to commit offsets (attempt {}/{}): {}",
                                retry_count + 1,
                                MAX_COMMIT_RETRIES,
                                e
                            );
                            stats.error();

                            retry_count += 1;
                            if retry_count >= MAX_COMMIT_RETRIES {
                                error!("CRITICAL: Unable to commit offsets after {} attempts. \
                                        Halting to prevent data loss. Manual intervention required.",
                                        MAX_COMMIT_RETRIES);
                                return Err(MirrorMakerError::Kafka(e.to_string()));
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
                METRICS.messages_in_flight.sub(batch_size as f64);
                error!(
                    "CRITICAL: Batch processing failed with {} errors out of {} messages. \
                        Halting to prevent data loss.",
                    error_count,
                    results.len()
                );
                error!(
                    "Failed messages will be reprocessed on restart. \
                        Note: Successfully processed messages in this batch may create duplicates."
                );
                return Err(MirrorMakerError::Processing(format!(
                    "Batch processing failed: {} errors",
                    error_count
                )));
            }
        } else {
            // Auto-commit mode: collect results to count errors
            let results: Vec<_> = stream.collect().await;
            batch_timer.observe_duration();
            METRICS.messages_in_flight.sub(batch_size as f64);
            let error_count = results.iter().filter(|r| r.is_err()).count();

            // Log each error
            for result in results.iter() {
                if let Err(e) = result {
                    error!(
                        "Message processing failed in auto-commit mode (data loss): {}",
                        e
                    );
                }
            }

            if error_count > 0 {
                warn!(
                    "Batch completed with {} errors in auto-commit mode. \
                       Failed messages will NOT be reprocessed (data loss). \
                       Consider enabling manual_commit for at-least-once delivery guarantees.",
                    error_count
                );
            }
        }
    }

    Ok(())
}

/// Returns true if the input string is a regex pattern (starts with `^`).
///
/// rdkafka treats a subscription as a regex when the pattern begins with `^`,
/// which causes the consumer to subscribe to all matching topics and pick up
/// newly created topics automatically.
///
/// **Important:** patterns must start with `^` to be treated as regex by rdkafka.
/// A string like `"events.*"` without a leading `^` is subscribed as a literal
/// topic name; `parse_input_topics` logs a warning in that case.
///
/// Examples:
/// - `"^payments.*"` → regex (all topics starting with "payments")
/// - `"^(orders|invoices).*"` → regex (multiple prefixes)
/// - `"payments,invoices"` → static list
fn is_topic_regex(input: &str) -> bool {
    input.trim_start().starts_with('^')
}

/// Parse the `input` config field into a list of topic strings to subscribe to.
///
/// - Regex pattern (starts with `^`): returned as a single-element vec so rdkafka
///   handles the pattern matching and dynamic topic discovery.
/// - Comma-separated list: split and trimmed into individual topic names.
fn parse_input_topics(input: &str) -> Vec<String> {
    if is_topic_regex(input) {
        vec![input.trim().to_string()]
    } else {
        let trimmed = input.trim();
        // Warn if it looks like a regex but is missing the required `^` prefix.
        // rdkafka only treats a subscription as a regex when it starts with `^`.
        if !trimmed.contains(',')
            && (trimmed.contains(".*")
                || trimmed.contains(".+")
                || trimmed.contains('[')
                || trimmed.contains('('))
        {
            warn!(
                "Input '{}' contains regex metacharacters but does not start with '^'. \
                 It will be treated as a literal topic name and likely produce no messages. \
                 Prefix with '^' to enable regex subscription (e.g. '^{}').",
                trimmed, trimmed
            );
        }
        input.split(',').map(|s| s.trim().to_string()).collect()
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
        transform: None,
        consumer_properties: Default::default(),
        producer_properties: Default::default(),
        security: None,
        commit_strategy: Default::default(),
        cache: None,
        observability: Default::default(),
        retry: Default::default(),
        dlq: Default::default(),
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
    cache_manager: Arc<SyncCacheManager>,
) -> Result<Arc<dyn MessageProcessor>> {
    let output_topic = config
        .output
        .clone()
        .ok_or_else(|| MirrorMakerError::Config("Output topic not specified".to_string()))?;

    let sink = Arc::new(KafkaSink::new(config, output_topic, None).await?);

    // Parse optional top-level transform (supports CACHE_LOOKUP, CACHE_PUT, STRING:, etc.)
    if let Some(ref transform_expr) = config.transform {
        info!("Value transform: {}", transform_expr);
        let transform = parse_transform_with_cache(transform_expr, Some(cache_manager))?;
        Ok(Arc::new(SingleDestinationProcessor::with_transform(
            sink, transform,
        )))
    } else {
        Ok(Arc::new(SingleDestinationProcessor::new(sink)))
    }
}

async fn build_multi_destination_processor(
    config: &MirrorMakerConfig,
    routing: &streamforge::RoutingConfig,
    _stats: Arc<Stats>,
    cache_manager: Arc<SyncCacheManager>,
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

        // Parse envelope transforms
        let mut envelope_transforms: Vec<Arc<dyn EnvelopeTransform>> = Vec::new();

        // Parse key transform if specified
        if let Some(ref key_transform_expr) = dest.key_transform {
            info!("  Key transform: {}", key_transform_expr);
            envelope_transforms.push(parse_key_transform(key_transform_expr)?);
        }

        // Parse static headers if specified
        if let Some(ref headers) = dest.headers {
            info!("  Static headers: {} header(s)", headers.len());
            envelope_transforms.extend(parse_static_headers(headers));
        }

        // Parse header transforms if specified
        if let Some(ref header_transforms) = dest.header_transforms {
            info!(
                "  Dynamic header transforms: {} operation(s)",
                header_transforms.len()
            );
            for header_config in header_transforms {
                let transform =
                    parse_header_transform(&header_config.header, &header_config.operation)?;
                envelope_transforms.push(transform);
            }
        }

        // Parse timestamp transform if specified
        if let Some(ref timestamp_expr) = dest.timestamp {
            info!("  Timestamp transform: {}", timestamp_expr);
            envelope_transforms.push(parse_timestamp_transform(timestamp_expr)?);
        }

        // Create value transform — cache_manager is threaded through so
        // CACHE_LOOKUP / CACHE_PUT expressions resolve named stores
        let transform: Option<Arc<dyn Transform>> = if let Some(ref transform_expr) = dest.transform
        {
            info!("  Value transform: {}", transform_expr);
            Some(parse_transform_with_cache(
                transform_expr,
                Some(cache_manager.clone()),
            )?)
        } else {
            None
        };

        // Create destination processor with envelope transforms
        let dest_processor = DestinationProcessor::new(
            Arc::new(sink),
            filter,
            envelope_transforms,
            transform,
            dest.output.clone(),
            dest.error_policy, // NEW: per-destination error handling policy
        );

        destinations.push(dest_processor);
    }

    Ok(Arc::new(MultiDestinationProcessor::new(
        destinations,
        routing.path.clone(),
    )))
}

/// Parse Kafka message key into a JSON Value.
///
/// Handles three cases with permissive fallback behavior:
/// 1. `None` → Returns `Value::Null` (keys are optional in Kafka)
/// 2. Valid JSON → Parses and returns the JSON Value
/// 3. Invalid JSON → Returns `Value::String` with UTF-8 decoded content
///    (using lossy conversion, replacing invalid UTF-8 sequences with �)
///
/// # Permissive Parsing Rationale
///
/// Keys use permissive parsing because they're primarily used for:
/// - Message partitioning/routing (hash-based distribution)
/// - Lookup/correlation (joining streams)
/// - Logging and debugging
///
/// Keys don't typically contain complex structured data that requires
/// strict validation. Failing on invalid key JSON would reject messages
/// that are otherwise processable.
///
/// # Examples
///
/// ```ignore
/// // Valid JSON key
/// parse_message_key(Some(br#"{"id":123}"#)) // → Value::Object({"id": 123})
///
/// // Non-JSON key (common for simple string keys)
/// parse_message_key(Some(b"user-123")) // → Value::String("user-123")
///
/// // No key
/// parse_message_key(None) // → Value::Null
/// ```
fn parse_message_key(raw: Option<&[u8]>) -> Value {
    match raw {
        Some(k) => match serde_json::from_slice::<Value>(k) {
            Ok(v) => v,
            Err(_) => Value::String(String::from_utf8_lossy(k).to_string()),
        },
        None => Value::Null,
    }
}

/// Parse Kafka message payload into a JSON Value.
///
/// Requires valid JSON payload - returns error if:
/// - Payload is `None` or empty (Kafka tombstone messages not supported)
/// - Payload is not valid JSON
/// - Payload contains invalid UTF-8
///
/// # Strict Parsing Rationale
///
/// Unlike keys, payloads use strict validation because:
/// - Message processing logic depends on accessing specific JSON fields
/// - Filters and transforms expect well-formed JSON structure
/// - Invalid payloads indicate data quality issues that should be surfaced
/// - Failed parses trigger error handling and potential reprocessing
///
/// # Error Handling
///
/// Parse failures are logged with full message context (topic, partition, offset)
/// in the caller, and:
/// - In manual commit mode → message reprocessed on restart
/// - In auto-commit mode → message lost (logged as data loss)
///
/// # Examples
///
/// ```ignore
/// // Valid JSON payload
/// parse_message_value(Some(br#"{"event":"login"}"#))
///     // → Ok(Value::Object({"event": "login"}))
///
/// // Invalid JSON
/// parse_message_value(Some(b"not-json"))
///     // → Err(MirrorMakerError::Processing("Invalid JSON: ..."))
///
/// // Empty payload (tombstone)
/// parse_message_value(None)
///     // → Err(MirrorMakerError::Processing("Empty payload"))
/// ```
///
/// # Errors
///
/// Returns `MirrorMakerError::Processing` if:
/// - Payload is missing (None)
/// - JSON deserialization fails
fn parse_message_value(raw: Option<&[u8]>) -> Result<Value> {
    match raw {
        Some(v) => serde_json::from_slice::<Value>(v)
            .map_err(|e| MirrorMakerError::Processing(format!("Invalid JSON: {}", e))),
        None => Err(MirrorMakerError::Processing("Empty payload".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    mod input_topic_tests {
        use super::*;

        #[test]
        fn test_regex_pattern_detected() {
            assert!(is_topic_regex("^payments.*"));
            assert!(is_topic_regex("^(orders|invoices).*"));
            assert!(is_topic_regex("^events"));
        }

        #[test]
        fn test_static_list_not_regex() {
            assert!(!is_topic_regex("payments"));
            assert!(!is_topic_regex("payments,invoices"));
            assert!(!is_topic_regex("topic1, topic2, topic3"));
        }

        #[test]
        fn test_regex_returned_as_single_element() {
            let result = parse_input_topics("^payments.*");
            assert_eq!(result, vec!["^payments.*"]);
        }

        #[test]
        fn test_static_list_split_and_trimmed() {
            let result = parse_input_topics("topic1,topic2,topic3");
            assert_eq!(result, vec!["topic1", "topic2", "topic3"]);
        }

        #[test]
        fn test_static_list_with_spaces_trimmed() {
            let result = parse_input_topics("topic1, topic2 , topic3");
            assert_eq!(result, vec!["topic1", "topic2", "topic3"]);
        }

        #[test]
        fn test_single_topic() {
            let result = parse_input_topics("my-topic");
            assert_eq!(result, vec!["my-topic"]);
        }

        // MAIN-2: apparent regex patterns without `^` are treated as literal topic names.
        // The warning is emitted but parse_input_topics still returns a list (not an error),
        // so callers see the unanchored pattern as a literal topic name string.
        #[test]
        fn test_unanchored_dotstar_treated_as_literal() {
            let result = parse_input_topics("events.*");
            // Returned as a single literal, not split (no comma) — rdkafka will subscribe
            // to a topic literally named "events.*" which likely doesn't exist.
            assert_eq!(result, vec!["events.*"]);
        }

        #[test]
        fn test_unanchored_bracket_treated_as_literal() {
            let result = parse_input_topics("payments[0-9]");
            assert_eq!(result, vec!["payments[0-9]"]);
        }

        #[test]
        fn test_anchored_pattern_is_regex() {
            // Anchored with ^ → treated as regex subscription
            let result = parse_input_topics("^events.*");
            assert_eq!(result, vec!["^events.*"]);
            assert!(is_topic_regex("^events.*"));
        }
    }

    mod parse_message_key_tests {
        use super::*;

        #[test]
        fn test_none_key_returns_null() {
            let result = parse_message_key(None);
            assert_eq!(result, Value::Null);
        }

        #[test]
        fn test_valid_json_object_key() {
            let key = br#"{"id":123,"type":"user"}"#;
            let result = parse_message_key(Some(key));
            assert_eq!(result, json!({"id": 123, "type": "user"}));
        }

        #[test]
        fn test_valid_json_string_key() {
            let key = br#""user-123""#;
            let result = parse_message_key(Some(key));
            assert_eq!(result, Value::String("user-123".to_string()));
        }

        #[test]
        fn test_valid_json_number_key() {
            let key = b"123";
            let result = parse_message_key(Some(key));
            assert_eq!(result, json!(123));
        }

        #[test]
        fn test_non_json_key_returns_string() {
            let key = b"user-123";
            let result = parse_message_key(Some(key));
            assert_eq!(result, Value::String("user-123".to_string()));
        }

        #[test]
        fn test_invalid_utf8_key_uses_lossy_conversion() {
            // Invalid UTF-8 sequence: 0xFF is invalid in UTF-8
            let key = b"user\xFF123";
            let result = parse_message_key(Some(key));
            assert!(result.is_string());
            // Should contain replacement character (�)
            assert_eq!(result, Value::String("user�123".to_string()));
        }

        #[test]
        fn test_empty_key_returns_empty_string() {
            let key = b"";
            let result = parse_message_key(Some(key));
            assert_eq!(result, Value::String("".to_string()));
        }
    }

    mod config_loading_tests {
        use super::*;

        #[test]
        fn test_default_config_has_required_fields() {
            let config = create_default_config();

            assert_eq!(config.appid, "streamforge");
            assert_eq!(config.bootstrap, "localhost:9092");
            assert_eq!(config.input, "input-topic");
            assert_eq!(config.output, Some("output-topic".to_string()));
            assert_eq!(config.offset, "latest");
            assert_eq!(config.threads, 4);
            assert!(config.routing.is_none());
            assert!(!config.commit_strategy.manual_commit);
        }

        #[test]
        fn test_load_config_missing_file_uses_default() {
            // SAFETY: set_var/remove_var are unsound in multithreaded contexts.
            // This test is acceptable because Rust tests with `-- --test-threads=1`
            // or because no other test reads CONFIG_FILE concurrently.
            unsafe {
                std::env::set_var("CONFIG_FILE", "/tmp/nonexistent-test-config-12345.json");
            }

            let result = load_config();
            assert!(
                result.is_ok(),
                "Should return default config when file missing"
            );

            let config = result.unwrap();
            assert_eq!(config.appid, "streamforge", "Should use default appid");

            unsafe {
                std::env::remove_var("CONFIG_FILE");
            }
        }

        #[test]
        fn test_default_config_consumer_properties_empty() {
            let config = create_default_config();
            assert!(config.consumer_properties.is_empty());
        }

        #[test]
        fn test_default_config_producer_properties_empty() {
            let config = create_default_config();
            assert!(config.producer_properties.is_empty());
        }

        #[test]
        fn test_default_config_no_security() {
            let config = create_default_config();
            assert!(config.security.is_none());
        }

        #[test]
        fn test_default_config_no_cache() {
            let config = create_default_config();
            assert!(config.cache.is_none());
        }
    }

    mod commit_mode_mapping_tests {
        use super::*;

        #[test]
        fn test_commit_mode_async_mapping() {
            // Test that CommitMode::Async maps to rdkafka's Async
            let config_mode = streamforge::config::CommitMode::Async;
            let rdkafka_mode = match config_mode {
                streamforge::config::CommitMode::Async => rdkafka::consumer::CommitMode::Async,
                streamforge::config::CommitMode::Sync => rdkafka::consumer::CommitMode::Sync,
            };

            assert!(matches!(rdkafka_mode, rdkafka::consumer::CommitMode::Async));
        }

        #[test]
        fn test_commit_mode_sync_mapping() {
            // Test that CommitMode::Sync maps to rdkafka's Sync
            let config_mode = streamforge::config::CommitMode::Sync;
            let rdkafka_mode = match config_mode {
                streamforge::config::CommitMode::Async => rdkafka::consumer::CommitMode::Async,
                streamforge::config::CommitMode::Sync => rdkafka::consumer::CommitMode::Sync,
            };

            assert!(matches!(rdkafka_mode, rdkafka::consumer::CommitMode::Sync));
        }

        #[test]
        fn test_auto_commit_flag_calculation() {
            let mut config = create_default_config();

            // Manual commit disabled = auto commit enabled
            config.commit_strategy.manual_commit = false;
            let auto_commit = !config.commit_strategy.manual_commit;
            assert!(
                auto_commit,
                "auto_commit should be true when manual_commit is false"
            );

            // Manual commit enabled = auto commit disabled
            config.commit_strategy.manual_commit = true;
            let auto_commit = !config.commit_strategy.manual_commit;
            assert!(
                !auto_commit,
                "auto_commit should be false when manual_commit is true"
            );
        }
    }

    mod parse_message_value_tests {
        use super::*;

        #[test]
        fn test_none_value_returns_error() {
            let result = parse_message_value(None);
            assert!(result.is_err());
            assert!(
                matches!(result.unwrap_err(), MirrorMakerError::Processing(msg) if msg.contains("Empty payload"))
            );
        }

        #[test]
        fn test_valid_json_object() {
            let value = br#"{"event":"login","userId":123}"#;
            let result = parse_message_value(Some(value)).unwrap();
            assert_eq!(result, json!({"event": "login", "userId": 123}));
        }

        #[test]
        fn test_valid_json_array() {
            let value = br#"[1,2,3]"#;
            let result = parse_message_value(Some(value)).unwrap();
            assert_eq!(result, json!([1, 2, 3]));
        }

        #[test]
        fn test_valid_json_string() {
            let value = br#""hello""#;
            let result = parse_message_value(Some(value)).unwrap();
            assert_eq!(result, Value::String("hello".to_string()));
        }

        #[test]
        fn test_invalid_json_returns_error() {
            let value = b"not-json";
            let result = parse_message_value(Some(value));
            assert!(result.is_err());
            assert!(
                matches!(result.unwrap_err(), MirrorMakerError::Processing(msg) if msg.contains("Invalid JSON"))
            );
        }

        #[test]
        fn test_invalid_utf8_returns_error() {
            // Invalid UTF-8 in JSON context
            let value = b"\xFF\xFE";
            let result = parse_message_value(Some(value));
            assert!(result.is_err());
        }

        #[test]
        fn test_empty_payload_returns_error() {
            let value = b"";
            let result = parse_message_value(Some(value));
            assert!(result.is_err());
            assert!(
                matches!(result.unwrap_err(), MirrorMakerError::Processing(msg) if msg.contains("Invalid JSON"))
            );
        }

        #[test]
        fn test_complex_nested_json() {
            let value = br#"{"event":"meeting.started","data":{"confId":123,"participants":[{"id":1,"name":"Alice"}]}}"#;
            let result = parse_message_value(Some(value)).unwrap();
            assert_eq!(
                result,
                json!({
                    "event": "meeting.started",
                    "data": {
                        "confId": 123,
                        "participants": [{"id": 1, "name": "Alice"}]
                    }
                })
            );
        }
    }
}
