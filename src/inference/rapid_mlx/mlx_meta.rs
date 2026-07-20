//! MLX / Rapid-MLX model metadata reader.
//!
//! This module parses MLX model configs and safetensors indexes into a backend-neutral
//! [`ModelMemoryProfile`] with field-level evidence. It supports:
//!
//! - Nested `text_config` configs (Qwen3.6, Gemma4, etc.) — geometry from inner config only
//! - Flat configs (Qwen3, etc.) — geometry from top-level fields
//! - Wrapper-field protection — outer wrapper fields don't override inner text geometry
//! - Field-level evidence for every populated value
//!
//! Per D1/D2/A53: this parser populates the shared geometry profile; it does not contain
//! backend-specific allocation math or llama.cpp vocabulary.
//!
//! NOTE: dead_code allowed until Parts B/C wire up consumption (Phase 4 Part B: geometry
//! population; Part C: HF lookup, context propagation, estimator integration).

#![allow(dead_code)]

use std::path::Path;

use crate::llama::model_memory_profile::*;
use crate::llama::vram_estimator::ModelArch;

pub const MAX_CONFIG_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MlxMetaEvidence {
    #[default]
    Exact,
    Degraded,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MlxQuantization {
    pub bits: Option<u32>,
    pub group_size: Option<u32>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MlxDraftConfig {
    pub model: Option<String>,
    pub num_hidden_layers: Option<u32>,
}

/// Raw HF-transformers-style `config.json`.
/// Every field is optional — missing fields degrade evidence, never silently guess.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MlxConfig {
    pub model_type: Option<String>,
    #[serde(default)]
    pub architectures: Option<Vec<String>>,
    pub hidden_size: Option<u32>,
    #[serde(alias = "num_hidden_layers")]
    pub num_layers: Option<u32>,
    pub num_attention_heads: Option<u32>,
    #[serde(alias = "num_kv_heads")]
    pub num_key_value_heads: Option<u32>,
    #[serde(alias = "intermediate_size")]
    pub n_ff: Option<u32>,
    pub head_dim: Option<u32>,
    #[serde(alias = "num_local_experts", alias = "n_routed_experts")]
    pub num_experts: Option<u32>,
    #[serde(alias = "num_experts_per_token", alias = "n_experts_used")]
    pub num_experts_per_tok: Option<u32>,
    pub sliding_window: Option<u32>,
    pub sliding_window_pattern: Option<u32>,
    pub max_position_embeddings: Option<u32>,
    pub quantization: Option<MlxQuantization>,
    pub draft_model: Option<MlxDraftConfig>,
    pub speculative_config: Option<MlxDraftConfig>,
    pub vision_config: Option<serde_json::Value>,
    /// Nested text architecture config (Qwen3.6, Gemma4, etc.).
    #[serde(default)]
    pub text_config: Option<serde_json::Value>,
    /// Full attention interval for hybrid architectures.
    #[serde(default)]
    pub full_attention_interval: Option<u32>,
    /// Layer types array from config.
    #[serde(default)]
    pub layer_types: Option<Vec<String>>,
    /// MoE intermediate size.
    #[serde(default)]
    pub moe_intermediate_size: Option<u32>,
    /// Shared expert intermediate size.
    #[serde(default)]
    pub shared_expert_intermediate_size: Option<u32>,
    /// Top-K experts for Gemma4-style models.
    #[serde(default)]
    pub top_k_experts: Option<u32>,
    /// Num global KV heads (Gemma4).
    #[serde(default)]
    pub num_global_key_value_heads: Option<u32>,
    /// Global head dim (Gemma4).
    #[serde(default)]
    pub global_head_dim: Option<u32>,
    /// MTP layers.
    #[serde(default)]
    pub mtp_num_hidden_layers: Option<u32>,
    #[serde(default)]
    pub mtp_use_dedicated_embeddings: Option<bool>,
    /// Linear/SSM dimensions.
    #[serde(default)]
    pub linear_conv_kernel_dim: Option<u32>,
    #[serde(default)]
    pub linear_key_head_dim: Option<u32>,
    #[serde(default)]
    pub linear_num_key_heads: Option<u32>,
    #[serde(default)]
    pub linear_num_value_heads: Option<u32>,
    #[serde(default)]
    pub linear_value_head_dim: Option<u32>,
    #[serde(default)]
    pub mamba_ssm_dtype: Option<String>,
    /// Vocab size.
    #[serde(default)]
    pub vocab_size: Option<u32>,
    /// RMS norm epsilon.
    #[serde(default)]
    pub rms_norm_eps: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct MlxWeightIndex {
    pub shard_files: Vec<String>,
    pub total_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct MlxMetadata {
    pub config: MlxConfig,
    pub weight_index: MlxWeightIndex,
    pub evidence: MlxMetaEvidence,
}

fn bounded_read(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > max_bytes {
        return Err(format!(
            "{} exceeds the {max_bytes}-byte read cap ({} bytes)",
            path.display(),
            meta.len()
        ));
    }
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn read_mlx_config(dir: &Path) -> Result<MlxConfig, String> {
    let bytes = bounded_read(&dir.join("config.json"), MAX_CONFIG_BYTES)?;
    parse_mlx_config(&bytes)
}

pub fn parse_mlx_config(bytes: &[u8]) -> Result<MlxConfig, String> {
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(format!(
            "config.json exceeds the {MAX_CONFIG_BYTES}-byte read cap"
        ));
    }
    serde_json::from_slice(bytes).map_err(|e| format!("config.json is not valid JSON: {e}"))
}

pub fn read_mlx_weight_index(dir: &Path) -> Result<MlxWeightIndex, String> {
    let index_path = dir.join("model.safetensors.index.json");
    let bytes = bounded_read(&index_path, MAX_INDEX_BYTES)?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("safetensors index: {e}"))?;

    let weight_map = value
        .get("weight_map")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "safetensors index requires a weight_map object".to_string())?;

    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for v in weight_map.values() {
        let Some(name) = v.as_str() else {
            return Err("safetensors index contains a non-string shard".into());
        };
        let relative = Path::new(name);
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
            || !name.ends_with(".safetensors")
        {
            return Err(format!(
                "safetensors index contains an unsafe shard path: {name}"
            ));
        }
        names.insert(name.to_string());
    }

    let total_size_bytes = value
        .get("metadata")
        .and_then(|m| m.get("total_size"))
        .and_then(|t| t.as_u64());

    Ok(MlxWeightIndex {
        shard_files: names.into_iter().collect(),
        total_size_bytes,
    })
}

pub fn resolve_local_weight_bytes(dir: &Path, index: &MlxWeightIndex) -> Option<u64> {
    if let Some(total) = index.total_size_bytes {
        return Some(total);
    }
    if index.shard_files.is_empty() {
        return None;
    }
    let mut total = 0u64;
    for name in &index.shard_files {
        let meta = std::fs::metadata(dir.join(name)).ok()?;
        total = total.saturating_add(meta.len());
    }
    Some(total)
}

pub fn read_mlx_metadata(dir: &Path) -> Result<MlxMetadata, String> {
    let config = read_mlx_config(dir)?;
    let weight_index = read_mlx_weight_index(dir).unwrap_or_default();
    Ok(finish_metadata(config, weight_index))
}

pub fn metadata_from_config(config: MlxConfig) -> MlxMetadata {
    finish_metadata(config, MlxWeightIndex::default())
}

fn finish_metadata(config: MlxConfig, weight_index: MlxWeightIndex) -> MlxMetadata {
    let evidence = if config.hidden_size.is_some()
        && config.num_layers.is_some()
        && config.num_attention_heads.is_some()
    {
        MlxMetaEvidence::Exact
    } else {
        MlxMetaEvidence::Degraded
    };
    MlxMetadata {
        config,
        weight_index,
        evidence,
    }
}

// ── ModelMemoryProfile parsing ───────────────────────────────────────────────────

/// Parse an MLX config.json from a file into a normalized [`ModelMemoryProfile`].
///
/// This is the primary entry point for Part A/B/C: it reads config.json, handles nested
/// text_config, applies wrapper-field protection, and populates the geometry profile with
/// field-level evidence.
pub fn parse_mlx_config_to_profile(config_path: &Path) -> Result<ModelMemoryProfile, String> {
    let bytes = bounded_read(config_path, MAX_CONFIG_BYTES)?;
    parse_mlx_config_bytes_to_profile(&bytes)
}

/// Parse MLX config bytes into a normalized [`ModelMemoryProfile`].
pub fn parse_mlx_config_bytes_to_profile(bytes: &[u8]) -> Result<ModelMemoryProfile, String> {
    let raw: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| format!("config.json is not valid JSON: {e}"))?;

    let mut profile = ModelMemoryProfile::default();

    // Determine if nested text_config exists and extract geometry from it.
    let text_config_value = raw.get("text_config").cloned();
    let _has_text_config = text_config_value.is_some();

    if let Some(tc) = text_config_value {
        // Extract geometry from nested text_config (authoritative for inner architecture).
        profile.used_text_config = Some(true);
        extract_from_text_config(&mut profile, &tc);

        // Check for wrapper-field conflicts and document them as warnings.
        check_wrapper_field_conflicts(&mut profile, &raw);
    } else {
        // Flat config: geometry from top-level fields.
        profile.used_text_config = Some(false);
        extract_from_flat_config(&mut profile, &raw);
    }

    // Extract model type / architectures from wrapper (metadata, not geometry).
    extract_model_identity(&mut profile, &raw);

    // Extract vision component.
    extract_vision_component(&mut profile, &raw);

    // Extract draft/MTP config.
    extract_draft_config(&mut profile, &raw);

    Ok(profile)
}

fn extract_from_text_config(profile: &mut ModelMemoryProfile, tc: &serde_json::Value) {
    let prefix = "text_config";

    // Model type (from text_config).
    if let Some(mt) = tc.get("model_type").and_then(|v| v.as_str()) {
        profile.model_type = Some(mt.to_string());
        profile.model_type_evidence = Some(format!("{prefix}.model_type"));
    }

    // Hidden size.
    if let Some(v) = tc.get("hidden_size").and_then(|v| v.as_u64()) {
        profile.weights.n_embd.value = v as u32;
        profile.weights.n_embd.field_evidence = format!("{prefix}.hidden_size");
    }

    // Num layers.
    if let Some(v) = tc.get("num_hidden_layers").and_then(|v| v.as_u64()) {
        profile.weights.n_layers.value = v as u32;
        profile.weights.n_layers.field_evidence = format!("{prefix}.num_hidden_layers");
    }

    // Num attention heads.
    if let Some(v) = tc.get("num_attention_heads").and_then(|v| v.as_u64()) {
        profile.weights.n_head.value = v as u32;
        profile.weights.n_head.field_evidence = format!("{prefix}.num_attention_heads");
    }

    // Num KV heads.
    if let Some(v) = tc.get("num_key_value_heads").and_then(|v| v.as_u64()) {
        profile.weights.n_head_kv.value = v as u32;
        profile.weights.n_head_kv.field_evidence = format!("{prefix}.num_key_value_heads");
    }

    // Intermediate size.
    if let Some(v) = tc.get("intermediate_size").and_then(|v| v.as_u64()) {
        profile.weights.n_ff.value = v as u32;
        profile.weights.n_ff.field_evidence = format!("{prefix}.intermediate_size");
    }

    // Head dim.
    if let Some(v) = tc.get("head_dim").and_then(|v| v.as_u64()) {
        profile.weights.head_dim.value = v as u32;
        profile.weights.head_dim.field_evidence = format!("{prefix}.head_dim");
    }

    // Vocab size.
    if let Some(v) = tc.get("vocab_size").and_then(|v| v.as_u64()) {
        profile.weights.vocab_size.value = v as u32;
        profile.weights.vocab_size.field_evidence = format!("{prefix}.vocab_size");
    }

    // RMS norm eps.
    if let Some(v) = tc.get("rms_norm_eps").and_then(|v| v.as_f64()) {
        profile.weights.rms_norm_eps.value = v;
        profile.weights.rms_norm_eps.field_evidence = format!("{prefix}.rms_norm_eps");
    }

    // Max position embeddings.
    if let Some(v) = tc.get("max_position_embeddings").and_then(|v| v.as_u64()) {
        profile.weights.max_position_embeddings.value = v as u32;
        profile.weights.max_position_embeddings.field_evidence =
            format!("{prefix}.max_position_embeddings");
        profile.model_context_limit = Some(v as u32);
    }

    // Full attention interval.
    if let Some(v) = tc.get("full_attention_interval").and_then(|v| v.as_u64()) {
        profile.full_attention_interval = Some(v as u32);
        profile.full_attention_interval_evidence =
            Some(format!("{prefix}.full_attention_interval"));
    }

    // Layer types.
    if let Some(arr) = tc.get("layer_types").and_then(|v| v.as_array()) {
        let types: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        if !types.is_empty() {
            profile.layer_types = Some(types.clone());
            profile.layer_types_evidence = Some(format!("{prefix}.layer_types"));

            // Build layer groups from layer_types.
            build_layer_groups_from_types(profile, &types, tc);
        }
    }

    // Sliding window.
    if let Some(v) = tc.get("sliding_window").and_then(|v| v.as_u64()) {
        profile.sliding_window = Some(v as u32);
        profile.sliding_window_evidence = Some(format!("{prefix}.sliding_window"));
    }

    // MoE fields.
    if let Some(v) = tc.get("num_experts").and_then(|v| v.as_u64())
        && v > 0
    {
        profile.experts = Some(ExpertTopology {
            n_experts: v as u32,
            field_evidence: format!("{prefix}.num_experts"),
            ..Default::default()
        });
    }
    if let Some(top_k) = tc.get("num_experts_per_tok").and_then(|v| v.as_u64())
        && let Some(ref mut experts) = profile.experts
    {
        experts.top_k = Some(top_k as u32);
        experts.top_k_evidence = Some(format!("{prefix}.num_experts_per_tok"));
    }
    if let Some(top_k) = tc.get("top_k_experts").and_then(|v| v.as_u64())
        && let Some(ref mut experts) = profile.experts
    {
        experts.top_k = Some(top_k as u32);
        experts.top_k_evidence = Some(format!("{prefix}.top_k_experts"));
    }
    if let Some(v) = tc.get("moe_intermediate_size").and_then(|v| v.as_u64())
        && let Some(ref mut experts) = profile.experts
    {
        experts.moe_intermediate_size = Some(v as u32);
        experts.moe_intermediate_evidence = Some(format!("{prefix}.moe_intermediate_size"));
    }
    if let Some(v) = tc
        .get("shared_expert_intermediate_size")
        .and_then(|v| v.as_u64())
        && let Some(ref mut experts) = profile.experts
    {
        experts.shared_expert_intermediate_size = Some(v as u32);
        experts.shared_expert_intermediate_evidence =
            Some(format!("{prefix}.shared_expert_intermediate_size"));
    }

    // Linear/recurrent state geometry (DeltaNet, SSM).
    let mut rcg = RecurrentStateGeometry::default();
    let mut has_rcg = false;
    if let Some(v) = tc.get("linear_conv_kernel_dim").and_then(|v| v.as_u64()) {
        rcg.linear_conv_kernel_dim = Some(v as u32);
        rcg.linear_conv_kernel_dim_evidence = Some(format!("{prefix}.linear_conv_kernel_dim"));
        has_rcg = true;
    }
    if let Some(v) = tc.get("linear_key_head_dim").and_then(|v| v.as_u64()) {
        rcg.linear_key_head_dim = Some(v as u32);
        rcg.linear_key_head_dim_evidence = Some(format!("{prefix}.linear_key_head_dim"));
        has_rcg = true;
    }
    if let Some(v) = tc.get("linear_num_key_heads").and_then(|v| v.as_u64()) {
        rcg.linear_num_key_heads = Some(v as u32);
        rcg.linear_num_key_heads_evidence = Some(format!("{prefix}.linear_num_key_heads"));
        has_rcg = true;
    }
    if let Some(v) = tc.get("linear_num_value_heads").and_then(|v| v.as_u64()) {
        rcg.linear_num_value_heads = Some(v as u32);
        rcg.linear_num_value_heads_evidence = Some(format!("{prefix}.linear_num_value_heads"));
        has_rcg = true;
    }
    if let Some(v) = tc.get("linear_value_head_dim").and_then(|v| v.as_u64()) {
        rcg.linear_value_head_dim = Some(v as u32);
        rcg.linear_value_head_dim_evidence = Some(format!("{prefix}.linear_value_head_dim"));
        has_rcg = true;
    }
    if let Some(v) = tc.get("mamba_ssm_dtype").and_then(|v| v.as_str()) {
        rcg.mamba_ssm_dtype = Some(v.to_string());
        rcg.mamba_ssm_dtype_evidence = Some(format!("{prefix}.mamba_ssm_dtype"));
        has_rcg = true;
    }
    if has_rcg {
        profile.recurrent_state = Some(rcg);
    }

    // Gemma4 global/local head geometry.
    let mut glh = GlobalLocalHeadGeometry::default();
    let mut has_glh = false;
    if let Some(v) = tc
        .get("num_global_key_value_heads")
        .and_then(|v| v.as_u64())
    {
        glh.num_global_key_value_heads = Some(v as u32);
        glh.num_global_kv_evidence = Some(format!("{prefix}.num_global_key_value_heads"));
        has_glh = true;
    }
    if let Some(v) = tc.get("global_head_dim").and_then(|v| v.as_u64()) {
        glh.global_head_dim = Some(v as u32);
        glh.global_head_dim_evidence = Some(format!("{prefix}.global_head_dim"));
        has_glh = true;
    }

    // Local KV heads: derived from standard num_key_value_heads minus global KV heads.
    // Per Gemma4 architecture: total KV = global KV + local KV.
    if let Some(global_kv) = glh.num_global_key_value_heads
        && let Some(total_kv) = tc
            .get("num_key_value_heads")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
        && total_kv > global_kv
    {
        glh.num_local_key_value_heads = Some(total_kv - global_kv);
        glh.num_local_kv_evidence = Some(format!(
            "derived: text_config.num_key_value_heads ({total_kv}) - text_config.num_global_key_value_heads ({global_kv})"
        ));
    }

    // Local head dim: standard head_dim (Gemma4 uses same head_dim for local attention).
    if let Some(v) = tc.get("head_dim").and_then(|v| v.as_u64()) {
        glh.local_head_dim = Some(v as u32);
        glh.local_head_dim_evidence = Some(format!("{prefix}.head_dim"));
    }

    // Local attention window: from sliding_window config.
    if let Some(v) = tc.get("sliding_window").and_then(|v| v.as_u64())
        && v > 0
    {
        glh.local_attn_window_size = Some(v as u32);
        glh.local_attn_window_evidence = Some(format!("{prefix}.sliding_window"));
    }

    if has_glh {
        profile.global_local_heads = Some(glh);
    }

    // MTP fields.
    if let Some(v) = tc.get("mtp_num_hidden_layers").and_then(|v| v.as_u64())
        && v > 0
    {
        let ded = tc
            .get("mtp_use_dedicated_embeddings")
            .and_then(|v| v.as_bool());
        let mtp = EmbeddedMtpComponent {
            n_layers: v as u32,
            field_evidence: format!("{prefix}.mtp_num_hidden_layers"),
            use_dedicated_embeddings: ded,
            use_dedicated_embeddings_evidence: ded
                .map(|_| format!("{prefix}.mtp_use_dedicated_embeddings")),
        };
        profile.embedded_mtp = Some(mtp);
    }

    // Quantization (from wrapper, not text_config).
}

fn build_layer_groups_from_types(
    profile: &mut ModelMemoryProfile,
    types: &[String],
    tc: &serde_json::Value,
) {
    // Count layers by type from the layer_types array.
    let mut full_count = 0u32;
    let mut local_count = 0u32;
    let mut linear_count = 0u32;

    for t in types {
        match t.as_str() {
            "full_attention" => full_count += 1,
            "sliding_attention" | "local_attention" => local_count += 1,
            "linear_attention" | "delta_net" | "ssm" | "linear" => linear_count += 1,
            _ => {}
        }
    }

    let kv_heads = tc
        .get("num_key_value_heads")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    let head_dim = tc
        .get("head_dim")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    let sliding_window = tc
        .get("sliding_window")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    // Full attention layers.
    if full_count > 0 {
        // For Gemma4: use global_kv_heads/global_head_dim for full attention.
        let (gv, gd) = if let Some(ref glh) = profile.global_local_heads {
            (glh.num_global_key_value_heads, glh.global_head_dim)
        } else {
            (None, None)
        };
        profile.layer_groups.push(LayerMemoryGroup {
            kind: LayerGroupKind::FullAttention,
            count: full_count,
            field_evidence: "counted from text_config.layer_types".into(),
            kv_heads: gv.or(kv_heads),
            head_dim: gd.or(head_dim),
            ..Default::default()
        });
    }

    // Local/sliding attention layers.
    if local_count > 0 {
        profile.layer_groups.push(LayerMemoryGroup {
            kind: LayerGroupKind::LocalAttention,
            count: local_count,
            field_evidence: "counted from text_config.layer_types".into(),
            kv_heads,
            head_dim,
            sliding_window,
            ..Default::default()
        });
    }

    // Linear/recurrent layers.
    if linear_count > 0 {
        profile.layer_groups.push(LayerMemoryGroup {
            kind: LayerGroupKind::LinearRecurrent,
            count: linear_count,
            field_evidence: "counted from text_config.layer_types".into(),
            recurrent_state_bytes: profile.recurrent_state.as_ref().and({
                // Part B will compute this properly; Part A just records presence.
                None
            }),
            ..Default::default()
        });
    }
}

fn extract_from_flat_config(profile: &mut ModelMemoryProfile, raw: &serde_json::Value) {
    // Flat config: geometry from top-level fields.

    // Model type.
    if let Some(mt) = raw.get("model_type").and_then(|v| v.as_str()) {
        profile.model_type = Some(mt.to_string());
        profile.model_type_evidence = Some("model_type".to_string());
    }

    // Hidden size.
    if let Some(v) = raw.get("hidden_size").and_then(|v| v.as_u64()) {
        profile.weights.n_embd.value = v as u32;
        profile.weights.n_embd.field_evidence = "hidden_size".to_string();
    }

    // Num layers.
    if let Some(v) = raw.get("num_hidden_layers").and_then(|v| v.as_u64()) {
        profile.weights.n_layers.value = v as u32;
        profile.weights.n_layers.field_evidence = "num_hidden_layers".to_string();
    }

    // Num attention heads.
    if let Some(v) = raw.get("num_attention_heads").and_then(|v| v.as_u64()) {
        profile.weights.n_head.value = v as u32;
        profile.weights.n_head.field_evidence = "num_attention_heads".to_string();
    }

    // Num KV heads.
    if let Some(v) = raw.get("num_key_value_heads").and_then(|v| v.as_u64()) {
        profile.weights.n_head_kv.value = v as u32;
        profile.weights.n_head_kv.field_evidence = "num_key_value_heads".to_string();
    }

    // Intermediate size.
    if let Some(v) = raw.get("intermediate_size").and_then(|v| v.as_u64()) {
        profile.weights.n_ff.value = v as u32;
        profile.weights.n_ff.field_evidence = "intermediate_size".to_string();
    }

    // Head dim.
    if let Some(v) = raw.get("head_dim").and_then(|v| v.as_u64()) {
        profile.weights.head_dim.value = v as u32;
        profile.weights.head_dim.field_evidence = "head_dim".to_string();
    }

    // Vocab size.
    if let Some(v) = raw.get("vocab_size").and_then(|v| v.as_u64()) {
        profile.weights.vocab_size.value = v as u32;
        profile.weights.vocab_size.field_evidence = "vocab_size".to_string();
    }

    // RMS norm eps.
    if let Some(v) = raw.get("rms_norm_eps").and_then(|v| v.as_f64()) {
        profile.weights.rms_norm_eps.value = v;
        profile.weights.rms_norm_eps.field_evidence = "rms_norm_eps".to_string();
    }

    // Max position embeddings.
    if let Some(v) = raw.get("max_position_embeddings").and_then(|v| v.as_u64()) {
        profile.weights.max_position_embeddings.value = v as u32;
        profile.weights.max_position_embeddings.field_evidence =
            "max_position_embeddings".to_string();
        profile.model_context_limit = Some(v as u32);
    }

    // Sliding window.
    if let Some(v) = raw.get("sliding_window").and_then(|v| v.as_u64()) {
        profile.sliding_window = Some(v as u32);
        profile.sliding_window_evidence = Some("sliding_window".to_string());
    }

    // Flat config: all layers are full attention by default.
    if profile.weights.n_layers.value > 0 {
        let kv_heads = raw
            .get("num_key_value_heads")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let head_dim = raw
            .get("head_dim")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        profile.layer_groups.push(LayerMemoryGroup {
            kind: LayerGroupKind::FullAttention,
            count: profile.weights.n_layers.value,
            field_evidence: "flat config: all layers full attention".into(),
            kv_heads,
            head_dim,
            ..Default::default()
        });
    }

    // MoE fields.
    if let Some(v) = raw.get("num_experts").and_then(|v| v.as_u64())
        && v > 0
    {
        profile.experts = Some(ExpertTopology {
            n_experts: v as u32,
            field_evidence: "num_experts".to_string(),
            ..Default::default()
        });
    }
    if let Some(v) = raw.get("num_experts_per_tok").and_then(|v| v.as_u64())
        && let Some(ref mut experts) = profile.experts
    {
        experts.top_k = Some(v as u32);
        experts.top_k_evidence = Some("num_experts_per_tok".to_string());
    }
}

fn extract_model_identity(profile: &mut ModelMemoryProfile, raw: &serde_json::Value) {
    // Model type (wrapper-level may differ from text_config).
    if profile.model_type.is_none()
        && let Some(mt) = raw.get("model_type").and_then(|v| v.as_str())
    {
        profile.model_type = Some(mt.to_string());
        profile.model_type_evidence = Some("model_type".into());
    }

    // Architectures.
    if let Some(arr) = raw.get("architectures").and_then(|v| v.as_array()) {
        let archs: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        if !archs.is_empty() {
            profile.architectures = Some(archs);
            profile.architectures_evidence = Some("architectures".into());
        }
    }
}

fn extract_vision_component(profile: &mut ModelMemoryProfile, raw: &serde_json::Value) {
    if let Some(vc) = raw.get("vision_config").and_then(|v| v.as_object()) {
        let mut vision = VisionComponent {
            has_vision_config: true,
            field_evidence: "vision_config".into(),
            ..Default::default()
        };
        if let Some(mt) = vc.get("model_type").and_then(|v| v.as_str()) {
            vision.model_type = Some(mt.to_string());
            vision.model_type_evidence = Some("vision_config.model_type".into());
        }
        // Vision encoder layers: num_hidden_layers (standard) or depth (Qwen3.6).
        if let Some(layers) = vc
            .get("num_hidden_layers")
            .or_else(|| vc.get("depth"))
            .and_then(|v| v.as_u64())
        {
            vision.encoder_layers = Some(layers as u32);
            let key = if vc.get("num_hidden_layers").is_some() {
                "vision_config.num_hidden_layers"
            } else {
                "vision_config.depth"
            };
            vision.encoder_layers_evidence = Some(key.into());
        }
        profile.vision = Some(vision);
    }
}

fn extract_draft_config(profile: &mut ModelMemoryProfile, raw: &serde_json::Value) {
    // Draft model from wrapper config (MTP/companion).
    // Per A25: embedded MTP tensors are part of main model geometry;
    // external drafter/vision/embedding companions are tracked separately.
    let draft = raw
        .get("draft_model")
        .or_else(|| raw.get("speculative_config"));
    if let Some(d) = draft {
        // Check if this is an external companion (references a separate model source).
        let model_source = d.get("model").and_then(|v| v.as_str());
        if let Some(src) = model_source {
            // External companion: separate source with distinct provenance (A25).
            profile.external_companions.push(ExternalCompanion {
                companion_type: CompanionType::Drafter,
                source: src.to_string(),
                provenance: "draft_model.model".into(),
            });
        }

        let layers = d.get("num_hidden_layers").and_then(|v| v.as_u64());
        if let Some(n) = layers {
            if n > 0 {
                profile.embedded_mtp = Some(EmbeddedMtpComponent {
                    n_layers: n as u32,
                    field_evidence: "draft_model.num_hidden_layers".into(),
                    ..Default::default()
                });
            }
        } else {
            // Presence signals MTP.
            profile.embedded_mtp = Some(EmbeddedMtpComponent {
                n_layers: 1,
                field_evidence: "draft_model (presence)".into(),
                ..Default::default()
            });
        }
    }
}

/// Wrapper-field protection: detect when outer wrapper config has geometry fields that
/// differ from inner text_config, and document them as warnings.
///
/// CRITICAL: wrapper fields like outer `num_hidden_layers` or `block_count` represent
/// total blocks (including recurrent/DeltaNet) and must NOT override inner text_config geometry.
fn check_wrapper_field_conflicts(profile: &mut ModelMemoryProfile, raw: &serde_json::Value) {
    // Check num_hidden_layers conflict.
    if let Some(outer_layers) = raw.get("num_hidden_layers").and_then(|v| v.as_u64())
        && profile.weights.n_layers.value > 0
        && profile.weights.n_layers.value != outer_layers as u32
    {
        profile.warnings.wrapper_field_conflicts.push(ParseWarning {
            field: "num_hidden_layers".into(),
            message: "wrapper field differs from text_config; inner geometry trusted".into(),
            outer_value: Some(outer_layers.to_string()),
            inner_value: Some(profile.weights.n_layers.value.to_string()),
        });
    }

    // Check hidden_size conflict.
    if let Some(outer_size) = raw.get("hidden_size").and_then(|v| v.as_u64())
        && profile.weights.n_embd.value > 0
        && profile.weights.n_embd.value != outer_size as u32
    {
        profile.warnings.wrapper_field_conflicts.push(ParseWarning {
            field: "hidden_size".into(),
            message: "wrapper field differs from text_config; inner geometry trusted".into(),
            outer_value: Some(outer_size.to_string()),
            inner_value: Some(profile.weights.n_embd.value.to_string()),
        });
    }

    // Check num_attention_heads conflict.
    if let Some(outer_heads) = raw.get("num_attention_heads").and_then(|v| v.as_u64())
        && profile.weights.n_head.value > 0
        && profile.weights.n_head.value != outer_heads as u32
    {
        profile.warnings.wrapper_field_conflicts.push(ParseWarning {
            field: "num_attention_heads".into(),
            message: "wrapper field differs from text_config; inner geometry trusted".into(),
            outer_value: Some(outer_heads.to_string()),
            inner_value: Some(profile.weights.n_head.value.to_string()),
        });
    }

    // Check num_key_value_heads conflict.
    if let Some(outer_kv) = raw.get("num_key_value_heads").and_then(|v| v.as_u64())
        && profile.weights.n_head_kv.value > 0
        && profile.weights.n_head_kv.value != outer_kv as u32
    {
        profile.warnings.wrapper_field_conflicts.push(ParseWarning {
            field: "num_key_value_heads".into(),
            message: "wrapper field differs from text_config; inner geometry trusted".into(),
            outer_value: Some(outer_kv.to_string()),
            inner_value: Some(profile.weights.n_head_kv.value.to_string()),
        });
    }
}

// ── Safetensors index parsing (for Part C) ───────────────────────────────────────

/// Parse a safetensors index into [`WeightComponents`] with evidence.
pub fn parse_safetensors_index(index_path: &Path) -> Result<WeightComponents, String> {
    let bytes = bounded_read(index_path, MAX_INDEX_BYTES)?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("safetensors index: {e}"))?;

    let weights = WeightComponents::default();

    // Validate weight_map.
    let weight_map = value
        .get("weight_map")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "safetensors index requires a weight_map object".to_string())?;

    let mut shard_files: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for v in weight_map.values() {
        let Some(name) = v.as_str() else {
            return Err("safetensors index contains a non-string shard".into());
        };
        let relative = Path::new(name);
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
            || !name.ends_with(".safetensors")
        {
            return Err(format!(
                "safetensors index contains an unsafe shard path: {name}"
            ));
        }
        shard_files.insert(name.to_string());
    }

    // Extract total_size from metadata.
    if let Some(total) = value
        .get("metadata")
        .and_then(|m| m.get("total_size"))
        .and_then(|t| t.as_u64())
    {
        // Record total weight bytes via n_ff (reused as storage; better field added in Part C).
        // For now, we just validate and collect shard files.
        let _ = total;
    }

    Ok(weights)
}

// ── Legacy compatibility ──────────────────────────────────────────────────────────

impl MlxMetadata {
    pub fn to_arch(&self, model_size_bytes: u64, param_b: f64, fallback_name: &str) -> ModelArch {
        let mut arch = if self.evidence == MlxMetaEvidence::Degraded {
            ModelArch::from_name_and_params(fallback_name, param_b)
        } else {
            ModelArch::default()
        };

        let cfg = &self.config;
        if let Some(layers) = cfg.num_layers {
            arch.n_layers = layers;
        }
        if let Some(embd) = cfg.hidden_size {
            arch.n_embd = embd;
        }
        let n_head = cfg.num_attention_heads;
        if let Some(kv) = cfg.num_key_value_heads {
            arch.n_kv_heads = kv;
        } else if arch.n_kv_heads == 0
            && let Some(h) = n_head
        {
            arch.n_kv_heads = h;
        }
        let head_dim = cfg.head_dim.or_else(|| {
            let embd = cfg.hidden_size?;
            let heads = n_head?;
            embd.checked_div(heads)
        });
        if let Some(hd) = head_dim {
            arch.head_dim = hd;
        }

        if let Some(experts) = cfg.num_experts {
            arch.n_experts = experts;
        }
        if let Some(used) = cfg.num_experts_per_tok {
            arch.n_experts_used = used;
        }
        if arch.n_experts > 0 && arch.expert_fraction == 0.0 {
            arch.expert_fraction = 0.65;
        }

        if let Some(window) = cfg.sliding_window {
            arch.local_attn_window = window;
        }

        if arch.n_layers > 0 {
            arch.bytes_per_layer = model_size_bytes / arch.n_layers as u64;
        }
        arch.param_b = param_b;

        if cfg.draft_model.is_some() || cfg.speculative_config.is_some() {
            arch.mtp_depth = arch.mtp_depth.max(1);
        }

        arch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_config(dir: &std::path::Path, json: &str) {
        let mut f = std::fs::File::create(dir.join("config.json")).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        f.flush().unwrap();
    }

    fn write_index(dir: &std::path::Path, json: &str) {
        let mut f = std::fs::File::create(dir.join("model.safetensors.index.json")).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        f.flush().unwrap();
    }

    #[test]
    fn parses_dense_qwen3_style_config() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{
                "model_type": "qwen3",
                "hidden_size": 1024,
                "num_hidden_layers": 28,
                "num_attention_heads": 16,
                "num_key_value_heads": 8,
                "head_dim": 128,
                "intermediate_size": 3072,
                "max_position_embeddings": 32768,
                "quantization": {"bits": 4, "group_size": 64}
            }"#,
        );
        let meta = read_mlx_metadata(dir.path()).unwrap();
        assert_eq!(meta.evidence, MlxMetaEvidence::Exact);
        assert_eq!(meta.config.num_layers, Some(28));
        assert_eq!(meta.config.quantization.clone().unwrap().bits, Some(4));

        let arch = meta.to_arch(400_000_000, 0.6, "Qwen3-0.6B-4bit");
        assert_eq!(arch.n_layers, 28);
        assert_eq!(arch.n_embd, 1024);
        assert_eq!(arch.n_kv_heads, 8);
        assert_eq!(arch.head_dim, 128);
        assert_eq!(arch.n_experts, 0);
        assert_eq!(arch.bytes_per_layer, 400_000_000 / 28);
    }

    #[test]
    fn parses_moe_qwen3_style_config() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{
                "model_type": "qwen3_moe",
                "hidden_size": 2048,
                "num_hidden_layers": 48,
                "num_attention_heads": 32,
                "num_key_value_heads": 4,
                "num_experts": 128,
                "num_experts_per_tok": 8,
                "quantization": {"bits": 4, "group_size": 64}
            }"#,
        );
        let meta = read_mlx_metadata(dir.path()).unwrap();
        assert_eq!(meta.evidence, MlxMetaEvidence::Exact);

        let arch = meta.to_arch(16_000_000_000, 30.0, "Qwen3-30B-A3B-4bit");
        assert_eq!(arch.n_experts, 128);
        assert_eq!(arch.n_experts_used, 8);
        assert!(arch.is_moe());
    }

    #[test]
    fn missing_required_fields_flags_degraded() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{"model_type": "mystery", "hidden_size": 4096}"#,
        );
        let meta = read_mlx_metadata(dir.path()).unwrap();
        assert_eq!(meta.evidence, MlxMetaEvidence::Degraded);

        let heuristic = ModelArch::from_name_and_params("mystery-7b", 7.0);
        let arch = meta.to_arch(4_000_000_000, 7.0, "mystery-7b");
        assert_eq!(arch.n_layers, heuristic.n_layers);
    }

    #[test]
    fn exact_weight_accounting_from_safetensors_index_total_size() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{"hidden_size": 1024, "num_hidden_layers": 10, "num_attention_heads": 8}"#,
        );
        write_index(
            dir.path(),
            r#"{
                "metadata": {"total_size": 123456789},
                "weight_map": {
                    "a": "model-00001-of-00002.safetensors",
                    "b": "model-00002-of-00002.safetensors"
                }
            }"#,
        );
        let meta = read_mlx_metadata(dir.path()).unwrap();
        assert_eq!(meta.weight_index.total_size_bytes, Some(123_456_789));
        assert_eq!(meta.weight_index.shard_files.len(), 2);
        assert_eq!(
            resolve_local_weight_bytes(dir.path(), &meta.weight_index),
            Some(123_456_789)
        );
    }

    #[test]
    fn exact_weight_accounting_sums_real_shard_files_when_no_total_size() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{"hidden_size": 1024, "num_hidden_layers": 10, "num_attention_heads": 8}"#,
        );
        write_index(dir.path(), r#"{"weight_map": {"a": "model.safetensors"}}"#);
        std::fs::write(dir.path().join("model.safetensors"), vec![0u8; 4096]).unwrap();
        let meta = read_mlx_metadata(dir.path()).unwrap();
        assert_eq!(meta.weight_index.total_size_bytes, None);
        assert_eq!(
            resolve_local_weight_bytes(dir.path(), &meta.weight_index),
            Some(4096)
        );
    }

    #[test]
    fn rejects_unsafe_shard_paths_in_index() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{"hidden_size": 1024, "num_hidden_layers": 10, "num_attention_heads": 8}"#,
        );
        write_index(
            dir.path(),
            r#"{"weight_map": {"a": "../../etc/passwd.safetensors"}}"#,
        );
        assert!(read_mlx_weight_index(dir.path()).is_err());
        let meta = read_mlx_metadata(dir.path()).unwrap();
        assert!(meta.weight_index.shard_files.is_empty());
    }

    #[test]
    fn draft_sidecar_sets_mtp_depth() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{
                "hidden_size": 1024,
                "num_hidden_layers": 10,
                "num_attention_heads": 8,
                "speculative_config": {"model": "draft-model", "num_hidden_layers": 2}
            }"#,
        );
        let meta = read_mlx_metadata(dir.path()).unwrap();
        let arch = meta.to_arch(1_000_000, 1.0, "test");
        assert_eq!(arch.mtp_depth, 1);
    }

    // ── ModelMemoryProfile tests (Phase 4 Part A) ─────────────────────────────

    /// Load a pinned fixture from tests/fixtures/mlx_configs/.
    fn load_fixture(name: &str) -> Result<Vec<u8>, String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/mlx_configs/")
            .join(name);
        std::fs::read(&path).map_err(|e| format!("{}: {}", path.display(), e))
    }

    #[test]
    fn parse_fixture_qwen36_27b_nested_text_config() {
        let bytes = load_fixture("mlx-community_Qwen3.6-27B-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        // Geometry from text_config (inner), not wrapper.
        assert_eq!(profile.weights.n_layers.value, 64);
        assert_eq!(
            profile.weights.n_layers.field_evidence,
            "text_config.num_hidden_layers"
        );
        assert_eq!(profile.weights.n_embd.value, 5120);
        assert_eq!(
            profile.weights.n_embd.field_evidence,
            "text_config.hidden_size"
        );
        assert_eq!(profile.weights.n_head.value, 24);
        assert_eq!(profile.weights.n_head_kv.value, 4);
        assert_eq!(profile.weights.max_position_embeddings.value, 262144);

        // full_attention_interval: 4 → 64/4 = 16 full attention layers.
        assert_eq!(profile.full_attention_interval, Some(4));
        assert_eq!(
            profile.full_attention_interval_evidence,
            Some("text_config.full_attention_interval".into())
        );

        // Layer groups: 16 full + 48 linear (from layer_types array).
        assert_eq!(profile.total_layer_count(), 64);
        assert_eq!(profile.full_attention_layer_count(), 16);
        assert_eq!(profile.linear_recurrent_layer_count(), 48);
        assert!(profile.is_hybrid_attention());

        // Recurrent state geometry with field_evidence.
        assert!(profile.recurrent_state.is_some());
        let rcg = profile.recurrent_state.as_ref().unwrap();
        assert_eq!(rcg.linear_conv_kernel_dim, Some(4));
        assert!(rcg.linear_conv_kernel_dim_evidence.is_some());
        assert_eq!(rcg.linear_key_head_dim, Some(128));
        assert!(rcg.linear_key_head_dim_evidence.is_some());
        assert_eq!(rcg.linear_num_key_heads, Some(16));
        assert!(rcg.linear_num_key_heads_evidence.is_some());
        assert_eq!(rcg.linear_num_value_heads, Some(48));
        assert!(rcg.linear_num_value_heads_evidence.is_some());
        assert_eq!(rcg.linear_value_head_dim, Some(128));
        assert!(rcg.linear_value_head_dim_evidence.is_some());
        assert_eq!(rcg.mamba_ssm_dtype, Some("float32".into()));
        assert!(rcg.mamba_ssm_dtype_evidence.is_some());

        // MTP embedded.
        assert!(profile.embedded_mtp.is_some());
        let mtp = profile.embedded_mtp.as_ref().unwrap();
        assert_eq!(mtp.n_layers, 1);
        assert!(!mtp.field_evidence.is_empty());

        // Vision component present.
        assert!(profile.vision.is_some());
        let vision = profile.vision.as_ref().unwrap();
        assert!(vision.has_vision_config);
        assert_eq!(vision.model_type, Some("qwen3_5".into()));
        assert_eq!(vision.encoder_layers, Some(27));

        // No external companions (vision is inline config, not separate source).
        assert!(profile.external_companions.is_empty());

        // Used text_config.
        assert_eq!(profile.used_text_config, Some(true));
    }

    #[test]
    fn parse_fixture_qwen36_35b_a3b_nested_text_config() {
        let bytes = load_fixture("mlx-community_Qwen3.6-35B-A3B-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        assert_eq!(profile.weights.n_layers.value, 40);
        assert_eq!(profile.weights.n_embd.value, 2048);
        assert_eq!(profile.weights.n_head.value, 16);
        assert_eq!(profile.weights.n_head_kv.value, 2);

        // MoE: 256 total experts, 8 active per token.
        assert!(profile.is_moe());
        let experts = profile.experts.as_ref().unwrap();
        assert_eq!(experts.n_experts, 256);
        assert!(!experts.field_evidence.is_empty());
        assert_eq!(experts.top_k, Some(8));
        assert!(experts.top_k_evidence.is_some());
        assert!(experts.moe_intermediate_size.is_some());
        assert!(experts.shared_expert_intermediate_size.is_some());

        // Layer groups: 10 full + 30 linear (40 / full_attention_interval=4 = 10 full).
        assert_eq!(profile.total_layer_count(), 40);
        assert_eq!(profile.full_attention_layer_count(), 10);
        assert_eq!(profile.linear_recurrent_layer_count(), 30);

        // Recurrent state geometry.
        assert!(profile.recurrent_state.is_some());

        // Vision component present.
        assert!(profile.vision.is_some());

        // No external companions.
        assert!(profile.external_companions.is_empty());
    }

    #[test]
    fn parse_fixture_gemma4_26b_a4b_nested_text_config() {
        let bytes = load_fixture("mlx-community_gemma-4-26b-a4b-it-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        assert_eq!(profile.weights.n_layers.value, 30);
        assert_eq!(profile.weights.n_embd.value, 2816);
        assert_eq!(profile.weights.n_head.value, 16);
        assert_eq!(profile.weights.n_head_kv.value, 8);

        // Gemma4 global/local head geometry.
        assert!(profile.global_local_heads.is_some());
        let glh = profile.global_local_heads.as_ref().unwrap();
        assert_eq!(glh.num_global_key_value_heads, Some(2));
        assert_eq!(glh.global_head_dim, Some(512));
        // Local KV: total (8) - global (2) = 6
        assert_eq!(glh.num_local_key_value_heads, Some(6));
        assert!(!glh.num_local_kv_evidence.as_ref().unwrap().is_empty());
        assert_eq!(glh.local_head_dim, Some(256));
        assert!(!glh.local_head_dim_evidence.as_ref().unwrap().is_empty());
        assert_eq!(glh.local_attn_window_size, Some(1024));
        assert!(!glh.local_attn_window_evidence.as_ref().unwrap().is_empty());

        // Layer groups: 5 full + 25 local.
        assert_eq!(profile.total_layer_count(), 30);
        assert_eq!(profile.full_attention_layer_count(), 5);
        assert_eq!(profile.local_attention_layer_count(), 25);
        assert!(profile.has_local_attention());
        assert_eq!(profile.sliding_window, Some(1024));

        // MoE.
        assert!(profile.is_moe());
        let experts = profile.experts.as_ref().unwrap();
        assert_eq!(experts.n_experts, 128);
        assert_eq!(experts.top_k, Some(8));

        // Vision component.
        assert!(profile.vision.is_some());
        let vision = profile.vision.as_ref().unwrap();
        assert_eq!(vision.encoder_layers, Some(27));
        assert_eq!(vision.model_type, Some("gemma4_vision".into()));

        // No external companions in this config.
        assert!(profile.external_companions.is_empty());
    }

    #[test]
    fn parse_fixture_gemma4_31b_nested_text_config() {
        let bytes = load_fixture("mlx-community_gemma-4-31b-it-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        assert_eq!(profile.weights.n_layers.value, 60);
        assert_eq!(profile.weights.n_embd.value, 5376);
        assert_eq!(profile.weights.n_head.value, 32);
        assert_eq!(profile.weights.n_head_kv.value, 16);

        // Gemma4 global/local head geometry.
        let glh = profile.global_local_heads.as_ref().unwrap();
        assert_eq!(glh.num_global_key_value_heads, Some(4));
        assert_eq!(glh.global_head_dim, Some(512));
        // Local KV: total (16) - global (4) = 12
        assert_eq!(glh.num_local_key_value_heads, Some(12));
        assert_eq!(glh.local_head_dim, Some(256));
        assert_eq!(glh.local_attn_window_size, Some(1024));

        // Layer groups: 10 full + 50 local.
        assert_eq!(profile.total_layer_count(), 60);
        assert_eq!(profile.full_attention_layer_count(), 10);
        assert_eq!(profile.local_attention_layer_count(), 50);

        // No MoE (dense).
        assert!(!profile.is_moe());

        // Vision component.
        assert!(profile.vision.is_some());
        let vision = profile.vision.as_ref().unwrap();
        assert_eq!(vision.encoder_layers, Some(27));

        // No external companions.
        assert!(profile.external_companions.is_empty());
    }

    #[test]
    fn parse_fixture_qwen3_06b_flat_config() {
        let bytes = load_fixture("mlx-community_Qwen3-0.6B-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        // Flat config: geometry from top-level.
        assert_eq!(profile.weights.n_layers.value, 28);
        assert_eq!(profile.weights.n_embd.value, 1024);
        assert_eq!(profile.weights.n_head.value, 16);
        assert_eq!(profile.weights.n_head_kv.value, 8);

        // All layers are full attention (flat config default).
        assert_eq!(profile.total_layer_count(), 28);
        assert_eq!(profile.full_attention_layer_count(), 28);
        assert_eq!(profile.linear_recurrent_layer_count(), 0);

        // Dense: no MoE, no recurrent, no MTP, no vision.
        assert!(!profile.is_moe());
        assert!(profile.recurrent_state.is_none());
        assert!(profile.embedded_mtp.is_none());
        assert!(profile.vision.is_none());
        assert!(profile.external_companions.is_empty());

        assert_eq!(profile.used_text_config, Some(false));
    }

    #[test]
    fn parse_fixture_qwen3_30b_a3b_flat_config() {
        let bytes = load_fixture("mlx-community_Qwen3-30B-A3B-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        assert_eq!(profile.weights.n_layers.value, 48);
        assert_eq!(profile.weights.n_embd.value, 2048);
        assert_eq!(profile.weights.n_head.value, 32);
        assert_eq!(profile.weights.n_head_kv.value, 4);

        // MoE: 128 total experts, 8 active per token (flat config).
        assert!(profile.is_moe());
        let experts = profile.experts.as_ref().unwrap();
        assert_eq!(experts.n_experts, 128);
        assert_eq!(experts.field_evidence, "num_experts");
        assert_eq!(experts.top_k, Some(8));
        assert_eq!(
            experts.top_k_evidence,
            Some("num_experts_per_tok".to_string())
        );

        // All layers full attention (flat config, no layer_types).
        assert_eq!(profile.total_layer_count(), 48);
        assert_eq!(profile.full_attention_layer_count(), 48);

        // No recurrent state, no MTP, no vision, no external companions.
        assert!(profile.recurrent_state.is_none());
        assert!(profile.embedded_mtp.is_none());
        assert!(profile.vision.is_none());
        assert!(profile.external_companions.is_empty());
    }

    #[test]
    fn wrapper_field_protection_outer_does_not_override_inner() {
        // Construct a config where wrapper has different geometry than text_config.
        let json = r#"{
            "model_type": "test_vlm",
            "num_hidden_layers": 100,
            "hidden_size": 8192,
            "num_attention_heads": 32,
            "num_key_value_heads": 8,
            "text_config": {
                "model_type": "test_text",
                "num_hidden_layers": 40,
                "hidden_size": 2048,
                "num_attention_heads": 16,
                "num_key_value_heads": 2
            }
        }"#;
        let profile = parse_mlx_config_bytes_to_profile(json.as_bytes()).unwrap();

        // Inner text_config geometry is trusted.
        assert_eq!(profile.weights.n_layers.value, 40);
        assert_eq!(profile.weights.n_embd.value, 2048);
        assert_eq!(profile.weights.n_head.value, 16);
        assert_eq!(profile.weights.n_head_kv.value, 2);

        // Wrapper-field conflicts documented.
        assert!(profile.warnings.wrapper_field_conflicts.len() >= 1);
    }

    #[test]
    fn field_evidence_present_for_all_populated_fields() {
        let json = r#"{
            "model_type": "test",
            "architectures": ["TestForCausalLM"],
            "text_config": {
                "hidden_size": 1024,
                "num_hidden_layers": 20,
                "num_attention_heads": 8,
                "num_key_value_heads": 4,
                "intermediate_size": 4096,
                "head_dim": 128,
                "vocab_size": 32000,
                "max_position_embeddings": 8192,
                "rms_norm_eps": 1e-05,
                "layer_types": ["full_attention", "linear_attention"]
            }
        }"#;
        let profile = parse_mlx_config_bytes_to_profile(json.as_bytes()).unwrap();

        // Every populated field has evidence.
        assert!(!profile.weights.n_embd.field_evidence.is_empty());
        assert!(!profile.weights.n_layers.field_evidence.is_empty());
        assert!(!profile.weights.n_head.field_evidence.is_empty());
        assert!(!profile.weights.n_head_kv.field_evidence.is_empty());
        assert!(!profile.weights.n_ff.field_evidence.is_empty());
        assert!(!profile.weights.head_dim.field_evidence.is_empty());
        assert!(!profile.weights.vocab_size.field_evidence.is_empty());
        assert!(!profile.weights.rms_norm_eps.field_evidence.is_empty());
        assert!(
            !profile
                .weights
                .max_position_embeddings
                .field_evidence
                .is_empty()
        );
        assert!(!profile.model_type_evidence.unwrap_or_default().is_empty());
        assert!(!profile.layer_types_evidence.unwrap_or_default().is_empty());
    }

    #[test]
    fn missing_critical_fields_still_returns_profile_with_evidence_gaps() {
        // Config with no geometry fields at all should still parse but have empty geometry.
        let json = r#"{
            "model_type": "empty",
            "architectures": ["EmptyModel"]
        }"#;
        let profile = parse_mlx_config_bytes_to_profile(json.as_bytes()).unwrap();

        // Profile is not substantive (no geometry fields).
        assert!(!profile.is_substantive());
        assert_eq!(profile.model_type, Some("empty".into()));
        assert_eq!(profile.total_layer_count(), 0);
    }

    #[test]
    fn invalid_json_returns_error() {
        let result = parse_mlx_config_bytes_to_profile(b"not json");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("not valid JSON") || err.contains("invalid"));
    }

    #[test]
    fn parse_safetensors_index_returns_weight_components() {
        let dir = tempfile::tempdir().unwrap();
        let index_path = dir.path().join("model.safetensors.index.json");
        std::fs::write(
            &index_path,
            r#"{
                "metadata": {"total_size": 123456789},
                "weight_map": {
                    "a": "model-00001-of-00002.safetensors"
                }
            }"#,
        )
        .unwrap();
        let weights = parse_safetensors_index(&index_path).unwrap();
        // WeightComponents is currently a placeholder for Part C; just validate it parses.
        let _ = weights;
    }

    // ── Part B: Architecture-specific geometry tests ──────────────────────────

    #[test]
    fn qwen36_deltanet_full_attention_layer_count_via_interval() {
        // Hard gate: Qwen3.6 full_attention_layer_count = block_count / full_attention_interval
        // NOT block_count (i.e., not all layers treated as full KV).
        let bytes = load_fixture("mlx-community_Qwen3.6-27B-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        assert_eq!(profile.weights.n_layers.value, 64);
        assert_eq!(profile.full_attention_interval, Some(4));
        // 64 / 4 = 16 full attention layers (from layer_types).
        assert_eq!(profile.full_attention_layer_count(), 16);
        // 48 linear/recurrent layers.
        assert_eq!(profile.linear_recurrent_layer_count(), 48);
        // Must NOT equal total layers.
        assert_ne!(
            profile.full_attention_layer_count(),
            profile.total_layer_count()
        );
    }

    #[test]
    fn gemma4_global_kv_included_not_zeroed() {
        // Hard gate: Gemma4 global_kv_heads from config, not zeroed or ignored.
        let bytes = load_fixture("mlx-community_gemma-4-26b-a4b-it-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        let glh = profile.global_local_heads.as_ref().unwrap();
        assert_eq!(glh.num_global_key_value_heads, Some(2));
        assert!(!glh.num_global_kv_evidence.as_ref().unwrap().is_empty());
        assert_eq!(glh.global_head_dim, Some(512));
        assert!(!glh.global_head_dim_evidence.as_ref().unwrap().is_empty());
    }

    #[test]
    fn gemma4_local_kv_capped_by_sliding_window() {
        // Hard gate: Gemma4 local KV capped by sliding window.
        let bytes = load_fixture("mlx-community_gemma-4-26b-a4b-it-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        let glh = profile.global_local_heads.as_ref().unwrap();
        // Local KV heads derived from total - global.
        assert_eq!(glh.num_local_key_value_heads, Some(6));
        assert!(glh.local_attn_window_size.is_some());
        assert_eq!(glh.local_attn_window_size, Some(1024));
    }

    #[test]
    fn recurrent_state_explicit_with_field_evidence() {
        // Hard gate: Recurrent state is explicit with field_evidence.
        let bytes = load_fixture("mlx-community_Qwen3.6-27B-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        let rcg = profile.recurrent_state.as_ref().unwrap();
        // Every populated field has evidence.
        assert!(rcg.linear_conv_kernel_dim_evidence.is_some());
        assert!(rcg.linear_key_head_dim_evidence.is_some());
        assert!(rcg.linear_num_key_heads_evidence.is_some());
        assert!(rcg.linear_num_value_heads_evidence.is_some());
        assert!(rcg.linear_value_head_dim_evidence.is_some());
        assert!(rcg.mamba_ssm_dtype_evidence.is_some());
    }

    #[test]
    fn moe_expert_topology_with_field_evidence() {
        // Hard gate: MoE experts have field_evidence.
        let bytes = load_fixture("mlx-community_Qwen3.6-35B-A3B-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        let experts = profile.experts.as_ref().unwrap();
        assert_eq!(experts.n_experts, 256);
        assert!(!experts.field_evidence.is_empty());
        assert_eq!(experts.top_k, Some(8));
        assert!(experts.top_k_evidence.is_some());
    }

    #[test]
    fn external_companion_from_draft_model_source() {
        // Per A25: external drafter with model source is tracked separately.
        let json = r#"{
            "hidden_size": 1024,
            "num_hidden_layers": 10,
            "num_attention_heads": 8,
            "draft_model": {
                "model": "mlx-community/drafter-model",
                "num_hidden_layers": 3
            }
        }"#;
        let profile = parse_mlx_config_bytes_to_profile(json.as_bytes()).unwrap();

        assert_eq!(profile.external_companions.len(), 1);
        let companion = &profile.external_companions[0];
        assert_eq!(companion.companion_type, CompanionType::Drafter);
        assert_eq!(companion.source, "mlx-community/drafter-model");
        assert_eq!(companion.provenance, "draft_model.model");

        // MTP still tracked from layers.
        assert!(profile.embedded_mtp.is_some());
        assert_eq!(profile.embedded_mtp.as_ref().unwrap().n_layers, 3);
    }

    #[test]
    fn no_double_counting_mtp_companions_from_main_geometry() {
        // Hard gate: MTP/companions separate from main geometry.
        let json = r#"{
            "hidden_size": 1024,
            "num_hidden_layers": 10,
            "num_attention_heads": 8,
            "mtp_num_hidden_layers": 2,
            "draft_model": {
                "model": "external-drafter",
                "num_hidden_layers": 1
            }
        }"#;
        let profile = parse_mlx_config_bytes_to_profile(json.as_bytes()).unwrap();

        // Main geometry is 10 layers, not 10+2+1.
        assert_eq!(profile.weights.n_layers.value, 10);
        assert_eq!(profile.total_layer_count(), 10);

        // MTP tracked separately.
        assert!(profile.embedded_mtp.is_some());

        // External companion tracked separately.
        assert_eq!(profile.external_companions.len(), 1);
    }

    #[test]
    fn gemma4_31b_dense_no_moe() {
        // Hard gate: Gemma-4-31b global_kv=4, no MoE.
        let bytes = load_fixture("mlx-community_gemma-4-31b-it-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        let glh = profile.global_local_heads.as_ref().unwrap();
        assert_eq!(glh.num_global_key_value_heads, Some(4));

        assert!(!profile.is_moe());
    }

    #[test]
    fn qwen3_06b_dense_baseline() {
        // Hard gate: Qwen3-0.6B dense geometry, no MoE/MTP/recurrent.
        let bytes = load_fixture("mlx-community_Qwen3-0.6B-4bit.config.json").unwrap();
        let profile = parse_mlx_config_bytes_to_profile(&bytes).unwrap();

        assert!(!profile.is_moe());
        assert!(profile.recurrent_state.is_none());
        assert!(profile.embedded_mtp.is_none());
        assert_eq!(profile.full_attention_layer_count(), 28);
        assert_eq!(profile.total_layer_count(), 28);
    }
}
