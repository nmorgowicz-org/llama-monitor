pub mod api;
pub mod auth;
#[path = "../gen/routes.rs"]
pub mod gen_routes;
#[path = "../gen/static_assets.rs"]
pub mod static_assets;
pub mod ws;

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use warp::Filter;
use warp::http::Method;
use warp_helmet::{ContentSecurityPolicy, Helmet, HelmetFilter};

use crate::config::AppConfig;
use crate::state::AppState;
use auth::AuthManager;

/// Global rate limiter: 200 req/s with 500 burst (generous for local-first use).
fn global_rate_limit() -> impl Filter<Extract = ((),), Error = warp::Rejection> + Clone {
    static WINDOW_START: AtomicU64 = AtomicU64::new(0);
    static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);

    let max_per_second = 200u64;
    let burst_allowance = 500u64;

    warp::any().and_then(move || async move {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let window_start = WINDOW_START.load(Ordering::Relaxed);
        let count = REQUEST_COUNT.load(Ordering::Relaxed);

        if now != window_start {
            WINDOW_START.store(now, Ordering::Relaxed);
            REQUEST_COUNT.store(1, Ordering::Relaxed);
            Ok(())
        } else if count >= max_per_second + burst_allowance {
            Err(warp::reject::custom(RateLimitReject))
        } else {
            REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    })
}

#[derive(Debug)]
struct RateLimitReject;

impl warp::reject::Reject for RateLimitReject {}

/// Warp filter: validate Origin header for mutating methods on /api/* routes.
/// - If Origin is present and method is POST/PUT/PATCH/DELETE, it must match
///   the server's own origin (host:port). Mismatch → 403.
/// - If Origin is absent, allow the request (curl, tools, etc.).
/// - GET is always allowed.
/// - When server is bound to 0.0.0.0, only the port is validated.
fn origin_guard(
    server_origin: String,
) -> impl Filter<Extract = ((),), Error = warp::Rejection> + Clone {
    let server_origin = server_origin.clone();
    warp::header::optional::<String>("origin")
        .and(warp::method())
        .and_then(move |origin: Option<String>, method: Method| {
            let server_origin = server_origin.clone();
            async move {
                if matches!(method, Method::POST | Method::PUT | Method::PATCH | Method::DELETE)
                    && origin.is_some()
                {
                    let origin = origin.unwrap_or_default();
                    let origin_host = origin
                        .trim_start_matches("http://")
                        .trim_start_matches("https://")
                        .trim_end_matches('/');

                    if !origin_host.is_empty() {
                        // If bound to 0.0.0.0, only check port matches
                        if server_origin.starts_with("0.0.0.0:") {
                            if !origin_host.ends_with(&server_origin[5..]) {
                                return Err(warp::reject::custom(OriginReject));
                            }
                        } else if origin_host != server_origin {
                            return Err(warp::reject::custom(OriginReject));
                        }
                    }
                }
                Ok(())
            }
        })
}

#[derive(Debug)]
struct OriginReject;

impl warp::reject::Reject for OriginReject {}

pub fn build_routes(
    state: AppState,
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
    bind_host: String,
) -> impl Filter<Extract = (impl warp::Reply,), Error = Infallible> + Clone {
    // Construct the server origin for Origin validation (host:port).
    // For 0.0.0.0, we accept any host with the correct port.
    let server_origin = format!("{}:{}", bind_host, app_config.port);

    let ws = ws::ws_route(state.clone());
    let api = api::api_routes(
        state,
        app_config.clone(),
        auth_manager.clone(),
        bind_host.clone(),
    );
    let public_api = api::auth_api_routes(auth_manager.clone());
    let static_files = static_routes();
    let compact = compact_route(app_config);
    let index = index_route(auth_manager.clone());

    // Apply Origin guard to api routes (mutating methods with Origin must match)
    let api_protected = api.and(origin_guard(server_origin)).map(|reply, ()| reply);

    let protected = ws.or(api_protected).or(compact);
    let protected = protected
        .and(auth_guard(auth_manager.clone()))
        .map(|reply, _: ()| reply);

    // Combine all non-index routes; helmet applies its CSP to these
    let non_index = protected.or(public_api).or(static_files);

    // Apply HTTP security headers to non-index routes
    // Custom CSP: allow external CDN scripts, fonts, styles, and data URIs (app requirements)
    // connect-src allows any HTTPS — needed for API calls and WebSocket connections
    // No 'unsafe-inline' for scripts.
    let csp = ContentSecurityPolicy::new()
        .default_src(vec!["'self'", "data:"])
        .connect_src(vec!["'self'", "https:", "wss:"])
        .script_src(vec!["'self'", "https://cdn.jsdelivr.net"])
        .style_src(vec![
            "'self'",
            "https://fonts.googleapis.com",
            "https://cdn.jsdelivr.net",
        ])
        .font_src(vec!["'self'", "https://fonts.gstatic.com"])
        .img_src(vec!["'self'", "data:", "https:"])
        .frame_src(vec!["'self'"]);
    let helmet: HelmetFilter = Helmet::new().add(csp).try_into().unwrap();
    let non_index = helmet.wrap(non_index);

    // Apply global rate limit to all routes (200 req/s, 500 burst).
    let rate_limited = global_rate_limit();
    let non_index = non_index.and(rate_limited).map(|reply, ()| reply);

    index.or(non_index).recover(handle_rejection)
}

/// Auth guard for protected routes.
fn auth_guard(
    auth_manager: AuthManager,
) -> impl Filter<Extract = ((),), Error = warp::Rejection> + Clone {
    warp::header::optional::<String>("Authorization")
        .and(warp::header::optional::<String>("cookie"))
        .and_then(
            move |auth_header: Option<String>, cookie_header: Option<String>| {
                let auth_manager = auth_manager.clone();
                async move {
                    if !auth_manager.has_any() {
                        return Ok(());
                    }
                    if auth_manager
                        .authenticate_request(auth_header.as_deref(), cookie_header.as_deref())
                    {
                        Ok(())
                    } else {
                        Err(warp::reject::custom(AuthReject {
                            challenge_basic: auth_manager.has_basic() && !auth_manager.has_form(),
                        }))
                    }
                }
            },
        )
}

#[derive(Debug)]
struct AuthReject {
    challenge_basic: bool,
}

impl warp::reject::Reject for AuthReject {}

use base64::Engine;
use once_cell::sync::Lazy;
static BASE64: Lazy<base64::engine::GeneralPurpose> =
    Lazy::new(|| base64::engine::general_purpose::STANDARD);

fn compact_route(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let port = app_config.port;
    warp::path("compact").and(warp::get()).map(move || {
        // Generate a per-request CSP nonce for inline script
        let nonce_bytes: [u8; 16] = rand_core_getrandom_u128().to_be_bytes();
        let nonce = BASE64.encode(nonce_bytes);

        // Inject port and nonce
        let html = static_assets::COMPACT_HTML
            .replace("__PORT__", &port.to_string())
            .replace("<script>", &format!("<script nonce=\"{}\">", nonce));

        // CSP for compact: nonce for inline script; 'unsafe-inline' for styles (inline <style> tag)
        let csp = format!(
            "default-src 'self' data:; \
             connect-src 'self' https: wss:; \
             script-src 'self' 'nonce-{}'; \
             style-src 'self' 'unsafe-inline'; \
             font-src 'self'; \
             img-src 'self' data:; \
             frame-src 'self'",
            nonce
        );

        warp::reply::with_header(
            warp::reply::with_header(
                warp::reply::with_header(html, "content-type", "text/html"),
                "content-security-policy",
                csp,
            ),
            "cache-control",
            "no-cache, no-store, must-revalidate",
        )
    })
}

fn static_routes() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    // All other static assets are served by generated routes
    gen_routes::static_routes()
}

fn index_route(
    auth_manager: AuthManager,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    // Special handling for index.html: inject version, platform, and CSP nonce
    warp::path::end()
        .and(warp::header::optional::<String>("Authorization"))
        .and(warp::header::optional::<String>("cookie"))
        .and_then(move |auth_header: Option<String>, cookie_header: Option<String>| {
            let auth_manager = auth_manager.clone();
            async move {
                if auth_manager.has_basic()
                    && !auth_manager.has_form()
                    && !auth_manager.authenticate_request(
                        auth_header.as_deref(),
                        cookie_header.as_deref(),
                    )
                {
                    return Err(warp::reject::custom(AuthReject {
                        challenge_basic: true,
                    }));
                }

                // Generate a per-request CSP nonce (URL-safe base64, 16 bytes)
                let nonce_bytes: [u8; 16] = rand_core_getrandom_u128().to_be_bytes();
                let nonce = BASE64.encode(nonce_bytes);

                let html = static_assets::INDEX_HTML
                    .replace("{{ VERSION }}", env!("CARGO_PKG_VERSION"))
                    .replace("{{ PLATFORM }}", std::env::consts::OS)
                    .replace("{{ NONCE }}", &nonce);

                // CSP for index.html: same as global, plus nonce for the version script
                // style-src keeps 'unsafe-inline' because index.html uses inline styles (display:none, etc.)
                let csp = format!(
                    "default-src 'self' data:; \
                     connect-src 'self' https: wss:; \
                     script-src 'self' 'nonce-{nonce}' https://cdn.jsdelivr.net; \
                     style-src 'self' 'unsafe-inline' https://fonts.googleapis.com https://cdn.jsdelivr.net; \
                     font-src 'self' https://fonts.gstatic.com; \
                     img-src 'self' data: https:; \
                     frame-src 'self'"
                );

                Ok::<_, warp::Rejection>(warp::reply::with_header(
                    warp::reply::with_header(html, "content-type", "text/html"),
                    "content-security-policy",
                    csp,
                ))
            }
        })
}

// Simple u128 helper for CSP nonce generation (no extra dependency)
fn rand_core_getrandom_u128() -> u128 {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id() as u128;
    // Mix timestamp, pid, and a rotating counter-like value
    ((ts.wrapping_mul(2654435761)) ^ pid).wrapping_add(0x9E3779B97F4A7C15u128)
}

async fn handle_rejection(err: warp::Rejection) -> Result<Box<dyn warp::reply::Reply>, Infallible> {
    if let Some(auth) = err.find::<AuthReject>() {
        let reply = warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
            warp::http::StatusCode::UNAUTHORIZED,
        );
        if auth.challenge_basic {
            return Ok(Box::new(warp::reply::with_header(
                reply,
                "WWW-Authenticate",
                "Basic realm=\"Llama Monitor\"",
            )));
        }
        return Ok(Box::new(reply));
    }

    if err.find::<RateLimitReject>().is_some() {
        return Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": "rate_limited" })),
            warp::http::StatusCode::TOO_MANY_REQUESTS,
        )));
    }

    if err.find::<OriginReject>().is_some() {
        return Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "ok": false,
                "error": "forbidden; invalid origin"
            })),
            warp::http::StatusCode::FORBIDDEN,
        )));
    }

    Ok(Box::new(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "error": "not_found" })),
        warp::http::StatusCode::NOT_FOUND,
    )))
}
