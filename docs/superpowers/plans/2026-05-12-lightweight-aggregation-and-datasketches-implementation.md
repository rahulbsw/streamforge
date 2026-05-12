# Lightweight Aggregation and DataSketches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add keyed tumbling-window aggregations to StreamForge destinations with `count`, `sum`, `avg`, `approx_distinct`, and quantile sketches, emitting derived metrics back to Kafka topics.

**Architecture:** Keep aggregation as a narrow extension of multi-destination routing. Existing filters and value transforms run first, then an aggregation destination updates keyed in-memory window state and flushes completed windows as JSON records to a normal Kafka sink. Sketch-backed metrics use the Rust `datasketches` crate behind a small wrapper so the config format and processor wiring stay stable even if the sketch library changes.

**Tech Stack:** Rust, Tokio, Serde YAML, existing `JsonPath` helpers, `rdkafka`, Prometheus metrics, `datasketches = "0.2.0"`.

---

## Scope Split

This plan intentionally covers only the first lightweight aggregation lane from the approved growth spec:

- keyed tumbling windows
- `count`
- `sum`
- `avg`
- `approx_distinct`
- quantiles via T-Digest style sketches
- YAML config, validation CLI, docs, and runnable examples

This plan explicitly does **not** cover:

- sliding or session windows
- joins
- SQL
- active-active replication changes
- heavy hitters / top-k
- operator form-builder UI changes

The existing UI YAML mode can consume the new config later, but the visual builder remains out of scope for this first aggregation delivery.

## File Structure

- Modify: `Cargo.toml`
  Add `datasketches = "0.2.0"` and keep the dependency surface limited to HLL and TDigest wrappers.

- Modify: `src/config.rs`
  Add aggregation-specific serde types and config validation rules on top of `DestinationConfig`.

- Create: `src/aggregation/mod.rs`
  Public module boundary and exports.

- Create: `src/aggregation/engine.rs`
  Window bucketing, grouped state, and flush lifecycle.

- Create: `src/aggregation/metric.rs`
  Metric accumulator trait and implementations for `count`, `sum`, `avg`, `approx_distinct`, and quantiles.

- Create: `src/aggregation/record.rs`
  Output record shaping for emitted aggregate payloads and deterministic group keys.

- Modify: `src/lib.rs`
  Export the aggregation module and config types.

- Modify: `src/processor.rs`
  Introduce an aggregation destination path and a `flush()` hook on processors.

- Modify: `src/processor_with_retry.rs`
  Forward `flush()` to the wrapped processor without changing retry semantics for normal message processing.

- Modify: `src/main.rs`
  Build aggregation destinations from routing config and run a periodic flush ticker.

- Modify: `src/bin/validate.rs`
  Validate aggregation config blocks and reject incompatible destination combinations.

- Modify: `src/observability/metrics.rs`
  Add aggregation-specific counters and gauges.

- Create: `tests/aggregation_config_validation.rs`
  Config schema and CLI validation coverage.

- Create: `tests/aggregation_engine_runtime.rs`
  Windowing and metric correctness coverage.

- Create: `tests/aggregation_sketches.rs`
  Approximate metric coverage with bounded-error assertions.

- Create: `docs/AGGREGATIONS.md`
  User-facing documentation for windowed derived metrics.

- Modify: `docs/index.md`
  Add the aggregation guide under build/runtime docs.

- Modify: `docs/DOCUMENTATION_INDEX.md`
  Add the aggregation guide to the curated docs map.

- Modify: `docs/USAGE.md`
  Add aggregation-specific usage patterns.

- Modify: `docs/_config.yml`
  Add the new guide to the public docs navigation under the usage section.

- Modify: `examples/README.md`
  Link to the new aggregation examples.

- Create: `examples/aggregation/orders-windowed-metrics.yaml`
  Example for `count`, `sum`, `avg`, and `approx_distinct`.

- Create: `examples/aggregation/orders-quantiles.yaml`
  Example for quantiles on numeric event fields.

## Config Shape

Phase 1 config should live inside an existing routing destination and use the destination `output` topic for emitted aggregate records:

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "orders-metrics-1m"
      filter: "/event_type,==,order_completed"
      transform: "CONSTRUCT:customer_id=/customer_id:region=/region:amount=/amount"
      aggregation:
        group_by:
          - name: customer_id
            path: "/customer_id"
          - name: region
            path: "/region"
        window:
          type: tumbling
          size_seconds: 60
          emit_interval_seconds: 5
        metrics:
          - name: order_count
            op: count
          - name: gross_amount
            op: sum
            path: "/amount"
          - name: avg_amount
            op: avg
            path: "/amount"
          - name: unique_customers
            op: approx_distinct
            path: "/customer_id"
```

Quantiles use the same structure with explicit percentiles:

```yaml
      aggregation:
        group_by:
          - name: region
            path: "/region"
        window:
          type: tumbling
          size_seconds: 60
          emit_interval_seconds: 5
        metrics:
          - name: amount_quantiles
            op: quantiles
            path: "/amount"
            percentiles: [0.5, 0.95, 0.99]
```

## Task 1: Add Aggregation Config Types and Validation

**Files:**
- Modify: `src/config.rs`
- Modify: `src/lib.rs`
- Modify: `src/bin/validate.rs`
- Create: `tests/aggregation_config_validation.rs`

- [ ] **Step 1: Write failing config validation tests**

```rust
use streamforge::MirrorMakerConfig;

#[test]
fn rejects_aggregation_with_key_transform() {
    let yaml = r#"
appid: agg-test
bootstrap: localhost:9092
input: raw-orders
routing:
  routing_type: filter
  destinations:
    - output: orders-metrics-1m
      key_transform: /customer_id
      aggregation:
        group_by:
          - name: customer_id
            path: /customer_id
        window:
          type: tumbling
          size_seconds: 60
          emit_interval_seconds: 5
        metrics:
          - name: order_count
            op: count
"#;

    let cfg: MirrorMakerConfig = serde_yaml::from_str(yaml).unwrap();
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("aggregation destinations cannot use key_transform"));
}

#[test]
fn rejects_quantiles_without_percentiles() {
    let yaml = r#"
appid: agg-test
bootstrap: localhost:9092
input: raw-orders
routing:
  routing_type: filter
  destinations:
    - output: orders-metrics-1m
      aggregation:
        group_by:
          - name: customer_id
            path: /customer_id
        window:
          type: tumbling
          size_seconds: 60
          emit_interval_seconds: 5
        metrics:
          - name: amount_quantiles
            op: quantiles
            path: /amount
"#;

    let cfg: MirrorMakerConfig = serde_yaml::from_str(yaml).unwrap();
    let err = cfg.validate().unwrap_err();
    assert!(err.to_string().contains("quantiles metrics require percentiles"));
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run:

```bash
cargo test aggregation_config_validation -- --nocapture
```

Expected:
- FAIL because `MirrorMakerConfig::validate()` and the aggregation config types do not exist yet.

- [ ] **Step 3: Add config types and validation rules**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregationConfig {
    pub group_by: Vec<AggregationGroupBy>,
    pub window: AggregationWindowConfig,
    pub metrics: Vec<AggregationMetricConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregationGroupBy {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregationWindowConfig {
    #[serde(rename = "type")]
    pub window_type: AggregationWindowType,
    pub size_seconds: u64,
    #[serde(default = "default_emit_interval_seconds")]
    pub emit_interval_seconds: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AggregationWindowType {
    Tumbling,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregationMetricConfig {
    pub name: String,
    pub op: AggregationOp,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub percentiles: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AggregationOp {
    Count,
    Sum,
    Avg,
    ApproxDistinct,
    Quantiles,
}
```

Add to `DestinationConfig`:

```rust
#[serde(default)]
pub aggregation: Option<AggregationConfig>,
```

Add config validation:

```rust
impl MirrorMakerConfig {
    pub fn validate(&self) -> crate::Result<()> {
        if let Some(routing) = &self.routing {
            for dest in &routing.destinations {
                if let Some(agg) = &dest.aggregation {
                    if dest.key_transform.is_some() {
                        return Err(crate::MirrorMakerError::Config(
                            "aggregation destinations cannot use key_transform".into(),
                        ));
                    }
                    if dest.headers.is_some() || dest.header_transforms.is_some() || dest.timestamp.is_some() {
                        return Err(crate::MirrorMakerError::Config(
                            "aggregation destinations cannot use header or timestamp transforms in v1".into(),
                        ));
                    }
                    agg.validate()?;
                }
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Wire validation into the CLI**

```rust
let config: MirrorMakerConfig = serde_yaml::from_str(&config_content)?;
config.validate()?;
```

- [ ] **Step 5: Re-run config validation**

Run:

```bash
cargo test aggregation_config_validation -- --nocapture
```

Expected:
- PASS for the new validation tests.

- [ ] **Step 6: Commit**

```bash
git add src/config.rs src/lib.rs src/bin/validate.rs tests/aggregation_config_validation.rs
git commit -m "feat: add aggregation config schema and validation"
```

## Task 2: Implement the Windowed Numeric Aggregation Core

**Files:**
- Create: `src/aggregation/mod.rs`
- Create: `src/aggregation/engine.rs`
- Create: `src/aggregation/metric.rs`
- Create: `src/aggregation/record.rs`
- Modify: `src/lib.rs`
- Create: `tests/aggregation_engine_runtime.rs`

- [ ] **Step 1: Write failing engine tests for `count`, `sum`, and `avg`**

```rust
use serde_json::json;
use streamforge::aggregation::{AggregationEngine, EmittedAggregate};
use streamforge::config::{
    AggregationConfig, AggregationGroupBy, AggregationMetricConfig, AggregationOp,
    AggregationWindowConfig, AggregationWindowType,
};

#[test]
fn flushes_one_grouped_tumbling_window() {
    let spec = AggregationConfig {
        group_by: vec![AggregationGroupBy {
            name: "region".into(),
            path: "/region".into(),
        }],
        window: AggregationWindowConfig {
            window_type: AggregationWindowType::Tumbling,
            size_seconds: 60,
            emit_interval_seconds: 5,
        },
        metrics: vec![
            AggregationMetricConfig { name: "order_count".into(), op: AggregationOp::Count, path: None, percentiles: None },
            AggregationMetricConfig { name: "gross_amount".into(), op: AggregationOp::Sum, path: Some("/amount".into()), percentiles: None },
            AggregationMetricConfig { name: "avg_amount".into(), op: AggregationOp::Avg, path: Some("/amount".into()), percentiles: None },
        ],
    };

    let mut engine = AggregationEngine::new(spec, "orders-metrics-1m");
    engine.observe(&json!({"region":"us","amount":10.0}), 1_000).unwrap();
    engine.observe(&json!({"region":"us","amount":20.0}), 20_000).unwrap();

    let emitted = engine.flush_expired(61_000).unwrap();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].value["group"]["region"], "us");
    assert_eq!(emitted[0].value["metrics"]["order_count"], 2);
    assert_eq!(emitted[0].value["metrics"]["gross_amount"], 30.0);
    assert_eq!(emitted[0].value["metrics"]["avg_amount"], 15.0);
}
```

- [ ] **Step 2: Run the new engine test**

Run:

```bash
cargo test aggregation_engine_runtime::flushes_one_grouped_tumbling_window -- --nocapture
```

Expected:
- FAIL because the `aggregation` module and engine do not exist yet.

- [ ] **Step 3: Implement `AggregationEngine`, output records, and numeric metric accumulators**

```rust
pub struct AggregationEngine {
    output_topic: String,
    spec: AggregationConfig,
    windows: std::collections::BTreeMap<WindowInstanceKey, AggregateWindow>,
}

pub struct EmittedAggregate {
    pub topic: String,
    pub key: Option<Vec<u8>>,
    pub value: serde_json::Value,
    pub timestamp_ms: i64,
}

pub trait MetricAccumulator: Send + Sync {
    fn observe(&mut self, value: &serde_json::Value) -> crate::Result<()>;
    fn snapshot(&self) -> serde_json::Value;
}
```

Use the emitted JSON shape below and keep it stable across all metric types:

```json
{
  "window": {
    "start_ms": 0,
    "end_ms": 60000,
    "type": "tumbling",
    "size_seconds": 60
  },
  "group": {
    "region": "us"
  },
  "metrics": {
    "order_count": 2,
    "gross_amount": 30.0,
    "avg_amount": 15.0
  }
}
```

- [ ] **Step 4: Re-run the numeric engine tests**

Run:

```bash
cargo test aggregation_engine_runtime -- --nocapture
```

Expected:
- PASS for grouped tumbling-window count/sum/avg behavior.

- [ ] **Step 5: Commit**

```bash
git add src/aggregation src/lib.rs tests/aggregation_engine_runtime.rs
git commit -m "feat: add tumbling window aggregation engine"
```

## Task 3: Wire Aggregation into Routing and Add Periodic Flush

**Files:**
- Modify: `src/processor.rs`
- Modify: `src/processor_with_retry.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add a failing processor lifecycle test**

```rust
use crate::processor::MessageProcessor;
use crate::{MessageEnvelope, Result, RetryConfig, RetryPolicy};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn flush_is_forwarded_through_retry_wrapper() {
    struct FlushAwareProcessor {
        flush_calls: Arc<AtomicU32>,
    }

    #[async_trait::async_trait]
    impl MessageProcessor for FlushAwareProcessor {
        async fn process(&self, _envelope: MessageEnvelope) -> Result<()> {
            Ok(())
        }

        async fn flush(&self) -> Result<()> {
            self.flush_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let flush_calls = Arc::new(AtomicU32::new(0));
    let processor = Arc::new(FlushAwareProcessor {
        flush_calls: flush_calls.clone(),
    });

    let wrapper = crate::processor_with_retry::ProcessorWithRetry::new(
        processor,
        RetryPolicy::new(RetryConfig {
            max_attempts: 1,
            ..Default::default()
        }),
        None,
        "agg-pipeline".to_string(),
    );

    wrapper.flush().await.unwrap();
    assert_eq!(flush_calls.load(Ordering::SeqCst), 1);
}
```

- [ ] **Step 2: Extend `MessageProcessor` with a default `flush()` hook**

```rust
#[async_trait::async_trait]
pub trait MessageProcessor: Send + Sync {
    async fn process(&self, envelope: MessageEnvelope) -> Result<()>;

    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}
```

Forward it in the retry wrapper:

```rust
#[async_trait::async_trait]
impl MessageProcessor for ProcessorWithRetry {
    async fn process(&self, envelope: MessageEnvelope) -> Result<()> {
        // existing code
    }

    async fn flush(&self) -> Result<()> {
        self.processor.flush().await
    }
}
```

- [ ] **Step 3: Introduce an aggregation destination path in `src/processor.rs`**

```rust
pub enum RoutedDestination {
    Immediate(DestinationProcessor),
    Aggregation(AggregationDestinationProcessor),
}

pub struct AggregationDestinationProcessor {
    name: String,
    filter: Arc<dyn Filter>,
    transform: Arc<dyn Transform>,
    engine: tokio::sync::Mutex<AggregationEngine>,
    sink: Arc<KafkaSink>,
}
```

Runtime rules:
- filter first
- value transform second
- aggregation observe third
- `flush()` emits completed windows to `sink`

- [ ] **Step 4: Start a periodic flush ticker from `src/main.rs`**

```rust
let flush_interval = config
    .routing
    .as_ref()
    .map(|routing| routing.destinations.iter()
        .filter_map(|d| d.aggregation.as_ref().map(|a| a.window.emit_interval_seconds))
        .min()
        .unwrap_or(5))
    .unwrap_or(5);

let flush_processor = processor.clone();
tokio::spawn(async move {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(flush_interval));
    loop {
        ticker.tick().await;
        if let Err(err) = flush_processor.flush().await {
            tracing::error!("aggregation flush failed: {}", err);
        }
    }
});
```

- [ ] **Step 5: Run processor lifecycle tests**

Run:

```bash
cargo test flush_forwards_completed_windows -- --nocapture
```

Expected:
- PASS once completed windows emit without requiring another input record.

- [ ] **Step 6: Commit**

```bash
git add src/processor.rs src/processor_with_retry.rs src/main.rs
git commit -m "feat: wire aggregation destinations into runtime"
```

## Task 4: Add DataSketches-Based Metrics

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/aggregation/metric.rs`
- Create: `tests/aggregation_sketches.rs`

- [ ] **Step 1: Add failing sketch tests**

```rust
use serde_json::json;
use streamforge::aggregation::AggregationEngine;
use streamforge::config::{
    AggregationConfig, AggregationMetricConfig, AggregationOp, AggregationWindowConfig,
    AggregationWindowType,
};

#[test]
fn approx_distinct_stays_within_reasonable_error() {
    let spec = AggregationConfig {
        group_by: vec![],
        window: AggregationWindowConfig {
            window_type: AggregationWindowType::Tumbling,
            size_seconds: 60,
            emit_interval_seconds: 5,
        },
        metrics: vec![AggregationMetricConfig {
            name: "unique_customers".into(),
            op: AggregationOp::ApproxDistinct,
            path: Some("/customer_id".into()),
            percentiles: None,
        }],
    };

    let mut engine = AggregationEngine::new(spec, "orders-metrics-1m");
    for i in 0..100 {
        engine
            .observe(&json!({"customer_id": format!("customer-{i}")}), i * 100)
            .unwrap();
    }

    let emitted = engine.flush_expired(61_000).unwrap();
    let estimate = emitted[0].value["metrics"]["unique_customers"]
        .as_f64()
        .unwrap();

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
            name: "amount_quantiles".into(),
            op: AggregationOp::Quantiles,
            path: Some("/amount".into()),
            percentiles: Some(vec![0.5, 0.95, 0.99]),
        }],
    };

    let mut engine = AggregationEngine::new(spec, "orders-metrics-1m");
    for amount in 1..=100 {
        engine
            .observe(&json!({"amount": amount as f64}), amount as i64)
            .unwrap();
    }

    let emitted = engine.flush_expired(61_000).unwrap();
    let quantiles = &emitted[0].value["metrics"]["amount_quantiles"];
    let p50 = quantiles["p50"].as_f64().unwrap();
    let p95 = quantiles["p95"].as_f64().unwrap();
    let p99 = quantiles["p99"].as_f64().unwrap();

    assert!((p50 - 50.0).abs() <= 5.0, "unexpected p50: {p50}");
    assert!((p95 - 95.0).abs() <= 8.0, "unexpected p95: {p95}");
    assert!((p99 - 99.0).abs() <= 8.0, "unexpected p99: {p99}");
}
```

- [ ] **Step 2: Add the sketch dependency and wrap it behind metric accumulators**

```toml
datasketches = "0.2.0"
```

Use only the modules needed for this phase:

```rust
use datasketches::hll::{HllSketch, HllType};
use datasketches::tdigest::TDigest;

pub enum SketchAccumulator {
    ApproxDistinct {
        path: crate::JsonPath,
        sketch: HllSketch,
    },
    Quantiles {
        path: crate::JsonPath,
        percentiles: Vec<f64>,
        digest: TDigest,
    },
}
```

Implementation rules:
- `approx_distinct` uses one HLL sketch per group/window.
- `quantiles` emits an object keyed by percentile strings, for example:

```json
{
  "amount_quantiles": {
    "p50": 10.0,
    "p95": 18.0,
    "p99": 20.0
  }
}
```

- [ ] **Step 3: Run sketch tests with bounded error assertions**

Run:

```bash
cargo test aggregation_sketches -- --nocapture
```

Expected:
- PASS with error-tolerant assertions, not exact-equality assertions.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml src/aggregation/metric.rs tests/aggregation_sketches.rs
git commit -m "feat: add sketch-backed aggregation metrics"
```

## Task 5: Add Observability, Docs, and Runnable Examples

**Files:**
- Modify: `src/observability/metrics.rs`
- Create: `docs/AGGREGATIONS.md`
- Modify: `docs/index.md`
- Modify: `docs/DOCUMENTATION_INDEX.md`
- Modify: `docs/USAGE.md`
- Modify: `docs/_config.yml`
- Modify: `examples/README.md`
- Create: `examples/aggregation/orders-windowed-metrics.yaml`
- Create: `examples/aggregation/orders-quantiles.yaml`

- [ ] **Step 1: Add aggregation-specific Prometheus metrics**

```rust
pub aggregation_updates: CounterVec,
pub aggregation_windows_open: GaugeVec,
pub aggregation_flushes: CounterVec,
pub aggregation_records_emitted: CounterVec,
```

Required labels:
- `destination`
- `metric` where appropriate
- `status` for flush success/error

- [ ] **Step 2: Add runnable example configs**

```yaml
# examples/aggregation/orders-windowed-metrics.yaml
appid: "orders-windowed-metrics"
bootstrap: "localhost:9092"
input: "raw-orders"
offset: "earliest"
threads: 2

routing:
  routing_type: "filter"
  destinations:
    - output: "orders-metrics-1m"
      filter: "/event_type,==,order_completed"
      transform: "CONSTRUCT:customer_id=/customer_id:region=/region:amount=/amount"
      aggregation:
        group_by:
          - name: customer_id
            path: "/customer_id"
          - name: region
            path: "/region"
        window:
          type: tumbling
          size_seconds: 60
          emit_interval_seconds: 5
        metrics:
          - name: order_count
            op: count
          - name: gross_amount
            op: sum
            path: "/amount"
          - name: avg_amount
            op: avg
            path: "/amount"
          - name: unique_customers
            op: approx_distinct
            path: "/customer_id"
```

- [ ] **Step 3: Document the feature and add it to the public docs map**

````md
<!-- docs/AGGREGATIONS.md -->
---
title: Aggregations
parent: Usage Guide
---

# Lightweight Aggregations

StreamForge can emit derived metric topics from filtered event streams without introducing a full SQL or stateful stream-processing runtime.

## Supported in v1

- Window type: `tumbling`
- Metrics: `count`, `sum`, `avg`, `approx_distinct`, `quantiles`
- State model: in-memory, per-process
- Output: JSON records written to a normal Kafka or Redpanda topic

## Output Shape

```json
{
  "window": {
    "start_ms": 0,
    "end_ms": 60000,
    "type": "tumbling",
    "size_seconds": 60
  },
  "group": {
    "region": "us"
  },
  "metrics": {
    "order_count": 2,
    "gross_amount": 30.0,
    "avg_amount": 15.0,
    "unique_customers": 2,
    "amount_quantiles": {
      "p50": 10.0,
      "p95": 18.0
    }
  }
}
```

## Out of Scope

- joins
- SQL
- sliding and session windows
- durable aggregation state
- UI form-builder support

## Validate Example Configs

```bash
cargo run --quiet --bin streamforge-validate -- examples/aggregation/orders-windowed-metrics.yaml
cargo run --quiet --bin streamforge-validate -- examples/aggregation/orders-quantiles.yaml
```
````

Add the public docs links explicitly:

```md
<!-- docs/index.md -->
## Build Pipelines

- [Usage Guide](USAGE.md)
- [Aggregations](AGGREGATIONS.md)
- [Advanced DSL Guide](ADVANCED_DSL_GUIDE.md)
- [YAML Configuration](YAML_CONFIGURATION.md)
```

```md
<!-- docs/DOCUMENTATION_INDEX.md -->
### Build Pipelines
1. [USAGE.md](USAGE.md) - End-to-end pipeline patterns
2. [AGGREGATIONS.md](AGGREGATIONS.md) - Windowed derived metrics and sketches
3. [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) - Full filter/transform reference
4. [YAML_CONFIGURATION.md](YAML_CONFIGURATION.md) - Author and review pipeline configs
5. [EXAMPLES.md](EXAMPLES.md) - Runnable configs and example packs
```

```md
<!-- docs/USAGE.md -->
## Derived Metrics with Aggregations

Use an `aggregation:` block inside a routing destination when you want StreamForge to emit a smaller metrics stream instead of forwarding every raw event.

- Aggregations run after the destination filter and value transform.
- Aggregated outputs go to the destination `output` topic.
- This mode is designed for simple rollups, not joins or SQL.
```

```yaml
# docs/_config.yml
defaults:
  - scope:
      path: "USAGE.md"
    values:
      has_children: true
  - scope:
      path: "AGGREGATIONS.md"
    values:
      parent: "Usage Guide"
```

- [ ] **Step 4: Verify the examples and docs wiring**

Run:

```bash
cargo run --quiet --bin streamforge-validate -- examples/aggregation/orders-windowed-metrics.yaml
cargo run --quiet --bin streamforge-validate -- examples/aggregation/orders-quantiles.yaml
rg -n "AGGREGATIONS|orders-windowed-metrics|approx_distinct|quantiles" docs examples/README.md docs/index.md docs/DOCUMENTATION_INDEX.md docs/_config.yml
```

Expected:
- both example validations PASS
- the `rg` command returns the new guide and example references

- [ ] **Step 5: Commit**

```bash
git add src/observability/metrics.rs docs/AGGREGATIONS.md docs/index.md docs/DOCUMENTATION_INDEX.md docs/USAGE.md docs/_config.yml examples/README.md examples/aggregation
git commit -m "docs: add aggregation guides and examples"
```

## Final Verification

- [ ] **Step 1: Run the focused aggregation test suites**

```bash
cargo test aggregation_config_validation -- --nocapture
cargo test aggregation_engine_runtime -- --nocapture
cargo test aggregation_sketches -- --nocapture
```

Expected:
- all aggregation-focused tests PASS

- [ ] **Step 2: Run the broader regression pass**

```bash
cargo test --quiet
```

Expected:
- PASS without breaking existing routing, filter, transform, retry, or docs validation behavior

- [ ] **Step 3: Final commit for any verification-only fixes**

```bash
git add Cargo.toml src/config.rs src/lib.rs src/aggregation src/processor.rs src/processor_with_retry.rs src/main.rs src/bin/validate.rs src/observability/metrics.rs tests/aggregation_config_validation.rs tests/aggregation_engine_runtime.rs tests/aggregation_sketches.rs docs/AGGREGATIONS.md docs/index.md docs/DOCUMENTATION_INDEX.md docs/USAGE.md docs/_config.yml examples/README.md examples/aggregation
git commit -m "test: finalize aggregation verification" || true
```

## Notes for the Implementer

- Keep aggregation state local and in-memory for this first release.
- Use the message timestamp when present; fall back to current wall-clock time when absent.
- Treat filter/transform errors inside aggregation destinations with the existing per-destination `error_policy`.
- Do not broaden this plan into SQL, joins, session windows, or UI form-builder work.
- The `datasketches` crate documentation for `0.2.0` explicitly warns that the Rust component is early in development, so keep sketch usage behind internal wrappers and avoid leaking crate-specific types into config or public APIs.
