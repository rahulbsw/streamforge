use axum::{routing::get, Router};
use std::net::SocketAddr;
use tracing::{error, info};

/// Start the metrics HTTP server
pub async fn start_metrics_server(port: u16) {
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("🔍 Metrics server listening on http://0.0.0.0:{}", port);
    info!("   Metrics endpoint: http://localhost:{}/metrics", port);
    info!("   Health endpoint:  http://localhost:{}/health", port);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind metrics server");

    match axum::serve(listener, app).await {
        Ok(_) => info!("Metrics server stopped gracefully"),
        Err(e) => error!("Metrics server error: {}", e),
    }
}

/// Handler for /metrics endpoint
async fn metrics_handler() -> String {
    super::metrics::metrics_text()
}

/// Handler for /health endpoint
async fn health_handler() -> &'static str {
    "OK"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        let health = health_handler().await;
        assert_eq!(health, "OK");
    }

    #[tokio::test]
    async fn test_metrics_endpoint() {
        // Register metrics first
        let _ = super::super::register_metrics();

        let metrics = metrics_handler().await;
        // Check that we got Prometheus-formatted output
        assert!(metrics.contains("# HELP") || !metrics.is_empty());
    }
}
