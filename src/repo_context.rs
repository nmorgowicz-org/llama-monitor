//! Whether this process is running out of a llama-monitor source checkout.
//!
//! Some diagnostics have a repo-side follow-up: a harness lane to run, a source
//! constant to update, an evidence doc to record in. Those are actionable if and only
//! if the reader has the checkout. Showing them to everyone else hands out instructions
//! that cannot be followed, which is worse than saying nothing.
//!
//! The test is deliberately not the build profile. `cargo build --release` inside the
//! checkout and a shipped release binary are the same profile, and a debug build could
//! still be copied somewhere with no sources beside it. What actually matters is
//! whether the files a diagnostic points at exist, so that is what gets checked:
//! walk up from the running executable looking for this crate's manifest with a
//! `scripts/` directory beside it. `target/debug/llama-monitor` and
//! `target/release/llama-monitor` both find it; an installed copy does not.
//!
//! `CARGO_MANIFEST_DIR` is not used: it is baked in at compile time and names a path on
//! the build machine, so on any other machine it is a confident wrong answer.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Name this crate's manifest must declare for an ancestor directory to count. A
/// checkout of some other project in the executable's ancestry is not our checkout.
const CRATE_NAMES: &[&str] = &["local-llm-foundry", "llama-monitor"];

/// How far up to look. `target/release/llama-monitor` is two levels below the root;
/// the extra room covers `target/<triple>/release/` cross-compilation layouts without
/// letting the search wander to the filesystem root on an installed binary.
const MAX_ANCESTORS: usize = 6;

/// A located source checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoContext {
    root: PathBuf,
}

impl RepoContext {
    /// Absolute path to a repo-relative file, or `None` if it is not there.
    ///
    /// Returning `None` for a missing file is the point: a diagnostic that names a
    /// harness script should verify the script exists before telling anyone to run it,
    /// because a renamed or deleted script is exactly the case where the instruction
    /// would waste the reader's time.
    pub fn file(&self, relative: &str) -> Option<PathBuf> {
        let path = self.root.join(relative);
        path.is_file().then_some(path)
    }
}

/// The checkout this process is running from, if any. Cached: the answer cannot change
/// while the process lives.
pub fn detect() -> Option<&'static RepoContext> {
    static DETECTED: OnceLock<Option<RepoContext>> = OnceLock::new();
    DETECTED
        .get_or_init(|| {
            let exe = std::env::current_exe().ok()?;
            detect_from(&exe)
        })
        .as_ref()
}

/// Walk up from `executable` looking for this crate's checkout root.
fn detect_from(executable: &Path) -> Option<RepoContext> {
    executable
        .ancestors()
        .skip(1)
        .take(MAX_ANCESTORS)
        .find(|dir| is_checkout_root(dir))
        .map(|root| RepoContext {
            root: root.to_path_buf(),
        })
}

/// A directory is our checkout root when it holds this crate's manifest and the
/// `scripts/` directory the maintainer-facing diagnostics point into.
fn is_checkout_root(dir: &Path) -> bool {
    if !dir.join("scripts").is_dir() {
        return false;
    }
    let Ok(manifest) = std::fs::read_to_string(dir.join("Cargo.toml")) else {
        return false;
    };
    manifest.lines().any(|line| {
        let line = line.trim();
        line.starts_with("name")
            && line.contains('=')
            && line.split('=').nth(1).is_some_and(|value| {
                CRATE_NAMES
                    .iter()
                    .any(|name| value.trim().trim_matches(|c| c == '"' || c == '\'') == *name)
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_checkout(root: &Path, crate_name: &str) {
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            format!("[package]\nname = \"{crate_name}\"\nversion = \"1.0.0\"\n"),
        )
        .unwrap();
    }

    fn place_exe(root: &Path, profile: &str) -> PathBuf {
        let dir = root.join("target").join(profile);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("llama-monitor");
        std::fs::write(&exe, b"binary").unwrap();
        exe
    }

    #[test]
    fn a_release_build_inside_the_checkout_is_a_repo_context() {
        let dir = tempfile::tempdir().unwrap();
        make_checkout(dir.path(), "local-llm-foundry");
        // The case the build profile alone would get wrong: release, but in-tree.
        let exe = place_exe(dir.path(), "release");
        assert_eq!(detect_from(&exe).unwrap().root, dir.path());
    }

    #[test]
    fn a_debug_build_inside_the_checkout_is_a_repo_context() {
        let dir = tempfile::tempdir().unwrap();
        make_checkout(dir.path(), "local-llm-foundry");
        assert!(detect_from(&place_exe(dir.path(), "debug")).is_some());
    }

    #[test]
    fn a_cross_compiled_target_triple_layout_still_finds_the_root() {
        let dir = tempfile::tempdir().unwrap();
        make_checkout(dir.path(), "local-llm-foundry");
        assert!(detect_from(&place_exe(dir.path(), "aarch64-apple-darwin/release")).is_some());
    }

    #[test]
    fn an_installed_binary_with_no_sources_beside_it_is_not_a_repo_context() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("usr").join("local").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let exe = bin.join("llama-monitor");
        std::fs::write(&exe, b"binary").unwrap();
        assert!(detect_from(&exe).is_none());
    }

    #[test]
    fn somebody_elses_checkout_in_the_ancestry_does_not_count() {
        let dir = tempfile::tempdir().unwrap();
        make_checkout(dir.path(), "some-other-crate");
        assert!(
            detect_from(&place_exe(dir.path(), "release")).is_none(),
            "a manifest for a different crate is not this crate's checkout"
        );
    }

    #[test]
    fn a_manifest_without_scripts_beside_it_does_not_count() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"local-llm-foundry\"\n",
        )
        .unwrap();
        assert!(detect_from(&place_exe(dir.path(), "release")).is_none());
    }

    #[test]
    fn a_named_file_is_only_reported_when_it_exists() {
        let dir = tempfile::tempdir().unwrap();
        make_checkout(dir.path(), "local-llm-foundry");
        let context = detect_from(&place_exe(dir.path(), "release")).unwrap();
        assert!(context.file("scripts/not-there.mjs").is_none());
        std::fs::write(dir.path().join("scripts").join("lane.mjs"), b"//").unwrap();
        assert_eq!(
            context.file("scripts/lane.mjs"),
            Some(context.root.join("scripts/lane.mjs"))
        );
    }
}
