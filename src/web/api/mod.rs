use std::path::PathBuf;
use std::sync::Arc;

use warp::Filter;

mod auth;
mod benchmark;
mod chat;
#[allow(unused_imports)]
pub(crate) use chat::legacy_chat_types;
mod common;
mod config;
mod db;
mod debug;
mod hf;
mod lhm;
mod llama_binary;
mod metrics;
mod models;
#[path = "presets.rs"]
mod preset_routes;
mod remote_agent;
mod self_update;
mod sensor_bridge;
mod sleep;
mod spawn_wizard;
mod templates;
mod tls;
mod tokens;
mod upstream;
mod vram;

pub(crate) use common::ApiError;
pub use common::check_api_token;
pub(crate) use common::{ApiCtx, ApiReply, ApiRoute, record_activity};
use common::{
    bearer_matches_api_token, bearer_matches_db_admin_token, check_db_admin_token, extract_bearer,
    unauthorized_api_token, unauthorized_db_admin_token, with_app_config,
};
pub use tokens::public_tokens_routes;
#[cfg(test)]
use tokens::token_bootstrap_allowed;

use crate::config::AppConfig;
use crate::llama::server::ServerConfig;
use crate::state::{self as app_state, AppState, SessionStatus};
use crate::web::auth::AuthManager;

// ========================
// Phase 0: Spawn Llama-Server v2 endpoints
// ========================

// 1) POST /api/spawn-wizard/mtp-draft-check

// 2) POST /api/spawn-wizard/import-launch-file

// 2) POST /api/chat-template/fetch

// 3) POST /api/chat-template/upload

// 4) GET /api/chat-template/dir

// 5) POST /api/chat-template/install-hf
// Downloads a Jinja template from HuggingFace and saves it with a stable name.
// Returns the cached path immediately if the file already exists.

// 6) POST /api/chat-template/install-url
// Downloads a community template from raw.githubusercontent.com and saves it
// with a stable name. The host allowlist keeps this separate from arbitrary
// URL fetching and prevents redirects to untrusted hosts.

// 7) POST /api/vram-estimate (architecture-aware breakdown)

// 4b) POST /api/vram/estimate (legacy)

// 5) POST /api/models/download/start

// 6) GET /api/models/download/:id/status

// 7) POST /api/models/download/:id/cancel

// ── POST /api/vram/quant-compare ─────────────────────────────────────────────
// Pre-download quant advisor: returns a comparison table of all quants for a
// given model (identified by param count + optional name) and available VRAM.

// ── POST /api/vram/auto-size ──────────────────────────────────────────────────
// Given model metadata + available VRAM + use case, return recommended settings
// plus a set of alternative scenarios for the scenario cards.

// ── Phase 2: POST /api/benchmark (with 15-second cooldown) ────────────────────

// ── Phase 2: POST /api/model-defaults ────────────────────────────────────────

// ── Phase 2: POST /api/moe-tune ──────────────────────────────────────────────

// ── Config-time performance advisor ───────────────────────────────────────────
// Predictive hints (dense-vs-MoE, KV type, MTP) for the Spawn Wizard / Preset
// Editor, computed from the model architecture before any benchmark is run.

// ── n_cpu_moe auto-tuner (estimate + optional empirical verify) ────────────────

// ── Offline depth sweep via llama-bench ───────────────────────────────────────

// ── Offline batch/ubatch sweep ───────────────────────────────────────────────
//
// Tries common (batch_size, ubatch_size) pairs via llama-bench measuring
// PP throughput only (no decode). Returns all probe results plus the pair
// with the highest PP t/s. Requires the server to be stopped (GPU free).

// ── Online MTP n-max sweep ────────────────────────────────────────────────────
//
// Probes each requested spec-draft-n-max value by stop → modify config →
// start → wait for health → stream a chat completion → measure gen t/s.
// Returns all probe results plus the recommended n_max.
// Expected duration: 1–3 min per probe (model load + inference), so 4–12 min
// total for a [1,2,3,4] sweep. The HTTP call is synchronous; the client should
// display an elapsed timer while waiting.

// ── Apple Silicon: set Metal GPU wired memory limit ───────────────────────────
// Uses osascript to invoke `sysctl iogpu.wired_limit_mb=N` with administrator
// privileges via the macOS native password dialog. No password touches the app.
// Only compiled on macOS; on other platforms returns a not-supported error.

// ── P3.1: HF Search (with rate limiting) ─────────────────────────────────────
// Rate limit: 10 requests per 60 seconds (global; per-instance).

// ── P3.1: HF Files ───────────────────────────────────────────────────────────

// ── GET /api/hf/community-picks ───────────────────────────────────────────────
// Reads ~/.config/llama-monitor/community-picks.json if present.
// Produced externally (e.g. by a Hermes cron scraping r/LocalLLaMA).

// ── GET /api/hf/quantizers ────────────────────────────────────────────────────
// Returns the active quantizer list for the wizard quick-picks.
// If hf-quantizers.json exists in config_dir, that list is returned (is_custom=true).
// Otherwise the built-in defaults are returned (is_custom=false).

// ── PUT /api/hf/quantizers ────────────────────────────────────────────────────
// Saves a user-customized quantizer list to hf-quantizers.json.
// Send an empty array to reset to defaults (deletes the file).

// ── GET /api/hf/download-dir ─────────────────────────────────────────────────
// Returns the directory where HF downloads will be saved (effective models dir).

// ── GET /api/hf/card?repo=owner/model ─────────────────────────────────────────
// Fetches the raw README.md for a HuggingFace repo and returns it as markdown text.
// Uses the stored HF token if present (required for gated models).

// ── GET /api/hf/meta?repo=owner/model ────────────────────────────────────────
// Returns tags and gated status for a HF repo.  Used by the tag-suggestion UI
// in the wizard hardware step and the models library.

// ── POST /api/hf/resolve-origin ───────────────────────────────────────────────
// Resolves the HF origin of a local GGUF file from its filename.
// Searches HF, scores candidates, returns ranked list with family + card URL.

// ── GET /api/hf/token ─────────────────────────────────────────────────────────
// Returns whether an HF token is saved; never returns the token itself.

// ── PUT /api/hf/token ─────────────────────────────────────────────────────────
// Saves an HF token to ~/.config/llama-monitor/hf-token.

// ── DELETE /api/hf/token ──────────────────────────────────────────────────────
// Removes the saved HF token file.

// ── P3.1: HF Download (with concurrency + cooldown) ──────────────────────────
// - Max 5 concurrent downloads.
// - 10-second cooldown between download starts.

// ── P3.2: Third-Party Models ─────────────────────────────────────────────────

// ── P3.3: Model Introspection ────────────────────────────────────────────────

// ── POST /api/models/gguf-meta ─────────────────────────────────────────────
// Reads GGUF header metadata for a local file and returns the architecture.
// Lightweight — only reads the KV header, never touches tensor data.

// ==================== RESTORE HINT ENDPOINT (T-060) ====================

pub fn api_restore_hint(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("sessions"))
        .and(warp::path("restore-hint"))
        .and(warp::get())
        .and(warp::any().map(move || state.clone()))
        .map(|state: AppState| {
            let server_running = *state.server_running.lock().unwrap();
            let active_id = state.active_session_id.lock().unwrap().clone();

            let sessions = state.sessions.lock().unwrap();
            let has_active_session = sessions
                .iter()
                .any(|s| s.id == active_id && !s.id.is_empty());
            let active_session_id = if has_active_session {
                Some(active_id.clone())
            } else {
                None
            };
            let active_session_status =
                sessions
                    .iter()
                    .find(|s| s.id == active_id)
                    .map(|s| match &s.status {
                        crate::state::SessionStatus::Running => "Running".to_string(),
                        crate::state::SessionStatus::Stopped => "Stopped".to_string(),
                        crate::state::SessionStatus::Disconnected => "Disconnected".to_string(),
                        crate::state::SessionStatus::Error(_) => "Error".to_string(),
                    });

            let has_chat_tabs = sessions
                .iter()
                .any(|s| s.connect_count > 0 || s.last_connected_at > 0);
            drop(sessions);

            let suggested_action = if server_running
                && has_active_session
                && active_session_status == Some("Running".to_string())
            {
                "resume_active"
            } else if server_running && has_active_session {
                "suggest_recent_attach"
            } else {
                "none"
            };

            warp::reply::json(&serde_json::json!({
                "server_running": server_running,
                "has_active_session": has_active_session,
                "active_session_id": active_session_id,
                "active_session_status": active_session_status,
                "has_chat_tabs": has_chat_tabs,
                "suggested_action": suggested_action
            }))
        })
}

pub fn api_routes(
    state: AppState,
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
    _bind_host: String,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let ctx = ApiCtx {
        state: state.clone(),
        config: app_config.clone(),
        auth: auth_manager.clone(),
    };

    let kill_llama = api_kill_llama(state.clone(), app_config.clone());
    let preset_routes = preset_routes::routes(ctx.clone());
    let template_routes = templates::routes(ctx.clone());
    let browse = api_browse(state.clone(), app_config.clone());
    let chat_storage = state.chat_storage.clone();
    let chat_routes = chat::routes(ctx.clone(), chat_storage.clone());
    let db_routes = db::routes(ctx.clone(), chat_storage.clone());

    let get_sessions = api_get_sessions(state.clone(), app_config.clone());
    let get_recent_sessions = api_get_recent_sessions(state.clone(), app_config.clone());
    let check_endpoint_health = api_check_endpoint_health(app_config.clone());
    let create_session = api_create_session(state.clone(), app_config.clone());
    let delete_session = api_delete_session(state.clone(), app_config.clone());
    let get_active_session = api_get_active_session(state.clone(), app_config.clone());
    let get_active_session_readiness =
        api_get_active_session_readiness(state.clone(), app_config.clone());
    let set_active_session = api_set_active_session(state.clone(), app_config.clone());
    let get_capabilities = api_get_capabilities(state.clone(), app_config.clone());
    let spawn_session_with_preset =
        api_spawn_session_with_preset(state.clone(), app_config.clone());
    let attach = api_attach(state.clone(), app_config.clone());
    let detach = api_detach(state.clone(), app_config.clone());
    let lhm_routes = lhm::routes(ctx.clone());
    let remote_agent_routes = remote_agent::routes(ctx.clone());
    let sensor_bridge_routes = sensor_bridge::routes(ctx.clone());

    // GPU / system metrics routes (used by spawn wizard VRAM estimation)
    let metrics_routes = metrics::routes(ctx.clone());

    // T-060: Restore hint endpoint (for browser reopen logic)
    let restore_hint_route = api_restore_hint(state.clone());

    // Group routes to avoid compiler overflow on long .or() chains
    let server_routes = kill_llama
        .or(attach)
        .or(detach)
        .or(sleep::routes(ctx.clone()))
        .or(restore_hint_route);

    let browse_with_chat = browse.or(chat_routes);
    let session_routes = get_sessions
        .or(get_recent_sessions)
        .or(create_session)
        .or(delete_session)
        .or(get_active_session)
        .or(get_active_session_readiness)
        .or(set_active_session)
        .or(get_capabilities)
        .or(spawn_session_with_preset)
        .or(check_endpoint_health);
    let bridge_routes = sensor_bridge_routes;

    let tls_routes = tls::routes(ctx.clone());

    let llama_binary_routes = llama_binary::routes(ctx.clone());

    // Phase 3 modules
    let models_routes = models::routes(ctx.clone());
    let config_routes = config::routes(ctx.clone());
    let spawn_wizard_routes = spawn_wizard::routes(ctx.clone());
    let vram_routes = vram::routes(ctx.clone());
    let benchmark_routes = benchmark::routes(ctx.clone());
    let hf_routes = hf::routes(ctx.clone());

    server_routes
        .or(metrics_routes)
        .or(preset_routes)
        .or(template_routes)
        .or(models_routes)
        .or(config_routes)
        .or(browse_with_chat)
        .or(db_routes)
        .or(session_routes)
        .or(lhm_routes)
        .or(remote_agent_routes)
        .or(bridge_routes)
        .or(tls_routes)
        .or(llama_binary_routes)
        .or(spawn_wizard_routes)
        .or(vram_routes)
        .or(benchmark_routes)
        .or(hf_routes)
        .or(debug::routes(ctx.clone()))
        .or(self_update::routes(ctx.clone()))
}

pub fn auth_api_routes(
    auth_manager: AuthManager,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    auth::routes(auth_manager)
}

fn api_browse(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "browse")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and_then(
            move |auth: Option<String>, query: std::collections::HashMap<String, String>| {
                let cfg = app_config.clone();
                let state = state.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }

                    // Build allowed roots:
                    // - Home directory (primary root).
                    // - Directories used for models, TLS certs, etc.
                    let mut allowed_roots: Vec<PathBuf> = Vec::new();

                    // Always allow home directory
                    if let Some(home) = dirs::home_dir()
                        && let Ok(canon) = home.canonicalize()
                    {
                        allowed_roots.push(canon);
                    }

                    // Allow models_dir (parent directory)
                    if let Some(ref models_dir) = state.models_dir
                        && let Some(parent) = models_dir.parent()
                        && let Ok(canon) = parent.canonicalize()
                    {
                        allowed_roots.push(canon);
                    }

                    // Allow extra configured model directories and their parents
                    if let Ok(settings) = state.ui_settings.lock() {
                        for dir_str in &settings.extra_models_dirs {
                            let dir = std::path::Path::new(dir_str);
                            if let Ok(canon) = dir.canonicalize() {
                                allowed_roots.push(canon);
                            }
                            if let Some(parent) = dir.parent()
                                && let Ok(canon) = parent.canonicalize()
                            {
                                allowed_roots.push(canon);
                            }
                        }
                    }

                    // Allow TLS custom cert/key parent directories
                    if let Ok(tls) = state.tls_config.lock() {
                        if let Some(ref cert_path) = tls.custom_cert_path
                            && let Some(parent) = cert_path.parent()
                            && let Ok(canon) = parent.canonicalize()
                        {
                            allowed_roots.push(canon);
                        }
                        if let Some(ref key_path) = tls.custom_key_path
                            && let Some(parent) = key_path.parent()
                            && let Ok(canon) = parent.canonicalize()
                        {
                            allowed_roots.push(canon);
                        }
                    }

                    // Remove duplicates
                    allowed_roots.sort();
                    allowed_roots.dedup();

                    let requested = query.get("path").cloned().unwrap_or_default();
                    let filter = query.get("filter").cloned().unwrap_or_default();

                    let dir = if requested.is_empty() {
                        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
                    } else {
                        PathBuf::from(&requested)
                    };

                    let dir = match dir.canonicalize() {
                        Ok(p) => p,
                        Err(_) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({
                                        "path": requested,
                                        "error": "Path not found"
                                    })),
                                    warp::http::StatusCode::NOT_FOUND,
                                ),
                            ));
                        }
                    };

                    // Enforce allowlist: directory must be under one of the allowed roots
                    if !allowed_roots.iter().any(|root| dir.starts_with(root)) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "path": dir.display().to_string(),
                                    "error": "Path not allowed"
                                })),
                                warp::http::StatusCode::FORBIDDEN,
                            ),
                        ));
                    }

                    if !dir.is_dir() {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "path": dir.display().to_string(),
                                    "error": "Not a directory"
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            ),
                        ));
                    }

                    let parent = dir
                        .parent()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();

                    let mut entries: Vec<serde_json::Value> = Vec::new();
                    if let Ok(read_dir) = std::fs::read_dir(&dir) {
                        for entry in read_dir.flatten() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            let meta = entry.metadata().ok();
                            let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
                            // Hide hidden files (e.g. .DS_Store) but show hidden directories
                            // so users can navigate into paths like ~/.config
                            if name.starts_with('.') && !is_dir {
                                continue;
                            }

                            if !is_dir && !filter.is_empty() {
                                let pass = match filter.as_str() {
                                    "gguf" => name.ends_with(".gguf"),
                                    "executable" => {
                                        #[cfg(unix)]
                                        {
                                            use std::os::unix::fs::PermissionsExt;
                                            meta.as_ref().is_some_and(|m| {
                                                m.permissions().mode() & 0o111 != 0
                                            })
                                        }
                                        #[cfg(not(unix))]
                                        {
                                            true
                                        }
                                    }
                                    _ => true,
                                };
                                if !pass {
                                    continue;
                                }
                            }

                            let size = if is_dir {
                                0
                            } else {
                                meta.as_ref().map(|m| m.len()).unwrap_or(0)
                            };
                            let size_display = if is_dir {
                                String::new()
                            } else if size >= 1_000_000_000 {
                                format!("{:.1} GB", size as f64 / 1_000_000_000.0)
                            } else if size >= 1_000_000 {
                                format!("{:.0} MB", size as f64 / 1_000_000.0)
                            } else {
                                format!("{:.0} KB", size as f64 / 1_000.0)
                            };

                            entries.push(serde_json::json!({
                                "name": name,
                                "is_dir": is_dir,
                                "size": size,
                                "size_display": size_display,
                                "path": entry.path().display().to_string(),
                            }));
                        }
                    }

                    entries.sort_by(|a, b| {
                        let a_dir = a["is_dir"].as_bool().unwrap_or(false);
                        let b_dir = b["is_dir"].as_bool().unwrap_or(false);
                        b_dir.cmp(&a_dir).then_with(|| {
                            a["name"]
                                .as_str()
                                .unwrap_or("")
                                .to_lowercase()
                                .cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
                        })
                    });

                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "path": dir.display().to_string(),
                                "parent": parent,
                                "entries": entries,
                            })),
                            warp::http::StatusCode::OK,
                        ),
                    ))
                }
            },
        )
}
// Helper for DB admin endpoints that need chat_storage
fn api_get_sessions(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();
    warp::path!("api" / "sessions")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let sessions = state.get_sessions();
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &sessions,
                )))
            }
        })
}

fn api_get_recent_sessions(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();
    warp::path!("api" / "sessions" / "recent")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let mut sessions = state.get_sessions();
                // Sort by last_connected_at descending
                sessions.sort_by_key(|s| std::cmp::Reverse(s.last_connected_at));
                // Limit to 10
                sessions.truncate(10);
                let active_id = state.active_session_id.lock().unwrap().clone();
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "sessions": sessions,
                        "active_session_id": active_id
                    }),
                )))
            }
        })
}

fn api_check_endpoint_health(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sessions" / "check-endpoint")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>,
                  params: std::collections::HashMap<String, String>,
                  cfg: Arc<AppConfig>| async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let url = match params.get("url") {
                    Some(u) if !u.is_empty() => u.clone(),
                    _ => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"error": "missing url"})),
                                warp::http::StatusCode::BAD_REQUEST,
                            ),
                        ));
                    }
                };
                // Proxy health check: try /health endpoint server-side
                let health_url = format!("{}/health", url.trim_end_matches('/'));
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(4))
                    .build()
                    .unwrap_or_default();
                let reachable = client
                    .get(&health_url)
                    .send()
                    .await
                    .map(|r| r.status().as_u16() < 500)
                    .unwrap_or(false);
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "reachable": reachable }),
                )))
            },
        )
}

fn api_create_session(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();
    warp::path!("api" / "sessions")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, session: app_state::Session, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                if state.add_session(session) {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
                    )
                } else {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Maximum sessions reached"}),
                        )),
                    )
                }
            }
        })
}

fn api_delete_session(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_DELETE_SESSION: AtomicU64 = AtomicU64::new(0);

    let app_config = app_config.clone();
    warp::path!("api" / "sessions" / String)
        .and(warp::path::end())
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(
            move |session_id: String, auth: Option<String>, cfg: Arc<AppConfig>| {
                let state = state.clone();
                async move {
                    if !check_db_admin_token(&auth, &cfg) {
                        return Ok(unauthorized_db_admin_token());
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_DELETE_SESSION.load(Ordering::Acquire);
                    if now - last < 5 {
                        let remaining = 5 - (now - last);
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "too soon; please wait",
                                    "seconds_remaining": remaining
                                })),
                                warp::http::StatusCode::TOO_MANY_REQUESTS,
                            ),
                        ));
                    }
                    LAST_DELETE_SESSION.store(now, Ordering::Release);

                    if state.remove_session(&session_id) {
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"ok": true})),
                        ))
                    } else {
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Session not found"}),
                            ),
                        ))
                    }
                }
            },
        )
}

fn api_get_active_session(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();
    warp::path!("api" / "sessions" / "active")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let session_id = state.active_session_id.lock().unwrap().clone();
                let sessions = state.sessions.lock().unwrap();
                let session = sessions.iter().find(|s| s.id == session_id).cloned();
                drop(sessions);

                match session {
                    Some(s) => {
                        let mode_str = match s.mode {
                            crate::state::SessionMode::Spawn { port, .. } => {
                                format!("Spawn:{}", port)
                            }
                            crate::state::SessionMode::Attach { endpoint, .. } => {
                                format!("Attach:{}", endpoint)
                            }
                        };
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "id": s.id,
                                "name": s.name,
                                "mode": mode_str,
                                "status": s.status,
                                "last_active": s.last_active,
                                "preset_id": s.preset_id
                            })),
                        ))
                    }
                    None => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({"error": "No active session"})),
                    )),
                }
            }
        })
}

fn api_get_active_session_readiness(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sessions" / "active" / "readiness")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        unauthorized_api_token(),
                    ));
                }

                let Some(session) = state.get_active_session() else {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "ready": false, "error": "No active session"}),
                        ),
                    ));
                };

                let (endpoint, api_key) = match session.mode {
                    crate::state::SessionMode::Spawn { port, api_key, .. } => {
                        (format!("http://127.0.0.1:{port}"), api_key)
                    }
                    crate::state::SessionMode::Attach { endpoint, api_key } => (endpoint, api_key),
                };

                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(2))
                    .build()
                    .map_err(|e| warp::reject::custom(ApiError::internal(e.to_string())))?;

                let with_auth = |mut req: reqwest::RequestBuilder| {
                    if let Some(key) = &api_key {
                        req = req.header("Authorization", format!("Bearer {}", key));
                    }
                    req
                };

                let root_ok = with_auth(client.get(&endpoint)).send().await.is_ok();
                let health_ok = with_auth(client.get(format!("{endpoint}/health")))
                    .send()
                    .await
                    .is_ok();
                let ready = root_ok || health_ok;

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "ready": ready,
                        "endpoint": endpoint,
                        "status": session.status,
                    }),
                )))
            }
        })
}

fn api_get_capabilities(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "capabilities")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        unauthorized_api_token(),
                    );
                }
                let capabilities = state.calculate_capabilities();
                let endpoint_kind = state.current_endpoint_kind();
                let session_kind = state.current_session_kind();
                let tray_mode = state.tray_mode.lock().unwrap().clone();

                let (system_reason, gpu_reason, cpu_temp_reason) =
                    state.calculate_availability_reasons();

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "capabilities": capabilities,
                        "endpoint_kind": endpoint_kind,
                        "session_kind": session_kind,
                        "tray_mode": tray_mode,
                        "availability": {
                            "system": system_reason,
                            "gpu": gpu_reason,
                            "cpu_temp": cpu_temp_reason
                        }
                    }),
                )))
            }
        })
}

fn api_set_active_session(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();
    warp::path!("api" / "sessions" / "active")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, payload: serde_json::Value, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let session_id = match payload.get("id") {
                    Some(v) => v.as_str().unwrap_or("").to_string(),
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Missing session id"}),
                        )));
                    }
                };
                if state.set_active_session(&session_id) {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
                    )
                } else {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Session not found"}),
                        )),
                    )
                }
            }
        })
}

fn api_spawn_session_with_preset(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_SPAWN_SESSION: AtomicU64 = AtomicU64::new(0);

    let app_config_inner = app_config.clone();
    warp::path!("api" / "sessions" / "spawn")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, payload: serde_json::Value, cfg: Arc<AppConfig>| {
            let state = state.clone();
            let app_config = app_config_inner.clone();
            async move {
                if !check_db_admin_token(&auth, &cfg) {
                    return Ok(unauthorized_db_admin_token());
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let last = LAST_SPAWN_SESSION.load(Ordering::Acquire);
                if now - last < 15 {
                    let remaining = 15 - (now - last);
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ),
                    ));
                }
                LAST_SPAWN_SESSION.store(now, Ordering::Release);

                let port: u16 = match payload.get("port") {
                    Some(v) => {
                        if let Some(p) = v.as_u64() {
                            p as u16
                        } else {
                            8001
                        }
                    }
                    None => 8001,
                };
                let name: String = match payload.get("name") {
                    Some(v) => {
                        if let Some(s) = v.as_str() {
                            s.to_string()
                        } else {
                            format!("Session on port {}", port)
                        }
                    }
                    None => format!("Session on port {}", port),
                };

                let Some(preset_id) = payload
                    .get("preset_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                else {
                    let config: ServerConfig = match serde_json::from_value(payload.clone()) {
                        Ok(config) => config,
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(
                                    &serde_json::json!({"ok": false, "error": format!("Invalid spawn payload: {}", e)}),
                                ),
                            ));
                        }
                    };

                    let session_name = if name != format!("Session on port {}", port) {
                        name.clone()
                    } else if !config.model_path.is_empty() {
                        let filename = std::path::Path::new(&config.model_path)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&config.model_path);
                        format!("Local: {}", filename)
                    } else if let Some(repo) = config.hf_repo.as_ref() {
                        format!("HF: {}", repo)
                    } else {
                        name.clone()
                    };

                    let session_id = app_state::generate_session_id();
                    let session = app_state::Session::new_spawn(
                        session_id.clone(),
                        session_name,
                        config.port,
                        String::new(),
                        config.bind_host.clone(),
                        config.api_key.clone(),
                    );

                    if !state.add_session(session) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Failed to create session"}),
                            ),
                        ));
                    }

                    state.set_active_session(&session_id);

                    // TODO: apply ui_settings overrides for llama_server_path / llama_server_cwd
                    // (V1 endpoint does this; parity needed if someone set a custom server path/CWD)
                    match crate::llama::server::start_server(&state, config, &app_config).await {
                        Ok(()) => {
                            state.update_session_status(&session_id, SessionStatus::Running);
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(
                                    &serde_json::json!({"ok": true, "session_id": session_id}),
                                ),
                            ));
                        }
                        Err(e) => {
                            state.remove_session(&session_id);
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(
                                    &serde_json::json!({"ok": false, "error": e.to_string()}),
                                ),
                            ));
                        }
                    }
                };

                let preset = {
                    let presets = state.presets.lock().unwrap();
                    match presets.iter().find(|p| p.id == preset_id).cloned() {
                        Some(p) => p,
                        None => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Preset not found"}),
                            )));
                        }
                    }
                };

                let session_id = app_state::generate_session_id();
                let session = app_state::Session::new_spawn(
                    session_id.clone(),
                    name.clone(),
                    port,
                    preset_id,
                    preset.bind_host.clone(),
                    preset.api_key.clone(),
                );

                if !state.add_session(session) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "Failed to create session"}),
                    )));
                }

                state.set_active_session(&session_id);

                let config = crate::llama::server::ServerConfig {
                    model_path: preset.model_path.clone(),
                    hf_repo: preset.hf_repo.clone(),
                    context_size: preset.context_size,
                    ctk: preset.ctk.clone(),
                    ctv: preset.ctv.clone(),
                    tensor_split: preset.tensor_split.clone(),
                    batch_size: preset.batch_size,
                    ubatch_size: preset.ubatch_size,
                    no_mmap: preset.no_mmap,
                    port,
                    ngram_spec: preset.ngram_spec,
                    parallel_slots: preset.parallel_slots,
                    temperature: preset.temperature,
                    top_p: preset.top_p,
                    top_k: preset.top_k,
                    min_p: preset.min_p,
                    repeat_penalty: preset.repeat_penalty,
                    presence_penalty: preset.presence_penalty,
                    n_cpu_moe: preset.n_cpu_moe,
                    gpu_layers: preset.gpu_layers,
                    mlock: preset.mlock,
                    flash_attn: preset.flash_attn.clone(),
                    split_mode: preset.split_mode.clone(),
                    main_gpu: preset.main_gpu,
                    threads: preset.threads,
                    threads_batch: preset.threads_batch,
                    rope_scaling: preset.rope_scaling.clone(),
                    rope_freq_base: preset.rope_freq_base,
                    rope_freq_scale: preset.rope_freq_scale,
                    spec: crate::llama::server::SpecDecodeConfig {
                        draft_model: preset.draft_model.clone(),
                        draft_min: preset.draft_min,
                        draft_max: preset.draft_max,
                        spec_ngram_size: preset.spec_ngram_size,
                        spec_type: preset.spec_type.clone(),
                        spec_default: preset.spec_default,
                        spec_draft_n_max: preset.spec_draft_n_max,
                        spec_draft_n_min: preset.spec_draft_n_min,
                        spec_draft_p_split: preset.spec_draft_p_split,
                        spec_draft_p_min: preset.spec_draft_p_min,
                        spec_draft_ngl: preset.spec_draft_ngl,
                        spec_draft_device: preset.spec_draft_device.clone(),
                        spec_draft_cpu_moe: preset.spec_draft_cpu_moe,
                        spec_draft_n_cpu_moe: preset.spec_draft_n_cpu_moe,
                        spec_draft_type_k: preset.spec_draft_type_k.clone(),
                        spec_draft_type_v: preset.spec_draft_type_v.clone(),
                        spec_ngram_mod_n_min: preset.spec_ngram_mod_n_min,
                        spec_ngram_mod_n_max: preset.spec_ngram_mod_n_max,
                        spec_ngram_mod_n_match: preset.spec_ngram_mod_n_match,
                        spec_ngram_simple_size_n: preset.spec_ngram_simple_size_n,
                        spec_ngram_simple_size_m: preset.spec_ngram_simple_size_m,
                        spec_ngram_simple_min_hits: preset.spec_ngram_simple_min_hits,
                        spec_ngram_map_k_size_n: preset.spec_ngram_map_k_size_n,
                        spec_ngram_map_k_size_m: preset.spec_ngram_map_k_size_m,
                        spec_ngram_map_k_min_hits: preset.spec_ngram_map_k_min_hits,
                        spec_ngram_map_k4v_size_n: preset.spec_ngram_map_k4v_size_n,
                        spec_ngram_map_k4v_size_m: preset.spec_ngram_map_k4v_size_m,
                        spec_ngram_map_k4v_min_hits: preset.spec_ngram_map_k4v_min_hits,
                    },
                    kv_unified: preset.kv_unified,
                    cache_idle_slots: preset.cache_idle_slots,
                    cache_ram_mib: preset.cache_ram_mib,
                    fit_enabled: preset.fit_enabled,
                    fit_ctx: preset.fit_ctx,
                    fit_target: preset.fit_target.clone(),
                    fit_print: preset.fit_print,
                    seed: preset.seed,
                    system_prompt_file: preset.system_prompt_file.clone(),
                    extra_args: preset.extra_args.clone(),
                    bind_host: preset.bind_host.clone(),
                    chat_template_file: preset.chat_template_file.clone(),
                    mmproj: preset.mmproj.clone(),
                    grammar: preset.grammar.clone(),
                    json_schema: preset.json_schema.clone(),
                    cache_type_k: preset.cache_type_k.clone(),
                    cache_type_v: preset.cache_type_v.clone(),
                    max_tokens: preset.max_tokens,
                    api_key: preset.api_key.clone(),
                    alias: preset.alias.clone(),
                    benchmark_mode: preset.benchmark_mode,
                    enable_thinking: preset.enable_thinking,
                    preserve_thinking: preset.preserve_thinking,
                    reasoning: preset.reasoning.clone(),
                    reasoning_budget: preset.reasoning_budget,
                    reasoning_budget_message: preset.reasoning_budget_message.clone(),
                    image_min_tokens: preset.image_min_tokens,
                    image_max_tokens: preset.image_max_tokens,
                    ..Default::default()
                };

                // TODO: apply ui_settings overrides for llama_server_path / llama_server_cwd
                // (V1 endpoint does this; parity needed if someone set a custom server path/CWD)
                match crate::llama::server::start_server(&state, config, &app_config).await {
                    Ok(()) => {
                        state.update_session_status(&session_id, SessionStatus::Running);
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": true, "session_id": session_id}),
                        )))
                    }
                    Err(e) => {
                        state.remove_session(&session_id);
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        )))
                    }
                }
            }
        })
}

fn api_attach(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_ATTACH: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "attach")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::headers_cloned())
        .and(warp::body::json())
        .and(with_app_config(app_config))
        .and_then(move |headers: warp::http::HeaderMap,
                      payload: serde_json::Map<String, serde_json::Value>,
                      cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                // Require api-token for attach (constant-time comparison via helper)
                let bearer_str = headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.strip_prefix("Bearer "));
                let has_token = bearer_matches_api_token(bearer_str, &cfg);

                if !has_token {
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": "unauthorized" })),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ));
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let last = LAST_ATTACH.load(Ordering::Acquire);
                if now - last < 10 {
                    let remaining = 10 - (now - last);
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "too soon; please wait",
                            "seconds_remaining": remaining
                        })),
                        warp::http::StatusCode::TOO_MANY_REQUESTS,
                    ));
                }
                LAST_ATTACH.store(now, Ordering::Release);

                let endpoint: String = match payload.get("endpoint") {
                    Some(v) => {
                        if let Some(s) = v.as_str() {
                            // Validate: must be http/https scheme with private/loopback host
                            let parsed = url::Url::parse(s).map_err(|_| warp::reject::not_found())?;
                            if !["http", "https"].contains(&parsed.scheme()) {
                                return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({"ok": false, "error": "Endpoint must use http:// or https://"})),
                                    warp::http::StatusCode::OK,
                                ));
                            }
                            if let Some(host) = parsed.host_str()
                                && let Ok(ip) = host.parse::<std::net::IpAddr>() {
                                    // is_private() is unstable; inline the check
                                    let private = matches!(ip, std::net::IpAddr::V4(v4)
                                        if v4.octets()[0] == 10
                                            || (v4.octets()[0] == 172 && (4..=11).contains(&v4.octets()[1]))
                                            || (v4.octets()[0] == 192 && v4.octets()[1] == 168));
                                    if !ip.is_loopback() && !private {
                                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                            warp::reply::json(&serde_json::json!({"ok": false, "error": "Endpoint must be on a private network"})),
                                            warp::http::StatusCode::OK,
                                        ));
                                    }
                                }
                            s.to_string()
                        } else {
                            return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"ok": false, "error": "Invalid endpoint"})),
                                warp::http::StatusCode::OK,
                            ));
                        }
                    }
                    None => {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({"ok": false, "error": "Missing endpoint"})),
                            warp::http::StatusCode::OK,
                        ));
                    }
                };

                // Extract optional API key for remote server authentication
                let caller_api_key: Option<String> = payload.get("api_key")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());

               // If caller did not send an api_key, check if a Spawn session for
                // this localhost:port already has one — allow attach to use it.
                let (api_key, _spawn_match_id) = {
                    let mut effective_key = caller_api_key.clone();
                    let mut match_id = None;

                    if effective_key.is_none()
                        && let Ok(parsed) = url::Url::parse(&endpoint)
                        && let Some(host) = parsed.host_str()
                        && let Some(port_num) = parsed.port()
                        && (host == "127.0.0.1" || host == "localhost") {
                            let sessions = state.sessions.lock().unwrap();
                            if let Some(s) = sessions.iter().find(|sess| {
                                if let crate::state::SessionMode::Spawn {
                                    port,
                                    api_key: sess_key,
                                    ..
                                } = &sess.mode {
                                    *port == port_num && sess_key.is_some()
                                } else {
                                    false
                                }
                            }) {
                                if let crate::state::SessionMode::Spawn {
                                    api_key: sess_key,
                                    ..
                                } = &s.mode {
                                    effective_key = sess_key.clone();
                                }
                                match_id = Some(s.id.clone());
                            }
                    }

                    (effective_key, match_id)
                };

                // Pre-attach health check
                let client = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(15))
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed to create HTTP client: {}", e)
                            })),
                            warp::http::StatusCode::OK,
                        ));
                    }
                };

                // Check if server is reachable (with API key if provided)
                eprintln!("[info] Health-checking llama-server at {}", endpoint);
                let mut health_req = client.get(&endpoint);
                if let Some(ref key) = api_key {
                    health_req = health_req.header("Authorization", format!("Bearer {}", key));
                }
                let server_up = match health_req.send().await {
                    Ok(resp) => {
                        eprintln!("[info] llama-server health check status: {}", resp.status());
                        true
                    }
                    Err(e) => {
                        eprintln!("[warn] llama-server health check failed: {}", e);
                        false
                    }
                };
                if !server_up {
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Cannot reach llama-server at {}. Is it running?", endpoint)
                        })),
                        warp::http::StatusCode::OK,
                    ));
                }

                // Check if metrics endpoint is available (with API key if provided)
                let mut metrics_req = client.get(format!("{}/health", endpoint.trim_end_matches('/')));
                if let Some(ref key) = api_key {
                    metrics_req = metrics_req.header("Authorization", format!("Bearer {}", key));
                }
                let metrics_available = metrics_req.send().await.is_ok();

               // Check if there's already an attach session for this endpoint
                let existing_session_id = {
                    let sessions = state.sessions.lock().unwrap();
                    // Prefer existing Attach session for this endpoint
                    let mut id = sessions.iter().find(|s| {
                        if let crate::state::SessionMode::Attach { endpoint: ep, .. } = &s.mode {
                            *ep == endpoint
                        } else {
                            false
                        }
                    }).map(|s| s.id.clone());

                    // If no Attach, see if this endpoint matches a Spawn session's
                    // localhost:port — reuse that Spawn session instead of
                    // creating a new Attach session.
                    if id.is_none()
                        && let Ok(parsed) = url::Url::parse(&endpoint)
                        && let Some(host) = parsed.host_str()
                        && let Some(port_num) = parsed.port()
                        && (host == "127.0.0.1" || host == "localhost") {
                            id = sessions.iter().find(|s| {
                                if let crate::state::SessionMode::Spawn {
                                    port,
                                    ..
                                } = &s.mode {
                                    *port == port_num
                                } else {
                                    false
                                }
                            }).map(|s| s.id.clone());
                    }

                    id
                };

                let session_id = if let Some(id) = existing_session_id {
                    // Reuse existing session (Attach or matched Spawn)
                    eprintln!("[info] Reusing existing session for {}", endpoint);
                    id
                } else {
                    // Create new session
                    let session_id = crate::state::generate_session_id();
                    let session = crate::state::Session::new_attach(
                        session_id.clone(),
                        format!("Attached: {}", endpoint),
                        endpoint,
                        api_key.clone(),
                    );
                    if !state.add_session(session) {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({"ok": false, "error": "Maximum sessions reached"})),
                            warp::http::StatusCode::OK,
                        ));
                    }
                    session_id
                };

                // Update session metadata for connection tracking
                {
                    let mut sessions = state.sessions.lock().unwrap();
                    if let Some(s) = sessions.iter_mut().find(|s| s.id == session_id) {
                        s.last_connected_at = now;
                        s.connect_count += 1;
                        s.last_error = None;
                    }
                }

                state.set_active_session(&session_id);
                state.llama_poll_notify.notify_waiters();
                Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "warning": if !metrics_available {
                            Some("llama-server is running but metrics endpoint (/health) is unavailable. Inference metrics will not be available. Start llama-server with --metrics flag to enable metrics.")
                        } else {
                            None
                        }
                    })),
                    warp::http::StatusCode::OK,
                ))
            }
        })
}

fn api_detach(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();
    warp::path!("api" / "detach")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let active_id = state.active_session_id.lock().unwrap().clone();
                if active_id.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "No active session to detach from"}),
                    )));
                }

                // Check if the active session is an attach session
                let sessions = state.sessions.lock().unwrap();
                let session = sessions.iter().find(|s| s.id == active_id);

                let is_attach = session.map(|s| matches!(s.mode, crate::state::SessionMode::Attach { .. }));

                if !is_attach.unwrap_or(false) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "Active session is not an attach session"}),
                    )));
                }

                // Update last_active on detach
                {
                    let mut sessions = state.sessions.lock().unwrap();
                    if let Some(s) = sessions.iter_mut().find(|s| s.id == active_id) {
                        s.last_active = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                    }
                }

                drop(sessions);
                // Clear the active session only - server_running is managed by the poller
                state.set_active_session("");
                // Notify poller so it stops polling immediately
                state.llama_poll_notify.notify_waiters();

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(&serde_json::json!({"ok": true}))))
            }
        })
}

fn api_kill_llama(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();

    warp::path!("api" / "kill-llama")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<serde_json::Value>())
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, body: serde_json::Value, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                // Require db-admin-token (elevated operation).
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_admin_token =
                    bearer_matches_db_admin_token(bearer.as_deref(), &cfg);

                if !has_admin_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized; db-admin-token required" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                // Require explicit confirmation.
                let confirm = body.get("confirm")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if confirm != "kill" {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "missing confirmation; send { \"confirm\": \"kill\" }" })),
                            warp::http::StatusCode::BAD_REQUEST,
                        ),
                    ));
                }

                // Kill the tracked child process and clear in-memory state
                // (server_child, local_server_running, server_config, llama_metrics).
                // Platform-specific pkill/taskkill below is a fallback if the child
                // reference was already lost.
                state.push_log("[monitor] kill-llama: kill-llama requested (best-effort)".into());
                if let Err(e) = crate::llama::server::stop_server(&state).await {
                    state.push_log(format!("[monitor] stop_server fallback: {}", e));
                }

                // Inline kill logic (platform-specific, fallback).
                #[cfg(target_os = "windows")]
                {
                    use std::process::Command;
                    match Command::new("taskkill")
                        .args(["/IM", "llama-server.exe", "/F"])
                        .output()
                    {
                        Ok(output) => {
                            if output.status.success() {
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({ "ok": true })),
                                ))
                            } else {
                                let err = String::from_utf8_lossy(&output.stderr);
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({ "ok": false, "error": err })),
                                ))
                            }
                        }
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "ok": false, "error": e.to_string() })),
                        )),
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    use std::process::Command;
                    match Command::new("pkill").args(["-f", "llama-server"]).output() {
                        Ok(output) => {
                            if output.status.success() {
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({ "ok": true })),
                                ))
                            } else {
                                let err = String::from_utf8_lossy(&output.stderr);
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({ "ok": false, "error": err })),
                                ))
                            }
                        }
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "ok": false, "error": e.to_string() })),
                        )),
                    }
                }
                #[cfg(target_os = "macos")]
                {
                    use std::process::Command;
                    match Command::new("pkill").args(["-f", "llama-server"]).output() {
                        Ok(output) => {
                            if output.status.success() {
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({ "ok": true })),
                                ))
                            } else {
                                let err = String::from_utf8_lossy(&output.stderr);
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({ "ok": false, "error": err })),
                                ))
                            }
                        }
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "ok": false, "error": e.to_string() })),
                        )),
                    }
                }
                #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": "Unsupported platform" })),
                    ))
                }
            }
        })
}
#[cfg(test)]
mod tests {
    use super::hf::resolve_hf_target_dir;
    use super::legacy_chat_types::*;
    use super::spawn_wizard::is_private_host;
    use super::token_bootstrap_allowed;

    use crate::chat_storage::ChatStorage;
    use crate::config::{self, AcmeConfig, TLSConfig, TlsMode};
    use crate::gpu::env::GpuEnv;
    use crate::state::{AppPaths, AppState};
    use crate::web::auth::AuthManager;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use warp::Filter;

    fn make_test_app_state(tls_config: TLSConfig) -> (AppState, Arc<config::AppConfig>) {
        let paths = AppPaths {
            presets_path: PathBuf::new(),
            templates_path: PathBuf::new(),
            models_dir: None,
            gpu_env_path: PathBuf::new(),
            ui_settings_path: PathBuf::new(),
            sessions_path: PathBuf::new(),
            model_tags_path: PathBuf::new(),
        };
        let cs = Arc::new(
            ChatStorage::open(&PathBuf::from(":memory:")).expect("open in-memory chat storage"),
        );
        let state = AppState::new(
            vec![],
            paths,
            GpuEnv::default(),
            crate::state::UiSettings::default(),
            cs,
            tls_config,
        );
        let app_config = Arc::new(config::AppConfig::for_test(
            Some("test-token".to_string()),
            None,
        ));
        (state, app_config)
    }

    fn tls_routes_filter(
        state: AppState,
        app_config: Arc<config::AppConfig>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        let auth = AuthManager::new(None, None, &TlsMode::None);
        super::tls::routes(super::ApiCtx {
            state,
            config: app_config,
            auth,
        })
    }

    fn auth_routes_filter(
        auth_manager: AuthManager,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        super::auth_api_routes(auth_manager)
    }

    #[test]
    fn token_bootstrap_allows_loopback_without_basic_auth() {
        let auth = AuthManager::new(None, None, &TlsMode::None);
        assert!(token_bootstrap_allowed(&auth, "127.0.0.1"));
        assert!(token_bootstrap_allowed(&auth, "localhost"));
    }

    #[test]
    fn resolve_hf_target_dir_rejects_path_traversal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let models_dir = tmp.path().join("models");
        let err = resolve_hf_target_dir(&models_dir, Some("../escape")).expect_err("rejects");
        assert!(err.contains("path traversal"));
    }

    #[test]
    fn resolve_hf_target_dir_creates_and_resolves_child_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let models_dir = tmp.path().join("models");
        let target = resolve_hf_target_dir(&models_dir, Some("nested/model-dir")).expect("path");
        assert!(target.starts_with(models_dir.canonicalize().expect("canonical models_dir")));
        assert!(target.exists());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_hf_target_dir_rechecks_symlink_escape_after_create() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().expect("tempdir");
        let models_dir = tmp.path().join("models");
        let outside_dir = tmp.path().join("outside");

        std::fs::create_dir_all(&outside_dir).expect("outside dir");
        std::fs::create_dir_all(&models_dir).expect("models dir");
        symlink(&outside_dir, models_dir.join("linked")).expect("symlink");

        let err =
            resolve_hf_target_dir(&models_dir, Some("linked/new-download")).expect_err("rejects");
        assert!(err.contains("escapes models_dir"));
    }

    // ===== SSRF guard tests (is_private_host) =====

    #[test]
    fn srf_blocks_localhost_variants() {
        for host in [
            "localhost",
            "LOCALHOST",
            "127.0.0.1",
            "127.0.0.255",
            "127.255.255.255",
            "::1",
            "0:0:0:0:0:0:0:1",
            "0.0.0.0",
        ] {
            assert!(is_private_host(host), "Should block '{}'", host);
        }
    }

    #[test]
    fn srf_blocks_ipv4_private_ranges() {
        // 10.x.x.x
        assert!(is_private_host("10.0.0.1"));
        assert!(is_private_host("10.255.255.255"));
        // 172.16-31.x.x
        assert!(is_private_host("172.16.0.1"));
        assert!(is_private_host("172.31.255.255"));
        // 192.168.x.x
        assert!(is_private_host("192.168.0.1"));
        assert!(is_private_host("192.168.255.255"));
        // Link-local
        assert!(is_private_host("169.254.100.1"));
        // Link-local (0x0a00)
        assert!(is_private_host("169.254.0.1"));
        // APIPA (169.254.0.0/16)
        assert!(is_private_host("169.254.1.1"));
    }

    #[test]
    fn srf_blocks_ipv6_private_ranges() {
        // ULA (fc00::/7)
        assert!(is_private_host("fc00::1"));
        assert!(is_private_host("fd00::dead:beef"));
        assert!(is_private_host("fdff:ffff:ffff:ffff:ffff:ffff:ffff:ffff"));
        // Link-local (fe80::/10)
        assert!(is_private_host("fe80::1"));
        assert!(is_private_host("febf:ffff:ffff:ffff:ffff:ffff:ffff:ffff"));
    }

    #[test]
    fn srf_blocks_internal_tlds() {
        for host in [
            "internal.local",
            "my-host.internal",
            "api.corp",
            "db.lan",
            "service.local.svc.cluster.local",
        ] {
            assert!(is_private_host(host), "Should block '{}'", host);
        }
    }

    #[test]
    fn srf_allows_public_hosts() {
        for host in [
            "huggingface.co",
            "cdn.huggingface.co",
            "google.com",
            "8.8.8.8",
            "1.1.1.1",
            "2001:4860:4860::8888",
        ] {
            assert!(!is_private_host(host), "Should allow '{}'", host);
        }
    }

    #[test]
    fn srf_allows_non_private_ipv4() {
        // 172.32 is the first non-private /12 after 172.16-31
        assert!(!is_private_host("172.32.0.1"));
        // 172.15 is the last non-private /12 before 172.16-31
        assert!(!is_private_host("172.15.255.255"));
        // Note: 203.0.113.0/24 and 198.51.100.0/24 are RFC-5737 documentation
        // ranges and are correctly blocked by is_private_host().
    }

    #[test]
    fn token_bootstrap_allows_all_when_no_auth_configured() {
        let auth = AuthManager::new(None, None, &TlsMode::None);
        // No Auth mode: fully open (local-first)
        assert!(token_bootstrap_allowed(&auth, "0.0.0.0"));
        assert!(token_bootstrap_allowed(&auth, "192.168.2.44"));
    }

    #[test]
    fn token_bootstrap_allows_non_loopback_host_when_no_auth() {
        let auth = AuthManager::new(None, None, &TlsMode::None);
        // No Auth mode: fully open regardless of bind address
        assert!(token_bootstrap_allowed(&auth, "0.0.0.0"));
        assert!(token_bootstrap_allowed(&auth, "192.168.2.44"));
    }

    #[test]
    fn token_bootstrap_rejects_spoofed_host_header_on_non_loopback_bind() {
        // Auth configured + non-loopback bind: Host header must NOT be trusted.
        let auth = AuthManager::new(
            AuthManager::parse_credentials("admin:secret"),
            None,
            &TlsMode::None,
        );
        assert!(!token_bootstrap_allowed(&auth, "0.0.0.0"));
    }

    #[test]
    fn token_bootstrap_allows_loopback_when_basic_auth_is_configured() {
        let auth = AuthManager::new(
            AuthManager::parse_credentials("admin:secret"),
            None,
            &TlsMode::None,
        );
        assert!(token_bootstrap_allowed(&auth, "127.0.0.1"));
        assert!(!token_bootstrap_allowed(&auth, "0.0.0.0"));
    }

    #[test]
    fn token_bootstrap_allows_loopback_when_form_auth_is_configured() {
        let auth = AuthManager::new(
            None,
            AuthManager::parse_credentials("admin:secret"),
            &TlsMode::None,
        );
        assert!(token_bootstrap_allowed(&auth, "127.0.0.1"));
        assert!(!token_bootstrap_allowed(&auth, "0.0.0.0"));
    }

    #[tokio::test]
    async fn form_auth_login_sets_session_cookie_and_status_reflects_it() {
        let auth = AuthManager::new(
            None,
            AuthManager::parse_credentials("admin:secret123"),
            &TlsMode::None,
        );
        let routes = auth_routes_filter(auth);

        let login_resp = warp::test::request()
            .method("POST")
            .path("/api/auth/login")
            .json(&serde_json::json!({
                "username": "admin",
                "password": "secret123",
            }))
            .reply(&routes)
            .await;

        assert_eq!(login_resp.status(), 200);
        let set_cookie = login_resp
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .expect("set-cookie header");
        assert!(set_cookie.contains("llama_monitor_session="));

        let status_resp = warp::test::request()
            .method("GET")
            .path("/api/auth/status")
            .header("cookie", set_cookie)
            .reply(&routes)
            .await;

        assert_eq!(status_resp.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(status_resp.body()).expect("valid JSON");
        assert_eq!(body["enabled"], true);
        assert_eq!(body["methods"]["form"], true);
        assert_eq!(body["authenticated"], true);
        assert_eq!(body["method"], "form");
        assert_eq!(body["username"], "admin");
    }

    #[tokio::test]
    async fn form_auth_logout_clears_session_cookie() {
        let auth = AuthManager::new(
            None,
            AuthManager::parse_credentials("admin:secret123"),
            &TlsMode::None,
        );
        let routes = auth_routes_filter(auth);

        let login_resp = warp::test::request()
            .method("POST")
            .path("/api/auth/login")
            .json(&serde_json::json!({
                "username": "admin",
                "password": "secret123",
            }))
            .reply(&routes)
            .await;

        let set_cookie = login_resp
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .expect("set-cookie header");

        let logout_resp = warp::test::request()
            .method("POST")
            .path("/api/auth/logout")
            .header("cookie", set_cookie)
            .reply(&routes)
            .await;

        assert_eq!(logout_resp.status(), 200);
        let clear_cookie = logout_resp
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .expect("set-cookie clear header");
        assert!(clear_cookie.contains("Max-Age=0"));
    }

    #[tokio::test]
    async fn tls_config_get_requires_api_token() {
        let (state, app_config) = make_test_app_state(TLSConfig::default());
        let routes = tls_routes_filter(state, app_config);

        // Without token -> 401
        let resp = warp::test::request()
            .method("GET")
            .path("/api/tls/config")
            .reply(&routes)
            .await;
        assert_eq!(resp.status(), 401);

        // With correct token -> 200
        let resp = warp::test::request()
            .method("GET")
            .path("/api/tls/config")
            .header("Authorization", "Bearer test-token")
            .reply(&routes)
            .await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).expect("valid JSON");
        assert_eq!(body["mode"], "none");
    }

    #[tokio::test]
    async fn tls_config_get_returns_acme_fields() {
        let mut dns_config = HashMap::new();
        dns_config.insert("CF_API_TOKEN".to_string(), "redacted".to_string());

        let tls_config = TLSConfig {
            mode: TlsMode::Acme,
            custom_cert_path: None,
            custom_key_path: None,
            acme: AcmeConfig {
                enabled: true,
                fqdn: "llama-monitor.example.com".to_string(),
                email: String::new(),
                environment: "staging".to_string(),
                dns_provider: "cloudflare".to_string(),
                dns_config,
                validation_delay: 300,
                last_renewal: None,
                cert_path: None,
                key_path: None,
            },
        };

        let (state, app_config) = make_test_app_state(tls_config);
        let routes = tls_routes_filter(state, app_config);

        let resp = warp::test::request()
            .method("GET")
            .path("/api/tls/config")
            .header("Authorization", "Bearer test-token")
            .reply(&routes)
            .await;

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).expect("valid JSON");
        assert_eq!(body["mode"], "acme");
        assert_eq!(body["acme"]["fqdn"], "llama-monitor.example.com");
        assert_eq!(body["acme"]["environment"], "staging");
        assert_eq!(body["acme"]["dnsProvider"], "cloudflare");
    }

    #[tokio::test]
    async fn tls_config_put_accepts_valid_acme() {
        let (state, app_config) = make_test_app_state(TLSConfig::default());
        let routes = tls_routes_filter(state.clone(), app_config);

        let payload = serde_json::json!({
            "mode": "acme",
            "acme": {
                "enabled": true,
                "fqdn": "llama-monitor.example.com",
                "environment": "staging",
                "dnsProvider": "cloudflare",
                "validationDelay": 300,
                "dnsConfig": {
                    "CF_API_TOKEN": "test-token"
                }
            }
        });

        let resp = warp::test::request()
            .method("PUT")
            .path("/api/tls/config")
            .header("Authorization", "Bearer test-token")
            .json(&payload)
            .reply(&routes)
            .await;

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).expect("valid JSON");
        assert_eq!(body["ok"], true);

        // Verify TLSConfig was updated in state
        let cfg = state.get_tls_config();
        assert_eq!(cfg.mode, TlsMode::Acme);
        assert_eq!(cfg.acme.fqdn, "llama-monitor.example.com");
        assert_eq!(cfg.acme.dns_provider, "cloudflare");
    }

    #[tokio::test]
    async fn tls_config_put_rejects_invalid_acme_missing_provider() {
        let (state, app_config) = make_test_app_state(TLSConfig::default());
        let routes = tls_routes_filter(state, app_config);

        let payload = serde_json::json!({
            "mode": "acme",
            "acme": {
                "enabled": true,
                "fqdn": "llama-monitor.example.com",
                "environment": "staging",
                "dnsProvider": "",
                "dnsConfig": {
                    "CF_API_TOKEN": "test-token"
                }
            }
        });

        let resp = warp::test::request()
            .method("PUT")
            .path("/api/tls/config")
            .header("Authorization", "Bearer test-token")
            .json(&payload)
            .reply(&routes)
            .await;

        assert_eq!(resp.status(), 400);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).expect("valid JSON");
        assert!(
            body["error"]
                .as_str()
                .map(|s| s.contains("dnsProvider"))
                .unwrap_or(false)
        );
    }

    fn make_minimal_chat_tab() -> ChatTab {
        ChatTab {
            id: "tab-1".to_string(),
            name: "Test Tab".to_string(),
            system_prompt: "You are helpful.".to_string(),
            ai_name: None,
            user_name: None,
            explicit_level: None,
            messages: vec![],
            total_input_tokens: None,
            total_output_tokens: None,
            model_params: ChatModelParams::default(),
            created_at: 0,
            updated_at: 0,
            auto_compact: None,
            auto_compact_summarize: None,
            compact_threshold: None,
            compact_mode: None,
            last_ctx_pct: None,
            active_template_id: None,
            context_notes: vec![],
            sidebar_width: 0,
            quick_guide_active: String::new(),
            armed_story_beats: vec![],
            role_boundary_custom: None,
            ai_gender: None,
        }
    }

    #[test]
    fn chat_tab_explicit_level_serialization() {
        let mut tab = make_minimal_chat_tab();
        tab.explicit_level = Some(1);

        let json = serde_json::to_string(&tab).expect("ChatTab should serialize");

        // Verify camelCase key in JSON
        assert!(
            json.contains("\"explicitLevel\""),
            "JSON should contain camelCase 'explicitLevel' field, got: {}",
            json
        );

        // Verify value is correct
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("JSON should parse to Value");
        assert_eq!(
            parsed.get("explicitLevel").and_then(|v| v.as_u64()),
            Some(1),
            "explicitLevel should be 1"
        );

        // Deserialize back and verify
        let deserialized: ChatTab =
            serde_json::from_str(&json).expect("ChatTab should deserialize from own JSON");
        assert_eq!(
            deserialized.explicit_level,
            Some(1),
            "explicit_level should round-trip to Some(1)"
        );
    }

    #[test]
    fn chat_tab_explicit_level_default() {
        let json = r#"{
            "id": "tab-1",
            "name": "Test Tab",
            "system_prompt": "You are helpful.",
            "messages": [],
            "model_params": {
                "temperature": 0.7,
                "top_p": 0.9,
                "top_k": 40,
                "min_p": 0.01,
                "repeat_penalty": 1.0
            },
            "created_at": 0,
            "updated_at": 0
        }"#;

        let result = serde_json::from_str::<ChatTab>(json);
        assert!(
            result.is_ok(),
            "Should deserialize without explicitLevel field"
        );

        let tab = result.unwrap();
        assert!(
            tab.explicit_level.is_none(),
            "explicit_level should default to None when field is absent"
        );
    }

    #[test]
    fn chat_tab_explicit_mode_alias_migration() {
        // The serde alias "explicit_mode" allows deserialization of JSON that uses
        // the legacy field name (instead of camelCase "explicitLevel").
        // The value must still be a u8 to match the explicit_level type.
        let json = r#"{
            "id": "tab-1",
            "name": "Test Tab",
            "system_prompt": "You are helpful.",
            "explicit_mode": 2,
            "messages": [],
            "model_params": {
                "temperature": 0.7,
                "top_p": 0.9,
                "top_k": 40,
                "min_p": 0.01,
                "repeat_penalty": 1.0
            },
            "created_at": 0,
            "updated_at": 0
        }"#;

        let result = serde_json::from_str::<ChatTab>(json);
        assert!(
            result.is_ok(),
            "Should deserialize legacy 'explicit_mode' field via alias"
        );

        let tab = result.unwrap();
        assert_eq!(
            tab.explicit_level,
            Some(2),
            "explicit_mode alias should map to explicit_level"
        );
    }

    #[test]
    fn chat_tab_explicit_level_all_states() {
        for level in [0u8, 1, 2] {
            let mut tab = make_minimal_chat_tab();
            tab.explicit_level = Some(level);

            let json = serde_json::to_string(&tab)
                .unwrap_or_else(|e| panic!("ChatTab should serialize for level {}: {}", level, e));

            // Verify the camelCase key is present
            assert!(
                json.contains("\"explicitLevel\""),
                "JSON for level {} should contain 'explicitLevel'",
                level
            );

            // Deserialize back and verify value
            let deserialized: ChatTab = serde_json::from_str(&json).unwrap_or_else(|e| {
                panic!("ChatTab should deserialize for level {}: {}", level, e)
            });
            assert_eq!(
                deserialized.explicit_level,
                Some(level),
                "explicit_level should round-trip for state {}",
                level
            );
        }
    }

    #[test]
    fn chat_message_compaction_metadata_round_trips() {
        let msg = ChatMessage {
            role: "system".to_string(),
            content: "## Persistent Facts\n- Keeps rolling memory".to_string(),
            timestamp_ms: 123,
            input_tokens: None,
            output_tokens: None,
            cumulative_input_tokens: None,
            cumulative_output_tokens: None,
            compaction_marker: Some(true),
            summarized: Some(true),
            dropped_count: Some(42),
            dropped_preview: Some(vec![CompactionPreview {
                role: "user".to_string(),
                snippet: "example".to_string(),
            }]),
            tokens_freed_estimate: Some(999),
            ctx_pct_before: Some(87.5),
            memory_version: Some(2),
            memory_domain: Some("coding".to_string()),
            summary_kind: Some("rolling-memory".to_string()),
            compacted_at: Some(456),
            compacted_message_count_total: Some(84),
            recent_tail_kept: Some(8),
            thinking_content: None,
        };

        let json = serde_json::to_string(&msg).expect("ChatMessage should serialize");
        let decoded: ChatMessage =
            serde_json::from_str(&json).expect("ChatMessage should deserialize from own JSON");

        assert_eq!(decoded.compaction_marker, Some(true));
        assert_eq!(decoded.memory_version, Some(2));
        assert_eq!(decoded.memory_domain.as_deref(), Some("coding"));
        assert_eq!(decoded.summary_kind.as_deref(), Some("rolling-memory"));
        assert_eq!(decoded.compacted_message_count_total, Some(84));
        assert_eq!(decoded.recent_tail_kept, Some(8));
        assert_eq!(
            decoded
                .dropped_preview
                .as_ref()
                .and_then(|rows| rows.first())
                .map(|row| row.snippet.as_str()),
            Some("example")
        );
    }

    // ── Route smoke tests ──────────────────────────────────────────────────────
    // Each test sends a properly-formed request (correct method + Content-Type)
    // without an API token and asserts 401, not 404.
    //
    // A 404 means the route was accidentally deleted from api_routes().
    // A 401 means the route exists and auth is working correctly.
    //
    // These tests exist specifically to catch the regression from commit ac643ab
    // where a worktree-agent silently deleted 27 handler functions.

    fn make_all_routes()
    -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        // Both api_token AND db_admin_token must be set so that handlers using
        // "check_db_admin_token OR check_api_token" still reject unauthenticated requests.
        let paths = crate::state::AppPaths {
            presets_path: PathBuf::new(),
            templates_path: PathBuf::new(),
            models_dir: None,
            gpu_env_path: PathBuf::new(),
            ui_settings_path: PathBuf::new(),
            sessions_path: PathBuf::new(),
            model_tags_path: PathBuf::new(),
        };
        let cs = Arc::new(
            crate::chat_storage::ChatStorage::open(&PathBuf::from(":memory:"))
                .expect("in-memory chat storage"),
        );
        let state = crate::state::AppState::new(
            vec![],
            paths,
            crate::gpu::env::GpuEnv::default(),
            crate::state::UiSettings::default(),
            cs,
            crate::config::TLSConfig::default(),
        );
        let app_config = Arc::new(crate::config::AppConfig::for_test(
            Some("test-token".to_string()),
            Some("db-admin-token".to_string()),
        ));
        let auth = AuthManager::new(None, None, &crate::config::TlsMode::None);
        super::api_routes(state, app_config, auth, "127.0.0.1".to_string())
    }

    macro_rules! route_smoke_tests {
        // $body: None for GET/DELETE (no body), Some("...json...") for POST/PUT
        // These tests build the full api_routes filter, which can blow the stack
        // on some platforms due to deep warp .or() recursion.
        // They are kept for future use but ignored by default.
        ( $( ($test_name:ident, $method:expr, $path:expr, $body:expr) ),* $(,)? ) => {
            $(
                #[ignore = "stack overflow risk: deep warp .or() recursion in api_routes"]
                #[tokio::test]
                async fn $test_name() {
                    let routes = make_all_routes();
                    let req = warp::test::request()
                        .method($method)
                        .path($path);
                    let body_str: Option<&str> = $body;
                    let resp = if let Some(b) = body_str {
                        req.header("Content-Type", "application/json")
                           .body(b)
                           .reply(&routes)
                           .await
                    } else {
                        req.reply(&routes).await
                    };
                    assert_ne!(
                        resp.status(), 404,
                        "Route {} {} returned 404 — it may have been deleted from api_routes()",
                        $method, $path
                    );
                    assert_eq!(
                        resp.status(), 401,
                        "Route {} {} should require auth (expected 401, got {})",
                        $method, $path, resp.status()
                    );
                }
            )*
        };
    }

    route_smoke_tests![
        // Spawn wizard import
        (
            route_spawn_wizard_import,
            "POST",
            "/api/spawn-wizard/import-launch-file",
            Some("{}")
        ),
        // Chat template
        (
            route_chat_template_fetch,
            "POST",
            "/api/chat-template/fetch",
            Some("{}")
        ),
        (
            route_chat_template_upload,
            "POST",
            "/api/chat-template/upload",
            Some("{}")
        ),
        (
            route_chat_template_install_hf,
            "POST",
            "/api/chat-template/install-hf",
            Some(
                "{\"repo\":\"froggeric/Qwen-Fixed-Chat-Templates\",\"file\":\"chat_template.jinja\",\"name\":\"test\"}"
            )
        ),
        (
            route_chat_template_install_url,
            "POST",
            "/api/chat-template/install-url",
            Some("{}")
        ),
        // VRAM estimation
        (
            route_vram_estimate,
            "POST",
            "/api/vram/estimate",
            Some("{}")
        ),
        (
            route_vram_estimate_breakdown,
            "POST",
            "/api/vram-estimate",
            Some("{}")
        ),
        (
            route_vram_quant_compare,
            "POST",
            "/api/vram/quant-compare",
            Some("{}")
        ),
        (
            route_vram_auto_size,
            "POST",
            "/api/vram/auto-size",
            Some("{}")
        ),
        // Model download
        (
            route_models_download_start,
            "POST",
            "/api/models/download/start",
            Some("{}")
        ),
        (
            route_models_download_status,
            "GET",
            "/api/models/download/test-id/status",
            None
        ),
        (
            route_models_download_cancel,
            "POST",
            "/api/models/download/test-id/cancel",
            Some("{}")
        ),
        // Benchmarking
        (route_benchmark, "POST", "/api/benchmark", Some("{}")),
        // Model metadata
        (
            route_model_defaults,
            "POST",
            "/api/model-defaults",
            Some("{}")
        ),
        (
            route_model_introspect,
            "POST",
            "/api/model/introspect",
            Some("{}")
        ),
        (
            route_third_party_models,
            "POST",
            "/api/third-party-models",
            Some("{}")
        ),
        // MoE tuning
        (route_moe_tune, "POST", "/api/moe-tune", Some("{}")),
        // Inference tuning advisor / bench
        (route_advise, "POST", "/api/advise", Some("{}")),
        (route_tune_ncpumoe, "POST", "/api/tune/ncpumoe", Some("{}")),
        (route_bench_sweep, "POST", "/api/bench/sweep", Some("{}")),
        // HuggingFace
        (route_hf_search, "POST", "/api/hf/search", Some("{}")),
        (route_hf_files, "POST", "/api/hf/files", Some("{}")),
        (
            route_hf_community_picks,
            "GET",
            "/api/hf/community-picks",
            None
        ),
        (route_hf_quantizers_get, "GET", "/api/hf/quantizers", None),
        // hf_quantizers_put expects Vec<UserQuantizer> — send empty array, not {}
        (
            route_hf_quantizers_put,
            "PUT",
            "/api/hf/quantizers",
            Some("[]")
        ),
        (route_hf_download_dir, "GET", "/api/hf/download-dir", None),
        (route_hf_token_get, "GET", "/api/hf/token", None),
        (route_hf_token_put, "PUT", "/api/hf/token", Some("{}")),
        (route_hf_token_delete, "DELETE", "/api/hf/token", None),
        // hf/card requires ?repo= param — without it we expect 400, not 404
        (route_hf_card, "GET", "/api/hf/card?repo=test%2Fmodel", None),
        (route_hf_download, "POST", "/api/hf/download", Some("{}")),
        // llama-server binary updater
        (
            route_llama_binary_version,
            "GET",
            "/api/llama-binary/version",
            None
        ),
        (
            route_llama_binary_latest,
            "GET",
            "/api/llama-binary/latest",
            None
        ),
        (
            route_llama_binary_update,
            "POST",
            "/api/llama-binary/update",
            Some("{}")
        ),
    ];
}
