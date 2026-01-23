use netspeed_lite::config::Config;
use serial_test::serial;
use std::env;

// Helper to clear all netspeed env vars before each test
fn clear_env_vars() {
    let keys = [
        "NETSPEED_BIND",
        "NETSPEED_SCHEDULE_MODE",
        "NETSPEED_INTERVAL_SECONDS",
        "NETSPEED_SCHEDULE",
        "NETSPEED_TIMEZONE",
        "NETSPEED_ALLOW_OVERLAP",
        "NETSPEED_TIMEOUT_SECONDS",
        "NETSPEED_NTFY_URL",
        "NETSPEED_NTFY_TOKEN",
        "NETSPEED_NTFY_TITLE",
        "NETSPEED_NTFY_TAGS",
        "NETSPEED_NTFY_PRIORITY",
        "NETSPEED_NTFY_CLICK",
        "NETSPEED_NOTIFY_ON",
        "NETSPEED_RESOURCE_INTERVAL_SECONDS",
    ];
    for key in &keys {
        env::remove_var(key);
    }
}

#[test]
#[serial]
fn test_default_configuration() {
    // Given: No environment variables are set
    clear_env_vars();

    // When: Loading configuration from environment
    let config = Config::from_env().expect("Failed to load default config");

    // Then: Should use all default values
    assert_eq!(config.server.bind_address, "0.0.0.0:9109");
    assert_eq!(config.schedule.interval_seconds, 3600);
    assert_eq!(config.schedule.timezone, "Europe/Brussels");
    assert!(!config.schedule.allow_overlap);
    assert_eq!(config.speedtest.timeout_seconds, 120);
    assert!(config.notify_on.success);
    assert!(config.notify_on.failure);
    assert_eq!(config.resource_interval_seconds, 15);
}

#[test]
#[serial]
fn test_invalid_timezone() {
    // Given: An invalid timezone is set
    clear_env_vars();
    env::set_var("NETSPEED_TIMEZONE", "Invalid/Timezone");

    // When: Loading configuration
    let result = Config::from_env();

    // Then: Should fail with timezone error
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid timezone"));
}

#[test]
#[serial]
fn test_zero_timeout_rejection() {
    // Given: Timeout is set to 0
    clear_env_vars();
    env::set_var("NETSPEED_TIMEOUT_SECONDS", "0");

    // When: Loading configuration
    let result = Config::from_env();

    // Then: Should reject zero timeout with error
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("must be greater than 0"));
}

#[test]
#[serial]
fn test_invalid_schedule_mode() {
    // Given: An invalid schedule mode is set
    clear_env_vars();
    env::set_var("NETSPEED_SCHEDULE_MODE", "invalid_mode");

    // When: Loading configuration
    let result = Config::from_env();

    // Then: Should fail with schedule mode error
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid schedule mode"));
}

#[test]
#[serial]
fn test_interval_mode() {
    // Given: Interval mode is configured with 1800 seconds
    clear_env_vars();
    env::set_var("NETSPEED_SCHEDULE_MODE", "interval");
    env::set_var("NETSPEED_INTERVAL_SECONDS", "1800");

    // When: Loading configuration
    let config = Config::from_env().expect("Failed to load config");

    // Then: Should use the specified interval
    assert_eq!(config.schedule.interval_seconds, 1800);
}

#[test]
#[serial]
fn test_cron_mode() {
    // Given: Cron mode is configured with an expression
    clear_env_vars();
    env::set_var("NETSPEED_SCHEDULE_MODE", "cron");
    env::set_var("NETSPEED_SCHEDULE", "0 */2 * * *");

    // When: Loading configuration
    let config = Config::from_env().expect("Failed to load config");

    // Then: Should use the cron expression
    assert_eq!(
        config.schedule.cron_expression,
        Some("0 */2 * * *".to_string())
    );
}

#[test]
#[serial]
fn test_ntfy_configuration() {
    // Given: Ntfy is fully configured
    clear_env_vars();
    env::set_var("NETSPEED_NTFY_URL", "https://ntfy.sh/test");
    env::set_var("NETSPEED_NTFY_TOKEN", "test_token");
    env::set_var("NETSPEED_NTFY_TITLE", "Test Title");
    env::set_var("NETSPEED_NTFY_TAGS", "test,tags");
    env::set_var("NETSPEED_NTFY_PRIORITY", "5");

    // When: Loading configuration
    let config = Config::from_env().expect("Failed to load config");

    // Then: Should load all ntfy settings correctly
    let ntfy = config.ntfy.expect("Ntfy config should be present");
    assert_eq!(ntfy.url, "https://ntfy.sh/test");
    assert_eq!(ntfy.token, Some("test_token".to_string()));
    assert_eq!(ntfy.title, "Test Title");
    assert_eq!(ntfy.tags, "test,tags");
    assert_eq!(ntfy.priority, 5);
}

#[test]
#[serial]
fn test_ntfy_optional() {
    // Given: No ntfy URL is configured
    clear_env_vars();

    // When: Loading configuration
    let config = Config::from_env().expect("Failed to load config");

    // Then: Ntfy config should be None
    assert!(config.ntfy.is_none());
}

#[test]
#[serial]
fn test_ntfy_priority_clamping() {
    // Given: Ntfy priority is set above maximum (10 > 5)
    clear_env_vars();
    env::set_var("NETSPEED_NTFY_URL", "https://ntfy.sh/test");
    env::set_var("NETSPEED_NTFY_PRIORITY", "10");

    // When: Loading configuration
    let config = Config::from_env().expect("Failed to load config");

    // Then: Priority should be clamped to maximum of 5
    let ntfy = config.ntfy.expect("Ntfy config should be present");
    assert_eq!(ntfy.priority, 5);
}

#[test]
#[serial]
fn test_notify_on_success_only() {
    // Given: Notify on is set to success only
    clear_env_vars();
    env::set_var("NETSPEED_NOTIFY_ON", "success");

    // When: Loading configuration
    let config = Config::from_env().expect("Failed to load config");

    // Then: Should only notify on success
    assert!(config.notify_on.success);
    assert!(!config.notify_on.failure);
}

#[test]
#[serial]
fn test_notify_on_failure_only() {
    // Given: Notify on is set to failure only
    clear_env_vars();
    env::set_var("NETSPEED_NOTIFY_ON", "failure");

    // When: Loading configuration
    let config = Config::from_env().expect("Failed to load config");

    // Then: Should only notify on failure
    assert!(!config.notify_on.success);
    assert!(config.notify_on.failure);
}

#[test]
#[serial]
fn test_allow_overlap_true() {
    // Given: Allow overlap is enabled
    clear_env_vars();
    env::set_var("NETSPEED_ALLOW_OVERLAP", "true");

    // When: Loading configuration
    let config = Config::from_env().expect("Failed to load config");

    // Then: Should allow overlapping runs
    assert!(config.schedule.allow_overlap);
}

#[test]
#[serial]
fn test_custom_bind_address() {
    // Given: Custom bind address is set
    clear_env_vars();
    env::set_var("NETSPEED_BIND", "127.0.0.1:8080");

    // When: Loading configuration
    let config = Config::from_env().expect("Failed to load config");

    // Then: Should use the custom address
    assert_eq!(config.server.bind_address, "127.0.0.1:8080");
}

#[test]
#[serial]
fn test_invalid_interval_seconds() {
    // Given: Interval seconds is not a number
    clear_env_vars();
    env::set_var("NETSPEED_INTERVAL_SECONDS", "not_a_number");

    // When: Loading configuration
    let result = Config::from_env();

    // Then: Should fail with parse error
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_invalid_timeout_seconds() {
    // Given: Timeout seconds is not a number
    clear_env_vars();
    env::set_var("NETSPEED_TIMEOUT_SECONDS", "not_a_number");

    // When: Loading configuration
    let result = Config::from_env();

    // Then: Should fail with parse error
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_resource_interval_configuration() {
    // Given: Resource interval is set to 30 seconds
    clear_env_vars();
    env::set_var("NETSPEED_RESOURCE_INTERVAL_SECONDS", "30");

    // When: Loading configuration
    let config = Config::from_env().expect("Failed to load config");

    // Then: Should use the specified interval
    assert_eq!(config.resource_interval_seconds, 30);
}

#[test]
#[serial]
fn test_invalid_resource_interval() {
    // Given: Resource interval is not a number
    clear_env_vars();
    env::set_var("NETSPEED_RESOURCE_INTERVAL_SECONDS", "invalid");

    // When: Loading configuration
    let result = Config::from_env();

    // Then: Should fail with parse error
    assert!(result.is_err());
}
