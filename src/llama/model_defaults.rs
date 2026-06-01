//! Model-specific generation defaults for spawn wizard.
//!
//! Provides safe, conservative defaults tuned per model family
//! (llama, deepseek, qwen, mistral, gemma, code models, MoE, etc.).

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ModelDefaults {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: i32,
    pub min_p: f32,
    pub repeat_penalty: f32,
    /// OpenAI-style presence penalty (flat logit reduction for any seen token).
    /// Maps to llama-server's --presence-penalty flag.
    /// 0.0 = disabled (default).
    pub presence_penalty: f32,
    pub max_tokens: u64,
}

impl Default for ModelDefaults {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            min_p: 0.05,
            repeat_penalty: 1.1,
            presence_penalty: 0.0,
            max_tokens: 2048,
        }
    }
}

/// A named sampling preset — shown as a pill in the wizard's review step.
/// The wizard auto-selects the first preset and lets users switch between them.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelPreset {
    pub name: String,
    #[serde(flatten)]
    pub defaults: ModelDefaults,
}

/// Return named presets for models that have meaningful mode variants
/// (e.g. Qwen3.6 thinking-general vs thinking-coding).
/// The first preset is the recommended default.
/// Returns a single "Default" preset for models with no special modes.
pub fn get_model_presets(name_or_repo: &str, size_bytes: u64, tags: &[String]) -> Vec<ModelPreset> {
    let lower = name_or_repo.to_ascii_lowercase();

    // Qwen3.6 family (including Qwopus and other derivatives):
    // two thinking-mode presets from https://unsloth.ai/docs/models/qwen3.6
    let is_qwen36 =
        (lower.contains("qwen3.6") || lower.contains("qwen36") || lower.contains("qwopus"))
            && (lower.contains("27b") || lower.contains("35b") || lower.contains("a3b"));

    if is_qwen36 {
        // https://unsloth.ai/docs/models/qwen3.6 — recommended max_tokens: 32k
        let thinking_general = ModelDefaults {
            temperature: 1.0,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 1.5,
            max_tokens: 32768,
        };
        let thinking_coding = ModelDefaults {
            temperature: 0.6,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 0.0,
            max_tokens: 32768,
        };
        return vec![
            ModelPreset {
                name: "Thinking — General".into(),
                defaults: thinking_general,
            },
            ModelPreset {
                name: "Thinking — Coding".into(),
                defaults: thinking_coding,
            },
        ];
    }

    // EXAONE 4.5: general-purpose vs OCR/document
    if lower.contains("exaone-4.5") || lower.contains("exaone4.5") {
        let general = ModelDefaults {
            temperature: 1.0,
            top_p: 0.95,
            top_k: 0,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 1.5,
            max_tokens: 2048,
        };
        let ocr = ModelDefaults {
            temperature: 0.6,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 1.5,
            max_tokens: 2048,
        };
        return vec![
            ModelPreset {
                name: "General purpose".into(),
                defaults: general,
            },
            ModelPreset {
                name: "OCR / Document".into(),
                defaults: ocr,
            },
        ];
    }

    // Default: single preset using existing per-family logic
    vec![ModelPreset {
        name: "Default".into(),
        defaults: get_model_defaults(name_or_repo, size_bytes, tags),
    }]
}

/// Determine model-specific generation defaults.
///
/// - name_or_repo: model name or HuggingFace repo path.
/// - size_bytes: model file size (used for MoE sizing hints).
/// - tags: optional tags like "moe", "vision", "code".
pub fn get_model_defaults(name_or_repo: &str, _size_bytes: u64, tags: &[String]) -> ModelDefaults {
    let lower = name_or_repo.to_ascii_lowercase();
    let is_moe = tags.iter().any(|t| t.eq_ignore_ascii_case("moe"))
        || lower.contains("moe")
        || lower.contains("mixtral")
        || lower.contains("deepseek-v3")
        || lower.contains("deepseek-r1")
        || lower.contains("granite-moe");

    let is_code = lower.contains("code")
        || lower.contains("coder")
        || lower.contains("codellama")
        || lower.contains("deepseek-coder")
        || lower.contains("qwen2.5-coder")
        || lower.contains("starcoder");

    let mut d = ModelDefaults::default();

    // EXAONE 4.5: general-purpose as single default
    // (presets handled by get_model_presets — this path only reached when called directly)
    if lower.contains("exaone-4.5") || lower.contains("exaone4.5") {
        d.temperature = 1.0;
        d.top_p = 0.95;
        d.top_k = 0;
        d.min_p = 0.0;
        d.repeat_penalty = 1.0;
        d.presence_penalty = 1.5;
        return d;
    }

    // Qwen3.6 family (including Qwopus derivatives): thinking-general as primary.
    // https://unsloth.ai/docs/models/qwen3.6
    let is_qwen36 =
        (lower.contains("qwen3.6") || lower.contains("qwen36") || lower.contains("qwopus"))
            && (lower.contains("27b") || lower.contains("35b") || lower.contains("a3b"));
    if is_qwen36 {
        d.temperature = 1.0;
        d.top_p = 0.95;
        d.top_k = 20;
        d.min_p = 0.0;
        d.repeat_penalty = 1.0;
        d.presence_penalty = 1.5;
        d.max_tokens = 32768;
        return d;
    }
    // Gemma 4 family: Unsloth-recommended defaults.
    // https://docs.unsloth.ai/docs/quick-connects/gemma-4
    else if (lower.contains("gemma-4") || lower.contains("gemma4"))
        && (lower.contains("2b")
            || lower.contains("4b")
            || lower.contains("15b")
            || lower.contains("26b")
            || lower.contains("31b"))
    {
        d.temperature = 1.0;
        d.top_p = 0.95;
        d.top_k = 64;
        d.min_p = 0.0;
        d.repeat_penalty = 1.0;
    }
    // Family-specific tweaks (applied first)
    else if lower.contains("llama") || lower.contains("meta-llama") {
        d.temperature = 0.7;
        d.top_p = 0.9;
        d.top_k = 40;
        d.min_p = 0.05;
        d.repeat_penalty = 1.1;
    } else if lower.contains("deepseek") {
        d.temperature = 0.6;
        d.top_p = 0.9;
        d.top_k = 40;
        d.min_p = 0.05;
        d.repeat_penalty = 1.1;
    } else if lower.contains("qwen") {
        d.temperature = 0.7;
        d.top_p = 0.8;
        d.top_k = 50;
        d.min_p = 0.05;
        d.repeat_penalty = 1.05;
    } else if lower.contains("mistral") || lower.contains("mixtral") {
        d.temperature = 0.6;
        d.top_p = 0.9;
        d.top_k = 40;
        d.min_p = 0.05;
        d.repeat_penalty = 1.1;
    } else if lower.contains("gemma") {
        d.temperature = 0.6;
        d.top_p = 0.95;
        d.top_k = 40;
        d.min_p = 0.05;
        d.repeat_penalty = 1.0;
    }

    // Code models: modestly lower temperature (after family tweaks),
    // but no longer overriding to 0.2 for all code models.
    if is_code && d.temperature > 0.4 {
        d.temperature = 0.3;
    }

    // MoE: slightly higher max_tokens.
    if is_moe {
        d.max_tokens = d.max_tokens.max(4096);
    }

    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_for_unknown_model() {
        let d = get_model_defaults("unknown-model.gguf", 1_000_000_000, &[]);
        assert_eq!(d.temperature, 0.7);
        assert_eq!(d.top_p, 0.9);
        assert_eq!(d.top_k, 40);
        assert_eq!(d.max_tokens, 2048);
    }

    #[test]
    fn code_model_lower_temperature() {
        let d = get_model_defaults(
            "codellama-7b-instruct.gguf",
            4_000_000_000,
            &["code".into()],
        );
        assert!(d.temperature <= 0.4);
        assert!(d.temperature > 0.2);
    }

    #[test]
    fn qwen36_27b_uses_unsloth_defaults() {
        // Source: https://docs.unsloth.ai/docs/quick-connects/qwen-3-6
        let d = get_model_defaults("Qwen3.6-27B-Instruct-Q8_0.gguf", 20_000_000_000, &[]);
        assert_eq!(d.temperature, 0.7);
        assert_eq!(d.top_p, 0.8);
        assert_eq!(d.top_k, 20);
        assert_eq!(d.min_p, 0.0);
        assert_eq!(d.repeat_penalty, 1.5);
    }

    #[test]
    fn qwen36_35b_a3b_uses_unsloth_defaults() {
        // Source: https://docs.unsloth.ai/docs/quick-connects/qwen-3-6
        let d = get_model_defaults("Qwen3.6-35B-A3B-Instruct-Q8_0.gguf", 25_000_000_000, &[]);
        assert_eq!(d.temperature, 0.7);
        assert_eq!(d.top_p, 0.8);
        assert_eq!(d.top_k, 20);
        assert_eq!(d.min_p, 0.0);
        assert_eq!(d.repeat_penalty, 1.5);
    }

    #[test]
    fn gemma4_e2b_uses_unsloth_defaults() {
        // Source: https://docs.unsloth.ai/docs/quick-connects/gemma-4
        let d = get_model_defaults("gemma-4-2b-it-Q8_0.gguf", 2_000_000_000, &[]);
        assert_eq!(d.temperature, 1.0);
        assert_eq!(d.top_p, 0.95);
        assert_eq!(d.top_k, 64);
        assert_eq!(d.min_p, 0.0);
        assert_eq!(d.repeat_penalty, 1.0);
    }

    #[test]
    fn gemma4_31b_uses_unsloth_defaults() {
        // Source: https://docs.unsloth.ai/docs/quick-connects/gemma-4
        let d = get_model_defaults("gemma-4-31b-it-Q8_0.gguf", 20_000_000_000, &[]);
        assert_eq!(d.temperature, 1.0);
        assert_eq!(d.top_p, 0.95);
        assert_eq!(d.top_k, 64);
        assert_eq!(d.min_p, 0.0);
        assert_eq!(d.repeat_penalty, 1.0);
    }

    #[test]
    fn gemma4_26b_a4b_uses_unsloth_defaults() {
        // Source: https://docs.unsloth.ai/docs/quick-connects/gemma-4
        let d = get_model_defaults("gemma-4-26b-a4b-it-Q8_0.gguf", 18_000_000_000, &[]);
        assert_eq!(d.temperature, 1.0);
        assert_eq!(d.top_p, 0.95);
        assert_eq!(d.top_k, 64);
        assert_eq!(d.min_p, 0.0);
        assert_eq!(d.repeat_penalty, 1.0);
    }

    #[test]
    fn moe_model_higher_max_tokens() {
        let d = get_model_defaults(
            "mixtral-8x7b-instruct.gguf",
            20_000_000_000,
            &["moe".into()],
        );
        assert!(d.max_tokens >= 4096);
    }

    #[test]
    fn exaone45_33b_uses_lg_official_defaults() {
        // Source: https://huggingface.co/LGAI-EXAONE/EXAONE-4.5-33B-GGUF
        // General purpose: temp=1.0, top_p=0.95, presence_penalty=1.5
        let d = get_model_defaults("EXAONE-4.5-33B-Q4_K_M.gguf", 20_000_000_000, &[]);
        assert_eq!(d.temperature, 1.0);
        assert_eq!(d.top_p, 0.95);
        assert_eq!(d.presence_penalty, 1.5);
        assert_eq!(d.repeat_penalty, 1.0);
    }
}
