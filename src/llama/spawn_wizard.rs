//! Central coordinator for the Spawn Llama-Server wizard.
//!
//! Responsibilities:
//! - Delegates to batch_import for launch-file parsing.
//! - Delegates to vram_estimator for VRAM estimation.
//! - Provides MoE tuning suggestions.
//! - Coordinates benchmark runs.

use crate::llama::batch_import;
use crate::llama::vram_estimator;

/// Suggestions for MoE tuning based on available VRAM and model size.
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize)]
pub struct MoeTuningSuggestion {
    pub recommended_n_cpu_moe: i32,
    pub note: String,
}

/// Generate MoE tuning suggestion.
#[allow(dead_code)]
pub fn suggest_moe_tuning(
    model_size_bytes: u64,
    available_vram_bytes: u64,
    total_experts: u64,
) -> MoeTuningSuggestion {
    if total_experts == 0 || available_vram_bytes == 0 {
        return MoeTuningSuggestion {
            recommended_n_cpu_moe: 0,
            note: "Insufficient information to suggest MoE tuning.".into(),
        };
    }

    // Rough heuristic:
    // - If VRAM >= model_size * 1.2, keep all experts in VRAM (n_cpu_moe=0).
    // - If VRAM is between 0.6 and 1.2 of model_size, offload some.
    // - If VRAM is very low, offload more to CPU.
    let ratio = if model_size_bytes > 0 {
        available_vram_bytes as f64 / model_size_bytes as f64
    } else {
        0.0
    };

    let recommended = if ratio >= 1.2 {
        0
    } else if ratio >= 0.8 {
        (total_experts as f64 * 0.25) as i32
    } else if ratio >= 0.5 {
        (total_experts as f64 * 0.5) as i32
    } else {
        (total_experts as f64 * 0.75) as i32
    };

    let recommended = recommended.min(total_experts as i32);

    let note = format!(
        "Based on available VRAM, keeping {} experts in VRAM is recommended for a balance of speed and memory usage.",
        total_experts - recommended as u64
    );

    MoeTuningSuggestion {
        recommended_n_cpu_moe: recommended,
        note,
    }
}

/// Run a short benchmark on the running llama-server.
///
/// Returns a simple report with:
/// - prompt_tokens_per_second
/// - gen_tokens_per_second
/// - time_to_first_token_ms
/// - verdict: "good" / "moderate" / "poor"
/// - hints: short suggestions
///
/// This is a skeleton for now; actual HTTP calls will be wired into
/// the API layer in Phase 2.
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize)]
pub struct BenchmarkResult {
    pub prompt_tokens_per_second: f64,
    pub gen_tokens_per_second: f64,
    pub time_to_first_token_ms: f64,
    pub verdict: String,
    pub hints: Vec<String>,
}

#[allow(dead_code)]
pub fn evaluate_benchmark(prompt_tps: f64, gen_tps: f64, ttft_ms: f64) -> BenchmarkResult {
    let mut hints: Vec<String> = Vec::new();
    let verdict = if gen_tps >= 30.0 {
        "good"
    } else if gen_tps >= 10.0 {
        "moderate"
    } else {
        "poor"
    };

    if gen_tps < 10.0 {
        hints.push("Consider reducing context size or using KV cache quantization.".into());
    }
    if ttft_ms > 2000.0 {
        hints.push("High latency; check GPU utilization and VRAM usage.".into());
    }
    if prompt_tps < 500.0 {
        hints.push("Prompt throughput is low; try increasing batch/ubatch size.".into());
    }

    BenchmarkResult {
        prompt_tokens_per_second: prompt_tps,
        gen_tokens_per_second: gen_tps,
        time_to_first_token_ms: ttft_ms,
        verdict: verdict.to_string(),
        hints,
    }
}

/// Re-export convenience for API layer.
#[allow(dead_code)]
pub fn parse_launch_script(content: &str, os: &str) -> Result<batch_import::ImportResult, String> {
    batch_import::parse_launch_script(content, os)
}

/// Re-export VRAM estimation for API layer.
#[allow(clippy::too_many_arguments, dead_code)]
pub fn estimate_vram(
    model_size_bytes: u64,
    context_size: u64,
    kv_quant: &str,
    batch_size: u32,
    ubatch_size: u32,
    speculative_decoding: bool,
    mmproj_size_bytes: u64,
    n_cpu_moe: Option<i32>,
    available_vram_bytes: u64,
) -> vram_estimator::VramEstimate {
    vram_estimator::estimate_vram(
        model_size_bytes,
        context_size,
        kv_quant,
        batch_size,
        ubatch_size,
        speculative_decoding,
        mmproj_size_bytes,
        n_cpu_moe,
        available_vram_bytes,
    )
}
