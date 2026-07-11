use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;
use llama_monitor::inference::supervisor::{Supervisor, SupervisedLaunch, BackendObserver};
use std::ffi::OsString;
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
    let supervisor = Supervisor::new(observer.clone());

    let launch = SupervisedLaunch {
        program: PathBuf::from("sh"),
        args: vec!["-c".into(), "echo 'hello world'".into()],
        env: vec![],
        cwd: None,
        port: 8000,
        redacted_summary: "test".to_string(),
    };

    let mut child = supervisor.spawn(launch).await.expect("Failed to spawn");
    
    // Give it a moment to run and capture output
    sleep(Duration::from_millis(200)).await;
    
    let logs = observer.logs.lock().unwrap();
    assert!(logs.contains(&"hello world".to_string()));
    
    let _ = child.kill().await;
}

#[tokio::test]
async fn test_supervisor_crash_detection() {
    let observer = Arc::new(MockObserver::new());
    let supervisor = Supervisor::new(observer.clone());

    let launch = SupervisedLaunch {
        program: PathBuf::from("sh"),
        args: vec!["-c".into(), "echo 'crashing...'; exit 1".into()],
        env: vec![],
        cwd: None,
        port: 8001,
        redacted_summary: "test crash".to_string(),
    };

    let mut child = supervisor.spawn(launch).await.expect("Failed to spawn");
    
    // Monitor the process. 
    // is_intentional returns false because we want to detect the crash.
    supervisor.monitor_death(
        child, 
        Arc::new(|| false), 
    ).await;
    
    let crash = observer.crash.lock().unwrap();
    assert!(crash.is_some());
    let (status, tail) = crash.as_ref().unwrap();
    assert!(!status.success());
    assert!(tail.contains(&"crashing...".to_string()));
}

#[tokio::test]
async fn test_supervisor_intentional_stop() {
    let observer = Arc::new(MockObserver::new());
    let supervisor = Supervisor::new(observer.clone());

    let launch = SupervisedLaunch {
        program: PathBuf::from("sh"),
        args: vec!["-c".into(), "sleep 10".into()],
        env: vec![],
        cwd: None,
        port: 8002,
        redacted_summary: "test stop".to_string(),
    };

    let mut child = supervisor.spawn(launch).await.expect("Failed to spawn");
    
    // We simulate an intentional stop by killing the child and setting is_intentional to true.
    let _ = child.kill().await;
    
    supervisor.monitor_death(
        child, 
        Arc::new(|| true), 
    ).await;
    
    let crash = observer.crash.lock().unwrap();
    assert!(crash.is_none(), "Crash should not be recorded for intentional stop");
}
