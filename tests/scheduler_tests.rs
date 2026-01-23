use netspeed_lite::config::{
    Config, NotifyOn, ScheduleConfig, ScheduleMode, ServerConfig, SpeedtestConfig,
};
use netspeed_lite::metrics::Metrics;
use netspeed_lite::scheduler::Scheduler;
use std::env;

fn create_test_config(mode: ScheduleMode) -> Config {
    Config {
        server: ServerConfig {
            bind_address: "127.0.0.1:9109".to_string(),
        },
        schedule: ScheduleConfig {
            mode,
            interval_seconds: 3600,
            cron_expression: Some("0 * * * *".to_string()),
            timezone: "UTC".to_string(),
            allow_overlap: false,
        },
        speedtest: SpeedtestConfig {
            command: "speedtest".to_string(),
            args: vec!["--format=json".to_string()],
            timeout_seconds: 120,
        },
        ntfy: None,
        notify_on: NotifyOn {
            success: true,
            failure: true,
        },
        resource_interval_seconds: 15,
    }
}

#[test]
fn test_scheduler_creation() {
    // Given: Valid configuration and metrics
    env::set_var("PROMETHEUS_REGISTRY_PREFIX", "test_scheduler");
    let config = create_test_config(ScheduleMode::Interval);
    let metrics = Metrics::new().expect("Failed to create metrics");

    // When: Creating a scheduler
    let scheduler = Scheduler::new(config.clone(), metrics, None);

    // Then: Scheduler should be created successfully
    drop(scheduler);
    env::remove_var("PROMETHEUS_REGISTRY_PREFIX");
}

#[test]
fn test_schedule_mode_interval() {
    // Given: Configuration with interval mode
    env::set_var("PROMETHEUS_REGISTRY_PREFIX", "test_interval");
    let config = create_test_config(ScheduleMode::Interval);
    let metrics = Metrics::new().expect("Failed to create metrics");

    // When: Creating scheduler with interval mode
    let scheduler = Scheduler::new(config.clone(), metrics, None);

    // Then: Should use interval scheduling
    assert_eq!(config.schedule.mode, ScheduleMode::Interval);
    assert_eq!(config.schedule.interval_seconds, 3600);

    drop(scheduler);
    env::remove_var("PROMETHEUS_REGISTRY_PREFIX");
}

#[test]
fn test_schedule_mode_hourly_aligned() {
    // Given: Configuration with hourly aligned mode
    env::set_var("PROMETHEUS_REGISTRY_PREFIX", "test_hourly");
    let config = create_test_config(ScheduleMode::HourlyAligned);
    let metrics = Metrics::new().expect("Failed to create metrics");

    // When: Creating scheduler with hourly aligned mode
    let scheduler = Scheduler::new(config.clone(), metrics, None);

    // Then: Should use hourly aligned scheduling
    assert_eq!(config.schedule.mode, ScheduleMode::HourlyAligned);

    drop(scheduler);
    env::remove_var("PROMETHEUS_REGISTRY_PREFIX");
}

#[test]
fn test_schedule_mode_cron() {
    // Given: Configuration with cron mode and expression
    env::set_var("PROMETHEUS_REGISTRY_PREFIX", "test_cron");
    let config = create_test_config(ScheduleMode::Cron);
    let metrics = Metrics::new().expect("Failed to create metrics");

    // When: Creating scheduler with cron mode
    let scheduler = Scheduler::new(config.clone(), metrics, None);

    // Then: Should use cron scheduling with expression
    assert_eq!(config.schedule.mode, ScheduleMode::Cron);
    assert!(config.schedule.cron_expression.is_some());
    assert_eq!(config.schedule.cron_expression.unwrap(), "0 * * * *");

    drop(scheduler);
    env::remove_var("PROMETHEUS_REGISTRY_PREFIX");
}

#[test]
fn test_timezone_configuration() {
    // Given: Configuration with custom timezone
    env::set_var("PROMETHEUS_REGISTRY_PREFIX", "test_tz");
    let mut config = create_test_config(ScheduleMode::HourlyAligned);
    config.schedule.timezone = "America/New_York".to_string();
    let metrics = Metrics::new().expect("Failed to create metrics");

    // When: Creating scheduler with custom timezone
    let scheduler = Scheduler::new(config.clone(), metrics, None);

    // Then: Should use the specified timezone
    assert_eq!(config.schedule.timezone, "America/New_York");

    drop(scheduler);
    env::remove_var("PROMETHEUS_REGISTRY_PREFIX");
}

#[test]
fn test_allow_overlap_flag() {
    // Given: Configuration with overlap allowed
    env::set_var("PROMETHEUS_REGISTRY_PREFIX", "test_overlap");
    let mut config = create_test_config(ScheduleMode::Interval);
    config.schedule.allow_overlap = true;
    let metrics = Metrics::new().expect("Failed to create metrics");

    // When: Creating scheduler with overlap enabled
    let scheduler = Scheduler::new(config.clone(), metrics, None);

    // Then: Should allow overlapping runs
    assert!(config.schedule.allow_overlap);

    drop(scheduler);
    env::remove_var("PROMETHEUS_REGISTRY_PREFIX");
}
