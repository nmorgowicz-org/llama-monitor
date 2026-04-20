use std::collections::{BTreeMap, VecDeque};
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::gpu::GpuMetrics;
use crate::gpu::env::GpuEnv;
use crate::llama::metrics::LlamaMetrics;
use crate::llama::server::ServerConfig;
use crate::models::DiscoveredModel;
use crate::presets::ModelPreset;
use crate::system::SystemMetrics;

const MAX_LOG_LINES: usize = 500;
const MAX_SESSIONS: usize = 10;

/// Persisted UI control-bar settings (survives page reload).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UiSettings {
    #[serde(default)]
    pub preset_id: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub llama_server_path: String,
    #[serde(default)]
    pub llama_server_cwd: String,
    #[serde(default)]
    pub models_dir: String,
    #[serde(default)]
    pub server_endpoint: String,
    #[serde(default = "default_llama_poll_interval")]
    pub llama_poll_interval: u64,
}

/// Session mode: either spawn a new server or attach to existing
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SessionMode {
    Spawn { port: u16 },
    Attach { endpoint: String },
}

impl Default for SessionMode {
    fn default() -> Self {
        SessionMode::Spawn { port: 8001 }
    }
}

/// A server session (active or inactive)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub mode: SessionMode,
    pub status: SessionStatus,
    pub last_active: u64,
}

impl Session {
    pub fn new_spawn(id: String, name: String, port: u16) -> Self {
        Self {
            id,
            name,
            mode: SessionMode::Spawn { port },
            status: SessionStatus::Stopped,
            last_active: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    pub fn new_attach(id: String, name: String, endpoint: String) -> Self {
        Self {
            id,
            name,
            mode: SessionMode::Attach { endpoint },
            status: SessionStatus::Disconnected,
            last_active: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

/// Session status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Default)]
pub enum SessionStatus {
    #[default]
    Stopped,
    Running,
    Disconnected,
    Error(String),
}

fn default_port() -> u16 {
    8001
}

fn default_llama_poll_interval() -> u64 {
    1
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            preset_id: String::new(),
            port: 8001,
            llama_server_path: String::new(),
            llama_server_cwd: String::new(),
            models_dir: String::new(),
            server_endpoint: String::new(),
            llama_poll_interval: default_llama_poll_interval(),
        }
    }
}

pub fn load_ui_settings(path: &Path) -> UiSettings {
    if path.exists()
        && let Ok(contents) = std::fs::read_to_string(path)
        && let Ok(s) = serde_json::from_str::<UiSettings>(&contents)
    {
        return s;
    }
    UiSettings::default()
}

pub fn save_ui_settings(path: &Path, settings: &UiSettings) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(settings)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Load sessions from file
pub fn load_sessions(path: &Path) -> Vec<Session> {
    if path.exists()
        && let Ok(contents) = std::fs::read_to_string(path)
        && let Ok(sessions) = serde_json::from_str::<Vec<Session>>(&contents)
    {
        return sessions;
    }
    // Return default session if no file exists
    vec![Session::new_spawn(
        "default".to_string(),
        "Default Session".to_string(),
        8001,
    )]
}

/// Save sessions to file
pub fn save_sessions(path: &Path, sessions: &[Session]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(&sessions)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Paths for app state
#[derive(Clone)]
pub struct AppPaths {
    pub presets_path: PathBuf,
    pub models_dir: Option<PathBuf>,
    pub gpu_env_path: PathBuf,
    pub ui_settings_path: PathBuf,
    pub sessions_path: PathBuf,
}

/// Generate unique session ID
pub fn generate_session_id() -> String {
    format!(
        "session_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    )
}

#[derive(Clone)]
pub struct AppState {
    pub gpu_metrics: Arc<Mutex<BTreeMap<String, GpuMetrics>>>,
    pub llama_metrics: Arc<Mutex<LlamaMetrics>>,
    pub server_logs: Arc<Mutex<VecDeque<String>>>,
    pub server_child: Arc<tokio::sync::Mutex<Option<tokio::process::Child>>>,
    pub server_running: Arc<Mutex<bool>>,
    pub server_config: Arc<Mutex<Option<ServerConfig>>>,
    pub llama_poll_notify: Arc<tokio::sync::Notify>,
    pub presets: Arc<Mutex<Vec<ModelPreset>>>,
    pub presets_path: PathBuf,
    pub discovered_models: Arc<Mutex<Vec<DiscoveredModel>>>,
    pub models_dir: Option<PathBuf>,
    pub gpu_env: Arc<Mutex<GpuEnv>>,
    pub gpu_env_path: PathBuf,
    pub ui_settings: Arc<Mutex<UiSettings>>,
    pub ui_settings_path: PathBuf,
    pub system_metrics: Arc<Mutex<SystemMetrics>>,
    pub sessions: Arc<Mutex<Vec<Session>>>,
    pub active_session_id: Arc<Mutex<String>>,
    #[expect(dead_code)]
    pub sessions_path: PathBuf,
}

impl AppState {
    pub fn new(
        presets: Vec<ModelPreset>,
        paths: AppPaths,
        gpu_env: GpuEnv,
        ui_settings: UiSettings,
    ) -> Self {
        let presets_path = paths.presets_path;
        let models_dir = paths.models_dir;
        let gpu_env_path = paths.gpu_env_path;
        let ui_settings_path = paths.ui_settings_path;
        let sessions_path = paths.sessions_path;
        let discovered = models_dir
            .as_ref()
            .and_then(|dir| crate::models::scan_models_dir(dir).ok())
            .unwrap_or_default();

        Self {
            gpu_metrics: Arc::new(Mutex::new(BTreeMap::new())),
            llama_metrics: Arc::new(Mutex::new(LlamaMetrics::default())),
            server_logs: Arc::new(Mutex::new(VecDeque::new())),
            server_child: Arc::new(tokio::sync::Mutex::new(None)),
            server_running: Arc::new(Mutex::new(false)),
            server_config: Arc::new(Mutex::new(None)),
            llama_poll_notify: Arc::new(tokio::sync::Notify::new()),
            presets: Arc::new(Mutex::new(presets)),
            presets_path,
            discovered_models: Arc::new(Mutex::new(discovered)),
            models_dir,
            gpu_env: Arc::new(Mutex::new(gpu_env)),
            gpu_env_path,
            ui_settings: Arc::new(Mutex::new(ui_settings)),
            ui_settings_path,
            system_metrics: Arc::new(Mutex::new(SystemMetrics {
                cpu_name: "Unknown CPU".to_string(),
                cpu_temp: 0.0,
                cpu_temp_available: false,
                cpu_load: 0,
                cpu_clock_mhz: 0,
                ram_total_gb: 0.0,
                ram_used_gb: 0.0,
                motherboard: "Unknown".to_string(),
            })),
            sessions: Arc::new(Mutex::new(Vec::new())),
            active_session_id: Arc::new(Mutex::new("".to_string())),
            sessions_path,
        }
    }

    pub fn push_log(&self, line: String) {
        let mut logs = self.server_logs.lock().unwrap();
        if logs.len() >= MAX_LOG_LINES {
            logs.pop_front();
        }
        logs.push_back(line);
    }

    pub fn get_sessions(&self) -> Vec<Session> {
        self.sessions.lock().unwrap().clone()
    }

    #[expect(dead_code)]
    pub fn get_active_session(&self) -> Option<Session> {
        let sessions = self.sessions.lock().unwrap();
        let active_id = self.active_session_id.lock().unwrap();
        sessions.iter().find(|s| s.id == *active_id).cloned()
    }

    pub fn set_active_session(&self, session_id: &str) -> bool {
        let mut active = self.active_session_id.lock().unwrap();
        let sessions = self.sessions.lock().unwrap();
        let exists = sessions.iter().any(|s| s.id == session_id);
        if exists {
            *active = session_id.to_string();
        }
        exists
    }

    pub fn add_session(&self, session: Session) -> bool {
        let mut sessions = match self.sessions.lock() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] Failed to acquire sessions lock: {e}");
                return false;
            }
        };
        if sessions.len() >= MAX_SESSIONS {
            return false;
        }
        let now = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_secs(),
            Err(_) => {
                eprintln!("[error] Failed to get system time");
                0
            }
        };
        let mut new_session = session;
        new_session.last_active = now;
        sessions.push(new_session);
        true
    }

    pub fn remove_session(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.lock().unwrap();
        let len_before = sessions.len();
        sessions.retain(|s| s.id != session_id);
        // Also update active session if it was removed
        let mut active = self.active_session_id.lock().unwrap();
        if *active == session_id && !sessions.is_empty() {
            *active = sessions[0].id.clone();
        }
        sessions.len() < len_before
    }

    pub fn update_session_status(&self, session_id: &str, status: SessionStatus) -> bool {
        let mut sessions = self.sessions.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        for session in sessions.iter_mut() {
            if session.id == session_id {
                session.status = status;
                session.last_active = now;
                return true;
            }
        }
        false
    }

    pub fn active_session_uses_local_metrics(&self) -> bool {
        let active_id = self.active_session_id.lock().unwrap().clone();
        if active_id.is_empty() {
            return true;
        }

        let session = {
            let sessions = self.sessions.lock().unwrap();
            sessions.iter().find(|s| s.id == active_id).cloned()
        };

        match session.map(|s| s.mode) {
            Some(SessionMode::Spawn { .. }) => true,
            Some(SessionMode::Attach { endpoint }) => endpoint_is_local(&endpoint),
            None => true,
        }
    }
}

fn endpoint_is_local(endpoint: &str) -> bool {
    let url = reqwest::Url::parse(endpoint)
        .or_else(|_| reqwest::Url::parse(&format!("http://{endpoint}")));
    let Ok(url) = url else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };

    host_is_local(host)
}

fn host_is_local(host: &str) -> bool {
    let host = host.trim_matches(['[', ']']);
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }

    let Ok(ip) = host.parse::<IpAddr>() else {
        return false;
    };

    if ip.is_loopback() {
        return true;
    }

    local_interface_ips().contains(&ip)
}

fn local_interface_ips() -> Vec<IpAddr> {
    let mut ips = Vec::new();

    let probes = [
        ("0.0.0.0:0", "8.8.8.8:80"),
        ("[::]:0", "[2001:4860:4860::8888]:80"),
    ];

    for (bind_addr, connect_addr) in probes {
        let Ok(socket) = UdpSocket::bind(bind_addr) else {
            continue;
        };
        let Ok(remote) = connect_addr.parse::<SocketAddr>() else {
            continue;
        };
        if socket.connect(remote).is_ok()
            && let Ok(local_addr) = socket.local_addr()
        {
            ips.push(local_addr.ip());
        }
    }

    ips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_locality_detects_loopback_hosts() {
        assert!(endpoint_is_local("http://localhost:8001"));
        assert!(endpoint_is_local("127.0.0.1:8001"));
        assert!(endpoint_is_local("http://[::1]:8001"));
    }

    #[test]
    fn endpoint_locality_rejects_nonlocal_ip() {
        assert!(!endpoint_is_local("http://203.0.113.10:8001"));
    }
}
