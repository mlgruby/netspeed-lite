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

#[tokio::main]
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
        // Only refresh process-specific information to keep overhead low
        let mut sys = sysinfo::System::new();
        let pid = sysinfo::Pid::from_u32(std::process::id());

        loop {
            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
            if let Some(process) = sys.process(pid) {
                resource_metrics
                    .process_cpu_usage
                    .set(process.cpu_usage() as f64);
                resource_metrics
                    .process_memory_bytes
                    .set(process.memory() as f64);
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

    // Wait for either task to complete (they shouldn't unless there's an error)
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
