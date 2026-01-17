//! # Notification System
//!
//! This module handles sending notifications (e.g., via ntfy.sh) when speed tests complete or fail.
//! It includes:
//! - Construction of notification payloads (JSON).
//! - Formatting of messages with emojis and details.
//! - Conditional sending based on `notify_on` configuration (success, failure, or both).
use crate::config::NtfyConfig;
use crate::metrics::Metrics;
use crate::runner::{ErrorCategory, RunOutcome, SpeedtestResult};
use anyhow::Result;
use std::time::Duration;

pub struct Notifier {
    config: NtfyConfig,
    metrics: Metrics,
}

impl Notifier {
    pub fn new(config: NtfyConfig, metrics: Metrics) -> Self {
        Self { config, metrics }
    }

    pub async fn notify(&self, outcome: &RunOutcome, duration: Duration) {
        let result = self.send_notification(outcome, duration).await;

        match result {
            Ok(_) => {
                tracing::info!("Notification sent successfully");
                self.metrics
                    .notify_total
                    .with_label_values(&["success"])
                    .inc();
            }
            Err(e) => {
                tracing::error!("Failed to send notification: {}", e);
                self.metrics
                    .notify_total
                    .with_label_values(&["failure"])
                    .inc();
            }
        }
    }

    async fn send_notification(&self, outcome: &RunOutcome, duration: Duration) -> Result<()> {
        let client = reqwest::Client::new();

        let (title, message) = match outcome {
            RunOutcome::Success(result) => {
                let title = format!("{} ✅", self.config.title);
                let message = format_success_message(result, duration);
                (title, message)
            }
            RunOutcome::Failure(error) => {
                let title = format!("{} ❌", self.config.title);
                let message = format_failure_message(error);
                (title, message)
            }
        };

        let mut request = client.post(&self.config.url);

        // Add authentication if configured
        if let Some(token) = &self.config.token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        // Add ntfy headers
        request = request
            .header("Title", title)
            .header("Tags", &self.config.tags)
            .header("Priority", self.config.priority.to_string());

        if let Some(click_url) = &self.config.click_url {
            request = request.header("Click", click_url);
        }

        // Send the message as body
        request = request.body(message);

        let response = request.send().await?;

        if !response.status().is_success() {
            anyhow::bail!("ntfy returned status: {}", response.status());
        }

        Ok(())
    }
}

pub fn format_success_message(result: &SpeedtestResult, duration: Duration) -> String {
    let download_mbps = result.download_bps / 1_000_000.0;
    let upload_mbps = result.upload_bps / 1_000_000.0;
    let latency_ms = result.latency_seconds * 1000.0;

    let mut message = format!(
        "⬇️ Download: {:.1} Mbps\n⬆️ Upload: {:.1} Mbps\n📡 Ping: {:.1} ms\n⏱️ Duration: {}s",
        download_mbps,
        upload_mbps,
        latency_ms,
        duration.as_secs()
    );

    if let Some(jitter) = result.jitter_seconds {
        let jitter_ms = jitter * 1000.0;
        message.push_str(&format!("\n📊 Jitter: {:.1} ms", jitter_ms));
    }

    if let Some(loss) = result.packet_loss_ratio {
        message.push_str(&format!("\n📉 Loss: {:.1}%", loss * 100.0));
    }

    message
}

pub fn format_failure_message(error: &ErrorCategory) -> String {
    match error {
        ErrorCategory::Timeout(seconds) => format!("timeout after {}s", seconds),
        ErrorCategory::CommandNotFound(cmd) => format!("command not found: {}", cmd),
        ErrorCategory::CommandFailed(code) => format!("exit={}", code),
        ErrorCategory::InvalidOutput(msg) => format!("invalid output: {}", msg),
        ErrorCategory::MissingFields(fields) => format!("missing fields: {}", fields),
        ErrorCategory::Internal(msg) => format!("internal error: {}", msg),
    }
}
