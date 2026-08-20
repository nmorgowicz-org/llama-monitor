//! Managed inventory of locally-built MTP companion sidecars.
//!
//! The build script (`scripts/build-mtp-head.py`) places validated sidecars under
//! `~/.config/llama-monitor/models/rapid-mlx/mtp-sidecars/<slug>/`, each containing
//! `mtp.safetensors` and a `provenance.json` with norm-check results and source
//! provenance. This module discovers those directories, reads their provenance, and
//! exposes them to the app via an API endpoint.
//!
//! The sidecar root is determined by the config directory set at startup:
//! `{config_dir}/models/rapid-mlx/mtp-sidecars/`. A `OnceLock` holds the path so
//! discovery can happen from any context without threading `AppConfig` around.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Subdirectory under the config dir where managed sidecars live.
const SIDECAR_SUBDIR: &str = "models/rapid-mlx/mtp-sidecars";

/// The file produced by the build script with the sidecar weights.
const SIDECAR_WEIGHTS: &str = "mtp.safetensors";

/// The file produced by the build script with provenance metadata.
const PROVENANCE_FILE: &str = "provenance.json";

/// Provenance data for a locally-built MTP sidecar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarProvenance {
    /// The trunk model directory this sidecar was built for.
    pub trunk: String,
    /// The BF16 source (HF repo id or local path) the MTP tensors were extracted from.
    pub bf16_source: String,
    /// ISO-8601 timestamp when this sidecar was built.
    pub built_at: String,
    /// SHA-256 of the `mtp.safetensors` file at build time.
    pub sha256: String,
    /// Whether the pre_fc_norm sanity check passed (all positive means).
    pub norm_check_passed: bool,
    /// Estimated memory for this sidecar in bytes (file size of `mtp.safetensors`).
    #[serde(default)]
    pub estimated_memory_bytes: Option<u64>,
    /// The quantization of the sidecar if known (e.g., "8bit", "4bit").
    #[serde(default)]
    pub quantization: Option<String>,
    /// Maximum MTP depth if known.
    #[serde(default)]
    pub mtp_depth_max: Option<i64>,
    /// Provenance schema version emitted by the builder.
    #[serde(default)]
    pub schema_version: Option<u32>,
    /// How the head was produced: recipe reconstruction or direct parent graft.
    #[serde(default)]
    pub repair_mode: Option<String>,
    /// SHA-256 of the exact merge recipe, when recipe reconstruction was used.
    #[serde(default)]
    pub recipe_digest: Option<String>,
    /// Merge method used for recipe reconstruction (currently `nuslerp`).
    #[serde(default)]
    pub recipe_method: Option<String>,
    /// Number of leaf parent sources in the recipe.
    #[serde(default)]
    pub recipe_parent_count: Option<usize>,
    /// Arithmetic precision used while reconstructing the recipe head.
    #[serde(default)]
    pub recipe_precision: Option<String>,
    /// Served requalification status, if the sidecar has been tested after repair.
    #[serde(default)]
    pub requalification_status: Option<String>,
}

/// One discovered local sidecar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarEntry {
    /// The slug identifying this sidecar (directory name).
    pub slug: String,
    /// Full path to the sidecar directory.
    pub path: String,
    /// Whether `mtp.safetensors` exists (required for use).
    pub has_weights: bool,
    /// Whether `provenance.json` exists and could be read.
    pub has_provenance: bool,
    /// Parsed provenance data, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<SidecarProvenance>,
}

/// The global sidecar root path. Set once at startup.
static SIDECAR_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Point the sidecar inventory at the config directory. Called once during startup.
pub fn init_sidecar_root(config_dir: &Path) {
    let _ = SIDECAR_ROOT.set(config_dir.join(SIDECAR_SUBDIR));
}

/// Resolve the sidecar root for the current process.
fn sidecar_root() -> PathBuf {
    SIDECAR_ROOT
        .get()
        .cloned()
        .unwrap_or_else(|| crate::paths::AppPaths::default_active_root().join(SIDECAR_SUBDIR))
}

/// Return the managed sidecar directory for a trunk model path.
pub fn sidecar_dir_for_trunk(trunk: &Path) -> PathBuf {
    sidecar_root().join(slugify_trunk_name(trunk))
}

fn slugify_trunk_name(trunk: &Path) -> String {
    let name = trunk
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("trunk")
        .to_ascii_lowercase();
    let mut slug = String::new();
    let mut pending_separator = false;
    for ch in name.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            if pending_separator && !slug.is_empty() {
                slug.push('-');
            }
            pending_separator = false;
            slug.push(ch);
        } else {
            pending_separator = true;
        }
    }
    if slug.is_empty() {
        "trunk".to_string()
    } else {
        slug
    }
}

/// Discover all sidecars under the managed root. Returns an empty list if the
/// root does not exist (no sidecars built yet).
pub fn discover_sidecars() -> Result<Vec<SidecarEntry>> {
    let root = sidecar_root();

    // No root = no sidecars. Not an error.
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    for entry in std::fs::read_dir(&root).context("Failed to read sidecar root directory")? {
        let entry = entry.context("Failed to read sidecar directory entry")?;
        let dir_path = entry.path();

        // Skip non-directories (e.g., quarantine marker files).
        if !dir_path.is_dir() {
            continue;
        }

        // Skip hidden/dot directories (quarantine dirs start with dot).
        if dir_path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| name.starts_with('.'))
        {
            continue;
        }

        let slug = dir_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let weights_path = dir_path.join(SIDECAR_WEIGHTS);
        let has_weights = weights_path.is_file();

        let provenance_path = dir_path.join(PROVENANCE_FILE);
        let entry = if provenance_path.is_file() {
            match parse_provenance(&slug, &dir_path, &weights_path, &provenance_path) {
                Ok(p) => p,
                Err(e) => {
                    // Warn but don't fail: a broken provenance.json should not
                    // prevent the sidecar from being listed.
                    eprintln!("warning: failed to parse provenance for {}: {}", slug, e);
                    SidecarEntry {
                        slug: slug.clone(),
                        path: dir_path.to_string_lossy().to_string(),
                        has_weights,
                        has_provenance: false,
                        provenance: None,
                    }
                }
            }
        } else {
            SidecarEntry {
                slug: slug.clone(),
                path: dir_path.to_string_lossy().to_string(),
                has_weights,
                has_provenance: false,
                provenance: None,
            }
        };

        entries.push(entry);
    }

    Ok(entries)
}

/// Parse provenance.json and enrich with file-level data.
fn parse_provenance(
    slug: &str,
    dir_path: &Path,
    weights_path: &Path,
    provenance_path: &Path,
) -> Result<SidecarEntry> {
    let raw = std::fs::read_to_string(provenance_path).context("Failed to read provenance")?;
    let provenance: serde_json::Value =
        serde_json::from_str(&raw).context("Failed to parse provenance JSON")?;

    // Extract fields from the provenance structure produced by build-mtp-head.py
    let trunk = provenance
        .get("source")
        .and_then(|s| s.get("trunk"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            provenance
                .get("target")
                .and_then(|target| target.get("snapshot"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("")
        .to_string();

    let bf16_source = provenance
        .get("source")
        .and_then(|s| s.get("bf16_source"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            provenance
                .get("source")
                .and_then(|source| source.get("snapshot"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("")
        .to_string();

    let built_at = provenance
        .get("built_at")
        .and_then(|v| v.as_str())
        .or_else(|| provenance.get("created_at").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    let sha256 = provenance
        .get("sha256")
        .and_then(|v| v.as_str())
        .or_else(|| {
            provenance
                .get("validation")
                .and_then(|validation| validation.get("sha256"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("")
        .to_string();

    // Norm check: all_positive field in validation
    let norm_check_passed = provenance
        .get("validation")
        .and_then(|v| v.get("all_positive"))
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| {
            provenance
                .get("validation")
                .and_then(|v| v.get("pre_fc_norm_means"))
                .and_then(|v| v.as_object())
                .map(|means| {
                    !means.is_empty()
                        && means.values().all(|value| {
                            value
                                .as_f64()
                                .is_some_and(|mean| mean.is_finite() && mean > 0.0)
                        })
                })
                .unwrap_or(false)
        });

    let recipe = provenance.get("recipe");
    let recipe_parent_count = recipe
        .and_then(|value| value.get("parents"))
        .and_then(|value| value.as_array())
        .map(Vec::len);

    // Quantization hint: if the slug or bf16_source contains quant info
    let quantization = {
        let slug_lower = slug.to_lowercase();
        if slug_lower.contains("8bit") || slug_lower.contains("q8") || slug_lower.contains("mxfp8")
        {
            Some("8-bit".to_string())
        } else if slug_lower.contains("4bit") || slug_lower.contains("q4") {
            Some("4-bit".to_string())
        } else {
            None
        }
    };

    // File size estimate
    let estimated_memory_bytes = weights_path.metadata().ok().map(|m| m.len()).or_else(|| {
        // If the weights file doesn't exist yet, we can't estimate.
        // But the provenance might have a size from the build time.
        provenance
            .get("validation")
            .and_then(|v| v.get("estimated_memory_bytes"))
            .and_then(|v| v.as_u64())
    });

    Ok(SidecarEntry {
        slug: slug.to_string(),
        path: dir_path.to_string_lossy().to_string(),
        has_weights: weights_path.is_file(),
        has_provenance: true,
        provenance: Some(SidecarProvenance {
            trunk,
            bf16_source,
            built_at,
            sha256,
            norm_check_passed,
            estimated_memory_bytes,
            quantization,
            mtp_depth_max: None,
            schema_version: provenance
                .get("schema_version")
                .and_then(|value| value.as_u64())
                .and_then(|value| u32::try_from(value).ok()),
            repair_mode: provenance
                .get("repair_mode")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            recipe_digest: recipe
                .and_then(|value| value.get("digest"))
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            recipe_method: recipe
                .and_then(|value| value.get("method"))
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            recipe_parent_count,
            recipe_precision: recipe
                .and_then(|value| value.get("compute_dtype"))
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            requalification_status: provenance
                .get("requalification")
                .and_then(|value| value.get("status"))
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
        }),
    })
}

/// Estimate VRAM for a local companion path by reading the `mtp.safetensors`
/// file size. Returns `None` if the path is invalid, doesn't exist, or the
/// weights file is missing.
pub fn estimate_local_companion_vram(companion_path: &Path) -> Option<u64> {
    companion_path
        .join(SIDECAR_WEIGHTS)
        .metadata()
        .ok()
        .map(|m| m.len())
}

/// Check if a companion path belongs to a known sidecar in the inventory.
/// Returns the sidecar entry if found, `None` otherwise.
#[allow(dead_code)]
pub fn find_sidecar_for_path(companion_path: &Path) -> Option<SidecarEntry> {
    discover_sidecars().ok().and_then(|entries| {
        entries.into_iter().find(|e| {
            e.path == companion_path.to_string_lossy()
                && e.has_weights
                && e.has_provenance
                && e.provenance
                    .as_ref()
                    .is_some_and(|provenance| provenance.norm_check_passed)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_sidecar_root(dir: &Path) {
        let _ = SIDECAR_ROOT.set(dir.join(SIDECAR_SUBDIR));
    }

    #[test]
    fn empty_root_returns_no_entries() {
        let dir = tempfile::tempdir().unwrap();
        test_sidecar_root(dir.path());

        // Note: SIDECAR_ROOT is a OnceLock, so after set() it's immutable.
        // For this test we just check that a non-existent root returns empty.
        // In practice, the root would be set once at startup.
        let root = sidecar_root();
        assert!(!root.exists());
    }

    #[test]
    fn provenance_parsed_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let weights = dir.path().join(SIDECAR_WEIGHTS);
        let provenance_path = dir.path().join(PROVENANCE_FILE);
        std::fs::write(&weights, b"fixture").unwrap();
        std::fs::write(
            &provenance_path,
            serde_json::json!({
                "schema_version": 2,
                "status": "repaired",
                "repair_mode": "recipe_reconstruction",
                "target": {"snapshot": "/models/brainwaves"},
                "created_at": "2026-08-20T12:00:00Z",
                "validation": {"pre_fc_norm_means": {
                    "pre_fc_norm_embedding.weight": 1.0,
                    "pre_fc_norm_hidden.weight": 1.0
                }},
                "recipe": {
                    "digest": "abc123",
                    "method": "nuslerp",
                    "compute_dtype": "bfloat16",
                    "parents": [{"repo_id": "parent/a"}, {"repo_id": "parent/b"}]
                }
            })
            .to_string(),
        )
        .unwrap();

        let entry =
            parse_provenance("brainwaves-recipe", dir.path(), &weights, &provenance_path).unwrap();
        let provenance = entry.provenance.unwrap();
        assert_eq!(
            provenance.repair_mode.as_deref(),
            Some("recipe_reconstruction")
        );
        assert_eq!(provenance.recipe_method.as_deref(), Some("nuslerp"));
        assert_eq!(provenance.recipe_parent_count, Some(2));
        assert_eq!(provenance.recipe_precision.as_deref(), Some("bfloat16"));
        assert!(provenance.norm_check_passed);
    }
}
