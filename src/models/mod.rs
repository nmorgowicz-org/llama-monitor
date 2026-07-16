use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub mod gguf_import;
// Phase 5.5 R2 is a compiled, testable research adapter with no production caller.
// Remove this allowance only when a later gated phase deliberately wires the module.
#[cfg_attr(not(test), allow(dead_code))]
pub mod gguf_recovery;
pub mod library;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveredModel {
    pub path: PathBuf,
    pub filename: String,
    pub size_bytes: u64,
    pub size_display: String,
    pub quant_type: Option<String>,
    pub model_name: Option<String>,
    pub is_split: bool,
    /// Inferred parameter count in billions (e.g. 7.0, 70.0).
    pub param_b: Option<f32>,
    /// Rough VRAM estimate in GB based on param_b + quant type.
    pub vram_est_gb: Option<f32>,
    /// "standard" | "imatrix" | "ud" — for badge coloring.
    pub quant_style: Option<&'static str>,
    /// Unix timestamp (seconds) of last modification.
    pub last_modified: u64,
    /// True if this looks like a vision projector file (mmproj).
    #[serde(default)]
    pub is_mmproj: bool,
    /// True if this looks like an MTP assistant / draft model file.
    #[serde(default)]
    pub is_draft_assistant: bool,
}

/// Scan a directory for .gguf model files.
/// For split models (e.g. -00001-of-00003.gguf), only the first shard is listed.
pub fn scan_models_dir(dir: &Path) -> Result<Vec<DiscoveredModel>> {
    let mut models = Vec::new();

    let entries = std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("failed to read models directory '{}': {e}", dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !filename.ends_with(".gguf") {
            continue;
        }

        // Skip split shards beyond the first
        let is_split = is_split_shard(&filename);
        if is_split && !is_first_shard(&filename) {
            continue;
        }

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue, // Skip files we can't read metadata for
        };
        let size_bytes = meta.len();
        let last_modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs()))
            .unwrap_or(0);
        let (model_name, quant_type) = parse_gguf_filename(&filename);
        let param_b = model_name.as_deref().and_then(infer_param_b);
        let quant_style = quant_type.as_deref().map(infer_quant_style);
        let mm_lower = filename.to_ascii_lowercase();
        let is_draft = is_draft_assistant_filename(&mm_lower, size_bytes);

        let vram_est_gb = {
            if size_bytes == 0 {
                None
            } else {
                Some((size_bytes as f32 * 1.5 / 1_073_741_824.0).ceil())
            }
        };

        let is_mmproj = mm_lower.contains("mmproj") || mm_lower.contains("projector");

        models.push(DiscoveredModel {
            path: path.clone(),
            filename,
            size_bytes,
            size_display: format_size(size_bytes),
            quant_type,
            model_name,
            is_split,
            param_b,
            vram_est_gb,
            quant_style,
            last_modified,
            is_mmproj,
            is_draft_assistant: is_draft,
        });
    }

    models.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(models)
}

/// Discover launchable GGUF files across both the legacy flat model directory
/// and the structured `gguf/` directory. This keeps startup, refresh, preset
/// classification, and the typed inventory in agreement during and after a
/// library migration.
pub fn scan_gguf_library(dir: &Path) -> Result<Vec<DiscoveredModel>> {
    let mut models = scan_models_dir(dir)?;
    let structured = dir.join("gguf");
    if structured.is_dir() {
        models.extend(scan_models_dir(&structured)?);
    }
    models.sort_by(|a, b| a.path.cmp(&b.path));
    models.dedup_by(|a, b| a.path == b.path);
    Ok(models)
}

/// Parse a GGUF filename to extract model name and quantization type.
/// Examples:
///   "Qwen3.5-27B-Q4_0.gguf" -> ("Qwen3.5-27B", "Q4_0")
///   "Qwen3-Coder-Next-Q4_1-00001-of-00003.gguf" -> ("Qwen3-Coder-Next", "Q4_1")
///   "model.gguf" -> ("model", None)
pub fn parse_gguf_filename(filename: &str) -> (Option<String>, Option<String>) {
    // Strip .gguf extension
    let stem = filename.strip_suffix(".gguf").unwrap_or(filename);

    // Strip split suffix like -00001-of-00003
    let stem = strip_split_suffix(stem);

    // Normalize spaces before quant-like tokens so "- Q4_K_M" -> "-Q4_K_M"
    let stem_norm = stem
        .replace("- ", "-")
        .replace(" -", "-")
        .trim()
        .to_string();
    let stem = stem_norm.as_str();

    // Try to find a quant type pattern: Q followed by digits, underscores, and letters
    // Common patterns: Q4_0, Q4_1, Q8_0, Q4_K_M, Q4_K_XL, Q2_K_XL, UD-Q8_K_XL
    // IQ* patterns: IQ2_XXS, IQ3_M, IQ1_XXS, etc. (llama.cpp importance-matrix quants)
    // Float/full-precision: F16, BF16, F32 (possibly with suffixes like -MTP)
    // Look for the last occurrence of a quant pattern

    // 1) UD-Q prefix (highest priority)
    if let Some(pos) = stem.rfind("-UD-Q") {
        let model_name = &stem[..pos];
        let quant_str = &stem[pos + 1..]; // includes "UD-Q..."
        if !model_name.is_empty() && quant_str.len() >= 3 {
            return (Some(model_name.to_string()), Some(quant_str.to_string()));
        }
    }

    // 2) IQ* patterns
    for pattern in &["-IQ", "_IQ"] {
        if let Some(pos) = stem.rfind(pattern) {
            let model_name = &stem[..pos];
            let quant_str = &stem[pos + 1..]; // includes "IQ..."
            if !model_name.is_empty() && quant_str.len() >= 3 {
                return (Some(model_name.to_string()), Some(quant_str.to_string()));
            }
        }
    }

    // 3) Q* patterns with separator (-Q or _Q)
    for pattern in &["-Q", "_Q"] {
        if let Some(pos) = stem.rfind(pattern) {
            let model_name = &stem[..pos];
            let quant_str = &stem[pos + 1..]; // includes "Q..."
            if !model_name.is_empty() && quant_str.len() >= 3 {
                return (Some(model_name.to_string()), Some(quant_str.to_string()));
            }
        }
    }

    // 4) F16 / BF16 / F32 (with separator)
    for pattern in &["-F16", "_F16", "-BF16", "_BF16", "-F32", "_F32"] {
        if let Some(pos) = stem.rfind(pattern) {
            let model_name = &stem[..pos];
            let quant_str = &stem[pos + 1..]; // includes e.g. "F16-MTP"
            if !model_name.is_empty() && quant_str.len() >= 3 {
                return (Some(model_name.to_string()), Some(quant_str.to_string()));
            }
        }
    }

    // No quant type found
    if !stem.is_empty() {
        (Some(stem.to_string()), None)
    } else {
        (None, None)
    }
}

fn is_split_shard(filename: &str) -> bool {
    // Pattern: -NNNNN-of-NNNNN.gguf
    let stem = filename.strip_suffix(".gguf").unwrap_or(filename);
    if let Some(of_pos) = stem.rfind("-of-") {
        let after_of = &stem[of_pos + 4..];
        let before_of = &stem[..of_pos];
        if after_of.chars().all(|c| c.is_ascii_digit())
            && after_of.len() == 5
            && let Some(dash_pos) = before_of.rfind('-')
        {
            let shard_num = &before_of[dash_pos + 1..];
            return shard_num.chars().all(|c| c.is_ascii_digit()) && shard_num.len() == 5;
        }
    }
    false
}

fn is_first_shard(filename: &str) -> bool {
    filename.contains("-00001-of-")
}

fn strip_split_suffix(stem: &str) -> &str {
    // Remove -NNNNN-of-NNNNN from the end
    if let Some(of_pos) = stem.rfind("-of-") {
        let before_of = &stem[..of_pos];
        if let Some(dash_pos) = before_of.rfind('-') {
            let shard_num = &before_of[dash_pos + 1..];
            if shard_num.chars().all(|c| c.is_ascii_digit()) && shard_num.len() == 5 {
                return &stem[..dash_pos];
            }
        }
    }
    stem
}

/// Extract parameter count in billions from a model name string.
/// Matches patterns like "70B", "7B", "72b", "13.5B", "30B-A3B" (active params ignored).
fn infer_param_b(model_name: &str) -> Option<f32> {
    // Walk left-to-right finding the first "<number>B" token (case-insensitive).
    // Stop at the first match — avoids "A3B" (active params) in MoE names.
    let upper = model_name.to_uppercase();
    let bytes = upper.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'B' && i > 0 {
            // Collect digits (and '.') immediately before the 'B'
            let mut j = i;
            while j > 0 && (bytes[j - 1].is_ascii_digit() || bytes[j - 1] == b'.') {
                j -= 1;
            }
            if j < i
                && let Ok(v) = std::str::from_utf8(&bytes[j..i])
                    .unwrap_or("")
                    .parse::<f32>()
                && (0.5..=2000.0).contains(&v)
            {
                return Some(v);
            }
        }
        i += 1;
    }
    None
}

/// Map a quant type string to its style: "standard" | "imatrix" | "ud".
fn infer_quant_style(quant_type: &str) -> &'static str {
    let u = quant_type.to_uppercase();
    if u.starts_with("UD-") || u.contains("UD_") {
        "ud"
    } else if u.starts_with("I1-") || u.starts_with("IQ") || u.contains("-I1-") {
        "imatrix"
    } else {
        "standard"
    }
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{} KB", bytes / 1024)
    }
}

/// Conservative heuristic to detect MTP assistant / draft model files.
fn is_draft_assistant_filename(name: &str, size_bytes: u64) -> bool {
    const SMALL_MB: u64 = 3_000_000_000; // ≤ 3 GB
    const MEDIUM_GB: u64 = 5_000_000_000; // ≤ 5 GB

    // Unambiguous MTP keywords: larger threshold allowed.
    let is_unambiguous =
        name.contains("mtp-draft") || name.contains("mtp_small") || name.contains("mtp-heads");
    if is_unambiguous && size_bytes <= MEDIUM_GB {
        return true;
    }

    // "mtp-" prefix: only if modest size (avoid tagging large models)
    if name.starts_with("mtp-") && size_bytes <= MEDIUM_GB {
        return true;
    }

    // Unsloth "-mtp.gguf" suffix: small threshold only
    if name.ends_with("-mtp.gguf") && size_bytes <= SMALL_MB {
        return true;
    }

    // Broad keywords require confirmed, non-zero size and small file.
    let is_broad = name.contains("mtp")
        || name.contains("draft")
        || name.contains("assistant")
        || name.contains("draft-model");
    is_broad && size_bytes > 0 && size_bytes <= SMALL_MB
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelClassification {
    /// Family slug: qwen35, qwen36, qwen3, llama3, gemma4, gemma, mistral, exaone, heretic, other.
    pub family: String,
    /// Size class: tiny (≤3B), small (4–7B), medium (8–17B), large (22–40B), huge (≥70B).
    pub size_class: String,
    /// Primary use hints: general, coding, roleplay, agentic, vision.
    pub primary_use: Vec<String>,
    /// MoE (mixture-of-experts) detected.
    pub is_moe: bool,
    /// Multi-token prediction / speculative heads detected.
    pub has_mtp: bool,
}

/// Classify a discovered model into family/size/use categories.
/// Intentionally heuristic, based on filename and existing metadata.
pub fn classify_model(m: &DiscoveredModel) -> ModelClassification {
    let name = m.model_name.as_deref().unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    let param_b = m.param_b.unwrap_or(0.0);

    let family = detect_family(&lower);
    let size_class = detect_size_class(param_b);
    let is_moe = lower.contains("moe")
        || lower.contains("a3b")
        || lower.contains("a10b")
        || lower.contains("mixtral")
        || lower.contains("deepseek")
        || lower.contains("qwen3.5")
        || lower.contains("qwen3.6")
        || lower.contains("qwen35")
        || lower.contains("qwen36");
    // Only mark has_mtp for small-ish models or when is_draft_assistant is set;
    // large models with "mtp" in the name (e.g. Ornstein3.6-27B-MTP) are not MTP assistants.
    let has_mtp = m.is_draft_assistant
        || ((lower.contains("mtp") || lower.contains("draft")) && m.size_bytes <= 5_000_000_000);
    let primary_use = detect_primary_use(&lower, is_moe);

    ModelClassification {
        family,
        size_class,
        primary_use,
        is_moe,
        has_mtp,
    }
}

fn detect_family(name: &str) -> String {
    // Order: most specific first.
    if name.contains("qwen3.6") || name.contains("qwen36") || name.contains("qwopus") {
        return "qwen36".into();
    }
    if name.contains("qwen3.5") || name.contains("qwen35") {
        return "qwen35".into();
    }
    if name.contains("qwen3") {
        return "qwen3".into();
    }
    if name.contains("exaone-4.5") || name.contains("exaone4.5") {
        return "exaone".into();
    }
    if (name.contains("gemma-4") || name.contains("gemma4"))
        && (name.contains("2b")
            || name.contains("4b")
            || name.contains("12b")
            || name.contains("15b")
            || name.contains("26b")
            || name.contains("31b"))
    {
        return "gemma4".into();
    }
    if name.contains("gemma") {
        return "gemma".into();
    }
    if name.contains("heretic") {
        return "heretic".into();
    }
    if name.contains("llama-3") || name.contains("llama3") {
        return "llama3".into();
    }
    if name.contains("mistral") || name.contains("mixtral") {
        return "mistral".into();
    }
    "other".into()
}

fn detect_size_class(param_b: f32) -> String {
    if param_b <= 0.0 {
        return "unknown".into();
    }
    if param_b <= 3.0 {
        "tiny"
    } else if param_b <= 7.0 {
        "small"
    } else if param_b <= 17.0 {
        "medium"
    } else if param_b <= 40.0 {
        "large"
    } else {
        "huge"
    }
    .into()
}

/// Derive family from GGUF architecture field (not from filename).
/// Returns None if unknown — callers must not guess.
pub fn infer_family_from_architecture(arch: &str) -> Option<String> {
    let a = arch.to_ascii_lowercase();

    // Llama family
    if a.starts_with("llama") {
        return Some("llama3".into());
    }

    // Qwen family (dense/MoE/VL variants grouped by generation)
    if a == "qwen3_6" || a == "qwen36" || a == "qwopus" {
        return Some("qwen36".into());
    }
    // "qwen35" and "qwen35moe" cover both Qwen3.5 and Qwen3.6 — without block_count we
    // default to qwen35. Callers with access to block_count (ensure_gguf_metadata) refine
    // this: bc ≥ 75 → qwen35, bc < 75 → qwen36.
    if a == "qwen3_5" || a == "qwen35" || a == "qwen35moe" || a == "qwen3_5moe" {
        return Some("qwen35".into());
    }
    if a.starts_with("qwen3") {
        return Some("qwen3".into());
    }
    if a.starts_with("qwen2") {
        return Some("qwen2".into());
    }
    if a == "qwen" {
        return Some("qwen".into());
    }

    // Gemma family (all generations)
    if a.starts_with("gemma") {
        // gemma4 / gemma_4 → distinct; others → gemma
        if a.starts_with("gemma4") || a == "gemma_4" {
            return Some("gemma4".into());
        }
        return Some("gemma".into());
    }

    // Mistral family (Mistral + Mixtral)
    if a.starts_with("mistral") || a.starts_with("mixtral") {
        return Some("mistral".into());
    }

    // DeepSeek family
    if a.starts_with("deepseek") {
        return Some("deepseek".into());
    }

    // Phi family (phi2/phi3/phimoe)
    if a.starts_with("phi") {
        return Some("phi".into());
    }

    // Falcon family
    if a.starts_with("falcon") {
        return Some("falcon".into());
    }

    // EXAONE family
    if a.starts_with("exaone") {
        return Some("exaone".into());
    }

    // Grok (xAI)
    if a.starts_with("grok") {
        return Some("grok".into());
    }

    // Mamba / SSM family
    if a.starts_with("mamba") || a == "jamba" {
        return Some("mamba".into());
    }

    // RWKV family
    if a.starts_with("rwkv") {
        return Some("rwkv".into());
    }

    // OLMo family
    if a.starts_with("olmo") {
        return Some("olmo".into());
    }

    // StableLM
    if a.starts_with("stablelm") {
        return Some("stablelm".into());
    }

    // Granite (IBM)
    if a.starts_with("granite") {
        return Some("granite".into());
    }

    // StarCoder family
    if a.starts_with("starcoder") {
        return Some("starcoder".into());
    }

    None
}

/// Derive size_class from an exact parameter count (u64).
pub fn infer_size_class_from_param_count(param_count: u64) -> Option<String> {
    if param_count == 0 {
        return None;
    }
    let b = param_count as f32 / 1_000_000_000.0;
    Some(detect_size_class(b))
}

fn detect_primary_use(name: &str, is_moe: bool) -> Vec<String> {
    let mut uses = Vec::new();

    // Vision / multimodal
    if name.contains("vl") || name.contains("vision") {
        uses.push("vision".into());
    }

    // Coding / agentic / roleplay / general heuristics
    let has_coding = name.contains("code") || name.contains("coder");
    let has_roleplay =
        name.contains("roleplay") || name.contains("chat") || name.contains("instruct");
    let has_agentic = name.contains("agentic") || name.contains("agent") || is_moe;

    if has_coding {
        uses.push("coding".into());
    }
    if has_agentic {
        uses.push("agentic".into());
    }
    if has_roleplay && !uses.contains(&"coding".to_string()) {
        uses.push("roleplay".into());
    }
    if uses.is_empty() {
        uses.push("general".into());
    }

    uses
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_filename() {
        let (name, quant) = parse_gguf_filename("Qwen3.5-27B-Q4_0.gguf");
        assert_eq!(name.as_deref(), Some("Qwen3.5-27B"));
        assert_eq!(quant.as_deref(), Some("Q4_0"));
    }

    #[test]
    fn test_parse_split_filename() {
        let (name, quant) = parse_gguf_filename("Qwen3-Coder-Next-Q4_1-00001-of-00003.gguf");
        assert_eq!(name.as_deref(), Some("Qwen3-Coder-Next"));
        assert_eq!(quant.as_deref(), Some("Q4_1"));
    }

    #[test]
    fn test_parse_k_quant() {
        let (name, quant) = parse_gguf_filename("Devstral-Small-2-24B-Q4_K_M.gguf");
        assert_eq!(name.as_deref(), Some("Devstral-Small-2-24B"));
        assert_eq!(quant.as_deref(), Some("Q4_K_M"));
    }

    #[test]
    fn test_parse_ud_quant() {
        let (name, quant) = parse_gguf_filename("Qwen3.5-122B-A10B-UD-Q2_K_XL.gguf");
        assert_eq!(name.as_deref(), Some("Qwen3.5-122B-A10B"));
        assert_eq!(quant.as_deref(), Some("UD-Q2_K_XL"));
    }

    #[test]
    fn test_parse_iq_quant_variants() {
        let (name, quant) = parse_gguf_filename("Qwen3-32B-A3B-IQ2_XXS.gguf");
        assert_eq!(name.as_deref(), Some("Qwen3-32B-A3B"));
        assert_eq!(quant.as_deref(), Some("IQ2_XXS"));

        let (name2, quant2) = parse_gguf_filename("Llama-3.1-70B-IQ3_M.gguf");
        assert_eq!(name2.as_deref(), Some("Llama-3.1-70B"));
        assert_eq!(quant2.as_deref(), Some("IQ3_M"));

        let (name3, quant3) = parse_gguf_filename("Gemma-2-27B-IQ4_XL.gguf");
        assert_eq!(name3.as_deref(), Some("Gemma-2-27B"));
        assert_eq!(quant3.as_deref(), Some("IQ4_XL"));
    }

    #[test]
    fn test_parse_no_quant() {
        let (name, quant) = parse_gguf_filename("model.gguf");
        assert_eq!(name.as_deref(), Some("model"));
        assert_eq!(quant, None);
    }

    #[test]
    fn test_is_split_shard() {
        assert!(is_split_shard("model-Q4_1-00001-of-00003.gguf"));
        assert!(is_split_shard("model-Q4_1-00002-of-00003.gguf"));
        assert!(!is_split_shard("model-Q4_1.gguf"));
    }

    #[test]
    fn test_is_first_shard() {
        assert!(is_first_shard("model-Q4_1-00001-of-00003.gguf"));
        assert!(!is_first_shard("model-Q4_1-00002-of-00003.gguf"));
    }

    #[test]
    fn test_scan_models_dir() {
        let dir =
            std::env::temp_dir().join(format!("llama-monitor-model-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // Create test files
        std::fs::write(dir.join("TestModel-Q4_0.gguf"), "fake model data").unwrap();
        std::fs::write(dir.join("Split-Q8_0-00001-of-00002.gguf"), "shard1").unwrap();
        std::fs::write(dir.join("Split-Q8_0-00002-of-00002.gguf"), "shard2").unwrap();
        std::fs::write(dir.join("readme.txt"), "not a model").unwrap();

        let models = scan_models_dir(&dir).unwrap();

        // Should find TestModel and first shard of Split, but not shard2 or readme
        assert_eq!(models.len(), 2);

        let names: Vec<&str> = models.iter().map(|m| m.filename.as_str()).collect();
        assert!(names.contains(&"TestModel-Q4_0.gguf"));
        assert!(names.contains(&"Split-Q8_0-00001-of-00002.gguf"));

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_scanner_includes_structured_gguf_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("gguf")).unwrap();
        std::fs::write(dir.path().join("gguf/Structured-Q4_0.gguf"), b"model").unwrap();
        let models = scan_gguf_library(dir.path()).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].filename, "Structured-Q4_0.gguf");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(1_500_000_000), "1.4 GB");
        assert_eq!(format_size(50_000_000), "47.7 MB");
        assert_eq!(format_size(500_000), "488 KB");
    }

    fn make_model(name: &str, param_b: f32) -> DiscoveredModel {
        use std::path::PathBuf;
        DiscoveredModel {
            path: PathBuf::from("/tmp/test.gguf"),
            filename: name.to_string(),
            size_bytes: 1_000_000_000,
            size_display: "976.6 MB".into(),
            quant_type: Some("Q4_0".into()),
            model_name: Some(name.into()),
            is_split: false,
            param_b: Some(param_b),
            vram_est_gb: Some(5.0),
            quant_style: Some("standard"),
            last_modified: 0,
            is_mmproj: false,
            is_draft_assistant: false,
        }
    }

    #[test]
    fn test_classify_qwen35() {
        let c = classify_model(&make_model("Qwen3.5-27B-Q4_0", 27.0));
        assert_eq!(c.family, "qwen35");
        assert_eq!(c.size_class, "large");
        assert!(c.is_moe);
    }

    #[test]
    fn test_classify_qwen36() {
        let c = classify_model(&make_model("Qwen3.6-27B-A3B-Q4_0", 27.0));
        assert_eq!(c.family, "qwen36");
        assert_eq!(c.size_class, "large");
        assert!(c.is_moe);
    }

    #[test]
    fn test_classify_llama3() {
        let c = classify_model(&make_model("Llama-3.2-3B-Instruct-Q4_0", 3.0));
        assert_eq!(c.family, "llama3");
        assert_eq!(c.size_class, "tiny");
        assert!(!c.is_moe);
    }

    #[test]
    fn test_classify_size_tiny() {
        let c = classify_model(&make_model("TinyModel-Q4_0", 1.5));
        assert_eq!(c.size_class, "tiny");
    }

    #[test]
    fn test_classify_size_small() {
        let c = classify_model(&make_model("SmallModel-Q4_0", 7.0));
        assert_eq!(c.size_class, "small");
    }

    #[test]
    fn test_classify_size_medium() {
        let c = classify_model(&make_model("MediumModel-Q4_0", 12.0));
        assert_eq!(c.size_class, "medium");
    }

    #[test]
    fn test_classify_size_huge() {
        let c = classify_model(&make_model("HugeModel-Q4_0", 70.0));
        assert_eq!(c.size_class, "huge");
    }

    #[test]
    fn test_classify_primary_use_coding() {
        let c = classify_model(&make_model("CodeModel-7B-Coder-Q4_0", 7.0));
        assert!(c.primary_use.contains(&"coding".to_string()));
    }

    #[test]
    fn test_classify_primary_use_vision() {
        let c = classify_model(&make_model("VisionModel-VL-7B-Q4_0", 7.0));
        assert!(c.primary_use.contains(&"vision".to_string()));
    }

    #[test]
    fn test_parse_f16_quant() {
        let (name, quant) = parse_gguf_filename("gemma-4-26B-A4B-it-F16-MTP.gguf");
        assert_eq!(name.as_deref(), Some("gemma-4-26B-A4B-it"));
        assert_eq!(quant.as_deref(), Some("F16-MTP"));
    }

    #[test]
    fn test_parse_bf16_quant() {
        let (name, quant) = parse_gguf_filename("model-BF16.gguf");
        assert_eq!(name.as_deref(), Some("model"));
        assert_eq!(quant.as_deref(), Some("BF16"));
    }

    #[test]
    fn test_parse_f32_quant() {
        let (name, quant) = parse_gguf_filename("model-F32.gguf");
        assert_eq!(name.as_deref(), Some("model"));
        assert_eq!(quant.as_deref(), Some("F32"));
    }

    #[test]
    fn test_parse_underscore_f16_quant() {
        let (name, quant) = parse_gguf_filename("model_F16.gguf");
        assert_eq!(name.as_deref(), Some("model"));
        assert_eq!(quant.as_deref(), Some("F16"));
    }

    #[test]
    fn test_parse_space_before_quant() {
        // Ornstein-style: "MTP- Q4_K_M" with space before quant
        let (name, quant) =
            parse_gguf_filename("Ornstein3.6-27B-MTP-NSC-ACE-SABER-MTP- Q4_K_M.gguf");
        assert_eq!(
            name.as_deref(),
            Some("Ornstein3.6-27B-MTP-NSC-ACE-SABER-MTP")
        );
        assert_eq!(quant.as_deref(), Some("Q4_K_M"));
    }

    #[test]
    fn test_draft_assistant_large_mtp_model() {
        // 15.7 GB with MTP in name should NOT be labeled as MTP/draft
        assert!(!is_draft_assistant_filename(
            "ornstein3.6-27b-mtp-nsc-ace-saber-mtp-q4_k_m.gguf",
            15_700_000_000
        ));
    }

    #[test]
    fn test_draft_assistant_small_mtp_model() {
        // 815 MB with MTP should be labeled MTP
        assert!(is_draft_assistant_filename(
            "mtp-draft-3b-q4_k_m.gguf",
            815_000_000
        ));
    }

    #[test]
    fn test_draft_assistant_unsloth_mtp() {
        // Unsloth convention: "-mtp.gguf" suffix, small model
        assert!(is_draft_assistant_filename(
            "qwen3-8b-instruct-mtp.gguf",
            2_500_000_000
        ));
    }

    #[test]
    fn test_classify_large_model_no_mtp() {
        // Large model with MTP in name should not get has_mtp
        let m = DiscoveredModel {
            path: std::path::PathBuf::from("/tmp/test.gguf"),
            filename: "Ornstein3.6-27B-MTP-Q4_K_M.gguf".into(),
            size_bytes: 15_700_000_000,
            size_display: "14.6 GB".into(),
            quant_type: Some("Q4_K_M".into()),
            model_name: Some("Ornstein3.6-27B-MTP".into()),
            is_split: false,
            param_b: Some(27.0),
            vram_est_gb: Some(16.0),
            quant_style: Some("standard"),
            last_modified: 0,
            is_mmproj: false,
            is_draft_assistant: false,
        };
        let c = classify_model(&m);
        assert!(
            !c.has_mtp,
            "large MTP-named model should not be classified as MTP"
        );
    }
}
