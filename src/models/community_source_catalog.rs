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
//! Served by `/api/hf/community-sources` (GET/PUT, plus `/entry` and `/reset`) in
//! `src/web/api/hf.rs`, which is what makes the "user-editable" claim above true rather
//! than aspirational.

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
    /// Every role, in the order the UI should offer them. Served to the frontend so the
    /// badge labels and their explanations come from this enum rather than being retyped
    /// in JavaScript and drifting from it.
    pub const ALL: [CommunitySourceRole; 7] = [
        CommunitySourceRole::OriginalAuthor,
        CommunitySourceRole::DatasetAuthor,
        CommunitySourceRole::GgufQuantizer,
        CommunitySourceRole::MlxConverter,
        CommunitySourceRole::Curator,
        CommunitySourceRole::MergerDistiller,
        CommunitySourceRole::Custom,
    ];

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
    /// Bundled quantizers hidden from the legacy quick-pick surface.
    /// They remain in the authoritative role catalog and reset restores them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_quantizers: Vec<String>,
    /// Explicit legacy quick-pick styles keyed by HF username.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub quantizer_styles: std::collections::BTreeMap<String, String>,
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

fn is_quantizer_entry(entry: &CommunitySourceEntry) -> bool {
    match entry.role {
        CommunitySourceRole::GgufQuantizer | CommunitySourceRole::MlxConverter => true,
        CommunitySourceRole::Curator | CommunitySourceRole::OriginalAuthor => entry
            .also_known_for
            .contains(&CommunitySourceRole::GgufQuantizer),
        _ => false,
    }
}

fn legacy_quantizer_metadata(
    username: &str,
) -> (CommunitySourceRole, Vec<CommunitySourceRole>, Vec<String>) {
    match username.to_ascii_lowercase().as_str() {
        "unsloth" => (
            CommunitySourceRole::OriginalAuthor,
            vec![CommunitySourceRole::GgufQuantizer],
            vec!["updated-finetune".into()],
        ),
        "davidau" => (
            CommunitySourceRole::OriginalAuthor,
            vec![
                CommunitySourceRole::MergerDistiller,
                CommunitySourceRole::GgufQuantizer,
            ],
            vec!["heretic".into(), "uncensored".into()],
        ),
        "mudler" => (
            CommunitySourceRole::Curator,
            vec![CommunitySourceRole::GgufQuantizer],
            Vec::new(),
        ),
        "mlx-community" | "lmstudio-community" | "nightmedia" => {
            (CommunitySourceRole::MlxConverter, Vec::new(), Vec::new())
        }
        _ => (CommunitySourceRole::GgufQuantizer, Vec::new(), Vec::new()),
    }
}

/// Apply the legacy `/api/hf/quantizers` replacement-list contract to the catalog.
///
/// Omitted bundled entries are hidden from quick-picks but retain their role evidence and
/// bundled flag. Omitted user-added quantizer entries are removed, matching the old list's
/// behavior. Changes are atomic: a rejected entry leaves the input catalog untouched.
pub fn replace_quantizers(
    catalog: &mut CommunitySourceCatalog,
    quantizers: Vec<crate::hf::UserQuantizer>,
) -> anyhow::Result<()> {
    let mut next = catalog.clone();
    let mut submitted = std::collections::HashSet::new();

    for quantizer in &quantizers {
        let username = quantizer.username.trim();
        if username.is_empty() {
            return Err(anyhow::anyhow!("Quantizer username cannot be empty"));
        }
        if !submitted.insert(username.to_string()) {
            return Err(anyhow::anyhow!("Duplicate quantizer username: {username}"));
        }

        if let Some(existing) = next
            .entries
            .iter_mut()
            .find(|entry| entry.username == username && is_quantizer_entry(entry))
        {
            existing.display_name = quantizer.display_name.clone();
            existing.description = quantizer.description.clone();
            existing.note = quantizer.note.clone();
            next.preferences
                .quantizer_styles
                .insert(username.to_string(), quantizer.quant_style.clone());
        } else {
            let (role, also_known_for, categories) = legacy_quantizer_metadata(username);
            upsert_entry(
                &mut next,
                CommunitySourceEntry {
                    username: username.to_string(),
                    display_name: quantizer.display_name.clone(),
                    description: quantizer.description.clone(),
                    role,
                    also_known_for,
                    categories,
                    note: quantizer.note.clone(),
                    bundled: false,
                },
            )?;
            next.preferences
                .quantizer_styles
                .insert(username.to_string(), quantizer.quant_style.clone());
        }
    }

    next.entries.retain(|entry| {
        !is_quantizer_entry(entry) || entry.bundled || submitted.contains(&entry.username)
    });
    next.preferences
        .hidden_quantizers
        .retain(|username| submitted.contains(username));
    next.preferences
        .quantizer_styles
        .retain(|username, _| submitted.contains(username));
    for entry in &next.entries {
        if is_quantizer_entry(entry)
            && entry.bundled
            && !submitted.contains(&entry.username)
            && !next.preferences.hidden_quantizers.contains(&entry.username)
        {
            next.preferences
                .hidden_quantizers
                .push(entry.username.clone());
        }
    }

    *catalog = next;
    Ok(())
}

/// Derive a backward-compatible quantizer list from the catalog for the
/// `/api/hf/quantizers` endpoint.
///
/// Rules:
/// - Include entries whose primary role is quantizer-related (GgufQuantizer,
///   MlxConverter) or whose also_known_for includes GgufQuantizer.
/// - Do NOT include OriginalAuthor entries unless they also have GgufQuantizer
///   in also_known_for (i.e. they actually publish quants).
/// - quant_style is NEVER guessed from role alone; it uses per-username
///   heuristics (and for unknown/custom entries, note/description hints).
/// - This keeps the UI behavior identical (same classes, same tooltips).
pub fn to_quantizers(catalog: &CommunitySourceCatalog) -> Vec<crate::hf::UserQuantizer> {
    catalog
        .entries
        .iter()
        .filter_map(|e| {
            if !is_quantizer_entry(e) || catalog.preferences.hidden_quantizers.contains(&e.username)
            {
                return None;
            }

            // Derive quant_style from username and metadata — never from role alone.
            let username_lower = e.username.to_ascii_lowercase();
            let derived_style = match username_lower.as_str() {
                "mradermacher" => "imatrix",
                "unsloth" => "ud",
                "mlx-community" | "nightmedia" => "mlx",
                "lmstudio-community" => "mlx",
                // For known standard quantizers:
                "bartowski" | "llmfan46" | "davidau" | "mudler" | "jackrong" | "prithivmlmods" => {
                    "standard"
                }
                // For unknown/custom entries, inspect note/description for hints
                _ => {
                    let note = e.note.as_deref().unwrap_or("");
                    let desc = e.description.to_ascii_lowercase();
                    let text = format!("{note} {desc}");
                    if text.contains("imatrix") || text.contains(".i1-") {
                        "imatrix"
                    } else if text.contains("ud ") || text.contains("dynamic") {
                        "ud"
                    } else {
                        "standard"
                    }
                }
            };

            Some(crate::hf::UserQuantizer {
                username: e.username.clone(),
                display_name: e.display_name.clone(),
                description: e.description.clone(),
                quant_style: catalog
                    .preferences
                    .quantizer_styles
                    .get(&e.username)
                    .cloned()
                    .unwrap_or_else(|| derived_style.to_string()),
                note: e.note.clone(),
            })
        })
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
            CommunitySourceEntry {
                username: "nightmedia".into(),
                display_name: "nightmedia".into(),
                description: "MLX model conversions, high-quality MLX quantizations.".into(),
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
    let quantizer_styles = quantizers
        .iter()
        .map(|quantizer| (quantizer.username.clone(), quantizer.quant_style.clone()))
        .collect();
    let entries: Vec<CommunitySourceEntry> = quantizers
        .into_iter()
        .map(|q| {
            let username_lower = q.username.to_ascii_lowercase();
            let is_mlx = username_lower == "mlx-community"
                || username_lower == "lmstudio-community"
                || username_lower == "nightmedia";

            let role = if username_lower == "unsloth" {
                CommunitySourceRole::OriginalAuthor
            } else if username_lower == "mudler" {
                CommunitySourceRole::Curator
            } else if username_lower == "davidau" {
                CommunitySourceRole::OriginalAuthor
            } else if is_mlx {
                CommunitySourceRole::MlxConverter
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
                CommunitySourceRole::Curator => {
                    if username_lower == "mudler" {
                        vec![CommunitySourceRole::GgufQuantizer]
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
        preferences: CommunitySourcePreferences {
            quantizer_styles,
            ..CommunitySourcePreferences::default()
        },
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

        let nightmedia = entries_for_username(&catalog, "nightmedia");
        assert_eq!(nightmedia.len(), 1);
        assert_eq!(nightmedia[0].role, CommunitySourceRole::MlxConverter);
    }

    #[test]
    fn to_quantizers_preserves_quant_styles() {
        let catalog = default_catalog();
        let q = to_quantizers(&catalog);
        let by_user: std::collections::HashMap<_, _> =
            q.iter().map(|u| (u.username.clone(), u)).collect();

        // Check key users (use actual catalog usernames, which preserve case)
        assert_eq!(by_user.get("bartowski").unwrap().quant_style, "standard");
        assert_eq!(by_user.get("mradermacher").unwrap().quant_style, "imatrix");
        assert_eq!(by_user.get("unsloth").unwrap().quant_style, "ud");
        assert_eq!(by_user.get("mlx-community").unwrap().quant_style, "mlx");
        assert_eq!(
            by_user.get("lmstudio-community").unwrap().quant_style,
            "mlx"
        );
        assert_eq!(by_user.get("nightmedia").unwrap().quant_style, "mlx");
        assert_eq!(by_user.get("llmfan46").unwrap().quant_style, "standard");
        assert_eq!(by_user.get("DavidAU").unwrap().quant_style, "standard");
        assert_eq!(by_user.get("mudler").unwrap().quant_style, "standard");
        assert_eq!(by_user.get("Jackrong").unwrap().quant_style, "standard");
        assert_eq!(
            by_user.get("prithivMLmods").unwrap().quant_style,
            "standard"
        );

        // Ensure we get all 11 quantizers from bundled entries
        assert!(q.len() >= 11, "to_quantizers returned too few: {}", q.len());
    }

    #[test]
    fn to_quantizers_includes_original_author_with_gguf_quantizer_role() {
        let catalog = default_catalog();
        let q = to_quantizers(&catalog);
        let usernames: Vec<_> = q.iter().map(|u| u.username.as_str()).collect();
        // unsloth (OriginalAuthor with also_known_for=[GgufQuantizer]) should be included
        assert!(
            usernames.contains(&"unsloth"),
            "unsloth should appear in quantizers"
        );
        // DavidAU (OriginalAuthor with also_known_for=[MergerDistiller, GgufQuantizer]) should
        // be included
        assert!(
            usernames.contains(&"DavidAU"),
            "DavidAU should appear in quantizers"
        );
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

    #[test]
    fn replace_quantizers_preserves_bundled_entries_and_explicit_style() {
        let mut catalog = default_catalog();
        let mut submitted = to_quantizers(&catalog);
        submitted.retain(|quantizer| quantizer.username == "bartowski");
        submitted[0].quant_style = "imatrix".into();

        replace_quantizers(&mut catalog, submitted).unwrap();

        assert_eq!(catalog.entries.len(), 11);
        assert!(entries_for_username(&catalog, "bartowski")[0].bundled);
        assert_eq!(to_quantizers(&catalog).len(), 1);
        assert_eq!(to_quantizers(&catalog)[0].quant_style, "imatrix");
        assert!(
            catalog
                .preferences
                .hidden_quantizers
                .contains(&"mradermacher".to_string())
        );
    }

    #[test]
    fn replace_quantizers_removes_omitted_user_entry_and_is_atomic_on_conflict() {
        let mut catalog = default_catalog();
        let mut submitted = to_quantizers(&catalog);
        submitted.push(crate::hf::UserQuantizer {
            username: "myuser".into(),
            display_name: "My User".into(),
            description: "Custom quantizer".into(),
            quant_style: "standard".into(),
            note: None,
        });
        replace_quantizers(&mut catalog, submitted).unwrap();
        assert_eq!(entries_for_username(&catalog, "myuser").len(), 1);

        let without_custom: Vec<_> = to_quantizers(&catalog)
            .into_iter()
            .filter(|quantizer| quantizer.username != "myuser")
            .collect();
        replace_quantizers(&mut catalog, without_custom).unwrap();
        assert!(entries_for_username(&catalog, "myuser").is_empty());

        catalog.entries.push(CommunitySourceEntry {
            username: "author-only".into(),
            display_name: "Author only".into(),
            description: "An original author".into(),
            role: CommunitySourceRole::OriginalAuthor,
            also_known_for: Vec::new(),
            categories: Vec::new(),
            note: None,
            bundled: false,
        });
        let before = serde_json::to_string(&catalog).unwrap();
        let result = replace_quantizers(
            &mut catalog,
            vec![crate::hf::UserQuantizer {
                username: "author-only".into(),
                display_name: "Author only".into(),
                description: "Should not overwrite role evidence".into(),
                quant_style: "standard".into(),
                note: None,
            }],
        );
        assert!(result.is_err());
        assert_eq!(serde_json::to_string(&catalog).unwrap(), before);
    }
}
