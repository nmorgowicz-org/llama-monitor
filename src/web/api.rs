use bytes::Bytes;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
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
    let get_models = api_get_models(state.clone());
    let refresh_models = api_refresh_models(state.clone());
    let get_gpu_env = api_get_gpu_env(state.clone());
    let put_gpu_env = api_put_gpu_env(state.clone());
    let get_settings = api_get_settings(state.clone());
    let put_settings = api_put_settings(state.clone());
    let browse = api_browse();
    let chat = api_chat(state.clone());
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

    start
        .or(stop)
        .or(create_preset)
        .or(update_preset)
        .or(delete_preset)
        .or(reset_presets)
        .or(get_presets)
        .or(get_models)
        .or(refresh_models)
        .or(put_gpu_env)
        .or(get_gpu_env)
        .or(put_settings)
        .or(get_settings)
        .or(kill_llama)
        .or(browse)
        .or(chat)
        .or(get_sessions)
        .or(create_session)
        .or(delete_session)
        .or(get_active_session)
        .or(set_active_session)
        .or(get_capabilities)
        .or(spawn_session_with_preset)
        .or(attach)
        .or(detach)
        .or(check_lhm)
        .or(start_lhm)
        .or(progress_lhm)
        .or(status_lhm)
        .or(install_lhm)
        .or(uninstall_lhm)
        .or(disable_lhm)
        .or(remote_agent_latest)
        .or(remote_agent_detect)
        .or(remote_agent_host_key)
        .or(remote_agent_trust_host)
        .or(remote_agent_status)
        .or(api_remote_agent_install(app_config.clone()))
        .or(api_remote_agent_start(app_config.clone()))
        .or(api_remote_agent_update(app_config.clone()))
        .or(api_remote_agent_stop(app_config))
        .or(remote_agent_remove)
        .or(sensor_bridge_status)
        .or(sensor_bridge_install)
        .or(sensor_bridge_uninstall)
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
            let old_dir = state.ui_settings.lock().unwrap().models_dir.clone();
            let new_dir = updated.models_dir.clone();

            let mut settings = state.ui_settings.lock().unwrap();
            *settings = updated;
            let _ = app_state::save_ui_settings(&state.ui_settings_path, &settings);
            drop(settings);

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

                match client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .body(body.to_vec())
                    .send()
                    .await
                {
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        let ct = resp
                            .headers()
                            .get("content-type")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("application/json")
                            .to_string();
                        let bytes = resp
                            .bytes()
                            .await
                            .map_err(|e| warp::reject::custom(ApiError(e.to_string())))?;
                        Ok::<_, warp::Rejection>(
                            warp::http::Response::builder()
                                .status(status)
                                .header("content-type", ct)
                                .body(bytes)
                                .unwrap(),
                        )
                    }
                    Err(e) => {
                        let err = format!(
                            r#"{{"error":{{"message":"{}","type":"proxy_error"}}}}"#,
                            e.to_string().replace('"', "'")
                        );
                        Ok(warp::http::Response::builder()
                            .status(502)
                            .header("content-type", "application/json")
                            .body(Bytes::from(err))
                            .unwrap())
                    }
                }
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
