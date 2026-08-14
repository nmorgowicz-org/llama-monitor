//! Resolution and capability evidence for tools shipped with llama.cpp.
//!
//! Calibration must use the exact managed bundle selected by
//! `AppConfig::llama_server_path`.  Looking up a tool on `PATH` would make a
//! receipt non-reproducible (and could silently select a different build).

use crate::inference::llama_cpp_capabilities::ExecutableIdentity;
use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub const TOOL_HELP_TIMEOUT: Duration = Duration::from_secs(8);
pub const TOOL_HELP_MAX_OUTPUT: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlamaCppTool {
    Bench,
    FitParams,
}

impl LlamaCppTool {
    pub const fn stem(self) -> &'static str {
        match self {
            Self::Bench => "llama-bench",
            Self::FitParams => "llama-fit-params",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTool {
    pub tool: LlamaCppTool,
    pub path: PathBuf,
    pub identity: ExecutableIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolHelpEvidence {
    pub tool: LlamaCppTool,
    pub identity: ExecutableIdentity,
    pub help_sha256: String,
    pub flags: BTreeSet<String>,
    pub exit_code: Option<i32>,
    pub output_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionalFitParams {
    Available(ResolvedTool),
    Missing,
    Unusable(String),
}

/// Return a sibling path without consulting `PATH` or an application-home
/// default.  `windows` is injectable so path/argv contracts can be tested on
/// every host.
pub fn sibling_tool_path_for(server_path: &Path, tool: LlamaCppTool, windows: bool) -> PathBuf {
    let filename = if windows {
        format!("{}.exe", tool.stem())
    } else {
        tool.stem().to_string()
    };
    server_path.with_file_name(filename)
}

pub fn sibling_tool_path(server_path: &Path, tool: LlamaCppTool) -> PathBuf {
    sibling_tool_path_for(server_path, tool, cfg!(windows))
}

/// Resolve a managed sibling and reject symlinks/special files before hashing.
pub fn resolve_tool(server_path: &Path, tool: LlamaCppTool) -> Result<ResolvedTool> {
    let path = sibling_tool_path(server_path, tool);
    let metadata = std::fs::symlink_metadata(&path)
        .with_context(|| format!("managed {} is unavailable", tool.stem()))?;
    if metadata.file_type().is_symlink() {
        bail!("managed {} sibling must not be a symlink", tool.stem());
    }
    if !metadata.is_file() {
        bail!("managed {} sibling is not a regular file", tool.stem());
    }
    let identity = ExecutableIdentity::from_path(&path)
        .with_context(|| format!("cannot fingerprint managed {}", tool.stem()))?;
    Ok(ResolvedTool {
        tool,
        path,
        identity,
    })
}

pub fn optional_fit_params(server_path: &Path) -> OptionalFitParams {
    match resolve_tool(server_path, LlamaCppTool::FitParams) {
        Ok(tool) => OptionalFitParams::Available(tool),
        Err(error) if error.to_string().contains("unavailable") => OptionalFitParams::Missing,
        Err(error) => OptionalFitParams::Unusable(error.to_string()),
    }
}

/// Parse only option tokens from help output.  Human prose is deliberately
/// not part of the contract; callers use the resulting exact flags.
pub fn flags_from_help(help: &str) -> BTreeSet<String> {
    help.lines()
        .flat_map(str::split_whitespace)
        .filter_map(|token| {
            let token = token.trim_matches(|c: char| c == ',' || c == ')' || c == ']' || c == ':');
            let flag = token.split_once('=').map_or(token, |(flag, _)| flag);
            (flag.starts_with('-') && flag.len() > 1).then(|| flag.to_string())
        })
        .collect()
}

pub async fn probe_help(tool: &ResolvedTool, cwd: &Path) -> Result<ToolHelpEvidence> {
    let mut command = Command::new(&tool.path);
    command
        .arg("--help")
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to launch managed {}", tool.tool.stem()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("missing help stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("missing help stderr"))?;
    let capture = async move {
        let mut stdout = stdout.take(TOOL_HELP_MAX_OUTPUT as u64);
        let mut stderr = stderr.take(TOOL_HELP_MAX_OUTPUT as u64);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let (out_result, err_result) =
            tokio::join!(stdout.read_to_end(&mut out), stderr.read_to_end(&mut err),);
        out_result?;
        err_result?;
        let truncated = out.len() >= TOOL_HELP_MAX_OUTPUT || err.len() >= TOOL_HELP_MAX_OUTPUT;
        Ok::<_, std::io::Error>((out, err, truncated))
    };
    let (stdout, stderr, output_truncated) = tokio::time::timeout(TOOL_HELP_TIMEOUT, capture)
        .await
        .map_err(|_| anyhow!("managed {} --help timed out", tool.tool.stem()))??;
    let status = child.wait().await?;
    let mut combined = stdout;
    combined.extend_from_slice(&stderr);
    let help_sha256 = Sha256::digest(&combined)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(ToolHelpEvidence {
        tool: tool.tool,
        identity: tool.identity.clone(),
        help_sha256,
        flags: flags_from_help(&String::from_utf8_lossy(&combined)),
        exit_code: status.code(),
        output_truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sibling_names_are_platform_specific_but_root_preserving() {
        let server = Path::new("/opt/managed bundle/llama-server");
        assert_eq!(
            sibling_tool_path_for(server, LlamaCppTool::Bench, false),
            PathBuf::from("/opt/managed bundle/llama-bench")
        );
        assert_eq!(
            sibling_tool_path_for(server, LlamaCppTool::FitParams, true),
            PathBuf::from("/opt/managed bundle/llama-fit-params.exe")
        );
    }

    #[test]
    fn help_flags_are_exact_and_deterministic() {
        let flags = flags_from_help("-m FILE  -b=512 --flash-attn, --n-cpu-moe <n>");
        assert!(flags.contains("-m"));
        assert!(flags.contains("-b"));
        assert!(flags.contains("--flash-attn"));
        assert!(flags.contains("--n-cpu-moe"));
        assert!(!flags.contains("FILE"));
    }

    #[test]
    fn missing_optional_fit_helper_degrades_without_hiding_bench() {
        let temp = tempfile::tempdir().expect("tempdir");
        let server = temp.path().join("llama-server");
        let bench = temp.path().join("llama-bench");
        std::fs::write(&server, b"server").expect("server fixture");
        std::fs::write(&bench, b"bench").expect("bench fixture");
        let resolved = resolve_tool(&server, LlamaCppTool::Bench).expect("bench sibling");
        assert_eq!(resolved.path, bench);
        assert!(matches!(
            optional_fit_params(&server),
            OptionalFitParams::Missing
        ));
    }
}
