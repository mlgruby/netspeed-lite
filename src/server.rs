//! # HTTP Server
//!
//! This module defines the Axum HTTP server that exposes the `/metrics` endpoint.
//! It serves the Prometheus metrics registry to be scraped by a Prometheus instance.
use crate::metrics::Metrics;
use crate::notifier::Notifier;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone)]
struct AppState {
    metrics: Metrics,
    notifier: Option<Notifier>,
}

/// Starts the HTTP server for exposing metrics and health endpoints.
///
/// This function creates an Axum router with the following routes:
/// - `GET /`: HTML landing page with links to endpoints
/// - `GET /metrics`: Prometheus metrics in text format
/// - `GET /healthz`: JSON health check status
/// - `POST /alertmanager`: Webhook endpoint for Alertmanager notifications
///
/// The server runs indefinitely until an error occurs or it's shut down.
///
/// # Arguments
///
/// * `bind_address` - Address to bind the server to (e.g., "0.0.0.0:9109")
/// * `metrics` - Metrics instance to expose via the `/metrics` endpoint
/// * `notifier` - Optional notifier for sending Alertmanager webhooks to ntfy
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
/// server::serve("127.0.0.1:9109".to_string(), metrics, None).await.unwrap();
/// # };
/// ```
pub async fn serve(
    bind_address: String,
    metrics: Metrics,
    notifier: Option<Notifier>,
) -> anyhow::Result<()> {
    let state = AppState { metrics, notifier };

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/metrics", get(metrics_handler))
        .route("/healthz", get(health_handler))
        .route("/alertmanager", post(alertmanager_handler))
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
            <div class="endpoint">
                <strong>Alertmanager Webhook:</strong> POST /alertmanager
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

// Alertmanager webhook structs
#[derive(Debug, Deserialize)]
struct AlertmanagerWebhook {
    status: String,
    #[serde(rename = "commonLabels")]
    common_labels: HashMap<String, String>,
    #[serde(rename = "commonAnnotations")]
    common_annotations: HashMap<String, String>,
    alerts: Vec<Alert>,
}

#[derive(Debug, Deserialize)]
struct Alert {
    #[serde(rename = "startsAt")]
    starts_at: String,
    #[serde(rename = "endsAt")]
    ends_at: String,
}

async fn alertmanager_handler(
    State(state): State<AppState>,
    Json(payload): Json<AlertmanagerWebhook>,
) -> Response {
    // Check if notifier is configured
    let Some(notifier) = &state.notifier else {
        tracing::warn!("Alertmanager webhook received but notifier not configured");
        return (StatusCode::SERVICE_UNAVAILABLE, "Notifier not configured").into_response();
    };

    // Format the alert message
    let (title, message) = format_alertmanager_message(&payload);

    // Send notification
    let result = send_alertmanager_notification(notifier, &title, &message, &payload).await;

    match result {
        Ok(_) => {
            tracing::info!("Alertmanager notification sent: {}", title);
            (StatusCode::OK, "OK").into_response()
        }
        Err(e) => {
            tracing::error!("Failed to send Alertmanager notification: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to send notification",
            )
                .into_response()
        }
    }
}

fn format_alertmanager_message(webhook: &AlertmanagerWebhook) -> (String, String) {
    // Determine emoji based on status
    let emoji = if webhook.status == "firing" {
        "🔴"
    } else {
        "✅"
    };

    // Get alert details
    let alert_name = webhook
        .common_labels
        .get("alertname")
        .cloned()
        .unwrap_or_else(|| "Unknown Alert".to_string());

    let instance = webhook.common_labels.get("instance").cloned();

    // Build title
    let title = format!("{} {}", emoji, alert_name);

    // Build message body
    let mut lines = Vec::new();

    // Add summary if available
    if let Some(summary) = webhook.common_annotations.get("summary") {
        lines.push(format!("📋 {}", summary));
    }

    // Add description if available
    if let Some(description) = webhook.common_annotations.get("description") {
        if !lines.is_empty() {
            lines.push(String::new()); // Empty line
        }
        lines.push(description.clone());
    }

    // Add instance info
    if let Some(inst) = instance {
        lines.push(String::new());
        lines.push(format!("🖥️ Instance: {}", inst));
    }

    // Add timing information from first alert
    if let Some(alert) = webhook.alerts.first() {
        lines.push(String::new());

        // Parse and format start time
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&alert.starts_at) {
            lines.push(format!("⏰ Started: {}", dt.format("%Y-%m-%d %H:%M:%S %Z")));
        }

        // Add end time if resolved
        if webhook.status == "resolved" {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&alert.ends_at) {
                lines.push(format!("⏱️ Ended: {}", dt.format("%Y-%m-%d %H:%M:%S %Z")));
            }
        }
    }

    // Add alert count if multiple
    if webhook.alerts.len() > 1 {
        lines.push(String::new());
        lines.push(format!("🔢 {} alerts in this group", webhook.alerts.len()));
    }

    let message = lines.join("\n");

    (title, message)
}

async fn send_alertmanager_notification(
    notifier: &Notifier,
    title: &str,
    message: &str,
    webhook: &AlertmanagerWebhook,
) -> anyhow::Result<()> {
    // Determine priority based on severity
    let severity = webhook
        .common_labels
        .get("severity")
        .map(|s| s.as_str())
        .unwrap_or("info");

    let priority = match severity {
        "critical" => 5,
        "warning" => 4,
        _ => 3,
    };

    // Build tags
    let tags = format!("prometheus,alert,{}", severity);

    // Send via notifier's HTTP client
    notifier
        .send_custom_notification(title, message, priority, &tags)
        .await
}
