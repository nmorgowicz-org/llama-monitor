use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use warp::Filter;
use warp::http::{HeaderMap, StatusCode};

use crate::config::AppConfig;
use crate::gpu::{self, GpuMetrics};
use crate::remote_ssh::{self, SshConnection};
use crate::state::{AppState, EndpointKind, SessionMode};
use crate::system::{self, SystemMetrics};

thread_local! {
    static LATEST_RELEASE_CACHE: Mutex<Option<(LatestReleaseInfo, Instant)>> = const { Mutex::new(None) };
}

const REMOTE_AGENT_DEFAULT_PORT: u16 = 7779;
const REMOTE_AGENT_POLL_INTERVAL: Duration = Duration::from_secs(2);
const REMOTE_AGENT_AUTOSTART_COOLDOWN: Duration = Duration::from_secs(30);
const REMOTE_AGENT_AUTOSTART_TIMEOUT: Duration = Duration::from_secs(15);
const GITHUB_LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/nmorgowicz-org/llama-monitor/releases/latest";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub system: SystemMetrics,
    pub gpu: BTreeMap<String, GpuMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAssetInfo {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub platform: String,
    pub arch: String,
    pub archive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestReleaseInfo {
    pub tag_name: String,
    pub name: Option<String>,
    pub assets: Vec<ReleaseAssetInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteAgentDetectRequest {
    pub ssh_target: String,
    #[serde(default)]
    pub ssh_connection: Option<SshConnection>,
    pub agent_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteAgentDetectResponse {
    pub ok: bool,
    pub ssh_target: String,
    pub os: String,
    pub arch: String,
    pub install_path: Option<String>,
    pub start_command: Option<String>,
    pub installed: bool,
    pub reachable: bool,
    pub managed_task_name: Option<String>,
    pub managed_task_installed: bool,
    pub managed_task_command: Option<String>,
    pub managed_task_matches: bool,
    pub installed_version: Option<String>,
    pub latest_release: Option<LatestReleaseInfo>,
    pub matching_asset: Option<ReleaseAssetInfo>,
    pub update_available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

pub async fn run_agent_server(app_config: Arc<AppConfig>) -> Result<()> {
    let bind_addr = format!("{}:{}", app_config.agent_host, app_config.agent_port)
        .parse::<SocketAddr>()
        .context("invalid agent bind address")?;
    let token = app_config.agent_token.clone();

    let backend = gpu::detect_backend(&app_config.gpu_backend);
    let gpu_metrics: Arc<Mutex<BTreeMap<String, GpuMetrics>>> =
        Arc::new(Mutex::new(BTreeMap::new()));

    {
        let gpu_metrics = Arc::clone(&gpu_metrics);
        let backend = Arc::clone(&backend);
        std::thread::spawn(move || {
            loop {
                match backend.read_metrics() {
                    Ok(metrics) => {
                        if let Ok(mut lock) = gpu_metrics.lock() {
                            *lock = metrics;
                        }
                    }
                    Err(e) => eprintln!("[agent] GPU metrics unavailable: {e}"),
                }

                std::thread::sleep(Duration::from_millis(500));
            }
        });
    }

    let auth =
        warp::any()
            .and(warp::header::headers_cloned())
            .and_then(move |headers: HeaderMap| {
                let token = token.clone();
                async move {
                    if let Some(expected) = token {
                        let valid = headers
                            .get("authorization")
                            .and_then(|value| value.to_str().ok())
                            .is_some_and(|value| value == format!("Bearer {expected}"));

                        if !valid {
                            return Err(warp::reject::custom(AgentAuthError));
                        }
                    }

                    Ok::<(), warp::Rejection>(())
                }
            });

    let health = warp::path("health")
        .and(warp::get())
        .and(auth.clone())
        .map(|_| warp::reply::json(&serde_json::json!({ "ok": true })));

    let info = {
        let info_bind_addr = bind_addr;
        warp::path("info")
            .and(warp::get())
            .and(auth.clone())
            .map(move |_| {
                warp::reply::json(&serde_json::json!({
                    "ok": true,
                    "version": env!("CARGO_PKG_VERSION"),
                    "mode": "agent",
                    "pid": std::process::id(),
                    "executable": std::env::current_exe()
                        .ok()
                        .map(|path| path.to_string_lossy().to_string()),
                    "bind": info_bind_addr.to_string(),
                    "platform": std::env::consts::OS,
                    "arch": std::env::consts::ARCH,
                }))
            })
    };

    let system_route = warp::path!("metrics" / "system")
        .and(warp::get())
        .and(auth.clone())
        .map(|_| warp::reply::json(&system::get_system_metrics()));

    let gpu_route = {
        let gpu_metrics = Arc::clone(&gpu_metrics);
        warp::path!("metrics" / "gpu")
            .and(warp::get())
            .and(auth.clone())
            .map(move |_| {
                let gpu = gpu_metrics.lock().unwrap().clone();
                warp::reply::json(&gpu)
            })
    };

    let metrics_route = {
        let gpu_metrics = Arc::clone(&gpu_metrics);
        warp::path("metrics")
            .and(warp::path::end())
            .and(warp::get())
            .and(auth)
            .map(move |_| {
                let metrics = AgentMetrics {
                    system: system::get_system_metrics(),
                    gpu: gpu_metrics.lock().unwrap().clone(),
                };
                warp::reply::json(&metrics)
            })
    };

    let routes = health
        .or(info)
        .or(system_route)
        .or(gpu_route)
        .or(metrics_route)
        .recover(handle_agent_rejection);

    if app_config.agent_token.is_none() {
        eprintln!("[agent] No --agent-token configured; metrics API is unauthenticated");
    }
    println!("[agent] Remote metrics agent listening on http://{bind_addr}");

    warp::serve(routes).run(bind_addr).await;
    Ok(())
}

pub async fn latest_release_info() -> Result<LatestReleaseInfo> {
    let cached = LATEST_RELEASE_CACHE.with(|cache| {
        let now = Instant::now();
        let cached = cache.try_lock().ok()?;
        if let Some((ref info, ref cached_at)) = *cached
            && now.duration_since(*cached_at) < Duration::from_secs(60)
        {
            return Some(info.clone());
        }
        None
    });

    if let Some(cached) = cached {
        return Ok(cached);
    }

    let release = reqwest::Client::new()
        .get(GITHUB_LATEST_RELEASE_URL)
        .header(reqwest::header::USER_AGENT, "llama-monitor")
        .send()
        .await?
        .error_for_status()?
        .json::<GithubRelease>()
        .await?;

    let release_info = LatestReleaseInfo {
        tag_name: release.tag_name,
        name: release.name,
        assets: release
            .assets
            .into_iter()
            .filter_map(asset_info_from_github_asset)
            .collect(),
    };

    LATEST_RELEASE_CACHE.with(|cache| {
        let _ = cache
            .try_lock()
            .map(|mut cached| *cached = Some((release_info.clone(), Instant::now())));
    });

    Ok(release_info)
}

pub async fn detect_remote_agent(req: RemoteAgentDetectRequest) -> RemoteAgentDetectResponse {
    let connection = req
        .ssh_connection
        .clone()
        .unwrap_or_else(|| SshConnection::from_target(&req.ssh_target));
    let ssh_target = if req.ssh_target.trim().is_empty() {
        connection.target_label()
    } else {
        req.ssh_target.trim().to_string()
    };

    if ssh_target.is_empty() || connection.host.trim().is_empty() {
        return RemoteAgentDetectResponse {
            ok: false,
            ssh_target,
            os: "unknown".to_string(),
            arch: "unknown".to_string(),
            install_path: None,
            start_command: None,
            installed: false,
            reachable: false,
            managed_task_name: None,
            managed_task_installed: false,
            managed_task_command: None,
            managed_task_matches: false,
            installed_version: None,
            latest_release: None,
            matching_asset: None,
            update_available: false,
            error: Some("Missing SSH target".to_string()),
        };
    }

    let remote_os = detect_remote_os_with(&connection).await;
    let os = remote_os.as_str().to_string();
    let arch = detect_remote_arch_with(&connection, remote_os).await;
    let install_path = install_path_for_os(remote_os).map(ToOwned::to_owned);
    let installed = if let Some(path) = install_path.as_deref() {
        remote_file_exists_with(&connection, remote_os, path).await
    } else {
        false
    };
    let managed_task =
        install::managed_task_status(&connection, remote_os, install_path.as_deref())
            .await
            .ok()
            .flatten();
    let reachable = if let Some(agent_url) = req.agent_url.as_deref() {
        agent_health_reachable(agent_url).await
    } else {
        false
    };

    let ssh_ok = remote_os != RemoteOs::Unknown;

    let installed_version = if installed && ssh_ok {
        install::get_remote_version_with(connection.clone())
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let latest_release = latest_release_info().await.ok();
    let matching_asset = latest_release
        .as_ref()
        .and_then(|release| release.matching_asset(&os, &arch).cloned());

    let update_available = if let (Some(installed_ver), Some(latest)) =
        (installed_version.as_deref(), latest_release.as_ref())
    {
        normalize_version_label(installed_ver) != normalize_version_label(&latest.tag_name)
    } else {
        false
    };

    let start_command = if ssh_ok {
        default_start_command_for_os_with(
            &connection,
            remote_os,
            install_path
                .as_deref()
                .unwrap_or("~/.config/llama-monitor/bin/llama-monitor"),
        )
        .await
    } else {
        String::new()
    };

    let error = if !ssh_ok {
        Some("Could not detect remote OS over SSH. Verify SSH connectivity and that remote host allows command execution.".to_string())
    } else if matching_asset.is_none() {
        Some(format!(
            "No release asset for {os} {arch}. Manual setup required: download the appropriate binary from the latest release and place it at {install_path:?}"
        ))
    } else {
        None
    };

    RemoteAgentDetectResponse {
        ok: error.is_none(),
        ssh_target,
        os,
        arch,
        install_path,
        start_command: if start_command.is_empty() {
            None
        } else {
            Some(start_command)
        },
        installed,
        reachable,
        managed_task_name: managed_task.as_ref().map(|task| task.name.clone()),
        managed_task_installed: managed_task.as_ref().is_some_and(|task| task.installed),
        managed_task_command: managed_task.as_ref().and_then(|task| task.command.clone()),
        managed_task_matches: managed_task
            .as_ref()
            .is_some_and(|task| task.matches_install_path),
        installed_version,
        latest_release,
        matching_asset,
        update_available,
        error,
    }
}

pub async fn remote_agent_poller(state: AppState, app_config: Arc<AppConfig>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .pool_max_idle_per_host(0)
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("[agent] Failed to build HTTP client: {e}");
            return;
        }
    };
    let mut last_autostart_attempt: Option<Instant> = None;
    let mut enabled = false;

    loop {
        if !enabled {
            state.llama_poll_notify.notified().await;
            enabled = true;
        }

        let settings = state.ui_settings.lock().unwrap().clone();
        let configured_url = first_non_empty([
            app_config.remote_agent_url.as_deref(),
            Some(settings.remote_agent_url.as_str()),
        ]);
        let url = remote_agent_url_for_active_session(&state, configured_url.as_deref());
        let token = first_non_empty([
            app_config.remote_agent_token.as_deref(),
            Some(settings.remote_agent_token.as_str()),
        ]);

        if let Some(url) = url {
            let mut request = client.get(format!("{}/metrics", url.trim_end_matches('/')));
            if let Some(token) = &token {
                request = request.bearer_auth(token);
            }

            match request.send().await {
                Ok(resp) if resp.status().is_success() => match resp.json::<AgentMetrics>().await {
                    Ok(metrics) => {
                        *state.system_metrics.lock().unwrap() = metrics.system;
                        *state.gpu_metrics.lock().unwrap() = metrics.gpu;
                        *state.remote_agent_connected.lock().unwrap() = true;
                        *state.remote_agent_url.lock().unwrap() = Some(url);
                        state.refresh_capability_state();
                    }
                    Err(e) => {
                        mark_disconnected(&state);
                        eprintln!("[agent] Failed to parse remote metrics: {e}");
                    }
                },
                Ok(resp) => {
                    mark_disconnected(&state);
                    eprintln!(
                        "[agent] Remote metrics request failed: HTTP {}",
                        resp.status()
                    );
                    maybe_autostart_remote_agent(
                        &state,
                        &app_config,
                        &settings,
                        &url,
                        &mut last_autostart_attempt,
                    )
                    .await;
                }
                Err(_) => {
                    mark_disconnected(&state);
                    maybe_autostart_remote_agent(
                        &state,
                        &app_config,
                        &settings,
                        &url,
                        &mut last_autostart_attempt,
                    )
                    .await;
                }
            }
        } else {
            mark_disconnected(&state);
            enabled = false;
        }

        tokio::time::sleep(REMOTE_AGENT_POLL_INTERVAL).await;
    }
}

async fn maybe_autostart_remote_agent(
    state: &AppState,
    app_config: &AppConfig,
    settings: &crate::state::UiSettings,
    agent_url: &str,
    last_attempt: &mut Option<Instant>,
) {
    let enabled = app_config.remote_agent_ssh_autostart || settings.remote_agent_ssh_autostart;
    if !enabled {
        return;
    }

    if state.current_endpoint_kind() != EndpointKind::Remote {
        return;
    }

    if last_attempt.is_some_and(|instant| instant.elapsed() < REMOTE_AGENT_AUTOSTART_COOLDOWN) {
        return;
    }
    *last_attempt = Some(Instant::now());

    let target = first_non_empty([
        app_config.remote_agent_ssh_target.as_deref(),
        Some(settings.remote_agent_ssh_target.as_str()),
    ])
    .or_else(|| remote_host_from_agent_url(agent_url));

    let Some(target) = target else {
        eprintln!("[agent] SSH autostart enabled but no SSH target is available");
        return;
    };

    let command = if let Some(command) = first_non_empty([
        app_config.remote_agent_ssh_command.as_deref(),
        Some(settings.remote_agent_ssh_command.as_str()),
    ]) {
        command
    } else {
        default_remote_agent_command_for_target(&target).await
    };

    eprintln!("[agent] Attempting remote agent autostart via ssh {target}");

    let connection = match remote_ssh::with_trusted_host_key(
        SshConnection::from_target(&target),
        &app_config.ssh_known_hosts_file,
    ) {
        Ok(connection) => connection,
        Err(e) => {
            eprintln!("[agent] Remote agent autostart blocked by SSH trust check: {e}");
            return;
        }
    };

    match tokio::time::timeout(
        REMOTE_AGENT_AUTOSTART_TIMEOUT,
        remote_ssh::exec(connection, command),
    )
    .await
    {
        Ok(Ok(output)) if output.status == 0 => {
            eprintln!("[agent] Remote agent autostart command completed");
        }
        Ok(Ok(output)) => {
            eprintln!(
                "[agent] Remote agent autostart command exited with status {}: {}",
                output.status,
                output.stderr.trim()
            );
        }
        Ok(Err(e)) => {
            eprintln!("[agent] Remote agent autostart command failed: {e}");
        }
        Err(_) => {
            eprintln!("[agent] Remote agent autostart timed out; use a detached remote command");
        }
    }
}

fn mark_disconnected(state: &AppState) {
    let was_connected = {
        let mut connected = state.remote_agent_connected.lock().unwrap();
        let was_connected = *connected;
        *connected = false;
        was_connected
    };
    if was_connected {
        state.refresh_capability_state();
    }
}

fn remote_agent_url_for_active_session(
    state: &AppState,
    configured_url: Option<&str>,
) -> Option<String> {
    if let Some(url) = configured_url.filter(|url| !url.trim().is_empty()) {
        return Some(url.trim().to_string());
    }

    if state.current_endpoint_kind() != EndpointKind::Remote {
        return None;
    }

    let active_id = state.active_session_id.lock().unwrap().clone();
    let session = {
        let sessions = state.sessions.lock().unwrap();
        sessions.iter().find(|s| s.id == active_id).cloned()
    }?;

    let SessionMode::Attach { endpoint } = session.mode else {
        return None;
    };

    let url = reqwest::Url::parse(&endpoint)
        .or_else(|_| reqwest::Url::parse(&format!("http://{endpoint}")))
        .ok()?;
    let host = url.host_str()?;
    Some(format!(
        "{}://{}:{}",
        url.scheme(),
        host,
        REMOTE_AGENT_DEFAULT_PORT
    ))
}

fn first_non_empty(values: [Option<&str>; 2]) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn remote_host_from_agent_url(agent_url: &str) -> Option<String> {
    let url = reqwest::Url::parse(agent_url).ok()?;
    url.host_str().map(ToOwned::to_owned)
}

async fn default_remote_agent_command_for_target(target: &str) -> String {
    default_start_command_for_os(
        detect_remote_os(target).await,
        "~/.config/llama-monitor/bin/llama-monitor",
    )
}

fn default_start_command_for_os(os: RemoteOs, install_path: &str) -> String {
    match os {
        RemoteOs::Windows => format!(
            "cmd.exe /C schtasks /Delete /TN \"{WINDOWS_AGENT_LEGACY_TASK_NAME}\" /F >NUL 2>NUL & schtasks /Create /TN \"{WINDOWS_AGENT_TASK_NAME}\" /TR \"\\\"{install_path}\\\" --agent --agent-host 0.0.0.0 --agent-port {REMOTE_AGENT_DEFAULT_PORT}\" /SC ONLOGON /F && schtasks /Run /TN \"{WINDOWS_AGENT_TASK_NAME}\""
        ),
        RemoteOs::Unix | RemoteOs::Macos => format!(
            "nohup {install_path} --agent --agent-host 0.0.0.0 --agent-port {REMOTE_AGENT_DEFAULT_PORT} > ~/.config/llama-monitor/agent.log 2>&1 &"
        ),
        RemoteOs::Unknown => format!(
            "{install_path} --agent --agent-host 0.0.0.0 --agent-port {REMOTE_AGENT_DEFAULT_PORT}"
        ),
    }
}

pub(crate) async fn default_start_command_for_os_with(
    connection: &SshConnection,
    os: RemoteOs,
    install_path: &str,
) -> String {
    let resolved_path = if os == RemoteOs::Windows {
        if let Some(appdata) = resolve_windows_appdata(connection).await {
            install_path.replace("%APPDATA%", &appdata)
        } else {
            install_path.to_string()
        }
    } else {
        install_path.to_string()
    };
    default_start_command_for_os(os, &resolved_path)
}

const WINDOWS_AGENT_TASK_NAME: &str = "LlamaMonitorAgent";
const WINDOWS_AGENT_LEGACY_TASK_NAME: &str = "llama-monitor-agent";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteOs {
    Windows,
    Unix,
    Macos,
    Unknown,
}

async fn detect_remote_os(target: &str) -> RemoteOs {
    detect_remote_os_with(&SshConnection::from_target(target)).await
}

async fn detect_remote_os_with(connection: &SshConnection) -> RemoteOs {
    let windows = remote_ssh::exec(connection.clone(), "cmd.exe /C ver".to_string());

    if let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(5), windows).await
        && output.status == 0
        && output.stdout.contains("Windows")
    {
        return RemoteOs::Windows;
    }

    let unix = remote_ssh::exec(connection.clone(), "uname -s".to_string());
    if let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(5), unix).await
        && output.status == 0
    {
        let name = output.stdout.to_ascii_lowercase();
        if name.contains("darwin") {
            return RemoteOs::Macos;
        }
        return RemoteOs::Unix;
    }

    RemoteOs::Unknown
}

/// Resolves `%APPDATA%` to its actual expanded path on the remote Windows host.
///
/// `schtasks /TR` stores the task command literally. When the task runs in the
/// Task Scheduler service context (which differs from the SSH session), env-var
/// expansion may resolve `%APPDATA%` to a system profile path instead of the
/// user's, causing "The system cannot find the path specified" on `schtasks /Run`.
/// Expanding the path at install/start time avoids this entirely.
pub(crate) async fn resolve_windows_appdata(connection: &SshConnection) -> Option<String> {
    match tokio::time::timeout(
        Duration::from_secs(5),
        remote_ssh::exec(connection.clone(), "cmd.exe /C echo %APPDATA%".to_string()),
    )
    .await
    {
        Ok(Ok(out)) if out.status == 0 => {
            let s = out.stdout.trim().to_string();
            // Guard: if %APPDATA% isn't set in the SSH env it echoes literally
            if s.is_empty() || s.starts_with('%') {
                None
            } else {
                Some(s)
            }
        }
        _ => None,
    }
}

async fn detect_remote_temp_dir(connection: &SshConnection, os: RemoteOs) -> String {
    let temp_cmd = match os {
        RemoteOs::Windows => "cmd.exe /C echo %TEMP%".to_string(),
        RemoteOs::Unix | RemoteOs::Macos => "echo /tmp".to_string(),
        RemoteOs::Unknown => return "/tmp".to_string(),
    };

    match tokio::time::timeout(
        Duration::from_secs(5),
        remote_ssh::exec(connection.clone(), temp_cmd),
    )
    .await
    {
        Ok(Ok(output)) if output.status == 0 && !output.stdout.trim().is_empty() => {
            output.stdout.trim().to_string()
        }
        _ => match os {
            RemoteOs::Windows => "C:\\\\Windows\\\\Temp".to_string(),
            _ => "/tmp".to_string(),
        },
    }
}

pub async fn detect_remote_os_for_connection(connection: SshConnection) -> RemoteOs {
    detect_remote_os_with(&connection).await
}

#[derive(Debug)]
struct AgentAuthError;

impl warp::reject::Reject for AgentAuthError {}

impl LatestReleaseInfo {
    pub fn matching_asset(&self, os: &str, arch: &str) -> Option<&ReleaseAssetInfo> {
        self.assets
            .iter()
            .find(|asset| asset.platform == os && asset.arch == normalize_arch(arch))
    }
}

fn asset_info_from_github_asset(asset: GithubAsset) -> Option<ReleaseAssetInfo> {
    let (platform, arch, archive) = match asset.name.as_str() {
        "llama-monitor-windows-x86_64.exe" => ("windows", "x86_64", false),
        "llama-monitor-linux-x86_64" => ("linux", "x86_64", false),
        "llama-monitor-linux-aarch64" => ("linux", "aarch64", false),
        "llama-monitor-macos-aarch64.tar.gz" => ("macos", "aarch64", true),
        _ => return None,
    };

    Some(ReleaseAssetInfo {
        name: asset.name,
        url: asset.browser_download_url,
        size: asset.size,
        platform: platform.to_string(),
        arch: arch.to_string(),
        archive,
    })
}

impl RemoteOs {
    fn as_str(self) -> &'static str {
        match self {
            RemoteOs::Windows => "windows",
            RemoteOs::Unix => "linux",
            RemoteOs::Macos => "macos",
            RemoteOs::Unknown => "unknown",
        }
    }
}

fn normalize_arch(arch: &str) -> String {
    match arch.trim().to_ascii_lowercase().as_str() {
        "amd64" | "x64" | "x86_64" => "x86_64".to_string(),
        "arm64" | "aarch64" => "aarch64".to_string(),
        other => other.to_string(),
    }
}

fn normalize_version_label(version: &str) -> String {
    version
        .split_whitespace()
        .last()
        .unwrap_or(version.trim())
        .trim_start_matches('v')
        .to_string()
}

fn install_path_for_os(os: RemoteOs) -> Option<&'static str> {
    match os {
        RemoteOs::Windows => Some("%APPDATA%\\llama-monitor\\bin\\llama-monitor.exe"),
        RemoteOs::Unix | RemoteOs::Macos => Some("~/.config/llama-monitor/bin/llama-monitor"),
        RemoteOs::Unknown => None,
    }
}

fn default_install_path_for_os(os: RemoteOs) -> String {
    install_path_for_os(os)
        .unwrap_or("/tmp/llama-monitor")
        .to_string()
}

async fn detect_remote_arch_with(connection: &SshConnection, os: RemoteOs) -> String {
    let command = match os {
        RemoteOs::Windows => "cmd.exe /C echo %PROCESSOR_ARCHITECTURE%",
        RemoteOs::Unix | RemoteOs::Macos => "uname -m",
        RemoteOs::Unknown => return "unknown".to_string(),
    };

    match tokio::time::timeout(
        Duration::from_secs(5),
        remote_ssh::exec(connection.clone(), command.to_string()),
    )
    .await
    {
        Ok(Ok(output)) if output.status == 0 => normalize_arch(output.stdout.trim()),
        _ => "unknown".to_string(),
    }
}

async fn remote_file_exists_with(connection: &SshConnection, os: RemoteOs, path: &str) -> bool {
    let command = match os {
        RemoteOs::Windows => format!("cmd.exe /C if exist \"{path}\" (echo yes)"),
        RemoteOs::Unix | RemoteOs::Macos => format!("test -x {path} && echo yes"),
        RemoteOs::Unknown => return false,
    };

    matches!(
        tokio::time::timeout(
            Duration::from_secs(5),
            remote_ssh::exec(connection.clone(), command)
        )
        .await,
        Ok(Ok(output))
            if output.status == 0 && output.stdout.contains("yes")
    )
}

async fn agent_health_reachable(agent_url: &str) -> bool {
    let Ok(resp) = reqwest::Client::new()
        .get(format!("{}/health", agent_url.trim_end_matches('/')))
        .timeout(Duration::from_secs(2))
        .send()
        .await
    else {
        return false;
    };

    resp.status().is_success()
}

async fn handle_agent_rejection(
    rejection: warp::Rejection,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    if rejection.find::<AgentAuthError>().is_some() {
        let reply = warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
            StatusCode::UNAUTHORIZED,
        );
        return Ok(reply);
    }

    let reply = warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "error": "not found" })),
        StatusCode::NOT_FOUND,
    );
    Ok(reply)
}

pub mod install {
    use super::*;
    use std::fs;
    use std::io;

    const REMOTE_AGENT_INSTALL_TIMEOUT: Duration = Duration::from_secs(60);

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RemoteAgentInstallRequest {
        pub ssh_target: String,
        #[serde(default)]
        pub ssh_connection: Option<SshConnection>,
        pub asset: ReleaseAssetInfo,
        pub install_path: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RemoteAgentInstallResponse {
        pub ok: bool,
        pub ssh_target: String,
        pub asset_name: String,
        pub install_path: String,
        pub installed: bool,
        pub error: Option<String>,
    }

    pub async fn install_remote_agent(
        ssh_target: &str,
        ssh_connection: Option<SshConnection>,
        asset: &ReleaseAssetInfo,
        install_path: Option<String>,
        os: RemoteOs,
    ) -> Result<RemoteAgentInstallResponse> {
        let connection = ssh_connection.unwrap_or_else(|| SshConnection::from_target(ssh_target));
        let remote_temp_dir = detect_remote_temp_dir(&connection, os).await;
        let remote_temp_name = remote_temp_name_for_asset(asset);
        let remote_temp_path = match os {
            RemoteOs::Windows => format!("{}\\{}", remote_temp_dir, remote_temp_name),
            _ => format!("{}/{}", remote_temp_dir, remote_temp_name),
        };

        transfer_asset_to_remote_temp(&connection, asset, os, &remote_temp_path).await?;

        let install_path = install_path
            .or_else(|| install_path_for_os(os).map(ToOwned::to_owned))
            .context("Could not determine install path")?;

        move_binary_to_install_path(&connection, &remote_temp_path, &install_path, os).await?;

        if os == RemoteOs::Unix || os == RemoteOs::Macos {
            set_executable_bit(&connection, &install_path, os).await?;
        }

        let installed = remote_file_exists_with(&connection, os, &install_path).await;

        Ok(RemoteAgentInstallResponse {
            ok: installed,
            ssh_target: connection.target_label(),
            asset_name: asset.name.clone(),
            install_path,
            installed,
            error: if installed {
                None
            } else {
                Some("Binary not found after install".to_string())
            },
        })
    }

    async fn download_asset_locally(asset: &ReleaseAssetInfo) -> Result<String> {
        let resp = reqwest::Client::new()
            .get(&asset.url)
            .header(reqwest::header::USER_AGENT, "llama-monitor")
            .send()
            .await?
            .error_for_status()?;

        let bytes = resp.bytes().await?;
        let temp_path = std::env::temp_dir().join(&asset.name);
        fs::write(&temp_path, &bytes)?;
        Ok(temp_path.to_string_lossy().to_string())
    }

    fn remote_temp_name_for_asset(asset: &ReleaseAssetInfo) -> String {
        if asset.archive {
            asset.name.trim_end_matches(".tar.gz").to_string()
        } else {
            asset.name.clone()
        }
    }

    async fn transfer_asset_to_remote_temp(
        connection: &SshConnection,
        asset: &ReleaseAssetInfo,
        os: RemoteOs,
        remote_temp_path: &str,
    ) -> Result<()> {
        if os == RemoteOs::Windows && !asset.archive {
            match download_asset_remotely(connection, asset, os, remote_temp_path).await {
                Ok(()) => return Ok(()),
                Err(remote_error) => {
                    let local_result =
                        download_and_copy_asset(connection, asset, os, remote_temp_path).await;
                    return local_result.map_err(|local_error| {
                        io::Error::other(format!(
                            "remote curl download failed ({remote_error}); local SCP fallback also failed ({local_error})"
                        ))
                        .into()
                    });
                }
            }
        }

        match download_and_copy_asset(connection, asset, os, remote_temp_path).await {
            Ok(()) => Ok(()),
            Err(copy_error) if !asset.archive => download_asset_remotely(
                connection,
                asset,
                os,
                remote_temp_path,
            )
            .await
            .map_err(|remote_error| {
                io::Error::other(format!(
                    "local SCP upload failed ({copy_error}); remote curl fallback also failed ({remote_error})"
                ))
                .into()
            }),
            Err(copy_error) => Err(copy_error),
        }
    }

    async fn download_and_copy_asset(
        connection: &SshConnection,
        asset: &ReleaseAssetInfo,
        os: RemoteOs,
        remote_temp_path: &str,
    ) -> Result<()> {
        let temp_local_path = download_asset_locally(asset).await?;
        let temp_extracted_path = if asset.archive {
            Some(extract_archive_with_timeout(&temp_local_path, asset).await?)
        } else {
            None
        };
        let binary_local_path = temp_extracted_path
            .as_deref()
            .unwrap_or(temp_local_path.as_str());

        copy_to_remote(connection, binary_local_path, remote_temp_path, os).await
    }

    async fn download_asset_remotely(
        connection: &SshConnection,
        asset: &ReleaseAssetInfo,
        os: RemoteOs,
        remote_temp_path: &str,
    ) -> Result<()> {
        let command = match os {
            RemoteOs::Windows => format!(
                "cmd.exe /C curl.exe -fL -o \"{}\" \"{}\"",
                remote_temp_path, asset.url
            ),
            RemoteOs::Unix | RemoteOs::Macos => {
                format!("curl -fL -o '{}' '{}'", remote_temp_path, asset.url)
            }
            RemoteOs::Unknown => return Err(io::Error::other("Unknown OS").into()),
        };

        let output = remote_ssh::exec(connection.clone(), command)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;

        if output.status == 0 {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "status {}: {}",
                output.status,
                output.stderr.trim()
            ))
            .into())
        }
    }

    async fn extract_archive_with_timeout(path: &str, asset: &ReleaseAssetInfo) -> Result<String> {
        tokio::time::timeout(REMOTE_AGENT_INSTALL_TIMEOUT, extract_archive(path, asset))
            .await?
            .map_err(|e| io::Error::other(format!("Archive extraction failed: {e}")).into())
    }

    async fn extract_archive(path: &str, asset: &ReleaseAssetInfo) -> Result<String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let binary_name = asset.name.trim_end_matches(".tar.gz");
        let temp_extracted = std::env::temp_dir().join(format!(
            "{}-{}-{timestamp}",
            binary_name,
            std::process::id()
        ));
        fs::create_dir_all(&temp_extracted)?;

        let output = tokio::process::Command::new("tar")
            .args(["-xzf", path, "-C", &temp_extracted.to_string_lossy()])
            .output()
            .await?;

        if !output.status.success() {
            Err(io::Error::other("Failed to extract archive").into())
        } else {
            extracted_binary_path(&temp_extracted, binary_name)
        }
    }

    fn extracted_binary_path(dir: &std::path::Path, binary_name: &str) -> Result<String> {
        let expected = dir.join(binary_name);
        if expected.is_file() {
            return Ok(expected.to_string_lossy().to_string());
        }

        let mut files = fs::read_dir(dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        files.sort();

        files
            .into_iter()
            .next()
            .map(|path| path.to_string_lossy().to_string())
            .ok_or_else(|| io::Error::other("Archive did not contain a binary file").into())
    }

    async fn copy_to_remote(
        connection: &SshConnection,
        local_path: &str,
        remote_path: &str,
        _os: RemoteOs,
    ) -> Result<()> {
        remote_ssh::copy_to_remote(
            connection.clone(),
            local_path.to_string(),
            remote_path.to_string(),
            0o755,
        )
        .await
    }

    async fn move_binary_to_install_path(
        connection: &SshConnection,
        temp_path: &str,
        install_path: &str,
        os: RemoteOs,
    ) -> Result<()> {
        // Extract directory from install_path using string manipulation
        // (Path API doesn't handle Windows env vars like %APPDATA%)
        let install_dir = match os {
            RemoteOs::Windows => {
                // Find last backslash
                if let Some(pos) = install_path.rfind('\\') {
                    install_path[..pos].to_string()
                } else {
                    return Err(io::Error::other("no directory in install path").into());
                }
            }
            RemoteOs::Unix | RemoteOs::Macos => {
                // Find last forward slash
                if let Some(pos) = install_path.rfind('/') {
                    install_path[..pos].to_string()
                } else {
                    return Err(io::Error::other("no directory in install path").into());
                }
            }
            RemoteOs::Unknown => return Err(io::Error::other("Unknown OS").into()),
        };

        let mkdir_command = match os {
            RemoteOs::Windows => format!(
                "cmd.exe /C if not exist \"{}\" mkdir \"{}\"",
                install_dir, install_dir
            ),
            RemoteOs::Unix | RemoteOs::Macos => format!("mkdir -p {}", install_dir),
            RemoteOs::Unknown => return Err(io::Error::other("Unknown OS").into()),
        };

        let output = remote_ssh::exec(connection.clone(), mkdir_command)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;

        if output.status != 0 {
            return Err(io::Error::other(format!(
                "Failed to create install dir: {}",
                output.stderr.trim()
            ))
            .into());
        }

        let command = match os {
            RemoteOs::Windows => format!("cmd.exe /C move /Y \"{temp_path}\" \"{install_path}\""),
            RemoteOs::Unix | RemoteOs::Macos => format!("mv {temp_path} {install_path}"),
            RemoteOs::Unknown => return Err(io::Error::other("Unknown OS").into()),
        };

        let output = remote_ssh::exec(connection.clone(), command)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;

        if output.status == 0 {
            Ok(())
        } else {
            Err(io::Error::other(format!("Failed to move binary: {}", output.stderr.trim())).into())
        }
    }

    async fn set_executable_bit(
        connection: &SshConnection,
        path: &str,
        os: RemoteOs,
    ) -> Result<()> {
        let output = match os {
            RemoteOs::Unix | RemoteOs::Macos => {
                remote_ssh::exec(connection.clone(), format!("chmod +x {path}"))
                    .await
                    .map_err(|e| io::Error::other(e.to_string()))?
            }
            _ => return Ok(()),
        };

        if output.status == 0 {
            Ok(())
        } else {
            Err(io::Error::other("Failed to set executable bit").into())
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RemoteAgentStartResponse {
        pub ok: bool,
        pub ssh_target: String,
        pub install_path: String,
        pub running: bool,
        pub health_reachable: bool,
        pub error: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RemoteAgentUpdateResponse {
        pub ok: bool,
        pub ssh_target: String,
        pub previous_version: Option<String>,
        pub new_version: Option<String>,
        pub updated: bool,
        pub error: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RemoteAgentStopResponse {
        pub ok: bool,
        pub ssh_target: String,
        pub stopped: bool,
        pub error: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RemoteAgentStatusResponse {
        pub ok: bool,
        pub ssh_target: String,
        pub os: String,
        pub install_path: String,
        pub installed: bool,
        pub running: bool,
        pub health_reachable: bool,
        pub installed_version: Option<String>,
        pub managed_task_name: Option<String>,
        pub managed_task_installed: bool,
        pub managed_task_command: Option<String>,
        pub managed_task_matches: bool,
        pub error: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RemoteAgentRemoveResponse {
        pub ok: bool,
        pub ssh_target: String,
        pub removed: bool,
        pub error: Option<String>,
    }

    #[derive(Debug, Clone)]
    pub struct ManagedTaskStatus {
        pub name: String,
        pub installed: bool,
        pub command: Option<String>,
        pub matches_install_path: bool,
    }

    pub async fn start_remote_agent(
        ssh_target: &str,
        ssh_connection: Option<SshConnection>,
        install_path: &str,
        command: &str,
    ) -> Result<RemoteAgentStartResponse> {
        let connection = ssh_connection.unwrap_or_else(|| SshConnection::from_target(ssh_target));
        let start_warning = match tokio::time::timeout(
            Duration::from_secs(3),
            remote_ssh::exec(connection.clone(), command.to_string()),
        )
        .await
        {
            Ok(Ok(output)) if output.status == 0 => None,
            Ok(Ok(output)) => {
                let error_msg = if !output.stderr.is_empty() {
                    format!("Start command failed: {}", output.stderr.trim())
                } else {
                    format!("Start command exited with status: {}", output.status)
                };
                return Ok(RemoteAgentStartResponse {
                    ok: false,
                    ssh_target: connection.target_label(),
                    install_path: install_path.to_string(),
                    running: false,
                    health_reachable: false,
                    error: Some(error_msg),
                });
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => Some(
                "Start command did not return within 3 seconds; checking agent health".to_string(),
            ),
        };

        let health_reachable = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if agent_health_reachable(&connection.agent_url(REMOTE_AGENT_DEFAULT_PORT)).await {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
        .await;

        let running = health_reachable.is_ok();

        let error = if !running {
            let health_error = match health_reachable {
                Err(tokio::time::error::Elapsed { .. }) => Some("Agent did not start within 10 seconds. Check if the agent is listening on 0.0.0.0:7779 and if the remote firewall allows inbound connections on port 7779.".to_string()),
                Ok(()) => None,
            };
            if health_error.is_some() {
                health_error
            } else if start_warning.is_some() {
                start_warning
            } else {
                Some("Agent started but is not reachable. Check SSH access and firewall rules on port 7779.".to_string())
            }
        } else {
            None
        };

        Ok(RemoteAgentStartResponse {
            ok: running,
            ssh_target: connection.target_label(),
            install_path: install_path.to_string(),
            running,
            health_reachable: running,
            error,
        })
    }

    pub async fn update_remote_agent(
        ssh_target: &str,
        ssh_connection: Option<SshConnection>,
    ) -> Result<RemoteAgentUpdateResponse> {
        let connection = ssh_connection.unwrap_or_else(|| SshConnection::from_target(ssh_target));
        let latest_release = latest_release_info().await?;
        let remote_version = get_remote_version_with(connection.clone()).await?;
        let remote_os = detect_remote_os_with(&connection).await;
        let remote_arch = detect_remote_arch_with(&connection, remote_os).await;

        let matching_asset = latest_release
            .matching_asset(remote_os.as_str(), &remote_arch)
            .cloned();

        if matching_asset.is_none() {
            return Ok(RemoteAgentUpdateResponse {
                ok: false,
                ssh_target: connection.target_label(),
                previous_version: remote_version,
                new_version: Some(latest_release.tag_name),
                updated: false,
                error: Some("No matching asset for remote platform".to_string()),
            });
        }

        let install_path = default_install_path_for_os(remote_os);
        let install_path_clone = install_path.clone();
        let stop_response = stop_remote_agent(ssh_target, Some(connection.clone())).await?;

        if !stop_response.stopped {
            return Ok(RemoteAgentUpdateResponse {
                ok: false,
                ssh_target: connection.target_label(),
                previous_version: remote_version,
                new_version: Some(latest_release.tag_name),
                updated: false,
                error: Some("Failed to stop agent before update".to_string()),
            });
        }

        let install_response = install_remote_agent(
            ssh_target,
            Some(connection.clone()),
            &matching_asset.unwrap(),
            Some(install_path_clone),
            remote_os,
        )
        .await?;

        if !install_response.installed {
            return Ok(RemoteAgentUpdateResponse {
                ok: false,
                ssh_target: connection.target_label(),
                previous_version: remote_version,
                new_version: Some(latest_release.tag_name),
                updated: false,
                error: Some("Failed to install updated agent".to_string()),
            });
        }

        let start_response = start_remote_agent(
            ssh_target,
            Some(connection.clone()),
            &install_path,
            &default_start_command_for_os_with(&connection, remote_os, &install_path).await,
        )
        .await?;

        Ok(RemoteAgentUpdateResponse {
            ok: start_response.running,
            ssh_target: connection.target_label(),
            previous_version: remote_version,
            new_version: Some(latest_release.tag_name),
            updated: start_response.running,
            error: if start_response.running {
                None
            } else {
                Some("Agent failed to start after update".to_string())
            },
        })
    }

    pub async fn stop_remote_agent(
        ssh_target: &str,
        ssh_connection: Option<SshConnection>,
    ) -> Result<RemoteAgentStopResponse> {
        let connection = ssh_connection.unwrap_or_else(|| SshConnection::from_target(ssh_target));
        let os = detect_remote_os_with(&connection).await;
        let command = match os {
            RemoteOs::Windows => {
                "cmd.exe /C taskkill /IM llama-monitor.exe /F >NUL 2>NUL & exit /B 0"
            }
            RemoteOs::Unix | RemoteOs::Macos => "pkill -f llama-monitor >/dev/null 2>&1; true",
            RemoteOs::Unknown => return Err(io::Error::other("Unknown OS").into()),
        };

        let output = remote_ssh::exec(connection.clone(), command.to_string()).await?;

        Ok(RemoteAgentStopResponse {
            ok: output.status == 0,
            ssh_target: connection.target_label(),
            stopped: output.status == 0,
            error: if output.status == 0 {
                None
            } else {
                Some("Failed to stop agent".to_string())
            },
        })
    }

    pub async fn status_remote_agent(
        ssh_target: &str,
        ssh_connection: Option<SshConnection>,
    ) -> Result<RemoteAgentStatusResponse> {
        let connection = ssh_connection.unwrap_or_else(|| SshConnection::from_target(ssh_target));
        let os = detect_remote_os_with(&connection).await;
        if os == RemoteOs::Unknown {
            return Ok(RemoteAgentStatusResponse {
                ok: false,
                ssh_target: connection.target_label(),
                os: os.as_str().to_string(),
                install_path: String::new(),
                installed: false,
                running: false,
                health_reachable: false,
                installed_version: None,
                managed_task_name: None,
                managed_task_installed: false,
                managed_task_command: None,
                managed_task_matches: false,
                error: Some("Unknown remote OS".to_string()),
            });
        }

        let install_path = default_install_path_for_os(os);
        let installed = remote_file_exists_with(&connection, os, &install_path).await;
        let health_reachable =
            agent_health_reachable(&connection.agent_url(REMOTE_AGENT_DEFAULT_PORT)).await;
        let installed_version = if installed {
            get_remote_version_with(connection.clone())
                .await
                .ok()
                .flatten()
        } else {
            None
        };
        let managed_task = managed_task_status(&connection, os, Some(&install_path))
            .await
            .ok()
            .flatten();
        let managed_task_name = managed_task.as_ref().map(|task| task.name.clone());
        let managed_task_installed = managed_task.as_ref().is_some_and(|task| task.installed);
        let managed_task_command = managed_task.as_ref().and_then(|task| task.command.clone());
        let managed_task_matches = managed_task
            .as_ref()
            .is_some_and(|task| task.matches_install_path);

        Ok(RemoteAgentStatusResponse {
            ok: true,
            ssh_target: connection.target_label(),
            os: os.as_str().to_string(),
            install_path,
            installed,
            running: health_reachable,
            health_reachable,
            installed_version,
            managed_task_name,
            managed_task_installed,
            managed_task_command,
            managed_task_matches,
            error: None,
        })
    }

    pub async fn remove_remote_agent(
        ssh_target: &str,
        ssh_connection: Option<SshConnection>,
    ) -> Result<RemoteAgentRemoveResponse> {
        let connection = ssh_connection.unwrap_or_else(|| SshConnection::from_target(ssh_target));
        let os = detect_remote_os_with(&connection).await;
        let install_path = default_install_path_for_os(os);
        let command = match os {
            RemoteOs::Windows => format!(
                "cmd.exe /C taskkill /IM llama-monitor.exe /F >NUL 2>NUL & schtasks /Delete /TN \"{WINDOWS_AGENT_TASK_NAME}\" /F >NUL 2>NUL & schtasks /Delete /TN \"{WINDOWS_AGENT_LEGACY_TASK_NAME}\" /F >NUL 2>NUL & del /F /Q \"{install_path}\" >NUL 2>NUL & exit /B 0"
            ),
            RemoteOs::Unix | RemoteOs::Macos => {
                format!("pkill -f llama-monitor >/dev/null 2>&1; rm -f {install_path}")
            }
            RemoteOs::Unknown => return Err(io::Error::other("Unknown OS").into()),
        };

        let output = remote_ssh::exec(connection.clone(), command).await?;

        Ok(RemoteAgentRemoveResponse {
            ok: output.status == 0,
            ssh_target: connection.target_label(),
            removed: output.status == 0,
            error: if output.status == 0 {
                None
            } else {
                Some("Failed to remove managed agent".to_string())
            },
        })
    }

    pub async fn managed_task_status(
        connection: &SshConnection,
        os: RemoteOs,
        install_path: Option<&str>,
    ) -> Result<Option<ManagedTaskStatus>> {
        if os != RemoteOs::Windows {
            return Ok(None);
        }

        let output = remote_ssh::exec(
            connection.clone(),
            format!("cmd.exe /C schtasks /Query /TN \"{WINDOWS_AGENT_TASK_NAME}\" /V /FO LIST"),
        )
        .await?;

        if output.status != 0 {
            return Ok(Some(ManagedTaskStatus {
                name: WINDOWS_AGENT_TASK_NAME.to_string(),
                installed: false,
                command: None,
                matches_install_path: false,
            }));
        }

        let command = output
            .stdout
            .lines()
            .find_map(|line| line.trim().strip_prefix("Task To Run:").map(str::trim))
            .filter(|value| !value.is_empty() && *value != "N/A")
            .map(ToOwned::to_owned);
        let matches_install_path = command.as_deref().is_some_and(|command| {
            install_path.is_some_and(|path| {
                command
                    .to_ascii_lowercase()
                    .contains(&path.to_ascii_lowercase())
            })
        });

        Ok(Some(ManagedTaskStatus {
            name: WINDOWS_AGENT_TASK_NAME.to_string(),
            installed: true,
            command,
            matches_install_path,
        }))
    }

    pub async fn get_remote_version_with(connection: SshConnection) -> Result<Option<String>> {
        let remote_os = detect_remote_os_with(&connection).await;
        let install_path = default_install_path_for_os(remote_os);
        let command = match remote_os {
            RemoteOs::Windows => format!("cmd.exe /C \"\"{install_path}\" --version\""),
            RemoteOs::Unix | RemoteOs::Macos => format!("{install_path} --version"),
            RemoteOs::Unknown => return Ok(None),
        };
        let output = remote_ssh::exec(connection, command).await?;

        if output.status == 0 {
            Ok(Some(output.stdout.trim().to_string()))
        } else {
            Ok(None)
        }
    }

    pub async fn default_install_path_for_target(ssh_target: &str) -> String {
        default_install_path_for_os(detect_remote_os(ssh_target).await)
    }

    pub async fn default_start_command_for_target(ssh_target: &str, install_path: &str) -> String {
        default_start_command_for_os(detect_remote_os(ssh_target).await, install_path)
    }

    pub async fn detect_remote_os_simple(ssh_target: &str) -> RemoteOs {
        detect_remote_os(ssh_target).await
    }
}

pub use install::{
    RemoteAgentInstallRequest, default_install_path_for_target, default_start_command_for_target,
    detect_remote_os_simple, install_remote_agent, remove_remote_agent, start_remote_agent,
    status_remote_agent, stop_remote_agent, update_remote_agent,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_cli_and_release_versions() {
        assert_eq!(normalize_version_label("llama-monitor 0.5.1"), "0.5.1");
        assert_eq!(normalize_version_label("other-agent 0.5.1"), "0.5.1");
        assert_eq!(normalize_version_label("v0.5.1"), "0.5.1");
    }
}
