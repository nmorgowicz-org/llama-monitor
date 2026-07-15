#![allow(clippy::collapsible_if)]
use crate::inference::rapid_mlx::runtime::RuntimeSource;
use std::io;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub struct Discovery;

#[allow(dead_code)]
impl Discovery {
    /// Resolves the rapid-mlx binary based on precedence:
    /// 1. Explicit path
    /// 2. Managed environment
    /// 3. PATH
    pub async fn resolve_binary(
        explicit_path: Option<&Path>,
        managed_path: Option<&Path>,
    ) -> io::Result<(PathBuf, RuntimeSource)> {
        if let Some(path) = explicit_path {
            if path.is_file() {
                return Ok((path.to_path_buf(), RuntimeSource::Custom));
            }
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "configured Rapid-MLX executable is not a file: {}",
                    path.display()
                ),
            ));
        }

        if let Some(path) = managed_path {
            if path.is_file() {
                return Ok((path.to_path_buf(), RuntimeSource::Managed));
            }
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "managed Rapid-MLX executable is not a file: {}",
                    path.display()
                ),
            ));
        }

        for binary in ["rapid-mlx", "vllm-mlx"] {
            if let Ok(path) = which::which(binary) {
                let source = Self::classify_source(&path);
                return Ok((path, source));
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "rapid-mlx binary not found on PATH or in configured locations",
        ))
    }

    pub fn classify_source(path: &Path) -> RuntimeSource {
        let path_str = path.to_string_lossy();
        if path_str.contains("/opt/homebrew")
            || path_str.contains("/home/linuxbrew")
            || path_str.contains("/Cellar/")
        {
            RuntimeSource::Homebrew
        } else if path_str.contains("/pipx/venvs/") || path_str.contains("/.local/pipx/") {
            RuntimeSource::Pipx
        } else if path_str.contains("site-packages")
            || path_str.contains("/.venv/bin/")
            || path_str.contains("/venv/bin/")
        {
            RuntimeSource::Pip
        } else {
            RuntimeSource::PathUnknown
        }
    }
}
