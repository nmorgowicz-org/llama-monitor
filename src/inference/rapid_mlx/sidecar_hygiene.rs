//! Classify Rapid-MLX snapshots and remove MTP heads from trunk directories.
//!
//! `mlx_lm` treats root-level `model*.safetensors` files as trunk shards. An
//! MTP head in that set silently shifts every trunk RMSNorm weight on load.
//! Hygiene therefore uses the config only to choose the scan policy, then uses
//! safetensors keys to identify a head. Filenames are enumeration hints, never
//! the classification signal.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const HEAD_ONLY_MODEL_TYPE: &str = "qwen3_5_mtp";
const LEGACY_HEAD_MODEL_TYPE: &str = "head";
const TRUNK_MODEL_TYPES: &[&str] = &["qwen3_5", "qwen3_5_moe", "hy_v3"];
const HEADER_LIMIT: u64 = 64 * 1024 * 1024;
const SIDECAR_FILE: &str = "mtp.safetensors";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotKind {
    HeadOnly,
    MtpEligibleTrunk,
    OtherTrunk,
}

/// Inspect a downloaded or pre-existing snapshot and relocate any MTP head
/// that could be ingested by the trunk loader.
pub(crate) fn inspect_and_repair_snapshot(snapshot: &Path) -> Result<()> {
    inspect_and_repair_snapshot_with_root(snapshot, None)
}

fn inspect_and_repair_snapshot_with_root(snapshot: &Path, test_root: Option<&Path>) -> Result<()> {
    let kind = classify_snapshot(snapshot)?;
    if kind == SnapshotKind::HeadOnly {
        bail!(
            "Rapid-MLX snapshot {} is a head-only MTP sidecar (model_type={HEAD_ONLY_MODEL_TYPE}); it is not a spawnable trunk. Import it as a companion sidecar instead",
            snapshot.display()
        );
    }
    if kind != SnapshotKind::MtpEligibleTrunk {
        return Ok(());
    }

    let candidates = root_safetensors_candidates(snapshot)?;
    let mut mtp_heads = Vec::new();
    for path in candidates {
        let keys = safetensors_tensor_names(&path)
            .with_context(|| format!("Inspecting safetensors header {}", path.display()))?;
        if keys.iter().any(|key| is_mtp_tensor_key(key)) {
            mtp_heads.push(path);
        }
    }

    if mtp_heads.len() > 1 {
        bail!(
            "Rapid-MLX snapshot {} contains multiple MTP head files; refusing ambiguous sidecar relocation",
            snapshot.display()
        );
    }
    if let Some(head) = mtp_heads.into_iter().next() {
        relocate_sidecar(snapshot, &head, test_root)?;
        remove_mtp_layer_count(snapshot)?;
    }
    Ok(())
}

fn classify_snapshot(snapshot: &Path) -> Result<SnapshotKind> {
    let config_path = snapshot.join("config.json");
    let config: Value = serde_json::from_reader(
        fs::File::open(&config_path)
            .with_context(|| format!("Reading {}", config_path.display()))?,
    )
    .with_context(|| format!("Parsing {}", config_path.display()))?;
    let model_type = config
        .get("model_type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if model_type == HEAD_ONLY_MODEL_TYPE || model_type == LEGACY_HEAD_MODEL_TYPE {
        Ok(SnapshotKind::HeadOnly)
    } else if TRUNK_MODEL_TYPES.contains(&model_type) {
        Ok(SnapshotKind::MtpEligibleTrunk)
    } else {
        Ok(SnapshotKind::OtherTrunk)
    }
}

fn root_safetensors_candidates(snapshot: &Path) -> Result<Vec<PathBuf>> {
    let mut candidates = Vec::new();
    for entry in
        fs::read_dir(snapshot).with_context(|| format!("Reading {}", snapshot.display()))?
    {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name == SIDECAR_FILE || (name.starts_with("model") && name.ends_with(".safetensors")) {
            candidates.push(path);
        }
    }
    Ok(candidates)
}

fn is_mtp_tensor_key(key: &str) -> bool {
    key.split('.').any(|part| part.starts_with("pre_fc_norm"))
}

fn safetensors_tensor_names(path: &Path) -> Result<Vec<String>> {
    let mut file = fs::File::open(path)?;
    let mut length = [0u8; 8];
    file.read_exact(&mut length)?;
    let header_len = u64::from_le_bytes(length);
    if header_len == 0 || header_len > HEADER_LIMIT {
        bail!(
            "Safetensors header length is implausible in {}",
            path.display()
        );
    }
    let mut header = vec![0u8; header_len as usize];
    file.read_exact(&mut header)?;
    let parsed: Value =
        serde_json::from_slice(&header).context("Safetensors header is invalid JSON")?;
    let object = parsed
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Safetensors header is not a JSON object"))?;
    Ok(object
        .keys()
        .filter(|key| key.as_str() != "__metadata__")
        .cloned()
        .collect())
}

fn relocate_sidecar(snapshot: &Path, source: &Path, test_root: Option<&Path>) -> Result<()> {
    let destination_dir = test_root
        .map(|root| root.join("test-sidecar"))
        .unwrap_or_else(|| {
            crate::inference::rapid_mlx::sidecar_inventory::sidecar_dir_for_trunk(snapshot)
        });
    fs::create_dir_all(&destination_dir)
        .with_context(|| format!("Creating sidecar directory {}", destination_dir.display()))?;
    let destination = destination_dir.join(SIDECAR_FILE);
    if destination.exists() {
        bail!(
            "Refusing to overwrite existing MTP sidecar {} while repairing {}",
            destination.display(),
            snapshot.display()
        );
    }
    fs::rename(source, &destination).with_context(|| {
        format!(
            "Moving MTP head {} outside trunk {}",
            source.display(),
            snapshot.display()
        )
    })?;
    Ok(())
}

fn remove_mtp_layer_count(snapshot: &Path) -> Result<()> {
    let config_path = snapshot.join("config.json");
    let mut config: Value = serde_json::from_reader(fs::File::open(&config_path)?)?;
    let Some(object) = config.as_object_mut() else {
        return Ok(());
    };
    if object.remove("mtp_num_hidden_layers").is_none() {
        return Ok(());
    }
    let temp_path = config_path.with_extension("json.hygiene-tmp");
    let bytes = serde_json::to_vec_pretty(&config)?;
    fs::write(&temp_path, bytes)?;
    fs::rename(&temp_path, &config_path).with_context(|| {
        format!(
            "Replacing repaired Rapid-MLX config {}",
            config_path.display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_safetensors(path: &Path, keys: &[&str]) {
        let header = serde_json::json!(
            keys.iter()
                .map(|key| (
                    (*key).to_string(),
                    serde_json::json!({"dtype":"F32","shape":[1],"data_offsets":[0,4]})
                ))
                .collect::<serde_json::Map<_, _>>()
        );
        let bytes = serde_json::to_vec(&header).unwrap();
        let mut output = Vec::new();
        output.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        output.extend_from_slice(&bytes);
        std::fs::write(path, output).unwrap();
    }

    fn config(path: &Path, model_type: &str, with_mtp_layer_count: bool) {
        let mut value = serde_json::json!({"model_type": model_type});
        if with_mtp_layer_count {
            value["mtp_num_hidden_layers"] = serde_json::json!(1);
        }
        std::fs::write(
            path.join("config.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn relocates_prefixed_mtp_head_and_removes_config_layer_count() {
        let root = tempdir().unwrap();
        config(root.path(), "qwen3_5", true);
        write_safetensors(
            &root.path().join("model-mtp.safetensors"),
            &["mtp.fc.weight", "mtp.pre_fc_norm_embedding.weight"],
        );
        write_safetensors(
            &root.path().join("model-00001-of-00001.safetensors"),
            &["model.layers.0.weight"],
        );
        write_safetensors(
            &root.path().join("model-vision-00001-of-00001.safetensors"),
            &["vision.encoder.weight"],
        );

        let sidecar_root = tempdir().unwrap();
        inspect_and_repair_snapshot_with_root(root.path(), Some(sidecar_root.path())).unwrap();
        assert!(!root.path().join("model-mtp.safetensors").exists());
        assert!(
            root.path()
                .join("model-vision-00001-of-00001.safetensors")
                .exists()
        );
        assert!(
            sidecar_root
                .path()
                .join("test-sidecar/mtp.safetensors")
                .exists()
        );
        assert_eq!(
            serde_json::from_reader::<_, Value>(
                fs::File::open(root.path().join("config.json")).unwrap()
            )
            .unwrap()
            .get("mtp_num_hidden_layers"),
            None
        );
    }

    #[test]
    fn config_lie_without_mtp_keys_is_left_untouched() {
        let root = tempdir().unwrap();
        config(root.path(), "qwen3_5_moe", true);
        write_safetensors(
            &root.path().join("model-00001-of-00001.safetensors"),
            &["model.layers.0.weight"],
        );
        inspect_and_repair_snapshot(root.path()).unwrap();
        assert!(
            root.path()
                .join("model-00001-of-00001.safetensors")
                .exists()
        );
        assert!(
            serde_json::from_reader::<_, Value>(
                fs::File::open(root.path().join("config.json")).unwrap()
            )
            .unwrap()
            .get("mtp_num_hidden_layers")
            .is_some()
        );
    }

    #[test]
    fn relocates_bare_mtp_namespace_from_mtp_named_file() {
        let root = tempdir().unwrap();
        config(root.path(), "qwen3_5", false);
        let source = root.path().join("mtp.safetensors");
        write_safetensors(&source, &["fc.weight", "pre_fc_norm_hidden.weight"]);
        let sidecar_root = tempdir().unwrap();

        inspect_and_repair_snapshot_with_root(root.path(), Some(sidecar_root.path())).unwrap();
        assert!(!source.exists());
        assert!(
            sidecar_root
                .path()
                .join("test-sidecar/mtp.safetensors")
                .exists()
        );
    }

    #[test]
    fn head_only_snapshot_is_not_spawnable() {
        let root = tempdir().unwrap();
        config(root.path(), HEAD_ONLY_MODEL_TYPE, false);
        write_safetensors(
            &root.path().join("model.safetensors"),
            &["pre_fc_norm_embedding.weight"],
        );
        let error = inspect_and_repair_snapshot(root.path()).unwrap_err();
        assert!(error.to_string().contains("head-only MTP sidecar"));
    }
}
