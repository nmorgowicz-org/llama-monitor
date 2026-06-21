use std::sync::Arc;

use warp::Filter;

use super::{check_api_token, with_app_config};
use crate::config::AppConfig;
use crate::web::auth::AuthManager;

/// Public token bootstrap routes.
///
/// Exposed before auth_guard so the frontend can retrieve the api-token /
/// db-admin-token without needing to be logged in via form/basic auth. Access
/// is still constrained by token_bootstrap_allowed (loopback or no auth) and
/// the caller's Origin.
pub fn public_tokens_routes(
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
    bind_host: String,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    api_internal_token(app_config.clone(), auth_manager.clone(), bind_host.clone())
        .or(api_db_admin_token(app_config, auth_manager, bind_host))
}

/// Token bootstrap policy:
/// - With any auth mode configured, the surrounding auth guard already
///   authenticated the request, so bootstrap is allowed.
/// - With no auth configured, bootstrap is restricted to loopback binds.
fn bind_host_is_loopback(bind_host: &str) -> bool {
    let host = bind_host.trim().trim_matches(['[', ']']);
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

pub(super) fn token_bootstrap_allowed(auth_manager: &AuthManager, bind_host: &str) -> bool {
    // No Auth mode: fully open (local-first).
    if !auth_manager.has_any() {
        return true;
    }
    // Auth configured: only allow bootstrap when bound to loopback.
    // Do NOT trust the Host header — attacker-controlled.
    bind_host_is_loopback(bind_host)
}

// GET /api/internal/api-token - Return internal API token for UI use
fn api_internal_token(
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
    bind_host: String,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "internal" / "api-token")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::header::optional::<String>("cookie"))
        .and(with_app_config(app_config))
        .map(
            move |auth: Option<String>, cookie: Option<String>, cfg: Arc<AppConfig>| {
                let already_authenticated = auth_manager
                    .authenticate_request(auth.as_deref(), cookie.as_deref())
                    || check_api_token(&auth, &cfg);
                if !already_authenticated && !token_bootstrap_allowed(&auth_manager, &bind_host) {
                    return Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "forbidden" })),
                        warp::http::StatusCode::FORBIDDEN,
                    )) as Box<dyn warp::reply::Reply>;
                }
                let live = cfg.live_api_token();
                let token = live.as_deref().unwrap_or("");
                Box::new(warp::reply::json(&serde_json::json!({ "token": token })))
            },
        )
}

// GET /api/db/admin-token - Return DB admin token for authenticated UI use
fn api_db_admin_token(
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
    bind_host: String,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "db" / "admin-token")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::header::optional::<String>("cookie"))
        .and(with_app_config(app_config))
        .map(
            move |auth: Option<String>, cookie: Option<String>, cfg: Arc<AppConfig>| {
                let already_authenticated = auth_manager
                    .authenticate_request(auth.as_deref(), cookie.as_deref())
                    || check_api_token(&auth, &cfg);
                if !already_authenticated && !token_bootstrap_allowed(&auth_manager, &bind_host) {
                    return Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "forbidden" })),
                        warp::http::StatusCode::FORBIDDEN,
                    )) as Box<dyn warp::reply::Reply>;
                }
                let live = cfg.live_db_admin_token();
                let token = live.as_deref().unwrap_or("");
                Box::new(warp::reply::json(&serde_json::json!({ "token": token })))
            },
        )
}
