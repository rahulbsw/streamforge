use super::*;
use crate::config::{CommitStrategyConfig, CompressionConfig, MirrorMakerConfig};

fn create_test_config() -> MirrorMakerConfig {
    MirrorMakerConfig {
        appid: "test-app".to_string(),
        bootstrap: "localhost:9092".to_string(),
        input: "test-input".to_string(),
        output: Some("test-output".to_string()),
        target_broker: None,
        offset: "latest".to_string(),
        threads: 4,
        performance: Default::default(),
        compression: CompressionConfig::default(),
        routing: None,
        transform: None,
        consumer_properties: HashMap::new(),
        producer_properties: HashMap::new(),
        security: None,
        commit_strategy: CommitStrategyConfig::default(),
        cache: None,
        observability: Default::default(),
        retry: Default::default(),
        dlq: Default::default(),
        wasm: None,
        udfs: None,
    }
}

#[tokio::test]
#[ignore] // Requires running Kafka
async fn test_kafka_sink_creation() {
    let config = create_test_config();
    let result = KafkaSink::new(&config, "test-topic".to_string(), None).await;

    // This will fail without a running Kafka, but tests the API
    assert!(result.is_err());
}

#[test]
fn test_multi_sink() {
    let multi = MultiSink::new();
    assert_eq!(multi.sinks.len(), 0);
}

mod topic_template_tests {
    use crate::envelope::MessageEnvelope;
    use serde_json::json;

    /// Mirror `KafkaSink::resolve_topic` logic for unit tests without a real broker.
    fn resolve(template: &str, is_template: bool, source: Option<&str>) -> Result<String, String> {
        if is_template {
            source
                .ok_or_else(|| "no source topic".to_string())
                .map(|src| template.replace("{source_topic}", src))
        } else {
            Ok(template.to_string())
        }
    }

    #[test]
    fn test_fixed_topic_unchanged() {
        assert_eq!(
            resolve("events-copy", false, Some("events")).unwrap(),
            "events-copy"
        );
    }

    #[test]
    fn test_template_replaced_with_source_topic() {
        assert_eq!(
            resolve("mirror.{source_topic}", true, Some("payments")).unwrap(),
            "mirror.payments"
        );
    }

    #[test]
    fn test_template_with_prefix_and_suffix() {
        assert_eq!(
            resolve("prod.{source_topic}.v2", true, Some("orders")).unwrap(),
            "prod.orders.v2"
        );
    }

    #[test]
    fn test_template_no_source_is_error() {
        // A missing source topic must now produce an error — not silently
        // route to "mirror.unknown".
        assert!(resolve("copy.{source_topic}", true, None).is_err());
    }

    #[test]
    fn test_is_template_detection() {
        assert!("mirror.{source_topic}".contains("{source_topic}"));
        assert!("{source_topic}-copy".contains("{source_topic}"));
        assert!(!"fixed-topic".contains("{source_topic}"));
    }

    #[test]
    fn test_envelope_source_topic_used() {
        let mut envelope = MessageEnvelope::new(json!({"event": "login"}));
        envelope.topic = Some("auth-events".to_string());

        let resolved = resolve("processed.{source_topic}", true, envelope.topic.as_deref());
        assert_eq!(resolved.unwrap(), "processed.auth-events");
    }
}
