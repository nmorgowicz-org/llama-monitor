//! Backend-neutral detection of whether a model has a vision component.
//!
//! Detection is deliberately separate from any one runtime's *support* for
//! vision. Rapid-MLX's vision path is currently unusable, but "does this model
//! have a vision tower" is a fact about the checkpoint, and every MLX loader
//! (rapid-mlx, MTPLX, mlx-vlm, future ones) needs the same answer from the same
//! artifacts. Keeping the detector here means a second loader inherits it rather
//! than reimplementing the heuristics.
//!
//! Two callers exist today: [`crate::hf::qualify`] reads the artifacts over HTTP
//! before download, and [`crate::inference::rapid_mlx::mlx_meta`] reads them off
//! local disk after. They previously disagreed — the local path recognised only
//! `vision_config` and never looked at the weights at all.
//!
//! Order of evidence, strongest first:
//!
//! 1. `config.json` names a vision component.
//! 2. The safetensors index contains vision tensors.
//! 3. Neither does, and both were readable → vision is *confirmed absent*.
//!
//! A repo name, a `vision` tag, and a `preprocessor_config.json` are somebody's
//! labelling, not the model's own artifact, so they are not handled here. Callers
//! that fall back to them must label the result as a heuristic; that is why this
//! module returns `None` for "the artifacts did not settle it" rather than
//! guessing.

use serde_json::Value;

/// Config keys that establish a vision component.
pub const VISION_CONFIG_KEYS: &[&str] = &["vision_config", "vision_tower", "mm_vision_tower"];

/// Tensor-name prefixes that only exist when a vision tower is actually present
/// in the weights. Matched against the safetensors index, so this reads the
/// model's own artifact rather than its packaging.
pub const VISION_TENSOR_MARKERS: &[&str] = &[
    "vision_tower",
    "vision_model",
    "visual.",
    "mm_projector",
    "multi_modal_projector",
    "image_newline",
];

/// A vision verdict together with the artifact that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionEvidence {
    /// Whether a vision component is present.
    pub vision: bool,
    /// What established the verdict, e.g. `config.json:vision_config` or
    /// `model.safetensors.index.json:vision_tower`.
    pub source: String,
}

/// The config key that establishes a vision component, if any.
pub fn vision_config_key(value: &Value) -> Option<&'static str> {
    let object = value.as_object()?;
    VISION_CONFIG_KEYS
        .iter()
        .find(|key| object.contains_key(**key))
        .copied()
}

/// The vision tensor marker present in a safetensors index, if any.
pub fn vision_tensor_marker(value: &Value) -> Option<&'static str> {
    let weight_map = value.get("weight_map")?.as_object()?;
    VISION_TENSOR_MARKERS.iter().copied().find(|marker| {
        weight_map
            .keys()
            .any(|tensor| tensor.to_ascii_lowercase().contains(*marker))
    })
}

/// Resolve a vision verdict from the model's own artifacts.
///
/// `None` for either argument means "could not read it", which is deliberately
/// distinct from "read it and found nothing": only the latter can confirm an
/// absence. Returns `None` when the artifacts do not settle the question, so the
/// caller can fall back to heuristics and label them as such.
pub fn resolve_from_artifacts(
    config: Option<&Value>,
    index: Option<&Value>,
) -> Option<VisionEvidence> {
    if let Some(value) = config
        && let Some(key) = vision_config_key(value)
    {
        return Some(VisionEvidence {
            vision: true,
            source: format!("config.json:{key}"),
        });
    }

    if let Some(value) = index
        && let Some(marker) = vision_tensor_marker(value)
    {
        return Some(VisionEvidence {
            vision: true,
            source: format!("model.safetensors.index.json:{marker}"),
        });
    }

    // Both artifacts read cleanly and neither carries a vision component. A
    // checkpoint can name no vision config yet still ship a tower, so the
    // absence is only confirmed when the weights were readable too.
    if config.is_some() && index.is_some() {
        return Some(VisionEvidence {
            vision: false,
            source: "config.json + model.safetensors.index.json show no vision component".into(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn config_keys_beyond_vision_config_are_recognised() {
        assert_eq!(
            vision_config_key(&json!({"vision_config": {}})),
            Some("vision_config")
        );
        assert_eq!(
            vision_config_key(&json!({"mm_vision_tower": "openai/clip-vit-large"})),
            Some("mm_vision_tower")
        );
        assert_eq!(vision_config_key(&json!({"num_hidden_layers": 32})), None);
    }

    #[test]
    fn tensor_marker_reads_the_weight_map() {
        let index = json!({"weight_map": {
            "model.layers.0.self_attn.q_proj.weight": "model-00001.safetensors",
            "vision_tower.encoder.layers.0.mlp.fc1.weight": "model-00002.safetensors",
        }});
        assert_eq!(vision_tensor_marker(&index), Some("vision_tower"));

        let text_only = json!({"weight_map": {
            "model.layers.0.self_attn.q_proj.weight": "model-00001.safetensors",
        }});
        assert_eq!(vision_tensor_marker(&text_only), None);
    }

    #[test]
    fn config_outranks_the_index() {
        let evidence = resolve_from_artifacts(
            Some(&json!({"vision_config": {"num_hidden_layers": 27}})),
            Some(&json!({"weight_map": {"model.layers.0.q.weight": "a"}})),
        )
        .unwrap();
        assert!(evidence.vision);
        assert_eq!(evidence.source, "config.json:vision_config");
    }

    #[test]
    fn weights_settle_it_when_the_config_is_silent() {
        let evidence = resolve_from_artifacts(
            Some(&json!({"num_hidden_layers": 32})),
            Some(&json!({"weight_map": {"visual.blocks.0.attn.qkv.weight": "a"}})),
        )
        .unwrap();
        assert!(evidence.vision);
        assert_eq!(evidence.source, "model.safetensors.index.json:visual.");
    }

    #[test]
    fn absence_is_confirmed_only_when_both_artifacts_were_readable() {
        let both = resolve_from_artifacts(
            Some(&json!({"num_hidden_layers": 32})),
            Some(&json!({"weight_map": {"model.layers.0.q.weight": "a"}})),
        )
        .unwrap();
        assert!(!both.vision);

        // A single-shard model has no index. That is not evidence of absence,
        // so the caller must be told the artifacts did not settle it.
        assert!(resolve_from_artifacts(Some(&json!({"num_hidden_layers": 32})), None).is_none());
        assert!(resolve_from_artifacts(None, None).is_none());
    }
}
