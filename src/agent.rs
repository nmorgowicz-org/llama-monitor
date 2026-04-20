use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use warp::Filter;
use warp::http::{HeaderMap, StatusCode};

use crate::config::AppConfig;
use crate::gpu::{self, GpuMetrics};
use crate::state::{AppState, EndpointKind, SessionMode};
use crate::system::{self, SystemMetrics};

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
    pub agent_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteAgentDetectResponse {
    pub ok: bool,
    pub ssh_target: String,
    pub os: String,
    pub arch: String,
    pub install_path: Option<String>,
    pub installed: bool,
    pub reachable: bool,
    pub latest_release: Option<LatestReleaseInfo>,
    pub matching_asset: Option<ReleaseAssetInfo>,
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
    let release = reqwest::Client::new()
        .get(GITHUB_LATEST_RELEASE_URL)
        .header(reqwest::header::USER_AGENT, "llama-monitor")
        .send()
        .await?
        .error_for_status()?
        .json::<GithubRelease>()
        .await?;

    Ok(LatestReleaseInfo {
        tag_name: release.tag_name,
        name: release.name,
        assets: release
            .assets
            .into_iter()
            .filter_map(asset_info_from_github_asset)
            .collect(),
    })
}

pub async fn detect_remote_agent(req: RemoteAgentDetectRequest) -> RemoteAgentDetectResponse {
    let ssh_target = req.ssh_target.trim().to_string();
    if ssh_target.is_empty() {
        return RemoteAgentDetectResponse {
            ok: false,
            ssh_target,
            os: "unknown".to_string(),
            arch: "unknown".to_string(),
            install_path: None,
            installed: false,
            reachable: false,
            latest_release: None,
            matching_asset: None,
            error: Some("Missing SSH target".to_string()),
        };
    }

    let remote_os = detect_remote_os(&ssh_target).await;
    let os = remote_os.as_str().to_string();
    let arch = detect_remote_arch(&ssh_target, remote_os).await;
    let install_path = install_path_for_os(remote_os).map(ToOwned::to_owned);
    let installed = if let Some(path) = install_path.as_deref() {
        remote_file_exists(&ssh_target, remote_os, path).await
    } else {
        false
    };
    let reachable = if let Some(agent_url) = req.agent_url.as_deref() {
        agent_health_reachable(agent_url).await
    } else {
        false
    };

    let latest_release = latest_release_info().await.ok();
    let matching_asset = latest_release
        .as_ref()
        .and_then(|release| release.matching_asset(&os, &arch).cloned());
    let error = if remote_os == RemoteOs::Unknown {
        Some("Could not detect remote OS over SSH".to_string())
    } else if matching_asset.is_none() {
        Some(format!("No release asset for {os} {arch}"))
    } else {
        None
    };

    RemoteAgentDetectResponse {
        ok: error.is_none(),
        ssh_target,
        os,
        arch,
        install_path,
        installed,
        reachable,
        latest_release,
        matching_asset,
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

    loop {
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

    let mut child = match Command::new("ssh").arg(&target).arg(&command).spawn() {
        Ok(child) => child,
        Err(e) => {
            eprintln!("[agent] Failed to launch ssh autostart command: {e}");
            return;
        }
    };

    match tokio::time::timeout(REMOTE_AGENT_AUTOSTART_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) if status.success() => {
            eprintln!("[agent] Remote agent autostart command completed");
        }
        Ok(Ok(status)) => {
            eprintln!("[agent] Remote agent autostart command exited with {status}");
        }
        Ok(Err(e)) => {
            eprintln!("[agent] Remote agent autostart command failed: {e}");
        }
        Err(_) => {
            let _ = child.kill().await;
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
    match detect_remote_os(target).await {
        RemoteOs::Windows => format!(
            "schtasks /Create /TN llama-monitor-agent /TR \"\\\"%APPDATA%\\llama-monitor\\bin\\llama-monitor.exe\\\" --agent --agent-host 0.0.0.0 --agent-port {REMOTE_AGENT_DEFAULT_PORT}\" /SC ONCE /ST 23:59 /F && schtasks /Run /TN llama-monitor-agent && schtasks /Delete /TN llama-monitor-agent /F"
        ),
        RemoteOs::Unix | RemoteOs::Macos => format!(
            "nohup ~/.config/llama-monitor/bin/llama-monitor --agent --agent-host 0.0.0.0 --agent-port {REMOTE_AGENT_DEFAULT_PORT} > ~/.config/llama-monitor/agent.log 2>&1 &"
        ),
        RemoteOs::Unknown => format!(
            "llama-monitor --agent --agent-host 0.0.0.0 --agent-port {REMOTE_AGENT_DEFAULT_PORT}"
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteOs {
    Windows,
    Unix,
    Macos,
    Unknown,
}

async fn detect_remote_os(target: &str) -> RemoteOs {
    let windows = Command::new("ssh")
        .arg(target)
        .args(["cmd.exe", "/C", "ver"])
        .output();

    if let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(5), windows).await
        && output.status.success()
        && String::from_utf8_lossy(&output.stdout).contains("Windows")
    {
        return RemoteOs::Windows;
    }

    let unix = Command::new("ssh").arg(target).arg("uname -s").output();
    if let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(5), unix).await
        && output.status.success()
    {
        let name = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
        if name.contains("darwin") {
            return RemoteOs::Macos;
        }
        return RemoteOs::Unix;
    }

    RemoteOs::Unknown
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

fn install_path_for_os(os: RemoteOs) -> Option<&'static str> {
    match os {
        RemoteOs::Windows => Some("%APPDATA%\\llama-monitor\\bin\\llama-monitor.exe"),
        RemoteOs::Unix | RemoteOs::Macos => Some("~/.config/llama-monitor/bin/llama-monitor"),
        RemoteOs::Unknown => None,
    }
}

async fn detect_remote_arch(target: &str, os: RemoteOs) -> String {
    let output = match os {
        RemoteOs::Windows => Command::new("ssh")
            .arg(target)
            .args(["cmd.exe", "/C", "echo", "%PROCESSOR_ARCHITECTURE%"])
            .output(),
        RemoteOs::Unix | RemoteOs::Macos => {
            Command::new("ssh").arg(target).arg("uname -m").output()
        }
        RemoteOs::Unknown => return "unknown".to_string(),
    };

    match tokio::time::timeout(Duration::from_secs(5), output).await {
        Ok(Ok(output)) if output.status.success() => {
            normalize_arch(String::from_utf8_lossy(&output.stdout).trim())
        }
        _ => "unknown".to_string(),
    }
}

async fn remote_file_exists(target: &str, os: RemoteOs, path: &str) -> bool {
    let output = match os {
        RemoteOs::Windows => Command::new("ssh")
            .arg(target)
            .args(["cmd.exe", "/C", "if", "exist", path, "(echo", "yes)"])
            .output(),
        RemoteOs::Unix | RemoteOs::Macos => Command::new("ssh")
            .arg(target)
            .arg(format!("test -x {path} && echo yes"))
            .output(),
        RemoteOs::Unknown => return false,
    };

    matches!(
        tokio::time::timeout(Duration::from_secs(5), output).await,
        Ok(Ok(output))
            if output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains("yes")
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
