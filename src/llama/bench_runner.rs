//! Offline `llama-bench` runner.
//!
//! Powers two capabilities surfaced in the Spawn Wizard / Preset Editor:
//! - **Depth sweep**: measure decode (tg) and prefill (pp) throughput at several
//!   context depths, exposing the long-context collapse that dominates agentic use.
//! - **Empirical `--n-cpu-moe` verify**: try a few offload values and report the
//!   fastest that actually runs, correcting the estimator's instant guess.
//!
//! All runs use `llama-bench -o json` so we parse structured output rather than
//! the human table. The binary is resolved as a sibling of the configured
//! `llama-server` (the llama.cpp release bundle ships both).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub const BENCH_MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchFailureKind {
    Launch,
    Timeout,
    Oom,
    NonZero,
    OutputLimit,
}

#[derive(Debug, Clone)]
pub struct BenchRunReceipt {
    pub stdout: String,
    pub stderr: String,
    pub wall_time: Duration,
    pub exit_code: Option<i32>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub failure: Option<BenchFailureKind>,
}

/// One measured point in a depth sweep.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SweepPoint {
    /// Context tokens already in the cache when the measurement was taken.
    pub depth: u64,
    /// Prefill throughput (tokens/s) at this depth (0.0 if not measured).
    pub pp_tps: f64,
    /// Decode/generation throughput (tokens/s) at this depth (0.0 if not measured).
    pub tg_tps: f64,
}

/// Result of an empirical `--n-cpu-moe` verification sweep.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NcpuMoeProbe {
    pub n_cpu_moe: i32,
    /// Short-context decode throughput (tokens/s); 0.0 means it failed to run/fit.
    pub tg_tps: f64,
}

/// Resolve the `llama-bench` binary that ships alongside `llama-server`.
pub fn llama_bench_path(server_path: &Path) -> PathBuf {
    crate::inference::llama_cpp_tools::sibling_tool_path(
        server_path,
        crate::inference::llama_cpp_tools::LlamaCppTool::Bench,
    )
}

/// Resolve the optional predictive-fit helper shipped by some llama.cpp
/// bundles. Its absence degrades predictive pruning only.
pub fn llama_fit_params_path(server_path: &Path) -> PathBuf {
    crate::inference::llama_cpp_tools::sibling_tool_path(
        server_path,
        crate::inference::llama_cpp_tools::LlamaCppTool::FitParams,
    )
}

fn fa_flag(flash_attn: bool) -> &'static str {
    if flash_attn { "1" } else { "0" }
}

/// Parse a `llama-bench -o json` array into depth-keyed points.
fn parse_sweep_json(stdout: &str) -> Result<Vec<SweepPoint>, String> {
    let arr: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("Failed to parse llama-bench JSON: {e}"))?;
    let rows = arr
        .as_array()
        .ok_or_else(|| "llama-bench JSON was not an array".to_string())?;

    use std::collections::BTreeMap;
    let mut by_depth: BTreeMap<u64, SweepPoint> = BTreeMap::new();

    for row in rows {
        // llama-bench emits numbers as JSON numbers; tolerate string forms too.
        let num = |k: &str| -> f64 {
            row.get(k)
                .and_then(|v| {
                    v.as_f64()
                        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                })
                .unwrap_or(0.0)
        };
        let depth = num("n_depth") as u64;
        let n_gen = num("n_gen") as u64;
        let n_prompt = num("n_prompt") as u64;
        let avg_ts = num("avg_ts");

        let entry = by_depth.entry(depth).or_insert(SweepPoint {
            depth,
            pp_tps: 0.0,
            tg_tps: 0.0,
        });
        if n_gen > 0 {
            entry.tg_tps = avg_ts;
        } else if n_prompt > 0 {
            entry.pp_tps = avg_ts;
        }
    }

    Ok(by_depth.into_values().collect())
}

/// Build the base llama-bench argument vector shared by sweeps and probes.
#[allow(clippy::too_many_arguments)]
fn base_args(
    model_path: &str,
    ngl: i32,
    flash_attn: bool,
    ctk: &str,
    ctv: &str,
    batch_size: u32,
    ubatch_size: u32,
    n_cpu_moe: Option<i32>,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-m".into(),
        model_path.into(),
        "-ngl".into(),
        ngl.to_string(),
        "-fa".into(),
        fa_flag(flash_attn).into(),
        "-ctk".into(),
        ctk.into(),
        "-ctv".into(),
        ctv.into(),
        "-b".into(),
        batch_size.to_string(),
        "-ub".into(),
        ubatch_size.to_string(),
        "-o".into(),
        "json".into(),
        "-r".into(),
        "1".into(),
    ];
    if let Some(n) = n_cpu_moe
        && n > 0
    {
        args.push("--n-cpu-moe".into());
        args.push(n.to_string());
    }
    args
}

async fn run_bench(
    bench_bin: &Path,
    cwd: &Path,
    args: &[String],
    timeout: Duration,
) -> Result<String, String> {
    if bench_bin.components().count() > 1 && !bench_bin.exists() {
        return Err(format!(
            "llama-bench not found at {}. It ships with the llama.cpp release alongside llama-server.",
            bench_bin.display()
        ));
    }

    let receipt = run_bench_receipt(bench_bin, cwd, args, timeout).await?;
    if let Some(failure) = receipt.failure {
        let tail: String = receipt
            .stderr
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(match failure {
            BenchFailureKind::Timeout => "llama-bench timed out".to_string(),
            BenchFailureKind::OutputLimit => {
                "llama-bench output exceeded the safety limit".to_string()
            }
            _ if tail.is_empty() => format!("llama-bench failed ({failure:?})"),
            _ => format!("llama-bench failed ({failure:?}): {tail}"),
        });
    }
    Ok(receipt.stdout)
}

/// Run a managed benchmark with bounded stdout/stderr and a structured
/// failure receipt. The existing sweep helpers retain their string API while
/// callers migrate to this safer primitive.
pub async fn run_bench_receipt(
    bench_bin: &Path,
    cwd: &Path,
    args: &[String],
    timeout: Duration,
) -> Result<BenchRunReceipt, String> {
    if !bench_bin.is_file() {
        return Err(format!(
            "llama-bench not found at {}. It ships with the llama.cpp release alongside llama-server.",
            bench_bin.display()
        ));
    }
    let started = Instant::now();
    let mut command = Command::new(bench_bin);
    command
        .current_dir(cwd)
        .args(args)
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to launch llama-bench: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "llama-bench stdout pipe unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "llama-bench stderr pipe unavailable".to_string())?;
    let capture = async move {
        let mut stdout = stdout.take(BENCH_MAX_OUTPUT_BYTES as u64);
        let mut stderr = stderr.take(BENCH_MAX_OUTPUT_BYTES as u64);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let (out_result, err_result) =
            tokio::join!(stdout.read_to_end(&mut out), stderr.read_to_end(&mut err),);
        out_result.map_err(|error| error.to_string())?;
        err_result.map_err(|error| error.to_string())?;
        let out_truncated = out.len() >= BENCH_MAX_OUTPUT_BYTES;
        let err_truncated = err.len() >= BENCH_MAX_OUTPUT_BYTES;
        Ok::<_, String>((out, err, out_truncated, err_truncated))
    };
    let (stdout, stderr, stdout_truncated, stderr_truncated) =
        match tokio::time::timeout(timeout, capture).await {
            Ok(result) => result?,
            Err(_) => {
                let _ = child.kill().await;
                return Ok(BenchRunReceipt {
                    stdout: String::new(),
                    stderr: String::new(),
                    wall_time: started.elapsed(),
                    exit_code: None,
                    stdout_truncated: false,
                    stderr_truncated: false,
                    failure: Some(BenchFailureKind::Timeout),
                });
            }
        };
    let status = child
        .wait()
        .await
        .map_err(|error| format!("Failed waiting for llama-bench: {error}"))?;
    let stdout_text = String::from_utf8_lossy(&stdout).into_owned();
    let stderr_text = String::from_utf8_lossy(&stderr).into_owned();
    let stderr_lower = stderr_text.to_ascii_lowercase();
    let failure = if stdout_truncated || stderr_truncated {
        Some(BenchFailureKind::OutputLimit)
    } else if status.success() {
        None
    } else if stderr_lower.contains("out of memory") || stderr_lower.contains("oom") {
        Some(BenchFailureKind::Oom)
    } else {
        Some(BenchFailureKind::NonZero)
    };
    Ok(BenchRunReceipt {
        stdout: stdout_text,
        stderr: stderr_text,
        wall_time: started.elapsed(),
        exit_code: status.code(),
        stdout_truncated,
        stderr_truncated,
        failure,
    })
}

/// Run a depth sweep: prefill (512) + decode (64) at each requested depth.
#[allow(clippy::too_many_arguments)]
pub async fn run_sweep(
    bench_bin: &Path,
    cwd: &Path,
    model_path: &str,
    ngl: i32,
    flash_attn: bool,
    ctk: &str,
    ctv: &str,
    batch_size: u32,
    ubatch_size: u32,
    depths: &[u64],
    n_cpu_moe: Option<i32>,
) -> Result<Vec<SweepPoint>, String> {
    run_sweep_with_tokens(
        bench_bin,
        cwd,
        model_path,
        ngl,
        flash_attn,
        ctk,
        ctv,
        batch_size,
        ubatch_size,
        depths,
        n_cpu_moe,
        512,
        64,
    )
    .await
}

/// Run a bounded depth sweep with explicit prompt and generation lengths.
/// Calibration uses this form so the workload recorded in its receipt is the
/// workload actually measured.
#[allow(clippy::too_many_arguments)]
pub async fn run_sweep_with_tokens(
    bench_bin: &Path,
    cwd: &Path,
    model_path: &str,
    ngl: i32,
    flash_attn: bool,
    ctk: &str,
    ctv: &str,
    batch_size: u32,
    ubatch_size: u32,
    depths: &[u64],
    n_cpu_moe: Option<i32>,
    prompt_tokens: u32,
    generation_tokens: u32,
) -> Result<Vec<SweepPoint>, String> {
    if depths.is_empty() {
        return Err("No depths requested".into());
    }
    let mut args = base_args(
        model_path,
        ngl,
        flash_attn,
        ctk,
        ctv,
        batch_size,
        ubatch_size,
        n_cpu_moe,
    );
    args.push("-p".into());
    args.push(prompt_tokens.to_string());
    args.push("-n".into());
    args.push(generation_tokens.to_string());
    args.push("-d".into());
    args.push(
        depths
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(","),
    );

    // Each depth requires a prefill of that many tokens; scale the budget.
    let max_depth = depths.iter().copied().max().unwrap_or(0);
    let timeout = Duration::from_secs(300 + (max_depth / 256));
    let stdout = run_bench(bench_bin, cwd, &args, timeout).await?;
    parse_sweep_json(&stdout)
}

/// Empirically probe a set of `--n-cpu-moe` values (short-context decode only)
/// and return the throughput for each. The caller picks the fastest that ran.
#[allow(clippy::too_many_arguments)]
pub async fn probe_ncpumoe(
    bench_bin: &Path,
    cwd: &Path,
    model_path: &str,
    ngl: i32,
    flash_attn: bool,
    ctk: &str,
    ctv: &str,
    batch_size: u32,
    ubatch_size: u32,
    candidates: &[i32],
) -> Vec<NcpuMoeProbe> {
    let mut out = Vec::new();
    for &n in candidates {
        let mut args = base_args(
            model_path,
            ngl,
            flash_attn,
            ctk,
            ctv,
            batch_size,
            ubatch_size,
            Some(n),
        );
        args.push("-p".into());
        args.push("0".into());
        args.push("-n".into());
        args.push("64".into());
        let tg_tps = match run_bench(bench_bin, cwd, &args, Duration::from_secs(240)).await {
            Ok(stdout) => parse_sweep_json(&stdout)
                .ok()
                .and_then(|pts| pts.first().map(|p| p.tg_tps))
                .unwrap_or(0.0),
            Err(_) => 0.0, // failed to run/fit at this offload level
        };
        out.push(NcpuMoeProbe {
            n_cpu_moe: n,
            tg_tps,
        });
    }
    out
}

/// One measured point in a batch/ubatch sweep.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchProbe {
    pub batch_size: u32,
    pub ubatch_size: u32,
    /// Prefill throughput (tokens/s); 0.0 means the run failed or didn't fit.
    pub pp_tps: f64,
}

/// Probe a set of (batch_size, ubatch_size) pairs measuring PP throughput only
/// (no decode). `prompt_tokens` should be representative of the user's actual
/// prompt length — larger values expose batch-size effects more clearly.
#[allow(clippy::too_many_arguments)]
pub async fn probe_batch(
    bench_bin: &Path,
    cwd: &Path,
    model_path: &str,
    ngl: i32,
    flash_attn: bool,
    ctk: &str,
    ctv: &str,
    candidates: &[(u32, u32)],
    prompt_tokens: u32,
    n_cpu_moe: Option<i32>,
) -> Vec<BatchProbe> {
    let mut out = Vec::new();
    for &(batch, ubatch) in candidates {
        let mut args = base_args(
            model_path, ngl, flash_attn, ctk, ctv, batch, ubatch, n_cpu_moe,
        );
        args.push("-p".into());
        args.push(prompt_tokens.to_string());
        args.push("-n".into());
        args.push("0".into()); // PP only
        args.push("-r".into());
        args.push("2".into()); // 2 runs for stability without too much wall time
        let pp_tps = match run_bench(bench_bin, cwd, &args, Duration::from_secs(120)).await {
            Ok(stdout) => parse_sweep_json(&stdout)
                .ok()
                .and_then(|pts| pts.into_iter().find(|p| p.pp_tps > 0.0))
                .map(|p| p.pp_tps)
                .unwrap_or(0.0),
            Err(_) => 0.0,
        };
        out.push(BatchProbe {
            batch_size: batch,
            ubatch_size: ubatch,
            pp_tps,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pp_and_tg_by_depth() {
        let json = r#"[
          {"n_prompt":512,"n_gen":0,"n_depth":0,"avg_ts":1500.0},
          {"n_prompt":0,"n_gen":64,"n_depth":0,"avg_ts":50.0},
          {"n_prompt":0,"n_gen":64,"n_depth":32768,"avg_ts":30.0}
        ]"#;
        let pts = parse_sweep_json(json).unwrap();
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0].depth, 0);
        assert!((pts[0].pp_tps - 1500.0).abs() < 0.01);
        assert!((pts[0].tg_tps - 50.0).abs() < 0.01);
        assert!((pts[1].tg_tps - 30.0).abs() < 0.01);
    }

    #[test]
    fn bench_path_is_sibling() {
        let p = llama_bench_path(Path::new("/opt/llama/bin/llama-server"));
        assert!(p.ends_with(if cfg!(windows) {
            "llama-bench.exe"
        } else {
            "llama-bench"
        }));
    }

    #[tokio::test]
    async fn bounded_receipt_classifies_missing_binary() {
        let result = run_bench_receipt(
            Path::new("/definitely/missing/llama-bench"),
            Path::new("."),
            &[],
            Duration::from_millis(20),
        )
        .await;
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bounded_receipt_classifies_oom_without_unbounded_output() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().expect("tempdir");
        let script = temp.path().join("fake-bench");
        std::fs::write(&script, "#!/bin/sh\necho 'out of memory' >&2\nexit 1\n").expect("script");
        let mut permissions = std::fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).expect("permissions");
        let receipt = run_bench_receipt(&script, temp.path(), &[], Duration::from_secs(5))
            .await
            .expect("receipt");
        assert_eq!(receipt.failure, Some(BenchFailureKind::Oom));
        assert!(!receipt.stdout_truncated);
        assert!(!receipt.stderr_truncated);
    }
}
