use crate::inference::InferenceBackend;
use crate::inference::rapid_mlx::RapidMlxConfig;
use anyhow::Result;
use std::path::Path;

/// Current preset schema version (D32).
/// v1: initial version with schema_version field; typed Rapid-MLX model_source
///     is authoritative; legacy model_path is read-migrated but never re-written.
/// v2: Phase 6 Part B — prefix_cache_enabled and prefix_cache_budget_bytes fields;
///     existing presets default to prefix_cache_enabled=false (safe default).
/// v3: Phase 7A — all Phase 7 config fields (KV/cache, batching, GPU, Web UI, safety);
///     existing presets load with None/defaults (safe degraded mode).
pub const PRESET_SCHEMA_VERSION: u32 = 3;

/// Forward-migrate a preset from any known version to current.
/// Returns `true` if migration was applied, `false` if already current.
pub fn migrate_preset(preset: &mut ModelPreset) -> bool {
    let from_version = preset.schema_version.unwrap_or(0);
    if from_version >= PRESET_SCHEMA_VERSION {
        return false;
    }
    let mut migrated = false;
    // v0 → v1: typed Rapid-MLX model_source migration
    if from_version < 1 {
        if let Some(rapid) = preset.rapid_mlx.as_mut()
            && rapid.model_source.is_none()
            && !rapid.model_path.is_empty()
            && let Ok(source) =
                crate::inference::rapid_mlx::model_resolver::source_from_legacy_model_path(
                    &rapid.model_path,
                )
        {
            rapid.model_source = Some(source);
            migrated = true;
        }
        preset.schema_version = Some(1);
    }
    // v1 → v2: Phase 6 Part B — add prefix_cache_enabled (default false) and prefix_cache_budget_bytes.
    // Existing presets remain with prefix_cache_enabled=false (safe default, never auto-enabled).
    if preset.schema_version.unwrap_or(1) < 2 {
        // Fields use #[serde(default)] so they deserialize safely; this migration
        // just bumps the schema_version marker for forward-compatibility tracking.
        preset.schema_version = Some(2);
        migrated = true;
    }
    // v2 → v3: Phase 7A — all Phase 7 config fields.
    // All fields use #[serde(default, skip_serializing_if = "Option::is_none")] so
    // existing presets load with None (safe degraded mode). Migration bumps schema marker.
    if preset.schema_version.unwrap_or(2) < 3 {
        preset.schema_version = Some(3);
        migrated = true;
    }
    migrated
}

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
    /// Schema version for forward migration (D32). `None` means v0 (pre-migration).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<u32>,
    /// Missing in legacy presets; serde defaults those to llama.cpp.
    #[serde(default)]
    pub backend: InferenceBackend,
    /// Backend-owned Rapid-MLX launch settings. Llama.cpp retains the legacy
    /// flat fields until the explicit preset-schema migration phase.
    #[serde(default)]
    pub rapid_mlx: Option<RapidMlxConfig>,
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
    pub tool_call_format: Option<String>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub reasoning_budget: Option<i32>,
    #[serde(default)]
    pub reasoning_budget_message: Option<String>,
    /// Persisted in the protected preset file; API responses always redact it.
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_configured: bool,
    /// API input control: explicitly remove an existing key. Never persisted.
    #[serde(default, skip_serializing)]
    pub clear_api_key: bool,
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
    /// Model layer count (GGUF `block_count`). Surfaced in the editor so users can
    /// tune `--n-cpu-moe`, which offloads expert layers and is bounded by this count.
    #[serde(default)]
    pub block_count: Option<u32>,
    /// Exact bytes per transformer layer, measured from the GGUF tensor directory
    /// (not estimated). The VRAM each `-ngl` layer occupies on the GPU.
    #[serde(default)]
    pub bytes_per_layer: Option<u64>,
    /// Exact routed-expert bytes per MoE layer, measured from the tensor directory.
    /// The VRAM freed per layer offloaded via `--n-cpu-moe`.
    #[serde(default)]
    pub expert_bytes_per_layer: Option<u64>,
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
                    // D32: forward-migrate presets from prior schema versions.
                    let mut any_migrated = false;
                    for preset in presets.iter_mut() {
                        if migrate_preset(preset) {
                            any_migrated = true;
                        }
                    }
                    if any_migrated {
                        let _ = save_presets(path, &presets);
                    }
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
            preset.block_count,
            preset.bytes_per_layer,
            preset.expert_bytes_per_layer,
        );
        ensure_gguf_metadata(preset);
        let after = (
            preset.architecture_kind.clone(),
            preset.active_params_b,
            preset.gguf_architecture.clone(),
            preset.block_count,
            preset.bytes_per_layer,
            preset.expert_bytes_per_layer,
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
        self.block_count = None;
        self.bytes_per_layer = None;
        self.expert_bytes_per_layer = None;
    }
}

/// Populate GGUF-derived metadata fields on a preset if they are missing.
/// Called:
/// - at startup via backfill_gguf_metadata() for all presets
/// - on preset create/update via the API
///
/// Safety rule (backwards-compatible and forward-safe):
/// - Only writes to fields that are currently None.
/// - Never overwrites user-edited or previously set values.
/// - Does not require "all or nothing": even if some fields are set, we still
///   attempt to fill the rest. This ensures that new GGUF-derived fields are
///   backfilled into existing presets without the user having to change model_path.
pub fn ensure_gguf_metadata(preset: &mut ModelPreset) {
    let model_path = preset.model_path.trim();
    if model_path.is_empty() {
        return;
    }

    // Attempt to read GGUF metadata.
    // Non-critical: if read fails (missing file, not a GGUF, etc.), leave fields as-is.
    let meta = match crate::llama::gguf_meta::read_gguf_metadata(Path::new(model_path)) {
        Ok(m) => m,
        Err(_) => {
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

    // Derive family from architecture (not filename).
    // Both "qwen35" and "qwen35moe" cover Qwen3.5 and Qwen3.6 — block_count disambiguates:
    // ≥75 → Qwen3.5 (e.g. 122B-A10B: 94 blocks); <75 → Qwen3.6 (27B dense: 64, 35B-A3B: 41).
    // Same heuristic used by vram.rs.
    if preset.family.is_none()
        && let Some(ref arch) = meta.architecture
    {
        let a = arch.to_ascii_lowercase();
        preset.family = if a == "qwen35" || a == "qwen35moe" {
            Some(
                match meta.block_count {
                    Some(bc) if bc >= 75 => "qwen35",
                    _ => "qwen36",
                }
                .into(),
            )
        } else {
            crate::models::infer_family_from_architecture(arch)
        };
    }

    // Derive size_class from param_count
    if preset.size_class.is_none()
        && let Some(pc) = meta.param_count
    {
        preset.size_class = crate::models::infer_size_class_from_param_count(pc);
    }

    // Store expert_count / expert_used_count and layer count from GGUF. block_count
    // is the model's layer count, surfaced in the editor so users can tune --n-cpu-moe
    // (which offloads expert layers and is bounded by the layer count).
    if preset.expert_count.is_none() {
        preset.expert_count = meta.expert_count;
    }
    if preset.expert_used_count.is_none() {
        preset.expert_used_count = meta.expert_used_count;
    }
    if preset.block_count.is_none() {
        preset.block_count = meta.block_count;
    }
    // Exact per-layer byte sizes measured from the tensor directory (real data).
    if preset.bytes_per_layer.is_none() {
        preset.bytes_per_layer = meta.bytes_per_layer();
    }
    if preset.expert_bytes_per_layer.is_none() {
        preset.expert_bytes_per_layer = meta.expert_bytes_per_layer();
    }

    // Derive architecture_kind + active_params_b from the shared GgufMetadata helpers
    // (same computation used by the spawn wizard's introspection path).
    if preset.architecture_kind.is_none() {
        preset.architecture_kind = Some(meta.architecture_kind());
    }
    if preset.active_params_b.is_none() {
        preset.active_params_b = meta.active_params_b();
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
        assert!(preset.block_count.is_none());
        assert!(preset.bytes_per_layer.is_none());
        assert!(preset.expert_bytes_per_layer.is_none());
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
    fn legacy_preset_without_backend_defaults_to_llama_cpp() {
        let preset: ModelPreset = serde_json::from_value(serde_json::json!({
            "name": "Legacy",
            "model_path": "/models/legacy.gguf"
        }))
        .unwrap();

        assert_eq!(preset.backend, InferenceBackend::LlamaCpp);
        assert!(preset.rapid_mlx.is_none());
    }

    #[test]
    fn rapid_mlx_preset_roundtrips_backend_owned_config() {
        let preset = ModelPreset {
            name: "Rapid".into(),
            backend: InferenceBackend::RapidMlx,
            rapid_mlx: Some(RapidMlxConfig {
                model_path: "/models/rapid".into(),
                served_model_name: Some("rapid-model".into()),
                port: 8123,
                ..Default::default()
            }),
            ..Default::default()
        };

        let decoded: ModelPreset =
            serde_json::from_str(&serde_json::to_string(&preset).unwrap()).unwrap();
        assert_eq!(decoded.backend, InferenceBackend::RapidMlx);
        let rapid = decoded.rapid_mlx.unwrap();
        assert_eq!(rapid.model_path, "/models/rapid");
        assert_eq!(rapid.served_model_name.as_deref(), Some("rapid-model"));
        assert_eq!(rapid.port, 8123);
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
            tool_call_format: Some("json".into()),
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
        assert_eq!(decoded.tool_call_format.as_deref(), Some("json"));
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

    // Phase 6 Part B: prefix cache persistence tests.

    #[test]
    fn test_prefix_cache_default_is_false() {
        let config = crate::inference::rapid_mlx::RapidMlxConfig::default();
        assert!(!config.prefix_cache_enabled);
        assert!(config.prefix_cache_budget_bytes.is_none());
    }

    #[test]
    fn test_prefix_cache_migration_v0_to_v2() {
        // v0 preset (no schema_version) with RapidMlxConfig — should migrate to v2
        // with prefix_cache_enabled=false (safe default).
        let json = serde_json::json!({
            "id": "test",
            "name": "Test",
            "backend": "rapid_mlx",
            "rapid_mlx": {
                "model_path": "/path/to/model",
                "port": 8000
            }
        });
        let mut preset: ModelPreset = serde_json::from_value(json).unwrap();
        let migrated = migrate_preset(&mut preset);
        assert!(migrated);
        assert_eq!(preset.schema_version, Some(3));
        // Safe default: prefix_cache_enabled=false
        assert!(!preset.rapid_mlx.as_ref().unwrap().prefix_cache_enabled);
    }

    #[test]
    fn test_prefix_cache_migration_v1_to_v2() {
        // v1 preset with schema_version=1 — should migrate to v3 (via v2→v3).
        let json = serde_json::json!({
            "id": "test",
            "name": "Test",
            "schema_version": 1,
            "backend": "rapid_mlx",
            "rapid_mlx": {
                "model_path": "/path/to/model",
                "port": 8000
            }
        });
        let mut preset: ModelPreset = serde_json::from_value(json).unwrap();
        let migrated = migrate_preset(&mut preset);
        assert!(migrated);
        assert_eq!(preset.schema_version, Some(3));
        // Safe default: prefix_cache_enabled=false
        assert!(!preset.rapid_mlx.as_ref().unwrap().prefix_cache_enabled);
    }

    #[test]
    fn test_prefix_cache_roundtrip_preserves_values() {
        // Test that prefix_cache_enabled and prefix_cache_budget_bytes survive save/load.
        let mut preset = ModelPreset {
            id: "test".into(),
            name: "Test".into(),
            backend: crate::inference::InferenceBackend::RapidMlx,
            rapid_mlx: Some(crate::inference::rapid_mlx::RapidMlxConfig {
                model_path: "/path/to/model".into(),
                prefix_cache_enabled: true,
                prefix_cache_budget_bytes: Some(1073741824), // 1 GiB
                ..Default::default()
            }),
            ..Default::default()
        };
        migrate_preset(&mut preset);

        let json = serde_json::to_value(&preset).unwrap();
        let loaded: ModelPreset = serde_json::from_value(json).unwrap();

        assert!(loaded.rapid_mlx.is_some());
        let rapid = loaded.rapid_mlx.unwrap();
        assert!(rapid.prefix_cache_enabled);
        assert_eq!(rapid.prefix_cache_budget_bytes, Some(1073741824));
    }

    #[test]
    fn test_prefix_cache_explicit_overrides_d30() {
        // User explicit prefix_cache_budget_bytes should be preserved and used over D30 auto-compute.
        let json = serde_json::json!({
            "id": "test",
            "name": "Test",
            "schema_version": 2,
            "backend": "rapid_mlx",
            "rapid_mlx": {
                "model_path": "/path/to/model",
                "prefix_cache_enabled": true,
                "prefix_cache_budget_bytes": 1572864000
            }
        });
        let preset: ModelPreset = serde_json::from_value(json).unwrap();
        let rapid = preset.rapid_mlx.unwrap();
        assert!(rapid.prefix_cache_enabled);
        assert_eq!(rapid.prefix_cache_budget_bytes, Some(1572864000u64));
    }

    #[test]
    fn test_runtime_metadata_prefix_cache_defaults() {
        let meta = crate::inference::rapid_mlx::runtime::RuntimeMetadata::default();
        assert!(!meta.prefix_cache_enabled);
        assert!(meta.prefix_cache_budget_bytes.is_none());
    }

    // Phase 7A: command-preview and preset migration tests.

    #[test]
    fn test_phase7_preset_roundtrip_preserves_all_fields() {
        use crate::inference::rapid_mlx::{KvCacheConfig, TurboQuantMode};

        let mut preset = ModelPreset {
            id: "test-p7".into(),
            name: "Phase7 Test".into(),
            backend: crate::inference::InferenceBackend::RapidMlx,
            rapid_mlx: Some(crate::inference::rapid_mlx::RapidMlxConfig {
                model_path: "/path/to/model".into(),
                kv_cache_dtype: Some(KvCacheConfig::Fp16),
                turboquant_mode: Some(TurboQuantMode::K8V4),
                prefix_cache_policy: Some("auto".into()),
                hybrid_cache_entries: Some(256),
                pflash_policy: Some("auto".into()),
                response_cache_policy: Some("on".into()),
                disk_checkpoint_policy: Some("off".into()),
                max_num_seqs: Some(128),
                max_concurrent_requests: Some(64),
                prefill_batch_size: Some(2048),
                completion_batch_size: Some(512),
                batching_policy: Some("auto".into()),
                concurrency_policy: Some("single_active".into()),
                reasoning_mode: Some("auto".into()),
                speculative_policy: Some("auto".into()),
                mllm_vision: Some("auto".into()),
                embeddings: Some("off".into()),
                gpu_memory_utilization: Some(0.85),
                web_ui_availability: Some("auto".into()),
                web_ui_static_path: None,
                web_ui_config_json: None,
                endpoint_compatibility: Some("openai_v1".into()),
                request_safety_policy: Some("auto".into()),
                sampling_mode: Some("auto".into()),
                parser_policy: Some("auto".into()),
                security_policy: Some("loopback_only".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        migrate_preset(&mut preset);

        let json = serde_json::to_value(&preset).unwrap();
        let loaded: ModelPreset = serde_json::from_value(json).unwrap();

        assert_eq!(loaded.schema_version, Some(3));
        let rapid = loaded.rapid_mlx.unwrap();
        assert_eq!(rapid.kv_cache_dtype, Some(KvCacheConfig::Fp16));
        assert_eq!(rapid.turboquant_mode, Some(TurboQuantMode::K8V4));
        assert_eq!(rapid.prefix_cache_policy, Some("auto".into()));
        assert_eq!(rapid.hybrid_cache_entries, Some(256));
        assert_eq!(rapid.pflash_policy, Some("auto".into()));
        assert_eq!(rapid.response_cache_policy, Some("on".into()));
        assert_eq!(rapid.disk_checkpoint_policy, Some("off".into()));
        assert_eq!(rapid.max_num_seqs, Some(128));
        assert_eq!(rapid.max_concurrent_requests, Some(64));
        assert_eq!(rapid.prefill_batch_size, Some(2048));
        assert_eq!(rapid.completion_batch_size, Some(512));
        assert_eq!(rapid.batching_policy, Some("auto".into()));
        assert_eq!(rapid.concurrency_policy, Some("single_active".into()));
        assert_eq!(rapid.reasoning_mode, Some("auto".into()));
        assert_eq!(rapid.speculative_policy, Some("auto".into()));
        assert_eq!(rapid.mllm_vision, Some("auto".into()));
        assert_eq!(rapid.embeddings, Some("off".into()));
        assert_eq!(rapid.gpu_memory_utilization, Some(0.85));
        assert_eq!(rapid.web_ui_availability, Some("auto".into()));
        assert_eq!(rapid.endpoint_compatibility, Some("openai_v1".into()));
        assert_eq!(rapid.request_safety_policy, Some("auto".into()));
        assert_eq!(rapid.sampling_mode, Some("auto".into()));
        assert_eq!(rapid.parser_policy, Some("auto".into()));
        assert_eq!(rapid.security_policy, Some("loopback_only".into()));
    }

    #[test]
    fn test_phase7_legacy_preset_defaults_safe() {
        // Legacy preset (pre-Phase 7) should load with all Phase 7 fields as None
        // (safe degraded mode per D32).
        let json = serde_json::json!({
            "id": "legacy",
            "name": "Legacy",
            "schema_version": 1,
            "backend": "rapid_mlx",
            "rapid_mlx": {
                "model_path": "/path/to/model",
                "port": 8000
            }
        });
        let mut preset: ModelPreset = serde_json::from_value(json).unwrap();
        let migrated = migrate_preset(&mut preset);
        assert!(migrated);
        assert_eq!(preset.schema_version, Some(3));

        let rapid = preset.rapid_mlx.unwrap();
        assert!(rapid.kv_cache_dtype.is_none());
        assert!(rapid.turboquant_mode.is_none());
        assert!(rapid.prefix_cache_policy.is_none());
        assert!(rapid.hybrid_cache_entries.is_none());
        assert!(rapid.pflash_policy.is_none());
        assert!(rapid.response_cache_policy.is_none());
        assert!(rapid.disk_checkpoint_policy.is_none());
        assert!(rapid.max_num_seqs.is_none());
        assert!(rapid.max_concurrent_requests.is_none());
        assert!(rapid.prefill_batch_size.is_none());
        assert!(rapid.completion_batch_size.is_none());
        assert!(rapid.batching_policy.is_none());
        assert!(rapid.concurrency_policy.is_none());
        assert!(rapid.reasoning_mode.is_none());
        assert!(rapid.speculative_policy.is_none());
        assert!(rapid.mllm_vision.is_none());
        assert!(rapid.embeddings.is_none());
        assert!(rapid.gpu_memory_utilization.is_none());
        assert!(rapid.web_ui_availability.is_none());
        assert!(rapid.endpoint_compatibility.is_none());
        assert!(rapid.request_safety_policy.is_none());
        assert!(rapid.sampling_mode.is_none());
        assert!(rapid.parser_policy.is_none());
        assert!(rapid.security_policy.is_none());
    }
}
