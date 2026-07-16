use crate::inference::InferenceBackend;
use crate::inference::rapid_mlx::model_resolver::{
    AuthoritativeSafetensorsSource, MlxConversionRecipe, RapidMlxModelSource,
};
use crate::models::gguf_recovery::{
    ExperimentalInventoryCacheKind, validate_experimental_inventory_cache,
};
use crate::models::{DiscoveredModel, scan_models_dir};
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const MAX_INVENTORY_ENTRIES: usize = 10_000;
const MAX_PROVENANCE_BYTES: u64 = 64 * 1024;
pub const HF_SOURCE_METADATA_NAME: &str = ".llama-monitor-source.json";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InventoryFormat {
    Gguf,
    Mlx,
    Transformers,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InventorySource {
    Local,
    HuggingFace,
    OfficialConversion,
    RecoveredGguf,
    RequantizedMlx,
    Legacy,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InventoryLifecycle {
    Ready,
    Incomplete,
    Converting,
    Invalid,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InventoryCompatibility {
    Verified,
    Experimental,
    Provisional,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompanionKind {
    Mmproj,
    Draft,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInventoryEntry {
    pub id: String,
    pub path: PathBuf,
    pub filename: String,
    pub size_bytes: u64,
    pub size_display: String,
    pub quant_type: Option<String>,
    pub model_name: Option<String>,
    pub is_split: bool,
    pub param_b: Option<f32>,
    pub vram_est_gb: Option<f32>,
    pub quant_style: Option<&'static str>,
    pub last_modified: u64,
    pub is_mmproj: bool,
    pub is_draft_assistant: bool,
    pub format: InventoryFormat,
    pub source: InventorySource,
    pub lifecycle: InventoryLifecycle,
    pub compatibility: InventoryCompatibility,
    pub supported_backends: Vec<InferenceBackend>,
    pub companion_kind: Option<CompanionKind>,
    pub legacy_location: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_source: Option<RapidMlxModelSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInventory {
    pub models_dir: PathBuf,
    pub entries: Vec<ModelInventoryEntry>,
    pub truncated: bool,
}

pub fn inventory(models_dir: &Path) -> Result<ModelInventory> {
    let root = canonical_library_root(models_dir)?;
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    add_gguf_directory(&root, true, &mut entries, &mut seen)?;
    add_legacy_partial_files(&root, &mut entries, &mut seen)?;
    add_gguf_directory(&root.join("gguf"), false, &mut entries, &mut seen)?;
    add_model_directories(
        &root,
        &root.join("mlx/native"),
        InventoryFormat::Mlx,
        InventorySource::Local,
        false,
        &mut entries,
        &mut seen,
    )?;
    add_experimental_mlx_caches(&root, &mut entries, &mut seen)?;
    add_model_directories(
        &root,
        &root.join("mlx/converted"),
        InventoryFormat::Mlx,
        InventorySource::OfficialConversion,
        false,
        &mut entries,
        &mut seen,
    )?;
    add_model_directories(
        &root,
        &root.join("transformers"),
        InventoryFormat::Transformers,
        InventorySource::Local,
        false,
        &mut entries,
        &mut seen,
    )?;
    add_hf_snapshots(&root, &mut entries, &mut seen)?;
    add_staging(&root, &mut entries, &mut seen)?;
    entries.sort_by(|a, b| {
        a.filename
            .to_ascii_lowercase()
            .cmp(&b.filename.to_ascii_lowercase())
            .then(a.path.cmp(&b.path))
    });
    let truncated = entries.len() > MAX_INVENTORY_ENTRIES;
    entries.truncate(MAX_INVENTORY_ENTRIES);
    Ok(ModelInventory {
        models_dir: root,
        entries,
        truncated,
    })
}

fn add_experimental_mlx_caches(
    root: &Path,
    entries: &mut Vec<ModelInventoryEntry>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    for (parent, child, source, kind) in [
        (
            root.join("rapid-mlx/imports"),
            "fp16",
            InventorySource::RecoveredGguf,
            ExperimentalInventoryCacheKind::RecoveredGguf,
        ),
        (
            root.join("rapid-mlx/requantized"),
            "model",
            InventorySource::RequantizedMlx,
            ExperimentalInventoryCacheKind::RequantizedMlx,
        ),
    ] {
        if !parent.is_dir() {
            continue;
        }
        for cache in fs::read_dir(&parent)?.take(MAX_INVENTORY_ENTRIES) {
            let cache = cache?;
            if cache.file_name() == ".staging"
                || cache.file_type()?.is_symlink()
                || !cache.file_type()?.is_dir()
            {
                continue;
            }
            let cache_path = cache.path();
            ensure_existing_inside(root, &cache_path)?;
            let Ok(summary) = validate_experimental_inventory_cache(&cache_path, kind) else {
                continue;
            };
            let model_path = cache_path.join(child);
            let metadata = match fs::symlink_metadata(&model_path) {
                Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => metadata,
                _ => continue,
            };
            ensure_existing_inside(root, &model_path)?;
            let canonical = model_path.canonicalize()?;
            if !seen.insert(canonical) {
                continue;
            }
            let mut entry = directory_entry(
                &model_path,
                InventoryFormat::Mlx,
                source,
                InventoryLifecycle::Ready,
                InventoryCompatibility::Experimental,
                false,
                None,
                Some(summary.provenance),
                root,
            )?;
            entry.last_modified = modified(&metadata);
            entry.quant_type = Some(summary.quant_type);
            entry.model_name = Some(if source == InventorySource::RecoveredGguf {
                "Recovered FP16 (Experimental)".into()
            } else {
                "Re-quantized MLX (Experimental)".into()
            });
            entries.push(entry);
        }
    }
    Ok(())
}

fn add_legacy_partial_files(
    root: &Path,
    entries: &mut Vec<ModelInventoryEntry>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(root)?.take(MAX_INVENTORY_ENTRIES) {
        let entry = entry?;
        if entry.file_type()?.is_symlink() || !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.to_ascii_lowercase().ends_with(".part") {
            continue;
        }
        let path = entry.path();
        let canonical = path.canonicalize()?;
        if !seen.insert(canonical) {
            continue;
        }
        entries.push(file_entry(
            &path,
            name,
            InventoryFormat::Gguf,
            InventorySource::Legacy,
            InventoryLifecycle::Incomplete,
            InventoryCompatibility::Unsupported,
            true,
        )?);
    }
    Ok(())
}

fn add_gguf_directory(
    root: &Path,
    legacy: bool,
    entries: &mut Vec<ModelInventoryEntry>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for model in scan_models_dir(root)? {
        let canonical = model.path.canonicalize()?;
        if !seen.insert(canonical) {
            continue;
        }
        entries.push(from_gguf(model, legacy));
    }
    Ok(())
}

fn from_gguf(model: DiscoveredModel, legacy: bool) -> ModelInventoryEntry {
    let companion_kind = if model.is_mmproj {
        Some(CompanionKind::Mmproj)
    } else if model.is_draft_assistant {
        Some(CompanionKind::Draft)
    } else {
        None
    };
    let compatibility = InventoryCompatibility::Verified;
    ModelInventoryEntry {
        id: inventory_id(&model.path),
        path: model.path.clone(),
        filename: model.filename,
        size_bytes: model.size_bytes,
        size_display: model.size_display,
        quant_type: model.quant_type,
        model_name: model.model_name,
        is_split: model.is_split,
        param_b: model.param_b,
        vram_est_gb: model.vram_est_gb,
        quant_style: model.quant_style,
        last_modified: model.last_modified,
        is_mmproj: model.is_mmproj,
        is_draft_assistant: model.is_draft_assistant,
        format: InventoryFormat::Gguf,
        source: if legacy {
            InventorySource::Legacy
        } else {
            InventorySource::Local
        },
        lifecycle: InventoryLifecycle::Ready,
        compatibility,
        supported_backends: vec![InferenceBackend::LlamaCpp],
        companion_kind,
        legacy_location: legacy,
        model_source: Some(RapidMlxModelSource::GgufFile {
            path: model.path.clone(),
        }),
        provenance: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn add_model_directories(
    library_root: &Path,
    parent: &Path,
    format: InventoryFormat,
    source: InventorySource,
    legacy: bool,
    entries: &mut Vec<ModelInventoryEntry>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    if !parent.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(parent)?.take(MAX_INVENTORY_ENTRIES + 1) {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_symlink() {
            entries.push(directory_entry(
                &path,
                format,
                source,
                InventoryLifecycle::Invalid,
                InventoryCompatibility::Unsupported,
                legacy,
                None,
                None,
                library_root,
            )?);
            continue;
        }
        if !entry.file_type()?.is_dir() {
            continue;
        }
        ensure_existing_inside(library_root, &path)?;
        let canonical = path.canonicalize()?;
        if !seen.insert(canonical) {
            continue;
        }
        let required = crate::inference::rapid_mlx::model_resolver::validate_model_directory(
            &path,
            library_root,
        )
        .is_ok();
        let verified_conversion = source != InventorySource::OfficialConversion
            || crate::inference::rapid_mlx::model_resolver::validate_cached_conversion(&path)
                .is_ok();
        let lifecycle = if required && verified_conversion {
            InventoryLifecycle::Ready
        } else {
            InventoryLifecycle::Invalid
        };
        let compatibility = if lifecycle == InventoryLifecycle::Ready {
            InventoryCompatibility::Verified
        } else {
            InventoryCompatibility::Unsupported
        };
        let provenance = if source == InventorySource::OfficialConversion {
            bounded_json(&path.join("llama-monitor-conversion.json"))
        } else {
            None
        };
        let model_source = if lifecycle != InventoryLifecycle::Ready {
            None
        } else if format == InventoryFormat::Mlx {
            Some(RapidMlxModelSource::MlxDirectory { path: path.clone() })
        } else if format == InventoryFormat::Transformers {
            crate::inference::rapid_mlx::model_resolver::model_content_hash(&path, library_root)
                .ok()
                .map(
                    |content_hash| RapidMlxModelSource::AuthoritativeSafetensors {
                        source: AuthoritativeSafetensorsSource::LocalDirectory {
                            path: path.clone(),
                        },
                        revision_or_hash: content_hash,
                        recipe: MlxConversionRecipe::Fp16,
                    },
                )
        } else {
            read_source_metadata(&path)
        };
        entries.push(directory_entry(
            &path,
            format,
            source,
            lifecycle,
            compatibility,
            legacy,
            model_source,
            provenance,
            library_root,
        )?);
    }
    Ok(())
}

fn add_hf_snapshots(
    root: &Path,
    entries: &mut Vec<ModelInventoryEntry>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    let hub = root.join("cache/huggingface/hub");
    if !hub.is_dir() {
        return Ok(());
    }
    for repo in fs::read_dir(&hub)?.take(MAX_INVENTORY_ENTRIES) {
        let repo = repo?;
        if repo.file_type()?.is_symlink() || !repo.file_type()?.is_dir() {
            continue;
        }
        let snapshots = repo.path().join("snapshots");
        if !snapshots.is_dir() {
            continue;
        }
        for snapshot in fs::read_dir(snapshots)?.take(1024) {
            let snapshot = snapshot?;
            if snapshot.file_type()?.is_symlink() || !snapshot.file_type()?.is_dir() {
                continue;
            }
            let path = snapshot.path();
            ensure_existing_inside(root, &path)?;
            let canonical = path.canonicalize()?;
            if !seen.insert(canonical) {
                continue;
            }
            let valid =
                crate::inference::rapid_mlx::model_resolver::validate_model_directory(&path, root)
                    .is_ok();
            let model_source = read_source_metadata(&path);
            let format = match model_source.as_ref() {
                Some(RapidMlxModelSource::HuggingFaceRepo { .. }) => InventoryFormat::Mlx,
                Some(RapidMlxModelSource::AuthoritativeSafetensors { .. }) => {
                    InventoryFormat::Transformers
                }
                _ => InventoryFormat::Unknown,
            };
            entries.push(directory_entry(
                &path,
                format,
                if model_source.is_some() {
                    InventorySource::HuggingFace
                } else {
                    InventorySource::Unknown
                },
                if valid && model_source.is_some() {
                    InventoryLifecycle::Ready
                } else if valid {
                    InventoryLifecycle::Unknown
                } else {
                    InventoryLifecycle::Incomplete
                },
                if valid && model_source.is_some() {
                    InventoryCompatibility::Provisional
                } else {
                    InventoryCompatibility::Unknown
                },
                false,
                model_source,
                None,
                root,
            )?);
        }
    }
    Ok(())
}

fn add_staging(
    root: &Path,
    entries: &mut Vec<ModelInventoryEntry>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    let staging = root.join(".staging");
    if !staging.is_dir() {
        return Ok(());
    }
    let mut stack = vec![staging];
    let mut visited = 0usize;
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(directory)? {
            visited += 1;
            if visited > MAX_INVENTORY_ENTRIES || entries.len() > MAX_INVENTORY_ENTRIES {
                return Ok(());
            }
            let entry = entry?;
            if entry.file_type()?.is_symlink() {
                continue;
            }
            let path = entry.path();
            ensure_existing_inside(root, &path)?;
            if entry.file_type()?.is_dir() {
                if stack.len() < 256 {
                    stack.push(path);
                }
                continue;
            }
            if !entry.file_type()?.is_file() {
                continue;
            }
            if entry.file_name() == "library-migration-journal.json" {
                continue;
            }
            let canonical = path.canonicalize()?;
            if !seen.insert(canonical) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let format = if name.to_ascii_lowercase().contains("gguf") {
                InventoryFormat::Gguf
            } else {
                InventoryFormat::Unknown
            };
            entries.push(file_entry(
                &path,
                name,
                format,
                InventorySource::Local,
                if path.to_string_lossy().contains(".converting-") {
                    InventoryLifecycle::Converting
                } else {
                    InventoryLifecycle::Incomplete
                },
                InventoryCompatibility::Unsupported,
                false,
            )?);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn directory_entry(
    path: &Path,
    format: InventoryFormat,
    source: InventorySource,
    lifecycle: InventoryLifecycle,
    compatibility: InventoryCompatibility,
    legacy: bool,
    model_source: Option<RapidMlxModelSource>,
    provenance: Option<serde_json::Value>,
    library_root: &Path,
) -> Result<ModelInventoryEntry> {
    let metadata = fs::symlink_metadata(path)?;
    let (size_bytes, size_known) = bounded_directory_size(path, library_root, 20_000)?;
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("model")
        .to_string();
    Ok(ModelInventoryEntry {
        id: inventory_id(path),
        path: path.to_path_buf(),
        filename: name.clone(),
        size_bytes,
        size_display: if size_known {
            format_bytes(size_bytes)
        } else {
            "Size unknown".into()
        },
        quant_type: None,
        model_name: Some(name),
        is_split: false,
        param_b: None,
        vram_est_gb: None,
        quant_style: None,
        last_modified: modified(&metadata),
        is_mmproj: false,
        is_draft_assistant: false,
        format,
        source,
        lifecycle,
        compatibility,
        supported_backends: if lifecycle == InventoryLifecycle::Ready && model_source.is_some() {
            vec![InferenceBackend::RapidMlx]
        } else {
            Vec::new()
        },
        companion_kind: None,
        legacy_location: legacy,
        model_source,
        provenance,
    })
}

fn file_entry(
    path: &Path,
    name: String,
    format: InventoryFormat,
    source: InventorySource,
    lifecycle: InventoryLifecycle,
    compatibility: InventoryCompatibility,
    legacy: bool,
) -> Result<ModelInventoryEntry> {
    let metadata = fs::symlink_metadata(path)?;
    Ok(ModelInventoryEntry {
        id: inventory_id(path),
        path: path.to_path_buf(),
        filename: name,
        size_bytes: metadata.len(),
        size_display: format_bytes(metadata.len()),
        quant_type: None,
        model_name: None,
        is_split: false,
        param_b: None,
        vram_est_gb: None,
        quant_style: None,
        last_modified: modified(&metadata),
        is_mmproj: false,
        is_draft_assistant: false,
        format,
        source,
        lifecycle,
        compatibility,
        supported_backends: Vec::new(),
        companion_kind: None,
        legacy_location: legacy,
        model_source: None,
        provenance: None,
    })
}

fn read_source_metadata(path: &Path) -> Option<RapidMlxModelSource> {
    let metadata = path.join(HF_SOURCE_METADATA_NAME);
    if metadata.metadata().ok()?.len() > MAX_PROVENANCE_BYTES {
        return None;
    }
    serde_json::from_reader(fs::File::open(metadata).ok()?).ok()
}

fn bounded_directory_size(
    path: &Path,
    allowed_root: &Path,
    max_files: usize,
) -> Result<(u64, bool)> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Ok((0, false));
    }
    let canonical_path = path.canonicalize()?;
    let canonical_allowed = allowed_root.canonicalize()?;
    let mut total = 0u64;
    let mut count = 0usize;
    let mut seen_files = HashSet::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                stack.push(entry.path());
                continue;
            }
            if entry.path().is_file() {
                let canonical = entry.path().canonicalize()?;
                if !canonical.starts_with(&canonical_path)
                    && !canonical.starts_with(&canonical_allowed)
                {
                    return Ok((total, false));
                }
                if !seen_files.insert(canonical.clone()) {
                    continue;
                }
                count += 1;
                if count > max_files {
                    return Ok((total, false));
                }
                total = total.saturating_add(canonical.metadata()?.len());
            }
        }
    }
    Ok((total, true))
}

fn bounded_json(path: &Path) -> Option<serde_json::Value> {
    let metadata = path.metadata().ok()?;
    if metadata.len() > MAX_PROVENANCE_BYTES {
        return None;
    }
    serde_json::from_reader(fs::File::open(path).ok()?).ok()
}

fn inventory_id(path: &Path) -> String {
    Sha256::digest(path.to_string_lossy().as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn modified(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_secs())
}
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{bytes} B")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationMove {
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceRewrite {
    pub file: PathBuf,
    pub replacements: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryMigrationPlan {
    pub plan_id: String,
    pub models_dir: PathBuf,
    pub moves: Vec<MigrationMove>,
    pub persistence_rewrites: Vec<PersistenceRewrite>,
    pub rollback: Vec<MigrationMove>,
    pub import_roots: Vec<PathBuf>,
    #[serde(default)]
    pub selected_hf_repos: Vec<String>,
    pub source_aliases: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum MigrationState {
    Planned,
    Moving,
    Rewriting,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MigrationJournal {
    plan: LibraryMigrationPlan,
    state: MigrationState,
    completed_moves: Vec<MigrationMove>,
    completed_rewrites: Vec<PathBuf>,
}

#[allow(dead_code)]
pub fn plan_migration(
    models_dir: &Path,
    persistence_files: &[PathBuf],
) -> Result<LibraryMigrationPlan> {
    plan_migration_with_imports(models_dir, persistence_files, &[])
}

pub fn plan_migration_with_imports(
    models_dir: &Path,
    persistence_files: &[PathBuf],
    import_roots: &[PathBuf],
) -> Result<LibraryMigrationPlan> {
    plan_migration_selected_hf(models_dir, persistence_files, import_roots, &[], None)
}

pub fn plan_migration_selected_hf(
    models_dir: &Path,
    persistence_files: &[PathBuf],
    import_roots: &[PathBuf],
    selected_hf_repos: &[String],
    shared_hf_hub: Option<&Path>,
) -> Result<LibraryMigrationPlan> {
    let original_root = models_dir.to_path_buf();
    let root = canonical_library_root(models_dir)?;
    let original_import_roots = import_roots.to_vec();
    let import_roots: Vec<PathBuf> = import_roots
        .iter()
        .map(|path| {
            if !path.is_dir() || fs::symlink_metadata(path)?.file_type().is_symlink() {
                bail!(
                    "Migration import root is not a safe directory: {}",
                    path.display()
                );
            }
            Ok(path.canonicalize()?)
        })
        .collect::<Result<_>>()?;
    let mut moves = Vec::new();
    collect_migration_moves(&root, &root, true, &mut moves)?;
    for import_root in &import_roots {
        if import_root == &root || import_root.starts_with(&root) {
            continue;
        }
        collect_migration_moves(&root, import_root, false, &mut moves)?;
    }
    let mut hf_import_roots = Vec::new();
    let mut original_hf_import_roots = Vec::new();
    if !selected_hf_repos.is_empty() {
        let shared =
            shared_hf_hub.ok_or_else(|| anyhow!("Shared Hugging Face cache is unavailable"))?;
        if !shared.is_dir() || fs::symlink_metadata(shared)?.file_type().is_symlink() {
            bail!("Shared Hugging Face hub is not a safe directory");
        }
        original_hf_import_roots.push(shared.to_path_buf());
        let shared = shared.canonicalize()?;
        hf_import_roots.push(shared.clone());
        collect_selected_hf_moves(&root, &shared, selected_hf_repos, &mut moves)?;
    }
    moves.sort_by(|a, b| a.source.cmp(&b.source));
    let mut destinations = HashSet::new();
    for movement in &moves {
        if !destinations.insert(movement.destination.clone()) {
            bail!(
                "Migration sources collide at destination {}",
                movement.destination.display()
            );
        }
    }
    let mut replacements: BTreeMap<String, String> = moves
        .iter()
        .map(|item| {
            (
                item.source.to_string_lossy().into_owned(),
                item.destination.to_string_lossy().into_owned(),
            )
        })
        .collect();
    for movement in &moves {
        if let Ok(relative) = movement.source.strip_prefix(&root) {
            replacements.insert(
                original_root.join(relative).to_string_lossy().into_owned(),
                movement.destination.to_string_lossy().into_owned(),
            );
        }
        for (canonical_import, original_import) in import_roots.iter().zip(&original_import_roots) {
            if let Ok(relative) = movement.source.strip_prefix(canonical_import) {
                replacements.insert(
                    original_import
                        .join(relative)
                        .to_string_lossy()
                        .into_owned(),
                    movement.destination.to_string_lossy().into_owned(),
                );
            }
        }
        for (canonical_import, original_import) in
            hf_import_roots.iter().zip(&original_hf_import_roots)
        {
            if let Ok(relative) = movement.source.strip_prefix(canonical_import) {
                replacements.insert(
                    original_import
                        .join(relative)
                        .to_string_lossy()
                        .into_owned(),
                    movement.destination.to_string_lossy().into_owned(),
                );
            }
        }
    }
    let mut persistence_rewrites = Vec::new();
    for file in persistence_files {
        if !file.is_file() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_reader(fs::File::open(file)?)
            .with_context(|| format!("Cannot safely parse persisted JSON {}", file.display()))?;
        let count = count_replacements(&value, &replacements);
        if count > 0 {
            persistence_rewrites.push(PersistenceRewrite {
                file: file.clone(),
                replacements: count,
            });
        }
    }
    let rewrite_fingerprints: Vec<_> = persistence_rewrites
        .iter()
        .map(|rewrite| {
            Ok::<_, anyhow::Error>((
                rewrite.file.clone(),
                rewrite.replacements,
                file_sha256(&rewrite.file)?,
            ))
        })
        .collect::<Result<_>>()?;
    let id_bytes = serde_json::to_vec(&(
        1u32,
        &moves,
        &import_roots,
        &hf_import_roots,
        selected_hf_repos,
        &replacements,
        rewrite_fingerprints,
    ))?;
    let plan_id: String = Sha256::digest(id_bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let rollback = moves
        .iter()
        .map(|item| MigrationMove {
            source: item.destination.clone(),
            destination: item.source.clone(),
        })
        .collect();
    Ok(LibraryMigrationPlan {
        plan_id,
        models_dir: root,
        moves,
        persistence_rewrites,
        rollback,
        import_roots: import_roots.into_iter().chain(hf_import_roots).collect(),
        selected_hf_repos: selected_hf_repos.to_vec(),
        source_aliases: replacements,
    })
}

fn collect_selected_hf_moves(
    models_root: &Path,
    shared_hub: &Path,
    repos: &[String],
    moves: &mut Vec<MigrationMove>,
) -> Result<()> {
    if repos.len() > 32 {
        bail!("At most 32 explicit Hugging Face repositories may be migrated at once");
    }
    for repo in repos {
        let parts: Vec<_> = repo.split('/').collect();
        if parts.len() != 2
            || parts.iter().any(|part| {
                part.is_empty()
                    || !part.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                    })
            })
        {
            bail!("Invalid explicit Hugging Face repository ID: {repo}");
        }
        let cache_name = format!("models--{}--{}", parts[0], parts[1]);
        let source = shared_hub.join(&cache_name);
        if !source.is_dir() {
            bail!("Selected Hugging Face cache repository was not found: {repo}");
        }
        if fs::symlink_metadata(&source)?.file_type().is_symlink() {
            bail!("Selected Hugging Face cache repository is a symlink");
        }
        let destination = models_root.join("cache/huggingface/hub").join(&cache_name);
        if destination.exists() {
            bail!(
                "Hugging Face cache migration collision: {}",
                destination.display()
            );
        }
        validate_new_destination(models_root, &destination)?;
        moves.push(MigrationMove {
            source,
            destination,
        });
        let lock_source = shared_hub.join(".locks").join(&cache_name);
        if lock_source.exists() {
            if fs::symlink_metadata(&lock_source)?.file_type().is_symlink() {
                bail!("Selected Hugging Face lock directory is a symlink");
            }
            let lock_destination = models_root
                .join("cache/huggingface/hub/.locks")
                .join(&cache_name);
            if lock_destination.exists() {
                bail!(
                    "Hugging Face lock migration collision: {}",
                    lock_destination.display()
                );
            }
            moves.push(MigrationMove {
                source: lock_source,
                destination: lock_destination,
            });
        }
    }
    Ok(())
}

fn write_selected_hf_source_metadata(root: &Path, repos: &[String]) -> Result<()> {
    for repo_id in repos {
        let Some((owner, name)) = repo_id.split_once('/') else {
            bail!("Invalid explicit Hugging Face repository ID: {repo_id}");
        };
        let snapshots = root
            .join("cache/huggingface/hub")
            .join(format!("models--{owner}--{name}"))
            .join("snapshots");
        if !snapshots.is_dir() {
            continue;
        }
        for snapshot in fs::read_dir(&snapshots)?.take(1024) {
            let snapshot = snapshot?;
            if snapshot.file_type()?.is_symlink() || !snapshot.file_type()?.is_dir() {
                continue;
            }
            let path = snapshot.path();
            ensure_existing_inside(root, &path)?;
            let revision = snapshot.file_name().to_string_lossy().into_owned();
            if revision.len() < 7 || revision.len() > 128 {
                bail!("Imported Hugging Face snapshot has an invalid revision name");
            }
            atomic_json(
                &path.join(HF_SOURCE_METADATA_NAME),
                &RapidMlxModelSource::HuggingFaceRepo {
                    repo_id: repo_id.clone(),
                    revision,
                },
            )?;
        }
    }
    Ok(())
}

fn collect_migration_moves(
    models_root: &Path,
    scan_root: &Path,
    include_partials: bool,
    moves: &mut Vec<MigrationMove>,
) -> Result<()> {
    for entry in fs::read_dir(scan_root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            bail!("Migration refuses root symlink: {}", entry.path().display());
        }
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_ascii_lowercase();
        let destination = if include_partials && lower.ends_with(".part") {
            Some(models_root.join(".staging/downloads").join(&name))
        } else if lower.ends_with(".gguf") {
            Some(models_root.join("gguf").join(&name))
        } else {
            None
        };
        let Some(destination) = destination else {
            continue;
        };
        if destination.exists() {
            bail!(
                "Migration collision: {} already exists",
                destination.display()
            );
        }
        validate_new_destination(models_root, &destination)?;
        moves.push(MigrationMove {
            source: entry.path(),
            destination,
        });
    }
    Ok(())
}

#[allow(dead_code)]
pub fn execute_migration(
    models_dir: &Path,
    persistence_files: &[PathBuf],
    expected_plan_id: &str,
) -> Result<LibraryMigrationPlan> {
    execute_migration_with_imports(models_dir, persistence_files, &[], expected_plan_id)
}

pub fn execute_migration_with_imports(
    models_dir: &Path,
    persistence_files: &[PathBuf],
    import_roots: &[PathBuf],
    expected_plan_id: &str,
) -> Result<LibraryMigrationPlan> {
    execute_migration_selected_hf(
        models_dir,
        persistence_files,
        import_roots,
        &[],
        None,
        expected_plan_id,
    )
}

pub fn execute_migration_selected_hf(
    models_dir: &Path,
    persistence_files: &[PathBuf],
    import_roots: &[PathBuf],
    selected_hf_repos: &[String],
    shared_hf_hub: Option<&Path>,
    expected_plan_id: &str,
) -> Result<LibraryMigrationPlan> {
    let original_root = models_dir.to_path_buf();
    let root = canonical_library_root(models_dir)?;
    let journal_path = root.join(".staging/library-migration-journal.json");
    let mut journal = if journal_path.is_file() {
        serde_json::from_reader::<_, MigrationJournal>(fs::File::open(&journal_path)?)?
    } else {
        let plan = plan_migration_selected_hf(
            &original_root,
            persistence_files,
            import_roots,
            selected_hf_repos,
            shared_hf_hub,
        )?;
        if plan.plan_id != expected_plan_id {
            bail!("Migration preview is stale; request a fresh preview");
        }
        let journal = MigrationJournal {
            plan,
            state: MigrationState::Planned,
            completed_moves: Vec::new(),
            completed_rewrites: Vec::new(),
        };
        atomic_json(&journal_path, &journal)?;
        journal
    };
    if journal.plan.plan_id != expected_plan_id || journal.plan.models_dir != root {
        bail!("Migration journal does not match requested library and plan");
    }
    if journal.plan.selected_hf_repos != selected_hf_repos {
        bail!("Migration repository selection does not match the journaled preview");
    }
    journal.state = MigrationState::Moving;
    atomic_json(&journal_path, &journal)?;
    for movement in journal.plan.moves.clone() {
        if journal.completed_moves.contains(&movement) {
            continue;
        }
        validate_move_state(&root, &journal.plan.import_roots, &movement)?;
        if movement.source.exists() {
            if let Some(parent) = movement.destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(&movement.source, &movement.destination).with_context(|| {
                format!(
                    "Same-filesystem migration rename failed: {} -> {}",
                    movement.source.display(),
                    movement.destination.display()
                )
            })?;
        }
        journal.completed_moves.push(movement);
        atomic_json(&journal_path, &journal)?;
    }
    write_selected_hf_source_metadata(&root, &journal.plan.selected_hf_repos)?;
    journal.state = MigrationState::Rewriting;
    atomic_json(&journal_path, &journal)?;
    let replacements = journal.plan.source_aliases.clone();
    for rewrite in journal.plan.persistence_rewrites.clone() {
        if journal.completed_rewrites.contains(&rewrite.file) {
            continue;
        }
        rewrite_json_paths(&rewrite.file, &replacements)?;
        journal.completed_rewrites.push(rewrite.file);
        atomic_json(&journal_path, &journal)?;
    }
    journal.state = MigrationState::Complete;
    atomic_json(&journal_path, &journal)?;
    Ok(journal.plan)
}

fn validate_move_state(
    root: &Path,
    import_roots: &[PathBuf],
    movement: &MigrationMove,
) -> Result<()> {
    validate_new_destination(root, &movement.destination)?;
    if movement.source.exists() {
        let metadata = fs::symlink_metadata(&movement.source)?;
        if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
            bail!("Migration source is no longer a regular file or directory");
        }
        let source = movement.source.canonicalize()?;
        if !source.starts_with(root)
            && !import_roots
                .iter()
                .any(|allowed| source.starts_with(allowed))
        {
            bail!("Migration source escapes approved model/config roots");
        }
        if movement.destination.exists() {
            bail!(
                "Migration collision appeared after preview: {}",
                movement.destination.display()
            );
        }
    } else if !movement.destination.is_file() && !movement.destination.is_dir() {
        bail!(
            "Migration source disappeared without a completed destination: {}",
            movement.source.display()
        );
    }
    Ok(())
}

fn rewrite_json_paths(path: &Path, replacements: &BTreeMap<String, String>) -> Result<()> {
    let mut value: serde_json::Value = serde_json::from_reader(fs::File::open(path)?)?;
    replace_values(&mut value, replacements)?;
    atomic_json(path, &value)
}

pub fn rewrite_in_memory_paths<T>(value: &mut T, plan: &LibraryMigrationPlan) -> Result<()>
where
    T: Serialize + serde::de::DeserializeOwned,
{
    let mut json = serde_json::to_value(&*value)?;
    replace_values(&mut json, &plan.source_aliases)?;
    *value = serde_json::from_value(json)?;
    Ok(())
}

fn replace_values(
    value: &mut serde_json::Value,
    replacements: &BTreeMap<String, String>,
) -> Result<()> {
    match value {
        serde_json::Value::String(text) => {
            if let Some(replacement) = replacement_for_path(text, replacements) {
                *text = replacement;
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                replace_values(item, replacements)?;
            }
        }
        serde_json::Value::Object(map) => {
            let old = std::mem::take(map);
            for (key, mut child) in old {
                replace_values(&mut child, replacements)?;
                let rewritten_key = replacement_for_path(&key, replacements).unwrap_or(key);
                if map.insert(rewritten_key.clone(), child).is_some() {
                    bail!("Persisted path-key rewrite would collide at key {rewritten_key}");
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn count_replacements(value: &serde_json::Value, replacements: &BTreeMap<String, String>) -> usize {
    match value {
        serde_json::Value::String(text) => {
            usize::from(replacement_for_path(text, replacements).is_some())
        }
        serde_json::Value::Array(items) => items
            .iter()
            .map(|item| count_replacements(item, replacements))
            .sum(),
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(key, item)| {
                usize::from(replacement_for_path(key, replacements).is_some())
                    + count_replacements(item, replacements)
            })
            .sum(),
        _ => 0,
    }
}

fn replacement_for_path(value: &str, replacements: &BTreeMap<String, String>) -> Option<String> {
    if let Some(exact) = replacements.get(value) {
        return Some(exact.clone());
    }
    let value_path = Path::new(value);
    if !value_path.is_absolute() {
        return None;
    }
    replacements.iter().find_map(|(source, destination)| {
        let source_path = Path::new(source);
        let relative = value_path.strip_prefix(source_path).ok()?;
        if relative.as_os_str().is_empty() {
            return None;
        }
        Some(
            Path::new(destination)
                .join(relative)
                .to_string_lossy()
                .into_owned(),
        )
    })
}

fn canonical_library_root(root: &Path) -> Result<PathBuf> {
    if !root.is_dir() {
        bail!(
            "Configured models_dir is not a directory: {}",
            root.display()
        );
    }
    if fs::symlink_metadata(root)?.file_type().is_symlink() {
        bail!("Configured models_dir may not be a symlink for migration");
    }
    root.canonicalize()
        .context("Failed to canonicalize configured models_dir")
}

fn ensure_existing_inside(root: &Path, path: &Path) -> Result<()> {
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(root) {
        bail!(
            "Model library entry escapes configured models_dir: {}",
            path.display()
        );
    }
    Ok(())
}

fn validate_new_destination(root: &Path, destination: &Path) -> Result<()> {
    if !destination.starts_with(root) {
        bail!("Migration destination escapes configured models_dir");
    }
    let mut ancestor = destination
        .parent()
        .ok_or_else(|| anyhow!("Migration destination has no parent"))?;
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| anyhow!("Migration destination has no existing ancestor"))?;
    }
    let canonical = ancestor.canonicalize()?;
    if !canonical.starts_with(root) {
        bail!("Migration destination crosses a symlink outside models_dir");
    }
    Ok(())
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("JSON path has no parent"))?;
    fs::create_dir_all(parent)?;
    let tmp = path.with_extension("json.tmp");
    let mut file = fs::File::create(&tmp)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.flush()?;
    file.sync_all()?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_experimental_cache(
        library: &Path,
        parent: &str,
        child: &str,
        kind: ExperimentalInventoryCacheKind,
    ) -> PathBuf {
        let parent = library.join(parent);
        fs::create_dir_all(&parent).unwrap();
        let cache = parent.join("building");
        fs::create_dir_all(cache.join(child)).unwrap();
        fs::write(cache.join(child).join("config.json"), b"{}").unwrap();
        fs::write(cache.join("validation.json"), b"{}").unwrap();
        let mut files = BTreeMap::new();
        files.insert(
            format!("{child}/config.json"),
            file_sha256(&cache.join(child).join("config.json")).unwrap(),
        );
        files.insert(
            "validation.json".into(),
            file_sha256(&cache.join("validation.json")).unwrap(),
        );
        let (key, manifest) =
            crate::models::gguf_recovery::inventory_test_manifest(kind, files).unwrap();
        fs::write(
            cache.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(cache.join(".complete"), b"").unwrap();
        let final_path = parent.join(key);
        fs::rename(cache, &final_path).unwrap();
        final_path
    }

    #[test]
    fn recovered_and_requantized_caches_are_first_class_but_not_launchable() {
        let library = tempfile::tempdir().unwrap();
        write_experimental_cache(
            library.path(),
            "rapid-mlx/imports",
            "fp16",
            ExperimentalInventoryCacheKind::RecoveredGguf,
        );
        write_experimental_cache(
            library.path(),
            "rapid-mlx/requantized",
            "model",
            ExperimentalInventoryCacheKind::RequantizedMlx,
        );

        let inventory = inventory(library.path()).unwrap();
        let recovered = inventory
            .entries
            .iter()
            .find(|entry| entry.source == InventorySource::RecoveredGguf)
            .unwrap();
        assert_eq!(recovered.format, InventoryFormat::Mlx);
        assert_eq!(
            recovered.compatibility,
            InventoryCompatibility::Experimental
        );
        assert_eq!(recovered.quant_type.as_deref(), Some("F16 recovered"));
        assert!(recovered.supported_backends.is_empty());
        assert!(recovered.model_source.is_none());
        assert_eq!(
            recovered
                .provenance
                .as_ref()
                .and_then(|value| value.get("lineage_kind"))
                .and_then(serde_json::Value::as_str),
            Some("gguf_recovered_fp16")
        );

        let requantized = inventory
            .entries
            .iter()
            .find(|entry| entry.source == InventorySource::RequantizedMlx)
            .unwrap();
        assert_eq!(requantized.quant_type.as_deref(), Some("affine_8bit_g64"));
        assert_eq!(
            requantized.compatibility,
            InventoryCompatibility::Experimental
        );
        assert!(requantized.supported_backends.is_empty());
        assert!(requantized.model_source.is_none());
    }

    #[test]
    fn experimental_cache_requires_completion_and_valid_status() {
        let library = tempfile::tempdir().unwrap();
        let cache = write_experimental_cache(
            library.path(),
            "rapid-mlx/imports",
            "fp16",
            ExperimentalInventoryCacheKind::RecoveredGguf,
        );
        fs::remove_file(cache.join(".complete")).unwrap();
        let missing_complete_inventory = inventory(library.path()).unwrap();
        assert!(
            missing_complete_inventory
                .entries
                .iter()
                .all(|entry| entry.source != InventorySource::RecoveredGguf)
        );

        fs::write(cache.join(".complete"), b"").unwrap();
        let mut manifest: serde_json::Value =
            serde_json::from_reader(fs::File::open(cache.join("manifest.json")).unwrap()).unwrap();
        manifest["status"] = serde_json::Value::String("incomplete".into());
        fs::write(
            cache.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(
            inventory(library.path())
                .unwrap()
                .entries
                .iter()
                .all(|entry| entry.source != InventorySource::RecoveredGguf)
        );
    }

    #[test]
    fn experimental_cache_rejects_files_outside_the_published_hash_closure() {
        let library = tempfile::tempdir().unwrap();
        let cache = write_experimental_cache(
            library.path(),
            "rapid-mlx/imports",
            "fp16",
            ExperimentalInventoryCacheKind::RecoveredGguf,
        );
        fs::write(
            cache.join("fp16").join("unmanifested.safetensors"),
            b"tampered",
        )
        .unwrap();

        assert!(
            inventory(library.path())
                .unwrap()
                .entries
                .iter()
                .all(|entry| entry.source != InventorySource::RecoveredGguf)
        );
    }

    #[cfg(unix)]
    #[test]
    fn experimental_cache_rejects_symlinked_manifest_without_disclosure() {
        use std::os::unix::fs::symlink;

        let library = tempfile::tempdir().unwrap();
        let cache = write_experimental_cache(
            library.path(),
            "rapid-mlx/imports",
            "fp16",
            ExperimentalInventoryCacheKind::RecoveredGguf,
        );
        fs::remove_file(cache.join("manifest.json")).unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            outside.path(),
            br#"{"launchable":false,"secret":"outside"}"#,
        )
        .unwrap();
        symlink(outside.path(), cache.join("manifest.json")).unwrap();

        let inventory = inventory(library.path()).unwrap();
        assert!(
            inventory
                .entries
                .iter()
                .all(|entry| entry.source != InventorySource::RecoveredGguf)
        );
        assert!(
            !serde_json::to_string(&inventory)
                .unwrap()
                .contains("outside")
        );
    }

    #[test]
    fn legacy_partial_file_has_first_class_incomplete_inventory_entry() {
        let library = tempfile::tempdir().unwrap();
        fs::write(library.path().join("download.gguf.part"), b"partial").unwrap();
        fs::create_dir(library.path().join(".staging")).unwrap();
        fs::write(
            library
                .path()
                .join(".staging/library-migration-journal.json"),
            b"{}",
        )
        .unwrap();

        let inventory = inventory(library.path()).unwrap();
        assert!(
            inventory
                .entries
                .iter()
                .all(|entry| entry.filename != "library-migration-journal.json")
        );
        let entry = inventory
            .entries
            .iter()
            .find(|entry| entry.filename == "download.gguf.part")
            .unwrap();
        assert_eq!(entry.format, InventoryFormat::Gguf);
        assert_eq!(entry.source, InventorySource::Legacy);
        assert_eq!(entry.lifecycle, InventoryLifecycle::Incomplete);
        assert_eq!(entry.compatibility, InventoryCompatibility::Unsupported);
        assert!(entry.supported_backends.is_empty());
    }

    #[test]
    fn migration_preview_moves_gguf_and_part_and_rewrites_exact_paths() {
        let temp = tempfile::tempdir().unwrap();
        let model = temp.path().join("model.gguf");
        let part = temp.path().join("download.gguf.part");
        fs::write(&model, b"gguf").unwrap();
        fs::write(&part, b"partial").unwrap();
        let presets = temp.path().join("presets.json");
        fs::write(
            &presets,
            serde_json::to_vec(&serde_json::json!({"model_path": model})).unwrap(),
        )
        .unwrap();
        let plan = plan_migration(temp.path(), std::slice::from_ref(&presets)).unwrap();
        assert_eq!(plan.moves.len(), 2);
        assert_eq!(plan.persistence_rewrites[0].replacements, 1);
        execute_migration(temp.path(), std::slice::from_ref(&presets), &plan.plan_id).unwrap();
        assert!(temp.path().join("gguf/model.gguf").is_file());
        assert!(
            temp.path()
                .join(".staging/downloads/download.gguf.part")
                .is_file()
        );
        let persisted: serde_json::Value =
            serde_json::from_reader(fs::File::open(presets).unwrap()).unwrap();
        assert!(
            persisted["model_path"]
                .as_str()
                .unwrap()
                .ends_with("gguf/model.gguf")
        );
    }

    #[test]
    fn migration_refuses_collision_and_root_symlink() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("gguf")).unwrap();
        fs::write(temp.path().join("same.gguf"), b"one").unwrap();
        fs::write(temp.path().join("gguf/same.gguf"), b"two").unwrap();
        assert!(plan_migration(temp.path(), &[]).is_err());
    }

    #[test]
    fn migration_rewrites_path_keys_and_imports_external_gguf_without_touching_jinja() {
        let temp = tempfile::tempdir().unwrap();
        let models = temp.path().join("models");
        let config = temp.path().join("config");
        fs::create_dir(&models).unwrap();
        fs::create_dir(&config).unwrap();
        let external = config.join("27B_MTP.gguf");
        fs::write(&external, b"assistant").unwrap();
        fs::write(config.join("chat-template.jinja"), b"preserve").unwrap();
        let tags = config.join("model-tags.json");
        let key = external.to_string_lossy().into_owned();
        fs::write(
            &tags,
            serde_json::to_vec(&serde_json::json!({"tags": {(key): ["draft"]}})).unwrap(),
        )
        .unwrap();
        let plan = plan_migration_with_imports(
            &models,
            std::slice::from_ref(&tags),
            std::slice::from_ref(&config),
        )
        .unwrap();
        assert_eq!(plan.moves.len(), 1);
        assert_eq!(plan.persistence_rewrites[0].replacements, 1);
        execute_migration_with_imports(
            &models,
            std::slice::from_ref(&tags),
            std::slice::from_ref(&config),
            &plan.plan_id,
        )
        .unwrap();
        assert!(models.join("gguf/27B_MTP.gguf").is_file());
        assert_eq!(
            fs::read(config.join("chat-template.jinja")).unwrap(),
            b"preserve"
        );
        let value: serde_json::Value =
            serde_json::from_reader(fs::File::open(tags).unwrap()).unwrap();
        assert!(
            value["tags"]
                .as_object()
                .unwrap()
                .keys()
                .next()
                .unwrap()
                .ends_with("gguf/27B_MTP.gguf")
        );
    }

    #[test]
    fn selected_hf_cache_migration_is_exact_and_preserves_unselected_repo() {
        let temp = tempfile::tempdir().unwrap();
        let models = temp.path().join("models");
        let shared = temp.path().join("shared-hub");
        fs::create_dir(&models).unwrap();
        fs::create_dir_all(shared.join("models--mlx-community--Qwen3-0.6B-4bit/snapshots/abcdef0"))
            .unwrap();
        fs::create_dir_all(shared.join("models--other--keep/snapshots/def")).unwrap();
        fs::write(
            shared.join("models--mlx-community--Qwen3-0.6B-4bit/snapshots/abcdef0/config.json"),
            b"{}",
        )
        .unwrap();
        let persisted_snapshot = shared
            .join("models--mlx-community--Qwen3-0.6B-4bit/snapshots/abcdef0")
            .to_string_lossy()
            .into_owned();
        let presets = temp.path().join("presets.json");
        fs::write(
            &presets,
            serde_json::to_vec(
                &serde_json::json!({"rapid_mlx": {"model_path": persisted_snapshot}}),
            )
            .unwrap(),
        )
        .unwrap();
        let selected = vec!["mlx-community/Qwen3-0.6B-4bit".to_string()];
        let plan = plan_migration_selected_hf(
            &models,
            std::slice::from_ref(&presets),
            &[],
            &selected,
            Some(&shared),
        )
        .unwrap();
        assert_eq!(plan.moves.len(), 1);
        assert_eq!(plan.persistence_rewrites[0].replacements, 1);
        execute_migration_selected_hf(
            &models,
            std::slice::from_ref(&presets),
            &[],
            &selected,
            Some(&shared),
            &plan.plan_id,
        )
        .unwrap();
        assert!(
            models
                .join("cache/huggingface/hub/models--mlx-community--Qwen3-0.6B-4bit")
                .is_dir()
        );
        assert!(shared.join("models--other--keep").is_dir());
        let source: RapidMlxModelSource = serde_json::from_reader(
            fs::File::open(
                models.join("cache/huggingface/hub/models--mlx-community--Qwen3-0.6B-4bit/snapshots/abcdef0/.llama-monitor-source.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(
            matches!(source, RapidMlxModelSource::HuggingFaceRepo { repo_id, revision } if repo_id == "mlx-community/Qwen3-0.6B-4bit" && revision == "abcdef0")
        );
        assert!(
            execute_migration_selected_hf(
                &models,
                std::slice::from_ref(&presets),
                &[],
                &[],
                Some(&shared),
                &plan.plan_id,
            )
            .unwrap_err()
            .to_string()
            .contains("journaled preview")
        );
        let persisted: serde_json::Value =
            serde_json::from_reader(fs::File::open(presets).unwrap()).unwrap();
        assert!(
            persisted["rapid_mlx"]["model_path"]
                .as_str()
                .unwrap()
                .contains("models/cache/huggingface/hub/models--mlx-community--Qwen3-0.6B-4bit/snapshots/abc")
        );
    }
}
