use std::path::PathBuf;
use std::sync::Arc;

use warp::Filter;

use crate::config::AppConfig;
use crate::state::AppState;

use super::common::{
    ApiCtx, ApiRoute, check_api_token, check_db_admin_token, unauthorized_api_token,
};
use crate::llama::vram_estimator::gguf_arch_to_heuristic_name;

/// Returns the user-configured models directory, or None if not set.
pub(crate) fn get_effective_models_dir(state: &AppState) -> Option<PathBuf> {
    if let Some(ref d) = state.models_dir {
        return Some(d.clone());
    }
    let s = state.ui_settings.lock().unwrap();
    if !s.models_dir.is_empty() {
        return Some(PathBuf::from(&s.models_dir));
    }
    None
}

// ── POST /api/models/download/start ──────────────────────────────────────────

fn api_models_download_start(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "download" / "start")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_db_admin_token(&auth, &cfg) && !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model = body["model"].as_str().unwrap_or("").to_string();
                let source = body["source"].as_str().unwrap_or("").to_string();

                if model.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'model' field"
                        })),
                    ));
                }

                if source != "hf" {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Unsupported source '{}'; only 'hf' is currently supported", source)
                        })),
                    ));
                }

                let (repo_id, file_path) = if model.contains('/') {
                    let parts: Vec<&str> = model.splitn(2, '/').collect();
                    (parts[0].to_string(), parts.get(1).unwrap_or(&"").to_string())
                } else {
                    (model.clone(), "model.gguf".to_string())
                };

                let target_dir = get_effective_models_dir(&state)
                    .unwrap_or_else(|| cfg.default_models_dir.clone());

                let hf_token = crate::hf::hf_load_token();

                match crate::model_download::start_download(
                    &repo_id,
                    &file_path,
                    None,
                    &target_dir,
                    hf_token,
                ) {
                    Ok(download_id) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "download_id": download_id
                        }))),
                    ),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to start download: {}", e)
                        })),
                    )),
                }
            }
        })
}

fn api_models_download_status(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "download" / String / "status")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |id: String, auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                match crate::model_download::get_download_status(&id) {
                    Some(status) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "status": status
                        })),
                    )),
                    None => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Download not found"
                            })),
                            warp::http::StatusCode::NOT_FOUND,
                        ),
                    )),
                }
            }
        })
}

fn api_models_download_cancel(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "download" / String / "cancel")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |id: String, auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let ok = crate::model_download::cancel_download(&id);
                if ok {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({ "ok": true }),
                    )))
                } else {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "ok": false,
                            "error": "Download not found or already finished"
                        }),
                    )))
                }
            }
        })
}

// ── P3.2: Third-Party Models ──────────────────────────────────────────────────

fn api_third_party_models(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "third-party-models")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let _ = body["include_subdirs"].as_bool().unwrap_or(true);

                let extra_dirs = state
                    .ui_settings
                    .lock()
                    .map(|s| s.extra_models_dirs.clone())
                    .unwrap_or_default();
                let models = crate::llama::spawn_wizard::scan_third_party_models(&extra_dirs);
                let models: Vec<serde_json::Value> = models
                    .into_iter()
                    .map(|m| {
                        serde_json::json!({
                            "path": m.path,
                            "name": m.name,
                            "source_tool": m.source_tool,
                            "size": m.size,
                        })
                    })
                    .collect();

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "models": models
                    }),
                )))
            }
        })
}

// ── P3.3: Model Introspection ──────────────────────────────────────────────

fn api_model_introspect(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "model" / "introspect")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model_path = body["model_path"].as_str().unwrap_or("").trim().to_string();
                if model_path.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'model_path' field"
                        })),
                    ));
                }

                // Security: only allow .gguf files or Ollama content-addressed blobs
                let is_gguf_ext = model_path.to_ascii_lowercase().ends_with(".gguf");
                let is_ollama_blob = model_path.contains("/blobs/sha256-")
                    || model_path.contains("\\blobs\\sha256-");
                if !is_gguf_ext && !is_ollama_blob {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "model_path must point to a .gguf file"
                        })),
                    ));
                }

                let canon = match std::path::Path::new(&model_path).canonicalize() {
                    Ok(p) => p,
                    Err(_) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Model file not found"
                            })),
                        ));
                    }
                };
                let models_dir = get_effective_models_dir(&state)
                    .unwrap_or_else(|| cfg.default_models_dir.clone());
                let in_models_dir = models_dir
                    .canonicalize()
                    .map(|d| canon.starts_with(&d))
                    .unwrap_or(false);
                let in_home = dirs::home_dir()
                    .and_then(|h| h.canonicalize().ok())
                    .map(|h| canon.starts_with(&h))
                    .unwrap_or(false);
                let in_extra = state
                    .ui_settings
                    .lock()
                    .map(|s| {
                        s.extra_models_dirs.iter().any(|d| {
                            std::path::Path::new(d)
                                .canonicalize()
                                .map(|cd| canon.starts_with(&cd))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);
                if !in_models_dir && !in_home && !in_extra {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "model_path is outside allowed directories"
                        })),
                    ));
                }

                let llama_server_path = cfg.llama_server_path.clone();
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    crate::llama::spawn_wizard::introspect_model(
                        &model_path,
                        llama_server_path.to_string_lossy().as_ref(),
                    ),
                )
                .await;

                let metadata = match result {
                    Ok(Ok(meta)) => meta,
                    Ok(Err(e)) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": e
                            })),
                        ));
                    }
                    Err(_) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Introspection timed out (30s)"
                            })),
                        ));
                    }
                };

                let file_size_bytes = std::fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0);

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "metadata": metadata,
                        "cached": metadata.cached,
                        "file_size_bytes": file_size_bytes
                    }),
                )))
            }
        })
}

// ── POST /api/models/gguf-meta ────────────────────────────────────────────────

fn api_models_gguf_meta(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "gguf-meta")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model_path = body["model_path"].as_str().unwrap_or("").trim().to_string();
                if model_path.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'model_path' field"
                        })),
                    ));
                }

                let meta = match crate::llama::gguf_meta::read_gguf_metadata(std::path::Path::new(
                    &model_path,
                )) {
                    Ok(m) => m,
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed to read GGUF metadata: {}", e)
                            })),
                        ));
                    }
                };

                // Number of full-attention (KV-bearing) layers. Prefer the real GGUF
                // value (block_count / full_attention_interval); fall back to the name
                // heuristic only for older GGUFs that don't record the interval.
                let n_attn_layers = meta.n_attn_layers().or_else(|| {
                    let arch_str = meta.architecture.as_ref()?;
                    let heuristic_name = gguf_arch_to_heuristic_name(arch_str);
                    let resolved = if arch_str.eq_ignore_ascii_case("qwen35") {
                        match meta.block_count {
                            Some(bc) if bc >= 75 => "qwen3_5".to_string(),
                            _ => heuristic_name,
                        }
                    } else {
                        heuristic_name
                    };
                    let param_b = meta.param_count.map(|p| p as f64 / 1e9).unwrap_or(0.0);
                    let arch = crate::llama::vram_estimator::ModelArch::from_name_and_params(
                        &resolved, param_b,
                    );
                    (arch.n_attn_layers < arch.n_layers).then_some(arch.n_attn_layers)
                });

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "architecture": meta.architecture,
                        "param_count": meta.param_count,
                        "block_count": meta.block_count,
                        "head_count": meta.head_count,
                        "head_count_kv": meta.head_count_kv,
                        "key_length": meta.key_length,
                        "key_length_swa": meta.key_length_swa,
                        "context_length": meta.context_length,
                        "embedding_length": meta.embedding_length,
                        "feed_forward_length": meta.feed_forward_length,
                        "expert_count": meta.expert_count,
                        "expert_used_count": meta.expert_used_count,
                        "mtp_depth": meta.mtp_depth,
                        "n_attn_layers": n_attn_layers,
                        "full_attention_interval": meta.full_attention_interval,
                        "linear_attn_state_bytes": meta.linear_attn_state_bytes(),
                        "sliding_window": meta.sliding_window,
                        "n_global_attn_layers": meta.n_global_attn_layers,
                        "global_kv_heads": meta.global_kv_heads,
                        "local_kv_heads": meta.local_kv_heads,
                    }),
                )))
            }
        })
}

// ── GET /api/models ───────────────────────────────────────────────────────────

fn api_get_models(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let models = state.discovered_models.lock().unwrap().clone();
            let tags = state.model_tags.lock().unwrap().tags.clone();
            let models_with_tags: Vec<serde_json::Value> = models
                .into_iter()
                .map(|m| {
                    let model_path = m.path.to_string_lossy().to_string();
                    let cls = crate::models::classify_model(&m);
                    let mut obj = serde_json::to_value(m).unwrap_or_default();
                    if let Some(model_obj) = obj.as_object_mut() {
                        let model_tags = tags.get(&model_path).cloned().unwrap_or_default();
                        model_obj.insert("tags".into(), serde_json::json!(model_tags));
                        model_obj.insert("classification".into(), serde_json::json!(cls));
                    }
                    obj
                })
                .collect();
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&models_with_tags)),
            ))
        })
}

fn api_refresh_models(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "refresh")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            if let Some(ref dir) = state.models_dir {
                match crate::models::scan_models_dir(dir) {
                    Ok(discovered) => {
                        let count = discovered.len();
                        *state.discovered_models.lock().unwrap() = discovered;
                        futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({"ok": true, "count": count}))),
                        ))
                    }
                    Err(e) => {
                        futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()}))),
                        ))
                    }
                }
            } else {
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(&serde_json::json!({"ok": false, "error": "no models directory configured (use --models-dir)"}))),
                ))
            }
        })
}

fn api_delete_model_file(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "file")
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let st = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let path_str = match body.get("path").and_then(|v| v.as_str()) {
                    Some(p) => p.to_string(),
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "missing path"}),
                            ),
                        ));
                    }
                };

                if !path_str.to_lowercase().ends_with(".gguf") {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "only .gguf files can be deleted"}),
                        ),
                    ));
                }

                let path = std::path::Path::new(&path_str);
                // Containment check: must be inside allowed model directories.
                let models_dir = get_effective_models_dir(&st)
                    .unwrap_or_else(|| cfg.default_models_dir.clone());
                let canon = match path.canonicalize() {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": format!("invalid path: {e}")}),
                            ),
                        ));
                    }
                };
                let in_models_dir = models_dir
                    .canonicalize()
                    .map(|d| canon.starts_with(&d))
                    .unwrap_or(false);
                let in_home = dirs::home_dir()
                    .and_then(|h| h.canonicalize().ok())
                    .map(|h| canon.starts_with(&h))
                    .unwrap_or(false);
                let in_extra = st
                    .ui_settings
                    .lock()
                    .map(|s| {
                        s.extra_models_dirs.iter().any(|d| {
                            std::path::Path::new(d)
                                .canonicalize()
                                .map(|cd| canon.starts_with(&cd))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);

                if !in_models_dir && !in_home && !in_extra {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "file is outside allowed model directories"}),
                        ),
                    ));
                }

                match std::fs::remove_file(&canon) {
                    Ok(_) => {
                        let mut models = st.discovered_models.lock().unwrap();
                        models.retain(|m| m.path.to_str() != Some(&path_str));
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"ok": true})),
                        ))
                    }
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": format!("Failed to delete: {e}")}),
                        ),
                    )),
                }
            }
        })
}

fn api_get_model_tags(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "tags")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let tags = state.model_tags.lock().unwrap().clone();
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&tags)),
            ))
        })
}

fn api_put_model_tags(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "tags")
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let st = state.clone();
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model_path = match body.get("model_path").and_then(|v| v.as_str()) {
                    Some(p) => p.to_string(),
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "missing model_path"}),
                            ),
                        ));
                    }
                };

                let new_tags = match body.get("tags") {
                    Some(t) => match t.as_array() {
                        Some(arr) => arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect::<Vec<String>>(),
                        None => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({"ok": false, "error": "tags must be an array of strings"})),
                            ));
                        }
                    },
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "missing tags"}),
                            ),
                        ));
                    }
                };

                let mut tags = st.model_tags.lock().unwrap();
                tags.tags.insert(model_path, new_tags);
                let tags_path = st.model_tags_path.clone();
                if let Err(e) = tags.save(&tags_path) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to save model tags: {e}")
                        })),
                    ));
                }

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({"ok": true}),
                )))
            }
        })
}

fn api_get_collections(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "collections")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let collections = state.preset_collections.lock().unwrap().clone();
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&collections)),
            ))
        })
}

fn api_create_collection(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "collections")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let st = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let name = match body.get("name").and_then(|v| v.as_str()) {
                    Some(n) if !n.is_empty() => n.to_string(),
                    _ => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "missing or empty name"}),
                            ),
                        ));
                    }
                };
                let description = body
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let preset_ids = body
                    .get("preset_ids")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let config_dir = st
                    .model_tags_path
                    .parent()
                    .unwrap_or(&std::path::PathBuf::from("."))
                    .to_path_buf();
                let mut collections = st.preset_collections.lock().unwrap();
                let id = crate::collections::unique_id("coll", &name, &collections.collections);
                let new = crate::collections::PresetCollection {
                    id,
                    name,
                    description,
                    preset_ids,
                };
                collections.collections.push(new.clone());
                if let Err(e) = crate::collections::save_collections(&config_dir, &collections) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to save collections: {e}")
                        })),
                    ));
                }
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({"ok": true, "collection": new}),
                )))
            }
        })
}

fn api_patch_collection(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "collections" / String)
        .and(warp::patch())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(
            move |id: String, auth: Option<String>, body: serde_json::Value| {
                let cfg = app_config.clone();
                let st = state.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    let config_dir = st
                        .model_tags_path
                        .parent()
                        .unwrap_or(&std::path::PathBuf::from("."))
                        .to_path_buf();
                    let mut collections = st.preset_collections.lock().unwrap();
                    let col = match collections.collections.iter_mut().find(|c| c.id == id) {
                        Some(c) => c,
                        None => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "collection not found"
                                })),
                            ));
                        }
                    };
                    if let Some(name) = body.get("name").and_then(|v| v.as_str()) {
                        col.name = name.to_string();
                    }
                    if let Some(desc) = body.get("description") {
                        col.description = if desc.is_null() {
                            None
                        } else {
                            desc.as_str().map(|s| s.to_string())
                        };
                    }
                    if let Some(ids) = body.get("preset_ids").and_then(|v| v.as_array()) {
                        col.preset_ids = ids
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                    }
                    if let Err(e) = crate::collections::save_collections(&config_dir, &collections)
                    {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed to save collections: {e}")
                            })),
                        ));
                    }
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": true}),
                    )))
                }
            },
        )
}

fn api_delete_collection(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "collections" / String)
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |id: String, auth: Option<String>| {
            let cfg = app_config.clone();
            let st = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let config_dir = st
                    .model_tags_path
                    .parent()
                    .unwrap_or(&std::path::PathBuf::from("."))
                    .to_path_buf();
                let mut collections = st.preset_collections.lock().unwrap();
                let before = collections.collections.len();
                collections.collections.retain(|c| c.id != id);
                if collections.collections.len() == before {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "collection not found"
                        })),
                    ));
                }
                if let Err(e) = crate::collections::save_collections(&config_dir, &collections) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to save collections: {e}")
                        })),
                    ));
                }
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({"ok": true}),
                )))
            }
        })
}

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state.clone();
    let config = ctx.config.clone();

    let mut r = api_models_download_start(state.clone(), config.clone())
        .or(api_models_download_status(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_models_download_cancel(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_third_party_models(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_model_introspect(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r.or(api_models_gguf_meta(config.clone())).unify().boxed();
    r = r
        .or(api_get_models(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_refresh_models(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_delete_model_file(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_get_model_tags(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_put_model_tags(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_get_collections(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_create_collection(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_patch_collection(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_delete_collection(state.clone(), config.clone()))
        .unify()
        .boxed();
    r
}
