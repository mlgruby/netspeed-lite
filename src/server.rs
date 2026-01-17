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
    Router,
};

#[derive(Clone)]
struct AppState {
    metrics: Metrics,
}

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

async fn health_handler(State(_state): State<AppState>) -> Response {
    // Simple health check - just return 200 OK
    // Could be extended to check last run timestamp, etc.
    (StatusCode::OK, "OK").into_response()
}
