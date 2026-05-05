pub mod api;
#[path = "../gen/routes.rs"]
pub mod gen_routes;
#[path = "../gen/static_assets.rs"]
pub mod static_assets;
pub mod ws;

use std::sync::Arc;
use warp::Filter;
use warp_helmet::{ContentSecurityPolicy, Helmet, HelmetFilter};

use crate::config::AppConfig;
use crate::state::AppState;

pub fn build_routes(
    state: AppState,
    app_config: Arc<AppConfig>,
    basic_auth: Option<(String, String)>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let ws = ws::ws_route(state.clone());
    let api = api::api_routes(state, app_config.clone());
    let static_files = static_routes();
    let compact = compact_route(app_config);

    let routes = ws.or(api).or(static_files).or(compact);

    // Always apply auth filter; when credentials are None it passes through
    let auth = basic_auth_guard(basic_auth);
    let routes = routes.and(auth).map(|reply, _: ()| reply);

    // Apply HTTP security headers to all responses
    // Custom CSP: allow external CDN scripts, fonts, styles, and data URIs (app requirements)
    // connect-src allows any HTTPS — needed for API calls and WebSocket connections
    let csp = ContentSecurityPolicy::new()
        .default_src(vec!["'self'", "data:"])
        .connect_src(vec!["'self'", "https:", "wss:"])
        .script_src(vec![
            "'self'",
            "'unsafe-inline'",
            "https://cdn.jsdelivr.net",
        ])
        .style_src(vec![
            "'self'",
            "'unsafe-inline'",
            "https://fonts.googleapis.com",
            "https://cdn.jsdelivr.net",
        ])
        .font_src(vec!["'self'", "https://fonts.gstatic.com"])
        .img_src(vec!["'self'", "data:", "https:"])
        .frame_src(vec!["'self'"]);
    let helmet: HelmetFilter = Helmet::new().add(csp).try_into().unwrap();
    helmet.wrap(routes)
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
        let html = static_assets::COMPACT_HTML.replace("__PORT__", &port.to_string());
        warp::reply::with_header(
            warp::reply::with_header(html, "content-type", "text/html"),
            "cache-control",
            "no-cache, no-store, must-revalidate",
        )
    })
}

fn static_routes() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    // Special handling for index.html: inject version and platform
    let index = warp::path::end().map(|| {
        let html = static_assets::INDEX_HTML
            .replace("{{ VERSION }}", env!("CARGO_PKG_VERSION"))
            .replace("{{ PLATFORM }}", std::env::consts::OS);
        warp::reply::html(html)
    });

    // All other static assets are served by generated routes
    index.or(gen_routes::static_routes())
}
