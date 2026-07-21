//! Semantic setting catalog for Rapid-MLX configuration.
//!
//! Per D6: backend-owned, schema/descriptor-driven, covering capability, evidence,
//! default, help, validation, serialization, command mapping, and unsupported reason.
//!
//! This is NOT a form generator. It is the authoritative Rust definition that
//! the UI consumes via API. Every exposed setting must trace:
//! capability/evidence -> typed schema -> validation -> launch mapping -> save/restore.
//!
//! Wires in types from execution_policy.rs and workload_scenarios.rs.

use crate::inference::rapid_mlx::capabilities::{CapabilitySnapshot, FeatureQualification};
use crate::llama::vram_estimator::WorkloadScenario;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::marker::PhantomData;

/// Validation context for settings that depend on runtime state.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ValidationContext<'a> {
    pub capabilities: Option<&'a CapabilitySnapshot>,
    pub workload_scenario: Option<WorkloadScenario>,
    pub _marker: PhantomData<&'a ()>,
}

impl<'a> Default for ValidationContext<'a> {
    fn default() -> Self {
        Self {
            capabilities: None,
            workload_scenario: None,
            _marker: PhantomData,
        }
    }
}

/// Validation error for a specific setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub setting_id: &'static str,
    pub message: String,
    pub code: &'static str,
}

/// Effective policy explanation for a setting.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectivePolicyExplanation {
    pub requested: serde_json::Value,
    pub effective: serde_json::Value,
    pub reason: Option<String>,
    pub reason_code: Option<&'static str>,
}

/// A single setting in the Rapid-MLX semantic catalog.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum RapidMlxSetting {
    KvCacheDtype,
    TurboquantMode,
    PrefixCachePolicy,
    HybridCacheEntries,
    PflashPolicy,
    ResponseCachePolicy,
    DiskCheckpointPolicy,
    MaxNumSeqs,
    MaxConcurrentRequests,
    PrefillBatchSize,
    CompletionBatchSize,
    BatchingPolicy,
    ConcurrencyPolicy,
    ReasoningMode,
    SpeculativePolicy,
    MllmVision,
    Embeddings,
    GpuMemoryUtilization,
    WebUiAvailability,
    WebUiStaticPath,
    WebUiConfigJson,
    EndpointCompatibility,
    RequestSafetyPolicy,
    SamplingMode,
    ParserPolicy,
    SecurityPolicy,
}

#[allow(dead_code)]
impl RapidMlxSetting {
    /// Stable ID used for serialization and API.
    pub fn id(&self) -> &'static str {
        match self {
            Self::KvCacheDtype => "kv_cache_dtype",
            Self::TurboquantMode => "turboquant_mode",
            Self::PrefixCachePolicy => "prefix_cache_policy",
            Self::HybridCacheEntries => "hybrid_cache_entries",
            Self::PflashPolicy => "pflash_policy",
            Self::ResponseCachePolicy => "response_cache_policy",
            Self::DiskCheckpointPolicy => "disk_checkpoint_policy",
            Self::MaxNumSeqs => "max_num_seqs",
            Self::MaxConcurrentRequests => "max_concurrent_requests",
            Self::PrefillBatchSize => "prefill_batch_size",
            Self::CompletionBatchSize => "completion_batch_size",
            Self::BatchingPolicy => "batching_policy",
            Self::ConcurrencyPolicy => "concurrency_policy",
            Self::ReasoningMode => "reasoning_mode",
            Self::SpeculativePolicy => "speculative_policy",
            Self::MllmVision => "mllm_vision",
            Self::Embeddings => "embeddings",
            Self::GpuMemoryUtilization => "gpu_memory_utilization",
            Self::WebUiAvailability => "web_ui_availability",
            Self::WebUiStaticPath => "web_ui_static_path",
            Self::WebUiConfigJson => "web_ui_config_json",
            Self::EndpointCompatibility => "endpoint_compatibility",
            Self::RequestSafetyPolicy => "request_safety_policy",
            Self::SamplingMode => "sampling_mode",
            Self::ParserPolicy => "parser_policy",
            Self::SecurityPolicy => "security_policy",
        }
    }

    /// Whether this setting is supported by the given capability snapshot.
    pub fn capability(&self, snapshot: &CapabilitySnapshot) -> bool {
        let has_flag = |flag: &str| snapshot.serve_flags.iter().any(|f| f == flag);

        match self {
            Self::KvCacheDtype => has_flag("--kv-cache-dtype"),
            Self::TurboquantMode => has_flag("--turboquant"),
            Self::PrefixCachePolicy => has_flag("--max-cache-blocks"),
            Self::HybridCacheEntries => has_flag("--hybrid-cache-entries"),
            Self::PflashPolicy => has_flag("--pflash"),
            Self::ResponseCachePolicy => has_flag("--response-cache"),
            Self::DiskCheckpointPolicy => has_flag("--disk-checkpoint"),
            Self::MaxNumSeqs => has_flag("--max-num-seqs"),
            Self::MaxConcurrentRequests => has_flag("--max-concurrent-requests"),
            Self::PrefillBatchSize => has_flag("--prefill-batch-size"),
            Self::CompletionBatchSize => has_flag("--completion-batch-size"),
            Self::BatchingPolicy => has_flag("--batching-policy"),
            Self::ConcurrencyPolicy => has_flag("--concurrency-policy"),
            Self::ReasoningMode => has_flag("--reasoning"),
            Self::SpeculativePolicy => has_flag("--speculative"),
            Self::MllmVision => {
                matches!(
                    snapshot.qualified_features.vision,
                    FeatureQualification::Available
                )
            }
            Self::Embeddings => {
                matches!(
                    snapshot.qualified_features.embeddings,
                    FeatureQualification::Available
                )
            }
            Self::GpuMemoryUtilization => has_flag("--gpu-memory-utilization"),
            Self::WebUiAvailability => has_flag("--ui") || has_flag("--no-ui"),
            Self::WebUiStaticPath => has_flag("--path"),
            Self::WebUiConfigJson => has_flag("--ui-config") || has_flag("--ui-config-file"),
            Self::EndpointCompatibility => true, // Inferred from snapshot features
            Self::RequestSafetyPolicy => true,   // Default policy always available
            Self::SamplingMode => true, // Always available via --default-* flags or request-level
            Self::ParserPolicy => has_flag("--tool-call-parser"),
            Self::SecurityPolicy => true, // Default policy always available
        }
    }

    /// Default value for this setting (JSON-serializable).
    pub fn default_value(&self) -> serde_json::Value {
        match self {
            Self::KvCacheDtype => serde_json::json!({"effective": "int4"}),
            Self::TurboquantMode => serde_json::json!("none"),
            Self::PrefixCachePolicy => serde_json::json!("auto"),
            Self::HybridCacheEntries => serde_json::json!(0),
            Self::PflashPolicy => serde_json::json!("auto"),
            Self::ResponseCachePolicy => serde_json::json!("auto"),
            Self::DiskCheckpointPolicy => serde_json::json!("auto"),
            Self::MaxNumSeqs => serde_json::json!(4),
            Self::MaxConcurrentRequests => serde_json::json!(16),
            Self::PrefillBatchSize => serde_json::json!(null),
            Self::CompletionBatchSize => serde_json::json!(null),
            Self::BatchingPolicy => serde_json::json!("auto"),
            Self::ConcurrencyPolicy => serde_json::json!("single_active"),
            Self::ReasoningMode => serde_json::json!("auto"),
            Self::SpeculativePolicy => serde_json::json!("auto"),
            Self::MllmVision => serde_json::json!("auto"),
            Self::Embeddings => serde_json::json!("auto"),
            Self::GpuMemoryUtilization => serde_json::json!(0.9),
            Self::WebUiAvailability => serde_json::json!("auto"),
            Self::WebUiStaticPath => serde_json::json!(null),
            Self::WebUiConfigJson => serde_json::json!(null),
            Self::EndpointCompatibility => serde_json::json!("openai_v1"),
            Self::RequestSafetyPolicy => serde_json::json!("auto"),
            Self::SamplingMode => serde_json::json!("auto"),
            Self::ParserPolicy => serde_json::json!("auto"),
            Self::SecurityPolicy => serde_json::json!("loopback_only"),
        }
    }

    /// Validate a value for this setting.
    pub fn validate<'a>(
        &self,
        value: &serde_json::Value,
        _context: &ValidationContext<'a>,
    ) -> Result<(), ValidationError> {
        let id = self.id();
        let invalid = |msg: String, code: &'static str| {
            Err(ValidationError {
                setting_id: id,
                message: msg,
                code,
            })
        };

        match self {
            Self::KvCacheDtype => {
                let effective = value
                    .get("effective")
                    .and_then(|v| v.as_str())
                    .unwrap_or("int4");
                match effective {
                    "int4" | "int8" | "bf16" => Ok(()),
                    _ => invalid(
                        format!(
                            "kv_cache_dtype.effective must be one of [int4, int8, bf16], got '{effective}'"
                        ),
                        "invalid_kv_dtype",
                    ),
                }
            }
            Self::TurboquantMode => {
                let mode = value.as_str().unwrap_or("none");
                match mode {
                    "none" | "k8v4" | "v4" => Ok(()),
                    _ => invalid(
                        format!("turboquant_mode must be one of [none, k8v4, v4], got '{mode}'"),
                        "invalid_turboquant_mode",
                    ),
                }
            }
            Self::HybridCacheEntries => {
                if let Some(n) = value.as_u64()
                    && n > 65536
                {
                    return invalid(
                        "hybrid_cache_entries exceeds maximum 65536".into(),
                        "hybrid_cache_entries_too_high",
                    );
                }
                Ok(())
            }
            Self::MaxNumSeqs => {
                if let Some(n) = value.as_u64()
                    && (n == 0 || n > 256)
                {
                    return invalid(
                        "max_num_seqs must be in range [1, 256]".into(),
                        "max_num_seqs_out_of_range",
                    );
                }
                Ok(())
            }
            Self::MaxConcurrentRequests => {
                if let Some(n) = value.as_u64()
                    && (n == 0 || n > 1024)
                {
                    return invalid(
                        "max_concurrent_requests must be in range [1, 1024]".into(),
                        "max_concurrent_requests_out_of_range",
                    );
                }
                Ok(())
            }
            Self::GpuMemoryUtilization => {
                if let Some(f) = value.as_f64()
                    && !(0.5..=1.0).contains(&f)
                {
                    return invalid(
                        "gpu_memory_utilization must be in range [0.5, 1.0]".into(),
                        "gpu_memory_utilization_out_of_range",
                    );
                }
                Ok(())
            }
            Self::WebUiStaticPath => {
                if let Some(path) = value.as_str()
                    && (path.contains("..") || (cfg!(unix) && path.starts_with('/')))
                {
                    return invalid(
                        "web_ui_static_path must be a relative path without traversal".into(),
                        "web_ui_static_path_traversal",
                    );
                }
                Ok(())
            }
            Self::WebUiConfigJson => {
                if let Some(json) = value.as_str()
                    && !json.is_empty()
                    && let Err(e) = serde_json::from_str::<serde_json::Value>(json)
                {
                    return invalid(
                        format!("web_ui_config_json is not valid JSON: {}", e),
                        "web_ui_config_json_invalid",
                    );
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Convert a setting value to CLI arguments.
    pub fn to_cli_args(&self, value: &serde_json::Value) -> Vec<String> {
        let mut args = Vec::new();

        match self {
            Self::KvCacheDtype => {
                let effective = value
                    .get("effective")
                    .and_then(|v| v.as_str())
                    .unwrap_or("int4");
                args.push("--kv-cache-dtype".into());
                args.push(effective.into());
            }
            Self::TurboquantMode => {
                let mode = value.as_str().unwrap_or("none");
                if mode != "none" {
                    args.push("--turboquant".into());
                    args.push(mode.into());
                }
            }
            Self::HybridCacheEntries => {
                if let Some(n) = value.as_u64()
                    && n > 0
                {
                    args.push("--hybrid-cache-entries".into());
                    args.push(n.to_string());
                }
            }
            Self::MaxNumSeqs => {
                if let Some(n) = value.as_u64()
                    && n != 4
                {
                    args.push("--max-num-seqs".into());
                    args.push(n.to_string());
                }
            }
            Self::MaxConcurrentRequests => {
                if let Some(n) = value.as_u64()
                    && n != 16
                {
                    args.push("--max-concurrent-requests".into());
                    args.push(n.to_string());
                }
            }
            Self::PrefillBatchSize => {
                if let Some(n) = value.as_u64() {
                    args.push("--prefill-batch-size".into());
                    args.push(n.to_string());
                }
            }
            Self::CompletionBatchSize => {
                if let Some(n) = value.as_u64() {
                    args.push("--completion-batch-size".into());
                    args.push(n.to_string());
                }
            }
            Self::GpuMemoryUtilization => {
                if let Some(f) = value.as_f64()
                    && f != 0.9
                {
                    args.push("--gpu-memory-utilization".into());
                    args.push(format!("{f}"));
                }
            }
            Self::WebUiStaticPath => {
                if let Some(path) = value.as_str()
                    && !path.is_empty()
                {
                    args.push("--path".into());
                    args.push(path.into());
                }
            }
            Self::WebUiConfigJson => {
                if let Some(json) = value.as_str()
                    && !json.is_empty()
                {
                    args.push("--ui-config".into());
                    args.push(json.into());
                }
            }
            _ => {}
        }

        args
    }

    /// Compute the effective value given a requested value and capabilities.
    pub fn effective_policy(
        &self,
        requested: &serde_json::Value,
        capabilities: &CapabilitySnapshot,
    ) -> serde_json::Value {
        match self {
            Self::KvCacheDtype => {
                let requested_eff = requested
                    .get("effective")
                    .and_then(|v| v.as_str())
                    .unwrap_or("int4");

                if !self.capability(capabilities) {
                    return serde_json::json!(null);
                }

                if requested_eff == "bf16" {
                    serde_json::json!({"effective": "int8", "reason": "model_safe_downgrade"})
                } else {
                    requested.clone()
                }
            }
            Self::TurboquantMode => {
                if !self.capability(capabilities) {
                    return serde_json::json!("none");
                }

                match requested.as_str().unwrap_or("none") {
                    "k8v4" | "v4" => {
                        serde_json::json!("none")
                    }
                    _ => requested.clone(),
                }
            }
            _ => {
                if !self.capability(capabilities) {
                    serde_json::json!(null)
                } else {
                    requested.clone()
                }
            }
        }
    }

    /// Unsupported reason when capability returns false.
    pub fn unsupported_reason(&self, snapshot: &CapabilitySnapshot) -> Option<String> {
        if self.capability(snapshot) {
            return None;
        }

        Some(match self {
            Self::KvCacheDtype => "Current runtime does not support --kv-cache-dtype".into(),
            Self::TurboquantMode => "Current runtime does not support --turboquant".into(),
            Self::ReasoningMode => "Current runtime does not support --reasoning".into(),
            Self::SpeculativePolicy => "Current runtime does not support --speculative".into(),
            Self::MllmVision => "Vision extra (mlx-vlm) is not installed or not qualified".into(),
            Self::Embeddings => {
                "Embeddings extra (mlx-embed) is not installed or not qualified".into()
            }
            Self::WebUiAvailability => "Web UI is not supported by this build".into(),
            Self::MaxNumSeqs => "Current runtime does not support --max-num-seqs".into(),
            Self::MaxConcurrentRequests => {
                "Current runtime does not support --max-concurrent-requests".into()
            }
            _ => "This setting is not supported by the current runtime".into(),
        })
    }
}

/// Complete catalog of all Rapid-MLX settings.
#[allow(dead_code)]
pub fn all_settings() -> &'static [RapidMlxSetting] {
    &[
        RapidMlxSetting::KvCacheDtype,
        RapidMlxSetting::TurboquantMode,
        RapidMlxSetting::PrefixCachePolicy,
        RapidMlxSetting::HybridCacheEntries,
        RapidMlxSetting::PflashPolicy,
        RapidMlxSetting::ResponseCachePolicy,
        RapidMlxSetting::DiskCheckpointPolicy,
        RapidMlxSetting::MaxNumSeqs,
        RapidMlxSetting::MaxConcurrentRequests,
        RapidMlxSetting::PrefillBatchSize,
        RapidMlxSetting::CompletionBatchSize,
        RapidMlxSetting::BatchingPolicy,
        RapidMlxSetting::ConcurrencyPolicy,
        RapidMlxSetting::ReasoningMode,
        RapidMlxSetting::SpeculativePolicy,
        RapidMlxSetting::MllmVision,
        RapidMlxSetting::Embeddings,
        RapidMlxSetting::GpuMemoryUtilization,
        RapidMlxSetting::WebUiAvailability,
        RapidMlxSetting::WebUiStaticPath,
        RapidMlxSetting::WebUiConfigJson,
        RapidMlxSetting::EndpointCompatibility,
        RapidMlxSetting::RequestSafetyPolicy,
        RapidMlxSetting::SamplingMode,
        RapidMlxSetting::ParserPolicy,
        RapidMlxSetting::SecurityPolicy,
    ]
}

/// Mutual exclusion rules for settings.
#[derive(Debug, Clone)]
pub struct MutualExclusionRule {
    pub settings: &'static [&'static str],
    pub error: &'static str,
}

/// All mutual exclusion rules.
/// Validates incompatible combinations before launch per Phase 7 builder brief item 3.
pub fn mutual_exclusion_rules() -> &'static [MutualExclusionRule] {
    &[
        MutualExclusionRule {
            settings: &["reasoning_mode", "sampling_mode"],
            error: "reasoning_mode=on and sampling_mode=model_default are mutually exclusive",
        },
        MutualExclusionRule {
            settings: &["pflash_policy", "turboquant_mode"],
            error: "pflash_policy bypasses TurboQuant; these policies cannot be combined",
        },
        MutualExclusionRule {
            settings: &["speculative_policy", "max_num_seqs"],
            error: "speculative_decoding requires dedicated runtime qualification; explicit max_num_seqs may conflict with MTP scheduling",
        },
    ]
}

/// Check mutual exclusions for a set of settings.
pub fn check_mutual_exclusions(
    settings: &BTreeMap<&'static str, serde_json::Value>,
) -> Result<(), ValidationError> {
    for rule in mutual_exclusion_rules() {
        let mut triggered = false;
        for setting_id in rule.settings {
            if let Some(value) = settings.get(setting_id)
                && (value.as_str().unwrap_or("") == "on"
                    || value.as_str().unwrap_or("") == "model_default")
            {
                triggered = true;
            }
        }
        if triggered {
            return Err(ValidationError {
                setting_id: rule.settings[0],
                message: rule.error.into(),
                code: "mutual_exclusion",
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_snapshot() -> CapabilitySnapshot {
        CapabilitySnapshot {
            executable_identity: Default::default(),
            rapid_mlx_version: "0.10.10".into(),
            help_hash: "hash".into(),
            serve_flags: vec![
                "--kv-cache-dtype".into(),
                "--turboquant".into(),
                "--max-cache-blocks".into(),
                "--reasoning".into(),
                "--max-num-seqs".into(),
                "--max-concurrent-requests".into(),
                "--prefill-batch-size".into(),
                "--completion-batch-size".into(),
                "--gpu-memory-utilization".into(),
            ],
            package_versions: vec![],
            installed_extras: Default::default(),
            qualified_features: Default::default(),
            mtp_concurrency: Default::default(),
            sampling_defaults: Default::default(),
            sampling_cascade: Default::default(),
            evidence_timestamp: 0,
            source: Default::default(),
        }
    }

    #[test]
    fn all_settings_have_unique_ids() {
        let ids: std::collections::HashSet<_> = all_settings().iter().map(|s| s.id()).collect();
        assert_eq!(ids.len(), all_settings().len());
    }

    #[test]
    fn kv_cache_dtype_capability_works() {
        let snapshot = test_snapshot();
        let setting = RapidMlxSetting::KvCacheDtype;
        assert!(setting.capability(&snapshot));
        assert!(setting.unsupported_reason(&snapshot).is_none());
    }

    #[test]
    fn turboquant_capability_works() {
        let snapshot = test_snapshot();
        let setting = RapidMlxSetting::TurboquantMode;
        assert!(setting.capability(&snapshot));
    }

    #[test]
    fn unsupported_setting_returns_reason() {
        let mut snapshot = CapabilitySnapshot::default();
        snapshot.serve_flags.push("--host".into());
        snapshot.serve_flags.push("--port".into());

        let setting = RapidMlxSetting::KvCacheDtype;
        assert!(!setting.capability(&snapshot));
        assert!(setting.unsupported_reason(&snapshot).is_some());
    }

    #[test]
    fn kv_cache_dtype_validation_accepts_valid_values() {
        let setting = RapidMlxSetting::KvCacheDtype;
        let ctx = ValidationContext::default();

        assert!(
            setting
                .validate(&serde_json::json!({"effective": "int4"}), &ctx)
                .is_ok()
        );
        assert!(
            setting
                .validate(&serde_json::json!({"effective": "int8"}), &ctx)
                .is_ok()
        );
        assert!(
            setting
                .validate(&serde_json::json!({"effective": "bf16"}), &ctx)
                .is_ok()
        );
    }

    #[test]
    fn kv_cache_dtype_validation_rejects_invalid_value() {
        let setting = RapidMlxSetting::KvCacheDtype;
        let ctx = ValidationContext::default();

        let err = setting
            .validate(&serde_json::json!({"effective": "fp16"}), &ctx)
            .unwrap_err();
        assert_eq!(err.code, "invalid_kv_dtype");
    }

    #[test]
    fn turboquant_mode_validation() {
        let setting = RapidMlxSetting::TurboquantMode;
        let ctx = ValidationContext::default();

        assert!(setting.validate(&serde_json::json!("none"), &ctx).is_ok());
        assert!(setting.validate(&serde_json::json!("k8v4"), &ctx).is_ok());
        assert!(setting.validate(&serde_json::json!("v4"), &ctx).is_ok());

        let err = setting
            .validate(&serde_json::json!("invalid"), &ctx)
            .unwrap_err();
        assert_eq!(err.code, "invalid_turboquant_mode");
    }

    #[test]
    fn hybrid_cache_entries_validation() {
        let setting = RapidMlxSetting::HybridCacheEntries;
        let ctx = ValidationContext::default();

        assert!(setting.validate(&serde_json::json!(100), &ctx).is_ok());
        assert!(setting.validate(&serde_json::json!(65536), &ctx).is_ok());

        let err = setting
            .validate(&serde_json::json!(70000), &ctx)
            .unwrap_err();
        assert_eq!(err.code, "hybrid_cache_entries_too_high");
    }

    #[test]
    fn max_num_seqs_validation() {
        let setting = RapidMlxSetting::MaxNumSeqs;
        let ctx = ValidationContext::default();

        assert!(setting.validate(&serde_json::json!(1), &ctx).is_ok());
        assert!(setting.validate(&serde_json::json!(256), &ctx).is_ok());

        let err = setting.validate(&serde_json::json!(0), &ctx).unwrap_err();
        assert_eq!(err.code, "max_num_seqs_out_of_range");

        let err = setting.validate(&serde_json::json!(300), &ctx).unwrap_err();
        assert_eq!(err.code, "max_num_seqs_out_of_range");
    }

    #[test]
    fn gpu_memory_utilization_validation() {
        let setting = RapidMlxSetting::GpuMemoryUtilization;
        let ctx = ValidationContext::default();

        assert!(setting.validate(&serde_json::json!(0.5), &ctx).is_ok());
        assert!(setting.validate(&serde_json::json!(0.9), &ctx).is_ok());
        assert!(setting.validate(&serde_json::json!(1.0), &ctx).is_ok());

        let err = setting.validate(&serde_json::json!(0.4), &ctx).unwrap_err();
        assert_eq!(err.code, "gpu_memory_utilization_out_of_range");
    }

    #[test]
    fn web_ui_static_path_traversal_rejected() {
        let setting = RapidMlxSetting::WebUiStaticPath;
        let ctx = ValidationContext::default();

        let err = setting
            .validate(&serde_json::json!("../escape"), &ctx)
            .unwrap_err();
        assert_eq!(err.code, "web_ui_static_path_traversal");
    }

    #[test]
    fn web_ui_config_json_validates_json() {
        let setting = RapidMlxSetting::WebUiConfigJson;
        let ctx = ValidationContext::default();

        assert!(setting.validate(&serde_json::json!("{}"), &ctx).is_ok());
        assert!(setting.validate(&serde_json::json!(""), &ctx).is_ok());

        let err = setting
            .validate(&serde_json::json!("not json"), &ctx)
            .unwrap_err();
        assert_eq!(err.code, "web_ui_config_json_invalid");
    }

    #[test]
    fn to_cli_args_kvcache_dtype() {
        let setting = RapidMlxSetting::KvCacheDtype;
        let args = setting.to_cli_args(&serde_json::json!({"effective": "int8"}));
        assert_eq!(args, vec!["--kv-cache-dtype", "int8"]);
    }

    #[test]
    fn to_cli_args_turboquant() {
        let setting = RapidMlxSetting::TurboquantMode;
        let args_none = setting.to_cli_args(&serde_json::json!("none"));
        assert!(args_none.is_empty());

        let args_k8v4 = setting.to_cli_args(&serde_json::json!("k8v4"));
        assert_eq!(args_k8v4, vec!["--turboquant", "k8v4"]);
    }

    #[test]
    fn to_cli_args_max_num_seqs_default_omitted() {
        let setting = RapidMlxSetting::MaxNumSeqs;
        let args_default = setting.to_cli_args(&serde_json::json!(4));
        assert!(args_default.is_empty());

        let args_explicit = setting.to_cli_args(&serde_json::json!(8));
        assert_eq!(args_explicit, vec!["--max-num-seqs", "8"]);
    }

    #[test]
    fn effective_policy_kvcache_dtype() {
        let snapshot = test_snapshot();
        let setting = RapidMlxSetting::KvCacheDtype;

        let effective =
            setting.effective_policy(&serde_json::json!({"effective": "int4"}), &snapshot);
        assert_eq!(effective, serde_json::json!({"effective": "int4"}));
    }

    #[test]
    fn effective_policy_turboquant_downgrade() {
        let snapshot = test_snapshot();
        let setting = RapidMlxSetting::TurboquantMode;

        let effective = setting.effective_policy(&serde_json::json!("k8v4"), &snapshot);
        assert_eq!(effective, serde_json::json!("none"));
    }

    #[test]
    fn all_settings_have_default_value() {
        for setting in all_settings() {
            let default = setting.default_value();
            assert!(
                !default.is_null()
                    || matches!(
                        setting,
                        RapidMlxSetting::PrefillBatchSize
                            | RapidMlxSetting::CompletionBatchSize
                            | RapidMlxSetting::WebUiStaticPath
                            | RapidMlxSetting::WebUiConfigJson
                    )
            );
        }
    }
}
