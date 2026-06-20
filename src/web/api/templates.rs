use std::sync::Arc;

use warp::Filter;

use crate::config::AppConfig;
use crate::presets;
use crate::state::AppState;

use super::{ApiCtx, ApiRoute, box_reply, check_api_token, unauthorized_api_token, with_app_config};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    api_get_templates(state.clone(), config.clone())
        .map(box_reply)
        .or(api_create_template(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_update_template(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_delete_template(state, config).map(box_reply))
        .unify()
        .boxed()
}

fn api_get_templates(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let templates = state.templates.lock().unwrap().clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&templates)),
            ))
        })
}

// ── Personas API ───────────────────────────────────────────────────────

fn api_create_template(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |auth: Option<String>, template: presets::SystemPromptTemplate| {
                let cfg = app_config.clone();
                if !check_api_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_api_token()));
                }
                let mut templates = state.templates.lock().unwrap();
                templates.push(template.clone());
                let _ = presets::save_templates(&state.templates_path, &templates);
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(
                        &serde_json::json!({"ok": true, "template": template}),
                    )),
                ))
            },
        )
}

fn api_update_template(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates" / String)
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |id: String, auth: Option<String>, updated: presets::SystemPromptTemplate| {
                let cfg = app_config.clone();
                if !check_api_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_api_token()));
                }
                let mut templates = state.templates.lock().unwrap();
                if let Some(existing) = templates.iter_mut().find(|t| t.id == id) {
                    *existing = updated.clone();
                    let _ = presets::save_templates(&state.templates_path, &templates);
                    futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": true, "template": updated}),
                        )),
                    ))
                } else {
                    futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "template not found"}),
                        )),
                    ))
                }
            },
        )
}

fn api_delete_template(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates" / String)
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |id: String, auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let mut templates = state.templates.lock().unwrap();
            let before = templates.len();
            templates.retain(|t| t.id != id);
            if templates.len() < before {
                let _ = presets::save_templates(&state.templates_path, &templates);
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
                ))
            } else {
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "template not found"}),
                    )),
                ))
            }
        })
}
