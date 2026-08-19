//! Typed, capability-gated llama-bench argv construction.
//!
//! The executor accepts this vector as structured arguments.  It never
//! accepts a shell command or writes candidate values into `extra_args`.

use super::LlamaCppCalibrationPatch;
use anyhow::{Result, bail};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct BenchInvocation<'a> {
    pub model_path: &'a Path,
    pub prompt_tokens: u32,
    pub generation_tokens: u32,
    pub depth: u64,
    pub repetitions: u32,
    pub base_context_size: u64,
    pub base_batch_size: u32,
    pub base_ubatch_size: u32,
    pub base_gpu_layers: i32,
    pub base_threads: Option<i32>,
    pub base_ctk: &'a str,
    pub base_ctv: &'a str,
    pub base_flash_attn: bool,
}

fn flag_supported(flags: &BTreeSet<String>, short: &str, long: &str) -> bool {
    flags.contains(short) || flags.contains(long)
}

fn push_pair(args: &mut Vec<OsString>, flag: &str, value: impl Into<OsString>) {
    args.push(OsString::from(flag));
    args.push(value.into());
}

/// Validate bounds and materialize a single benchmark invocation.
pub fn build_bench_argv(
    invocation: BenchInvocation<'_>,
    patch: &LlamaCppCalibrationPatch,
    supported_flags: &BTreeSet<String>,
) -> Result<Vec<OsString>> {
    if invocation.prompt_tokens == 0 || invocation.generation_tokens == 0 {
        bail!("benchmark workload must request prompt and generated tokens");
    }
    if invocation.depth == 0 || invocation.depth > invocation.base_context_size {
        bail!("benchmark depth must be within the configured context");
    }
    if invocation.repetitions == 0 {
        bail!("benchmark repetitions must be non-zero");
    }
    let context_size = patch.context_size.unwrap_or(invocation.base_context_size);
    let batch_size = patch.batch_size.unwrap_or(invocation.base_batch_size);
    let ubatch_size = patch.ubatch_size.unwrap_or(invocation.base_ubatch_size);
    if context_size == 0 || context_size < invocation.depth {
        bail!("candidate context must contain benchmark depth");
    }
    if batch_size == 0 || ubatch_size == 0 || ubatch_size > batch_size {
        bail!("candidate requires 0 < ubatch_size <= batch_size");
    }
    let gpu_layers = patch.gpu_layers.unwrap_or(invocation.base_gpu_layers);
    if gpu_layers < -1 {
        bail!("gpu layer count must be -1 or non-negative");
    }
    if (patch.threads.is_some() || invocation.base_threads.is_some())
        && !flag_supported(supported_flags, "-t", "--threads")
    {
        bail!("candidate threads factor is not advertised by llama-bench --help");
    }
    if (patch.flash_attn.is_some() || invocation.base_flash_attn)
        && !flag_supported(supported_flags, "-fa", "--flash-attn")
    {
        bail!("candidate flash-attention factor is not advertised by llama-bench --help");
    }
    if patch.n_cpu_moe.is_some() && !flag_supported(supported_flags, "-ncmoe", "--n-cpu-moe") {
        bail!("candidate MoE offload factor is not advertised by llama-bench --help");
    }

    let mut args = Vec::with_capacity(28);
    push_pair(&mut args, "-m", invocation.model_path.as_os_str());
    push_pair(&mut args, "-p", invocation.prompt_tokens.to_string());
    push_pair(&mut args, "-n", invocation.generation_tokens.to_string());
    push_pair(&mut args, "-d", invocation.depth.to_string());
    push_pair(&mut args, "-b", batch_size.to_string());
    push_pair(&mut args, "-ub", ubatch_size.to_string());
    push_pair(&mut args, "-ngl", gpu_layers.to_string());
    push_pair(
        &mut args,
        "-ctk",
        patch.ctk.as_deref().unwrap_or(invocation.base_ctk),
    );
    push_pair(
        &mut args,
        "-ctv",
        patch.ctv.as_deref().unwrap_or(invocation.base_ctv),
    );
    if let Some(threads) = patch.threads.or(invocation.base_threads) {
        push_pair(&mut args, "-t", threads.to_string());
    }
    if let Some(flash_attn) = patch
        .flash_attn
        .or(invocation.base_flash_attn.then_some(true))
    {
        push_pair(&mut args, "-fa", if flash_attn { "1" } else { "0" });
    }
    if let Some(n_cpu_moe) = patch.n_cpu_moe {
        push_pair(&mut args, "-ncmoe", n_cpu_moe.to_string());
    }
    push_pair(&mut args, "-o", "json");
    push_pair(&mut args, "-r", invocation.repetitions.to_string());
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::llama_cpp_tools::{LlamaCppTool, sibling_tool_path_for};
    use std::path::PathBuf;

    fn flags() -> BTreeSet<String> {
        [
            "-t",
            "--threads",
            "-fa",
            "--flash-attn",
            "-ncmoe",
            "--n-cpu-moe",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }

    #[test]
    fn golden_argv_keeps_paths_with_spaces_as_one_argument() {
        let model = PathBuf::from("C:/Models/My GGUF/model.gguf");
        let args = build_bench_argv(
            BenchInvocation {
                model_path: &model,
                prompt_tokens: 512,
                generation_tokens: 256,
                depth: 4096,
                repetitions: 2,
                base_context_size: 8192,
                base_batch_size: 2048,
                base_ubatch_size: 512,
                base_gpu_layers: -1,
                base_threads: Some(8),
                base_ctk: "q8_0",
                base_ctv: "q8_0",
                base_flash_attn: false,
            },
            &LlamaCppCalibrationPatch {
                batch_size: Some(1024),
                ubatch_size: Some(256),
                flash_attn: Some(true),
                n_cpu_moe: Some(4),
                ..Default::default()
            },
            &flags(),
        )
        .expect("valid argv");
        assert_eq!(args[0], OsString::from("-m"));
        assert_eq!(args[1], model.as_os_str());
        assert!(
            args.windows(2)
                .any(|pair| pair == [OsString::from("-ub"), OsString::from("256")])
        );
        assert_eq!(
            sibling_tool_path_for(
                Path::new("C:/bundle/llama-server.exe"),
                LlamaCppTool::Bench,
                true
            ),
            PathBuf::from("C:/bundle/llama-bench.exe")
        );
    }

    #[test]
    fn invalid_batch_relationship_and_unsupported_factors_fail_closed() {
        let model = PathBuf::from("model.gguf");
        let invocation = BenchInvocation {
            model_path: &model,
            prompt_tokens: 1,
            generation_tokens: 1,
            depth: 1,
            repetitions: 1,
            base_context_size: 2,
            base_batch_size: 2,
            base_ubatch_size: 2,
            base_gpu_layers: 1,
            base_threads: None,
            base_ctk: "q8_0",
            base_ctv: "q8_0",
            base_flash_attn: false,
        };
        assert!(
            build_bench_argv(
                invocation,
                &LlamaCppCalibrationPatch {
                    ubatch_size: Some(3),
                    ..Default::default()
                },
                &BTreeSet::new()
            )
            .is_err()
        );
        assert!(
            build_bench_argv(
                invocation,
                &LlamaCppCalibrationPatch {
                    flash_attn: Some(true),
                    ..Default::default()
                },
                &BTreeSet::new()
            )
            .is_err()
        );
    }
}
