use llama_monitor::inference::metrics::InferenceMetricsSnapshot;
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

    let poller = RapidMlxPoller::new(&host, port);
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

    let poller = RapidMlxPoller::new(&host, port);
    let snapshot = poller.poll().await.expect("Poll should succeed");

    assert_eq!(snapshot.generation_tokens_per_second, Some(0.0));
    assert_eq!(snapshot.prompt_tokens_per_second, Some(0.0));
    assert_eq!(snapshot.active_memory_bytes, None);
}
