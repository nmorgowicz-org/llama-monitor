#![allow(clippy::collapsible_if)]
use crate::inference::rapid_mlx::runtime::RuntimeSource;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

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
        if let Some(path) = explicit_path
            && path.exists()
        {
            return Ok((path.to_path_buf(), RuntimeSource::Custom));
        }

        if let Some(path) = managed_path
            && path.exists()
        {
            return Ok((path.to_path_buf(), RuntimeSource::Managed));
        }

        // Fallback to PATH
        if let Ok(path) = which::which("rapid-mlx") {
            // Try to determine if it's homebrew or pip
            let source = Self::determine_source(&path).await;
            return Ok((path, source));
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "rapid-mlx binary not found on PATH or in configured locations",
        ))
    }

    async fn determine_source(path: &Path) -> RuntimeSource {
        let path_str = path.to_string_lossy();
        if path_str.contains("homebrew") || path_str.contains("/opt/homebrew") {
            RuntimeSource::Homebrew
        } else if path_str.contains("site-packages") || path_str.contains(".local/bin") {
            RuntimeSource::Pip
        } else {
            RuntimeSource::PathUnknown
        }
    }

    /// Probes the binary to ensure it is usable.
    pub async fn probe_version(binary_path: &Path) -> io::Result<String> {
        let output = Command::new(binary_path).arg("--version").output()?;

        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(version)
        } else {
            Err(io::Error::other(format!(
                "rapid-mlx --version failed with status: {}",
                output.status
            )))
        }
    }
}
