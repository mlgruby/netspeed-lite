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
    client: reqwest::Client,
}

impl Notifier {
    /// Creates a new Notifier instance with an HTTP client configured for ntfy.sh.
    ///
    /// The HTTP client is created with:
    /// - 30-second timeout for requests
    /// - Connection pooling with max 1 idle connection per host
    ///
    /// # Arguments
    ///
    /// * `config` - ntfy.sh configuration including URL, token, and notification preferences
    /// * `metrics` - Metrics instance for tracking notification success/failure
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created (rare, indicates system issues).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use netspeed_lite::config::NtfyConfig;
    /// use netspeed_lite::metrics::Metrics;
    /// use netspeed_lite::notifier::Notifier;
    ///
    /// let config = NtfyConfig {
    ///     url: "https://ntfy.sh/mytopic".to_string(),
    ///     token: None,
    ///     title: "netspeed-lite".to_string(),
    ///     tags: "speedtest,isp".to_string(),
    ///     priority: 3,
    ///     click_url: None,
    /// };
    /// let metrics = Metrics::new().unwrap();
    /// let notifier = Notifier::new(config, metrics);
    /// ```
    pub fn new(config: NtfyConfig, metrics: Metrics) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .pool_max_idle_per_host(1)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            metrics,
            client,
        }
    }

    /// Sends a notification about a speedtest run outcome.
    ///
    /// This function formats the notification message based on the outcome (success or failure),
    /// sends it to the configured ntfy.sh endpoint, and updates metrics.
    ///
    /// # Arguments
    ///
    /// * `outcome` - The result of the speedtest run (Success or Failure)
    /// * `duration` - How long the speedtest took to complete
    ///
    /// # Behavior
    ///
    /// On success:
    /// - Logs an info message
    /// - Increments `notify_total{outcome="success"}` metric
    ///
    /// On failure:
    /// - Logs an error message
    /// - Increments `notify_total{outcome="failure"}` metric
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use netspeed_lite::notifier::Notifier;
    /// use netspeed_lite::runner::{RunOutcome, SpeedtestResult};
    /// use std::time::Duration;
    ///
    /// # async {
    /// # let notifier: Notifier = unimplemented!();
    /// let result = SpeedtestResult {
    ///     download_bps: 100_000_000.0,
    ///     upload_bps: 10_000_000.0,
    ///     latency_seconds: 0.020,
    ///     jitter_seconds: Some(0.002),
    ///     packet_loss_ratio: None,
    /// };
    /// notifier.notify(&RunOutcome::Success(result), Duration::from_secs(30)).await;
    /// # };
    /// ```
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

        let mut request = self.client.post(&self.config.url);

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

/// Formats a success notification message with speedtest results.
///
/// Converts speedtest results into a human-readable message with:
/// - Download speed in Mbps
/// - Upload speed in Mbps
/// - Latency in milliseconds
/// - Duration in seconds
/// - Jitter in milliseconds (if available)
/// - Packet loss percentage (if available)
///
/// # Arguments
///
/// * `result` - The speedtest results to format
/// * `duration` - How long the test took
///
/// # Returns
///
/// A formatted string with emoji icons suitable for notifications.
///
/// # Examples
///
/// ```
/// use netspeed_lite::notifier::format_success_message;
/// use netspeed_lite::runner::SpeedtestResult;
/// use std::time::Duration;
///
/// let result = SpeedtestResult {
///     download_bps: 100_000_000.0,
///     upload_bps: 10_000_000.0,
///     latency_seconds: 0.020,
///     jitter_seconds: Some(0.002),
///     packet_loss_ratio: None,
/// };
/// let message = format_success_message(&result, Duration::from_secs(30));
/// assert!(message.contains("100.0 Mbps"));
/// ```
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

/// Formats a failure notification message from an error category.
///
/// Converts error information into a concise, human-readable message.
///
/// # Arguments
///
/// * `error` - The error category that caused the failure
///
/// # Returns
///
/// A formatted error message string.
///
/// # Examples
///
/// ```
/// use netspeed_lite::notifier::format_failure_message;
/// use netspeed_lite::runner::ErrorCategory;
///
/// let error = ErrorCategory::Timeout(120);
/// let message = format_failure_message(&error);
/// assert_eq!(message, "timeout after 120s");
/// ```
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
