use std::sync::Arc;
use warp::Filter;

use crate::config::AppConfig;
use crate::gpu::env::{self as gpu_env, GPU_ARCHITECTURES, GpuEnv};
use crate::llama::server::{self, ServerConfig};
use crate::models;
use crate::presets::{self, ModelPreset};
use crate::state::AppState;

pub fn api_routes(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let start = api_start(state.clone(), app_config);
    let stop = api_stop(state.clone());
    let get_presets = api_get_presets(state.clone());
    let create_preset = api_create_preset(state.clone());
    let update_preset = api_update_preset(state.clone());
    let delete_preset = api_delete_preset(state.clone());
    let reset_presets = api_reset_presets(state.clone());
    let get_models = api_get_models(state.clone());
    let refresh_models = api_refresh_models(state.clone());
    let get_gpu_env = api_get_gpu_env(state.clone());
    let put_gpu_env = api_put_gpu_env(state.clone());
    let chat = api_chat(state);

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
        .or(chat)
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
                match server::start_server(&state, config, &app_config).await {
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

fn api_chat(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat")
        .and(warp::post())
        .and(warp::body::bytes())
        .and_then(move |body: bytes::Bytes| {
            let state = state.clone();
            async move {
                let port = state
                    .server_config
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|c| c.port)
                    .unwrap_or(8080);
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
        })
}
