use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde_json::{json, Value};
use streamforge::filter::*;
use streamforge::filter_parser::parse_filter;

fn create_test_message() -> Value {
    json!({
        "message": {
            "siteId": 15000,
            "confId": 12345,
            "status": "active",
            "email": "user@example.com",
            "priority": "high",
            "enabled": true,
            "amount": 100.50,
            "count": 42
        },
        "users": [
            {"id": 1, "status": "active", "name": "Alice"},
            {"id": 2, "status": "active", "name": "Bob"},
            {"id": 3, "status": "inactive", "name": "Charlie"}
        ],
        "metadata": {
            "timestamp": 1234567890,
            "version": "2.0.0"
        }
    })
}

fn bench_simple_filter(c: &mut Criterion) {
    let msg = create_test_message();

    c.bench_function("filter/simple_numeric_gt", |b| {
        let filter = JsonPathFilter::new("/message/siteId", ">", "10000").unwrap();
        b.iter(|| filter.evaluate(black_box(&msg)))
    });

    c.bench_function("filter/simple_string_eq", |b| {
        let filter = JsonPathFilter::new("/message/status", "==", "active").unwrap();
        b.iter(|| filter.evaluate(black_box(&msg)))
    });

    c.bench_function("filter/simple_boolean", |b| {
        let filter = JsonPathFilter::new("/message/enabled", "==", "true").unwrap();
        b.iter(|| filter.evaluate(black_box(&msg)))
    });
}

fn bench_boolean_logic(c: &mut Criterion) {
    let msg = create_test_message();

    c.bench_function("filter/and_two_conditions", |b| {
        let filter1 = Box::new(JsonPathFilter::new("/message/siteId", ">", "10000").unwrap());
        let filter2 = Box::new(JsonPathFilter::new("/message/status", "==", "active").unwrap());
        let and_filter = AndFilter::new(vec![filter1, filter2]);
        b.iter(|| and_filter.evaluate(black_box(&msg)))
    });

    c.bench_function("filter/and_three_conditions", |b| {
        let filter1 = Box::new(JsonPathFilter::new("/message/siteId", ">", "10000").unwrap());
        let filter2 = Box::new(JsonPathFilter::new("/message/status", "==", "active").unwrap());
        let filter3 = Box::new(JsonPathFilter::new("/message/enabled", "==", "true").unwrap());
        let and_filter = AndFilter::new(vec![filter1, filter2, filter3]);
        b.iter(|| and_filter.evaluate(black_box(&msg)))
    });

    c.bench_function("filter/or_two_conditions", |b| {
        let filter1 = Box::new(JsonPathFilter::new("/message/priority", "==", "high").unwrap());
        let filter2 = Box::new(JsonPathFilter::new("/message/priority", "==", "urgent").unwrap());
        let or_filter = OrFilter::new(vec![filter1, filter2]);
        b.iter(|| or_filter.evaluate(black_box(&msg)))
    });

    c.bench_function("filter/not_condition", |b| {
        let filter = Box::new(JsonPathFilter::new("/message/status", "==", "inactive").unwrap());
        let not_filter = NotFilter::new(filter);
        b.iter(|| not_filter.evaluate(black_box(&msg)))
    });
}

fn bench_regex_filter(c: &mut Criterion) {
    let msg = create_test_message();

    c.bench_function("filter/regex_simple", |b| {
        let filter = RegexFilter::new("/message/status", "^active").unwrap();
        b.iter(|| filter.evaluate(black_box(&msg)))
    });

    c.bench_function("filter/regex_email", |b| {
        let filter = RegexFilter::new("/message/email", r"^[\w\.-]+@[\w\.-]+\.\w+$").unwrap();
        b.iter(|| filter.evaluate(black_box(&msg)))
    });

    c.bench_function("filter/regex_version", |b| {
        let filter = RegexFilter::new("/metadata/version", r"^2\.").unwrap();
        b.iter(|| filter.evaluate(black_box(&msg)))
    });
}

fn bench_array_filter(c: &mut Criterion) {
    let msg = create_test_message();

    c.bench_function("filter/array_all", |b| {
        let element_filter = Box::new(JsonPathFilter::new("/id", ">", "0").unwrap());
        let filter = ArrayFilter::new("/users", element_filter, ArrayFilterMode::All).unwrap();
        b.iter(|| filter.evaluate(black_box(&msg)))
    });

    c.bench_function("filter/array_any", |b| {
        let element_filter = Box::new(JsonPathFilter::new("/status", "==", "active").unwrap());
        let filter = ArrayFilter::new("/users", element_filter, ArrayFilterMode::Any).unwrap();
        b.iter(|| filter.evaluate(black_box(&msg)))
    });
}

fn bench_filter_parser(c: &mut Criterion) {
    c.bench_function("parser/simple_filter", |b| {
        b.iter(|| parse_filter(black_box("/message/siteId,>,10000")))
    });

    c.bench_function("parser/and_filter", |b| {
        b.iter(|| {
            parse_filter(black_box(
                "AND:/message/siteId,>,10000:/message/status,==,active",
            ))
        })
    });

    c.bench_function("parser/regex_filter", |b| {
        b.iter(|| parse_filter(black_box(r"REGEX:/email,^[\w\.-]+@[\w\.-]+\.\w+$")))
    });

    c.bench_function("parser/array_filter", |b| {
        b.iter(|| parse_filter(black_box("ARRAY_ALL:/users,/status,==,active")))
    });
}

fn bench_filter_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter/throughput");

    for msg_count in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*msg_count as u64));

        group.bench_with_input(
            BenchmarkId::new("simple", msg_count),
            msg_count,
            |b, &count| {
                let filter = JsonPathFilter::new("/message/siteId", ">", "10000").unwrap();
                let msg = create_test_message();
                b.iter(|| {
                    for _ in 0..count {
                        black_box(filter.evaluate(black_box(&msg)).unwrap());
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("complex", msg_count),
            msg_count,
            |b, &count| {
                // Complex filter: AND with 3 conditions
                let filter = parse_filter(
                    "AND:/message/siteId,>,10000:/message/status,==,active:/message/userId,>,500",
                )
                .unwrap();
                let msg = create_test_message();
                b.iter(|| {
                    for _ in 0..count {
                        black_box(filter.evaluate(black_box(&msg)).unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_simple_filter,
    bench_boolean_logic,
    bench_regex_filter,
    bench_array_filter,
    bench_filter_parser,
    bench_filter_throughput
);
criterion_main!(benches);
