use anyhow::Result;
use std::path::{Path, PathBuf};

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

        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let (model_name, quant_type) = parse_gguf_filename(&filename);
        let param_b = model_name.as_deref().and_then(infer_param_b);
        let quant_style = quant_type.as_deref().map(infer_quant_style);
        let vram_est_gb = param_b
            .zip(quant_type.as_deref())
            .map(|(pb, qt)| estimate_vram_gb(pb, qt));

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
        });
    }

    models.sort_by(|a, b| a.filename.cmp(&b.filename));
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

    // Try to find a quant type pattern: Q followed by digits, underscores, and letters
    // Common patterns: Q4_0, Q4_1, Q8_0, Q4_K_M, Q4_K_XL, Q2_K_XL, UD-Q8_K_XL
    // Look for the last occurrence of a quant pattern
    let quant_patterns = [
        "-UD-Q", "-Q", "_Q", // with separator
    ];

    for pattern in &quant_patterns {
        if let Some(pos) = stem.rfind(pattern) {
            let sep_len = pattern.len() - 1; // length of separator before Q
            let quant_start = pos + 1 + sep_len; // skip separator, include Q
            let model_name = &stem[..pos];
            let quant_str = if pattern.starts_with("-UD-") {
                // Include "UD-" prefix in quant type
                &stem[pos + 1..]
            } else {
                &stem[quant_start - 1..] // include the Q
            };

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

/// Rough VRAM estimate in GB: model weights + ~1 GB overhead.
/// Uses bits-per-weight lookup for the given quant type.
fn estimate_vram_gb(param_b: f32, quant_type: &str) -> f32 {
    let bpw: f32 = match quant_type.to_uppercase().as_str() {
        q if q.contains("Q2") => 2.5,
        q if q.contains("Q3") => 3.5,
        q if q.contains("Q4") => 4.5,
        q if q.contains("Q5") => 5.5,
        q if q.contains("Q6") => 6.5,
        q if q.contains("Q8") => 8.5,
        q if q.contains("F16") || q.contains("BF16") => 16.0,
        q if q.contains("F32") => 32.0,
        _ => 4.5, // default Q4 equivalent
    };
    let weights_gb = param_b * 1e9 * bpw / 8.0 / 1_073_741_824.0;
    (weights_gb + 1.0).ceil() // +1 GB overhead, round up
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
    fn test_format_size() {
        assert_eq!(format_size(1_500_000_000), "1.4 GB");
        assert_eq!(format_size(50_000_000), "47.7 MB");
        assert_eq!(format_size(500_000), "488 KB");
    }
}
