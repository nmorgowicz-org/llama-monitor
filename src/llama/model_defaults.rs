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
            max_tokens: 2048,
        }
    }
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

    // Code models: lower temperature for deterministic output.
    if is_code {
        d.temperature = 0.2;
        d.top_p = 0.9;
        d.top_k = 20;
        d.min_p = 0.05;
        d.repeat_penalty = 1.1;
    }

    // Family-specific tweaks
    if lower.contains("llama") || lower.contains("meta-llama") {
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
}
