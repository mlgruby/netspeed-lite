//! # HTTP Server
//!
//! This module defines the Axum HTTP server that exposes the `/metrics` endpoint.
//! It serves the Prometheus metrics registry to be scraped by a Prometheus instance.
use crate::metrics::Metrics;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;

#[derive(Clone)]
struct AppState {
    metrics: Metrics,
}

/// Starts the HTTP server for exposing metrics and health endpoints.
///
/// This function creates an Axum router with the following routes:
/// - `GET /`: HTML landing page with links to endpoints
/// - `GET /metrics`: Prometheus metrics in text format
/// - `GET /healthz`: JSON health check status
///
/// The server runs indefinitely until an error occurs or it's shut down.
///
/// # Arguments
///
/// * `bind_address` - Address to bind the server to (e.g., "0.0.0.0:9109")
/// * `metrics` - Metrics instance to expose via the `/metrics` endpoint
///
/// # Returns
///
/// Returns `Ok(())` if the server shuts down gracefully, or `Err` if:
/// - The bind address is invalid or already in use
/// - A critical server error occurs
///
/// # Examples
///
/// ```no_run
/// use netspeed_lite::metrics::Metrics;
/// use netspeed_lite::server;
///
/// # async {
/// let metrics = Metrics::new().unwrap();
/// server::serve("127.0.0.1:9109".to_string(), metrics).await.unwrap();
/// # };
/// ```
pub async fn serve(bind_address: String, metrics: Metrics) -> anyhow::Result<()> {
    let state = AppState { metrics };

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/metrics", get(metrics_handler))
        .route("/healthz", get(health_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    tracing::info!("HTTP server listening on {}", bind_address);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn root_handler() -> Html<&'static str> {
    Html(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>netspeed-lite</title>
            <style>
                body { font-family: sans-serif; max-width: 600px; margin: 50px auto; padding: 20px; }
                h1 { color: #333; }
                a { color: #0066cc; text-decoration: none; }
                a:hover { text-decoration: underline; }
                .endpoint { margin: 10px 0; padding: 10px; background: #f5f5f5; border-radius: 4px; }
            </style>
        </head>
        <body>
            <h1>netspeed-lite</h1>
            <p>ISP speed monitor with Prometheus metrics and ntfy notifications</p>
            <div class="endpoint">
                <strong>Metrics:</strong> <a href="/metrics">/metrics</a>
            </div>
            <div class="endpoint">
                <strong>Health:</strong> <a href="/healthz">/healthz</a>
            </div>
        </body>
        </html>
        "#,
    )
}

async fn metrics_handler(State(state): State<AppState>) -> Response {
    match state.metrics.render() {
        Ok(metrics) => (
            StatusCode::OK,
            [("Content-Type", "text/plain; version=0.0.4")],
            metrics,
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to render metrics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to render metrics",
            )
                .into_response()
        }
    }
}

#[derive(Serialize)]
struct HealthStatus {
    status: String,
    last_run_timestamp: f64,
    last_success_timestamp: f64,
}

async fn health_handler(State(state): State<AppState>) -> Response {
    let last_run = state.metrics.run_timestamp_seconds.get();
    let last_success = state.metrics.last_success.get();

    // Determine status based on whether we've had a successful run
    let status = if last_success > 0.0 {
        "healthy"
    } else if last_run > 0.0 {
        "unhealthy"
    } else {
        "initializing"
    };

    let health = HealthStatus {
        status: status.to_string(),
        last_run_timestamp: last_run,
        last_success_timestamp: if last_success > 0.0 { last_run } else { 0.0 },
    };

    // Return 503 if never successfully run or last run failed
    let status_code = if status == "healthy" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status_code, Json(health)).into_response()
}
