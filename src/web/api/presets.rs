use std::sync::Arc;

use warp::Filter;

use crate::config::AppConfig;
use crate::presets::{self, ModelPreset};
use crate::state::AppState;

use super::{
    ApiCtx, ApiRoute, box_reply, check_api_token, unauthorized_api_token, with_app_config,
};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    api_get_presets(state.clone(), config.clone())
        .map(box_reply)
        .or(api_get_preset(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_create_preset(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_update_preset(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_delete_preset(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_reset_presets(state, config).map(box_reply))
        .unify()
        .boxed()
}

fn api_get_presets(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let presets = state.presets.lock().unwrap().clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&presets)),
            ))
        })
}

fn api_get_preset(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets" / String)
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(
            move |id: String, auth: Option<String>, cfg: Arc<AppConfig>| {
                if !check_api_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_api_token()));
                }
                let preset = {
                    let presets = state.presets.lock().unwrap();
                    presets.iter().find(|p| p.id == id).cloned()
                };
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(&match preset {
                        Some(preset) => serde_json::json!({"ok": true, "preset": preset}),
                        None => serde_json::json!({"ok": false, "error": "preset not found"}),
                    })),
                ))
            },
        )
}

fn api_create_preset(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move |auth: Option<String>, mut preset: ModelPreset| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            if preset.id.trim().is_empty() {
                preset.id = presets::next_id();
            }

            // Populate GGUF metadata if model_path is set
            presets::ensure_gguf_metadata(&mut preset);

            let mut presets = state.presets.lock().unwrap();
            presets.push(preset.clone());
            let _ = presets::save_presets(&state.presets_path, &presets);
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(
                    &serde_json::json!({"ok": true, "preset": preset}),
                )),
            ))
        })
}

fn api_update_preset(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets" / String)
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |id: String, auth: Option<String>, mut updated: ModelPreset| {
                let cfg = app_config.clone();
                if !check_api_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_api_token()));
                }
                updated.id = id.clone();

                // Populate GGUF metadata if model_path changed/exists
                presets::ensure_gguf_metadata(&mut updated);

                let mut presets = state.presets.lock().unwrap();
                if let Some(existing) = presets.iter_mut().find(|p| p.id == id) {
                    *existing = updated.clone();
                    let _ = presets::save_presets(&state.presets_path, &presets);
                    futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": true, "preset": updated}),
                        )),
                    ))
                } else {
                    futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "preset not found"}),
                        )),
                    ))
                }
            },
        )
}

fn api_delete_preset(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets" / String)
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |id: String, auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let mut presets = state.presets.lock().unwrap();
            let before = presets.len();
            presets.retain(|p| p.id != id);
            if presets.len() < before {
                let _ = presets::save_presets(&state.presets_path, &presets);
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
                ))
            } else {
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "preset not found"}),
                    )),
                ))
            }
        })
}

fn api_reset_presets(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets" / "reset")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let defaults = presets::default_presets();
            let mut presets = state.presets.lock().unwrap();
            *presets = defaults;
            let _ = presets::save_presets(&state.presets_path, &presets);
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
            ))
        })
}

// ── Template API ───────────────────────────────────────────────────────
