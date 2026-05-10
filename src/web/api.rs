use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::OnceLock;
use warp::Filter;
use warp::reject::Reject;

#[derive(Debug)]
struct ApiError(String);

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "API error: {}", self.0)
    }
}

impl std::error::Error for ApiError {}

impl Reject for ApiError {}

use crate::config::AppConfig;
use crate::gpu::env::{self as gpu_env, GPU_ARCHITECTURES, GpuEnv};

#[cfg(target_os = "windows")]
use crate::lhm;

use crate::lhm_persistence as lhm_persist;
use crate::llama::server::{self, ServerConfig};
use crate::models;
use crate::presets::{self, ModelPreset};
use crate::remote_ssh::{self, SshConnection};
use crate::state::{self as app_state, AppState, SessionStatus, UiSettings};

/// Chat message structure for persistence
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub timestamp_ms: u64,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default, alias = "cumulativeInputTokens")]
    pub cumulative_input_tokens: Option<u64>,
    #[serde(default, alias = "cumulativeOutputTokens")]
    pub cumulative_output_tokens: Option<u64>,
    #[serde(default)]
    pub compaction_marker: Option<bool>,
}

/// Context note for guided generation (character details, setting info, etc.)
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ContextNote {
    pub section: String,
    pub content: String,
    #[serde(default)]
    pub created_at: u64,
}

/// Model parameters for a chat tab
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ChatModelParams {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub min_p: f32,
    pub repeat_penalty: f32,
    pub max_tokens: Option<u32>,
}

impl Default for ChatModelParams {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            min_p: 0.01,
            repeat_penalty: 1.0,
            max_tokens: None,
        }
    }
}

/// Chat tab structure for persistence
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ChatTab {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
    #[serde(default)]
    pub ai_name: Option<String>,
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default, rename = "explicitLevel", alias = "explicit_mode")]
    pub explicit_level: Option<u8>,
    pub messages: Vec<ChatMessage>,
    // Serialized as camelCase so GET responses and PUT bodies use identical names.
    // Alias keeps existing disk files readable if they were written as snake_case.
    #[serde(rename = "totalInputTokens", alias = "total_input_tokens", default)]
    pub total_input_tokens: Option<u64>,
    #[serde(rename = "totalOutputTokens", alias = "total_output_tokens", default)]
    pub total_output_tokens: Option<u64>,
    pub model_params: ChatModelParams,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub auto_compact: Option<bool>,
    #[serde(default)]
    pub compact_threshold: Option<f32>,
    /// Last known context window percentage (0–100). Derived client-side and
    /// persisted so the dashboard can show it on page load without a live session.
    #[serde(rename = "lastCtxPct", default)]
    pub last_ctx_pct: Option<f32>,
    #[serde(rename = "activeTemplateId", default)]
    pub active_template_id: Option<String>,
    /// Guided generation: persistent context notes (character, setting, plot details)
    #[serde(default)]
    pub context_notes: Vec<ContextNote>,
    /// Guided generation: remembered sidebar width in pixels (default: 280)
    #[serde(default, rename = "sidebarWidth")]
    pub sidebar_width: u32,
}

/// Suggestion request for guided generation
#[derive(serde::Deserialize, Debug)]
pub struct SuggestionRequest {
    pub tab_id: String,
    pub category: String,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub context_depth: Option<u32>,
    pub prompt: Option<String>,
}

/// Suggestion response
#[derive(serde::Serialize, Debug)]
pub struct SuggestionResponse {
    pub suggestions: Vec<String>,
    pub category: String,
    pub count: u32,
}

static CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Set the config directory (called once at startup from AppConfig).
pub fn set_config_dir(path: PathBuf) {
    CONFIG_DIR.set(path).ok();
}

/// Load prompts from static/prompts directory
fn load_prompts_from_files() -> HashMap<String, String> {
    let mut prompts = HashMap::new();

    // Try to load from static/prompts directory relative to executable
    let prompts_dir = PathBuf::from("static/prompts");

    if let Ok(entries) = fs::read_dir(&prompts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                && let Ok(content) = fs::read_to_string(&path)
            {
                prompts.insert(stem.to_string(), content);
            }
        }
    }

    prompts
}

fn chat_tabs_path() -> PathBuf {
    if let Some(dir) = CONFIG_DIR.get() {
        dir.join("chat-tabs.json")
    } else {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("llama-monitor")
            .join("chat-tabs.json")
    }
}

fn api_check_lhm() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "lhm" / "check")
        .and(warp::get())
        .and_then(move || async move {
            #[cfg(target_os = "windows")]
            {
                let running = lhm::is_lhm_running();
                let installed = lhm::is_lhm_installed();
                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                    "running": running,
                    "installed": installed,
                    "available": running
                })))
            }

            #[cfg(not(target_os = "windows"))]
            {
                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                    "running": false,
                    "installed": false,
                    "available": false,
                    "error": "Not supported on this platform"
                })))
            }
        })
}

fn api_lhm_start() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "lhm" / "start")
        .and(warp::post())
        .and_then(move || {
            async move {
                #[cfg(target_os = "windows")]
                {
                    match lhm::start_lhm().await {
                        Ok(()) => Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"success": true}),
                        )),
                        Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"success": false, "error": e}),
                        )),
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"success": false, "error": "Not supported on this platform"}),
                    ))
                }
            }
        })
}

fn api_lhm_status(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let lhm_disabled_file = app_config.lhm_disabled_file.clone();
    warp::path!("api" / "lhm" / "status")
        .and(warp::get())
        .and_then(move || {
            #[allow(unused_variables)]
            let file = lhm_disabled_file.clone();
            async move {
                #[cfg(target_os = "windows")]
                {
                    match lhm_persist::load_lhm_disabled(&file) {
                        Ok(disabled) => Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"disabled": disabled}),
                        )),
                        Err(_) => Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"disabled": false}),
                        )),
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"disabled": false}),
                    ))
                }
            }
        })
}

fn api_lhm_progress() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    warp::path!("api" / "lhm" / "progress")
        .and(warp::get())
        .and_then(move || {
            #[cfg(target_os = "windows")]
            {
                async move {
                    use std::fs;

                    let local_app_data = match std::env::var("LOCALAPPDATA") {
                        Ok(val) => val,
                        Err(_) => {
                            return Ok::<_, warp::Rejection>(warp::reply::json(
                                &serde_json::json!({"progress": "error: LOCALAPPDATA not set"}),
                            ));
                        }
                    };
                    let progress_file = std::path::Path::new(&local_app_data)
                        .join("LibreHardwareMonitor")
                        .join("install_progress.txt");

                    let progress = fs::read_to_string(&progress_file)
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| "not_started".to_string());

                    Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"progress": progress}),
                    ))
                }
            }

            #[cfg(not(target_os = "windows"))]
            {
                async move {
                    Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"progress": "not_supported"}),
                    ))
                }
            }
        })
}

fn api_lhm_install() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    warp::path!("api" / "lhm" / "install")
        .and(warp::post())
        .and_then(move || {
            async move {
                #[cfg(target_os = "windows")]
                {
                    eprintln!("[API] /api/lhm/install called");
                    match lhm::download_and_install_lhm().await {
                        Ok(()) => {
                            eprintln!("[API] LHM install succeeded");
                            Ok::<_, warp::Rejection>(warp::reply::json(
                                &serde_json::json!({"success": true}),
                            ))
                        }
                        Err(e) => {
                            eprintln!("[API] LHM install failed: {}", e);
                            Ok::<_, warp::Rejection>(warp::reply::json(
                                &serde_json::json!({"success": false, "error": e}),
                            ))
                        }
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    eprintln!("[API] /api/lhm/install called (non-Windows, not supported)");
                    Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"success": false, "error": "Not supported on this platform"}),
                    ))
                }
            }
        })
}

fn api_lhm_uninstall() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    warp::path!("api" / "lhm" / "uninstall")
        .and(warp::post())
        .and_then(move || {
            async move {
                #[cfg(target_os = "windows")]
                {
                    eprintln!("[API] /api/lhm/uninstall called");
                    match lhm::uninstall_lhm() {
                        Ok(()) => {
                            eprintln!("[API] LHM uninstall succeeded");
                            Ok::<_, warp::Rejection>(warp::reply::json(
                                &serde_json::json!({"success": true}),
                            ))
                        }
                        Err(e) => {
                            eprintln!("[API] LHM uninstall failed: {}", e);
                            Ok::<_, warp::Rejection>(warp::reply::json(
                                &serde_json::json!({"success": false, "error": e}),
                            ))
                        }
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    eprintln!("[API] /api/lhm/uninstall called (non-Windows, not supported)");
                    Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"success": false, "error": "Not supported on this platform"}),
                    ))
                }
            }
        })
}

fn api_sensor_bridge_status()
-> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sensor-bridge" / "status")
        .and(warp::get())
        .and_then(|| async move {
            #[cfg(target_os = "windows")]
            {
                let installed = lhm::is_local_sensor_bridge_service_installed();
                let running = lhm::is_local_sensor_bridge_running();
                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                    "installed": installed,
                    "running": running,
                    "available": lhm::is_sensor_bridge_available(),
                })))
            }
            #[cfg(not(target_os = "windows"))]
            {
                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                    "installed": false,
                    "running": false,
                    "available": false,
                })))
            }
        })
}

fn api_sensor_bridge_install()
-> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sensor-bridge" / "install")
        .and(warp::post())
        .and_then(|| async move {
            #[cfg(target_os = "windows")]
            {
                match lhm::install_local_sensor_bridge() {
                    Ok(()) => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                        "started": true,
                        "message": "UAC prompt launched — approve it on your desktop to install the sensor service",
                    }))),
                    Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                        "started": false,
                        "error": e,
                    }))),
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                    "started": false,
                    "error": "Not supported on this platform",
                })))
            }
        })
}

fn api_sensor_bridge_uninstall()
-> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sensor-bridge" / "uninstall")
        .and(warp::post())
        .and_then(|| async move {
            #[cfg(target_os = "windows")]
            {
                match lhm::uninstall_local_sensor_bridge() {
                    Ok(()) => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                        "started": true,
                        "message": "UAC prompt launched — approve it on your desktop to remove the sensor service",
                    }))),
                    Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                        "started": false,
                        "error": e,
                    }))),
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                    "started": false,
                    "error": "Not supported on this platform",
                })))
            }
        })
}

fn api_disable_lhm(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let lhm_disabled_file = app_config.lhm_disabled_file.clone();
    warp::path!("api" / "lhm" / "disable")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |body: serde_json::Value| {
            let disabled = body["disabled"].as_bool().unwrap_or(false);
            #[allow(unused_variables)]
            let file = lhm_disabled_file.clone();
            async move {
                let result = lhm_persist::save_lhm_disabled(&file, disabled)
                    .map(|_| warp::reply::json(&serde_json::json!({"ok": true})))
                    .unwrap_or_else(|e| {
                        warp::reply::json(&serde_json::json!({"ok": false, "error": e}))
                    });
                Ok::<_, warp::Rejection>(result)
            }
        })
}

pub fn api_routes(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let start = api_start(state.clone(), app_config.clone());
    let stop = api_stop(state.clone());
    let kill_llama = api_kill_llama(state.clone());
    let get_presets = api_get_presets(state.clone());
    let create_preset = api_create_preset(state.clone());
    let update_preset = api_update_preset(state.clone());
    let delete_preset = api_delete_preset(state.clone());
    let reset_presets = api_reset_presets(state.clone());
    let get_templates = api_get_templates(state.clone());
    let create_template = api_create_template(state.clone());
    let update_template = api_update_template(state.clone());
    let delete_template = api_delete_template(state.clone());
    let get_models = api_get_models(state.clone());
    let refresh_models = api_refresh_models(state.clone());
    let get_gpu_env = api_get_gpu_env(state.clone());
    let put_gpu_env = api_put_gpu_env(state.clone());
    let get_settings = api_get_settings(state.clone());
    let put_settings = api_put_settings(state.clone());
    let browse = api_browse();
    let chat = api_chat(state.clone());
    let chat_abort = api_chat_abort(state.clone());
    let chat_suggestions = api_chat_suggestions(state.clone());
    let get_chat_tabs = api_get_chat_tabs();
    let put_chat_tabs = api_put_chat_tabs();
    let get_sessions = api_get_sessions(state.clone());
    let create_session = api_create_session(state.clone());
    let delete_session = api_delete_session(state.clone());
    let get_active_session = api_get_active_session(state.clone());
    let set_active_session = api_set_active_session(state.clone());
    let get_capabilities = api_get_capabilities(state.clone());
    let spawn_session_with_preset =
        api_spawn_session_with_preset(state.clone(), app_config.clone());
    let attach = api_attach(state.clone());
    let detach = api_detach(state.clone());
    let check_lhm = api_check_lhm();
    let start_lhm = api_lhm_start();
    let install_lhm = api_lhm_install();
    let uninstall_lhm = api_lhm_uninstall();
    let progress_lhm = api_lhm_progress();
    let status_lhm = api_lhm_status(app_config.clone());
    let disable_lhm = api_disable_lhm(app_config.clone());
    let remote_agent_latest = api_remote_agent_latest_release();
    let remote_agent_detect = api_remote_agent_detect(app_config.clone());
    let remote_agent_host_key = api_remote_agent_ssh_host_key(app_config.clone());
    let remote_agent_trust_host = api_remote_agent_ssh_trust(app_config.clone());
    let remote_agent_status = api_remote_agent_status(app_config.clone());
    let remote_agent_remove = api_remote_agent_remove(app_config.clone());
    let sensor_bridge_status = api_sensor_bridge_status();
    let sensor_bridge_install = api_sensor_bridge_install();
    let sensor_bridge_uninstall = api_sensor_bridge_uninstall();

    // Group routes to avoid compiler overflow on long .or() chains
    let server_routes = start.or(stop).or(kill_llama).or(attach).or(detach);
    let preset_routes = get_presets
        .or(create_preset)
        .or(update_preset)
        .or(delete_preset)
        .or(reset_presets);
    let template_routes = get_templates
        .or(create_template)
        .or(update_template)
        .or(delete_template);
    let model_routes = get_models.or(refresh_models);
    let config_routes = get_gpu_env
        .or(put_gpu_env)
        .or(get_settings)
        .or(put_settings);
    let chat_routes = browse
        .or(chat)
        .or(chat_abort)
        .or(chat_suggestions)
        .or(get_chat_tabs)
        .or(put_chat_tabs);
    let session_routes = get_sessions
        .or(create_session)
        .or(delete_session)
        .or(get_active_session)
        .or(set_active_session)
        .or(get_capabilities)
        .or(spawn_session_with_preset);
    let lhm_routes = check_lhm
        .or(start_lhm)
        .or(progress_lhm)
        .or(status_lhm)
        .or(install_lhm)
        .or(uninstall_lhm)
        .or(disable_lhm);
    let agent_routes = remote_agent_latest
        .or(remote_agent_detect)
        .or(remote_agent_host_key)
        .or(remote_agent_trust_host)
        .or(remote_agent_status)
        .or(api_remote_agent_install(app_config.clone()))
        .or(api_remote_agent_start(app_config.clone()))
        .or(api_remote_agent_update(app_config.clone()))
        .or(api_remote_agent_stop(app_config));
    let bridge_routes = remote_agent_remove
        .or(sensor_bridge_status)
        .or(sensor_bridge_install)
        .or(sensor_bridge_uninstall);

    server_routes
        .or(preset_routes)
        .or(template_routes)
        .or(model_routes)
        .or(config_routes)
        .or(chat_routes)
        .or(session_routes)
        .or(lhm_routes)
        .or(agent_routes)
        .or(bridge_routes)
        .or(api_self_update())
}

fn api_remote_agent_latest_release()
-> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "remote-agent" / "releases" / "latest")
        .and(warp::get())
        .and_then(move || async move {
            match crate::agent::latest_release_info().await {
                Ok(release) => Ok::<_, warp::Rejection>(warp::reply::json(
                    &serde_json::json!({"ok": true, "release": release}),
                )),
                Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(
                    &serde_json::json!({"ok": false, "error": e.to_string()}),
                )),
            }
        })
}

fn api_remote_agent_detect(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "remote-agent" / "detect")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |mut request: crate::agent::RemoteAgentDetectRequest| {
            let app_config = app_config.clone();
            async move {
                match hydrate_ssh_connection(
                    request.ssh_connection.take(),
                    &request.ssh_target,
                    &app_config,
                ) {
                    Ok(connection) => request.ssh_connection = Some(connection),
                    Err(e) => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ));
                    }
                }
                let response = crate::agent::detect_remote_agent(request).await;
                Ok::<_, warp::Rejection>(warp::reply::json(&response))
            }
        })
}

fn api_remote_agent_ssh_host_key(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "remote-agent" / "ssh" / "host-key")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |request: serde_json::Map<String, serde_json::Value>| {
            let app_config = app_config.clone();
            async move {
                let target = request
                    .get("ssh_target")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                let connection = ssh_connection_from_request(&request, target);
                match remote_ssh::scan_host_key(connection, app_config.ssh_known_hosts_file.clone())
                    .await
                {
                    Ok(info) => Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": true, "host_key": info}),
                    )),
                    Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": e.to_string()}),
                    )),
                }
            }
        })
}

fn api_remote_agent_ssh_trust(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "remote-agent" / "ssh" / "trust")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |request: serde_json::Map<String, serde_json::Value>| {
            let app_config = app_config.clone();
            async move {
                let target = request
                    .get("ssh_target")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                let key_hex = request
                    .get("key_hex")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                let connection = ssh_connection_from_request(&request, target);
                match remote_ssh::scan_host_key(
                    connection.clone(),
                    app_config.ssh_known_hosts_file.clone(),
                )
                .await
                {
                    Ok(info) if info.key_hex == key_hex.trim().to_ascii_lowercase() => {}
                    Ok(_) => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Host key changed between scan and trust confirmation"}),
                        ));
                    }
                    Err(e) => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ));
                    }
                }
                match remote_ssh::trust_host_key(
                    &app_config.ssh_known_hosts_file,
                    &connection,
                    key_hex,
                ) {
                    Ok(()) => Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": true}),
                    )),
                    Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": e.to_string()}),
                    )),
                }
            }
        })
}

fn ssh_connection_from_request(
    request: &serde_json::Map<String, serde_json::Value>,
    target: &str,
) -> SshConnection {
    request
        .get("ssh_connection")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_else(|| SshConnection::from_target(target))
}

fn hydrate_ssh_connection(
    connection: Option<SshConnection>,
    target: &str,
    app_config: &AppConfig,
) -> anyhow::Result<SshConnection> {
    let connection = connection.unwrap_or_else(|| SshConnection::from_target(target));
    remote_ssh::with_trusted_host_key(connection, &app_config.ssh_known_hosts_file)
}

fn api_remote_agent_install(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "remote-agent" / "install")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(
            move |mut request: crate::agent::RemoteAgentInstallRequest| {
                let app_config = app_config.clone();
                async move {
                    crate::agent::suppress_remote_agent_autostart();
                    request.ssh_connection = match hydrate_ssh_connection(
                        request.ssh_connection.take(),
                        &request.ssh_target,
                        &app_config,
                    ) {
                        Ok(connection) => Some(connection),
                        Err(e) => {
                            return Ok::<_, warp::Rejection>(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": e.to_string()}),
                            ));
                        }
                    };
                    let remote_os = if let Some(connection) = request.ssh_connection.clone() {
                        crate::agent::detect_remote_os_for_connection(connection).await
                    } else {
                        crate::agent::detect_remote_os_simple(&request.ssh_target).await
                    };
                    match crate::agent::install_remote_agent(
                        request.ssh_target.trim(),
                        request.ssh_connection.clone(),
                        &request.asset,
                        request.install_path.clone(),
                        remote_os,
                    )
                    .await
                    {
                        Ok(response) => Ok::<_, warp::Rejection>(warp::reply::json(&response)),
                        Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        )),
                    }
                }
            },
        )
}

fn api_remote_agent_status(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "remote-agent" / "status")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |request: serde_json::Map<String, serde_json::Value>| {
            let app_config = app_config.clone();
            async move {
                let ssh_target = match request.get("ssh_target") {
                    Some(v) => v.as_str().unwrap_or("").to_string(),
                    None => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Missing ssh_target"}),
                        ));
                    }
                };
                let ssh_connection = match hydrate_ssh_connection(
                    request
                        .get("ssh_connection")
                        .and_then(|value| serde_json::from_value(value.clone()).ok()),
                    &ssh_target,
                    &app_config,
                ) {
                    Ok(connection) => Some(connection),
                    Err(e) => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ));
                    }
                };
                match crate::agent::status_remote_agent(&ssh_target, ssh_connection).await {
                    Ok(response) => Ok(warp::reply::json(&response)),
                    Err(e) => Ok(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": e.to_string()}),
                    )),
                }
            }
        })
}

fn api_remote_agent_start(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "remote-agent" / "start")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |request: serde_json::Map<String, serde_json::Value>| {
            let app_config = app_config.clone();
            async move {
                crate::agent::suppress_remote_agent_autostart();
                let ssh_target = match request.get("ssh_target") {
                    Some(v) => v.as_str().unwrap_or("").to_string(),
                    None => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Missing ssh_target"}),
                        ));
                    }
                };
                let install_path = match request.get("install_path") {
                    Some(v) => v.as_str().unwrap_or("").to_string(),
                    None => crate::agent::default_install_path_for_target(&ssh_target).await,
                };
                let ssh_connection = match hydrate_ssh_connection(
                    request
                        .get("ssh_connection")
                        .and_then(|value| serde_json::from_value(value.clone()).ok()),
                    &ssh_target,
                    &app_config,
                ) {
                    Ok(connection) => Some(connection),
                    Err(e) => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ));
                    }
                };
                let command = if let Some(ref conn) = ssh_connection {
                    // Detect OS via SSH for accurate command building
                    let remote_os = crate::agent::detect_remote_os_with(conn).await;
                    // Derive install path from detected OS instead of trusting frontend
                    let resolved_install_path =
                        crate::agent::default_install_path_for_os(remote_os);
                    crate::agent::default_start_command_for_os_with(
                        conn,
                        remote_os,
                        &resolved_install_path,
                    )
                    .await
                } else {
                    // Fallback: use frontend's command or build default
                    match request.get("start_command") {
                        Some(v) => v.as_str().unwrap_or("").to_string(),
                        None => {
                            crate::agent::default_start_command_for_target(
                                &ssh_target,
                                &install_path,
                            )
                            .await
                        }
                    }
                };
                match crate::agent::start_remote_agent(
                    &ssh_target,
                    ssh_connection,
                    &install_path,
                    &command,
                )
                .await
                {
                    Ok(response) => Ok(warp::reply::json(&response)),
                    Err(e) => Ok(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": e.to_string()}),
                    )),
                }
            }
        })
}

fn api_remote_agent_update(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "remote-agent" / "update")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |request: serde_json::Map<String, serde_json::Value>| {
            let app_config = app_config.clone();
            async move {
                crate::agent::suppress_remote_agent_autostart();
                let ssh_target = match request.get("ssh_target") {
                    Some(v) => v.as_str().unwrap_or("").to_string(),
                    None => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Missing ssh_target"}),
                        ));
                    }
                };
                let ssh_connection = match hydrate_ssh_connection(
                    request
                        .get("ssh_connection")
                        .and_then(|value| serde_json::from_value(value.clone()).ok()),
                    &ssh_target,
                    &app_config,
                ) {
                    Ok(connection) => Some(connection),
                    Err(e) => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ));
                    }
                };
                match crate::agent::update_remote_agent(&ssh_target, ssh_connection).await {
                    Ok(response) => Ok(warp::reply::json(&response)),
                    Err(e) => Ok(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": e.to_string()}),
                    )),
                }
            }
        })
}

fn api_remote_agent_stop(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "remote-agent" / "stop")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |request: serde_json::Map<String, serde_json::Value>| {
            let app_config = app_config.clone();
            async move {
                crate::agent::suppress_remote_agent_autostart();
                let ssh_target = match request.get("ssh_target") {
                    Some(v) => v.as_str().unwrap_or("").to_string(),
                    None => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Missing ssh_target"}),
                        ));
                    }
                };
                let ssh_connection = match hydrate_ssh_connection(
                    request
                        .get("ssh_connection")
                        .and_then(|value| serde_json::from_value(value.clone()).ok()),
                    &ssh_target,
                    &app_config,
                ) {
                    Ok(connection) => Some(connection),
                    Err(e) => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ));
                    }
                };
                match crate::agent::stop_remote_agent(&ssh_target, ssh_connection).await {
                    Ok(response) => Ok(warp::reply::json(&response)),
                    Err(e) => Ok(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": e.to_string()}),
                    )),
                }
            }
        })
}

fn api_remote_agent_remove(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "remote-agent" / "remove")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |request: serde_json::Map<String, serde_json::Value>| {
            let app_config = app_config.clone();
            async move {
                crate::agent::suppress_remote_agent_autostart();
                let ssh_target = match request.get("ssh_target") {
                    Some(v) => v.as_str().unwrap_or("").to_string(),
                    None => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Missing ssh_target"}),
                        ));
                    }
                };
                let ssh_connection = match hydrate_ssh_connection(
                    request
                        .get("ssh_connection")
                        .and_then(|value| serde_json::from_value(value.clone()).ok()),
                    &ssh_target,
                    &app_config,
                ) {
                    Ok(connection) => Some(connection),
                    Err(e) => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ));
                    }
                };
                match crate::agent::remove_remote_agent(&ssh_target, ssh_connection).await {
                    Ok(response) => Ok(warp::reply::json(&response)),
                    Err(e) => Ok(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": e.to_string()}),
                    )),
                }
            }
        })
}

fn api_start(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "start")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |config: ServerConfig| {
            let state = state.clone();
            let app_config = app_config.clone();
            async move {
                let ui = state.ui_settings.lock().unwrap().clone();
                let mut eff_config = (*app_config).clone();
                if !ui.llama_server_path.is_empty() {
                    eff_config.llama_server_path = PathBuf::from(&ui.llama_server_path);
                }
                if !ui.llama_server_cwd.is_empty() {
                    eff_config.llama_server_cwd = PathBuf::from(&ui.llama_server_cwd);
                }
                match server::start_server(&state, config, &eff_config).await {
                    Ok(()) => Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": true}),
                    )),
                    Err(e) => Ok(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": e.to_string()}),
                    )),
                }
            }
        })
}

fn api_stop(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "stop")
        .and(warp::post())
        .and_then(move || {
            let state = state.clone();
            async move {
                match server::stop_server(&state).await {
                    Ok(()) => Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": true}),
                    )),
                    Err(e) => Ok(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": e.to_string()}),
                    )),
                }
            }
        })
}

fn api_get_presets(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets")
        .and(warp::get())
        .map(move || {
            let presets = state.presets.lock().unwrap().clone();
            warp::reply::json(&presets)
        })
}

fn api_create_preset(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets")
        .and(warp::post())
        .and(warp::body::json())
        .map(move |preset: ModelPreset| {
            let mut presets = state.presets.lock().unwrap();
            presets.push(preset.clone());
            let _ = presets::save_presets(&state.presets_path, &presets);
            warp::reply::json(&serde_json::json!({"ok": true, "preset": preset}))
        })
}

fn api_update_preset(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets" / String)
        .and(warp::put())
        .and(warp::body::json())
        .map(move |id: String, updated: ModelPreset| {
            let mut presets = state.presets.lock().unwrap();
            if let Some(existing) = presets.iter_mut().find(|p| p.id == id) {
                *existing = updated.clone();
                let _ = presets::save_presets(&state.presets_path, &presets);
                warp::reply::json(&serde_json::json!({"ok": true, "preset": updated}))
            } else {
                warp::reply::json(&serde_json::json!({"ok": false, "error": "preset not found"}))
            }
        })
}

fn api_delete_preset(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets" / String)
        .and(warp::delete())
        .map(move |id: String| {
            let mut presets = state.presets.lock().unwrap();
            let before = presets.len();
            presets.retain(|p| p.id != id);
            if presets.len() < before {
                let _ = presets::save_presets(&state.presets_path, &presets);
                warp::reply::json(&serde_json::json!({"ok": true}))
            } else {
                warp::reply::json(&serde_json::json!({"ok": false, "error": "preset not found"}))
            }
        })
}

fn api_reset_presets(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets" / "reset")
        .and(warp::post())
        .map(move || {
            let defaults = presets::default_presets();
            let mut presets = state.presets.lock().unwrap();
            *presets = defaults;
            let _ = presets::save_presets(&state.presets_path, &presets);
            warp::reply::json(&serde_json::json!({"ok": true}))
        })
}

// ── Template API ───────────────────────────────────────────────────────

fn api_get_templates(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates")
        .and(warp::get())
        .map(move || {
            let templates = state.templates.lock().unwrap().clone();
            warp::reply::json(&templates)
        })
}

// ── Personas API ───────────────────────────────────────────────────────

fn api_create_template(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates")
        .and(warp::post())
        .and(warp::body::json())
        .map(move |template: presets::SystemPromptTemplate| {
            let mut templates = state.templates.lock().unwrap();
            templates.push(template.clone());
            let _ = presets::save_templates(&state.templates_path, &templates);
            warp::reply::json(&serde_json::json!({"ok": true, "template": template}))
        })
}

fn api_update_template(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates" / String)
        .and(warp::put())
        .and(warp::body::json())
        .map(move |id: String, updated: presets::SystemPromptTemplate| {
            let mut templates = state.templates.lock().unwrap();
            if let Some(existing) = templates.iter_mut().find(|t| t.id == id) {
                *existing = updated.clone();
                let _ = presets::save_templates(&state.templates_path, &templates);
                warp::reply::json(&serde_json::json!({"ok": true, "template": updated}))
            } else {
                warp::reply::json(&serde_json::json!({"ok": false, "error": "template not found"}))
            }
        })
}

fn api_delete_template(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates" / String)
        .and(warp::delete())
        .map(move |id: String| {
            let mut templates = state.templates.lock().unwrap();
            let before = templates.len();
            templates.retain(|t| t.id != id);
            if templates.len() < before {
                let _ = presets::save_templates(&state.templates_path, &templates);
                warp::reply::json(&serde_json::json!({"ok": true}))
            } else {
                warp::reply::json(&serde_json::json!({"ok": false, "error": "template not found"}))
            }
        })
}

fn api_get_models(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models").and(warp::get()).map(move || {
        let models = state.discovered_models.lock().unwrap().clone();
        warp::reply::json(&models)
    })
}

fn api_refresh_models(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "refresh")
        .and(warp::post())
        .map(move || {
            if let Some(ref dir) = state.models_dir {
                match models::scan_models_dir(dir) {
                    Ok(discovered) => {
                        let count = discovered.len();
                        *state.discovered_models.lock().unwrap() = discovered;
                        warp::reply::json(&serde_json::json!({"ok": true, "count": count}))
                    }
                    Err(e) => {
                        warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()}))
                    }
                }
            } else {
                warp::reply::json(
                    &serde_json::json!({"ok": false, "error": "no models directory configured (use --models-dir)"}),
                )
            }
        })
}

fn api_get_gpu_env(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "gpu-env")
        .and(warp::get())
        .map(move || {
            let env = state.gpu_env.lock().unwrap().clone();
            let detected = gpu_env::detect_gpus();
            warp::reply::json(&serde_json::json!({
                "env": env,
                "architectures": GPU_ARCHITECTURES,
                "detected": detected,
            }))
        })
}

fn api_put_gpu_env(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "gpu-env")
        .and(warp::put())
        .and(warp::body::json())
        .map(move |updated: GpuEnv| {
            let mut env = state.gpu_env.lock().unwrap();
            *env = updated;
            let _ = gpu_env::save_gpu_env(&state.gpu_env_path, &env);
            warp::reply::json(&serde_json::json!({"ok": true}))
        })
}

fn api_get_settings(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "settings")
        .and(warp::get())
        .map(move || {
            let settings = state.ui_settings.lock().unwrap().clone();
            warp::reply::json(&settings)
        })
}

fn api_put_settings(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "settings")
        .and(warp::put())
        .and(warp::body::json())
        .map(move |updated: UiSettings| {
            // Detect if this is a partial update (only ws_push_interval_ms set, rest are defaults)
            let is_partial = updated.preset_id.is_empty()
                && updated.port == 8001
                && updated.llama_server_path.is_empty()
                && updated.llama_server_cwd.is_empty()
                && updated.models_dir.is_empty()
                && updated.server_endpoint.is_empty()
                && updated.llama_poll_interval == 1
                && updated.remote_agent_url.is_empty()
                && updated.remote_agent_token.is_empty()
                && !updated.remote_agent_ssh_autostart
                && updated.remote_agent_ssh_target.is_empty()
                && updated.remote_agent_ssh_command.is_empty()
                && updated.explicit_mode_policy.is_empty()
                && updated.context_card_view == "gauge";

            let mut settings = state.ui_settings.lock().unwrap();
            let old_dir = settings.models_dir.clone();
            let old_token = settings.remote_agent_token.clone();
            let old_push_interval = settings.ws_push_interval_ms;

            if is_partial {
                // Partial update: only apply ws_push_interval_ms
                settings.ws_push_interval_ms = updated.ws_push_interval_ms;
            } else {
                // Full update: replace all settings
                *settings = updated;
            }

            let new_dir = settings.models_dir.clone();
            let token_changed =
                settings.remote_agent_token != old_token && !settings.remote_agent_token.is_empty();
            let push_interval_changed = settings.ws_push_interval_ms != old_push_interval;

            let _ = app_state::save_ui_settings(&state.ui_settings_path, &settings);
            drop(settings);

            if token_changed {
                state.agent_poll_notify.notify_waiters();
            }
            if push_interval_changed {
                state.llama_poll_notify.notify_waiters();
            }

            if new_dir != old_dir
                && !new_dir.is_empty()
                && let Ok(discovered) = crate::models::scan_models_dir(&PathBuf::from(&new_dir))
            {
                *state.discovered_models.lock().unwrap() = discovered;
            }

            warp::reply::json(&serde_json::json!({"ok": true}))
        })
}

fn api_browse() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "browse")
        .and(warp::get())
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .map(|query: std::collections::HashMap<String, String>| {
            let requested = query.get("path").cloned().unwrap_or_default();
            let filter = query.get("filter").cloned().unwrap_or_default();

            let dir = if requested.is_empty() {
                dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
            } else {
                PathBuf::from(&requested)
            };

            let dir = match dir.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    return warp::reply::json(&serde_json::json!({
                        "path": requested,
                        "error": "Path not found"
                    }));
                }
            };

            if !dir.is_dir() {
                return warp::reply::json(&serde_json::json!({
                    "path": dir.display().to_string(),
                    "error": "Not a directory"
                }));
            }

            let parent = dir
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default();

            let mut entries: Vec<serde_json::Value> = Vec::new();
            if let Ok(read_dir) = std::fs::read_dir(&dir) {
                for entry in read_dir.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') {
                        continue;
                    }
                    let meta = entry.metadata().ok();
                    let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());

                    if !is_dir && !filter.is_empty() {
                        let pass = match filter.as_str() {
                            "gguf" => name.ends_with(".gguf"),
                            "executable" => {
                                #[cfg(unix)]
                                {
                                    use std::os::unix::fs::PermissionsExt;
                                    meta.as_ref()
                                        .is_some_and(|m| m.permissions().mode() & 0o111 != 0)
                                }
                                #[cfg(not(unix))]
                                {
                                    true
                                }
                            }
                            _ => true,
                        };
                        if !pass {
                            continue;
                        }
                    }

                    let size = if is_dir {
                        0
                    } else {
                        meta.as_ref().map(|m| m.len()).unwrap_or(0)
                    };
                    let size_display = if is_dir {
                        String::new()
                    } else if size >= 1_000_000_000 {
                        format!("{:.1} GB", size as f64 / 1_000_000_000.0)
                    } else if size >= 1_000_000 {
                        format!("{:.0} MB", size as f64 / 1_000_000.0)
                    } else {
                        format!("{:.0} KB", size as f64 / 1_000.0)
                    };

                    entries.push(serde_json::json!({
                        "name": name,
                        "is_dir": is_dir,
                        "size": size,
                        "size_display": size_display,
                        "path": entry.path().display().to_string(),
                    }));
                }
            }

            entries.sort_by(|a, b| {
                let a_dir = a["is_dir"].as_bool().unwrap_or(false);
                let b_dir = b["is_dir"].as_bool().unwrap_or(false);
                b_dir.cmp(&a_dir).then_with(|| {
                    a["name"]
                        .as_str()
                        .unwrap_or("")
                        .to_lowercase()
                        .cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
                })
            });

            warp::reply::json(&serde_json::json!({
                "path": dir.display().to_string(),
                "parent": parent,
                "entries": entries,
            }))
        })
}

fn api_chat(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat")
        .and(warp::post())
        .and(warp::body::bytes())
        .and_then(move |body: bytes::Bytes| {
            let state = state.clone();
            async move {
                // Derive endpoint from active session — no user-controlled input
                let session = state
                    .get_active_session()
                    .ok_or(warp::reject::not_found())?;

                let url = match &session.mode {
                    crate::state::SessionMode::Spawn { port } => {
                        format!("http://127.0.0.1:{port}/v1/chat/completions")
                    }
                    crate::state::SessionMode::Attach { endpoint } => {
                        format!("{endpoint}/v1/chat/completions")
                    }
                };

                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(120))
                    .build()
                    .map_err(|e| warp::reject::custom(ApiError(e.to_string())))?;

                // Stream response from upstream — Tokio cancels automatically on client disconnect
                let resp = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .body(body.to_vec())
                    .send()
                    .await
                    .map_err(|e| warp::reject::custom(ApiError(e.to_string())))?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let err_body = resp.text().await.unwrap_or_default();
                    return Err(warp::reject::custom(ApiError(format!(
                        "upstream {}: {}",
                        status, err_body
                    ))));
                }

                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);

                // Forward SSE events to client — stops if client disconnects (tx closed)
                tokio::spawn(async move {
                    use futures_util::StreamExt;
                    let mut stream = resp.bytes_stream();
                    let mut buf = String::new();

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                // Stop if client disconnected
                                if tx.is_closed() {
                                    return;
                                }

                                buf.push_str(&String::from_utf8_lossy(&bytes));

                                // Process complete lines
                                while let Some(pos) = buf.find('\n') {
                                    let line = buf[..pos].to_string();
                                    buf = buf[pos + 1..].to_string();

                                    if let Some(data) = line.strip_prefix("data: ")
                                        && !data.trim().is_empty()
                                    {
                                        let _ = tx.send(Ok::<_, warp::Error>(
                                            warp::sse::Event::default().data(data.to_string()),
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Ok::<_, warp::Error>(
                                    warp::sse::Event::default().data(format!(
                                        "{{\"error\":\"{}\"}}",
                                        e.to_string().replace('"', "'")
                                    )),
                                ));
                                break;
                            }
                        }
                    }
                });

                Ok::<_, warp::Rejection>(warp::sse::reply(stream))
            }
        })
}

fn api_chat_abort(
    _state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "abort")
        .and(warp::post())
        .and_then(move || async move {
            Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": true})))
        })
}

fn api_chat_suggestions(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "suggestions")
        .and(warp::post())
        .and(warp::body::json::<SuggestionRequest>())
        .and_then(move |req: SuggestionRequest| {
            let state = state.clone();
            async move {
                // Get tabs and find the requested one
                let path = chat_tabs_path();
                let tabs: Vec<ChatTab> = tokio::fs::read_to_string(&path).await
                    .ok()
                    .and_then(|raw| serde_json::from_str(&raw).ok())
                    .unwrap_or_default();

                let tab = tabs.iter().find(|t| t.id == req.tab_id)
                    .ok_or(warp::reject::not_found())?;

                // Get last N messages
                let depth = req.context_depth.unwrap_or(10) as usize;
                let messages: Vec<ChatMessage> = tab.messages.iter()
                    .rev()
                    .take(depth)
                    .cloned()
                    .collect();

                // Build conversation context
                let mut context = String::new();
                for msg in &messages {
                    let role = if msg.role == "user" { "User" } else { "Assistant" };
                    context.push_str(&format!("{}: {}\n", role, msg.content));
                }

                // Load prompts from files
                let file_prompts = load_prompts_from_files();

                // Get or use default prompt (file-based or hardcoded fallback)
                let default_prompt = req.prompt.or_else(|| {
                    file_prompts.get(&req.category).cloned().or_else(|| {
                        match req.category.as_str() {
                            "plot-twist" => Some("You are a plot twist specialist. Based on the conversation below, suggest {} unexpected, surprising events that could happen next.\n\nFormat as a numbered list. Prioritize: betrayals, revelations, power reversals, unexpected arrivals, hidden truths.\n\n[conversation context]".to_string()),
                            "new-character" => Some("You are a character introduction specialist. Based on the conversation below, suggest {} new characters that could enter the story.\n\nFormat as: [Character Name]: [Brief description and how they connect to current story]\n\n[conversation context]".to_string()),
                            _ => Some("You are a creative brainstorming partner. Based on the conversation below, suggest {} varied, actionable next steps the user could take.\n\nFormat as a numbered list. Prioritize variety: dialogue, action, investigation, social, creative approaches.\n\n[conversation context]".to_string()),
                        }
                    })
                });

                let count = req.count.unwrap_or(5);
                let prompt = default_prompt
                    .unwrap_or_default()
                    .replace("{count}", &count.to_string())
                    .replace("[conversation context]", &context);

                // Build messages for suggestion request
                let suggestion_messages = vec![
                    serde_json::json!({"role": "system", "content": "You are a helpful creative writing assistant. Generate suggestions in a numbered list format. Be concise and actionable."}),
                    serde_json::json!({"role": "user", "content": prompt}),
                ];

                // Call llama.cpp
                let session = state.get_active_session()
                    .ok_or(warp::reject::not_found())?;

                let url = match &session.mode {
                    crate::state::SessionMode::Spawn { port } => {
                        format!("http://127.0.0.1:{port}/v1/chat/completions")
                    }
                    crate::state::SessionMode::Attach { endpoint } => {
                        format!("{endpoint}/v1/chat/completions")
                    }
                };

                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .build()
                    .map_err(|e| warp::reject::custom(ApiError(e.to_string())))?;

                let response = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({
                        "messages": suggestion_messages,
                        "stream": false,
                        "temperature": 0.8,
                        "max_tokens": 512,
                    }))
                    .send()
                    .await
                    .map_err(|e| warp::reject::custom(ApiError(e.to_string())))?;

                if !response.status().is_success() {
                    let status = response.status();
                    let err_body = response.text().await.unwrap_or_default();
                    return Err(warp::reject::custom(ApiError(format!(
                        "upstream {}: {}",
                        status, err_body
                    ))));
                }

                let response_json: serde_json::Value = response.json()
                    .await
                    .map_err(|e| warp::reject::custom(ApiError(e.to_string())))?;

                // Extract and parse suggestions
                let content = response_json.get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|c| c.first())
                    .and_then(|c| c.get("message"))
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .ok_or(warp::reject::not_found())?;

                let suggestions = parse_suggestions(content);
                let count = suggestions.len() as u32;

                Ok::<_, warp::Rejection>(warp::reply::json(&SuggestionResponse {
                    suggestions,
                    category: req.category,
                    count,
                }))
            }
        })
}

fn parse_suggestions(text: &str) -> Vec<String> {
    // Pathweaver format: [EMOJI] TITLE\nDESCRIPTION with --- separators
    // Split by --- separator and parse each block
    let blocks: Vec<&str> = text.split("---").collect();
    let mut suggestions = Vec::new();

    for block in blocks {
        let lines: Vec<&str> = block
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        if lines.len() >= 2 {
            // First line is title with emoji, rest is description
            let title = lines[0];
            let description = lines[1..].join(" ");
            if !title.is_empty() && !description.is_empty() {
                // Combine title and description with newline
                suggestions.push(format!("{}\n{}", title, description));
            }
        }
    }

    // If Pathweaver format yielded nothing, try numbered list: "1. Option" or "1) Option"
    if suggestions.is_empty() {
        let numbered = regex::Regex::new(r"^\d+[\.\)]\s*(.+)$").ok();
        if let Some(re) = numbered {
            let numbered_suggestions: Vec<String> = text
                .lines()
                .filter_map(|line| re.captures(line))
                .filter_map(|caps| caps.get(1))
                .map(|m| m.as_str().trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !numbered_suggestions.is_empty() {
                return numbered_suggestions;
            }
        }
    }

    // Try bullet list: "- Option" or "* Option"
    if suggestions.is_empty() {
        let bullets = regex::Regex::new(r"^[-*]\s+(.+)$").ok();
        if let Some(re) = bullets {
            let bullet_suggestions: Vec<String> = text
                .lines()
                .filter_map(|line| re.captures(line))
                .filter_map(|caps| caps.get(1))
                .map(|m| m.as_str().trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !bullet_suggestions.is_empty() {
                return bullet_suggestions;
            }
        }
    }

    // Fallback: split by newlines, filter empty and short lines
    if suggestions.is_empty() {
        suggestions = text
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.len() > 2)
            .filter(|s| !s.starts_with('['))
            .collect();
    }

    suggestions
}

fn compute_chat_tab_totals(mut tab: ChatTab) -> ChatTab {
    if tab.total_input_tokens.is_none() || tab.total_output_tokens.is_none() {
        let mut total_input: u64 = 0;
        let mut total_output: u64 = 0;
        for msg in &tab.messages {
            if let Some(input) = msg.input_tokens {
                total_input += input;
            }
            if let Some(output) = msg.output_tokens {
                total_output += output;
            }
        }
        tab.total_input_tokens = Some(total_input);
        tab.total_output_tokens = Some(total_output);
    }
    tab
}

fn api_get_chat_tabs() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    warp::path!("api" / "chat" / "tabs")
        .and(warp::get())
        .and_then(|| async move {
            let path = chat_tabs_path();
            if path.exists() {
                match tokio::fs::read_to_string(&path).await {
                    Ok(raw) => match serde_json::from_str::<Vec<ChatTab>>(&raw) {
                        Ok(tabs) => {
                            let tabs: Vec<ChatTab> =
                                tabs.into_iter().map(compute_chat_tab_totals).collect();
                            Ok::<_, warp::Rejection>(warp::reply::json(&tabs))
                        }
                        Err(_) => {
                            Ok::<_, warp::Rejection>(warp::reply::json(&Vec::<ChatTab>::new()))
                        }
                    },
                    Err(_) => Ok::<_, warp::Rejection>(warp::reply::json(&Vec::<ChatTab>::new())),
                }
            } else {
                Ok::<_, warp::Rejection>(warp::reply::json(&Vec::<ChatTab>::new()))
            }
        })
}

fn api_put_chat_tabs() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    warp::path!("api" / "chat" / "tabs")
        .and(warp::put())
        .and(warp::body::bytes())
        .and_then(|body: bytes::Bytes| async move {
            let body_str = String::from_utf8_lossy(&body);
            let tabs: Vec<ChatTab> = match serde_json::from_slice(&body) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Chat tabs deserialize error: {} (body preview: {})", e, &body_str[..body_str.len().min(500)]);
                    return Ok::<Box<dyn warp::reply::Reply + Send + Sync>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()})),
                            warp::http::StatusCode::BAD_REQUEST,
                        )
                    ));
                }
            };
            let path = chat_tabs_path();
            // Protect against overwriting with fewer messages (stale frontend data)
            let new_msg_count = tabs.iter().map(|t| t.messages.len()).sum::<usize>();
            if let Ok(existing) = tokio::fs::read_to_string(&path).await
                && let Ok(existing_tabs) = serde_json::from_str::<Vec<ChatTab>>(&existing)
            {
                let existing_msg_count = existing_tabs.iter().map(|t| t.messages.len()).sum::<usize>();
                if new_msg_count < existing_msg_count && new_msg_count < 10 {
                    eprintln!("Chat tabs: rejecting PUT - would lose {} messages ({} -> {})",
                             existing_msg_count - new_msg_count, existing_msg_count, new_msg_count);
                    return Ok::<Box<dyn warp::reply::Reply + Send + Sync>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({"ok": false, "error": "Would lose messages - rejecting stale data"})),
                            warp::http::StatusCode::CONFLICT,
                        )
                    ));
                }
            }
            match serde_json::to_string_pretty(&tabs) {
                Ok(json) => match tokio::fs::write(&path, json).await {
                    Ok(_) => Ok::<Box<dyn warp::reply::Reply + Send + Sync>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({"ok": true})),
                    )),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply + Send + Sync>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()})),
                    )),
                },
                Err(e) => Ok::<Box<dyn warp::reply::Reply + Send + Sync>, warp::Rejection>(Box::new(
                    warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()})),
                )),
            }
        })
}

fn api_get_sessions(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sessions")
        .and(warp::path::end())
        .and(warp::get())
        .and_then(move || {
            let state = state.clone();
            async move {
                let sessions = state.get_sessions();
                Ok::<_, warp::Rejection>(warp::reply::json(&sessions))
            }
        })
}

fn api_create_session(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sessions")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |session: app_state::Session| {
            let state = state.clone();
            async move {
                if state.add_session(session) {
                    Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": true})))
                } else {
                    Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "Maximum sessions reached"}),
                    ))
                }
            }
        })
}

fn api_delete_session(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sessions" / String)
        .and(warp::path::end())
        .and(warp::delete())
        .and_then(move |session_id: String| {
            let state = state.clone();
            async move {
                if state.remove_session(&session_id) {
                    Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": true})))
                } else {
                    Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "Session not found"}),
                    ))
                }
            }
        })
}

fn api_get_active_session(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sessions" / "active")
        .and(warp::path::end())
        .and(warp::get())
        .and_then(move || {
            let state = state.clone();
            async move {
                let session_id = state.active_session_id.lock().unwrap().clone();
                let sessions = state.sessions.lock().unwrap();
                let session = sessions.iter().find(|s| s.id == session_id).cloned();
                drop(sessions);

                match session {
                    Some(s) => {
                        let mode_str = match s.mode {
                            crate::state::SessionMode::Spawn { port } => format!("Spawn:{}", port),
                            crate::state::SessionMode::Attach { endpoint } => {
                                format!("Attach:{}", endpoint)
                            }
                        };
                        Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                            "id": s.id,
                            "name": s.name,
                            "mode": mode_str,
                            "status": s.status,
                            "last_active": s.last_active
                        })))
                    }
                    None => Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"error": "No active session"}),
                    )),
                }
            }
        })
}

fn api_get_capabilities(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "capabilities")
        .and(warp::path::end())
        .and(warp::get())
        .and_then(move || {
            let state = state.clone();
            async move {
                let capabilities = state.calculate_capabilities();
                let endpoint_kind = state.current_endpoint_kind();
                let session_kind = state.current_session_kind();
                let tray_mode = state.tray_mode.lock().unwrap().clone();

                let (system_reason, gpu_reason, cpu_temp_reason) =
                    state.calculate_availability_reasons();

                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                    "capabilities": capabilities,
                    "endpoint_kind": endpoint_kind,
                    "session_kind": session_kind,
                    "tray_mode": tray_mode,
                    "availability": {
                        "system": system_reason,
                        "gpu": gpu_reason,
                        "cpu_temp": cpu_temp_reason
                    }
                })))
            }
        })
}

fn api_set_active_session(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sessions" / "active")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |payload: serde_json::Value| {
            let state = state.clone();
            async move {
                let session_id = match payload.get("id") {
                    Some(v) => v.as_str().unwrap_or("").to_string(),
                    None => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Missing session id"}),
                        ));
                    }
                };
                if state.set_active_session(&session_id) {
                    Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": true})))
                } else {
                    Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "Session not found"}),
                    ))
                }
            }
        })
}

fn api_spawn_session_with_preset(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sessions" / "spawn")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |payload: serde_json::Value| {
            let state = state.clone();
            let app_config = app_config.clone();
            async move {
                let port: u16 = match payload.get("port") {
                    Some(v) => {
                        if let Some(p) = v.as_u64() {
                            p as u16
                        } else {
                            8001
                        }
                    }
                    None => 8001,
                };
                let name: String = match payload.get("name") {
                    Some(v) => {
                        if let Some(s) = v.as_str() {
                            s.to_string()
                        } else {
                            format!("Session on port {}", port)
                        }
                    }
                    None => format!("Session on port {}", port),
                };
                let preset_id: String = match payload.get("preset_id") {
                    Some(v) => {
                        if let Some(s) = v.as_str() {
                            s.to_string()
                        } else {
                            return Ok::<_, warp::Rejection>(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Invalid preset_id"}),
                            ));
                        }
                    }
                    None => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Missing preset_id"}),
                        ));
                    }
                };

                let preset = {
                    let presets = state.presets.lock().unwrap();
                    match presets.iter().find(|p| p.id == preset_id).cloned() {
                        Some(p) => p,
                        None => {
                            return Ok::<_, warp::Rejection>(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Preset not found"}),
                            ));
                        }
                    }
                };

                let session_id = app_state::generate_session_id();
                let session = app_state::Session::new_spawn(
                    session_id.clone(),
                    name.clone(),
                    port,
                    preset_id,
                );

                if !state.add_session(session) {
                    return Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "Failed to create session"}),
                    ));
                }

                state.set_active_session(&session_id);

                let config = crate::llama::server::ServerConfig {
                    model_path: preset.model_path.clone(),
                    context_size: preset.context_size,
                    ctk: preset.ctk.clone(),
                    ctv: preset.ctv.clone(),
                    tensor_split: preset.tensor_split.clone(),
                    batch_size: preset.batch_size,
                    ubatch_size: preset.ubatch_size,
                    no_mmap: preset.no_mmap,
                    port,
                    ngram_spec: preset.ngram_spec,
                    parallel_slots: preset.parallel_slots,
                    temperature: preset.temperature,
                    top_p: preset.top_p,
                    top_k: preset.top_k,
                    min_p: preset.min_p,
                    repeat_penalty: preset.repeat_penalty,
                    n_cpu_moe: preset.n_cpu_moe,
                    gpu_layers: preset.gpu_layers,
                    mlock: preset.mlock,
                    flash_attn: preset.flash_attn.clone(),
                    split_mode: preset.split_mode.clone(),
                    main_gpu: preset.main_gpu,
                    threads: preset.threads,
                    threads_batch: preset.threads_batch,
                    rope_scaling: preset.rope_scaling.clone(),
                    rope_freq_base: preset.rope_freq_base,
                    rope_freq_scale: preset.rope_freq_scale,
                    draft_model: preset.draft_model.clone(),
                    draft_min: preset.draft_min,
                    draft_max: preset.draft_max,
                    spec_ngram_size: preset.spec_ngram_size,
                    seed: preset.seed,
                    system_prompt_file: preset.system_prompt_file.clone(),
                    extra_args: preset.extra_args.clone(),
                };

                match crate::llama::server::start_server(&state, config, &app_config).await {
                    Ok(()) => {
                        state.update_session_status(&session_id, SessionStatus::Running);
                        Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": true, "session_id": session_id}),
                        ))
                    }
                    Err(e) => {
                        state.remove_session(&session_id);
                        Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ))
                    }
                }
            }
        })
}

fn api_attach(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "attach")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |payload: serde_json::Map<String, serde_json::Value>| {
            let state = state.clone();
            async move {
                let endpoint: String = match payload.get("endpoint") {
                    Some(v) => {
                        if let Some(s) = v.as_str() {
                            // Validate: must be http/https scheme with private/loopback host
                            let parsed = url::Url::parse(s).map_err(|_| warp::reject::not_found())?;
                            if !["http", "https"].contains(&parsed.scheme()) {
                                return Ok::<_, warp::Rejection>(warp::reply::json(
                                    &serde_json::json!({"ok": false, "error": "Endpoint must use http:// or https://"}),
                                ));
                            }
                            if let Some(host) = parsed.host_str()
                                && let Ok(ip) = host.parse::<std::net::IpAddr>() {
                                    // is_private() is unstable; inline the check
                                    let private = matches!(ip, std::net::IpAddr::V4(v4)
                                        if v4.octets()[0] == 10
                                            || (v4.octets()[0] == 172 && (4..=11).contains(&v4.octets()[1]))
                                            || (v4.octets()[0] == 192 && v4.octets()[1] == 168));
                                    if !ip.is_loopback() && !private {
                                        return Ok::<_, warp::Rejection>(warp::reply::json(
                                            &serde_json::json!({"ok": false, "error": "Endpoint must be on a private network"}),
                                        ));
                                    }
                                }
                            s.to_string()
                        } else {
                            return Ok::<_, warp::Rejection>(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Invalid endpoint"}),
                            ));
                        }
                    }
                    None => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Missing endpoint"}),
                        ));
                    }
                };

                // Pre-attach health check
                let client = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(5))
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({
                                "ok": false,
                                "error": format!("Failed to create HTTP client: {}", e)
                            }),
                        ));
                    }
                };

                // Check if server is reachable
                let server_up = client.get(&endpoint).send().await.is_ok();
                if !server_up {
                    return Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({
                            "ok": false,
                            "error": format!("Cannot reach llama-server at {}. Is it running?", endpoint)
                        }),
                    ));
                }

                // Check if metrics endpoint is available
                let metrics_available = client
                    .get(format!("{}/health", endpoint.trim_end_matches('/')))
                    .send()
                    .await
                    .is_ok();

                // Check if there's already an attach session for this endpoint
                let existing_session_id = {
                    let sessions = state.sessions.lock().unwrap();
                    sessions.iter().find(|s| {
                        if let crate::state::SessionMode::Attach { endpoint: ep } = &s.mode {
                            *ep == endpoint
                        } else {
                            false
                        }
                    }).map(|s| s.id.clone())
                };

                let session_id = if let Some(id) = existing_session_id {
                    // Reuse existing session
                    eprintln!("[info] Reusing existing attach session for {}", endpoint);
                    id
                } else {
                    // Create new session
                    let session_id = crate::state::generate_session_id();
                    let session = crate::state::Session::new_attach(
                        session_id.clone(),
                        format!("Attached: {}", endpoint),
                        endpoint,
                    );
                    if !state.add_session(session) {
                        return Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Maximum sessions reached"}),
                        ));
                    }
                    session_id
                };

                state.set_active_session(&session_id);
                state.llama_poll_notify.notify_waiters();
                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                    "ok": true,
                    "warning": if !metrics_available {
                        Some("llama-server is running but metrics endpoint (/health) is unavailable. Inference metrics will not be available. Start llama-server with --metrics flag to enable metrics.")
                    } else {
                        None
                    }
                })))
            }
        })
}

fn api_detach(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "detach")
        .and(warp::path::end())
        .and(warp::post())
        .and_then(move || {
            let state = state.clone();
            async move {
                let active_id = state.active_session_id.lock().unwrap().clone();
                if active_id.is_empty() {
                    return Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "No active session to detach from"}),
                    ));
                }

                // Check if the active session is an attach session
                let sessions = state.sessions.lock().unwrap();
                let session = sessions.iter().find(|s| s.id == active_id);

                let is_attach = session.map(|s| matches!(s.mode, crate::state::SessionMode::Attach { .. }));

                if !is_attach.unwrap_or(false) {
                    return Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "Active session is not an attach session"}),
                    ));
                }

                drop(sessions);
                // Clear the active session only - server_running is managed by the poller
                state.set_active_session("");

                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": true})))
            }
        })
}

fn api_kill_llama(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "kill-llama")
        .and(warp::post())
        .and_then(move || {
            let _state = state.clone();
            async move {
                #[cfg(target_os = "windows")]
                {
                    match Command::new("taskkill")
                        .args(["/IM", "llama-server.exe", "/F"])
                        .output()
                    {
                        Ok(output) => {
                            if output.status.success() {
                                Ok::<_, warp::Rejection>(warp::reply::json(
                                    &serde_json::json!({"ok": true}),
                                ))
                            } else {
                                let err = String::from_utf8_lossy(&output.stderr);
                                Ok::<_, warp::Rejection>(warp::reply::json(
                                    &serde_json::json!({"ok": false, "error": err}),
                                ))
                            }
                        }
                        Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        )),
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    match Command::new("pkill").args(["-f", "llama-server"]).output() {
                        Ok(output) => {
                            if output.status.success() {
                                Ok::<_, warp::Rejection>(warp::reply::json(
                                    &serde_json::json!({"ok": true}),
                                ))
                            } else {
                                let err = String::from_utf8_lossy(&output.stderr);
                                Ok::<_, warp::Rejection>(warp::reply::json(
                                    &serde_json::json!({"ok": false, "error": err}),
                                ))
                            }
                        }
                        Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        )),
                    }
                }
                #[cfg(target_os = "macos")]
                {
                    match Command::new("pkill").args(["-f", "llama-server"]).output() {
                        Ok(output) => {
                            if output.status.success() {
                                Ok::<_, warp::Rejection>(warp::reply::json(
                                    &serde_json::json!({"ok": true}),
                                ))
                            } else {
                                let err = String::from_utf8_lossy(&output.stderr);
                                Ok::<_, warp::Rejection>(warp::reply::json(
                                    &serde_json::json!({"ok": false, "error": err}),
                                ))
                            }
                        }
                        Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        )),
                    }
                }
                #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
                {
                    Ok::<_, warp::Rejection>(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "Unsupported platform"}),
                    ))
                }
            }
        })
}

fn api_self_update() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    warp::path!("api" / "self-update")
        .and(warp::post())
        .and_then(|| async move {
            match crate::agent::self_update_binary().await {
                Ok(result) => {
                    // All platforms: schedule exit so the OS / user can relaunch with
                    // the freshly written binary. On Windows the batch helper also
                    // restarts automatically once this PID disappears.
                    tokio::spawn(async {
                        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                        std::process::exit(0);
                    });
                    Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "tag_name": result.tag_name,
                        "restart_required": true
                    })))
                }
                Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                    "ok": false,
                    "error": e.to_string()
                }))),
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_minimal_chat_tab() -> ChatTab {
        ChatTab {
            id: "tab-1".to_string(),
            name: "Test Tab".to_string(),
            system_prompt: "You are helpful.".to_string(),
            ai_name: None,
            user_name: None,
            explicit_level: None,
            messages: vec![],
            total_input_tokens: None,
            total_output_tokens: None,
            model_params: ChatModelParams::default(),
            created_at: 0,
            updated_at: 0,
            auto_compact: None,
            compact_threshold: None,
            last_ctx_pct: None,
            active_template_id: None,
            context_notes: vec![],
            sidebar_width: 0,
        }
    }

    #[test]
    fn chat_tab_explicit_level_serialization() {
        let mut tab = make_minimal_chat_tab();
        tab.explicit_level = Some(1);

        let json = serde_json::to_string(&tab).expect("ChatTab should serialize");

        // Verify camelCase key in JSON
        assert!(
            json.contains("\"explicitLevel\""),
            "JSON should contain camelCase 'explicitLevel' field, got: {}",
            json
        );

        // Verify value is correct
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("JSON should parse to Value");
        assert_eq!(
            parsed.get("explicitLevel").and_then(|v| v.as_u64()),
            Some(1),
            "explicitLevel should be 1"
        );

        // Deserialize back and verify
        let deserialized: ChatTab =
            serde_json::from_str(&json).expect("ChatTab should deserialize from own JSON");
        assert_eq!(
            deserialized.explicit_level,
            Some(1),
            "explicit_level should round-trip to Some(1)"
        );
    }

    #[test]
    fn chat_tab_explicit_level_default() {
        let json = r#"{
            "id": "tab-1",
            "name": "Test Tab",
            "system_prompt": "You are helpful.",
            "messages": [],
            "model_params": {
                "temperature": 0.7,
                "top_p": 0.9,
                "top_k": 40,
                "min_p": 0.01,
                "repeat_penalty": 1.0
            },
            "created_at": 0,
            "updated_at": 0
        }"#;

        let result = serde_json::from_str::<ChatTab>(json);
        assert!(
            result.is_ok(),
            "Should deserialize without explicitLevel field"
        );

        let tab = result.unwrap();
        assert!(
            tab.explicit_level.is_none(),
            "explicit_level should default to None when field is absent"
        );
    }

    #[test]
    fn chat_tab_explicit_mode_alias_migration() {
        // The serde alias "explicit_mode" allows deserialization of JSON that uses
        // the legacy field name (instead of camelCase "explicitLevel").
        // The value must still be a u8 to match the explicit_level type.
        let json = r#"{
            "id": "tab-1",
            "name": "Test Tab",
            "system_prompt": "You are helpful.",
            "explicit_mode": 2,
            "messages": [],
            "model_params": {
                "temperature": 0.7,
                "top_p": 0.9,
                "top_k": 40,
                "min_p": 0.01,
                "repeat_penalty": 1.0
            },
            "created_at": 0,
            "updated_at": 0
        }"#;

        let result = serde_json::from_str::<ChatTab>(json);
        assert!(
            result.is_ok(),
            "Should deserialize legacy 'explicit_mode' field via alias"
        );

        let tab = result.unwrap();
        assert_eq!(
            tab.explicit_level,
            Some(2),
            "explicit_mode alias should map to explicit_level"
        );
    }

    #[test]
    fn chat_tab_explicit_level_all_states() {
        for level in [0u8, 1, 2] {
            let mut tab = make_minimal_chat_tab();
            tab.explicit_level = Some(level);

            let json = serde_json::to_string(&tab)
                .unwrap_or_else(|e| panic!("ChatTab should serialize for level {}: {}", level, e));

            // Verify the camelCase key is present
            assert!(
                json.contains("\"explicitLevel\""),
                "JSON for level {} should contain 'explicitLevel'",
                level
            );

            // Deserialize back and verify value
            let deserialized: ChatTab = serde_json::from_str(&json).unwrap_or_else(|e| {
                panic!("ChatTab should deserialize for level {}: {}", level, e)
            });
            assert_eq!(
                deserialized.explicit_level,
                Some(level),
                "explicit_level should round-trip for state {}",
                level
            );
        }
    }
}
