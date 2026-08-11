//! Pure, receipt-ready model-root relocation planning.
//!
//! The application-home migration deliberately retains model trees. This
//! module owns the later explicit “keep here” / “move into Foundry” decision
//! without guessing from filenames or touching external Hugging Face roots.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRootChoice {
    KeepLegacy,
    MoveIntoFoundry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelResourceClass {
    ManagedModel,
    ManagedRuntime,
    PartialDownload,
    HuggingFaceCache,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRelocationEntry {
    pub relative_path: PathBuf,
    pub class: ModelResourceClass,
    pub bytes: u64,
    pub is_directory: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRelocationPlan {
    pub schema_version: u32,
    pub plan_id: String,
    pub choice: ModelRootChoice,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub entries: Vec<ModelRelocationEntry>,
    pub required_copy_bytes: u64,
    pub retained_external_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRelocationReceipt {
    pub schema_version: u32,
    pub plan_id: String,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub copied_entries: Vec<PathBuf>,
    pub retained_source: bool,
    #[serde(default)]
    pub retained_external_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelRelocationJournal {
    schema_version: u32,
    plan_id: String,
    completed_entries: Vec<PathBuf>,
}

pub fn plan_model_root_relocation(
    source: &Path,
    destination: &Path,
    choice: ModelRootChoice,
) -> Result<ModelRelocationPlan> {
    let metadata = fs::symlink_metadata(source).context("model root is not readable")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        bail!("model root must be a real directory");
    }
    if source == destination {
        bail!("model relocation source and destination are identical");
    }
    if destination.exists()
        && (!destination.is_dir() || fs::read_dir(destination)?.next().is_some())
    {
        bail!("model relocation destination is not empty");
    }

    let mut entries = Vec::new();
    collect(source, source, &mut entries)?;
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let required_copy_bytes = if choice == ModelRootChoice::MoveIntoFoundry {
        entries
            .iter()
            .filter(|entry| !entry.is_directory)
            .map(|entry| entry.bytes)
            .sum()
    } else {
        0
    };
    let retained_external_roots = if choice == ModelRootChoice::KeepLegacy {
        vec![source.to_path_buf()]
    } else {
        Vec::new()
    };
    let plan_id = hex_digest(&Sha256::digest(serde_json::to_vec(&(
        1u32,
        choice,
        source,
        destination,
        &entries,
        &retained_external_roots,
    ))?));
    Ok(ModelRelocationPlan {
        schema_version: 1,
        plan_id,
        choice,
        source: source.to_path_buf(),
        destination: destination.to_path_buf(),
        entries,
        required_copy_bytes,
        retained_external_roots,
    })
}

pub fn relocation_receipt_path(plan: &ModelRelocationPlan) -> PathBuf {
    plan.destination
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            ".local-llm-foundry-model-relocation-{}.json",
            plan.plan_id
        ))
}

fn relocation_journal_path(plan: &ModelRelocationPlan) -> PathBuf {
    plan.destination
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            ".local-llm-foundry-model-relocation-{}.journal",
            plan.plan_id
        ))
}

/// Copy-first model relocation. The source is retained until a later,
/// receipt-scoped cleanup action; rerunning a completed plan returns its receipt.
pub fn execute_model_root_relocation(plan: &ModelRelocationPlan) -> Result<ModelRelocationReceipt> {
    if plan.choice == ModelRootChoice::KeepLegacy {
        return Ok(ModelRelocationReceipt {
            schema_version: 1,
            plan_id: plan.plan_id.clone(),
            source: plan.source.clone(),
            destination: plan.destination.clone(),
            copied_entries: Vec::new(),
            retained_source: true,
            retained_external_roots: plan.retained_external_roots.clone(),
        });
    }
    if let Ok(file) = fs::File::open(relocation_receipt_path(plan))
        && let Ok(receipt) = serde_json::from_reader(file)
    {
        return Ok(receipt);
    }
    let journal_path = relocation_journal_path(plan);
    let current = if journal_path.is_file() {
        let mut current = plan.clone();
        let mut entries = Vec::new();
        collect(&plan.source, &plan.source, &mut entries)?;
        entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        current.entries = entries;
        current
    } else {
        let current = plan_model_root_relocation(&plan.source, &plan.destination, plan.choice)?;
        if current.plan_id != plan.plan_id {
            bail!("model relocation preview is stale");
        }
        current
    };
    let mut journal = if journal_path.is_file() {
        let journal: ModelRelocationJournal =
            serde_json::from_reader(fs::File::open(&journal_path)?)?;
        if journal.plan_id != plan.plan_id {
            bail!("model relocation journal plan does not match preview");
        }
        journal
    } else {
        ModelRelocationJournal {
            schema_version: 1,
            plan_id: plan.plan_id.clone(),
            completed_entries: Vec::new(),
        }
    };
    fs::create_dir_all(&plan.destination)?;
    let mut copied_entries = Vec::new();
    for entry in &current.entries {
        let source = plan.source.join(&entry.relative_path);
        let destination = plan.destination.join(&entry.relative_path);
        if entry.is_directory {
            fs::create_dir_all(&destination)?;
            continue;
        }
        if journal
            .completed_entries
            .iter()
            .any(|path| path == &entry.relative_path)
        {
            copied_entries.push(entry.relative_path.clone());
            continue;
        }
        if destination.exists() {
            if fs::metadata(&destination)?.len() != entry.bytes {
                bail!(
                    "model relocation refuses to overwrite {}",
                    destination.display()
                );
            }
            journal.completed_entries.push(entry.relative_path.clone());
            copied_entries.push(entry.relative_path.clone());
            fs::write(&journal_path, serde_json::to_vec_pretty(&journal)?)?;
            continue;
        }
        fs::create_dir_all(
            destination
                .parent()
                .context("model destination has no parent")?,
        )?;
        let temporary = destination.with_extension("local-llm-foundry-part");
        fs::copy(&source, &temporary)?;
        if fs::metadata(&temporary)?.len() != entry.bytes {
            let _ = fs::remove_file(&temporary);
            bail!(
                "model relocation verification failed for {}",
                entry.relative_path.display()
            );
        }
        fs::rename(&temporary, &destination)?;
        copied_entries.push(entry.relative_path.clone());
        journal.completed_entries.push(entry.relative_path.clone());
        fs::write(&journal_path, serde_json::to_vec_pretty(&journal)?)?;
    }
    let receipt = ModelRelocationReceipt {
        schema_version: 1,
        plan_id: plan.plan_id.clone(),
        source: plan.source.clone(),
        destination: plan.destination.clone(),
        copied_entries,
        retained_source: true,
        retained_external_roots: plan.retained_external_roots.clone(),
    };
    let path = relocation_receipt_path(plan);
    let temporary = path.with_extension("json.local-llm-foundry-part");
    fs::write(&temporary, serde_json::to_vec_pretty(&receipt)?)?;
    fs::rename(temporary, path)?;
    let _ = fs::remove_file(journal_path);
    Ok(receipt)
}

fn collect(root: &Path, current: &Path, entries: &mut Vec<ModelRelocationEntry>) -> Result<()> {
    for item in fs::read_dir(current)? {
        let item = item?;
        let path = item.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            bail!("model relocation refuses symlink: {}", path.display());
        }
        let relative_path = path
            .strip_prefix(root)
            .context("model relocation path escaped source")?
            .to_path_buf();
        let is_directory = metadata.is_dir();
        let class = classify(&relative_path);
        entries.push(ModelRelocationEntry {
            relative_path,
            class,
            bytes: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
            is_directory,
        });
        if is_directory {
            collect(root, &path, entries)?;
        }
    }
    Ok(())
}

fn classify(path: &Path) -> ModelResourceClass {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    if lower.contains("huggingface") || lower.contains("cache/hub") {
        ModelResourceClass::HuggingFaceCache
    } else if lower.contains(".staging") || lower.ends_with(".part") {
        ModelResourceClass::PartialDownload
    } else if lower.starts_with("rapid-mlx") || lower.starts_with("runtimes") {
        ModelResourceClass::ManagedRuntime
    } else if lower.starts_with("gguf")
        || lower.starts_with("mlx")
        || lower.starts_with("transformers")
    {
        ModelResourceClass::ManagedModel
    } else {
        ModelResourceClass::Unknown
    }
}

fn hex_digest(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_choice_is_non_mutating_and_retains_explicit_external_root() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("legacy-models");
        let destination = root.path().join("foundry-models");
        fs::create_dir_all(source.join("gguf/.staging")).unwrap();
        fs::write(source.join("gguf/model.gguf"), b"model").unwrap();
        let plan =
            plan_model_root_relocation(&source, &destination, ModelRootChoice::KeepLegacy).unwrap();
        assert_eq!(plan.required_copy_bytes, 0);
        assert_eq!(plan.retained_external_roots, vec![source.clone()]);
        assert!(!destination.exists());
    }

    #[test]
    fn move_choice_is_deterministic_and_classifies_partial_cache_runtime() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("legacy-models");
        let destination = root.path().join("foundry-models");
        fs::create_dir_all(source.join("mlx/native")).unwrap();
        fs::create_dir_all(source.join("cache/huggingface/hub")).unwrap();
        fs::create_dir_all(source.join(".staging")).unwrap();
        fs::create_dir_all(source.join("rapid-mlx")).unwrap();
        fs::write(source.join("mlx/native/model.safetensors"), b"model").unwrap();
        fs::write(source.join(".staging/model.part"), b"part").unwrap();
        let first =
            plan_model_root_relocation(&source, &destination, ModelRootChoice::MoveIntoFoundry)
                .unwrap();
        let second =
            plan_model_root_relocation(&source, &destination, ModelRootChoice::MoveIntoFoundry)
                .unwrap();
        assert_eq!(first.plan_id, second.plan_id);
        assert!(first.required_copy_bytes > 0);
        assert!(
            first
                .entries
                .iter()
                .any(|entry| entry.class == ModelResourceClass::PartialDownload)
        );
        assert!(
            first
                .entries
                .iter()
                .any(|entry| entry.class == ModelResourceClass::HuggingFaceCache)
        );
        assert!(
            first
                .entries
                .iter()
                .any(|entry| entry.class == ModelResourceClass::ManagedRuntime)
        );
    }

    #[test]
    fn move_execution_is_copy_first_verified_and_idempotent() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("legacy-models");
        let destination = root.path().join("foundry-models");
        fs::create_dir_all(source.join("gguf")).unwrap();
        fs::write(source.join("gguf/model.gguf"), b"model").unwrap();
        let plan =
            plan_model_root_relocation(&source, &destination, ModelRootChoice::MoveIntoFoundry)
                .unwrap();
        let receipt = execute_model_root_relocation(&plan).unwrap();
        assert!(receipt.retained_source);
        assert_eq!(
            fs::read(destination.join("gguf/model.gguf")).unwrap(),
            b"model"
        );
        assert!(source.join("gguf/model.gguf").is_file());
        let replay = execute_model_root_relocation(&plan).unwrap();
        assert_eq!(replay.plan_id, receipt.plan_id);
    }
}
