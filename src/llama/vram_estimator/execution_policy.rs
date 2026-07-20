//! Rapid-MLX execution policy types and memory breakdown.
//!
//! Per D1/D2: backend-native semantics, no llama `ctk`/`ctv` vocabulary.
//! This module defines:
//! - `KvCacheDtype`: Rapid's --kv-cache-dtype {bf16,int8,int4}
//! - `TurboQuantMode`: Rapid's --turboquant {v4,k8v4,none}
//! - `RapidMlxExecutionPolicy`: requested vs effective policy with overrides
//! - `PolicyReason`: machine-readable explanations for effective ≠ requested
//! - `MemoryBreakdown`: component-level, additive memory accounting
//!
//! Phase 4 `ModelMemoryProfile` geometry is input to these types, not owned here.
//! No runtime math/formulas yet — documented field meanings only.
//!
//! NOTE: dead_code allowed until Parts 2-5 wire up consumption in estimator,
//! wizard, presets, and API endpoints.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// ── KV cache dtype (Rapid-native) ─────────────────────────────────────────────

/// KV cache dtype for Rapid-MLX (--kv-cache-dtype CLI flag).
///
/// Rapid-native vocabulary only — no llama.cpp `ctk`/`ctv` strings.
///
/// Evidence:
/// - `vllm_mlx/cli.py:6833`: `--kv-cache-dtype` choices {bf16, int8, int4}, default int4
/// - `vllm_mlx/cli.py:6846`: `--reasoning` pins KV cache dtype to int8
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum KvCacheDtype {
    /// bf16 — full-precision KV cache.
    /// Source: `--kv-cache-dtype bf16`.
    Bf16,
    /// int8 — 8-bit quantized KV cache.
    /// Source: `--kv-cache-dtype int8`.
    /// Also used automatically when `reasoning_mode=true` (Phase 0 E3 fact-pin).
    Int8,
    /// int4 — 4-bit quantized KV cache (Rapid default).
    /// Source: `--kv-cache-dtype int4` or omitted (default).
    #[default]
    Int4,
}

// ── TurboQuant mode (Rapid-native) ────────────────────────────────────────────

/// TurboQuant reusable-prompt storage policy (--turboquant CLI flag).
///
/// Per D31: TurboQuant applies ONLY to retained prefix snapshots.
/// It does NOT reduce model weights, active-generation KV, recurrent state, MTP state,
/// prefill/transient memory, or every cache path.
///
/// Evidence:
/// - `vllm_mlx/server.py:1920`: `--turboquant` choices {v4, k8v4, none}
/// - `vllm_mlx/turboquant.py`: packing/decompression implementation
/// - `vllm_mlx/memory_cache.py:1302-1375`: retained-prefix compression/decompression path
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TurboQuantMode {
    /// V-only compression (Expert legacy/A-B).
    /// Source: `--turboquant v4`.
    V4,
    /// K8V4 compression (Advanced trial).
    /// Source: `--turboquant k8v4`.
    K8V4,
    /// Disabled — Rapid's Standard retained-storage policy (normally int4, not uncompressed FP16).
    /// Source: `--turboquant none` or omitted (default).
    /// Serialized as "none" to match Rapid's CLI vocabulary.
    #[default]
    #[serde(rename = "none")]
    Disabled,
}

// ── Policy reasons ────────────────────────────────────────────────────────────

/// Machine-readable explanation for why effective policy differs from requested.
///
/// Each variant is a self-contained reason that can be displayed or logged.
/// No internal state; serialized as snake_case strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyReason {
    /// `reasoning_mode=true` forced KV dtype to int8 regardless of requested dtype.
    /// Evidence: `vllm_mlx/cli.py:6846`.
    ReasoningModeOverridesKvToInt8,
    /// Model is bf16-only; downgraded from requested bf16 KV to int8 for safety.
    /// This is a model-safe downgrade path.
    ModelSafeDowngradeFromBf16ToInt8,
}

impl std::fmt::Display for PolicyReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReasoningModeOverridesKvToInt8 => {
                write!(f, "reasoning_mode_overrides_kv_to_int8")
            }
            Self::ModelSafeDowngradeFromBf16ToInt8 => {
                write!(f, "model_safe_downgrade_from_bf16_to_int8")
            }
        }
    }
}

// ── Rapid-MLX execution policy ───────────────────────────────────────────────

/// Rapid-MLX execution policy for memory estimation.
///
/// Per D1/D2: backend-native semantics with requested vs effective fields.
/// This type is independent of llama.cpp execution policies; it MUST NOT contain
/// llama `ctk`/`ctv` vocabulary.
///
/// The `effective_kv_dtype` field captures what Rapid will actually use after:
/// - reasoning_mode int8 override
/// - model-safe bf16 downgrade
pub struct RapidMlxExecutionPolicy {
    /// KV cache dtype requested by the user/config.
    /// Source: `--kv-cache-dtype` CLI flag, default int4 if omitted.
    pub kv_cache_dtype: Option<KvCacheDtype>,
    /// Whether reasoning mode is enabled (--reasoning flag).
    /// When true, forces effective_kv_dtype to int8 (Phase 0 E3 fact-pin).
    /// Source: `--reasoning` CLI flag.
    pub reasoning_mode: bool,
    /// TurboQuant mode requested by the user/config.
    /// Source: `--turboquant` CLI flag, default `Disabled` (Standard) if omitted.
    pub turboquant: Option<TurboQuantMode>,
    /// The KV dtype Rapid will actually use after all overrides and safety checks.
    /// Always populated: never None. Derived from kv_cache_dtype + reasoning_mode + safety rules.
    pub effective_kv_dtype: KvCacheDtype,
}

impl RapidMlxExecutionPolicy {
    /// Construct policy with automatic effective_kv_dtype derivation.
    ///
    /// Rules:
    /// 1. If reasoning_mode=true → effective is Int8 (E3 fact-pin)
    /// 2. Otherwise use requested kv_cache_dtype, or default Int4
    pub fn new(
        kv_cache_dtype: Option<KvCacheDtype>,
        reasoning_mode: bool,
        turboquant: Option<TurboQuantMode>,
    ) -> Self {
        let effective_kv_dtype = if reasoning_mode {
            KvCacheDtype::Int8
        } else {
            kv_cache_dtype.unwrap_or_default()
        };

        Self {
            kv_cache_dtype,
            reasoning_mode,
            turboquant,
            effective_kv_dtype,
        }
    }

    /// Compute the requested KV dtype (before overrides) or None if not specified.
    pub fn requested_kv_dtype(&self) -> Option<KvCacheDtype> {
        self.kv_cache_dtype
    }

    /// Compute reasons why effective differs from requested (if any).
    pub fn effective_reasons(&self) -> Vec<PolicyReason> {
        let mut reasons = Vec::new();

        // Rule 1: reasoning_mode forces int8
        if self.reasoning_mode && self.kv_cache_dtype.is_some_and(|d| d != KvCacheDtype::Int8) {
            reasons.push(PolicyReason::ReasoningModeOverridesKvToInt8);
        }

        reasons
    }
}

impl Default for RapidMlxExecutionPolicy {
    fn default() -> Self {
        Self::new(None, false, None)
    }
}

impl Serialize for RapidMlxExecutionPolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("RapidMlxExecutionPolicy", 4)?;
        state.serialize_field("kv_cache_dtype", &self.kv_cache_dtype)?;
        state.serialize_field("reasoning_mode", &self.reasoning_mode)?;
        state.serialize_field("turboquant", &self.turboquant)?;
        state.serialize_field("effective_kv_dtype", &self.effective_kv_dtype)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for RapidMlxExecutionPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        struct Helper {
            #[serde(default)]
            kv_cache_dtype: Option<KvCacheDtype>,
            #[serde(default)]
            reasoning_mode: bool,
            #[serde(default)]
            turboquant: Option<TurboQuantMode>,
            #[serde(default)]
            effective_kv_dtype: KvCacheDtype,
        }

        let h = Helper::deserialize(deserializer)?;
        Ok(Self {
            kv_cache_dtype: h.kv_cache_dtype,
            reasoning_mode: h.reasoning_mode,
            turboquant: h.turboquant,
            effective_kv_dtype: h.effective_kv_dtype,
        })
    }
}

// ── Memory breakdown (additive components) ───────────────────────────────────

/// Component-level memory accounting for a Rapid-MLX configuration.
///
/// Per D1: backend-native semantics. All components are additive; none double-counted.
/// `ModelMemoryProfile` geometry is input, not owned here.
///
/// This is a types-only structure: no runtime math/formulas yet.
#[derive(Default)]
pub struct MemoryBreakdown {
    /// Model weights resident in memory (quantized as loaded).
    pub weights_bytes: u64,
    /// Active KV cache for current generation(s).
    /// This is the "hot" portion for tokens being generated/prefilled.
    pub active_kv_bytes: u64,
    /// Retained prefix KV cache (reusable prompt storage).
    /// Subject to TurboQuant compression when enabled.
    pub retained_kv_bytes: u64,
    /// Multi-Token Prediction (MTP) state and draft head weights.
    /// Additive even while inactive (D25).
    pub mtp_bytes: u64,
    /// Recurrent/Mamba state for linear-attention layers.
    pub recurrent_state_bytes: u64,
    /// TurboQuant transient decompression peak (when TurboQuant is active).
    /// Visible as a short-lived peak during retained-prefix decode.
    pub turboquant_transient_peak_bytes: u64,
    /// Runtime overhead (Metal command buffers, MLX internals, etc.).
    pub runtime_overhead_bytes: u64,
    /// Reserved headroom for safety margin.
    pub headroom_bytes: u64,
    /// The policy that was requested.
    pub requested_policy: RapidMlxExecutionPolicy,
    /// The policy that will be used (after overrides/safety checks).
    pub effective_policy: RapidMlxExecutionPolicy,
    /// Machine-readable reasons for effective ≠ requested.
    pub reasons: Vec<PolicyReason>,
}

impl MemoryBreakdown {
    /// Sum of all additive memory components (excluding reasons/policy metadata).
    pub fn total_bytes(&self) -> u64 {
        self.weights_bytes
            + self.active_kv_bytes
            + self.retained_kv_bytes
            + self.mtp_bytes
            + self.recurrent_state_bytes
            + self.turboquant_transient_peak_bytes
            + self.runtime_overhead_bytes
            + self.headroom_bytes
    }
}

impl Serialize for MemoryBreakdown {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("MemoryBreakdown", 11)?;
        state.serialize_field("weights_bytes", &self.weights_bytes)?;
        state.serialize_field("active_kv_bytes", &self.active_kv_bytes)?;
        state.serialize_field("retained_kv_bytes", &self.retained_kv_bytes)?;
        state.serialize_field("mtp_bytes", &self.mtp_bytes)?;
        state.serialize_field("recurrent_state_bytes", &self.recurrent_state_bytes)?;
        state.serialize_field(
            "turboquant_transient_peak_bytes",
            &self.turboquant_transient_peak_bytes,
        )?;
        state.serialize_field("runtime_overhead_bytes", &self.runtime_overhead_bytes)?;
        state.serialize_field("headroom_bytes", &self.headroom_bytes)?;
        state.serialize_field("requested_policy", &self.requested_policy)?;
        state.serialize_field("effective_policy", &self.effective_policy)?;
        state.serialize_field("reasons", &self.reasons)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for MemoryBreakdown {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        struct Helper {
            #[serde(default)]
            weights_bytes: u64,
            #[serde(default)]
            active_kv_bytes: u64,
            #[serde(default)]
            retained_kv_bytes: u64,
            #[serde(default)]
            mtp_bytes: u64,
            #[serde(default)]
            recurrent_state_bytes: u64,
            #[serde(default)]
            turboquant_transient_peak_bytes: u64,
            #[serde(default)]
            runtime_overhead_bytes: u64,
            #[serde(default)]
            headroom_bytes: u64,
            #[serde(default)]
            requested_policy: RapidMlxExecutionPolicy,
            #[serde(default)]
            effective_policy: RapidMlxExecutionPolicy,
            #[serde(default)]
            reasons: Vec<PolicyReason>,
        }

        let h = Helper::deserialize(deserializer)?;
        Ok(Self {
            weights_bytes: h.weights_bytes,
            active_kv_bytes: h.active_kv_bytes,
            retained_kv_bytes: h.retained_kv_bytes,
            mtp_bytes: h.mtp_bytes,
            recurrent_state_bytes: h.recurrent_state_bytes,
            turboquant_transient_peak_bytes: h.turboquant_transient_peak_bytes,
            runtime_overhead_bytes: h.runtime_overhead_bytes,
            headroom_bytes: h.headroom_bytes,
            requested_policy: h.requested_policy,
            effective_policy: h.effective_policy,
            reasons: h.reasons,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    // ── KvCacheDtype tests ────────────────────────────────────────────────────

    #[test]
    fn kv_cache_dtype_bf16_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&KvCacheDtype::Bf16).unwrap(),
            "\"bf16\""
        );
    }

    #[test]
    fn kv_cache_dtype_int8_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&KvCacheDtype::Int8).unwrap(),
            "\"int8\""
        );
    }

    #[test]
    fn kv_cache_dtype_int4_is_default() {
        let default_value: KvCacheDtype = Default::default();
        assert_eq!(default_value, KvCacheDtype::Int4);
    }

    #[test]
    fn kv_cache_dtype_roundtrip() {
        for (input, expected) in [
            ("\"bf16\"", KvCacheDtype::Bf16),
            ("\"int8\"", KvCacheDtype::Int8),
            ("\"int4\"", KvCacheDtype::Int4),
        ] {
            let parsed: KvCacheDtype = serde_json::from_str(input).unwrap();
            assert_eq!(parsed, expected);
            let serialized = serde_json::to_string(&parsed).unwrap();
            assert_eq!(serialized, input);
        }
    }

    // ── TurboQuantMode tests ─────────────────────────────────────────────────

    #[test]
    fn turboquant_mode_variants_roundtrip() {
        for (input, expected) in [
            ("\"v4\"", TurboQuantMode::V4),
            ("\"k8v4\"", TurboQuantMode::K8V4),
            ("\"none\"", TurboQuantMode::Disabled),
        ] {
            let parsed: TurboQuantMode = serde_json::from_str(input).unwrap();
            assert_eq!(parsed, expected);
            let serialized = serde_json::to_string(&parsed).unwrap();
            assert_eq!(serialized, input);
        }
    }

    #[test]
    fn turboquant_disabled_is_default() {
        let default_value: TurboQuantMode = Default::default();
        assert_eq!(default_value, TurboQuantMode::Disabled);
    }

    // ── RapidMlxExecutionPolicy tests ────────────────────────────────────────

    #[test]
    fn policy_serializes_correctly() {
        let policy = RapidMlxExecutionPolicy::new(Some(KvCacheDtype::Int8), false, None);
        let json = serde_json::to_string(&policy).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["kv_cache_dtype"], "int8");
        assert_eq!(value["reasoning_mode"], false);
        assert!(value["turboquant"].is_null());
        assert_eq!(value["effective_kv_dtype"], "int8");
    }

    #[test]
    fn policy_roundtrip() {
        let policy = RapidMlxExecutionPolicy::new(
            Some(KvCacheDtype::Bf16),
            false,
            Some(TurboQuantMode::K8V4),
        );
        let json = serde_json::to_string(&policy).unwrap();
        let restored: RapidMlxExecutionPolicy = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.kv_cache_dtype, policy.kv_cache_dtype);
        assert_eq!(restored.reasoning_mode, policy.reasoning_mode);
        assert_eq!(restored.turboquant, policy.turboquant);
        assert_eq!(restored.effective_kv_dtype, policy.effective_kv_dtype);
    }

    #[test]
    fn reasoning_mode_true_forces_effective_int8() {
        // Explicitly requested bf16, but reasoning_mode forces int8
        let policy = RapidMlxExecutionPolicy::new(Some(KvCacheDtype::Bf16), true, None);
        assert_eq!(policy.effective_kv_dtype, KvCacheDtype::Int8);
        assert_eq!(policy.kv_cache_dtype, Some(KvCacheDtype::Bf16)); // requested preserved
    }

    #[test]
    fn reasoning_mode_true_with_explicit_int8_no_conflict() {
        let policy = RapidMlxExecutionPolicy::new(Some(KvCacheDtype::Int8), true, None);
        assert_eq!(policy.effective_kv_dtype, KvCacheDtype::Int8);
    }

    #[test]
    fn reasoning_mode_true_with_explicit_int4_overridden() {
        let policy = RapidMlxExecutionPolicy::new(Some(KvCacheDtype::Int4), true, None);
        assert_eq!(policy.effective_kv_dtype, KvCacheDtype::Int8);
    }

    #[test]
    fn effective_reasons_includes_reasoning_override_when_applicable() {
        let policy = RapidMlxExecutionPolicy::new(Some(KvCacheDtype::Bf16), true, None);
        let reasons = policy.effective_reasons();
        assert_eq!(reasons.len(), 1);
        assert_eq!(reasons[0], PolicyReason::ReasoningModeOverridesKvToInt8);
    }

    #[test]
    fn effective_reasons_empty_when_no_override_needed() {
        let policy = RapidMlxExecutionPolicy::new(Some(KvCacheDtype::Int4), false, None);
        let reasons = policy.effective_reasons();
        assert!(reasons.is_empty());
    }

    #[test]
    fn policy_defaults_are_sane() {
        let policy = RapidMlxExecutionPolicy::default();
        assert_eq!(policy.kv_cache_dtype, None); // not specified
        assert!(!policy.reasoning_mode);
        assert_eq!(policy.turboquant, None);
        assert_eq!(policy.effective_kv_dtype, KvCacheDtype::Int4); // Rapid default
    }

    // ── MemoryBreakdown tests ────────────────────────────────────────────────

    #[test]
    fn memory_breakdown_additive_total() {
        let breakdown = MemoryBreakdown {
            weights_bytes: 1_000_000_000,
            active_kv_bytes: 500_000_000,
            retained_kv_bytes: 200_000_000,
            mtp_bytes: 100_000_000,
            recurrent_state_bytes: 50_000_000,
            turboquant_transient_peak_bytes: 75_000_000,
            runtime_overhead_bytes: 25_000_000,
            headroom_bytes: 50_000_000,
            requested_policy: Default::default(),
            effective_policy: Default::default(),
            reasons: Vec::new(),
        };

        // Sum: 1000M + 500M + 200M + 100M + 50M + 75M + 25M + 50M = 2,000,000,000
        assert_eq!(breakdown.total_bytes(), 2_000_000_000);
    }

    #[test]
    fn memory_breakdown_roundtrip() {
        let breakdown = MemoryBreakdown {
            weights_bytes: 1_000_000,
            active_kv_bytes: 100_000,
            retained_kv_bytes: 50_000,
            mtp_bytes: 0,
            recurrent_state_bytes: 0,
            turboquant_transient_peak_bytes: 0,
            runtime_overhead_bytes: 10_000,
            headroom_bytes: 5_000,
            requested_policy: RapidMlxExecutionPolicy::new(
                Some(KvCacheDtype::Int4),
                false,
                Some(TurboQuantMode::K8V4),
            ),
            effective_policy: RapidMlxExecutionPolicy::new(
                Some(KvCacheDtype::Int4),
                false,
                Some(TurboQuantMode::K8V4),
            ),
            reasons: vec![],
        };

        let json = serde_json::to_string(&breakdown).unwrap();
        let restored: MemoryBreakdown = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.total_bytes(), breakdown.total_bytes());
        assert_eq!(restored.weights_bytes, breakdown.weights_bytes);
        assert_eq!(
            restored.requested_policy.turboquant,
            breakdown.requested_policy.turboquant
        );
    }

    #[test]
    fn memory_breakdown_reasons_populated_for_overridden_policy() {
        let requested_policy = RapidMlxExecutionPolicy::new(Some(KvCacheDtype::Bf16), true, None);
        let effective_policy = RapidMlxExecutionPolicy::new(Some(KvCacheDtype::Bf16), true, None);

        let breakdown = MemoryBreakdown {
            weights_bytes: 1_000_000,
            active_kv_bytes: 0,
            retained_kv_bytes: 0,
            mtp_bytes: 0,
            recurrent_state_bytes: 0,
            turboquant_transient_peak_bytes: 0,
            runtime_overhead_bytes: 0,
            headroom_bytes: 0,
            requested_policy,
            effective_policy,
            reasons: vec![PolicyReason::ReasoningModeOverridesKvToInt8],
        };

        let json = serde_json::to_string(&breakdown).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(
            value["reasons"].as_array().unwrap().len(),
            1,
            "reasons array should contain exactly one reason"
        );
        assert_eq!(
            value["reasons"][0].as_str().unwrap(),
            "reasoning_mode_overrides_kv_to_int8"
        );
    }

    // ── D1 hard gate: no llama vocabulary ────────────────────────────────────

    #[test]
    fn no_llama_kv_vocabulary_used() {
        // Prove that our types use Rapid-native naming, not llama ctk/ctv strings.
        // The serde JSON output should ONLY be one of our Rapid-native values.
        // Llama.cpp KV vocabulary includes: f16, q8_0, q4_0, q5_0, q4_k_m, q6_k, q2_k, etc.

        let all_dtypes = [KvCacheDtype::Bf16, KvCacheDtype::Int8, KvCacheDtype::Int4];
        let rapid_native_values = [
            "\"bf16\"", // Rapid: --kv-cache-dtype bf16
            "\"int8\"", // Rapid: --kv-cache-dtype int8
            "\"int4\"", // Rapid: --kv-cache-dtype int4 (default)
        ];

        for dtype in all_dtypes {
            let json = serde_json::to_string(&dtype).unwrap();
            assert!(
                rapid_native_values.contains(&json.as_str()),
                "Rapid-native KvCacheDtype must serialize to one of {{bf16,int8,int4}}, got '{}'",
                json
            );
        }
    }

    #[test]
    fn policy_json_contains_no_llama_ctk_ctv() {
        let policy = RapidMlxExecutionPolicy::new(Some(KvCacheDtype::Int8), false, None);
        let json = serde_json::to_string(&policy).unwrap();

        // The llama.cpp KV vocabulary leaks via ctk/ctv field names
        assert!(
            !json.contains("ctk"),
            "RapidMlxExecutionPolicy JSON must not contain llama 'ctk' vocabulary"
        );
        assert!(
            !json.contains("ctv"),
            "RapidMlxExecutionPolicy JSON must not contain llama 'ctv' vocabulary"
        );
    }
}
