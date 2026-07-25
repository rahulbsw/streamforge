use crate::observability::METRICS;
use crate::{MirrorMakerError, Result};
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use rdkafka::producer::future_producer::OwnedDeliveryResult;
use rdkafka::producer::DeliveryFuture;
use tracing::{debug, error};

type DeliveryOutcome =
    std::result::Result<OwnedDeliveryResult, futures::channel::oneshot::Canceled>;

/// Tracks queued producer deliveries without spawning one Tokio task per record.
///
/// A delivery failure permanently faults the tracker. This is deliberate:
/// queued delivery errors occur after `send` has returned and therefore cannot
/// safely be attributed to a later envelope for message-level retry.
pub(super) struct DeliveryState {
    pending: FuturesUnordered<DeliveryFuture>,
    failure: Option<MirrorMakerError>,
    destination: String,
}

impl DeliveryState {
    pub(super) fn new(destination: String) -> Self {
        Self {
            pending: FuturesUnordered::new(),
            failure: None,
            destination,
        }
    }

    fn record_outcome(&mut self, outcome: DeliveryOutcome) {
        match outcome {
            Ok(Ok((partition, offset))) => {
                METRICS
                    .messages_delivered
                    .with_label_values(&[&self.destination])
                    .inc();
                debug!(
                    "Message delivered: partition={}, offset={}",
                    partition, offset
                );
            }
            Ok(Err((err, _message))) => {
                error!("Asynchronous Kafka delivery failed: {err}");
                if self.failure.is_none() {
                    self.failure = Some(err.into());
                }
            }
            Err(err) => {
                error!("Kafka delivery future was canceled: {err}");
                if self.failure.is_none() {
                    self.failure = Some(MirrorMakerError::Kafka(format!(
                        "producer dropped before delivery completed: {err}"
                    )));
                }
            }
        }
    }

    pub(super) fn push(&mut self, delivery: DeliveryFuture) {
        self.pending.push(delivery);
    }

    pub(super) fn failure(&self) -> Option<MirrorMakerError> {
        self.failure.clone()
    }

    /// Remove every delivery which is already complete without waiting.
    pub(super) fn reap_ready(&mut self) {
        loop {
            let Some(outcome) = self.pending.next().now_or_never().flatten() else {
                break;
            };
            self.record_outcome(outcome);
        }
    }

    /// Apply bounded backpressure and surface any previously observed failure.
    pub(super) async fn wait_for_capacity(&mut self, max_pending: usize) -> Result<()> {
        self.reap_ready();
        if let Some(err) = self.failure() {
            return Err(err);
        }

        while self.pending.len() >= max_pending {
            if let Some(outcome) = self.pending.next().await {
                self.record_outcome(outcome);
            }
            if let Some(err) = self.failure() {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Wait for every tracked delivery and return the first delivery failure.
    pub(super) async fn drain(&mut self) -> Result<()> {
        while let Some(outcome) = self.pending.next().await {
            self.record_outcome(outcome);
        }

        match self.failure() {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rdkafka::error::{KafkaError, RDKafkaErrorCode};
    use rdkafka::message::{OwnedMessage, Timestamp};

    #[test]
    fn delivery_success_does_not_fault_tracker() {
        let mut state = DeliveryState::new("target".to_string());
        state.record_outcome(Ok(Ok((2, 17))));
        assert!(state.failure().is_none());
    }

    #[test]
    fn delivery_error_faults_tracker() {
        let mut state = DeliveryState::new("target".to_string());
        let message = OwnedMessage::new(
            Some(b"value".to_vec()),
            None,
            "target".to_string(),
            Timestamp::NotAvailable,
            0,
            0,
            None,
        );

        state.record_outcome(Ok(Err((
            KafkaError::MessageProduction(RDKafkaErrorCode::MessageTimedOut),
            message,
        ))));

        let error = state.failure().expect("delivery error must fault tracker");
        assert!(error.to_string().contains("Message production error"));
    }

    #[tokio::test]
    async fn canceled_delivery_faults_tracker() {
        let mut state = DeliveryState::new("target".to_string());
        let (sender, receiver) = futures::channel::oneshot::channel::<OwnedDeliveryResult>();
        drop(sender);

        state.record_outcome(receiver.await);

        let error = state
            .failure()
            .expect("canceled delivery must fault tracker");
        assert!(error
            .to_string()
            .contains("producer dropped before delivery completed"));
    }
}
