#[path = "fixtures/wasm/support.rs"]
mod support;

use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use streamforge::{MessageEnvelope, WasmErrorKind, WasmRegistry, WasmRuntimeConfig, WasmWorld};

#[test]
fn invokes_all_three_v1_worlds_and_preserves_host_provenance() {
    let registry = WasmRegistry::load(&support::default_config(vec![
        support::module("filter", "filter-v1.wasm", WasmWorld::Filter),
        support::module(
            "value",
            "value-transform-v1.wasm",
            WasmWorld::ValueTransform,
        ),
        support::module(
            "envelope",
            "envelope-transform-v1.wasm",
            WasmWorld::EnvelopeTransform,
        ),
    ]))
    .unwrap();
    let envelope = MessageEnvelope::new(json!({"allow": true}))
        .key(json!("key"))
        .with_header_str("existing".to_string(), "header")
        .timestamp(41)
        .source("source-topic".to_string(), 3, 99);

    assert!(registry.invoke_filter("filter", &envelope).unwrap());
    assert!(!registry
        .invoke_filter("filter", &MessageEnvelope::new(json!({"allow": false})))
        .unwrap());
    assert_eq!(
        registry
            .invoke_value_transform("value", &json!({"tag": "alpha"}))
            .unwrap(),
        json!({"tag": "alpha"})
    );

    let output = registry
        .invoke_envelope_transform("envelope", &envelope)
        .unwrap();
    assert_eq!(output.key, envelope.key);
    assert_eq!(output.value, *envelope.value);
    assert_eq!(
        output.headers.get("x-streamforge-wasm"),
        Some(&b"ok".to_vec())
    );
    assert_eq!(output.timestamp, Some(42));
    assert_eq!(envelope.topic.as_deref(), Some("source-topic"));
    assert_eq!(envelope.partition, Some(3));
    assert_eq!(envelope.offset, Some(99));
}

#[test]
fn classifies_structured_guest_errors_without_payload_echo() {
    let registry = WasmRegistry::load(&support::default_config(vec![support::module(
        "filter",
        "filter-v1.wasm",
        WasmWorld::Filter,
    )]))
    .unwrap();
    let error = registry
        .invoke_filter(
            "filter",
            &MessageEnvelope::new(json!({
                "echo_error": true,
                "secret_payload": "must-not-appear"
            })),
        )
        .unwrap_err();

    assert_eq!(error.kind(), WasmErrorKind::Guest);
    assert_eq!(error.module(), Some("filter"));
    assert!(error.message().contains("Rejected"));
    assert!(!error.message().contains("must-not-appear"));
    assert!(!error.message().contains("echo_error"));
}

#[test]
fn rejects_declared_world_mismatch_and_wrong_invocation_world() {
    let declared_mismatch = support::load_error(&support::default_config(vec![support::module(
        "wrong",
        "value-transform-v1.wasm",
        WasmWorld::Filter,
    )]));
    assert_eq!(declared_mismatch.kind(), WasmErrorKind::Abi);

    let registry = WasmRegistry::load(&support::default_config(vec![support::module(
        "value",
        "value-transform-v1.wasm",
        WasmWorld::ValueTransform,
    )]))
    .unwrap();
    let invocation_mismatch = registry
        .invoke_filter("value", &MessageEnvelope::new(json!({"id": 1})))
        .unwrap_err();
    assert_eq!(invocation_mismatch.kind(), WasmErrorKind::WorldMismatch);
}

#[test]
fn rejects_components_with_unresolved_host_imports() {
    let error = support::load_error(&support::default_config(vec![support::module(
        "importing",
        "forbidden-import.wasm",
        WasmWorld::Filter,
    )]));

    assert_eq!(error.kind(), WasmErrorKind::Abi);
    assert!(error.message().contains("imports are not permitted"));
}

#[test]
fn interrupts_an_infinite_guest_loop() {
    let runtime = WasmRuntimeConfig {
        max_execution_ms: 5,
        epoch_tick_ms: 1,
        ..WasmRuntimeConfig::default()
    };
    let registry = WasmRegistry::load(&support::config(
        vec![support::module(
            "filter",
            "filter-v1.wasm",
            WasmWorld::Filter,
        )],
        runtime,
    ))
    .unwrap();
    let error = registry
        .invoke_filter("filter", &MessageEnvelope::new(json!({"loop": true})))
        .unwrap_err();

    assert_eq!(error.kind(), WasmErrorKind::Timeout);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn epoch_interrupt_preserves_scheduler_liveness_when_workers_run_looping_guests() {
    let runtime = WasmRuntimeConfig {
        max_execution_ms: 20,
        epoch_tick_ms: 1,
        ..WasmRuntimeConfig::default()
    };
    let registry = Arc::new(
        WasmRegistry::load(&support::config(
            vec![support::module(
                "filter",
                "filter-v1.wasm",
                WasmWorld::Filter,
            )],
            runtime,
        ))
        .unwrap(),
    );

    let (heartbeat_sender, heartbeat_receiver) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = heartbeat_sender.send(());
    });
    tokio::task::yield_now().await;

    let loops = (0..2)
        .map(|_| {
            let registry = registry.clone();
            tokio::spawn(async move {
                registry.invoke_filter("filter", &MessageEnvelope::new(json!({"loop": true})))
            })
        })
        .collect::<Vec<_>>();

    tokio::time::timeout(Duration::from_secs(2), heartbeat_receiver)
        .await
        .expect("heartbeat stalled while synchronous guests occupied workers")
        .expect("heartbeat task was canceled");

    for guest in loops {
        let error = guest.await.unwrap().unwrap_err();
        assert_eq!(error.kind(), WasmErrorKind::Timeout);
    }
}

#[test]
fn rejects_output_over_the_configured_limit_before_json_decode() {
    let runtime = WasmRuntimeConfig {
        max_output_bytes: 64,
        ..WasmRuntimeConfig::default()
    };
    let registry = WasmRegistry::load(&support::config(
        vec![support::module(
            "value",
            "value-transform-v1.wasm",
            WasmWorld::ValueTransform,
        )],
        runtime,
    ))
    .unwrap();
    let error = registry
        .invoke_value_transform("value", &json!({"large": true}))
        .unwrap_err();

    assert_eq!(error.kind(), WasmErrorKind::OutputLimit);
}

#[test]
fn rejects_input_over_the_configured_limit_before_guest_invocation() {
    let runtime = WasmRuntimeConfig {
        max_input_bytes: 16,
        ..WasmRuntimeConfig::default()
    };
    let registry = WasmRegistry::load(&support::config(
        vec![support::module(
            "filter",
            "filter-v1.wasm",
            WasmWorld::Filter,
        )],
        runtime,
    ))
    .unwrap();
    let error = registry
        .invoke_filter(
            "filter",
            &MessageEnvelope::new(json!({"payload": "larger-than-sixteen-bytes"})),
        )
        .unwrap_err();

    assert_eq!(error.kind(), WasmErrorKind::InputLimit);
}

#[test]
fn rejects_components_exceeding_memory_and_table_pool_limits() {
    let memory_error = support::load_error(&support::config(
        vec![support::module(
            "filter",
            "filter-v1.wasm",
            WasmWorld::Filter,
        )],
        WasmRuntimeConfig {
            max_memory_bytes: 64 * 1024,
            ..WasmRuntimeConfig::default()
        },
    ));
    assert_eq!(
        memory_error.kind(),
        WasmErrorKind::ResourceLimit,
        "{}",
        memory_error.message()
    );

    let table_error = support::load_error(&support::config(
        vec![support::module(
            "filter",
            "filter-v1.wasm",
            WasmWorld::Filter,
        )],
        WasmRuntimeConfig {
            max_table_elements: 1,
            ..WasmRuntimeConfig::default()
        },
    ));
    assert_eq!(
        table_error.kind(),
        WasmErrorKind::ResourceLimit,
        "{}",
        table_error.message()
    );
}

#[test]
fn classifies_guest_stack_exhaustion_as_a_resource_limit() {
    let runtime = WasmRuntimeConfig {
        max_wasm_stack_bytes: 64 * 1024,
        // Leave enough epoch budget for the configured stack ceiling, rather
        // than the execution deadline, to be the first resource exhausted.
        max_execution_ms: 1_000,
        ..WasmRuntimeConfig::default()
    };
    let registry = WasmRegistry::load(&support::config(
        vec![support::module(
            "filter",
            "filter-v1.wasm",
            WasmWorld::Filter,
        )],
        runtime,
    ))
    .unwrap();
    let error = registry
        .invoke_filter("filter", &MessageEnvelope::new(json!({"stack": true})))
        .unwrap_err();

    assert_eq!(
        error.kind(),
        WasmErrorKind::ResourceLimit,
        "{}",
        error.message()
    );
}

#[test]
fn fresh_instances_reset_mutable_guest_state() {
    let registry = WasmRegistry::load(&support::default_config(vec![support::module(
        "filter",
        "filter-v1.wasm",
        WasmWorld::Filter,
    )]))
    .unwrap();
    let envelope = MessageEnvelope::new(json!({"state_probe": true}));

    assert!(registry.invoke_filter("filter", &envelope).unwrap());
    assert!(registry.invoke_filter("filter", &envelope).unwrap());
}

#[test]
fn pooling_resources_are_reclaimed_across_many_sequential_invocations() {
    let registry = WasmRegistry::load(&support::default_config(vec![support::module(
        "filter",
        "filter-v1.wasm",
        WasmWorld::Filter,
    )]))
    .unwrap();
    let envelope = MessageEnvelope::new(json!({"allow": true}));

    for _ in 0..4_096 {
        assert!(registry.invoke_filter("filter", &envelope).unwrap());
    }
}

#[test]
fn concurrent_invocations_do_not_cross_contaminate_input_tags() {
    let registry = Arc::new(
        WasmRegistry::load(&support::config(
            vec![support::module(
                "value",
                "value-transform-v1.wasm",
                WasmWorld::ValueTransform,
            )],
            WasmRuntimeConfig {
                max_execution_ms: 1_000,
                ..WasmRuntimeConfig::default()
            },
        ))
        .unwrap(),
    );
    let handles = (0..16)
        .map(|tag| {
            let registry = registry.clone();
            std::thread::spawn(move || {
                let input = json!({"tag": tag, "payload": vec![tag; 8]});
                let output = registry.invoke_value_transform("value", &input).unwrap();
                (input, output)
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        let (input, output) = handle.join().unwrap();
        assert_eq!(output, input);
    }
}
