use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use streamforge::{
    MessageEnvelope, WasmAbiVersion, WasmConfig, WasmErrorKind, WasmModuleConfig, WasmRegistry,
    WasmRuntimeConfig, WasmWorld,
};

const FILTER_MODULE: &str = "benchmark-filter";
const VALUE_MODULE: &str = "benchmark-value";
const ENVELOPE_MODULE: &str = "benchmark-envelope";
const PAYLOAD_SIZES: [(&str, usize); 3] = [("256b", 256), ("4kib", 4 * 1024), ("64kib", 64 * 1024)];

fn fixture_root() -> PathBuf {
    std::env::var_os("STREAMFORGE_WASM_BENCH_FIXTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wasm/generated")
        })
}

fn fixture_config() -> WasmConfig {
    let module_root = fixture_root();
    let manifest_path = module_root.join("SHA256SUMS");
    let manifest = fs::read_to_string(&manifest_path).unwrap_or_else(|failure| {
        panic!(
            "cannot read WASM fixture digest manifest {}: {failure}",
            manifest_path.display()
        )
    });
    let digests = manifest
        .lines()
        .map(|line| {
            let mut fields = line.split_whitespace();
            let digest = fields.next().expect("fixture digest must be present");
            let file = fields.next().expect("fixture file name must be present");
            assert!(fields.next().is_none(), "unexpected SHA256SUMS fields");
            (file.to_string(), digest.to_string())
        })
        .collect::<HashMap<_, _>>();
    let module = |name: &str, file: &str, world| WasmModuleConfig {
        name: name.to_string(),
        path: file.into(),
        sha256: digests
            .get(file)
            .unwrap_or_else(|| panic!("{file} is missing from {}", manifest_path.display()))
            .clone(),
        world,
        abi: WasmAbiVersion::V1,
    };
    // Avoid measuring scheduler stalls as guest timeouts in success-path
    // microbenchmarks. The dedicated timeout benchmark uses a 1 ms budget.
    let runtime = WasmRuntimeConfig {
        max_execution_ms: 100,
        ..WasmRuntimeConfig::default()
    };
    WasmConfig {
        module_root,
        runtime,
        modules: vec![
            module(FILTER_MODULE, "filter-v1.wasm", WasmWorld::Filter),
            module(
                VALUE_MODULE,
                "value-transform-v1.wasm",
                WasmWorld::ValueTransform,
            ),
            module(
                ENVELOPE_MODULE,
                "envelope-transform-v1.wasm",
                WasmWorld::EnvelopeTransform,
            ),
        ],
    }
}

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
    assert_eq!(payload.len(), target_bytes);
    payload
}

fn benchmark_envelope(payload: &[u8]) -> MessageEnvelope {
    let value = serde_json::from_slice(payload).expect("benchmark payload must be valid JSON");
    let mut envelope = MessageEnvelope::new(value);
    envelope.key = Some(serde_json::json!("order-123"));
    envelope.topic = Some("orders".to_string());
    envelope.partition = Some(3);
    envelope.offset = Some(42);
    envelope.timestamp = Some(1_700_000_000_000);
    std::sync::Arc::make_mut(&mut envelope.headers)
        .insert("content-type".to_string(), b"application/json".to_vec());
    envelope
}

fn startup_load_compile(c: &mut Criterion) {
    let config = fixture_config();
    let mut group = c.benchmark_group("wasm_udf/startup");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_secs(2));
    group.bench_function("load_verify_compile_link_probe_and_shutdown", |b| {
        b.iter(|| black_box(WasmRegistry::load(black_box(&config)).expect("load fixture registry")))
    });
    group.finish();
}

fn native_no_udf_control(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm_udf/native_no_udf");
    group.sample_size(20);
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(750));

    for (size_name, target_bytes) in PAYLOAD_SIZES {
        let payload = synthetic_payload(target_bytes);
        group.throughput(Throughput::Bytes(payload.len() as u64));
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
    }
    group.finish();
}

fn udf_invocations(c: &mut Criterion) {
    let registry = WasmRegistry::load(&fixture_config()).expect("load fixture registry");
    let mut group = c.benchmark_group("wasm_udf/invoke");
    group.sample_size(20);
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(750));

    for (size_name, target_bytes) in PAYLOAD_SIZES {
        let payload = synthetic_payload(target_bytes);
        let value: Value = serde_json::from_slice(&payload).unwrap();
        let envelope = benchmark_envelope(&payload);
        group.throughput(Throughput::Bytes(payload.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("filter/one_destination", size_name),
            &envelope,
            |b, envelope| {
                b.iter(|| {
                    black_box(
                        registry
                            .invoke_filter(FILTER_MODULE, black_box(envelope))
                            .unwrap(),
                    )
                });
            },
        );
        group.throughput(Throughput::Bytes((payload.len() * 4) as u64));
        group.bench_with_input(
            BenchmarkId::new("filter/four_destinations", size_name),
            &envelope,
            |b, envelope| {
                b.iter(|| {
                    for _ in 0..4 {
                        black_box(
                            registry
                                .invoke_filter(FILTER_MODULE, black_box(envelope))
                                .unwrap(),
                        );
                    }
                });
            },
        );
        group.throughput(Throughput::Bytes(payload.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("value_transform/one_destination", size_name),
            &value,
            |b, value| {
                b.iter(|| {
                    black_box(
                        registry
                            .invoke_value_transform(VALUE_MODULE, black_box(value))
                            .unwrap(),
                    )
                });
            },
        );
        group.throughput(Throughput::Bytes((payload.len() * 4) as u64));
        group.bench_with_input(
            BenchmarkId::new("value_transform/four_destinations", size_name),
            &value,
            |b, value| {
                b.iter(|| {
                    for _ in 0..4 {
                        black_box(
                            registry
                                .invoke_value_transform(VALUE_MODULE, black_box(value))
                                .unwrap(),
                        );
                    }
                });
            },
        );
        group.throughput(Throughput::Bytes(payload.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("envelope_transform/one_destination", size_name),
            &envelope,
            |b, envelope| {
                b.iter(|| {
                    black_box(
                        registry
                            .invoke_envelope_transform(ENVELOPE_MODULE, black_box(envelope))
                            .unwrap(),
                    )
                });
            },
        );
        group.throughput(Throughput::Bytes((payload.len() * 4) as u64));
        group.bench_with_input(
            BenchmarkId::new("envelope_transform/four_destinations", size_name),
            &envelope,
            |b, envelope| {
                b.iter(|| {
                    for _ in 0..4 {
                        black_box(
                            registry
                                .invoke_envelope_transform(ENVELOPE_MODULE, black_box(envelope))
                                .unwrap(),
                        );
                    }
                });
            },
        );
    }
    group.finish();
}

fn udf_failure_paths(c: &mut Criterion) {
    let registry = WasmRegistry::load(&fixture_config()).expect("load fixture registry");
    let guest_error = MessageEnvelope::new(serde_json::json!({"guest_error": true}));
    let guest_error_value = serde_json::json!({"guest_error": true});
    assert_eq!(
        registry
            .invoke_filter(FILTER_MODULE, &guest_error)
            .unwrap_err()
            .kind(),
        WasmErrorKind::Guest
    );
    assert_eq!(
        registry
            .invoke_value_transform(VALUE_MODULE, &guest_error_value)
            .unwrap_err()
            .kind(),
        WasmErrorKind::Guest
    );

    let mut output_limit_config = fixture_config();
    output_limit_config.runtime.max_output_bytes = 1024;
    let output_limit_registry =
        WasmRegistry::load(&output_limit_config).expect("load output-limit fixture registry");
    let oversized_output = serde_json::json!({"large": true});
    assert_eq!(
        output_limit_registry
            .invoke_value_transform(VALUE_MODULE, &oversized_output)
            .unwrap_err()
            .kind(),
        WasmErrorKind::OutputLimit
    );

    let mut group = c.benchmark_group("wasm_udf/failure");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_millis(400));
    group.bench_function("filter_guest_error", |b| {
        b.iter(|| {
            black_box(
                registry
                    .invoke_filter(FILTER_MODULE, black_box(&guest_error))
                    .unwrap_err(),
            )
        });
    });
    group.bench_function("value_transform_guest_error", |b| {
        b.iter(|| {
            black_box(
                registry
                    .invoke_value_transform(VALUE_MODULE, black_box(&guest_error_value))
                    .unwrap_err(),
            )
        });
    });
    group.bench_function("value_transform_output_limit", |b| {
        b.iter(|| {
            black_box(
                output_limit_registry
                    .invoke_value_transform(VALUE_MODULE, black_box(&oversized_output))
                    .unwrap_err(),
            )
        });
    });
    group.finish();
}

fn udf_timeout_path(c: &mut Criterion) {
    let mut config = fixture_config();
    config.runtime.max_execution_ms = 1;
    config.runtime.epoch_tick_ms = 1;
    let registry = WasmRegistry::load(&config).expect("load timeout fixture registry");
    let looping = MessageEnvelope::new(serde_json::json!({"loop": true}));
    assert_eq!(
        registry
            .invoke_filter(FILTER_MODULE, &looping)
            .unwrap_err()
            .kind(),
        WasmErrorKind::Timeout
    );

    let mut group = c.benchmark_group("wasm_udf/failure");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(25));
    group.measurement_time(Duration::from_millis(150));
    group.bench_function("filter_timeout_1ms_budget", |b| {
        b.iter(|| {
            black_box(
                registry
                    .invoke_filter(FILTER_MODULE, black_box(&looping))
                    .unwrap_err(),
            )
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    startup_load_compile,
    native_no_udf_control,
    udf_invocations,
    udf_failure_paths,
    udf_timeout_path
);
criterion_main!(benches);
