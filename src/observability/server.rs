use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};
use tracing::{error, info};

use super::metrics::METRICS;

const RUNTIME_INITIALIZING: &str = "runtime_initializing";
const KAFKA_INITIALIZING: &str = "kafka_initializing";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KafkaReadinessFailure {
    ConsumerError,
    StreamEnded,
}

impl KafkaReadinessFailure {
    fn code(self) -> &'static str {
        match self {
            Self::ConsumerError => "kafka_consumer_error",
            Self::StreamEnded => "kafka_stream_ended",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::ConsumerError => "Kafka consumer is not currently usable",
            Self::StreamEnded => "Kafka consumer stream ended",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReadinessState {
    inner: Arc<RwLock<ReadinessChecks>>,
}

#[derive(Clone, Debug)]
struct ReadinessChecks {
    runtime_ready: bool,
    kafka_ready: bool,
    kafka_failure: Option<KafkaReadinessFailure>,
}

impl Default for ReadinessState {
    fn default() -> Self {
        Self::new()
    }
}

impl ReadinessState {
    pub fn new() -> Self {
        METRICS.ready.set(0.0);
        Self {
            inner: Arc::new(RwLock::new(ReadinessChecks {
                runtime_ready: false,
                kafka_ready: false,
                kafka_failure: None,
            })),
        }
    }

    pub fn mark_runtime_ready(&self) {
        self.update(|checks| checks.runtime_ready = true);
    }

    pub fn mark_kafka_ready(&self) {
        self.update(|checks| {
            checks.kafka_ready = true;
            checks.kafka_failure = None;
        });
        METRICS
            .kafka_connections
            .with_label_values(&[super::metrics::labels::CONNECTION_TYPE_CONSUMER])
            .set(1.0);
    }

    pub fn mark_kafka_unready(&self, failure: KafkaReadinessFailure) {
        self.update(|checks| {
            checks.kafka_ready = false;
            checks.kafka_failure = Some(failure);
        });
        METRICS
            .kafka_connections
            .with_label_values(&[super::metrics::labels::CONNECTION_TYPE_CONSUMER])
            .set(0.0);
    }

    pub fn is_ready(&self) -> bool {
        let checks = self.snapshot();
        checks.runtime_ready && checks.kafka_ready
    }

    fn update(&self, update: impl FnOnce(&mut ReadinessChecks)) {
        {
            let mut checks = self
                .inner
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            update(&mut checks);
            METRICS
                .ready
                .set(f64::from(checks.runtime_ready && checks.kafka_ready));
        }
    }

    fn snapshot(&self) -> ReadinessChecks {
        self.inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn response(&self) -> ReadinessResponse {
        let checks = self.snapshot();
        let mut reasons = Vec::with_capacity(2);

        if !checks.runtime_ready {
            reasons.push(ReadinessReason {
                component: "runtime",
                code: RUNTIME_INITIALIZING,
                message: "StreamForge runtime is still initializing",
            });
        }

        if !checks.kafka_ready {
            let (code, message) = checks
                .kafka_failure
                .map(|failure| (failure.code(), failure.message()))
                .unwrap_or((KAFKA_INITIALIZING, "Kafka consumer is still initializing"));
            reasons.push(ReadinessReason {
                component: "kafka",
                code,
                message,
            });
        }

        ReadinessResponse {
            status: if reasons.is_empty() {
                "ready"
            } else {
                "not_ready"
            },
            checks: vec![
                ReadinessCheck {
                    component: "runtime",
                    ready: checks.runtime_ready,
                },
                ReadinessCheck {
                    component: "kafka",
                    ready: checks.kafka_ready,
                },
            ],
            reasons,
        }
    }
}

#[derive(Debug, Serialize)]
struct ReadinessResponse {
    status: &'static str,
    checks: Vec<ReadinessCheck>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reasons: Vec<ReadinessReason>,
}

#[derive(Debug, Serialize)]
struct ReadinessCheck {
    component: &'static str,
    ready: bool,
}

#[derive(Debug, Serialize)]
struct ReadinessReason {
    component: &'static str,
    code: &'static str,
    message: &'static str,
}

/// Start the metrics HTTP server with a readiness state owned by the server.
///
/// This compatibility entry point keeps `/health` and `/metrics` behavior for
/// embedders. The state remains not ready until an embedder uses
/// [`start_observability_server_on`] with its runtime state.
pub async fn start_metrics_server(port: u16) {
    start_metrics_server_on(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port).await;
}

/// Start the metrics HTTP server on an explicit bind address.
pub async fn start_metrics_server_on(bind_address: IpAddr, port: u16) {
    start_observability_server_on(
        bind_address,
        port,
        "/metrics".to_string(),
        ReadinessState::new(),
    )
    .await;
}

/// Start the metrics, liveness, and readiness server.
pub async fn start_observability_server_on(
    bind_address: IpAddr,
    port: u16,
    metrics_path: String,
    readiness: ReadinessState,
) {
    let app = Router::new()
        .route(&metrics_path, get(metrics_handler))
        .route("/health", get(health_handler))
        .route("/ready", get(readiness_handler))
        .with_state(readiness);

    let addr = SocketAddr::new(bind_address, port);
    info!(
        bind_address = %bind_address,
        port,
        metrics_path,
        health_path = "/health",
        readiness_path = "/ready",
        "observability server listening"
    );

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind observability server");

    match axum::serve(listener, app).await {
        Ok(_) => info!("observability server stopped gracefully"),
        Err(error) => error!(
            error_category = "observability_server",
            error = %error,
            "observability server failed"
        ),
    }
}

async fn metrics_handler() -> String {
    super::metrics::metrics_text()
}

async fn health_handler() -> &'static str {
    "OK"
}

async fn readiness_handler(State(readiness): State<ReadinessState>) -> Response {
    let response = readiness.response();
    let status = if response.status == "ready" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(response)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn health_endpoint_remains_backward_compatible() {
        assert_eq!(health_handler().await, "OK");
    }

    #[tokio::test]
    async fn metrics_endpoint_contains_prometheus_output() {
        super::super::register_metrics().unwrap();
        let metrics = metrics_handler().await;
        assert!(metrics.contains("# HELP streamforge_build_info"));
        assert!(metrics.contains("# HELP streamforge_ready"));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn readiness_is_503_with_bounded_initialization_reasons() {
        let response = readiness_handler(State(ReadinessState::new())).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["status"], "not_ready");
        assert_eq!(value["reasons"][0]["code"], RUNTIME_INITIALIZING);
        assert_eq!(value["reasons"][1]["code"], KAFKA_INITIALIZING);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn readiness_transitions_to_200_when_all_dependencies_are_ready() {
        let readiness = ReadinessState::new();
        readiness.mark_runtime_ready();
        readiness.mark_kafka_ready();

        let response = readiness_handler(State(readiness)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["status"], "ready");
        assert!(value.get("reasons").is_none());
        assert_eq!(METRICS.ready.get(), 1.0);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn kafka_failure_is_safe_and_recoverable() {
        let readiness = ReadinessState::new();
        readiness.mark_runtime_ready();
        readiness.mark_kafka_ready();
        readiness.mark_kafka_unready(KafkaReadinessFailure::ConsumerError);

        let response = readiness_handler(State(readiness.clone())).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["reasons"][0]["code"], "kafka_consumer_error");
        assert!(!body.windows(4).any(|window| window == b"pass"));

        readiness.mark_kafka_ready();
        assert!(readiness.is_ready());
    }
}
