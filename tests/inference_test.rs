use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;
use llama_monitor::inference::supervisor::{Supervisor, SupervisedLaunch, BackendObserver};
use llama_monitor::state::AppState;
use std::path::PathBuf;


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

#[tokio::test]
async fn test_supervisor_spawn_and_log() {
    let observer = Arc::new(MockObserver::new());
    let state = Arc::new(AppState::default());
    let launch = SupervisedLaunch {
        program: PathBuf::from("sh"),
        args: vec!["-c".into(), "echo 'hello world'".into()],
        env: vec![],
        cwd: None,
        port: 8000,
        redacted_summary: "test".to_string(),
    };
    let supervisor = Arc::new(Supervisor::new(launch.clone(), observer.clone(), state));
    
    supervisor.clone().start().await.expect("Failed to start supervisor");
    
    // Give it a moment to run and capture output
    sleep(Duration::from_millis(500)).await;
    
    let logs = observer.logs.lock().unwrap();
    assert!(logs.contains(&"hello world".to_string()));
    
    supervisor.stop().await.expect("Failed to stop supervisor");
}

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
    let supervisor = Arc::new(Supervisor::new(launch.clone(), observer.clone(), state));
    
    supervisor.clone().start().await.expect("Failed to start supervisor");
    
    // Wait for process to exit and monitor to fire
    sleep(Duration::from_millis(500)).await;
    
    let crash = observer.crash.lock().unwrap();
    assert!(crash.is_some());
    let (status, tail) = crash.as_ref().unwrap();
    assert!(!status.success());
    assert!(tail.contains(&"crashing...".to_string()));
}

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
    let supervisor = Arc::new(Supervisor::new(launch.clone(), observer.clone(), state.clone()));
    
    supervisor.clone().start().await.expect("Failed to start supervisor");
    
    // Simulate an intentional stop
    state.server_stopping.store(true, std::sync::atomic::Ordering::Relaxed);
    supervisor.stop().await.expect("Failed to stop supervisor");
    
    // Wait for monitor to finish
    sleep(Duration::from_millis(500)).await;
    
    let crash = observer.crash.lock().unwrap();
    assert!(crash.is_none(), "Crash should not be recorded for intentional stop");
}

