use netspeed_lite::metrics::Metrics;
use netspeed_lite::server;
use std::env;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_server_starts_and_responds() {
    // Given: A running HTTP server with metrics
    env::set_var("PROMETHEUS_REGISTRY_PREFIX", "test_integration_server");
    let metrics = Metrics::new().expect("Failed to create metrics");
    let bind_address = "127.0.0.1:19109".to_string();
    let server_handle = tokio::spawn(async move { server::serve(bind_address, metrics).await });
    sleep(Duration::from_millis(100)).await;

    // When: Making requests to root endpoint
    let response = reqwest::get("http://127.0.0.1:19109/")
        .await
        .expect("Failed to request root");

    // Then: Should return HTML landing page
    assert_eq!(response.status(), 200);
    let body = response.text().await.expect("Failed to read body");
    assert!(body.contains("netspeed-lite"));
    assert!(body.contains("/metrics"));
    assert!(body.contains("/healthz"));

    // When: Requesting metrics endpoint
    let response = reqwest::get("http://127.0.0.1:19109/metrics")
        .await
        .expect("Failed to request metrics");

    // Then: Should return Prometheus format metrics
    assert_eq!(response.status(), 200);
    let body = response.text().await.expect("Failed to read body");
    assert!(body.contains("netspeed"));

    // When: Requesting health endpoint
    let response = reqwest::get("http://127.0.0.1:19109/healthz")
        .await
        .expect("Failed to request health");

    // Then: Should return initializing status (no tests run yet)
    assert_eq!(response.status(), 503);
    let body = response.text().await.expect("Failed to read body");
    assert!(body.contains("status"));
    assert!(body.contains("initializing"));

    // Cleanup
    server_handle.abort();
    env::remove_var("PROMETHEUS_REGISTRY_PREFIX");
}

#[tokio::test]
async fn test_metrics_format() {
    // Given: Metrics with test values set
    env::set_var("PROMETHEUS_REGISTRY_PREFIX", "test_metrics_format");
    let metrics = Metrics::new().expect("Failed to create metrics");
    metrics.download_bps.set(100_000_000.0);
    metrics.upload_bps.set(10_000_000.0);
    metrics.latency_seconds.set(0.020);
    metrics.runs_total.with_label_values(&["success"]).inc();

    // When: Rendering metrics
    let rendered = metrics.render().expect("Failed to render metrics");

    // Then: Should be in valid Prometheus text format
    assert!(rendered.contains("# HELP"));
    assert!(rendered.contains("# TYPE"));
    assert!(rendered.contains("netspeed"));
    assert!(rendered.contains("download_bps"));
    assert!(rendered.contains("upload_bps"));
    assert!(rendered.contains("latency_seconds"));

    env::remove_var("PROMETHEUS_REGISTRY_PREFIX");
}

#[tokio::test]
async fn test_health_check_states() {
    // Given: A running server with modifiable metrics
    env::set_var("PROMETHEUS_REGISTRY_PREFIX", "test_health_states");
    let metrics = Metrics::new().expect("Failed to create metrics");
    let bind_address = "127.0.0.1:19110".to_string();
    let test_metrics = metrics.clone();
    let server_handle = tokio::spawn(async move { server::serve(bind_address, metrics).await });
    sleep(Duration::from_millis(100)).await;

    // When: Checking health before any runs
    // Then: Should return initializing status with 503
    let response = reqwest::get("http://127.0.0.1:19110/healthz")
        .await
        .expect("Failed to request health");
    assert_eq!(response.status(), 503);
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["status"], "initializing");

    // When: Setting metrics to indicate successful run
    test_metrics.last_success.set(1.0);
    test_metrics.run_timestamp_seconds.set(1234567890.0);

    let response = reqwest::get("http://127.0.0.1:19110/healthz")
        .await
        .expect("Failed to request health");

    // Then: Should return healthy status with 200
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["status"], "healthy");
    assert_eq!(body["last_run_timestamp"], 1234567890.0);

    // When: Setting metrics to indicate failed run
    test_metrics.last_success.set(0.0);
    test_metrics.run_timestamp_seconds.set(1234567900.0);

    let response = reqwest::get("http://127.0.0.1:19110/healthz")
        .await
        .expect("Failed to request health");

    // Then: Should return unhealthy status with 503
    assert_eq!(response.status(), 503);
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["status"], "unhealthy");

    // Cleanup
    server_handle.abort();
    env::remove_var("PROMETHEUS_REGISTRY_PREFIX");
}

#[tokio::test]
async fn test_metrics_content_type() {
    // Given: A running server
    env::set_var("PROMETHEUS_REGISTRY_PREFIX", "test_content_type");
    let metrics = Metrics::new().expect("Failed to create metrics");
    let bind_address = "127.0.0.1:19111".to_string();
    let server_handle = tokio::spawn(async move { server::serve(bind_address, metrics).await });
    sleep(Duration::from_millis(100)).await;

    // When: Requesting metrics endpoint
    let response = reqwest::get("http://127.0.0.1:19111/metrics")
        .await
        .expect("Failed to request metrics");

    // Then: Should return correct Prometheus content type
    assert_eq!(response.status(), 200);
    let content_type = response
        .headers()
        .get("content-type")
        .expect("Content-Type header missing");
    assert_eq!(content_type, "text/plain; version=0.0.4");

    // Cleanup
    server_handle.abort();
    env::remove_var("PROMETHEUS_REGISTRY_PREFIX");
}

#[tokio::test]
async fn test_concurrent_requests() {
    // Given: A running server
    env::set_var("PROMETHEUS_REGISTRY_PREFIX", "test_concurrent");
    let metrics = Metrics::new().expect("Failed to create metrics");
    let bind_address = "127.0.0.1:19112".to_string();
    let server_handle = tokio::spawn(async move { server::serve(bind_address, metrics).await });
    sleep(Duration::from_millis(100)).await;

    // When: Making 10 concurrent requests to metrics endpoint
    let mut handles = vec![];
    for _ in 0..10 {
        let handle = tokio::spawn(async move {
            reqwest::get("http://127.0.0.1:19112/metrics")
                .await
                .expect("Failed to request metrics")
        });
        handles.push(handle);
    }

    // Then: All requests should succeed
    for handle in handles {
        let response = handle.await.expect("Task panicked");
        assert_eq!(response.status(), 200);
    }

    // Cleanup
    server_handle.abort();
    env::remove_var("PROMETHEUS_REGISTRY_PREFIX");
}
