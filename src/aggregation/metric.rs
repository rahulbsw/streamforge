use crate::{
    config::{AggregationMetricConfig, AggregationOp},
    jsonpath::JsonPath,
    MirrorMakerError, Result,
};
use datasketches::{
    hll::{HllSketch, HllType},
    tdigest::TDigestMut,
};
use serde_json::{Map, Number, Value};
use std::collections::HashSet;

const APPROX_DISTINCT_LG_K: u8 = 12;
const QUANTILES_K: u16 = 100;

#[derive(Debug, Clone)]
pub(crate) struct CompiledMetric {
    pub(crate) name: String,
    kind: MetricKind,
}

#[derive(Debug, Clone)]
enum MetricKind {
    Count,
    Sum {
        path: JsonPath,
    },
    Avg {
        path: JsonPath,
    },
    ApproxDistinct {
        path: JsonPath,
    },
    Quantiles {
        path: JsonPath,
        percentiles: Vec<f64>,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum MetricInput {
    Count,
    Numeric(f64),
    Distinct(String),
}

#[derive(Debug, Clone)]
pub(crate) enum MetricState {
    Count(u64),
    Sum(f64),
    Avg {
        sum: f64,
        count: u64,
    },
    ApproxDistinct(HllSketch),
    Quantiles {
        percentiles: Vec<f64>,
        digest: TDigestMut,
    },
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
            AggregationOp::ApproxDistinct => MetricKind::ApproxDistinct {
                path: required_metric_path(&config)?,
            },
            AggregationOp::Quantiles => MetricKind::Quantiles {
                path: required_metric_path(&config)?,
                percentiles: validated_percentiles(&config)?,
            },
        };

        Ok(Self {
            name: config.name,
            kind,
        })
    }

    pub(crate) fn initial_state(&self) -> MetricState {
        match &self.kind {
            MetricKind::Count => MetricState::Count(0),
            MetricKind::Sum { .. } => MetricState::Sum(0.0),
            MetricKind::Avg { .. } => MetricState::Avg { sum: 0.0, count: 0 },
            MetricKind::ApproxDistinct { .. } => {
                MetricState::ApproxDistinct(HllSketch::new(APPROX_DISTINCT_LG_K, HllType::Hll8))
            }
            MetricKind::Quantiles { percentiles, .. } => MetricState::Quantiles {
                percentiles: percentiles.clone(),
                digest: TDigestMut::new(QUANTILES_K),
            },
        }
    }

    pub(crate) fn extract_input(&self, value: &Value) -> Result<MetricInput> {
        match &self.kind {
            MetricKind::Count => Ok(MetricInput::Count),
            MetricKind::Sum { path } | MetricKind::Avg { path } => {
                let number = extract_metric_number(&self.name, path, value)?;
                Ok(MetricInput::Numeric(number))
            }
            MetricKind::ApproxDistinct { path } => Ok(MetricInput::Distinct(
                extract_metric_distinct_value(path, value)?,
            )),
            MetricKind::Quantiles { path, .. } => {
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
            (MetricState::ApproxDistinct(sketch), MetricInput::Distinct(value)) => {
                sketch.update(value)
            }
            (MetricState::Quantiles { digest, .. }, MetricInput::Numeric(value)) => {
                digest.update(*value);
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
            MetricState::ApproxDistinct(sketch) => {
                finite_number_value(metric_name, sketch.estimate())
            }
            MetricState::Quantiles {
                percentiles,
                digest,
            } => quantiles_json_value(metric_name, percentiles, digest),
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

fn extract_metric_distinct_value(path: &JsonPath, value: &Value) -> Result<String> {
    let extracted =
        path.extract_owned(value)
            .ok_or_else(|| MirrorMakerError::JsonPathNotFound {
                path: path.path.clone(),
                value: Some(value.to_string()),
            })?;

    Ok(match extracted {
        Value::Null => "null".to_string(),
        Value::Bool(value) => format!("b:{value}"),
        Value::Number(value) => format!("n:{value}"),
        Value::String(value) => format!("s:{value}"),
        other => format!("j:{other}"),
    })
}

fn validated_percentiles(config: &AggregationMetricConfig) -> Result<Vec<f64>> {
    let percentiles = config.percentiles.clone().ok_or_else(|| {
        MirrorMakerError::Config("quantiles metrics require percentiles".to_string())
    })?;

    if percentiles.is_empty() {
        return Err(MirrorMakerError::Config(
            "quantiles metrics require percentiles".to_string(),
        ));
    }

    let mut keys = HashSet::new();
    for percentile in &percentiles {
        if !percentile.is_finite() {
            return Err(MirrorMakerError::Config(format!(
                "quantiles metric '{}' has non-finite percentile",
                config.name
            )));
        }
        if !(0.0..=1.0).contains(percentile) {
            return Err(MirrorMakerError::Config(format!(
                "quantiles metric '{}' percentile must be in [0.0, 1.0], got {}",
                config.name, percentile
            )));
        }

        let key = percentile_key(*percentile);
        if !keys.insert(key.clone()) {
            return Err(MirrorMakerError::Config(format!(
                "quantiles metric '{}' contains duplicate percentile key '{}'",
                config.name, key
            )));
        }
    }

    Ok(percentiles)
}

fn quantiles_json_value(
    metric_name: &str,
    percentiles: &[f64],
    digest: &TDigestMut,
) -> Result<Value> {
    let frozen = digest.clone().freeze();
    let mut values = Map::new();
    for percentile in percentiles {
        let key = percentile_key(*percentile);
        let quantile = frozen.quantile(*percentile).ok_or_else(|| {
            MirrorMakerError::Processing(format!(
                "aggregation metric '{metric_name}' could not compute quantile '{key}'"
            ))
        })?;
        values.insert(key, finite_number_value(metric_name, quantile)?);
    }
    Ok(Value::Object(values))
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

fn percentile_key(percentile: f64) -> String {
    format!("p{}", percentile)
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
