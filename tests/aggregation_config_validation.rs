use streamforge::MirrorMakerConfig;
use uuid::Uuid;

fn write_temp_config(contents: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("streamforge-{}.yaml", Uuid::new_v4()));
    std::fs::write(&path, contents).expect("temp config should write");
    path
}

fn parse_config(yaml: &str) -> MirrorMakerConfig {
    serde_yaml::from_str(yaml).expect("config should parse")
}

fn base_aggregation_yaml(aggregation_block: &str) -> String {
    format!(
        r#"
appid: agg-test
bootstrap: localhost:9092
input: raw-orders
routing:
  routing_type: filter
  destinations:
    - output: orders-metrics-1m
{aggregation_block}
"#
    )
}

fn aggregation_block(
    extra_destination_fields: &str,
    metrics_block: &str,
    window_block: &str,
) -> String {
    format!(
        r#"      {extra_destination_fields}aggregation:
        group_by:
          - name: customer_id
            path: /customer_id
        window:
{window_block}
        metrics:
{metrics_block}"#
    )
}

#[test]
fn rejects_aggregation_with_key_transform() {
    let yaml = base_aggregation_yaml(&aggregation_block(
        "key_transform: /customer_id\n      ",
        "          - name: order_count\n            op: count\n",
        "          type: tumbling\n          size_seconds: 60\n          emit_interval_seconds: 5\n",
    ));

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err
        .to_string()
        .contains("aggregation destinations cannot use key_transform"));
}

#[test]
fn rejects_aggregation_with_header_or_timestamp_transforms() {
    let yaml = base_aggregation_yaml(&aggregation_block(
        "timestamp: CURRENT\n      headers:\n        x-source: streamforge\n      ",
        "          - name: order_count\n            op: count\n",
        "          type: tumbling\n          size_seconds: 60\n          emit_interval_seconds: 5\n",
    ));

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err
        .to_string()
        .contains("aggregation destinations cannot use header or timestamp transforms in v1"));
}

#[test]
fn rejects_aggregation_with_manual_commit() {
    let yaml = format!(
        r#"
appid: agg-test
bootstrap: localhost:9092
input: raw-orders
commit_strategy:
  manual_commit: true
routing:
  routing_type: filter
  destinations:
    - output: orders-metrics-1m
{}
"#,
        aggregation_block(
            "",
            "          - name: order_count\n            op: count\n",
            "          type: tumbling\n          size_seconds: 60\n          emit_interval_seconds: 5\n",
        )
    );

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err.to_string().contains(
        "aggregation destinations do not support commit_strategy.manual_commit=true in v1"
    ));
}

#[test]
fn rejects_quantiles_without_percentiles() {
    let yaml = base_aggregation_yaml(&aggregation_block(
        "",
        "          - name: amount_quantiles\n            op: quantiles\n            path: /amount\n",
        "          type: tumbling\n          size_seconds: 60\n          emit_interval_seconds: 5\n",
    ));

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err
        .to_string()
        .contains("quantiles metrics require percentiles"));
}

#[test]
fn rejects_quantiles_with_out_of_range_percentiles() {
    let yaml = base_aggregation_yaml(&aggregation_block(
        "",
        concat!(
            "          - name: amount_quantiles\n",
            "            op: quantiles\n",
            "            path: /amount\n",
            "            percentiles:\n",
            "              - 1.1\n"
        ),
        "          type: tumbling\n          size_seconds: 60\n          emit_interval_seconds: 5\n",
    ));

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err.to_string().contains("percentile must be in [0.0, 1.0]"));
}

#[test]
fn rejects_quantiles_with_duplicate_percentiles() {
    let yaml = base_aggregation_yaml(&aggregation_block(
        "",
        concat!(
            "          - name: amount_quantiles\n",
            "            op: quantiles\n",
            "            path: /amount\n",
            "            percentiles:\n",
            "              - 0.5\n",
            "              - 0.50\n"
        ),
        "          type: tumbling\n          size_seconds: 60\n          emit_interval_seconds: 5\n",
    ));

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err
        .to_string()
        .contains("contains duplicate percentile key"));
}

#[test]
fn rejects_quantiles_with_non_finite_percentiles() {
    let yaml = base_aggregation_yaml(&aggregation_block(
        "",
        concat!(
            "          - name: amount_quantiles\n",
            "            op: quantiles\n",
            "            path: /amount\n",
            "            percentiles:\n",
            "              - .nan\n"
        ),
        "          type: tumbling\n          size_seconds: 60\n          emit_interval_seconds: 5\n",
    ));

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err.to_string().contains("has non-finite percentile"));
}

#[test]
fn rejects_value_metrics_without_path() {
    let yaml = base_aggregation_yaml(&aggregation_block(
        "",
        "          - name: total_amount\n            op: sum\n",
        "          type: tumbling\n          size_seconds: 60\n          emit_interval_seconds: 5\n",
    ));

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err.to_string().contains("sum metrics require path"));
}

#[test]
fn rejects_empty_group_by() {
    let yaml = base_aggregation_yaml(
        r#"      aggregation:
        group_by: []
        window:
          type: tumbling
          size_seconds: 60
          emit_interval_seconds: 5
        metrics:
          - name: order_count
            op: count
"#,
    );

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err.to_string().contains("group_by cannot be empty"));
}

#[test]
fn rejects_blank_group_by_name() {
    let yaml = base_aggregation_yaml(
        r#"      aggregation:
        group_by:
          - name: "   "
            path: /customer_id
        window:
          type: tumbling
          size_seconds: 60
          emit_interval_seconds: 5
        metrics:
          - name: order_count
            op: count
"#,
    );

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err
        .to_string()
        .contains("group_by entries require non-empty name"));
}

#[test]
fn rejects_blank_group_by_path() {
    let yaml = base_aggregation_yaml(
        r#"      aggregation:
        group_by:
          - name: customer_id
            path: "   "
        window:
          type: tumbling
          size_seconds: 60
          emit_interval_seconds: 5
        metrics:
          - name: order_count
            op: count
"#,
    );

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err
        .to_string()
        .contains("group_by entries require non-empty path"));
}

#[test]
fn rejects_empty_metrics() {
    let yaml = base_aggregation_yaml(
        r#"      aggregation:
        group_by:
          - name: customer_id
            path: /customer_id
        window:
          type: tumbling
          size_seconds: 60
          emit_interval_seconds: 5
        metrics: []
"#,
    );

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err.to_string().contains("metrics cannot be empty"));
}

#[test]
fn rejects_invalid_aggregation_via_from_file() {
    let yaml = base_aggregation_yaml(&aggregation_block(
        "key_transform: /customer_id\n      ",
        "          - name: order_count\n            op: count\n",
        "          type: tumbling\n          size_seconds: 60\n          emit_interval_seconds: 5\n",
    ));
    let path = write_temp_config(&yaml);

    let err = MirrorMakerConfig::from_file(path.to_str().expect("utf-8 path")).unwrap_err();
    let _ = std::fs::remove_file(path);

    assert!(err
        .to_string()
        .contains("aggregation destinations cannot use key_transform"));
}

#[test]
fn rejects_non_positive_window_size() {
    let yaml = base_aggregation_yaml(&aggregation_block(
        "",
        "          - name: order_count\n            op: count\n",
        "          type: tumbling\n          size_seconds: 0\n          emit_interval_seconds: 5\n",
    ));

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err.to_string().contains("window size_seconds must be > 0"));
}

#[test]
fn rejects_invalid_emit_interval() {
    let yaml = base_aggregation_yaml(&aggregation_block(
        "",
        "          - name: order_count\n            op: count\n",
        "          type: tumbling\n          size_seconds: 60\n          emit_interval_seconds: 61\n",
    ));

    let cfg = parse_config(&yaml);
    let err = cfg.validate().unwrap_err();

    assert!(err
        .to_string()
        .contains("window emit_interval_seconds must be > 0 and <= size_seconds"));
}

#[test]
fn accepts_valid_aggregation_config() {
    let yaml = base_aggregation_yaml(&aggregation_block(
        "",
        concat!(
            "          - name: order_count\n",
            "            op: count\n",
            "          - name: total_amount\n",
            "            op: sum\n",
            "            path: /amount\n",
            "          - name: amount_quantiles\n",
            "            op: quantiles\n",
            "            path: /amount\n",
            "            percentiles:\n",
            "              - 0.5\n",
            "              - 0.95\n"
        ),
        "          type: tumbling\n          size_seconds: 60\n          emit_interval_seconds: 5\n",
    ));

    let cfg = parse_config(&yaml);

    cfg.validate().expect("config should validate");
}
