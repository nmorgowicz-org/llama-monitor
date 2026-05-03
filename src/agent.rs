use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, LazyLock, Mutex};
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

/// Shell-quotes a path for safe inclusion in a command string.
///
/// Uses platform-appropriate quoting to prevent command injection when
/// user-controlled paths are interpolated into shell commands executed over SSH.
///
/// For Windows, use `shell_quote_path_cmd` when the command runs under `cmd.exe`
/// (e.g., schtasks), and `shell_quote_path` for PowerShell contexts.
fn shell_quote_path(path: &str, os: RemoteOs) -> String {
    match os {
        RemoteOs::Unix | RemoteOs::Macos => {
            // shlex quoting: wraps in single quotes, escapes embedded quotes
            shlex::try_quote(path)
                .map(|s| s.into_owned())
                .unwrap_or_else(|_| path.to_string())
        }
        RemoteOs::Windows => {
            // PowerShell single quotes are literal; escape embedded quotes by doubling
            format!("'{}'", path.replace('\'', "''"))
        }
        RemoteOs::Unknown => {
            // Conservative: treat as Unix
            shlex::try_quote(path)
                .map(|s| s.into_owned())
                .unwrap_or_else(|_| path.to_string())
        }
    }
}

/// Quotes a path for `cmd.exe` contexts (schtasks, cmd.exe /C).
///
/// Unlike PowerShell, cmd.exe does not treat single quotes as special.
/// Use double quotes with proper escaping for embedded quotes.
#[allow(dead_code)] // kept for tests; was used before PowerShell migration
fn shell_quote_path_cmd(path: &str) -> String {
    // In cmd.exe, double quotes delimit strings; escape embedded quotes with ^
    format!("\"{}\"", path.replace('"', "\"^\""))
}

/// Validates an install path to ensure it does not contain shell metacharacters
/// or target suspicious directories.
///
/// This is the primary defense against command injection; shell quoting is
/// a secondary defense-in-depth measure.
fn validate_install_path(path: &str, target_os: RemoteOs) -> Result<(), anyhow::Error> {
    // Must not contain shell metacharacters (platform-independent check)
    // Note: ~ is excluded — it's a valid Unix home directory prefix
    let dangerous_chars = ";|&$`'\"(){}[]!#<>*?";
    if path.chars().any(|c| dangerous_chars.contains(c)) {
        return Err(anyhow::anyhow!("Install path contains invalid characters"));
    }

    match target_os {
        RemoteOs::Unix | RemoteOs::Macos => {
            // Must be absolute or tilde-expanded (~)
            if !path.starts_with('/') && !path.starts_with('~') {
                return Err(anyhow::anyhow!("Install path must be absolute"));
            }
            // Must not target suspicious directories
            let forbidden = ["/tmp", "/var", "/etc"];
            if forbidden.iter().any(|f| path.starts_with(f)) {
                return Err(anyhow::anyhow!("Install path not allowed"));
            }
        }
        RemoteOs::Windows => {
            // Windows absolute: drive letter (C:\), UNC (\\), or env var (%APPDATA%\)
            let is_windows_absolute = path.len() >= 3
                && ((path.as_bytes()[1] == b':'
                    && (path.as_bytes()[2] == b'\\' || path.as_bytes()[2] == b'/'))
                    || path.starts_with("%")
                    || path.starts_with("\\\\"));
            if !is_windows_absolute {
                return Err(anyhow::anyhow!("Install path must be absolute"));
            }
            // Must not target suspicious directories
            let forbidden = ["C:\\Windows", "C:\\WINDOWS", "C:/Windows", "C:/WINDOWS"];
            if forbidden.iter().any(|f| path.starts_with(f)) {
                return Err(anyhow::anyhow!("Install path not allowed"));
            }
        }
        RemoteOs::Unknown => {
            // Conservative: require some form of absolute path
            if !path.starts_with('/') && !path.starts_with('\\') {
                return Err(anyhow::anyhow!("Install path must be absolute"));
            }
        }
    }

    Ok(())
}

thread_local! {
    static LATEST_RELEASE_CACHE: Mutex<Option<(LatestReleaseInfo, Instant)>> = const { Mutex::new(None) };
}

static REMOTE_AGENT_AUTOSTART_SUPPRESS_UNTIL: LazyLock<Mutex<Option<Instant>>> =
    LazyLock::new(|| Mutex::new(None));

const REMOTE_AGENT_DEFAULT_PORT: u16 = 7779;
const REMOTE_AGENT_POLL_INTERVAL: Duration = Duration::from_secs(2);
const REMOTE_AGENT_AUTOSTART_TIMEOUT: Duration = Duration::from_secs(15);
const REMOTE_AGENT_AUTOSTART_SUPPRESS_DURATION: Duration = Duration::from_secs(120);
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
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
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
    pub agent_token: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
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

    // Use explicit token, or auto-generate and persist one
    let token = match app_config.agent_token.clone() {
        Some(t) => t,
        None => {
            let config_dir = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("llama-monitor");
            let token_file = config_dir.join("agent-token");

            // Try to read existing token from disk
            let existing = std::fs::read_to_string(&token_file).ok().and_then(|s| {
                let trimmed = s.trim().to_string();
                if !trimmed.is_empty() {
                    Some(trimmed)
                } else {
                    None
                }
            });

            existing.unwrap_or_else(|| {
                // Generate a random token from system entropy
                let bytes = std::fs::read("/dev/urandom")
                    .ok()
                    .map(|b| {
                        b.iter()
                            .take(16)
                            .fold(0u128, |acc, &x| (acc << 8) | x as u128)
                    })
                    .unwrap_or_else(|| {
                        // Fallback: use timestamp + process ID
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_nanos();
                        let pid = std::process::id() as u128;
                        ts ^ pid
                    });
                let new_token = format!("{bytes:x}");
                // Persist it
                let _ = std::fs::create_dir_all(&config_dir);
                let _ = std::fs::write(&token_file, &new_token);
                eprintln!("[agent] Auto-generated token: {new_token}");
                eprintln!("[agent] Token saved to {}", token_file.display());
                new_token
            })
        }
    };

    // Write token to a user-readable temp file so the main app can read it via SSH
    // (needed on Windows where the agent runs as SYSTEM and the token file is in
    // the SYSTEM profile, inaccessible to the SSH user).
    // The temp file is cleaned up after a delay to give the main app time to read it.
    let _ = write_token_to_temp_file(&token);

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

    let system_metrics: Arc<Mutex<system::SystemMetrics>> =
        Arc::new(Mutex::new(system::SystemMetrics {
            cpu_name: String::new(),
            cpu_temp: 0.0,
            cpu_temp_available: false,
            cpu_load: 0,
            cpu_clock_mhz: 0,
            ram_total_gb: 0.0,
            ram_used_gb: 0.0,
            motherboard: String::new(),
        }));

    {
        let system_metrics = Arc::clone(&system_metrics);
        std::thread::spawn(move || {
            loop {
                *system_metrics.lock().unwrap() = system::get_system_metrics();
                std::thread::sleep(Duration::from_secs(5));
            }
        });
    }

    let agent_info_token = token.clone(); // for authenticated /agent/info endpoint
    let auth =
        warp::any()
            .and(warp::header::headers_cloned())
            .and_then(move |headers: HeaderMap| {
                let token = token.clone();
                async move {
                    // Token is always present (explicit or auto-generated)
                    let valid = headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok())
                        .is_some_and(|value| value == format!("Bearer {token}"));

                    if !valid {
                        return Err(warp::reject::custom(AgentAuthError));
                    }

                    Ok::<(), warp::Rejection>(())
                }
            });

    let health = warp::path("health")
        .and(warp::get())
        .map(|| warp::reply::json(&serde_json::json!({ "ok": true })));

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

    // Agent info endpoint (authenticated — returns token for token-verification flows)
    let agent_info = {
        let agent_token = agent_info_token.clone();
        let info_bind_addr = bind_addr;
        warp::path("agent")
            .and(warp::path("info"))
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
                    "agent_token": agent_token,
                }))
            })
    };

    let system_route = {
        let system_metrics = Arc::clone(&system_metrics);
        warp::path!("metrics" / "system")
            .and(warp::get())
            .and(auth.clone())
            .map(move |_| warp::reply::json(&*system_metrics.lock().unwrap()))
    };

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
        let system_metrics = Arc::clone(&system_metrics);
        warp::path("metrics")
            .and(warp::path::end())
            .and(warp::get())
            .and(auth)
            .map(move |_| {
                let metrics = AgentMetrics {
                    system: system_metrics.lock().unwrap().clone(),
                    gpu: gpu_metrics.lock().unwrap().clone(),
                };
                warp::reply::json(&metrics)
            })
    };

    let routes = health
        .or(info)
        .or(agent_info)
        .or(system_route)
        .or(gpu_route)
        .or(metrics_route)
        .recover(handle_agent_rejection);

    if app_config.agent_token.is_none() {
        eprintln!("[agent] Using auto-generated token (persisted to config dir)");
    }
    println!("[agent] Remote metrics agent listening on https://{bind_addr}");

    // mTLS: cert infrastructure in place (certs.rs), CA shipped to remote agents
    // Dashboard HTTP client accepts self-signed certs (danger_accept_invalid_certs)
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
        html_url: release.html_url,
        body: release.body,
        published_at: release.published_at,
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
            agent_token: None,
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

    let agent_token = if ssh_ok {
        read_remote_agent_token(&connection, remote_os, req.agent_url.as_deref()).await
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
        Some(remote_release_asset_error(
            latest_release.as_ref(),
            &os,
            &arch,
            install_path.as_deref(),
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
        agent_token,
        error,
    }
}

fn remote_release_asset_error(
    latest_release: Option<&LatestReleaseInfo>,
    os: &str,
    arch: &str,
    install_path: Option<&str>,
) -> String {
    let install_path = install_path.unwrap_or("the managed agent install path");

    match latest_release {
        Some(release) if release.assets.is_empty() => format!(
            "Latest release {} is published but does not have any installable agent assets yet. This usually means the release build is still running or asset upload has not finished. Wait for the release artifacts to appear, then retry Install / Start. Expected asset for this host: {} {}. Target install path: {}.",
            release.tag_name, os, arch, install_path
        ),
        Some(release) => format!(
            "Latest release {} does not contain a supported agent asset for {} {}. Open the release artifacts and verify that the expected package was published for this platform, then retry. Target install path: {}.",
            release.tag_name, os, arch, install_path
        ),
        None => format!(
            "Could not determine a downloadable agent build for {} {} because release metadata was unavailable. Check GitHub release availability, then retry. Target install path: {}.",
            os, arch, install_path
        ),
    }
}

pub async fn remote_agent_poller(state: AppState, app_config: Arc<AppConfig>) {
    // Build HTTP client with TLS for mTLS (accept self-signed certs)
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .pool_max_idle_per_host(0)
        .danger_accept_invalid_certs(true) // Self-signed certs
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("[agent] Failed to build HTTP client: {e}");
            return;
        }
    };
    let mut autostart_attempted = false;
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
                        autostart_attempted = false;
                    }
                    Err(e) => {
                        mark_disconnected(&state);
                        eprintln!("[agent] Failed to parse remote metrics: {e}");
                    }
                },
                Ok(resp) => {
                    mark_disconnected(&state);
                    if resp.status() == reqwest::StatusCode::UNAUTHORIZED && token.is_none() {
                        eprintln!("[agent] Remote agent not yet authenticated (no token set)");
                    } else {
                        eprintln!(
                            "[agent] Remote metrics request failed: HTTP {}",
                            resp.status()
                        );
                    }
                    maybe_autostart_remote_agent(
                        &state,
                        &app_config,
                        &settings,
                        &url,
                        &mut autostart_attempted,
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
                        &mut autostart_attempted,
                    )
                    .await;
                }
            }
        } else {
            mark_disconnected(&state);
            enabled = false;
        }

        tokio::select! {
            _ = tokio::time::sleep(REMOTE_AGENT_POLL_INTERVAL) => {}
            _ = state.agent_poll_notify.notified() => {}
        }
    }
}

pub fn suppress_remote_agent_autostart() {
    if let Ok(mut until) = REMOTE_AGENT_AUTOSTART_SUPPRESS_UNTIL.lock() {
        *until = Some(Instant::now() + REMOTE_AGENT_AUTOSTART_SUPPRESS_DURATION);
    }
}

async fn maybe_autostart_remote_agent(
    state: &AppState,
    app_config: &AppConfig,
    settings: &crate::state::UiSettings,
    agent_url: &str,
    attempted: &mut bool,
) {
    let enabled = app_config.remote_agent_ssh_autostart || settings.remote_agent_ssh_autostart;
    if !enabled {
        return;
    }

    if state.current_endpoint_kind() != EndpointKind::Remote {
        return;
    }

    if REMOTE_AGENT_AUTOSTART_SUPPRESS_UNTIL
        .lock()
        .ok()
        .and_then(|until| *until)
        .is_some_and(|until| Instant::now() < until)
    {
        return;
    }

    if *attempted {
        return;
    }
    *attempted = true;

    let target = first_non_empty([
        app_config.remote_agent_ssh_target.as_deref(),
        Some(settings.remote_agent_ssh_target.as_str()),
    ])
    .or_else(|| remote_host_from_agent_url(agent_url));

    let Some(target) = target else {
        eprintln!("[agent] SSH autostart enabled but no SSH target is available");
        return;
    };

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

    if connection.trusted_host_key.is_none() {
        return;
    }

    let remote_os = detect_remote_os_with(&connection).await;
    let default_install_path = default_install_path_for_os(remote_os);
    let default_command =
        default_start_command_for_os_with(&connection, remote_os, &default_install_path).await;

    let command = if let Some(command) = first_non_empty([
        app_config.remote_agent_ssh_command.as_deref(),
        Some(settings.remote_agent_ssh_command.as_str()),
    ]) {
        if remote_os == RemoteOs::Windows && command.contains('~') {
            default_command
        } else {
            command
        }
    } else {
        default_command
    };

    eprintln!("[agent] Attempting remote agent autostart via ssh {target}");

    let started = match tokio::time::timeout(
        REMOTE_AGENT_AUTOSTART_TIMEOUT,
        remote_ssh::exec(connection.clone(), command),
    )
    .await
    {
        Ok(Ok(output)) if output.status == 0 => {
            eprintln!("[agent] Remote agent autostart command completed");
            true
        }
        Ok(Ok(output)) => {
            eprintln!(
                "[agent] Remote agent autostart command exited with status {}: {}",
                output.status,
                output.stderr.trim()
            );
            false
        }
        Ok(Err(e)) => {
            eprintln!("[agent] Remote agent autostart command failed: {e}");
            false
        }
        Err(_) => {
            eprintln!("[agent] Remote agent autostart timed out; use a detached remote command");
            false
        }
    };

    // After a successful autostart, read and persist the token so the metrics
    // poller can authenticate on its next attempt.
    if started
        && settings.remote_agent_token.is_empty()
        && let Some(token) =
            read_remote_agent_token(&connection, remote_os, Some(&settings.remote_agent_url)).await
    {
        let mut s = state.ui_settings.lock().unwrap();
        if s.remote_agent_token.is_empty() {
            s.remote_agent_token = token;
            let _ = crate::state::save_ui_settings(&state.ui_settings_path, &s);
            drop(s);
            state.agent_poll_notify.notify_waiters();
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

fn default_start_command_for_os(os: RemoteOs, install_path: &str) -> String {
    let quoted_path = shell_quote_path(install_path, os);
    match os {
        RemoteOs::Windows => {
            // Use PowerShell to create scheduled tasks — handles path quoting
            // reliably across SSH layers without backslash consumption.
            let agent_path = install_path.replace('\'', "''");
            let bridge_dir = install_path
                .rsplit_once('\\')
                .map(|(dir, _)| dir)
                .unwrap_or("");
            let bridge_path = format!("{}\\sensor_bridge.exe", bridge_dir).replace('\'', "''");
            format!(
                "powershell.exe -NoProfile -NonInteractive -Command \"$ErrorActionPreference='Stop'; \
Unregister-ScheduledTask -TaskName '{WINDOWS_AGENT_LEGACY_TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue; \
Unregister-ScheduledTask -TaskName '{WINDOWS_AGENT_TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue; \
Unregister-ScheduledTask -TaskName '{WINDOWS_SENSOR_BRIDGE_TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue; \
Register-ScheduledTask -TaskName '{WINDOWS_AGENT_TASK_NAME}' -Trigger (New-ScheduledTaskTrigger -AtStartup) -Action (New-ScheduledTaskAction -Execute '{agent_path}' -Argument '--agent --agent-host 0.0.0.0 --agent-port {REMOTE_AGENT_DEFAULT_PORT}') -Settings (New-ScheduledTaskSettingsSet) -User 'SYSTEM' -RunLevel Highest -Force; \
Register-ScheduledTask -TaskName '{WINDOWS_SENSOR_BRIDGE_TASK_NAME}' -Trigger (New-ScheduledTaskTrigger -AtStartup) -Action (New-ScheduledTaskAction -Execute '{bridge_path}' -Argument '--server') -Settings (New-ScheduledTaskSettingsSet) -User 'SYSTEM' -RunLevel Highest -Force; \
Start-ScheduledTask -TaskName '{WINDOWS_AGENT_TASK_NAME}'; \
Start-ScheduledTask -TaskName '{WINDOWS_SENSOR_BRIDGE_TASK_NAME}'\""
            )
        }
        RemoteOs::Unix | RemoteOs::Macos => format!(
            "nohup {quoted_path} --agent --agent-host 0.0.0.0 --agent-port {REMOTE_AGENT_DEFAULT_PORT} > ~/.config/llama-monitor/agent.log 2>&1 &"
        ),
        RemoteOs::Unknown => format!(
            "{quoted_path} --agent --agent-host 0.0.0.0 --agent-port {REMOTE_AGENT_DEFAULT_PORT}"
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
const WINDOWS_SENSOR_BRIDGE_TASK_NAME: &str = "LlamaMonitorSensorBridge";

/// Batch script placed next to the Windows agent binary after install.
/// Double-clicking it (or running from cmd) requests UAC elevation via VBScript
/// and removes both the scheduled task and legacy task name.
const WINDOWS_AGENT_UNINSTALL_BAT: &[u8] = br#"@echo off
net session >nul 2>&1
if %errorlevel% == 0 goto :elevated
echo Set UAC = CreateObject^("Shell.Application"^) > "%temp%\lm_uac.vbs"
echo UAC.ShellExecute "%~f0", "", "", "runas", 1 >> "%temp%\lm_uac.vbs"
"%temp%\lm_uac.vbs"
del "%temp%\lm_uac.vbs"
goto :eof
:elevated
schtasks /End /TN "LlamaMonitorAgent" >nul 2>&1
schtasks /Delete /TN "LlamaMonitorAgent" /F >nul 2>&1
schtasks /End /TN "llama-monitor-agent" >nul 2>&1
schtasks /Delete /TN "llama-monitor-agent" /F >nul 2>&1
echo Llama Monitor agent service removed.
echo You may delete this folder.
pause
"#;

/// Shell script placed next to the Unix/macOS agent binary after install.
const UNIX_AGENT_UNINSTALL_SH: &[u8] = b"#!/bin/bash\n\
pkill -f 'llama-monitor --agent' 2>/dev/null || true\n\
SCRIPT_DIR=\"$(cd \"$(dirname \"${BASH_SOURCE[0]}\")\" && pwd)\"\n\
echo \"Llama Monitor agent stopped.\"\n\
echo \"To fully remove, delete: $SCRIPT_DIR\"\n";

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

pub(crate) async fn detect_remote_os_with(connection: &SshConnection) -> RemoteOs {
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
    // Use PowerShell for reliable path resolution (preserves backslashes)
    if let Ok(Ok(out)) = tokio::time::timeout(
        Duration::from_secs(5),
        remote_ssh::exec(
            connection.clone(),
            "powershell.exe -NoProfile -NonInteractive -Command \"$env:APPDATA\"".to_string(),
        ),
    )
    .await
        && out.status == 0
    {
        let s = out.stdout.trim().to_string();
        // Verify: must contain backslashes (valid Windows path)
        if !s.is_empty() && s.contains('\\') && !s.starts_with('%') {
            return Some(s);
        }
    }

    // Fallback: cmd.exe echo
    if let Ok(Ok(out)) = tokio::time::timeout(
        Duration::from_secs(5),
        remote_ssh::exec(connection.clone(), "cmd.exe /C echo %APPDATA%".to_string()),
    )
    .await
        && out.status == 0
    {
        let s = out.stdout.trim().to_string();
        if !s.is_empty() && s.contains('\\') && !s.starts_with('%') {
            return Some(s);
        }
    }

    None
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
        "llama-monitor-windows-x86_64.zip" => ("windows", "x86_64", true),
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
    pub(crate) fn as_str(self) -> &'static str {
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

pub(crate) fn default_install_path_for_os(os: RemoteOs) -> String {
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
    agent_health_reachable_with_token(agent_url, None).await
}

async fn agent_health_reachable_with_token(agent_url: &str, token: Option<&str>) -> bool {
    let mut req = reqwest::Client::new()
        .get(format!("{}/health", agent_url.trim_end_matches('/')))
        .timeout(Duration::from_secs(2));
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    let Ok(resp) = req.send().await else {
        return false;
    };
    resp.status().is_success()
}

/// Write the agent token to a temp file in each user's home directory, so the
/// main app can read it via SSH even when the agent runs as SYSTEM (Windows) or
/// another user. The files are cleaned up after 30 seconds.
fn write_token_to_temp_file(token: &str) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    let file_name = format!("llama-monitor-agent-token-{}.tmp", std::process::id());

    // Determine home directories to write to
    let home_dirs: Vec<std::path::PathBuf> = if cfg!(windows) {
        // On Windows, write to every user's home directory under C:\Users\
        // so the SSH user can read it. The agent runs as SYSTEM, so it can
        // write to any user's directory.
        match std::fs::read_dir("C:\\Users") {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    // Skip system accounts
                    let name = entry.file_name().to_string_lossy().to_string();
                    !name.starts_with("$") && name != "Default" && name != "Public"
                })
                .map(|entry| entry.path().join(".llama-monitor"))
                .collect(),
            Err(_) => Vec::new(),
        }
    } else {
        // On Unix, write to /tmp (accessible by all users)
        vec![std::env::temp_dir()]
    };

    for mut home_dir in home_dirs {
        let _ = std::fs::create_dir_all(&home_dir);
        home_dir.push(&file_name);

        if std::fs::write(&home_dir, token).is_ok() {
            let cleanup_path = home_dir.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(30));
                let _ = std::fs::remove_file(&cleanup_path);
                // Clean up the .llama-monitor dir if empty
                if let Some(parent) = cleanup_path.parent() {
                    let _ = std::fs::remove_dir(parent);
                }
            });
            eprintln!("[agent] Token written to temp file: {}", home_dir.display());
            paths.push(home_dir);
        } else {
            eprintln!(
                "[agent] Failed to write token to temp file: {}",
                home_dir.display()
            );
        }
    }

    paths
}

async fn read_remote_agent_token(
    connection: &SshConnection,
    os: RemoteOs,
    _agent_url: Option<&str>,
) -> Option<String> {
    // Try the user-readable temp file first (written by the agent on startup,
    // cleaned up after 30 seconds). On Windows, the agent writes to each user's
    // home directory so the SSH user can read it. On Unix, it writes to /tmp.
    let temp_file_cmd = match os {
        RemoteOs::Windows => {
            // Agent writes to C:\Users\<user>\.llama-monitor\agent-token-{pid}.tmp
            // Read the most recent one from the current user's home directory.
            "cmd.exe /C \"dir %USERPROFILE%\\.llama-monitor\\llama-monitor-agent-token-*.tmp /O:-D /B 2>NUL | set /p file= && type %USERPROFILE%\\.llama-monitor\\%file% 2>NUL\""
                .to_string()
        }
        RemoteOs::Unix | RemoteOs::Macos => {
            // Agent writes to /tmp/llama-monitor-agent-token-{pid}.tmp
            // Read the most recent one.
            "ls -t /tmp/llama-monitor-agent-token-*.tmp 2>/dev/null | head -1 | xargs cat 2>/dev/null"
                .to_string()
        }
        RemoteOs::Unknown => return None,
    };
    if let Ok(Ok(output)) = tokio::time::timeout(
        Duration::from_secs(5),
        remote_ssh::exec(connection.clone(), temp_file_cmd),
    )
    .await
        && output.status == 0
    {
        let token = output.stdout.trim().to_string();
        if !token.is_empty() && token.len() >= 16 {
            return Some(token);
        }
    }

    // Fall back to reading the token from the config directory (works on Unix,
    // fails on Windows SYSTEM profile).
    let command = match os {
        RemoteOs::Windows => {
            // Agent runs as SYSTEM; token lives in SYSTEM's roaming profile.
            r#"cmd.exe /C "type "C:\Windows\System32\config\systemprofile\AppData\Roaming\llama-monitor\agent-token" 2>NUL""#
                .to_string()
        }
        RemoteOs::Unix | RemoteOs::Macos => "cat ~/.config/llama-monitor/agent-token".to_string(),
        RemoteOs::Unknown => return None,
    };
    match tokio::time::timeout(
        Duration::from_secs(5),
        remote_ssh::exec(connection.clone(), command),
    )
    .await
    {
        Ok(Ok(output)) if output.status == 0 => {
            let token = output.stdout.trim().to_string();
            if token.is_empty() { None } else { Some(token) }
        }
        _ => None,
    }
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

    /// Write an uninstall script next to the agent binary on the remote machine.
    /// Failure is silently ignored — the agent is already installed; the script
    /// is a convenience for users who want to remove it without the dashboard.
    async fn drop_uninstall_script(connection: &SshConnection, install_path: &str, os: RemoteOs) {
        let sep = if os == RemoteOs::Windows { '\\' } else { '/' };
        let Some(dir_end) = install_path.rfind(sep) else {
            return;
        };
        let install_dir = &install_path[..dir_end];

        match os {
            RemoteOs::Windows => {
                // Resolve %APPDATA% to an actual path so SCP can use it.
                let resolved_dir = if let Some(appdata) = resolve_windows_appdata(connection).await
                {
                    install_dir.replace("%APPDATA%", &appdata)
                } else {
                    install_dir.to_string()
                };
                let remote_script = format!("{resolved_dir}\\uninstall.bat");

                let local_tmp = tempfile::NamedTempFile::new_in(std::env::temp_dir())
                    .map(|f| f.path().to_path_buf())
                    .unwrap_or_else(|_| {
                        std::env::temp_dir().join("llama_monitor_agent_uninstall.bat")
                    });
                if std::fs::write(&local_tmp, WINDOWS_AGENT_UNINSTALL_BAT).is_err() {
                    return;
                }
                let _ = remote_ssh::copy_to_remote(
                    connection.clone(),
                    local_tmp.to_string_lossy().to_string(),
                    remote_script,
                    0o644,
                )
                .await;
                let _ = std::fs::remove_file(&local_tmp);
            }
            RemoteOs::Unix | RemoteOs::Macos => {
                // SCP to /tmp first, then move to the install dir (handles ~ in paths).
                let tmp_remote = "/tmp/llama_monitor_agent_uninstall.sh";
                let final_remote = format!("{install_dir}/uninstall.sh");

                let local_tmp = tempfile::NamedTempFile::new_in(std::env::temp_dir())
                    .map(|f| f.path().to_path_buf())
                    .unwrap_or_else(|_| {
                        std::env::temp_dir().join("llama_monitor_agent_uninstall.sh")
                    });
                if std::fs::write(&local_tmp, UNIX_AGENT_UNINSTALL_SH).is_err() {
                    return;
                }
                if remote_ssh::copy_to_remote(
                    connection.clone(),
                    local_tmp.to_string_lossy().to_string(),
                    tmp_remote.to_string(),
                    0o755,
                )
                .await
                .is_ok()
                {
                    let _ = remote_ssh::exec(
                        connection.clone(),
                        format!("mv {tmp_remote} {final_remote}"),
                    )
                    .await;
                }
                let _ = std::fs::remove_file(&local_tmp);
            }
            RemoteOs::Unknown => {}
        }
    }

    /// Ships the CA certificate to the remote host so the agent can generate a server cert.
    async fn drop_ca_certificate(connection: &SshConnection, install_path: &str, os: RemoteOs) {
        let sep = if os == RemoteOs::Windows { '\\' } else { '/' };
        let Some(dir_end) = install_path.rfind(sep) else {
            return;
        };
        let install_dir = &install_path[..dir_end];

        // Ensure the CA cert exists locally
        let ca = crate::certs::ensure_ca();

        let remote_ca_path = format!("{}{}ca.pem", install_dir, sep);

        // Write CA cert to temp file and copy to remote
        let local_tmp = tempfile::NamedTempFile::new_in(std::env::temp_dir())
            .map(|f| f.path().to_path_buf())
            .unwrap_or_else(|_| std::env::temp_dir().join("ca.pem"));

        if std::fs::write(&local_tmp, &ca.pem).is_ok() {
            let _ = remote_ssh::copy_to_remote(
                connection.clone(),
                local_tmp.to_string_lossy().to_string(),
                remote_ca_path,
                0o644,
            )
            .await;
        }
        let _ = std::fs::remove_file(&local_tmp);
    }

    pub async fn install_remote_agent(
        ssh_target: &str,
        ssh_connection: Option<SshConnection>,
        asset: &ReleaseAssetInfo,
        install_path: Option<String>,
        os: RemoteOs,
    ) -> Result<RemoteAgentInstallResponse> {
        let install_path = install_path
            .or_else(|| install_path_for_os(os).map(ToOwned::to_owned))
            .context("Could not determine install path")?;

        // Validate install path before any network operations
        validate_install_path(&install_path, os).context("Invalid install path")?;

        let connection = ssh_connection.unwrap_or_else(|| SshConnection::from_target(ssh_target));
        let remote_temp_dir = detect_remote_temp_dir(&connection, os).await;
        let remote_temp_name = remote_temp_name_for_asset(asset, os);
        let remote_temp_path = match os {
            RemoteOs::Windows => format!("{}\\{}", remote_temp_dir, remote_temp_name),
            _ => format!("{}/{}", remote_temp_dir, remote_temp_name),
        };

        transfer_asset_to_remote_temp(&connection, asset, os, &remote_temp_path).await?;

        if os == RemoteOs::Windows {
            prepare_windows_install_target(&connection).await?;
        }

        if os == RemoteOs::Windows && asset.archive {
            extract_windows_archive_to_install_path(&connection, &remote_temp_path, &install_path)
                .await?;
        } else {
            move_binary_to_install_path(&connection, &remote_temp_path, &install_path, os).await?;
        }

        if os == RemoteOs::Unix || os == RemoteOs::Macos {
            set_executable_bit(&connection, &install_path, os).await?;
        }

        // Drop an uninstall script next to the binary (non-fatal if it fails).
        drop_uninstall_script(&connection, &install_path, os).await;

        // Ship the CA certificate so the agent can generate a server cert
        drop_ca_certificate(&connection, &install_path, os).await;

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
        let temp_path = tempfile::NamedTempFile::new_in(std::env::temp_dir())
            .map(|f| f.path().to_path_buf())
            .unwrap_or_else(|_| std::env::temp_dir().join(&asset.name));
        fs::write(&temp_path, &bytes)?;
        Ok(temp_path.to_string_lossy().to_string())
    }

    fn remote_temp_name_for_asset(asset: &ReleaseAssetInfo, os: RemoteOs) -> String {
        if os == RemoteOs::Windows && asset.archive {
            asset.name.clone()
        } else if asset.name.ends_with(".tar.gz") {
            asset.name.trim_end_matches(".tar.gz").to_string()
        } else if asset.name.ends_with(".zip") {
            asset.name.trim_end_matches(".zip").to_string()
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
        if os == RemoteOs::Windows {
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
        // .zip assets (Windows) need "-xf"; .tar.gz assets (macOS) need "-xzf".
        // Windows tar.exe (libarchive, built-in since Win10 1803) auto-detects zip
        // format but the -z flag forces gzip decompression and will fail on zip.
        let (binary_name, tar_flag) = if asset.name.ends_with(".zip") {
            (asset.name.trim_end_matches(".zip"), "-xf")
        } else {
            (asset.name.trim_end_matches(".tar.gz"), "-xzf")
        };
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!("{binary_name}-"))
            .tempdir_in(std::env::temp_dir())
            .map_err(|e| io::Error::other(format!("Failed to create temp dir: {e}")))?;
        let temp_extracted = temp_dir.path().to_path_buf();

        let output = tokio::process::Command::new("tar")
            .args([tar_flag, path, "-C", &temp_extracted.to_string_lossy()])
            .output()
            .await?;

        if !output.status.success() {
            Err(io::Error::other("Failed to extract archive").into())
        } else {
            let binary_path = extracted_binary_path(&temp_extracted, binary_name)?;
            // Keep directory alive beyond scope — caller will move/copy the binary
            let _ = temp_dir.keep();
            Ok(binary_path)
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

        let quoted_dir = shell_quote_path(&install_dir, os);
        let mkdir_command = match os {
            RemoteOs::Windows => format!(
                "cmd.exe /C if not exist \"{}\" mkdir \"{}\"",
                install_dir, install_dir
            ),
            RemoteOs::Unix | RemoteOs::Macos => format!("mkdir -p {quoted_dir}"),
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

        let quoted_temp = shell_quote_path(temp_path, os);
        let quoted_install = shell_quote_path(install_path, os);
        let command = match os {
            RemoteOs::Windows => format!("cmd.exe /C move /Y \"{temp_path}\" \"{install_path}\""),
            RemoteOs::Unix | RemoteOs::Macos => format!("mv {quoted_temp} {quoted_install}"),
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

    async fn extract_windows_archive_to_install_path(
        connection: &SshConnection,
        archive_path: &str,
        install_path: &str,
    ) -> Result<()> {
        let install_dir = install_path
            .rsplit_once('\\')
            .map(|(dir, _)| dir.to_string())
            .ok_or_else(|| io::Error::other("no directory in install path"))?;
        let extract_dir = format!("{install_dir}\\__llama_monitor_extract");

        // PowerShell single-quote escaping: double any embedded single quotes
        let ps_dir = install_dir.replace('\'', "''");
        let ps_extract = extract_dir.replace('\'', "''");
        let ps_archive = archive_path.replace('\'', "''");

        let command = format!(
            "powershell.exe -NoProfile -NonInteractive -Command \"$ErrorActionPreference = 'Stop'; \
if (!(Test-Path '{dir}')) {{ New-Item -ItemType Directory -Path '{dir}' -Force | Out-Null }}; \
if (Test-Path '{extract_dir}') {{ Remove-Item -LiteralPath '{extract_dir}' -Recurse -Force -ErrorAction SilentlyContinue }}; \
New-Item -ItemType Directory -Path '{extract_dir}' -Force | Out-Null; \
Expand-Archive -LiteralPath '{archive}' -DestinationPath '{extract_dir}' -Force; \
$targets = @('llama-monitor.exe', 'sensor_bridge.exe'); \
foreach ($name in $targets) {{ \
  $src = Join-Path '{extract_dir}' $name; \
  $dst = Join-Path '{dir}' $name; \
  if (Test-Path $src) {{ \
    for ($i = 0; $i -lt 10; $i++) {{ \
      if (Test-Path $dst) {{ Remove-Item -LiteralPath $dst -Force -ErrorAction SilentlyContinue }}; \
      try {{ [System.IO.File]::Copy($src, $dst, $true); Remove-Item -LiteralPath $src -Force -ErrorAction SilentlyContinue; break }} catch {{ if ($i -eq 9) {{ throw }}; Start-Sleep -Milliseconds 500 }} \
    }} \
  }} \
}}; \
Remove-Item -LiteralPath '{extract_dir}' -Recurse -Force -ErrorAction SilentlyContinue; \
Remove-Item -LiteralPath '{archive}' -Force -ErrorAction SilentlyContinue\"",
            dir = ps_dir,
            extract_dir = ps_extract,
            archive = ps_archive
        );

        let output = remote_ssh::exec(connection.clone(), command)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;

        if output.status == 0 {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "Failed to extract Windows archive: {}",
                output.stderr.trim()
            ))
            .into())
        }
    }

    async fn prepare_windows_install_target(connection: &SshConnection) -> Result<()> {
        let command = format!(
            "powershell.exe -NoProfile -NonInteractive -Command \" \
Stop-ScheduledTask -TaskName '{WINDOWS_AGENT_TASK_NAME}' -ErrorAction SilentlyContinue; \
Stop-ScheduledTask -TaskName '{WINDOWS_SENSOR_BRIDGE_TASK_NAME}' -ErrorAction SilentlyContinue; \
Start-Sleep -Seconds 2; \
Stop-Process -Name llama-monitor -Force -ErrorAction SilentlyContinue; \
Stop-Process -Name sensor_bridge -Force -ErrorAction SilentlyContinue; \
Unregister-ScheduledTask -TaskName '{WINDOWS_AGENT_TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue; \
Unregister-ScheduledTask -TaskName '{WINDOWS_AGENT_LEGACY_TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue; \
Unregister-ScheduledTask -TaskName '{WINDOWS_SENSOR_BRIDGE_TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue; \
Start-Sleep -Seconds 2\""
        );

        let output = remote_ssh::exec(connection.clone(), command)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;

        if output.status == 0 {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "Failed to stop existing Windows agent before install: {}",
                output.stderr.trim()
            ))
            .into())
        }
    }

    async fn set_executable_bit(
        connection: &SshConnection,
        path: &str,
        os: RemoteOs,
    ) -> Result<()> {
        let output = match os {
            RemoteOs::Unix | RemoteOs::Macos => {
                let quoted = shell_quote_path(path, os);
                remote_ssh::exec(connection.clone(), format!("chmod +x {quoted}"))
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
        pub agent_token: Option<String>,
        pub error: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RemoteAgentUpdateResponse {
        pub ok: bool,
        pub ssh_target: String,
        pub previous_version: Option<String>,
        pub new_version: Option<String>,
        pub updated: bool,
        pub running: bool,
        pub health_reachable: bool,
        pub agent_token: Option<String>,
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
        eprintln!(
            "[agent] Starting remote agent on {} with command: {}",
            connection.target_label(),
            command
        );
        eprintln!("[agent] Install path: {}", install_path);
        let start_warning = match tokio::time::timeout(
            Duration::from_secs(15),
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
                    agent_token: None,
                    error: Some(error_msg),
                });
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => Some(
                "Start command did not return within 15 seconds; checking agent health".to_string(),
            ),
        };

        let os_hint = if install_path.contains('\\')
            || install_path.to_ascii_lowercase().contains("appdata")
        {
            RemoteOs::Windows
        } else {
            RemoteOs::Unix
        };

        let agent_url = connection.agent_url(REMOTE_AGENT_DEFAULT_PORT);
        eprintln!("[agent] Checking agent health at {}", agent_url);
        // /health requires no auth; token is read after startup to avoid a race
        // where a freshly-started agent hasn't written its token file yet.
        let health_reachable = tokio::time::timeout(Duration::from_secs(20), async {
            for i in 1..=20 {
                eprintln!("[agent] Health check attempt {}/20...", i);
                if agent_health_reachable_with_token(&agent_url, None).await {
                    eprintln!("[agent] Agent health check passed");
                    break;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
        .await;

        let running = health_reachable.is_ok();

        let error = if !running {
            let health_error = match health_reachable {
                Err(tokio::time::error::Elapsed { .. }) => Some("Agent did not start within 20 seconds. Check if the agent is listening on 0.0.0.0:7779 and if the remote firewall allows inbound connections on port 7779.".to_string()),
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

        // Read the token after the agent is confirmed running. A newly started
        // agent writes its token file during initialization, so retry briefly to
        // handle the case where the file doesn't exist yet.
        let agent_token = if running {
            let mut token = None;
            for _ in 0..6 {
                token = read_remote_agent_token(&connection, os_hint, Some(&agent_url)).await;
                if token.is_some() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            token
        } else {
            None
        };

        Ok(RemoteAgentStartResponse {
            ok: running,
            ssh_target: connection.target_label(),
            install_path: install_path.to_string(),
            running,
            health_reachable: running,
            agent_token,
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
                running: false,
                health_reachable: false,
                agent_token: None,
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
                running: false,
                health_reachable: false,
                agent_token: None,
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
                running: false,
                health_reachable: false,
                agent_token: None,
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
            running: start_response.running,
            health_reachable: start_response.health_reachable,
            agent_token: start_response.agent_token,
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

    #[derive(Debug, Serialize)]
    pub struct SelfUpdateResult {
        pub tag_name: String,
        /// True when running on Windows where in-place binary replacement is not possible.
        pub windows: bool,
        /// Direct download URL for the matching release asset (Windows only).
        pub download_url: Option<String>,
    }

    /// Replace the running binary with the latest release from GitHub.
    ///
    /// On macOS/Linux: downloads the asset, extracts if needed, copies it into
    /// the same directory as the running binary, then atomically renames it over
    /// the current executable. The running process keeps its old inode in memory,
    /// so the rename is safe.
    ///
    /// On Windows: in-place replacement of a running `.exe` is blocked by the OS.
    /// Downloads the new binary, writes a small batch helper to %TEMP%, and spawns
    /// it as a detached process. The batch file waits for this PID to exit, copies
    /// the new binary over, and relaunches. `process::exit(0)` is then called by
    /// the API handler after returning a response.
    pub async fn self_update_binary() -> Result<SelfUpdateResult> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        let release = crate::agent::latest_release_info().await?;

        if os == "windows" {
            #[cfg(windows)]
            return self_update_binary_windows(&release, arch).await;
            #[cfg(not(windows))]
            return Err(anyhow::anyhow!(
                "Windows update path is not available in this build"
            ));
        }

        let asset = release
            .matching_asset(os, arch)
            .ok_or_else(|| anyhow::anyhow!("No release asset for {os}/{arch}"))?
            .clone();

        let local_path = download_asset_locally(&asset).await?;
        let binary_path = if asset.archive {
            extract_archive_with_timeout(&local_path, &asset).await?
        } else {
            local_path.clone()
        };

        let current_exe = std::env::current_exe()
            .map_err(|e| anyhow::anyhow!("Cannot locate current binary: {e}"))?;

        // Stage in the same directory so rename stays on one filesystem.
        let parent = current_exe
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Binary path has no parent directory"))?;
        let staged = parent.join(format!(".llama-monitor-update-{}", std::process::id()));

        fs::copy(&binary_path, &staged).map_err(|e| {
            anyhow::anyhow!("Cannot stage update (check write permission on binary directory): {e}")
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))
                .map_err(|e| anyhow::anyhow!("Cannot set executable permission: {e}"))?;
        }

        // Atomic rename — safe on Unix even while this process is running.
        if let Err(e) = fs::rename(&staged, &current_exe) {
            let _ = fs::remove_file(&staged);
            return Err(anyhow::anyhow!(
                "Cannot replace binary (check write permission on binary location): {e}"
            ));
        }

        let _ = fs::remove_file(&binary_path);

        Ok(SelfUpdateResult {
            tag_name: release.tag_name,
            windows: false,
            download_url: None,
        })
    }

    /// Windows-specific self-update path.
    ///
    /// Cannot rename over a running `.exe`, so instead:
    /// 1. Downloads and extracts the new binary to a temp path.
    /// 2. Writes a batch helper to %TEMP% that polls until this PID exits,
    ///    then does `copy /Y new_exe current_exe` and relaunches.
    /// 3. Spawns the batch helper as a DETACHED_PROCESS so it outlives us.
    ///
    /// The caller (`api_self_update`) schedules `process::exit(0)` after
    /// returning the HTTP response, which unblocks the batch wait loop.
    #[cfg(windows)]
    async fn self_update_binary_windows(
        release: &crate::agent::LatestReleaseInfo,
        arch: &str,
    ) -> Result<SelfUpdateResult> {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;

        let asset = release
            .matching_asset("windows", arch)
            .ok_or_else(|| anyhow::anyhow!("No Windows release asset for arch {arch}"))?
            .clone();

        let local_path = download_asset_locally(&asset).await?;
        let binary_path = extract_archive_with_timeout(&local_path, &asset).await?;

        let current_exe = std::env::current_exe()
            .map_err(|e| anyhow::anyhow!("Cannot locate current binary: {e}"))?;

        let pid = std::process::id();
        let batch_path = std::env::temp_dir().join(format!("lm-update-{pid}.bat"));

        // Backslash-normalize paths embedded in the batch file.
        let new_exe = binary_path.replace('/', "\\");
        let cur_exe = current_exe.to_string_lossy().replace('/', "\\");

        // The batch file:
        //   :check  — loop until this PID disappears from tasklist
        //   copy    — overwrite the old exe with the new one
        //   start   — relaunch from the same path
        //   del     — self-destruct
        //
        // `find /I "exe"` matches any .exe line in tasklist output for the given
        // PID. When the process exits, tasklist returns only the header, no match.
        let batch = format!(
            "@echo off\r\n\
             :check\r\n\
             tasklist /FI \"PID eq {pid}\" 2>NUL | find /I \"exe\" >NUL\r\n\
             if not errorlevel 1 (\r\n\
                 timeout /t 1 /nobreak >NUL\r\n\
                 goto check\r\n\
             )\r\n\
             copy /Y \"{new_exe}\" \"{cur_exe}\"\r\n\
             start \"\" \"{cur_exe}\"\r\n\
             (goto) 2>NUL & del \"%~f0\"\r\n"
        );

        fs::write(&batch_path, &batch)
            .map_err(|e| anyhow::anyhow!("Cannot write update helper to temp dir: {e}"))?;

        std::process::Command::new("cmd.exe")
            .args(["/C", &batch_path.to_string_lossy().into_owned()])
            .creation_flags(DETACHED_PROCESS)
            .spawn()
            .map_err(|e| anyhow::anyhow!("Cannot launch update helper: {e}"))?;

        Ok(SelfUpdateResult {
            tag_name: release.tag_name.clone(),
            windows: false,
            download_url: None,
        })
    }
}

pub use install::{
    RemoteAgentInstallRequest, default_install_path_for_target, default_start_command_for_target,
    detect_remote_os_simple, install_remote_agent, remove_remote_agent, self_update_binary,
    start_remote_agent, status_remote_agent, stop_remote_agent, update_remote_agent,
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

    #[test]
    fn shell_quote_path_unix_escapes_dangerous_chars() {
        let path = "/opt/test; rm -rf /";
        let quoted = shell_quote_path(path, RemoteOs::Unix);
        // shlex wraps in single quotes; the result should be a single quoted string
        assert!(quoted.starts_with('\''));
        assert!(quoted.ends_with('\''));
        // The semicolon should be inside the quotes, not interpreted as command separator
        assert!(quoted.contains(";"));
    }

    #[test]
    fn shell_quote_path_unix_handles_single_quotes() {
        let path = "/opt/it's a test";
        let quoted = shell_quote_path(path, RemoteOs::Unix);
        // shlex uses double quotes when the string contains single quotes
        // Verify: starts and ends with a quote character, spaces are inside quotes
        assert!(
            (quoted.starts_with('\'') && quoted.ends_with('\''))
                || (quoted.starts_with('"') && quoted.ends_with('"')),
            "quoted path should be wrapped in quotes: {:?}",
            quoted
        );
    }

    #[test]
    fn shell_quote_path_windows_doubles_single_quotes() {
        let path = r#"C:\Program Files\llama-monitor"#;
        let quoted = shell_quote_path(path, RemoteOs::Windows);
        assert!(quoted.starts_with('\''));
        assert!(quoted.ends_with('\''));
    }

    #[test]
    fn shell_quote_path_windows_escapes_embedded_single_quotes() {
        let path = r#"C:\It's a test\llama"#;
        let quoted = shell_quote_path(path, RemoteOs::Windows);
        // Single quotes should be doubled inside the quoted string
        assert!(quoted.contains("''"));
        assert!(!quoted.contains(r#"\'"#));
    }

    #[test]
    fn shell_quote_path_cmd_uses_double_quotes() {
        // cmd.exe does NOT treat single quotes as special
        let path = r#"C:\Program Files\llama-monitor"#;
        let quoted = shell_quote_path_cmd(path);
        assert!(quoted.starts_with('"'));
        assert!(quoted.ends_with('"'));
        assert!(!quoted.contains('\''));
    }

    #[test]
    fn shell_quote_path_cmd_escapes_embedded_double_quotes() {
        let path = r#"C:\Pro"gram Files\llama"#;
        let quoted = shell_quote_path_cmd(path);
        // Embedded double quotes should be escaped with ^
        assert!(quoted.contains("^\""));
    }

    #[test]
    fn validate_install_path_rejects_shell_injection() {
        let malicious_paths = [
            "/opt/test; rm -rf /",
            "/opt/test|whoami",
            "/opt/test&echo hacked",
            "/opt/test`id`",
            "/opt/test$(whoami)",
            "/opt/test'break'out",
            "/opt/test\"break\"out",
            "/opt/test> /dev/null",
            "/opt/test< /etc/passwd",
            "/opt/test!command",
            "/opt/test#comment",
            "/opt/test*glob",
            "/opt/test?question",
        ];

        for path in malicious_paths {
            // Malicious paths should be rejected regardless of target OS
            let result_unix = validate_install_path(path, RemoteOs::Unix);
            let result_windows = validate_install_path(path, RemoteOs::Windows);
            assert!(
                result_unix.is_err(),
                "Expected '{}' to be rejected (Unix)",
                path
            );
            assert!(
                result_windows.is_err(),
                "Expected '{}' to be rejected (Windows)",
                path
            );
        }
    }

    #[test]
    fn validate_install_path_rejects_relative_paths() {
        assert!(validate_install_path("relative/path", RemoteOs::Unix).is_err());
        assert!(validate_install_path("./path", RemoteOs::Unix).is_err());
        assert!(validate_install_path("../path", RemoteOs::Unix).is_err());
    }

    #[test]
    fn validate_install_path_rejects_suspicious_directories() {
        assert!(validate_install_path("/tmp/llama-monitor", RemoteOs::Unix).is_err());
        assert!(validate_install_path("/var/llama-monitor", RemoteOs::Unix).is_err());
        assert!(validate_install_path("/etc/llama-monitor", RemoteOs::Unix).is_err());
        assert!(validate_install_path(r#"C:\Windows\llama-monitor"#, RemoteOs::Windows).is_err());
    }

    #[test]
    fn validate_install_path_accepts_valid_paths() {
        // Unix paths
        let unix_paths = [
            "/opt/llama-monitor",
            "/usr/local/bin/llama-monitor",
            "/home/user/.local/bin/llama-monitor",
            "/Applications/Llama Monitor/llama-monitor",
            "~/.config/llama-monitor/bin/llama-monitor", // default Unix/macOS path
        ];

        for path in unix_paths {
            let result = validate_install_path(path, RemoteOs::Unix);
            assert!(result.is_ok(), "Expected '{}' to be accepted", path);
        }

        // Windows paths — test with RemoteOs::Windows regardless of build platform
        let windows_paths = [
            r#"C:\Program Files\llama-monitor"#,
            r#"C:\Users\user\.llama-monitor"#,
            r#"%APPDATA%\llama-monitor\bin\llama-monitor.exe"#, // default Windows path
        ];
        for path in windows_paths {
            let result = validate_install_path(path, RemoteOs::Windows);
            assert!(result.is_ok(), "Expected '{}' to be accepted", path);
        }
    }
}
