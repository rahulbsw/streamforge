use crate::metrics::Stats;
use crate::observability::{labels, METRICS};
use crate::processor::MessageProcessor;
use crate::{MessageEnvelope, MirrorMakerError, Result};
use rdkafka::message::{Headers, Message, OwnedMessage};
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePosition {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
}

#[derive(Debug)]
pub struct ProcessingCompletion {
    pub position: SourcePosition,
    pub result: Result<()>,
}

struct WorkItem {
    message: OwnedMessage,
}

/// Fixed, bounded worker lanes for source-partition-affine processing.
///
/// Every `(topic, partition)` maps to one FIFO lane, so records from the same
/// source partition are processed in consumption order. Different lanes run as
/// independent Tokio tasks and therefore make `threads` an actual logical
/// processing-worker count.
pub struct PartitionOrderedExecutor {
    lanes: Vec<mpsc::Sender<WorkItem>>,
    completions: mpsc::UnboundedReceiver<ProcessingCompletion>,
    workers: Vec<JoinHandle<()>>,
}

impl PartitionOrderedExecutor {
    pub fn new(
        processor: Arc<dyn MessageProcessor>,
        stats: Arc<Stats>,
        worker_count: usize,
        queue_capacity: usize,
    ) -> Self {
        assert!(worker_count > 0, "worker_count must be positive");
        assert!(queue_capacity > 0, "queue_capacity must be positive");

        let (completion_tx, completions) = mpsc::unbounded_channel();
        let mut lanes = Vec::with_capacity(worker_count);
        let mut workers = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            let (lane_tx, mut lane_rx) = mpsc::channel::<WorkItem>(queue_capacity);
            let processor = processor.clone();
            let stats = stats.clone();
            let completion_tx = completion_tx.clone();

            workers.push(tokio::spawn(async move {
                while let Some(item) = lane_rx.recv().await {
                    let position = source_position(&item.message);
                    let result = process_owned_message(&processor, &stats, item.message).await;
                    METRICS.messages_in_flight.dec();

                    if completion_tx
                        .send(ProcessingCompletion { position, result })
                        .is_err()
                    {
                        break;
                    }
                }
            }));
            lanes.push(lane_tx);
        }

        Self {
            lanes,
            completions,
            workers,
        }
    }

    pub async fn dispatch(&self, message: OwnedMessage) -> Result<()> {
        let lane = lane_index(message.topic(), message.partition(), self.lanes.len());
        METRICS.messages_in_flight.inc();

        if self.lanes[lane].send(WorkItem { message }).await.is_err() {
            METRICS.messages_in_flight.dec();
            return Err(MirrorMakerError::Processing(format!(
                "partition worker lane {lane} closed unexpectedly"
            )));
        }

        Ok(())
    }

    pub async fn next_completion(&mut self) -> Option<ProcessingCompletion> {
        self.completions.recv().await
    }

    /// Stop intake, drain all bounded lanes, and return completions that were
    /// produced while the workers were shutting down.
    pub async fn shutdown(mut self) -> Result<Vec<ProcessingCompletion>> {
        self.lanes.clear();

        for worker in self.workers {
            worker.await.map_err(|join_error| {
                MirrorMakerError::Processing(format!(
                    "partition worker terminated unexpectedly: {join_error}"
                ))
            })?;
        }

        let mut remaining = Vec::new();
        while let Ok(completion) = self.completions.try_recv() {
            remaining.push(completion);
        }
        Ok(remaining)
    }
}

fn lane_index(topic: &str, partition: i32, worker_count: usize) -> usize {
    let mut hasher = DefaultHasher::new();
    topic.hash(&mut hasher);
    let topic_lane = hasher.finish() as usize % worker_count;
    let partition_lane = partition.rem_euclid(worker_count as i32) as usize;
    (topic_lane + partition_lane) % worker_count
}

fn source_position(message: &OwnedMessage) -> SourcePosition {
    SourcePosition {
        topic: message.topic().to_string(),
        partition: message.partition(),
        offset: message.offset(),
    }
}

async fn process_owned_message(
    processor: &Arc<dyn MessageProcessor>,
    stats: &Arc<Stats>,
    message: OwnedMessage,
) -> Result<()> {
    stats.processed();
    METRICS.messages_consumed.inc();

    let position = source_position(&message);
    let key = parse_message_key(message.key());
    let value = match parse_message_value(message.payload()) {
        Ok(value) => value,
        Err(parse_error) => {
            error!(
                "Failed to parse message: {} (topic={}, partition={}, offset={})",
                parse_error, position.topic, position.partition, position.offset
            );
            stats.error();
            METRICS
                .processing_errors
                .with_label_values(&[labels::ERROR_TYPE_PARSE])
                .inc();
            return Err(parse_error);
        }
    };

    let key = (!key.is_null()).then_some(key);
    let mut envelope = MessageEnvelope::with_key(key, value);

    if let Some(headers) = message.headers() {
        let envelope_headers = Arc::make_mut(&mut envelope.headers);
        for header in headers.iter() {
            envelope_headers.insert(
                header.key.to_string(),
                header.value.map(ToOwned::to_owned).unwrap_or_default(),
            );
        }
    }

    envelope.timestamp = message.timestamp().to_millis();
    envelope.topic = Some(position.topic.clone());
    envelope.partition = Some(position.partition);
    envelope.offset = Some(position.offset);

    match processor.process(envelope).await {
        Ok(()) => {
            stats.completed();
            Ok(())
        }
        Err(processing_error) => {
            error!(
                "Failed to process message: {} (topic={}, partition={}, offset={})",
                processing_error, position.topic, position.partition, position.offset
            );
            stats.error();
            METRICS
                .processing_errors
                .with_label_values(&[labels::ERROR_TYPE_PROCESSING])
                .inc();
            Err(processing_error)
        }
    }
}

pub fn parse_message_key(raw: Option<&[u8]>) -> Value {
    match raw {
        Some(key) => serde_json::from_slice::<Value>(key)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(key).to_string())),
        None => Value::Null,
    }
}

pub fn parse_message_value(raw: Option<&[u8]>) -> Result<Value> {
    match raw {
        Some(value) => serde_json::from_slice::<Value>(value)
            .map_err(|error| MirrorMakerError::Processing(format!("Invalid JSON: {error}"))),
        None => Err(MirrorMakerError::Processing("Empty payload".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rdkafka::message::Timestamp;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::sync::{Barrier, Semaphore};

    struct RecordingProcessor {
        offsets: Mutex<Vec<(i32, i64)>>,
    }

    struct BarrierProcessor {
        barrier: Arc<Barrier>,
    }

    #[async_trait]
    impl MessageProcessor for BarrierProcessor {
        async fn process(&self, _envelope: MessageEnvelope) -> Result<()> {
            self.barrier.wait().await;
            Ok(())
        }
    }

    struct GateProcessor {
        started: AtomicUsize,
        permits: Arc<Semaphore>,
    }

    #[async_trait]
    impl MessageProcessor for GateProcessor {
        async fn process(&self, _envelope: MessageEnvelope) -> Result<()> {
            self.started.fetch_add(1, Ordering::SeqCst);
            let _permit = self.permits.acquire().await.unwrap();
            Ok(())
        }
    }

    #[async_trait]
    impl MessageProcessor for RecordingProcessor {
        async fn process(&self, envelope: MessageEnvelope) -> Result<()> {
            if envelope.partition == Some(0) {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            self.offsets
                .lock()
                .unwrap()
                .push((envelope.partition.unwrap(), envelope.offset.unwrap()));
            Ok(())
        }
    }

    fn message(partition: i32, offset: i64) -> OwnedMessage {
        OwnedMessage::new(
            Some(br#"{"ok":true}"#.to_vec()),
            None,
            "input".to_string(),
            Timestamp::NotAvailable,
            partition,
            offset,
            None,
        )
    }

    #[test]
    fn adjacent_partitions_use_distinct_lanes_when_capacity_allows() {
        let lane0 = lane_index("input", 0, 8);
        let lane1 = lane_index("input", 1, 8);
        assert_ne!(lane0, lane1);
    }

    #[tokio::test]
    async fn preserves_order_within_each_source_partition() {
        let processor = Arc::new(RecordingProcessor {
            offsets: Mutex::new(Vec::new()),
        });
        let processor_trait: Arc<dyn MessageProcessor> = processor.clone();
        let executor = PartitionOrderedExecutor::new(processor_trait, Arc::new(Stats::new()), 2, 8);

        for offset in 0..4 {
            executor.dispatch(message(0, offset)).await.unwrap();
            executor.dispatch(message(1, offset)).await.unwrap();
        }

        let completions = executor.shutdown().await.unwrap();
        assert_eq!(completions.len(), 8);
        assert!(completions
            .iter()
            .all(|completion| completion.result.is_ok()));

        let mut by_partition: HashMap<i32, Vec<i64>> = HashMap::new();
        for (partition, offset) in processor.offsets.lock().unwrap().iter().copied() {
            by_partition.entry(partition).or_default().push(offset);
        }
        assert_eq!(by_partition[&0], vec![0, 1, 2, 3]);
        assert_eq!(by_partition[&1], vec![0, 1, 2, 3]);
    }

    #[tokio::test]
    async fn different_partition_lanes_execute_concurrently() {
        let processor: Arc<dyn MessageProcessor> = Arc::new(BarrierProcessor {
            barrier: Arc::new(Barrier::new(2)),
        });
        let executor = PartitionOrderedExecutor::new(processor, Arc::new(Stats::new()), 2, 2);

        executor.dispatch(message(0, 0)).await.unwrap();
        executor.dispatch(message(1, 0)).await.unwrap();

        let completions = tokio::time::timeout(Duration::from_millis(100), executor.shutdown())
            .await
            .expect("partition workers did not execute concurrently")
            .unwrap();
        assert_eq!(completions.len(), 2);
    }

    #[tokio::test]
    async fn bounded_lane_applies_backpressure() {
        let permits = Arc::new(Semaphore::new(0));
        let processor = Arc::new(GateProcessor {
            started: AtomicUsize::new(0),
            permits: permits.clone(),
        });
        let processor_trait: Arc<dyn MessageProcessor> = processor.clone();
        let executor = PartitionOrderedExecutor::new(processor_trait, Arc::new(Stats::new()), 1, 1);

        executor.dispatch(message(0, 0)).await.unwrap();
        while processor.started.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        executor.dispatch(message(0, 1)).await.unwrap();

        assert!(
            tokio::time::timeout(Duration::from_millis(20), executor.dispatch(message(0, 2)))
                .await
                .is_err(),
            "third record should block behind one active and one queued record"
        );

        permits.add_permits(2);
        let completions = executor.shutdown().await.unwrap();
        assert_eq!(completions.len(), 2);
    }
}
