use std::path::PathBuf;
use std::os::unix::ffi::OsString;
use std::time::Instant;
use std::process::ExitStatus;

/// Everything the supervisor needs to spawn a process. 
pub struct SupervisedLaunch {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
    pub cwd: Option<PathBuf>,
    pub port: u16,
    pub redacted_summary: String,
}

/// Callbacks the supervisor fires back into the backend adapter.
pub trait BackendObserver: Send + Sync + 'static {
    /// Called for every stdout/stderr line from the child process.
    fn on_log_line(&self, line: &str);
    /// Called on unexpected exit (not triggered by stop()).
    fn on_crash(&self, exit_status: ExitStatus, tail: Vec<String>);
}
