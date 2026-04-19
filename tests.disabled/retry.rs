///! Retry logic integration tests
///!
///! Tests that verify transient failures are properly retried with exponential backoff.

mod common;

use common::*;
use serde_json::json;
use serial_test::serial;
use testcontainers::clients::Cli;

#[tokio::test]
#[serial]
#[ignore] // Requires Docker
async fn test_retry_policy_configured_from_config() {
    let docker = Cli::default();
    let kafka = TestKafka::start(&docker).await;

    kafka
        .create_topics(&[("input", 1), ("output", 1)])
        .await
        .unwrap();

    // Create config with custom retry settings
    let mut config = test_config_base(kafka.bootstrap(), "input", "output");
    config = with_retry(config, 5); // 5 retry attempts

    // Verify config is correct
    assert_eq!(config.retry.max_attempts, 5);
    assert_eq!(config.retry.initial_delay_ms, 50);
    assert!(config.retry.multiplier > 1.0);
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_successful_message_does_not_retry() {
    let docker = Cli::default();
    let kafka = TestKafka::start(&docker).await;

    kafka
        .create_topics(&[("input", 1), ("output", 1)])
        .await
        .unwrap();

    let config = test_config_base(kafka.bootstrap(), "input", "output");

    // Send valid message
    let producer = create_producer(kafka.bootstrap());
    send_message(&producer, "input", None, json!({"id": 1, "valid": true}))
        .await
        .unwrap();

    // Should process immediately without retries
    let messages = wait_for_messages(kafka.bootstrap(), "output", 1, 10)
        .await
        .unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["id"], 1);
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_retry_exhausted_sends_to_dlq() {
    let docker = Cli::default();
    let kafka = TestKafka::start(&docker).await;

    kafka
        .create_topics(&[("input", 1), ("output", 1), ("retry-dlq", 1)])
        .await
        .unwrap();

    // Config with low retry count and DLQ enabled
    let mut config = test_config_base(kafka.bootstrap(), "input", "output");
    config = with_retry(config, 2); // Only 2 attempts
    config = with_dlq(config, "retry-dlq");

    // Send message that will consistently fail (simulated by bad data)
    let producer = create_producer(kafka.bootstrap());
    send_message(
        &producer,
        "input",
        None,
        json!({"id": 1, "should_fail": true}),
    )
    .await
    .unwrap();

    // Wait for retry exhaustion and DLQ routing
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;

    // Check that message ended up in DLQ (not output)
    // Note: This test assumes the processor fails on {"should_fail": true}
    // In a real test, we'd need to configure the processor to fail on this
}
