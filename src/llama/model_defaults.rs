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
    /// --chat-template-kwargs {"enable_thinking": ...} — toggles CoT reasoning.
    /// None means the flag is not set (server default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_thinking: Option<bool>,
    /// --chat-template-kwargs {"preserve_thinking": ...} — include prior reasoning
    /// in multi-turn context (Qwen3.6 series).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preserve_thinking: Option<bool>,
    /// --reasoning-budget N: -1 = unlimited, 0 = disabled, N = token cap.
    /// None means the flag is not set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_budget: Option<i32>,
    /// --reasoning flag: whether to enable reasoning/extended CoT behavior.
    /// True when this model family benefits from structured reasoning.
    pub reasoning: bool,
    /// --reasoning-budget-message: text that marks the start of final answer.
    /// None means the flag is not set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_budget_message: Option<String>,
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
            max_tokens: 32768,
            enable_thinking: None,
            preserve_thinking: None,
            reasoning_budget: None,
            reasoning: false,
            reasoning_budget_message: None,
        }
    }
}

/// A named sampling preset — shown as a pill in the wizard's review step.
/// The wizard auto-selects the first preset and lets users switch between them.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelPreset {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(flatten)]
    pub defaults: ModelDefaults,
}

/// Return named presets for models that have meaningful mode variants
/// (e.g. Qwen3.6 thinking-general vs thinking-coding).
/// The first preset is the recommended default.
/// Returns generic fallback presets for models with no special modes.
pub fn get_model_presets(name_or_repo: &str, size_bytes: u64, tags: &[String]) -> Vec<ModelPreset> {
    let lower = name_or_repo.to_ascii_lowercase();

    // Qwen3.5 family: hybrid-reasoning MoE + dense, thinking-enabled.
    // Presets from https://unsloth.ai/docs/models/qwen3.5
    let is_qwen35 = lower.contains("qwen3.5") || lower.contains("qwen35");

    if is_qwen35 {
        let thinking_general = ModelDefaults {
            temperature: 1.0,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 1.5,
            max_tokens: 32768,
            enable_thinking: Some(true),
            preserve_thinking: None,
            reasoning_budget: None,
            reasoning: true,
            reasoning_budget_message: None,
        };
        let thinking_agentic_coding = ModelDefaults {
            temperature: 1.0,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 0.0,
            max_tokens: 32768,
            enable_thinking: Some(true),
            preserve_thinking: None,
            reasoning_budget: None,
            reasoning: true,
            reasoning_budget_message: None,
        };
        let thinking_precise_coding = ModelDefaults {
            temperature: 0.6,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 0.0,
            max_tokens: 32768,
            enable_thinking: Some(true),
            preserve_thinking: None,
            reasoning_budget: None,
            reasoning: true,
            reasoning_budget_message: None,
        };
        let not_thinking_general = ModelDefaults {
            temperature: 0.7,
            top_p: 0.8,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 1.5,
            max_tokens: 32768,
            enable_thinking: Some(false),
            preserve_thinking: Some(false),
            reasoning_budget: None,
            reasoning: false,
            reasoning_budget_message: None,
        };
        let not_thinking_reasoning = ModelDefaults {
            temperature: 1.0,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 1.5,
            max_tokens: 32768,
            enable_thinking: Some(false),
            preserve_thinking: Some(false),
            reasoning_budget: None,
            reasoning: false,
            reasoning_budget_message: None,
        };
        return vec![
            ModelPreset {
                name: "Thinking (General)".into(),
                description: Some(
                    "Recommended default for general chat and reasoning tasks.".into(),
                ),
                defaults: thinking_general,
            },
            ModelPreset {
                name: "Thinking (Agentic / Coding)".into(),
                description: Some(
                    "Optimized for agentic tool use, planning, and code generation.".into(),
                ),
                defaults: thinking_agentic_coding,
            },
            ModelPreset {
                name: "Thinking (Precise coding)".into(),
                description: Some(
                    "Lower temperature for deterministic coding and debugging.".into(),
                ),
                defaults: thinking_precise_coding,
            },
            ModelPreset {
                name: "Non-thinking (General)".into(),
                description: Some("Balanced chat mode with thinking explicitly disabled.".into()),
                defaults: not_thinking_general,
            },
            ModelPreset {
                name: "Non-thinking (Reasoning)".into(),
                description: Some("High-entropy chat mode without visible thinking blocks.".into()),
                defaults: not_thinking_reasoning,
            },
        ];
    }

    // Qwen3.6 family (including Qwopus and other derivatives):
    // preset values from https://unsloth.ai/docs/models/qwen3.6
    let is_qwen36 =
        (lower.contains("qwen3.6") || lower.contains("qwen36") || lower.contains("qwopus"))
            && (lower.contains("27b") || lower.contains("35b") || lower.contains("a3b"));

    if is_qwen36 {
        let agentic_coding = ModelDefaults {
            temperature: 1.0,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 0.0,
            max_tokens: 32768,
            enable_thinking: Some(true),
            preserve_thinking: Some(true),
            reasoning_budget: Some(16384),
            reasoning: true,
            reasoning_budget_message: Some("\nFinal Answer:".into()),
        };
        let creative_roleplay = ModelDefaults {
            temperature: 1.0,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 1.5,
            max_tokens: 32768,
            enable_thinking: Some(true),
            preserve_thinking: Some(true),
            reasoning_budget: Some(16384),
            reasoning: true,
            reasoning_budget_message: Some("\nFinal Answer:".into()),
        };
        let precise_coding = ModelDefaults {
            temperature: 0.6,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 0.0,
            max_tokens: 32768,
            enable_thinking: Some(true),
            preserve_thinking: Some(true),
            reasoning_budget: Some(16384),
            reasoning: true,
            reasoning_budget_message: Some("\nFinal Answer:".into()),
        };
        return vec![
            ModelPreset {
                name: "Agentic / Coding (thinking)".into(),
                description: Some(
                    "Recommended default for coding agents and tool-heavy work.".into(),
                ),
                defaults: agentic_coding,
            },
            ModelPreset {
                name: "Creative / Roleplay (thinking)".into(),
                description: Some(
                    "Higher presence penalty for looser, more exploratory outputs.".into(),
                ),
                defaults: creative_roleplay,
            },
            ModelPreset {
                name: "Precise coding (thinking)".into(),
                description: Some(
                    "Lower temperature for deterministic coding and debugging.".into(),
                ),
                defaults: precise_coding,
            },
            ModelPreset {
                name: "Non-thinking general".into(),
                description: Some("Balanced chat mode with thinking explicitly disabled.".into()),
                defaults: ModelDefaults {
                    temperature: 0.7,
                    top_p: 0.8,
                    top_k: 20,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 1.5,
                    max_tokens: 32768,
                    enable_thinking: Some(false),
                    preserve_thinking: Some(false),
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
            },
            ModelPreset {
                name: "Non-thinking reasoning".into(),
                description: Some("High-entropy chat mode without visible thinking blocks.".into()),
                defaults: ModelDefaults {
                    temperature: 1.0,
                    top_p: 0.95,
                    top_k: 20,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 1.5,
                    max_tokens: 32768,
                    enable_thinking: Some(false),
                    preserve_thinking: Some(false),
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
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
            enable_thinking: None,
            preserve_thinking: None,
            reasoning_budget: None,
            reasoning: false,
            reasoning_budget_message: None,
        };
        let ocr = ModelDefaults {
            temperature: 0.6,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 1.5,
            max_tokens: 2048,
            enable_thinking: None,
            preserve_thinking: None,
            reasoning_budget: None,
            reasoning: false,
            reasoning_budget_message: None,
        };
        return vec![
            ModelPreset {
                name: "General purpose".into(),
                description: Some(
                    "Balanced default for chat, analysis, and mixed workloads.".into(),
                ),
                defaults: general,
            },
            ModelPreset {
                name: "OCR / Document".into(),
                description: Some(
                    "Lower temperature preset for extraction and document-focused tasks.".into(),
                ),
                defaults: ocr,
            },
        ];
    }

    if (lower.contains("gemma-4") || lower.contains("gemma4"))
        && (lower.contains("2b")
            || lower.contains("4b")
            || lower.contains("12b")
            || lower.contains("15b")
            || lower.contains("26b")
            || lower.contains("31b"))
    {
        return vec![
            ModelPreset {
                name: "General".into(),
                description: Some("Recommended default for everyday chat and analysis.".into()),
                defaults: get_model_defaults(name_or_repo, size_bytes, tags),
            },
            ModelPreset {
                name: "Creative / Roleplay".into(),
                description: Some(
                    "Slightly wider sampling for character voice and fiction.".into(),
                ),
                defaults: ModelDefaults {
                    temperature: 1.0,
                    top_p: 0.97,
                    top_k: 64,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 0.0,
                    max_tokens: 8192,
                    enable_thinking: Some(true),
                    preserve_thinking: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
            },
            ModelPreset {
                name: "Precise / Agentic".into(),
                description: Some(
                    "Lower temperature preset for structured tool use and coding.".into(),
                ),
                defaults: ModelDefaults {
                    temperature: 0.7,
                    top_p: 0.95,
                    top_k: 64,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 0.25,
                    max_tokens: 8192,
                    enable_thinking: Some(true),
                    preserve_thinking: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
            },
        ];
    }

    vec![
        ModelPreset {
            name: "General".into(),
            description: Some("Safe fallback for unknown or older GGUFs.".into()),
            defaults: ModelDefaults {
                temperature: 0.9,
                top_p: 0.95,
                top_k: 64,
                min_p: 0.03,
                repeat_penalty: 1.05,
                presence_penalty: 0.0,
                max_tokens: 4096,
                enable_thinking: None,
                preserve_thinking: None,
                reasoning_budget: None,
                reasoning: false,
                reasoning_budget_message: None,
            },
        },
        ModelPreset {
            name: "Creative / Roleplay".into(),
            description: Some(
                "Broader fallback preset for finetunes and RP-oriented models.".into(),
            ),
            defaults: ModelDefaults {
                temperature: 1.0,
                top_p: 0.97,
                top_k: 100,
                min_p: 0.02,
                repeat_penalty: 1.1,
                presence_penalty: 0.1,
                max_tokens: 4096,
                enable_thinking: None,
                preserve_thinking: None,
                reasoning_budget: None,
                reasoning: false,
                reasoning_budget_message: None,
            },
        },
    ]
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
        d.enable_thinking = Some(true);
        d.preserve_thinking = Some(true);
        d.reasoning_budget = Some(16384);
        d.reasoning = true;
        d.reasoning_budget_message = Some("\nFinal Answer:".into());
        return d;
    }
    // Gemma 4 family: Unsloth deployment guide defaults.
    // https://unsloth.ai/docs — temp=1.0, top_p=0.95, top_k=64, enable_thinking=true
    else if (lower.contains("gemma-4") || lower.contains("gemma4"))
        && (lower.contains("2b")
            || lower.contains("4b")
            || lower.contains("12b")
            || lower.contains("15b")
            || lower.contains("26b")
            || lower.contains("31b"))
    {
        d.temperature = 1.0;
        d.top_p = 0.95;
        d.top_k = 64;
        d.min_p = 0.0;
        d.repeat_penalty = 1.0;
        d.enable_thinking = Some(true);
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
        assert_eq!(d.max_tokens, 32768);
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
    fn gemma4_12b_uses_google_defaults() {
        let d = get_model_defaults("gemma-4-12B-it-qat-Q4_0.gguf", 6_700_000_000, &[]);
        assert_eq!(d.temperature, 1.0);
        assert_eq!(d.top_p, 0.95);
        assert_eq!(d.top_k, 64);
        assert_eq!(d.enable_thinking, Some(true));
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

    #[test]
    fn qwen36_exposes_all_planned_presets() {
        let presets = get_model_presets("Qwen3.6-30B-A3B", 0, &[]);
        assert_eq!(presets.len(), 5);
        assert_eq!(presets[0].name, "Agentic / Coding (thinking)");
        assert_eq!(presets[1].name, "Creative / Roleplay (thinking)");
        assert_eq!(presets[2].name, "Precise coding (thinking)");
        assert_eq!(presets[3].defaults.enable_thinking, Some(false));
        assert!(!presets[4].defaults.reasoning);
    }

    #[test]
    fn qwen35_122b_a10b_exposes_presets_including_agentic() {
        // Source: https://unsloth.ai/docs/models/qwen3.5
        let presets = get_model_presets("Qwen3.5-122B-A10B-GGUF", 0, &[]);
        assert_eq!(presets.len(), 5);
        assert_eq!(presets[0].name, "Thinking (General)");
        assert_eq!(presets[1].name, "Thinking (Agentic / Coding)");
        assert_eq!(presets[2].name, "Thinking (Precise coding)");
        assert_eq!(
            presets[1].defaults.temperature, 1.0,
            "agentic uses temp 1.0"
        );
        assert_eq!(
            presets[2].defaults.temperature, 0.6,
            "precise uses temp 0.6"
        );
        assert!(presets[1].defaults.reasoning, "agentic has reasoning");
        assert!(
            presets[3].defaults.enable_thinking == Some(false),
            "non-thinking general"
        );
    }

    #[test]
    fn qwen35_reap_variant_gets_qwen35_presets() {
        // REAP (expert-pruned) variants must still match Qwen3.5 presets.
        let presets = get_model_presets("Qwen3.5-122B-A10B-REAP-20-i1-GGUF", 0, &[]);
        assert_eq!(presets.len(), 5);
        assert_eq!(presets[0].name, "Thinking (General)");
    }

    #[test]
    fn gemma4_exposes_three_presets() {
        let presets = get_model_presets("gemma-4-26b-it", 0, &[]);
        assert_eq!(presets.len(), 3);
        assert_eq!(presets[0].name, "General");
        assert_eq!(presets[1].name, "Creative / Roleplay");
        assert_eq!(presets[2].name, "Precise / Agentic");
    }

    #[test]
    fn generic_models_get_fallback_presets() {
        let presets = get_model_presets("my-custom-rp-model.gguf", 0, &[]);
        assert_eq!(presets.len(), 2);
        assert_eq!(presets[0].name, "General");
        assert_eq!(presets[1].name, "Creative / Roleplay");
    }
}
