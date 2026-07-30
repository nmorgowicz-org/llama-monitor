//! Workload-scenario definitions for VRAM estimation.
//!
//! Replaces generic "8k context" quant comparison with real usage patterns.
//! Each scenario encodes typical memory characteristics for a concrete workload.
//!
//! Phase 5a Part 4, Builder item 10: scenario metadata only — no estimator math here.
//! These types are consumed by quant_comparison_table and full_estimate to produce
//! workload-fit recommendations instead of generic-memory recommendations.
//!
//! NOTE: many helper methods (name, description, as_key, etc.) are public API
//! consumed by the UI/API in Phase 7 — dead_code allowed until then.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// ── KV dtype recommendation per workload ──────────────────────────────────────

/// Recommended KV cache dtype for a workload scenario.
///
/// Used for Rapid-MLX estimates (maps to KvCacheDtype in execution_policy.rs).
/// For llama.cpp, this is advisory: the quant comparison still uses q8_0/q4_0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RecommendedKvDtype {
    /// Full-precision KV: reasoning, complex tool-calling, long-context accuracy.
    Bf16,
    /// 8-bit KV: balanced coherence/memory (default for agentic/coding).
    #[default]
    Int8,
    /// 4-bit KV: memory-efficient, acceptable for roleplay/long-context storytelling.
    Int4,
}

impl RecommendedKvDtype {
    /// Serialize to the string used in Rapid-MLX CLI (--kv-cache-dtype).
    pub fn as_cli_value(self) -> &'static str {
        match self {
            Self::Bf16 => "bf16",
            Self::Int8 => "int8",
            Self::Int4 => "int4",
        }
    }
}

// ── Workload scenario ─────────────────────────────────────────────────────────

/// A workload scenario representing a concrete usage pattern.
///
/// Each scenario defines the typical memory footprint characteristics:
/// - Planning context tokens: tokens needed for the current generation sequence
/// - Retained cache tokens: prefix cache kept between turns
/// - Slot count: parallel generation slots (affects active KV)
/// - KV dtype recommendation: minimum for acceptable coherence
/// - TurboQuant eligibility hint: whether retained-cache compression is beneficial
///
/// These are metadata definitions; the estimator math lives in estimate.rs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkloadScenario {
    /// Interactive chat in the built-in chat UI or an external chat client.
    /// Single-session, moderate context, single-slot.
    InteractiveChat {
        /// Configurable planning context window. Default: 32K tokens.
        planning_context_tokens: u64,
        /// Prefix cache between turns. Default: 8K tokens.
        retained_cache_tokens: u64,
    },
    /// Coding or agentic session with tool calls, retained context, planning window.
    /// Higher retained cache for context reuse across tool calls.
    /// This is the canonical workload (A32: 80% priority).
    CodingAgent {
        /// Configurable planning context window. Default: 128K tokens.
        planning_context_tokens: u64,
        /// Retained cache for tool-call context and conversation history. Default: 32K tokens.
        retained_cache_tokens: u64,
    },
    /// Tool/research agent: external client, may need multi-slot for overlapping agents.
    /// High retained cache for multi-session history.
    /// Second-priority workload (A32: 20% of canonical).
    ToolResearchAgent {
        /// Configurable planning context window. Default: 128K tokens.
        planning_context_tokens: u64,
        /// Retained cache for multi-session history and tools. Default: 48K tokens.
        retained_cache_tokens: u64,
        /// Slot count for overlapping agent sessions. Default: 2.
        /// When >1 and MTP active, produces D25 warning.
        parallel_slots: u32,
    },
    /// Batch/evaluation workload: high concurrency, short-lived sessions.
    /// Minimal retained cache; many slots; no long-term reuse.
    BatchEval {
        /// Configurable planning context window. Default: 8K tokens.
        planning_context_tokens: u64,
        /// Prefix cache retained between batches. Default: 0 (no retention).
        retained_cache_tokens: u64,
        /// Slot count for concurrent batch jobs. Default: 4.
        parallel_slots: u32,
    },
    /// Roleplay or storytelling: long-context, retained context for continuity.
    /// Lower KV precision acceptable; single-slot typical.
    Roleplay {
        /// Configurable planning context window. Default: 64K tokens.
        planning_context_tokens: u64,
        /// Retained cache for story/character continuity. Default: 32K tokens.
        retained_cache_tokens: u64,
    },
}

impl Default for WorkloadScenario {
    /// Default is CodingAgent (canonical workload per A32).
    fn default() -> Self {
        Self::CodingAgent {
            planning_context_tokens: 128_000,
            retained_cache_tokens: 32_000,
        }
    }
}

impl WorkloadScenario {
    /// Human-readable name for API/UI display.
    pub fn name(&self) -> &'static str {
        match self {
            Self::InteractiveChat { .. } => "Interactive chat",
            Self::CodingAgent { .. } => "Coding / agentic",
            Self::ToolResearchAgent { .. } => "Tool / research agent",
            Self::BatchEval { .. } => "Batch / eval",
            Self::Roleplay { .. } => "Roleplay / storytelling",
        }
    }

    /// One-sentence description for teaching.
    pub fn description(&self) -> &'static str {
        match self {
            Self::InteractiveChat { .. } => {
                "Single-session chat with moderate context. Optimized for response quality in built-in chat."
            }
            Self::CodingAgent { .. } => {
                "Tool-calling coding agent with retained context. Balanced for coherence and context reuse across generations."
            }
            Self::ToolResearchAgent { .. } => {
                "External research agent with multi-slot concurrency and long history. Prioritizes retained cache for overlapping sessions."
            }
            Self::BatchEval { .. } => {
                "High-concurrency batch processing with short-lived sessions. Minimal cache; maximum throughput."
            }
            Self::Roleplay { .. } => {
                "Long-context creative writing with character and story continuity. Lower KV precision acceptable."
            }
        }
    }

    /// Planning context tokens for the scenario.
    pub fn planning_context_tokens(&self) -> u64 {
        match self {
            Self::InteractiveChat {
                planning_context_tokens,
                ..
            } => *planning_context_tokens,
            Self::CodingAgent {
                planning_context_tokens,
                ..
            } => *planning_context_tokens,
            Self::ToolResearchAgent {
                planning_context_tokens,
                ..
            } => *planning_context_tokens,
            Self::BatchEval {
                planning_context_tokens,
                ..
            } => *planning_context_tokens,
            Self::Roleplay {
                planning_context_tokens,
                ..
            } => *planning_context_tokens,
        }
    }

    /// Retained prefix-cache tokens for the scenario.
    pub fn retained_cache_tokens(&self) -> u64 {
        match self {
            Self::InteractiveChat {
                retained_cache_tokens,
                ..
            } => *retained_cache_tokens,
            Self::CodingAgent {
                retained_cache_tokens,
                ..
            } => *retained_cache_tokens,
            Self::ToolResearchAgent {
                retained_cache_tokens,
                ..
            } => *retained_cache_tokens,
            Self::BatchEval {
                retained_cache_tokens,
                ..
            } => *retained_cache_tokens,
            Self::Roleplay {
                retained_cache_tokens,
                ..
            } => *retained_cache_tokens,
        }
    }

    /// Parallel slot count for the scenario.
    pub fn parallel_slots(&self) -> u32 {
        match self {
            Self::InteractiveChat { .. } => 1,
            Self::CodingAgent { .. } => 1,
            Self::ToolResearchAgent { parallel_slots, .. } => (*parallel_slots).max(1),
            Self::BatchEval { parallel_slots, .. } => (*parallel_slots).max(1),
            Self::Roleplay { .. } => 1,
        }
    }

    /// Recommended KV dtype for this workload.
    ///
    /// Used for Rapid-MLX estimates. For llama.cpp, quant comparison
    /// uses the existing agentic/general/roleplay logic.
    pub fn recommended_kv_dtype(&self) -> RecommendedKvDtype {
        match self {
            Self::InteractiveChat { .. } => RecommendedKvDtype::Int8,
            Self::CodingAgent { .. } => RecommendedKvDtype::Int8, // agentic: q8_0 minimum
            Self::ToolResearchAgent { .. } => RecommendedKvDtype::Int8, // tool-calling accuracy
            Self::BatchEval { .. } => RecommendedKvDtype::Int4,   // throughput over precision
            Self::Roleplay { .. } => RecommendedKvDtype::Int4,    // context > precision
        }
    }

    /// Whether TurboQuant (retained-prefix compression) is beneficial for this scenario.
    ///
    /// Only scenarios with significant retained cache benefit from TurboQuant.
    pub fn turboquant_eligible(&self) -> bool {
        match self {
            Self::CodingAgent { .. } => true,
            Self::ToolResearchAgent { .. } => true,
            Self::InteractiveChat {
                retained_cache_tokens,
                ..
            } => *retained_cache_tokens >= 8_000,
            Self::Roleplay {
                retained_cache_tokens,
                ..
            } => *retained_cache_tokens >= 16_000,
            Self::BatchEval { .. } => false, // minimal retained cache
        }
    }

    /// Whether this scenario typically needs multi-slot concurrency.
    ///
    /// Used for D25 MTP admission warnings: multi-slot + MTP = conflict.
    pub fn needs_multi_slot(&self) -> bool {
        match self {
            Self::ToolResearchAgent { parallel_slots, .. } => *parallel_slots > 1,
            Self::BatchEval { parallel_slots, .. } => *parallel_slots > 1,
            _ => false,
        }
    }

    /// Whether this scenario is appropriate for MTP (speculative decoding).
    ///
    /// Per D25: capability ≠ recommendation. MTP is eligible for single-stream
    /// workloads but not automatically recommended for all scenarios.
    ///
    /// This answers the *concurrency* half of admission only. The decode-shape
    /// half (greedy sampling, no logits processors) lives in
    /// [`DecodeRequestShape::permits_speculation`] because it is a property of
    /// the request, not of the scenario. Both halves must hold; see
    /// [`MtpAdmissionResult::compute`].
    pub fn mtp_eligible(&self) -> bool {
        // Multi-slot scenarios conflict with MTP's single-active constraint.
        !self.needs_multi_slot()
    }

    /// The decode shape this scenario is *expected* to run under.
    ///
    /// These are product-defined defaults, not measured values: they describe
    /// how a typical request in this scenario is shaped so admission can tell
    /// the user whether a speculative scheduler would engage at all. A caller
    /// that knows the real request parameters should pass those instead of
    /// these defaults.
    pub fn default_decode_shape(&self) -> DecodeRequestShape {
        match self {
            // Chat and roleplay are sampled for variety.
            Self::InteractiveChat { .. } => DecodeRequestShape {
                temperature: 0.7,
                constrained_tools: false,
                reasoning: false,
            },
            Self::Roleplay { .. } => DecodeRequestShape {
                temperature: 0.9,
                constrained_tools: false,
                reasoning: false,
            },
            // Coding and research agents sample *and* install a tool grammar.
            Self::CodingAgent { .. } => DecodeRequestShape {
                temperature: 0.6,
                constrained_tools: true,
                reasoning: true,
            },
            Self::ToolResearchAgent { .. } => DecodeRequestShape {
                temperature: 0.6,
                constrained_tools: true,
                reasoning: true,
            },
            // Batch eval is the one scenario that is reproducibility-driven,
            // so it is the one shape a greedy-only scheduler can serve.
            Self::BatchEval { .. } => DecodeRequestShape {
                temperature: 0.0,
                constrained_tools: false,
                reasoning: false,
            },
        }
    }

    /// Serialize as the scenario key string used in API queries.
    pub fn as_key(&self) -> &'static str {
        match self {
            Self::InteractiveChat { .. } => "interactive_chat",
            Self::CodingAgent { .. } => "coding_agent",
            Self::ToolResearchAgent { .. } => "tool_research_agent",
            Self::BatchEval { .. } => "batch_eval",
            Self::Roleplay { .. } => "roleplay",
        }
    }

    /// Parse a WorkloadScenario from an API key string.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "interactive_chat" => Some(Self::InteractiveChat {
                planning_context_tokens: 32_000,
                retained_cache_tokens: 8_000,
            }),
            "coding_agent" => Some(Self::CodingAgent {
                planning_context_tokens: 128_000,
                retained_cache_tokens: 32_000,
            }),
            "tool_research_agent" => Some(Self::ToolResearchAgent {
                planning_context_tokens: 128_000,
                retained_cache_tokens: 48_000,
                parallel_slots: 2,
            }),
            "batch_eval" => Some(Self::BatchEval {
                planning_context_tokens: 8_000,
                retained_cache_tokens: 0,
                parallel_slots: 4,
            }),
            "roleplay" => Some(Self::Roleplay {
                planning_context_tokens: 64_000,
                retained_cache_tokens: 32_000,
            }),
            _ => None,
        }
    }

    /// Resolve the stable product-profile IDs used by the wizard as well as
    /// the estimator's compact scenario keys.  Profiles and scenarios are not
    /// the same UI concept, but they must map to one memory policy everywhere.
    pub fn from_profile_or_key(key: &str) -> Option<Self> {
        let canonical = match key {
            "interactive_coding_agent" => "coding_agent",
            "roleplay_storytelling" => "roleplay",
            "general_chat" => "interactive_chat",
            "deterministic_batch_eval" => "batch_eval",
            other => other,
        };
        Self::from_key(canonical)
    }

    /// Returns all predefined scenario keys.
    pub fn all_keys() -> &'static [&'static str] {
        &[
            "interactive_chat",
            "coding_agent",
            "tool_research_agent",
            "batch_eval",
            "roleplay",
        ]
    }
}

// ── Estimator input derived from scenario ─────────────────────────────────────

/// Estimator parameters derived from a workload scenario.
///
/// This is the typed bridge between scenario metadata and the estimator.
/// It is NOT a scenario itself; it is what the estimator needs from a scenario.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ScenarioEstimatorParams {
    /// Tokens for active KV computation (planning context).
    pub planning_context_tokens: u64,
    /// Tokens for retained prefix cache (between-turn reuse).
    pub retained_cache_tokens: u64,
    /// Parallel generation slots.
    pub parallel_slots: u32,
    /// Recommended KV dtype (Rapid-MLX only).
    pub recommended_kv_dtype: RecommendedKvDtype,
    /// Whether TurboQuant is beneficial for this scenario.
    pub turboquant_eligible: bool,
    /// Whether multi-slot concurrency is needed.
    pub needs_multi_slot: bool,
    /// Whether MTP is appropriate for this scenario.
    pub mtp_eligible: bool,
    /// Client type: built-in app or external client.
    pub client_type: ClientType,
}

/// Whether the client is llama-monitor's built-in chat or an external client.
///
/// External clients have different compaction ownership and context pressure.
/// Per Builder item 14: this drives external_client_fit vs app_fit variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    /// Built-in llama-monitor chat UI.
    /// Typically single-slot, moderate context, managed compaction.
    #[default]
    App,
    /// External client (OpenCode, Hermes, SillyTavern, etc.).
    /// Higher retained cache, external compaction ownership, possible multi-slot.
    ExternalClient,
}

impl WorkloadScenario {
    /// Derive estimator parameters from this scenario.
    pub fn to_estimator_params(self, client_type: ClientType) -> ScenarioEstimatorParams {
        ScenarioEstimatorParams {
            planning_context_tokens: self.planning_context_tokens(),
            retained_cache_tokens: self.retained_cache_tokens(),
            parallel_slots: self.parallel_slots(),
            recommended_kv_dtype: self.recommended_kv_dtype(),
            turboquant_eligible: self.turboquant_eligible(),
            needs_multi_slot: self.needs_multi_slot(),
            mtp_eligible: self.mtp_eligible(),
            client_type,
        }
    }
}

// ── Rapid MTP modeling (Builder item 13) ──────────────────────────────────────

/// Rapid-MLX Multi-Token Prediction mode for estimation.
///
/// Per D25: MTP eligibility/dispatch stops qualification for unknown combinations.
/// Capability ≠ recommendation (D25): MTP eligible ≠ MTP should be default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MtpMode {
    /// MTP disabled; standard autoregressive decoding.
    #[default]
    Disabled,
    /// Embedded MTP (Qwen3-style draft layers in same process).
    /// MTP weights counted via ModelMemoryProfile.mtp_components.
    /// mtp_bytes included in primary MemoryBreakdown.
    /// No separate process; same server handles MTP.
    Embedded,
    /// External drafter (separate model process).
    /// Drafter's weights + KV counted separately as ExternalCompanion.
    /// Total = primary_estimate.total_bytes + drafter_estimate.total_bytes.
    External,
}

/// An external companion model (drafter, vision, embedding).
///
/// Per A25: each drafter, vision tower, and embedding model is an explicit
/// source component with separate download, provenance, lifecycle, and additive memory.
///
/// For MTP: represents the external drafter model in External mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCompanion {
    /// Human-readable label (e.g. "Qwen3-0.6B drafter").
    pub label: String,
    /// Companion type.
    #[serde(rename = "type")]
    pub companion_type: CompanionType,
    /// Total memory bytes for this companion (weights + KV + overhead).
    pub total_bytes: u64,
    /// Model weights bytes (resident).
    pub weights_bytes: u64,
    /// KV cache bytes for this companion.
    pub kv_cache_bytes: u64,
    /// Source identifier (HF repo, local path, or builtin).
    pub source: String,
}

/// Type of external companion model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionType {
    /// External speculative decoding drafter.
    Drafter,
    /// Vision encoder (mmproj + vision tower).
    Vision,
    /// Text embedding model.
    Embedding,
}

/// D25 admission result for MTP + concurrency interaction.
///
/// Per D25: Rapid MTP uses single-active-greedy fast path (one primary stream at a time).
/// If MTP active and user requests multi-slot concurrency, this captures the fit/warning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MtpAdmissionResult {
    /// MTP mode that will be used.
    pub mode: MtpMode,
    /// Whether MTP is eligible for this model (based on architecture/capability).
    pub eligible: bool,
    /// Whether MTP is recommended for this workload scenario.
    /// Capability ≠ recommendation (D25).
    pub recommended_for_workload: bool,
    /// Whether the speculative scheduler would actually engage under the decode
    /// shape this workload runs at.
    ///
    /// Distinct from `recommended_for_workload`: a model can be eligible and the
    /// scenario single-stream, and the scheduler still never run because the
    /// request is sampled or installs a logits processor. When this is false the
    /// MTP head is paid for in memory and never used.
    pub engages_for_workload: bool,
    /// Why the scheduler would fall through to plain autoregressive decode.
    /// Empty when `engages_for_workload` is true.
    pub fallthroughs: Vec<SpecDecodeFallthrough>,
    /// Warnings about MTP + concurrency interaction.
    pub warnings: Vec<MtpWarning>,
    /// The concurrency policy that will be used.
    pub concurrency_policy: ConcurrencyPolicy,
}

/// Per-request decode conditions that determine whether a speculative
/// scheduler can engage, independent of model architecture.
///
/// Rapid-MLX 0.11.1's MTP scheduler is greedy-only and bails when any logits
/// processor is installed, so either condition alone silently disables
/// speculation. See `docs/reference/rapid-mlx-mtp-evidence.md`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DecodeRequestShape {
    /// Sampling temperature. Anything above zero is non-greedy.
    pub temperature: f32,
    /// Whether a constrained tool grammar is installed for this request.
    pub constrained_tools: bool,
    /// Whether reasoning/thinking mode is on.
    pub reasoning: bool,
}

impl Default for DecodeRequestShape {
    /// Greedy, unconstrained: the only shape a greedy-only scheduler serves.
    fn default() -> Self {
        Self {
            temperature: 0.0,
            constrained_tools: false,
            reasoning: false,
        }
    }
}

impl DecodeRequestShape {
    /// Whether a greedy-only speculative scheduler can engage for this request.
    pub fn permits_speculation(&self) -> bool {
        self.fallthroughs().is_empty()
    }

    /// The reasons speculation would fall through for this request, if any.
    pub fn fallthroughs(&self) -> Vec<SpecDecodeFallthrough> {
        let mut out = Vec::new();
        if self.temperature > 0.0 {
            out.push(SpecDecodeFallthrough::NonGreedySampling);
        }
        if self.constrained_tools {
            out.push(SpecDecodeFallthrough::LogitsProcessorInstalled);
        }
        out
    }
}

/// Why a speculative scheduler falls through to plain autoregressive decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecDecodeFallthrough {
    /// Temperature above zero; the scheduler only handles argmax.
    NonGreedySampling,
    /// A logits processor (constrained tool grammar) is installed.
    LogitsProcessorInstalled,
}

/// Inputs to [`MtpAdmissionResult::compute_with_shape`].
///
/// Grouped rather than passed positionally because admission now depends on
/// both model facts and request facts, and the two are easy to transpose.
#[derive(Debug, Clone)]
pub struct MtpAdmissionParams<'a> {
    /// Requested MTP mode.
    pub mtp_mode: MtpMode,
    /// Workload scenario.
    pub scenario: &'a WorkloadScenario,
    /// MTP depth from architecture (0 = no MTP).
    pub arch_mtp_depth: u32,
    /// Requested parallel slots.
    pub parallel_slots: u32,
    /// Concurrency policy.
    pub concurrency_policy: ConcurrencyPolicy,
    /// Decode shape requests in this workload will actually use.
    pub decode_shape: DecodeRequestShape,
}

/// Warnings about MTP admission and concurrency interaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MtpWarning {
    /// Multi-slot requested but MTP requires single-stream.
    MultiSlotConflictsWithSingleStreamMtp,
    /// MTP eligible but not recommended for this workload.
    MtpEligibleButNotRecommended,
    /// External drafter requires separate model download.
    ExternalDrafterRequiresSeparateDownload,
    /// MTP eligibility unknown; cannot qualify.
    MtpEligibilityUnknown,
    /// Model and scenario both qualify, but the decode shape this workload runs
    /// at makes the speculative scheduler fall through every request.
    SchedulerFallsThroughForWorkload,
}

/// Concurrency policy for MTP admission (D25).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConcurrencyPolicy {
    /// Single active generation (D25 default for near-capacity fitting).
    /// MTP is permitted.
    #[default]
    SingleActive,
    /// Allow overlap (Advanced); multiple active generations permitted.
    /// MTP is disabled for overlapping streams.
    AllowOverlap,
}

impl MtpAdmissionResult {
    /// Compute D25 admission result given MTP configuration, scenario, and architecture.
    ///
    /// `mtp_mode`: requested MTP mode
    /// `scenario`: workload scenario
    /// `arch_mtp_depth`: MTP depth from architecture (0 = no MTP)
    /// `parallel_slots`: requested parallel slots
    /// `concurrency_policy`: concurrency policy
    ///
    /// The decode shape is taken from [`WorkloadScenario::default_decode_shape`].
    /// Use [`Self::compute_with_shape`] when the real request parameters are known.
    pub fn compute(
        mtp_mode: MtpMode,
        scenario: &WorkloadScenario,
        arch_mtp_depth: u32,
        parallel_slots: u32,
        concurrency_policy: ConcurrencyPolicy,
    ) -> Self {
        Self::compute_with_shape(MtpAdmissionParams {
            mtp_mode,
            scenario,
            arch_mtp_depth,
            parallel_slots,
            concurrency_policy,
            decode_shape: scenario.default_decode_shape(),
        })
    }

    /// Compute D25 admission with an explicit decode shape.
    pub fn compute_with_shape(params: MtpAdmissionParams<'_>) -> Self {
        let MtpAdmissionParams {
            mtp_mode,
            scenario,
            arch_mtp_depth,
            parallel_slots,
            concurrency_policy,
            decode_shape,
        } = params;
        let mut warnings = Vec::new();

        // Check eligibility: MTP requires arch support.
        // Unknown combinations stop qualification per D25.
        let eligible = match mtp_mode {
            MtpMode::Disabled => false,
            MtpMode::Embedded => arch_mtp_depth > 0,
            MtpMode::External => true, // external drafter is self-qualified by presence
        };

        // If requesting MTP but arch doesn't support it, flag unknown.
        if matches!(mtp_mode, MtpMode::Embedded) && arch_mtp_depth == 0 {
            warnings.push(MtpWarning::MtpEligibilityUnknown);
        }

        // External drafter warning: requires separate download.
        if mtp_mode == MtpMode::External {
            warnings.push(MtpWarning::ExternalDrafterRequiresSeparateDownload);
        }

        // Would the scheduler engage at all under this workload's decode shape?
        // A sampled or grammar-constrained request never reaches it.
        let mtp_requested = matches!(mtp_mode, MtpMode::Embedded | MtpMode::External);
        let fallthroughs = if mtp_requested {
            decode_shape.fallthroughs()
        } else {
            Vec::new()
        };
        let engages_for_workload = eligible && fallthroughs.is_empty();

        // D25: MTP capability ≠ recommendation.
        // Eligible, single-stream, and actually reached are three separate gates.
        let workload_needs_mtp = scenario.mtp_eligible();
        let recommended_for_workload = eligible && workload_needs_mtp && engages_for_workload;
        if eligible && !workload_needs_mtp && mtp_requested {
            warnings.push(MtpWarning::MtpEligibleButNotRecommended);
        }
        if eligible && workload_needs_mtp && mtp_requested && !fallthroughs.is_empty() {
            warnings.push(MtpWarning::SchedulerFallsThroughForWorkload);
        }

        // D25: MTP requires single-stream. Multi-slot conflicts.
        let effective_concurrency_policy = match (mtp_mode, concurrency_policy, parallel_slots > 1)
        {
            (MtpMode::Disabled, _, _) => concurrency_policy,
            // MTP + single active = fine
            (MtpMode::Embedded | MtpMode::External, ConcurrencyPolicy::SingleActive, _) => {
                ConcurrencyPolicy::SingleActive
            }
            // MTP + overlap requested + multi-slot = conflict warning
            (MtpMode::Embedded | MtpMode::External, ConcurrencyPolicy::AllowOverlap, true) => {
                warnings.push(MtpWarning::MultiSlotConflictsWithSingleStreamMtp);
                // MTP uses primary-only path; multi-slot agents queue sequentially.
                ConcurrencyPolicy::SingleActive
            }
            // MTP + overlap but single slot = OK
            (MtpMode::Embedded | MtpMode::External, ConcurrencyPolicy::AllowOverlap, false) => {
                ConcurrencyPolicy::SingleActive
            }
        };

        Self {
            mode: mtp_mode,
            eligible,
            recommended_for_workload,
            engages_for_workload,
            fallthroughs,
            warnings,
            concurrency_policy: effective_concurrency_policy,
        }
    }

    /// Returns true if any warnings would block or require user confirmation.
    pub fn has_conflicts(&self) -> bool {
        self.warnings.iter().any(|w| {
            matches!(
                w,
                MtpWarning::MultiSlotConflictsWithSingleStreamMtp
                    | MtpWarning::MtpEligibilityUnknown
                    | MtpWarning::SchedulerFallsThroughForWorkload
            )
        })
    }
}

/// MTP configuration for estimator options.
///
/// Passed via EstimatorOptions to model MTP memory costs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MtpConfig {
    /// Requested MTP mode.
    pub mode: MtpMode,
    /// Depth of embedded MTP heads (from architecture). Zero = no embedded MTP.
    pub embedded_depth: u32,
    /// External drafter companion estimate (if mode = External).
    pub external_drafter: Option<ExternalCompanion>,
}

impl MtpConfig {
    /// Returns the total MTP bytes to include in the estimate.
    ///
    /// - Embedded: uses mtp_overhead_bytes based on depth
    /// - External: uses external_drafter.total_bytes
    /// - Disabled: 0
    pub fn total_mtp_bytes(&self, primary_model_bytes: u64) -> u64 {
        match self.mode {
            MtpMode::Disabled => 0,
            MtpMode::Embedded => {
                if self.embedded_depth > 0 {
                    crate::llama::vram_estimator::mtp_overhead_bytes(
                        primary_model_bytes,
                        self.embedded_depth,
                    )
                } else {
                    0
                }
            }
            MtpMode::External => self
                .external_drafter
                .as_ref()
                .map(|c| c.total_bytes)
                .unwrap_or(0),
        }
    }

    /// Returns the MTP bytes counted as part of the primary process (for MemoryBreakdown).
    ///
    /// - Embedded: full mtp_bytes in primary breakdown
    /// - External: 0 in primary breakdown (counted in companion)
    /// - Disabled: 0
    pub fn primary_breakdown_mtp_bytes(&self, primary_model_bytes: u64) -> u64 {
        match self.mode {
            MtpMode::Disabled => 0,
            MtpMode::Embedded => {
                if self.embedded_depth > 0 {
                    crate::llama::vram_estimator::mtp_overhead_bytes(
                        primary_model_bytes,
                        self.embedded_depth,
                    )
                } else {
                    0
                }
            }
            MtpMode::External => 0, // external companion counted separately
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scenario_is_coding_agent() {
        let scenario = WorkloadScenario::default();
        assert_eq!(scenario.name(), "Coding / agentic");
        assert_eq!(scenario.planning_context_tokens(), 128_000);
        assert_eq!(scenario.retained_cache_tokens(), 32_000);
    }

    #[test]
    fn all_scenarios_serialize_deserialize() {
        for key in WorkloadScenario::all_keys() {
            let scenario = WorkloadScenario::from_key(key).unwrap();
            let json = serde_json::to_string(&scenario).unwrap();
            let restored: WorkloadScenario = serde_json::from_str(&json).unwrap();
            assert_eq!(restored.as_key(), scenario.as_key());
        }
    }

    #[test]
    fn coding_agent_is_coding_agent_default() {
        let scenario = WorkloadScenario::CodingAgent {
            planning_context_tokens: 128_000,
            retained_cache_tokens: 32_000,
        };
        assert_eq!(scenario.name(), "Coding / agentic");
        assert_eq!(scenario.parallel_slots(), 1);
        assert!(!scenario.needs_multi_slot());
        assert!(scenario.mtp_eligible());
        assert!(scenario.turboquant_eligible());
    }

    #[test]
    fn tool_research_agent_multi_slot() {
        let scenario = WorkloadScenario::ToolResearchAgent {
            planning_context_tokens: 128_000,
            retained_cache_tokens: 48_000,
            parallel_slots: 2,
        };
        assert_eq!(scenario.parallel_slots(), 2);
        assert!(scenario.needs_multi_slot());
        assert!(!scenario.mtp_eligible()); // multi-slot conflicts with MTP
    }

    #[test]
    fn batch_eval_no_mtp() {
        let scenario = WorkloadScenario::BatchEval {
            planning_context_tokens: 8_000,
            retained_cache_tokens: 0,
            parallel_slots: 4,
        };
        assert_eq!(scenario.parallel_slots(), 4);
        assert!(scenario.needs_multi_slot());
        assert!(!scenario.mtp_eligible());
        assert!(!scenario.turboquant_eligible());
    }

    #[test]
    fn roleplay_low_kv_precision() {
        let scenario = WorkloadScenario::Roleplay {
            planning_context_tokens: 64_000,
            retained_cache_tokens: 32_000,
        };
        assert_eq!(scenario.recommended_kv_dtype(), RecommendedKvDtype::Int4);
        assert!(scenario.mtp_eligible());
    }

    #[test]
    fn product_profile_ids_resolve_to_estimator_scenarios() {
        assert!(matches!(
            WorkloadScenario::from_profile_or_key("interactive_coding_agent"),
            Some(WorkloadScenario::CodingAgent { .. })
        ));
        assert!(matches!(
            WorkloadScenario::from_profile_or_key("roleplay_storytelling"),
            Some(WorkloadScenario::Roleplay { .. })
        ));
        assert!(matches!(
            WorkloadScenario::from_profile_or_key("deterministic_batch_eval"),
            Some(WorkloadScenario::BatchEval { .. })
        ));
    }

    #[test]
    fn distinct_scenarios_produce_distinct_params() {
        let coding = WorkloadScenario::CodingAgent {
            planning_context_tokens: 128_000,
            retained_cache_tokens: 32_000,
        }
        .to_estimator_params(ClientType::App);

        let roleplay = WorkloadScenario::Roleplay {
            planning_context_tokens: 64_000,
            retained_cache_tokens: 32_000,
        }
        .to_estimator_params(ClientType::App);

        // Different planning context
        assert_ne!(
            coding.planning_context_tokens, roleplay.planning_context_tokens,
            "Coding agent and roleplay must have different planning context"
        );
        // Different KV dtype recommendation
        assert_ne!(
            coding.recommended_kv_dtype, roleplay.recommended_kv_dtype,
            "Coding agent and roleplay must have different KV dtype"
        );
    }

    #[test]
    fn external_client_params_distinct_from_app() {
        let scenario = WorkloadScenario::CodingAgent {
            planning_context_tokens: 128_000,
            retained_cache_tokens: 32_000,
        };

        let app_params = scenario.to_estimator_params(ClientType::App);
        let ext_params = scenario.to_estimator_params(ClientType::ExternalClient);

        // Scenario params are identical except client_type
        assert_eq!(
            app_params.planning_context_tokens,
            ext_params.planning_context_tokens
        );
        assert_eq!(
            app_params.retained_cache_tokens,
            ext_params.retained_cache_tokens
        );
        // But client_type differs
        assert_ne!(app_params.client_type, ext_params.client_type);
    }

    #[test]
    fn recommended_kv_dtype_as_cli_values() {
        assert_eq!(RecommendedKvDtype::Bf16.as_cli_value(), "bf16");
        assert_eq!(RecommendedKvDtype::Int8.as_cli_value(), "int8");
        assert_eq!(RecommendedKvDtype::Int4.as_cli_value(), "int4");
    }

    #[test]
    fn from_key_unknown_returns_none() {
        assert!(WorkloadScenario::from_key("unknown_scenario").is_none());
        assert!(WorkloadScenario::from_key("").is_none());
    }

    // ── Builder item 13: MTP modeling tests ───────────────────────────────────

    #[test]
    fn mtp_mode_serializes_correctly() {
        assert_eq!(
            serde_json::to_string(&MtpMode::Disabled).unwrap(),
            "\"disabled\""
        );
        assert_eq!(
            serde_json::to_string(&MtpMode::Embedded).unwrap(),
            "\"embedded\""
        );
        assert_eq!(
            serde_json::to_string(&MtpMode::External).unwrap(),
            "\"external\""
        );
    }

    #[test]
    fn embedded_mtp_eligible_with_depth() {
        let scenario = WorkloadScenario::CodingAgent {
            planning_context_tokens: 128_000,
            retained_cache_tokens: 32_000,
        };
        // Greedy shape isolates architecture/concurrency admission from the
        // decode-shape gate, which has its own test below.
        let result = MtpAdmissionResult::compute_with_shape(MtpAdmissionParams {
            mtp_mode: MtpMode::Embedded,
            scenario: &scenario,
            arch_mtp_depth: 3, // arch has MTP depth 3
            parallel_slots: 1, // single slot
            concurrency_policy: ConcurrencyPolicy::SingleActive,
            decode_shape: DecodeRequestShape::default(),
        });
        assert!(
            result.eligible,
            "Embedded MTP should be eligible when arch has depth > 0"
        );
        assert!(
            !result.has_conflicts(),
            "No conflicts for single-slot greedy with embedded MTP"
        );
        assert!(
            result.engages_for_workload,
            "Greedy unconstrained decode should reach the scheduler"
        );
    }

    #[test]
    fn embedded_mtp_not_eligible_without_depth() {
        let result = MtpAdmissionResult::compute(
            MtpMode::Embedded,
            &WorkloadScenario::CodingAgent {
                planning_context_tokens: 128_000,
                retained_cache_tokens: 32_000,
            },
            0, // no MTP depth in arch
            1,
            ConcurrencyPolicy::SingleActive,
        );
        assert!(
            !result.eligible,
            "Embedded MTP not eligible when arch has no depth"
        );
        assert!(
            result.warnings.contains(&MtpWarning::MtpEligibilityUnknown),
            "Must warn when MTP eligibility unknown"
        );
    }

    #[test]
    fn external_mtp_requires_download_warning() {
        let result = MtpAdmissionResult::compute(
            MtpMode::External,
            &WorkloadScenario::CodingAgent {
                planning_context_tokens: 128_000,
                retained_cache_tokens: 32_000,
            },
            0,
            1,
            ConcurrencyPolicy::SingleActive,
        );
        assert!(
            result
                .warnings
                .contains(&MtpWarning::ExternalDrafterRequiresSeparateDownload),
            "External MTP must warn about separate download"
        );
    }

    /// The test above hands `compute` a slot count of 2 directly, which proves the rule and
    /// proves nothing about whether any caller ever supplies it. `/api/vram-estimate` did not:
    /// it read `parallel_slots` from the request body, defaulted to 1 on absence, and never
    /// consulted the scenario, so the warning above was unreachable for a caller that had only
    /// picked a workload. Route the slot count the way a caller has to.
    #[test]
    fn a_multi_slot_scenario_supplies_the_slot_count_that_triggers_the_conflict() {
        let scenario = WorkloadScenario::ToolResearchAgent {
            planning_context_tokens: 128_000,
            retained_cache_tokens: 48_000,
            parallel_slots: 2,
        };
        let slots = scenario.to_estimator_params(ClientType::App).parallel_slots;
        assert_eq!(slots, 2, "the scenario must carry its own slot count");

        let result = MtpAdmissionResult::compute(
            MtpMode::Embedded,
            &scenario,
            3,
            slots,
            ConcurrencyPolicy::AllowOverlap,
        );
        assert!(
            result
                .warnings
                .contains(&MtpWarning::MultiSlotConflictsWithSingleStreamMtp)
        );

        // A caller that states its own slot count still wins: scenario values fill gaps only.
        let single = MtpAdmissionResult::compute(
            MtpMode::Embedded,
            &scenario,
            3,
            1,
            ConcurrencyPolicy::AllowOverlap,
        );
        assert!(
            !single
                .warnings
                .contains(&MtpWarning::MultiSlotConflictsWithSingleStreamMtp)
        );
    }

    #[test]
    fn d25_multi_slot_conflicts_with_mtp() {
        let scenario = WorkloadScenario::ToolResearchAgent {
            planning_context_tokens: 128_000,
            retained_cache_tokens: 48_000,
            parallel_slots: 2,
        };
        let result = MtpAdmissionResult::compute(
            MtpMode::Embedded,
            &scenario,
            3,
            2,
            ConcurrencyPolicy::AllowOverlap,
        );
        assert!(
            result
                .warnings
                .contains(&MtpWarning::MultiSlotConflictsWithSingleStreamMtp),
            "Multi-slot must conflict with single-stream MTP"
        );
        assert!(
            result.has_conflicts(),
            "Multi-slot + MTP must have conflicts"
        );
        // Effective policy should be SingleActive (MTP primary path)
        assert_eq!(
            result.concurrency_policy,
            ConcurrencyPolicy::SingleActive,
            "MTP forces single-active policy"
        );
    }

    #[test]
    fn capability_not_recommendation() {
        // Batch eval scenario: MTP eligible by arch but not recommended for workload.
        let scenario = WorkloadScenario::BatchEval {
            planning_context_tokens: 8_000,
            retained_cache_tokens: 0,
            parallel_slots: 4,
        };
        let result = MtpAdmissionResult::compute(
            MtpMode::Embedded,
            &scenario,
            3,
            1, // single slot for this test
            ConcurrencyPolicy::SingleActive,
        );
        // Eligible by architecture
        assert!(result.eligible);
        // But not recommended for batch eval workload
        assert!(
            !result.recommended_for_workload,
            "MTP should not be recommended for batch eval"
        );
    }

    #[test]
    fn coding_agent_eligible_but_scheduler_falls_through() {
        // A coding agent is single-stream, so it clears the concurrency gate,
        // but it samples and installs a tool grammar — so a greedy-only,
        // processor-intolerant scheduler never runs. Eligible must not imply
        // recommended here: the MTP head would be paid for and never used.
        let scenario = WorkloadScenario::CodingAgent {
            planning_context_tokens: 128_000,
            retained_cache_tokens: 32_000,
        };
        let result = MtpAdmissionResult::compute(
            MtpMode::Embedded,
            &scenario,
            3,
            1,
            ConcurrencyPolicy::SingleActive,
        );
        assert!(result.eligible, "arch depth 3 is eligible");
        assert!(scenario.mtp_eligible(), "single-stream clears concurrency");
        assert!(
            !result.engages_for_workload,
            "sampled + grammar-constrained decode never reaches the scheduler"
        );
        assert!(
            !result.recommended_for_workload,
            "must not recommend MTP for a workload where it cannot engage"
        );
        assert!(
            result
                .fallthroughs
                .contains(&SpecDecodeFallthrough::NonGreedySampling),
            "temperature 0.6 is a fallthrough reason"
        );
        assert!(
            result
                .fallthroughs
                .contains(&SpecDecodeFallthrough::LogitsProcessorInstalled),
            "tool grammar is a fallthrough reason"
        );
        assert!(
            result
                .warnings
                .contains(&MtpWarning::SchedulerFallsThroughForWorkload),
            "user must be told the head will not be used"
        );
    }

    #[test]
    fn batch_eval_greedy_shape_reaches_scheduler() {
        // Batch eval is the one default shape that can speculate; with a single
        // slot it should engage and be recommended.
        let scenario = WorkloadScenario::BatchEval {
            planning_context_tokens: 8_000,
            retained_cache_tokens: 0,
            parallel_slots: 1,
        };
        assert!(scenario.default_decode_shape().permits_speculation());
        let result = MtpAdmissionResult::compute(
            MtpMode::Embedded,
            &scenario,
            3,
            1,
            ConcurrencyPolicy::SingleActive,
        );
        assert!(result.engages_for_workload);
        assert!(result.recommended_for_workload);
        assert!(result.fallthroughs.is_empty());
    }

    #[test]
    fn decode_shape_fallthrough_reasons_are_independent() {
        let sampled = DecodeRequestShape {
            temperature: 0.7,
            constrained_tools: false,
            reasoning: false,
        };
        assert_eq!(
            sampled.fallthroughs(),
            vec![SpecDecodeFallthrough::NonGreedySampling]
        );

        let grammar = DecodeRequestShape {
            temperature: 0.0,
            constrained_tools: true,
            reasoning: false,
        };
        assert_eq!(
            grammar.fallthroughs(),
            vec![SpecDecodeFallthrough::LogitsProcessorInstalled]
        );

        // Reasoning alone does not stop the scheduler; it is recorded because it
        // changes acceptance, not eligibility.
        let reasoning = DecodeRequestShape {
            temperature: 0.0,
            constrained_tools: false,
            reasoning: true,
        };
        assert!(reasoning.permits_speculation());
    }

    #[test]
    fn mtp_config_embedded_bytes_calculated() {
        let config = MtpConfig {
            mode: MtpMode::Embedded,
            embedded_depth: 3,
            external_drafter: None,
        };
        let primary_bytes = 2_000_000_000u64; // 2GB model
        let mtp_bytes = config.primary_breakdown_mtp_bytes(primary_bytes);
        // mtp_overhead_bytes = model_size * 0.015 * depth = 2GB * 0.015 * 3 = 90MB
        let expected = (primary_bytes as f64 * 0.015 * 3.0) as u64;
        assert_eq!(mtp_bytes, expected);
        assert_eq!(config.total_mtp_bytes(primary_bytes), mtp_bytes);
    }

    #[test]
    fn mtp_config_external_no_primary_bytes() {
        let companion = ExternalCompanion {
            label: "Qwen3-0.6B drafter".into(),
            companion_type: CompanionType::Drafter,
            total_bytes: 800_000_000u64,
            weights_bytes: 600_000_000u64,
            kv_cache_bytes: 200_000_000u64,
            source: "mlx-community/Qwen3-0.6B".into(),
        };
        let config = MtpConfig {
            mode: MtpMode::External,
            embedded_depth: 0,
            external_drafter: Some(companion.clone()),
        };
        let primary_bytes = 2_000_000_000u64;
        // External MTP: primary breakdown has 0 mtp_bytes
        assert_eq!(
            config.primary_breakdown_mtp_bytes(primary_bytes),
            0,
            "External MTP should not add bytes to primary breakdown"
        );
        // Total MTP bytes comes from companion
        assert_eq!(config.total_mtp_bytes(primary_bytes), 800_000_000u64);
    }

    #[test]
    fn mtp_config_disabled_zero_bytes() {
        let config = MtpConfig {
            mode: MtpMode::Disabled,
            embedded_depth: 0,
            external_drafter: None,
        };
        assert_eq!(config.total_mtp_bytes(1_000_000_000u64), 0);
        assert_eq!(config.primary_breakdown_mtp_bytes(1_000_000_000u64), 0);
    }

    #[test]
    fn external_companion_additive_no_double_count() {
        // Verify that external companion is additive and not double-counted.
        let companion = ExternalCompanion {
            label: "Drafter".into(),
            companion_type: CompanionType::Drafter,
            total_bytes: 500_000_000u64,
            weights_bytes: 400_000_000u64,
            kv_cache_bytes: 100_000_000u64,
            source: "hf://test/drafter".into(),
        };
        let primary_total = 3_000_000_000u64;
        let total_estimate = primary_total + companion.total_bytes;
        // Total should be additive, not double-counted
        assert_eq!(total_estimate, 3_500_000_000u64);
        // Companion total includes its own weights + KV + overhead
        assert_eq!(
            companion.total_bytes,
            companion.weights_bytes + companion.kv_cache_bytes,
            "Companion total = weights + KV (no double count)"
        );
    }

    #[test]
    fn mtp_admission_json_serialization() {
        let result = MtpAdmissionResult::compute(
            MtpMode::Embedded,
            &WorkloadScenario::default(),
            3,
            1,
            ConcurrencyPolicy::SingleActive,
        );
        let json = serde_json::to_string(&result).unwrap();
        let restored: MtpAdmissionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.mode, result.mode);
        assert_eq!(restored.eligible, result.eligible);
        assert_eq!(restored.warnings, result.warnings);
    }
}
