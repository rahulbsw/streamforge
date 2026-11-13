use crate::observability::METRICS;
use crate::observability::{KafkaReadinessFailure, ReadinessState};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::Offset;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Continuously verify that the Kafka dependency can answer metadata requests.
///
/// The probe uses a blocking worker because librdkafka's metadata call is
/// synchronous. Readiness is intentionally independent of optional lag
/// monitoring so disabling lag metrics does not disable dependency checks.
pub async fn start_kafka_readiness_monitor(
    consumer: Arc<StreamConsumer>,
    readiness: ReadinessState,
    interval_secs: u64,
) {
    let mut ticker = interval(Duration::from_secs(interval_secs.max(1)));

    loop {
        ticker.tick().await;

        let probe_consumer = consumer.clone();
        let probe = tokio::task::spawn_blocking(move || {
            probe_consumer.fetch_metadata(None, Duration::from_secs(5))
        })
        .await;

        match probe {
            Ok(Ok(_)) => readiness.mark_kafka_ready(),
            Ok(Err(error)) => {
                readiness.mark_kafka_unready(KafkaReadinessFailure::ConsumerError);
                warn!(
                    error_category = "kafka_readiness",
                    error = %error,
                    "Kafka readiness probe failed"
                );
            }
            Err(error) => {
                readiness.mark_kafka_unready(KafkaReadinessFailure::ConsumerError);
                error!(
                    error_category = "kafka_readiness_task",
                    error = %error,
                    "Kafka readiness probe task failed"
                );
            }
        }
    }
}

/// Start monitoring Kafka consumer lag
pub async fn start_lag_monitor(consumer: Arc<StreamConsumer>, interval_secs: u64) {
    info!(
        interval_seconds = interval_secs,
        "consumer lag monitor started"
    );

    let mut ticker = interval(Duration::from_secs(interval_secs));

    loop {
        ticker.tick().await;

        match monitor_lag(&consumer).await {
            Ok(total_lag) => {
                if total_lag > 0 {
                    debug!(consumer_lag = total_lag, "consumer lag observed");
                }
            }
            Err(error) => {
                warn!(
                    error_category = "kafka_lag",
                    error = %error,
                    "consumer lag monitor failed"
                );
            }
        }
    }
}

async fn monitor_lag(consumer: &StreamConsumer) -> Result<i64, Box<dyn std::error::Error>> {
    // Get current assignment
    let assignment = consumer.assignment()?;

    if assignment.count() == 0 {
        debug!("No partitions assigned yet, skipping lag monitoring");
        return Ok(0);
    }

    let mut total_lag = 0i64;

    for element in assignment.elements() {
        let topic = element.topic();
        let partition = element.partition();

        // Get current position (where consumer is at)
        let position = match consumer.position()?.find_partition(topic, partition) {
            Some(tpl) => match tpl.offset() {
                Offset::Offset(offset) => offset,
                Offset::Invalid => {
                    debug!(
                        topic,
                        partition,
                        error_category = "invalid_offset",
                        "consumer lag sample skipped"
                    );
                    continue;
                }
                _ => {
                    debug!(
                        topic,
                        partition,
                        error_category = "non_offset_position",
                        "consumer lag sample skipped"
                    );
                    continue;
                }
            },
            None => {
                debug!(
                    topic,
                    partition,
                    error_category = "position_unavailable",
                    "consumer lag sample skipped"
                );
                continue;
            }
        };

        // Get high watermark (latest message in partition)
        match consumer.fetch_watermarks(topic, partition, Duration::from_secs(5)) {
            Ok((_low, high)) => {
                let lag = high - position;
                total_lag += lag;

                // Cache partition string to avoid repeated allocations
                let partition_str = partition.to_string();

                // Update metrics
                METRICS
                    .consumer_lag
                    .with_label_values(&[topic, &partition_str])
                    .set(lag as f64);

                METRICS
                    .consumer_offset
                    .with_label_values(&[topic, &partition_str])
                    .set(position as f64);

                METRICS
                    .consumer_high_watermark
                    .with_label_values(&[topic, &partition_str])
                    .set(high as f64);

                if lag > 10000 {
                    warn!(
                        topic,
                        partition,
                        consumer_lag = lag,
                        offset = position,
                        high_watermark = high,
                        "high consumer lag detected"
                    );
                }
            }
            Err(error) => {
                error!(
                    error_category = "kafka_watermark",
                    topic,
                    partition,
                    error = %error,
                    "failed to fetch Kafka watermarks"
                );
            }
        }
    }

    Ok(total_lag)
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_lag_calculation() {
        // Simple arithmetic test
        let high_watermark = 1000i64;
        let current_offset = 750i64;
        let lag = high_watermark - current_offset;
        assert_eq!(lag, 250);
    }
}
