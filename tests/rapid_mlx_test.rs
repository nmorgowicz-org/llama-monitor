use llama_monitor::inference::rapid_mlx::poller::RapidMlxPoller;
use llama_monitor::inference::rapid_mlx::runtime::{RuntimeMetadata, RuntimeSource};
use llama_monitor::inference::rapid_mlx::{RapidMlxAdapter, discovery::Discovery};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use warp::Filter;

use llama_monitor::inference::rapid_mlx::compatibility;
#[cfg(all(unix, target_os = "macos", target_arch = "aarch64"))]
use llama_monitor::inference::supervisor::{BackendObserver, Supervisor};
#[cfg(all(unix, target_os = "macos", target_arch = "aarch64"))]
use llama_monitor::state::AppState;
#[cfg(all(unix, target_os = "macos", target_arch = "aarch64"))]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[cfg(all(unix, target_os = "macos", target_arch = "aarch64"))]
use std::sync::{Arc, Mutex};

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

#[test]
fn test_rapid_mlx_discovery_source_classification() {
    assert_eq!(
        Discovery::classify_source(std::path::Path::new(
            "/opt/homebrew/Cellar/rapid-mlx/0.10.9/bin/rapid-mlx"
        )),
        RuntimeSource::Homebrew
    );
    assert_eq!(
        Discovery::classify_source(std::path::Path::new(
            "/Users/test/.local/pipx/venvs/rapid-mlx/bin/rapid-mlx"
        )),
        RuntimeSource::Pipx
    );
    assert_eq!(
        Discovery::classify_source(std::path::Path::new(
            "/Users/test/project/.venv/bin/rapid-mlx"
        )),
        RuntimeSource::Pip
    );
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

#[tokio::test]
async fn test_rapid_mlx_status_uses_api_key() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let status = warp::path!("v1" / "status")
        .and(warp::header::optional::<String>("authorization"))
        .map(|authorization: Option<String>| {
            if authorization.as_deref() == Some("Bearer status-secret") {
                warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "status": "idle",
                        "model": "fixture",
                        "generation_tps": 0.0,
                        "prompt_tps": 0.0
                    })),
                    warp::http::StatusCode::OK,
                )
            } else {
                warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({"error": "unauthorized"})),
                    warp::http::StatusCode::UNAUTHORIZED,
                )
            }
        });
    let cache = warp::path!("v1" / "cache" / "stats")
        .map(|| warp::reply::json(&serde_json::json!({"enabled": false})));
    tokio::spawn(warp::serve(status.or(cache)).run(([127, 0, 0, 1], port)));
    tokio::time::sleep(Duration::from_millis(25)).await;

    let snapshot = RapidMlxPoller::new("127.0.0.1", port, Some("status-secret"))
        .poll()
        .await
        .unwrap();
    assert_eq!(snapshot.model.as_deref(), Some("fixture"));
    assert_eq!(snapshot.generation_tokens_per_second, Some(0.0));

    let wrong_key = RapidMlxPoller::new("127.0.0.1", port, Some("wrong-secret"))
        .poll()
        .await
        .unwrap_err();
    assert!(format!("{wrong_key:#}").contains("401"));

    let no_key = RapidMlxPoller::new("127.0.0.1", port, None)
        .poll()
        .await
        .unwrap_err();
    assert!(format!("{no_key:#}").contains("401"));
}

#[tokio::test]
async fn rapid_native_cancellation_degrades_without_a_public_request_id_contract() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let cancel = warp::path!("v1" / "requests" / String / "cancel")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .map({
            let requests = requests.clone();
            move |request_id: String, authorization: Option<String>| {
                requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if request_id == "chatcmpl-fixture"
                    && authorization.as_deref() == Some("Bearer cancel-secret")
                {
                    warp::reply::with_status("cancelled", warp::http::StatusCode::OK)
                } else {
                    warp::reply::with_status("unauthorized", warp::http::StatusCode::UNAUTHORIZED)
                }
            }
        });
    tokio::spawn(warp::serve(cancel).run(([127, 0, 0, 1], port)));
    tokio::time::sleep(Duration::from_millis(25)).await;

    let runtime = RuntimeMetadata {
        executable_path: "rapid-mlx".into(),
        version: "0.10.9".into(),
        source: RuntimeSource::Managed,
    };
    let mut supported = RapidMlxAdapter::new(runtime, "model".into());
    supported.host = "127.0.0.1".into();
    supported.configure_runtime(
        compatibility::CompatibilityProfile::verified_baseline(),
        Some("cancel-secret".into()),
    );
    assert!(
        supported
            .cancel_request(port, "chatcmpl-fixture")
            .await
            .unwrap_err()
            .to_string()
            .contains("unavailable")
    );
    assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert!(
        supported
            .cancel_request(port, "../escape")
            .await
            .unwrap_err()
            .to_string()
            .contains("invalid request ID")
    );
    assert!(
        supported
            .cancel_request(port, &"x".repeat(129))
            .await
            .unwrap_err()
            .to_string()
            .contains("invalid request ID")
    );

    supported.configure_runtime(
        compatibility::CompatibilityProfile {
            state: compatibility::CompatibilityState::Provisional,
            version: "0.10.9+nightly".into(),
            capabilities: compatibility::ServeCapabilities::verified_baseline(),
        },
        Some("cancel-secret".into()),
    );
    assert!(
        supported
            .cancel_request(port, "chatcmpl-fixture")
            .await
            .unwrap_err()
            .to_string()
            .contains("unavailable")
    );
}

#[cfg(all(unix, target_os = "macos", target_arch = "aarch64"))]
struct FixtureObserver {
    crash: Mutex<Option<(std::process::ExitStatus, Vec<String>)>>,
}

#[cfg(all(unix, target_os = "macos", target_arch = "aarch64"))]
impl BackendObserver for FixtureObserver {
    fn on_log_line(&self, _line: &str) {}

    fn on_crash(&self, status: std::process::ExitStatus, tail: Vec<String>) {
        self.crash.lock().unwrap().replace((status, tail));
    }
}

#[cfg(all(unix, target_os = "macos", target_arch = "aarch64"))]
fn fixture_binary(script_body: &str) -> (tempfile::TempDir, PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("rapid-mlx");
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'rapid-mlx 0.10.9'; exit 0; fi\nif [ \"$1\" = \"serve\" ] && [ \"$2\" = \"--help\" ]; then echo '--host --port --log-level --served-model-name --timeout --max-cache-blocks'; exit 0; fi\n{script_body}\n"
    );
    std::fs::write(&binary, script).unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    (dir, binary)
}

#[cfg(all(unix, target_os = "macos", target_arch = "aarch64"))]
#[tokio::test]
async fn fixture_lifecycle_discovers_loads_becomes_ready_and_stops() {
    let (_runtime_dir, binary) = fixture_binary(
        "echo loading; trap 'printf stopped > \"$STOP_MARKER\"; exit 0' TERM; while :; do sleep 0.05; done",
    );
    let (resolved, source) = Discovery::resolve_binary(Some(&binary), None)
        .await
        .unwrap();
    let profile = compatibility::probe(&resolved, source).await.unwrap();

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let ready = Arc::new(AtomicBool::new(false));
    let attempts = Arc::new(AtomicUsize::new(0));
    let route = warp::path!("health" / "ready").map({
        let ready = ready.clone();
        let attempts = attempts.clone();
        move || {
            attempts.fetch_add(1, Ordering::Relaxed);
            if ready.load(Ordering::Acquire) {
                warp::reply::with_status("ready", warp::http::StatusCode::OK)
            } else {
                warp::reply::with_status("loading", warp::http::StatusCode::SERVICE_UNAVAILABLE)
            }
        }
    });
    tokio::spawn(warp::serve(route).run(([127, 0, 0, 1], port)));
    tokio::time::sleep(Duration::from_millis(25)).await;

    let mut adapter = RapidMlxAdapter::new(
        RuntimeMetadata {
            executable_path: resolved,
            version: profile.version.clone(),
            source,
        },
        PathBuf::from("fixture-model"),
    );
    adapter.host = "127.0.0.1".into();
    adapter.port = port;
    adapter.configure_runtime(profile, None);
    adapter.validate().await.unwrap();
    let mut launch = adapter.build_launch().await.unwrap();
    let stop_marker = _runtime_dir.path().join("stopped");
    launch
        .env
        .push(("STOP_MARKER".into(), stop_marker.as_os_str().to_owned()));
    let observer = Arc::new(FixtureObserver {
        crash: Mutex::new(None),
    });
    let state = Arc::new(AppState::default());
    let supervisor = Arc::new(Supervisor::new(launch, observer.clone(), state, 0));
    supervisor.clone().start().await.unwrap();

    let readiness = adapter.await_ready(port, Instant::now() + Duration::from_secs(3));
    tokio::pin!(readiness);
    assert!(
        tokio::time::timeout(Duration::from_millis(150), readiness.as_mut())
            .await
            .is_err(),
        "loading response must not be accepted as ready"
    );
    assert!(attempts.load(Ordering::Acquire) > 0);
    ready.store(true, Ordering::Release);
    readiness.await.unwrap();
    supervisor.stop().await.unwrap();

    assert_eq!(std::fs::read_to_string(stop_marker).unwrap(), "stopped");
    assert!(observer.crash.lock().unwrap().is_none());
}

#[cfg(all(unix, target_os = "macos", target_arch = "aarch64"))]
#[tokio::test]
async fn fixture_early_exit_propagates_actionable_tail() {
    let (_runtime_dir, binary) =
        fixture_binary("echo 'fixture model load failed: unsupported architecture' >&2; exit 23");
    let (resolved, source) = Discovery::resolve_binary(Some(&binary), None)
        .await
        .unwrap();
    let profile = compatibility::probe(&resolved, source).await.unwrap();
    let mut adapter = RapidMlxAdapter::new(
        RuntimeMetadata {
            executable_path: resolved,
            version: profile.version.clone(),
            source,
        },
        PathBuf::from("fixture-model"),
    );
    adapter.configure_runtime(profile, None);
    let observer = Arc::new(FixtureObserver {
        crash: Mutex::new(None),
    });
    let state = Arc::new(AppState::default());
    let supervisor = Arc::new(Supervisor::new(
        adapter.build_launch().await.unwrap(),
        observer.clone(),
        state,
        0,
    ));
    supervisor.clone().start().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), supervisor.wait_for_exit())
        .await
        .unwrap();

    let crash = observer.crash.lock().unwrap();
    let (status, tail) = crash.as_ref().expect("fixture crash should propagate");
    assert_eq!(status.code(), Some(23));
    assert!(
        tail.iter()
            .any(|line| line.contains("unsupported architecture"))
    );
}
