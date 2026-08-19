//! Converter-free GGUF import compatibility inspection.
//!
//! This module reads bounded metadata only. It never converts or writes model weights,
//! invokes external tools, or performs network access.

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::UNIX_EPOCH;
use tokio::sync::Semaphore;

use crate::llama::gguf_meta::{
    GgufMetadata, MAX_INSPECTION_HEADER_BYTES, read_gguf_header_inventory, read_gguf_metadata,
};

pub const REPORT_SCHEMA_VERSION: u32 = 1;
const PROFILE_VERSION: &str = "gguf-import-profiles-v1";
const INSPECTION_TIMEOUT_SECS: u64 = 15;
const MAX_SOURCE_BYTES: u64 = 4 * 1024 * 1024 * 1024 * 1024;
static INSPECTION_GATE: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(2)));

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GgufImportPreviewRequest {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImportCompatibility {
    /// Reserved for profiles promoted by R5 structural/runtime/parity gates. R1 never
    /// constructs this state.
    #[allow(dead_code)]
    Verified,
    Experimental,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceIdentity {
    pub canonical_path: PathBuf,
    pub size_bytes: u64,
    pub modified_unix_ms: Option<u128>,
    pub identity_kind: &'static str,
    pub metadata_sha256: String,
    pub metadata_bytes_hashed: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuantInventoryEntry {
    pub quant_type: String,
    pub tensor_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenizerObservations {
    pub model: bool,
    pub tokens: bool,
    pub merges: bool,
    pub scores: bool,
    pub bos_token_id: bool,
    pub eos_token_id: bool,
    pub chat_template: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigObservations {
    pub block_count: bool,
    pub context_length: bool,
    pub embedding_length: bool,
    pub attention_heads: bool,
    pub kv_heads: bool,
    pub feed_forward_length: bool,
    pub expert_topology: bool,
    pub hybrid_attention: bool,
    pub state_space: bool,
    pub sliding_window: bool,
    pub mtp_depth: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuxiliaryAssetObservations {
    pub is_projector_gguf: bool,
    pub requires_multimodal_assets: bool,
    pub requires_mtp_runtime: bool,
    pub nearby_files_are_not_trusted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceWarning {
    pub tier: &'static str,
    pub estimated_fp16_staging_bytes: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GgufImportCompatibilityReport {
    pub schema_version: u32,
    pub profile_version: &'static str,
    pub source: SourceIdentity,
    pub gguf_version: u32,
    pub architecture: String,
    pub architecture_kind: String,
    pub tensor_count: u64,
    pub quant_inventory: Vec<QuantInventoryEntry>,
    pub tokenizer: TokenizerObservations,
    pub config: ConfigObservations,
    pub auxiliary_assets: AuxiliaryAssetObservations,
    pub compatibility: ImportCompatibility,
    pub missing_profile_fields: Vec<String>,
    pub missing_assets: Vec<String>,
    pub warnings: Vec<String>,
    pub unsupported_reasons: Vec<String>,
    pub resource: ResourceWarning,
    pub remediation: Vec<String>,
}

/// Inspect a library-relative GGUF path. Work is capped at two blocking workers and a
/// fixed timeout so malformed/local files cannot occupy Tokio.
pub async fn inspect_async(
    requested_path: PathBuf,
    models_dir: PathBuf,
) -> Result<GgufImportCompatibilityReport> {
    let permit = tokio::time::timeout(
        std::time::Duration::from_secs(INSPECTION_TIMEOUT_SECS),
        INSPECTION_GATE.clone().acquire_owned(),
    )
    .await
    .context("Timed out waiting for a GGUF inspection worker")?
    .map_err(|_| anyhow!("GGUF inspection worker pool is unavailable"))?;

    tokio::time::timeout(
        std::time::Duration::from_secs(INSPECTION_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            inspect(&requested_path, &models_dir)
        }),
    )
    .await
    .context("GGUF compatibility inspection timed out")?
    .context("GGUF compatibility inspection worker failed")?
}

pub fn inspect(requested_path: &Path, models_dir: &Path) -> Result<GgufImportCompatibilityReport> {
    let path = validate_source_path(requested_path, models_dir)?;
    let stat = fs::metadata(&path).with_context(|| format!("Cannot stat '{}'", path.display()))?;
    if stat.len() == 0 || stat.len() > MAX_SOURCE_BYTES {
        bail!("GGUF source size is outside the supported inspection bounds");
    }

    // Strict inventory runs first so oversized/incomplete headers fail before the broader
    // architecture reader is allowed to parse the file.
    let header = read_gguf_header_inventory(&path, MAX_INSPECTION_HEADER_BYTES)
        .map_err(|error| anyhow!(error))?;
    let metadata = read_gguf_metadata(&path).map_err(|error| anyhow!(error))?;
    let metadata_sha256 = hash_prefix(&path, header.header_bytes)?;
    ensure_source_unchanged(&path, &stat)?;
    let keys = &header.metadata_keys;
    let architecture = metadata
        .architecture
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            anyhow!("GGUF has no general.architecture; filename fallback is forbidden")
        })?;

    let tokenizer = tokenizer_observations(keys);
    let config = config_observations(&metadata);
    let auxiliary_assets = AuxiliaryAssetObservations {
        is_projector_gguf: matches!(architecture.as_str(), "clip" | "mmproj"),
        requires_multimodal_assets: matches!(architecture.as_str(), "clip" | "mmproj")
            || keys.iter().any(|key| key.starts_with("clip.")),
        requires_mtp_runtime: metadata.mtp_depth.is_some_and(|depth| depth > 0),
        nearby_files_are_not_trusted: true,
    };
    let quant_inventory = header
        .quant_types
        .iter()
        .map(|(quant_type, tensor_count)| QuantInventoryEntry {
            quant_type: quant_type.clone(),
            tensor_count: *tensor_count,
        })
        .collect::<Vec<_>>();

    let mut policy = evaluate_policy(
        &architecture,
        &metadata,
        &header.quant_types,
        &tokenizer,
        &auxiliary_assets,
    );
    policy.missing_profile_fields.sort();
    policy.missing_profile_fields.dedup();
    policy.missing_assets.sort();
    policy.missing_assets.dedup();
    policy.warnings.sort();
    policy.warnings.dedup();
    policy.unsupported_reasons.sort();
    policy.unsupported_reasons.dedup();
    policy.remediation.sort();
    policy.remediation.dedup();

    let estimated_fp16 = metadata.param_count.and_then(|count| count.checked_mul(2));
    let resource = resource_warning(estimated_fp16);
    let modified_unix_ms = stat
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis());

    Ok(GgufImportCompatibilityReport {
        schema_version: REPORT_SCHEMA_VERSION,
        profile_version: PROFILE_VERSION,
        source: SourceIdentity {
            canonical_path: path,
            size_bytes: stat.len(),
            modified_unix_ms,
            identity_kind: "bounded_gguf_header_sha256",
            metadata_sha256,
            metadata_bytes_hashed: header.header_bytes,
        },
        gguf_version: header.version,
        architecture_kind: metadata.architecture_kind(),
        architecture,
        tensor_count: header.tensor_count,
        quant_inventory,
        tokenizer,
        config,
        auxiliary_assets,
        compatibility: policy.compatibility,
        missing_profile_fields: policy.missing_profile_fields,
        missing_assets: policy.missing_assets,
        warnings: policy.warnings,
        unsupported_reasons: policy.unsupported_reasons,
        resource,
        remediation: policy.remediation,
    })
}

fn validate_source_path(requested_path: &Path, models_dir: &Path) -> Result<PathBuf> {
    if requested_path.as_os_str().is_empty() {
        bail!("A GGUF path is required");
    }
    let raw_path = requested_path.as_os_str().to_string_lossy();
    if requested_path.is_absolute() || raw_path.starts_with('/') || raw_path.starts_with('\\') {
        bail!("GGUF path must be relative to the configured models_dir");
    }
    if requested_path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        bail!("GGUF path traversal is not allowed");
    }
    let root = models_dir
        .canonicalize()
        .with_context(|| format!("Cannot resolve models_dir '{}'", models_dir.display()))?;
    let relative_input = requested_path.to_path_buf();
    let candidate = root.join(&relative_input);
    let mut lexical = root.clone();
    for component in relative_input.components() {
        if matches!(component, Component::CurDir) {
            continue;
        }
        lexical.push(component);
        let metadata = fs::symlink_metadata(&lexical)
            .with_context(|| format!("Cannot inspect path component '{}'", lexical.display()))?;
        if metadata.file_type().is_symlink() {
            bail!("Symlinked GGUF paths are not allowed");
        }
    }
    let canonical = candidate
        .canonicalize()
        .with_context(|| format!("Cannot resolve GGUF '{}'", candidate.display()))?;
    let relative = canonical
        .strip_prefix(&root)
        .map_err(|_| anyhow!("GGUF must be inside the configured models_dir"))?;
    if relative.as_os_str().is_empty() {
        bail!("GGUF path must name a file");
    }
    if !canonical.is_file() {
        bail!("GGUF path must name a regular file");
    }
    if canonical
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case("gguf"))
    {
        bail!("Only .gguf model files can be inspected");
    }
    Ok(canonical)
}

fn hash_prefix(path: &Path, bytes: u64) -> Result<String> {
    let mut file = File::open(path)?;
    let mut remaining = bytes;
    let mut buffer = [0u8; 64 * 1024];
    let mut hash = Sha256::new();
    while remaining > 0 {
        let length = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let count = file.read(&mut buffer[..length])?;
        if count == 0 {
            bail!("GGUF ended while hashing its bounded header identity");
        }
        hash.update(&buffer[..count]);
        remaining -= count as u64;
    }
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn ensure_source_unchanged(path: &Path, before: &fs::Metadata) -> Result<()> {
    let link = fs::symlink_metadata(path)?;
    if link.file_type().is_symlink() || !link.is_file() {
        bail!("GGUF source changed during inspection");
    }
    if path.canonicalize()? != path {
        bail!("GGUF source path changed during inspection");
    }
    let after = fs::metadata(path)?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        bail!("GGUF source changed during inspection");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != after.dev() || before.ino() != after.ino() {
            bail!("GGUF source changed during inspection");
        }
    }
    Ok(())
}

fn has_key(keys: &[String], suffix: &str) -> bool {
    keys.iter().any(|key| key == suffix)
}

fn tokenizer_observations(keys: &[String]) -> TokenizerObservations {
    TokenizerObservations {
        model: has_key(keys, "tokenizer.ggml.model"),
        tokens: has_key(keys, "tokenizer.ggml.tokens"),
        merges: has_key(keys, "tokenizer.ggml.merges"),
        scores: has_key(keys, "tokenizer.ggml.scores"),
        bos_token_id: has_key(keys, "tokenizer.ggml.bos_token_id"),
        eos_token_id: has_key(keys, "tokenizer.ggml.eos_token_id"),
        chat_template: has_key(keys, "tokenizer.chat_template"),
    }
}

fn config_observations(metadata: &GgufMetadata) -> ConfigObservations {
    ConfigObservations {
        block_count: metadata.block_count.is_some(),
        context_length: metadata.context_length.is_some(),
        embedding_length: metadata.embedding_length.is_some(),
        attention_heads: metadata.head_count.is_some(),
        kv_heads: metadata.head_count_kv.is_some()
            || (metadata.global_kv_heads.is_some() && metadata.local_kv_heads.is_some()),
        feed_forward_length: metadata.feed_forward_length.is_some(),
        expert_topology: metadata.expert_count.is_some() && metadata.expert_used_count.is_some(),
        hybrid_attention: metadata.full_attention_interval.is_some()
            || metadata.n_global_attn_layers.is_some(),
        state_space: metadata.ssm_inner_size.is_some()
            && metadata.ssm_state_size.is_some()
            && metadata.ssm_conv_kernel.is_some(),
        sliding_window: metadata.sliding_window.is_some(),
        mtp_depth: metadata.mtp_depth,
    }
}

struct PolicyResult {
    compatibility: ImportCompatibility,
    missing_profile_fields: Vec<String>,
    missing_assets: Vec<String>,
    warnings: Vec<String>,
    unsupported_reasons: Vec<String>,
    remediation: Vec<String>,
}

fn evaluate_policy(
    architecture: &str,
    metadata: &GgufMetadata,
    quant_types: &BTreeMap<String, u64>,
    tokenizer: &TokenizerObservations,
    assets: &AuxiliaryAssetObservations,
) -> PolicyResult {
    let mut result = PolicyResult {
        compatibility: ImportCompatibility::Experimental,
        missing_profile_fields: Vec::new(),
        missing_assets: Vec::new(),
        warnings: Vec::new(),
        unsupported_reasons: Vec::new(),
        remediation: vec!["Continue using the original GGUF with llama.cpp.".into()],
    };

    let difficult_qwen = matches!(
        architecture,
        "qwen35" | "qwen35moe" | "qwen3_5" | "qwen3_6" | "qwen3next"
    );
    if difficult_qwen {
        result.missing_profile_fields.extend(
            [
                "exact hybrid full-attention layer pattern",
                "complete SSM/DeltaNet config mapping",
                "MoE router/expert/shared-expert tensor closure",
                "target MLX model-class contract",
            ]
            .into_iter()
            .map(str::to_string),
        );
        result
            .unsupported_reasons
            .push("Qwen3.5/Qwen3.6 hybrid, MoE, and tensor mappings are not yet verified.".into());
    } else if architecture == "gemma4" {
        result.missing_profile_fields.extend(
            [
                "exact sliding-window layer pattern",
                "global/local shared-KV layout",
                "per-layer input embedding dimensions",
                "final-logit softcapping",
                "target MLX model-class contract",
            ]
            .into_iter()
            .map(str::to_string),
        );
        result
            .unsupported_reasons
            .push("Gemma 4 alternating attention and tensor mappings are not yet verified.".into());
    } else if !matches!(architecture, "llama" | "qwen2" | "mistral") {
        result
            .missing_profile_fields
            .push(format!("explicit {architecture} architecture profile"));
        result.unsupported_reasons.push(format!(
            "Architecture '{architecture}' has no explicit GGUF import profile; no Llama fallback is allowed."
        ));
    }

    for (field, present) in [
        ("block_count", metadata.block_count.is_some()),
        ("context_length", metadata.context_length.is_some()),
        ("embedding_length", metadata.embedding_length.is_some()),
        ("attention.head_count", metadata.head_count.is_some()),
    ] {
        if !present {
            result.missing_profile_fields.push(field.into());
        }
    }
    if !tokenizer.model {
        result
            .missing_assets
            .push("tokenizer model identity".into());
    }
    if !tokenizer.tokens {
        result.missing_assets.push("tokenizer vocabulary".into());
    }
    if !tokenizer.bos_token_id || !tokenizer.eos_token_id {
        result
            .missing_assets
            .push("tokenizer BOS/EOS special-token IDs".into());
    }
    if !tokenizer.chat_template {
        result
            .warnings
            .push("No embedded chat template was observed; an authoritative template is required for chat parity.".into());
    }

    if assets.requires_mtp_runtime {
        result
            .missing_profile_fields
            .push("MTP/NextN head tensor and execution mapping".into());
        result.unsupported_reasons.push(
            "MTP/NextN output is unsupported until both conversion and Rapid-MLX runtime semantics are verified.".into(),
        );
    }
    if assets.requires_multimodal_assets {
        result.missing_assets.extend(
            [
                "paired language model",
                "vision tower/projector contract",
                "processor config and image-token mapping",
            ]
            .into_iter()
            .map(str::to_string),
        );
        result.unsupported_reasons.push(
            "Multimodal/projector GGUF import is unsupported without a complete verified asset set.".into(),
        );
    }

    for quant in quant_types.keys() {
        if quant.starts_with("UNKNOWN_") {
            result.unsupported_reasons.push(format!(
                "Unknown tensor quantization type {quant} is unsupported; tensor types are never guessed."
            ));
        } else if quant.starts_with("IQ1")
            || quant.starts_with("IQ2")
            || quant.starts_with("IQ3")
            || quant.starts_with("Q2")
            || quant.starts_with("Q3")
            || quant.starts_with("TQ")
        {
            result.unsupported_reasons.push(format!(
                "Low-bit tensor quantization {quant} is disabled until model-specific fidelity proof exists."
            ));
        } else if quant.starts_with("IQ4") {
            result.warnings.push(format!(
                "{quant} requires a separate importance-aware profile; dequantization cannot preserve its original importance recipe."
            ));
            result
                .missing_profile_fields
                .push(format!("{quant} dequantization profile"));
        } else if quant.starts_with("Q4") || quant.starts_with("Q5") {
            result.warnings.push(format!(
                "{quant} is already quantized; a later MLX quantization adds compounded approximation."
            ));
        }
    }

    if !result.unsupported_reasons.is_empty()
        || !result.missing_profile_fields.is_empty()
        || !result.missing_assets.is_empty()
    {
        result.compatibility = ImportCompatibility::Unsupported;
        result.remediation.push(
            "Provide authoritative BF16/F16 safetensors for the supported official MLX conversion path when available.".into(),
        );
        result.remediation.push(
            "Wait for a versioned architecture profile with complete tensor, config, tokenizer, runtime, and parity evidence.".into(),
        );
    } else {
        result.remediation.push(
            "This source is only an experimental candidate; R1 does not authorize weight conversion or launch.".into(),
        );
    }
    result
}

fn resource_warning(estimated_fp16: Option<u64>) -> ResourceWarning {
    let (tier, message) = match estimated_fp16 {
        Some(bytes) if bytes >= 128 * 1024 * 1024 * 1024 => (
            "extreme",
            "Recovered FP16 staging alone may exceed 128 GiB; final output, temporary shards, and safety margin are additional.",
        ),
        Some(bytes) if bytes >= 32 * 1024 * 1024 * 1024 => (
            "high",
            "Recovered FP16 staging may require tens of GiB; final output, temporary shards, and safety margin are additional.",
        ),
        Some(_) => (
            "moderate",
            "FP16 staging is only the baseline; final output, temporary shards, and filesystem safety margin are additional.",
        ),
        None => (
            "unknown",
            "Parameter count is unavailable, so disk and memory requirements cannot be estimated safely.",
        ),
    };
    ResourceWarning {
        tier,
        estimated_fp16_staging_bytes: estimated_fp16,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    enum Kv<'a> {
        U32(u32),
        U64(u64),
        String(&'a str),
        Strings(&'a [&'a str]),
    }

    fn write_string(out: &mut Vec<u8>, value: &str) {
        out.extend_from_slice(&(value.len() as u64).to_le_bytes());
        out.extend_from_slice(value.as_bytes());
    }

    fn fixture(path: &Path, kv: &[(&str, Kv<'_>)], quant_types: &[u32]) {
        let mut out = Vec::new();
        out.extend_from_slice(b"GGUF");
        out.extend_from_slice(&3u32.to_le_bytes());
        out.extend_from_slice(&(quant_types.len() as u64).to_le_bytes());
        out.extend_from_slice(&(kv.len() as u64).to_le_bytes());
        for (key, value) in kv {
            write_string(&mut out, key);
            match value {
                Kv::U32(value) => {
                    out.extend_from_slice(&4u32.to_le_bytes());
                    out.extend_from_slice(&value.to_le_bytes());
                }
                Kv::U64(value) => {
                    out.extend_from_slice(&10u32.to_le_bytes());
                    out.extend_from_slice(&value.to_le_bytes());
                }
                Kv::String(value) => {
                    out.extend_from_slice(&8u32.to_le_bytes());
                    write_string(&mut out, value);
                }
                Kv::Strings(values) => {
                    out.extend_from_slice(&9u32.to_le_bytes());
                    out.extend_from_slice(&8u32.to_le_bytes());
                    out.extend_from_slice(&(values.len() as u64).to_le_bytes());
                    for value in *values {
                        write_string(&mut out, value);
                    }
                }
            }
        }
        for (index, quant_type) in quant_types.iter().enumerate() {
            write_string(&mut out, &format!("blk.0.tensor_{index}.weight"));
            out.extend_from_slice(&1u32.to_le_bytes());
            out.extend_from_slice(&32u64.to_le_bytes());
            out.extend_from_slice(&quant_type.to_le_bytes());
            out.extend_from_slice(&(index as u64 * 32).to_le_bytes());
        }
        let padding = (32 - out.len() % 32) % 32;
        out.resize(out.len() + padding + quant_types.len() * 32, 0);
        let mut file = File::create(path).unwrap();
        file.write_all(&out).unwrap();
    }

    fn base_kv<'a>(architecture: &'a str) -> Vec<(&'a str, Kv<'a>)> {
        vec![
            ("general.architecture", Kv::String(architecture)),
            ("general.parameter_count", Kv::U64(7_000_000_000)),
            ("llama.block_count", Kv::U32(32)),
            ("llama.context_length", Kv::U32(8192)),
            ("llama.embedding_length", Kv::U32(4096)),
            ("llama.attention.head_count", Kv::U32(32)),
            ("tokenizer.ggml.model", Kv::String("llama")),
            ("tokenizer.ggml.tokens", Kv::Strings(&["a", "b"])),
            ("tokenizer.ggml.bos_token_id", Kv::U32(1)),
            ("tokenizer.ggml.eos_token_id", Kv::U32(2)),
        ]
    }

    #[test]
    fn known_llama_is_experimental_and_reports_quant_inventory() {
        let temp = tempfile::tempdir().unwrap();
        let models = temp.path().join("models");
        fs::create_dir_all(models.join("gguf")).unwrap();
        let path = models.join("gguf/model.gguf");
        fixture(&path, &base_kv("llama"), &[14, 14, 8]);
        let report = inspect(Path::new("gguf/model.gguf"), &models).unwrap();
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.compatibility, ImportCompatibility::Experimental);
        assert_eq!(report.tensor_count, 3);
        assert!(
            report
                .quant_inventory
                .iter()
                .any(|entry| { entry.quant_type == "Q6_K" && entry.tensor_count == 2 })
        );
        assert_eq!(report.source.metadata_sha256.len(), 64);
        assert!(report.source.metadata_bytes_hashed < report.source.size_bytes);
    }

    #[test]
    fn qwen36_hybrid_moe_fails_with_exact_profile_gaps() {
        let temp = tempfile::tempdir().unwrap();
        let models = temp.path().join("models");
        fs::create_dir_all(&models).unwrap();
        let path = models.join("qwen.gguf");
        let mut kv = base_kv("qwen35moe");
        kv.extend([
            ("qwen35moe.block_count", Kv::U32(40)),
            ("qwen35moe.full_attention_interval", Kv::U32(4)),
            ("qwen35moe.expert_count", Kv::U32(256)),
            ("qwen35moe.expert_used_count", Kv::U32(8)),
            ("qwen35moe.ssm.inner_size", Kv::U32(4096)),
            ("qwen35moe.nextn_predict_layers", Kv::U32(1)),
            ("clip.projector_type", Kv::String("qwen3_6")),
        ]);
        fixture(&path, &kv, &[14]);
        let report = inspect(Path::new("qwen.gguf"), &models).unwrap();
        assert_eq!(report.compatibility, ImportCompatibility::Unsupported);
        assert!(
            report
                .missing_profile_fields
                .contains(&"complete SSM/DeltaNet config mapping".to_string())
        );
        assert!(
            report
                .missing_profile_fields
                .contains(&"MoE router/expert/shared-expert tensor closure".to_string())
        );
        assert!(
            report
                .missing_profile_fields
                .contains(&"MTP/NextN head tensor and execution mapping".to_string())
        );
        assert!(
            report
                .missing_assets
                .contains(&"processor config and image-token mapping".to_string())
        );
    }

    #[test]
    fn gemma4_mtp_projector_policy_fails_early() {
        let temp = tempfile::tempdir().unwrap();
        let models = temp.path().join("models");
        fs::create_dir_all(&models).unwrap();
        let path = models.join("gemma.gguf");
        let mut kv = base_kv("gemma4");
        kv.extend([
            ("gemma4.block_count", Kv::U32(60)),
            ("gemma4.nextn_predict_layers", Kv::U32(1)),
            ("clip.projector_type", Kv::String("gemma4")),
        ]);
        fixture(&path, &kv, &[13]);
        let report = inspect(Path::new("gemma.gguf"), &models).unwrap();
        assert_eq!(report.compatibility, ImportCompatibility::Unsupported);
        assert!(
            report
                .missing_profile_fields
                .contains(&"exact sliding-window layer pattern".to_string())
        );
        assert!(
            report
                .missing_profile_fields
                .contains(&"MTP/NextN head tensor and execution mapping".to_string())
        );
        assert!(
            report
                .missing_assets
                .contains(&"processor config and image-token mapping".to_string())
        );
    }

    #[test]
    fn unknown_architecture_and_low_or_unknown_quant_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        let models = temp.path().join("models");
        fs::create_dir_all(&models).unwrap();
        let path = models.join("mystery.gguf");
        fixture(&path, &base_kv("mystery"), &[10, 999]);
        let report = inspect(Path::new("mystery.gguf"), &models).unwrap();
        assert_eq!(report.compatibility, ImportCompatibility::Unsupported);
        assert!(
            report
                .unsupported_reasons
                .iter()
                .any(|reason| reason.contains("no explicit"))
        );
        assert!(
            report
                .unsupported_reasons
                .iter()
                .any(|reason| reason.contains("Q2_K"))
        );
        assert!(
            report
                .unsupported_reasons
                .iter()
                .any(|reason| reason.contains("UNKNOWN_999"))
        );
    }

    #[test]
    fn rejects_traversal_outside_non_gguf_and_symlink() {
        let temp = tempfile::tempdir().unwrap();
        let models = temp.path().join("models");
        fs::create_dir_all(&models).unwrap();
        fs::write(models.join("note.txt"), b"not a model").unwrap();
        assert!(validate_source_path(Path::new("../outside.gguf"), &models).is_err());
        assert!(validate_source_path(&models.join("inside.gguf"), &models).is_err());
        assert!(validate_source_path(Path::new("\\server\\share.gguf"), &models).is_err());
        assert!(validate_source_path(Path::new("note.txt"), &models).is_err());
        let outside = temp.path().join("outside.gguf");
        fs::write(&outside, b"GGUF").unwrap();
        assert!(validate_source_path(&outside, &models).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, models.join("linked.gguf")).unwrap();
            assert!(validate_source_path(Path::new("linked.gguf"), &models).is_err());
        }
    }

    #[test]
    fn rejects_header_over_configured_bound() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large.gguf");
        fixture(&path, &base_kv("llama"), &[14]);
        let error = read_gguf_header_inventory(&path, 32).unwrap_err();
        assert!(error.contains("inspection limit"));
    }

    #[test]
    fn rejects_header_without_declared_tensor_data() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("truncated.gguf");
        fixture(&path, &base_kv("llama"), &[14]);
        let size = fs::metadata(&path).unwrap().len();
        let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(size - 32).unwrap();
        let error = read_gguf_header_inventory(&path, MAX_INSPECTION_HEADER_BYTES).unwrap_err();
        assert!(error.contains("no tensor data"));
    }

    #[test]
    fn local_difficult_models_fail_before_conversion_when_available() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let models = home.join(".config/llama-monitor/models");
        let cases = [
            (
                "gguf/Qwen3.6-35B-A3B-uncensored-heretic-Q6_K.gguf",
                "complete SSM/DeltaNet config mapping",
            ),
            (
                "gguf/gemma-4-31B-it-UD-Q4_K_XL.gguf",
                "exact sliding-window layer pattern",
            ),
        ];
        for (relative, expected_gap) in cases {
            let path = models.join(relative);
            if !path.is_file() {
                continue;
            }
            let report = inspect(Path::new(relative), &models)
                .unwrap_or_else(|error| panic!("{relative}: {error:#}"));
            assert_eq!(report.compatibility, ImportCompatibility::Unsupported);
            assert!(
                report
                    .missing_profile_fields
                    .iter()
                    .any(|field| field == expected_gap),
                "{relative}: expected exact gap '{expected_gap}', got {:?}",
                report.missing_profile_fields
            );
            assert!(report.source.metadata_bytes_hashed <= MAX_INSPECTION_HEADER_BYTES);
        }
    }
}
