//! Central coordinator for the Spawn Llama-Server wizard.
//!
//! Responsibilities:
//! - Delegates to batch_import for launch-file parsing.
//! - Delegates to vram_estimator for VRAM estimation.
//! - Provides MoE tuning suggestions.
//! - Coordinates benchmark runs.
//! - Third-party model import helpers.
//! - Model introspection via llama.cpp.

use crate::llama::batch_import;
use crate::llama::vram_estimator;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Suggestions for MoE tuning based on available VRAM and model size.
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize)]
pub struct MoeTuningSuggestion {
    pub recommended_n_cpu_moe: i32,
    pub note: String,
}

/// Generate MoE tuning suggestion.
pub fn suggest_moe_tuning(
    model_size_bytes: u64,
    available_vram_bytes: u64,
    total_experts: u64,
) -> MoeTuningSuggestion {
    // Conservative default when info is missing
    if total_experts == 0 || available_vram_bytes == 0 || model_size_bytes == 0 {
        return MoeTuningSuggestion {
            recommended_n_cpu_moe: 2,
            note: "Limited information provided; using a conservative MoE setting. Adjust based on observed VRAM usage.".into(),
        };
    }

    // Rough heuristic:
    // - If VRAM >= model_size * 1.2, keep all experts in VRAM (n_cpu_moe=0).
    // - If VRAM is between 0.6 and 1.2 of model_size, offload some.
    // - If VRAM is very low, offload more to CPU.
    let ratio = available_vram_bytes as f64 / model_size_bytes as f64;

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
        total_experts.saturating_sub(recommended as u64)
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
/// Parameters:
/// - model_size_bytes, available_vram_bytes, total_experts are optional
///   and used to generate MoE/VRAM-aware hints.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BenchmarkResult {
    pub prompt_tokens_per_second: f64,
    pub gen_tokens_per_second: f64,
    pub time_to_first_token_ms: f64,
    pub verdict: String,
    pub hints: Vec<String>,
}

pub fn run_benchmark(
    prompt_tps: f64,
    gen_tps: f64,
    ttft_ms: f64,
    model_size_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    total_experts: u64,
) -> BenchmarkResult {
    let mut hints: Vec<String> = Vec::new();

    // Verdict logic:
    // "good": gen_tps >= 30 and ttft_ms <= 800
    // "moderate": gen_tps between 10-30 or ttft_ms between 800-2000
    // "poor": gen_tps < 10 or ttft_ms > 2000
    let verdict = if gen_tps >= 30.0 && ttft_ms <= 800.0 {
        "good"
    } else if (10.0..30.0).contains(&gen_tps) || (800.0..=2000.0).contains(&ttft_ms) {
        "moderate"
    } else {
        "poor"
    };

    // Hints: generation throughput
    if gen_tps < 10.0 {
        hints.push("Consider increasing GPU layers or reducing context size.".to_string());
    }

    // Hints: latency
    if ttft_ms > 2000.0 {
        hints.push("High latency; reduce context size or enable flash attention.".to_string());
    }

    // Hints: prompt throughput
    if prompt_tps < 500.0 {
        hints.push(
            "Slow prompt processing; increase batch size or use a faster backend.".to_string(),
        );
    }

    // MoE-specific hints
    if total_experts > 0 {
        let (model_size, vram) = match (model_size_bytes, available_vram_bytes) {
            (Some(m), Some(v)) => (m, v),
            _ => {
                hints.push(
                    "For MoE models, try increasing n_cpu_moe within available VRAM.".to_string(),
                );
                return BenchmarkResult {
                    prompt_tokens_per_second: prompt_tps,
                    gen_tokens_per_second: gen_tps,
                    time_to_first_token_ms: ttft_ms,
                    verdict: verdict.to_string(),
                    hints,
                };
            }
        };

        let suggestion = suggest_moe_tuning(model_size, vram, total_experts);
        if suggestion.recommended_n_cpu_moe > 0 {
            hints.push(format!(
                "For MoE models, try increasing n_cpu_moe to {} within available VRAM.",
                suggestion.recommended_n_cpu_moe
            ));
        }
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

// ── P3.2: Third-Party Model Import ────────────────────────────────────────────

/// Return common third-party model directories for the current platform.
///
/// Only directories that actually exist are included.
pub fn get_common_model_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // Ollama
        if let Some(home) = std::env::var_os("HOME") {
            let ollama = PathBuf::from(home.clone())
                .join("Library")
                .join("Application Support")
                .join("Ollama")
                .join("models");
            if ollama.is_dir() {
                dirs.push(ollama);
            }

            // LM Studio
            let lm = PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("LM Studio");
            if lm.is_dir() {
                dirs.push(lm);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Ollama
        if let Some(home) = std::env::var_os("HOME") {
            let ollama = PathBuf::from(home.clone()).join(".ollama").join("models");
            if ollama.is_dir() {
                dirs.push(ollama);
            }

            // LM Studio
            let lm = PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("lm-studio");
            if lm.is_dir() {
                dirs.push(lm);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Ollama
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let ollama = PathBuf::from(local).join("Ollama").join("models");
            if ollama.is_dir() {
                dirs.push(ollama);
            }

            // LM Studio
            let lm = PathBuf::from(local).join("LM Studio");
            if lm.is_dir() {
                dirs.push(lm);
            }
        }
    }

    dirs
}

/// Recursively scan given directories for .gguf files (max depth 3).
pub fn find_gguf_in_dirs(dirs: &[PathBuf], include_subdirs: bool) -> Vec<PathBuf> {
    let mut results = Vec::new();
    for dir in dirs {
        if !include_subdirs {
            // Only top-level .gguf files.
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && path
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
                    {
                        results.push(path);
                    }
                }
            }
        } else {
            walk_gguf(dir, 0, 3, &mut results);
        }
    }
    results.sort();
    results.dedup();
    results
}

fn walk_gguf(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
    if depth > max_depth {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
        {
            out.push(path);
        } else if path.is_dir() {
            walk_gguf(&path, depth + 1, max_depth, out);
        }
    }
}

// ── P3.3: Model Introspection ─────────────────────────────────────────────────

/// Metadata extracted from llama.cpp introspection.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ModelMetadata {
    pub n_layers: Option<u32>,
    pub n_ctx_train: Option<u32>,
    pub n_embd: Option<u32>,
    pub n_ff: Option<u32>,
    pub n_exp: Option<u32>,
    pub mmproj_required: bool,
    pub cached: bool,
}

/// Run llama-server --print-model-metadata and parse output.
///
/// Caches results in config_dir/model-cache/<sha256_of_model_path>.json.
pub async fn introspect_model(
    model_path: &str,
    llama_server_path: &str,
) -> Result<ModelMetadata, String> {
    // Check cache first.
    if let Ok(cached) = load_model_cache(model_path) {
        return Ok(ModelMetadata {
            cached: true,
            ..cached
        });
    }

    // Run llama-server --print-model-metadata.
    let output = tokio::task::spawn_blocking({
        let lp = llama_server_path.to_string();
        let mp = model_path.to_string();
        move || {
            let mut cmd = Command::new(&lp);
            cmd.arg("--print-model-metadata")
                .arg("--model")
                .arg(&mp)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            let child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    return Err(format!("Failed to run llama-server: {}", e));
                }
            };
            let out = match child.wait_with_output() {
                Ok(o) => o,
                Err(e) => {
                    return Err(format!("Failed to wait for llama-server: {}", e));
                }
            };
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let combined = format!("{}{}", stdout, stderr);
            Ok(combined)
        }
    })
    .await
    .map_err(|e| format!("Introspection task failed: {}", e))?
    .map_err(|e| format!("Introspection failed: {}", e))?;

    let meta = parse_model_metadata(&output);

    // Cache result.
    let _ = save_model_cache(model_path, &meta);

    Ok(meta)
}

fn parse_model_metadata(output: &str) -> ModelMetadata {
    let mut meta = ModelMetadata::default();

    let lines: Vec<&str> = output.lines().collect();

    for line in &lines {
        let trimmed = line.trim();
        // n_layers
        if let Some(val) = extract_int_after("n_layers", trimmed) {
            meta.n_layers = Some(val);
        }
        // n_ctx_train
        if let Some(val) = extract_int_after("n_ctx_train", trimmed) {
            meta.n_ctx_train = Some(val);
        }
        // n_embd
        if let Some(val) = extract_int_after("n_embd", trimmed) {
            meta.n_embd = Some(val);
        }
        // n_ff
        if let Some(val) = extract_int_after("n_ff", trimmed) {
            meta.n_ff = Some(val);
        }
        // n_exp (MoE)
        if let Some(val) = extract_int_after("n_exp", trimmed) {
            meta.n_exp = Some(val);
        }
        // mmproj hints
        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("mmproj")
            || lower.contains("vision")
            || lower.contains("clip")
            || lower.contains("multimodal")
        {
            meta.mmproj_required = true;
        }
    }

    meta
}

fn extract_int_after(prefix: &str, line: &str) -> Option<u32> {
    let line = line.trim();
    let idx = line.find(prefix)?;
    let after = &line[idx + prefix.len()..];
    let after = after.trim_start();
    // Expect pattern like "n_layers: 32" or "n_layers = 32"
    let after = after
        .strip_prefix(':')
        .or_else(|| after.strip_prefix('='))
        .unwrap_or(after)
        .trim_start();
    after.split_whitespace().next()?.parse().ok()
}

fn model_cache_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Home directory not found".to_string())?;
    let dir = home
        .join(".config")
        .join("llama-monitor")
        .join("model-cache");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create model-cache dir: {}", e))?;
    Ok(dir)
}

fn model_cache_path(model_path: &str) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    let dir = model_cache_dir()?;
    let mut hasher = Sha256::new();
    hasher.update(model_path.as_bytes());
    let hash = hasher.finalize();
    let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    Ok(dir.join(format!("{}.json", &hex[..16])))
}

fn load_model_cache(model_path: &str) -> Result<ModelMetadata, String> {
    let path = model_cache_path(model_path)?;
    if !path.exists() {
        return Err("No cache".to_string());
    }
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read cache: {}", e))?;
    let meta: ModelMetadata =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse cache: {}", e))?;
    Ok(meta)
}

fn save_model_cache(model_path: &str, meta: &ModelMetadata) -> Result<(), String> {
    let path = model_cache_path(model_path)?;
    let json = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("Failed to serialize cache: {}", e))?;
    std::fs::write(&path, &json).map_err(|e| format!("Failed to write cache: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_common_model_dirs_is_platform_specific() {
        let dirs = get_common_model_dirs();
        // Must not panic; platform-specific dirs may be empty.
        for d in &dirs {
            assert!(d.is_dir(), "Returned dir should exist: {:?}", d);
        }
    }

    #[test]
    fn test_find_gguf_in_dirs_basic() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();

        // Create files.
        fs::write(root.join("model.gguf"), "x").unwrap();
        fs::write(root.join("readme.txt"), "x").unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub").join("model2.gguf"), "x").unwrap();

        let ggufs = find_gguf_in_dirs(&[root.to_path_buf()], true);
        assert_eq!(ggufs.len(), 2);
        assert!(
            ggufs
                .iter()
                .any(|p| p.file_name().map_or(false, |n| n == "model.gguf"))
        );
        assert!(
            ggufs
                .iter()
                .any(|p| p.file_name().map_or(false, |n| n == "model2.gguf"))
        );
    }

    #[test]
    fn test_find_gguf_in_dirs_no_subdirs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();

        fs::write(root.join("model.gguf"), "x").unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub").join("model2.gguf"), "x").unwrap();

        let ggufs = find_gguf_in_dirs(&[root.to_path_buf()], false);
        assert_eq!(ggufs.len(), 1);
        assert!(ggufs[0].file_name().map_or(false, |n| n == "model.gguf"));
    }

    #[test]
    fn test_parse_model_metadata_basic() {
        let output = "
            n_layers: 32
            n_ctx_train: 8192
            n_embd: 4096
            n_ff: 14336
            n_exp: 8
        ";
        let meta = parse_model_metadata(output);
        assert_eq!(meta.n_layers, Some(32));
        assert_eq!(meta.n_ctx_train, Some(8192));
        assert_eq!(meta.n_embd, Some(4096));
        assert_eq!(meta.n_ff, Some(14336));
        assert_eq!(meta.n_exp, Some(8));
        assert!(!meta.mmproj_required);
    }

    #[test]
    fn test_parse_model_metadata_mmproj_hint() {
        let output = "
            n_layers: 24
            mmproj: vision-encoder.gguf
        ";
        let meta = parse_model_metadata(output);
        assert!(meta.mmproj_required);
    }

    #[test]
    fn test_extract_int_after() {
        assert_eq!(extract_int_after("n_layers", "n_layers: 32"), Some(32));
        assert_eq!(extract_int_after("n_layers", "n_layers = 32"), Some(32));
        assert_eq!(extract_int_after("n_layers", "n_layers:xyz"), None);
        assert_eq!(extract_int_after("n_layers", "other"), None);
    }

    #[test]
    fn test_model_metadata_serde_default() {
        let json = r#"{"n_layers":16}"#;
        let meta: ModelMetadata = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(meta.n_layers, Some(16));
        assert_eq!(meta.n_ctx_train, None);
        assert!(!meta.mmproj_required);
        assert!(!meta.cached);
    }
}
