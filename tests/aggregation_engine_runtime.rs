use serde_json::{json, Value};
use streamforge::{
    aggregation::AggregationEngine, AggregationConfig, AggregationGroupBy, AggregationMetricConfig,
    AggregationOp, AggregationWindowConfig, AggregationWindowType,
};

fn test_config() -> AggregationConfig {
    AggregationConfig {
        group_by: vec![
            AggregationGroupBy {
                name: "customer_id".to_string(),
                path: "/customer_id".to_string(),
            },
            AggregationGroupBy {
                name: "region".to_string(),
                path: "/region".to_string(),
            },
        ],
        window: AggregationWindowConfig {
            window_type: AggregationWindowType::Tumbling,
            size_seconds: 60,
            emit_interval_seconds: 5,
        },
        metrics: vec![
            AggregationMetricConfig {
                name: "order_count".to_string(),
                op: AggregationOp::Count,
                path: None,
                percentiles: None,
            },
            AggregationMetricConfig {
                name: "total_amount".to_string(),
                op: AggregationOp::Sum,
                path: Some("/amount".to_string()),
                percentiles: None,
            },
            AggregationMetricConfig {
                name: "avg_amount".to_string(),
                op: AggregationOp::Avg,
                path: Some("/amount".to_string()),
                percentiles: None,
            },
        ],
    }
}

fn count_sum_config() -> AggregationConfig {
    AggregationConfig {
        group_by: vec![AggregationGroupBy {
            name: "customer_id".to_string(),
            path: "/customer_id".to_string(),
        }],
        window: AggregationWindowConfig {
            window_type: AggregationWindowType::Tumbling,
            size_seconds: 60,
            emit_interval_seconds: 5,
        },
        metrics: vec![AggregationMetricConfig {
            name: "total_amount".to_string(),
            op: AggregationOp::Sum,
            path: Some("/amount".to_string()),
            percentiles: None,
        }],
    }
}

fn find_group<'a>(
    records: &'a [streamforge::aggregation::AggregateEmission],
    customer_id: &str,
) -> &'a Value {
    records
        .iter()
        .find(|record| record.value["group"]["customer_id"] == json!(customer_id))
        .map(|record| &record.value)
        .expect("group should exist")
}

#[test]
fn aggregates_grouped_tumbling_windows_and_flushes_only_expired_windows() {
    let mut engine = AggregationEngine::new(test_config(), "orders-metrics-1m".to_string())
        .expect("engine should build");

    engine
        .observe(
            &json!({"customer_id": "cust-1", "region": "west", "amount": 10.0}),
            1_000,
        )
        .expect("first event should aggregate");
    engine
        .observe(
            &json!({"customer_id": "cust-1", "region": "west", "amount": 20.0}),
            15_000,
        )
        .expect("second event should aggregate");
    engine
        .observe(
            &json!({"customer_id": "cust-2", "region": "east", "amount": 7.0}),
            30_000,
        )
        .expect("third event should aggregate");
    engine
        .observe(
            &json!({"customer_id": "cust-1", "region": "west", "amount": 5.0}),
            61_000,
        )
        .expect("next-window event should aggregate");

    assert!(engine
        .flush_expired(59_999)
        .expect("window still open")
        .is_empty());

    let first_flush = engine
        .flush_expired(60_000)
        .expect("first window should flush");
    assert_eq!(first_flush.len(), 2);
    assert!(first_flush
        .iter()
        .all(|record| record.output_topic == "orders-metrics-1m"));
    assert_eq!(
        first_flush[0].group_key.as_str(),
        r#"[{"name":"customer_id","value":"cust-1"},{"name":"region","value":"west"}]"#
    );
    assert_eq!(
        first_flush[1].group_key.as_str(),
        r#"[{"name":"customer_id","value":"cust-2"},{"name":"region","value":"east"}]"#
    );

    assert_eq!(
        find_group(&first_flush, "cust-1"),
        &json!({
            "window": {
                "start_ms": 0,
                "end_ms": 60_000,
                "type": "tumbling",
                "size_seconds": 60
            },
            "group": {
                "customer_id": "cust-1",
                "region": "west"
            },
            "metrics": {
                "order_count": 2,
                "total_amount": 30.0,
                "avg_amount": 15.0
            }
        })
    );
    assert_eq!(
        find_group(&first_flush, "cust-2"),
        &json!({
            "window": {
                "start_ms": 0,
                "end_ms": 60_000,
                "type": "tumbling",
                "size_seconds": 60
            },
            "group": {
                "customer_id": "cust-2",
                "region": "east"
            },
            "metrics": {
                "order_count": 1,
                "total_amount": 7.0,
                "avg_amount": 7.0
            }
        })
    );

    let second_flush = engine
        .flush_expired(120_000)
        .expect("second window should flush");
    assert_eq!(
        second_flush
            .iter()
            .map(|record| &record.value)
            .collect::<Vec<_>>(),
        vec![&json!({
            "window": {
                "start_ms": 60_000,
                "end_ms": 120_000,
                "type": "tumbling",
                "size_seconds": 60
            },
            "group": {
                "customer_id": "cust-1",
                "region": "west"
            },
            "metrics": {
                "order_count": 1,
                "total_amount": 5.0,
                "avg_amount": 5.0
            }
        })]
    );

    assert!(engine
        .flush_expired(180_000)
        .expect("all windows already flushed")
        .is_empty());
}

#[test]
fn rejects_late_events_for_windows_that_were_already_flushed() {
    let mut engine =
        AggregationEngine::new(test_config(), "orders-metrics-1m".to_string()).expect("engine");

    engine
        .observe(
            &json!({"customer_id": "cust-1", "region": "west", "amount": 10.0}),
            1_000,
        )
        .expect("initial event should aggregate");

    let first_flush = engine
        .flush_expired(60_000)
        .expect("first flush should succeed");
    assert_eq!(first_flush.len(), 1);

    let late_event_err = engine
        .observe(
            &json!({"customer_id": "cust-1", "region": "west", "amount": 20.0}),
            30_000,
        )
        .expect_err("late event should be rejected");
    assert!(late_event_err
        .to_string()
        .contains("late event for flushed window"));

    engine
        .observe(
            &json!({"customer_id": "cust-1", "region": "west", "amount": 5.0}),
            61_000,
        )
        .expect("next-window event should still be accepted");

    let second_flush = engine
        .flush_expired(120_000)
        .expect("second window should flush once");
    assert_eq!(second_flush.len(), 1);
    assert_eq!(
        second_flush[0].value["metrics"],
        json!({
            "order_count": 1,
            "total_amount": 5.0,
            "avg_amount": 5.0
        })
    );
}

#[test]
fn rejects_duplicate_group_by_names_during_construction() {
    let mut config = test_config();
    config.group_by.push(AggregationGroupBy {
        name: "customer_id".to_string(),
        path: "/customer_id_copy".to_string(),
    });

    let err =
        AggregationEngine::new(config, "orders-metrics-1m".to_string()).expect_err("duplicate");
    assert!(err
        .to_string()
        .contains("duplicate aggregation group_by name 'customer_id'"));
}

#[test]
fn rejects_duplicate_metric_names_during_construction() {
    let mut config = test_config();
    config.metrics.push(AggregationMetricConfig {
        name: "order_count".to_string(),
        op: AggregationOp::Count,
        path: None,
        percentiles: None,
    });

    let err =
        AggregationEngine::new(config, "orders-metrics-1m".to_string()).expect_err("duplicate");
    assert!(err
        .to_string()
        .contains("duplicate aggregation metric name 'order_count'"));
}

#[test]
fn flush_expired_keeps_window_state_when_emission_build_fails() {
    let mut engine = AggregationEngine::new(count_sum_config(), "orders-metrics-1m".to_string())
        .expect("engine");

    engine
        .observe(&json!({"customer_id": "cust-1", "amount": 1e308}), 1_000)
        .expect("first large value should aggregate");
    engine
        .observe(&json!({"customer_id": "cust-1", "amount": 1e308}), 2_000)
        .expect("second large value should aggregate");

    let first_err = engine
        .flush_expired(60_000)
        .expect_err("non-finite aggregate should fail emission");
    assert!(first_err
        .to_string()
        .contains("produced a non-finite value"));

    let second_err = engine
        .flush_expired(60_000)
        .expect_err("window state should remain after failed emission");
    assert!(second_err
        .to_string()
        .contains("produced a non-finite value"));
}
