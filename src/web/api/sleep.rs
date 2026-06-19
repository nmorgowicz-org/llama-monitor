use std::sync::Arc;

use warp::Filter;

use crate::config::AppConfig;
use crate::state::AppState;

use super::{ApiCtx, ApiReply, ApiRoute, check_api_token, unauthorized_api_token, with_app_config};

fn box_reply<R>(reply: R) -> ApiReply
where
    R: warp::Reply + 'static,
{
    Box::new(reply)
}

fn touch_activity(state: &AppState) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    state
        .last_activity_at
        .store(now, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    api_sleep_mode_get(state.clone(), config.clone())
        .map(box_reply)
        .or(api_sleep_mode_toggle(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_sleep_mode_set(state, config).map(box_reply))
        .unify()
        .boxed()
}

fn api_sleep_mode_get(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sleep-mode")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .map(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            if !check_api_token(&auth, &cfg) {
                return unauthorized_api_token();
            }
            let enabled = state.sleep_mode.load(std::sync::atomic::Ordering::Relaxed);
            let config = state.sleep_mode_config.lock().unwrap().clone();
            Box::new(warp::reply::json(&serde_json::json!({
                "enabled": enabled,
                "config": config
            }))) as Box<dyn warp::reply::Reply>
        })
}

fn api_sleep_mode_toggle(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sleep-mode" / "toggle")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .map(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            if !check_api_token(&auth, &cfg) {
                return unauthorized_api_token();
            }
            touch_activity(&state);
            let enabled = state.sleep_mode.load(std::sync::atomic::Ordering::Relaxed);
            let next = !enabled;
            state
                .sleep_mode_manual
                .store(next, std::sync::atomic::Ordering::Relaxed);
            state
                .sleep_mode
                .store(next, std::sync::atomic::Ordering::Relaxed);
            state.sleep_notify.notify_waiters();
            eprintln!(
                "[monitoring] manual toggle: monitoring={} (manual={})",
                !next, next
            );
            Box::new(warp::reply::json(&serde_json::json!({
                "ok": true,
                "enabled": next,
                "sleep_mode": next
            }))) as Box<dyn warp::reply::Reply>
        })
}

fn api_sleep_mode_set(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sleep-mode" / "set")
        .and(warp::post())
        .and(warp::body::json::<serde_json::Value>())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .map(
            move |body: serde_json::Value, auth: Option<String>, cfg: Arc<AppConfig>| {
                if !check_api_token(&auth, &cfg) {
                    return unauthorized_api_token();
                }
                touch_activity(&state);
                let enabled = body
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                state
                    .sleep_mode_manual
                    .store(enabled, std::sync::atomic::Ordering::Relaxed);
                state
                    .sleep_mode
                    .store(enabled, std::sync::atomic::Ordering::Relaxed);
                state.sleep_notify.notify_waiters();
                Box::new(warp::reply::json(&serde_json::json!({
                    "ok": true,
                    "enabled": enabled,
                    "sleep_mode": enabled
                }))) as Box<dyn warp::reply::Reply>
            },
        )
}
