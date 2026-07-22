//! HF Qualification and Identity APIs (Phase 8A2).
//!
//! Authoritative post-selection qualification and authorship resolution.
//! Search is candidate discovery only; this module is the truth step.

use crate::hf::{
    fetch_raw_bytes_at, hf_get_model_info, hf_load_token, list_repo_siblings,
};
use crate::models::community_source_catalog::{load_catalog, entries_for_username};
use std::path::Path;
use std::sync::{LazyLock, Mutex};

static CATALOG_CACHE: LazyLock<Mutex<Option<crate::models::community_source_catalog::CommunitySourceCatalog>>> =
    LazyLock::new(|| Mutex::new(None));

/// Load or return the cached catalog.
fn get_cached_catalog(config_dir: &Path) -> crate::models::community_source_catalog::CommunitySourceCatalog {
    let mut guard = CATALOG_CACHE.lock().unwrap();
    guard.as_ref().cloned().unwrap_or_else(|| {
        let catalog = load_catalog(config_dir);
        *guard = Some(catalog.clone());
        catalog
    })
}

// ── Qualification ─────────────────────────────────────────────────────────────

/// Qualification request for POST /api/hf/qualify.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualifyRequest {
    /// HF repo id (owner/name). Required.
    pub repo_id: String,
    /// Revision to pin (branch name or commit SHA). Defaults to "main".
    #[serde(default)]
    pub revision: String,
    /// Target backend for qualification hints.
    #[serde(default)]
    pub backend: Option<String>,
}

/// Authoritative qualification evidence for a specific HF repo + revision.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfQualification {
    pub repo_id: String,
    pub revision: String,
    pub backend_hint: String,
    pub qualified_at: u64,
    pub format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HfConfigEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<HfTokenizerEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<HfTemplateEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight_index: Option<HfWeightIndexEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<HfExtrasEvidence>,
    pub runtime_snapshot: HfRuntimeSnapshot,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    /// Whether this repo is provisionally qualified for the requested backend.
    pub backend_qualified: bool,
    pub qualification_reason: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfConfigEvidence {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_layers: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_attention_heads: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_key_value_heads: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intermediate_size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rope_theta: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param_count_estimate: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sliding_window: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_dim: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfTokenizerEvidence {
    pub tokenizer_file_present: bool,
    pub special_tokens_present: bool,
    #[serde(default)]
    pub tokenizer_type: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfTemplateEvidence {
    pub has_chat_template: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_family: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfWeightIndexEvidence {
    pub weight_file_count: u64,
    pub total_weight_bytes: u64,
    #[serde(default)]
    pub has_mmproj: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfExtrasEvidence {
    #[serde(default)]
    pub vision: bool,
    #[serde(default)]
    pub tool_use: bool,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_config_params: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme_model_family: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security_signals: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfRuntimeSnapshot {
    pub tags: Vec<String>,
    pub gated: bool,
    pub private: bool,
    pub author: String,
}

/// Perform authoritative post-selection qualification (builder item 2).
pub async fn hf_qualify_repo(req: QualifyRequest) -> Result<HfQualification, String> {
    let repo_id = req.repo_id.trim();
    if !crate::hf::validate_hf_repo_id(repo_id) {
        return Err("Invalid repo_id format. Expected: owner/repo".into());
    }
    let revision = req.revision.trim().to_string();
    let target_backend = req.backend.as_deref().unwrap_or("llama.cpp").to_lowercase();

    let start = std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .unwrap_or_default()
        .as_secs();
    let mut errors = Vec::new();

    // Fetch runtime snapshot (best-effort; gated repos may require token)
    let runtime_snapshot = match hf_get_model_info(repo_id).await {
        Ok(info) => {
            let author = repo_id.split('/').next().unwrap_or("").to_string();
            HfRuntimeSnapshot {
                tags: info.tags.clone(),
                gated: info.gated,
                private: info.private,
                author,
            }
        }
        Err(e) => {
            errors.push(format!("runtime snapshot: {e}"));
            HfRuntimeSnapshot {
                tags: Vec::new(),
                gated: false,
                private: false,
                author: repo_id.split('/').next().unwrap_or("").to_string(),
            }
        }
    };

    // List repo files to determine format
    let siblings = match list_repo_siblings(repo_id).await {
        Ok(sibs) => sibs,
        Err(e) => {
            errors.push(format!("listing files: {e}"));
            return Ok(qualification_error(repo_id, revision, target_backend, errors));
        }
    };

    // Detect format (never trust HF filter=mlx)
    let format = detect_format(&siblings);

    // Collect evidence based on format
    let config = collect_config_evidence(repo_id, &revision, &siblings, &mut errors).await;
    let tokenizer = collect_tokenizer_evidence(&siblings);
    let chat_template = collect_template_evidence(repo_id, &revision, &siblings, &mut errors).await;
    let weight_index = collect_weight_evidence(repo_id, &siblings, &mut errors).await;
    let extras = collect_extras_evidence(repo_id, &siblings, &runtime_snapshot, &mut errors).await;

    // Determine backend hint and qualification
    let backend_hint = determine_backend_hint(&format, config.as_ref(), extras.as_ref());
    let backend_qualified = determine_qualification(&format, &target_backend, config.as_ref(), &siblings);
    let qualification_reason = build_qualification_reason(&format, &target_backend, &backend_hint, &backend_qualified, config.as_ref(), extras.as_ref());

    Ok(HfQualification {
        repo_id: repo_id.to_string(),
        revision,
        backend_hint,
        qualified_at: start,
        format,
        config,
        tokenizer,
        chat_template,
        weight_index,
        extras,
        runtime_snapshot,
        errors,
        backend_qualified,
        qualification_reason,
    })
}

/// Determine repo format from file listing (never trust HF filter=mlx).
fn detect_format(siblings: &[String]) -> String {
    let has_gguf = siblings.iter().any(|p| p.to_ascii_lowercase().ends_with(".gguf"));
    let has_mlx_config = siblings.iter().any(|p| p == "config.json");
    let has_safetensors = siblings.iter().any(|p| {
        let l = p.to_ascii_lowercase();
        l.ends_with(".safetensors") && !l.contains("index.json")
    });
    let has_index = siblings.iter().any(|p| {
        let l = p.to_ascii_lowercase();
        l.ends_with(".safetensors.index.json")
    });
    let has_pickle = siblings.iter().any(|p| p.to_ascii_lowercase().ends_with(".bin") || p.to_ascii_lowercase().ends_with(".pt"));

    if has_gguf {
        "gguf".into()
    } else if has_mlx_config {
        "mlx".into()
    } else if has_safetensors || has_index {
        "safetensors".into()
    } else if has_pickle {
        "pytorch".into()
    } else {
        "unknown".into()
    }
}

/// Collect config evidence from config.json or GGUF header.
async fn collect_config_evidence(
    repo_id: &str,
    revision: &str,
    siblings: &[String],
    errors: &mut Vec<String>,
) -> Option<HfConfigEvidence> {
    // Prefer config.json for MLX/safetensors repos
    if siblings.iter().any(|p| p == "config.json") {
        return collect_config_from_json(repo_id, revision).await;
    }

    // For GGUF repos, try fetching header metadata
    if siblings.iter().any(|p| p.to_ascii_lowercase().ends_with(".gguf")) {
        return collect_config_from_gguf(repo_id, siblings).await;
    }

    None
}

async fn collect_config_from_json(repo_id: &str, revision: &str) -> Option<HfConfigEvidence> {
    // Try MLX config first
    if let Ok(cfg) = crate::hf::fetch_mlx_config_revision_aware(repo_id, revision, "model.safetensors").await {
        let name_lower = repo_id.to_ascii_lowercase();
        let model_type = cfg.model_type.clone().unwrap_or_else(|| infer_model_type_from_name(&name_lower));

        Some(HfConfigEvidence {
            source: "config.json (MLX)".into(),
            model_type: Some(model_type.clone()),
            architecture: Some(model_type),
            hidden_size: cfg.hidden_size,
            num_layers: cfg.num_layers,
            num_attention_heads: cfg.num_attention_heads,
            num_key_value_heads: cfg.num_key_value_heads,
            intermediate_size: cfg.n_ff.or(cfg.moe_intermediate_size).or(cfg.shared_expert_intermediate_size),
            rope_theta: None,
            param_count_estimate: None,
            context_length: cfg.max_position_embeddings,
            sliding_window: cfg.sliding_window,
            head_dim: cfg.head_dim.or(cfg.global_head_dim),
        })
    } else {
        // Fallback: raw JSON parse for non-MLX config
        collect_config_from_raw_json(repo_id, revision).await
    }
}

async fn collect_config_from_raw_json(repo_id: &str, revision: &str) -> Option<HfConfigEvidence> {
    match fetch_raw_bytes_at(repo_id, revision, "config.json", 512 * 1024).await {
        Ok(bytes) => {
            let text = match String::from_utf8(bytes) {
                Ok(t) => t,
                Err(_) => return None,
            };
            let v: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => return None,
            };

            let num_layers = v["n_layer"].as_u64()
                .or_else(|| v["n_layers"].as_u64())
                .or_else(|| v["num_hidden_layers"].as_u64())
                .or_else(|| v["num_layers"].as_u64())
                .or_else(|| v["transformer_layers"].as_u64())
                .map(|n| n as u32);

            let hidden_size = v["n_embd"].as_u64()
                .or_else(|| v["hidden_size"].as_u64())
                .or_else(|| v["d_model"].as_u64())
                .map(|n| n as u32);

            let num_attention_heads = v["n_head"].as_u64()
                .or_else(|| v["num_attention_heads"].as_u64())
                .map(|n| n as u32);

            let num_key_value_heads = v["n_head_kv"].as_u64()
                .or_else(|| v["num_key_value_heads"].as_u64())
                .map(|n| n as u32);

            let intermediate_size = v["n_inner"].as_u64()
                .or_else(|| v["intermediate_size"].as_u64())
                .or_else(|| v["ffn_dim"].as_u64())
                .map(|n| n as u32);

            let context_length = v["n_ctx"].as_u64()
                .or_else(|| v["max_position_embeddings"].as_u64())
                .map(|n| n as u32);

            let model_type = v["model_type"].as_str().map(|s| s.to_string());

            Some(HfConfigEvidence {
                source: "config.json (raw)".into(),
                model_type,
                architecture: None,
                hidden_size,
                num_layers,
                num_attention_heads,
                num_key_value_heads,
                intermediate_size,
                rope_theta: None,
                param_count_estimate: None,
                context_length,
                sliding_window: None,
                head_dim: None,
            })
        }
        Err(_) => None,
    }
}

async fn collect_config_from_gguf(repo_id: &str, siblings: &[String]) -> Option<HfConfigEvidence> {
    // Find first GGUF file
    let gguf_path = siblings.iter()
        .find(|p| p.to_ascii_lowercase().ends_with(".gguf"))
        .map(|s| s.as_str())?;

    match crate::hf::fetch_gguf_header_metadata(repo_id, gguf_path).await {
        Ok(meta) => {
            Some(HfConfigEvidence {
                source: "gguf".into(),
                model_type: meta.architecture.clone(),
                architecture: meta.architecture,
                hidden_size: meta.embedding_length,
                num_layers: meta.block_count,
                num_attention_heads: meta.head_count,
                num_key_value_heads: meta.head_count_kv,
                intermediate_size: meta.feed_forward_length,
                rope_theta: None,
                param_count_estimate: meta.param_count,
                context_length: meta.context_length,
                sliding_window: meta.sliding_window,
                head_dim: meta.key_length,
            })
        }
        Err(_) => None,
    }
}

/// Collect tokenizer evidence from file listing.
fn collect_tokenizer_evidence(siblings: &[String]) -> Option<HfTokenizerEvidence> {
    let has_tokenizer_json = siblings.iter().any(|p| p.contains("tokenizer.json"));
    let has_tokenizer_model = siblings.iter().any(|p| p.contains("tokenizer.model"));
    let has_tokenizer_config = siblings.iter().any(|p| p.contains("tokenizer_config.json"));
    let has_special_tokens = siblings.iter().any(|p| p.contains("special_tokens"));
    let has_merges = siblings.iter().any(|p| p.contains("merges.txt"));

    let tokenizer_present = has_tokenizer_json || has_tokenizer_model || has_tokenizer_config;
    if !tokenizer_present {
        return None;
    }

    let tokenizer_type = if has_tokenizer_json {
        "huggingface".into()
    } else if has_merges || has_tokenizer_model {
        "bpe".into()
    } else {
        "unknown".into()
    };

    Some(HfTokenizerEvidence {
        tokenizer_file_present: tokenizer_present,
        special_tokens_present: has_special_tokens || has_tokenizer_config,
        tokenizer_type,
    })
}

/// Collect chat template evidence.
async fn collect_template_evidence(
    repo_id: &str,
    revision: &str,
    siblings: &[String],
    errors: &mut Vec<String>,
) -> Option<HfTemplateEvidence> {
    // Check for standalone template file
    let standalone_template = siblings.iter().any(|p| {
        let l = p.to_ascii_lowercase();
        l.contains("chat_template") && (l.contains(".jinja") || l.contains(".json"))
    });

    let mut template_source: Option<String> = standalone_template.then_some("file".into());
    let mut template_family: Option<String> = None;

    // Check config.json for embedded template
    if template_source.is_none()
        && let Ok(bytes) = fetch_raw_bytes_at(repo_id, revision, "config.json", 256 * 1024).await
        && let Ok(text) = String::from_utf8(bytes)
        && text.contains("chat_template")
    {
        template_source = Some("config.json".into());
        template_family = detect_template_family_from_text(&text);
    }

    // Check README for chat template or template family hints
    if template_family.is_none() {
        if let Ok(readme) = fetch_readme_text(repo_id).await {
            template_family = detect_template_family_from_text(&readme)
                .or_else(|| detect_template_family_from_name(repo_id));
        }
    }

    let has_template = template_source.is_some();
    if !has_template {
        return None;
    }

    Some(HfTemplateEvidence {
        has_chat_template: true,
        template_source,
        template_family,
    })
}

/// Detect template family from text content.
fn detect_template_family_from_text(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("chatml") || lower.contains("chat ml") {
        Some("chatml".into())
    } else if lower.contains("zephyr") || lower.contains("mistral-chat") {
        Some("zephyr".into())
    } else if lower.contains("llama-3") || lower.contains("llama3") {
        Some("llama3".into())
    } else if lower.contains("gemma") {
        Some("gemma".into())
    } else if lower.contains("qwen") {
        Some("qwen".into())
    } else if lower.contains("phi3") || lower.contains("phi-3") {
        Some("phi3".into())
    } else {
        None
    }
}

/// Detect template family from repo name.
fn detect_template_family_from_name(repo_id: &str) -> Option<String> {
    let lower = repo_id.to_ascii_lowercase();
    if lower.contains("gemma") {
        Some("gemma".into())
    } else if lower.contains("qwen") {
        Some("qwen".into())
    } else if lower.contains("llama") {
        Some("llama".into())
    } else {
        None
    }
}

/// Collect weight index evidence.
async fn collect_weight_evidence(
    repo_id: &str,
    siblings: &[String],
    errors: &mut Vec<String>,
) -> Option<HfWeightIndexEvidence> {
    let weight_files: Vec<&String> = siblings.iter()
        .filter(|p| {
            let l = p.to_ascii_lowercase();
            l.ends_with(".safetensors") || l.ends_with(".gguf")
        })
        .collect();

    if weight_files.is_empty() {
        return None;
    }

    let total_bytes = match fetch_weight_total_bytes(repo_id, siblings).await {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("weight sizes: {e}"));
            0
        }
    };

    let has_mmproj = siblings.iter().any(|p| {
        let l = p.to_ascii_lowercase();
        l.contains("mmproj") || l.contains("projector")
    });

    Some(HfWeightIndexEvidence {
        weight_file_count: weight_files.len() as u64,
        total_weight_bytes: total_bytes,
        has_mmproj,
    })
}

async fn fetch_weight_total_bytes(repo_id: &str, siblings: &[String]) -> Result<u64, String> {
    let url = format!("https://huggingface.co/api/models/{repo_id}/tree/main");
    let mut req = crate::hf::HF_HTTP_CLIENT.get(&url);
    if let Some(tok) = hf_load_token() {
        req = req.bearer_auth(tok);
    }
    let resp = req.send().await.map_err(|e| format!("tree request: {e}"))?;
    if !resp.status().is_success() {
        return Ok(0);
    }
    let items: Vec<serde_json::Value> = resp.json().await.map_err(|e| format!("tree parse: {e}"))?;

    let mut total: u64 = 0;
    for item in items {
        let path = item.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let lower = path.to_ascii_lowercase();
        if !(lower.ends_with(".safetensors") || lower.ends_with(".gguf")) {
            continue;
        }
        let size = item
            .get("lfs")
            .and_then(|lfs| lfs.get("size"))
            .and_then(|v| v.as_u64())
            .or_else(|| item.get("size").and_then(|v| v.as_u64()))
            .unwrap_or(0);
        if size > 0 {
            total = total.saturating_add(size);
        }
    }
    Ok(total)
}

/// Collect extras evidence.
async fn collect_extras_evidence(
    repo_id: &str,
    siblings: &[String],
    runtime: &HfRuntimeSnapshot,
    errors: &mut Vec<String>,
) -> Option<HfExtrasEvidence> {
    let name_lower = repo_id.to_ascii_lowercase();
    let tags_lower: Vec<String> = runtime.tags.iter()
        .map(|t| t.to_ascii_lowercase())
        .collect();

    let has_vision_files = siblings.iter().any(|p| {
        let l = p.to_ascii_lowercase();
        l.contains("vision") || l.contains("mmproj") || l.contains("projector")
    });
    let has_vision_tag = tags_lower.iter().any(|t| t.contains("vision"));
    let has_vision_in_config = siblings.iter().any(|p| p == "preprocessor_config.json");
    let vision = has_vision_files || has_vision_tag || has_vision_in_config;

    let tool_use = tags_lower.iter().any(|t| t.contains("tool_use") || t.contains("tools"))
        || name_lower.contains("tool") && name_lower.contains("use")
        || name_lower.contains("coder") || name_lower.contains("code")
        || name_lower.contains("function_calling");

    let reasoning = tags_lower.iter().any(|t| t.contains("reasoning") || t.contains("cot"))
        || name_lower.contains("reason") || name_lower.contains("thinking")
        || name_lower.contains("r1") || name_lower.contains("r0");

    let gen_config = match fetch_generation_config(repo_id).await {
        Ok(v) => v,
        Err(_) => None,
    };

    let security_signals = detect_security_signals(siblings);
    let readme_model_family = fetch_readme_model_family_hint(repo_id).await;

    Some(HfExtrasEvidence {
        vision,
        tool_use,
        reasoning,
        generation_config_params: gen_config,
        readme_model_family,
        security_signals,
    })
}

async fn fetch_generation_config(repo_id: &str) -> Result<Option<serde_json::Value>, String> {
    match fetch_raw_bytes_at(repo_id, "main", "generation_config.json", 64 * 1024).await {
        Ok(bytes) => {
            let text = String::from_utf8(bytes).map_err(|e| format!("utf8: {e}"))?;
            let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("parse: {e}"))?;
            Ok(Some(v))
        }
        Err(_) => Ok(None),
    }
}

/// Detect security-relevant signals.
fn detect_security_signals(siblings: &[String]) -> Vec<String> {
    let mut signals = Vec::new();
    for p in siblings {
        let lower = p.to_ascii_lowercase();
        if lower.contains("download") && (lower.contains(".py") || lower.contains(".sh")) {
            signals.push("Script file detected (download_*)".to_string());
        }
        if lower.contains("modeling") && lower.contains(".py") {
            signals.push("Python model code present (requires execution)".to_string());
        }
        if lower.contains(".env") || lower.contains("credentials") || lower.contains("secret") {
            signals.push("Potential credential file detected".to_string());
        }
    }
    signals.truncate(10);
    signals
}

async fn fetch_readme_text(repo_id: &str) -> Result<String, String> {
    let url = format!("https://huggingface.co/{repo_id}/raw/main/README.md");
    let mut req = crate::hf::HF_HTTP_CLIENT.get(&url);
    if let Some(tok) = hf_load_token() {
        req = req.bearer_auth(tok);
    }
    let resp = req.send().await.map_err(|e| format!("readme request: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("readme read: {e}"))?;
    if bytes.len() > 256 * 1024 {
        return Err("Readme too large".into());
    }
    String::from_utf8(bytes.to_vec()).map_err(|e| format!("readme utf8: {e}"))
}

async fn fetch_readme_model_family_hint(repo_id: &str) -> Option<String> {
    match fetch_readme_text(repo_id).await {
        Ok(readme) => {
            for line in readme.lines().take(50) {
                let ll = line.to_ascii_lowercase();
                if (ll.contains("model_name:") || ll.contains("model_family:")) && line.contains(':') {
                    let value = line.split(':').nth(1)?.trim().to_string();
                    if !value.is_empty() && value.len() < 100 {
                        return Some(value);
                    }
                }
            }
            Some(crate::hf::infer_family_from_name(repo_id))
        }
        Err(_) => None,
    }
}

/// Determine the suggested backend.
fn determine_backend_hint(format: &str, config: Option<&HfConfigEvidence>, _extras: Option<&HfExtrasEvidence>) -> String {
    let model_type = config.and_then(|c| c.model_type.as_deref())
        .or_else(|| config.and_then(|c| c.architecture.as_deref()))
        .unwrap_or("");

    let name_lower = model_type.to_ascii_lowercase();

    let llama_cpp_native = name_lower.is_empty() ||
        name_lower.contains("llama") ||
        name_lower.contains("mistral") ||
        name_lower.contains("phi") ||
        name_lower.contains("qwen") ||
        name_lower.contains("gemma") ||
        name_lower.contains("falcon") ||
        name_lower.contains("internlm") ||
        name_lower.contains("yi");

    let mlx_native = name_lower.contains("mlx") || name_lower.contains("mamba") || name_lower.contains("ssm");

    if format == "gguf" {
        return "llama.cpp".into();
    }
    if format == "mlx" {
        return "mlx".into();
    }
    if llama_cpp_native {
        return "llama.cpp (via GGUF conversion)".into();
    }
    if mlx_native {
        return "mlx".into();
    }
    "llama.cpp (via GGUF conversion)".into()
}

/// Determine if repo is provisionally qualified for the requested backend.
fn determine_qualification(format: &str, target: &str, config: Option<&HfConfigEvidence>, siblings: &[String]) -> bool {
    let target = target.trim();

    if target == "llama.cpp" {
        if format == "gguf" {
            return true;
        }
        if (format == "safetensors" || format == "mlx") && config.is_some() {
            return true;
        }
    }

    if target == "mlx" || target == "rapid-mlx" || target == "rapid_mlx" {
        if format == "mlx" {
            return true;
        }
        if format == "safetensors" && config.is_some() {
            return true;
        }
    }

    false
}

/// Build human-readable qualification reason.
fn build_qualification_reason(
    format: &str,
    target: &str,
    backend_hint: &str,
    qualified: &bool,
    config: Option<&HfConfigEvidence>,
    extras: Option<&HfExtrasEvidence>,
) -> String {
    if !qualified {
        return format!(
            "Not qualified for {}: format '{}' — {}",
            target,
            format,
            if config.is_none() {
                "no config evidence available"
            } else {
                "format not directly compatible"
            }
        );
    }

    let model_type = config.and_then(|c| c.model_type.as_deref()).unwrap_or("unknown");
    let extra_hints = extras.as_ref().map(|e| {
        let mut parts = Vec::new();
        if e.vision { parts.push("vision"); }
        if e.tool_use { parts.push("tool-use"); }
        if e.reasoning { parts.push("reasoning"); }
        parts.join(", ")
    }).unwrap_or_default();

    if extra_hints.is_empty() {
        format!("Qualified for {}: format '{}', architecture '{}' — suggested backend: {}",
            target, format, model_type, backend_hint)
    } else {
        format!("Qualified for {}: format '{}', architecture '{}' (hints: {}) — suggested backend: {}",
            target, format, model_type, extra_hints, backend_hint)
    }
}

/// Error qualification result when critical steps fail.
fn qualification_error(repo_id: &str, revision: String, backend: String, mut errors: Vec<String>) -> HfQualification {
    if errors.is_empty() {
        errors.push("Failed to inspect repo files".into());
    }
    HfQualification {
        repo_id: repo_id.to_string(),
        revision,
        backend_hint: "unknown".into(),
        qualified_at: 0,
        format: "unknown".into(),
        config: None,
        tokenizer: None,
        chat_template: None,
        weight_index: None,
        extras: None,
        runtime_snapshot: HfRuntimeSnapshot {
            tags: Vec::new(),
            gated: false,
            private: false,
            author: repo_id.split('/').next().unwrap_or("").to_string(),
        },
        errors,
        backend_qualified: false,
        qualification_reason: format!("Failed: {}", errors.join("; ")),
    }
}

/// Infer model type from name.
fn infer_model_type_from_name(name: &str) -> String {
    if name.contains("qwen3.6") || name.contains("qwen36") { "qwen3.6".into() }
    else if name.contains("qwen3.5") || name.contains("qwen35") { "qwen3.5".into() }
    else if name.contains("qwen3") { "qwen3".into() }
    else if name.contains("qwen") { "qwen".into() }
    else if name.contains("llama-3.3") || name.contains("llama33") { "llama3.3".into() }
    else if name.contains("llama-3") || name.contains("llama3") { "llama3".into() }
    else if name.contains("llama") { "llama".into() }
    else if name.contains("gemma-4") || name.contains("gemma4") { "gemma4".into() }
    else if name.contains("gemma") { "gemma".into() }
    else if name.contains("mistral") { "mistral".into() }
    else if name.contains("phi") { "phi".into() }
    else { "unknown".into() }
}

// ── Identity ──────────────────────────────────────────────────────────────────

/// Identity request for POST /api/hf/identity.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityRequest {
    /// HF repo id. Required.
    pub repo_id: String,
    /// Revision to use (defaults to "main").
    #[serde(default)]
    pub revision: String,
    /// Optional config dir for catalog loading.
    #[serde(default)]
    pub config_dir: Option<String>,
}

/// Authorship and lineage identity resolution.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfIdentity {
    pub repo_id: String,
    pub revision: String,
    /// Original model author (distinct from quantizer/converter/publisher).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_author: Option<HfIdentityEntity>,
    /// Base model(s) this repo derives from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_model: Option<Vec<String>>,
    /// Whether this is a finetune.
    #[serde(default)]
    pub is_finetune: bool,
    /// Whether this is a merge/distillation.
    #[serde(default)]
    pub is_merge_distill: bool,
    /// Dataset(s) used (if stated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_used: Option<Vec<String>>,
    /// Who published this specific repo (may differ from original author).
    pub artifact_publisher: HfIdentityEntity,
    /// Converter role (for MLX/GGUF conversions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub converter_role: Option<HfIdentityConverter>,
    /// All resolved roles as (CommunitySourceRole, username) pairs.
    #[serde(default)]
    pub roles: Vec<HfIdentityRole>,
    /// Confidence in the overall resolution.
    pub resolution_confidence: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfIdentityEntity {
    pub username: String,
    pub display_name: String,
    pub confidence: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfIdentityConverter {
    pub username: String,
    pub role: String,
    pub format: String,
    pub original_source: Option<String>,
    pub confidence: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfIdentityRole {
    pub role: String,
    pub username: String,
    pub confidence: String,
}

/// Resolve identity and lineage (builder item 4).
pub async fn hf_resolve_identity(
    req: IdentityRequest,
    config_dir: &Path,
) -> Result<HfIdentity, String> {
    let repo_id = req.repo_id.trim();
    if !crate::hf::validate_hf_repo_id(repo_id) {
        return Err("Invalid repo_id format".into());
    }
    let revision = req.revision.trim().to_string();
    let mut errors = Vec::new();

    let catalog = get_cached_catalog(config_dir);

    // Fetch runtime info
    let info = match hf_get_model_info(repo_id).await {
        Ok(i) => i,
        Err(e) => {
            errors.push(format!("model info: {e}"));
            return Ok(identity_minimal(repo_id, revision, errors));
        }
    };

    // List repo files
    let siblings = match list_repo_siblings(repo_id).await {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("listing files: {e}"));
            Vec::new()
        }
    };

    let repo_owner = repo_id.split('/').next().unwrap_or("").to_string();
    let owner_lower = repo_owner.to_ascii_lowercase();
    let name_lower = repo_id.to_ascii_lowercase();

    // Detect format
    let format = detect_format(&siblings);

    // Extract base model from tags
    let base_models = info.tags.iter()
        .filter(|t| t.starts_with("base_model:"))
        .map(|t| t.strip_prefix("base_model:").unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    // Fetch README for deeper lineage
    let readme = fetch_readme_text(repo_id).await.unwrap_or_default();

    // Resolve roles
    let (original_author, is_finetune, is_merge_distill, dataset_used, converter_role, mut roles) =
        resolve_identity_roles(&repo_owner, &owner_lower, &name_lower, &siblings, &format, &info.tags, &base_models, &readme, &catalog);

    // Artifact publisher is always the repo owner
    let catalog_entry = entries_for_username(&catalog, &repo_owner).first();
    let artifact_publisher = HfIdentityEntity {
        username: repo_owner.clone(),
        display_name: catalog_entry.map(|e| e.display_name.clone()).unwrap_or_else(|| repo_owner.clone()),
        confidence: "high".into(),
        evidence: vec![format!("HF repo owner of {repo_id}")],
    };

    roles.push(HfIdentityRole {
        role: "artifact_publisher".into(),
        username: repo_owner,
        confidence: "high".into(),
    });

    let resolution_confidence = determine_resolution_confidence(
        &original_author,
        &converter_role,
        &is_finetune,
        &base_models,
        &errors,
    );

    Ok(HfIdentity {
        repo_id: repo_id.to_string(),
        revision,
        original_author,
        base_model: if base_models.is_empty() { None } else { Some(base_models) },
        is_finetune,
        is_merge_distill,
        dataset_used,
        artifact_publisher,
        converter_role,
        roles,
        resolution_confidence,
        errors,
    })
}

/// Resolve all identity roles from evidence.
fn resolve_identity_roles(
    repo_owner: &str,
    owner_lower: &str,
    name_lower: &str,
    siblings: &[String],
    format: &str,
    tags: &[String],
    base_models: &[String],
    readme: &str,
    catalog: &crate::models::community_source_catalog::CommunitySourceCatalog,
) -> (Option<HfIdentityEntity>, bool, bool, Option<Vec<String>>, Option<HfIdentityConverter>, Vec<HfIdentityRole>) {
    let mut roles = Vec::new();
    let mut original_author: Option<HfIdentityEntity> = None;
    let mut converter_role: Option<HfIdentityConverter> = None;
    let mut is_finetune = false;
    let mut is_merge_distill = false;

    let is_gguf = format == "gguf";
    let is_mlx = format == "mlx";

    // 1. Converter role
    if is_gguf || is_mlx {
        let role_name = if is_gguf { "GgufQuantizer" } else { "MlxConverter" };
        let catalog_entry = entries_for_username(catalog, repo_owner).first();

        converter_role = Some(HfIdentityConverter {
            username: repo_owner.to_string(),
            role: role_name.to_string(),
            format: format.to_string(),
            original_source: detect_original_source_from_readme(readme, tags),
            confidence: if catalog_entry.is_some() { "high" } else { "medium" },
        });

        roles.push(HfIdentityRole {
            role: role_name.to_string(),
            username: repo_owner.to_string(),
            confidence: converter_role.as_ref().unwrap().confidence.clone(),
        });
    }

    // 2. Original author — distinct from converter
    original_author = detect_first_party_author(owner_lower, name_lower, tags, siblings);

    if original_author.is_none() {
        original_author = detect_community_author(owner_lower, name_lower, tags, readme);
    }

    if original_author.is_none() && !base_models.is_empty() {
        if let Some(entry) = entries_for_username(catalog, repo_owner).first() {
            if matches!(entry.role, crate::models::community_source_catalog::CommunitySourceRole::OriginalAuthor) {
                original_author = Some(HfIdentityEntity {
                    username: repo_owner.to_string(),
                    display_name: entry.display_name.clone(),
                    confidence: "high".into(),
                    evidence: vec![format!("Catalog entry as {}", entry.role.label())],
                });
            }
        }
    }

    // 3. Finetune detection
    let tags_lower: Vec<String> = tags.iter().map(|t| t.to_ascii_lowercase()).collect();
    let has_ft_name = name_lower.contains("ft-") || name_lower.contains("-ft") ||
        name_lower.contains("finetune") || name_lower.contains("fine-tune");
    let has_ft_tag = tags_lower.iter().any(|t| t.contains("fine-tune") || t.contains("finetune") || t.contains("ft:"));
    let has_ft_readme = readme.to_ascii_lowercase().contains("fine-tune") || readme.to_ascii_lowercase().contains("finetune");

    if !base_models.is_empty() || has_ft_name || has_ft_tag || has_ft_readme {
        is_finetune = true;
    }

    // 4. Merge/distill detection
    let has_merge_name = name_lower.contains("merge") || name_lower.contains("distill") ||
        name_lower.contains("heretic") || name_lower.contains("abliterated");
    let has_merge_tag = tags_lower.iter().any(|t| t.contains("merge") || t.contains("distill"));

    if has_merge_name || has_merge_tag {
        is_merge_distill = true;
    }

    // 5. Add original author to roles
    if let Some(ref author) = original_author {
        roles.push(HfIdentityRole {
            role: "OriginalAuthor".into(),
            username: author.username.clone(),
            confidence: author.confidence.clone(),
        });
    }

    // 6. Dataset lineage
    let dataset_used = extract_datasets(readme, tags);

    (original_author, is_finetune, is_merge_distill, dataset_used, converter_role, roles)
}

/// Detect first-party original author.
fn detect_first_party_author(
    owner_lower: &str,
    name_lower: &str,
    tags: &[String],
    siblings: &[String],
) -> Option<HfIdentityEntity> {
    let first_party = &[
        ("meta-llama", "Meta", "llama"),
        ("qwenlm", "Qwen", "qwen"),
        ("google", "Google", "gemma"),
        ("mistralai", "Mistral AI", "mistral"),
        ("microsoft", "Microsoft", "phi"),
        ("deepseek-ai", "DeepSeek", "deepseek"),
    ];

    for (org, display, keyword) in first_party {
        if owner_lower == *org ||
            (name_lower.contains(*keyword) && has_first_party_evidence(tags, siblings)) {
            return Some(HfIdentityEntity {
                username: (*org).into(),
                display_name: (*display).into(),
                confidence: "high".into(),
                evidence: vec!["First-party model author".into()],
            });
        }
    }
    None
}

fn has_first_party_evidence(tags: &[String], siblings: &[String]) -> bool {
    let has_safetensors = siblings.iter().any(|p| p.to_ascii_lowercase().ends_with(".safetensors"));
    let has_config = siblings.iter().any(|p| p == "config.json");
    let has_model_card = tags.iter().any(|t| t.starts_with("license:"));
    has_safetensors && has_config && has_model_card
}

/// Detect community finetune/merge author.
fn detect_community_author(
    owner_lower: &str,
    name_lower: &str,
    tags: &[String],
    readme: &str,
) -> Option<HfIdentityEntity> {
    let tags_lower: Vec<String> = tags.iter().map(|t| t.to_ascii_lowercase()).collect();

    if owner_lower == "unsloth" {
        return Some(HfIdentityEntity {
            username: "unsloth".into(),
            display_name: "Unsloth".into(),
            confidence: "high".into(),
            evidence: vec!["Unsloth fine-tune and distillation author".into()],
        });
    }

    if owner_lower == "davidau" {
        return Some(HfIdentityEntity {
            username: "DavidAU".into(),
            display_name: "DavidAU".into(),
            confidence: "high".into(),
            evidence: vec!["DavidAU: heretic/abliterated variant specialist".into()],
        });
    }

    if let Some(creator) = extract_readme_creator(readme) {
        return Some(HfIdentityEntity {
            username: creator.clone(),
            display_name: creator,
            confidence: "medium".into(),
            evidence: vec!["README creator field".into()],
        });
    }

    let has_creation_evidence = tags_lower.iter().any(|t|
        t.contains("fine-tune") || t.contains("merge") || t.contains("distill")
    ) || name_lower.contains("ft-") || name_lower.contains("heretic");

    if has_creation_evidence {
        return Some(HfIdentityEntity {
            username: owner_lower.to_string(),
            display_name: owner_lower.to_string(),
            confidence: "provisional".into(),
            evidence: vec!["Creation evidence in tags/name; no known-author match".into()],
        });
    }

    None
}

fn extract_readme_creator(readme: &str) -> Option<String> {
    for line in readme.lines().take(50) {
        let ll = line.to_ascii_lowercase();
        if ll.starts_with("creator:") && line.contains(':') {
            return Some(line.split(':').nth(1)?.trim().to_string());
        }
    }
    None
}

fn detect_original_source_from_readme(readme: &str, tags: &[String]) -> Option<String> {
    if let Some(base) = tags.iter().find(|t| t.starts_with("base_model:")) {
        return Some(base.strip_prefix("base_model:").unwrap_or("").trim().to_string());
    }

    for line in readme.lines().take(100) {
        let ll = line.to_ascii_lowercase();
        if (ll.contains("based on") || ll.contains("original:") || ll.contains("from"))
            && line.contains('/')
        {
            for segment in line.split(|c: char| !c.is_alphanumeric() && !matches!(c, '/' | '_' | '-' | '.')) {
                if segment.len() > 3 && segment.contains('/') && segment.split('/').count() == 2 {
                    return Some(segment.trim().to_string());
                }
            }
        }
    }
    None
}

fn extract_datasets(readme: &str, tags: &[String]) -> Option<Vec<String>> {
    let mut datasets = Vec::new();
    for tag in tags {
        if tag.starts_with("dataset:") {
            datasets.push(tag.strip_prefix("dataset:").unwrap_or("").trim().to_string());
        }
    }
    for line in readme.lines().take(100) {
        let ll = line.to_ascii_lowercase();
        if ll.contains("dataset") || ll.contains("training data") {
            for segment in line.split(|c: char| !c.is_alphanumeric() && !matches!(c, '/' | '_' | '-' | '.')) {
                if segment.len() > 4 && segment.contains('/') && !datasets.contains(&segment.trim().to_string()) {
                    datasets.push(segment.trim().to_string());
                }
            }
        }
    }
    if datasets.is_empty() { None } else { Some(datasets) }
}

fn determine_resolution_confidence(
    original_author: &Option<HfIdentityEntity>,
    converter_role: &Option<HfIdentityConverter>,
    is_finetune: &bool,
    base_models: &[String],
    errors: &[String],
) -> String {
    if !errors.is_empty() {
        return "low".into();
    }

    let author_confidence = original_author.as_ref().map(|a| a.confidence.as_str()).unwrap_or("");

    if author_confidence == "high" && base_models.is_empty() && !is_finetune {
        "high".into()
    } else if author_confidence == "high" && (!base_models.is_empty() || is_finetune) {
        "high".into()
    } else if author_confidence == "medium" || converter_role.is_some() {
        "medium".into()
    } else if original_author.is_some() {
        "low".into()
    } else {
        "provisional".into()
    }
}

fn identity_minimal(repo_id: &str, revision: String, errors: Vec<String>) -> HfIdentity {
    let owner = repo_id.split('/').next().unwrap_or(repo_id).to_string();
    HfIdentity {
        repo_id: repo_id.to_string(),
        revision,
        original_author: None,
        base_model: None,
        is_finetune: false,
        is_merge_distill: false,
        dataset_used: None,
        artifact_publisher: HfIdentityEntity {
            username: owner.clone(),
            display_name: owner,
            confidence: "high".into(),
            evidence: vec!["Inferred from repo_id".into()],
        },
        converter_role: None,
        roles: Vec::new(),
        resolution_confidence: "low".into(),
        errors,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_format_gguf() {
        let siblings = vec!["model-Q4_K_M.gguf".into()];
        assert_eq!(detect_format(&siblings), "gguf");
    }

    #[test]
    fn test_detect_format_mlx() {
        let siblings = vec!["config.json".into(), "weights.safetensors".into()];
        assert_eq!(detect_format(&siblings), "mlx");
    }

    #[test]
    fn test_detect_format_safetensors() {
        let siblings = vec!["model.safetensors".into()];
        assert_eq!(detect_format(&siblings), "safetensors");
    }

    #[test]
    fn test_backend_hint_gguf_is_llamacpp() {
        assert_eq!(determine_backend_hint("gguf", None, None), "llama.cpp");
    }

    #[test]
    fn test_backend_hint_mlx_is_mlx() {
        assert_eq!(determine_backend_hint("mlx", None, None), "mlx");
    }

    #[test]
    fn test_qualification_gguf_for_llamacpp() {
        let siblings = vec!["model.gguf".into()];
        let qualified = determine_qualification("gguf", "llama.cpp", None, &siblings);
        assert!(qualified);
    }

    #[test]
    fn test_qualification_mlx_for_rapid() {
        let siblings = vec!["config.json".into()];
        let config = Some(HfConfigEvidence {
            source: "test".into(),
            model_type: Some("qwen3".into()),
            ..Default::default()
        });
        let qualified = determine_qualification("mlx", "rapid-mlx", config.as_ref(), &siblings);
        assert!(qualified);
    }

    #[test]
    fn test_security_signals_detects_scripts() {
        let siblings = vec!["download_model.py".into()];
        let signals = detect_security_signals(&siblings);
        assert!(!signals.is_empty());
        assert!(signals[0].contains("Script"));
    }

    #[test]
    fn test_model_type_inference() {
        assert_eq!(infer_model_type_from_name("Qwen3.6-27B"), "qwen3.6");
        assert_eq!(infer_model_type_from_name("Llama-3.3-70B"), "llama3.3");
        assert_eq!(infer_model_type_from_name("Gemma-4-12B"), "gemma4");
    }

    #[test]
    fn test_qualify_request_deser() {
        let json = r#"{"repoId":"test/model"}"#;
        let req: QualifyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.repo_id, "test/model");
        assert_eq!(req.revision, "");
        assert!(req.backend.is_none());
    }

    #[test]
    fn test_identity_request_deser() {
        let json = r#"{"repoId":"test/model","revision":"abc123"}"#;
        let req: IdentityRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.repo_id, "test/model");
        assert_eq!(req.revision, "abc123");
    }
}
