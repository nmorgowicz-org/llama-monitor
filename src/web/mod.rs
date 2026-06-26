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
use api::ApiError;
use auth::AuthManager;

/// Global rate limiter: essentially unlimited for local-first use.
fn global_rate_limit() -> impl Filter<Extract = ((),), Error = warp::Rejection> + Clone {
    static WINDOW_START: AtomicU64 = AtomicU64::new(0);
    static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);

    let limit = u64::MAX; // effectively unlimited

    warp::any().and_then(move || async move {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let window_start = WINDOW_START.load(Ordering::Acquire);

        if now != window_start {
            // Atomically claim the window reset; only one thread wins the CAS.
            if WINDOW_START
                .compare_exchange(window_start, now, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                REQUEST_COUNT.store(1, Ordering::Release);
                return Ok(());
            }
            // Another thread already reset the window; fall through to count.
        }

        // Increment first, then check — avoids the load/check/add TOCTOU.
        let prev = REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);
        if prev >= limit {
            REQUEST_COUNT.fetch_sub(1, Ordering::Relaxed);
            Err(warp::reject::custom(RateLimitReject))
        } else {
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
                if matches!(
                    method,
                    Method::POST | Method::PUT | Method::PATCH | Method::DELETE
                ) && origin.is_some()
                {
                    let origin = origin.unwrap_or_default();
                    let origin_host = origin
                        .trim_start_matches("http://")
                        .trim_start_matches("https://")
                        .trim_end_matches('/');

                    if !origin_host.is_empty() {
                        // If bound to 0.0.0.0 accept any host on the correct port.
                        // Use exact port comparison, not ends_with, to prevent suffix bypass.
                        if let Some(server_port) = server_origin.strip_prefix("0.0.0.0:") {
                            let origin_port = origin_host.rsplit(':').next().unwrap_or("");
                            if origin_port != server_port {
                                return Err(warp::reject::custom(OriginReject));
                            }
                        } else {
                            // Normalize localhost <-> 127.0.0.1: browsers may use either
                            // for the same loopback address.
                            let normalize = |h: &str| {
                                if let Some(port) = h.strip_prefix("localhost:") {
                                    format!("127.0.0.1:{}", port)
                                } else if let Some(port) = h.strip_prefix("127.0.0.1:") {
                                    format!("127.0.0.1:{}", port)
                                } else {
                                    h.to_string()
                                }
                            };
                            if normalize(origin_host) != normalize(&server_origin) {
                                return Err(warp::reject::custom(OriginReject));
                            }
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
    let public_api = api::auth_api_routes(auth_manager.clone()).or(api::public_tokens_routes(
        app_config.clone(),
        auth_manager.clone(),
        bind_host.clone(),
    ));
    let static_files = static_routes();
    let compact = compact_route(app_config.clone());
    let index = index_route(auth_manager.clone());

    // Apply Origin guard to api routes (mutating methods with Origin must match)
    let api_protected = api.and(origin_guard(server_origin)).map(|reply, ()| reply);

    // compact is auth-guarded but excluded from helmet so its own per-request CSP
    // (which needs 'unsafe-inline' for the inline <style> block and a nonce for the
    // inline <script> that injects __COMPACT_PORT__) is the only CSP header sent.
    // Two CSP headers would cause browsers to apply both and pick the most restrictive,
    // silently blocking the inline styles and the port-injection script.
    let compact_protected = compact
        .and(auth_guard(auth_manager.clone(), app_config.clone()))
        .map(|reply, _: ()| reply);

    let protected = ws.or(api_protected);
    let protected = protected
        .and(auth_guard(auth_manager.clone(), app_config.clone()))
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
    let non_index = non_index.and(rate_limited.clone()).map(|reply, ()| reply);
    let compact_protected = compact_protected.and(rate_limited).map(|reply, ()| reply);

    // Route priority:
    // - non_index: /api/*, /ws, static assets (no SPA fallback)
    // - compact_protected: /compact with its own CSP
    // - index: exact root '/' plus SPA fallback for any other GET path
    // Only GET is allowed via index/SPA fallback; non-GET unmatched → 404 via handle_rejection.
    non_index
        .or(compact_protected)
        .or(index)
        .recover(handle_rejection)
}

/// Auth guard for protected routes.
///
/// Allows:
/// - No auth configured.
/// - Valid session cookie (Form Login).
/// - Valid Basic Auth header.
/// - Valid api-token (Bearer) for programmatic and internal API calls.
fn auth_guard(
    auth_manager: AuthManager,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = ((),), Error = warp::Rejection> + Clone {
    warp::header::optional::<String>("Authorization")
        .and(warp::header::optional::<String>("cookie"))
        .and_then(
            move |auth_header: Option<String>, cookie_header: Option<String>| {
                let auth_manager = auth_manager.clone();
                let cfg = app_config.clone();
                async move {
                    // No auth configured: allow all requests.
                    if !auth_manager.has_any() {
                        return Ok(());
                    }

                    // Allow if Form Login session is valid.
                    if auth_manager
                        .authenticate_request(auth_header.as_deref(), cookie_header.as_deref())
                    {
                        return Ok(());
                    }

                    // Allow if a valid api-token is present (Bearer).
                    // This lets internal/API callers (chat, remote-agent, etc.) bypass
                    // the UI-level auth guard while still enforcing their own token auth.
                    if let Some(token) = extract_bearer_token(auth_header.as_deref())
                        && api::check_api_token(&Some(format!("Bearer {token}")), &cfg)
                    {
                        return Ok(());
                    }

                    Err(warp::reject::custom(AuthReject {
                        challenge_basic: auth_manager.has_basic() && !auth_manager.has_form(),
                    }))
                }
            },
        )
}

fn extract_bearer_token(header: Option<&str>) -> Option<&str> {
    let header = header?;
    let without_prefix = header.strip_prefix("Bearer ")?;
    let token = without_prefix.trim();
    if token.is_empty() { None } else { Some(token) }
}

#[derive(Debug)]
struct AuthReject {
    challenge_basic: bool,
}

impl warp::reject::Reject for AuthReject {}

#[derive(Debug)]
struct JsonParseError;

impl warp::reject::Reject for JsonParseError {}

/// A warp-compatible JSON body filter that:
/// - Limits size to 2MB.
/// - Returns 400 (via JsonParseError) on parse failure instead of letting it
///   fall through as a generic rejection that could become 404.
pub fn safe_json_body<T: serde::de::DeserializeOwned>()
-> impl Filter<Extract = (T,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(2_000_000)
        .and(warp::body::bytes())
        .and_then(move |body: bytes::Bytes| async move {
            let body = body.as_ref();
            serde_json::from_slice::<T>(body).map_err(|_| warp::reject::custom(JsonParseError))
        })
}

/// Like safe_json_body but with a 256 KB limit, used for HF endpoints.
pub fn hf_json_body<T: serde::de::DeserializeOwned>()
-> impl Filter<Extract = (T,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(256 * 1024)
        .and(warp::body::bytes())
        .and_then(move |body: bytes::Bytes| async move {
            let body = body.as_ref();
            serde_json::from_slice::<T>(body).map_err(|_| warp::reject::custom(JsonParseError))
        })
}

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
             style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
             font-src 'self' https://fonts.gstatic.com; \
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

fn index_route(auth_manager: AuthManager) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    // Special handling for index.html:
    // - Exact match at '/' (as before).
    // - Fallback for any GET path (SPA routing) for paths not handled by API/static/compact.
    //
    // This route is placed last in the .or() chain so that:
    // - /api/*, /ws, and static assets take priority.
    // - /compact is excluded and handled separately.

    // Helper: generate index.html with version, platform, and CSP nonce.
    let serve_index =
        warp::header::optional::<String>("Authorization")
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
            });

    let root = warp::path::end().and(serve_index.clone());

    // SPA fallback: any GET path that hasn't been matched (e.g. /chat, /logs, /settings).
    let spa_fallback = warp::path::full()
        .and(serve_index)
        .map(|_path, reply| reply);

    // Enforce: only GET allowed for SPA routes; non-GET → 404 via handle_rejection.
    let spa = root.or(spa_fallback);
    spa
        .and(warp::method())
        .and_then(|reply, method| async move {
            if method == warp::http::Method::GET {
                Ok(reply)
            } else {
                Err(warp::reject::not_found())
            }
        })
}

// Simple u128 helper for CSP nonce generation (no extra dependency)
fn rand_core_getrandom_u128() -> u128 {
    use rand::TryRng;
    use rand::rngs::SysRng;
    let mut buf = [0u8; 16];
    SysRng.try_fill_bytes(&mut buf).expect("SysRng failed");
    u128::from_be_bytes(buf)
}

pub async fn handle_rejection(
    err: warp::Rejection,
) -> Result<Box<dyn warp::reply::Reply>, Infallible> {
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

    if let Some(api_err) = err.find::<ApiError>() {
        return Ok(Box::new(warp::reply::with_status(
            api_err.message.clone(),
            api_err.status,
        )));
    }

    // JSON parse / invalid body → 400, not 404.
    if err.find::<JsonParseError>().is_some() {
        return Ok(Box::new(warp::reply::with_status(
            warp::reply::json(
                &serde_json::json!({ "ok": false, "error": "bad_request; invalid JSON" }),
            ),
            warp::http::StatusCode::BAD_REQUEST,
        )));
    }

    Ok(Box::new(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "error": "not_found" })),
        warp::http::StatusCode::NOT_FOUND,
    )))
}
