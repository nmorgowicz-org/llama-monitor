use llama_monitor::inference::rapid_mlx::poller::RapidMlxPoller;
use mockito::Server;
use serde_json::json;

#[tokio::test]
async fn test_rapid_mlx_poller_unit_conversion() {
    let mut server = Server::new_async().await;
    let url = server.url().parse::<url::Url>().expect("Invalid URL");
    let host = url.host_str().expect("No host").to_string();
    let port = url.port().expect("No port");

    let _m = server
        .mock("GET", "/v1/status")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "status": "idle",
                "metal": {
                    "active_memory_gb": 1.0,
                    "peak_memory_gb": 2.0,
                    "cache_memory_gb": 0.5
                }
            })
            .to_string(),
        )
        .create();

    let poller = RapidMlxPoller::new(&host, port, None);
    let snapshot = poller.poll().await.expect("Poll should succeed");

    assert_eq!(snapshot.active_memory_bytes, Some(1_073_741_824));
    assert_eq!(snapshot.peak_memory_bytes, Some(2 * 1_073_741_824));
    assert_eq!(
        snapshot.cache_memory_bytes,
        Some((0.5 * 1_073_741_824.0) as u64)
    );
}

#[tokio::test]
async fn test_rapid_mlx_poller_zero_vs_none() {
    let mut server = Server::new_async().await;
    let url = server.url().parse::<url::Url>().expect("Invalid URL");
    let host = url.host_str().expect("No host").to_string();
    let port = url.port().expect("No port");

    let _m = server
        .mock("GET", "/v1/status")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "status": "idle",
                "generation_tps": 0.0,
                "prompt_tps": 0.0
                // active_memory_gb is missing
            })
            .to_string(),
        )
        .create();

    let poller = RapidMlxPoller::new(&host, port, None);
    let snapshot = poller.poll().await.expect("Poll should succeed");

    assert_eq!(snapshot.generation_tokens_per_second, Some(0.0));
    assert_eq!(snapshot.prompt_tokens_per_second, Some(0.0));
    assert_eq!(snapshot.active_memory_bytes, None);
}

#[tokio::test]
async fn test_cache_error_status_degrades_to_absent() {
    let mut server = Server::new_async().await;
    let url = server.url().parse::<url::Url>().expect("Invalid URL");
    let host = url.host_str().expect("No host").to_string();
    let port = url.port().expect("No port");

    let _status = server
        .mock("GET", "/v1/status")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({"status": "idle"}).to_string())
        .create();
    let _cache = server
        .mock("GET", "/v1/cache/stats")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(json!({"error": "unauthorized"}).to_string())
        .create();

    let snapshot = RapidMlxPoller::new(&host, port, None)
        .poll()
        .await
        .expect("required status remains usable");
    assert!(snapshot.cache_metrics.is_none());
}

#[tokio::test]
async fn fixture_status_is_tolerant_authenticated_and_maps_recognized_cache() {
    let mut server = Server::new_async().await;
    let _health = server
        .mock("GET", "/health")
        .match_header("authorization", "Bearer test-secret")
        .with_status(200)
        .with_body(r#"{"status":"ok"}"#)
        .create();
    let _status = server
        .mock("GET", "/v1/status")
        .match_header("authorization", "Bearer test-secret")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(include_str!("fixtures/rapid_mlx/status_idle.json"))
        .create();
    let _cache = server
        .mock("GET", "/v1/cache/stats")
        .match_header("authorization", "Bearer test-secret")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(include_str!("fixtures/rapid_mlx/cache_stats_text.json"))
        .create();

    let snapshot = RapidMlxPoller::from_base_url(server.url(), Some("test-secret"))
        .poll()
        .await
        .expect("fixture should map");
    assert_eq!(snapshot.generation_tokens_per_second, Some(0.0));
    assert_eq!(snapshot.active_memory_bytes, Some(1_073_741_824));
    assert_eq!(snapshot.global_cache_hit_rate, Some(0.8));
    assert_eq!(snapshot.global_cache_entries, Some(4));
    let cache = snapshot.cache_metrics.expect("recognized cache metrics");
    assert_eq!(cache["hit_rate"], 0.8);
    assert!(cache.get("future_release_field").is_none());
}

#[tokio::test]
async fn vision_cache_is_recognized_without_status_cache() {
    let mut server = Server::new_async().await;
    let _status = server
        .mock("GET", "/v1/status")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(include_str!("fixtures/rapid_mlx/status_generating.json"))
        .create();
    let _cache = server
        .mock("GET", "/v1/cache/stats")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(include_str!("fixtures/rapid_mlx/cache_stats_vision.json"))
        .create();

    let snapshot = RapidMlxPoller::from_base_url(server.url(), None)
        .poll()
        .await
        .expect("vision cache should be optional and recognized");
    assert!(snapshot.cache_metrics.is_some());
    assert_eq!(
        snapshot.cache_metrics.as_ref().unwrap()["multimodal_cache_kinds"],
        json!([
            "multimodal_kv_cache",
            "pixel_values_cache",
            "pil_image_cache"
        ])
    );
    assert_eq!(snapshot.running_requests, Some(1));
    assert_eq!(
        snapshot.active_requests,
        Some(vec![json!({"request_id": "req-1", "status": "generating"})])
    );
}

#[tokio::test]
async fn invalid_cache_and_opaque_request_fields_are_not_forwarded() {
    let mut server = Server::new_async().await;
    let _status = server
        .mock("GET", "/v1/status")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "status": "generating",
                "cache": {"hits": "bad", "hit_rate": 4.2, "radix": {"future": true}},
                "requests": [{
                    "request_id": "safe-id",
                    "status": "generating",
                    "prompt": "must not reach the dashboard",
                    "future": {"opaque": true}
                }]
            })
            .to_string(),
        )
        .create();
    let _cache = server
        .mock("GET", "/v1/cache/stats")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({"multimodal_kv_cache": "not-an-object", "unknown": true}).to_string())
        .create();

    let snapshot = RapidMlxPoller::from_base_url(server.url(), None)
        .poll()
        .await
        .expect("invalid optional telemetry should be omitted");
    assert!(snapshot.cache_metrics.is_none());
    assert_eq!(
        snapshot.active_requests,
        Some(vec![
            json!({"request_id": "safe-id", "status": "generating"})
        ])
    );
}

#[tokio::test]
async fn malformed_or_invalid_required_status_is_rejected() {
    for body in [
        r#"{"status":"idle","generation_tps":-1.0}"#,
        r#"{"status":[]}"#,
    ] {
        let mut server = Server::new_async().await;
        let _status = server
            .mock("GET", "/v1/status")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();
        assert!(
            RapidMlxPoller::from_base_url(server.url(), None)
                .poll()
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn missing_or_unknown_status_is_reachable_schema_drift() {
    let mut server = Server::new_async().await;
    let _health = server.mock("GET", "/health").with_status(200).create();
    let _status = server
        .mock("GET", "/v1/status")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"future":"shape"}"#)
        .create();
    let snapshot = RapidMlxPoller::from_base_url(server.url(), None)
        .poll()
        .await
        .expect("unknown optional status should degrade, not fail");
    assert!(snapshot.ready.is_none());
}
