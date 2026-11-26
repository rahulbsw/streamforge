//! Apache Fluss destination sink.

use crate::config::{MirrorMakerConfig, SinkConfig};
use crate::envelope::MessageEnvelope;
use crate::processor::SinkWriter;
use crate::sink_factory::SinkFactory;
use crate::{MirrorMakerError, Result};
use async_trait::async_trait;
use fluss::client::{AppendWriter, FlussConnection, UpsertWriter};
use fluss::config::Config as FlussClientConfig;
use fluss::metadata::{Column, DataType, TablePath};
use fluss::row::GenericRow;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::sync::Arc;

const DEFAULT_DATABASE: &str = "fluss";

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum KeySource {
    Row,
    KafkaKey,
}

impl Default for KeySource {
    fn default() -> Self {
        Self::Row
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FlussOptions {
    #[serde(default)]
    database: Option<String>,
    #[serde(default)]
    key_source: KeySource,
    #[serde(default)]
    security_sasl_password_file: Option<String>,
}

/// Registered by the binary before any processors are built.
pub struct FlussSinkFactory;

#[async_trait]
impl SinkFactory for FlussSinkFactory {
    fn sink_type(&self) -> &'static str {
        "fluss"
    }

    async fn create(
        &self,
        _config: &MirrorMakerConfig,
        sink: &SinkConfig,
        output: &str,
        _partition: Option<&str>,
    ) -> Result<Arc<dyn SinkWriter>> {
        let (client_config, options) = parse_config(sink)?;
        let (database, table) = resolve_table_path(output, options.database.as_deref())?;
        let connection = FlussConnection::new(client_config)
            .await
            .map_err(|error| sink_error("connect to Fluss", error))?;
        let path = TablePath::new(database, table);
        let fluss_table = connection
            .get_table(&path)
            .await
            .map_err(|error| sink_error("load configured Fluss table", error))?;
        let info = fluss_table.get_table_info().clone();
        let writer = if fluss_table.has_primary_key() {
            Writer::Upsert(
                fluss_table
                    .new_upsert()
                    .and_then(|upsert| upsert.create_writer())
                    .map_err(|error| sink_error("create Fluss upsert writer", error))?,
            )
        } else {
            Writer::Append(
                fluss_table
                    .new_append()
                    .and_then(|append| append.create_writer())
                    .map_err(|error| sink_error("create Fluss append writer", error))?,
            )
        };
        drop(fluss_table);

        Ok(Arc::new(FlussSink {
            _connection: connection,
            writer,
            info,
            key_source: options.key_source,
        }))
    }
}

enum Writer {
    Append(AppendWriter),
    Upsert(UpsertWriter),
}

struct FlussSink {
    _connection: FlussConnection,
    writer: Writer,
    info: fluss::metadata::TableInfo,
    key_source: KeySource,
}

#[async_trait]
impl SinkWriter for FlussSink {
    async fn send(&self, envelope: MessageEnvelope) -> Result<()> {
        let schema = self.info.get_schema();
        let row = map_json_row(
            schema.columns(),
            &schema.primary_key_column_names(),
            &envelope,
            self.key_source,
        )?;
        let result = match &self.writer {
            Writer::Append(writer) => writer
                .append(&row)
                .map_err(|error| sink_error("queue Fluss append", error))?,
            Writer::Upsert(writer) => writer
                .upsert(&row)
                .map_err(|error| sink_error("queue Fluss upsert", error))?,
        };
        result
            .await
            .map_err(|error| sink_error("await Fluss write acknowledgement", error))
    }

    async fn flush(&self) -> Result<()> {
        match &self.writer {
            Writer::Append(writer) => writer
                .flush()
                .await
                .map_err(|error| sink_error("flush Fluss append writer", error))?,
            Writer::Upsert(writer) => writer
                .flush()
                .await
                .map_err(|error| sink_error("flush Fluss upsert writer", error))?,
        }
        Ok(())
    }
}

fn parse_config(sink: &SinkConfig) -> Result<(FlussClientConfig, FlussOptions)> {
    let value = sink
        .properties
        .get("fluss")
        .ok_or_else(|| MirrorMakerError::Config("sink.type=fluss requires sink.fluss".into()))?;
    let object = value.as_object().ok_or_else(|| {
        MirrorMakerError::Config("sink.fluss must be a mapping of Fluss client options".into())
    })?;

    if object.contains_key("security_sasl_password") {
        return Err(MirrorMakerError::Config(
            "sink.fluss.security_sasl_password is not accepted; use security_sasl_password_file pointing to a mounted secret file".into(),
        ));
    }

    let options_value = serde_json::to_value(FlussOptions {
        database: None,
        key_source: KeySource::default(),
        security_sasl_password_file: None,
    })?;
    let mut options_object = options_value.as_object().cloned().unwrap_or_default();
    let native_config = FlussClientConfig::default();
    let native_value = serde_json::to_value(&native_config)?;
    let mut native_object = native_value.as_object().cloned().unwrap_or_default();
    // The upstream client intentionally skips the secret when serializing Config.
    native_object.insert(
        "security_sasl_password".into(),
        Value::String(String::new()),
    );

    for (name, setting) in object {
        if options_object.contains_key(name) {
            options_object.insert(name.clone(), setting.clone());
        } else if native_object.contains_key(name) {
            native_object.insert(name.clone(), setting.clone());
        } else {
            return Err(MirrorMakerError::Config(format!(
                "unsupported sink.fluss option '{name}'"
            )));
        }
    }

    let options: FlussOptions =
        serde_json::from_value(Value::Object(options_object)).map_err(|error| {
            MirrorMakerError::Config(format!("invalid sink.fluss options: {error}"))
        })?;
    if let Some(path) = options.security_sasl_password_file.as_deref() {
        let mut password = fs::read_to_string(path).map_err(|error| {
            MirrorMakerError::Config(format!(
                "could not read sink.fluss.security_sasl_password_file: {error}"
            ))
        })?;
        while password.ends_with(['\n', '\r']) {
            password.pop();
        }
        native_object.insert("security_sasl_password".into(), Value::String(password));
    }
    let client_config: FlussClientConfig = serde_json::from_value(Value::Object(native_object))
        .map_err(|error| {
            MirrorMakerError::Config(format!("invalid sink.fluss client config: {error}"))
        })?;

    client_config.validate_security().map_err(|message| {
        MirrorMakerError::Config(format!("invalid sink.fluss security: {message}"))
    })?;
    client_config.validate_writer().map_err(|message| {
        MirrorMakerError::Config(format!("invalid sink.fluss writer settings: {message}"))
    })?;

    Ok((client_config, options))
}

fn resolve_table_path(output: &str, database: Option<&str>) -> Result<(String, String)> {
    let mut parts = output.split('.');
    let first = parts.next().unwrap_or_default();
    let second = parts.next();
    if parts.next().is_some() || first.is_empty() {
        return Err(MirrorMakerError::Config(
            "Fluss destination output must be 'table' or 'database.table'".into(),
        ));
    }
    match (database, second) {
        (Some(_db), Some(_)) => Err(MirrorMakerError::Config(
            "set the Fluss database either in sink.fluss.database or as output database.table, not both".into(),
        )),
        (Some(db), None) => Ok((db.to_string(), first.to_string())),
        (None, Some(table)) if !table.is_empty() => Ok((first.to_string(), table.to_string())),
        (None, None) => Ok((DEFAULT_DATABASE.to_string(), first.to_string())),
        _ => Err(MirrorMakerError::Config("invalid Fluss table path".into())),
    }
}

fn map_json_row(
    columns: &[Column],
    primary_keys: &[&str],
    envelope: &MessageEnvelope,
    key_source: KeySource,
) -> Result<GenericRow<'static>> {
    let object = envelope.value.as_object().ok_or_else(|| {
        MirrorMakerError::Serialization("Fluss row value must be a JSON object".into())
    })?;
    let allowed = columns
        .iter()
        .map(Column::name)
        .collect::<std::collections::HashSet<_>>();
    if let Some(unknown) = object.keys().find(|name| !allowed.contains(name.as_str())) {
        return Err(MirrorMakerError::Serialization(format!(
            "Fluss row contains unknown column '{unknown}'"
        )));
    }

    let key_values = match key_source {
        KeySource::Row => None,
        KeySource::KafkaKey => Some(parse_key_values(envelope.key.as_ref(), primary_keys)?),
    };
    let mut row = GenericRow::new(columns.len());
    for (index, column) in columns.iter().enumerate() {
        let source_value = key_values
            .as_ref()
            .and_then(|values| values.get(column.name()))
            .or_else(|| object.get(column.name()));
        let datum = match source_value {
            Some(Value::Null) | None if column.data_type().is_nullable() => fluss::row::Datum::Null,
            Some(value) => map_scalar(value, column)?,
            None => {
                return Err(MirrorMakerError::Serialization(format!(
                    "required Fluss column '{}' is missing",
                    column.name()
                )));
            }
        };
        row.set_field(index, datum);
    }
    Ok(row.into_owned())
}

fn parse_key_values(key: Option<&Value>, primary_keys: &[&str]) -> Result<Map<String, Value>> {
    let key = key.ok_or_else(|| {
        MirrorMakerError::Serialization(
            "sink.fluss.key_source=kafka_key requires a Kafka message key".into(),
        )
    })?;
    match key {
        Value::Object(values) => {
            if let Some(unknown) = values
                .keys()
                .find(|name| !primary_keys.contains(&name.as_str()))
            {
                return Err(MirrorMakerError::Serialization(format!(
                    "Kafka key contains non-primary-key Fluss column '{unknown}'"
                )));
            }
            Ok(values.clone())
        }
        Value::Null => Err(MirrorMakerError::Serialization(
            "Fluss primary key cannot be null".into(),
        )),
        scalar => {
            if primary_keys.len() != 1 {
                return Err(MirrorMakerError::Serialization(
                    "a scalar Kafka key can only map to a Fluss table with exactly one primary-key column; use a JSON object key for composite keys".into(),
                ));
            }
            Ok(Map::from_iter([(
                primary_keys[0].to_string(),
                scalar.clone(),
            )]))
        }
    }
}

fn map_scalar(value: &Value, column: &Column) -> Result<fluss::row::Datum<'static>> {
    use fluss::row::Datum;
    let invalid = || {
        MirrorMakerError::Serialization(format!(
            "JSON value does not match Fluss column '{}' type {}",
            column.name(),
            column.data_type()
        ))
    };
    Ok(match column.data_type() {
        DataType::Boolean(_) => Datum::Bool(value.as_bool().ok_or_else(invalid)?),
        DataType::TinyInt(_) => Datum::Int8(
            i8::try_from(value.as_i64().ok_or_else(invalid)?).map_err(|_| invalid())?,
        ),
        DataType::SmallInt(_) => Datum::Int16(
            i16::try_from(value.as_i64().ok_or_else(invalid)?).map_err(|_| invalid())?,
        ),
        DataType::Int(_) => Datum::Int32(
            i32::try_from(value.as_i64().ok_or_else(invalid)?).map_err(|_| invalid())?,
        ),
        DataType::BigInt(_) => Datum::Int64(value.as_i64().ok_or_else(invalid)?),
        DataType::Float(_) => Datum::Float32(
            (value
                .as_f64()
                .filter(|number| number.is_finite() && (*number as f32).is_finite())
                .ok_or_else(invalid)?
                as f32)
                .into(),
        ),
        DataType::Double(_) => Datum::Float64(
            value
                .as_f64()
                .filter(|number| number.is_finite())
                .ok_or_else(invalid)?
                .into(),
        ),
        DataType::Char(_) | DataType::String(_) => {
            Datum::String(value.as_str().ok_or_else(invalid)?.to_string().into())
        }
        DataType::Bytes(_) | DataType::Binary(_) => {
            let bytes = value
                .as_array()
                .ok_or_else(invalid)?
                .iter()
                .map(|item| {
                    u8::try_from(item.as_u64().ok_or_else(invalid)?)
                        .map_err(|_| invalid())
                })
                .collect::<Result<Vec<_>>>()?;
            Datum::Blob(bytes.into())
        }
        _ => {
            return Err(MirrorMakerError::Serialization(format!(
                "Fluss column '{}' has unsupported JSON mapping type {}; supported types are boolean, integer, floating point, string, bytes, and binary",
                column.name(),
                column.data_type()
            )))
        }
    })
}

fn sink_error(action: &str, error: impl std::fmt::Display) -> MirrorMakerError {
    MirrorMakerError::Generic(format!("Fluss sink failed to {action}: {error}"))
}
