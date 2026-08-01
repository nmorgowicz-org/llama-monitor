//! Persistent cache for MTP companion model pin resolutions.
//!
//! Resolves an external `owner/repo` to its immutable commit `sha` and records
//! whether `trust_remote_code` is required. The cache avoids redundant HF API
//! calls on every preflight while still allowing forced re-checks when the
//! upstream repo may have changed.
//!
//! The pin cache is stored under the same config directory as other app state.
//! A cached pin is considered valid until explicitly invalidated or until the
//! upstream sha has changed (re-checked at launch time).

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Cache file name under the config directory.
const PIN_CACHE_FILE: &str = "mtp-pins.json";

/// Schema version — bump only for a format change that older readers cannot
/// understand. A cache written by a newer schema is ignored rather than guessed.
const SCHEMA_VERSION: u32 = 1;

/// Refuse absurdly large caches.
const MAX_CACHE_BYTES: u64 = 128 * 1024;

/// How long a pin is considered "fresh" before a re-check is recommended.
/// After this duration the UI can suggest a re-check, but the pin is not
/// invalidated — only an upstream sha change invalidates it.
const PIN_FRESHNESS_HOURS: u64 = 24;

/// A resolved and pinned MTP companion model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtpPin {
    /// The Hugging Face repo id this pin belongs to.
    pub repo_id: String,
    /// The immutable commit sha the repo was resolved to.
    pub revision: String,
    /// Whether this repo requires `trust_remote_code` to load.
    pub trust_remote_code_required: bool,
    /// ISO-8601 timestamp when this pin was last resolved from HF.
    pub resolved_at: String,
    /// ISO-8601 timestamp when this pin was last re-checked against upstream.
    #[serde(default)]
    pub last_recheck_at: String,
    /// Whether the last re-check found the upstream sha unchanged.
    /// `None` means never re-checked (first resolution).
    #[serde(default)]
    pub upstream_unchanged: Option<bool>,
    /// Rough estimate of companion memory in bytes (Q4 quantization).
    #[serde(default)]
    pub estimated_memory_bytes: Option<u64>,
}

impl MtpPin {
    /// Returns true if this pin is considered stale (older than freshness window).
    /// This does not invalidate the pin — it just means a re-check is recommended.
    pub fn is_stale(&self) -> bool {
        chrono::DateTime::parse_from_rfc3339(&self.resolved_at)
            .map(|dt| {
                chrono::Utc::now()
                    .signed_duration_since(dt.with_timezone(&chrono::Utc))
                    .num_hours()
                    >= PIN_FRESHNESS_HOURS as i64
            })
            .unwrap_or(true)
    }
}

/// On-disk shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct PinCacheFile {
    schema_version: u32,
    pins: BTreeMap<String, MtpPin>,
}

/// Reader/writer for the MTP pin cache.
#[derive(Debug, Clone)]
pub struct MtpPinCache {
    path: PathBuf,
}

impl MtpPinCache {
    pub fn at(root: &Path) -> Self {
        Self {
            path: root.join(PIN_CACHE_FILE),
        }
    }

    /// Look up a cached pin for the given repo id.
    pub fn get(&self, repo_id: &str) -> Option<MtpPin> {
        self.load().ok()?.pins.get(repo_id).cloned()
    }

    /// Insert or update a pin for the given repo id.
    pub fn insert(&self, pin: MtpPin) -> Result<()> {
        let mut file = self.load().unwrap_or_default();
        file.schema_version = SCHEMA_VERSION;
        file.pins.insert(pin.repo_id.clone(), pin);
        self.write(&file)
    }

    /// Remove a pin from the cache.
    pub fn remove(&self, repo_id: &str) -> Result<()> {
        let mut file = self.load().unwrap_or_default();
        file.pins.remove(repo_id);
        self.write(&file)
    }

    /// Return all cached pins.
    pub fn all(&self) -> Vec<MtpPin> {
        self.load()
            .ok()
            .map(|f| f.pins.into_values().collect())
            .unwrap_or_default()
    }

    /// Re-check a pin against upstream: re-resolve the repo via HF API and
    /// compare the sha. If the sha has changed, update the pin with the new
    /// revision and set `upstream_unchanged` to `false`. If unchanged, update
    /// `last_recheck_at` and set `upstream_unchanged` to `true`.
    pub async fn recheck(&self, repo_id: &str) -> Result<MtpPin> {
        let old_pin = self
            .get(repo_id)
            .ok_or_else(|| anyhow::anyhow!("no cached pin for {repo_id}"))?;

        // Re-resolve via the HF API (same as the initial resolution).
        let fresh = crate::hf::resolve_speculative_model_preflight(repo_id)
            .await
            .map_err(|e| anyhow::anyhow!("re-check failed: {e}"))?;

        let upstream_unchanged = fresh.revision == old_pin.revision;

        let mut new_pin = old_pin.clone();
        new_pin.last_recheck_at = chrono::Utc::now().to_rfc3339();
        new_pin.upstream_unchanged = Some(upstream_unchanged);

        // If the upstream sha changed, update the pin with the new revision.
        if !upstream_unchanged {
            new_pin.revision = fresh.revision.clone();
            new_pin.trust_remote_code_required = fresh.trust_remote_code_required;
            new_pin.resolved_at = chrono::Utc::now().to_rfc3339();
        }

        self.insert(new_pin.clone())?;
        Ok(new_pin)
    }

    fn load(&self) -> Result<PinCacheFile> {
        let meta = std::fs::metadata(&self.path)?;
        if meta.len() > MAX_CACHE_BYTES {
            bail!(
                "{} is {} bytes, too large to be a pin cache",
                self.path.display(),
                meta.len()
            );
        }
        let raw = std::fs::read_to_string(&self.path)?;
        let file: PinCacheFile = serde_json::from_str(&raw)?;
        if file.schema_version > SCHEMA_VERSION {
            bail!(
                "{} was written by schema {} but this build understands {SCHEMA_VERSION}",
                self.path.display(),
                file.schema_version
            );
        }
        Ok(file)
    }

    fn write(&self, file: &PinCacheFile) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Cannot create pin cache directory {}", parent.display())
            })?;
        }
        let temp = self.path.with_extension("json.tmp");
        let body = serde_json::to_string_pretty(file)?;
        std::fs::write(&temp, format!("{body}\n"))
            .with_context(|| format!("Cannot write {}", temp.display()))?;
        std::fs::rename(&temp, &self.path)
            .with_context(|| format!("Cannot install {}", self.path.display()))?;
        Ok(())
    }
}

/// Process-wide pin cache instance. Set once from the resolved config directory
/// at startup so that `--config-dir` is honoured.
static PIN_CACHE: OnceLock<MtpPinCache> = OnceLock::new();

/// Initialize the pin cache at the given config directory.
pub fn init_pin_cache(config_dir: &Path) {
    let _ = PIN_CACHE.set(MtpPinCache::at(config_dir));
}

/// Get the process-wide pin cache.
///
/// Panics if called before `init_pin_cache`. Use only after startup has
/// initialized the cache (see `main.rs`).
pub fn pin_cache() -> &'static MtpPinCache {
    PIN_CACHE
        .get()
        .expect("MTP pin cache not initialized; call init_pin_cache() at startup")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cache() -> MtpPinCache {
        let dir = tempfile::tempdir().unwrap();
        MtpPinCache::at(dir.path())
    }

    #[test]
    fn absent_cache_yields_no_pin() {
        let cache = test_cache();
        assert!(cache.get("org/model").is_none());
    }

    #[test]
    fn insert_and_get_pin() {
        let cache = test_cache();
        let pin = MtpPin {
            repo_id: "org/model".into(),
            revision: "abc123".into(),
            trust_remote_code_required: true,
            resolved_at: "2026-08-01T00:00:00Z".into(),
            estimated_memory_bytes: None,
            last_recheck_at: "".into(),
            upstream_unchanged: None,
        };
        cache.insert(pin.clone()).unwrap();
        let got = cache.get("org/model").unwrap();
        assert_eq!(got.repo_id, "org/model");
        assert_eq!(got.revision, "abc123");
        assert!(got.trust_remote_code_required);
    }

    #[test]
    fn remove_pin() {
        let cache = test_cache();
        let pin = MtpPin {
            repo_id: "org/model".into(),
            revision: "abc123".into(),
            trust_remote_code_required: false,
            resolved_at: "2026-08-01T00:00:00Z".into(),
            estimated_memory_bytes: None,
            last_recheck_at: "".into(),
            upstream_unchanged: None,
        };
        cache.insert(pin).unwrap();
        assert!(cache.get("org/model").is_some());
        cache.remove("org/model").unwrap();
        assert!(cache.get("org/model").is_none());
    }

    #[test]
    fn pin_stale_after_24_hours() {
        let pin = MtpPin {
            repo_id: "org/model".into(),
            revision: "abc123".into(),
            trust_remote_code_required: false,
            resolved_at: chrono::Utc::now()
                .checked_sub_days(chrono::Days::new(1))
                .unwrap()
                .to_rfc3339(),
            last_recheck_at: "".into(),
            upstream_unchanged: None,
            estimated_memory_bytes: None,
        };
        assert!(pin.is_stale());
    }

    #[test]
    fn pin_fresh_within_24_hours() {
        let pin = MtpPin {
            repo_id: "org/model".into(),
            revision: "abc123".into(),
            trust_remote_code_required: false,
            resolved_at: (chrono::Utc::now() - chrono::Duration::hours(12)).to_rfc3339(),
            last_recheck_at: "".into(),
            upstream_unchanged: None,
            estimated_memory_bytes: None,
        };
        assert!(!pin.is_stale());
    }

    #[test]
    fn all_returns_all_pins() {
        let cache = test_cache();
        let pin1 = MtpPin {
            repo_id: "org/model1".into(),
            revision: "abc123".into(),
            trust_remote_code_required: false,
            resolved_at: "2026-08-01T00:00:00Z".into(),
            estimated_memory_bytes: None,
            last_recheck_at: "".into(),
            upstream_unchanged: None,
        };
        let pin2 = MtpPin {
            repo_id: "org/model2".into(),
            revision: "def456".into(),
            trust_remote_code_required: true,
            resolved_at: "2026-08-01T00:00:00Z".into(),
            estimated_memory_bytes: None,
            last_recheck_at: "".into(),
            upstream_unchanged: None,
        };
        cache.insert(pin1).unwrap();
        cache.insert(pin2).unwrap();
        let all = cache.all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn future_schema_cache_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let cache = MtpPinCache::at(dir.path());
        std::fs::write(
            cache.path.clone(),
            format!(
                "{{\"schema_version\":{},\"pins\":{{\"org/model\":{{\"repo_id\":\"org/model\",\"revision\":\"abc\",\"trust_remote_code_required\":false,\"resolved_at\":\"2026-01-01T00:00:00Z\"}}}}}}",
                SCHEMA_VERSION + 1
            ),
        )
        .unwrap();
        assert!(cache.get("org/model").is_none());
    }
}
