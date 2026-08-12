use std::fmt;
use std::str::FromStr;
use tracing_subscriber::EnvFilter;

pub const LOG_FORMAT_ENV: &str = "STREAMFORGE_LOG_FORMAT";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}

impl FromStr for LogFormat {
    type Err = InvalidLogFormat;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            _ => Err(InvalidLogFormat(value.to_string())),
        }
    }
}

#[derive(Debug)]
pub struct InvalidLogFormat(String);

impl fmt::Display for InvalidLogFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{LOG_FORMAT_ENV} must be either 'text' or 'json', got {:?}",
            self.0
        )
    }
}

impl std::error::Error for InvalidLogFormat {}

pub fn init_tracing_from_env() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let format = std::env::var(LOG_FORMAT_ENV)
        .unwrap_or_else(|_| "text".to_string())
        .parse::<LogFormat>()?;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    match format {
        LogFormat::Text => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .compact()
            .try_init()?,
        LogFormat::Json => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .flatten_event(true)
            .with_current_span(false)
            .with_span_list(false)
            .try_init()?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::fmt::MakeWriter;

    #[derive(Clone, Default)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    struct SharedWriterGuard(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriterGuard {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for SharedWriter {
        type Writer = SharedWriterGuard;

        fn make_writer(&'a self) -> Self::Writer {
            SharedWriterGuard(self.0.clone())
        }
    }

    #[test]
    fn log_format_defaults_to_text() {
        assert_eq!(LogFormat::default(), LogFormat::Text);
    }

    #[test]
    fn log_format_accepts_only_documented_values() {
        assert_eq!("text".parse::<LogFormat>().unwrap(), LogFormat::Text);
        assert_eq!("JSON".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert!("pretty".parse::<LogFormat>().is_err());
    }

    #[test]
    fn json_events_are_line_delimited_and_use_safe_operational_fields() {
        let writer = SharedWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .flatten_event(true)
            .with_current_span(false)
            .with_span_list(false)
            .with_writer(writer.clone())
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(
                pipeline = "orders",
                topic = "orders.input",
                partition = 3,
                offset = 42,
                destination = "orders.output",
                retry_attempt = 1,
                delivery_mode = "acknowledged",
                error_category = "processing",
                "record processing failed"
            );
        });

        let bytes = writer
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let output = String::from_utf8(bytes).unwrap();
        let lines: Vec<_> = output.lines().collect();
        assert_eq!(lines.len(), 1);

        let event: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(event["pipeline"], "orders");
        assert_eq!(event["topic"], "orders.input");
        assert_eq!(event["partition"], 3);
        assert_eq!(event["offset"], 42);
        assert_eq!(event["destination"], "orders.output");
        assert_eq!(event["retry_attempt"], 1);
        assert_eq!(event["delivery_mode"], "acknowledged");
        assert_eq!(event["error_category"], "processing");
        assert!(event.get("payload").is_none());
        assert!(event.get("headers").is_none());
        assert!(event.get("password").is_none());
        assert!(event.get("secret").is_none());
    }
}
