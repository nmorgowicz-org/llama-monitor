//! Where a downloaded model file came from.
//!
//! The library cards have had a lineage row since Phase 8B2, gated on `hf_repo_id ||
//! originRepo || repo_id`. None of those fields existed on `ModelInventoryEntry`, and a
//! live scan of all 68 real inventory entries found no sidecar on disk to supply them, so
//! the row had never rendered once. The read paths were real; the write path was missing.
//!
//! This is that write path. `/api/hf/download` knows the repo, the file inside it, and --
//! from Hugging Face's `X-Repo-Commit` response header -- the exact commit it resolved to.
//! All three are discarded the moment the stream closes unless something records them.
//!
//! One sidecar per directory rather than one per file: a single `gguf/` directory routinely
//! holds a dozen quants of the same model, and the inventory scan already walks directories,
//! so a per-directory map costs one read per directory instead of one per model.
//!
//! Deliberately *not* recorded here: which role the uploader holds. That lives in
//! [`crate::models::community_source_catalog`], is user-editable, and would be frozen at
//! download time if it were copied into the sidecar. The catalog is consulted at render
//! time so correcting a role fixes every model at once.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

/// Sidecar filename. Dot-prefixed so it does not show up as a model in directory listings,
/// and distinct from `.llama-monitor-source.json` (a single `RapidMlxModelSource` describing
/// a whole snapshot directory) and `llama-monitor-conversion.json` (an MLX conversion recipe).
pub const PROVENANCE_SIDECAR_NAME: &str = ".llama-monitor-provenance.json";

/// A sidecar past this size is treated as absent rather than parsed. It is a small map of
/// short strings; anything larger is either corrupt or hostile.
const MAX_SIDECAR_BYTES: u64 = 512 * 1024;

/// Cap on records kept per directory. Oldest downloads are dropped first, so a directory
/// that is churned through thousands of files keeps provenance for what is currently there.
const MAX_RECORDS_PER_DIR: usize = 1024;

/// Serializes the read-modify-write of any sidecar. Companion downloads (a model and its
/// mmproj) finish within milliseconds of each other into the same directory, and both call
/// `record_download`; without this the second write would drop the first.
static SIDECAR_WRITE_LOCK: Mutex<()> = Mutex::new(());

/// What a downloaded file's origin was, as known at the moment the download completed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProvenance {
    /// `owner/name` on Hugging Face.
    pub repo_id: String,

    /// The commit the download actually resolved to, from the `X-Repo-Commit` response
    /// header.
    ///
    /// `None` means Hugging Face did not send the header and the file came from whatever
    /// `main` pointed at during the download. That is a materially weaker claim than a
    /// pinned commit and callers must not present it as one -- it is the difference between
    /// "this file is that commit" and "this file was `main` at some point".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,

    /// Path of the file *inside* the repo. Differs from the local filename whenever the
    /// download used `save_as`, or whenever the file lived in a repo subdirectory, so it
    /// cannot be reconstructed from the local name.
    pub remote_path: String,

    /// Unix seconds at completion. Not the file mtime, which a later touch or a copy
    /// between directories would overwrite.
    pub downloaded_at: u64,

    /// Bytes written. Recorded so a truncated or later-replaced file can be spotted by
    /// comparing against the file actually on disk.
    pub size_bytes: u64,
}

impl DownloadProvenance {
    /// Canonical Hugging Face URL for this exact file, pinned to the recorded commit when
    /// there is one. This is what a "view source" link should point at.
    pub fn source_url(&self) -> String {
        crate::hf::hf_resolve_download_url_at(
            &self.repo_id,
            &self.remote_path,
            self.revision.as_deref().unwrap_or("main"),
        )
    }

}

/// What the inventory API sends to the browser.
///
/// Separate from the on-disk record for the same reason [`RapidMlxModelSourceView`] is: the
/// wire shape carries derived fields (`sourceUrl`, `pinned`) that must not be written into
/// the sidecar, where they would go stale the moment the derivation changed.
///
/// [`RapidMlxModelSourceView`]: crate::inference::rapid_mlx::model_resolver::RapidMlxModelSourceView
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProvenanceView {
    pub repo_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    pub remote_path: String,
    pub downloaded_at: u64,
    pub size_bytes: u64,
    /// Link to the exact file, pinned to the recorded commit when there is one.
    pub source_url: String,
    /// Whether `revision` is a real commit rather than an unrecorded branch tip. The UI must
    /// not present an unpinned model as reproducible, so this is stated rather than left to
    /// be inferred from `revision` being absent.
    pub pinned: bool,
}

impl From<&DownloadProvenance> for DownloadProvenanceView {
    fn from(record: &DownloadProvenance) -> Self {
        Self {
            repo_id: record.repo_id.clone(),
            revision: record.revision.clone(),
            remote_path: record.remote_path.clone(),
            downloaded_at: record.downloaded_at,
            size_bytes: record.size_bytes,
            source_url: record.source_url(),
            pinned: record.revision.is_some(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProvenanceSidecar {
    version: u32,
    /// Local filename (not path) -> provenance. Keyed by the name on disk because that is
    /// what the inventory scan has in hand.
    entries: BTreeMap<String, DownloadProvenance>,
}

impl Default for ProvenanceSidecar {
    fn default() -> Self {
        Self {
            version: 1,
            entries: BTreeMap::new(),
        }
    }
}

/// Read every provenance record in `dir`.
///
/// A missing, oversized, or unparseable sidecar yields an empty map rather than an error:
/// provenance decorates a model card, and a corrupt sidecar must not stop the library from
/// listing models.
pub fn load_directory(dir: &Path) -> BTreeMap<String, DownloadProvenance> {
    read_sidecar(&dir.join(PROVENANCE_SIDECAR_NAME))
        .unwrap_or_default()
        .entries
}

/// Origin of a model *directory* -- an MLX model, say, downloaded file by file into one
/// folder.
///
/// Returns a record only when every file in the directory came from the same repo. A
/// directory holding files from two repos has no single origin, and picking one of them
/// would put a confident, wrong lineage on the card; the honest answer there is none. The
/// most recent download wins on the remaining fields, since that is what the directory
/// currently reflects.
pub fn directory_origin(dir: &Path) -> Option<DownloadProvenance> {
    let entries = load_directory(dir);
    let mut newest: Option<DownloadProvenance> = None;
    for record in entries.into_values() {
        match &newest {
            Some(current) if current.repo_id != record.repo_id => return None,
            Some(current) if current.downloaded_at >= record.downloaded_at => {}
            _ => newest = Some(record),
        }
    }
    newest
}

/// Record where `filename` in `dir` came from, merging into any existing sidecar.
///
/// Overwrites an existing record for the same filename: re-downloading a file replaces it on
/// disk, so the old origin is no longer true of what is there.
pub fn record_download(
    dir: &Path,
    filename: &str,
    provenance: DownloadProvenance,
) -> Result<()> {
    if filename.is_empty() || filename.contains('/') || filename.contains('\\') {
        return Err(anyhow!(
            "provenance key must be a plain filename, got {filename:?}"
        ));
    }

    let path = dir.join(PROVENANCE_SIDECAR_NAME);
    let _guard = SIDECAR_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // Read under the lock: reading before taking it would reintroduce the lost-update race
    // the lock exists to prevent.
    let mut sidecar = read_sidecar(&path).unwrap_or_default();
    sidecar.entries.insert(filename.to_string(), provenance);

    while sidecar.entries.len() > MAX_RECORDS_PER_DIR {
        // Drop the oldest download. `BTreeMap` orders by filename, so the key has to be
        // found by scanning the values.
        let Some(oldest) = sidecar
            .entries
            .iter()
            .min_by_key(|(_, record)| record.downloaded_at)
            .map(|(name, _)| name.clone())
        else {
            break;
        };
        sidecar.entries.remove(&oldest);
    }

    write_sidecar(&path, &sidecar)
}

/// Forget a file's provenance -- used when the file itself is deleted, so a later download
/// of a different file under the same name does not inherit a stale origin.
pub fn forget(dir: &Path, filename: &str) -> Result<()> {
    let path = dir.join(PROVENANCE_SIDECAR_NAME);
    let _guard = SIDECAR_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let mut sidecar = read_sidecar(&path).unwrap_or_default();
    if sidecar.entries.remove(filename).is_none() {
        return Ok(());
    }
    if sidecar.entries.is_empty() {
        // Leaving an empty sidecar behind would make the directory look tracked when it
        // holds nothing.
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    write_sidecar(&path, &sidecar)
}

fn read_sidecar(path: &Path) -> Option<ProvenanceSidecar> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    // A symlinked sidecar would let a file placed inside the models directory redirect the
    // read anywhere on disk.
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return None;
    }
    if metadata.len() > MAX_SIDECAR_BYTES {
        return None;
    }
    serde_json::from_reader(std::fs::File::open(path).ok()?).ok()
}

fn write_sidecar(path: &Path, sidecar: &ProvenanceSidecar) -> Result<()> {
    use std::io::Write;

    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("provenance sidecar path has no parent"))?;
    std::fs::create_dir_all(parent)?;

    // Write-then-rename: a crash mid-write must not leave a truncated sidecar that would
    // silently read back as "no provenance for anything in this directory".
    let tmp = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&tmp)?;
    serde_json::to_writer_pretty(&mut file, sidecar)?;
    file.flush()?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(repo: &str, remote: &str, at: u64) -> DownloadProvenance {
        DownloadProvenance {
            repo_id: repo.into(),
            revision: Some("abc1234def5678".into()),
            remote_path: remote.into(),
            downloaded_at: at,
            size_bytes: 4_096,
        }
    }

    #[test]
    fn records_and_reads_back() {
        let tmp = tempfile::tempdir().expect("tempdir");
        record_download(
            tmp.path(),
            "model-Q4_K_M.gguf",
            sample("bartowski/Qwen3-8B-GGUF", "Qwen3-8B-Q4_K_M.gguf", 100),
        )
        .expect("record");

        let found = load_directory(tmp.path())
            .remove("model-Q4_K_M.gguf")
            .expect("recorded");
        assert_eq!(found.repo_id, "bartowski/Qwen3-8B-GGUF");
        // save_as renamed the file locally; the remote path is the only way back to it.
        assert_eq!(found.remote_path, "Qwen3-8B-Q4_K_M.gguf");
    }

    #[test]
    fn a_second_download_into_the_same_directory_keeps_the_first() {
        let tmp = tempfile::tempdir().expect("tempdir");
        record_download(tmp.path(), "model.gguf", sample("a/b", "model.gguf", 100)).expect("first");
        record_download(tmp.path(), "mmproj.gguf", sample("a/b", "mmproj.gguf", 101))
            .expect("second");

        let all = load_directory(tmp.path());
        assert_eq!(all.len(), 2, "companion download dropped the model's record");
    }

    #[test]
    fn redownloading_replaces_the_stale_origin() {
        let tmp = tempfile::tempdir().expect("tempdir");
        record_download(tmp.path(), "model.gguf", sample("old/repo", "model.gguf", 100))
            .expect("first");
        record_download(tmp.path(), "model.gguf", sample("new/repo", "model.gguf", 200))
            .expect("second");

        let found = load_directory(tmp.path()).remove("model.gguf").expect("recorded");
        assert_eq!(found.repo_id, "new/repo");
        assert_eq!(load_directory(tmp.path()).len(), 1);
    }

    #[test]
    fn missing_sidecar_is_empty_not_an_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(load_directory(tmp.path()).is_empty());
        assert!(directory_origin(tmp.path()).is_none());
    }

    #[test]
    fn corrupt_sidecar_does_not_break_the_library() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(PROVENANCE_SIDECAR_NAME), b"{not json")
            .expect("write garbage");

        assert!(load_directory(tmp.path()).is_empty());
        // And it must still be recoverable by recording again, not permanently poisoned.
        record_download(tmp.path(), "model.gguf", sample("a/b", "model.gguf", 100))
            .expect("record over garbage");
        assert!(load_directory(tmp.path()).contains_key("model.gguf"));
    }

    #[test]
    fn rejects_a_path_as_the_filename_key() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(
            record_download(tmp.path(), "../escape.gguf", sample("a/b", "x", 1)).is_err(),
            "a key containing a path separator would let one directory's sidecar describe another"
        );
    }

    #[test]
    fn forget_removes_one_record_and_deletes_an_emptied_sidecar() {
        let tmp = tempfile::tempdir().expect("tempdir");
        record_download(tmp.path(), "a.gguf", sample("a/b", "a.gguf", 1)).expect("a");
        record_download(tmp.path(), "b.gguf", sample("a/b", "b.gguf", 2)).expect("b");

        forget(tmp.path(), "a.gguf").expect("forget a");
        assert_eq!(load_directory(tmp.path()).len(), 1);

        forget(tmp.path(), "b.gguf").expect("forget b");
        assert!(!tmp.path().join(PROVENANCE_SIDECAR_NAME).exists());
        // Forgetting something that was never recorded is not an error.
        forget(tmp.path(), "b.gguf").expect("idempotent");
    }

    #[test]
    fn source_url_pins_to_the_recorded_commit() {
        let record = sample("bartowski/Qwen3-8B-GGUF", "Qwen3-8B-Q4_K_M.gguf", 1);
        assert!(record.source_url().contains("/resolve/abc1234def5678/"));

        let unpinned = DownloadProvenance {
            revision: None,
            ..record
        };
        // No recorded commit means the link can only point at the branch.
        assert!(unpinned.source_url().contains("/resolve/main/"));
    }

    #[test]
    fn a_directory_with_files_from_two_repos_has_no_single_origin() {
        let tmp = tempfile::tempdir().expect("tempdir");
        record_download(tmp.path(), "a.safetensors", sample("mlx-community/Qwen3-8B-4bit", "a", 1))
            .expect("a");
        assert_eq!(
            directory_origin(tmp.path()).map(|p| p.repo_id),
            Some("mlx-community/Qwen3-8B-4bit".to_string()),
        );

        record_download(tmp.path(), "b.safetensors", sample("someone/else", "b", 2)).expect("b");
        assert!(
            directory_origin(tmp.path()).is_none(),
            "picking one of two repos would put a confident, wrong lineage on the card",
        );
    }

    #[test]
    fn directory_origin_reports_the_most_recent_download() {
        let tmp = tempfile::tempdir().expect("tempdir");
        record_download(tmp.path(), "a.safetensors", sample("a/b", "a", 100)).expect("a");
        record_download(tmp.path(), "z.safetensors", sample("a/b", "z", 500)).expect("z");
        // Alphabetically last but chronologically last too -- and the reverse case:
        record_download(tmp.path(), "m.safetensors", sample("a/b", "m", 300)).expect("m");

        assert_eq!(directory_origin(tmp.path()).expect("origin").remote_path, "z");
    }

    #[test]
    fn the_view_marks_an_unpinned_download_as_such() {
        let record = sample("a/b", "model.gguf", 1);
        assert!(DownloadProvenanceView::from(&record).pinned);

        let unpinned = DownloadProvenance { revision: None, ..record };
        let view = DownloadProvenanceView::from(&unpinned);
        assert!(!view.pinned, "a branch-tip download must not read as reproducible");
        assert!(view.source_url.contains("/resolve/main/"));
    }

    #[test]
    fn a_symlinked_sidecar_is_ignored() {
        #[cfg(unix)]
        {
            let tmp = tempfile::tempdir().expect("tempdir");
            let elsewhere = tmp.path().join("elsewhere.json");
            std::fs::write(&elsewhere, br#"{"version":1,"entries":{}}"#).expect("write");
            let dir = tmp.path().join("models");
            std::fs::create_dir_all(&dir).expect("dir");
            std::os::unix::fs::symlink(&elsewhere, dir.join(PROVENANCE_SIDECAR_NAME))
                .expect("symlink");

            assert!(load_directory(&dir).is_empty());
        }
    }
}
