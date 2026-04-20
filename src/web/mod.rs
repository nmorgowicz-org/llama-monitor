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
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let ws = ws::ws_route(state.clone());
    let api = api::api_routes(state, app_config.clone());
    let static_files = static_routes();
    let compact = compact_route(app_config);

    ws.or(api).or(static_files).or(compact)
}

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
