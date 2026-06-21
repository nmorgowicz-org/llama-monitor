use std::path::PathBuf;
use std::sync::Arc;

use warp::Filter;

use crate::config::AppConfig;
use crate::state::AppState;

use super::common::{ApiCtx, ApiRoute, box_reply, check_api_token, unauthorized_api_token};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    api_browse(state, config).map(box_reply).boxed()
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

                    let mut allowed_roots: Vec<PathBuf> = Vec::new();

                    if let Some(home) = dirs::home_dir()
                        && let Ok(canon) = home.canonicalize()
                    {
                        allowed_roots.push(canon);
                    }

                    if let Some(ref models_dir) = state.models_dir
                        && let Some(parent) = models_dir.parent()
                        && let Ok(canon) = parent.canonicalize()
                    {
                        allowed_roots.push(canon);
                    }

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
