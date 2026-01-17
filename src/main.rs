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
    tokio::spawn(async move {
        let mut cpu_tracker = CpuTracker::new();

        loop {
            // Update Memory (RSS)
            if let Ok(bytes) = read_memory_rss() {
                resource_metrics.process_memory_bytes.set(bytes as f64);
            }

            // Update CPU Usage
            if let Ok(usage) = read_cpu_usage(&mut cpu_tracker) {
                resource_metrics.process_cpu_usage.set(usage);
            }

            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });

    // Start HTTP server
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server::serve(config.server.bind_address.clone(), metrics).await {
            tracing::error!("Server error: {}", e);
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = scheduler_handle => {
            tracing::error!("Scheduler task exited unexpectedly");
        }
        _ = server_handle => {
            tracing::error!("Server task exited unexpectedly");
        }
    }

    Ok(())
}

// --- Resource Monitoring Helpers (Linux /proc) ---

fn read_memory_rss() -> Result<u64> {
    let content = std::fs::read_to_string("/proc/self/status")?;
    for line in content.lines() {
        if line.starts_with("VmRSS:") {
            // Example: VmRSS:    5632 kB
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb: u64 = parts[1].parse()?;
                return Ok(kb * 1024); // Convert kB to bytes
            }
        }
    }
    Ok(0)
}

struct CpuTracker {
    last_proc_ticks: u64,
    last_sys_ticks: u64,
}

impl CpuTracker {
    fn new() -> Self {
        Self {
            last_proc_ticks: 0,
            last_sys_ticks: 0,
        }
    }
}

fn read_cpu_usage(tracker: &mut CpuTracker) -> Result<f64> {
    // 1. Read process ticks from /proc/self/stat
    // Format: pid... utime(13) stime(14)
    let stat_content = std::fs::read_to_string("/proc/self/stat")?;
    let close_paren_idx = stat_content
        .rfind(')')
        .ok_or_else(|| anyhow::anyhow!("Invalid stat fmt"))?;
    let after_paren = &stat_content[close_paren_idx + 1..];
    let parts: Vec<&str> = after_paren.split_whitespace().collect();

    // utime is index 11 (13-2), stime is index 12 (14-2) relative to parts after ')'
    if parts.len() < 13 {
        return Ok(0.0);
    }
    let utime: u64 = parts[11].parse()?;
    let stime: u64 = parts[12].parse()?;
    let current_proc_ticks = utime + stime;

    // 2. Read system ticks from /proc/stat
    let sys_content = std::fs::read_to_string("/proc/stat")?;
    let first_line = sys_content
        .lines()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Empty /proc/stat"))?;
    // skip "cpu"
    let sys_parts: Vec<&str> = first_line.split_whitespace().skip(1).collect();
    let mut current_sys_ticks = 0;
    for part in sys_parts {
        current_sys_ticks += part.parse::<u64>().unwrap_or(0);
    }

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
