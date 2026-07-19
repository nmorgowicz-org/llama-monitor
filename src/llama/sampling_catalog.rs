//! Unified, provenance-bearing sampling mode catalog (Gap 3.4, A51/A52).
//!
//! Replaces `get_model_presets`/`SAMPLING_DEFAULTS` as public authority.
//! One Rust-owned catalog for both backends. Parsing adapters may still
//! populate it; frontend never duplicates this logic.

use crate::llama::model_defaults::{ModelDefaults, ModelPreset};

/// Stable sampling mode ID for cross-surface identification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplingModeId {
    /// Model-author or upstream-published defaults.
    ModelDefault,
    /// General chat: balanced temperature and penalties.
    General,
    /// Interactive coding agent: optimized for tool use, planning, code generation.
    CodingAgentic,
    /// Precise/deterministic: lower temperature, higher reliability.
    PreciseDeterministic,
    /// Creative/roleplay: wider sampling, character voice, fiction.
    CreativeRoleplay,
    /// Custom user configuration.
    Custom,
}

impl SamplingModeId {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ModelDefault => "model_default",
            Self::General => "general",
            Self::CodingAgentic => "coding_agentic",
            Self::PreciseDeterministic => "precise_deterministic",
            Self::CreativeRoleplay => "creative_roleplay",
            Self::Custom => "custom",
        }
    }
}

/// Source provenance for a sampling mode.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplingSource {
    /// Exact pinned Unsloth-published value (authoritative per A52).
    Unsloth { url: String, retrieval_date: String },
    /// Model-author config/card defaults.
    ModelAuthor { source: String },
    /// Qualified Rapid-MLX runtime alias profile.
    RuntimeAlias,
    /// Llama Monitor starting point (explicitly labeled guidance).
    LlamaMonitor,
}

/// Workload badges indicating appropriate use cases.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadBadge {
    General,
    Coding,
    Agentic,
    ToolUse,
    Creative,
    Roleplay,
    Deterministic,
    Reasoning,
    Fast,
}

/// A single sampling mode from the catalog.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SamplingMode {
    pub id: SamplingModeId,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(flatten)]
    pub defaults: ModelDefaults,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<SamplingSource>,
    pub workload_badges: Vec<WorkloadBadge>,
    /// Which fields are actually sent by the backend (omission-only semantics).
    /// `true` means this backend supports setting this field.
    pub llama_cpp_coverage: BackendFieldCoverage,
    pub rapid_mlx_coverage: BackendFieldCoverage,
}

/// Backend-native field coverage for a sampling mode.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BackendFieldCoverage {
    pub temperature: bool,
    pub top_p: bool,
    pub top_k: bool,
    pub min_p: bool,
    pub repeat_penalty: bool,
    pub presence_penalty: bool,
    pub frequency_penalty: bool,
    pub enable_thinking: bool,
    pub preserve_thinking: bool,
    pub reasoning_budget: bool,
}

impl BackendFieldCoverage {
    pub fn full() -> Self {
        Self {
            temperature: true,
            top_p: true,
            top_k: true,
            min_p: true,
            repeat_penalty: true,
            presence_penalty: true,
            frequency_penalty: false,
            enable_thinking: true,
            preserve_thinking: true,
            reasoning_budget: true,
        }
    }

    /// Rapid-MLX v0.10.12 coverage: --default-temperature, --default-top-p,
    /// --default-top-k, --default-min-p, --default-repetition-penalty,
    /// --default-presence-penalty, --default-frequency-penalty exist.
    /// Reasoning/thinking are model/template-controlled, not server defaults.
    pub fn rapid_mlx_current() -> Self {
        Self {
            temperature: true,
            top_p: true,
            top_k: true,
            min_p: true,
            repeat_penalty: true,
            presence_penalty: true,
            frequency_penalty: true,
            enable_thinking: false,
            preserve_thinking: false,
            reasoning_budget: false,
        }
    }
}

/// The authoritative cross-backend sampling catalog.
///
/// Resolves finetunes by architecture/family before filename.
/// Preserves all curated modes per A51. Provides universal choices
/// for every model.
pub struct SamplingCatalog;

impl SamplingCatalog {
    /// Get sampling modes for a model by family/architecture.
    ///
    /// # Args
    /// - `name_or_repo`: model name or HF repo ID
    /// - `size_bytes`: model file size (for sizing hints)
    /// - `tags`: optional tags like "moe", "vision", "code"
    /// - `gguf_arch`: GGUF `general.architecture` (for finetune resolution)
    /// - `arch_family`: client-side family detection (for opaque finetunes)
    /// - `backend`: which backend (affects field coverage reporting)
    ///
    /// Returns the full mode list; first is the recommended default.
    pub fn modes_for_model(
        name_or_repo: &str,
        _size_bytes: u64,
        _tags: &[String],
        gguf_arch: &str,
        arch_family: &str,
        _backend: crate::inference::InferenceBackend,
    ) -> Vec<SamplingMode> {
        let llama_coverage = BackendFieldCoverage::full();
        let rapid_coverage = BackendFieldCoverage::rapid_mlx_current();

        let lower = name_or_repo.to_ascii_lowercase();
        let arch_lower = gguf_arch.to_ascii_lowercase();
        let family_lower = arch_family.to_ascii_lowercase();

        // Qwen3.5 family: presets from https://unsloth.ai/docs/models/qwen3.5
        let is_qwen35 = lower.contains("qwen3.5")
            || lower.contains("qwen35")
            || arch_lower == "qwen3_5"
            || arch_lower == "qwen3.5"
            || family_lower == "qwen3.5";

        if is_qwen35 {
            return Self::qwen35_modes(llama_coverage, rapid_coverage);
        }

        // Qwen3.6 family (including Qwopus): presets from https://unsloth.ai/docs/models/qwen3.6
        let arch_is_qwen36 = matches!(
            arch_lower.as_str(),
            "qwen35" | "qwen35moe" | "qwen3_6" | "qwen3.6"
        );
        let is_qwen36 = arch_is_qwen36
            || family_lower == "qwen3.6"
            || (lower.contains("qwen3.6") || lower.contains("qwen36") || lower.contains("qwopus"))
                && (lower.contains("27b") || lower.contains("35b") || lower.contains("a3b"));

        if is_qwen36 {
            return Self::qwen36_modes(llama_coverage, rapid_coverage);
        }

        // Generic Qwen3 reasoning/distilled models
        let is_qwen3_reasoning = lower.contains("qwen3")
            && !is_qwen35
            && !is_qwen36
            && (lower.contains("reasoning")
                || lower.contains("thinking")
                || lower.contains("distill")
                || lower.contains("coder-next"));
        if is_qwen3_reasoning {
            return Self::qwen3_reasoning_modes(llama_coverage, rapid_coverage);
        }

        // EXAONE 4.5: general-purpose vs OCR/document
        if lower.contains("exaone-4.5") || lower.contains("exaone4.5") {
            return Self::exaone45_modes(llama_coverage, rapid_coverage);
        }

        // Gemma4 family
        let is_gemma4 = arch_lower == "gemma4"
            || family_lower == "gemma4"
            || (lower.contains("gemma-4") || lower.contains("gemma4"))
                && (lower.contains("2b")
                    || lower.contains("4b")
                    || lower.contains("12b")
                    || lower.contains("15b")
                    || lower.contains("26b")
                    || lower.contains("31b"));
        if is_gemma4 {
            return Self::gemma4_modes(llama_coverage, rapid_coverage);
        }

        // Universal fallback: A51 guarantees every model has General, Coding/Agentic,
        // Precise/Deterministic, Creative/Roleplay, and Custom.
        Self::universal_modes(llama_coverage, rapid_coverage)
    }

    /// Convert catalog modes to legacy ModelPreset format for backward compatibility.
    pub fn modes_as_presets(
        name_or_repo: &str,
        size_bytes: u64,
        tags: &[String],
        gguf_arch: &str,
        arch_family: &str,
    ) -> Vec<ModelPreset> {
        let modes = Self::modes_for_model(
            name_or_repo,
            size_bytes,
            tags,
            gguf_arch,
            arch_family,
            crate::inference::InferenceBackend::LlamaCpp,
        );
        modes
            .into_iter()
            .map(|m| ModelPreset {
                name: m.name,
                description: m.description,
                defaults: m.defaults,
            })
            .collect()
    }

    // ------------------------------------------------------------------
    // Family-specific mode sets
    // ------------------------------------------------------------------

    fn unsloth_qwen_source() -> SamplingSource {
        SamplingSource::Unsloth {
            url: "https://unsloth.ai/docs/models/qwen3.5".into(),
            retrieval_date: "2026-07-18".into(),
        }
    }

    fn unsloth_qwen36_source() -> SamplingSource {
        SamplingSource::Unsloth {
            url: "https://unsloth.ai/docs/models/qwen3.6".into(),
            retrieval_date: "2026-07-18".into(),
        }
    }

    fn exaone_source() -> SamplingSource {
        SamplingSource::ModelAuthor {
            source: "https://huggingface.co/LGAI-EXAONE/EXAONE-4.5-33B-GGUF".into(),
        }
    }

    fn qwen35_modes(
        llama_cov: BackendFieldCoverage,
        rapid_cov: BackendFieldCoverage,
    ) -> Vec<SamplingMode> {
        vec![
            SamplingMode {
                id: SamplingModeId::General,
                name: "Thinking (General)".into(),
                description: Some(
                    "Recommended default for general chat and reasoning tasks.".into(),
                ),
                defaults: ModelDefaults {
                    temperature: 1.0,
                    top_p: 0.95,
                    top_k: 20,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 1.5,
                    max_tokens: 32768,
                    enable_thinking: Some(true),
                    preserve_thinking: None,
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: true,
                    reasoning_budget_message: None,
                },
                provenance: Some(Self::unsloth_qwen_source()),
                workload_badges: vec![WorkloadBadge::General, WorkloadBadge::Reasoning],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::CodingAgentic,
                name: "Thinking (Agentic / Coding)".into(),
                description: Some(
                    "Optimized for agentic tool use, planning, and code generation.".into(),
                ),
                defaults: ModelDefaults {
                    temperature: 1.0,
                    top_p: 0.95,
                    top_k: 20,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 0.0,
                    max_tokens: 32768,
                    enable_thinking: Some(true),
                    preserve_thinking: None,
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: true,
                    reasoning_budget_message: None,
                },
                provenance: Some(Self::unsloth_qwen_source()),
                workload_badges: vec![
                    WorkloadBadge::Coding,
                    WorkloadBadge::Agentic,
                    WorkloadBadge::ToolUse,
                ],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::PreciseDeterministic,
                name: "Thinking (Precise coding)".into(),
                description: Some(
                    "Lower temperature for deterministic coding and debugging.".into(),
                ),
                defaults: ModelDefaults {
                    temperature: 0.6,
                    top_p: 0.95,
                    top_k: 20,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 0.0,
                    max_tokens: 32768,
                    enable_thinking: Some(true),
                    preserve_thinking: None,
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: true,
                    reasoning_budget_message: None,
                },
                provenance: Some(Self::unsloth_qwen_source()),
                workload_badges: vec![WorkloadBadge::Coding, WorkloadBadge::Deterministic],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::CreativeRoleplay,
                name: "Non-thinking (General)".into(),
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
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(Self::unsloth_qwen_source()),
                workload_badges: vec![WorkloadBadge::General],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::Custom,
                name: "Non-thinking (Reasoning)".into(),
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
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(Self::unsloth_qwen_source()),
                workload_badges: vec![WorkloadBadge::Reasoning],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
        ]
    }

    fn qwen36_modes(
        llama_cov: BackendFieldCoverage,
        rapid_cov: BackendFieldCoverage,
    ) -> Vec<SamplingMode> {
        vec![
            SamplingMode {
                id: SamplingModeId::CodingAgentic,
                name: "Agentic / Coding (thinking)".into(),
                description: Some(
                    "Recommended default for coding agents and tool-heavy work.".into(),
                ),
                defaults: ModelDefaults {
                    temperature: 1.0,
                    top_p: 0.95,
                    top_k: 20,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 0.0,
                    max_tokens: 32768,
                    enable_thinking: Some(true),
                    preserve_thinking: Some(true),
                    tool_call_format: None,
                    reasoning_budget: Some(16384),
                    reasoning: true,
                    reasoning_budget_message: Some("\nFinal Answer:".into()),
                },
                provenance: Some(Self::unsloth_qwen36_source()),
                workload_badges: vec![
                    WorkloadBadge::Coding,
                    WorkloadBadge::Agentic,
                    WorkloadBadge::ToolUse,
                    WorkloadBadge::Reasoning,
                ],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::CreativeRoleplay,
                name: "Creative / Roleplay (thinking)".into(),
                description: Some(
                    "Higher presence penalty for looser, more exploratory outputs.".into(),
                ),
                defaults: ModelDefaults {
                    temperature: 1.0,
                    top_p: 0.95,
                    top_k: 20,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 1.5,
                    max_tokens: 32768,
                    enable_thinking: Some(true),
                    preserve_thinking: Some(true),
                    tool_call_format: None,
                    reasoning_budget: Some(16384),
                    reasoning: true,
                    reasoning_budget_message: Some("\nFinal Answer:".into()),
                },
                provenance: Some(Self::unsloth_qwen36_source()),
                workload_badges: vec![
                    WorkloadBadge::Creative,
                    WorkloadBadge::Roleplay,
                    WorkloadBadge::Reasoning,
                ],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::PreciseDeterministic,
                name: "Precise coding (thinking)".into(),
                description: Some(
                    "Lower temperature for deterministic coding and debugging.".into(),
                ),
                defaults: ModelDefaults {
                    temperature: 0.6,
                    top_p: 0.95,
                    top_k: 20,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 0.0,
                    max_tokens: 32768,
                    enable_thinking: Some(true),
                    preserve_thinking: Some(true),
                    tool_call_format: None,
                    reasoning_budget: Some(16384),
                    reasoning: true,
                    reasoning_budget_message: Some("\nFinal Answer:".into()),
                },
                provenance: Some(Self::unsloth_qwen36_source()),
                workload_badges: vec![
                    WorkloadBadge::Coding,
                    WorkloadBadge::Deterministic,
                    WorkloadBadge::Reasoning,
                ],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::General,
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
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(Self::unsloth_qwen36_source()),
                workload_badges: vec![WorkloadBadge::General],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::Custom,
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
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(Self::unsloth_qwen36_source()),
                workload_badges: vec![WorkloadBadge::Reasoning],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
        ]
    }

    fn qwen3_reasoning_modes(
        llama_cov: BackendFieldCoverage,
        rapid_cov: BackendFieldCoverage,
    ) -> Vec<SamplingMode> {
        vec![
            SamplingMode {
                id: SamplingModeId::General,
                name: "Thinking (General)".into(),
                description: Some(
                    "Recommended default for general chat and reasoning tasks.".into(),
                ),
                defaults: ModelDefaults {
                    temperature: 1.0,
                    top_p: 0.95,
                    top_k: 20,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 1.5,
                    max_tokens: 32768,
                    enable_thinking: Some(true),
                    preserve_thinking: None,
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: true,
                    reasoning_budget_message: None,
                },
                provenance: Some(SamplingSource::LlamaMonitor),
                workload_badges: vec![WorkloadBadge::General, WorkloadBadge::Reasoning],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::CodingAgentic,
                name: "Thinking (Coding)".into(),
                description: Some(
                    "Lower temperature for deterministic coding and debugging.".into(),
                ),
                defaults: ModelDefaults {
                    temperature: 0.6,
                    top_p: 0.95,
                    top_k: 20,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 0.0,
                    max_tokens: 32768,
                    enable_thinking: Some(true),
                    preserve_thinking: None,
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: true,
                    reasoning_budget_message: None,
                },
                provenance: Some(SamplingSource::LlamaMonitor),
                workload_badges: vec![WorkloadBadge::Coding, WorkloadBadge::Agentic],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::Custom,
                name: "Non-thinking".into(),
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
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(SamplingSource::LlamaMonitor),
                workload_badges: vec![WorkloadBadge::General],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
        ]
    }

    fn exaone45_modes(
        llama_cov: BackendFieldCoverage,
        rapid_cov: BackendFieldCoverage,
    ) -> Vec<SamplingMode> {
        vec![
            SamplingMode {
                id: SamplingModeId::General,
                name: "General purpose".into(),
                description: Some(
                    "Balanced default for chat, analysis, and mixed workloads.".into(),
                ),
                defaults: ModelDefaults {
                    temperature: 1.0,
                    top_p: 0.95,
                    top_k: 0,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 1.5,
                    max_tokens: 2048,
                    enable_thinking: None,
                    preserve_thinking: None,
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(Self::exaone_source()),
                workload_badges: vec![WorkloadBadge::General],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::PreciseDeterministic,
                name: "OCR / Document".into(),
                description: Some(
                    "Lower temperature preset for extraction and document-focused tasks.".into(),
                ),
                defaults: ModelDefaults {
                    temperature: 0.6,
                    top_p: 0.95,
                    top_k: 20,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 1.5,
                    max_tokens: 2048,
                    enable_thinking: None,
                    preserve_thinking: None,
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(Self::exaone_source()),
                workload_badges: vec![WorkloadBadge::Deterministic],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
        ]
    }

    fn gemma4_modes(
        llama_cov: BackendFieldCoverage,
        rapid_cov: BackendFieldCoverage,
    ) -> Vec<SamplingMode> {
        vec![
            SamplingMode {
                id: SamplingModeId::General,
                name: "General".into(),
                description: Some("Recommended default for everyday chat and analysis.".into()),
                defaults: ModelDefaults {
                    temperature: 1.0,
                    top_p: 0.95,
                    top_k: 64,
                    min_p: 0.0,
                    repeat_penalty: 1.0,
                    presence_penalty: 0.0,
                    max_tokens: 8192,
                    enable_thinking: Some(true),
                    preserve_thinking: None,
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(SamplingSource::ModelAuthor {
                    source: "https://docs.unsloth.ai/docs/quick-connects/gemma-4".into(),
                }),
                workload_badges: vec![WorkloadBadge::General],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::CreativeRoleplay,
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
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(SamplingSource::ModelAuthor {
                    source: "https://docs.unsloth.ai/docs/quick-connects/gemma-4".into(),
                }),
                workload_badges: vec![WorkloadBadge::Creative, WorkloadBadge::Roleplay],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::CodingAgentic,
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
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(SamplingSource::ModelAuthor {
                    source: "https://docs.unsloth.ai/docs/quick-connects/gemma-4".into(),
                }),
                workload_badges: vec![
                    WorkloadBadge::Coding,
                    WorkloadBadge::Agentic,
                    WorkloadBadge::ToolUse,
                ],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
        ]
    }

    /// Universal modes guaranteed for every model (A51).
    fn universal_modes(
        llama_cov: BackendFieldCoverage,
        rapid_cov: BackendFieldCoverage,
    ) -> Vec<SamplingMode> {
        vec![
            SamplingMode {
                id: SamplingModeId::General,
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
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(SamplingSource::LlamaMonitor),
                workload_badges: vec![WorkloadBadge::General],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::CodingAgentic,
                name: "Coding / Agentic".into(),
                description: Some("Optimized for code generation and tool use.".into()),
                defaults: ModelDefaults {
                    temperature: 0.5,
                    top_p: 0.9,
                    top_k: 40,
                    min_p: 0.05,
                    repeat_penalty: 1.1,
                    presence_penalty: 0.0,
                    max_tokens: 4096,
                    enable_thinking: None,
                    preserve_thinking: None,
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(SamplingSource::LlamaMonitor),
                workload_badges: vec![WorkloadBadge::Coding, WorkloadBadge::Agentic],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::PreciseDeterministic,
                name: "Precise / Deterministic".into(),
                description: Some("Lowest temperature for deterministic outputs.".into()),
                defaults: ModelDefaults {
                    temperature: 0.2,
                    top_p: 0.9,
                    top_k: 20,
                    min_p: 0.0,
                    repeat_penalty: 1.1,
                    presence_penalty: 0.0,
                    max_tokens: 4096,
                    enable_thinking: None,
                    preserve_thinking: None,
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(SamplingSource::LlamaMonitor),
                workload_badges: vec![WorkloadBadge::Deterministic],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
            SamplingMode {
                id: SamplingModeId::CreativeRoleplay,
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
                    tool_call_format: None,
                    reasoning_budget: None,
                    reasoning: false,
                    reasoning_budget_message: None,
                },
                provenance: Some(SamplingSource::LlamaMonitor),
                workload_badges: vec![WorkloadBadge::Creative, WorkloadBadge::Roleplay],
                llama_cpp_coverage: llama_cov.clone(),
                rapid_mlx_coverage: rapid_cov.clone(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwen36_all_modes_visible() {
        let modes = SamplingCatalog::modes_for_model(
            "Qwen3.6-30B-A3B",
            0,
            &[],
            "",
            "",
            crate::inference::InferenceBackend::LlamaCpp,
        );
        assert_eq!(modes.len(), 5);
        assert_eq!(modes[0].id, SamplingModeId::CodingAgentic);
        assert_eq!(modes[0].name, "Agentic / Coding (thinking)");
        assert_eq!(modes[1].name, "Creative / Roleplay (thinking)");
        assert_eq!(modes[2].name, "Precise coding (thinking)");
    }

    #[test]
    fn qwen36_finetune_via_gguf_arch() {
        let modes = SamplingCatalog::modes_for_model(
            "Qwable-v1.Q5_K_M.gguf",
            0,
            &[],
            "qwen35moe",
            "",
            crate::inference::InferenceBackend::LlamaCpp,
        );
        assert_eq!(modes.len(), 5);
        assert_eq!(modes[0].id, SamplingModeId::CodingAgentic);
    }

    #[test]
    fn qwen35_all_modes() {
        let modes = SamplingCatalog::modes_for_model(
            "Qwen3.5-122B-A10B-GGUF",
            0,
            &[],
            "",
            "",
            crate::inference::InferenceBackend::LlamaCpp,
        );
        assert_eq!(modes.len(), 5);
        assert_eq!(modes[0].id, SamplingModeId::General);
        assert_eq!(modes[0].name, "Thinking (General)");
    }

    #[test]
    fn gemma4_three_modes() {
        let modes = SamplingCatalog::modes_for_model(
            "gemma-4-26b-it",
            0,
            &[],
            "",
            "",
            crate::inference::InferenceBackend::LlamaCpp,
        );
        assert_eq!(modes.len(), 3);
        assert_eq!(modes[0].id, SamplingModeId::General);
        assert_eq!(modes[1].id, SamplingModeId::CreativeRoleplay);
        assert_eq!(modes[2].id, SamplingModeId::CodingAgentic);
    }

    #[test]
    fn universal_fallback_has_all_choices() {
        let modes = SamplingCatalog::modes_for_model(
            "my-custom-rp-model.gguf",
            0,
            &[],
            "",
            "",
            crate::inference::InferenceBackend::LlamaCpp,
        );
        assert!(modes.len() >= 4);
        let ids: Vec<_> = modes.iter().map(|m| m.id).collect();
        assert!(ids.contains(&SamplingModeId::General));
        assert!(ids.contains(&SamplingModeId::CodingAgentic));
        assert!(ids.contains(&SamplingModeId::PreciseDeterministic));
        assert!(ids.contains(&SamplingModeId::CreativeRoleplay));
    }

    #[test]
    fn rapid_mlx_coverage_is_honest() {
        let modes = SamplingCatalog::modes_for_model(
            "Qwen3.6-30B-A3B",
            0,
            &[],
            "",
            "",
            crate::inference::InferenceBackend::RapidMlx,
        );
        for mode in &modes {
            assert!(mode.rapid_mlx_coverage.temperature);
            assert!(mode.rapid_mlx_coverage.top_p);
            assert!(mode.rapid_mlx_coverage.top_k);
            assert!(!mode.rapid_mlx_coverage.enable_thinking);
            assert!(!mode.rapid_mlx_coverage.preserve_thinking);
            assert!(!mode.rapid_mlx_coverage.reasoning_budget);
        }
    }

    #[test]
    fn unsloth_values_are_exact() {
        // Source: https://unsloth.ai/docs/models/qwen3.6
        let modes = SamplingCatalog::modes_for_model(
            "Qwen3.6-30B-A3B",
            0,
            &[],
            "",
            "",
            crate::inference::InferenceBackend::LlamaCpp,
        );
        let agentic = modes
            .iter()
            .find(|m| m.id == SamplingModeId::CodingAgentic)
            .unwrap();
        assert_eq!(agentic.defaults.temperature, 1.0);
        assert_eq!(agentic.defaults.top_p, 0.95);
        assert_eq!(agentic.defaults.top_k, 20);
        assert_eq!(agentic.defaults.presence_penalty, 0.0);
        assert_eq!(agentic.defaults.enable_thinking, Some(true));
        assert_eq!(agentic.defaults.preserve_thinking, Some(true));
        assert_eq!(agentic.defaults.reasoning_budget, Some(16384));
    }
}
