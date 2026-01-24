//! # NetSpeed-Lite Main Application
//!
//! This is the entry point for the NetSpeed-Lite application.
//! It handles:
//! - Setting up logging/tracing.
//! - Loading configuration.
//! - Initializing Prometheus metrics.
//! - Spawning background tasks for:
//!   - Running speed tests (based on schedule).
//!   - Collecting resource usage metrics (CPU/Memory).
//! - Starting the HTTP server for metrics exposure.
//!
//! The application uses `tokio` as the async runtime.
mod config;
mod metrics;
mod notifier;
mod runner;
mod scheduler;
mod server;

use anyhow::Result;
use config::Config;
use metrics::Metrics;
use notifier::Notifier;
use scheduler::Scheduler;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting netspeed-lite");

    // Load configuration
    let config = Config::from_env()?;
    tracing::info!("Configuration loaded successfully");
    tracing::debug!("Bind address: {}", config.server.bind_address);
    tracing::debug!(
        "Schedule mode: {:?}, interval: {}s",
        config.schedule.mode,
        config.schedule.interval_seconds
    );
    tracing::debug!("Timezone: {}", config.schedule.timezone);

    // Initialize metrics
    let metrics = Metrics::new()?;
    tracing::info!("Metrics initialized");

    // Initialize notifier if configured
    let notifier = config.ntfy.clone().map(|ntfy_config| {
        tracing::info!("Notifier configured for {}", ntfy_config.url);
        Notifier::new(ntfy_config, metrics.clone())
    });

    // Create scheduler
    let scheduler = Scheduler::new(config.clone(), metrics.clone(), notifier);

    // Spawn scheduler task
    let scheduler_handle = tokio::spawn(async move {
        scheduler.run().await;
    });

    // Spawn resource monitoring task
    let resource_metrics = metrics.clone();
    let resource_interval = config.resource_interval_seconds;
    let resource_handle = tokio::spawn(async move {
        let mut cpu_tracker = CpuTracker::new();

        loop {
            // Update Memory (RSS)
            match read_memory_rss().await {
                Ok(bytes) => resource_metrics.process_memory_bytes.set(bytes as f64),
                Err(e) => tracing::warn!("Failed to read memory RSS: {}", e),
            }

            // Update CPU Usage
            match read_cpu_usage(&mut cpu_tracker).await {
                Ok(usage) => resource_metrics.process_cpu_usage.set(usage),
                Err(e) => tracing::warn!("Failed to read CPU usage: {}", e),
            }

            tokio::time::sleep(std::time::Duration::from_secs(resource_interval)).await;
        }
    });

    // Start HTTP server
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server::serve(config.server.bind_address.clone(), metrics).await {
            tracing::error!("Server error: {}", e);
        }
    });

    // Wait for any task to complete
    tokio::select! {
        _ = scheduler_handle => {
            tracing::error!("Scheduler task exited unexpectedly");
        }
        _ = server_handle => {
            tracing::error!("Server task exited unexpectedly");
        }
        _ = resource_handle => {
            tracing::error!("Resource monitor task exited unexpectedly");
        }
    }

    Ok(())
}

// --- Resource Monitoring Helpers (Linux /proc) ---

/// Reads the process's Resident Set Size (RSS) memory usage from `/proc/self/status`.
///
/// This function parses the `VmRSS` field from the Linux proc filesystem,
/// which represents the amount of physical memory currently in use by the process.
///
/// # Returns
///
/// Returns `Ok(u64)` with memory usage in bytes, or `Err` if:
/// - The `/proc/self/status` file cannot be read (non-Linux systems)
/// - The `VmRSS` field is not found
/// - The value cannot be parsed
///
/// Returns `Ok(0)` if the file is read but VmRSS is not found.
///
/// # Platform Support
///
/// This function only works on Linux. On other platforms, it will return an error.
async fn read_memory_rss() -> Result<u64> {
    let content = std::fs::read_to_string("/proc/self/status")?;
    for line in content.lines() {
        if line.starts_with("VmRSS:") {
            // Example: VmRSS:    5632 kB
            if let Some(kb_str) = line.split_whitespace().nth(1) {
                let kb: u64 = kb_str.parse()?;
                return Ok(kb * 1024); // Convert kB to bytes
            }
        }
    }
    Ok(0)
}

/// Tracks CPU usage state between measurements.
///
/// This struct stores the previous tick counts to calculate CPU usage delta.
struct CpuTracker {
    last_proc_ticks: u64,
    last_sys_ticks: u64,
}

impl CpuTracker {
    /// Creates a new CpuTracker with initial tick counts of 0.
    fn new() -> Self {
        Self {
            last_proc_ticks: 0,
            last_sys_ticks: 0,
        }
    }
}

/// Reads the process's CPU usage percentage from `/proc/self/stat` and `/proc/stat`.
///
/// This function calculates CPU usage by:
/// 1. Reading process CPU ticks (utime + stime) from `/proc/self/stat`
/// 2. Reading total system CPU ticks from `/proc/stat`
/// 3. Computing the delta since the last measurement
/// 4. Calculating percentage: (process_delta / system_delta) * 100
///
/// # Arguments
///
/// * `tracker` - Mutable reference to CpuTracker storing previous tick counts
///
/// # Returns
///
/// Returns `Ok(f64)` with CPU usage percentage (0.0 to 100.0+), or `Err` if:
/// - The proc files cannot be read (non-Linux systems)
/// - The file format is invalid
/// - Values cannot be parsed
///
/// Returns `Ok(0.0)` if this is the first measurement (no delta available) or
/// if the system delta is 0.
///
/// # Platform Support
///
/// This function only works on Linux. On other platforms, it will return an error.
///
/// # Note
///
/// CPU usage can exceed 100% on multi-core systems if the process uses multiple cores.
async fn read_cpu_usage(tracker: &mut CpuTracker) -> Result<f64> {
    // 1. Read process ticks from /proc/self/stat
    // Format: pid... utime(13) stime(14)
    let stat_content = std::fs::read_to_string("/proc/self/stat")?;
    let close_paren_idx = stat_content
        .rfind(')')
        .ok_or_else(|| anyhow::anyhow!("Invalid stat fmt"))?;
    let after_paren = &stat_content[close_paren_idx + 1..];

    // utime is index 11 (13-2), stime is index 12 (14-2) relative to parts after ')'
    let mut parts = after_paren.split_whitespace();
    let utime: u64 = parts
        .nth(11)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("Failed to parse utime"))?;
    let stime: u64 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("Failed to parse stime"))?;
    let current_proc_ticks = utime + stime;

    // 2. Read system ticks from /proc/stat
    let sys_content = std::fs::read_to_string("/proc/stat")?;
    let first_line = sys_content
        .lines()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Empty /proc/stat"))?;
    // skip "cpu" and sum all tick values
    let current_sys_ticks: u64 = first_line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse::<u64>().ok())
        .sum();

    // 3. Calculate Delta
    let delta_proc = current_proc_ticks.saturating_sub(tracker.last_proc_ticks);
    let delta_sys = current_sys_ticks.saturating_sub(tracker.last_sys_ticks);

    tracker.last_proc_ticks = current_proc_ticks;
    tracker.last_sys_ticks = current_sys_ticks;

    if delta_sys == 0 {
        return Ok(0.0);
    }

    // Percentage = (proc_delta / sys_delta) * 100
    // Units (jiffies) cancel out, so no need for CLK_TCK
    Ok((delta_proc as f64 / delta_sys as f64) * 100.0)
}
