use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use warp::Filter;

use crate::config::AppConfig;
use crate::gpu::env::{self as gpu_env, GPU_ARCHITECTURES, GpuEnv};
use crate::llama::server::{self, ServerConfig};
use crate::models;
use crate::presets::{self, ModelPreset};
use crate::state::{self as app_state, AppState, SessionStatus, UiSettings};

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
    let spawn_session_with_preset = api_spawn_session_with_preset(state.clone(), app_config.clone());

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
        .or(spawn_session_with_preset)
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
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(warp::body::bytes())
        .and_then(
            move |query: std::collections::HashMap<String, String>, body: bytes::Bytes| {
                let state = state.clone();
                async move {
                    let port = query
                        .get("port")
                        .and_then(|p| p.parse::<u16>().ok())
                        .unwrap_or_else(|| {
                            state
                                .server_config
                                .lock()
                                .unwrap()
                                .as_ref()
                                .map(|c| c.port)
                                .unwrap_or(8080)
                        });
                    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");

                    let client = reqwest::Client::new();
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
                            let stream = resp.bytes_stream();
                            let body = warp::hyper::Body::wrap_stream(stream);
                            Ok::<_, warp::Rejection>(
                                warp::http::Response::builder()
                                    .status(status)
                                    .header("content-type", ct)
                                    .body(body)
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
                                .body(warp::hyper::Body::from(err))
                                .unwrap())
                        }
                    }
                }
            },
        )
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
                    Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": "Maximum sessions reached"})))
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
                    Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": "Session not found"})))
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
                            crate::state::SessionMode::Attach { endpoint } => format!("Attach:{}", endpoint),
                        };
                        Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                            "id": s.id,
                            "name": s.name,
                            "mode": mode_str,
                            "status": s.status,
                            "last_active": s.last_active
                        })))
                    }
                    None => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"error": "No active session"})))
                }
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
                    None => return Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": "Missing session id"}))),
                };
                if state.set_active_session(&session_id) {
                    Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": true})))
                } else {
                    Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": "Session not found"})))
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
                // Extract port, name, and preset_id from payload
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
                            return Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": "Invalid preset_id"})));
                        }
                    }
                    None => {
                        return Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": "Missing preset_id"})));
                    }
                };
                
                // Load the preset (scope the lock)
                let preset = {
                    let presets = state.presets.lock().unwrap();
                    match presets.iter().find(|p| p.id == preset_id).cloned() {
                        Some(p) => p,
                        None => return Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": "Preset not found"}))),
                    }
                };
                
                // Create session
                let session_id = app_state::generate_session_id();
                let session = app_state::Session::new_spawn(session_id.clone(), name.clone(), port);
                
                if !state.add_session(session) {
                    return Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": "Failed to create session"})));
                }
                
                // Set as active
                state.set_active_session(&session_id);
                
                // Build server config from preset
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
                        // Update session status to Running
                        state.update_session_status(&session_id, SessionStatus::Running);
                        Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": true, "session_id": session_id})))
                    }
                    Err(e) => {
                        state.remove_session(&session_id);
                        Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()})))
                    }
                }
            }
        })
}

fn api_kill_llama(state: AppState) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
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
                                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": true})))
                            } else {
                                let err = String::from_utf8_lossy(&output.stderr);
                                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": err})))
                            }
                        }
                        Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()}))),
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    match Command::new("pkill")
                        .args(["-f", "llama-server"])
                        .output()
                    {
                        Ok(output) => {
                            if output.status.success() {
                                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": true})))
                            } else {
                                let err = String::from_utf8_lossy(&output.stderr);
                                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": err})))
                            }
                        }
                        Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()}))),
                    }
                }
                #[cfg(target_os = "macos")]
                {
                    match Command::new("pkill")
                        .args(["-f", "llama-server"])
                        .output()
                    {
                        Ok(output) => {
                            if output.status.success() {
                                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": true})))
                            } else {
                                let err = String::from_utf8_lossy(&output.stderr);
                                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": err})))
                            }
                        }
                        Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()}))),
                    }
                }
                #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
                {
                    Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": false, "error": "Unsupported platform"})))
                }
            }
        })
}
