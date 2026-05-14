use serde_json::json;
use streamforge::{
    aggregation::AggregationEngine, AggregationConfig, AggregationMetricConfig, AggregationOp,
    AggregationWindowConfig, AggregationWindowType,
};

#[test]
fn approx_distinct_stays_within_reasonable_error_band() {
    let spec = AggregationConfig {
        group_by: vec![],
        window: AggregationWindowConfig {
            window_type: AggregationWindowType::Tumbling,
            size_seconds: 60,
            emit_interval_seconds: 5,
        },
        metrics: vec![AggregationMetricConfig {
            name: "unique_customers".to_string(),
            op: AggregationOp::ApproxDistinct,
            path: Some("/customer_id".to_string()),
            percentiles: None,
        }],
    };

    let mut engine =
        AggregationEngine::new(spec, "orders-metrics-1m".to_string()).expect("engine builds");

    for i in 0..100 {
        engine
            .observe(
                &json!({"customer_id": format!("customer-{i}")}),
                (i as u64) * 100,
            )
            .expect("event should aggregate");
    }

    let emitted = engine.flush_expired(61_000).expect("window should flush");
    let estimate = emitted[0].value["metrics"]["unique_customers"]
        .as_f64()
        .expect("estimate should be numeric");

    assert!(
        (90.0..=110.0).contains(&estimate),
        "approx_distinct estimate out of bounds: {estimate}"
    );
}

#[test]
fn quantiles_emit_requested_percentiles() {
    let spec = AggregationConfig {
        group_by: vec![],
        window: AggregationWindowConfig {
            window_type: AggregationWindowType::Tumbling,
            size_seconds: 60,
            emit_interval_seconds: 5,
        },
        metrics: vec![AggregationMetricConfig {
            name: "amount_quantiles".to_string(),
            op: AggregationOp::Quantiles,
            path: Some("/amount".to_string()),
            percentiles: Some(vec![0.5, 0.95, 0.3333333]),
        }],
    };

    let mut engine =
        AggregationEngine::new(spec, "orders-metrics-1m".to_string()).expect("engine builds");

    for amount in 1..=100 {
        engine
            .observe(&json!({"amount": amount as f64}), amount as u64)
            .expect("event should aggregate");
    }

    let emitted = engine.flush_expired(61_000).expect("window should flush");
    let quantiles = &emitted[0].value["metrics"]["amount_quantiles"];
    let p50 = quantiles["p0.5"].as_f64().expect("p0.5 should be numeric");
    let p95 = quantiles["p0.95"]
        .as_f64()
        .expect("p0.95 should be numeric");
    let p3333333 = quantiles["p0.3333333"]
        .as_f64()
        .expect("p0.3333333 should be numeric");

    assert!((p50 - 50.0).abs() <= 5.0, "unexpected p50: {p50}");
    assert!((p95 - 95.0).abs() <= 8.0, "unexpected p95: {p95}");
    assert!(
        (p3333333 - 33.0).abs() <= 6.0,
        "unexpected p0.3333333: {p3333333}"
    );
}
