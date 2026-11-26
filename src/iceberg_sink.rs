//! Native Apache Iceberg append sink.
//!
//! The first implementation writes append-only batches to existing Iceberg
//! tables. Row-level updates and deletes are not supported.

mod config;
use config::parse_config;

use crate::config::{MirrorMakerConfig, SinkConfig};
use crate::envelope::MessageEnvelope;
use crate::error::MirrorMakerError;
use crate::processor::SinkWriter;
use crate::sink_factory::SinkFactory;
use crate::Result;
use arrow_array::RecordBatch;
use arrow_array::UInt32Array;
use arrow_json::ReaderBuilder;
use arrow_schema::Schema as ArrowSchema;
use arrow_select::take::take;
use async_trait::async_trait;
use iceberg::arrow::{arrow_primitive_to_literal, schema_to_arrow_schema};
use iceberg::spec::DataFileFormat;
use iceberg::spec::{PartitionKey, Struct};
use iceberg::table::Table;
use iceberg::transaction::Transaction;
use iceberg::transform::create_transform_function;
use iceberg::writer::base_writer::data_file_writer::DataFileWriterBuilder;
use iceberg::writer::file_writer::location_generator::{
    DefaultFileNameGenerator, DefaultLocationGenerator,
};
use iceberg::writer::file_writer::rolling_writer::RollingFileWriterBuilder;
use iceberg::writer::file_writer::ParquetWriterBuilder;
use iceberg::writer::partitioning::fanout_writer::FanoutWriter;
use iceberg::writer::partitioning::unpartitioned_writer::UnpartitionedWriter;
use iceberg::writer::partitioning::PartitioningWriter;
use iceberg::{Catalog, CatalogBuilder};
use iceberg_catalog_rest::RestCatalogBuilder;
use iceberg_storage_opendal::OpenDalResolvingStorageFactory;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

pub struct IcebergSinkFactory;

#[async_trait]
impl SinkFactory for IcebergSinkFactory {
    fn sink_type(&self) -> &'static str {
        "iceberg"
    }

    async fn create(
        &self,
        _config: &MirrorMakerConfig,
        sink: &SinkConfig,
        output: &str,
        _partition: Option<&str>,
    ) -> Result<Arc<dyn SinkWriter>> {
        let (catalog_name, props, table_ident, batch_size, batch_interval) =
            parse_config(sink, output)?;
        let catalog = Arc::new(
            RestCatalogBuilder::default()
                .with_storage_factory(Arc::new(OpenDalResolvingStorageFactory::new()))
                .load(catalog_name, props)
                .await
                .map_err(iceberg_error)?,
        );
        let table = catalog
            .load_table(&table_ident)
            .await
            .map_err(iceberg_error)?;
        let arrow_schema = Arc::new(
            schema_to_arrow_schema(table.metadata().current_schema()).map_err(iceberg_error)?,
        );
        let channel_capacity = batch_size.min(8192).saturating_mul(2).max(1);
        let (tx, rx) = mpsc::channel(channel_capacity);
        tokio::spawn(run_writer(
            rx,
            catalog,
            table,
            arrow_schema,
            batch_size,
            Duration::from_millis(batch_interval),
        ));
        Ok(Arc::new(IcebergSink { tx }))
    }
}

enum Command {
    Append(Value, oneshot::Sender<Result<()>>),
    Flush(oneshot::Sender<Result<()>>),
}

struct IcebergSink {
    tx: mpsc::Sender<Command>,
}

#[async_trait]
impl SinkWriter for IcebergSink {
    async fn send(&self, envelope: MessageEnvelope) -> Result<()> {
        let (ack, reply) = oneshot::channel();
        self.tx
            .send(Command::Append((*envelope.value).clone(), ack))
            .await
            .map_err(|_| MirrorMakerError::Config("Iceberg writer stopped".into()))?;
        reply
            .await
            .map_err(|_| MirrorMakerError::Config("Iceberg writer stopped before commit".into()))?
    }

    async fn flush(&self) -> Result<()> {
        let (ack, reply) = oneshot::channel();
        self.tx
            .send(Command::Flush(ack))
            .await
            .map_err(|_| MirrorMakerError::Config("Iceberg writer stopped".into()))?;
        reply
            .await
            .map_err(|_| MirrorMakerError::Config("Iceberg writer stopped before flush".into()))?
    }
}

async fn run_writer(
    mut rx: mpsc::Receiver<Command>,
    catalog: Arc<dyn Catalog>,
    mut table: Table,
    arrow_schema: Arc<ArrowSchema>,
    batch_size: usize,
    batch_interval: Duration,
) {
    let mut rows: Vec<Value> = Vec::new();
    let mut acknowledgements = Vec::new();
    let mut deadline = None;
    loop {
        let command = if rows.is_empty() {
            rx.recv().await
        } else {
            let batch_deadline = *deadline.get_or_insert_with(|| Instant::now() + batch_interval);
            match tokio::time::timeout_at(batch_deadline, rx.recv()).await {
                Ok(command) => command,
                Err(_) => {
                    let result = match commit_rows(&catalog, &table, &arrow_schema, &rows).await {
                        Ok(updated) => {
                            table = updated;
                            Ok(())
                        }
                        Err(error) => Err(error),
                    };
                    reply_all(std::mem::take(&mut acknowledgements), result);
                    rows.clear();
                    deadline = None;
                    continue;
                }
            }
        };
        match command {
            Some(Command::Append(row, ack)) => {
                rows.push(row);
                acknowledgements.push(ack);
                deadline.get_or_insert_with(|| Instant::now() + batch_interval);
                if rows.len() >= batch_size {
                    let result = match commit_rows(&catalog, &table, &arrow_schema, &rows).await {
                        Ok(updated) => {
                            table = updated;
                            Ok(())
                        }
                        Err(error) => Err(error),
                    };
                    reply_all(std::mem::take(&mut acknowledgements), result);
                    rows.clear();
                    deadline = None;
                }
            }
            Some(Command::Flush(ack)) => {
                let result = if rows.is_empty() {
                    Ok(())
                } else {
                    let result = match commit_rows(&catalog, &table, &arrow_schema, &rows).await {
                        Ok(updated) => {
                            table = updated;
                            Ok(())
                        }
                        Err(error) => Err(error),
                    };
                    reply_all(std::mem::take(&mut acknowledgements), result.clone());
                    rows.clear();
                    deadline = None;
                    result
                };
                let _ = ack.send(result);
            }
            None => {
                if !rows.is_empty() {
                    let result = commit_rows(&catalog, &table, &arrow_schema, &rows)
                        .await
                        .map(|_| ());
                    reply_all(acknowledgements, result);
                }
                break;
            }
        }
    }
}

fn reply_all(acks: Vec<oneshot::Sender<Result<()>>>, result: Result<()>) {
    for ack in acks {
        let _ = ack.send(result.clone());
    }
}

async fn commit_rows(
    catalog: &Arc<dyn Catalog>,
    table: &Table,
    arrow_schema: &Arc<ArrowSchema>,
    rows: &[Value],
) -> Result<Table> {
    if rows.is_empty() {
        return Ok(table.clone());
    }
    let mut json = Vec::new();
    for row in rows {
        if !row.is_object() {
            return Err(MirrorMakerError::Config(
                "Iceberg sink expects each event value to be a JSON object".into(),
            ));
        }
        serde_json::to_writer(&mut json, row)
            .map_err(|error| MirrorMakerError::Serialization(error.to_string()))?;
        json.push(b'\n');
    }
    let mut reader = ReaderBuilder::new(arrow_schema.clone())
        .with_batch_size(rows.len())
        .build(Cursor::new(json))
        .map_err(|error| MirrorMakerError::Serialization(error.to_string()))?;
    let batch: RecordBatch = reader
        .next()
        .transpose()
        .map_err(|error| MirrorMakerError::Serialization(error.to_string()))?
        .ok_or_else(|| MirrorMakerError::Serialization("empty Iceberg Arrow batch".into()))?;

    let files = write_batch(table, batch).await?;
    if files.is_empty() {
        return Err(MirrorMakerError::Config(
            "Iceberg writer produced no data files".into(),
        ));
    }
    let transaction = Transaction::new(table);
    let action = transaction.fast_append().add_data_files(files);
    use iceberg::transaction::ApplyTransactionAction;
    action
        .apply(transaction)
        .map_err(iceberg_error)?
        .commit(catalog.as_ref())
        .await
        .map_err(iceberg_error)
}

async fn write_batch(table: &Table, batch: RecordBatch) -> Result<Vec<iceberg::spec::DataFile>> {
    let spec = table.metadata().default_partition_spec();
    let locations = DefaultLocationGenerator::new(table.metadata()).map_err(iceberg_error)?;
    let names = DefaultFileNameGenerator::new("streamforge".into(), None, DataFileFormat::Parquet);
    let parquet = ParquetWriterBuilder::from_table_properties(
        &table.metadata().table_properties(),
        table.metadata().current_schema().clone(),
    )
    .map_err(iceberg_error)?;
    let rolling = RollingFileWriterBuilder::new_with_default_file_size(
        parquet,
        table.file_io().clone(),
        locations,
        names,
    );
    let builder = DataFileWriterBuilder::new(rolling);
    if spec.is_unpartitioned() {
        let mut writer = UnpartitionedWriter::new(builder);
        writer.write(batch).await.map_err(iceberg_error)?;
        writer.close().await.map_err(iceberg_error)
    } else {
        let partition_groups = partition_groups(table, &batch)?;
        let mut writer = FanoutWriter::new(builder);
        for (data, row_indices) in partition_groups {
            let key = PartitionKey::new(
                spec.as_ref().clone(),
                table.metadata().current_schema().clone(),
                data,
            );
            let row_indices = UInt32Array::from(row_indices);
            let columns = batch
                .columns()
                .iter()
                .map(|column| take(column, &row_indices, None))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|error| MirrorMakerError::Serialization(error.to_string()))?;
            let partition_batch = RecordBatch::try_new(batch.schema(), columns)
                .map_err(|error| MirrorMakerError::Serialization(error.to_string()))?;
            writer
                .write(key, partition_batch)
                .await
                .map_err(iceberg_error)?;
        }
        writer.close().await.map_err(iceberg_error)
    }
}

fn partition_groups(table: &Table, batch: &RecordBatch) -> Result<HashMap<Struct, Vec<u32>>> {
    let iceberg_schema = table.metadata().current_schema();
    let spec = table.metadata().default_partition_spec();
    let partition_type = spec.partition_type(iceberg_schema).map_err(iceberg_error)?;
    let mut columns = Vec::with_capacity(spec.fields().len());
    for (index, partition_field) in spec.fields().iter().enumerate() {
        let source = iceberg_schema
            .field_by_id(partition_field.source_id)
            .ok_or_else(|| {
                MirrorMakerError::Config("Iceberg partition source field is missing".into())
            })?;
        if !iceberg_schema
            .as_struct()
            .fields()
            .iter()
            .any(|field| field.id == source.id)
        {
            return Err(MirrorMakerError::Config(format!(
                "Iceberg partition source '{}' must be a top-level table field",
                source.name
            )));
        }
        let source_index = batch.schema().index_of(&source.name).map_err(|_| {
            MirrorMakerError::Config(format!(
                "Iceberg partition source '{}' must be a top-level table field",
                source.name
            ))
        })?;
        let function =
            create_transform_function(&partition_field.transform).map_err(iceberg_error)?;
        let transformed = function
            .transform(batch.column(source_index).clone())
            .map_err(iceberg_error)?;
        let literals =
            arrow_primitive_to_literal(&transformed, &partition_type.fields()[index].field_type)
                .map_err(iceberg_error)?;
        columns.push(literals);
    }
    let mut groups = HashMap::new();
    for row in 0..batch.num_rows() {
        let values: Vec<_> = columns.iter().map(|column| column[row].clone()).collect();
        let data: Struct = values.into_iter().collect();
        groups.entry(data).or_insert_with(Vec::new).push(row as u32);
    }
    Ok(groups)
}

fn iceberg_error(error: impl std::fmt::Display) -> MirrorMakerError {
    MirrorMakerError::Config(format!("Iceberg operation failed: {error}"))
}
