use anyhow::Result;
use std::path::Path;

fn null_as_zero_u32<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    use serde::Deserialize;
    Ok(Option::<u32>::deserialize(d)?.unwrap_or(0))
}
fn null_as_zero_u64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    use serde::Deserialize;
    Ok(Option::<u64>::deserialize(d)?.unwrap_or(0))
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ModelPreset {
    #[serde(default = "next_id")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub model_path: String,
    #[serde(default, deserialize_with = "null_as_zero_u64")]
    pub context_size: u64,
    #[serde(default)]
    pub ctk: String,
    #[serde(default)]
    pub ctv: String,
    #[serde(default)]
    pub tensor_split: String,
    #[serde(default, deserialize_with = "null_as_zero_u32")]
    pub batch_size: u32,
    #[serde(default, deserialize_with = "null_as_zero_u32")]
    pub ubatch_size: u32,
    #[serde(default)]
    pub no_mmap: bool,
    #[serde(default)]
    pub ngram_spec: bool,
    #[serde(default, deserialize_with = "null_as_zero_u32")]
    pub parallel_slots: u32,
    // Generation
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub top_k: Option<i32>,
    #[serde(default)]
    pub min_p: Option<f64>,
    #[serde(default)]
    pub repeat_penalty: Option<f64>,
    #[serde(default)]
    pub presence_penalty: Option<f64>,
    // CPU MOE
    #[serde(default)]
    pub n_cpu_moe: Option<i32>,
    // Model & memory
    #[serde(default)]
    pub gpu_layers: Option<i32>,
    #[serde(default)]
    pub mlock: bool,
    // Attention
    #[serde(default)]
    pub flash_attn: String,
    // GPU distribution
    #[serde(default)]
    pub split_mode: String,
    #[serde(default)]
    pub main_gpu: Option<u32>,
    // Threading
    #[serde(default)]
    pub threads: Option<i32>,
    #[serde(default)]
    pub threads_batch: Option<i32>,
    // Priority
    #[serde(default)]
    pub prio: Option<i32>,
    #[serde(default)]
    pub prio_batch: Option<i32>,
    // Rope scaling (override auto-YaRN)
    #[serde(default)]
    pub rope_scaling: String,
    #[serde(default)]
    pub rope_freq_base: Option<f64>,
    #[serde(default)]
    pub rope_freq_scale: Option<f64>,
    // Speculative decoding (granular)
    #[serde(default)]
    pub draft_model: String,
    #[serde(default)]
    pub draft_min: Option<u32>,
    #[serde(default)]
    pub draft_max: Option<u32>,
    #[serde(default)]
    pub spec_ngram_size: Option<u32>,
    // Spec V2: full spec-type and per-type knobs
    #[serde(default)]
    pub spec_type: Option<String>,
    #[serde(default)]
    pub spec_default: bool,
    #[serde(default)]
    pub spec_draft_n_max: Option<u32>,
    #[serde(default)]
    pub spec_draft_n_min: Option<u32>,
    #[serde(default)]
    pub spec_draft_p_split: Option<f32>,
    #[serde(default)]
    pub spec_draft_p_min: Option<f32>,
    #[serde(default)]
    pub spec_draft_ngl: Option<i32>,
    #[serde(default)]
    pub spec_draft_device: Option<String>,
    #[serde(default)]
    pub spec_draft_cpu_moe: bool,
    #[serde(default)]
    pub spec_draft_n_cpu_moe: Option<i32>,
    #[serde(default)]
    pub spec_draft_type_k: Option<String>,
    #[serde(default)]
    pub spec_draft_type_v: Option<String>,
    // ngram-mod knobs
    #[serde(default)]
    pub spec_ngram_mod_n_min: Option<u32>,
    #[serde(default)]
    pub spec_ngram_mod_n_max: Option<u32>,
    #[serde(default)]
    pub spec_ngram_mod_n_match: Option<u32>,
    // ngram-simple knobs
    #[serde(default)]
    pub spec_ngram_simple_size_n: Option<u32>,
    #[serde(default)]
    pub spec_ngram_simple_size_m: Option<u32>,
    #[serde(default)]
    pub spec_ngram_simple_min_hits: Option<u32>,
    // ngram-map-k knobs
    #[serde(default)]
    pub spec_ngram_map_k_size_n: Option<u32>,
    #[serde(default)]
    pub spec_ngram_map_k_size_m: Option<u32>,
    #[serde(default)]
    pub spec_ngram_map_k_min_hits: Option<u32>,
    // ngram-map-k4v knobs
    #[serde(default)]
    pub spec_ngram_map_k4v_size_n: Option<u32>,
    #[serde(default)]
    pub spec_ngram_map_k4v_size_m: Option<u32>,
    #[serde(default)]
    pub spec_ngram_map_k4v_min_hits: Option<u32>,
    // KV cache
    #[serde(default)]
    pub kv_unified: Option<bool>,
    #[serde(default)]
    pub cache_idle_slots: Option<bool>,
    #[serde(default)]
    pub cache_ram_mib: Option<i32>,
    // Fit
    #[serde(default)]
    pub fit_enabled: Option<bool>,
    #[serde(default)]
    pub fit_ctx: Option<u32>,
    #[serde(default)]
    pub fit_target: Option<String>,
    #[serde(default)]
    pub fit_print: Option<bool>,
    // Advanced
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub system_prompt_file: String,
    #[serde(default)]
    pub extra_args: String,
    #[serde(default)]
    pub bind_host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,

    // Spawn V2: extended fields
    #[serde(default)]
    pub hf_repo: Option<String>,
    #[serde(default)]
    pub chat_template_file: Option<String>,
    #[serde(default)]
    pub mmproj: Option<String>,
    // Vision token budget (only meaningful when mmproj is set)
    #[serde(default)]
    pub image_min_tokens: Option<u32>,
    #[serde(default)]
    pub image_max_tokens: Option<u32>,
    #[serde(default)]
    pub grammar: Option<String>,
    #[serde(default)]
    pub json_schema: Option<String>,
    #[serde(default)]
    pub cache_type_k: Option<String>,
    #[serde(default)]
    pub cache_type_v: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub enable_thinking: Option<bool>,
    #[serde(default)]
    pub preserve_thinking: Option<bool>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub reasoning_budget: Option<i32>,
    #[serde(default)]
    pub reasoning_budget_message: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub benchmark_mode: bool,
    /// User-assigned tags for organization (general, coding, roleplay, custom).
    #[serde(default)]
    pub tags: Vec<String>,

    // GGUF-derived metadata (populated on preset save when model_path exists).
    // These replace any name-based guessing for UI labels.
    /// `general.architecture` from GGUF header (e.g. "qwen3_6", "llama").
    #[serde(default)]
    pub gguf_architecture: Option<String>,
    /// `general.parameter_count` from GGUF header.
    #[serde(default)]
    pub param_count: Option<u64>,
    /// Human-readable family slug derived from architecture (e.g. "qwen36", "llama3", "gemma4").
    #[serde(default)]
    pub family: Option<String>,
    /// Size class derived from param_count: tiny/small/medium/large/huge.
    #[serde(default)]
    pub size_class: Option<String>,
    /// Architecture label: "dense" | "moe" | "hybrid_moe".
    #[serde(default)]
    pub architecture_kind: Option<String>,
    /// Total MoE experts per layer (for MoE / hybrid-moE models).
    #[serde(default)]
    pub expert_count: Option<u32>,
    /// Active MoE experts per token (for MoE / hybrid-moE models).
    #[serde(default)]
    pub expert_used_count: Option<u32>,
    /// Effective active parameters in billions (for MoE / hybrid-moE models).
    /// For dense models, equals total params; for MoE, only counts params
    /// used per token (backbone + active experts).
    #[serde(default)]
    pub active_params_b: Option<f64>,
}

pub fn next_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("p{ts}")
}

/// Load presets from disk, falling back to defaults if file doesn't exist.
pub fn load_presets(path: &Path) -> Vec<ModelPreset> {
    if path.exists() {
        match std::fs::read_to_string(path) {
            Ok(contents) => match serde_json::from_str::<Vec<ModelPreset>>(&contents) {
                Ok(mut presets) if !presets.is_empty() => {
                    // Backfill GGUF-derived metadata (architecture_kind, active_params_b,
                    // expert counts, etc.) for presets saved before these fields existed,
                    // then persist so the welcome page / launch cards render correctly
                    // without requiring the user to re-save each preset.
                    backfill_gguf_metadata(path, &mut presets);
                    return presets;
                }
                Ok(_) => eprintln!("[warn] Presets file is empty, using defaults"),
                Err(e) => eprintln!("[warn] Failed to parse presets file: {e}, using defaults"),
            },
            Err(e) => eprintln!("[warn] Failed to read presets file: {e}, using defaults"),
        }
    }
    let presets = default_presets();
    // Try to save defaults to disk for future editing
    let _ = save_presets(path, &presets);
    presets
}

/// Run [`ensure_gguf_metadata`] over every preset and persist the result if any
/// preset gained new metadata. Used at load time so existing presets pick up
/// fields added after they were first saved (e.g. architecture labels). Presets
/// whose model file is missing are left untouched (ensure_gguf_metadata is a no-op).
fn backfill_gguf_metadata(path: &Path, presets: &mut [ModelPreset]) {
    let mut changed = false;
    for preset in presets.iter_mut() {
        let before = (
            preset.architecture_kind.clone(),
            preset.active_params_b,
            preset.gguf_architecture.clone(),
        );
        ensure_gguf_metadata(preset);
        let after = (
            preset.architecture_kind.clone(),
            preset.active_params_b,
            preset.gguf_architecture.clone(),
        );
        if before != after {
            changed = true;
        }
    }
    if changed {
        let _ = save_presets(path, presets);
    }
}

/// Save presets to disk atomically (write tmp, rename).
pub fn save_presets(path: &Path, presets: &[ModelPreset]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(presets)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

impl ModelPreset {
    /// Reset every GGUF-derived metadata field to `None`.
    ///
    /// Call this whenever `model_path` changes so [`ensure_gguf_metadata`] recomputes
    /// from the new file instead of keeping stale values (it only refills `None` fields).
    ///
    /// This is the single source of truth for "which fields come from the GGUF" — when a
    /// new GGUF-derived field is added to [`ModelPreset`], add it here and nowhere else.
    pub fn clear_gguf_metadata(&mut self) {
        self.gguf_architecture = None;
        self.param_count = None;
        self.family = None;
        self.size_class = None;
        self.architecture_kind = None;
        self.expert_count = None;
        self.expert_used_count = None;
        self.active_params_b = None;
    }
}

/// Populate GGUF-derived metadata fields on a preset if they are missing.
/// Called from preset save endpoints so cards get accurate labels.
///
/// Only writes; never overwrites existing family/size_class values (backwards-compatible).
pub fn ensure_gguf_metadata(preset: &mut ModelPreset) {
    let model_path = preset.model_path.trim();
    if model_path.is_empty() {
        return;
    }

    // Only fill when metadata is incomplete.
    // New architecture_kind/expert/active_params_b fields are included in the
    // "complete" check so existing presets with older fields get them as well.
    if preset.gguf_architecture.is_some()
        && preset.family.is_some()
        && preset.param_count.is_some()
        && preset.size_class.is_some()
        && preset.architecture_kind.is_some()
        && preset.active_params_b.is_some()
    {
        return;
    }

    let meta = match crate::llama::gguf_meta::read_gguf_metadata(Path::new(model_path)) {
        Ok(m) => m,
        Err(_) => {
            // Non-critical: leave fields as-is and log quietly
            return;
        }
    };

    // Store architecture (authoritative)
    if preset.gguf_architecture.is_none() {
        preset.gguf_architecture = meta.architecture.clone();
    }

    // Store param_count (authoritative)
    if preset.param_count.is_none() {
        preset.param_count = meta.param_count;
    }

    // Derive family from architecture (not filename)
    if preset.family.is_none()
        && let Some(ref arch) = meta.architecture
    {
        preset.family = crate::models::infer_family_from_architecture(arch);
    }

    // Derive size_class from param_count
    if preset.size_class.is_none()
        && let Some(pc) = meta.param_count
    {
        preset.size_class = crate::models::infer_size_class_from_param_count(pc);
    }

    // Store expert_count / expert_used_count from GGUF.
    if preset.expert_count.is_none() {
        preset.expert_count = meta.expert_count;
    }
    if preset.expert_used_count.is_none() {
        preset.expert_used_count = meta.expert_used_count;
    }

    // Derive architecture_kind:
    // - MoE: expert_count is set and > 0
    // - Hybrid: MoE + full_attention_interval > 1 (some layers full attention, others linear/DeltaNet)
    // - Dense: everything else
    if preset.architecture_kind.is_none() {
        let expert_count = meta.expert_count.unwrap_or(0);
        let is_moe = expert_count > 0;
        let is_hybrid_attn = meta.full_attention_interval.is_some_and(|v| v > 1);

        preset.architecture_kind = if is_moe && is_hybrid_attn {
            Some("hybrid_moe".into())
        } else if is_moe {
            Some("moe".into())
        } else {
            Some("dense".into())
        };
    }

    // Derive active_params_b for MoE/hybrid-moE models.
    // For dense: active = total.
    // For MoE/hybrid-moE: estimate from backbone + active experts.
    if preset.active_params_b.is_none()
        && let Some(total_params) = meta.param_count
    {
        preset.active_params_b = compute_active_params_b(&meta, total_params);
    }
}

/// Estimate "active parameters" in billions from GGUF metadata.
///
/// For dense models: active = total.
/// For MoE/hybrid-MoE: active ≈ backbone_params + (N_used / N_experts) * expert_params.
///
/// Fallback: if data is incomplete, falls back to total / (1 + N_experts / N_used)
/// if that ratio is reasonable; otherwise total params (param_b).
fn compute_active_params_b(
    meta: &crate::llama::gguf_meta::GgufMetadata,
    total_params: u64,
) -> Option<f64> {
    let is_moe = meta.expert_count.is_some_and(|e| e > 0);
    if !is_moe {
        return Some(total_params as f64 / 1e9);
    }

    let n_experts = meta.expert_count?;
    let n_used = meta.expert_used_count?;
    if n_experts == 0 || n_used == 0 || n_used > n_experts {
        // Invalid expert ratio → fallback to total.
        return Some(total_params as f64 / 1e9);
    }

    // Attempt a structural estimate from GGUF:
    // backbone ≈ attention projections + token-embedding/output projection
    // experts_total = P - backbone
    // active ≈ backbone + N_used * (experts_total / N_experts)
    let n_layers = meta.block_count;
    let head_count = meta.head_count;
    let head_count_kv = meta.head_count_kv;
    let kv_len = meta.key_length;
    let embd = meta.embedding_length;

    let have_enough = matches!(
        (n_layers, head_count, head_count_kv, kv_len, embd),
        (Some(_), Some(_), Some(_), Some(_), Some(_))
    );

    if have_enough {
        let n_layers = n_layers.unwrap();
        let head_count = head_count.unwrap();
        let head_count_kv = head_count_kv.unwrap();
        let kv_len = kv_len.unwrap();
        let embd = embd.unwrap();

        // Approximate the dense "backbone" (non-expert) parameter count. This is
        // a deliberately rough lower bound — it does not model shared experts,
        // dense-then-MoE layer splits, or attention variants — so it is guarded by
        // the sanity clamps below. The FFN/expert weights are intentionally *not*
        // counted here: in MoE GGUFs `feed_forward_length` is the per-expert
        // intermediate size, so they fall into `expert_total` instead.
        //
        // Per layer, attention Q/K/V/O projections (head_dim == key_length):
        //   embd * head_dim * (2*n_head + 2*n_head_kv)
        // Plus a one-off token-embedding + output projection: ~2 * embd^2.
        let head_dim = kv_len as u64;
        let attn_per_layer =
            embd as u64 * head_dim * (2 * head_count as u64 + 2 * head_count_kv as u64);
        let backbone_total: u64 = n_layers as u64 * attn_per_layer + 2 * embd as u64 * embd as u64;

        let expert_total = if total_params > backbone_total {
            total_params - backbone_total
        } else {
            // Backbone estimate exceeds total (bad input); fall back to simple ratio.
            return simple_moe_active(total_params, n_experts, n_used);
        };

        // If the expert portion is <10% of total, the structural estimate is
        // clearly off (a real MoE keeps most weight in experts), so fall back.
        if (expert_total as f64) < (total_params as f64 * 0.1) {
            return simple_moe_active(total_params, n_experts, n_used);
        }

        let per_expert = expert_total / n_experts as u64;
        let active: f64 = (backbone_total as f64) + (n_used as f64 * per_expert as f64);
        let active_b = active / 1e9;

        // Sanity: active must be < total and > 0. If ratio is off, use simple.
        if active > 0.0 && active_b < total_params as f64 / 1e9 {
            Some(active_b)
        } else {
            simple_moe_active(total_params, n_experts, n_used)
        }
    } else {
        // Not enough GGUF fields → simple ratio.
        simple_moe_active(total_params, n_experts, n_used)
    }
}

/// Simple ratio-based active param estimate for MoE:
/// active ≈ total / (1 + N_experts / N_used)
/// Only used when structural estimate is not possible.
fn simple_moe_active(total: u64, n_experts: u32, n_used: u32) -> Option<f64> {
    if n_experts == 0 || n_used == 0 {
        return Some(total as f64 / 1e9);
    }
    let ratio = n_experts as f64 / n_used as f64;
    let active_b = total as f64 / (1.0 + ratio) / 1e9;
    if active_b > 0.0 && active_b < total as f64 / 1e9 {
        Some(active_b)
    } else {
        Some(total as f64 / 1e9)
    }
}

// ── System Prompt Templates ────────────────────────────────────────────────────

/// User-created or user-modified system prompt templates.
/// Stored on disk; merged with frontend defaults at runtime.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExplicitPolicies {
    #[serde(default)]
    pub level1: Option<String>,
    #[serde(default)]
    pub level2: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemPromptTemplate {
    #[serde(default = "template_next_id")]
    pub id: String,
    pub name: String,
    pub prompt: String,
    #[serde(default)]
    pub explicit_policies: Option<ExplicitPolicies>,
}

fn template_next_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("t{ts}")
}

/// Load user templates from disk. Returns empty vec on any error (defaults are in the frontend).
pub fn load_templates(path: &Path) -> Vec<SystemPromptTemplate> {
    match std::fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<Vec<SystemPromptTemplate>>(&contents) {
            Ok(templates) => templates,
            Err(e) => {
                eprintln!("[warn] Failed to parse templates file {:?}: {e}", path);
                vec![]
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let templates = vec![];
            if !path.as_os_str().is_empty()
                && let Err(e) = save_templates(path, &templates)
            {
                eprintln!(
                    "[warn] Failed to initialize missing templates file {:?}: {e}",
                    path
                );
            }
            templates
        }
        Err(e) => {
            eprintln!("[warn] Failed to read templates file {:?}: {e}", path);
            vec![]
        }
    }
}

/// Save user templates to disk atomically.
pub fn save_templates(path: &Path, templates: &[SystemPromptTemplate]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(templates)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn default_presets() -> Vec<ModelPreset> {
    vec![ModelPreset {
        id: "default-1".into(),
        name: "Example: 128K context".into(),
        context_size: 128000,
        ctk: "q8_0".into(),
        ctv: "f16".into(),
        ngram_spec: true,
        batch_size: 2048,
        ubatch_size: 2048,
        no_mmap: false,
        parallel_slots: 1,
        ..Default::default()
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn missing_templates_file_is_recreated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("templates.json");

        let templates = load_templates(&path);

        assert!(templates.is_empty());
        assert!(path.exists());
        assert_eq!(load_templates(&path).len(), 0);
    }

    #[test]
    fn clear_gguf_metadata_resets_all_derived_fields() {
        // Guards against the recurring bug where a new GGUF-derived field is added
        // to ModelPreset but not to the model_path-change reset, leaving stale data.
        let mut preset = ModelPreset {
            gguf_architecture: Some("qwen3moe".into()),
            param_count: Some(30_000_000_000),
            family: Some("qwen".into()),
            size_class: Some("large".into()),
            architecture_kind: Some("moe".into()),
            expert_count: Some(128),
            expert_used_count: Some(8),
            active_params_b: Some(3.3),
            ..Default::default()
        };

        preset.clear_gguf_metadata();

        assert!(preset.gguf_architecture.is_none());
        assert!(preset.param_count.is_none());
        assert!(preset.family.is_none());
        assert!(preset.size_class.is_none());
        assert!(preset.architecture_kind.is_none());
        assert!(preset.expert_count.is_none());
        assert!(preset.expert_used_count.is_none());
        assert!(preset.active_params_b.is_none());
    }

    #[test]
    fn test_default_presets_not_empty() {
        let presets = default_presets();
        assert!(!presets.is_empty());
        assert_eq!(presets.len(), 1);
    }

    #[test]
    fn test_preset_serialization_roundtrip() {
        let presets = default_presets();
        let json = serde_json::to_string(&presets).unwrap();
        let deserialized: Vec<ModelPreset> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), presets.len());
        assert_eq!(deserialized[0].name, presets[0].name);
        assert_eq!(deserialized[0].id, "default-1");
    }

    #[test]
    fn test_reasoning_fields_roundtrip() {
        let preset = ModelPreset {
            id: "p1".into(),
            name: "Reasoning".into(),
            model_path: "/tmp/model.gguf".into(),
            reasoning: Some("on".into()),
            reasoning_budget: Some(16384),
            reasoning_budget_message: Some("\nFinal Answer:".into()),
            enable_thinking: Some(true),
            preserve_thinking: Some(true),
            presence_penalty: Some(1.5),
            ..Default::default()
        };
        let json = serde_json::to_string(&preset).unwrap();
        let decoded: ModelPreset = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.reasoning.as_deref(), Some("on"));
        assert_eq!(decoded.reasoning_budget, Some(16384));
        assert_eq!(
            decoded.reasoning_budget_message.as_deref(),
            Some("\nFinal Answer:")
        );
        assert_eq!(decoded.enable_thinking, Some(true));
        assert_eq!(decoded.preserve_thinking, Some(true));
        assert_eq!(decoded.presence_penalty, Some(1.5));
    }

    #[test]
    fn test_all_default_presets_have_ids() {
        let presets = default_presets();
        for (i, p) in presets.iter().enumerate() {
            assert_eq!(p.id, format!("default-{}", i + 1));
        }
    }

    #[test]
    fn test_load_save_roundtrip() {
        // Use PID in the directory name to avoid races when multiple cargo
        // test processes run simultaneously (e.g., concurrent CI builds).
        let dir = std::env::temp_dir().join(format!("llama-monitor-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test-presets.json");

        let presets = default_presets();
        save_presets(&path, &presets).unwrap();

        let loaded = load_presets(&path);
        assert_eq!(loaded.len(), presets.len());
        assert_eq!(loaded[0].name, presets[0].name);

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn test_load_missing_file_returns_defaults() {
        let path = std::env::temp_dir().join(format!(
            "llama-monitor-missing-presets-{}.json",
            std::process::id()
        ));
        let loaded = load_presets(&path);
        assert_eq!(loaded.len(), 1);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_corrupt_file_returns_defaults() {
        let path = std::env::temp_dir().join(format!(
            "llama-monitor-corrupt-presets-{}.json",
            std::process::id()
        ));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"not json").unwrap();
        let loaded = load_presets(&path);
        assert_eq!(loaded.len(), 1);
        std::fs::remove_file(&path).ok();
    }
}
