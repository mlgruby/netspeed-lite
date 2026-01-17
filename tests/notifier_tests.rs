use netspeed_lite::notifier::{format_failure_message, format_success_message};
use netspeed_lite::runner::{ErrorCategory, SpeedtestResult};
use std::time::Duration;

#[test]
fn test_format_success_message() {
    let result = SpeedtestResult {
        download_bps: 812_300_000.0,
        upload_bps: 42_100_000.0,
        latency_seconds: 0.0184,
        jitter_seconds: Some(0.0021),
        packet_loss_ratio: None,
    };

    let duration = Duration::from_secs(30);
    let message = format_success_message(&result, duration);

    assert!(message.contains("⬇️ Download: 812.3 Mbps"));
    assert!(message.contains("⬆️ Upload: 42.1 Mbps"));
    assert!(message.contains("📡 Ping: 18.4 ms"));
    assert!(message.contains("⏱️ Duration: 30s"));
    assert!(message.contains("📊 Jitter: 2.1 ms"));
}

#[test]
fn test_format_failure_timeout() {
    let error = ErrorCategory::Timeout(120);
    let message = format_failure_message(&error);
    assert_eq!(message, "timeout after 120s");
}

#[test]
fn test_format_failure_command_failed() {
    let error = ErrorCategory::CommandFailed(1);
    let message = format_failure_message(&error);
    assert_eq!(message, "exit=1");
}
