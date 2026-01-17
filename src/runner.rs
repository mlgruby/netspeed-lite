//! # Speedtest Runner
//!
//! This module is responsible for executing the speedtest CLI command and parsing its output.
//! It handles:
//! - Constructing the command with proper arguments.
//! - Executing the process and capturing stdout/stderr.
//! - Parsing the JSON output into a `SpeedtestResult` struct.
//! - Handling parsing errors and standardizing the result format.
use anyhow::Result;
use serde::Deserialize;
use std::process::Stdio;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub struct SpeedtestResult {
    pub download_bps: f64,
    pub upload_bps: f64,
    pub latency_seconds: f64,
    pub jitter_seconds: Option<f64>,
    pub packet_loss_ratio: Option<f64>,
}

#[derive(Debug)]
pub enum RunOutcome {
    Success(SpeedtestResult),
    Failure(ErrorCategory),
}

#[derive(Debug, Error)]
pub enum ErrorCategory {
    #[error("Command timed out after {0} seconds")]
    Timeout(u64),

    #[error("Command not found: {0}")]
    CommandNotFound(String),

    #[error("Command failed with exit code {0}")]
    CommandFailed(i32),

    #[error("Invalid output: {0}")]
    InvalidOutput(String),

    #[error("Missing required fields: {0}")]
    MissingFields(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Deserialize)]
struct SpeedtestOutput {
    download: Option<BandwidthInfo>,
    upload: Option<BandwidthInfo>,
    ping: Option<PingInfo>,
}

#[derive(Debug, Deserialize)]
struct BandwidthInfo {
    bandwidth: Option<f64>, // in bytes per second
}

#[derive(Debug, Deserialize)]
struct PingInfo {
    latency: Option<f64>, // in milliseconds
    jitter: Option<f64>,  // in milliseconds
}

pub struct RunResult {
    pub outcome: RunOutcome,
    pub duration: Duration,
}

pub async fn run_speedtest(command: &str, args: &[String], timeout_seconds: u64) -> RunResult {
    let start = Instant::now();

    let outcome = match execute_speedtest(command, args, timeout_seconds).await {
        Ok(result) => RunOutcome::Success(result),
        Err(e) => RunOutcome::Failure(e),
    };

    let duration = start.elapsed();

    RunResult { outcome, duration }
}

async fn execute_speedtest(
    command: &str,
    args: &[String],
    timeout_seconds: u64,
) -> Result<SpeedtestResult, ErrorCategory> {
    let timeout_duration = Duration::from_secs(timeout_seconds);

    let child = Command::new(command)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ErrorCategory::CommandNotFound(command.to_string())
            } else {
                ErrorCategory::Internal(format!("Failed to spawn command: {}", e))
            }
        })?;

    let output = timeout(timeout_duration, child.wait_with_output())
        .await
        .map_err(|_| ErrorCategory::Timeout(timeout_seconds))?
        .map_err(|e| ErrorCategory::Internal(format!("Failed to wait for command: {}", e)))?;

    if !output.status.success() {
        let exit_code = output.status.code().unwrap_or(-1);
        return Err(ErrorCategory::CommandFailed(exit_code));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_speedtest_output(&stdout)
}

pub fn parse_speedtest_output(json_str: &str) -> Result<SpeedtestResult, ErrorCategory> {
    let output: SpeedtestOutput = serde_json::from_str(json_str)
        .map_err(|e| ErrorCategory::InvalidOutput(format!("JSON parse error: {}", e)))?;

    // Extract download bandwidth (bytes/s -> bits/s)
    let download_bps = output
        .download
        .and_then(|d| d.bandwidth)
        .ok_or_else(|| ErrorCategory::MissingFields("download.bandwidth".to_string()))?
        * 8.0; // Convert bytes to bits

    // Extract upload bandwidth (bytes/s -> bits/s)
    let upload_bps = output
        .upload
        .and_then(|u| u.bandwidth)
        .ok_or_else(|| ErrorCategory::MissingFields("upload.bandwidth".to_string()))?
        * 8.0; // Convert bytes to bits

    // Extract latency (ms -> seconds)
    let latency_seconds = output
        .ping
        .as_ref()
        .and_then(|p| p.latency)
        .ok_or_else(|| ErrorCategory::MissingFields("ping.latency".to_string()))?
        / 1000.0; // Convert ms to seconds

    // Extract optional jitter (ms -> seconds)
    let jitter_seconds = output
        .ping
        .as_ref()
        .and_then(|p| p.jitter)
        .map(|j| j / 1000.0);

    // Validate values
    if download_bps < 0.0 || download_bps.is_nan() {
        return Err(ErrorCategory::InvalidOutput(format!(
            "Invalid download speed: {}",
            download_bps
        )));
    }

    if upload_bps < 0.0 || upload_bps.is_nan() {
        return Err(ErrorCategory::InvalidOutput(format!(
            "Invalid upload speed: {}",
            upload_bps
        )));
    }

    if latency_seconds < 0.0 || latency_seconds.is_nan() {
        return Err(ErrorCategory::InvalidOutput(format!(
            "Invalid latency: {}",
            latency_seconds
        )));
    }

    Ok(SpeedtestResult {
        download_bps,
        upload_bps,
        latency_seconds,
        jitter_seconds,
        packet_loss_ratio: None, // Ookla CLI doesn't provide packet loss
    })
}
