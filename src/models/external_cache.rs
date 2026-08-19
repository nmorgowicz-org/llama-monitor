//! Audit of Hugging Face caches that sit *outside* the managed model library.
//!
//! The app points its own downloads at `<models_dir>/cache/huggingface/hub`, but a machine
//! that has ever run `hf download`, `mlx_lm.convert`, or a bare `transformers` script also
//! has a user-wide `~/.cache/huggingface/hub`, and that one grows without anything in the
//! app knowing. On this developer's box it reached 337 GB while the managed cache held
//! 1.3 GB.
//!
//! This module answers the only questions that make that number actionable: what is in
//! there, how big is each repo, is it already in the library, and what kind of model is
//! it. [`audit`] itself never mutates — an audit that also deletes is an audit nobody can
//! safely run twice — so reclaiming space is a separate, explicit call ([`remove_repo`])
//! that a caller has to mean.
//!
//! Every judgement carries where it came from. A `kind` read out of `config.json` and a
//! `kind` guessed from a repo name are not the same claim, and a caller deciding what to
//! delete deserves to know which one it is looking at.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Repos scanned per audit. Well past any real cache; a guard against a directory that
/// is not a hub at all.
const MAX_REPOS: usize = 4096;

/// Revisions inspected per repo when looking for a `config.json`.
const MAX_REVISIONS: usize = 64;

/// Cap on files examined inside one repo, so a pathological tree cannot stall the scan.
const MAX_FILES_PER_REPO: usize = 8192;

/// What a cached repo appears to be, and therefore whether it is a candidate for
/// deletion, import, or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum ModelKind {
    /// Speech, audio, or music. Called out separately because it is the one family this
    /// project has deliberately excluded from scope, so it is the first thing a user
    /// clearing space will want to see grouped.
    Audio,
    /// Text generation.
    Text,
    /// Multimodal / vision-language.
    Vision,
    /// Embeddings, reranking, classification.
    Embedding,
    /// Nothing readable said what this is. Not a licence to delete it.
    #[default]
    Unknown,
}

/// How [`CachedRepo::kind`] was arrived at.
///
/// Kept separate from the verdict itself: "the config says whisper" and "the folder name
/// contains whisper" support very different levels of confidence, and collapsing them is
/// how a name coincidence turns into a deleted model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum KindSource {
    /// Read from a snapshot's `config.json`.
    Config,
    /// Inferred from the repo id, because no config was readable.
    RepoName,
    /// Neither worked.
    #[default]
    None,
}

/// One `models--org--name` directory in an external hub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedRepo {
    /// Reconstructed `org/name`, or the raw directory name if it does not decode.
    pub repo_id: String,
    pub path: PathBuf,
    /// Bytes actually occupied. Symlinks are skipped, so the HF layout's
    /// `snapshots/<rev>/file -> ../../blobs/<sha>` cannot double-count.
    pub bytes: u64,
    pub file_count: usize,
    /// Newest mtime under the repo, as a unix timestamp. For a blob store this is
    /// effectively when the repo was last downloaded or touched.
    pub last_modified_unix: Option<u64>,
    pub revisions: Vec<String>,
    pub kind: ModelKind,
    pub kind_source: KindSource,
    /// Modalities the config declares alongside the primary kind. A Gemma-4 or Qwen3.6
    /// checkpoint is a text model that also carries vision and audio towers; reporting
    /// that as `kind: Audio` would file the largest text models on the box under the one
    /// bucket a user is most likely to bulk-delete.
    pub has_vision: bool,
    pub has_audio: bool,
    /// What the repo name suggests, recorded only when it disagrees with `kind`.
    ///
    /// `bosonai/higgs-audio-v3-tts-4b` has an ordinary text-generation config and is a
    /// TTS model; the disagreement is information, so it is surfaced rather than resolved
    /// by picking a winner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_hint: Option<ModelKind>,
    /// Whether the managed library already has this repo id cached. A duplicate is the
    /// safest thing to reclaim, which is exactly why it must be reported as a fact about
    /// two directories rather than inferred from a name alone.
    pub in_library: bool,
    /// Set when the scan could not read part of this repo. The size is then a floor, not
    /// a total, and a caller should not present it as exact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_reason: Option<String>,
}

/// Result of auditing one external hub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCacheAudit {
    pub root: PathBuf,
    /// Sum of [`CachedRepo::bytes`]. Not `du` of the root: loose files that are not repo
    /// directories are reported separately rather than folded into a total that would
    /// then not match the rows.
    pub total_bytes: u64,
    /// Bytes under `root` that are not in a `models--*` repo directory.
    pub unaccounted_bytes: u64,
    pub repos: Vec<CachedRepo>,
    /// True when [`MAX_REPOS`] was hit and the listing is incomplete.
    pub truncated: bool,
}

/// What a single [`remove_repo`] call reclaimed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedRepo {
    pub repo_id: String,
    pub path: PathBuf,
    /// Measured immediately before the delete, so it is what was actually freed rather
    /// than a figure from an audit that may be minutes stale.
    pub bytes: u64,
    pub file_count: usize,
}

/// Delete one `models--*` repo directory from an external hub.
///
/// Takes a repo id, never a path, and resolves it by listing the root and matching against
/// what [`audit`] would have reported. That inverts the usual validate-then-trust order:
/// instead of checking a caller-supplied path for traversal, the set of deletable
/// directories is enumerated from disk and the request can only select from it. A `..`, an
/// absolute path, or a symlink pointing out of the cache has nothing to match.
///
/// All the refusals in [`audit`] apply — a symlinked root, and the managed library cache,
/// which must never be deletable through the door marked "external".
pub fn remove_repo(root: &Path, repo_id: &str, library_hub: Option<&Path>) -> Result<RemovedRepo> {
    let meta = fs::symlink_metadata(root)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        bail!(
            "Refusing to delete inside {}: not a directory, or a symlink whose target is \
             what would actually be removed",
            root.display()
        );
    }
    let canonical = root.canonicalize()?;
    if library_hub
        .and_then(|path| path.canonicalize().ok())
        .as_deref()
        == Some(canonical.as_path())
    {
        bail!(
            "{} is the managed library cache; delete models there through the library, not \
             the external-cache audit",
            canonical.display()
        );
    }

    let mut matches = Vec::new();
    for entry in fs::read_dir(&canonical)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if file_type.is_symlink() || !file_type.is_dir() || !name.starts_with("models--") {
            continue;
        }
        if repo_id_from_dir(&name) == repo_id {
            matches.push(entry.path());
        }
    }
    let target = match matches.len() {
        0 => bail!("No cached repo named {repo_id} in {}", canonical.display()),
        1 => matches.remove(0),
        // Two directory names collapsing to one repo id means the id is not enough to say
        // what to delete. Guessing here would delete the wrong model.
        count => bail!(
            "{count} directories in {} map to {repo_id}; refusing to guess which to delete",
            canonical.display()
        ),
    };

    let stats = walk_files(&target).unwrap_or_default();
    fs::remove_dir_all(&target)?;
    Ok(RemovedRepo {
        repo_id: repo_id.to_string(),
        path: target,
        bytes: stats.bytes,
        file_count: stats.files,
    })
}

/// Default user-wide hub, when it exists.
///
/// Deliberately not consulted by anything that writes: this is the cache the app does
/// *not* own, and the only reason to look at it is to report on it.
pub fn shared_hub() -> Option<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join(".cache/huggingface/hub"))
        .filter(|path| path.is_dir())
}

/// Audit `root`, treating `library_hub` as the managed cache for duplicate detection.
///
/// Refuses a root that is a symlink or is the managed hub itself: auditing the library as
/// though it were external would invite an "external, safe to delete" reading of models
/// the app is actively serving.
pub fn audit(root: &Path, library_hub: Option<&Path>) -> Result<ExternalCacheAudit> {
    let meta = fs::symlink_metadata(root)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        bail!(
            "Not a directory to audit: {} (a symlinked cache root is refused, because \
             what it points at is what would be deleted)",
            root.display()
        );
    }
    let canonical = root.canonicalize()?;
    let library = library_hub.and_then(|path| path.canonicalize().ok());
    if library.as_deref() == Some(canonical.as_path()) {
        bail!(
            "{} is the managed library cache, not an external one",
            canonical.display()
        );
    }
    let library_repos = library
        .as_deref()
        .map(read_repo_dir_names)
        .unwrap_or_default();

    let mut repos = Vec::new();
    let mut unaccounted_bytes = 0;
    let mut truncated = false;
    for (index, entry) in fs::read_dir(&canonical)?.enumerate() {
        let entry = entry?;
        if index >= MAX_REPOS {
            truncated = true;
            break;
        }
        let file_type = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if file_type.is_symlink() {
            continue;
        }
        if !file_type.is_dir() || !name.starts_with("models--") {
            // Loose files and non-repo directories (`CACHEDIR.TAG`, `datasets--*`, stray
            // downloads) are counted but not offered as rows: this audit only knows how
            // to reason about model repos.
            unaccounted_bytes += directory_bytes(&entry.path()).unwrap_or(0);
            continue;
        }
        repos.push(inspect_repo(&entry.path(), &name, &library_repos));
    }
    repos.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.repo_id.cmp(&b.repo_id)));
    Ok(ExternalCacheAudit {
        root: canonical,
        total_bytes: repos.iter().map(|repo| repo.bytes).sum(),
        unaccounted_bytes,
        repos,
        truncated,
    })
}

/// Directory names of every `models--*` repo in a hub, for duplicate detection.
fn read_repo_dir_names(hub: &Path) -> HashSet<String> {
    let Ok(entries) = fs::read_dir(hub) else {
        return HashSet::new();
    };
    entries
        .flatten()
        .take(MAX_REPOS)
        .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("models--"))
        .collect()
}

/// `models--mlx-community--Qwen3-4bit` -> `mlx-community/Qwen3-4bit`.
///
/// Falls back to the raw directory name rather than failing: an unrecognised name is
/// still a directory taking up space, and hiding it would make the total unexplainable.
fn repo_id_from_dir(name: &str) -> String {
    match name.strip_prefix("models--") {
        Some(rest) if rest.contains("--") => rest.replacen("--", "/", 1),
        Some(rest) => rest.to_string(),
        None => name.to_string(),
    }
}

fn inspect_repo(path: &Path, dir_name: &str, library_repos: &HashSet<String>) -> CachedRepo {
    let mut partial_reason = None;
    let walk = walk_files(path);
    if let Err(ref error) = walk {
        partial_reason = Some(format!("Size is a floor, not a total: {error}"));
    }
    let stats = walk.unwrap_or_default();
    let revisions = read_revisions(path);
    let assessment = classify(path, &revisions, dir_name);
    CachedRepo {
        repo_id: repo_id_from_dir(dir_name),
        path: path.to_path_buf(),
        bytes: stats.bytes,
        file_count: stats.files,
        last_modified_unix: stats.newest_mtime,
        revisions,
        kind: assessment.kind,
        kind_source: assessment.kind_source,
        has_vision: assessment.has_vision,
        has_audio: assessment.has_audio,
        name_hint: assessment.name_hint,
        in_library: library_repos.contains(dir_name),
        partial_reason,
    }
}

#[derive(Debug, Default)]
struct WalkStats {
    bytes: u64,
    files: usize,
    newest_mtime: Option<u64>,
}

/// Sum regular files under `root`, following no symlinks.
///
/// Skipping symlinks is what makes the number meaningful in an HF cache: every file in
/// `snapshots/<rev>/` is a link into `blobs/`, so following them would report roughly
/// double the space a deletion would actually reclaim.
fn walk_files(root: &Path) -> Result<WalkStats> {
    let mut stats = WalkStats::default();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if stats.files >= MAX_FILES_PER_REPO {
                return Ok(stats);
            }
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(entry.path());
                continue;
            }
            let meta = entry.metadata()?;
            stats.bytes += meta.len();
            stats.files += 1;
            let since_epoch = meta
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok());
            if let Some(since) = since_epoch {
                let secs = since.as_secs();
                stats.newest_mtime = Some(stats.newest_mtime.map_or(secs, |t| t.max(secs)));
            }
        }
    }
    Ok(stats)
}

/// Same as [`walk_files`] but bytes only, for the non-repo remainder.
fn directory_bytes(path: &Path) -> Result<u64> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Ok(0);
    }
    if meta.is_file() {
        return Ok(meta.len());
    }
    Ok(walk_files(path)?.bytes)
}

fn read_revisions(repo: &Path) -> Vec<String> {
    let snapshots = repo.join("snapshots");
    let Ok(entries) = fs::read_dir(&snapshots) else {
        return Vec::new();
    };
    entries
        .flatten()
        .take(MAX_REVISIONS)
        .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

/// Everything the audit concluded about one repo's modality.
#[derive(Debug, Default)]
struct Assessment {
    kind: ModelKind,
    kind_source: KindSource,
    has_vision: bool,
    has_audio: bool,
    name_hint: Option<ModelKind>,
}

/// Decide what this repo is, preferring a config over a name.
fn classify(repo: &Path, revisions: &[String], dir_name: &str) -> Assessment {
    let repo_id = repo_id_from_dir(dir_name).to_ascii_lowercase();
    let hint = kind_from_name(&repo_id);
    for revision in revisions {
        let config = repo.join("snapshots").join(revision).join("config.json");
        // read_to_string follows the symlink into blobs/, which is wanted here: we are
        // reading content, not measuring space.
        let Ok(raw) = fs::read_to_string(&config) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if let Some(mut assessment) = assess_config(&parsed) {
            assessment.name_hint = hint.filter(|guess| *guess != assessment.kind);
            return assessment;
        }
    }
    match hint {
        Some(kind) => Assessment {
            kind,
            kind_source: KindSource::RepoName,
            ..Default::default()
        },
        None => Assessment {
            kind: ModelKind::Unknown,
            kind_source: KindSource::None,
            ..Default::default()
        },
    }
}

/// Classify from `config.json`.
///
/// Precedence is the whole design here, learned from what the real cache contains:
///
/// 1. An architecture that names an audio task (`WhisperForConditionalGeneration`) with no
///    text decoder is a dedicated audio model.
/// 2. A `text_config`, or a causal-LM architecture, means a text-generation model — even
///    when `vision_config` and `audio_config` sit beside it. Those towers become
///    capability flags. Treating them as the verdict filed 238 GB of Qwen3.6 and Gemma-4
///    text models as "vision", and 41 GB of them as "audio".
/// 3. Only then do vision or embedding markers decide.
fn assess_config(config: &serde_json::Value) -> Option<Assessment> {
    let has_vision = config.get("vision_config").is_some();
    let has_audio = config.get("audio_config").is_some();
    let mut haystack = String::new();
    if let Some(architectures) = config.get("architectures").and_then(|v| v.as_array()) {
        for architecture in architectures {
            if let Some(name) = architecture.as_str() {
                haystack.push_str(&name.to_ascii_lowercase());
                haystack.push(' ');
            }
        }
    }
    for field in ["model_type", "pipeline_tag"] {
        if let Some(value) = config.get(field).and_then(|v| v.as_str()) {
            haystack.push_str(&value.to_ascii_lowercase());
            haystack.push(' ');
        }
    }
    let has_text_decoder = config.get("text_config").is_some()
        || config.get("thinker_config").is_some()
        || haystack.contains("forcausallm");
    let settle = |kind: ModelKind| {
        Some(Assessment {
            kind,
            kind_source: KindSource::Config,
            has_vision,
            has_audio,
            name_hint: None,
        })
    };
    if !has_text_decoder && AUDIO_MARKERS.iter().any(|m| haystack.contains(m)) {
        return settle(ModelKind::Audio);
    }
    if has_text_decoder {
        return settle(ModelKind::Text);
    }
    if haystack.is_empty() {
        return None;
    }
    if has_vision || VISION_MARKERS.iter().any(|m| haystack.contains(m)) {
        return settle(ModelKind::Vision);
    }
    if EMBEDDING_MARKERS.iter().any(|m| haystack.contains(m)) {
        return settle(ModelKind::Embedding);
    }
    if haystack.contains("forconditionalgeneration") {
        return settle(ModelKind::Text);
    }
    None
}

/// Last resort, when no config could be read. Names are evidence, not proof, which is why
/// this result is always tagged [`KindSource::RepoName`].
fn kind_from_name(repo_id: &str) -> Option<ModelKind> {
    for marker in AUDIO_MARKERS {
        if repo_id.contains(marker) {
            return Some(ModelKind::Audio);
        }
    }
    for marker in EMBEDDING_MARKERS {
        if repo_id.contains(marker) {
            return Some(ModelKind::Embedding);
        }
    }
    for marker in VISION_MARKERS {
        if repo_id.contains(marker) {
            return Some(ModelKind::Vision);
        }
    }
    None
}

const AUDIO_MARKERS: &[&str] = &[
    "whisper", "parakeet", "kokoro", "bark", "musicgen", "encodec", "wav2vec", "hubert",
    "seamless", "speecht5", "sesame", "csm-", "moshi", "orpheus", "dia-", "xtts", "vits", "audio",
    "-tts", "tts-", "-asr", "voice",
];

const VISION_MARKERS: &[&str] = &[
    "-vl", "vlm", "llava", "clip", "siglip", "qwen2-vl", "qwen3-vl", "internvl", "pixtral",
    "florence", "idefics", "-vision",
];

const EMBEDDING_MARKERS: &[&str] = &[
    "embed",
    "bge-",
    "gte-",
    "e5-",
    "reranker",
    "rerank",
    "sentence-transformers",
    "minilm",
    "nomic-embed",
    "jina-embed",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Build `<hub>/models--<slug>` with `blobs/` holding real bytes and `snapshots/rev`
    /// linking to them, i.e. the layout huggingface_hub actually writes.
    fn write_repo(hub: &Path, slug: &str, blob_bytes: &[(&str, usize)], config: Option<&str>) {
        let repo = hub.join(format!("models--{slug}"));
        let blobs = repo.join("blobs");
        let snapshot = repo.join("snapshots/rev0");
        fs::create_dir_all(&blobs).unwrap();
        fs::create_dir_all(&snapshot).unwrap();
        for (name, size) in blob_bytes {
            let blob = blobs.join(name);
            fs::write(&blob, vec![0u8; *size]).unwrap();
            #[cfg(unix)]
            std::os::unix::fs::symlink(&blob, snapshot.join(name)).unwrap();
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(&blob, snapshot.join(name)).unwrap();
        }
        if let Some(body) = config {
            let blob = blobs.join("config-blob");
            fs::write(&blob, body).unwrap();
            #[cfg(unix)]
            std::os::unix::fs::symlink(&blob, snapshot.join("config.json")).unwrap();
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(&blob, snapshot.join("config.json")).unwrap();
        }
    }

    #[test]
    fn size_counts_blobs_once_and_never_through_snapshot_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        write_repo(
            &hub,
            "org--model",
            &[("sha-a", 1000), ("sha-b", 2000)],
            None,
        );

        let audit = audit(&hub, None).unwrap();
        assert_eq!(audit.repos.len(), 1);
        assert_eq!(
            audit.repos[0].bytes, 3000,
            "following snapshot symlinks would report 6000 and overstate what a delete \
             would reclaim"
        );
        assert_eq!(audit.total_bytes, 3000);
    }

    #[test]
    fn repo_ids_are_reconstructed_and_rows_sort_by_size() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        write_repo(&hub, "small-org--tiny", &[("a", 10)], None);
        write_repo(&hub, "mlx-community--Qwen3-4bit", &[("a", 9000)], None);

        let audit = audit(&hub, None).unwrap();
        assert_eq!(audit.repos[0].repo_id, "mlx-community/Qwen3-4bit");
        assert_eq!(audit.repos[1].repo_id, "small-org/tiny");
    }

    #[test]
    fn a_config_outranks_a_name_that_suggests_something_else() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        // The name says audio; the config says a text decoder. The config wins, because
        // deleting a text model because its name contains "voice" is the exact mistake
        // this split is meant to prevent.
        write_repo(
            &hub,
            "someone--voice-assistant-llm",
            &[("a", 10)],
            Some(r#"{"model_type":"qwen3","architectures":["Qwen3ForCausalLM"]}"#),
        );
        let audit = audit(&hub, None).unwrap();
        assert_eq!(audit.repos[0].kind, ModelKind::Text);
        assert_eq!(audit.repos[0].kind_source, KindSource::Config);
    }

    #[test]
    fn a_name_only_guess_is_labelled_as_one() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        write_repo(&hub, "openai--whisper-large-v3", &[("a", 10)], None);
        let audit = audit(&hub, None).unwrap();
        assert_eq!(audit.repos[0].kind, ModelKind::Audio);
        assert_eq!(
            audit.repos[0].kind_source,
            KindSource::RepoName,
            "with no config to read, the verdict must not present itself as read from one"
        );
    }

    #[test]
    fn a_multimodal_text_model_is_text_with_capability_flags() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        // Gemma-4's real shape: a text decoder with vision *and* audio towers beside it.
        // Filing this as Audio put 41 GB of text models in the bucket a user clearing
        // space would delete first.
        write_repo(
            &hub,
            "ailexleon--gemma-4-26B-it-mlx-vlm-4Bit",
            &[("a", 10)],
            Some(
                r#"{"architectures":["Gemma4ForConditionalGeneration"],"model_type":"gemma4",
                    "text_config":{},"vision_config":{},"audio_config":{}}"#,
            ),
        );
        let audit = audit(&hub, None).unwrap();
        let repo = &audit.repos[0];
        assert_eq!(repo.kind, ModelKind::Text);
        assert!(repo.has_vision && repo.has_audio);
    }

    #[test]
    fn a_dedicated_audio_model_with_no_text_decoder_is_audio() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        write_repo(
            &hub,
            "openai--whisper-large-v3",
            &[("a", 10)],
            Some(r#"{"architectures":["WhisperForConditionalGeneration"],"model_type":"whisper"}"#),
        );
        let audit = audit(&hub, None).unwrap();
        assert_eq!(audit.repos[0].kind, ModelKind::Audio);
        assert_eq!(audit.repos[0].kind_source, KindSource::Config);
    }

    #[test]
    fn a_name_that_disagrees_with_the_config_is_reported_not_resolved() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        // bosonai/higgs-audio-v3-tts-4b really is a TTS model, and its config really does
        // look like plain text generation. Neither source is wrong enough to discard.
        write_repo(
            &hub,
            "bosonai--higgs-audio-v3-tts-4b",
            &[("a", 10)],
            Some(
                r#"{"architectures":["HiggsMultimodalQwen3ForConditionalGeneration"],
                    "model_type":"higgs_multimodal_qwen3","text_config":{}}"#,
            ),
        );
        let audit = audit(&hub, None).unwrap();
        let repo = &audit.repos[0];
        assert_eq!(repo.kind, ModelKind::Text);
        assert_eq!(repo.kind_source, KindSource::Config);
        assert_eq!(
            repo.name_hint,
            Some(ModelKind::Audio),
            "a caller deciding what to delete needs to see the disagreement"
        );
    }

    #[test]
    fn an_agreeing_name_adds_no_hint() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        write_repo(
            &hub,
            "openai--whisper-tiny",
            &[("a", 10)],
            Some(r#"{"architectures":["WhisperForConditionalGeneration"]}"#),
        );
        let audit = audit(&hub, None).unwrap();
        assert!(audit.repos[0].name_hint.is_none());
    }

    #[test]
    fn a_repo_already_in_the_library_is_flagged_as_a_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let external = dir.path().join("external");
        let library = dir.path().join("library");
        fs::create_dir_all(&external).unwrap();
        fs::create_dir_all(&library).unwrap();
        write_repo(&external, "org--shared", &[("a", 10)], None);
        write_repo(&external, "org--only-outside", &[("a", 20)], None);
        write_repo(&library, "org--shared", &[("a", 10)], None);

        let audit = audit(&external, Some(&library)).unwrap();
        let shared = audit
            .repos
            .iter()
            .find(|repo| repo.repo_id == "org/shared")
            .unwrap();
        let outside = audit
            .repos
            .iter()
            .find(|repo| repo.repo_id == "org/only-outside")
            .unwrap();
        assert!(shared.in_library);
        assert!(!outside.in_library);
    }

    #[test]
    fn non_repo_contents_are_counted_but_not_offered_as_rows() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        write_repo(&hub, "org--model", &[("a", 100)], None);
        fs::write(hub.join("CACHEDIR.TAG"), vec![0u8; 7]).unwrap();
        fs::create_dir_all(hub.join("datasets--org--set")).unwrap();
        fs::write(hub.join("datasets--org--set/data"), vec![0u8; 50]).unwrap();

        let audit = audit(&hub, None).unwrap();
        assert_eq!(audit.repos.len(), 1, "only model repos become rows");
        assert_eq!(
            audit.unaccounted_bytes, 57,
            "the remainder is reported rather than folded into a total the rows cannot explain"
        );
    }

    #[test]
    fn auditing_the_library_cache_as_external_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        let error = audit(&hub, Some(&hub)).unwrap_err().to_string();
        assert!(error.contains("managed library cache"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_root_is_refused_because_the_target_is_what_would_be_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        fs::create_dir_all(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(audit(&link, None).is_err());
    }

    #[test]
    fn removing_a_repo_reports_what_it_freed_and_leaves_its_neighbours_alone() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        write_repo(&hub, "acme--doomed", &[("a", 4096), ("b", 2048)], None);
        write_repo(&hub, "acme--keeper", &[("a", 1024)], None);

        let removed = remove_repo(&hub, "acme/doomed", None).unwrap();
        assert_eq!(removed.bytes, 4096 + 2048);
        assert_eq!(removed.file_count, 2);
        assert!(!hub.join("models--acme--doomed").exists());
        assert!(hub.join("models--acme--keeper").exists());
    }

    #[test]
    fn a_repo_id_that_is_really_a_path_matches_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        write_repo(&hub, "acme--keeper", &[("a", 1024)], None);
        let outside = dir.path().join("precious");
        fs::create_dir_all(&outside).unwrap();

        // Ids are matched against directories found on disk, so traversal has nothing to
        // resolve against: these are simply repos that do not exist.
        for attempt in ["../precious", "/etc", "..", "acme/keeper/../../precious"] {
            let error = remove_repo(&hub, attempt, None).unwrap_err().to_string();
            assert!(error.contains("No cached repo named"), "{attempt}: {error}");
        }
        assert!(outside.exists());
        assert!(hub.join("models--acme--keeper").exists());
    }

    #[test]
    fn deleting_from_the_managed_cache_through_this_door_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        write_repo(&hub, "acme--served", &[("a", 1024)], None);

        let error = remove_repo(&hub, "acme/served", Some(&hub))
            .unwrap_err()
            .to_string();
        assert!(error.contains("managed library cache"), "{error}");
        assert!(hub.join("models--acme--served").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_repo_directory_is_not_deletable() {
        let dir = tempfile::tempdir().unwrap();
        let hub = dir.path().join("hub");
        fs::create_dir_all(&hub).unwrap();
        let elsewhere = dir.path().join("elsewhere");
        fs::create_dir_all(elsewhere.join("blobs")).unwrap();
        fs::write(elsewhere.join("blobs/a"), vec![0u8; 32]).unwrap();
        std::os::unix::fs::symlink(&elsewhere, hub.join("models--acme--linked")).unwrap();

        // A link in the cache is someone else's storage borrowed, and `remove_dir_all`
        // through it would take the target with it.
        assert!(remove_repo(&hub, "acme/linked", None).is_err());
        assert!(elsewhere.join("blobs/a").exists());
    }
}
