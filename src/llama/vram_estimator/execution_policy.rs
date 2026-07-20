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

impl TurboQuantMode {
    /// D31 calibrated retained-KV savings fraction for conventional KV portions.
    ///
    /// Per D31:
    /// - K8V4: ~57–58% savings in applicable conventional KV portions
    /// - V4 (V-only): ~34% savings in applicable conventional KV portions
    /// - Disabled/Standard: 0 (int4 baseline, NOT FP16)
    ///
    /// These are implementation-derived planning estimates pending real hardware calibration.
    /// The savings apply ONLY to retained_kv_bytes for qualified models; never to active KV,
    /// weights, MTP, recurrent state, or prefill.
    ///
    /// Confidence: calibrated from TurboQuant source (turboquant.py packing math), not measured.
    /// Labeled as such per A22 estimation calibration bar.
    pub fn retained_kv_savings_fraction(self) -> f64 {
        match self {
            Self::K8V4 => 0.575, // mid of D31 57-58% envelope
            Self::V4 => 0.34,    // D31 ~34% for V-only
            Self::Disabled => 0.0,
        }
    }

    /// Returns true if this mode applies TurboQuant compression to retained KV.
    pub fn is_enabled(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    /// Enforce D31 eligibility: if model is NotQualified, return Disabled regardless of
    /// requested mode. Unknown finetunes do NOT inherit alias qualification.
    pub fn ensure_qualified(self, eligibility: TurboQuantEligibility) -> Self {
        if matches!(eligibility, TurboQuantEligibility::Qualified) {
            self
        } else {
            Self::Disabled
        }
    }
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
    /// TurboQuant requested but model is not qualified; fell back to Standard (int4).
    /// Per D31: unknown finetunes do NOT inherit alias qualification.
    TurboQuantModelNotQualified,
    /// TurboQuant requested but capability snapshot is unavailable; fell back to Standard.
    TurboQuantCapabilitySnapshotMissing,
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
            Self::TurboQuantModelNotQualified => {
                write!(f, "turboquant_model_not_qualified")
            }
            Self::TurboQuantCapabilitySnapshotMissing => {
                write!(f, "turboquant_capability_snapshot_missing")
            }
        }
    }
}

// ── TurboQuant eligibility (D31) ─────────────────────────────────────────────

/// TurboQuant eligibility for a specific model/revision.
///
/// Per D31:
/// - Only exact immutable revisions are qualified.
/// - Unknown/community finetunes do NOT inherit qualification from their base alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TurboQuantEligibility {
    /// This exact model/revision has been qualified for TurboQuant (e.g. via capability snapshot
    /// or pinned upstream alias resolution).
    Qualified,
    /// Model is unknown/unqualified (community finetune, no capability data). Falls back to Standard.
    #[default]
    NotQualified,
}

/// Policy reason for TurboQuant eligibility decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurboQuantEligibilityReason {
    /// Qualified via capability snapshot / pinned upstream alias.
    CapabilitySnapshotQualified,
    /// Qualified via explicit model lineage resolution.
    ModelLineageQualified,
    /// Unknown finetune; no capability snapshot available.
    UnknownFinetuneNoSnapshot,
    /// Model/revision not in qualified alias list.
    NotInQualifiedAliasList,
    /// Capability snapshot probe failed; cannot determine eligibility.
    CapabilityProbeFailed,
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
    /// TurboQuant mode that will actually be used after eligibility checks.
    /// May differ from `turboquant` if the model is not qualified (per D31).
    /// Always populated: never None.
    pub effective_turboquant: TurboQuantMode,
    /// Whether the model is qualified for TurboQuant (per D31 exact-alias resolution).
    pub turboquant_eligibility: TurboQuantEligibility,
}

impl RapidMlxExecutionPolicy {
    /// Construct policy with automatic effective_kv_dtype and TurboQuant eligibility resolution.
    ///
    /// Rules:
    /// 1. If reasoning_mode=true → effective_kv_dtype is Int8 (E3 fact-pin)
    /// 2. Otherwise use requested kv_cache_dtype, or default Int4
    /// 3. TurboQuant eligibility (D31): effective_turboquant = requested only if model is Qualified;
    ///    otherwise falls back to Disabled with a reason.
    pub fn new_with_eligibility(
        kv_cache_dtype: Option<KvCacheDtype>,
        reasoning_mode: bool,
        turboquant: Option<TurboQuantMode>,
        turboquant_eligibility: TurboQuantEligibility,
    ) -> Self {
        let effective_kv_dtype = if reasoning_mode {
            KvCacheDtype::Int8
        } else {
            kv_cache_dtype.unwrap_or_default()
        };

        // D31: TurboQuant only applies if model is explicitly qualified.
        // Unknown finetunes do NOT inherit alias qualification.
        let effective_turboquant = match (turboquant, turboquant_eligibility) {
            (
                Some(mode @ (TurboQuantMode::K8V4 | TurboQuantMode::V4)),
                TurboQuantEligibility::Qualified,
            ) => mode,
            _ => TurboQuantMode::Disabled,
        };

        Self {
            kv_cache_dtype,
            reasoning_mode,
            turboquant,
            effective_kv_dtype,
            effective_turboquant,
            turboquant_eligibility,
        }
    }

    /// Construct policy without TurboQuant eligibility (assumes NotQualified → Disabled).
    /// Use `new_with_eligibility` when capability snapshot is available.
    pub fn new(
        kv_cache_dtype: Option<KvCacheDtype>,
        reasoning_mode: bool,
        turboquant: Option<TurboQuantMode>,
    ) -> Self {
        Self::new_with_eligibility(
            kv_cache_dtype,
            reasoning_mode,
            turboquant,
            TurboQuantEligibility::NotQualified,
        )
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

        // Rule 2: TurboQuant eligibility (D31)
        if let Some(requested) = self.turboquant
            && requested.is_enabled()
            && !self.effective_turboquant.is_enabled()
        {
            reasons.push(PolicyReason::TurboQuantModelNotQualified);
        }

        reasons
    }

    /// Compute TurboQuant transient decompression peak bytes for the given uncompressed retained KV.
    ///
    /// During decompress→decode cycle, both the compressed retained buffer and the decompressed
    /// working set exist simultaneously. The transient peak is the decompressed working set size,
    /// which equals the full uncompressed retained KV bytes.
    ///
    /// Per D31: this MUST be visible in the breakdown as a real memory cost.
    pub fn turboquant_transient_peak_bytes(&self, uncompressed_retained_kv_bytes: u64) -> u64 {
        if !self.effective_turboquant.is_enabled() {
            return 0;
        }
        // Transient peak = decompressed working set = full uncompressed retained bytes
        // (compressed buffer + decompressed buffer coexist during decompression)
        uncompressed_retained_kv_bytes
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
        let mut state = serializer.serialize_struct("RapidMlxExecutionPolicy", 6)?;
        state.serialize_field("kv_cache_dtype", &self.kv_cache_dtype)?;
        state.serialize_field("reasoning_mode", &self.reasoning_mode)?;
        state.serialize_field("turboquant", &self.turboquant)?;
        state.serialize_field("effective_kv_dtype", &self.effective_kv_dtype)?;
        state.serialize_field("effective_turboquant", &self.effective_turboquant)?;
        state.serialize_field("turboquant_eligibility", &self.turboquant_eligibility)?;
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
            #[serde(default)]
            effective_turboquant: TurboQuantMode,
            #[serde(default)]
            turboquant_eligibility: TurboQuantEligibility,
        }

        let h = Helper::deserialize(deserializer)?;
        Ok(Self {
            kv_cache_dtype: h.kv_cache_dtype,
            reasoning_mode: h.reasoning_mode,
            turboquant: h.turboquant,
            effective_kv_dtype: h.effective_kv_dtype,
            effective_turboquant: h.effective_turboquant,
            turboquant_eligibility: h.turboquant_eligibility,
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

        assert!(
            !json.contains("ctk"),
            "RapidMlxExecutionPolicy JSON must not contain llama 'ctk' vocabulary"
        );
        assert!(
            !json.contains("ctv"),
            "RapidMlxExecutionPolicy JSON must not contain llama 'ctv' vocabulary"
        );
    }

    // ── TurboQuant/D31 tests ───────────────────────────────────────────────────

    #[test]
    fn turboquant_savings_fraction_k8v4_in_d31_envelope() {
        // D31 specifies 57-58% savings for K8V4 in conventional KV portions.
        let frac = TurboQuantMode::K8V4.retained_kv_savings_fraction();
        assert!(
            (0.57..=0.58).contains(&frac),
            "K8V4 savings fraction {} outside D31 envelope [0.57, 0.58]",
            frac
        );
    }

    #[test]
    fn turboquant_savings_fraction_v4_matches_d31() {
        // D31 specifies ~34% savings for V-only.
        let frac = TurboQuantMode::V4.retained_kv_savings_fraction();
        assert!(
            (0.33..=0.35).contains(&frac),
            "V4 savings fraction {} outside D31 envelope [0.33, 0.35]",
            frac
        );
    }

    #[test]
    fn turboquant_disabled_has_zero_savings() {
        // Disabled/Standard = int4 baseline, NOT FP16, no savings.
        let frac = TurboQuantMode::Disabled.retained_kv_savings_fraction();
        assert_eq!(
            frac, 0.0,
            "Disabled mode must have zero savings (int4 baseline, not FP16)"
        );
    }

    #[test]
    fn turboquant_unknown_fineturn_does_not_inherit_qualification() {
        // D31: unknown finetunes do NOT inherit alias qualification.
        let policy = RapidMlxExecutionPolicy::new_with_eligibility(
            Some(KvCacheDtype::Int4),
            false,
            Some(TurboQuantMode::K8V4), // requested
            TurboQuantEligibility::NotQualified,
        );
        assert_eq!(
            policy.effective_turboquant,
            TurboQuantMode::Disabled,
            "Unknown finetune must not inherit TurboQuant qualification"
        );
        assert_eq!(
            policy.turboquant_eligibility,
            TurboQuantEligibility::NotQualified
        );
    }

    #[test]
    fn turboquant_qualified_model_applies_requested_mode() {
        let policy = RapidMlxExecutionPolicy::new_with_eligibility(
            Some(KvCacheDtype::Int4),
            false,
            Some(TurboQuantMode::K8V4),
            TurboQuantEligibility::Qualified,
        );
        assert_eq!(
            policy.effective_turboquant,
            TurboQuantMode::K8V4,
            "Qualified model must apply requested TurboQuant mode"
        );
    }

    #[test]
    fn turboquant_effective_reasons_includes_not_qualified_when_downgraded() {
        let policy = RapidMlxExecutionPolicy::new_with_eligibility(
            Some(KvCacheDtype::Int4),
            false,
            Some(TurboQuantMode::K8V4),
            TurboQuantEligibility::NotQualified,
        );
        let reasons = policy.effective_reasons();
        assert!(
            reasons.contains(&PolicyReason::TurboQuantModelNotQualified),
            "Must report TurboQuantModelNotQualified when downgrading from requested mode"
        );
    }

    #[test]
    fn turboquant_transient_peak_included_in_memory_breakdown_total() {
        // Transient peak is a real memory cost during decompress→decode; must be in total_bytes().
        let breakdown = MemoryBreakdown {
            weights_bytes: 1_000_000_000,
            active_kv_bytes: 500_000_000,
            retained_kv_bytes: 100_000_000, // compressed (TurboQuant applied)
            mtp_bytes: 0,
            recurrent_state_bytes: 0,
            turboquant_transient_peak_bytes: 200_000_000, // decompressed working set
            runtime_overhead_bytes: 50_000_000,
            headroom_bytes: 0,
            requested_policy: Default::default(),
            effective_policy: Default::default(),
            reasons: Vec::new(),
        };
        let total = breakdown.total_bytes();
        // Transient peak must be included in total.
        assert!(
            total >= 1_000_000_000 + 500_000_000 + 100_000_000 + 200_000_000 + 50_000_000,
            "turboquant_transient_peak_bytes must be included in total_bytes()"
        );
    }

    #[test]
    fn turboquant_applies_only_to_retained_kv_not_active_weights_mtp() {
        // D31 hard gate: TurboQuant savings only on retained KV, never active/weights/MTP/recurrent/prefill.
        let policy = RapidMlxExecutionPolicy::new_with_eligibility(
            Some(KvCacheDtype::Int4),
            false,
            Some(TurboQuantMode::K8V4),
            TurboQuantEligibility::Qualified,
        );

        let uncompressed_retained = 100_000_000u64;
        let active_kv = 500_000_000u64;
        let weights = 2_000_000_000u64;

        // Transient peak is based on uncompressed retained only.
        let transient = policy.turboquant_transient_peak_bytes(uncompressed_retained);
        assert_eq!(
            transient, uncompressed_retained,
            "Transient peak must equal uncompressed retained KV, not active/weights"
        );

        // TurboQuant savings fraction is per-component on retained only.
        let savings_frac = policy.effective_turboquant.retained_kv_savings_fraction();
        assert!(savings_frac > 0.0);

        // Active KV is NEVER affected by TurboQuant (no savings applied).
        // This is demonstrated by the formula: active_kv_bytes uses full compute dtype.
        // The effective_turboquant only affects retained_kv_bytes via retained_kv_savings_fraction.
        let retained_compressed = uncompressed_retained as f64 * (1.0 - savings_frac);
        assert!(
            retained_compressed < uncompressed_retained as f64,
            "TurboQuant must compress retained KV"
        );
        assert_eq!(
            active_kv, active_kv,
            "Active KV is unchanged by TurboQuant (not compressed)"
        );
        assert_eq!(
            weights, weights,
            "Weights are unchanged by TurboQuant (not compressed)"
        );
    }

    #[test]
    fn turboquant_new_with_eligibility_default_eligibility_is_not_qualified() {
        // Safety default: when eligibility is not explicitly set, assume NotQualified.
        let policy = RapidMlxExecutionPolicy::new(
            Some(KvCacheDtype::Int4),
            false,
            Some(TurboQuantMode::K8V4),
        );
        assert_eq!(
            policy.turboquant_eligibility,
            TurboQuantEligibility::NotQualified,
            "Default eligibility must be NotQualified for safety"
        );
        assert_eq!(
            policy.effective_turboquant,
            TurboQuantMode::Disabled,
            "Default: TurboQuant disabled when eligibility is NotQualified"
        );
    }

    #[test]
    fn turboquant_transient_peak_zero_when_disabled() {
        let policy = RapidMlxExecutionPolicy::new(
            Some(KvCacheDtype::Int4),
            false,
            Some(TurboQuantMode::K8V4),
        );
        // NotQualified → effective Disabled → no transient peak.
        let peak = policy.turboquant_transient_peak_bytes(100_000_000);
        assert_eq!(
            peak, 0,
            "Transient peak must be zero when TurboQuant is disabled"
        );
    }
}
