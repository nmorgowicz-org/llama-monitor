use llama_monitor::inference::rapid_mlx::runtime::{RuntimeMetadata, RuntimeSource};
use llama_monitor::inference::rapid_mlx::{RapidMlxAdapter, discovery::Discovery};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use warp::Filter;

#[tokio::test]
async fn test_rapid_mlx_platform_validation() {
    let runtime = RuntimeMetadata {
        executable_path: PathBuf::from("/bin/ls"),
        version: "0.1.0".to_string(),
        source: RuntimeSource::Custom,
    };
    let adapter = RapidMlxAdapter::new(runtime, PathBuf::from("/tmp"));

    let result = adapter.validate().await;

    if std::env::consts::OS == "macos" && std::env::consts::ARCH == "aarch64" {
        assert!(result.is_ok(), "Should be valid on macOS ARM64");
    } else {
        assert!(
            result.is_err(),
            "Should be invalid on other platforms: {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
    }
}

#[tokio::test]
async fn test_rapid_mlx_binary_resolution() {
    let dir = tempdir().unwrap();
    let bin_path = dir.path().join("rapid-mlx");

    // Create a dummy executable file
    let mut file = File::create(&bin_path).unwrap();
    writeln!(file, "#!/bin/sh\necho '0.1.0'").unwrap();

    // On Unix, make it executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    // Test explicit path
    let (resolved, source) = Discovery::resolve_binary(Some(bin_path.as_path()), None)
        .await
        .expect("Should resolve explicit path");
    assert_eq!(resolved, bin_path);
    assert_eq!(source, RuntimeSource::Custom);

    // Test managed path
    let (resolved, source) = Discovery::resolve_binary(None, Some(bin_path.as_path()))
        .await
        .expect("Should resolve managed path");
    assert_eq!(resolved, bin_path);
    assert_eq!(source, RuntimeSource::Managed);
}

#[tokio::test]
async fn test_rapid_mlx_await_ready_success() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let route = warp::path!("health" / "ready")
        .map(|| warp::reply::with_status("OK", warp::http::StatusCode::OK));

    tokio::spawn(warp::serve(route).run(([127, 0, 0, 1], port)));

    let runtime = RuntimeMetadata {
        executable_path: PathBuf::from("/bin/ls"),
        version: "0.1.0".to_string(),
        source: RuntimeSource::Custom,
    };
    let mut adapter = RapidMlxAdapter::new(runtime, PathBuf::from("/tmp"));
    adapter.host = "127.0.0.1".to_string();

    let deadline = Instant::now() + Duration::from_secs(2);
    let result = adapter.await_ready(port, deadline).await;

    assert!(
        result.is_ok(),
        "Should become ready when /health/ready returns 200"
    );
}

#[tokio::test]
async fn test_rapid_mlx_await_ready_failure() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let route = warp::path!("health" / "ready")
        .map(|| warp::reply::with_status("Not Ready", warp::http::StatusCode::NOT_FOUND));

    tokio::spawn(warp::serve(route).run(([127, 0, 0, 1], port)));

    let runtime = RuntimeMetadata {
        executable_path: PathBuf::from("/bin/ls"),
        version: "0.1.0".to_string(),
        source: RuntimeSource::Custom,
    };
    let mut adapter = RapidMlxAdapter::new(runtime, PathBuf::from("/tmp"));
    adapter.host = "127.0.0.1".to_string();

    let deadline = Instant::now() + Duration::from_millis(500);
    let result = adapter.await_ready(port, deadline).await;

    assert!(
        result.is_err(),
        "Should fail when /health/ready does not return 200"
    );
}
