//! Cross-platform path invariants for Calibration inputs.
//!
//! The upstream DOE parser measured a path before proving that it was a regular
//! file. `fopen`/`fseek`/`ftell` report different errors for directories on
//! macOS, Linux, and Windows. Calibration keeps the invariant in one native
//! helper instead: classify the directory entry first, then canonicalize only a
//! verified regular file.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegularFileError {
    NotFound,
    Symlink,
    NotRegular,
    Canonicalize,
}

/// Calibration job and receipt identifiers are generated internally, but they
/// cross an HTTP path boundary. Restrict them to a short filename-safe token
/// before joining them to any application-home directory.
pub fn is_safe_calibration_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// Verify a path is an existing, non-symlink regular file and return its
/// canonical path. The classification is platform-neutral and deliberately
/// does not expose OS error strings as an API contract.
pub fn require_regular_file(path: &Path) -> Result<PathBuf, RegularFileError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| RegularFileError::NotFound)?;
    if metadata.file_type().is_symlink() {
        return Err(RegularFileError::Symlink);
    }
    if !metadata.is_file() {
        return Err(RegularFileError::NotRegular);
    }
    path.canonicalize()
        .map_err(|_| RegularFileError::Canonicalize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn classifies_regular_file_without_platform_error_text() {
        let temp = tempdir().expect("tempdir");
        let file = temp.path().join("model.gguf");
        fs::write(&file, b"fixture").expect("write fixture");
        let canonical = require_regular_file(&file).expect("regular file");
        assert!(canonical.is_absolute());
        assert!(canonical.is_file());
    }

    #[test]
    fn rejects_directory_before_any_size_or_read_operation() {
        let temp = tempdir().expect("tempdir");
        assert_eq!(
            require_regular_file(temp.path()),
            Err(RegularFileError::NotRegular)
        );
    }

    #[test]
    fn rejects_missing_path_with_stable_classification() {
        let temp = tempdir().expect("tempdir");
        assert_eq!(
            require_regular_file(&temp.path().join("missing.gguf")),
            Err(RegularFileError::NotFound)
        );
    }

    #[test]
    fn calibration_ids_are_filename_safe() {
        assert!(is_safe_calibration_id("job-01_test"));
        assert!(!is_safe_calibration_id(""));
        assert!(!is_safe_calibration_id("../escape"));
        assert!(!is_safe_calibration_id("/absolute"));
        assert!(!is_safe_calibration_id("job with spaces"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_even_when_target_is_regular() {
        let temp = tempdir().expect("tempdir");
        let file = temp.path().join("model.gguf");
        let link = temp.path().join("link.gguf");
        fs::write(&file, b"fixture").expect("write fixture");
        std::os::unix::fs::symlink(&file, &link).expect("symlink fixture");
        assert_eq!(require_regular_file(&link), Err(RegularFileError::Symlink));
    }
}
