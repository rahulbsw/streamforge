use crate::{config::AggregationWindowType, MirrorMakerError, Result};
use serde::Serialize;
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub struct GroupKey {
    encoded: String,
    fields: Vec<(String, Value)>,
}

#[derive(Debug, Clone)]
pub struct AggregateEmission {
    pub output_topic: String,
    pub group_key: GroupKey,
    pub value: Value,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct WindowDescriptor {
    pub(crate) start_ms: u64,
    pub(crate) end_ms: u64,
    pub(crate) window_type: AggregationWindowType,
    pub(crate) size_seconds: u64,
}

impl GroupKey {
    pub(crate) fn new(fields: Vec<(String, Value)>) -> Result<Self> {
        let canonical_fields = fields
            .iter()
            .map(|(name, value)| CanonicalGroupField {
                name: name.as_str(),
                value,
            })
            .collect::<Vec<_>>();
        let encoded = serde_json::to_string(&canonical_fields)
            .map_err(|err| MirrorMakerError::Serialization(err.to_string()))?;

        Ok(Self { encoded, fields })
    }

    pub fn as_str(&self) -> &str {
        &self.encoded
    }

    pub(crate) fn to_group_value(&self) -> Value {
        let mut group = Map::new();
        for (name, value) in &self.fields {
            group.insert(name.clone(), value.clone());
        }
        Value::Object(group)
    }
}

impl AggregateEmission {
    pub(crate) fn new(
        output_topic: String,
        group_key: GroupKey,
        window: WindowDescriptor,
        metrics: Map<String, Value>,
    ) -> Self {
        let value = Value::Object(Map::from_iter([
            ("window".to_string(), window.to_json_value()),
            ("group".to_string(), group_key.to_group_value()),
            ("metrics".to_string(), Value::Object(metrics)),
        ]));

        Self {
            output_topic,
            group_key,
            value,
        }
    }
}

impl WindowDescriptor {
    fn to_json_value(self) -> Value {
        Value::Object(Map::from_iter([
            ("start_ms".to_string(), Value::from(self.start_ms)),
            ("end_ms".to_string(), Value::from(self.end_ms)),
            (
                "type".to_string(),
                Value::String(match self.window_type {
                    AggregationWindowType::Tumbling => "tumbling".to_string(),
                }),
            ),
            ("size_seconds".to_string(), Value::from(self.size_seconds)),
        ]))
    }
}

impl PartialEq for GroupKey {
    fn eq(&self, other: &Self) -> bool {
        self.encoded == other.encoded
    }
}

impl Eq for GroupKey {}

impl PartialOrd for GroupKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GroupKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.encoded.cmp(&other.encoded)
    }
}

impl Hash for GroupKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.encoded.hash(state);
    }
}

#[derive(Serialize)]
struct CanonicalGroupField<'a> {
    name: &'a str,
    value: &'a Value,
}
