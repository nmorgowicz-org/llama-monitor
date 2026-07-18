use anyhow::{Context, Result, anyhow};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

const INFO_TIMEOUT: Duration = Duration::from_secs(10);
#[allow(dead_code)]
const MODELS_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_OUTPUT_BYTES: usize = 256 * 1024;

/// Minimum minor version at which the `rapid-mlx info <model>` output layout is trusted.
/// Below this, parsing may produce incorrect fields, so we return a minimal profile
/// with no recommendation hints (fields fall back to defaults/unknowns).
pub(crate) const MIN_TRUSTED_MINOR: u64 = 10;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ModelProfile {
    #[serde(default)]
    pub tool_format: Option<String>,
    #[serde(default)]
    pub reasoning_parser: Option<String>,
    #[serde(default)]
    pub architecture: Option<String>,
    #[serde(default)]
    pub spec_decode: SpecDecodeSupport,
    #[serde(default)]
    pub mtp_path: Option<MtpPathStatus>,
    #[serde(default)]
    pub kv_share: Option<bool>,
    #[serde(default)]
    pub throttle: Option<bool>,
    #[serde(default)]
    pub suffix_tier: Option<String>,
    #[serde(default)]
    pub dflash_eligibility: Eligibility,
    #[serde(default)]
    pub ddtree_eligibility: Eligibility,
    #[serde(default)]
    pub extras: ExtraCapabilities,
    #[serde(default)]
    pub is_finetune: bool,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpecDecodeSupport {
    Supported,
    Unsupported,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MtpPathStatus {
    Enabled,
    Disabled,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct Eligibility {
    pub supported: Option<bool>,
    #[serde(default)]
    pub reasons: BTreeMap<String, Option<String>>,
}

#[allow(dead_code)]
impl Eligibility {
    pub fn is_eligible(&self) -> bool {
        self.supported == Some(true)
    }

    pub fn is_ineligible(&self) -> bool {
        self.supported == Some(false)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ExtraCapabilities {
    #[serde(default)]
    pub vision: bool,
    #[serde(default)]
    pub has_vision_tower: bool,
    #[serde(default)]
    pub embeddings: bool,
    #[serde(default)]
    pub mtp_dflash: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[allow(dead_code)]
pub struct ModelListEntry {
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone)]
struct QueryOutput {
    stdout: String,
}

#[allow(clippy::type_complexity)]
static VERSION_CACHE: OnceLock<Arc<std::sync::RwLock<Option<(String, u64)>>>> = OnceLock::new();

/// Query (and cache) the `rapid-mlx --version` output as `(exact_string, minor)`.
/// Shared by callers that need to version-guard text-scraping of other
/// subcommands (e.g. `info`, `bench`) before trusting their output layout.
pub(crate) async fn cached_version(binary: &Path) -> Result<Option<(String, u64)>> {
    let cache = VERSION_CACHE
        .get_or_init(|| Arc::new(std::sync::RwLock::new(None)))
        .clone();
    if let Some(version) = cache.read().unwrap().as_ref().cloned() {
        return Ok(Some(version));
    }
    let output = run_query(binary, &["--version"], INFO_TIMEOUT, MAX_OUTPUT_BYTES).await?;
    let text = output.stdout.trim();
    let parsed = parse_version_number(text);
    if let Some((exact, minor)) = parsed {
        *cache.write().unwrap() = Some((exact.clone(), minor));
        Ok(Some((exact, minor)))
    } else {
        Ok(None)
    }
}

fn parse_version_number(text: &str) -> Option<(String, u64)> {
    for start in 0..text.len() {
        if !text.as_bytes()[start].is_ascii_digit() {
            continue;
        }
        let mut cursor = start;
        let bytes = text.as_bytes();
        if let Some(_major) = parse_num(bytes, &mut cursor)
            && bytes.get(cursor) == Some(&b'.')
        {
            cursor += 1;
            if let Some(minor) = parse_num(bytes, &mut cursor)
                && bytes.get(cursor) == Some(&b'.')
            {
                cursor += 1;
                if let Some(_patch) = parse_num(bytes, &mut cursor) {
                    let suffix_end = bytes[cursor..]
                        .iter()
                        .position(|b| b.is_ascii_whitespace())
                        .map_or(bytes.len(), |off| cursor + off);
                    let exact = text[start..suffix_end].to_string();
                    return Some((exact, minor));
                }
            }
        }
    }
    None
}

fn parse_num(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
    let start = *cursor;
    while bytes.get(*cursor).is_some_and(u8::is_ascii_digit) {
        *cursor += 1;
    }
    (start != *cursor)
        .then(|| {
            std::str::from_utf8(&bytes[start..*cursor])
                .ok()?
                .parse()
                .ok()
        })
        .flatten()
}

pub async fn fetch_model_profile(binary: &Path, model_id: &str) -> Result<Option<ModelProfile>> {
    if model_id.is_empty()
        || model_id.contains("..")
        || model_id.starts_with('/')
        || model_id.starts_with('\\')
    {
        return Err(anyhow!("Invalid model identifier"));
    }

    let version_trusted = match cached_version(binary).await {
        Ok(Some((_, minor))) => minor >= MIN_TRUSTED_MINOR,
        Ok(None) => false,
        Err(_) => false,
    };

    let output = run_query(binary, &["info", model_id], INFO_TIMEOUT, MAX_OUTPUT_BYTES).await?;
    if !output.stdout.is_empty() && output.stdout.contains("Error:") {
        return Ok(None);
    }
    if output.stdout.trim().is_empty() {
        return Ok(None);
    }

    parse_model_profile(&output.stdout, version_trusted, model_id)
}

#[allow(dead_code)]
pub async fn fetch_model_list(binary: &Path) -> Result<Vec<ModelListEntry>> {
    let output = run_query(binary, &["models"], MODELS_TIMEOUT, MAX_OUTPUT_BYTES).await?;
    parse_model_list(&output.stdout)
}

async fn run_query(
    binary: &Path,
    args: &[&str],
    timeout: Duration,
    max_bytes: usize,
) -> Result<QueryOutput> {
    let mut cmd = Command::new(binary);
    cmd.args(args)
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .with_context(|| format!("Failed to execute rapid-mlx query: {}", binary.display()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Failed to capture rapid-mlx query stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("Failed to capture rapid-mlx query stderr"))?;

    let capture = async move {
        let (stdout_bytes, stderr_bytes, status) = tokio::try_join!(
            read_bounded(stdout, max_bytes),
            read_bounded(stderr, max_bytes),
            async { child.wait().await.map_err(Into::<anyhow::Error>::into) }
        )?;
        if !status.success() {
            let stderr_text = String::from_utf8_lossy(&stderr_bytes).trim().to_string();
            if stderr_text.contains("Error: model not found")
                || stderr_text.contains("unknown model")
                || stderr_text.contains("unrecognized model")
            {
                return Ok(QueryOutput {
                    stdout: String::new(),
                });
            }
            return Err(anyhow!(
                "rapid-mlx query failed: {}",
                stderr_text.chars().take(500).collect::<String>()
            ));
        }
        Ok(QueryOutput {
            stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
        })
    };

    tokio::time::timeout(timeout, capture)
        .await
        .with_context(|| "rapid-mlx query timed out")?
}

async fn read_bounded<R>(reader: R, max_bytes: usize) -> Result<Vec<u8>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut bytes = Vec::with_capacity(max_bytes.min(8192));
    reader
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > max_bytes {
        anyhow::bail!("rapid-mlx query output exceeded {} byte limit", max_bytes);
    }
    Ok(bytes)
}

fn parse_model_profile(
    output: &str,
    version_trusted: bool,
    model_id: &str,
) -> Result<Option<ModelProfile>> {
    let mut profile = ModelProfile::default();
    let lines: Vec<&str> = output.lines().collect();

    let mut current_section = "";
    let mut eligibility: Option<Eligibility> = None;
    let mut detected_hf_repo: Option<String> = None;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.contains("## DFlash eligibility")
            || trimmed.contains("## DFlash Eligibility")
            || trimmed.contains("DFlash eligibility")
        {
            eligibility = Some(Eligibility::default());
            current_section = "dflash";
            if trimmed.contains("✓")
                && (trimmed.contains("eligible") || trimmed.contains("supported"))
            {
                profile.dflash_eligibility.supported = Some(true);
            } else if trimmed.contains("✗")
                || trimmed.contains("ineligible")
                || trimmed.contains("not supported")
            {
                profile.dflash_eligibility.supported = Some(false);
            }
            continue;
        }
        if trimmed.contains("## DDTree eligibility")
            || trimmed.contains("## DDTree Eligibility")
            || trimmed.contains("DDTree eligibility")
        {
            eligibility = Some(Eligibility::default());
            current_section = "ddtree";
            if trimmed.contains("✓")
                && (trimmed.contains("eligible") || trimmed.contains("supported"))
            {
                profile.ddtree_eligibility.supported = Some(true);
            } else if trimmed.contains("✗")
                || trimmed.contains("ineligible")
                || trimmed.contains("not supported")
            {
                profile.ddtree_eligibility.supported = Some(false);
            }
            continue;
        }
        if (trimmed.starts_with("##") || trimmed.starts_with("│") && trimmed.contains("│"))
            && trimmed.contains(":")
            && let Some((key, value)) = extract_pair(trimmed)
        {
            if version_trusted {
                let key_lower = key.to_ascii_lowercase().replace([' ', '_', '-'], "");
                let value_lower = value.to_ascii_lowercase();

                match key_lower.as_str() {
                    "toolformat" | "tool" => {
                        profile.tool_format = Some(value.to_string());
                    }
                    "reasoningparser" | "reasoning" => {
                        profile.reasoning_parser = Some(value.to_string());
                    }
                    "architecture" | "arch" => {
                        profile.architecture = Some(value.to_string());
                    }
                    "specdecode" | "speculative" => {
                        profile.spec_decode = parse_bool_value(&value_lower, '✓', '✗');
                    }
                    "mtp" | "mtpath" | "mtp-path" | "mtppath" => {
                        profile.mtp_path = match value_lower.as_str() {
                            v if v.contains("enabled") || v == "yes" || v == "✓" => {
                                Some(MtpPathStatus::Enabled)
                            }
                            v if v.contains("disabled") || v == "no" || v == "✗" => {
                                Some(MtpPathStatus::Disabled)
                            }
                            _ => Some(MtpPathStatus::Unknown),
                        };
                    }
                    "kvshare" | "kv-share" => {
                        profile.kv_share = Some(parse_yes_no(&value_lower));
                    }
                    "throttle" => {
                        profile.throttle = Some(parse_yes_no(&value_lower));
                    }
                    "suffixtier" | "suffix-tier" | "suffix" => {
                        profile.suffix_tier = Some(value.to_string());
                    }
                    _ => {}
                }

                if !current_section.is_empty()
                    && (trimmed.contains("Declared")
                        || trimmed.contains("MoE")
                        || trimmed.contains("Precision")
                        || trimmed.contains("Drafter")
                        || trimmed.contains("Runtime")
                        || trimmed.contains("Supported"))
                    && let Some(ref mut elig) = eligibility
                {
                    let reason_key = key.to_string();
                    let reason_val = Some(value.to_string());
                    elig.reasons.insert(reason_key, reason_val);
                    if value_lower.contains("supported") && !value_lower.contains("not supported") {
                        elig.supported = Some(true);
                    } else if value_lower.contains("not supported") || value_lower.contains("✗") {
                        elig.supported = Some(false);
                    }
                }

                if !current_section.is_empty()
                    && (key_lower.contains("supported") || key_lower.contains("eligible"))
                    && let Some(ref mut elig) = eligibility
                {
                    elig.supported = Some(parse_yes_no(&value_lower));
                }
            }

            if trimmed.contains("Supported") && (trimmed.contains("✓") || trimmed.contains("Yes"))
            {
                if current_section == "dflash" {
                    profile.dflash_eligibility.supported = Some(true);
                }
                if current_section == "ddtree" {
                    profile.ddtree_eligibility.supported = Some(true);
                }
            }
            if trimmed.contains("Not supported")
                || (trimmed.contains("Supported") && trimmed.contains("✗"))
            {
                if current_section == "dflash" {
                    profile.dflash_eligibility.supported = Some(false);
                }
                if current_section == "ddtree" {
                    profile.ddtree_eligibility.supported = Some(false);
                }
            }
        }

        let line_lower = line.to_ascii_lowercase();
        if line_lower.contains("vision") && line_lower.contains("tower") {
            profile.extras.has_vision_tower = true;
            profile.extras.vision = true;
        }
        if line_lower.contains("vision") && (trimmed.contains("✓") || trimmed.contains("yes")) {
            profile.extras.vision = true;
        }
        if line_lower.contains("embeddings") {
            profile.extras.embeddings = true;
        }
        if line_lower.contains("mtp-dflash") || line_lower.contains("mtp_dflash") {
            profile.extras.mtp_dflash = true;
        }

        if trimmed.starts_with("##") && !(trimmed.contains("DFlash") || trimmed.contains("DDTree"))
        {
            current_section = "";
            eligibility = None;
        }

        if (trimmed.starts_with("Model:") || trimmed.starts_with("Name:"))
            && let Some(id_part) = trimmed.split_once(':').map(|(_, rest)| rest.trim())
            && id_part.contains('/')
            && id_part.split('/').count() == 2
        {
            detected_hf_repo = Some(id_part.to_string());
        }
    }

    if eligibility.is_some() {
        if current_section == "dflash" {
            profile.dflash_eligibility = eligibility.take().unwrap_or_default();
        }
        if current_section == "ddtree" {
            profile.ddtree_eligibility = eligibility.take().unwrap_or_default();
        }
    }

    if let Some(ref repo) = detected_hf_repo {
        profile.is_finetune = repo == model_id;
    }

    if !profile.extras.vision {
        profile.extras.vision = vision_keywords_match(model_id);
    }
    if !profile.extras.vision
        && let Some(ref repo) = detected_hf_repo
    {
        profile.extras.vision = vision_keywords_match(repo);
    }
    if profile.extras.vision && !profile.extras.has_vision_tower {
        profile.extras.has_vision_tower = true;
    }

    Ok(Some(profile))
}

fn extract_pair(line: &str) -> Option<(String, String)> {
    let clean = line.trim_start_matches('│').trim_end_matches('│').trim();
    let colon_pos = clean.find(':')?;
    let key = clean[..colon_pos].trim().to_string();
    let value = clean[colon_pos + 1..].trim().to_string();
    if key.is_empty() || value.is_empty() {
        return None;
    }
    Some((key, value))
}

fn parse_bool_value(value: &str, yes_char: char, no_char: char) -> SpecDecodeSupport {
    if value.contains(yes_char) || value == "yes" || value == "supported" {
        SpecDecodeSupport::Supported
    } else if value.contains(no_char) || value == "no" || value == "unsupported" {
        SpecDecodeSupport::Unsupported
    } else {
        SpecDecodeSupport::Unknown
    }
}

fn parse_yes_no(value: &str) -> bool {
    value.contains("yes") || value.contains("✓") || value == "enabled" || value == "true"
}

fn vision_keywords_match(id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    lower.contains("vl")
        || lower.contains("vision")
        || lower.contains("multimodal")
        || lower.contains("mllm")
        || lower.contains("vlm")
}

#[allow(dead_code)]
fn parse_model_list(output: &str) -> Result<Vec<ModelListEntry>> {
    let mut entries = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('│') {
            continue;
        }
        if let Some((name, display_name)) = parse_list_line(trimmed) {
            entries.push(ModelListEntry { name, display_name });
        }
    }
    Ok(entries)
}

#[allow(dead_code)]
fn parse_list_line(line: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    let name = parts[0].to_string();
    let display = parts[1..].join(" ").trim().to_string();
    Some((name, display))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::FutureExt;

    #[test]
    fn parses_version_numbers_from_various_formats() {
        assert_eq!(
            parse_version_number("rapid-mlx 0.10.9"),
            Some(("0.10.9".into(), 10))
        );
        assert_eq!(
            parse_version_number("Rapid-MLX version v0.11.2\n"),
            Some(("0.11.2".into(), 11))
        );
        assert_eq!(parse_version_number("development"), None);
    }

    #[test]
    fn model_profile_serde_default_is_safe() {
        let json = r#"{}"#;
        let profile: ModelProfile = serde_json::from_str(json).unwrap();
        assert!(profile.tool_format.is_none());
        assert!(!profile.extras.vision);
    }

    #[test]
    fn eligibility_is_eligible_methods_work() {
        let yes = Eligibility {
            supported: Some(true),
            reasons: BTreeMap::new(),
        };
        let no = Eligibility {
            supported: Some(false),
            reasons: BTreeMap::new(),
        };
        let unknown = Eligibility {
            supported: None,
            reasons: BTreeMap::new(),
        };
        assert!(yes.is_eligible());
        assert!(!yes.is_ineligible());
        assert!(no.is_ineligible());
        assert!(!no.is_eligible());
        assert!(!unknown.is_eligible());
        assert!(!unknown.is_ineligible());
    }

    #[test]
    fn model_id_validation_rejects_paths() {
        assert!(
            fetch_model_profile(Path::new("rapid-mlx"), "../etc/passwd")
                .now_or_never()
                .is_some_and(|r| r.is_err())
        );
        assert!(
            fetch_model_profile(Path::new("rapid-mlx"), "/etc/passwd")
                .now_or_never()
                .is_some_and(|r| r.is_err())
        );
    }

    #[test]
    fn spec_decode_support_serializes_to_snake_case() {
        let supported = SpecDecodeSupport::Supported;
        let json = serde_json::to_string(&supported).unwrap();
        assert_eq!(json, r#""supported""#);
    }

    #[test]
    fn extra_capabilities_is_empty_by_default() {
        let caps = ExtraCapabilities::default();
        assert!(!caps.vision);
        assert!(!caps.has_vision_tower);
        assert!(!caps.embeddings);
        assert!(!caps.mtp_dflash);
    }

    #[tokio::test]
    #[ignore = "requires rapid-mlx CLI installed; run manually to verify parser against real output"]
    async fn parses_real_rapid_mlx_info_output_contract() {
        let output = std::process::Command::new("rapid-mlx")
            .args(["info", "qwen3-0.6b-4bit"])
            .output()
            .expect("rapid-mlx CLI must be installed for this contract test")
            .stdout;
        let text = String::from_utf8_lossy(&output);

        let profile = parse_model_profile(&text, true, "qwen3-0.6b-4bit")
            .unwrap()
            .expect("should parse");

        assert_eq!(profile.tool_format, Some("hermes".into()));
        assert_eq!(profile.reasoning_parser, Some("qwen3".into()));
        assert_eq!(profile.architecture, Some("pure attention".into()));
        assert_eq!(profile.spec_decode, SpecDecodeSupport::Supported);
        assert_eq!(profile.mtp_path, Some(MtpPathStatus::Disabled));
        assert_eq!(profile.kv_share, Some(false));
        assert_eq!(profile.throttle, Some(false));
        assert_eq!(profile.dflash_eligibility.supported, Some(false));
        assert_eq!(profile.ddtree_eligibility.supported, Some(false));
        assert!(!profile.extras.vision);
        assert!(!profile.extras.embeddings);
        assert!(!profile.is_finetune);
    }

    #[test]
    fn finetune_detection_marks_unknown_hf_repos() {
        let output =
            "Model: user/my-finetuned-model\nTool format: hermes\nSpec decode: ✓ supported";

        let profile = parse_model_profile(output, true, "user/my-finetuned-model")
            .unwrap()
            .unwrap();
        assert!(profile.is_finetune);

        let profile2 = parse_model_profile(output, true, "known-alias")
            .unwrap()
            .unwrap();
        assert!(!profile2.is_finetune);
    }

    #[test]
    fn untrusted_version_returns_minimal_profile() {
        let output = r#"┌──────────────────────────────────────────────────────────────┐
│ Model: mlx-community/Qwen3-0.6B-4bit                         │
│ ──────────────────────────────────────────────────────────── │
│ Tool format      : hermes                                    │
│ Reasoning parser : qwen3                                     │
│ Architecture     : pure attention                            │
│ Spec decode      : ✓ supported                               │
└──────────────────────────────────────────────────────────────┘"#;

        let profile = parse_model_profile(output, false, "qwen3-0.6b-4bit")
            .unwrap()
            .unwrap();
        assert!(profile.tool_format.is_none());
        assert!(profile.reasoning_parser.is_none());
        assert!(profile.architecture.is_none());
        assert_eq!(profile.spec_decode, SpecDecodeSupport::Unknown);
    }

    #[test]
    fn vision_detected_by_model_id_keyword() {
        let output = r#"┌──────────────────────────────────────────────────────────────┐
│ Model: mlx-community/Qwen3-VL-30B-4bit                       │
│ ──────────────────────────────────────────────────────────── │
│ Tool format      : hermes                                    │
└──────────────────────────────────────────────────────────────┘"#;

        for model_id in &[
            "qwen3-vl-30b-4bit",
            "Qwen2.5-VL-3B",
            "phi-3-vision",
            "llava-multimodal",
            "mllm-7b",
            "paligemma-vlm-13b",
        ] {
            let profile = parse_model_profile(output, true, model_id)
                .unwrap()
                .unwrap();
            assert!(
                profile.extras.vision,
                "vision should be true for {}",
                model_id
            );
            assert!(
                profile.extras.has_vision_tower,
                "has_vision_tower should be true for {}",
                model_id
            );
        }
    }

    #[test]
    fn vision_detected_by_hf_repo_keyword() {
        let output = r#"┌──────────────────────────────────────────────────────────────┐
│ Model: mlx-community/Qwen2.5-VL-7B-4bit                      │
└──────────────────────────────────────────────────────────────┘"#;

        let profile = parse_model_profile(output, true, "some-alias-without-vl")
            .unwrap()
            .unwrap();
        assert!(
            profile.extras.vision,
            "should detect vision from HF repo path"
        );
        assert!(
            profile.extras.has_vision_tower,
            "should set has_vision_tower from HF repo path"
        );
    }

    #[test]
    fn non_vision_models_not_false_positive() {
        let output = r#"┌──────────────────────────────────────────────────────────────┐
│ Model: mlx-community/Qwen3-0.6B-4bit                         │
└──────────────────────────────────────────────────────────────┘"#;

        for model_id in &[
            "qwen3-0.6b-4bit",
            "llama-3.1-8b",
            "gemma-2-9b",
            "mistral-nemo",
        ] {
            let profile = parse_model_profile(output, true, model_id)
                .unwrap()
                .unwrap();
            assert!(
                !profile.extras.vision,
                "vision should be false for {}",
                model_id
            );
        }
    }
}
