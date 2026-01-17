//! # Configuration Management
//!
//! This module handles loading and validating application configuration from environment variables.
//! It uses `serde` for deserialization and provides defaults where appropriate.
//!
//! Key components:
//! - `Config`: The main configuration struct.
//! - `ScheduleMode`: Enum defining how tests are scheduled (Hourly, Interval, Cron).
//! - `SpeedtestConfig`: Configuration specific to the speedtest command.
//!
//! Note: The speedtest command and arguments are hardcoded to ensure compatibility
//! with the Ookla Speedtest CLI installed in the Docker container.
use anyhow::{Context, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub schedule: ScheduleConfig,
    pub speedtest: SpeedtestConfig,
    pub ntfy: Option<NtfyConfig>,
    pub notify_on: NotifyOn,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_address: String,
}

#[derive(Debug, Clone)]
pub struct ScheduleConfig {
    pub mode: ScheduleMode,
    pub interval_seconds: u64,
    pub cron_expression: Option<String>,
    pub timezone: String,
    pub allow_overlap: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScheduleMode {
    HourlyAligned,
    Interval,
    Cron,
}

#[derive(Debug, Clone)]
pub struct SpeedtestConfig {
    pub command: String,
    pub args: Vec<String>,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct NtfyConfig {
    pub url: String,
    pub token: Option<String>,
    pub title: String,
    pub tags: String,
    pub priority: u8,
    pub click_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NotifyOn {
    pub success: bool,
    pub failure: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bind_address = env::var("NETSPEED_BIND").unwrap_or_else(|_| "0.0.0.0:9109".to_string());

        let schedule_mode = match env::var("NETSPEED_SCHEDULE_MODE")
            .unwrap_or_else(|_| "hourly_aligned".to_string())
            .as_str()
        {
            "hourly_aligned" => ScheduleMode::HourlyAligned,
            "interval" => ScheduleMode::Interval,
            "cron" => ScheduleMode::Cron,
            other => anyhow::bail!("Invalid schedule mode: {}", other),
        };

        let interval_seconds = env::var("NETSPEED_INTERVAL_SECONDS")
            .unwrap_or_else(|_| "3600".to_string())
            .parse()
            .context("Invalid NETSPEED_INTERVAL_SECONDS")?;

        let cron_expression = env::var("NETSPEED_SCHEDULE").ok();

        let timezone =
            env::var("NETSPEED_TIMEZONE").unwrap_or_else(|_| "Europe/Brussels".to_string());

        // Validate timezone
        timezone
            .parse::<chrono_tz::Tz>()
            .with_context(|| format!("Invalid timezone: {}", timezone))?;

        let allow_overlap = env::var("NETSPEED_ALLOW_OVERLAP")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .context("Invalid NETSPEED_ALLOW_OVERLAP")?;

        // Hardcoded Ookla Speedtest configuration
        let command = "speedtest".to_string();

        let args = vec![
            "--format=json".to_string(),
            "--accept-license".to_string(),
            "--accept-gdpr".to_string(),
        ];

        let timeout_seconds = env::var("NETSPEED_TIMEOUT_SECONDS")
            .unwrap_or_else(|_| "120".to_string())
            .parse()
            .context("Invalid NETSPEED_TIMEOUT_SECONDS")?;

        if timeout_seconds == 0 {
            anyhow::bail!("NETSPEED_TIMEOUT_SECONDS must be greater than 0");
        }

        let ntfy_url = env::var("NETSPEED_NTFY_URL").ok();
        let ntfy = ntfy_url.map(|url| NtfyConfig {
            url,
            token: env::var("NETSPEED_NTFY_TOKEN").ok(),
            title: env::var("NETSPEED_NTFY_TITLE").unwrap_or_else(|_| "netspeed-lite".to_string()),
            tags: env::var("NETSPEED_NTFY_TAGS").unwrap_or_else(|_| "speedtest,isp".to_string()),
            priority: env::var("NETSPEED_NTFY_PRIORITY")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .unwrap_or(3)
                .clamp(1, 5),
            click_url: env::var("NETSPEED_NTFY_CLICK").ok(),
        });

        let notify_on_str =
            env::var("NETSPEED_NOTIFY_ON").unwrap_or_else(|_| "success,failure".to_string());
        let notify_on = NotifyOn {
            success: notify_on_str.contains("success"),
            failure: notify_on_str.contains("failure"),
        };

        Ok(Config {
            server: ServerConfig { bind_address },
            schedule: ScheduleConfig {
                mode: schedule_mode,
                interval_seconds,
                cron_expression,
                timezone,
                allow_overlap,
            },
            speedtest: SpeedtestConfig {
                command,
                args,
                timeout_seconds,
            },
            ntfy,
            notify_on,
        })
    }
}
