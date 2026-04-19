///! Happy path integration tests
///!
///! Tests that verify the basic consume → process → produce → commit flow works correctly.

mod common;

use common::*;
use serde_json::json;
use serial_test::serial;
use testcontainers::clients::Cli;

#[tokio::test]
#[serial]
#[ignore] // Requires Docker - run with: cargo test --test happy_path_test -- --ignored
async fn test_basic_consume_produce_flow() {
    // Start test Kafka
    let docker = Cli::default();
    let kafka = TestKafka::start(&docker).await;

    // Create topics
    kafka
        .create_topics(&[("input-topic", 1), ("output-topic", 1)])
        .await
        .expect("Failed to create topics");

    // Send test messages
    let producer = create_producer(kafka.bootstrap());
    for i in 0..10 {
        send_message(
            &producer,
            "input-topic",
            Some(&format!("key-{}", i)),
            json!({"id": i, "message": format!("test-{}", i)}),
        )
        .await
        .expect("Failed to send message");
    }

    // Wait for messages to appear in output
    let messages = wait_for_messages(kafka.bootstrap(), "output-topic", 10, 15)
        .await
        .expect("Failed to consume messages");

    // Verify all messages arrived
    assert_eq!(messages.len(), 10, "Expected 10 messages in output");

    // Verify message content
    for (i, msg) in messages.iter().enumerate() {
        assert_eq!(msg["id"], i, "Message ID mismatch");
        assert!(
            msg["message"].as_str().unwrap().starts_with("test-"),
            "Message content mismatch"
        );
    }
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_passthrough_preserves_data() {
    let docker = Cli::default();
    let kafka = TestKafka::start(&docker).await;

    kafka
        .create_topics(&[("input", 1), ("output", 1)])
        .await
        .unwrap();

    // Send complex nested message
    let producer = create_producer(kafka.bootstrap());
    let test_data = json!({
        "user": {
            "id": 12345,
            "name": "Alice",
            "email": "alice@example.com"
        },
        "timestamp": 1700000000,
        "metadata": {
            "source": "api",
            "version": "1.0"
        }
    });

    send_message(&producer, "input", Some("user-12345"), test_data.clone())
        .await
        .unwrap();

    // Verify data preserved exactly
    let messages = wait_for_messages(kafka.bootstrap(), "output", 1, 10)
        .await
        .unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0], test_data, "Data was modified in passthrough");
}
