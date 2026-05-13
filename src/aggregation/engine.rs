use crate::{
    aggregation::{
        metric::{CompiledMetric, MetricInput, MetricState},
        record::{AggregateEmission, GroupKey, WindowDescriptor},
    },
    config::{AggregationConfig, AggregationWindowType},
    jsonpath::JsonPath,
    MirrorMakerError, Result,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug)]
pub struct AggregationEngine {
    output_topic: String,
    window_type: AggregationWindowType,
    window_size_ms: u64,
    window_size_seconds: u64,
    min_open_window_start_ms: u64,
    group_by: Vec<GroupByField>,
    metrics: Vec<CompiledMetric>,
    windows: BTreeMap<u64, BTreeMap<GroupKey, AggregateBucket>>,
    pending_flush: Option<Vec<AggregateEmission>>,
}

#[derive(Debug)]
struct GroupByField {
    name: String,
    path: JsonPath,
}

#[derive(Debug)]
struct AggregateBucket {
    metrics: Vec<MetricState>,
}

impl AggregationEngine {
    pub fn new(config: AggregationConfig, output_topic: String) -> Result<Self> {
        if output_topic.trim().is_empty() {
            return Err(MirrorMakerError::Config(
                "aggregation output topic cannot be empty".to_string(),
            ));
        }
        if config.window.size_seconds == 0 {
            return Err(MirrorMakerError::Config(
                "window size_seconds must be > 0".to_string(),
            ));
        }

        let window_size_ms = config
            .window
            .size_seconds
            .checked_mul(1_000)
            .ok_or_else(|| {
                MirrorMakerError::Config("window size overflows milliseconds".to_string())
            })?;

        let mut group_names = HashSet::new();
        let group_by = config
            .group_by
            .into_iter()
            .map(|field| {
                if !group_names.insert(field.name.clone()) {
                    return Err(MirrorMakerError::Config(format!(
                        "duplicate aggregation group_by name '{}'",
                        field.name
                    )));
                }

                Ok(GroupByField {
                    name: field.name,
                    path: JsonPath::new(&field.path),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let metrics = config
            .metrics
            .into_iter()
            .map(CompiledMetric::new)
            .collect::<Result<Vec<_>>>()?;
        let mut metric_names = HashSet::new();
        for metric in &metrics {
            if !metric_names.insert(metric.name.clone()) {
                return Err(MirrorMakerError::Config(format!(
                    "duplicate aggregation metric name '{}'",
                    metric.name
                )));
            }
        }

        Ok(Self {
            output_topic,
            window_type: config.window.window_type,
            window_size_ms,
            window_size_seconds: config.window.size_seconds,
            min_open_window_start_ms: 0,
            group_by,
            metrics,
            windows: BTreeMap::new(),
            pending_flush: None,
        })
    }

    pub fn observe(&mut self, value: &Value, timestamp_ms: u64) -> Result<()> {
        let window_start = timestamp_ms - (timestamp_ms % self.window_size_ms);
        if window_start < self.min_open_window_start_ms {
            return Err(MirrorMakerError::Processing(format!(
                "late event for flushed window: window_start_ms={} closed_before_ms={}",
                window_start, self.min_open_window_start_ms
            )));
        }

        let group_key = self.group_key_for(value)?;
        let inputs = self
            .metrics
            .iter()
            .map(|metric| metric.extract_input(value))
            .collect::<Result<Vec<_>>>()?;

        let groups = self.windows.entry(window_start).or_default();
        let bucket = groups
            .entry(group_key)
            .or_insert_with(|| AggregateBucket::new(&self.metrics));
        bucket.apply(&inputs);

        Ok(())
    }

    pub fn prepare_flush_expired(&mut self, now_ms: u64) -> Result<Vec<AggregateEmission>> {
        if let Some(pending_flush) = &self.pending_flush {
            return Ok(pending_flush.clone());
        }

        let latest_expired_start = match now_ms.checked_sub(self.window_size_ms) {
            Some(value) => value,
            None => return Ok(Vec::new()),
        };
        let expired_starts = self
            .windows
            .range(..=latest_expired_start)
            .map(|(start, _)| *start)
            .collect::<Vec<_>>();

        let mut emitted = Vec::new();
        for start_ms in expired_starts {
            if let Some(groups) = self.windows.get(&start_ms) {
                let window = WindowDescriptor {
                    start_ms,
                    end_ms: start_ms + self.window_size_ms,
                    window_type: self.window_type,
                    size_seconds: self.window_size_seconds,
                };

                for (group_key, bucket) in groups {
                    emitted.push(AggregateEmission::new(
                        self.output_topic.clone(),
                        group_key.clone(),
                        window,
                        bucket.to_metrics_json(&self.metrics)?,
                    ));
                }
            }
        }

        let flushed_starts = self
            .windows
            .range(..=latest_expired_start)
            .map(|(start, _)| *start)
            .collect::<Vec<_>>();

        for start_ms in &flushed_starts {
            self.windows.remove(start_ms);
        }

        if !flushed_starts.is_empty() {
            self.pending_flush = Some(emitted.clone());
        }

        self.min_open_window_start_ms = self
            .min_open_window_start_ms
            .max(now_ms - (now_ms % self.window_size_ms));

        Ok(emitted)
    }

    pub fn commit_flush(&mut self) {
        self.pending_flush = None;
    }

    pub fn flush_expired(&mut self, now_ms: u64) -> Result<Vec<AggregateEmission>> {
        let emitted = self.prepare_flush_expired(now_ms)?;
        if emitted.is_empty() {
            return Ok(emitted);
        }

        self.commit_flush();
        Ok(emitted)
    }

    pub fn open_window_count(&self) -> usize {
        self.windows.len()
    }

    fn group_key_for(&self, value: &Value) -> Result<GroupKey> {
        let fields = self
            .group_by
            .iter()
            .map(|field| {
                field
                    .path
                    .extract_owned(value)
                    .map(|group_value| (field.name.clone(), group_value))
                    .ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                        path: field.path.path.clone(),
                        value: Some(value.to_string()),
                    })
            })
            .collect::<Result<Vec<_>>>()?;

        GroupKey::new(fields)
    }
}

impl AggregateBucket {
    fn new(metrics: &[CompiledMetric]) -> Self {
        Self {
            metrics: metrics.iter().map(CompiledMetric::initial_state).collect(),
        }
    }

    fn apply(&mut self, inputs: &[MetricInput]) {
        for (state, input) in self.metrics.iter_mut().zip(inputs.iter()) {
            state.apply(input);
        }
    }

    fn to_metrics_json(&self, metrics: &[CompiledMetric]) -> Result<Map<String, Value>> {
        let mut values = Map::new();
        for (metric, state) in metrics.iter().zip(self.metrics.iter()) {
            values.insert(metric.name.clone(), state.as_json_value(&metric.name)?);
        }
        Ok(values)
    }
}
