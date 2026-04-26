pub mod api;
pub mod static_assets;
pub mod ws;

use std::sync::Arc;
use warp::Filter;

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
    routes.and(auth).map(|reply, _: ()| reply)
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
    let index = warp::path::end().map(|| warp::reply::html(static_assets::INDEX_HTML));

    let css = warp::path("style.css")
        .and(warp::get())
        .map(|| warp::reply::with_header(static_assets::STYLE_CSS, "content-type", "text/css"));

    let js = warp::path("app.js").and(warp::get()).map(|| {
        warp::reply::with_header(
            static_assets::APP_JS,
            "content-type",
            "application/javascript",
        )
    });

    let manifest = warp::path("manifest.json").and(warp::get()).map(|| {
        warp::reply::with_header(
            static_assets::MANIFEST_JSON,
            "content-type",
            "application/manifest+json",
        )
    });

    let sw = warp::path("sw.js").and(warp::get()).map(|| {
        warp::reply::with_header(
            static_assets::SW_JS,
            "content-type",
            "application/javascript",
        )
    });

    let lhm_js = warp::path("lhm.js").and(warp::get()).map(|| {
        warp::reply::with_header(
            static_assets::LHM_JS,
            "content-type",
            "application/javascript",
        )
    });

    let icon = warp::path("icon.svg")
        .and(warp::get())
        .map(|| warp::reply::with_header(static_assets::ICON_SVG, "content-type", "image/svg+xml"));

    index.or(css).or(js).or(lhm_js).or(manifest).or(sw).or(icon)
}
