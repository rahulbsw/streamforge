use crate::envelope::MessageEnvelope;
use crate::wasm::bindings::envelope_transform::exports::streamforge::udf::types as envelope_types;
use crate::wasm::bindings::filter::exports::streamforge::udf::types as filter_types;
use crate::wasm::error::{error, WasmError, WasmErrorKind};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

const MAX_KAFKA_HEADER_NAME_BYTES: usize = i16::MAX as usize;

/// Validated mutable fields returned by a full-envelope UDF.
#[derive(Debug, Clone, PartialEq)]
pub struct WasmEnvelopeOutput {
    pub key: Option<Value>,
    pub value: Value,
    pub headers: HashMap<String, Vec<u8>>,
    pub timestamp: Option<i64>,
}

pub(crate) fn filter_input(
    module: &str,
    envelope: &MessageEnvelope,
    max_input_bytes: usize,
) -> Result<(filter_types::EnvelopeInput, usize), WasmError> {
    let encoded = encode_envelope(module, envelope, max_input_bytes)?;
    let size = encoded.size;
    Ok((
        filter_types::EnvelopeInput {
            key: encoded.key,
            value: encoded.value,
            headers: encoded
                .headers
                .into_iter()
                .map(|(name, value)| filter_types::Header { name, value })
                .collect(),
            timestamp: encoded.timestamp,
            source_topic: encoded.source_topic,
            source_partition: encoded.source_partition,
            source_offset: encoded.source_offset,
        },
        size,
    ))
}

pub(crate) fn envelope_transform_input(
    module: &str,
    envelope: &MessageEnvelope,
    max_input_bytes: usize,
) -> Result<(envelope_types::EnvelopeInput, usize), WasmError> {
    let encoded = encode_envelope(module, envelope, max_input_bytes)?;
    let size = encoded.size;
    Ok((
        envelope_types::EnvelopeInput {
            key: encoded.key,
            value: encoded.value,
            headers: encoded
                .headers
                .into_iter()
                .map(|(name, value)| envelope_types::Header { name, value })
                .collect(),
            timestamp: encoded.timestamp,
            source_topic: encoded.source_topic,
            source_partition: encoded.source_partition,
            source_offset: encoded.source_offset,
        },
        size,
    ))
}

pub(crate) fn value_input(
    module: &str,
    value: &Value,
    max_input_bytes: usize,
) -> Result<Vec<u8>, WasmError> {
    let bytes = serde_json::to_vec(value).map_err(|failure| {
        error(
            WasmErrorKind::InvalidOutput,
            Some(module),
            format!("host value could not be encoded as JSON: {failure}"),
        )
    })?;
    ensure_limit(
        module,
        "input",
        bytes.len(),
        max_input_bytes,
        WasmErrorKind::InputLimit,
    )?;
    Ok(bytes)
}

pub(crate) fn decode_value_output(
    module: &str,
    bytes: Vec<u8>,
    max_output_bytes: usize,
) -> Result<Value, WasmError> {
    ensure_limit(
        module,
        "output",
        bytes.len(),
        max_output_bytes,
        WasmErrorKind::OutputLimit,
    )?;
    serde_json::from_slice(&bytes).map_err(|failure| {
        error(
            WasmErrorKind::InvalidOutput,
            Some(module),
            format!("UDF returned invalid JSON: {failure}"),
        )
    })
}

pub(crate) fn decode_envelope_output(
    module: &str,
    output: envelope_types::EnvelopeOutput,
    max_output_bytes: usize,
) -> Result<(WasmEnvelopeOutput, usize), WasmError> {
    let mut size = output.value.len();
    if let Some(key) = &output.key {
        size = size.checked_add(key.len()).ok_or_else(|| {
            error(
                WasmErrorKind::OutputLimit,
                Some(module),
                "UDF envelope output size overflow",
            )
        })?;
    }

    let mut headers = HashMap::with_capacity(output.headers.len());
    let mut names = HashSet::with_capacity(output.headers.len());
    for header in output.headers {
        validate_header_name(module, &header.name, WasmErrorKind::InvalidOutput)?;
        if !names.insert(header.name.clone()) {
            return Err(error(
                WasmErrorKind::InvalidOutput,
                Some(module),
                format!("UDF returned duplicate header name {:?}", header.name),
            ));
        }
        size = size
            .checked_add(header.name.len())
            .and_then(|total| total.checked_add(header.value.len()))
            .ok_or_else(|| {
                error(
                    WasmErrorKind::OutputLimit,
                    Some(module),
                    "UDF envelope output size overflow",
                )
            })?;
        if size > max_output_bytes {
            return Err(limit_error(
                module,
                "output",
                size,
                max_output_bytes,
                WasmErrorKind::OutputLimit,
            ));
        }
        headers.insert(header.name, header.value);
    }

    ensure_limit(
        module,
        "output",
        size,
        max_output_bytes,
        WasmErrorKind::OutputLimit,
    )?;

    let key = output
        .key
        .map(|bytes| {
            serde_json::from_slice(&bytes).map_err(|failure| {
                error(
                    WasmErrorKind::InvalidOutput,
                    Some(module),
                    format!("UDF returned an invalid JSON key: {failure}"),
                )
            })
        })
        .transpose()?;
    let value = serde_json::from_slice(&output.value).map_err(|failure| {
        error(
            WasmErrorKind::InvalidOutput,
            Some(module),
            format!("UDF returned an invalid JSON value: {failure}"),
        )
    })?;

    Ok((
        WasmEnvelopeOutput {
            key,
            value,
            headers,
            timestamp: output.timestamp,
        },
        size,
    ))
}

#[derive(Debug)]
struct EncodedEnvelope {
    size: usize,
    key: Option<Vec<u8>>,
    value: Vec<u8>,
    headers: Vec<(String, Vec<u8>)>,
    timestamp: Option<i64>,
    source_topic: Option<String>,
    source_partition: Option<i32>,
    source_offset: Option<i64>,
}

fn encode_envelope(
    module: &str,
    envelope: &MessageEnvelope,
    max_input_bytes: usize,
) -> Result<EncodedEnvelope, WasmError> {
    let key = envelope
        .key
        .as_ref()
        .map(serde_json::to_vec)
        .transpose()
        .map_err(|failure| {
            error(
                WasmErrorKind::InvalidOutput,
                Some(module),
                format!("host key could not be encoded as JSON: {failure}"),
            )
        })?;
    let value = serde_json::to_vec(envelope.value.as_ref()).map_err(|failure| {
        error(
            WasmErrorKind::InvalidOutput,
            Some(module),
            format!("host value could not be encoded as JSON: {failure}"),
        )
    })?;

    let mut size = value.len();
    if let Some(key) = &key {
        size = size.checked_add(key.len()).ok_or_else(|| {
            limit_error(
                module,
                "input",
                usize::MAX,
                max_input_bytes,
                WasmErrorKind::InputLimit,
            )
        })?;
    }

    let mut headers: Vec<_> = envelope
        .headers
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    headers.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    for (name, header_value) in &headers {
        validate_header_name(module, name, WasmErrorKind::InputLimit)?;
        size = size
            .checked_add(name.len())
            .and_then(|total| total.checked_add(header_value.len()))
            .ok_or_else(|| {
                limit_error(
                    module,
                    "input",
                    usize::MAX,
                    max_input_bytes,
                    WasmErrorKind::InputLimit,
                )
            })?;
    }
    if let Some(topic) = &envelope.topic {
        size = size.checked_add(topic.len()).ok_or_else(|| {
            limit_error(
                module,
                "input",
                usize::MAX,
                max_input_bytes,
                WasmErrorKind::InputLimit,
            )
        })?;
    }
    ensure_limit(
        module,
        "input",
        size,
        max_input_bytes,
        WasmErrorKind::InputLimit,
    )?;

    Ok(EncodedEnvelope {
        size,
        key,
        value,
        headers,
        timestamp: envelope.timestamp,
        source_topic: envelope.topic.clone(),
        source_partition: envelope.partition,
        source_offset: envelope.offset,
    })
}

fn validate_header_name(module: &str, name: &str, kind: WasmErrorKind) -> Result<(), WasmError> {
    if name.is_empty() || name.len() > MAX_KAFKA_HEADER_NAME_BYTES || name.contains('\0') {
        return Err(error(
            kind,
            Some(module),
            format!("header name must be 1-{MAX_KAFKA_HEADER_NAME_BYTES} bytes and contain no NUL"),
        ));
    }
    Ok(())
}

fn ensure_limit(
    module: &str,
    direction: &str,
    actual: usize,
    maximum: usize,
    kind: WasmErrorKind,
) -> Result<(), WasmError> {
    if actual > maximum {
        return Err(limit_error(module, direction, actual, maximum, kind));
    }
    Ok(())
}

fn limit_error(
    module: &str,
    direction: &str,
    actual: usize,
    maximum: usize,
    kind: WasmErrorKind,
) -> WasmError {
    error(
        kind,
        Some(module),
        format!("{direction} is {actual} bytes, exceeding configured limit {maximum}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn value_output_must_be_valid_and_bounded_json() {
        assert_eq!(
            decode_value_output("value", br#"{"ok":true}"#.to_vec(), 64).unwrap(),
            json!({"ok": true})
        );
        assert_eq!(
            decode_value_output("value", b"not-json".to_vec(), 64)
                .unwrap_err()
                .kind(),
            WasmErrorKind::InvalidOutput
        );
        assert_eq!(
            decode_value_output("value", vec![b'x'; 65], 64)
                .unwrap_err()
                .kind(),
            WasmErrorKind::OutputLimit
        );
    }

    #[test]
    fn envelope_input_is_deterministic_and_bounded() {
        let mut envelope = MessageEnvelope::new(json!({"ok": true}));
        std::sync::Arc::make_mut(&mut envelope.headers).insert("z".into(), vec![2]);
        std::sync::Arc::make_mut(&mut envelope.headers).insert("a".into(), vec![1]);
        let encoded = encode_envelope("filter", &envelope, 1024).unwrap();
        assert_eq!(encoded.headers[0].0, "a");
        assert_eq!(encoded.headers[1].0, "z");
        assert_eq!(
            encode_envelope("filter", &envelope, 1).unwrap_err().kind(),
            WasmErrorKind::InputLimit
        );
    }

    #[test]
    fn envelope_output_rejects_malformed_json_and_headers_transactionally() {
        let output = |value, headers| envelope_types::EnvelopeOutput {
            key: Some(br#""key""#.to_vec()),
            value,
            headers,
            timestamp: Some(42),
        };

        assert_eq!(
            decode_envelope_output("envelope", output(b"not-json".to_vec(), vec![]), 1024)
                .unwrap_err()
                .kind(),
            WasmErrorKind::InvalidOutput
        );

        let duplicate_headers = vec![
            envelope_types::Header {
                name: "duplicate".into(),
                value: vec![1],
            },
            envelope_types::Header {
                name: "duplicate".into(),
                value: vec![2],
            },
        ];
        assert_eq!(
            decode_envelope_output(
                "envelope",
                output(br#"{"ok":true}"#.to_vec(), duplicate_headers),
                1024,
            )
            .unwrap_err()
            .kind(),
            WasmErrorKind::InvalidOutput
        );

        let invalid_header = vec![envelope_types::Header {
            name: "bad\0name".into(),
            value: vec![],
        }];
        assert_eq!(
            decode_envelope_output(
                "envelope",
                output(br#"{"ok":true}"#.to_vec(), invalid_header),
                1024,
            )
            .unwrap_err()
            .kind(),
            WasmErrorKind::InvalidOutput
        );
    }
}
