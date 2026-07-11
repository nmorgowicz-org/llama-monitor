use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::ffi::OsString;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

/// Everything the supervisor needs to spawn a process. Secrets are safe in `env`
/// — the supervisor never logs env values. `redacted_summary` is the only thing
/// shown in UI diagnostics and logs.
#[derive(Debug, Clone)]
pub struct SupervisedLaunch {
    pub program:           PathBuf,
    pub args:              Vec<OsString>,
    pub env:               Vec<(OsString, OsString)>,
    pub cwd:               Option<PathBuf>,
    pub port:              u16,
    pub redacted_summary:  String,
}

/// Callbacks the supervisor fires back into the backend adapter.
pub trait BackendObserver: Send + Sync + 'static {
    /// Called for every stdout/stderr line from the child process.
    fn on_log_line(&self, line: &str);
    /// Called on unexpected exit (not triggered by stop()).
    fn on_crash(&self, exit_status: std::process::ExitStatus, tail: Vec<String>);
}

pub struct Supervisor {
    pub observer: Arc<dyn BackendObserver>,
    logs: Mutex<HashMap<u32, Arc<Mutex<Vec<String>>>>>,
}

impl Supervisor {
    pub fn new(observer: Arc<dyn BackendObserver>) -> Self {
        Self { 
            observer,
            logs: Mutex::new(HashMap::new()),
        }
    }

    /// Spawns the process and returns the child handle.
    pub async fn spawn(&self, launch: SupervisedLaunch) -> Result<Child> {
        let mut cmd = Command::new(&launch.program);
        cmd.args(&launch.args);
        cmd.envs(launch.env);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if let Some(cwd) = &launch.cwd {
            cmd.current_dir(cwd);
        }

        let mut child = cmd.spawn()?;
        let pid = child.id().ok_or_else(|| anyhow::anyhow!("Failed to get child PID"))?;
        
        let log_buffer = Arc::new(Mutex::new(Vec::new()));
        self.logs.lock().unwrap().insert(pid, Arc::clone(&log_buffer));
        
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let observer = Arc::clone(&self.observer);

        if let Some(out) = stdout {
            let obs = Arc::clone(&observer);
            let buf = Arc::clone(&log_buffer);
            tokio::spawn(async move {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    buf.lock().unwrap().push(line.clone());
                    obs.on_log_line(&line);
                }
            });
        }

        if let Some(err) = stderr {
            let obs = Arc::clone(&observer);
            let buf = Arc::clone(&log_buffer);
            tokio::spawn(async move {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    buf.lock().unwrap().push(line.clone());
                    obs.on_log_line(&line);
                }
            });
        }

        Ok(child)
    }

    /// Monitors a child process for unexpected exit.
    /// `is_intentional` is a closure that returns true if the process is being stopped.
    pub async fn monitor_death(
        &self, 
        mut child: Child, 
        is_intentional: Arc<dyn Fn() -> bool + Send + Sync>,
    ) {
        let pid = child.id();
        match child.wait().await {
            Ok(status) => {
                if is_intentional() {
                    if let Some(p) = pid {
                        self.logs.lock().unwrap().remove(&p);
                    }
                    return;
                }
                
                let tail = if let Some(p) = pid {
                    let buffer = self.logs.lock().unwrap().remove(&p);
                    buffer.map(|b| {
                        b.lock().unwrap().iter()
                            .filter(|l| !l.starts_with("[monitor]"))
                            .cloned()
                            .collect()
                    }).unwrap_or_default()
                } else {
                    Vec::new()
                };
                
                self.observer.on_crash(status, tail);
            }
            Err(_) => {
                if let Some(p) = pid {
                    self.logs.lock().unwrap().remove(&p);
                }
            }
        }
    }
}
