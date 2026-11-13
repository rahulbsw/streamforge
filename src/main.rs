use futures::stream::StreamExt;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{Headers, Message};
#[cfg(test)]
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
    init_tracing_from_env, labels, register_metrics, start_kafka_readiness_monitor,
    start_lag_monitor, start_observability_server_on, KafkaReadinessFailure, ReadinessState,
    METRICS,
};
use streamforge::partition_pipeline::{
    parse_message_key, parse_message_value, PartitionOrderedExecutor, ProcessingCompletion,
};
use streamforge::processor::{
    DestinationProcessor, MessageProcessor, MultiDestinationProcessor, SingleDestinationProcessor,
};
use streamforge::processor_with_retry::ProcessorWithRetry;
use streamforge::{
    compose_filter, compose_value_transform, DeadLetterQueue, MessageEnvelope, MirrorMakerConfig,
    MirrorMakerError, Result, RetryPolicy, SyncCacheManager, WasmEnvelopeTransform, WasmFilter,
    WasmRegistry, WasmValueTransform,
};
use tokio::time::interval;
use tracing::{error, info, warn};

const CLI_HELP: &str = "\
StreamForge Kafka selective replication engine

Usage: streamforge

Configuration:
  CONFIG_FILE  YAML or JSON engine configuration (default: config.json)

Options:
  -h, --help     Print help
  -V, --version  Print version
";

fn metadata_argument(args: impl IntoIterator<Item = String>) -> Option<&'static str> {
    match args.into_iter().next().as_deref() {
        Some("-h" | "--help") => Some(CLI_HELP),
        Some("-V" | "--version") => Some(env!("CARGO_PKG_VERSION")),
        _ => None,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Some(output) = metadata_argument(std::env::args().skip(1)) {
        println!("{output}");
        return Ok(());
    }

    init_tracing_from_env().map_err(|error| {
        MirrorMakerError::Config(format!("Failed to initialize logging: {error}"))
    })?;

    info!(version = env!("CARGO_PKG_VERSION"), "starting StreamForge");

    // Load configuration (from file or environment)
    let config = load_config()?;
    info!(pipeline = %config.appid, "configuration loaded");

    let readiness = ReadinessState::new();

    // Initialize observability (metrics)
    if config.observability.metrics_enabled {
        register_metrics()
            .map_err(|e| MirrorMakerError::Config(format!("Failed to register metrics: {}", e)))?;
        info!("metrics registered");

        // Start metrics HTTP server
        let metrics_port = config.observability.metrics_port;
        let metrics_bind_address = config.observability.metrics_bind_address;
        let metrics_path = config.observability.metrics_path.clone();
        let server_readiness = readiness.clone();
        tokio::spawn(async move {
            start_observability_server_on(
                metrics_bind_address,
                metrics_port,
                metrics_path,
                server_readiness,
            )
            .await;
        });
    } else {
        info!("metrics disabled in configuration");
    }

    // Record service start time for uptime metric
    let start_time = std::time::Instant::now();

    // Create statistics
    let stats = Arc::new(Stats::new());

    // Shared sync cache manager — used by CACHE_LOOKUP / CACHE_PUT transforms
    let cache_manager = Arc::new(SyncCacheManager::new());

    // Verify and compile all digest-pinned components before constructing any
    // Kafka client. Native-only configurations do not initialize Wasmtime.
    let wasm_registry = config
        .wasm
        .as_ref()
        .map(WasmRegistry::load)
        .transpose()
        .map_err(|error| {
            MirrorMakerError::Config(format!(
                "WebAssembly UDF startup validation failed: {error}"
            ))
        })?
        .map(Arc::new);
    if let Some(registry) = &wasm_registry {
        info!(
            module_count = registry.module_names().count(),
            "WebAssembly UDF registry verified and compiled"
        );
    }

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
        info!(pipeline = %config.appid, "multi-destination routing enabled");
        build_multi_destination_processor(
            &config,
            routing,
            stats.clone(),
            cache_manager.clone(),
            wasm_registry.clone(),
        )
        .await?
    } else {
        info!(pipeline = %config.appid, "single-destination mode enabled");
        build_single_destination_processor(
            &config,
            stats.clone(),
            cache_manager.clone(),
            wasm_registry.clone(),
        )
        .await?
    };

    let retry_policy = RetryPolicy::new(config.retry.clone());
    let dlq = if config.dlq.enabled {
        info!(
            dlq_topic = %config.dlq.topic,
            max_retries = config.dlq.max_dlq_retries,
            "DLQ enabled"
        );
        Some(Arc::new(DeadLetterQueue::new(
            config.dlq.clone(),
            &config.bootstrap,
        )?))
    } else {
        info!("DLQ disabled - terminal failures will halt the pipeline");
        None
    };
    METRICS
        .kafka_connections
        .with_label_values(&[labels::CONNECTION_TYPE_PRODUCER])
        .set(1.0);
    readiness.mark_runtime_ready();

    info!(
        "Retry policy: max_attempts={}, initial_delay={}ms, max_delay={}ms",
        config.retry.max_attempts, config.retry.initial_delay_ms, config.retry.max_delay_ms
    );

    // Preserve the zero-wrapper compatibility path only when neither retries
    // nor DLQ delivery can occur. A one-attempt policy still needs the wrapper
    // when DLQ is enabled so terminal destination failures are delivered once.
    let processor: Arc<dyn MessageProcessor> =
        if config.retry.max_attempts == 1 && !config.dlq.enabled {
            info!("Retry and DLQ disabled - using base processor directly");
            base_processor
        } else {
            Arc::new(ProcessorWithRetry::new(
                base_processor,
                retry_policy,
                dlq,
                config.appid.clone(),
            ))
        };

    if let Some(interval_seconds) = aggregation_flush_interval_seconds(&config) {
        info!(
            "Starting aggregation completion-check ticker (poll interval: {}s)",
            interval_seconds
        );

        let flush_processor = processor.clone();
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_seconds));
            ticker.tick().await;

            loop {
                ticker.tick().await;

                if let Err(err) = flush_processor.flush().await {
                    error!("Aggregation flush failed: {}", err);
                }
            }
        });
    }

    // Create Kafka consumer
    let consumer = create_consumer(&config)?;

    // Subscribe to input topics (supports comma-separated list or regex pattern)
    let topics = parse_input_topics(&config.input);
    let topic_refs: Vec<&str> = topics.iter().map(String::as_str).collect();
    consumer.subscribe(&topic_refs)?;
    if is_topic_regex(&config.input) {
        info!(
            pipeline = %config.appid,
            subscription_type = "regex",
            topic_count = topic_refs.len(),
            "Kafka subscription configured"
        );
    } else {
        info!(
            pipeline = %config.appid,
            subscription_type = "topics",
            topic_count = topic_refs.len(),
            "Kafka subscription configured"
        );
    }

    // Wrap consumer in Arc for sharing
    let consumer = Arc::new(consumer);

    // Readiness is based on an actual Kafka metadata request, not merely local
    // consumer construction or subscription.
    {
        let readiness_consumer = consumer.clone();
        let readiness_state = readiness.clone();
        tokio::spawn(async move {
            start_kafka_readiness_monitor(readiness_consumer, readiness_state, 10).await;
        });
    }

    // Start consumer lag monitoring
    if config.observability.lag_monitoring_enabled {
        let consumer_for_lag = consumer.clone();
        let lag_interval = config.observability.lag_monitoring_interval_secs;

        tokio::spawn(async move {
            start_lag_monitor(consumer_for_lag, lag_interval).await;
        });

        info!(
            interval_seconds = lag_interval,
            "consumer lag monitoring started"
        );
    } else {
        info!("consumer lag monitoring disabled");
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

    if config.performance.processing_mode == streamforge::ProcessingMode::PartitionOrdered {
        info!(
            "Starting partition-ordered processing (workers: {}, queue_capacity_per_worker: {})",
            config.threads, config.performance.worker_queue_capacity
        );
        run_partition_ordered_pipeline(
            consumer,
            processor.clone(),
            stats,
            readiness,
            config.threads,
            config.performance.worker_queue_capacity,
            config.performance.consumer_batch_timeout_ms,
        )
        .await?;
        return Ok(());
    }

    // Compatibility processing loop with configurable batched concurrency.
    let (batch_size_limit, batch_fill_timeout_ms, parallelism) =
        processing_runtime_settings(&config);
    let manual_commit = config.commit_strategy.manual_commit;
    let commit_mode = match config.commit_strategy.commit_mode {
        streamforge::config::CommitMode::Async => rdkafka::consumer::CommitMode::Async,
        streamforge::config::CommitMode::Sync => rdkafka::consumer::CommitMode::Sync,
    };

    info!(
        "Starting concurrent message processing (parallelism: {}, batch_size: {}, batch_fill_timeout_ms: {})",
        parallelism, batch_size_limit, batch_fill_timeout_ms
    );

    if manual_commit {
        info!(
            "Using batch-level commits for at-least-once delivery (mode: {:?})",
            commit_mode
        );
    }

    let mut message_stream = consumer.stream();
    let pipeline_name = config.appid.clone();

    loop {
        // Collect batch of messages with single deadline
        let mut batch = Vec::with_capacity(batch_size_limit);
        let deadline = tokio::time::Instant::now() + Duration::from_millis(batch_fill_timeout_ms);
        let mut stream_ended = false;

        for _ in 0..batch_size_limit {
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
                readiness.mark_kafka_unready(KafkaReadinessFailure::StreamEnded);
                info!("Consumer stream ended, shutting down");
                break;
            }
            // The configured timeout already provides backoff; continue to the next batch.
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
                let readiness = readiness.clone();
                let pipeline = pipeline_name.clone();

                async move {
                    match msg_result {
                        Ok(msg) => {
                            readiness.mark_kafka_ready();
                            stats.processed();
                            METRICS.messages_consumed.inc();

                            let key = parse_message_key(msg.key());
                            let value = match parse_message_value(msg.payload()) {
                                Ok(v) => v,
                                Err(e) => {
                                    error!(
                                        pipeline = %pipeline,
                                        error_category = "parse",
                                        topic = msg.topic(),
                                        partition = msg.partition(),
                                        offset = msg.offset(),
                                        error = %e,
                                        "message parsing failed"
                                    );
                                    stats.error();
                                    METRICS
                                        .processing_errors
                                        .with_label_values(&[labels::ERROR_TYPE_PARSE])
                                        .inc();
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
                                        header.value.map(|v| v.to_vec()).unwrap_or_default(),
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
                                        pipeline = %pipeline,
                                        error_category = "processing",
                                        topic = msg.topic(),
                                        partition = msg.partition(),
                                        offset = msg.offset(),
                                        error = %e,
                                        "message processing failed"
                                    );
                                    stats.error();
                                    METRICS
                                        .processing_errors
                                        .with_label_values(&[labels::ERROR_TYPE_PROCESSING])
                                        .inc();
                                    Err(e)
                                }
                            }
                        }
                        Err(e) => {
                            readiness.mark_kafka_unready(KafkaReadinessFailure::ConsumerError);
                            error!(
                                pipeline = %pipeline,
                                error_category = "kafka_consumer",
                                error = %e,
                                "Kafka consumer error"
                            );
                            stats.error();
                            METRICS
                                .processing_errors
                                .with_label_values(&[labels::ERROR_TYPE_KAFKA])
                                .inc();
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

    let mut config = if std::path::Path::new(&config_path).exists() {
        info!("Loading configuration from: {}", config_path);
        MirrorMakerConfig::from_file(&config_path)?
    } else {
        // Create default config for testing
        warn!("Config file not found, using default configuration");
        create_default_config()
    };

    config.validate()?;
    config.apply_performance_property_defaults();
    Ok(config)
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
        performance: Default::default(),
        compression: Default::default(),
        routing: None,
        transform: None,
        consumer_properties: Default::default(),
        producer_properties: Default::default(),
        security: None,
        target_security: None,
        commit_strategy: Default::default(),
        cache: None,
        observability: Default::default(),
        retry: Default::default(),
        dlq: Default::default(),
        wasm: None,
        udfs: None,
    }
}

fn processing_runtime_settings(config: &MirrorMakerConfig) -> (usize, u64, usize) {
    let performance = &config.performance;
    let parallelism = config
        .threads
        .saturating_mul(performance.parallelism_factor)
        .max(1);

    (
        performance.consumer_batch_size,
        performance.consumer_batch_timeout_ms,
        parallelism,
    )
}

async fn run_partition_ordered_pipeline(
    consumer: Arc<StreamConsumer>,
    processor: Arc<dyn MessageProcessor>,
    stats: Arc<Stats>,
    readiness: ReadinessState,
    worker_count: usize,
    worker_queue_capacity: usize,
    idle_flush_timeout_ms: u64,
) -> Result<()> {
    let mut executor = PartitionOrderedExecutor::new(
        processor.clone(),
        stats.clone(),
        worker_count,
        worker_queue_capacity,
    );
    let mut message_stream = consumer.stream();
    let idle_flush = tokio::time::sleep(Duration::from_millis(idle_flush_timeout_ms));
    tokio::pin!(idle_flush);
    let mut flush_pending = false;

    loop {
        tokio::select! {
            completion = executor.next_completion() => {
                if let Some(completion) = completion {
                    report_auto_commit_completion(completion);
                    flush_pending = true;
                    idle_flush
                        .as_mut()
                        .reset(tokio::time::Instant::now() + Duration::from_millis(idle_flush_timeout_ms));
                }
            }
            _ = &mut idle_flush, if flush_pending => {
                processor.flush().await?;
                flush_pending = false;
            }
            message = message_stream.next() => {
                match message {
                    Some(Ok(message)) => {
                        readiness.mark_kafka_ready();
                        executor.dispatch(message.detach()).await?;
                    }
                    Some(Err(kafka_error)) => {
                        readiness.mark_kafka_unready(KafkaReadinessFailure::ConsumerError);
                        error!(
                            error_category = "kafka_consumer",
                            error = %kafka_error,
                            "Kafka consumer error"
                        );
                        stats.error();
                        METRICS
                            .processing_errors
                            .with_label_values(&[labels::ERROR_TYPE_KAFKA])
                            .inc();
                    }
                    None => {
                        readiness.mark_kafka_unready(KafkaReadinessFailure::StreamEnded);
                        info!("Consumer stream ended, draining partition workers");
                        break;
                    }
                }
            }
        }
    }

    for completion in executor.shutdown().await? {
        report_auto_commit_completion(completion);
    }
    processor.flush().await?;
    Ok(())
}

fn report_auto_commit_completion(completion: ProcessingCompletion) {
    if let Err(processing_error) = completion.result {
        error!(
            "Message processing failed in auto-commit mode (data loss): {} \
             (topic={}, partition={}, offset={})",
            processing_error,
            completion.position.topic,
            completion.position.partition,
            completion.position.offset
        );
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
    if !auto_commit {
        info!("Manual commit enabled - at-least-once semantics");
        info!("Commit mode: {:?}", config.commit_strategy.commit_mode);
    } else {
        warn!("Auto-commit enabled - at-most-once semantics (messages may be lost on failure)");
    }

    // Apply security configuration
    config.try_apply_source_security(&mut consumer_config)?;

    // Apply user-provided consumer properties (can override security settings if needed)
    for (key, value) in &config.consumer_properties {
        consumer_config.set(key, value);
    }

    // Commit strategy is a reliability contract, not a performance override.
    // Re-apply these after arbitrary client properties so user-provided maps
    // cannot silently contradict the validated StreamForge mode.
    consumer_config
        .set("enable.auto.commit", auto_commit.to_string())
        .set("enable.auto.offset.store", "true");

    let consumer: StreamConsumer = consumer_config.create()?;
    Ok(consumer)
}

async fn build_single_destination_processor(
    config: &MirrorMakerConfig,
    _stats: Arc<Stats>,
    cache_manager: Arc<SyncCacheManager>,
    wasm_registry: Option<Arc<WasmRegistry>>,
) -> Result<Arc<dyn MessageProcessor>> {
    let output_topic = config
        .output
        .clone()
        .ok_or_else(|| MirrorMakerError::Config("Output topic not specified".to_string()))?;

    let sink = Arc::new(KafkaSink::new(config, output_topic.clone(), None).await?);

    // Preserve the native-only construction path exactly when no UDF is bound.
    if config.udfs.is_none() {
        if let Some(ref transform_expr) = config.transform {
            info!(
                destination = %output_topic,
                transform_type = "value",
                "destination transform configured"
            );
            let transform = parse_transform_with_cache(transform_expr, Some(cache_manager))?;
            return Ok(Arc::new(
                SingleDestinationProcessor::with_transform_for_destination(
                    sink,
                    transform,
                    output_topic.as_str(),
                ),
            ));
        }
        return Ok(Arc::new(SingleDestinationProcessor::for_destination(
            sink,
            output_topic.as_str(),
        )));
    }

    let udfs = config
        .udfs
        .as_ref()
        .expect("validated UDF branch requires top-level udfs");
    let native_transform: Option<Arc<dyn Transform>> =
        if let Some(ref transform_expr) = config.transform {
            info!(
                destination = %output_topic,
                transform_type = "value",
                "destination transform configured"
            );
            Some(parse_transform_with_cache(
                transform_expr,
                Some(cache_manager),
            )?)
        } else {
            None
        };
    let filter = compose_filter(
        None,
        build_wasm_filter(&wasm_registry, udfs.filter.as_deref())?,
    );
    let transform = compose_value_transform(
        native_transform,
        build_wasm_value_transform(&wasm_registry, udfs.value_transform.as_deref())?,
    );
    let mut envelope_transforms: Vec<Arc<dyn EnvelopeTransform>> = Vec::new();
    if let Some(transform) =
        build_wasm_envelope_transform(&wasm_registry, udfs.envelope_transform.as_deref())?
    {
        envelope_transforms.push(transform);
    }

    let error_policy = if config.dlq.enabled {
        streamforge::config::ErrorPolicy::Dlq
    } else {
        streamforge::config::ErrorPolicy::Fail
    };
    let destination = DestinationProcessor::new(
        sink,
        filter,
        envelope_transforms,
        transform,
        output_topic,
        error_policy,
    );
    Ok(Arc::new(MultiDestinationProcessor::new(
        vec![destination],
        None,
    )))
}

async fn build_multi_destination_processor(
    config: &MirrorMakerConfig,
    routing: &streamforge::RoutingConfig,
    _stats: Arc<Stats>,
    cache_manager: Arc<SyncCacheManager>,
    wasm_registry: Option<Arc<WasmRegistry>>,
) -> Result<Arc<dyn MessageProcessor>> {
    let mut destinations = Vec::new();

    for dest in &routing.destinations {
        info!(destination = %dest.output, "setting up destination");
        validate_aggregation_destination(dest)?;

        // Create sink
        let sink =
            Arc::new(KafkaSink::new(config, dest.output.clone(), dest.partition.clone()).await?);

        // Create filter if specified
        let native_filter: Option<Arc<dyn Filter>> = if let Some(ref filter_expr) = dest.filter {
            info!(destination = %dest.output, "destination filter configured");
            Some(parse_filter(filter_expr)?)
        } else {
            None
        };
        let filter = compose_filter(
            native_filter,
            build_wasm_filter(
                &wasm_registry,
                dest.udfs.as_ref().and_then(|udfs| udfs.filter.as_deref()),
            )?,
        );

        // Create value transform — cache_manager is threaded through so
        // CACHE_LOOKUP / CACHE_PUT expressions resolve named stores
        let native_transform: Option<Arc<dyn Transform>> =
            if let Some(ref transform_expr) = dest.transform {
                info!(
                    destination = %dest.output,
                    transform_type = "value",
                    "destination transform configured"
                );
                Some(parse_transform_with_cache(
                    transform_expr,
                    Some(cache_manager.clone()),
                )?)
            } else {
                None
            };
        let transform = compose_value_transform(
            native_transform,
            build_wasm_value_transform(
                &wasm_registry,
                dest.udfs
                    .as_ref()
                    .and_then(|udfs| udfs.value_transform.as_deref()),
            )?,
        );

        let dest_processor = if let Some(aggregation) = dest.aggregation.clone() {
            info!(
                "  Aggregation: {} metric(s), completion check every {}s",
                aggregation.metrics.len(),
                aggregation.window.emit_interval_seconds
            );
            DestinationProcessor::with_aggregation(
                sink,
                filter,
                transform,
                aggregation,
                dest.output.clone(),
                dest.error_policy,
            )?
        } else {
            // Envelope transforms stay on the immediate-send path only.
            let mut envelope_transforms: Vec<Arc<dyn EnvelopeTransform>> = Vec::new();

            if let Some(ref key_transform_expr) = dest.key_transform {
                info!(
                    destination = %dest.output,
                    transform_type = "key",
                    "destination transform configured"
                );
                envelope_transforms.push(parse_key_transform(key_transform_expr)?);
            }

            if let Some(ref headers) = dest.headers {
                info!("  Static headers: {} header(s)", headers.len());
                envelope_transforms.extend(parse_static_headers(headers));
            }

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

            if let Some(ref timestamp_expr) = dest.timestamp {
                info!(
                    destination = %dest.output,
                    transform_type = "timestamp",
                    "destination transform configured"
                );
                envelope_transforms.push(parse_timestamp_transform(timestamp_expr)?);
            }
            if let Some(transform) = build_wasm_envelope_transform(
                &wasm_registry,
                dest.udfs
                    .as_ref()
                    .and_then(|udfs| udfs.envelope_transform.as_deref()),
            )? {
                envelope_transforms.push(transform);
            }

            DestinationProcessor::new(
                sink,
                filter,
                envelope_transforms,
                transform,
                dest.output.clone(),
                dest.error_policy,
            )
        };

        destinations.push(dest_processor);
    }

    Ok(Arc::new(MultiDestinationProcessor::new(
        destinations,
        routing.path.clone(),
    )))
}

fn build_wasm_filter(
    registry: &Option<Arc<WasmRegistry>>,
    module: Option<&str>,
) -> Result<Option<Arc<dyn Filter>>> {
    let Some(module) = module else {
        return Ok(None);
    };
    let registry = required_wasm_registry(registry, module)?;
    info!(module, "  WebAssembly filter");
    WasmFilter::new(registry, module)
        .map(|filter| Some(Arc::new(filter) as Arc<dyn Filter>))
        .map_err(wasm_binding_error)
}

fn build_wasm_value_transform(
    registry: &Option<Arc<WasmRegistry>>,
    module: Option<&str>,
) -> Result<Option<Arc<dyn Transform>>> {
    let Some(module) = module else {
        return Ok(None);
    };
    let registry = required_wasm_registry(registry, module)?;
    info!(module, "  WebAssembly value transform");
    WasmValueTransform::new(registry, module)
        .map(|transform| Some(Arc::new(transform) as Arc<dyn Transform>))
        .map_err(wasm_binding_error)
}

fn build_wasm_envelope_transform(
    registry: &Option<Arc<WasmRegistry>>,
    module: Option<&str>,
) -> Result<Option<Arc<dyn EnvelopeTransform>>> {
    let Some(module) = module else {
        return Ok(None);
    };
    let registry = required_wasm_registry(registry, module)?;
    info!(module, "  WebAssembly envelope transform");
    WasmEnvelopeTransform::new(registry, module)
        .map(|transform| Some(Arc::new(transform) as Arc<dyn EnvelopeTransform>))
        .map_err(wasm_binding_error)
}

fn required_wasm_registry(
    registry: &Option<Arc<WasmRegistry>>,
    module: &str,
) -> Result<Arc<WasmRegistry>> {
    registry.clone().ok_or_else(|| {
        MirrorMakerError::Config(format!(
            "WebAssembly module {module:?} is bound without a loaded wasm registry"
        ))
    })
}

fn wasm_binding_error(error: streamforge::WasmError) -> MirrorMakerError {
    MirrorMakerError::Config(format!("WebAssembly UDF binding failed: {error}"))
}

fn validate_aggregation_destination(dest: &streamforge::DestinationConfig) -> Result<()> {
    if dest.aggregation.is_some() && dest.output.contains("{source_topic}") {
        return Err(MirrorMakerError::Config(
            "aggregation destinations do not support output templates containing '{source_topic}'"
                .to_string(),
        ));
    }

    Ok(())
}

fn aggregation_flush_interval_seconds(config: &MirrorMakerConfig) -> Option<u64> {
    config
        .routing
        .as_ref()?
        .destinations
        .iter()
        .filter_map(|dest| {
            dest.aggregation
                .as_ref()
                .map(|aggregation| aggregation.window.emit_interval_seconds)
        })
        .min()
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
            assert_eq!(config.performance.consumer_batch_size, 100);
            assert_eq!(config.performance.consumer_batch_timeout_ms, 100);
            assert_eq!(config.performance.parallelism_factor, 10);
            assert!(config.routing.is_none());
            assert!(!config.commit_strategy.manual_commit);
        }

        #[test]
        fn test_processing_runtime_settings_use_performance_config() {
            let mut config = create_default_config();
            config.threads = 8;
            config.performance.consumer_batch_size = 2000;
            config.performance.consumer_batch_timeout_ms = 50;
            config.performance.parallelism_factor = 15;

            assert_eq!(processing_runtime_settings(&config), (2000, 50, 120));
        }

        #[test]
        fn test_processing_runtime_settings_saturate_parallelism() {
            let mut config = create_default_config();
            config.threads = usize::MAX;
            config.performance.parallelism_factor = 2;

            let (_, _, parallelism) = processing_runtime_settings(&config);
            assert_eq!(parallelism, usize::MAX);
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

    mod aggregation_flush_interval_tests {
        use super::*;

        #[test]
        fn test_returns_none_when_no_aggregation_destinations_exist() {
            let mut config = create_default_config();
            config.routing = Some(streamforge::RoutingConfig {
                routing_type: "filter".to_string(),
                path: None,
                destinations: vec![streamforge::DestinationConfig {
                    output: "immediate.topic".to_string(),
                    match_value: None,
                    filter: None,
                    transform: None,
                    error_policy: Default::default(),
                    key_transform: None,
                    headers: None,
                    header_transforms: None,
                    timestamp: None,
                    aggregation: None,
                    partition: None,
                    broadcast: false,
                    description: None,
                    udfs: None,
                }],
            });

            assert_eq!(aggregation_flush_interval_seconds(&config), None);
        }

        #[test]
        fn test_returns_minimum_emit_interval_across_aggregation_destinations() {
            let mut config = create_default_config();
            config.routing = Some(streamforge::RoutingConfig {
                routing_type: "filter".to_string(),
                path: None,
                destinations: vec![
                    streamforge::DestinationConfig {
                        output: "immediate.topic".to_string(),
                        match_value: None,
                        filter: None,
                        transform: None,
                        error_policy: Default::default(),
                        key_transform: None,
                        headers: None,
                        header_transforms: None,
                        timestamp: None,
                        aggregation: None,
                        partition: None,
                        broadcast: false,
                        description: None,
                        udfs: None,
                    },
                    streamforge::DestinationConfig {
                        output: "aggregate-slow.topic".to_string(),
                        match_value: None,
                        filter: None,
                        transform: None,
                        error_policy: Default::default(),
                        key_transform: None,
                        headers: None,
                        header_transforms: None,
                        timestamp: None,
                        aggregation: Some(streamforge::AggregationConfig {
                            group_by: vec![streamforge::AggregationGroupBy {
                                name: "tenant".to_string(),
                                path: "/tenant".to_string(),
                            }],
                            window: streamforge::AggregationWindowConfig {
                                window_type: streamforge::AggregationWindowType::Tumbling,
                                size_seconds: 60,
                                emit_interval_seconds: 15,
                            },
                            metrics: vec![streamforge::AggregationMetricConfig {
                                name: "count".to_string(),
                                op: streamforge::AggregationOp::Count,
                                path: None,
                                percentiles: None,
                            }],
                        }),
                        partition: None,
                        broadcast: false,
                        description: None,
                        udfs: None,
                    },
                    streamforge::DestinationConfig {
                        output: "aggregate-fast.topic".to_string(),
                        match_value: None,
                        filter: None,
                        transform: None,
                        error_policy: Default::default(),
                        key_transform: None,
                        headers: None,
                        header_transforms: None,
                        timestamp: None,
                        aggregation: Some(streamforge::AggregationConfig {
                            group_by: vec![streamforge::AggregationGroupBy {
                                name: "tenant".to_string(),
                                path: "/tenant".to_string(),
                            }],
                            window: streamforge::AggregationWindowConfig {
                                window_type: streamforge::AggregationWindowType::Tumbling,
                                size_seconds: 60,
                                emit_interval_seconds: 5,
                            },
                            metrics: vec![streamforge::AggregationMetricConfig {
                                name: "count".to_string(),
                                op: streamforge::AggregationOp::Count,
                                path: None,
                                percentiles: None,
                            }],
                        }),
                        partition: None,
                        broadcast: false,
                        description: None,
                        udfs: None,
                    },
                ],
            });

            assert_eq!(aggregation_flush_interval_seconds(&config), Some(5));
        }

        #[test]
        fn test_rejects_template_output_for_aggregation_destination() {
            let destination = streamforge::DestinationConfig {
                output: "aggregates.{source_topic}".to_string(),
                match_value: None,
                filter: None,
                transform: None,
                error_policy: Default::default(),
                key_transform: None,
                headers: None,
                header_transforms: None,
                timestamp: None,
                aggregation: Some(streamforge::AggregationConfig {
                    group_by: vec![streamforge::AggregationGroupBy {
                        name: "tenant".to_string(),
                        path: "/tenant".to_string(),
                    }],
                    window: streamforge::AggregationWindowConfig {
                        window_type: streamforge::AggregationWindowType::Tumbling,
                        size_seconds: 60,
                        emit_interval_seconds: 5,
                    },
                    metrics: vec![streamforge::AggregationMetricConfig {
                        name: "count".to_string(),
                        op: streamforge::AggregationOp::Count,
                        path: None,
                        percentiles: None,
                    }],
                }),
                partition: None,
                broadcast: false,
                description: None,
                udfs: None,
            };

            let result = validate_aggregation_destination(&destination);

            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                MirrorMakerError::Config(message)
                    if message.contains("aggregation destinations do not support output templates")
            ));
        }

        #[test]
        fn test_allows_fixed_output_for_aggregation_destination() {
            let destination = streamforge::DestinationConfig {
                output: "aggregates.fixed".to_string(),
                match_value: None,
                filter: None,
                transform: None,
                error_policy: Default::default(),
                key_transform: None,
                headers: None,
                header_transforms: None,
                timestamp: None,
                aggregation: Some(streamforge::AggregationConfig {
                    group_by: vec![streamforge::AggregationGroupBy {
                        name: "tenant".to_string(),
                        path: "/tenant".to_string(),
                    }],
                    window: streamforge::AggregationWindowConfig {
                        window_type: streamforge::AggregationWindowType::Tumbling,
                        size_seconds: 60,
                        emit_interval_seconds: 5,
                    },
                    metrics: vec![streamforge::AggregationMetricConfig {
                        name: "count".to_string(),
                        op: streamforge::AggregationOp::Count,
                        path: None,
                        percentiles: None,
                    }],
                }),
                partition: None,
                broadcast: false,
                description: None,
                udfs: None,
            };

            assert!(validate_aggregation_destination(&destination).is_ok());
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

    #[test]
    fn metadata_arguments_are_side_effect_free() {
        assert_eq!(metadata_argument(["--help".to_string()]), Some(CLI_HELP));
        assert_eq!(
            metadata_argument(["--version".to_string()]),
            Some(env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(metadata_argument(["--unknown".to_string()]), None);
        assert_eq!(metadata_argument(Vec::<String>::new()), None);
    }
}
