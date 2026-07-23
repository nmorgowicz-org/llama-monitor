use std::path::{Path, PathBuf};
use std::sync::Arc;

use once_cell::sync::Lazy;
use warp::Filter;
use warp::http::StatusCode;

use crate::config::AppConfig;
use crate::state::AppState;

use super::common::{ApiCtx, ApiRoute, check_api_token, try_cooldown, unauthorized_api_token};
use super::models::get_effective_models_dir;

static HF_REPO_RE: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"^[a-zA-Z0-9_-]+/[a-zA-Z0-9._-]+$").unwrap());

fn validate_hf_repo_id(repo_id: &str) -> bool {
    HF_REPO_RE.is_match(repo_id)
}

/// Map the `format` field of an `/api/hf/search` request body to the
/// `HfModelFormat` threaded into the outgoing HF API `filter=` query param.
/// Anything other than `"mlx"` (including absent/empty/unrecognized values)
/// falls back to `Gguf` for backward compatibility.
pub(crate) fn parse_hf_format_param(raw: &str) -> crate::hf::HfModelFormat {
    match raw.to_lowercase().as_str() {
        "mlx" => crate::hf::HfModelFormat::Mlx,
        "both" | "all" => crate::hf::HfModelFormat::Both,
        _ => crate::hf::HfModelFormat::Gguf,
    }
}

pub(crate) fn resolve_hf_target_dir(
    models_dir: &Path,
    target_path: Option<&str>,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(models_dir).map_err(|e| {
        format!(
            "Failed to create models_dir {}: {}",
            models_dir.display(),
            e
        )
    })?;
    let models_dir_canon = models_dir.canonicalize().map_err(|e| {
        format!(
            "Failed to resolve models_dir {}: {}",
            models_dir.display(),
            e
        )
    })?;

    let Some(tp) = target_path else {
        return Ok(models_dir.to_path_buf());
    };

    if tp.contains("..") || tp.starts_with('\\') || tp.starts_with('/') {
        return Err("Invalid target_path: path traversal not allowed".to_string());
    }

    let candidate = models_dir.join(tp);
    std::fs::create_dir_all(&candidate)
        .map_err(|e| format!("Failed to create target_path: {}", e))?;
    let candidate_canon = candidate
        .canonicalize()
        .map_err(|e| format!("Failed to resolve target_path: {}", e))?;

    if !candidate_canon.starts_with(&models_dir_canon) {
        return Err("target_path escapes models_dir".to_string());
    }

    Ok(candidate_canon)
}

fn api_hf_search(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    // (window_start_secs, request_count) — protected by Mutex to avoid TOCTOU races.
    static HF_SEARCH_RATE: std::sync::LazyLock<std::sync::Mutex<(u64, u64)>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new((0, 0)));

    warp::path!("api" / "hf" / "search")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let now = std::time::SystemTime::UNIX_EPOCH
                    .elapsed()
                    .unwrap_or_default()
                    .as_secs();

                // Check and update rate limit atomically under the Mutex.
                let rate_limited = {
                    let mut guard = HF_SEARCH_RATE.lock().unwrap();
                    let (ref mut window_start, ref mut count) = *guard;
                    if now.saturating_sub(*window_start) >= 60 {
                        *window_start = now;
                        *count = 1;
                        false
                    } else if *count >= 10 {
                        true
                    } else {
                        *count += 1;
                        false
                    }
                };

                if rate_limited {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Rate limited: too many HF search requests. Try again in 60 seconds."
                            })),
                            StatusCode::TOO_MANY_REQUESTS,
                        )),
                    );
                }

                let query = body["query"].as_str().unwrap_or("").trim().to_string();
                let author = body["author"].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                let limit: u64 = body["limit"].as_u64().unwrap_or(20).min(100);

                let sort = match body["sort"].as_str().unwrap_or("downloads") {
                    "likes"     => crate::hf::HfSort::Likes,
                    "newest"    | "createdAt" => crate::hf::HfSort::CreatedAt,
                    "trending"  => crate::hf::HfSort::Trending,
                    _           => crate::hf::HfSort::Downloads,
                };

                // Require at least a query or an author — unless sorting by trending or
                // downloads (in which case empty query returns a global popular/trending list).
                if query.is_empty()
                    && author.is_none()
                    && sort != crate::hf::HfSort::Trending
                    && sort != crate::hf::HfSort::Downloads
                {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Provide 'query' or 'author' (or both)"
                        }))),
                    );
                }

                let cursor = body["cursor"].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                let format = parse_hf_format_param(body["format"].as_str().unwrap_or("gguf"));
                let quants_only = body["quantsOnly"].as_bool().unwrap_or(false);
                let params = crate::hf::HfSearchParams { query, author, sort, limit: limit as usize, cursor, format, quants_only };

                match crate::hf::hf_search_models(&params).await {
                    Ok((models, next_cursor)) => {
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "models": models,
                            "next_cursor": next_cursor
                        }))))
                    },
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": e
                        }))),
                    ),
                }
            }
        })
}

fn api_hf_files(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "files")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let repo_id = body["repo_id"].as_str().unwrap_or("").trim().to_string();
                let format = body["format"].as_str().unwrap_or("gguf");
                if repo_id.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'repo_id' field"
                        })),
                    ));
                }
                if !validate_hf_repo_id(&repo_id) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Invalid repo_id format. Expected: owner/repo"
                            })),
                            StatusCode::BAD_REQUEST,
                        ),
                    ));
                }

                let result: Result<Vec<serde_json::Value>, String> = if format == "mlx" {
                    crate::hf::hf_list_mlx_files(&repo_id).await
                } else {
                    match crate::hf::hf_list_gguf_files(&repo_id).await {
                        Ok(files) => Ok(files.into_iter().map(|f| serde_json::json!(f)).collect()),
                        Err(e) => Err(e),
                    }
                };

                match result {
                    Ok(files) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "files": files
                        })),
                    )),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": e
                        })),
                    )),
                }
            }
        })
}

fn api_hf_community_picks(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "community-picks")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let path = cfg.config_dir.join("community-picks.json");
                let body = if path.exists() {
                    match std::fs::read_to_string(&path) {
                        Ok(s) => match serde_json::from_str::<serde_json::Value>(&s) {
                            Ok(v) => serde_json::json!({ "ok": true, "data": v }),
                            Err(e) => serde_json::json!({
                                "ok": false,
                                "error": format!("community-picks.json parse error: {e}")
                            }),
                        },
                        Err(e) => serde_json::json!({
                            "ok": false,
                            "error": format!("community-picks.json read error: {e}")
                        }),
                    }
                } else {
                    serde_json::json!({ "ok": true, "data": null })
                };
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &body,
                )))
            }
        })
}

fn api_hf_quantizers(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "quantizers")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                if let Some(user_list) = crate::hf::load_user_quantizers(&cfg.config_dir) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({ "ok": true, "quantizers": user_list, "is_custom": true }),
                    )));
                }
                let defaults: Vec<crate::hf::UserQuantizer> = crate::hf::known_gguf_quantizers()
                    .iter().map(Into::into).collect();
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "ok": true, "quantizers": defaults, "is_custom": false }),
                )))
            }
        })
}

fn api_hf_quantizers_put(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "quantizers")
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(
            warp::body::content_length_limit(256 * 1024)
                .and(warp::body::json::<Vec<crate::hf::UserQuantizer>>()),
        )
        .and_then(
            move |auth: Option<String>, body: Vec<crate::hf::UserQuantizer>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    // Empty list = reset to defaults (remove user file)
                    if body.is_empty() {
                        let _ = std::fs::remove_file(cfg.config_dir.join("hf-quantizers.json"));
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "ok": true, "reset": true })),
                        ));
                    }
                    match crate::hf::save_user_quantizers(&cfg.config_dir, &body) {
                        Ok(()) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "ok": true })),
                        )),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({ "ok": false, "error": format!("{e}") }),
                            ),
                        )),
                    }
                }
            },
        )
}

fn api_hf_download_dir(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "download-dir")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            let st = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let dir =
                    get_effective_models_dir(&st).unwrap_or_else(|| cfg.default_models_dir.clone());
                let configured = get_effective_models_dir(&st).is_some();
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "dir": dir.to_string_lossy(),
                        "configured": configured
                    }),
                )))
            }
        })
}

fn api_hf_card(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "card")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and_then(
            move |auth: Option<String>, params: std::collections::HashMap<String, String>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }

                    let repo = match params.get("repo") {
                        Some(r) if !r.is_empty() => r.clone(),
                        _ => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({
                                        "error": "Missing required query param: repo"
                                    })),
                                    warp::http::StatusCode::BAD_REQUEST,
                                ),
                            ));
                        }
                    };

                    // Basic path-traversal guard: repo must be "owner/name" with no dots or slashes beyond that
                    let parts: Vec<&str> = repo.splitn(3, '/').collect();
                    if parts.len() != 2 || parts.iter().any(|p| p.is_empty() || p.contains("..")) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(
                                    &serde_json::json!({ "error": "Invalid repo id" }),
                                ),
                                warp::http::StatusCode::BAD_REQUEST,
                            ),
                        ));
                    }

                    let url = format!("https://huggingface.co/{}/raw/main/README.md", repo);

                    let builder = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(20))
                        .user_agent("llama-monitor");

                    let client = match builder.build() {
                        Ok(c) => c,
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({ "error": e.to_string() })),
                            ));
                        }
                    };

                    let mut req = client.get(&url);
                    if let Some(token) = crate::hf::hf_load_token() {
                        req = req.header("Authorization", format!("Bearer {}", token));
                    }

                    let resp = match req.send().await {
                        Ok(r) => r,
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({ "error": e.to_string() })),
                            ));
                        }
                    };

                    if resp.status() == reqwest::StatusCode::NOT_FOUND {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "markdown": "" })),
                        ));
                    }

                    if !resp.status().is_success() {
                        let status = resp.status().as_u16();
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "error": format!("HuggingFace returned HTTP {}", status)
                            })),
                        ));
                    }

                    // Cap at 256 KB — large READMEs still render, but we don't buffer unlimited data
                    let bytes = match resp.bytes().await {
                        Ok(b) => b,
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({ "error": e.to_string() })),
                            ));
                        }
                    };
                    let markdown =
                        String::from_utf8_lossy(&bytes[..bytes.len().min(256 * 1024)]).into_owned();

                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({ "markdown": markdown }),
                    )))
                }
            },
        )
}

fn api_hf_meta(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "meta")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and_then(
            move |auth: Option<String>, params: std::collections::HashMap<String, String>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    let repo = match params.get("repo") {
                        Some(r) if !r.is_empty() => r.clone(),
                        _ => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(
                                    &serde_json::json!({"ok": false, "error": "missing repo param"}),
                                ),
                            ));
                        }
                    };
                    let parts: Vec<&str> = repo.splitn(3, '/').collect();
                    if parts.len() != 2 || parts.iter().any(|p| p.is_empty() || p.contains("..")) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "invalid repo id"}),
                            ),
                        ));
                    }
                    match crate::hf::hf_get_model_info(&repo).await {
                        Ok(info) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": true,
                                "tags": info.tags,
                                "gated": info.gated,
                                "private": info.private,
                            })),
                        )),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": e.to_string()}),
                            ),
                        )),
                    }
                }
            },
        )
}

fn api_hf_resolve_origin(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "resolve-origin")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let filename = body["filename"].as_str().unwrap_or("").trim().to_string();
                let size_bytes: u64 = body["size_bytes"].as_u64().unwrap_or(0);

                if filename.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "filename is required"}),
                        ),
                    ));
                }

                match crate::hf::hf_resolve_origin(&filename, size_bytes).await {
                    Ok(result) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "confident": result.confident,
                            "candidates": result.candidates,
                            "model_stem": result.model_stem,
                            "errors": result.errors,
                        })),
                    )),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": e,
                        })),
                    )),
                }
            }
        })
}

fn api_hf_token_get(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "token")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let set = crate::hf::hf_load_token().is_some();
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "set": set }),
                )))
            }
        })
}

fn api_hf_token_put(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "token")
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let token = body["token"].as_str().unwrap_or("").trim().to_string();
                if token.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({ "ok": false, "error": "token is required" }),
                        ),
                    ));
                }
                match crate::hf::hf_save_token(&token) {
                    Ok(()) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": true })),
                    )),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({ "ok": false, "error": e.to_string() }),
                        ),
                    )),
                }
            }
        })
}

fn api_hf_token_delete(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "token")
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let token_path = cfg.config_dir.join("hf-token");
                if token_path.exists() {
                    let _ = std::fs::remove_file(&token_path);
                }
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "ok": true }),
                )))
            }
        })
}

fn api_hf_download(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::AtomicU64;
    static HF_DOWNLOAD_LAST_START: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "hf" / "download")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let repo_id = body["repo_id"].as_str().unwrap_or("").trim().to_string();
                let file_path = body["file_path"].as_str().unwrap_or("").trim().to_string();
                let target_path: Option<String> =
                    body["target_path"].as_str().map(|s| s.trim().to_string());
                let save_as: Option<String> =
                    body["save_as"].as_str().map(|s| s.trim().to_string());
                let resume: bool = body["resume"].as_bool().unwrap_or(false);
                // Companion downloads (e.g. mmproj alongside a model) bypass the
                // 10-second cooldown so both files can start simultaneously.
                let companion: bool = body["companion"].as_bool().unwrap_or(false);

                if repo_id.is_empty() || file_path.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'repo_id' or 'file_path'"
                        })),
                    ));
                }
                if !validate_hf_repo_id(&repo_id) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Invalid repo_id format. Expected: owner/repo"
                            })),
                            StatusCode::BAD_REQUEST,
                        ),
                    ));
                }

                // Path traversal guard: reject "..", leading "/", leading "\\"
                if file_path.contains("..")
                    || file_path.starts_with('/')
                    || file_path.starts_with("\\")
                {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Invalid file_path: path traversal not allowed"
                        })),
                    ));
                }
                if save_as
                    .as_ref()
                    .is_some_and(|sa| sa.contains("..") || sa.contains('/') || sa.contains('\\'))
                {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Invalid save_as: must be a plain filename"
                        })),
                    ));
                }

                // Determine target directory.
                let models_dir = get_effective_models_dir(&state)
                    .unwrap_or_else(|| cfg.default_models_dir.clone());
                let target_dir = match resolve_hf_target_dir(&models_dir, target_path.as_deref()) {
                    Ok(path) => path,
                    Err(error) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": error
                            })),
                        ));
                    }
                };

                // Cooldown between starts: 10 seconds. Companion downloads (e.g.
                // mmproj alongside a model) are exempt so both can start together.
                // If the previous download failed, allow immediate retry (no cooldown).
                if !companion {
                    let now = std::time::SystemTime::UNIX_EPOCH
                        .elapsed()
                        .unwrap_or_default()
                        .as_secs();

                    // Skip cooldown if the previous download failed (same or different file).
                    use std::sync::atomic::Ordering;
                    let last_failed = crate::model_download::MODEL_DOWNLOAD_LAST_FAILED
                        .load(Ordering::Relaxed);
                    let cooldown_ok = last_failed > 0
                        && now.saturating_sub(last_failed) < 60;

                    if !cooldown_ok {
                        let (dl_ok, _) = try_cooldown(&HF_DOWNLOAD_LAST_START, now, 10);
                        if !dl_ok {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({
                                        "ok": false,
                                        "error": "Too soon; please wait 10 seconds between downloads."
                                    })),
                                    StatusCode::TOO_MANY_REQUESTS,
                                ),
                            ));
                        }
                    }
                }

                let effective_filename = save_as.as_deref().unwrap_or(&file_path);
                let local_path = target_dir
                    .join(effective_filename)
                    .to_string_lossy()
                    .into_owned();
                match crate::hf::hf_start_download(
                    &repo_id,
                    &file_path,
                    save_as.as_deref(),
                    &target_dir,
                    resume,
                ) {
                    Ok(download_id) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "download_id": download_id,
                            "local_path": local_path
                        }))),
                    ),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": e
                        })),
                    )),
                }
            }
        })
}

/// POST /api/hf/mlx-derivatives — discover MLX derivatives for a source repo.
/// Input: repoId (string), revision (string, optional).
/// Output: native_mlx_derivatives, conversion_recipes, original_author_preserved, etc.
fn api_hf_mlx_derivatives(
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "mlx-derivatives")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(
            warp::body::content_length_limit(64 * 1024)
                .and(warp::body::json::<serde_json::Value>()),
        )
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let repo_id = body
                    .get("repoId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                // revision is accepted for future use (pinned source revision tracking)
                let _revision = body
                    .get("revision")
                    .and_then(|v| v.as_str())
                    .unwrap_or("main")
                    .to_string();

                if !crate::hf::validate_hf_repo_id(&repo_id) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "error": "Invalid repoId" })),
                    ));
                }

                match crate::hf::hf_discover_mlx_derivatives(&repo_id).await {
                    Ok(result) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&result),
                    )),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": e })),
                            StatusCode::INTERNAL_SERVER_ERROR,
                        ),
                    )),
                }
            }
        })
}

/// POST /api/hf/qualify — authoritative qualification for a repo+revision.
/// Input: repoId (string), revision (string, optional), backend (string, optional).
/// Output: HfQualification with backend_hint, qualification_state, etc.
fn api_hf_qualify(
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "qualify")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(
            warp::body::content_length_limit(64 * 1024)
                .and(warp::body::json::<crate::hf::QualifyRequest>()),
        )
        .and_then(move |auth: Option<String>, req: crate::hf::QualifyRequest| {
            let cfg = config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                match crate::hf::qualify::hf_qualify_repo(req).await {
                    Ok(result) => {
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&result),
                        ))
                    }
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": e })),
                            StatusCode::INTERNAL_SERVER_ERROR,
                        ),
                    )),
                }
            }
        })
}

/// POST /api/hf/identity — authorship and lineage resolution.
/// Input: repoId (string), revision (string, optional), configDir (string, optional).
/// Output: HfIdentity with original_author, converter_role, roles, etc.
fn api_hf_identity(
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "identity")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(
            warp::body::content_length_limit(64 * 1024)
                .and(warp::body::json::<crate::hf::IdentityRequest>()),
        )
        .and_then(move |auth: Option<String>, req: crate::hf::IdentityRequest| {
            let cfg = config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let explicit_config_dir = req.config_dir.clone();
                let config_dir = explicit_config_dir
                    .as_ref()
                    .map(std::path::Path::new)
                    .unwrap_or_else(|| cfg.config_dir.as_ref());
                match crate::hf::qualify::hf_resolve_identity(req, config_dir).await {
                    Ok(result) => {
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&result),
                        ))
                    }
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": e })),
                            StatusCode::INTERNAL_SERVER_ERROR,
                        ),
                    )),
                }
            }
        })
}

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state.clone();
    let config = ctx.config.clone();

    let mut r = api_hf_search(state.clone(), config.clone())
        .or(api_hf_files(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_hf_community_picks(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_hf_quantizers(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_hf_quantizers_put(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_hf_download_dir(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r.or(api_hf_card(config.clone())).unify().boxed();
    r = r.or(api_hf_meta(config.clone())).unify().boxed();
    r = r.or(api_hf_resolve_origin(config.clone())).unify().boxed();
    r = r.or(api_hf_token_get(config.clone())).unify().boxed();
    r = r.or(api_hf_token_put(config.clone())).unify().boxed();
    r = r.or(api_hf_token_delete(config.clone())).unify().boxed();
    r = r
        .or(api_hf_download(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r.or(api_hf_mlx_derivatives(config.clone())).unify().boxed();
    r = r.or(api_hf_qualify(config.clone())).unify().boxed();
    r = r.or(api_hf_identity(config.clone())).unify().boxed();
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Covers the `format` threading from an `/api/hf/search` request body
    /// through to the `HfModelFormat` passed into `HfSearchParams` (and, via
    /// `HfModelFormat::as_api_filter`, to the outgoing `filter=` HF API query
    /// param) without a live HF call.
    #[test]
    fn parse_hf_format_param_threads_mlx_and_gguf() {
        assert!(matches!(
            parse_hf_format_param("mlx"),
            crate::hf::HfModelFormat::Mlx
        ));
        // Case-insensitive.
        assert!(matches!(
            parse_hf_format_param("MLX"),
            crate::hf::HfModelFormat::Mlx
        ));
        assert!(matches!(
            parse_hf_format_param("gguf"),
            crate::hf::HfModelFormat::Gguf
        ));
        assert!(matches!(
            parse_hf_format_param("both"),
            crate::hf::HfModelFormat::Both
        ));
        assert!(matches!(
            parse_hf_format_param("ALL"),
            crate::hf::HfModelFormat::Both
        ));
        // Unrecognized / absent format falls back to Gguf (backward compat).
        assert!(matches!(
            parse_hf_format_param(""),
            crate::hf::HfModelFormat::Gguf
        ));
        assert!(matches!(
            parse_hf_format_param("bogus"),
            crate::hf::HfModelFormat::Gguf
        ));
    }
}
