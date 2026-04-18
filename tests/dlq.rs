///! DLQ (Dead Letter Queue) integration tests
///!
///! Tests that verify failed messages are correctly routed to DLQ with proper metadata.

mod common;

use common::*;
use rdkafka::message::{Headers, Message};
use serde_json::json;
use serial_test::serial;
use testcontainers::clients::Cli;

#[tokio::test]
#[serial]
#[ignore] // Requires Docker
async fn test_filter_failure_sends_to_dlq() {
    let docker = Cli::default();
    let kafka = TestKafka::start(&docker).await;

    kafka
        .create_topics(&[("input-topic", 1), ("output-topic", 1), ("test-dlq", 1)])
        .await
        .unwrap();

    // Create config with filter that will fail on some messages
    let mut config = test_config_base(kafka.bootstrap(), "input-topic", "output-topic");
    config = with_dlq(config, "test-dlq");

    // Send messages: some valid, some invalid for the filter
    let producer = create_producer(kafka.bootstrap());

    // Valid message (has status field)
    send_message(
        &producer,
        "input-topic",
        None,
        json!({"id": 1, "status": "active"}),
    )
    .await
    .unwrap();

    // Invalid message (missing status field - will fail filter)
    send_message(
        &producer,
        "input-topic",
        None,
        json!({"id": 2, "missing_field": true}),
    )
    .await
    .unwrap();

    // Wait for DLQ message
    let dlq_consumer = create_consumer(kafka.bootstrap(), "dlq-test-group", "test-dlq");

    // Give processor time to fail and send to DLQ
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Check DLQ has the failed message
    use futures::stream::StreamExt;
    let mut stream = dlq_consumer.stream();

    if let Some(Ok(msg)) = stream.next().await {
        // Verify payload is the bad message
        let payload = msg.payload().unwrap();
        let value: serde_json::Value = serde_json::from_slice(payload).unwrap();
        assert_eq!(value["id"], 2, "Wrong message in DLQ");

        // Verify DLQ headers exist
        let headers = msg.headers().expect("No headers in DLQ message");
        let mut found_error_type = false;
        let mut found_source_topic = false;

        for h in headers.iter() {
            match h.key {
                "x-streamforge-error-type" => {
                    found_error_type = true;
                    let error_type = String::from_utf8_lossy(h.value.unwrap());
                    assert!(
                        !error_type.is_empty(),
                        "Error type header is empty"
                    );
                }
                "x-streamforge-source-topic" => {
                    found_source_topic = true;
                    let topic = String::from_utf8_lossy(h.value.unwrap());
                    assert_eq!(topic, "input-topic");
                }
                _ => {}
            }
        }

        assert!(found_error_type, "Missing x-streamforge-error-type header");
        assert!(found_source_topic, "Missing x-streamforge-source-topic header");
    } else {
        panic!("No message in DLQ after failure");
    }
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_dlq_preserves_original_headers() {
    let docker = Cli::default();
    let kafka = TestKafka::start(&docker).await;

    kafka
        .create_topics(&[("input", 1), ("dlq", 1)])
        .await
        .unwrap();

    // Send message with custom headers that will fail processing
    let producer = create_producer(kafka.bootstrap());
    let record = rdkafka::producer::FutureRecord::to("input")
        .payload(b"{\"bad\":\"data\"}")
        .headers(rdkafka::message::OwnedHeaders::new().insert(rdkafka::message::Header {
            key: "custom-header",
            value: Some(b"custom-value"),
        }));

    producer
        .send(record, std::time::Duration::from_secs(5))
        .await
        .unwrap();

    // Wait for DLQ
    let dlq_consumer = create_consumer(kafka.bootstrap(), "dlq-check", "dlq");
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    use futures::stream::StreamExt;
    if let Some(Ok(msg)) = dlq_consumer.stream().next().await {
        let headers = msg.headers().unwrap();
        let mut found_custom = false;
        let mut found_error = false;

        for h in headers.iter() {
            match h.key {
                "custom-header" => {
                    found_custom = true;
                    assert_eq!(h.value.unwrap(), b"custom-value");
                }
                "x-streamforge-error" => {
                    found_error = true;
                }
                _ => {}
            }
        }

        assert!(found_custom, "Original custom header not preserved in DLQ");
        assert!(found_error, "Streamforge error header not added");
    } else {
        panic!("No DLQ message");
    }
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_dlq_includes_timestamp() {
    let docker = Cli::default();
    let kafka = TestKafka::start(&docker).await;

    kafka.create_topics(&[("input", 1), ("dlq", 1)]).await.unwrap();

    // Send bad message
    let producer = create_producer(kafka.bootstrap());
    send_message(&producer, "input", None, json!({"invalid": "message"}))
        .await
        .unwrap();

    // Check DLQ for timestamp header
    let dlq_consumer = create_consumer(kafka.bootstrap(), "ts-check", "dlq");
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    use futures::stream::StreamExt;
    if let Some(Ok(msg)) = dlq_consumer.stream().next().await {
        let headers = msg.headers().unwrap();
        let mut found_timestamp = false;

        for h in headers.iter() {
            if h.key == "x-streamforge-timestamp" {
                found_timestamp = true;
                let ts_str = String::from_utf8_lossy(h.value.unwrap());
                // Verify it's an ISO 8601 timestamp
                assert!(ts_str.contains("T"), "Timestamp not in ISO 8601 format");
                assert!(ts_str.contains("Z") || ts_str.contains("+"), "Missing timezone");
            }
        }

        assert!(found_timestamp, "Missing timestamp header in DLQ message");
    }
}
