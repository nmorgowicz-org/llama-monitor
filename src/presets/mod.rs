use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    pub no_mmap: bool,
    pub ngram_spec: bool,
    pub parallel_slots: u32,
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
    // Advanced
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub system_prompt_file: String,
    #[serde(default)]
    pub extra_args: String,
}

fn next_id() -> String {
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

pub fn default_presets() -> Vec<ModelPreset> {
    vec![
        ModelPreset {
            id: "default-1".into(),
            name: "Qwen3-Coder-Next Q4_1 (256K f16 fast ~57t/s)".into(),
            model_path: "/home/artefact/models/Qwen3-Coder-Next-Q4_1-00001-of-00003.gguf".into(),
            context_size: 256000,
            ctk: "f16".into(),
            ctv: "f16".into(),
            tensor_split: "7,8,8,8".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: true,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-2".into(),
            name: "Qwen3-Coder-Next Q4_1 (400K turbo3 YaRN)".into(),
            model_path: "/home/artefact/models/Qwen3-Coder-Next-Q4_1-00001-of-00003.gguf".into(),
            context_size: 400000,
            ctk: "turbo3".into(),
            ctv: "turbo3".into(),
            tensor_split: "7,8,8,8".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: true,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-3".into(),
            name: "Qwen3-Coder-Next Q4_0 (256K f16 fast)".into(),
            model_path: "/home/artefact/models/Qwen3-Coder-Next-Q4_0.gguf".into(),
            context_size: 256000,
            ctk: "f16".into(),
            ctv: "f16".into(),
            tensor_split: "7,8,8,8".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: true,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-4".into(),
            name: "Gemma-4-26B-A4B-it-Q8_0 (256K f16 fast +ngram)".into(),
            model_path: "/home/artefact/models/gemma-4-26B-A4B-it-Q8_0.gguf".into(),
            context_size: 256000,
            ctk: "f16".into(),
            ctv: "f16".into(),
            tensor_split: "7,8,8,8".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: true,
            ngram_spec: true,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-5".into(),
            name: "Devstral 24B Q8_0 (327K turbo3 YaRN +ngram)".into(),
            model_path: "/home/artefact/models/Devstral-Small-2-24B-Instruct-2512-Q8_0.gguf".into(),
            context_size: 327680,
            ctk: "turbo3".into(),
            ctv: "turbo3".into(),
            tensor_split: "".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: true,
            ngram_spec: true,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-6".into(),
            name: "Devstral 24B Q4_K_M (393K turbo3 YaRN +ngram) [no gfx906 opt]".into(),
            model_path: "/home/artefact/models/Devstral-Small-2-24B-Instruct-2512-Q4_K_M.gguf"
                .into(),
            context_size: 393216,
            ctk: "turbo3".into(),
            ctv: "turbo3".into(),
            tensor_split: "".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: true,
            ngram_spec: true,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-7".into(),
            name: "Qwen3.5-9B Q4_1 (131K turbo3K+f16V fast 1GPU)".into(),
            model_path: "/home/artefact/models/Qwen3.5-9B-Q4_1.gguf".into(),
            context_size: 131072,
            ctk: "turbo3".into(),
            ctv: "f16".into(),
            tensor_split: "".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: false,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-8".into(),
            name: "Qwen3.5-9B Q4_1 (163K turbo3 ~59t/s 1GPU)".into(),
            model_path: "/home/artefact/models/Qwen3.5-9B-Q4_1.gguf".into(),
            context_size: 163840,
            ctk: "turbo3".into(),
            ctv: "turbo3".into(),
            tensor_split: "".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: false,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-9".into(),
            name: "Qwen3.5-27B Q4_0 (360K turbo3K+f16V fast YaRN)".into(),
            model_path: "/home/artefact/models/Qwen3.5-27B-Q4_0.gguf".into(),
            context_size: 360448,
            ctk: "turbo3".into(),
            ctv: "f16".into(),
            tensor_split: "".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: false,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-10".into(),
            name: "Qwen3.5-27B Q4_0 (1M turbo3 YaRN)".into(),
            model_path: "/home/artefact/models/Qwen3.5-27B-Q4_0.gguf".into(),
            context_size: 1048576,
            ctk: "turbo3".into(),
            ctv: "turbo3".into(),
            tensor_split: "".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: false,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-11".into(),
            name: "Qwen3.5-27B Q4_0 (600K turbo3 YaRN)".into(),
            model_path: "/home/artefact/models/Qwen3.5-27B-Q4_0.gguf".into(),
            context_size: 600000,
            ctk: "turbo3".into(),
            ctv: "turbo3".into(),
            tensor_split: "".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: false,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-12".into(),
            name: "Qwen3.5-27B Q8_0 (458K turbo3 YaRN)".into(),
            model_path: "/home/artefact/models/Qwen3.5-27B-Q8_0.gguf".into(),
            context_size: 458752,
            ctk: "turbo3".into(),
            ctv: "turbo3".into(),
            tensor_split: "".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: true,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-13".into(),
            name: "Qwen3.5-122B-A10B Q2 (360K turbo3 YaRN)".into(),
            model_path: "/home/artefact/models/Qwen3.5-122B-A10B-UD-Q2_K_XL.gguf".into(),
            context_size: 360448,
            ctk: "turbo3".into(),
            ctv: "turbo3".into(),
            tensor_split: "7,8,8,8".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: true,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-14".into(),
            name: "Qwen3-Coder-30B MoE Q4_K_XL (491K turbo3 YaRN) [no gfx906 opt]".into(),
            model_path: "/home/artefact/models/Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf".into(),
            context_size: 491520,
            ctk: "turbo3".into(),
            ctv: "turbo3".into(),
            tensor_split: "".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: true,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-15".into(),
            name: "Qwen3.5-35B-A3B Q8 (393K turbo3 YaRN)".into(),
            model_path: "/home/artefact/models/qwen3.5-35b-a3b/Qwen3.5-35B-A3B-UD-Q8_K_XL.gguf"
                .into(),
            context_size: 393216,
            ctk: "turbo3".into(),
            ctv: "turbo3".into(),
            tensor_split: "7,8,8,8".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: true,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
        ModelPreset {
            id: "default-16".into(),
            name: "Nemotron-Cascade 30B Q4_1 (393K turbo3 YaRN)".into(),
            model_path: "/home/artefact/models/nvidia_Nemotron-Cascade-2-30B-A3B-Q4_1.gguf".into(),
            context_size: 393216,
            ctk: "turbo3".into(),
            ctv: "turbo3".into(),
            tensor_split: "".into(),
            batch_size: 2048,
            ubatch_size: 2048,
            no_mmap: true,
            ngram_spec: false,
            parallel_slots: 1,
            gpu_layers: None,
            mlock: false,
            flash_attn: String::new(),
            split_mode: String::new(),
            main_gpu: None,
            threads: None,
            threads_batch: None,
            rope_scaling: String::new(),
            rope_freq_base: None,
            rope_freq_scale: None,
            draft_model: String::new(),
            draft_min: None,
            draft_max: None,
            spec_ngram_size: None,
            seed: None,
            system_prompt_file: String::new(),
            extra_args: String::new(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_default_presets_not_empty() {
        let presets = default_presets();
        assert!(!presets.is_empty());
        assert_eq!(presets.len(), 16);
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
    fn test_all_default_presets_have_ids() {
        let presets = default_presets();
        for (i, p) in presets.iter().enumerate() {
            assert_eq!(p.id, format!("default-{}", i + 1));
        }
    }

    #[test]
    fn test_load_save_roundtrip() {
        let dir = std::env::temp_dir().join("llama-monitor-test");
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
        let path = std::path::PathBuf::from("/tmp/nonexistent-llama-presets-12345.json");
        let loaded = load_presets(&path);
        assert_eq!(loaded.len(), 16);
        // Clean up the file that load_presets creates
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_corrupt_file_returns_defaults() {
        let path = std::env::temp_dir().join("corrupt-presets.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"not json").unwrap();
        let loaded = load_presets(&path);
        assert_eq!(loaded.len(), 16);
        std::fs::remove_file(&path).ok();
    }
}
