use crate::{
    config::{AggregationMetricConfig, AggregationOp},
    jsonpath::JsonPath,
    MirrorMakerError, Result,
};
use serde_json::{Number, Value};

#[derive(Debug, Clone)]
pub(crate) struct CompiledMetric {
    pub(crate) name: String,
    kind: MetricKind,
}

#[derive(Debug, Clone)]
enum MetricKind {
    Count,
    Sum { path: JsonPath },
    Avg { path: JsonPath },
}

#[derive(Debug, Clone)]
pub(crate) enum MetricInput {
    Count,
    Numeric(f64),
}

#[derive(Debug, Clone)]
pub(crate) enum MetricState {
    Count(u64),
    Sum(f64),
    Avg { sum: f64, count: u64 },
}

impl CompiledMetric {
    pub(crate) fn new(config: AggregationMetricConfig) -> Result<Self> {
        let kind = match config.op {
            AggregationOp::Count => MetricKind::Count,
            AggregationOp::Sum => MetricKind::Sum {
                path: required_metric_path(&config)?,
            },
            AggregationOp::Avg => MetricKind::Avg {
                path: required_metric_path(&config)?,
            },
            unsupported => {
                return Err(MirrorMakerError::Config(format!(
                    "aggregation op '{unsupported:?}' is not supported in Task 2"
                )));
            }
        };

        Ok(Self {
            name: config.name,
            kind,
        })
    }

    pub(crate) fn initial_state(&self) -> MetricState {
        match self.kind {
            MetricKind::Count => MetricState::Count(0),
            MetricKind::Sum { .. } => MetricState::Sum(0.0),
            MetricKind::Avg { .. } => MetricState::Avg { sum: 0.0, count: 0 },
        }
    }

    pub(crate) fn extract_input(&self, value: &Value) -> Result<MetricInput> {
        match &self.kind {
            MetricKind::Count => Ok(MetricInput::Count),
            MetricKind::Sum { path } | MetricKind::Avg { path } => {
                let number = extract_metric_number(&self.name, path, value)?;
                Ok(MetricInput::Numeric(number))
            }
        }
    }
}

impl MetricState {
    pub(crate) fn apply(&mut self, input: &MetricInput) {
        match (self, input) {
            (MetricState::Count(count), MetricInput::Count) => *count += 1,
            (MetricState::Sum(sum), MetricInput::Numeric(value)) => *sum += value,
            (MetricState::Avg { sum, count }, MetricInput::Numeric(value)) => {
                *sum += value;
                *count += 1;
            }
            _ => unreachable!("compiled metric input/state mismatch"),
        }
    }

    pub(crate) fn as_json_value(&self, metric_name: &str) -> Result<Value> {
        match self {
            MetricState::Count(count) => Ok(Value::Number(Number::from(*count))),
            MetricState::Sum(sum) => finite_number_value(metric_name, *sum),
            MetricState::Avg { sum, count } => {
                let average = if *count == 0 {
                    0.0
                } else {
                    *sum / (*count as f64)
                };
                finite_number_value(metric_name, average)
            }
        }
    }
}

fn required_metric_path(config: &AggregationMetricConfig) -> Result<JsonPath> {
    let path = config.path.as_deref().ok_or_else(|| {
        MirrorMakerError::Config(format!("{} metrics require path", op_name(config.op)))
    })?;
    Ok(JsonPath::new(path))
}

fn extract_metric_number(metric_name: &str, path: &JsonPath, value: &Value) -> Result<f64> {
    if let Some(number) = path.extract_f64(value) {
        return finite_value(metric_name, &path.path, number);
    }

    if path.extract_owned(value).is_none() {
        return Err(MirrorMakerError::JsonPathNotFound {
            path: path.path.clone(),
            value: Some(value.to_string()),
        });
    }

    Err(MirrorMakerError::Processing(format!(
        "aggregation metric '{metric_name}' requires a numeric value at path '{}'",
        path.path
    )))
}

fn finite_value(metric_name: &str, path: &str, value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(MirrorMakerError::Processing(format!(
            "aggregation metric '{metric_name}' produced a non-finite value at path '{path}'"
        )))
    }
}

fn finite_number_value(metric_name: &str, value: f64) -> Result<Value> {
    if !value.is_finite() {
        return Err(MirrorMakerError::Processing(format!(
            "aggregation metric '{metric_name}' produced a non-finite value"
        )));
    }

    Number::from_f64(value).map(Value::Number).ok_or_else(|| {
        MirrorMakerError::Processing(format!(
            "aggregation metric '{metric_name}' produced a non-finite value"
        ))
    })
}

fn op_name(op: AggregationOp) -> &'static str {
    match op {
        AggregationOp::Count => "count",
        AggregationOp::Sum => "sum",
        AggregationOp::Avg => "avg",
        AggregationOp::ApproxDistinct => "approx_distinct",
        AggregationOp::Quantiles => "quantiles",
    }
}
