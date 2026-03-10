use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde_json::{json, Value};
use std::collections::HashMap;
use streamforge::filter::*;
use streamforge::filter_parser::parse_transform;

fn create_test_message() -> Value {
    json!({
        "message": {
            "confId": 12345,
            "siteId": 67890,
            "status": "active",
            "timestamp": 1234567890
        },
        "order": {
            "id": 100,
            "price": 100.00,
            "tax": 8.00,
            "discount": 10.00,
            "items": 5
        },
        "users": [
            {"id": 1, "name": "Alice", "email": "alice@example.com"},
            {"id": 2, "name": "Bob", "email": "bob@example.com"},
            {"id": 3, "name": "Charlie", "email": "charlie@example.com"}
        ],
        "data": {
            "nested": {
                "value": 42
            }
        }
    })
}

fn bench_simple_transform(c: &mut Criterion) {
    let msg = create_test_message();

    c.bench_function("transform/extract_field", |b| {
        let transform = JsonPathTransform::new("/message/confId").unwrap();
        b.iter(|| transform.transform(black_box(msg.clone())))
    });

    c.bench_function("transform/extract_object", |b| {
        let transform = JsonPathTransform::new("/message").unwrap();
        b.iter(|| transform.transform(black_box(msg.clone())))
    });

    c.bench_function("transform/extract_nested", |b| {
        let transform = JsonPathTransform::new("/data/nested/value").unwrap();
        b.iter(|| transform.transform(black_box(msg.clone())))
    });
}

fn bench_object_construction(c: &mut Criterion) {
    let msg = create_test_message();

    c.bench_function("transform/construct_small", |b| {
        let mut fields = HashMap::new();
        fields.insert("id".to_string(), "/message/confId".to_string());
        fields.insert("site".to_string(), "/message/siteId".to_string());
        let transform = ObjectConstructTransform::new(fields).unwrap();
        b.iter(|| transform.transform(black_box(msg.clone())))
    });

    c.bench_function("transform/construct_medium", |b| {
        let mut fields = HashMap::new();
        fields.insert("id".to_string(), "/message/confId".to_string());
        fields.insert("site".to_string(), "/message/siteId".to_string());
        fields.insert("status".to_string(), "/message/status".to_string());
        fields.insert("timestamp".to_string(), "/message/timestamp".to_string());
        let transform = ObjectConstructTransform::new(fields).unwrap();
        b.iter(|| transform.transform(black_box(msg.clone())))
    });

    c.bench_function("transform/construct_large", |b| {
        let mut fields = HashMap::new();
        fields.insert("confId".to_string(), "/message/confId".to_string());
        fields.insert("siteId".to_string(), "/message/siteId".to_string());
        fields.insert("status".to_string(), "/message/status".to_string());
        fields.insert("timestamp".to_string(), "/message/timestamp".to_string());
        fields.insert("orderId".to_string(), "/order/id".to_string());
        fields.insert("price".to_string(), "/order/price".to_string());
        fields.insert("tax".to_string(), "/order/tax".to_string());
        fields.insert("discount".to_string(), "/order/discount".to_string());
        let transform = ObjectConstructTransform::new(fields).unwrap();
        b.iter(|| transform.transform(black_box(msg.clone())))
    });
}

fn bench_array_transform(c: &mut Criterion) {
    let msg = create_test_message();

    c.bench_function("transform/array_map_simple", |b| {
        let element_transform = Box::new(JsonPathTransform::new("/id").unwrap());
        let transform = ArrayMapTransform::new("/users", element_transform).unwrap();
        b.iter(|| transform.transform(black_box(msg.clone())))
    });

    c.bench_function("transform/array_map_nested", |b| {
        let element_transform = Box::new(JsonPathTransform::new("/email").unwrap());
        let transform = ArrayMapTransform::new("/users", element_transform).unwrap();
        b.iter(|| transform.transform(black_box(msg.clone())))
    });
}

fn bench_arithmetic_transform(c: &mut Criterion) {
    let msg = create_test_message();

    c.bench_function("transform/arithmetic_add", |b| {
        let transform = ArithmeticTransform::new_with_paths(
            ArithmeticOp::Add,
            "/order/price",
            "/order/tax"
        ).unwrap();
        b.iter(|| transform.transform(black_box(msg.clone())))
    });

    c.bench_function("transform/arithmetic_sub", |b| {
        let transform = ArithmeticTransform::new_with_paths(
            ArithmeticOp::Sub,
            "/order/price",
            "/order/discount"
        ).unwrap();
        b.iter(|| transform.transform(black_box(msg.clone())))
    });

    c.bench_function("transform/arithmetic_mul_constant", |b| {
        let transform = ArithmeticTransform::new_with_constant(
            ArithmeticOp::Mul,
            "/order/price",
            1.08
        ).unwrap();
        b.iter(|| transform.transform(black_box(msg.clone())))
    });

    c.bench_function("transform/arithmetic_div", |b| {
        let transform = ArithmeticTransform::new_with_paths(
            ArithmeticOp::Div,
            "/order/price",
            "/order/items"
        ).unwrap();
        b.iter(|| transform.transform(black_box(msg.clone())))
    });
}

fn bench_transform_parser(c: &mut Criterion) {
    c.bench_function("parser/simple_transform", |b| {
        b.iter(|| parse_transform(black_box("/message/confId")))
    });

    c.bench_function("parser/construct_transform", |b| {
        b.iter(|| parse_transform(black_box("CONSTRUCT:id=/message/confId:site=/message/siteId")))
    });

    c.bench_function("parser/array_map_transform", |b| {
        b.iter(|| parse_transform(black_box("ARRAY_MAP:/users,/id")))
    });

    c.bench_function("parser/arithmetic_transform", |b| {
        b.iter(|| parse_transform(black_box("ARITHMETIC:ADD,/price,/tax")))
    });
}

fn bench_transform_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("transform/throughput");

    for msg_count in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*msg_count as u64));

        group.bench_with_input(BenchmarkId::new("simple", msg_count), msg_count, |b, &count| {
            let transform = JsonPathTransform::new("/message/confId").unwrap();
            let msg = create_test_message();
            b.iter(|| {
                for _ in 0..count {
                    black_box(transform.transform(black_box(msg.clone())).unwrap());
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("construct", msg_count), msg_count, |b, &count| {
            let mut fields = HashMap::new();
            fields.insert("id".to_string(), "/message/confId".to_string());
            fields.insert("site".to_string(), "/message/siteId".to_string());
            fields.insert("status".to_string(), "/message/status".to_string());
            let transform = ObjectConstructTransform::new(fields).unwrap();
            let msg = create_test_message();
            b.iter(|| {
                for _ in 0..count {
                    black_box(transform.transform(black_box(msg.clone())).unwrap());
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("arithmetic", msg_count), msg_count, |b, &count| {
            let transform = ArithmeticTransform::new_with_constant(
                ArithmeticOp::Mul,
                "/order/price",
                1.08
            ).unwrap();
            let msg = create_test_message();
            b.iter(|| {
                for _ in 0..count {
                    black_box(transform.transform(black_box(msg.clone())).unwrap());
                }
            });
        });
    }

    group.finish();
}

fn bench_combined_operations(c: &mut Criterion) {
    let msg = create_test_message();

    c.bench_function("combined/filter_and_transform", |b| {
        let filter = JsonPathFilter::new("/message/siteId", ">", "10000").unwrap();
        let transform = JsonPathTransform::new("/message/confId").unwrap();
        b.iter(|| {
            if filter.evaluate(black_box(&msg)).unwrap() {
                black_box(transform.transform(black_box(msg.clone())).unwrap());
            }
        });
    });

    c.bench_function("combined/complex_filter_and_construct", |b| {
        let filter1 = Box::new(JsonPathFilter::new("/message/siteId", ">", "10000").unwrap());
        let filter2 = Box::new(JsonPathFilter::new("/message/status", "==", "active").unwrap());
        let filter = AndFilter::new(vec![filter1, filter2]);

        let mut fields = HashMap::new();
        fields.insert("id".to_string(), "/message/confId".to_string());
        fields.insert("site".to_string(), "/message/siteId".to_string());
        let transform = ObjectConstructTransform::new(fields).unwrap();

        b.iter(|| {
            if filter.evaluate(black_box(&msg)).unwrap() {
                black_box(transform.transform(black_box(msg.clone())).unwrap());
            }
        });
    });
}

criterion_group!(
    benches,
    bench_simple_transform,
    bench_object_construction,
    bench_array_transform,
    bench_arithmetic_transform,
    bench_transform_parser,
    bench_transform_throughput,
    bench_combined_operations
);
criterion_main!(benches);
