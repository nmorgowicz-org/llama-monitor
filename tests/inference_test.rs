use llama_monitor::inference::llama_cpp::ServerConfig;
use llama_monitor::inference::supervisor::{BackendObserver, SupervisedLaunch, Supervisor};
use llama_monitor::state::AppState;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

struct MockObserver {
    logs: Mutex<Vec<String>>,
    crash: Mutex<Option<(std::process::ExitStatus, Vec<String>)>>,
}

impl MockObserver {
    fn new() -> Self {
        Self {
            logs: Mutex::new(Vec::new()),
            crash: Mutex::new(None),
        }
    }
}

impl BackendObserver for MockObserver {
    fn on_log_line(&self, line: &str) {
        self.logs.lock().unwrap().push(line.to_string());
    }
    fn on_crash(&self, exit_status: std::process::ExitStatus, tail: Vec<String>) {
        self.crash.lock().unwrap().replace((exit_status, tail));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn test_supervisor_spawn_and_log() {
    let observer = Arc::new(MockObserver::new());
    let state = Arc::new(AppState::default());
    let launch = SupervisedLaunch {
        program: PathBuf::from("sh"),
        args: vec![
            "-c".into(),
            "printf '%s\\n' \"$SUPERVISOR_TEST_ENV\"".into(),
        ],
        env: vec![("SUPERVISOR_TEST_ENV".into(), "hello world".into())],
        cwd: None,
        port: 8000,
        redacted_summary: "test".to_string(),
    };
    let supervisor = Arc::new(Supervisor::new(launch.clone(), observer.clone(), state, 0));

    supervisor
        .clone()
        .start()
        .await
        .expect("Failed to start supervisor");

    // Give it a moment to run and capture output
    sleep(Duration::from_millis(500)).await;

    let logs = observer.logs.lock().unwrap();
    assert!(logs.contains(&"hello world".to_string()));

    supervisor.stop().await.expect("Failed to stop supervisor");
}

#[cfg(unix)]
#[tokio::test]
async fn test_supervisor_crash_detection() {
    let observer = Arc::new(MockObserver::new());
    let state = Arc::new(AppState::default());
    let launch = SupervisedLaunch {
        program: PathBuf::from("sh"),
        args: vec!["-c".into(), "echo 'crashing...'; exit 1".into()],
        env: vec![],
        cwd: None,
        port: 8001,
        redacted_summary: "test crash".to_string(),
    };
    let supervisor = Arc::new(Supervisor::new(launch.clone(), observer.clone(), state, 0));

    *supervisor.state.server_running.lock().unwrap() = true;
    *supervisor.state.local_server_running.lock().unwrap() = true;
    *supervisor.state.server_config.lock().unwrap() = Some(ServerConfig::default());

    supervisor
        .clone()
        .start()
        .await
        .expect("Failed to start supervisor");

    // Wait for process to exit and monitor to fire
    sleep(Duration::from_millis(500)).await;

    let crash = observer.crash.lock().unwrap();
    assert!(crash.is_some());
    let (status, tail) = crash.as_ref().unwrap();
    assert!(!status.success());
    assert!(tail.contains(&"crashing...".to_string()));
    assert!(!*supervisor.state.server_running.lock().unwrap());
    assert!(!*supervisor.state.local_server_running.lock().unwrap());
    assert!(supervisor.state.server_config.lock().unwrap().is_none());
    assert!(supervisor.state.server_child.lock().await.is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn test_wait_for_exit_observes_completed_crash_cleanup() {
    let observer = Arc::new(MockObserver::new());
    let state = Arc::new(AppState::default());
    state
        .server_generation
        .store(9, std::sync::atomic::Ordering::Release);
    *state.server_running.lock().unwrap() = true;
    *state.local_server_running.lock().unwrap() = true;
    *state.server_config.lock().unwrap() = Some(ServerConfig::default());
    let launch = SupervisedLaunch {
        program: PathBuf::from("sh"),
        args: vec!["-c".into(), "echo 'early failure'; exit 7".into()],
        env: vec![],
        cwd: None,
        port: 8004,
        redacted_summary: "early exit ordering".to_string(),
    };
    let supervisor = Arc::new(Supervisor::new(launch, observer.clone(), state.clone(), 9));

    supervisor.clone().start().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), supervisor.wait_for_exit())
        .await
        .expect("early exit should be observed");

    let crash = observer.crash.lock().unwrap();
    let (status, tail) = crash
        .as_ref()
        .expect("crash callback must finish before wait_for_exit returns");
    assert_eq!(status.code(), Some(7));
    assert!(tail.contains(&"early failure".to_string()));
    drop(crash);
    assert!(state.server_child.lock().await.is_none());
    assert!(!*state.server_running.lock().unwrap());
    assert!(!*state.local_server_running.lock().unwrap());
}

#[cfg(unix)]
#[tokio::test]
async fn test_supervisor_intentional_stop() {
    let observer = Arc::new(MockObserver::new());
    let state = Arc::new(AppState::default());
    let launch = SupervisedLaunch {
        program: PathBuf::from("sh"),
        args: vec!["-c".into(), "sleep 10".into()],
        env: vec![],
        cwd: None,
        port: 8002,
        redacted_summary: "test stop".to_string(),
    };
    let supervisor = Arc::new(Supervisor::new(
        launch.clone(),
        observer.clone(),
        state.clone(),
        0,
    ));

    supervisor
        .clone()
        .start()
        .await
        .expect("Failed to start supervisor");

    // Simulate an intentional stop
    state
        .server_stopping
        .store(true, std::sync::atomic::Ordering::Relaxed);
    supervisor.stop().await.expect("Failed to stop supervisor");

    // Wait for monitor to finish
    sleep(Duration::from_millis(500)).await;

    let crash = observer.crash.lock().unwrap();
    assert!(
        crash.is_none(),
        "Crash should not be recorded for intentional stop"
    );
    drop(crash);
    assert!(supervisor.state.server_child.lock().await.is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn test_stop_cancels_process_while_readiness_is_pending() {
    let observer = Arc::new(MockObserver::new());
    let state = Arc::new(AppState::default());
    state
        .server_generation
        .store(11, std::sync::atomic::Ordering::Release);
    let launch = SupervisedLaunch {
        program: PathBuf::from("sh"),
        args: vec!["-c".into(), "sleep 30".into()],
        env: vec![],
        cwd: None,
        port: 8003,
        redacted_summary: "pending readiness".to_string(),
    };
    let supervisor = Arc::new(Supervisor::new(launch, observer.clone(), state.clone(), 11));
    let pid = supervisor.clone().start().await.unwrap();

    let readiness_waiter = {
        let supervisor = supervisor.clone();
        tokio::spawn(async move { supervisor.wait_for_exit().await })
    };
    tokio::time::timeout(Duration::from_secs(2), supervisor.stop())
        .await
        .expect("stop should not wait for a readiness deadline")
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), readiness_waiter)
        .await
        .expect("pending readiness should observe process exit")
        .unwrap();

    let alive = tokio::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .await
        .unwrap()
        .success();
    assert!(!alive, "stopped process should no longer exist");
    assert!(observer.crash.lock().unwrap().is_none());
}
