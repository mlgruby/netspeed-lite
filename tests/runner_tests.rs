use netspeed_lite::runner::{parse_speedtest_output, ErrorCategory};

#[test]
fn test_parse_valid_output() {
    let json = r#"{
        "download": {"bandwidth": 101537500},
        "upload": {"bandwidth": 5262500},
        "ping": {"latency": 18.4, "jitter": 2.1}
    }"#;

    let result = parse_speedtest_output(json).unwrap();
    assert_eq!(result.download_bps, 812300000.0); // 101537500 * 8
    assert_eq!(result.upload_bps, 42100000.0); // 5262500 * 8
    assert_eq!(result.latency_seconds, 0.0184); // 18.4 / 1000
                                                // Use approximate comparison for jitter due to floating point precision
    assert!((result.jitter_seconds.unwrap() - 0.0021).abs() < 1e-10);
}

#[test]
fn test_parse_missing_download() {
    let json = r#"{
        "upload": {"bandwidth": 5262500},
        "ping": {"latency": 18.4}
    }"#;

    let result = parse_speedtest_output(json);
    assert!(matches!(result, Err(ErrorCategory::MissingFields(_))));
}

#[test]
fn test_parse_invalid_json() {
    let json = "not json";
    let result = parse_speedtest_output(json);
    assert!(matches!(result, Err(ErrorCategory::InvalidOutput(_))));
}
