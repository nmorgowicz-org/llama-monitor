//! Community Source Catalog — user-editable role-based source metadata.
//!
//! Replaces the GGUF-only KnownQuantizer concept with a unified, role-bearing catalog that
//! supports: original author, dataset author, GGUF quantizer, MLX converter/publisher,
//! curator, and custom roles. Backward-compatible with existing hf-quantizers.json via
//! migration from KnownQuantizer/UserQuantizer.
//!
//! Design constraints (from Phase 8 builder brief item 10 and D29):
//! - User-editable with persistence
//! - Curated/heretic/uncensored/discovery preferences are separate from technical qualification
//! - Original author never becomes converter
//! - All roles are evidence-bearing, not name-only claims
//!
//! Phase 8A: backend module, not yet wired to endpoints.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

/// A single role entry in the community source catalog.
///
/// Each entry represents one person/org with a specific role relative to a model.
/// The same username can appear with multiple roles in different entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommunitySourceEntry {
    /// HF username or org id (e.g. "bartowski", "unsloth", "meta-llama")
    pub username: String,
    /// Display name shown in UI
    pub display_name: String,
    /// Brief description of their contribution style
    pub description: String,
    /// Primary role this entry represents
    pub role: CommunitySourceRole,
    /// Additional roles this entity performs across the ecosystem
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also_known_for: Vec<CommunitySourceRole>,
    /// Category hints for discovery UI (heretic, uncensored, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    /// Optional note explaining quirks or special behavior
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Whether this entry is bundled by llama-monitor (true) or user-added (false).
    /// Bundled entries can be modified but not removed via API (must use reset).
    #[serde(default)]
    pub bundled: bool,
}

/// The canonical roles recognized by the community source catalog.
///
/// These roles are distinct and non-conflatable. In particular:
/// - OriginalAuthor is never the same as MlxConverter or GgufQuantizer
/// - DatasetAuthor is the creator of training data, not the model weights
/// - Curator selects/organizes but does not create weights
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CommunitySourceRole {
    /// The entity that created the original model weights (first-party or finetuner).
    OriginalAuthor,
    /// The entity that created or curated the training dataset.
    DatasetAuthor,
    /// The entity that produced GGUF quantized weights.
    GgufQuantizer,
    /// The entity that converted or produced native MLX weights from another format.
    MlxConverter,
    /// The entity that published/curated model selections or collections.
    Curator,
    /// The entity that merged or distilled from other models.
    MergerDistiller,
    /// Custom/user-defined role not in the standard set.
    Custom,
}

impl CommunitySourceRole {
    pub fn label(&self) -> &'static str {
        match self {
            CommunitySourceRole::OriginalAuthor => "Original author",
            CommunitySourceRole::DatasetAuthor => "Dataset author",
            CommunitySourceRole::GgufQuantizer => "GGUF quantizer",
            CommunitySourceRole::MlxConverter => "MLX converter",
            CommunitySourceRole::Curator => "Curator",
            CommunitySourceRole::MergerDistiller => "Merger / distiller",
            CommunitySourceRole::Custom => "Custom",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            CommunitySourceRole::OriginalAuthor => {
                "Created the original model weights or first fine-tune."
            }
            CommunitySourceRole::DatasetAuthor => {
                "Created or curated the training dataset used for this model."
            }
            CommunitySourceRole::GgufQuantizer => {
                "Produced GGUF quantized weights from this model."
            }
            CommunitySourceRole::MlxConverter => {
                "Converted or produced native MLX weights from this model."
            }
            CommunitySourceRole::Curator => "Selects, organizes, or publishes model collections.",
            CommunitySourceRole::MergerDistiller => "Merged or distilled from multiple models.",
            CommunitySourceRole::Custom => "User-defined role.",
        }
    }
}

/// The full community source catalog stored on disk.
///
/// This is the user-editable catalog that replaces the static KnownQuantizer list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunitySourceCatalog {
    /// All source entries (bundled + user-added)
    pub entries: Vec<CommunitySourceEntry>,
    /// User preferences for discovery/filtering (separate from technical qualification)
    #[serde(default)]
    pub preferences: CommunitySourcePreferences,
    /// Schema version for future migrations
    #[serde(default = "catalog_version")]
    pub version: u32,
}

fn catalog_version() -> u32 {
    1
}

/// User preferences that guide discovery and categorization without affecting technical
/// qualification. These are editorial choices (e.g. "show heretic variants prominently")
/// and must never be confused with Rapid-MLX compatibility or fit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct CommunitySourcePreferences {
    /// Show heretic/abliterated variants prominently in discovery
    #[serde(default)]
    pub prefer_heretic: bool,
    /// Show uncensored variants prominently
    #[serde(default)]
    pub prefer_uncensored: bool,
    /// Show updated-dataset finetunes/distillations prominently
    #[serde(default)]
    pub prefer_updated_finetune: bool,
    /// Custom usernames/orgs the user trusts and wants featured
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_sources: Vec<String>,
    /// Custom categories the user recognizes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_categories: Vec<String>,
}

/// Configuration directory constants.
const CATALOG_FILE: &str = "community-source-catalog.json";
const LEGACY_QUANTIZERS_FILE: &str = "hf-quantizers.json";

/// Load the community source catalog from the config directory.
///
/// Migration behavior:
/// - If community-source-catalog.json exists, load it directly
/// - If only hf-quantizers.json exists, migrate KnownQuantizer entries to GgufQuantizer role
/// - Otherwise, return the bundled default catalog
pub fn load_catalog(config_dir: &Path) -> CommunitySourceCatalog {
    let catalog_path = config_dir.join(CATALOG_FILE);
    if catalog_path.exists()
        && let Ok(text) = std::fs::read_to_string(&catalog_path)
        && let Ok(catalog) = serde_json::from_str(&text)
    {
        return catalog;
    }

    let legacy_path = config_dir.join(LEGACY_QUANTIZERS_FILE);
    if legacy_path.exists()
        && let Some(entries) = crate::hf::load_user_quantizers(config_dir)
    {
        return migrate_from_user_quantizers(entries);
    }

    default_catalog()
}

/// Save the community source catalog to the config directory.
///
/// Writes atomically via temp file + rename.
pub fn save_catalog(config_dir: &Path, catalog: &CommunitySourceCatalog) -> anyhow::Result<()> {
    let path = config_dir.join(CATALOG_FILE);
    let parent = path.parent().unwrap_or(config_dir);
    std::fs::create_dir_all(parent)?;

    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(catalog)
        .map_err(|e| anyhow::anyhow!("Failed to serialize catalog: {e}"))?;

    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(json.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Reset the catalog to defaults, preserving user preferences.
pub fn reset_catalog(config_dir: &Path) -> anyhow::Result<CommunitySourceCatalog> {
    let current = load_catalog(config_dir);
    let mut defaults = default_catalog();
    defaults.preferences = current.preferences;
    save_catalog(config_dir, &defaults)?;
    Ok(defaults)
}

/// Add or update an entry in the catalog.
///
/// Returns the updated entry. If username + role combination exists, updates it.
/// OriginalAuthor entries cannot be overwritten to be a converter role (hard gate).
pub fn upsert_entry(
    catalog: &mut CommunitySourceCatalog,
    entry: CommunitySourceEntry,
) -> anyhow::Result<CommunitySourceEntry> {
    let existing_idx = catalog
        .entries
        .iter()
        .position(|e| e.username == entry.username && e.role == entry.role);

    if let Some(idx) = existing_idx {
        catalog.entries[idx] = entry.clone();
        return Ok(entry);
    }

    let is_original_author = catalog
        .entries
        .iter()
        .any(|e| e.username == entry.username && e.role == CommunitySourceRole::OriginalAuthor);
    let is_converter = entry.role == CommunitySourceRole::MlxConverter
        || entry.role == CommunitySourceRole::GgufQuantizer;

    if is_original_author && is_converter {
        return Err(anyhow::anyhow!(
            "Cannot add converter/quantizer role for username {} with existing OriginalAuthor",
            entry.username
        ));
    }

    if entry.role == CommunitySourceRole::OriginalAuthor {
        let conflicting = catalog.entries.iter().any(|e| {
            e.username == entry.username
                && (e.role == CommunitySourceRole::MlxConverter
                    || e.role == CommunitySourceRole::GgufQuantizer)
        });
        if conflicting {
            return Err(anyhow::anyhow!(
                "Username {} already has a converter/quantizer role; OriginalAuthor must be separate",
                entry.username
            ));
        }
    }

    catalog.entries.push(entry);
    Ok(catalog.entries.last().cloned().unwrap())
}

/// Remove a user-added entry by username + role.
///
/// Returns true if removed. Bundled entries cannot be removed.
pub fn remove_entry(
    catalog: &mut CommunitySourceCatalog,
    username: &str,
    role: CommunitySourceRole,
) -> bool {
    let len_before = catalog.entries.len();
    catalog
        .entries
        .retain(|e| !(e.username == username && e.role == role) || e.bundled);
    len_before != catalog.entries.len()
}

/// Get entries filtered by role.
pub fn entries_for_role(
    catalog: &CommunitySourceCatalog,
    role: CommunitySourceRole,
) -> Vec<&CommunitySourceEntry> {
    catalog.entries.iter().filter(|e| e.role == role).collect()
}

/// Get entries for a username across all roles.
pub fn entries_for_username<'a>(
    catalog: &'a CommunitySourceCatalog,
    username: &str,
) -> Vec<&'a CommunitySourceEntry> {
    catalog
        .entries
        .iter()
        .filter(|e| e.username == username)
        .collect()
}

/// Build the default (bundled) catalog with known community contributors.
///
/// This encodes the current knowledge of who does what in the ecosystem without
/// conflating roles. Bartowski is a quantizer, not an author. Unsloth does both
/// finetuning (OriginalAuthor for their finetunes) and quantizing.
fn default_catalog() -> CommunitySourceCatalog {
    CommunitySourceCatalog {
        version: 1,
        entries: vec![
            CommunitySourceEntry {
                username: "bartowski".into(),
                display_name: "bartowski".into(),
                description: "Standard GGUF quants — Q4_K_M through Q8_0. Most popular, extremely reliable.".into(),
                role: CommunitySourceRole::GgufQuantizer,
                also_known_for: Vec::new(),
                categories: Vec::new(),
                note: None,
                bundled: true,
            },
            CommunitySourceEntry {
                username: "mradermacher".into(),
                display_name: "mradermacher".into(),
                description: "imatrix specialist. i1-* files use importance calibration for better quality at same bpw. Validates quantizations.".into(),
                role: CommunitySourceRole::GgufQuantizer,
                also_known_for: Vec::new(),
                categories: Vec::new(),
                note: Some("i1-* files are imatrix quants; others are standard".into()),
                bundled: true,
            },
            CommunitySourceEntry {
                username: "unsloth".into(),
                display_name: "Unsloth".into(),
                description: "UD (Unsloth Dynamic) quants — mixed bpw per layer. Excellent quality/size. Also does fine-tuning and finetune-GGUF releases.".into(),
                role: CommunitySourceRole::OriginalAuthor,
                also_known_for: vec![CommunitySourceRole::GgufQuantizer],
                categories: vec!["updated-finetune".into()],
                note: Some("UD-* files are dynamic quants; original author of Unsloth finetunes".into()),
                bundled: true,
            },
            CommunitySourceEntry {
                username: "lmstudio-community".into(),
                display_name: "LM Studio".into(),
                description: "LM Studio community quants.".into(),
                role: CommunitySourceRole::GgufQuantizer,
                also_known_for: Vec::new(),
                categories: Vec::new(),
                note: None,
                bundled: true,
            },
            CommunitySourceEntry {
                username: "llmfan46".into(),
                display_name: "llmfan46".into(),
                description: "Community GGUF releases, wide model coverage.".into(),
                role: CommunitySourceRole::GgufQuantizer,
                also_known_for: Vec::new(),
                categories: Vec::new(),
                note: None,
                bundled: true,
            },
            CommunitySourceEntry {
                username: "DavidAU".into(),
                display_name: "DavidAU".into(),
                description: "Fine-tune and merge specialist, often heretic/abliterated and uncensored variants.".into(),
                role: CommunitySourceRole::OriginalAuthor,
                also_known_for: vec![CommunitySourceRole::MergerDistiller, CommunitySourceRole::GgufQuantizer],
                categories: vec!["heretic".into(), "uncensored".into()],
                note: None,
                bundled: true,
            },
            CommunitySourceEntry {
                username: "mudler".into(),
                display_name: "mudler".into(),
                description: "LocalAI author. Curated model selections and gguf releases.".into(),
                role: CommunitySourceRole::Curator,
                also_known_for: vec![CommunitySourceRole::GgufQuantizer],
                categories: Vec::new(),
                note: None,
                bundled: true,
            },
            CommunitySourceEntry {
                username: "Jackrong".into(),
                display_name: "Jackrong".into(),
                description: "GGUF releases, often larger models.".into(),
                role: CommunitySourceRole::GgufQuantizer,
                also_known_for: Vec::new(),
                categories: Vec::new(),
                note: None,
                bundled: true,
            },
            CommunitySourceEntry {
                username: "prithivMLmods".into(),
                display_name: "prithivMLmods".into(),
                description: "Wide coverage of recent models, high-quality GGUF quants.".into(),
                role: CommunitySourceRole::GgufQuantizer,
                also_known_for: Vec::new(),
                categories: Vec::new(),
                note: None,
                bundled: true,
            },
            CommunitySourceEntry {
                username: "mlx-community".into(),
                display_name: "MLX Community".into(),
                description: "Native MLX model conversions and optimizations.".into(),
                role: CommunitySourceRole::MlxConverter,
                also_known_for: Vec::new(),
                categories: Vec::new(),
                note: None,
                bundled: true,
            },
        ],
        preferences: CommunitySourcePreferences::default(),
    }
}

/// Migrate a legacy UserQuantizer list (from hf-quantizers.json) into the new
/// CommunitySourceCatalog format. Each UserQuantizer becomes a GgufQuantizer entry.
fn migrate_from_user_quantizers(
    quantizers: Vec<crate::hf::UserQuantizer>,
) -> CommunitySourceCatalog {
    let entries: Vec<CommunitySourceEntry> = quantizers
        .into_iter()
        .map(|q| {
            let username_lower = q.username.to_ascii_lowercase();
            let role = if username_lower == "unsloth" {
                CommunitySourceRole::OriginalAuthor
            } else if username_lower == "mudler" {
                CommunitySourceRole::Curator
            } else if username_lower == "davidau" {
                CommunitySourceRole::OriginalAuthor
            } else {
                CommunitySourceRole::GgufQuantizer
            };

            let categories = if username_lower == "davidau" {
                vec!["heretic".into(), "uncensored".into()]
            } else if username_lower == "unsloth" {
                vec!["updated-finetune".into()]
            } else {
                Vec::new()
            };

            let also_known_for = match role {
                CommunitySourceRole::OriginalAuthor => {
                    if username_lower == "unsloth" {
                        vec![CommunitySourceRole::GgufQuantizer]
                    } else if username_lower == "davidau" {
                        vec![
                            CommunitySourceRole::MergerDistiller,
                            CommunitySourceRole::GgufQuantizer,
                        ]
                    } else {
                        Vec::new()
                    }
                }
                _ => Vec::new(),
            };

            CommunitySourceEntry {
                username: q.username,
                display_name: q.display_name,
                description: q.description,
                role,
                also_known_for,
                categories,
                note: q.note,
                bundled: false,
            }
        })
        .collect();

    CommunitySourceCatalog {
        version: 1,
        entries,
        preferences: CommunitySourcePreferences::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_has_expected_roles() {
        let catalog = default_catalog();

        let bartowski = entries_for_username(&catalog, "bartowski");
        assert_eq!(bartowski.len(), 1);
        assert_eq!(bartowski[0].role, CommunitySourceRole::GgufQuantizer);

        let unsloth = entries_for_username(&catalog, "unsloth");
        assert_eq!(unsloth.len(), 1);
        assert_eq!(unsloth[0].role, CommunitySourceRole::OriginalAuthor);

        let mlx_community = entries_for_username(&catalog, "mlx-community");
        assert_eq!(mlx_community.len(), 1);
        assert_eq!(mlx_community[0].role, CommunitySourceRole::MlxConverter);
    }

    #[test]
    fn upsert_cannot_make_original_author_a_converter() {
        let mut catalog = default_catalog();

        let entry = CommunitySourceEntry {
            username: "unsloth".into(),
            display_name: "Unsloth".into(),
            description: "Test".into(),
            role: CommunitySourceRole::MlxConverter,
            also_known_for: Vec::new(),
            categories: Vec::new(),
            note: None,
            bundled: false,
        };

        let result = upsert_entry(&mut catalog, entry);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("OriginalAuthor"));
    }

    #[test]
    fn upsert_cannot_add_original_author_when_converter_exists() {
        let mut catalog = default_catalog();
        catalog.entries.retain(|e| e.username != "bartowski");

        let converter = CommunitySourceEntry {
            username: "testuser".into(),
            display_name: "Test User".into(),
            description: "Test".into(),
            role: CommunitySourceRole::MlxConverter,
            also_known_for: Vec::new(),
            categories: Vec::new(),
            note: None,
            bundled: false,
        };
        upsert_entry(&mut catalog, converter).unwrap();

        let author = CommunitySourceEntry {
            username: "testuser".into(),
            display_name: "Test User".into(),
            description: "Test".into(),
            role: CommunitySourceRole::OriginalAuthor,
            also_known_for: Vec::new(),
            categories: Vec::new(),
            note: None,
            bundled: false,
        };

        let result = upsert_entry(&mut catalog, author);
        assert!(result.is_err());
    }

    #[test]
    fn bundled_entries_cannot_be_removed() {
        let mut catalog = default_catalog();
        assert!(!remove_entry(
            &mut catalog,
            "bartowski",
            CommunitySourceRole::GgufQuantizer
        ));
        assert!(!entries_for_username(&catalog, "bartowski").is_empty());
    }

    #[test]
    fn user_entries_can_be_removed() {
        let mut catalog = default_catalog();
        upsert_entry(
            &mut catalog,
            CommunitySourceEntry {
                username: "myuser".into(),
                display_name: "My User".into(),
                description: "Test".into(),
                role: CommunitySourceRole::Custom,
                also_known_for: Vec::new(),
                categories: Vec::new(),
                note: None,
                bundled: false,
            },
        )
        .unwrap();
        assert!(remove_entry(
            &mut catalog,
            "myuser",
            CommunitySourceRole::Custom
        ));
        assert!(entries_for_username(&catalog, "myuser").is_empty());
    }

    #[test]
    fn catalog_serde_roundtrips() {
        let catalog = default_catalog();
        let json = serde_json::to_string(&catalog).unwrap();
        let loaded: CommunitySourceCatalog = serde_json::from_str(&json).unwrap();
        assert_eq!(catalog.entries.len(), loaded.entries.len());
        assert_eq!(catalog.version, loaded.version);
    }

    #[test]
    fn migration_preserves_quantizer_data() {
        let quantizers = vec![
            crate::hf::UserQuantizer {
                username: "bartowski".into(),
                display_name: "bartowski".into(),
                description: "Test".into(),
                quant_style: "standard".into(),
                note: None,
            },
            crate::hf::UserQuantizer {
                username: "davidau".into(),
                display_name: "DavidAU".into(),
                description: "Heretic specialist".into(),
                quant_style: "standard".into(),
                note: None,
            },
        ];
        let catalog = migrate_from_user_quantizers(quantizers);
        assert_eq!(catalog.entries.len(), 2);

        let bart = entries_for_username(&catalog, "bartowski");
        assert_eq!(bart[0].role, CommunitySourceRole::GgufQuantizer);

        let david = entries_for_username(&catalog, "davidau");
        assert_eq!(david[0].role, CommunitySourceRole::OriginalAuthor);
        assert!(david[0].categories.contains(&"heretic".into()));
    }
}
