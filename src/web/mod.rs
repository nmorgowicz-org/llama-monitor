pub mod api;
#[path = "../gen/routes.rs"]
pub mod gen_routes;
#[path = "../gen/static_assets.rs"]
pub mod static_assets;
pub mod ws;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use warp::Filter;
use warp_helmet::{ContentSecurityPolicy, Helmet, HelmetFilter};

use crate::config::AppConfig;
use crate::state::AppState;

/// Global rate limiter: 200 req/s with 500 burst (generous for local-first use).
fn global_rate_limit() -> impl Filter<Extract = ((),), Error = warp::Rejection> + Clone {
    static WINDOW_START: AtomicU64 = AtomicU64::new(0);
    static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);

    let max_per_second = 200u64;
    let burst_allowance = 500u64;

    warp::any().and_then(move || {
        async move {
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
        }
    })
}

#[derive(Debug)]
struct RateLimitReject;

impl warp::reject::Reject for RateLimitReject {}

pub fn build_routes(
    state: AppState,
    app_config: Arc<AppConfig>,
    basic_auth: Option<(String, String)>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let ws = ws::ws_route(state.clone());
    let api = api::api_routes(state, app_config.clone());
    let static_files = static_routes();
    let compact = compact_route(app_config);
    let index = index_route();

    // Combine all non-index routes; helmet applies its CSP to these
    let non_index = ws.or(api).or(static_files).or(compact);

    // Always apply auth filter; when credentials are None it passes through
    let auth = basic_auth_guard(basic_auth);
    let non_index = non_index.and(auth.clone()).map(|reply, _: ()| reply);

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

    // Index route has its own per-request CSP with nonce; auth still applies.
    let index = index.and(auth).map(|reply, _: ()| reply);

    // Apply global rate limit to all routes (200 req/s, 500 burst).
    let rate_limited = global_rate_limit();
    let non_index = non_index.and(rate_limited).map(|reply, ()| reply);

    index.or(non_index)
}

/// Basic Auth guard — returns Ok(()) when credentials are valid or auth is disabled.
fn basic_auth_guard(
    credentials: Option<(String, String)>,
) -> impl Filter<Extract = ((),), Error = warp::Rejection> + Clone {
    warp::header::optional::<String>("Authorization").and_then(
        move |auth_header: Option<String>| {
            let credentials = credentials.clone();
            async move {
                // If auth is not configured, pass through
                let (expected_user, expected_pass) = match credentials {
                    Some((u, p)) => (u, p),
                    None => return Ok(()),
                };

                let Some(header) = auth_header else {
                    return Err(warp::reject::custom(BasicAuthReject));
                };

                let Some(creds) = header.strip_prefix("Basic ") else {
                    return Err(warp::reject::custom(BasicAuthReject));
                };

                let decoded = match BASE64.decode(creds.as_bytes()) {
                    Ok(d) => d,
                    Err(_) => return Err(warp::reject::custom(BasicAuthReject)),
                };
                let decoded_str = match std::str::from_utf8(&decoded) {
                    Ok(s) => s,
                    Err(_) => return Err(warp::reject::custom(BasicAuthReject)),
                };

                let Some(colon_pos) = decoded_str.find(':') else {
                    return Err(warp::reject::custom(BasicAuthReject));
                };

                let user = &decoded_str[..colon_pos];
                let pass = &decoded_str[colon_pos + 1..];

                if user == expected_user && pass == expected_pass {
                    Ok(())
                } else {
                    Err(warp::reject::custom(BasicAuthReject))
                }
            }
        },
    )
}

#[derive(Debug)]
struct BasicAuthReject;

impl warp::reject::Reject for BasicAuthReject {}

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

fn index_route() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    // Special handling for index.html: inject version, platform, and CSP nonce
    warp::path::end().map(|| {
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

        warp::reply::with_header(
            warp::reply::with_header(html, "content-type", "text/html"),
            "content-security-policy",
            csp,
        )
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
