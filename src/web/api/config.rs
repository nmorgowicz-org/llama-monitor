use std::path::PathBuf;
use std::sync::Arc;

use warp::Filter;

use crate::config::{AppConfig, DashboardAuthConfig, clear_auth_config, save_auth_config};
use crate::gpu::env::{self as gpu_env, GPU_ARCHITECTURES, GpuEnv};
use crate::state::{self as app_state, AppState, UiSettings};
use crate::web::auth::{AuthManager, AuthSource};

use super::common::{
    ApiCtx, ApiRoute, bearer_matches_api_token, check_api_token, unauthorized_api_token,
    with_app_config,
};

fn api_get_auth_config(
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "auth" / "config")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .map(move |auth: Option<String>| {
            let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
            if !bearer_matches_api_token(bearer.as_deref(), &app_config) {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                    warp::http::StatusCode::UNAUTHORIZED,
                )) as Box<dyn warp::reply::Reply>;
            }
            let view = auth_manager.config_view();
            Box::new(warp::reply::json(&serde_json::json!({
                "source": match view.source {
                    AuthSource::None => "none",
                    AuthSource::Config => "config",
                    AuthSource::Cli => "cli",
                },
                "basicEnabled": view.basic_enabled,
                "formEnabled": view.form_enabled,
                "username": view.username,
                "managedByCli": matches!(view.source, AuthSource::Cli),
                "recoveryCommand": "llama-monitor --clear-auth-config",
                "recoveryFile": app_config.auth_config_file.display().to_string(),
            }))) as Box<dyn warp::reply::Reply>
        })
}

fn api_put_auth_config(
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    #[derive(serde::Deserialize)]
    struct UpdateAuthConfigRequest {
        basic_enabled: bool,
        form_enabled: bool,
        username: String,
        #[serde(default)]
        current_password: String,
        #[serde(default)]
        new_password: String,
    }

    warp::path!("api" / "auth" / "config")
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::content_length_limit(64 * 1024))
        .and(warp::body::json())
        .map(move |auth: Option<String>, req: UpdateAuthConfigRequest| {
            let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
            if !bearer_matches_api_token(bearer.as_deref(), &app_config) {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                    warp::http::StatusCode::UNAUTHORIZED,
                )) as Box<dyn warp::reply::Reply>;
            }

            if matches!(auth_manager.source(), AuthSource::Cli) {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "error": "managed_by_cli",
                        "message": "This instance is using startup auth flags. Remove --basic-auth/--form-auth to manage dashboard access in the app."
                    })),
                    warp::http::StatusCode::CONFLICT,
                )) as Box<dyn warp::reply::Reply>;
            }

            if !req.basic_enabled && !req.form_enabled {
                if let Err(err) = clear_auth_config(&app_config.config_dir) {
                    return Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "error": "save_failed",
                            "message": err.to_string(),
                        })),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    )) as Box<dyn warp::reply::Reply>;
                }
                auth_manager.disable();
                return Box::new(warp::reply::json(&serde_json::json!({
                    "ok": true,
                    "message": "Dashboard auth disabled.",
                }))) as Box<dyn warp::reply::Reply>;
            }

            let username = req.username.trim();
            if username.is_empty() {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "error": "invalid_username",
                        "message": "Username is required when dashboard auth is enabled."
                    })),
                    warp::http::StatusCode::BAD_REQUEST,
                )) as Box<dyn warp::reply::Reply>;
            }

            let existing_view = auth_manager.config_view();
            let changing_password = !req.new_password.trim().is_empty();
            if changing_password && req.new_password.len() < 8 {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "error": "weak_password",
                        "message": "Use at least 8 characters for the dashboard password."
                    })),
                    warp::http::StatusCode::BAD_REQUEST,
                )) as Box<dyn warp::reply::Reply>;
            }

            if matches!(existing_view.source, AuthSource::Config)
                && existing_view.username.is_some()
                && changing_password
                && !auth_manager.verify_any_password(&req.current_password)
            {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "error": "invalid_current_password",
                        "message": "Current password did not match the stored dashboard password."
                    })),
                    warp::http::StatusCode::UNAUTHORIZED,
                )) as Box<dyn warp::reply::Reply>;
            }

            let password_hash = if changing_password {
                match AuthManager::hash_password(&req.new_password) {
                    Some(hash) => hash,
                    None => {
                        return Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "error": "hash_failed",
                                "message": "Failed to hash the new password."
                            })),
                            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                        )) as Box<dyn warp::reply::Reply>;
                    }
                }
            } else {
                let current = crate::config::load_auth_config(&app_config.config_dir);
                if current.password_hash.is_empty() {
                    return Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "error": "missing_password",
                            "message": "Enter a new password to enable dashboard auth."
                        })),
                        warp::http::StatusCode::BAD_REQUEST,
                    )) as Box<dyn warp::reply::Reply>;
                }
                current.password_hash
            };

            let cfg = DashboardAuthConfig {
                basic_enabled: req.basic_enabled,
                form_enabled: req.form_enabled,
                username: username.to_string(),
                password_hash,
            };

            if let Err(err) = save_auth_config(&app_config.config_dir, &cfg) {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "error": "save_failed",
                        "message": err.to_string(),
                    })),
                    warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                )) as Box<dyn warp::reply::Reply>;
            }

            auth_manager.replace_with_config(cfg);

            Box::new(warp::reply::json(&serde_json::json!({
                "ok": true,
                "message": if changing_password {
                    "Dashboard access updated and sessions refreshed."
                } else {
                    "Dashboard access updated."
                }
            }))) as Box<dyn warp::reply::Reply>
        })
}

fn api_get_gpu_env(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "gpu-env")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let env = state.gpu_env.lock().unwrap().clone();
            let detected = gpu_env::detect_gpus();
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&serde_json::json!({
                    "env": env,
                    "architectures": GPU_ARCHITECTURES,
                    "detected": detected,
                }))),
            ))
        })
}

fn api_put_gpu_env(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "gpu-env")
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move |auth: Option<String>, updated: GpuEnv| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let mut env = state.gpu_env.lock().unwrap();
            *env = updated;
            let _ = gpu_env::save_gpu_env(&state.gpu_env_path, &env);
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
            ))
        })
}

fn api_get_settings(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "settings")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let settings = state.ui_settings.lock().unwrap().clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let masked = mask_remote_agent_token(settings);
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&masked)),
            ))
        })
}

fn api_get_settings_full(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();

    warp::path!("api" / "settings" / "full")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .map(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
            let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

            if !has_api_token {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                    warp::http::StatusCode::UNAUTHORIZED,
                )) as Box<dyn warp::reply::Reply>;
            }

            let mut settings = state.ui_settings.lock().unwrap().clone();
            if settings.llama_server_path.is_empty() {
                settings.llama_server_path = cfg.llama_server_path.display().to_string();
            }
            Box::new(warp::reply::json(&settings))
        })
}

fn is_masked_token(t: &str) -> bool {
    t.contains('•')
}

fn mask_remote_agent_token(mut s: UiSettings) -> UiSettings {
    if s.remote_agent_token.len() <= 8 {
        if !s.remote_agent_token.is_empty() {
            s.remote_agent_token = "••••".to_string();
        }
    } else {
        let t = &s.remote_agent_token;
        let masked = format!("{}••••••••••••••••{}", &t[..4], &t[t.len() - 4..]);
        s.remote_agent_token = masked;
    }
    s
}

fn api_put_settings(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "settings")
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move |auth: Option<String>, mut updated: UiSettings| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            // Detect if this is a partial update (only ws_push_interval_ms set, rest are defaults)
            let is_partial = updated.preset_id.is_empty()
                && updated.port == 8001
                && updated.llama_server_path.is_empty()
                && updated.llama_server_cwd.is_empty()
                && updated.models_dir.is_empty()
                && updated.server_endpoint.is_empty()
                && updated.llama_poll_interval == 1
                && updated.remote_agent_url.is_empty()
                && updated.remote_agent_token.is_empty()
                && !updated.remote_agent_ssh_autostart
                && updated.remote_agent_ssh_target.is_empty()
                && updated.remote_agent_ssh_command.is_empty()
                && updated.explicit_mode_policy.is_empty()
                && updated.context_card_view == "gauge";

            let mut settings = state.ui_settings.lock().unwrap();
            let old_dir = settings.models_dir.clone();
            let old_token = settings.remote_agent_token.clone();
            let old_push_interval = settings.ws_push_interval_ms;

            if is_partial {
                settings.ws_push_interval_ms = updated.ws_push_interval_ms;
            } else {
                let incoming_token = updated.remote_agent_token.clone();
                if is_masked_token(&incoming_token) && !old_token.is_empty() {
                    updated.remote_agent_token = old_token.clone();
                }
                *settings = updated;
            }

            let new_dir = settings.models_dir.clone();
            let token_changed =
                settings.remote_agent_token != old_token && !settings.remote_agent_token.is_empty();
            let push_interval_changed = settings.ws_push_interval_ms != old_push_interval;

            let _ = app_state::save_ui_settings(&state.ui_settings_path, &settings);
            drop(settings);

            if token_changed {
                state.agent_poll_notify.notify_waiters();
            }
            if push_interval_changed {
                state.llama_poll_notify.notify_waiters();
            }

            if new_dir != old_dir
                && !new_dir.is_empty()
                && let Ok(discovered) = crate::models::scan_gguf_library(&PathBuf::from(&new_dir))
            {
                *state.discovered_models.lock().unwrap() = discovered;
            }

            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
            ))
        })
}

fn api_rotate_agent_token(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    let state = state.clone();
    let app_config = app_config.clone();

    warp::path!("api" / "rotate-agent-token")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                let new_token = crate::config::generate_random_token();

                let mut settings = state.ui_settings.lock().unwrap();
                settings.remote_agent_token = new_token;
                let _ = app_state::save_ui_settings(&state.ui_settings_path, &settings);
                drop(settings);

                state.agent_poll_notify.notify_waiters();

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "message": "Agent token rotated"
                    }),
                )))
            }
        })
}

fn api_rotate_api_token(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();

    warp::path!("api" / "rotate-api-token")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, cfg: Arc<AppConfig>| async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                let new_token = crate::config::generate_random_token();

                let config_dir = cfg.config_dir.clone();
                let token_file = config_dir.join("api-token");
                let stored = crate::config::encrypt_value(&new_token);
                if let Err(e) = std::fs::write(&token_file, &stored) {
                    eprintln!("[api] Failed to write rotated api-token to {token_file:?}: {e}");
                }
                crate::config::harden_file_permissions(&token_file);
                cfg.update_live_api_token(new_token);

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "message": "API token rotated successfully."
                    }),
                )))
            },
        )
}

fn api_rotate_db_admin_token(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();

    warp::path!("api" / "rotate-db-admin-token")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, cfg: Arc<AppConfig>| async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                let new_token = crate::config::generate_random_token();

                let config_dir = cfg.config_dir.clone();
                let token_file = config_dir.join("db-admin-token");
                let stored = crate::config::encrypt_value(&new_token);
                if let Err(e) = std::fs::write(&token_file, &stored) {
                    eprintln!(
                        "[api] Failed to write rotated db-admin-token to {token_file:?}: {e}"
                    );
                }
                crate::config::harden_file_permissions(&token_file);
                cfg.update_live_db_admin_token(new_token);

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "message": "DB admin token rotated successfully."
                    }),
                )))
            },
        )
}

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state.clone();
    let config = ctx.config.clone();
    let auth = ctx.auth.clone();

    let mut r = api_get_auth_config(config.clone(), auth.clone())
        .or(api_put_auth_config(config.clone(), auth.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_get_gpu_env(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_put_gpu_env(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_get_settings(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_get_settings_full(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_put_settings(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_rotate_agent_token(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r.or(api_rotate_api_token(config.clone())).unify().boxed();
    r = r
        .or(api_rotate_db_admin_token(config.clone()))
        .unify()
        .boxed();
    r
}
