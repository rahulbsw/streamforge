use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use serde_json::Value;
use streamforge::filter::{Filter, JsonPathFilter, JsonPathTransform, RegexFilter, Transform};
use streamforge::MessageEnvelope;

const PAYLOAD_SIZES: [(&str, usize); 3] = [("256b", 256), ("4kib", 4 * 1024), ("64kib", 64 * 1024)];

/// Build valid JSON at the requested size while retaining fields used by the
/// filter, regex, and transform stages.
fn synthetic_payload(target_bytes: usize) -> Vec<u8> {
    const PREFIX: &str = concat!(
        r#"{"user":{"id":"user-123","email":"alice@example.com","active":true},"#,
        r#""event":{"kind":"order.created"},"padding":""#
    );
    const SUFFIX: &str = "\"}";

    assert!(target_bytes >= PREFIX.len() + SUFFIX.len());

    let mut payload = Vec::with_capacity(target_bytes);
    payload.extend_from_slice(PREFIX.as_bytes());
    payload.resize(target_bytes - SUFFIX.len(), b'x');
    payload.extend_from_slice(SUFFIX.as_bytes());
    debug_assert_eq!(payload.len(), target_bytes);
    payload
}

/// Synthetic, in-memory coverage of the CPU and allocation stages around the
/// current JSON envelope pipeline. Kafka/network latency is intentionally
/// excluded.
fn synthetic_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("synthetic_pipeline");
    group.sample_size(10);
    group.warm_up_time(std::time::Duration::from_millis(200));
    group.measurement_time(std::time::Duration::from_millis(500));

    let path_filter = JsonPathFilter::new("/user/active", "==", "true").unwrap();
    let regex_filter =
        RegexFilter::new("/user/email", r"^[a-z]+(?:\.[a-z]+)*@example\.com$").unwrap();
    let transform = JsonPathTransform::new("/user/email").unwrap();

    for (size_name, target_bytes) in PAYLOAD_SIZES {
        let payload = synthetic_payload(target_bytes);
        let parsed: Value = serde_json::from_slice(&payload).unwrap();
        let envelope = MessageEnvelope::new(parsed.clone());

        group.throughput(Throughput::Bytes(payload.len() as u64));

        // Input allocation is excluded: this measures JSON bytes -> Value.
        group.bench_with_input(
            BenchmarkId::new("parse_bytes", size_name),
            &payload,
            |b, bytes| {
                b.iter(|| {
                    black_box(serde_json::from_slice::<Value>(black_box(bytes.as_slice())).unwrap())
                });
            },
        );

        // Value construction is excluded: this measures Value -> JSON bytes.
        group.bench_with_input(
            BenchmarkId::new("serialize_value", size_name),
            &parsed,
            |b, value| {
                b.iter(|| black_box(serde_json::to_vec(black_box(value)).unwrap()));
            },
        );

        // Input allocation is excluded; both parse and serialization are timed.
        group.bench_with_input(
            BenchmarkId::new("parse_serialize_round_trip", size_name),
            &payload,
            |b, bytes| {
                b.iter(|| {
                    let value: Value = serde_json::from_slice(black_box(bytes.as_slice())).unwrap();
                    black_box(serde_json::to_vec(&value).unwrap())
                });
            },
        );

        // Models current one-destination ingestion preparation. Payload creation
        // is excluded; JSON parsing and envelope allocation are included.
        group.bench_with_input(
            BenchmarkId::new("one_destination_passthrough_preparation", size_name),
            &payload,
            |b, bytes| {
                b.iter(|| {
                    let value: Value = serde_json::from_slice(black_box(bytes.as_slice())).unwrap();
                    black_box(MessageEnvelope::new(value))
                });
            },
        );

        // Fan-out reports destination operations. Envelope construction is
        // excluded; each iteration times four Arc-backed envelope clones.
        group.throughput(Throughput::Elements(4));
        group.bench_with_input(
            BenchmarkId::new("four_destination_arc_fan_out", size_name),
            &envelope,
            |b, envelope| {
                b.iter(|| {
                    black_box((
                        envelope.clone(),
                        envelope.clone(),
                        envelope.clone(),
                        envelope.clone(),
                    ))
                });
            },
        );

        group.throughput(Throughput::Elements(1));

        // Filter construction and JSON parsing are excluded.
        group.bench_with_input(
            BenchmarkId::new("one_path_filter", size_name),
            &envelope,
            |b, envelope| {
                b.iter(|| black_box(path_filter.evaluate_envelope(black_box(envelope)).unwrap()));
            },
        );

        // Regex compilation and JSON parsing are excluded.
        group.bench_with_input(
            BenchmarkId::new("one_regex_filter", size_name),
            &envelope,
            |b, envelope| {
                b.iter(|| black_box(regex_filter.evaluate_envelope(black_box(envelope)).unwrap()));
            },
        );

        // Transform consumes an owned Value. iter_batched performs the required
        // deep clone as untimed setup, isolating the transform stage itself.
        group.bench_with_input(
            BenchmarkId::new("one_transform", size_name),
            &parsed,
            |b, value| {
                b.iter_batched(
                    || value.clone(),
                    |owned| black_box(transform.transform(black_box(owned)).unwrap()),
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(benches, synthetic_pipeline);
criterion_main!(benches);
