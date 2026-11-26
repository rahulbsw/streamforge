use crate::config::SinkConfig;
use crate::error::MirrorMakerError;
use crate::Result;
use iceberg::TableIdent;
use serde_json::Value;
use std::collections::HashMap;

const DEFAULT_BATCH_SIZE: usize = 1000;
const DEFAULT_BATCH_INTERVAL_MS: u64 = 1000;

pub(super) fn parse_config(
    sink: &SinkConfig,
    output: &str,
) -> Result<(String, HashMap<String, String>, TableIdent, usize, u64)> {
    let iceberg = sink
        .properties
        .get("iceberg")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            MirrorMakerError::Config(
                "sink.iceberg must be an object containing catalog and properties".into(),
            )
        })?;
    if let Some(key) = iceberg
        .keys()
        .find(|key| !["catalog", "table", "properties"].contains(&key.as_str()))
    {
        return Err(MirrorMakerError::Config(format!(
            "unknown sink.iceberg option '{key}'; put native Iceberg settings under properties"
        )));
    }

    let catalog = iceberg
        .get("catalog")
        .and_then(Value::as_object)
        .ok_or_else(|| MirrorMakerError::Config("sink.iceberg.catalog must be an object".into()))?;
    let catalog_name = match catalog.get("type") {
        Some(Value::String(kind)) if kind == "rest" => kind.clone(),
        Some(Value::String(_)) => {
            return Err(MirrorMakerError::Config(
                "Iceberg sink currently supports the REST catalog".into(),
            ))
        }
        _ => {
            return Err(MirrorMakerError::Config(
                "sink.iceberg.catalog.type must be 'rest'".into(),
            ))
        }
    };

    let mut properties = HashMap::new();
    insert_string_properties(catalog, &mut properties, &["type"])?;
    match iceberg.get("properties") {
        Some(Value::Object(native_properties)) => {
            insert_string_properties(native_properties, &mut properties, &[])?;
        }
        Some(_) => {
            return Err(MirrorMakerError::Config(
                "sink.iceberg.properties must be an object of native Iceberg properties".into(),
            ))
        }
        None => {}
    }
    if !properties.contains_key("uri") && !properties.contains_key("rest.uri") {
        return Err(MirrorMakerError::Config(
            "sink.iceberg.catalog.uri is required for the REST catalog".into(),
        ));
    }

    let table = match iceberg.get("table") {
        Some(Value::String(table)) if !table.is_empty() => table.as_str(),
        Some(_) => {
            return Err(MirrorMakerError::Config(
                "sink.iceberg.table must be a fully qualified namespace.table string".into(),
            ))
        }
        None => output,
    };
    let table_ident = parse_table_ident(table)?;

    let flush = match sink.properties.get("flush") {
        Some(Value::Object(flush)) => Some(flush),
        Some(_) => {
            return Err(MirrorMakerError::Config(
                "sink.flush must be an object".into(),
            ))
        }
        None => None,
    };
    if let Some(flush) = flush {
        if let Some(key) = flush
            .keys()
            .find(|key| !["max_buffered_events", "interval_ms"].contains(&key.as_str()))
        {
            return Err(MirrorMakerError::Config(format!(
                "unknown Iceberg flush option '{key}'"
            )));
        }
    }
    let batch_size = usize::try_from(parse_positive_flush_value(
        flush,
        "max_buffered_events",
        DEFAULT_BATCH_SIZE as u64,
    )?)
    .map_err(|_| {
        MirrorMakerError::Config("flush.max_buffered_events exceeds this platform's limit".into())
    })?;
    let batch_interval =
        parse_positive_flush_value(flush, "interval_ms", DEFAULT_BATCH_INTERVAL_MS)?;

    Ok((
        catalog_name,
        properties,
        table_ident,
        batch_size,
        batch_interval,
    ))
}

fn insert_string_properties(
    properties: &serde_json::Map<String, Value>,
    output: &mut HashMap<String, String>,
    ignored_keys: &[&str],
) -> Result<()> {
    reject_inline_secrets(properties)?;
    for (key, value) in properties {
        if ignored_keys.contains(&key.as_str()) {
            continue;
        }
        let value = value.as_str().ok_or_else(|| {
            MirrorMakerError::Config(format!("Iceberg property '{key}' must be a string"))
        })?;
        if url_contains_inline_credentials(value) {
            return Err(MirrorMakerError::Config(
                "inline credentials in Iceberg property URLs are rejected".into(),
            ));
        }
        output.insert(key.clone(), value.to_string());
    }
    Ok(())
}

fn parse_positive_flush_value(
    flush: Option<&serde_json::Map<String, Value>>,
    key: &str,
    default: u64,
) -> Result<u64> {
    match flush.and_then(|flush| flush.get(key)) {
        None => Ok(default),
        Some(Value::Number(number)) => {
            number.as_u64().filter(|value| *value > 0).ok_or_else(|| {
                MirrorMakerError::Config(format!("flush.{key} must be a positive integer"))
            })
        }
        Some(_) => Err(MirrorMakerError::Config(format!(
            "flush.{key} must be a positive integer"
        ))),
    }
}

fn reject_inline_secrets(properties: &serde_json::Map<String, Value>) -> Result<()> {
    const SECRET_MARKERS: &[&str] = &[
        "password",
        "secret",
        "token",
        "credential",
        "access-key",
        "private-key",
        "api-key",
        "authorization",
        "signature",
    ];
    if let Some(key) = properties.keys().find(|key| {
        let key = key.to_ascii_lowercase().replace('_', "-");
        !["-file", ".file"]
            .iter()
            .any(|suffix| key.ends_with(suffix))
            && SECRET_MARKERS.iter().any(|marker| key.contains(marker))
    }) {
        return Err(MirrorMakerError::Config(format!(
            "inline Iceberg credential property '{key}' is rejected; use a supported mounted credential file or environment-based provider"
        )));
    }
    Ok(())
}

fn url_contains_inline_credentials(value: &str) -> bool {
    let Some((scheme, remainder)) = value.split_once("://") else {
        return false;
    };
    if scheme.is_empty() {
        return false;
    }
    let authority = remainder.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return true;
    }
    let query = remainder
        .split_once('?')
        .map(|(_, query)| query.split('#').next().unwrap_or_default())
        .unwrap_or_default();
    query.split('&').any(|parameter| {
        let key = parameter
            .split_once('=')
            .map(|(key, _)| key)
            .unwrap_or(parameter)
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>();
        [
            "password",
            "token",
            "credential",
            "secret",
            "accesskey",
            "privatekey",
            "apikey",
            "authorization",
            "signature",
        ]
        .iter()
        .any(|marker| key.contains(marker))
    })
}

fn parse_table_ident(output: &str) -> Result<TableIdent> {
    let parts: Vec<_> = output.split('.').collect();
    if parts.len() < 2 || parts.iter().any(|part| part.trim().is_empty()) {
        return Err(MirrorMakerError::Config(
            "Iceberg output must be a fully qualified namespace.table identifier".into(),
        ));
    }
    TableIdent::from_strs(parts).map_err(iceberg_error)
}

fn iceberg_error(error: impl std::fmt::Display) -> MirrorMakerError {
    MirrorMakerError::Config(format!("Iceberg operation failed: {error}"))
}
