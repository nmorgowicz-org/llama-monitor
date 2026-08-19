//! Adoption and removal of whole *model directories* in the managed library.
//!
//! GGUF models are single files, so the library has always been able to take one in and
//! delete one again. Directory-shaped models — MLX, Transformers — could do neither: the
//! inventory would discover a directory placed under `models/mlx/native` by hand, but
//! nothing in the app could put it there or take it away. That gap is why a developer
//! validating MLX ends up with model trees scattered across `~`, which is the state this
//! module exists to make unnecessary.
//!
//! Two operations, deliberately kept apart:
//!
//! - [`plan_adoption`] and [`adopt_directory`] bring an outside directory in. The plan is
//!   read-only and answers "what would this cost and how" before anything is written,
//!   because the honest answer is sometimes 27 GB of copying.
//! - [`remove_managed_directory`] deletes one that is already in.
//!
//! Both follow the same enumerate-then-select rule as
//! [`crate::models::external_cache::remove_repo`]: a caller names a destination slug or an
//! existing entry, never a path the code then has to prove safe. The set of directories
//! that can be written or removed is derived from the library root, so a `..`, an absolute
//! path, or a symlink pointing out of the tree has nothing to match rather than being
//! filtered out by a check someone could later weaken.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// Files walked in a source directory before the scan gives up. A model directory holds
/// tens of files; four thousand means the caller pointed at something else.
const MAX_SOURCE_FILES: usize = 4096;

/// Directory nesting walked in a source. MLX and Transformers layouts are flat or nearly
/// so; deep trees are a sign of a repository or a home directory, not a model.
const MAX_SOURCE_DEPTH: usize = 4;

/// Library subdirectories that hold one model per immediate child, and are therefore the
/// only places this module will create or destroy a directory.
///
/// The app's Hugging Face cache is deliberately absent. A snapshot directory there is a
/// view onto a shared blob store, so removing it reclaims almost nothing and leaves the
/// blobs orphaned; that layout has its own repo-level delete in
/// [`crate::models::external_cache`]. Excluding it here by not listing it — rather than by
/// testing for it — means a future root added to this list cannot accidentally inherit
/// cache semantics.
const MANAGED_MODEL_PARENTS: &[&str] = &[
    "mlx/native",
    "mlx/converted",
    "transformers",
    "rapid-mlx/imports",
    "rapid-mlx/requantized",
];

/// How the bytes will get to their destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdoptionMethod {
    /// Same filesystem: every regular file gets a hard link, so adoption is effectively
    /// free and costs no extra disk. The two paths then share one inode, which is worth
    /// telling the user: editing the file in either place edits both.
    Hardlink,
    /// Different filesystems, where a link is impossible. Every byte is copied and the
    /// library ends up owning an independent second copy.
    Copy,
}

/// What [`adopt_directory`] would do, computed without writing anything.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdoptionPlan {
    pub source: PathBuf,
    /// Where it would land. Always an immediate child of `mlx/native` under the library.
    pub destination: PathBuf,
    /// The directory name that will be created, after slugging.
    pub slug: String,
    pub method: AdoptionMethod,
    pub bytes: u64,
    pub file_count: usize,
    /// Symlinks in the source that adoption will replace with real files or links to their
    /// targets. A symlink farm over an HF cache is a legitimate MLX layout, but a library
    /// entry that breaks when an unrelated cache is cleaned is not, so adoption resolves
    /// them and says how many it resolved.
    pub resolved_symlinks: usize,
    /// Things a user should know before agreeing, in their own words. Empty is the normal
    /// case; a non-empty list is not a refusal.
    pub warnings: Vec<String>,
}

/// Outcome of a completed adoption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdoptedModel {
    pub destination: PathBuf,
    pub slug: String,
    pub method: AdoptionMethod,
    pub bytes: u64,
    pub file_count: usize,
}

/// What [`remove_managed_directory`] reclaimed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedDirectory {
    pub path: PathBuf,
    /// Measured immediately before the delete, so it reports what was actually freed
    /// rather than a figure from an inventory scan that may be minutes stale.
    pub bytes: u64,
    pub file_count: usize,
    /// True when files in the directory were hard links, so the bytes are shared with
    /// whatever the adoption source was and the disk will not shrink by this much.
    pub shared_links: bool,
}

/// Turn an arbitrary name into a directory name the library can hold.
///
/// Conservative on purpose: lowercase alphanumerics, plus dash, underscore, and a dot that
/// follows an alphanumeric. Everything else collapses to a single dash and edge punctuation
/// goes. A slug is not a display name — the model's own `config.json` supplies that — so
/// losing case and punctuation costs nothing, and building the name out of an allowed set
/// removes every path-shaped character in one pass instead of enumerating dangerous ones.
///
/// The dot rule is what keeps a version number (`Qwen3.6`) readable while making `..`
/// unrepresentable, so no output of this function can traverse.
fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_')
            || (ch == '.' && out.ends_with(|c: char| c.is_ascii_alphanumeric()))
        {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches(['-', '.', '_']).to_string();
    trimmed.chars().take(96).collect()
}

/// Cheap structural check that a directory is plausibly a model, before anything is copied.
///
/// Deliberately *not* [`crate::inference::rapid_mlx::model_resolver::validate_model_directory`],
/// which is the authoritative check and runs after materialization. That one refuses an
/// asset reached through a symlink leaving its approved root — correct for a model the app
/// is about to serve in place, and wrong here, because a symlink farm over a Hugging Face
/// cache is precisely the layout adoption exists to convert into a self-contained directory.
/// Running it against the source would reject every cache-backed model on the machine.
///
/// So the order is: plausible shape (cheap, symlink-tolerant) → materialize → authoritative
/// validation on real files. The authoritative check is never skipped, only moved to the
/// point where it can be answered honestly.
fn precheck_model_shape(dir: &Path) -> Result<()> {
    if !dir.join("config.json").is_file() {
        bail!(
            "{} has no config.json, so nothing can say what model it is",
            dir.display()
        );
    }
    let has_tokenizer = [
        "tokenizer.json",
        "tokenizer.model",
        "sentencepiece.bpe.model",
    ]
    .iter()
    .any(|name| dir.join(name).is_file())
        || (dir.join("vocab.json").is_file() && dir.join("merges.txt").is_file());
    if !has_tokenizer {
        bail!(
            "{} has no tokenizer (tokenizer.json, a SentencePiece model, or vocab.json plus \
             merges.txt), so it cannot be served",
            dir.display()
        );
    }
    let has_weights = fs::read_dir(dir)?.flatten().any(|entry| {
        entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.ends_with(".safetensors"))
    });
    if !has_weights {
        bail!("{} contains no .safetensors weights", dir.display());
    }
    Ok(())
}

/// Bytes, file count, and symlink count under `dir`, refusing anything that does not look
/// like a model directory long before the expensive part.
fn survey_source(dir: &Path) -> Result<(u64, usize, usize)> {
    let mut bytes = 0u64;
    let mut files = 0usize;
    let mut links = 0usize;
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    while let Some((current, depth)) = stack.pop() {
        if depth > MAX_SOURCE_DEPTH {
            bail!(
                "{} nests deeper than {MAX_SOURCE_DEPTH} levels; that is not the shape of a \
                 model directory",
                dir.display()
            );
        }
        for entry in
            fs::read_dir(&current).with_context(|| format!("Cannot read {}", current.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() {
                // Measure the target, since that is what will be materialized.
                match fs::metadata(&path) {
                    Ok(target) if target.is_file() => {
                        links += 1;
                        files += 1;
                        bytes += target.len();
                    }
                    Ok(_) => bail!(
                        "{} is a symlink to a directory; adopt the directory it points at \
                         instead, so the library owns a layout that does not depend on it",
                        path.display()
                    ),
                    Err(_) => bail!(
                        "{} is a broken symlink, so the source is incomplete",
                        path.display()
                    ),
                }
            } else if meta.is_dir() {
                stack.push((path, depth + 1));
            } else if meta.is_file() {
                files += 1;
                bytes += meta.len();
            }
            if files > MAX_SOURCE_FILES {
                bail!(
                    "{} holds more than {MAX_SOURCE_FILES} files; that is not the shape of a \
                     model directory",
                    dir.display()
                );
            }
        }
    }
    Ok((bytes, files, links))
}

/// Same-filesystem check, which is what decides link versus copy.
fn same_filesystem(a: &Path, b: &Path) -> Result<bool> {
    #[cfg(unix)]
    {
        Ok(fs::metadata(a)?.dev() == fs::metadata(b)?.dev())
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        // Windows has no stable std metadata device-id equivalent. Copying is
        // the conservative choice; it preserves correctness when a volume
        // boundary or filesystem hard-link policy is unknown.
        Ok(false)
    }
}

fn has_shared_hardlink(meta: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        meta.nlink() > 1
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}

/// The library's own `mlx/native`, created if the library exists but has never held one.
fn native_root(models_dir: &Path) -> Result<PathBuf> {
    let root = models_dir.canonicalize().with_context(|| {
        format!(
            "Model library {} does not exist yet; nothing can be adopted into it",
            models_dir.display()
        )
    })?;
    let native = root.join("mlx/native");
    fs::create_dir_all(&native).with_context(|| format!("Cannot create {}", native.display()))?;
    native.canonicalize().map_err(Into::into)
}

/// Describe an adoption without performing it.
///
/// Validates the source as a servable model directory *first*. Copying 27 GB and then
/// discovering there is no tokenizer is the failure mode this ordering exists to prevent.
pub fn plan_adoption(models_dir: &Path, source: &Path, name: Option<&str>) -> Result<AdoptionPlan> {
    let meta = fs::symlink_metadata(source)
        .with_context(|| format!("No such directory: {}", source.display()))?;
    if meta.file_type().is_symlink() {
        bail!(
            "{} is a symlink. Adopt the directory it points at, so the library entry does \
             not break when the link's target moves.",
            source.display()
        );
    }
    if !meta.is_dir() {
        bail!(
            "{} is a file. Directory adoption is for MLX and Transformers models; a single \
             GGUF file belongs in the library's gguf directory.",
            source.display()
        );
    }
    let canonical_source = source.canonicalize()?;
    let native = native_root(models_dir)?;
    let library_root = native
        .parent()
        .and_then(Path::parent)
        .unwrap_or(&native)
        .to_path_buf();
    if canonical_source.starts_with(&library_root) {
        bail!(
            "{} is already inside the model library, so it is already managed and the \
             inventory can see it. Adoption would only make a second copy.",
            canonical_source.display()
        );
    }

    // Cheap refusal first: never survey, and certainly never copy, something that is not
    // shaped like a model at all.
    precheck_model_shape(&canonical_source).with_context(|| {
        format!(
            "{} is not a servable model directory",
            canonical_source.display()
        )
    })?;

    let slug = slugify(name.unwrap_or_else(|| {
        canonical_source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("model")
    }));
    if slug.is_empty() {
        bail!("That name has no characters a directory name can keep; pick another");
    }
    let destination = native.join(&slug);
    if destination.exists() {
        bail!(
            "The library already has an MLX model named {slug}. Pick a different name, or \
             remove that one first."
        );
    }

    let (bytes, file_count, resolved_symlinks) = survey_source(&canonical_source)?;
    let method = if same_filesystem(&canonical_source, &native)? {
        AdoptionMethod::Hardlink
    } else {
        AdoptionMethod::Copy
    };

    let mut warnings = Vec::new();
    if method == AdoptionMethod::Hardlink {
        warnings.push(format!(
            "Same filesystem, so this costs no additional disk space: the {file_count} files \
             will be hard-linked. The library and {} will share the same bytes, and deleting \
             one side does not free them.",
            canonical_source.display()
        ));
    } else {
        warnings.push(format!(
            "Different filesystem, so every byte has to be copied. The library will use an \
             additional {:.1} GB and the two copies will be independent.",
            bytes as f64 / 1e9
        ));
    }
    if resolved_symlinks > 0 {
        warnings.push(format!(
            "{resolved_symlinks} of the {file_count} files are symlinks — typically into a \
             Hugging Face cache. Adoption resolves them, so the library entry keeps working \
             after that cache is cleaned."
        ));
    }

    Ok(AdoptionPlan {
        source: canonical_source,
        destination,
        slug,
        method,
        bytes,
        file_count,
        resolved_symlinks,
        warnings,
    })
}

/// Adopt `source` into `<models_dir>/mlx/native/<slug>`.
///
/// Re-plans internally rather than trusting a plan handed back by a caller: a plan is a
/// description for a human to approve, not a capability token, and the directory may have
/// changed since it was made.
///
/// Assembles into a `.adopting-<slug>` staging directory and renames it into place at the
/// end. A half-copied model that the inventory discovers is worse than no model — it would
/// be offered, launched, and fail — and the staging name starts with a dot so the
/// inventory's own scan skips it in the meantime.
pub fn adopt_directory(
    models_dir: &Path,
    source: &Path,
    name: Option<&str>,
) -> Result<AdoptedModel> {
    let plan = plan_adoption(models_dir, source, name)?;
    let native = native_root(models_dir)?;
    let staging = native.join(format!(".adopting-{}", plan.slug));
    if staging.exists() {
        fs::remove_dir_all(&staging).with_context(|| {
            format!(
                "An interrupted adoption left {} behind and it could not be cleared",
                staging.display()
            )
        })?;
    }
    fs::create_dir(&staging)?;

    let result = materialize(&plan.source, &staging, plan.method);
    if let Err(err) = result {
        // Leaving a partial tree under a dot-directory would hide a growing pile of bytes.
        let _ = fs::remove_dir_all(&staging);
        return Err(err);
    }

    // Validate what was actually written, not what the source promised. A file that
    // vanished mid-copy is exactly the case the pre-flight check cannot see.
    if let Err(err) =
        crate::inference::rapid_mlx::model_resolver::validate_model_directory(&staging, &staging)
    {
        let _ = fs::remove_dir_all(&staging);
        return Err(err.context(
            "The adopted copy did not validate, so it was discarded rather than added to the \
             library",
        ));
    }

    fs::rename(&staging, &plan.destination).with_context(|| {
        format!(
            "Could not move the adopted model into place at {}",
            plan.destination.display()
        )
    })?;

    Ok(AdoptedModel {
        destination: plan.destination,
        slug: plan.slug,
        method: plan.method,
        bytes: plan.bytes,
        file_count: plan.file_count,
    })
}

/// Recreate `source`'s tree inside `dest`, linking or copying each file.
fn materialize(source: &Path, dest: &Path, method: AdoptionMethod) -> Result<()> {
    let mut stack = vec![(source.to_path_buf(), dest.to_path_buf())];
    while let Some((from, to)) = stack.pop() {
        for entry in fs::read_dir(&from)? {
            let entry = entry?;
            let dst_path = to.join(entry.file_name());
            let meta = fs::symlink_metadata(entry.path())?;
            // Resolve symlinks here rather than relying on the link call to do it. On
            // macOS and the BSDs `link(2)` does *not* follow a symlink — it links the link
            // itself — so a cache-backed farm would arrive in the library as a farm again,
            // and the authoritative validation would then correctly refuse it as an asset
            // escaping its root. `survey_source` has already refused broken links and links
            // to directories, so what canonicalizes here is a regular file.
            let src_path = if meta.file_type().is_symlink() {
                entry.path().canonicalize()?
            } else {
                entry.path()
            };
            if src_path.is_dir() {
                fs::create_dir_all(&dst_path)?;
                stack.push((src_path, dst_path));
                continue;
            }
            match method {
                AdoptionMethod::Hardlink => {
                    // Linking the resolved blob, not the link: the library's copy then
                    // survives the source cache being cleaned, because the inode outlives
                    // every name but the last.
                    fs::hard_link(&src_path, &dst_path).with_context(|| {
                        format!(
                            "Could not hard-link {} into the library",
                            src_path.display()
                        )
                    })?;
                }
                AdoptionMethod::Copy => {
                    fs::copy(&src_path, &dst_path)
                        .with_context(|| format!("Could not copy {}", src_path.display()))?;
                }
            }
        }
    }
    Ok(())
}

/// Delete a model directory that the library already manages.
///
/// Takes the path an inventory entry reported, but does not trust it as a path: the
/// candidates are enumerated from [`MANAGED_MODEL_PARENTS`] under the library root and the
/// request can only select one of them. A path outside the library, or inside the HF cache,
/// has nothing to match.
///
/// A request may also name a path *inside* a candidate, in which case the candidate is what
/// gets deleted. That is not laxity: the experimental caches under `rapid-mlx/imports` and
/// `rapid-mlx/requantized` wrap the servable model one level down (`<cache>/fp16`,
/// `<cache>/model`), so the inventory reports that inner path while the unit that means
/// anything on disk is the wrapper. Deleting only the inner directory would leave a cache
/// the inventory still walks and a provenance file describing a model that is gone. The
/// returned [`RemovedDirectory::path`] always says what was actually removed, so a caller
/// can show the user the wrapper rather than what they clicked.
pub fn remove_managed_directory(models_dir: &Path, path: &Path) -> Result<RemovedDirectory> {
    let root = models_dir
        .canonicalize()
        .with_context(|| format!("Model library {} does not exist", models_dir.display()))?;
    let requested = path
        .canonicalize()
        .with_context(|| format!("No such model directory: {}", path.display()))?;

    let mut candidates = HashSet::new();
    for parent in MANAGED_MODEL_PARENTS {
        let parent_dir = root.join(parent);
        let Ok(entries) = fs::read_dir(&parent_dir) else {
            continue;
        };
        for entry in entries {
            let entry = entry?;
            let child = entry.path();
            // A symlinked entry is skipped: what it points at is what would be deleted,
            // and that is not necessarily in the library at all.
            if fs::symlink_metadata(&child)
                .map(|m| m.file_type().is_symlink() || !m.is_dir())
                .unwrap_or(true)
            {
                continue;
            }
            if let Ok(canonical) = child.canonicalize() {
                candidates.insert(canonical);
            }
        }
    }

    // Exact match first, then the enclosing candidate. Longest-prefix is not needed —
    // candidates are siblings, so at most one can contain the request.
    let target = if candidates.contains(&requested) {
        requested.clone()
    } else if let Some(enclosing) = candidates
        .iter()
        .find(|candidate| requested.starts_with(candidate))
    {
        enclosing.clone()
    } else {
        bail!(
            "{} is not a model directory the library manages. Managed directories are the \
             immediate children of {} under {}. Models in the app's Hugging Face cache are \
             removed per repo instead, because a snapshot directory only holds links into a \
             shared blob store.",
            requested.display(),
            MANAGED_MODEL_PARENTS.join(", "),
            root.display()
        );
    };

    let (bytes, file_count, shared_links) = measure_managed(&target)?;
    fs::remove_dir_all(&target)
        .with_context(|| format!("Could not delete {}", target.display()))?;
    Ok(RemovedDirectory {
        path: target,
        bytes,
        file_count,
        shared_links,
    })
}

/// Bytes and file count under a managed directory, plus whether its files are hard links.
///
/// Best-effort: an unreadable subtree yields a smaller figure rather than blocking a
/// delete the user has already approved.
fn measure_managed(dir: &Path) -> Result<(u64, usize, bool)> {
    let mut bytes = 0u64;
    let mut files = 0usize;
    let mut shared = false;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                files += 1;
                bytes += meta.len();
                if has_shared_hardlink(&meta) {
                    shared = true;
                }
            }
        }
    }
    Ok((bytes, files, shared))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[cfg(unix)]
    fn symlink_file(source: &Path, destination: &Path) {
        std::os::unix::fs::symlink(source, destination).unwrap();
    }

    #[cfg(windows)]
    fn symlink_file(source: &Path, destination: &Path) {
        std::os::windows::fs::symlink_file(source, destination).unwrap();
    }

    #[cfg(unix)]
    fn symlink_dir(source: &Path, destination: &Path) {
        std::os::unix::fs::symlink(source, destination).unwrap();
    }

    #[cfg(windows)]
    fn symlink_dir(source: &Path, destination: &Path) {
        std::os::windows::fs::symlink_dir(source, destination).unwrap();
    }

    /// A directory that `validate_model_directory` accepts: config, tokenizer, weights.
    fn write_model(dir: &Path, weight_bytes: usize) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("config.json"),
            br#"{"architectures":["LlamaForCausalLM"]}"#,
        )
        .unwrap();
        fs::write(dir.join("tokenizer.json"), b"{}").unwrap();
        fs::write(dir.join("tokenizer_config.json"), b"{}").unwrap();
        let mut f = File::create(dir.join("model.safetensors")).unwrap();
        f.write_all(&vec![0u8; weight_bytes]).unwrap();
    }

    fn library(tmp: &Path) -> PathBuf {
        let dir = tmp.join("library");
        fs::create_dir_all(dir.join("mlx/native")).unwrap();
        dir
    }

    #[test]
    fn slugify_strips_every_path_shaped_character() {
        assert_eq!(slugify("Qwen3.6-27B MXFP8"), "qwen3.6-27b-mxfp8");
        assert_eq!(slugify("../../etc/passwd"), "etc-passwd");
        assert_eq!(slugify("/absolute/path"), "absolute-path");
        assert_eq!(slugify("..."), "");
        assert_eq!(slugify("a/../b"), "a-b");
        // No output can traverse or name a parent, whatever went in.
        for hostile in ["..", "../..", "./.", "a/../../b", "\\..\\..", "%2e%2e"] {
            let slug = slugify(hostile);
            assert!(
                !slug.contains("..") && !slug.contains('/') && !slug.contains('\\'),
                "slugify({hostile:?}) produced {slug:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_plan_reports_hardlinking_when_source_and_library_share_a_filesystem() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = library(tmp.path());
        let src = tmp.path().join("outside/my-model");
        write_model(&src, 1024);

        let plan = plan_adoption(&lib, &src, None).unwrap();
        assert_eq!(plan.method, AdoptionMethod::Hardlink);
        assert_eq!(plan.slug, "my-model");
        assert_eq!(plan.file_count, 4);
        assert!(plan.bytes >= 1024);
        assert!(plan.destination.ends_with("mlx/native/my-model"));
        assert!(
            plan.warnings
                .iter()
                .any(|w| w.contains("no additional disk")),
            "a link-based adoption must say the bytes are shared: {:?}",
            plan.warnings
        );
    }

    #[test]
    fn planning_refuses_a_directory_that_is_not_a_servable_model() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = library(tmp.path());
        let src = tmp.path().join("outside/not-a-model");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("notes.txt"), b"hello").unwrap();

        let err = plan_adoption(&lib, &src, None).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("not a servable model directory"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn planning_refuses_a_source_already_inside_the_library() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = library(tmp.path());
        let src = lib.join("mlx/native/already-here");
        write_model(&src, 16);

        let err = plan_adoption(&lib, &src, None).unwrap_err();
        assert!(
            format!("{err:#}").contains("already inside the model library"),
            "unexpected error: {err:#}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn adoption_hardlinks_the_files_and_leaves_no_staging_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = library(tmp.path());
        let src = tmp.path().join("outside/subject");
        write_model(&src, 4096);

        let adopted = adopt_directory(&lib, &src, None).unwrap();
        assert_eq!(adopted.method, AdoptionMethod::Hardlink);
        assert!(adopted.destination.join("model.safetensors").is_file());

        // Hard-linked, not copied: one inode, two names on Unix. Windows does not expose
        // inode/link-count metadata through std, so verify the destination has the same bytes
        // and rely on the production hard-link operation's successful method classification.
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let a = fs::metadata(src.join("model.safetensors")).unwrap();
            let b = fs::metadata(adopted.destination.join("model.safetensors")).unwrap();
            assert_eq!(a.ino(), b.ino());
            assert!(b.nlink() >= 2);
        }
        #[cfg(windows)]
        assert_eq!(
            fs::read(src.join("model.safetensors")).unwrap(),
            fs::read(adopted.destination.join("model.safetensors")).unwrap()
        );

        let leftovers: Vec<_> = fs::read_dir(lib.join("mlx/native"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with(".adopting-"))
            .collect();
        assert!(leftovers.is_empty(), "staging left behind: {leftovers:?}");
    }

    #[test]
    fn a_symlink_farm_is_resolved_so_the_entry_survives_its_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = library(tmp.path());
        let blobs = tmp.path().join("cache/blobs");
        fs::create_dir_all(&blobs).unwrap();
        fs::write(blobs.join("weights"), vec![7u8; 2048]).unwrap();

        let src = tmp.path().join("outside/farm");
        write_model(&src, 8);
        fs::remove_file(src.join("model.safetensors")).unwrap();
        symlink_file(&blobs.join("weights"), &src.join("model.safetensors"));

        let plan = plan_adoption(&lib, &src, None).unwrap();
        assert_eq!(plan.resolved_symlinks, 1);
        assert!(plan.bytes >= 2048, "the target's size must be counted");

        let adopted = adopt_directory(&lib, &src, None).unwrap();
        let landed = adopted.destination.join("model.safetensors");
        assert!(
            !fs::symlink_metadata(&landed)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the adopted copy must not be a symlink"
        );

        // Deleting the cache the source pointed at must not break the library entry.
        fs::remove_dir_all(tmp.path().join("cache")).unwrap();
        assert_eq!(fs::read(&landed).unwrap().len(), 2048);
    }

    #[test]
    fn a_broken_symlink_makes_the_source_incomplete_rather_than_partially_adopted() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = library(tmp.path());
        let src = tmp.path().join("outside/broken");
        write_model(&src, 8);
        // Named so it passes the shape pre-check and is caught by the survey, which is the
        // stage that has to notice a link with nothing behind it.
        symlink_file(&tmp.path().join("gone"), &src.join("extra.bin"));

        let err = plan_adoption(&lib, &src, None).unwrap_err();
        assert!(
            format!("{err:#}").contains("broken symlink"),
            "unexpected error: {err:#}"
        );
        assert!(
            fs::read_dir(lib.join("mlx/native"))
                .unwrap()
                .next()
                .is_none(),
            "a refused plan must not have created anything"
        );
    }

    #[test]
    fn adopting_the_same_name_twice_is_refused_rather_than_merged() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = library(tmp.path());
        let src = tmp.path().join("outside/dup");
        write_model(&src, 8);
        adopt_directory(&lib, &src, None).unwrap();

        let err = adopt_directory(&lib, &src, None).unwrap_err();
        assert!(
            format!("{err:#}").contains("already has an MLX model named dup"),
            "unexpected error: {err:#}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn removal_deletes_a_managed_directory_and_reports_shared_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = library(tmp.path());
        let src = tmp.path().join("outside/subject");
        write_model(&src, 4096);
        let adopted = adopt_directory(&lib, &src, None).unwrap();

        let removed = remove_managed_directory(&lib, &adopted.destination).unwrap();
        assert_eq!(removed.file_count, 4);
        assert!(removed.bytes >= 4096);
        assert!(
            removed.shared_links,
            "hard-linked files must be reported as shared, since the disk will not shrink"
        );
        assert!(!adopted.destination.exists());
        // The source is untouched: that is the whole point of reporting shared bytes.
        assert!(src.join("model.safetensors").is_file());
    }

    #[test]
    fn removal_only_accepts_directories_the_library_manages() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = library(tmp.path());

        // A model in the HF cache: real, in the library tree, and deliberately not
        // removable through this door.
        let cached = lib.join("cache/huggingface/hub/models--acme--keeper/snapshots/rev1");
        write_model(&cached, 32);
        let err = remove_managed_directory(&lib, &cached).unwrap_err();
        assert!(
            format!("{err:#}").contains("not a model directory the library manages"),
            "unexpected error: {err:#}"
        );
        assert!(cached.join("model.safetensors").is_file());

        // Outside the library entirely.
        let outside = tmp.path().join("outside/precious");
        write_model(&outside, 32);
        assert!(remove_managed_directory(&lib, &outside).is_err());
        assert!(outside.join("model.safetensors").is_file());

        // The parent root itself is not a model.
        assert!(remove_managed_directory(&lib, &lib.join("mlx/native")).is_err());
        assert!(lib.join("mlx/native").is_dir());
    }

    #[test]
    fn removal_refuses_traversal_through_a_managed_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = library(tmp.path());
        let victim = tmp.path().join("outside/victim");
        write_model(&victim, 32);
        write_model(&lib.join("mlx/native/real"), 32);

        for attempt in [
            lib.join("mlx/native/../../../outside/victim"),
            lib.join("mlx/native/real/.."),
            PathBuf::from("/etc"),
        ] {
            assert!(
                remove_managed_directory(&lib, &attempt).is_err(),
                "{} should not resolve to a deletable model",
                attempt.display()
            );
        }
        assert!(victim.join("model.safetensors").is_file());
        assert!(lib.join("mlx/native/real/model.safetensors").is_file());
    }

    #[test]
    fn removal_of_a_nested_cache_entry_reclaims_the_wrapper_it_lives_in() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = library(tmp.path());
        // The shape `add_experimental_mlx_caches` reports: the servable model is one level
        // below the managed child, so the inventory hands us `<cache>/fp16` while the unit
        // that means anything on disk is `<cache>`.
        let cache = lib.join("rapid-mlx/imports/some-repo");
        write_model(&cache.join("fp16"), 64);
        fs::write(cache.join("import.log"), b"provenance").unwrap();
        let expected = cache.canonicalize().unwrap();

        let removed = remove_managed_directory(&lib, &cache.join("fp16")).unwrap();
        assert_eq!(
            removed.path, expected,
            "the wrapper is what was removed, so that is what must be reported"
        );
        assert!(!cache.exists());
    }

    #[test]
    fn a_symlinked_managed_entry_is_not_deletable_because_the_target_is_what_would_go() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = library(tmp.path());
        let real = tmp.path().join("outside/real");
        write_model(&real, 32);
        symlink_dir(&real, &lib.join("mlx/native/link"));

        assert!(remove_managed_directory(&lib, &lib.join("mlx/native/link")).is_err());
        assert!(real.join("model.safetensors").is_file());
    }
}
