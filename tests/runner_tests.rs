use netspeed_lite::runner::{parse_speedtest_output, ErrorCategory};

#[test]
fn test_parse_valid_output() {
    // Given: Valid JSON output from Ookla Speedtest CLI
    let json = r#"{
        "download": {"bandwidth": 101537500},
        "upload": {"bandwidth": 5262500},
        "ping": {"latency": 18.4, "jitter": 2.1}
    }"#;

    // When: Parsing the output
    let result = parse_speedtest_output(json).unwrap();

    // Then: Should convert units correctly (bytes->bits, ms->seconds)
    assert_eq!(result.download_bps, 812300000.0); // 101537500 * 8
    assert_eq!(result.upload_bps, 42100000.0); // 5262500 * 8
    assert_eq!(result.latency_seconds, 0.0184); // 18.4 / 1000
                                                // Use approximate comparison for jitter due to floating point precision
    assert!((result.jitter_seconds.unwrap() - 0.0021).abs() < 1e-10);
}

#[test]
fn test_parse_missing_download() {
    // Given: JSON output missing the required download field
    let json = r#"{
        "upload": {"bandwidth": 5262500},
        "ping": {"latency": 18.4}
    }"#;

    // When: Parsing the output
    let result = parse_speedtest_output(json);

    // Then: Should fail with MissingFields error
    assert!(matches!(result, Err(ErrorCategory::MissingFields(_))));
}

#[test]
fn test_parse_invalid_json() {
    // Given: Invalid JSON string
    let json = "not json";

    // When: Parsing the output
    let result = parse_speedtest_output(json);

    // Then: Should fail with InvalidOutput error
    assert!(matches!(result, Err(ErrorCategory::InvalidOutput(_))));
}
