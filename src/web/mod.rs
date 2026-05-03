pub mod api;
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
    let index = warp::path::end().map(|| {
        let html = static_assets::INDEX_HTML
            .replace("{{ VERSION }}", env!("CARGO_PKG_VERSION"))
            .replace("{{ PLATFORM }}", std::env::consts::OS);
        warp::reply::html(html)
    });

    // Helper: serve static JS with no-cache (force browser to reload on every request)
    fn js_reply(content: &'static str) -> impl warp::Reply {
        warp::reply::with_header(
            warp::reply::with_header(content, "content-type", "application/javascript"),
            "cache-control",
            "no-cache, no-store, must-revalidate",
        )
    }

    // Helper: serve static CSS with cache headers
    fn css_reply(content: &'static str) -> impl warp::Reply {
        warp::reply::with_header(
            warp::reply::with_header(content, "content-type", "text/css"),
            "cache-control",
            "max-age=3600",
        )
    }

    let css_tokens = warp::path("css")
        .and(warp::path("tokens.css"))
        .and(warp::get())
        .map(|| css_reply(static_assets::CSS_TOKENS));
    let css_base = warp::path("css")
        .and(warp::path("base.css"))
        .and(warp::get())
        .map(|| css_reply(static_assets::CSS_BASE));
    let css_layout = warp::path("css")
        .and(warp::path("layout.css"))
        .and(warp::get())
        .map(|| css_reply(static_assets::CSS_LAYOUT));
    let css_cards_inference = warp::path("css")
        .and(warp::path("cards-inference.css"))
        .and(warp::get())
        .map(|| css_reply(static_assets::CSS_CARDS_INFERENCE));
    let css_agent_modal = warp::path("css")
        .and(warp::path("agent-modal.css"))
        .and(warp::get())
        .map(|| css_reply(static_assets::CSS_AGENT_MODAL));
    let css_cards_hardware = warp::path("css")
        .and(warp::path("cards-hardware.css"))
        .and(warp::get())
        .map(|| css_reply(static_assets::CSS_CARDS_HARDWARE));
    let css_components = warp::path("css")
        .and(warp::path("components.css"))
        .and(warp::get())
        .map(|| css_reply(static_assets::CSS_COMPONENTS));
    let css_chat = warp::path("css")
        .and(warp::path("chat.css"))
        .and(warp::get())
        .map(|| css_reply(static_assets::CSS_CHAT));
    let css_setup_view = warp::path("css")
        .and(warp::path("setup-view.css"))
        .and(warp::get())
        .map(|| css_reply(static_assets::CSS_SETUP_VIEW));
    let css_settings_modal = warp::path("css")
        .and(warp::path("settings-modal.css"))
        .and(warp::get())
        .map(|| css_reply(static_assets::CSS_SETTINGS_MODAL));

    // Module bootstrap and supporting files (Phase 1 of app.js refactor)
    let js_bootstrap = warp::path("js")
        .and(warp::path("bootstrap.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::BOOTSTRAP_JS));
    let js_compat_globals = warp::path("js")
        .and(warp::path("compat"))
        .and(warp::path("globals.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::COMPAT_GLOBALS_JS));
    let js_core_format = warp::path("js")
        .and(warp::path("core"))
        .and(warp::path("format.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::CORE_FORMAT_JS));
    let js_core_app_state = warp::path("js")
        .and(warp::path("core"))
        .and(warp::path("app-state.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::CORE_APP_STATE_JS));
    let js_features_dashboard_ws = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("dashboard-ws.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_DASHBOARD_WS_JS));
    let js_features_file_browser = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("file-browser.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_FILE_BROWSER_JS));
    let js_features_file_browser_launcher = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("file-browser-launcher.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_FILE_BROWSER_LAUNCHER_JS));
    let js_features_presets = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("presets.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_PRESETS_JS));
    let js_features_sessions = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("sessions.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_SESSIONS_JS));
    let js_features_attach_detach = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("attach-detach.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_ATTACH_DETACH_JS));
    let js_features_animate = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("animate.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_ANIMATE_JS));
    let js_features_chat_params = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("chat-params.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_CHAT_PARAMS_JS));
    let js_features_chat_render = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("chat-render.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_CHAT_RENDER_JS));
    let js_features_chat_state = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("chat-state.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_CHAT_STATE_JS));
    let js_features_chat_templates = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("chat-templates.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_CHAT_TEMPLATES_JS));
    let js_features_chat_transport = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("chat-transport.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_CHAT_TRANSPORT_JS));
    let js_features_context_card = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("context-card.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_CONTEXT_CARD_JS));
    let js_features_config = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("config.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_CONFIG_JS));
    let js_features_lhm = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("lhm.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_LHM_JS));
    let js_features_models = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("models.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_MODELS_JS));
    let js_features_nav = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("nav.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_NAV_JS));
    let js_features_remote_agent = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("remote-agent.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_REMOTE_AGENT_JS));
    let js_features_sensor_bridge = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("sensor-bridge.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_SENSOR_BRIDGE_JS));
    let js_features_settings = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("settings.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_SETTINGS_JS));
    let js_features_setup_view = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("setup-view.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_SETUP_VIEW_JS));
    let js_features_shortcuts = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("shortcuts.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_SHORTCUTS_JS));
    let js_features_updates = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("updates.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_UPDATES_JS));
    let js_features_user_menu = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("user-menu.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_USER_MENU_JS));
    let js_features_dashboard_render = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("dashboard-render.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_DASHBOARD_RENDER_JS));
    let js_features_toast = warp::path("js")
        .and(warp::path("features"))
        .and(warp::path("toast.js"))
        .and(warp::get())
        .map(|| js_reply(static_assets::FEATURES_TOAST_JS));

    let manifest = warp::path("manifest.json").and(warp::get()).map(|| {
        warp::reply::with_header(
            static_assets::MANIFEST_JSON,
            "content-type",
            "application/manifest+json",
        )
    });

    let sw = warp::path("sw.js")
        .and(warp::get())
        .map(|| js_reply(static_assets::SW_JS));

    let icon = warp::path("icon.svg")
        .and(warp::get())
        .map(|| warp::reply::with_header(static_assets::ICON_SVG, "content-type", "image/svg+xml"));

    index
        .or(css_tokens)
        .or(css_base)
        .or(css_layout)
        .or(css_cards_inference)
        .or(css_agent_modal)
        .or(css_cards_hardware)
        .or(css_components)
        .or(css_chat)
        .or(css_setup_view)
        .or(css_settings_modal)
        .or(js_bootstrap)
        .or(js_compat_globals)
        .or(js_core_format)
        .or(js_core_app_state)
        .or(js_features_dashboard_ws)
        .or(js_features_file_browser)
        .or(js_features_file_browser_launcher)
        .or(js_features_presets)
        .or(js_features_sessions)
        .or(js_features_attach_detach)
        .or(js_features_animate)
        .or(js_features_chat_params)
        .or(js_features_chat_render)
        .or(js_features_chat_state)
        .or(js_features_chat_templates)
        .or(js_features_chat_transport)
        .or(js_features_context_card)
        .or(js_features_config)
        .or(js_features_lhm)
        .or(js_features_models)
        .or(js_features_nav)
        .or(js_features_remote_agent)
        .or(js_features_sensor_bridge)
        .or(js_features_settings)
        .or(js_features_setup_view)
        .or(js_features_shortcuts)
        .or(js_features_updates)
        .or(js_features_user_menu)
        .or(js_features_dashboard_render)
        .or(js_features_toast)
        .or(manifest)
        .or(sw)
        .or(icon)
}
