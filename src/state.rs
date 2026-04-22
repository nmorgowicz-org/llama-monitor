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

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsCapabilities {
    pub inference: bool,
    pub system: bool,
    pub gpu: bool,
    pub cpu_temperature: bool,
    pub memory: bool,
    pub host_metrics: bool,
    pub tray: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum EndpointKind {
    Local,
    Remote,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub enum SessionKind {
    Spawn,
    Attach,
    None,
}

#[derive(Debug, Clone, serde::Serialize)]
#[allow(dead_code)]
pub enum TrayMode {
    Desktop,
    Headless,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum AvailabilityReason {
    Available,
    RemoteEndpoint,
    NoDisplay,
    TrayUnavailable,
    SensorUnavailable,
    BackendUnavailable,
    CommandMissing,
    PermissionDenied,
    MetricsUnreachable,
    NotApplicable,
}

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
    #[serde(default)]
    pub remote_agent_url: String,
    #[serde(default)]
    pub remote_agent_token: String,
    #[serde(default)]
    pub remote_agent_ssh_autostart: bool,
    #[serde(default)]
    pub remote_agent_ssh_target: String,
    #[serde(default)]
    pub remote_agent_ssh_command: String,
}

/// Session mode: either spawn a new server or attach to existing
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
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
    pub created_at: u64,
    pub last_active: u64,
}

impl Session {
    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    pub fn new_spawn(id: String, name: String, port: u16) -> Self {
        let now = Self::now();
        Self {
            id,
            name,
            mode: SessionMode::Spawn { port },
            status: SessionStatus::Stopped,
            created_at: now,
            last_active: now,
        }
    }

    pub fn new_attach(id: String, name: String, endpoint: String) -> Self {
        let now = Self::now();
        Self {
            id,
            name,
            mode: SessionMode::Attach { endpoint },
            status: SessionStatus::Disconnected,
            created_at: now,
            last_active: now,
        }
    }

    pub fn is_inactive(&self) -> bool {
        matches!(
            self.status,
            SessionStatus::Stopped | SessionStatus::Disconnected
        )
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
            remote_agent_url: String::new(),
            remote_agent_token: String::new(),
            remote_agent_ssh_autostart: false,
            remote_agent_ssh_target: String::new(),
            remote_agent_ssh_command: String::new(),
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
    pub server_running: Arc<Mutex<bool>>, // Whether active endpoint is reachable (for inference)
    pub local_server_running: Arc<Mutex<bool>>, // Whether a local llama-server was spawned by this app
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
    #[allow(dead_code)]
    pub sessions_path: PathBuf,
    pub capabilities: Arc<Mutex<MetricsCapabilities>>,
    pub endpoint_kind: Arc<Mutex<EndpointKind>>,
    pub session_kind: Arc<Mutex<SessionKind>>,
    pub tray_mode: Arc<Mutex<TrayMode>>,
    pub remote_agent_connected: Arc<Mutex<bool>>,
    pub remote_agent_url: Arc<Mutex<Option<String>>>,
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

        let sessions = load_sessions(&sessions_path);
        let active_session_id = String::new();
        let endpoint_kind = EndpointKind::Unknown;
        let session_kind = SessionKind::None;

        let initial_capabilities = MetricsCapabilities {
            inference: true,
            system: false,
            gpu: false,
            cpu_temperature: false,
            memory: false,
            host_metrics: false,
            tray: true,
        };

        let state = Self {
            gpu_metrics: Arc::new(Mutex::new(BTreeMap::new())),
            llama_metrics: Arc::new(Mutex::new(LlamaMetrics::default())),
            server_logs: Arc::new(Mutex::new(VecDeque::new())),
            server_child: Arc::new(tokio::sync::Mutex::new(None)),
            server_running: Arc::new(Mutex::new(false)),
            local_server_running: Arc::new(Mutex::new(false)),
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
            sessions: Arc::new(Mutex::new(sessions)),
            active_session_id: Arc::new(Mutex::new(active_session_id)),
            sessions_path,
            capabilities: Arc::new(Mutex::new(initial_capabilities)),
            endpoint_kind: Arc::new(Mutex::new(endpoint_kind)),
            session_kind: Arc::new(Mutex::new(session_kind)),
            tray_mode: Arc::new(Mutex::new(TrayMode::Headless)),
            remote_agent_connected: Arc::new(Mutex::new(false)),
            remote_agent_url: Arc::new(Mutex::new(None)),
        };

        // Prune old inactive sessions on startup (older than 7 days)
        state.prune_old_sessions(7 * 24 * 60 * 60);

        state
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

    #[allow(dead_code)]
    pub fn get_active_session(&self) -> Option<Session> {
        let sessions = self.sessions.lock().unwrap();
        let active_id = self.active_session_id.lock().unwrap();
        sessions.iter().find(|s| s.id == *active_id).cloned()
    }

    pub fn set_active_session(&self, session_id: &str) -> bool {
        let exists = {
            let sessions = self.sessions.lock().unwrap();
            session_id.is_empty() || sessions.iter().any(|s| s.id == session_id)
        };
        if !exists {
            return false;
        }

        *self.active_session_id.lock().unwrap() = session_id.to_string();
        self.refresh_capability_state();
        if !session_id.is_empty() {
            self.llama_poll_notify.notify_waiters();
        }
        true
    }

    pub fn add_session(&self, mut session: Session) -> bool {
        let mut sessions = match self.sessions.lock() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] Failed to acquire sessions lock: {e}");
                return false;
            }
        };

        let now = Session::now();
        session.last_active = now;

        // If at capacity, try to remove old inactive sessions first
        if sessions.len() >= MAX_SESSIONS {
            // Sort by created_at (oldest first), prefer inactive sessions
            let mut indices: Vec<usize> = (0..sessions.len()).collect();
            indices.sort_by(|&a, &b| {
                let sa = &sessions[a];
                let sb = &sessions[b];
                // Inactive sessions first, then by created_at
                (sa.is_inactive() as i32, sa.created_at)
                    .cmp(&(sb.is_inactive() as i32, sb.created_at))
            });

            // Find first inactive session to remove
            for &idx in &indices {
                if sessions[idx].is_inactive() {
                    eprintln!(
                        "[info] Auto-removing old inactive session: {} ({})",
                        sessions[idx].name, sessions[idx].id
                    );
                    sessions.remove(idx);
                    break;
                }
            }

            // If still at capacity and no inactive sessions, we're truly full
            if sessions.len() >= MAX_SESSIONS {
                eprintln!(
                    "[warn] Session limit reached: all {} sessions are active",
                    sessions.len()
                );
                return false;
            }
        }

        sessions.push(session);
        true
    }

    /// Remove inactive sessions older than the given duration (in seconds)
    pub fn prune_old_sessions(&self, max_age_secs: u64) -> usize {
        let now = Session::now();
        let mut removed = 0;

        let mut sessions = match self.sessions.lock() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] Failed to acquire sessions lock for prune: {e}");
                return 0;
            }
        };

        let len_before = sessions.len();
        sessions.retain(|s| {
            let is_old = now.saturating_sub(s.created_at) > max_age_secs;
            let is_inactive = s.is_inactive();
            if is_old && is_inactive {
                eprintln!(
                    "[info] Pruning old inactive session: {} ({} days old)",
                    s.name, is_old as u64
                );
                removed += 1;
                false
            } else {
                true
            }
        });

        if sessions.len() < len_before {
            eprintln!("[info] Pruned {} old inactive session(s)", removed);
        }

        removed
    }

    pub fn remove_session(&self, session_id: &str) -> bool {
        let (removed, active_changed) = {
            let mut sessions = self.sessions.lock().unwrap();
            let len_before = sessions.len();
            sessions.retain(|s| s.id != session_id);
            let removed = sessions.len() < len_before;

            let mut active = self.active_session_id.lock().unwrap();
            let active_changed = if *active == session_id {
                active.clear();
                true
            } else {
                false
            };

            (removed, active_changed)
        };

        if active_changed {
            self.refresh_capability_state();
        }

        removed
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

    pub fn current_session_kind(&self) -> SessionKind {
        let active_id = self.active_session_id.lock().unwrap().clone();
        if active_id.is_empty() {
            return SessionKind::None;
        }

        let session = {
            let sessions = self.sessions.lock().unwrap();
            sessions.iter().find(|s| s.id == active_id).cloned()
        };

        match session.map(|s| s.mode) {
            Some(SessionMode::Spawn { .. }) => SessionKind::Spawn,
            Some(SessionMode::Attach { .. }) => SessionKind::Attach,
            None => SessionKind::None,
        }
    }

    pub fn current_endpoint_kind(&self) -> EndpointKind {
        let active_id = self.active_session_id.lock().unwrap().clone();
        if active_id.is_empty() {
            return EndpointKind::Unknown;
        }

        let session = {
            let sessions = self.sessions.lock().unwrap();
            sessions.iter().find(|s| s.id == active_id).cloned()
        };

        match session.map(|s| s.mode) {
            Some(SessionMode::Spawn { .. }) => EndpointKind::Local,
            Some(SessionMode::Attach { endpoint }) => endpoint_kind_from_endpoint(&endpoint),
            None => EndpointKind::Unknown,
        }
    }

    pub fn refresh_capability_state(&self) {
        let capabilities = self.calculate_capabilities();
        let endpoint_kind = self.current_endpoint_kind();
        let session_kind = self.current_session_kind();

        *self.capabilities.lock().unwrap() = capabilities;
        *self.endpoint_kind.lock().unwrap() = endpoint_kind;
        *self.session_kind.lock().unwrap() = session_kind;
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
            Some(SessionMode::Attach { .. }) => false, // Attach never uses local metrics - only inference stats
            None => true,
        }
    }

    pub fn remote_agent_connected(&self) -> bool {
        *self.remote_agent_connected.lock().unwrap()
    }

    pub fn host_metrics_available(&self) -> bool {
        self.active_session_uses_local_metrics() || self.remote_agent_connected()
    }

    #[allow(dead_code)]
    pub fn calculate_capabilities(&self) -> MetricsCapabilities {
        let active_id = self.active_session_id.lock().unwrap().clone();
        if active_id.is_empty() {
            return MetricsCapabilities {
                inference: true,
                system: false,
                gpu: false,
                cpu_temperature: false,
                memory: false,
                host_metrics: false,
                tray: true,
            };
        }

        let session = {
            let sessions = self.sessions.lock().unwrap();
            sessions.iter().find(|s| s.id == active_id).cloned()
        };

        match session {
            Some(s) if matches!(s.mode, SessionMode::Spawn { .. }) => MetricsCapabilities {
                inference: true,
                system: true,
                gpu: true,
                cpu_temperature: true,
                memory: true,
                host_metrics: true,
                tray: true,
            },
            Some(_) if self.remote_agent_connected() => MetricsCapabilities {
                inference: true,
                system: true,
                gpu: true,
                cpu_temperature: true,
                memory: true,
                host_metrics: true,
                tray: true,
            },
            Some(_) => MetricsCapabilities {
                inference: true,
                system: false,
                gpu: false,
                cpu_temperature: false,
                memory: false,
                host_metrics: false,
                tray: true,
            },
            None => MetricsCapabilities {
                inference: true,
                system: false,
                gpu: false,
                cpu_temperature: false,
                memory: false,
                host_metrics: false,
                tray: true,
            },
        }
    }

    #[allow(dead_code)]
    pub fn calculate_availability_reasons(
        &self,
    ) -> (AvailabilityReason, AvailabilityReason, AvailabilityReason) {
        let capabilities = self.calculate_capabilities();

        let system_reason = if capabilities.system {
            AvailabilityReason::Available
        } else {
            AvailabilityReason::RemoteEndpoint
        };

        let gpu_reason = if capabilities.gpu {
            AvailabilityReason::Available
        } else {
            AvailabilityReason::RemoteEndpoint
        };

        let cpu_temp_reason = if capabilities.cpu_temperature {
            AvailabilityReason::Available
        } else {
            AvailabilityReason::RemoteEndpoint
        };

        (system_reason, gpu_reason, cpu_temp_reason)
    }
}

#[allow(dead_code)]
pub fn endpoint_kind_from_endpoint(endpoint: &str) -> EndpointKind {
    if endpoint_is_local(endpoint) {
        EndpointKind::Local
    } else {
        EndpointKind::Remote
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

    fn test_paths(sessions_path: PathBuf) -> AppPaths {
        AppPaths {
            presets_path: PathBuf::new(),
            models_dir: None,
            gpu_env_path: PathBuf::new(),
            ui_settings_path: PathBuf::new(),
            sessions_path,
        }
    }

    #[test]
    fn endpoint_detection_with_various_hosts() {
        let local_hosts = [
            "http://localhost:8001",
            "127.0.0.1:8001",
            "http://[::1]:8001",
        ];
        for host in local_hosts {
            assert!(endpoint_is_local(host));
            assert_eq!(endpoint_kind_from_endpoint(host), EndpointKind::Local);
        }

        let remote_hosts = [
            "http://203.0.113.10:8001",
            "http://192.168.1.100:8001",
            "http://example.com:8001",
        ];
        for host in remote_hosts {
            assert!(!endpoint_is_local(host));
            assert_eq!(endpoint_kind_from_endpoint(host), EndpointKind::Remote);
        }
    }

    #[test]
    fn availability_reason_roundtrip() {
        let reasons = [
            AvailabilityReason::Available,
            AvailabilityReason::RemoteEndpoint,
            AvailabilityReason::NoDisplay,
            AvailabilityReason::TrayUnavailable,
            AvailabilityReason::SensorUnavailable,
            AvailabilityReason::BackendUnavailable,
            AvailabilityReason::CommandMissing,
            AvailabilityReason::PermissionDenied,
            AvailabilityReason::MetricsUnreachable,
            AvailabilityReason::NotApplicable,
        ];

        for reason in reasons {
            let json = serde_json::to_string(&reason).unwrap();
            let deserialized: AvailabilityReason = serde_json::from_str(&json).unwrap();
            assert_eq!(reason, deserialized);
        }
    }

    #[test]
    fn capabilities_based_on_session_mode() {
        let test_cases = [
            (
                "spawn",
                true,
                true,
                true,
                AvailabilityReason::Available,
                AvailabilityReason::Available,
                AvailabilityReason::Available,
            ),
            (
                "attach",
                true,
                false,
                false,
                AvailabilityReason::RemoteEndpoint,
                AvailabilityReason::RemoteEndpoint,
                AvailabilityReason::RemoteEndpoint,
            ),
        ];

        for (
            mode,
            exp_inference,
            exp_system,
            exp_gpu,
            exp_sys_reason,
            exp_gpu_reason,
            exp_cpu_reason,
        ) in test_cases
        {
            let paths = test_paths(PathBuf::new());
            let state = AppState::new(vec![], paths, GpuEnv::default(), UiSettings::default());
            let session = if mode == "spawn" {
                Session::new_spawn("test".to_string(), "Test".to_string(), 8001)
            } else {
                Session::new_attach(
                    "test".to_string(),
                    "Test".to_string(),
                    "http://remote.example.com:8001".to_string(),
                )
            };
            state.add_session(session);
            state.set_active_session("test");

            let caps = state.calculate_capabilities();
            assert!(caps.inference == exp_inference, "inference for {mode}");
            assert!(caps.system == exp_system, "system for {mode}");
            assert!(caps.gpu == exp_gpu, "gpu for {mode}");

            let (system_reason, gpu_reason, cpu_temp_reason) =
                state.calculate_availability_reasons();
            assert_eq!(system_reason, exp_sys_reason, "system_reason for {mode}");
            assert_eq!(gpu_reason, exp_gpu_reason, "gpu_reason for {mode}");
            assert_eq!(
                cpu_temp_reason, exp_cpu_reason,
                "cpu_temp_reason for {mode}"
            );
        }
    }

    #[test]
    fn startup_does_not_auto_activate_persisted_sessions() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sessions_path = std::env::temp_dir().join(format!(
            "llama-monitor-state-test-{}-{suffix}-sessions.json",
            std::process::id(),
        ));
        let sessions = vec![Session::new_attach(
            "saved".to_string(),
            "Saved".to_string(),
            "http://remote.example.com:8001".to_string(),
        )];
        std::fs::write(&sessions_path, serde_json::to_string(&sessions).unwrap()).unwrap();

        let state = AppState::new(
            vec![],
            test_paths(sessions_path.clone()),
            GpuEnv::default(),
            UiSettings::default(),
        );

        assert!(state.active_session_id.lock().unwrap().is_empty());
        assert_eq!(state.current_session_kind(), SessionKind::None);
        assert_eq!(state.current_endpoint_kind(), EndpointKind::Unknown);
        assert!(!state.calculate_capabilities().host_metrics);

        let _ = std::fs::remove_file(sessions_path);
    }

    #[test]
    fn set_active_session_rejects_missing_session_without_mutating() {
        let state = AppState::new(
            vec![],
            test_paths(PathBuf::new()),
            GpuEnv::default(),
            UiSettings::default(),
        );
        state.add_session(Session::new_spawn(
            "existing".to_string(),
            "Existing".to_string(),
            8001,
        ));

        assert!(state.set_active_session("existing"));
        assert!(!state.set_active_session("missing"));
        assert_eq!(*state.active_session_id.lock().unwrap(), "existing");
    }

    #[test]
    fn removing_active_session_clears_active_state() {
        let state = AppState::new(
            vec![],
            test_paths(PathBuf::new()),
            GpuEnv::default(),
            UiSettings::default(),
        );
        state.add_session(Session::new_spawn(
            "existing".to_string(),
            "Existing".to_string(),
            8001,
        ));

        assert!(state.set_active_session("existing"));
        assert!(state.remove_session("existing"));
        assert!(state.active_session_id.lock().unwrap().is_empty());
        assert_eq!(state.current_session_kind(), SessionKind::None);
        assert!(!state.calculate_capabilities().host_metrics);
    }

    #[test]
    fn metrics_capabilities_serializes_to_json() {
        let caps = MetricsCapabilities {
            inference: true,
            system: true,
            gpu: false,
            cpu_temperature: false,
            memory: false,
            host_metrics: false,
            tray: true,
        };
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["inference"], true);
        assert_eq!(json["system"], true);
        assert_eq!(json["gpu"], false);
    }
}
