use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use warp::Filter;

use crate::config::AppConfig;
use crate::inference::launch::{
    launch_local, request_from_api_payload, request_from_preset, request_from_session,
};
use crate::llama::server::stop_server;
use crate::state::{self as app_state, AppState, SessionMode, SessionStatus};

use super::common::{ApiCtx, ApiError, ApiRoute, box_reply};
use super::common::{
    bearer_matches_api_token, bearer_matches_db_admin_token, check_api_token, check_db_admin_token,
    unauthorized_api_token, unauthorized_db_admin_token, with_app_config,
};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    api_restore_hint(state.clone())
        .map(box_reply)
        .or(api_get_sessions(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_get_recent_sessions(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_check_endpoint_health(config.clone()).map(box_reply))
        .unify()
        .or(api_create_session(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_delete_session(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_get_active_session(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_get_active_session_readiness(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_set_active_session(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_get_capabilities(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_spawn_session_with_preset(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_attach(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_detach(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_kill_llama(state, config).map(box_reply))
        .unify()
        .boxed()
}

// ==================== RESTORE HINT ENDPOINT ====================

fn api_restore_hint(
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
                        SessionStatus::Running => "Running".to_string(),
                        SessionStatus::Stopped => "Stopped".to_string(),
                        SessionStatus::Disconnected => "Disconnected".to_string(),
                        SessionStatus::Error(_) => "Error".to_string(),
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

// ==================== SESSION CRUD ====================

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
                sessions.sort_by_key(|s| std::cmp::Reverse(s.last_connected_at));
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
                            SessionMode::Spawn { port, .. } => {
                                format!("Spawn:{}", port)
                            }
                            SessionMode::Attach { endpoint, .. } => {
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
                    SessionMode::Spawn { port, api_key, .. } => {
                        (format!("http://127.0.0.1:{port}"), api_key)
                    }
                    SessionMode::Attach { endpoint, api_key } => (endpoint, api_key),
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

                let requested_port = payload
                    .get("port")
                    .and_then(|value| value.as_u64())
                    .and_then(|port| u16::try_from(port).ok());
                let port = requested_port.unwrap_or(8001);
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

                if let Some(restored_id) = payload.get("session_id").and_then(|v| v.as_str()) {
                    let session = state
                        .get_sessions()
                        .into_iter()
                        .find(|session| session.id == restored_id);
                    let Some(session) = session else {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"ok": false, "error": "Restored session not found"})),
                                warp::http::StatusCode::BAD_REQUEST,
                            ),
                        ));
                    };
                    let presets = state.presets.lock().unwrap().clone();
                    let transient_api_key = payload.get("api_key").and_then(|v| v.as_str());
                    let request = match request_from_session(&session, &presets, transient_api_key) {
                        Ok(request) => request,
                        Err(error) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({"ok": false, "error": error.to_string()})),
                                    warp::http::StatusCode::BAD_REQUEST,
                                ),
                            ));
                        }
                    };
                    let response_backend = request.backend();
                    let response_port = request.port();
                    let previous_active_id = state.active_session_id.lock().unwrap().clone();
                    state.set_active_session(restored_id);
                    return match launch_local(Arc::new(state.clone()), request, &app_config).await {
                        Ok(()) => {
                            state.update_session_status(restored_id, SessionStatus::Running);
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({"ok": true, "session_id": restored_id, "backend": response_backend, "port": response_port})),
                            ))
                        }
                        Err(error) => {
                            restore_active_session_after_failed_launch(
                                &state,
                                restored_id,
                                &previous_active_id,
                            );
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({"ok": false, "error": error.to_string()})),
                                    warp::http::StatusCode::BAD_REQUEST,
                                ),
                            ))
                        }
                    };
                }

                let Some(preset_id) = payload
                    .get("preset_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                else {
                    let request = match request_from_api_payload(&payload) {
                        Ok(request) => request,
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()})),
                                    warp::http::StatusCode::BAD_REQUEST,
                                ),
                            ));
                        }
                    };

                    let session_name = if name != format!("Session on port {}", port) {
                        name.clone()
                    } else if !request.model_identity().is_empty() {
                        let identity = request.model_identity();
                        let filename = std::path::Path::new(&identity)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&identity);
                        format!("Local: {}", filename)
                    } else {
                        name.clone()
                    };

                    let session_id = app_state::generate_session_id();
                    let mut session = app_state::Session::new_spawn_with_backend(
                        session_id.clone(),
                        session_name,
                        request.port(),
                        String::new(),
                        request.bind_host(),
                        request.api_key(),
                        request.backend(),
                        Some(request.model_identity()),
                    );
                    session.launch = Some(request.for_persistence());

                    if !state.add_session(session) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Failed to create session"}),
                            ),
                        ));
                    }

                    state.set_active_session(&session_id);
                    let response_backend = request.backend();
                    let response_port = request.port();

                    match launch_local(Arc::new(state.clone()), request, &app_config).await {
                        Ok(()) => {
                            state.update_session_status(&session_id, SessionStatus::Running);
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(
                                    &serde_json::json!({"ok": true, "session_id": session_id, "backend": response_backend, "port": response_port}),
                                ),
                            ));
                        }
                        Err(e) => {
                            state.remove_session(&session_id);
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()})),
                                    warp::http::StatusCode::BAD_REQUEST,
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

                let request = match request_from_preset(&preset, requested_port) {
                    Ok(request) => request,
                    Err(error) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"ok": false, "error": error.to_string()})),
                                warp::http::StatusCode::BAD_REQUEST,
                            ),
                        ));
                    }
                };
                let session_id = app_state::generate_session_id();
                let mut session = app_state::Session::new_spawn_with_backend(
                    session_id.clone(),
                    name.clone(),
                    request.port(),
                    preset_id,
                    request.bind_host(),
                    request.api_key(),
                    request.backend(),
                    Some(request.model_identity()),
                );
                session.launch = Some(request.for_persistence());

                if !state.add_session(session) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "Failed to create session"}),
                    )));
                }

                state.set_active_session(&session_id);
                let response_backend = request.backend();
                let response_port = request.port();

                match launch_local(Arc::new(state.clone()), request, &app_config).await {
                    Ok(()) => {
                        state.update_session_status(&session_id, SessionStatus::Running);
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": true, "session_id": session_id, "backend": response_backend, "port": response_port}),
                        )))
                    }
                    Err(e) => {
                        state.remove_session(&session_id);
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()})),
                                warp::http::StatusCode::BAD_REQUEST,
                            ),
                        ))
                    }
                }
            }
        })
}

fn restore_active_session_after_failed_launch(
    state: &AppState,
    failed_session_id: &str,
    previous_active_id: &str,
) {
    let failed_session_is_still_active =
        state.active_session_id.lock().unwrap().as_str() == failed_session_id;
    if failed_session_is_still_active {
        state.set_active_session(previous_active_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_restore_rolls_back_active_session() {
        let state = AppState::default();
        assert!(state.add_session(app_state::Session::new_spawn(
            "previous".into(),
            "Previous".into(),
            8001,
            String::new(),
            None,
            None,
        )));
        assert!(state.add_session(app_state::Session::new_spawn(
            "failed".into(),
            "Failed".into(),
            8002,
            String::new(),
            None,
            None,
        )));
        assert!(state.set_active_session("previous"));
        assert!(state.set_active_session("failed"));

        restore_active_session_after_failed_launch(&state, "failed", "previous");

        assert_eq!(state.active_session_id.lock().unwrap().as_str(), "previous");
    }

    #[test]
    fn failed_restore_does_not_overwrite_newer_active_selection() {
        let state = AppState::default();
        for id in ["previous", "failed", "newer"] {
            assert!(state.add_session(app_state::Session::new_spawn(
                id.into(),
                id.into(),
                8001,
                String::new(),
                None,
                None,
            )));
        }
        assert!(state.set_active_session("newer"));

        restore_active_session_after_failed_launch(&state, "failed", "previous");

        assert_eq!(state.active_session_id.lock().unwrap().as_str(), "newer");
    }
}

fn api_attach(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
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
                            let parsed = url::Url::parse(s).map_err(|_| warp::reject::not_found())?;
                            if !["http", "https"].contains(&parsed.scheme()) {
                                return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({"ok": false, "error": "Endpoint must use http:// or https://"})),
                                    warp::http::StatusCode::OK,
                                ));
                            }
                            if let Some(host) = parsed.host_str()
                                && let Ok(ip) = host.parse::<std::net::IpAddr>() {
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

                let caller_api_key: Option<String> = payload.get("api_key")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());

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
                                if let SessionMode::Spawn {
                                    port,
                                    api_key: sess_key,
                                    ..
                                } = &sess.mode {
                                    *port == port_num && sess_key.is_some()
                                } else {
                                    false
                                }
                            }) {
                                if let SessionMode::Spawn {
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

                let mut metrics_req = client.get(format!("{}/health", endpoint.trim_end_matches('/')));
                if let Some(ref key) = api_key {
                    metrics_req = metrics_req.header("Authorization", format!("Bearer {}", key));
                }
                let metrics_available = metrics_req.send().await.is_ok();

                let existing_session_id = {
                    let sessions = state.sessions.lock().unwrap();
                    let mut id = sessions.iter().find(|s| {
                        if let SessionMode::Attach { endpoint: ep, .. } = &s.mode {
                            *ep == endpoint
                        } else {
                            false
                        }
                    }).map(|s| s.id.clone());

                    if id.is_none()
                        && let Ok(parsed) = url::Url::parse(&endpoint)
                        && let Some(host) = parsed.host_str()
                        && let Some(port_num) = parsed.port()
                        && (host == "127.0.0.1" || host == "localhost") {
                            id = sessions.iter().find(|s| {
                                if let SessionMode::Spawn {
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
                    eprintln!("[info] Reusing existing session for {}", endpoint);
                    id
                } else {
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

                let sessions = state.sessions.lock().unwrap();
                let session = sessions.iter().find(|s| s.id == active_id);

                let is_attach = session.map(|s| matches!(s.mode, SessionMode::Attach { .. }));

                if !is_attach.unwrap_or(false) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "Active session is not an attach session"}),
                    )));
                }

                {
                    let mut sessions = state.sessions.lock().unwrap();
                    if let Some(s) = sessions.iter_mut().find(|s| s.id == active_id) {
                        s.last_active = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                    }
                }

                drop(sessions);
                state.set_active_session("");
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

                state.push_log("[monitor] kill-llama: kill-llama requested (best-effort)".into());
                let had_managed_process = state.server_child.lock().await.is_some()
                    || state.supervisor.lock().await.is_some();
                match stop_server(&state).await {
                    Ok(()) if had_managed_process => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "ok": true })),
                        ));
                    }
                    Ok(()) => {
                        state.push_log(
                            "[monitor] kill-llama: no supervised process; trying legacy process cleanup"
                                .into(),
                        );
                    }
                    Err(e) => {
                        state.push_log(format!("[monitor] stop_server fallback: {}", e));
                    }
                }

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
