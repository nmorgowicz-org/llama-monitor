use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use std::ffi::OsString;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use crate::state::AppState;

/// Everything the supervisor needs to spawn a process. Secrets are safe in `env`
/// — the supervisor never logs env values. `redacted_summary` is the only thing
/// shown in UI diagnostics and logs.
#[derive(Debug, Clone)]
pub struct SupervisedLaunch {
    pub program:           PathBuf,
    pub args:              Vec<OsString>,
    pub env:               Vec<(OsString, OsString)>,
    pub cwd:               Option<PathBuf>,
#[allow(dead_code)]
pub port:              u16,
#[allow(dead_code)]
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
    pub launch: SupervisedLaunch,
    pub state: Arc<AppState>,
    logs: Mutex<HashMap<u32, Arc<Mutex<Vec<String>>>>>,
    child: Mutex<Option<Child>>,
}

impl Supervisor {
    pub fn new(
        launch: SupervisedLaunch,
        observer: Arc<dyn BackendObserver>,
        state: Arc<AppState>,
    ) -> Self {
        Self { 
            observer,
            launch,
            state,
            logs: Mutex::new(HashMap::new()),
            child: Mutex::new(None),
        }
    }

    pub async fn start(self: Arc<Self>) -> Result<()> {
        let child = self.spawn(self.launch.clone()).await?;
        let pid = child.id().ok_or_else(|| anyhow::anyhow!("Failed to get child PID"))?;
        
        *self.child.lock().unwrap() = Some(child);

        let stdout = self.child.lock().unwrap().as_mut().and_then(|c| c.stdout.take());
        let stderr = self.child.lock().unwrap().as_mut().and_then(|c| c.stderr.take());
        let observer = Arc::clone(&self.observer);
        let state = Arc::clone(&self.state);
        
        let log_buffer = Arc::new(Mutex::new(Vec::new()));
        self.logs.lock().unwrap().insert(pid, Arc::clone(&log_buffer));
        
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
        
        let supervisor = Arc::clone(&self);
        tokio::spawn(async move {
            supervisor.monitor_death_internal(state).await;
        });
        
        Ok(())
    }

    pub async fn spawn(&self, launch: SupervisedLaunch) -> Result<Child> {
        let mut cmd = Command::new(&launch.program);
        cmd.args(&launch.args);
        for (k, v) in launch.env.into_iter() {
            cmd.env(k, v);
        }
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if let Some(cwd) = &launch.cwd {
            cmd.current_dir(cwd);
        }

        Ok(cmd.spawn()?)
    }

    pub async fn monitor_death_internal(
        &self, 
        state: Arc<AppState>,
    ) {
        let pid = {
            let child_lock = self.child.lock().unwrap();
            child_lock.as_ref().and_then(|c| c.id())
        };

        let child = self.child.lock().unwrap().take();
        
        if let Some(mut child) = child {
            match child.wait().await {
                Ok(status) => {
                    if state.server_stopping.load(Ordering::Relaxed) {
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

    pub async fn stop(&self) -> Result<()> {
        let child = {
            let mut lock = self.child.lock().unwrap();
            lock.take()
        };

        if let Some(child) = child {
            let pid = child.id().ok_or_else(|| anyhow::anyhow!("Failed to get child PID"))?;
            
            #[cfg(unix)]
            {
                let _ = tokio::process::Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .status()
                    .await;
            }
            #[cfg(windows)]
            {
                let mut kill_cmd = tokio::process::Command::new("taskkill");
                crate::platform::no_window_tokio(&mut kill_cmd);
                let _ = kill_cmd
                    .args(["/F", "/PID", &pid.to_string()])
                    .status()
                    .await;
            }
            
            return Ok(());
        }
        Ok(())
    }
}
