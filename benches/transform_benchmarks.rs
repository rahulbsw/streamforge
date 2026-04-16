use criterion::{criterion_group, criterion_main, Criterion};
use serde_json::json;
use streamforge::rhai_dsl::RhaiEngine;
use streamforge::MessageEnvelope;

fn make_engine() -> RhaiEngine {
    RhaiEngine::new(None)
}

pub fn transform_benchmarks(c: &mut Criterion) {
    let e = make_engine();

    // Field extraction
    let t_extract = e.compile_transform(r#"msg["user"]"#).unwrap();
    let env_extract = MessageEnvelope::new(json!({"user": {"id": "u1", "name": "Alice"}}));
    c.bench_function("transform/field_extract", |b| {
        b.iter(|| t_extract.transform(&env_extract).unwrap())
    });

    // Object construction — 2 fields
    let t_construct2 = e
        .compile_transform(r#"#{ id: msg["userId"], email: msg["email"] }"#)
        .unwrap();
    let env_c2 = MessageEnvelope::new(json!({"userId": "u1", "email": "a@b.com"}));
    c.bench_function("transform/construct_2_fields", |b| {
        b.iter(|| t_construct2.transform(&env_c2).unwrap())
    });

    // Object construction — 4 fields
    let t_construct4 = e
        .compile_transform(
            r#"#{ id: msg["userId"], email: msg["email"], name: msg["name"], tier: msg["tier"] }"#,
        )
        .unwrap();
    let env_c4 = MessageEnvelope::new(
        json!({"userId": "u1", "email": "a@b.com", "name": "Alice", "tier": "premium"}),
    );
    c.bench_function("transform/construct_4_fields", |b| {
        b.iter(|| t_construct4.transform(&env_c4).unwrap())
    });

    // String lower
    let t_lower = e
        .compile_transform(r#"msg + #{ email: msg["email"].to_lower() }"#)
        .unwrap();
    let env_lower = MessageEnvelope::new(json!({"email": "USER@EXAMPLE.COM"}));
    c.bench_function("transform/string_lower", |b| {
        b.iter(|| t_lower.transform(&env_lower).unwrap())
    });

    // Arithmetic
    let t_arith = e.compile_transform(r#"msg["price"] * 1.08"#).unwrap();
    let env_arith = MessageEnvelope::new(json!({"price": 100.0}));
    c.bench_function("transform/arithmetic", |b| {
        b.iter(|| t_arith.transform(&env_arith).unwrap())
    });

    // if/else (conditional)
    let t_if = e
        .compile_transform(
            r#"if msg["score"] > 90 { "A" } else if msg["score"] > 80 { "B" } else { "C" }"#,
        )
        .unwrap();
    let env_if = MessageEnvelope::new(json!({"score": 85}));
    c.bench_function("transform/if_else", |b| {
        b.iter(|| t_if.transform(&env_if).unwrap())
    });

    // switch/CASE
    let t_switch = e
        .compile_transform(
            r#"switch msg["tier"] { "premium" => "GOLD", "basic" => "SILVER", _ => "BRONZE" }"#,
        )
        .unwrap();
    let env_switch = MessageEnvelope::new(json!({"tier": "premium"}));
    c.bench_function("transform/switch_case", |b| {
        b.iter(|| t_switch.transform(&env_switch).unwrap())
    });

    // Coalesce (??)
    let t_coalesce = e
        .compile_transform(r#"msg["preferredName"] ?? msg["displayName"] ?? msg["email"]"#)
        .unwrap();
    let env_coal = MessageEnvelope::new(json!({"displayName": "Alice Smith", "email": "a@b.com"}));
    c.bench_function("transform/coalesce", |b| {
        b.iter(|| t_coalesce.transform(&env_coal).unwrap())
    });

    // Array map
    let t_arr = e
        .compile_transform(r#"msg["users"].map(|u| u["id"])"#)
        .unwrap();
    let env_arr = MessageEnvelope::new(json!({
        "users": [{"id": 1, "name": "A"}, {"id": 2, "name": "B"}, {"id": 3, "name": "C"}]
    }));
    c.bench_function("transform/array_map", |b| {
        b.iter(|| t_arr.transform(&env_arr).unwrap())
    });

    // Multi-statement script
    let t_multi = e
        .compile_transform(
            r#"
            let lower = msg["email"].to_lower();
            let domain = lower.split("@")[1];
            #{ email: lower, domain: domain }
            "#,
        )
        .unwrap();
    let env_multi = MessageEnvelope::new(json!({"email": "User@Example.com"}));
    c.bench_function("transform/multistatement", |b| {
        b.iter(|| t_multi.transform(&env_multi).unwrap())
    });

    // Envelope-aware (reads key in transform)
    let t_env = e
        .compile_transform(
            r#"if key.starts_with("vip-") { msg + #{ tier: "premium" } } else { msg }"#,
        )
        .unwrap();
    let mut env_env = MessageEnvelope::new(json!({"name": "Alice"}));
    env_env.key = Some(json!("vip-user1"));
    c.bench_function("transform/envelope_aware", |b| {
        b.iter(|| t_env.transform(&env_env).unwrap())
    });
}

criterion_group!(benches, transform_benchmarks);
criterion_main!(benches);
