//! MLX / Rapid-MLX model metadata reader.
//!
//! Rapid-MLX models ship as a directory (or an equivalent HuggingFace repo) containing:
//! - `config.json` — an HF-transformers-style architecture config (`hidden_size`,
//!   `num_hidden_layers`, `num_attention_heads`, `num_key_value_heads`, `intermediate_size`,
//!   MoE fields, sliding-window/attention-pattern fields, a Rapid-MLX `quantization` block, and
//!   an optional draft/MTP/speculative sidecar sub-config).
//! - `model.safetensors.index.json` — the `weight_map` (+ a `metadata.total_size` field on
//!   HF-exported indexes) used for exact per-model byte accounting.
//!
//! This module mirrors `crate::llama::gguf_meta`: every field is optional and missing/
//! unrecognized fields degrade to [`MlxMetaEvidence::Degraded`] rather than being silently
//! guessed at — callers must check `evidence` and fall back to a name/param heuristic
//! (`ModelArch::from_name_and_params`) instead of presenting a degraded guess as authoritative.

use std::path::Path;

use crate::llama::vram_estimator::ModelArch;

/// `config.json` is always a small file; bound the read so a hostile/corrupt file can't force
/// an unbounded allocation.
pub const MAX_CONFIG_BYTES: u64 = 8 * 1024 * 1024;
/// `model.safetensors.index.json` can list thousands of shards for very large MoE models;
/// still bounded to avoid unbounded reads.
pub const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;

/// How much of an [`MlxMetadata`] was derived from real, parseable model files versus a
/// name/param heuristic fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MlxMetaEvidence {
    /// The required architecture fields (`hidden_size`, `num_hidden_layers`,
    /// `num_attention_heads`) were present and parsed directly from `config.json`.
    #[default]
    Exact,
    /// One or more required fields were missing/unrecognized; the resulting `ModelArch` falls
    /// back to `ModelArch::from_name_and_params` for the fields that could not be read.
    Degraded,
}

/// Rapid-MLX quantization metadata (`quantization` block in `config.json`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MlxQuantization {
    pub bits: Option<u32>,
    pub group_size: Option<u32>,
}

/// Draft/MTP/speculative-decoding sidecar config, whether nested inline in `config.json`
/// (`draft_model` / `speculative_config`) or read from a sibling `draft/config.json`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MlxDraftConfig {
    /// Local path or repo id of the draft model, when specified inline.
    pub model: Option<String>,
    pub num_hidden_layers: Option<u32>,
}

/// HF-transformers-style `config.json`. Every field is optional: an MLX config that omits a
/// field (or uses a naming convention we don't recognize yet) must not be silently guessed —
/// see [`MlxMetaEvidence`].
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MlxConfig {
    pub model_type: Option<String>,
    pub hidden_size: Option<u32>,
    pub num_hidden_layers: Option<u32>,
    pub num_attention_heads: Option<u32>,
    #[serde(alias = "num_kv_heads")]
    pub num_key_value_heads: Option<u32>,
    pub intermediate_size: Option<u32>,
    pub head_dim: Option<u32>,
    // ── MoE ───────────────────────────────────────────────────────────────────
    #[serde(alias = "num_local_experts", alias = "n_routed_experts")]
    pub num_experts: Option<u32>,
    #[serde(alias = "num_experts_per_token", alias = "n_experts_used")]
    pub num_experts_per_tok: Option<u32>,
    // ── Sliding-window / alternating attention ──────────────────────────────────
    pub sliding_window: Option<u32>,
    pub sliding_window_pattern: Option<u32>,
    pub max_position_embeddings: Option<u32>,
    // ── Rapid-MLX quantization ───────────────────────────────────────────────────
    pub quantization: Option<MlxQuantization>,
    // ── Draft / MTP / speculative sidecar ────────────────────────────────────────
    pub draft_model: Option<MlxDraftConfig>,
    pub speculative_config: Option<MlxDraftConfig>,
    // ── Vision / projector sub-config ────────────────────────────────────────────
    /// Presence alone signals a multimodal model. The vision tower's exact VRAM cost isn't
    /// derivable from `config.json` alone (it depends on the projector's real tensor sizes),
    /// so we only use this to flag the model as needing a `mmproj`-equivalent budget — the
    /// exact byte count still comes from the safetensors index / caller-supplied size, same as
    /// the GGUF `mmproj_bytes` path.
    pub vision_config: Option<serde_json::Value>,
}

/// Exact per-tensor-file accounting derived from `model.safetensors.index.json`.
#[derive(Debug, Clone, Default)]
pub struct MlxWeightIndex {
    /// Shard file names (relative, validated to not escape the model directory).
    pub shard_files: Vec<String>,
    /// `metadata.total_size`, when the index carries it (real HF-exported indexes always do).
    pub total_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct MlxMetadata {
    pub config: MlxConfig,
    pub weight_index: MlxWeightIndex,
    pub evidence: MlxMetaEvidence,
}

fn bounded_read(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > max_bytes {
        return Err(format!(
            "{} exceeds the {max_bytes}-byte read cap ({} bytes)",
            path.display(),
            meta.len()
        ));
    }
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Parse an MLX `config.json` from a local model directory.
pub fn read_mlx_config(dir: &Path) -> Result<MlxConfig, String> {
    let bytes = bounded_read(&dir.join("config.json"), MAX_CONFIG_BYTES)?;
    parse_mlx_config(&bytes)
}

/// Parse an MLX `config.json` from raw bytes (e.g. a HuggingFace GET response body).
pub fn parse_mlx_config(bytes: &[u8]) -> Result<MlxConfig, String> {
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(format!(
            "config.json exceeds the {MAX_CONFIG_BYTES}-byte read cap"
        ));
    }
    serde_json::from_slice(bytes).map_err(|e| format!("config.json is not valid JSON: {e}"))
}

/// Parse `model.safetensors.index.json` from a local model directory.
///
/// Shard names are validated the same way as `model_resolver::safetensors_files`: no absolute
/// paths, no `..` traversal, and only `.safetensors` files — an unsafe entry degrades the
/// index to empty rather than being trusted.
pub fn read_mlx_weight_index(dir: &Path) -> Result<MlxWeightIndex, String> {
    let index_path = dir.join("model.safetensors.index.json");
    let bytes = bounded_read(&index_path, MAX_INDEX_BYTES)?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("safetensors index: {e}"))?;

    let weight_map = value
        .get("weight_map")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "safetensors index requires a weight_map object".to_string())?;

    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for v in weight_map.values() {
        let Some(name) = v.as_str() else {
            return Err("safetensors index contains a non-string shard".into());
        };
        let relative = Path::new(name);
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
            || !name.ends_with(".safetensors")
        {
            return Err(format!(
                "safetensors index contains an unsafe shard path: {name}"
            ));
        }
        names.insert(name.to_string());
    }

    let total_size_bytes = value
        .get("metadata")
        .and_then(|m| m.get("total_size"))
        .and_then(|t| t.as_u64());

    Ok(MlxWeightIndex {
        shard_files: names.into_iter().collect(),
        total_size_bytes,
    })
}

/// Sum the on-disk size of every shard in the index (exact, real-file accounting), preferring
/// the index's own `metadata.total_size` when present (avoids re-statting every shard).
pub fn resolve_local_weight_bytes(dir: &Path, index: &MlxWeightIndex) -> Option<u64> {
    if let Some(total) = index.total_size_bytes {
        return Some(total);
    }
    if index.shard_files.is_empty() {
        return None;
    }
    let mut total = 0u64;
    for name in &index.shard_files {
        let meta = std::fs::metadata(dir.join(name)).ok()?;
        total = total.saturating_add(meta.len());
    }
    Some(total)
}

/// Read full MLX metadata (config + weight index) from a local model directory.
///
/// Never fails on a missing/unparseable safetensors index (weight accounting simply degrades
/// to the caller-supplied file size); only a missing/invalid `config.json` is a hard error,
/// since it's the sole source of architecture fields.
pub fn read_mlx_metadata(dir: &Path) -> Result<MlxMetadata, String> {
    let config = read_mlx_config(dir)?;
    let weight_index = read_mlx_weight_index(dir).unwrap_or_default();
    Ok(finish_metadata(config, weight_index))
}

/// Build metadata from an already-parsed [`MlxConfig`] with no local weight index available
/// (HF pre-download path: the file listing supplies the size instead).
pub fn metadata_from_config(config: MlxConfig) -> MlxMetadata {
    finish_metadata(config, MlxWeightIndex::default())
}

fn finish_metadata(config: MlxConfig, weight_index: MlxWeightIndex) -> MlxMetadata {
    let evidence = if config.hidden_size.is_some()
        && config.num_hidden_layers.is_some()
        && config.num_attention_heads.is_some()
    {
        MlxMetaEvidence::Exact
    } else {
        MlxMetaEvidence::Degraded
    };
    MlxMetadata {
        config,
        weight_index,
        evidence,
    }
}

impl MlxMetadata {
    /// Convert MLX config metadata into the backend-neutral `ModelArch` shared with the GGUF
    /// estimator path. When `evidence` is [`MlxMetaEvidence::Degraded`], the architecture-shape
    /// fields (layers/embd/heads) start from `ModelArch::from_name_and_params` and are then
    /// overridden with any real fields present — mirroring
    /// `ModelMetadata::to_arch`'s GGUF-overrides-heuristic pattern.
    pub fn to_arch(&self, model_size_bytes: u64, param_b: f64, fallback_name: &str) -> ModelArch {
        let mut arch = if self.evidence == MlxMetaEvidence::Degraded {
            ModelArch::from_name_and_params(fallback_name, param_b)
        } else {
            ModelArch::default()
        };

        let cfg = &self.config;
        if let Some(layers) = cfg.num_hidden_layers {
            arch.n_layers = layers;
        }
        if let Some(embd) = cfg.hidden_size {
            arch.n_embd = embd;
        }
        let n_head = cfg.num_attention_heads;
        if let Some(kv) = cfg.num_key_value_heads {
            arch.n_kv_heads = kv;
        } else if arch.n_kv_heads == 0
            && let Some(h) = n_head
        {
            arch.n_kv_heads = h;
        }
        let head_dim = cfg.head_dim.or_else(|| {
            let embd = cfg.hidden_size?;
            let heads = n_head?;
            embd.checked_div(heads)
        });
        if let Some(hd) = head_dim {
            arch.head_dim = hd;
        }

        if let Some(experts) = cfg.num_experts {
            arch.n_experts = experts;
        }
        if let Some(used) = cfg.num_experts_per_tok {
            arch.n_experts_used = used;
        }
        if arch.n_experts > 0 && arch.expert_fraction == 0.0 {
            arch.expert_fraction = 0.65;
        }

        if let Some(window) = cfg.sliding_window {
            arch.local_attn_window = window;
        }

        // Exact per-layer weight accounting: prefer the real on-disk/HF-listed size over any
        // heuristic guess, same as the GGUF tensor-directory path.
        if arch.n_layers > 0 {
            arch.bytes_per_layer = model_size_bytes / arch.n_layers as u64;
        }
        arch.param_b = param_b;

        // Draft/MTP sidecar: presence alone is enough to reserve a nonzero MTP budget; we don't
        // yet have a real per-model calibration for Rapid-MLX draft-head cost, so this reuses
        // the same `mtp_depth` slot the GGUF path uses (`mtp_overhead_bytes` in the estimator
        // applies a flat ~1.5%-of-weights-per-depth approximation for both backends).
        if cfg.draft_model.is_some() || cfg.speculative_config.is_some() {
            arch.mtp_depth = arch.mtp_depth.max(1);
        }

        arch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_config(dir: &std::path::Path, json: &str) {
        let mut f = std::fs::File::create(dir.join("config.json")).unwrap();
        f.write_all(json.as_bytes()).unwrap();
    }

    fn write_index(dir: &std::path::Path, json: &str) {
        let mut f = std::fs::File::create(dir.join("model.safetensors.index.json")).unwrap();
        f.write_all(json.as_bytes()).unwrap();
    }

    // Real config.json fields verified against the Qwen3 architecture family used by
    // https://huggingface.co/mlx-community/Qwen3-0.6B-4bit/blob/main/config.json
    #[test]
    fn parses_dense_qwen3_style_config() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{
                "model_type": "qwen3",
                "hidden_size": 1024,
                "num_hidden_layers": 28,
                "num_attention_heads": 16,
                "num_key_value_heads": 8,
                "head_dim": 128,
                "intermediate_size": 3072,
                "max_position_embeddings": 32768,
                "quantization": {"bits": 4, "group_size": 64}
            }"#,
        );
        let meta = read_mlx_metadata(dir.path()).unwrap();
        assert_eq!(meta.evidence, MlxMetaEvidence::Exact);
        assert_eq!(meta.config.num_hidden_layers, Some(28));
        assert_eq!(meta.config.quantization.clone().unwrap().bits, Some(4));

        let arch = meta.to_arch(400_000_000, 0.6, "Qwen3-0.6B-4bit");
        assert_eq!(arch.n_layers, 28);
        assert_eq!(arch.n_embd, 1024);
        assert_eq!(arch.n_kv_heads, 8);
        assert_eq!(arch.head_dim, 128);
        assert_eq!(arch.n_experts, 0);
        assert_eq!(arch.bytes_per_layer, 400_000_000 / 28);
    }

    // MoE config fields verified against the Qwen3-MoE architecture family used by
    // https://huggingface.co/mlx-community/Qwen3-30B-A3B-4bit/blob/main/config.json
    #[test]
    fn parses_moe_qwen3_style_config() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{
                "model_type": "qwen3_moe",
                "hidden_size": 2048,
                "num_hidden_layers": 48,
                "num_attention_heads": 32,
                "num_key_value_heads": 4,
                "num_experts": 128,
                "num_experts_per_tok": 8,
                "quantization": {"bits": 4, "group_size": 64}
            }"#,
        );
        let meta = read_mlx_metadata(dir.path()).unwrap();
        assert_eq!(meta.evidence, MlxMetaEvidence::Exact);

        let arch = meta.to_arch(16_000_000_000, 30.0, "Qwen3-30B-A3B-4bit");
        assert_eq!(arch.n_experts, 128);
        assert_eq!(arch.n_experts_used, 8);
        assert!(arch.is_moe());
    }

    #[test]
    fn missing_required_fields_flags_degraded() {
        let dir = tempfile::tempdir().unwrap();
        // No num_hidden_layers / num_attention_heads: cannot be authoritative.
        write_config(
            dir.path(),
            r#"{"model_type": "mystery", "hidden_size": 4096}"#,
        );
        let meta = read_mlx_metadata(dir.path()).unwrap();
        assert_eq!(meta.evidence, MlxMetaEvidence::Degraded);

        // to_arch must fall back to the name/param heuristic rather than inventing an
        // architecture from the incomplete config.
        let heuristic = ModelArch::from_name_and_params("mystery-7b", 7.0);
        let arch = meta.to_arch(4_000_000_000, 7.0, "mystery-7b");
        assert_eq!(arch.n_layers, heuristic.n_layers);
    }

    #[test]
    fn exact_weight_accounting_from_safetensors_index_total_size() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{"hidden_size": 1024, "num_hidden_layers": 10, "num_attention_heads": 8}"#,
        );
        write_index(
            dir.path(),
            r#"{
                "metadata": {"total_size": 123456789},
                "weight_map": {
                    "a": "model-00001-of-00002.safetensors",
                    "b": "model-00002-of-00002.safetensors"
                }
            }"#,
        );
        let meta = read_mlx_metadata(dir.path()).unwrap();
        assert_eq!(meta.weight_index.total_size_bytes, Some(123_456_789));
        assert_eq!(meta.weight_index.shard_files.len(), 2);
        assert_eq!(
            resolve_local_weight_bytes(dir.path(), &meta.weight_index),
            Some(123_456_789)
        );
    }

    #[test]
    fn exact_weight_accounting_sums_real_shard_files_when_no_total_size() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{"hidden_size": 1024, "num_hidden_layers": 10, "num_attention_heads": 8}"#,
        );
        write_index(dir.path(), r#"{"weight_map": {"a": "model.safetensors"}}"#);
        std::fs::write(dir.path().join("model.safetensors"), vec![0u8; 4096]).unwrap();
        let meta = read_mlx_metadata(dir.path()).unwrap();
        assert_eq!(meta.weight_index.total_size_bytes, None);
        assert_eq!(
            resolve_local_weight_bytes(dir.path(), &meta.weight_index),
            Some(4096)
        );
    }

    #[test]
    fn rejects_unsafe_shard_paths_in_index() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{"hidden_size": 1024, "num_hidden_layers": 10, "num_attention_heads": 8}"#,
        );
        write_index(
            dir.path(),
            r#"{"weight_map": {"a": "../../etc/passwd.safetensors"}}"#,
        );
        assert!(read_mlx_weight_index(dir.path()).is_err());
        // read_mlx_metadata degrades the index to empty rather than failing the whole read.
        let meta = read_mlx_metadata(dir.path()).unwrap();
        assert!(meta.weight_index.shard_files.is_empty());
    }

    #[test]
    fn draft_sidecar_sets_mtp_depth() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            dir.path(),
            r#"{
                "hidden_size": 1024,
                "num_hidden_layers": 10,
                "num_attention_heads": 8,
                "speculative_config": {"model": "draft-model", "num_hidden_layers": 2}
            }"#,
        );
        let meta = read_mlx_metadata(dir.path()).unwrap();
        let arch = meta.to_arch(1_000_000, 1.0, "test");
        assert_eq!(arch.mtp_depth, 1);
    }
}
