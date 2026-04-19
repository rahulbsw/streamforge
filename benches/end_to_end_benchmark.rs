use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::json;
use std::sync::Arc;
use streamforge::filter::{Filter, JsonPathFilter, JsonPathTransform, Transform};
use streamforge::MessageEnvelope;

/// Benchmark end-to-end message processing pipeline
fn benchmark_processing_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");

    // Test message
    let test_message = json!({
        "user": {
            "id": "user-123",
            "email": "test@example.com",
            "age": 30,
            "active": true
        },
        "order": {
            "id": "order-456",
            "amount": 99.99,
            "status": "completed"
        }
    });

    // Single filter + transform (common case)
    group.bench_function("filter_transform_single", |b| {
        let filter = JsonPathFilter::new("/user/active", "==", "true").unwrap();
        let transform = JsonPathTransform::new("/user/email").unwrap();

        b.iter(|| {
            let envelope = MessageEnvelope::new(test_message.clone());
            let passes = filter.evaluate(&envelope.value).unwrap();
            if passes {
                let _ = transform
                    .transform(Arc::try_unwrap(envelope.value).unwrap_or_else(|arc| (*arc).clone()))
                    .unwrap();
            }
        });
    });

    // Multi-destination simulation (4 destinations)
    group.bench_function("multi_destination_4", |b| {
        let filters: Vec<Arc<dyn Filter>> = vec![
            Arc::new(JsonPathFilter::new("/user/active", "==", "true").unwrap()),
            Arc::new(JsonPathFilter::new("/order/status", "==", "completed").unwrap()),
            Arc::new(JsonPathFilter::new("/user/age", ">", "18").unwrap()),
            Arc::new(JsonPathFilter::new("/order/amount", ">", "50").unwrap()),
        ];

        b.iter(|| {
            let envelope = MessageEnvelope::new(test_message.clone());

            // Simulate multi-destination processing with cheap Arc clones
            for filter in &filters {
                let env_clone = envelope.clone();
                let _ = black_box(filter.evaluate(&env_clone.value).unwrap());
            }
        });
    });

    // Heavy JSON path extraction (pre-parsing benefit)
    group.bench_function("heavy_jsonpath_extraction", |b| {
        let paths = vec![
            JsonPathTransform::new("/user/id").unwrap(),
            JsonPathTransform::new("/user/email").unwrap(),
            JsonPathTransform::new("/user/age").unwrap(),
            JsonPathTransform::new("/order/id").unwrap(),
            JsonPathTransform::new("/order/amount").unwrap(),
            JsonPathTransform::new("/order/status").unwrap(),
        ];

        b.iter(|| {
            let envelope = MessageEnvelope::new(test_message.clone());
            let value = Arc::try_unwrap(envelope.value).unwrap_or_else(|arc| (*arc).clone());

            for path in &paths {
                let _ = black_box(path.transform(value.clone()).unwrap());
            }
        });
    });

    group.finish();
}

/// Benchmark envelope cloning performance (Arc benefit)
fn benchmark_envelope_cloning(c: &mut Criterion) {
    let mut group = c.benchmark_group("envelope_cloning");

    // Small message (1KB)
    let small_msg = json!({
        "id": "test-123",
        "value": "x".repeat(900)
    });

    // Large message (10KB)
    let large_msg = json!({
        "id": "test-456",
        "data": "x".repeat(9900)
    });

    for (name, msg) in [("small_1kb", small_msg), ("large_10kb", large_msg)] {
        group.bench_with_input(BenchmarkId::new("clone", name), &msg, |b, msg| {
            let envelope = MessageEnvelope::new(msg.clone());
            b.iter(|| {
                let _ = black_box(envelope.clone());
            });
        });

        group.bench_with_input(BenchmarkId::new("clone_4x", name), &msg, |b, msg| {
            let envelope = MessageEnvelope::new(msg.clone());
            b.iter(|| {
                // Simulate multi-destination (4 clones)
                let e1 = envelope.clone();
                let e2 = envelope.clone();
                let e3 = envelope.clone();
                let e4 = envelope.clone();
                black_box((e1, e2, e3, e4));
            });
        });
    }

    group.finish();
}

/// Benchmark JSON path pre-parsing benefit
fn benchmark_jsonpath_preparsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("jsonpath");

    let test_value = json!({
        "level1": {
            "level2": {
                "level3": {
                    "level4": {
                        "value": "target"
                    }
                }
            }
        }
    });

    // Deep path extraction (benefits most from pre-parsing)
    group.bench_function("deep_path_extraction", |b| {
        let transform = JsonPathTransform::new("/level1/level2/level3/level4/value").unwrap();

        b.iter(|| {
            let _ = black_box(transform.transform(test_value.clone()).unwrap());
        });
    });

    group.finish();
}

/// Benchmark filter evaluation with pre-resolved metrics
fn benchmark_filter_with_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_metrics");

    let test_value = json!({
        "age": 30,
        "status": "active",
        "verified": true
    });

    // Simple filter (pre-resolved metrics benefit)
    group.bench_function("simple_filter_eval", |b| {
        let filter = JsonPathFilter::new("/age", ">", "18").unwrap();

        b.iter(|| {
            let _ = black_box(filter.evaluate(&test_value).unwrap());
        });
    });

    // Complex AND filter
    group.bench_function("and_filter_eval", |b| {
        use streamforge::filter::AndFilter;

        let filter1 = Box::new(JsonPathFilter::new("/age", ">", "18").unwrap());
        let filter2 = Box::new(JsonPathFilter::new("/status", "==", "active").unwrap());
        let and_filter = AndFilter::new(vec![filter1, filter2]);

        b.iter(|| {
            let _ = black_box(and_filter.evaluate(&test_value).unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_processing_pipeline,
    benchmark_envelope_cloning,
    benchmark_jsonpath_preparsing,
    benchmark_filter_with_metrics
);
criterion_main!(benches);
