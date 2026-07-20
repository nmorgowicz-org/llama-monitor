//! Backend-neutral model memory profile with field-level evidence.
//!
//! This module defines the shared geometry vocabulary that both GGUF and MLX parsers
//! populate. It intentionally contains NO runtime flags, no backend-specific allocation
//! behavior, and no llama.cpp vocabulary leakage into MLX paths. Per A53/D1/D2:
//!
//! - The shared profile describes model geometry and components only.
//! - Separate backend execution policies produce memory breakdowns from this geometry.
//! - Every populated field carries field_evidence identifying its source.
//!
//! NOTE: dead_code allowed until Parts B/C wire up consumption (Phase 4 Part B: geometry
//! population, MoE/MTP/companions; Part C: HF lookup, context propagation, estimator integration).

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// A single geometry component with its provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidencedField<T> {
    pub value: T,
    /// Identifies which config field (or fallback) produced this value.
    pub field_evidence: String,
}

impl<T: Default> Default for EvidencedField<T> {
    fn default() -> Self {
        Self {
            value: T::default(),
            field_evidence: String::new(),
        }
    }
}

/// Helper for serde skip_serializing_if on EvidencedField<u32>.
fn ef_is_empty_u32(ef: &EvidencedField<u32>) -> bool {
    ef.value == 0 && ef.field_evidence.is_empty()
}

/// Helper for serde skip_serializing_if on EvidencedField<f64>.
fn ef_is_empty_f64(ef: &EvidencedField<f64>) -> bool {
    ef.value == 0.0 && ef.field_evidence.is_empty()
}

/// Normalized layer groups: full attention, local/sliding attention, linear/recurrent,
/// and plain linear (no attention). Per D2: these capture the memory-relevant geometry
/// without backend-specific allocation semantics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayerMemoryGroup {
    pub kind: LayerGroupKind,
    /// Number of layers in this group.
    pub count: u32,
    /// Which config field proved this count.
    pub field_evidence: String,
    /// KV heads for layers in this group (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_heads: Option<u32>,
    /// Head dimension for this group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_dim: Option<u32>,
    /// Sliding window for local-attention layers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sliding_window: Option<u32>,
    /// Recurrent state size in bytes for linear/recurrent layers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrent_state_bytes: Option<u64>,
}

/// The normalized attention/compute type for a layer group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LayerGroupKind {
    #[default]
    FullAttention,
    LocalAttention,
    LinearRecurrent,
    LinearOnly,
}

/// Weight and quantization components with evidence.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WeightComponents {
    #[serde(skip_serializing_if = "ef_is_empty_u32")]
    pub n_embd: EvidencedField<u32>,
    #[serde(skip_serializing_if = "ef_is_empty_u32")]
    pub n_head: EvidencedField<u32>,
    #[serde(skip_serializing_if = "ef_is_empty_u32")]
    pub n_head_kv: EvidencedField<u32>,
    #[serde(skip_serializing_if = "ef_is_empty_u32")]
    pub n_layers: EvidencedField<u32>,
    #[serde(skip_serializing_if = "ef_is_empty_u32")]
    pub n_ff: EvidencedField<u32>,
    #[serde(skip_serializing_if = "ef_is_empty_u32")]
    pub head_dim: EvidencedField<u32>,
    #[serde(skip_serializing_if = "ef_is_empty_u32")]
    pub vocab_size: EvidencedField<u32>,
    #[serde(skip_serializing_if = "ef_is_empty_f64")]
    pub rms_norm_eps: EvidencedField<f64>,
    /// Model context ceiling from config.
    #[serde(skip_serializing_if = "ef_is_empty_u32")]
    pub max_position_embeddings: EvidencedField<u32>,
    /// Quantization bits if present in config.
    #[serde(skip_serializing_if = "ef_is_empty_u32")]
    pub quant_bits: EvidencedField<u32>,
    /// Quantization group size if present in config.
    #[serde(skip_serializing_if = "ef_is_empty_u32")]
    pub quant_group_size: EvidencedField<u32>,
}

impl WeightComponents {
    fn is_empty(&self) -> bool {
        self.n_embd.is_empty()
            && self.n_head.is_empty()
            && self.n_head_kv.is_empty()
            && self.n_layers.is_empty()
            && self.n_ff.is_empty()
            && self.head_dim.is_empty()
            && self.vocab_size.is_empty()
            && self.rms_norm_eps.is_empty()
            && self.max_position_embeddings.is_empty()
            && self.quant_bits.is_empty()
            && self.quant_group_size.is_empty()
    }
}

impl EvidencedField<u32> {
    fn is_empty(&self) -> bool {
        self.value == 0 && self.field_evidence.is_empty()
    }
}

impl EvidencedField<f64> {
    fn is_empty(&self) -> bool {
        self.value == 0.0 && self.field_evidence.is_empty()
    }
}

/// Expert topology for MoE models.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExpertTopology {
    pub n_experts: u32,
    pub field_evidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k_evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_expert_intermediate_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_expert_intermediate_evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moe_intermediate_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moe_intermediate_evidence: Option<String>,
}

impl ExpertTopology {
    pub fn is_some(&self) -> bool {
        self.n_experts > 0
    }
}

/// Recurrent/linear attention state geometry (DeltaNet, SSM, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecurrentStateGeometry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear_key_head_dim: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear_key_head_dim_evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear_value_head_dim: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear_value_head_dim_evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear_num_key_heads: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear_num_key_heads_evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear_num_value_heads: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear_num_value_heads_evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear_conv_kernel_dim: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear_conv_kernel_dim_evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mamba_ssm_dtype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mamba_ssm_dtype_evidence: Option<String>,
}

impl RecurrentStateGeometry {
    pub fn is_some(&self) -> bool {
        self.linear_key_head_dim.is_some()
            || self.linear_value_head_dim.is_some()
            || self.linear_num_key_heads.is_some()
            || self.linear_num_value_heads.is_some()
            || self.linear_conv_kernel_dim.is_some()
    }
}

/// Global/local head geometry for models like Gemma4.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalLocalHeadGeometry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_global_key_value_heads: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_global_kv_evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_head_dim: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_head_dim_evidence: Option<String>,
}

impl GlobalLocalHeadGeometry {
    pub fn is_some(&self) -> bool {
        self.num_global_key_value_heads.is_some() || self.global_head_dim.is_some()
    }
}

/// Embedded MTP (Multi-Token Prediction) component.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmbeddedMtpComponent {
    pub n_layers: u32,
    pub field_evidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_dedicated_embeddings: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_dedicated_embeddings_evidence: Option<String>,
}

impl EmbeddedMtpComponent {
    pub fn is_some(&self) -> bool {
        self.n_layers > 0
    }
}

/// Vision component geometry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisionComponent {
    pub has_vision_config: bool,
    pub field_evidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_type_evidence: Option<String>,
}

impl VisionComponent {
    pub fn is_some(&self) -> bool {
        self.has_vision_config
    }
}

/// Warnings about config discrepancies detected during parsing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ParseWarnings {
    #[serde(default)]
    pub wrapper_field_conflicts: Vec<ParseWarning>,
    #[serde(default)]
    pub missing_critical_fields: Vec<ParseWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseWarning {
    pub field: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outer_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inner_value: Option<String>,
}

impl ParseWarnings {
    fn is_empty(&self) -> bool {
        self.wrapper_field_conflicts.is_empty() && self.missing_critical_fields.is_empty()
    }
}

/// The backend-neutral model memory profile.
///
/// Per D1/A53:
/// - GGUF and MLX parsers populate this one evidence-bearing normalized profile.
/// - Separate llama.cpp and Rapid-MLX execution policies/calculators produce memory breakdowns.
/// - Shared geometry must never force shared runtime math or llama vocabulary into MLX.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelMemoryProfile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_type_evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architectures: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architectures_evidence: Option<String>,
    pub weights: WeightComponents,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub layer_groups: Vec<LayerMemoryGroup>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experts: Option<ExpertTopology>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrent_state: Option<RecurrentStateGeometry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vision: Option<VisionComponent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedded_mtp: Option<EmbeddedMtpComponent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_attention_interval: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_attention_interval_evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_types_evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_local_heads: Option<GlobalLocalHeadGeometry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sliding_window: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sliding_window_evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_context_limit: Option<u32>,
    #[serde(skip_serializing_if = "ParseWarnings::is_empty")]
    pub warnings: ParseWarnings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_text_config: Option<bool>,
}

impl ModelMemoryProfile {
    pub fn is_substantive(&self) -> bool {
        !self.weights.is_empty()
    }

    pub fn total_layer_count(&self) -> u32 {
        self.layer_groups.iter().map(|g| g.count).sum()
    }

    pub fn full_attention_layer_count(&self) -> u32 {
        self.layer_groups
            .iter()
            .filter(|g| matches!(g.kind, LayerGroupKind::FullAttention))
            .map(|g| g.count)
            .sum()
    }

    pub fn local_attention_layer_count(&self) -> u32 {
        self.layer_groups
            .iter()
            .filter(|g| matches!(g.kind, LayerGroupKind::LocalAttention))
            .map(|g| g.count)
            .sum()
    }

    pub fn linear_recurrent_layer_count(&self) -> u32 {
        self.layer_groups
            .iter()
            .filter(|g| matches!(g.kind, LayerGroupKind::LinearRecurrent))
            .map(|g| g.count)
            .sum()
    }

    pub fn is_moe(&self) -> bool {
        self.experts.as_ref().map(|e| e.is_some()).unwrap_or(false)
    }

    pub fn is_hybrid_attention(&self) -> bool {
        self.full_attention_layer_count() > 0
            && self.full_attention_layer_count() < self.total_layer_count()
    }

    pub fn has_local_attention(&self) -> bool {
        self.local_attention_layer_count() > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_group_counts_are_correct() {
        let mut profile = ModelMemoryProfile::default();
        profile.layer_groups.push(LayerMemoryGroup {
            kind: LayerGroupKind::FullAttention,
            count: 10,
            field_evidence: "counted from layer_types".into(),
            kv_heads: Some(4),
            head_dim: Some(256),
            ..Default::default()
        });
        profile.layer_groups.push(LayerMemoryGroup {
            kind: LayerGroupKind::LinearRecurrent,
            count: 30,
            field_evidence: "counted from layer_types".into(),
            recurrent_state_bytes: Some(1024),
            ..Default::default()
        });

        assert_eq!(profile.total_layer_count(), 40);
        assert_eq!(profile.full_attention_layer_count(), 10);
        assert_eq!(profile.linear_recurrent_layer_count(), 30);
        assert!(profile.is_hybrid_attention());
        assert!(!profile.has_local_attention());
    }

    #[test]
    fn local_attention_group_counts_are_correct() {
        let mut profile = ModelMemoryProfile::default();
        profile.layer_groups.push(LayerMemoryGroup {
            kind: LayerGroupKind::FullAttention,
            count: 5,
            field_evidence: "counted".into(),
            kv_heads: Some(2),
            head_dim: Some(512),
            ..Default::default()
        });
        profile.layer_groups.push(LayerMemoryGroup {
            kind: LayerGroupKind::LocalAttention,
            count: 25,
            field_evidence: "counted".into(),
            kv_heads: Some(8),
            head_dim: Some(256),
            sliding_window: Some(1024),
            ..Default::default()
        });

        assert_eq!(profile.local_attention_layer_count(), 25);
        assert!(profile.has_local_attention());
        // full + local = hybrid (multiple layer types)
        assert!(profile.is_hybrid_attention());
    }

    #[test]
    fn moe_detection() {
        let mut profile = ModelMemoryProfile::default();
        profile.experts = Some(ExpertTopology {
            n_experts: 128,
            field_evidence: "text_config.num_experts".into(),
            top_k: Some(8),
            top_k_evidence: Some("text_config.top_k_experts".into()),
            ..Default::default()
        });
        assert!(profile.is_moe());
    }
}
