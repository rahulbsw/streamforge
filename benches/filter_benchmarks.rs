use criterion::{criterion_group, criterion_main, Criterion};
use serde_json::json;
use streamforge::rhai_dsl::RhaiEngine;
use streamforge::MessageEnvelope;

fn make_engine() -> RhaiEngine {
    RhaiEngine::new(None)
}

pub fn filter_benchmarks(c: &mut Criterion) {
    let e = make_engine();

    // Simple equality
    let f_eq = e.compile_filter(r#"msg["status"] == "active""#).unwrap();
    let env_active = MessageEnvelope::new(json!({"status": "active"}));
    c.bench_function("filter/simple_eq", |b| {
        b.iter(|| f_eq.evaluate_envelope(&env_active).unwrap())
    });

    // Numeric comparison
    let f_gt = e.compile_filter(r#"msg["score"] > 80"#).unwrap();
    let env_score = MessageEnvelope::new(json!({"score": 90}));
    c.bench_function("filter/numeric_gt", |b| {
        b.iter(|| f_gt.evaluate_envelope(&env_score).unwrap())
    });

    // AND with 2 conditions
    let f_and2 = e
        .compile_filter(r#"msg["status"] == "active" && msg["score"] > 80"#)
        .unwrap();
    let env_and2 = MessageEnvelope::new(json!({"status": "active", "score": 90}));
    c.bench_function("filter/and_2_conditions", |b| {
        b.iter(|| f_and2.evaluate_envelope(&env_and2).unwrap())
    });

    // AND with 3 conditions
    let f_and3 = e
        .compile_filter(
            r#"msg["status"] == "active" && msg["score"] > 80 && msg["tier"] == "premium""#,
        )
        .unwrap();
    let env_and3 =
        MessageEnvelope::new(json!({"status": "active", "score": 90, "tier": "premium"}));
    c.bench_function("filter/and_3_conditions", |b| {
        b.iter(|| f_and3.evaluate_envelope(&env_and3).unwrap())
    });

    // String contains (regex-like)
    let f_contains = e
        .compile_filter(r#"msg["email"].contains("@company.com")"#)
        .unwrap();
    let env_email = MessageEnvelope::new(json!({"email": "user@company.com"}));
    c.bench_function("filter/string_contains", |b| {
        b.iter(|| f_contains.evaluate_envelope(&env_email).unwrap())
    });

    // is_null_or_empty
    let f_null = e
        .compile_filter(r#"is_null_or_empty(msg["email"])"#)
        .unwrap();
    let env_empty = MessageEnvelope::new(json!({"email": ""}));
    c.bench_function("filter/is_null_or_empty", |b| {
        b.iter(|| f_null.evaluate_envelope(&env_empty).unwrap())
    });

    // Key prefix
    let f_key = e.compile_filter(r#"key.starts_with("premium-")"#).unwrap();
    let mut env_key = MessageEnvelope::new(json!({}));
    env_key.key = Some(json!("premium-user1"));
    c.bench_function("filter/key_starts_with", |b| {
        b.iter(|| f_key.evaluate_envelope(&env_key).unwrap())
    });

    // Header equality
    let f_hdr = e
        .compile_filter(r#"headers["x-tenant"] == "production""#)
        .unwrap();
    let mut env_hdr = MessageEnvelope::new(json!({}));
    env_hdr
        .headers
        .insert("x-tenant".to_string(), b"production".to_vec());
    c.bench_function("filter/header_eq", |b| {
        b.iter(|| f_hdr.evaluate_envelope(&env_hdr).unwrap())
    });

    // in list
    let f_in = e
        .compile_filter(r#"msg["tier"] in ["premium", "enterprise"]"#)
        .unwrap();
    let env_in = MessageEnvelope::new(json!({"tier": "premium"}));
    c.bench_function("filter/in_list", |b| {
        b.iter(|| f_in.evaluate_envelope(&env_in).unwrap())
    });

    // Complex compound
    let f_complex = e
        .compile_filter(
            r#"
            msg["status"] == "active"
            && msg["score"] > 75
            && !is_null_or_empty(msg["email"])
            && msg["tier"] in ["premium", "enterprise"]
            && key.starts_with("user-")
            "#,
        )
        .unwrap();
    let mut env_complex = MessageEnvelope::new(json!({
        "status": "active", "score": 90,
        "email": "user@example.com", "tier": "premium"
    }));
    env_complex.key = Some(json!("user-123"));
    c.bench_function("filter/complex_compound", |b| {
        b.iter(|| f_complex.evaluate_envelope(&env_complex).unwrap())
    });
}

criterion_group!(benches, filter_benchmarks);
criterion_main!(benches);
