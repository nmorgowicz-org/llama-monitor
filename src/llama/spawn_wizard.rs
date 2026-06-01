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

/// A single actionable tuning suggestion returned with benchmark results.
/// The frontend uses `param` + `value` to patch the running config and respawn.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BenchmarkSuggestion {
    /// Short label shown on the Apply button card, e.g. "Enable flash attention".
    pub label: String,
    /// One-line explanation of why this helps.
    pub description: String,
    /// The config field name that should be changed, e.g. "flash_attn".
    pub param: String,
    /// The value to set, serialized as JSON (string, number, or bool).
    pub value: serde_json::Value,
}

/// Run a short benchmark on the running llama-server.
///
/// Returns a simple report with:
/// - prompt_tokens_per_second
/// - gen_tokens_per_second
/// - time_to_first_token_ms
/// - verdict: "good" / "moderate" / "poor"
/// - hints: short human-readable strings
/// - suggestions: structured actionable tuning steps
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
    pub suggestions: Vec<BenchmarkSuggestion>,
}

pub fn classify_benchmark_result(
    prompt_tps: f64,
    gen_tps: f64,
    ttft_ms: f64,
    model_size_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    total_experts: u64,
) -> BenchmarkResult {
    let mut hints: Vec<String> = Vec::new();
    let mut suggestions: Vec<BenchmarkSuggestion> = Vec::new();

    // Homelab-calibrated thresholds — not datacenter numbers.
    // A mid-range consumer GPU doing 8–15 t/s on a 13B model is working well.
    let verdict = if gen_tps >= 15.0 && ttft_ms <= 1500.0 {
        "good"
    } else if gen_tps >= 4.0 && ttft_ms <= 3000.0 {
        "moderate"
    } else {
        "poor"
    };

    if gen_tps < 4.0 {
        hints.push("Very slow generation — try reducing GPU layers or context size.".to_string());
    }

    // Only suggest flash attention when TTFT is noticeably high (> 1.5 s)
    if ttft_ms > 1500.0 {
        hints.push("Slow first-token response; try enabling flash attention.".to_string());
        suggestions.push(BenchmarkSuggestion {
            label: "Enable flash attention".to_string(),
            description: "Cuts time-to-first-token and reduces VRAM pressure at large context."
                .to_string(),
            param: "flash_attn".to_string(),
            value: serde_json::json!("on"),
        });
    }

    // Only suggest context reduction when gen_tps is very low — don't penalise users
    // who intentionally configured a large context for their use case.
    if gen_tps < 5.0 {
        suggestions.push(BenchmarkSuggestion {
            label: "Try a smaller context window".to_string(),
            description: "If you don't need very long context, reducing to 8 192 tokens can noticeably speed things up.".to_string(),
            param: "context_size".to_string(),
            value: serde_json::json!(8192),
        });
    }

    // Prompt throughput — homelab threshold is lower than server-class hardware
    if prompt_tps < 300.0 {
        hints.push("Slow prompt processing — a larger batch size may help.".to_string());
        suggestions.push(BenchmarkSuggestion {
            label: "Increase batch size to 4 096".to_string(),
            description: "Helps the GPU process incoming prompt tokens more efficiently."
                .to_string(),
            param: "batch_size".to_string(),
            value: serde_json::json!(4096),
        });
    }

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
                    suggestions,
                };
            }
        };

        let moe = suggest_moe_tuning(model_size, vram, total_experts);
        if moe.recommended_n_cpu_moe > 0 {
            hints.push(format!(
                "For MoE models, try increasing n_cpu_moe to {} within available VRAM.",
                moe.recommended_n_cpu_moe
            ));
            suggestions.push(BenchmarkSuggestion {
                label: format!("Offload {} MoE experts to CPU", moe.recommended_n_cpu_moe),
                description: moe.note.clone(),
                param: "n_cpu_moe".to_string(),
                value: serde_json::json!(moe.recommended_n_cpu_moe),
            });
        }
    }

    BenchmarkResult {
        prompt_tokens_per_second: prompt_tps,
        gen_tokens_per_second: gen_tps,
        time_to_first_token_ms: ttft_ms,
        verdict: verdict.to_string(),
        hints,
        suggestions,
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
/// A model discovered from a third-party tool's local storage.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ThirdPartyModel {
    /// Absolute path to the GGUF (or Ollama blob) file.
    pub path: String,
    /// Human-readable name derived from the tool's naming scheme.
    pub name: String,
    /// Which tool owns this model (e.g. "Ollama", "LM Studio", "Jan").
    pub source_tool: String,
    /// File size in bytes.
    pub size: u64,
}

/// Scan all known third-party tool directories and return discovered models.
/// `extra_dirs` are user-configured additional locations; each is labeled with its dir name.
pub fn scan_third_party_models(extra_dirs: &[String]) -> Vec<ThirdPartyModel> {
    let mut out: Vec<ThirdPartyModel> = Vec::new();
    scan_ollama(&mut out);
    scan_lm_studio(&mut out);
    scan_jan(&mut out);
    scan_gpt4all(&mut out);
    scan_hf_cache(&mut out);
    for dir_str in extra_dirs {
        let dir = PathBuf::from(dir_str);
        if dir.is_dir() {
            let label = dir
                .file_name()
                .map(|n| format!("Local — {}", n.to_string_lossy()))
                .unwrap_or_else(|| format!("Local — {dir_str}"));
            scan_gguf_dir(&dir, &label, simple_stem_name, 5, &mut out);
        }
    }
    out
}

// ── Ollama ────────────────────────────────────────────────────────────────────
// Ollama stores GGUFs as content-addressed blobs (sha256-<hash>) without the
// .gguf extension. We parse the JSON manifests to map digest → display name.

fn scan_ollama(out: &mut Vec<ThirdPartyModel>) {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return,
    };
    // Respect OLLAMA_MODELS override
    let models_dir = std::env::var_os("OLLAMA_MODELS")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".ollama").join("models"));

    let manifests_dir = models_dir.join("manifests");
    let blobs_dir = models_dir.join("blobs");
    if !manifests_dir.is_dir() || !blobs_dir.is_dir() {
        return;
    }

    let mut seen_blobs: std::collections::HashSet<String> = std::collections::HashSet::new();
    walk_ollama_manifests(
        &manifests_dir,
        &manifests_dir,
        &blobs_dir,
        &mut seen_blobs,
        out,
    );
}

fn walk_ollama_manifests(
    root: &Path,
    dir: &Path,
    blobs_dir: &Path,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<ThirdPartyModel>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_ollama_manifests(root, &path, blobs_dir, seen, out);
        } else if path.is_file() {
            parse_ollama_manifest(&path, root, blobs_dir, seen, out);
        }
    }
}

fn parse_ollama_manifest(
    manifest_path: &Path,
    root: &Path,
    blobs_dir: &Path,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<ThirdPartyModel>,
) {
    let content = match fs::read_to_string(manifest_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let manifest: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return,
    };
    let layers = match manifest["layers"].as_array() {
        Some(l) => l,
        None => return,
    };
    for layer in layers {
        let media_type = layer["mediaType"].as_str().unwrap_or("");
        if media_type != "application/vnd.ollama.image.model" {
            continue;
        }
        let digest = match layer["digest"].as_str() {
            Some(d) => d,
            None => continue,
        };
        // "sha256:abc123..." → blob file "sha256-abc123..."
        let blob_name = digest.replace(':', "-");
        if !seen.insert(blob_name.clone()) {
            continue;
        }
        let blob_path = blobs_dir.join(&blob_name);
        if !blob_path.is_file() {
            continue;
        }
        let size = blob_path.metadata().map(|m| m.len()).unwrap_or(0);
        let name = ollama_display_name(manifest_path, root);
        out.push(ThirdPartyModel {
            path: blob_path.to_string_lossy().into_owned(),
            name,
            source_tool: "Ollama".to_string(),
            size,
        });
    }
}

/// Derive a human-readable name from the manifest file path relative to the
/// manifests root, e.g.:
///   registry.ollama.ai/library/llama3.2/latest  → llama3.2:latest
///   registry.ollama.ai/bartowski/Llama-3/Q4_K_M → bartowski/Llama-3:Q4_K_M
fn ollama_display_name(manifest_path: &Path, root: &Path) -> String {
    let rel = match manifest_path.strip_prefix(root) {
        Ok(r) => r,
        Err(_) => return manifest_path.to_string_lossy().into_owned(),
    };
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    // parts: [registry, namespace_or_"library", model, tag]
    match parts.len() {
        4.. => {
            let namespace = &parts[1];
            let model = &parts[parts.len() - 2];
            let tag = &parts[parts.len() - 1];
            if namespace == "library" {
                format!("{model}:{tag}")
            } else {
                format!("{namespace}/{model}:{tag}")
            }
        }
        2.. => {
            let model = &parts[parts.len() - 2];
            let tag = &parts[parts.len() - 1];
            format!("{model}:{tag}")
        }
        _ => rel.to_string_lossy().into_owned(),
    }
}

// ── LM Studio ─────────────────────────────────────────────────────────────────
// Current path: ~/.lmstudio/models  (macOS/Linux, post-0.3)
// Alternate:    ~/.cache/lm-studio/models
// Windows:      %USERPROFILE%\.lmstudio\models

fn scan_lm_studio(out: &mut Vec<ThirdPartyModel>) {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return,
    };
    let candidates = vec![
        home.join(".lmstudio").join("models"),
        home.join(".cache").join("lm-studio").join("models"),
        #[cfg(target_os = "macos")]
        home.join("Library")
            .join("Application Support")
            .join("LM Studio")
            .join("models"),
    ];
    for dir in candidates {
        if dir.is_dir() {
            scan_gguf_dir(&dir, "LM Studio", lm_studio_name, 4, out);
        }
    }
}

fn lm_studio_name(path: &Path) -> String {
    // Structure: <models_root>/<publisher>/<repo>/<file.gguf>
    // Use the filename (without .gguf) as the display name; it's already descriptive.
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

// ── Jan ───────────────────────────────────────────────────────────────────────
// macOS: ~/Library/Application Support/Jan/models
// Linux: ~/.jan/models
// Windows: %APPDATA%\Jan\models

fn scan_jan(out: &mut Vec<ThirdPartyModel>) {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return,
    };
    let candidates = vec![
        #[cfg(target_os = "macos")]
        home.join("Library")
            .join("Application Support")
            .join("Jan")
            .join("models"),
        #[cfg(not(target_os = "macos"))]
        home.join(".jan").join("models"),
        #[cfg(target_os = "windows")]
        dirs::data_dir()
            .unwrap_or_default()
            .join("Jan")
            .join("models"),
    ];
    for dir in candidates {
        if dir.is_dir() {
            scan_gguf_dir(&dir, "Jan", simple_stem_name, 3, out);
        }
    }
}

// ── GPT4All ───────────────────────────────────────────────────────────────────
// macOS: ~/Library/Application Support/nomic.ai/GPT4All
// Linux: ~/.local/share/nomic.ai/GPT4All
// Windows: %LOCALAPPDATA%\nomic.ai\GPT4All

fn scan_gpt4all(out: &mut Vec<ThirdPartyModel>) {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return,
    };
    let candidates = vec![
        #[cfg(target_os = "macos")]
        home.join("Library")
            .join("Application Support")
            .join("nomic.ai")
            .join("GPT4All"),
        #[cfg(target_os = "linux")]
        home.join(".local")
            .join("share")
            .join("nomic.ai")
            .join("GPT4All"),
        #[cfg(target_os = "windows")]
        dirs::data_local_dir()
            .unwrap_or_default()
            .join("nomic.ai")
            .join("GPT4All"),
    ];
    for dir in candidates {
        if dir.is_dir() {
            scan_gguf_dir(&dir, "GPT4All", simple_stem_name, 2, out);
        }
    }
}

// ── HuggingFace cache ─────────────────────────────────────────────────────────
// Default: ~/.cache/huggingface/hub
// Override: HF_HUB_CACHE, or HF_HOME/hub
// Structure: models--{org}--{repo}/snapshots/{revision}/*.gguf
// Files are symlinks to blobs; we follow them to get canonical paths and dedupe.

fn scan_hf_cache(out: &mut Vec<ThirdPartyModel>) {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return,
    };
    let hub_dir = std::env::var_os("HF_HUB_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HF_HOME").map(|h| PathBuf::from(h).join("hub")))
        .unwrap_or_else(|| home.join(".cache").join("huggingface").join("hub"));

    if !hub_dir.is_dir() {
        return;
    }

    let entries = match fs::read_dir(&hub_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut seen_canonical: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in entries.flatten() {
        let model_dir = entry.path();
        let dir_name = model_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !dir_name.starts_with("models--") || !model_dir.is_dir() {
            continue;
        }

        // "models--Qwen--Qwen3-8B-GGUF" → "Qwen/Qwen3-8B-GGUF"
        let repo_name = dir_name
            .strip_prefix("models--")
            .map(|s| s.replacen("--", "/", 1))
            .unwrap_or_else(|| dir_name.clone());

        let snapshots_dir = model_dir.join("snapshots");
        if !snapshots_dir.is_dir() {
            continue;
        }

        let rev_entries = match fs::read_dir(&snapshots_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for rev_entry in rev_entries.flatten() {
            let rev_dir = rev_entry.path();
            if !rev_dir.is_dir() {
                continue;
            }
            let gguf_entries = match fs::read_dir(&rev_dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for gguf_entry in gguf_entries.flatten() {
                let gguf_path = gguf_entry.path();
                let fname = gguf_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !fname.to_lowercase().ends_with(".gguf") || fname.starts_with('.') {
                    continue;
                }
                // Follow symlinks to canonical path for deduplication
                let canonical = gguf_path
                    .canonicalize()
                    .unwrap_or_else(|_| gguf_path.clone());
                let canonical_str = canonical.to_string_lossy().into_owned();
                if !seen_canonical.insert(canonical_str.clone()) {
                    continue;
                }
                let size = canonical.metadata().map(|m| m.len()).unwrap_or(0);
                let stem = fname
                    .strip_suffix(".gguf")
                    .or_else(|| fname.strip_suffix(".GGUF"))
                    .unwrap_or(&fname);
                let name = format!("{repo_name}/{stem}");
                out.push(ThirdPartyModel {
                    path: canonical_str,
                    name,
                    source_tool: "HuggingFace".to_string(),
                    size,
                });
            }
        }
    }
}

// ── Generic GGUF scanner ──────────────────────────────────────────────────────

fn scan_gguf_dir(
    dir: &Path,
    tool: &str,
    name_fn: fn(&Path) -> String,
    max_depth: usize,
    out: &mut Vec<ThirdPartyModel>,
) {
    let mut paths: Vec<PathBuf> = Vec::new();
    walk_gguf(dir, 0, max_depth, &mut paths);
    for path in paths {
        let size = path.metadata().map(|m| m.len()).unwrap_or(0);
        let name = name_fn(&path);
        out.push(ThirdPartyModel {
            path: path.to_string_lossy().into_owned(),
            name,
            source_tool: tool.to_string(),
            size,
        });
    }
}

fn simple_stem_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

// ── Legacy helpers (kept for existing tests) ──────────────────────────────────

#[allow(dead_code)]
pub fn get_common_model_dirs() -> Vec<PathBuf> {
    scan_third_party_models(&[])
        .into_iter()
        .filter_map(|m| {
            let p = PathBuf::from(&m.path);
            p.parent().map(|parent| parent.to_path_buf())
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

/// Recursively scan given directories for .gguf files (max depth 3).
#[allow(dead_code)]
pub fn find_gguf_in_dirs(dirs: &[PathBuf], include_subdirs: bool) -> Vec<PathBuf> {
    let mut results = Vec::new();
    for dir in dirs {
        if !include_subdirs {
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
    // ── Core architecture ────────────────────────────────────────────────────
    pub n_layers: Option<u32>,
    pub n_ctx_train: Option<u32>,
    pub n_embd: Option<u32>,
    pub n_ff: Option<u32>,
    /// Total attention heads (query heads; used to derive head_dim = n_embd / n_head).
    pub n_head: Option<u32>,
    /// KV heads (GQA/MQA compressed).  Parsed from `n_head_kv` or `n_kv_heads`.
    pub n_kv_heads: Option<u32>,
    /// Per-head dimension = n_embd / n_head.
    pub head_dim: Option<u32>,
    // ── Architecture family ───────────────────────────────────────────────────
    /// GGUF `general.architecture` value (e.g. "qwen3_6", "gemma4", "llama").
    /// When present, overrides filename-based heuristic selection so that
    /// renamed/finetuned models (e.g. "Pantheon-27B" from Qwen3.6 base) still
    /// get the correct hybrid-DeltaNet / sliding-window treatment.
    pub gguf_arch: Option<String>,
    // ── MoE ─────────────────────────────────────────────────────────────────
    /// Total experts per layer (from `n_experts` / `expert_count` / `n_exp`).
    pub n_experts: Option<u32>,
    /// Active experts per token (from `n_experts_used` / `n_exp_used`).
    pub n_experts_used: Option<u32>,
    // ── Multi-token prediction ───────────────────────────────────────────────
    /// MTP prediction depth (0 = none).
    pub mtp_depth: Option<u32>,
    // ── Multimodal ───────────────────────────────────────────────────────────
    pub mmproj_required: bool,
    // ── Cache marker ─────────────────────────────────────────────────────────
    pub cached: bool,
}

impl ModelMetadata {
    /// Convert to a `ModelArch` suitable for VRAM estimation.
    /// Falls back to heuristics when fields are absent.
    #[allow(dead_code)]
    pub fn to_arch(
        &self,
        model_name: &str,
        param_b: f64,
    ) -> crate::llama::vram_estimator::ModelArch {
        use crate::llama::vram_estimator::ModelArch;

        // Compute head_dim from introspection or fall back to n_embd / n_head.
        let head_dim = self.head_dim.or_else(|| {
            let embd = self.n_embd?;
            let heads = self.n_head?;
            embd.checked_div(heads)
        });

        // Prefer GGUF architecture string over filename when choosing the heuristic.
        // This ensures that renamed finetunes (e.g. "Pantheon-27B" based on Qwen3.6)
        // still get the correct hybrid-DeltaNet / sliding-window heuristic.
        let heuristic_name = self
            .gguf_arch
            .as_deref()
            .map(gguf_arch_to_heuristic_name)
            .unwrap_or(model_name);
        let heuristic = ModelArch::from_name_and_params(heuristic_name, param_b);

        ModelArch {
            n_layers: self.n_layers.unwrap_or(heuristic.n_layers),
            n_kv_heads: self.n_kv_heads.unwrap_or(heuristic.n_kv_heads),
            head_dim: head_dim.unwrap_or(heuristic.head_dim),
            n_global_attn_layers: heuristic.n_global_attn_layers,
            local_attn_window: heuristic.local_attn_window,
            local_kv_heads: heuristic.local_kv_heads,
            // Hybrid linear attention: preserve from heuristic; introspection will refine
            n_attn_layers: heuristic.n_attn_layers,
            linear_attn_state_bytes: heuristic.linear_attn_state_bytes,
            n_experts: self.n_experts.unwrap_or(heuristic.n_experts),
            n_experts_used: self.n_experts_used.unwrap_or(heuristic.n_experts_used),
            expert_fraction: heuristic.expert_fraction,
            mtp_depth: self.mtp_depth.unwrap_or(heuristic.mtp_depth),
            global_head_dim: heuristic.global_head_dim,
            mmproj_bytes: 0, // filled in separately when mmproj path is known
            param_b,
        }
    }
}

/// Introspect a local GGUF model file and return its architecture metadata.
///
/// **Primary path**: reads the GGUF binary header directly — no subprocess,
/// no dependency on a specific llama.cpp version, works even before a binary
/// is downloaded. Works on any GGUF file from any era.
///
/// **Fallback**: if the direct read fails (e.g. corrupt/partial file), falls
/// back to `llama-server --print-model-metadata` text parsing.
///
/// Results are cached in `~/.config/llama-monitor/model-cache/<sha256>.json`.
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

    // ── Primary: parse GGUF binary directly ──────────────────────────────────
    let path = std::path::Path::new(model_path);
    if path.exists() {
        match crate::llama::gguf_meta::read_gguf_metadata(path) {
            Ok(gguf) => {
                let meta = gguf.to_model_metadata();
                let _ = save_model_cache(model_path, &meta);
                return Ok(meta);
            }
            Err(e) => {
                // Log and fall through to subprocess
                eprintln!(
                    "[llama-monitor] GGUF direct read failed for '{model_path}': {e}; falling back to llama-server"
                );
            }
        }
    }

    if llama_server_path.trim().is_empty() {
        return Err(
            "GGUF metadata could not be read directly, and llama_server_path is not configured for fallback introspection.".to_string(),
        );
    }

    // ── Fallback: llama-server --print-model-metadata ─────────────────────────
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
            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => return Err(format!("Failed to run llama-server: {e}")),
            };
            // Poll with a 20-second deadline so a hung binary doesn't block the thread pool.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if std::time::Instant::now() >= deadline => {
                        let _ = child.kill();
                        return Err(
                            "Introspection subprocess timed out after 20 seconds".to_string()
                        );
                    }
                    Ok(None) => std::thread::sleep(std::time::Duration::from_millis(200)),
                    Err(e) => {
                        let _ = child.kill();
                        return Err(format!("Wait error: {e}"));
                    }
                }
            }
            let out = match child.wait_with_output() {
                Ok(o) => o,
                Err(e) => return Err(format!("Failed to wait for llama-server: {e}")),
            };
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            Ok(format!("{stdout}{stderr}"))
        }
    })
    .await
    .map_err(|e| format!("Introspection task failed: {e}"))?
    .map_err(|e| format!("Introspection failed: {e}"))?;

    let meta = parse_model_metadata(&output);
    let _ = save_model_cache(model_path, &meta);
    Ok(meta)
}

/// Map a GGUF `general.architecture` value to a synthetic model name that
/// `ModelArch::from_name_and_params` can pattern-match against.
///
/// This ensures renamed finetunes get the right hybrid/sliding-window heuristic
/// regardless of what the user calls the file.
#[allow(dead_code)]
fn gguf_arch_to_heuristic_name(gguf_arch: &str) -> &str {
    match gguf_arch {
        "qwen3_6" | "qwen3.6" => "qwen3.6-model",
        "qwen3_5" | "qwen3.5" => "qwen3.5-model",
        "qwen3_coder_next" | "qwen3-coder-next" => "qwen3-coder-next",
        "gemma4" | "gemma-4" => "gemma4-model",
        "gemma3" | "gemma-3" => "gemma3-model",
        // For all other architectures, pass through as-is — from_name_and_params
        // will fall through to standard_heuristic which is correct for llama, mistral, etc.
        other => other,
    }
}

fn parse_model_metadata(output: &str) -> ModelMetadata {
    let mut meta = ModelMetadata::default();

    for line in output.lines() {
        let t = line.trim();
        let lower = t.to_ascii_lowercase();

        // ── GGUF architecture family ──────────────────────────────────────────
        // Parses "general.architecture: qwen3_6" or "general.architecture = qwen3_6"
        if meta.gguf_arch.is_none() {
            for prefix in &["general.architecture", "arch"] {
                if lower.starts_with(prefix) {
                    let rest = &t[prefix.len()..].trim_start();
                    let value = rest
                        .strip_prefix(':')
                        .or_else(|| rest.strip_prefix('='))
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty());
                    if let Some(v) = value {
                        meta.gguf_arch = Some(v.to_ascii_lowercase());
                        break;
                    }
                }
            }
        }

        // ── Core architecture ─────────────────────────────────────────────────
        macro_rules! try_field {
            ($field:ident, $($prefix:expr),+) => {
                if meta.$field.is_none() {
                    $(if let Some(v) = extract_int_after($prefix, t) {
                        meta.$field = Some(v);
                    })+
                }
            };
        }

        try_field!(n_layers, "n_layer", "n_layers", "block_count");
        try_field!(n_ctx_train, "n_ctx_train", "context_length");
        try_field!(n_embd, "n_embd", "embedding_length");
        try_field!(n_ff, "n_ff", "feed_forward_length");
        try_field!(n_head, "n_head", "attention.head_count", "head_count");
        try_field!(
            n_kv_heads,
            "n_head_kv",
            "n_kv_heads",
            "attention.head_count_kv",
            "head_count_kv",
            "n_gqa"
        );
        try_field!(head_dim, "head_dim", "key_length", "attention.key_length");
        try_field!(
            n_experts,
            "n_expert",
            "n_experts",
            "expert_count",
            "expert.count",
            "n_exp"
        );
        try_field!(
            n_experts_used,
            "n_expert_used",
            "n_experts_used",
            "expert.used_count",
            "n_exp_used",
            "experts_used"
        );
        try_field!(
            mtp_depth,
            "mtp_depth",
            "multi_token_prediction_depth",
            "num_nextn_predict_layers",
            "next_n_token_count"
        );

        // ── Multimodal detection ──────────────────────────────────────────────
        if lower.contains("mmproj")
            || lower.contains("vision_model")
            || lower.contains("clip.")
            || lower.contains("clip_model")
            || lower.contains("visual.")
            || (lower.contains("multimodal") && !lower.contains("//"))
        {
            meta.mmproj_required = true;
        }
    }

    // Derive head_dim from n_embd / n_head if not directly available
    if meta.head_dim.is_none()
        && let (Some(embd), Some(heads)) = (meta.n_embd, meta.n_head)
    {
        meta.head_dim = embd.checked_div(heads);
    }

    // n_kv_heads fallback: if n_gqa was parsed as n_kv_heads, convert:
    // some models report GQA ratio instead of absolute count.
    // If n_kv_heads looks like a ratio (< 8) and n_head is large, it might be n_head / gqa.
    // Leave as-is; the caller can cross-check.

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
    // Mix in mtime + size so a replaced-in-place file doesn't return stale metadata.
    if let Ok(meta) = std::fs::metadata(model_path) {
        hasher.update(meta.len().to_le_bytes());
        if let Ok(mtime) = meta.modified().and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| std::io::Error::other("mtime"))
        }) {
            hasher.update(mtime.as_secs().to_le_bytes());
        }
    }
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
        assert_eq!(meta.n_experts, Some(8)); // n_exp parses into n_experts
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

    #[test]
    fn test_parse_model_metadata_extracts_gguf_arch() {
        let output = "
            general.architecture: qwen3_6
            n_layers: 64
            n_head_kv: 4
        ";
        let meta = parse_model_metadata(output);
        assert_eq!(meta.gguf_arch.as_deref(), Some("qwen3_6"));
        assert_eq!(meta.n_layers, Some(64));
    }

    #[test]
    fn test_gguf_arch_overrides_filename_for_renamed_finetune() {
        // "Pantheon-Reasoning-27B" has no Qwen3.6 signals in the name,
        // but its GGUF general.architecture = "qwen3_6" reveals the true family.
        // to_arch() should pick the hybrid-DeltaNet heuristic, not standard_heuristic.
        let meta = ModelMetadata {
            gguf_arch: Some("qwen3_6".to_string()),
            n_layers: Some(64),
            n_kv_heads: Some(4),
            ..Default::default()
        };
        let arch = meta.to_arch("Pantheon-Reasoning-27B-Q4_K_M.gguf", 27.0);
        // Must be detected as hybrid DeltaNet (n_attn_layers < n_layers)
        assert!(
            arch.is_hybrid_attn(),
            "gguf_arch=qwen3_6 must trigger hybrid-DeltaNet heuristic even for renamed models"
        );
        assert_eq!(arch.n_layers, 64, "n_layers from introspection");
        assert_eq!(arch.n_kv_heads, 4, "n_kv_heads from introspection");
        assert_eq!(arch.n_attn_layers, 16, "only 16 of 64 layers use KV cache");
    }
}
