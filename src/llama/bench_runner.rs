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
use std::time::Duration;
use tokio::process::Command;

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
    let name = if cfg!(windows) {
        "llama-bench.exe"
    } else {
        "llama-bench"
    };
    server_path.with_file_name(name)
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

    let mut cmd = Command::new(bench_bin);
    cmd.current_dir(cwd);
    cmd.args(args);
    cmd.kill_on_drop(true);

    let output = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| "llama-bench timed out".to_string())?
        .map_err(|e| format!("Failed to launch llama-bench: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr.lines().rev().take(5).collect::<Vec<_>>().join(" | ");
        return Err(format!("llama-bench exited with error: {tail}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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
    args.push("512".into());
    args.push("-n".into());
    args.push("64".into());
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
}
