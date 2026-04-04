use crate::observability::METRICS;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::Offset;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Start monitoring Kafka consumer lag
pub async fn start_lag_monitor(
    consumer: Arc<StreamConsumer>,
    interval_secs: u64,
) {
    info!(
        "🔍 Starting consumer lag monitor (interval: {}s)",
        interval_secs
    );

    let mut ticker = interval(Duration::from_secs(interval_secs));

    loop {
        ticker.tick().await;

        match monitor_lag(&consumer).await {
            Ok(total_lag) => {
                if total_lag > 0 {
                    debug!("Consumer lag: {} messages behind", total_lag);
                }
            }
            Err(e) => {
                warn!("Failed to monitor consumer lag: {}", e);
            }
        }
    }
}

async fn monitor_lag(
    consumer: &StreamConsumer,
) -> Result<i64, Box<dyn std::error::Error>> {
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
                        "Invalid offset for {}-{}, skipping",
                        topic, partition
                    );
                    continue;
                }
                _ => {
                    debug!(
                        "Non-offset position for {}-{}, skipping",
                        topic, partition
                    );
                    continue;
                }
            },
            None => {
                debug!("No position found for {}-{}, skipping", topic, partition);
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
                        "High lag detected: topic={} partition={} lag={} (offset={} high={})",
                        topic, partition, lag, position, high
                    );
                }
            }
            Err(e) => {
                error!(
                    "Failed to fetch watermarks for {}-{}: {}",
                    topic, partition, e
                );
            }
        }
    }

    Ok(total_lag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lag_calculation() {
        // Simple arithmetic test
        let high_watermark = 1000i64;
        let current_offset = 750i64;
        let lag = high_watermark - current_offset;
        assert_eq!(lag, 250);
    }
}
