use std::sync::Arc;

use warp::Filter;

use crate::config::AppConfig;
use crate::state::AppState;

use super::{
    ApiCtx, ApiRoute, box_reply, check_api_token, unauthorized_api_token, with_app_config,
};

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
            let config = state.sleep_mode_config.lock().unwrap().clone();
            let mode_str = state.sleep_mode_str();
            let enabled = mode_str != "off";
            Box::new(warp::reply::json(&serde_json::json!({
                "mode": mode_str,
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

            let is_manual = state
                .sleep_mode_manual
                .load(std::sync::atomic::Ordering::Relaxed);
            let current = state.sleep_mode.load(std::sync::atomic::Ordering::Relaxed);

            // Auto-sleep cycle: off <-> sleep (skip logs-only)
            // Manual cycle: off -> logs-only -> sleep -> off
            let next = if is_manual {
                // Full 3-way cycle
                match current {
                    0 => 1u8, // off -> logs-only
                    1 => 2u8, // logs-only -> sleep
                    _ => 0u8, // sleep -> off
                }
            } else {
                // Binary: off <-> sleep
                if current == 0 {
                    2u8 // off -> sleep
                } else {
                    0u8 // sleep -> off
                }
            };

            // When user manually cycles (including to logs-only), mark as manual.
            state
                .sleep_mode_manual
                .store(next != 0, std::sync::atomic::Ordering::Relaxed);
            state
                .sleep_mode
                .store(next, std::sync::atomic::Ordering::Relaxed);
            state.sleep_notify.notify_waiters();

            eprintln!(
                "[monitoring] manual toggle: sleep_mode={} (manual={})",
                state.sleep_mode_str(),
                next != 0
            );

            let mode_str = state.sleep_mode_str();
            Box::new(warp::reply::json(&serde_json::json!({
                "ok": true,
                "mode": mode_str,
                "enabled": mode_str != "off",
                "sleep_mode": mode_str != "off"
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

                // Prefer explicit "mode" field; fall back to "enabled" for backward compat.
                let mode_val = body.get("mode").and_then(|v| v.as_str()).unwrap_or("off");

                let next: u8 = match mode_val {
                    "off" => 0,
                    "logs-only" => 1,
                    "sleep" => 2,
                    _ => {
                        // Fallback for legacy boolean "enabled"
                        let enabled = body
                            .get("enabled")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        if enabled {
                            2 // sleep
                        } else {
                            0 // off
                        }
                    }
                };

                // Any explicit set is considered manual
                state
                    .sleep_mode_manual
                    .store(next != 0, std::sync::atomic::Ordering::Relaxed);
                state
                    .sleep_mode
                    .store(next, std::sync::atomic::Ordering::Relaxed);
                state.sleep_notify.notify_waiters();

                let mode_str = state.sleep_mode_str();
                Box::new(warp::reply::json(&serde_json::json!({
                    "ok": true,
                    "mode": mode_str,
                    "enabled": mode_str != "off",
                    "sleep_mode": mode_str != "off"
                }))) as Box<dyn warp::reply::Reply>
            },
        )
}
