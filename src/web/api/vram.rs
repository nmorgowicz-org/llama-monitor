use std::sync::Arc;

use warp::Filter;

use crate::config::AppConfig;
use crate::state::AppState;

use super::common::{
    ApiCtx, ApiRoute, check_api_token, check_db_admin_token, unauthorized_api_token,
    unauthorized_db_admin_token,
};

// 7) POST /api/vram-estimate (architecture-aware breakdown)
fn api_vram_estimate_breakdown(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "vram-estimate")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model_path = body["model_path"].as_str().unwrap_or("").to_string();
                let n_ctx = body["n_ctx"].as_u64().unwrap_or(4096);
                let _gpu_layers = body["gpu_layers"].as_i64().unwrap_or(-1);
                let parallel_slots = body["parallel_slots"].as_u64().unwrap_or(1) as u32;
                let ubatch_size = body["ubatch_size"].as_u64().unwrap_or(2048) as u32;
                let ctk = body["ctk"].as_str().unwrap_or("q8_0").to_string();
                let ctv = body["ctv"].as_str().unwrap_or("q8_0").to_string();
                let n_cpu_moe = body["n_cpu_moe"].as_i64().map(|v| v as i32).unwrap_or(0);
                let available_vram_bytes = body["available_vram_bytes"].as_u64().unwrap_or(0);
                let is_unified_memory = body["is_unified_memory"].as_bool().unwrap_or(false);
                // mmproj_path: path to the vision projector GGUF; size read from disk.
                // mmproj_bytes: explicit size override (used when path is unavailable).
                let mmproj_path = body["mmproj_path"].as_str().unwrap_or("").to_string();
                let mmproj_bytes_override = body["mmproj_bytes"].as_u64();
                // HuggingFace coordinates for pre-download introspection: when there is no
                // local file yet, the GGUF KV header is range-fetched so the estimate uses the
                // model's real architecture instead of name-based guesses.
                let hf_repo_id = body["hf_repo_id"].as_str().unwrap_or("").to_string();
                let hf_file_path = body["hf_file_path"].as_str().unwrap_or("").to_string();
                let model_size_override = body["model_size_bytes"].as_u64();

                // Resolve (model_size_bytes, arch) from a local file (preferred) or, failing
                // that, by range-fetching the GGUF header straight from HuggingFace.
                let (model_size_bytes, mut arch) = if !model_path.is_empty() {
                    let size = match std::fs::metadata(&model_path) {
                        Ok(m) => m.len(),
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": format!("Cannot stat model file: {e}")
                                })),
                            ));
                        }
                    };
                    let arch = match crate::llama::gguf_meta::read_gguf_metadata(
                        std::path::Path::new(&model_path),
                    ) {
                        Ok(meta) => meta
                            .to_model_metadata()
                            .to_arch(&model_path, meta.param_b().unwrap_or(0.0)),
                        Err(_) => crate::llama::vram_estimator::ModelArch::from_name_and_params(
                            &model_path,
                            crate::llama::vram_estimator::estimate_param_b_from_size(size, 4.85),
                        ),
                    };
                    (size, arch)
                } else if !hf_repo_id.is_empty() && !hf_file_path.is_empty() {
                    // Size must be supplied by the caller (from the HF file listing).
                    let size = model_size_override.unwrap_or(0);
                    if size == 0 {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "model_size_bytes is required when introspecting a HuggingFace model"
                            })),
                        ));
                    }
                    let arch =
                        match crate::hf::fetch_gguf_header_metadata(&hf_repo_id, &hf_file_path).await
                        {
                            Ok(meta) => meta
                                .to_model_metadata()
                                .to_arch(&hf_file_path, meta.param_b().unwrap_or(0.0)),
                            // Range-fetch failed (offline / gated / no range support): fall back
                            // to the name heuristic so the caller still gets a (rougher) estimate.
                            Err(_) => crate::llama::vram_estimator::ModelArch::from_name_and_params(
                                &hf_file_path,
                                crate::llama::vram_estimator::estimate_param_b_from_size(size, 4.85),
                            ),
                        };
                    (size, arch)
                } else {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "model_path, or hf_repo_id + hf_file_path, is required"
                        })),
                    ));
                };

                // Override mmproj_bytes from explicit path or body field.
                if let Some(explicit) = mmproj_bytes_override {
                    arch.mmproj_bytes = explicit;
                } else if !mmproj_path.is_empty() {
                    arch.mmproj_bytes = std::fs::metadata(&mmproj_path).map(|m| m.len()).unwrap_or(0);
                }

                let breakdown = crate::llama::vram_estimator::full_estimate(
                    model_size_bytes,
                    &arch,
                    n_ctx,
                    &ctk,
                    &ctv,
                    parallel_slots,
                    ubatch_size,
                    n_cpu_moe,
                    available_vram_bytes,
                    is_unified_memory,
                );

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "weights_bytes": breakdown.weights_bytes,
                        "kv_cache_bytes": breakdown.kv_cache_bytes,
                        "linear_attn_state_bytes": breakdown.linear_attn_state_bytes,
                        "mmproj_bytes": breakdown.mmproj_bytes,
                        "mtp_bytes": breakdown.mtp_bytes,
                        "overhead_bytes": breakdown.overhead_bytes,
                        "total_bytes": breakdown.total_bytes,
                        "available_bytes": breakdown.available_bytes,
                        "headroom_bytes": breakdown.headroom_bytes,
                        "ram_bytes": breakdown.ram_bytes,
                        "recommendation": serde_json::to_value(&breakdown.recommendation).unwrap_or(serde_json::Value::Null),
                        "note": breakdown.note
                    }))),
                )
            }
        })
}

// 4b) POST /api/vram/estimate (legacy)
fn api_vram_estimate(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "vram" / "estimate")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                // model: local path used to determine file size (optional when
                // model_size_bytes is provided explicitly).
                let model = body["model"].as_str().unwrap_or("").to_string();
                let context_length = body["context_length"].as_u64().unwrap_or(4096);
                // n_cpu_moe: number of transformer layers whose expert tensors stay
                // on CPU (0 = all expert tensors on GPU).
                let n_cpu_moe = body["n_cpu_moe"].as_i64().map(|v| v as i32);

                // model_size_bytes can be supplied explicitly (e.g. for HF models where
                // there is no local file yet), otherwise inferred from the filesystem.
                let model_size_bytes = body["model_size_bytes"].as_u64().unwrap_or_else(|| {
                    if model.is_empty() {
                        0
                    } else {
                        std::fs::metadata(&model).map(|m| m.len()).unwrap_or(0)
                    }
                });

                if model_size_bytes == 0 {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Could not determine model size. Provide a local model path or set 'model_size_bytes' explicitly."
                        }))),
                    );
                }

                let kv_quant = body["kv_quant"].as_str().unwrap_or("q8_0").to_string();
                let batch_size = body["batch_size"].as_u64().unwrap_or(2048) as u32;
                let ubatch_size = body["ubatch_size"].as_u64().unwrap_or(2048) as u32;
                let speculative_decoding = body["speculative_decoding"].as_bool().unwrap_or(false);
                let mmproj_size_bytes = body["mmproj_size_bytes"].as_u64().unwrap_or(0);
                let available_vram_bytes = body["available_vram_bytes"].as_u64().unwrap_or(0);

                let estimate = crate::llama::vram_estimator::estimate_vram(
                    model_size_bytes,
                    context_length,
                    &kv_quant,
                    batch_size,
                    ubatch_size,
                    speculative_decoding,
                    mmproj_size_bytes,
                    n_cpu_moe,
                    available_vram_bytes,
                );

                let estimated_vram_mb =
                    (estimate.estimated_vram_bytes as f64) / (1024.0 * 1024.0);

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "estimated_vram_mb": estimated_vram_mb,
                        "estimated_vram_bytes": estimate.estimated_vram_bytes,
                        "estimated_ram_bytes": estimate.estimated_ram_bytes,
                        "available_vram_bytes": estimate.available_vram_bytes,
                        "recommendation": serde_json::to_value(&estimate.recommendation).unwrap_or(serde_json::Value::Null),
                        "note": estimate.note
                    }))),
                )
            }
        })
}

// ── POST /api/vram/quant-compare ─────────────────────────────────────────────
fn api_vram_quant_compare(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "vram" / "quant-compare")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let param_b = body["param_b"].as_f64().unwrap_or(0.0);
                if param_b <= 0.0 {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "param_b must be a positive number (model parameter count in billions)"
                        })),
                    ));
                }

                let model_name = body["model_name"].as_str().unwrap_or("").to_string();
                let available_vram_bytes = body["available_vram_bytes"].as_u64().unwrap_or(0);
                let parallel_slots = body["parallel_slots"].as_u64().unwrap_or(1) as u32;
                let is_unified_memory = body["is_unified_memory"].as_bool().unwrap_or(false);

                let use_case = match body["use_case"].as_str().unwrap_or("general") {
                    "agentic" => crate::llama::vram_estimator::UseCase::Agentic,
                    "roleplay" => crate::llama::vram_estimator::UseCase::Roleplay,
                    _ => crate::llama::vram_estimator::UseCase::General,
                };

                // Optionally accept explicit arch fields to improve accuracy when
                // called after introspection.
                let arch = build_arch_from_body(&body, &model_name, param_b);

                let table = crate::llama::vram_estimator::quant_comparison_table(
                    param_b,
                    &arch,
                    &model_name,
                    available_vram_bytes,
                    use_case,
                    parallel_slots,
                    is_unified_memory,
                );

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "ok": true, "quants": table }),
                )))
            }
        })
}

// ── POST /api/vram/auto-size ──────────────────────────────────────────────────
fn api_vram_auto_size(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "vram" / "auto-size")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model_name = body["model_name"].as_str().unwrap_or("").to_string();
                let param_b = body["param_b"].as_f64().unwrap_or(0.0);
                let available_vram_bytes = body["available_vram_bytes"].as_u64().unwrap_or(0);
                let parallel_slots = body["parallel_slots"].as_u64().unwrap_or(1).max(1) as u32;
                let fit_granularity = body["fit_granularity"].as_u64().unwrap_or(1024).max(512);
                let is_unified_memory = body["is_unified_memory"].as_bool().unwrap_or(false);

                let use_case = match body["use_case"].as_str().unwrap_or("general") {
                    "agentic" => crate::llama::vram_estimator::UseCase::Agentic,
                    "roleplay" => crate::llama::vram_estimator::UseCase::Roleplay,
                    _ => crate::llama::vram_estimator::UseCase::General,
                };

                // Model size: explicit bytes > local file stat > param_b heuristic
                let model_size_bytes = body["model_size_bytes"].as_u64().unwrap_or_else(|| {
                    let path = body["model_path"].as_str().unwrap_or("");
                    if !path.is_empty() {
                        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
                    } else {
                        0
                    }
                });

                // We need *some* size info
                if model_size_bytes == 0 && param_b <= 0.0 {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Provide model_size_bytes, model_path, or param_b"
                        })),
                    ));
                }

                // Read GGUF metadata to get authoritative architecture family
                // (e.g. "qwen35" even if the filename says "Pantheon-Reasoning-27B")
                let gguf_read = body["model_path"].as_str().and_then(|path_str| {
                    let path = std::path::Path::new(path_str);
                    path.exists()
                        .then(|| crate::llama::gguf_meta::read_gguf_metadata(path))
                        .transpose()
                        .ok()
                        .flatten()
                });

                // Resolve gguf_arch: prefer GGUF file's general.architecture,
                // then fall back to body field, then empty string.
                // "qwen35" is shared by Qwen3.5 and Qwen3.6 — we distinguish via block_count.
                let (gguf_arch, gguf_block_count, gguf_context_length) = match &gguf_read {
                    Some(meta) => {
                        let arch = meta
                            .architecture
                            .as_deref()
                            .unwrap_or(body["gguf_arch"].as_str().unwrap_or(""))
                            .to_string();
                        let bc = meta.block_count;
                        let ctx = meta.context_length;
                        (arch, bc, ctx)
                    }
                    None => (
                        body["gguf_arch"].as_str().unwrap_or("").to_string(),
                        None,
                        None,
                    ),
                };

                // Map qwen35 to the correct heuristic name using block_count:
                // Qwen3.6 family: ~64 layers (some GGUFs report 65 from extra
                // embedding layers). Qwen3.5 family: 96 layers.
                // Threshold at 75: anything below = Qwen3.6, above = Qwen3.5.
                let resolved_arch = if gguf_arch == "qwen35" {
                    match gguf_block_count {
                        Some(bc) if bc >= 75 => "qwen3_5".to_string(),
                        _ => "qwen3_6".to_string(),
                    }
                } else {
                    gguf_arch.clone()
                };

                // Inject resolved arch into body so build_arch_from_body can use it
                let mut enriched_body = body.clone();
                enriched_body["gguf_arch"] = serde_json::json!(resolved_arch);

                // Also cap auto-size at the model's training context length
                let context_cap = gguf_context_length.map(|c| c as u64);

                // When the GGUF file is present, build the arch straight from its real
                // metadata (full_attention_interval, ssm.*, per-layer head_count_kv,
                // sliding_window, …) — the authoritative source. Only fall back to the
                // body/name heuristic for the pre-download advisor where no file exists.
                let arch = match &gguf_read {
                    Some(meta) => meta.to_model_metadata().to_arch(&model_name, param_b),
                    None => build_arch_from_body(&enriched_body, &model_name, param_b),
                };

                // If model_size_bytes is not given, estimate from param_b + quant
                let quant_hint = body["quant"].as_str().unwrap_or("q4_k_m");
                let model_bytes = if model_size_bytes > 0 {
                    model_size_bytes
                } else {
                    crate::llama::vram_estimator::estimate_model_size_bytes(param_b, quant_hint)
                };

                let result = crate::llama::vram_estimator::auto_size(
                    model_bytes,
                    &arch,
                    available_vram_bytes,
                    use_case,
                    parallel_slots,
                    fit_granularity,
                    is_unified_memory,
                    context_cap, // n_ctx_train cap from GGUF metadata
                );

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "ok": true, "result": result }),
                )))
            }
        })
}

use crate::llama::vram_estimator::gguf_arch_to_heuristic_name;

/// Build a `ModelArch` from a JSON request body, falling back to heuristics
/// when introspection fields are absent.
///
/// When `gguf_arch` is present in the body, it is used as the authoritative
/// source for the heuristic name instead of the filename (which can be misleading
/// for renamed finetunes like "Qwopus3.6").
pub(crate) fn build_arch_from_body(
    body: &serde_json::Value,
    model_name: &str,
    param_b: f64,
) -> crate::llama::vram_estimator::ModelArch {
    // Use the original model name (with size/MoE hints like "35B-A3B") as the
    // primary heuristic source so MoE and scale are recognized.
    let mut heuristic =
        crate::llama::vram_estimator::ModelArch::from_name_and_params(model_name, param_b);

    // If that yielded a weak/default-looking arch (no MoE, no hybrid, no sliding window),
    // and we have a known GGUF architecture string, fall back to deriving from that.
    let heuristic_name = body["gguf_arch"]
        .as_str()
        .map(gguf_arch_to_heuristic_name)
        .unwrap_or_else(|| model_name.to_string());

    if heuristic.n_experts == 0
        && !heuristic.is_hybrid_attn()
        && !heuristic.has_local_attn()
        && !heuristic.is_moe()
    {
        heuristic =
            crate::llama::vram_estimator::ModelArch::from_name_and_params(&heuristic_name, param_b);
    }

    let n_layers = body["n_layers"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.n_layers);
    let n_kv_heads = body["n_kv_heads"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.n_kv_heads);
    let head_dim = body["head_dim"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.head_dim);
    let global_head_dim = body["global_head_dim"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.global_head_dim);
    let n_experts = body["n_experts"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.n_experts);
    let n_exp_used = body["n_experts_used"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.n_experts_used);
    let mtp_depth = body["mtp_depth"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.mtp_depth);
    let mmproj_bytes = body["mmproj_bytes"]
        .as_u64()
        .unwrap_or(heuristic.mmproj_bytes);
    let expert_frac = body["expert_fraction"]
        .as_f64()
        .unwrap_or(heuristic.expert_fraction);

    // Hybrid DeltaNet: override from body if provided, otherwise preserve heuristic
    let n_attn_layers = body["n_attn_layers"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.n_attn_layers);
    let linear_attn_state_bytes = body["linear_attn_state_bytes"]
        .as_u64()
        .unwrap_or(heuristic.linear_attn_state_bytes);
    // Sliding-window (Gemma): override from body if provided
    let n_global_attn_layers = body["n_global_attn_layers"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.n_global_attn_layers);
    let local_attn_window = body["local_attn_window"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.local_attn_window);
    let local_kv_heads = body["local_kv_heads"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.local_kv_heads);

    crate::llama::vram_estimator::ModelArch {
        n_layers,
        n_kv_heads,
        head_dim,
        n_global_attn_layers,
        local_attn_window,
        local_kv_heads,
        n_attn_layers,
        linear_attn_state_bytes,
        n_experts,
        n_experts_used: n_exp_used,
        expert_fraction: expert_frac,
        global_head_dim,
        mtp_depth,
        mmproj_bytes,
        // n_embd comes from GGUF or heuristic; body can override via "n_embd" field
        n_embd: body["n_embd"]
            .as_u64()
            .map(|v| v as u32)
            .unwrap_or(heuristic.n_embd),
        param_b,
    }
}

// ── Apple Silicon: set Metal GPU wired memory limit ───────────────────────────
// Uses osascript to invoke `sysctl iogpu.wired_limit_mb=N` with administrator
// privileges via the macOS native password dialog. No password touches the app.
// Only compiled on macOS; on other platforms returns a not-supported error.

#[cfg(target_os = "macos")]
fn api_set_metal_gpu_limit(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "system" / "set-metal-gpu-limit")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                // Use db-admin-token: this changes a system-level parameter (iogpu.wired_limit_mb).
                if !check_db_admin_token(&auth, &cfg) {
                    return Ok(unauthorized_db_admin_token());
                }

                let limit_mb = match body["limit_mb"].as_u64() {
                    Some(v) if v > 0 => v,
                    _ => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "limit_mb must be a positive integer (MiB)"
                            })),
                        ));
                    }
                };

                // Single-line osascript command (AppleScript string literals cannot span
                // newlines). Use full binary paths so the restricted do-shell-script PATH
                // (/usr/bin:/bin:/usr/sbin:/sbin) is never an issue.
                // Logic: apply sysctl immediately, then upsert the line in /etc/sysctl.conf
                // for persistence across reboots. Subshell grouping avoids if/then/fi.
                let manual_cmd = format!(
                    "sudo /usr/sbin/sysctl -w iogpu.wired_limit_mb={n} && grep -q '^iogpu.wired_limit_mb=' /etc/sysctl.conf 2>/dev/null && sudo /usr/bin/sed -i '' 's/iogpu.wired_limit_mb=.*/iogpu.wired_limit_mb={n}/' /etc/sysctl.conf || echo 'iogpu.wired_limit_mb={n}' | sudo /usr/bin/tee -a /etc/sysctl.conf",
                    n = limit_mb
                );
                let shell_cmd = format!(
                    "/usr/sbin/sysctl iogpu.wired_limit_mb={n} && (/usr/bin/grep -q '^iogpu.wired_limit_mb=' /etc/sysctl.conf 2>/dev/null && /usr/bin/sed -i '' 's/iogpu.wired_limit_mb=.*/iogpu.wired_limit_mb={n}/' /etc/sysctl.conf || /bin/echo 'iogpu.wired_limit_mb={n}' >> /etc/sysctl.conf)",
                    n = limit_mb
                );
                let script = format!(
                    "do shell script \"{cmd}\" with administrator privileges",
                    cmd = shell_cmd.replace('"', "\\\"")
                );

                let run_result = tokio::task::spawn_blocking(move || {
                    std::process::Command::new("/usr/bin/osascript")
                        .args(["-e", &script])
                        .output()
                })
                .await;

                let reply = match run_result {
                    Ok(Ok(output)) if output.status.success() => {
                        let actual = crate::gpu::apple::read_iogpu_wired_limit_mb();
                        if actual >= limit_mb {
                            serde_json::json!({
                                "ok": true,
                                "limit_mb": actual,
                                "note": "Applied immediately and saved to /etc/sysctl.conf — will persist across reboots."
                            })
                        } else {
                            // osascript exited 0 but sysctl read-back shows no change.
                            // Most likely the server PATH can't find sysctl or the
                            // kernel parameter name differs on this macOS version.
                            serde_json::json!({
                                "ok": false,
                                "error": format!(
                                    "osascript exited 0 but iogpu.wired_limit_mb read back as {} MB (expected {}). The setting may not have applied.",
                                    actual, limit_mb
                                ),
                                "manual_cmd": manual_cmd
                            })
                        }
                    }
                    Ok(Ok(output)) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let combined = format!("{}{}", stdout.trim(), stderr.trim());
                        let msg = if combined.contains("User canceled")
                            || combined.contains("cancelled")
                            || combined.contains("(-128)")
                        {
                            "Cancelled — password dialog was dismissed.".to_string()
                        } else {
                            format!("osascript failed: {combined}")
                        };
                        serde_json::json!({ "ok": false, "error": msg, "manual_cmd": manual_cmd })
                    }
                    Ok(Err(e)) => {
                        serde_json::json!({ "ok": false, "error": format!("Failed to launch osascript: {e}"), "manual_cmd": manual_cmd })
                    }
                    Err(e) => {
                        serde_json::json!({ "ok": false, "error": format!("Task error: {e}"), "manual_cmd": manual_cmd })
                    }
                };

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                    warp::reply::json(&reply),
                ))
            }
        })
}

fn api_get_system_info(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "system" / "info")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let metrics = state.system_metrics.lock().unwrap().clone();
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "p_cores": metrics.p_cores,
                        "e_cores": metrics.e_cores,
                        "cpu_name": metrics.cpu_name,
                    }),
                )))
            }
        })
}

#[cfg(target_os = "macos")]
fn api_get_metal_gpu_limit(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "system" / "metal-gpu-limit")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let limit_mb = crate::gpu::apple::read_iogpu_wired_limit_mb();
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "limit_mb": limit_mb,
                        "custom": limit_mb > 0,
                    }),
                )))
            }
        })
}

#[cfg(not(target_os = "macos"))]
fn api_set_metal_gpu_limit(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "system" / "set-metal-gpu-limit")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, _body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                // Use db-admin-token: this changes a system-level parameter.
                if !check_db_admin_token(&auth, &cfg) {
                    return Ok(unauthorized_db_admin_token());
                }
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": false,
                        "error": "Metal GPU limit tuning is only available on macOS."
                    }),
                )))
            }
        })
}

#[cfg(not(target_os = "macos"))]
fn api_get_metal_gpu_limit(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "system" / "metal-gpu-limit")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "limit_mb": 0,
                        "custom": false,
                    }),
                )))
            }
        })
}

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state.clone();
    let config = ctx.config.clone();

    let mut r = api_vram_estimate_breakdown(state.clone(), config.clone())
        .or(api_vram_estimate(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_vram_quant_compare(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_vram_auto_size(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_get_system_info(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_get_metal_gpu_limit(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_set_metal_gpu_limit(state.clone(), config.clone()))
        .unify()
        .boxed();
    r
}
