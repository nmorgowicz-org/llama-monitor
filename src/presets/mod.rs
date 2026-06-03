use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ModelPreset {
    #[serde(default = "next_id")]
    pub id: String,
    pub name: String,
    pub model_path: String,
    pub context_size: u64,
    pub ctk: String,
    pub ctv: String,
    pub tensor_split: String,
    pub batch_size: u32,
    pub ubatch_size: u32,
    #[serde(default)]
    pub no_mmap: bool,
    #[serde(default)]
    pub ngram_spec: bool,
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
    pub threads: Option<u32>,
    #[serde(default)]
    pub threads_batch: Option<u32>,
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
    // Fit
    #[serde(default)]
    pub fit_enabled: Option<bool>,
    #[serde(default)]
    pub fit_ctx: Option<u32>,
    #[serde(default)]
    pub fit_target: Option<String>,
    #[serde(default)]
    pub fit_print: Option<bool>,
    // Misc
    #[serde(default)]
    pub ignore_eos: bool,
    // Advanced
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub system_prompt_file: String,
    #[serde(default)]
    pub extra_args: String,
    #[serde(default)]
    pub bind_host: Option<String>,

    // Spawn V2: extended fields
    #[serde(default)]
    pub hf_repo: Option<String>,
    #[serde(default)]
    pub chat_template_file: Option<String>,
    #[serde(default)]
    pub mmproj: Option<String>,
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
                Ok(presets) if !presets.is_empty() => return presets,
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
    if path.exists() {
        match std::fs::read_to_string(path) {
            Ok(contents) => match serde_json::from_str::<Vec<SystemPromptTemplate>>(&contents) {
                Ok(templates) => return templates,
                Err(e) => eprintln!("[warn] Failed to parse templates file: {e}"),
            },
            Err(e) => eprintln!("[warn] Failed to read templates file: {e}"),
        }
    }
    vec![]
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
        no_mmap: true,
        parallel_slots: 1,
        ..Default::default()
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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
