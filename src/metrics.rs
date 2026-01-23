//! # Metrics Collection
//!
//! This module handles Prometheus metrics definition and registration.
//! It uses the `prometheus` crate to define Gauges and Counters.
//!
//! Metrics include:
//! - Speed test results: `netspeed_download_bps`, `netspeed_upload_bps`, `netspeed_latency_seconds`.
//! - Network quality: `netspeed_jitter_seconds`, `netspeed_packet_loss_ratio`.
//! - Operational: `netspeed_last_run_seconds`, `netspeed_notify_total`.
//! - Resource usage: `netspeed_process_cpu_usage`, `netspeed_process_memory_bytes`.
use prometheus::{Encoder, Gauge, IntCounterVec, Opts, Registry, TextEncoder};
use std::sync::Arc;

#[derive(Clone)]
pub struct Metrics {
    registry: Arc<Registry>,

    // Run status & counters
    pub last_success: Gauge,
    pub runs_total: IntCounterVec,
    pub run_duration_seconds: Gauge,
    pub run_timestamp_seconds: Gauge,

    // Resource usage
    pub process_cpu_usage: Gauge,
    pub process_memory_bytes: Gauge,

    // Measurements
    pub download_bps: Gauge,
    pub upload_bps: Gauge,
    pub latency_seconds: Gauge,
    pub jitter_seconds: Gauge,
    pub packet_loss_ratio: Gauge,

    // Operational
    pub notify_total: IntCounterVec,
}

impl Metrics {
    /// Creates a new Metrics instance with all Prometheus metrics registered.
    ///
    /// This function initializes and registers the following metrics:
    /// - `netspeed_last_success`: Gauge indicating if last run was successful (0 or 1)
    /// - `netspeed_runs_total`: Counter for total runs by outcome (success/failure/skipped)
    /// - `netspeed_run_duration_seconds`: Gauge for last run duration
    /// - `netspeed_run_timestamp_seconds`: Gauge for last run timestamp
    /// - `netspeed_process_cpu_usage`: Gauge for process CPU usage percentage
    /// - `netspeed_process_memory_bytes`: Gauge for process memory in bytes
    /// - `netspeed_download_bps`: Gauge for download speed in bits per second
    /// - `netspeed_upload_bps`: Gauge for upload speed in bits per second
    /// - `netspeed_latency_seconds`: Gauge for latency in seconds
    /// - `netspeed_jitter_seconds`: Gauge for jitter in seconds (optional)
    /// - `netspeed_packet_loss_ratio`: Gauge for packet loss ratio 0-1 (optional)
    /// - `netspeed_notify_total`: Counter for notifications sent by outcome
    ///
    /// # Returns
    ///
    /// Returns `Ok(Metrics)` if all metrics are successfully registered, or `Err` if
    /// metric registration fails (e.g., duplicate metric names).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use netspeed_lite::metrics::Metrics;
    ///
    /// let metrics = Metrics::new().expect("Failed to create metrics");
    /// metrics.download_bps.set(100_000_000.0); // 100 Mbps
    /// ```
    pub fn new() -> anyhow::Result<Self> {
        let registry = Registry::new();

        // Run status & counters
        let last_success = Gauge::new(
            "netspeed_last_success",
            "Whether the last run was successful (0 or 1)",
        )?;
        registry.register(Box::new(last_success.clone()))?;

        let runs_total = IntCounterVec::new(
            Opts::new("netspeed_runs_total", "Total number of speed test runs"),
            &["outcome"],
        )?;
        registry.register(Box::new(runs_total.clone()))?;

        let run_duration_seconds = Gauge::new(
            "netspeed_run_duration_seconds",
            "Duration of the last speed test run in seconds",
        )?;
        registry.register(Box::new(run_duration_seconds.clone()))?;

        let run_timestamp_seconds = Gauge::new(
            "netspeed_run_timestamp_seconds",
            "Unix timestamp of the last speed test completion",
        )?;
        registry.register(Box::new(run_timestamp_seconds.clone()))?;

        // Resource usage
        let process_cpu_usage =
            Gauge::new("netspeed_process_cpu_usage", "Process CPU usage percentage")?;
        registry.register(Box::new(process_cpu_usage.clone()))?;

        let process_memory_bytes = Gauge::new(
            "netspeed_process_memory_bytes",
            "Process memory usage in bytes",
        )?;
        registry.register(Box::new(process_memory_bytes.clone()))?;

        // Measurements
        let download_bps =
            Gauge::new("netspeed_download_bps", "Download speed in bits per second")?;
        registry.register(Box::new(download_bps.clone()))?;

        let upload_bps = Gauge::new("netspeed_upload_bps", "Upload speed in bits per second")?;
        registry.register(Box::new(upload_bps.clone()))?;

        let latency_seconds = Gauge::new("netspeed_latency_seconds", "Latency in seconds")?;
        registry.register(Box::new(latency_seconds.clone()))?;

        let jitter_seconds = Gauge::new("netspeed_jitter_seconds", "Jitter in seconds (optional)")?;
        registry.register(Box::new(jitter_seconds.clone()))?;

        let packet_loss_ratio = Gauge::new(
            "netspeed_packet_loss_ratio",
            "Packet loss ratio from 0 to 1 (optional)",
        )?;
        registry.register(Box::new(packet_loss_ratio.clone()))?;

        // Operational
        let notify_total = IntCounterVec::new(
            Opts::new(
                "netspeed_notify_total",
                "Total number of notifications sent",
            ),
            &["outcome"],
        )?;
        registry.register(Box::new(notify_total.clone()))?;

        Ok(Metrics {
            registry: Arc::new(registry),
            last_success,
            runs_total,
            run_duration_seconds,
            run_timestamp_seconds,
            process_cpu_usage,
            process_memory_bytes,
            download_bps,
            upload_bps,
            latency_seconds,
            jitter_seconds,
            packet_loss_ratio,
            notify_total,
        })
    }

    /// Renders all registered metrics in Prometheus text format.
    ///
    /// This function gathers all metrics from the registry and encodes them
    /// using the Prometheus text format (version 0.0.4).
    ///
    /// # Returns
    ///
    /// Returns `Ok(String)` containing the metrics in Prometheus format, or `Err` if
    /// encoding fails or the output is not valid UTF-8.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use netspeed_lite::metrics::Metrics;
    ///
    /// let metrics = Metrics::new().expect("Failed to create metrics");
    /// let output = metrics.render().expect("Failed to render metrics");
    /// println!("{}", output);
    /// ```
    pub fn render(&self) -> anyhow::Result<String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new().expect("Failed to create metrics")
    }
}
