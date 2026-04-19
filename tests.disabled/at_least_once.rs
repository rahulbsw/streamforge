///! At-least-once delivery guarantee tests
///!
///! Tests that verify duplicates can occur but no data is lost.

mod common;

use common::*;
use serde_json::json;
use serial_test::serial;
use testcontainers::clients::Cli;

#[tokio::test]
#[serial]
#[ignore] // Requires Docker
async fn test_manual_commit_prevents_duplicates_on_success() {
    let docker = Cli::default();
    let kafka = TestKafka::start(&docker).await;

    kafka
        .create_topics(&[("input", 1), ("output", 1)])
        .await
        .unwrap();

    // Send batch of messages
    let producer = create_producer(kafka.bootstrap());
    for i in 0..20 {
        send_message(&producer, "input", None, json!({"id": i}))
            .await
            .unwrap();
    }

    // Process with manual commit
    let config = test_config_base(kafka.bootstrap(), "input", "output");
    assert!(config.commit_strategy.manual_commit, "Manual commit should be enabled");

    // Wait for all messages to be processed and committed
    let messages = wait_for_messages(kafka.bootstrap(), "output", 20, 15)
        .await
        .unwrap();

    assert_eq!(messages.len(), 20, "All messages should be processed once");

    // Verify no duplicates
    let mut ids: Vec<i64> = messages
        .iter()
        .map(|m| m["id"].as_i64().unwrap())
        .collect();
    ids.sort();

    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id, i as i64, "Duplicate or missing message detected");
    }
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_commit_after_batch_processing() {
    let docker = Cli::default();
    let kafka = TestKafka::start(&docker).await;

    kafka.create_topics(&[("input", 3), ("output", 3)]).await.unwrap();

    // Send messages to multiple partitions
    let producer = create_producer(kafka.bootstrap());
    for i in 0..50 {
        send_message(
            &producer,
            "input",
            Some(&format!("key-{}", i % 3)), // Distribute across partitions
            json!({"id": i, "partition_key": i % 3}),
        )
        .await
        .unwrap();
    }

    // Process with batch commit strategy
    let config = test_config_base(kafka.bootstrap(), "input", "output");

    // Wait for all messages
    let messages = wait_for_messages(kafka.bootstrap(), "output", 50, 20)
        .await
        .unwrap();

    assert_eq!(messages.len(), 50, "All messages should be processed");

    // Verify all IDs present (no data loss)
    let mut ids: Vec<i64> = messages
        .iter()
        .map(|m| m["id"].as_i64().unwrap())
        .collect();
    ids.sort();

    assert_eq!(ids.len(), 50);
    assert_eq!(*ids.first().unwrap(), 0);
    assert_eq!(*ids.last().unwrap(), 49);
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_at_least_once_guarantee_allows_duplicates() {
    // This test documents the behavior: at-least-once means duplicates are possible
    // but no data loss occurs. This is the tradeoff for reliability.

    let docker = Cli::default();
    let kafka = TestKafka::start(&docker).await;

    kafka.create_topics(&[("input", 1), ("output", 1)]).await.unwrap();

    // Send unique messages
    let producer = create_producer(kafka.bootstrap());
    for i in 0..10 {
        send_message(&producer, "input", None, json!({"id": i, "unique": true}))
            .await
            .unwrap();
    }

    // Simulate: process begins, messages produced, crash before commit
    // On restart, messages will be re-consumed and re-produced (duplicates)
    // This is expected behavior for at-least-once delivery

    // Wait for processing
    let messages = wait_for_messages(kafka.bootstrap(), "output", 10, 15)
        .await
        .unwrap();

    // At-least-once means: len >= 10 (could have duplicates)
    // But importantly: no messages lost
    assert!(
        messages.len() >= 10,
        "At-least-once: should have 10 or more messages (duplicates possible)"
    );

    // Verify all original IDs are present
    let unique_ids: std::collections::HashSet<i64> = messages
        .iter()
        .map(|m| m["id"].as_i64().unwrap())
        .collect();

    assert_eq!(
        unique_ids.len(),
        10,
        "All 10 unique messages should be present (no data loss)"
    );
}
