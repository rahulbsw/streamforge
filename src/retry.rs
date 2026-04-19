//! Retry logic with exponential backoff
//!
//! Implements retry policies for transient failures according to the
//! recovery actions defined in error.rs.

use crate::{MirrorMakerError, Result};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

/// Retry configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,

    /// Initial delay in milliseconds
    pub initial_delay_ms: u64,

    /// Maximum delay in milliseconds (cap for exponential backoff)
    pub max_delay_ms: u64,

    /// Backoff multiplier (typically 2.0 for exponential)
    pub multiplier: f64,

    /// Jitter factor (0.0-1.0, adds randomness to delay)
    pub jitter: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 30_000,
            multiplier: 2.0,
            jitter: 0.1,
        }
    }
}

impl RetryConfig {
    /// Calculate delay for given attempt number (0-indexed)
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let base_delay = self.initial_delay_ms as f64 * self.multiplier.powi(attempt as i32);
        let capped_delay = base_delay.min(self.max_delay_ms as f64);

        // Add jitter: random value between (1 - jitter) and (1 + jitter)
        let jitter_factor = if self.jitter > 0.0 {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            1.0 + rng.gen_range(-self.jitter..self.jitter)
        } else {
            1.0
        };

        let final_delay = (capped_delay * jitter_factor) as u64;
        Duration::from_millis(final_delay)
    }
}

/// Retry a fallible operation with exponential backoff
///
/// # Arguments
/// * `operation` - The operation to retry (must be idempotent)
/// * `config` - Retry configuration
/// * `operation_name` - Human-readable name for logging
///
/// # Returns
/// * `Ok(T)` if operation succeeds
/// * `Err(RetryExhausted)` if all attempts fail
///
/// # Example
/// ```ignore
/// let result = retry_with_backoff(
///     || producer.send(msg),
///     &config,
///     "produce_message"
/// ).await?;
/// ```
pub async fn retry_with_backoff<F, Fut, T>(
    mut operation: F,
    config: &RetryConfig,
    operation_name: &str,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error = None;

    for attempt in 0..config.max_attempts {
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    debug!(
                        operation = operation_name,
                        attempt = attempt + 1,
                        "Operation succeeded after retry"
                    );
                }
                return Ok(result);
            }
            Err(err) => {
                // Check if error is recoverable
                if !err.is_recoverable() {
                    debug!(
                        operation = operation_name,
                        error = %err,
                        "Error not recoverable, aborting retry"
                    );
                    return Err(err);
                }

                last_error = Some(err);

                // Don't sleep after last attempt
                if attempt < config.max_attempts - 1 {
                    let delay = config.calculate_delay(attempt);
                    warn!(
                        operation = operation_name,
                        attempt = attempt + 1,
                        max_attempts = config.max_attempts,
                        delay_ms = delay.as_millis(),
                        error = %last_error.as_ref().unwrap(),
                        "Operation failed, retrying after delay"
                    );
                    sleep(delay).await;
                }
            }
        }
    }

    // All attempts exhausted
    let last_err = last_error.unwrap();
    Err(MirrorMakerError::RetryExhausted {
        message: format!(
            "{} failed after {} attempts",
            operation_name, config.max_attempts
        ),
        attempts: config.max_attempts,
        last_error: last_err.to_string(),
    })
}

/// Retry policy for specific error types
pub struct RetryPolicy {
    config: RetryConfig,
}

impl RetryPolicy {
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(RetryConfig::default())
    }

    /// Retry an operation according to the policy
    pub async fn execute<F, Fut, T>(&self, operation: F, operation_name: &str) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        retry_with_backoff(operation, &self.config, operation_name).await
    }

    /// Get the retry configuration
    pub fn config(&self) -> &RetryConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_calculate_delay() {
        let config = RetryConfig {
            max_attempts: 5,
            initial_delay_ms: 100,
            max_delay_ms: 10_000,
            multiplier: 2.0,
            jitter: 0.0, // No jitter for predictable tests
        };

        // Attempt 0: 100ms * 2^0 = 100ms
        assert_eq!(config.calculate_delay(0), Duration::from_millis(100));

        // Attempt 1: 100ms * 2^1 = 200ms
        assert_eq!(config.calculate_delay(1), Duration::from_millis(200));

        // Attempt 2: 100ms * 2^2 = 400ms
        assert_eq!(config.calculate_delay(2), Duration::from_millis(400));

        // Attempt 10: Would be 102,400ms, but capped at 10,000ms
        assert_eq!(config.calculate_delay(10), Duration::from_millis(10_000));
    }

    #[test]
    fn test_jitter_range() {
        let config = RetryConfig {
            initial_delay_ms: 1000,
            jitter: 0.2, // 20% jitter
            ..Default::default()
        };

        // Test 100 times to ensure jitter is within range
        for _ in 0..100 {
            let delay = config.calculate_delay(0);
            let delay_ms = delay.as_millis() as u64;

            // Should be between 800ms and 1200ms (1000 ± 20%)
            assert!(delay_ms >= 800, "Delay {} < 800ms", delay_ms);
            assert!(delay_ms <= 1200, "Delay {} > 1200ms", delay_ms);
        }
    }

    #[tokio::test]
    async fn test_retry_succeeds_immediately() {
        let config = RetryConfig::default();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let operation = || {
            let count = call_count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok::<i32, MirrorMakerError>(42)
            }
        };

        let result = retry_with_backoff(operation, &config, "test_op").await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_delay_ms: 10, // Short delay for test
            ..Default::default()
        };

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let operation = || {
            let count = call_count_clone.clone();
            async move {
                let attempts = count.fetch_add(1, Ordering::SeqCst);
                if attempts < 2 {
                    // Fail first 2 attempts
                    Err(MirrorMakerError::KafkaProducer {
                        message: "Queue full".into(),
                        destination: None,
                        recoverable: true, // Important: must be recoverable
                    })
                } else {
                    Ok::<i32, MirrorMakerError>(42)
                }
            }
        };

        let result = retry_with_backoff(operation, &config, "test_op").await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_delay_ms: 10,
            ..Default::default()
        };

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let operation = || {
            let count = call_count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err::<i32, MirrorMakerError>(MirrorMakerError::KafkaProducer {
                    message: "Always fails".into(),
                    destination: None,
                    recoverable: true,
                })
            }
        };

        let result = retry_with_backoff(operation, &config, "test_op").await;

        assert!(matches!(
            result,
            Err(MirrorMakerError::RetryExhausted { .. })
        ));
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_non_recoverable_error_no_retry() {
        let config = RetryConfig::default();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let operation = || {
            let count = call_count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                // Non-recoverable error
                Err::<i32, MirrorMakerError>(MirrorMakerError::Config("Invalid config".into()))
            }
        };

        let result = retry_with_backoff(operation, &config, "test_op").await;

        assert!(matches!(result, Err(MirrorMakerError::Config(_))));
        // Should only be called once (no retry)
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_policy() {
        let policy = RetryPolicy::default();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let operation = || {
            let count = call_count_clone.clone();
            async move {
                let attempts = count.fetch_add(1, Ordering::SeqCst);
                if attempts < 1 {
                    Err(MirrorMakerError::KafkaProducer {
                        message: "Fail once".into(),
                        destination: None,
                        recoverable: true,
                    })
                } else {
                    Ok::<i32, MirrorMakerError>(42)
                }
            }
        };

        let result = policy.execute(operation, "test_policy").await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }
}
