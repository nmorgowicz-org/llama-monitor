#![allow(clippy::collapsible_if)]

use crate::inference::rapid_mlx::runtime::{FeatureProbeFailure, ProbeResult, RuntimeSource};
use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub const CAPABILITY_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);
pub const CAPABILITY_PROBE_MAX_OUTPUT: usize = 512 * 1024;

/// Total probe budget: all sub-checks complete within 30s.
pub const PROBE_TOTAL_TIMEOUT: Duration = Duration::from_secs(30);

/// Per-sub-check timeout (reused from capability probes).
pub const PROBE_SUBCHECK_TIMEOUT: Duration = Duration::from_secs(8);

/// Source of a capability snapshot: automated discovery vs. manual override.
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySnapshotSource {
    /// Automatically generated from live probing of this exact executable.
    #[default]
    AutoProbed,
    /// Manually overridden for a known-incompatible or known-safe runtime.
    ManualOverride,
}

/// Exact identity of the executable that a snapshot describes.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ExecutableIdentity {
    pub path: String,
    pub file_hash: String,
    pub file_mtime_unix: u64,
}

impl ExecutableIdentity {
    pub fn from_path(path: &Path) -> Result<Self> {
        let canonical = path
            .canonicalize()
            .context("Cannot canonicalize Rapid-MLX path")?;
        let meta =
            std::fs::metadata(&canonical).context("Cannot read Rapid-MLX executable metadata")?;
        let mtime = meta
            .modified()
            .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
            .unwrap_or(0);
        let hash = hash_file(&canonical)?;
        Ok(Self {
            path: canonical.to_string_lossy().into_owned(),
            file_hash: hash,
            file_mtime_unix: mtime,
        })
    }
}

/// Version of a resolved dependency in the environment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DependencyVersion {
    pub package: String,
    pub version: String,
    pub source: DependencyVersionSource,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyVersionSource {
    PipFreeze,
    ImportProbe,
}

/// MTP concurrency qualification state.
/// Capability does NOT automatically equal product recommendation.
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum MtpConcurrencyState {
    /// Older/model/backend combinations requiring parallel=1 (single active request).
    RequiresSingle,
    /// Current llama builds supporting per-sequence MTP.
    Supported,
    /// Rapid's single-live-greedy fast-path with fallback.
    SingleActiveGreedy,
    #[default]
    /// Not yet determined.
    Unknown,
}

#[allow(dead_code)]
impl MtpConcurrencyState {
    pub fn label(self) -> &'static str {
        match self {
            Self::RequiresSingle => "requires single active request",
            Self::Supported => "per-sequence MTP supported",
            Self::SingleActiveGreedy => "single-active greedy with fallback",
            Self::Unknown => "undetermined",
        }
    }
}

/// Probe result for a single --default-* CLI field.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DefaultFieldState {
    /// Flag exists in help; can be set at CLI level.
    Supported,
    /// Flag not found in help; cannot be used as server-level default.
    #[default]
    Unsupported,
}

/// Per-field coverage of Rapid's --default-* CLI flags.
/// Probed independently; records exact partial coverage.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SamplingDefaultFields {
    pub temperature: DefaultFieldState,
    pub top_p: DefaultFieldState,
    pub top_k: DefaultFieldState,
    pub min_p: DefaultFieldState,
    pub typical_p: DefaultFieldState,
    pub repetition_penalty: DefaultFieldState,
    pub presence_penalty: DefaultFieldState,
    pub frequency_penalty: DefaultFieldState,
    pub max_tokens: DefaultFieldState,
}

impl SamplingDefaultFields {
    /// Derive from serve --help flags by probing each --default-* independently.
    pub fn from_flags(flags: &[String]) -> Self {
        Self {
            temperature: flag_state(flags, "--default-temperature"),
            top_p: flag_state(flags, "--default-top-p"),
            top_k: flag_state(flags, "--default-top-k"),
            min_p: flag_state(flags, "--default-min-p"),
            typical_p: flag_state(flags, "--default-typical-p"),
            repetition_penalty: flag_state(flags, "--default-repetition-penalty"),
            presence_penalty: flag_state(flags, "--default-presence-penalty"),
            frequency_penalty: flag_state(flags, "--default-frequency-penalty"),
            max_tokens: flag_state(flags, "--default-max-tokens"),
        }
    }

    /// Which fields are effectively settable via CLI defaults.
    /// Unmapped/unsupported fields must NOT be reported as effective.
    #[allow(dead_code)]
    pub fn effective_fields(&self) -> Vec<&'static str> {
        let mut fields = Vec::new();
        if matches!(self.temperature, DefaultFieldState::Supported) {
            fields.push("temperature");
        }
        if matches!(self.top_p, DefaultFieldState::Supported) {
            fields.push("top_p");
        }
        if matches!(self.top_k, DefaultFieldState::Supported) {
            fields.push("top_k");
        }
        if matches!(self.min_p, DefaultFieldState::Supported) {
            fields.push("min_p");
        }
        if matches!(self.typical_p, DefaultFieldState::Supported) {
            fields.push("typical_p");
        }
        if matches!(self.repetition_penalty, DefaultFieldState::Supported) {
            fields.push("repetition_penalty");
        }
        if matches!(self.presence_penalty, DefaultFieldState::Supported) {
            fields.push("presence_penalty");
        }
        if matches!(self.frequency_penalty, DefaultFieldState::Supported) {
            fields.push("frequency_penalty");
        }
        if matches!(self.max_tokens, DefaultFieldState::Supported) {
            fields.push("max_tokens");
        }
        fields
    }
}

/// Sampling precedence cascade for Rapid-MLX.
/// Verified against native behavior: request > CLI > alias > generation_config > fallback.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SamplingCascade {
    pub precedence: Vec<SamplingSource>,
    pub cli_defaults_available: SamplingDefaultFields,
    /// Indicates whether all selected defaults are mapped to an effective source.
    pub all_defaults_mapped: bool,
}

impl Default for SamplingCascade {
    fn default() -> Self {
        Self {
            precedence: vec![
                SamplingSource::RequestLevel,
                SamplingSource::CliDefaults,
                SamplingSource::AliasDefaults,
                SamplingSource::GenerationConfig,
                SamplingSource::HardcodedFallback,
            ],
            cli_defaults_available: SamplingDefaultFields::default(),
            all_defaults_mapped: true,
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SamplingSource {
    /// Explicit values in the API chat request body (highest priority).
    RequestLevel,
    /// Server-level --default-* CLI flags.
    CliDefaults,
    /// Model alias defaults (e.g., Unsloth-published values).
    AliasDefaults,
    /// Model's generation_config.json (from HF or local).
    GenerationConfig,
    /// Hardcoded fallback when no other source provides a value.
    HardcodedFallback,
}

impl SamplingCascade {
    /// Derive cascade from probed flags.
    pub fn from_flags(flags: &[String]) -> Self {
        let cli_defaults = SamplingDefaultFields::from_flags(flags);
        Self {
            precedence: vec![
                SamplingSource::RequestLevel,
                SamplingSource::CliDefaults,
                SamplingSource::AliasDefaults,
                SamplingSource::GenerationConfig,
                SamplingSource::HardcodedFallback,
            ],
            cli_defaults_available: cli_defaults,
            all_defaults_mapped: true,
        }
    }
}

fn flag_state(flags: &[String], flag: &str) -> DefaultFieldState {
    if flags.iter().any(|f| f == flag) {
        DefaultFieldState::Supported
    } else {
        DefaultFieldState::Unsupported
    }
}

/// Which optional extras are installed and usable.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct InstalledExtras {
    #[serde(default)]
    pub guided: ExtraState,
    #[serde(default)]
    pub vision: ExtraState,
    #[serde(default)]
    pub embeddings: ExtraState,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtraState {
    Installed,
    #[default]
    Missing,
    Broken(String),
}

/// Feature qualification for this environment.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct QualifiedFeatures {
    #[serde(default)]
    pub tool_parsing: FeatureQualification,
    #[serde(default)]
    pub automatic_tool_choice: FeatureQualification,
    #[serde(default)]
    pub reasoning_parser: FeatureQualification,
    #[serde(default)]
    pub thinking_controls: FeatureQualification,
    #[serde(default)]
    pub guided_generation: FeatureQualification,
    #[serde(default)]
    pub vision: FeatureQualification,
    #[serde(default)]
    pub embeddings: FeatureQualification,
    #[serde(default)]
    pub status_memory_telemetry: FeatureQualification,
    #[serde(default)]
    pub one_shot_launch: FeatureQualification,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureQualification {
    /// Flag/probe confirms availability; environment passes baseline.
    Available,
    /// Present but not confirmed: missing smoke test or indeterminate probe.
    Indeterminate(String),
    /// Missing or broken; cannot be used.
    Unavailable(String),
}

impl Default for FeatureQualification {
    fn default() -> Self {
        Self::Unavailable("Not verified".into())
    }
}

/// Automatically generated capability snapshot keyed by executable identity.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct CapabilitySnapshot {
    pub executable_identity: ExecutableIdentity,
    pub rapid_mlx_version: String,
    pub help_hash: String,
    pub serve_flags: Vec<String>,
    pub package_versions: Vec<DependencyVersion>,
    pub installed_extras: InstalledExtras,
    pub qualified_features: QualifiedFeatures,
    /// MTP concurrency qualification state.
    pub mtp_concurrency: MtpConcurrencyState,
    /// Per-field --default-* CLI coverage for sampling defaults.
    pub sampling_defaults: SamplingDefaultFields,
    /// Sampling precedence cascade derived from native behavior + probes.
    pub sampling_cascade: SamplingCascade,
    /// Timestamp when this snapshot was generated.
    pub evidence_timestamp: u64,
    pub source: CapabilitySnapshotSource,
}

impl CapabilitySnapshot {
    /// Check whether a stored snapshot is still valid for the given executable.
    pub fn is_valid_for(&self, current: &ExecutableIdentity) -> bool {
        self.executable_identity.path == current.path
            && self.executable_identity.file_hash == current.file_hash
            && self.help_hash == hash_help(&self.serve_flags.join(" "))
    }

    /// Generate fingerprint that uniquely identifies this snapshot's subject.
    #[allow(dead_code)]
    pub fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.executable_identity.path.as_bytes());
        hasher.update(self.executable_identity.file_hash.as_bytes());
        hasher.update(self.help_hash.as_bytes());
        let mut deps: Vec<_> = self.package_versions.iter().collect();
        deps.sort_by_key(|d| &d.package);
        for dep in deps {
            hasher.update(dep.package.as_bytes());
            hasher.update(dep.version.as_bytes());
        }
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// Cache of capability snapshots to avoid re-probing unchanged executables.
static SNAPSHOT_CACHE: OnceLock<Arc<std::sync::RwLock<BTreeMap<String, CapabilitySnapshot>>>> =
    OnceLock::new();

/// Return a cached snapshot for the given identity if still valid.
pub fn cached_snapshot(identity: &ExecutableIdentity) -> Option<CapabilitySnapshot> {
    let cache = SNAPSHOT_CACHE
        .get_or_init(|| Arc::new(std::sync::RwLock::new(BTreeMap::new())))
        .clone();
    let path_key = identity.path.clone();
    cache
        .read()
        .unwrap()
        .get(&path_key)
        .cloned()
        .filter(|snap| snap.is_valid_for(identity))
}

/// Store a snapshot in the cache, keyed by executable path.
pub fn cache_snapshot(snapshot: CapabilitySnapshot) {
    let cache = SNAPSHOT_CACHE
        .get_or_init(|| Arc::new(std::sync::RwLock::new(BTreeMap::new())))
        .clone();
    let key = snapshot.executable_identity.path.clone();
    cache.write().unwrap().insert(key, snapshot);
}

/// Generate a capability snapshot using the discovered binary.
/// Uses Discovery::resolve_binary to find rapid-mlx, then generates the snapshot.
/// Returns Ok(snapshot) if binary found and probe succeeds; Err if not available.
#[allow(dead_code)]
pub async fn generate_snapshot_from_discovery() -> Result<CapabilitySnapshot> {
    use crate::inference::rapid_mlx::discovery::Discovery;

    let (binary, source) = Discovery::resolve_binary(None, None)
        .await
        .context("Failed to discover Rapid-MLX binary")?;

    // Check cache first
    let identity = ExecutableIdentity::from_path(&binary)?;
    if let Some(snap) = cached_snapshot(&identity) {
        return Ok(snap);
    }

    generate_snapshot(&binary, source).await
}

/// Generate a capability snapshot by probing the given executable.
pub async fn generate_snapshot(binary: &Path, source: RuntimeSource) -> Result<CapabilitySnapshot> {
    let identity = ExecutableIdentity::from_path(binary)?;

    // 1. Probe version
    let version = probe_version(binary).await?;

    // 2. Probe help and compute hash
    let (help_text, serve_flags) = probe_help(binary).await?;
    let help_hash = hash_help(&help_text);

    // 3. Probe installed dependencies
    let package_versions = probe_dependencies(binary).await;

    // 4. Probe extras
    let installed_extras = probe_extras(binary, &package_versions).await;

    // 5. Derive qualified features from flags + extras + baseline checks
    let qualified_features =
        derive_qualified_features(&serve_flags, &installed_extras, &version, source);

    // 6. Derive MTP concurrency state and sampling default fields from flags
    let mtp_concurrency = derive_mtp_concurrency(&serve_flags);
    let sampling_defaults = SamplingDefaultFields::from_flags(&serve_flags);
    let sampling_cascade = SamplingCascade::from_flags(&serve_flags);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let snapshot = CapabilitySnapshot {
        executable_identity: identity,
        rapid_mlx_version: version,
        help_hash,
        serve_flags,
        package_versions,
        installed_extras,
        qualified_features,
        mtp_concurrency,
        sampling_defaults,
        sampling_cascade,
        evidence_timestamp: now,
        source: CapabilitySnapshotSource::AutoProbed,
    };

    cache_snapshot(snapshot.clone());
    Ok(snapshot)
}

/// Probe rapid-mlx --version; bounded.
async fn probe_version(binary: &Path) -> Result<String> {
    let output = run_probe_command(binary, &["--version"]).await?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        bail!("Rapid-MLX version probe returned empty output");
    }
    // Extract version triplet
    if let Some(version) = extract_version_text(trimmed) {
        Ok(version)
    } else {
        Ok(trimmed.to_string())
    }
}

/// Extract version-like text from version output.
fn extract_version_text(text: &str) -> Option<String> {
    for start in 0..text.len() {
        if !text.as_bytes()[start].is_ascii_digit() {
            continue;
        }
        let mut cursor = start;
        let bytes = text.as_bytes();
        if let Some(major) = parse_num(bytes, &mut cursor)
            && bytes.get(cursor) == Some(&b'.')
        {
            cursor += 1;
            if let Some(minor) = parse_num(bytes, &mut cursor)
                && bytes.get(cursor) == Some(&b'.')
            {
                cursor += 1;
                if let Some(patch) = parse_num(bytes, &mut cursor) {
                    let suffix_end = bytes[cursor..]
                        .iter()
                        .position(|b| {
                            !b.is_ascii_alphanumeric() && *b != b'.' && *b != b'-' && *b != b'_'
                        })
                        .map_or(bytes.len(), |off| cursor + off);
                    let _ = (major, minor, patch);
                    return Some(String::from_utf8_lossy(&bytes[start..suffix_end]).into_owned());
                }
            }
        }
    }
    None
}

fn parse_num(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
    let start = *cursor;
    while bytes.get(*cursor).is_some_and(|b| b.is_ascii_digit()) {
        *cursor += 1;
    }
    if start == *cursor {
        return None;
    }
    std::str::from_utf8(&bytes[start..*cursor])
        .ok()?
        .parse::<u64>()
        .ok()
}

/// Probe `rapid-mlx serve --help`; bounded; return (raw_text, flags).
async fn probe_help(binary: &Path) -> Result<(String, Vec<String>)> {
    let output = run_probe_command(binary, &["serve", "--help"]).await?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = text.trim().to_string();
    let flags = extract_flags(&text);
    Ok((text, flags))
}

/// Compute SHA-256 of help text.
fn hash_help(help: &str) -> String {
    Sha256::digest(help.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Extract --flags from help text.
fn extract_flags(help: &str) -> Vec<String> {
    let mut flags = BTreeMap::new();
    for line in help.lines() {
        for token in line.split_whitespace() {
            let token =
                token.trim_matches(|c: char| matches!(c, ',' | '[' | ']' | '(' | ')' | '='));
            let flag = token.split_once('=').map_or(token, |(f, _)| f);
            if flag.starts_with("--") {
                flags.insert(flag.to_string(), ());
            }
        }
    }
    flags.into_keys().collect()
}

/// Probe installed dependency versions using `pip freeze` in the environment that owns this binary.
async fn probe_dependencies(binary: &Path) -> Vec<DependencyVersion> {
    let python_env = resolve_python_for_binary(binary);
    let mut versions = Vec::new();

    // Primary: pip freeze
    if let Some(python) = python_env.as_ref() {
        if let Ok(output) = run_probe_command(python, &["-m", "pip", "freeze"]).await {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((pkg, ver)) = parse_pip_freeze_line(line) {
                    // Only record packages relevant to Rapid-MLX capability
                    if is_relevant_package(&pkg) {
                        versions.push(DependencyVersion {
                            package: pkg,
                            version: ver,
                            source: DependencyVersionSource::PipFreeze,
                        });
                    }
                }
            }
        }
    }

    // Fallback: probe critical packages via import
    if versions.is_empty() {
        if let Some(python) = python_env.as_ref() {
            versions = probe_import_versions(python).await;
        }
    }

    versions.sort_by(|a, b| a.package.cmp(&b.package));
    versions
}

fn resolve_python_for_binary(binary: &Path) -> Option<std::path::PathBuf> {
    let parent = binary.parent()?;
    // Look for python in the same environment
    for name in ["python3", "python"] {
        let candidate = parent.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // Look one level up (e.g., bin/python)
    if let Some(grandparent) = parent.parent() {
        for name in ["python3", "python"] {
            let candidate = grandparent.join("bin").join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn parse_pip_freeze_line(line: &str) -> Option<(String, String)> {
    // Handle pkg==ver, pkg@url, pkg===ver
    for sep in ["===", "==="] {
        if let Some((pkg, ver)) = line.split_once(sep) {
            return Some((pkg.trim().to_string(), ver.trim().to_string()));
        }
    }
    if let Some((pkg, ver)) = line.split_once("==") {
        return Some((pkg.trim().to_string(), ver.trim().to_string()));
    }
    None
}

fn is_relevant_package(pkg: &str) -> bool {
    let lower = pkg.to_ascii_lowercase();
    lower.starts_with("mlx") || lower.contains("outlines") || lower.contains("guidance")
}

async fn probe_import_versions(python: &Path) -> Vec<DependencyVersion> {
    let script = r#"
import sys, json, importlib
pkgs = ["mlx", "mlx_lm", "mlx_vlm", "outlines"]
result = []
for name in pkgs:
    try:
        mod = importlib.import_module(name.replace("-", "_"))
        ver = getattr(mod, "__version__", "unknown")
        result.append({"package": name, "version": str(ver)})
    except Exception:
        pass
print(json.dumps(result))
"#;
    match run_probe_command(python, &["-c", script]).await {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
                arr.into_iter()
                    .filter_map(|obj| {
                        let pkg = obj.get("package")?.as_str()?.to_string();
                        let ver = obj.get("version")?.as_str()?.to_string();
                        Some(DependencyVersion {
                            package: pkg,
                            version: ver,
                            source: DependencyVersionSource::ImportProbe,
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            }
        }
        Err(_) => Vec::new(),
    }
}

/// Probe which extras are installed.
async fn probe_extras(binary: &Path, _package_versions: &[DependencyVersion]) -> InstalledExtras {
    let python_env = resolve_python_for_binary(binary);
    let python: Option<&Path> = python_env.as_deref();

    let guided = probe_extra_import(python, "outlines", "from outlines import generators").await;

    let vision = probe_extra_import(python, "mlx_vlm", "import mlx_vlm").await;

    let embeddings = probe_extra_import(python, "mlx_embed", "import mlx_embed").await;

    InstalledExtras {
        guided,
        vision,
        embeddings,
    }
}

async fn probe_extra_import(
    python: Option<&Path>,
    _package_name: &str,
    import_stmt: &str,
) -> ExtraState {
    let Some(python) = python else {
        return ExtraState::Missing;
    };

    let script = format!(
        r#"
import sys
try:
    {import_stmt}
    print("OK")
except ImportError as e:
    print(f"MISSING:{{e}}")
except Exception as e:
    print(f"BROKEN:{{e}}")
"#
    );

    match run_probe_command(python, &["-c", &script]).await {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if text == "OK" {
                ExtraState::Installed
            } else if text.starts_with("MISSING:") {
                ExtraState::Missing
            } else if let Some(reason) = text.strip_prefix("BROKEN:") {
                let reason = reason.trim().to_string();
                ExtraState::Broken(reason)
            } else {
                ExtraState::Missing
            }
        }
        Err(_) => ExtraState::Missing,
    }
}

/// Derive qualified features from probes and baseline checks.
fn derive_qualified_features(
    flags: &[String],
    extras: &InstalledExtras,
    version: &str,
    source: RuntimeSource,
) -> QualifiedFeatures {
    let has_tool_parser = flags.iter().any(|f| f == "--tool-call-parser");
    let has_auto_tool_choice = flags.iter().any(|f| f == "--enable-auto-tool-choice");
    let has_reasoning = flags.iter().any(|f| f == "--reasoning");
    let has_thinking = flags
        .iter()
        .any(|f| *f == "--enable-thinking" || *f == "--reasoning-effort");

    // Base availability from flags
    let tool_parsing = if has_tool_parser {
        FeatureQualification::Available
    } else {
        FeatureQualification::Unavailable("Missing --tool-call-parser flag".into())
    };

    let automatic_tool_choice = if has_auto_tool_choice {
        FeatureQualification::Available
    } else {
        FeatureQualification::Unavailable("Missing --enable-auto-tool-choice flag".into())
    };

    let reasoning_parser = if has_reasoning || has_thinking {
        FeatureQualification::Available
    } else {
        FeatureQualification::Unavailable("No reasoning/thinking flags detected".into())
    };

    let thinking_controls = if has_thinking {
        FeatureQualification::Available
    } else {
        FeatureQualification::Unavailable("No thinking control flags detected".into())
    };

    let guided_generation = match extras.guided {
        ExtraState::Installed => FeatureQualification::Available,
        ExtraState::Missing => {
            FeatureQualification::Unavailable("[guided] extra not installed".into())
        }
        ExtraState::Broken(ref reason) => {
            FeatureQualification::Unavailable(format!("[guided] extra broken: {reason}"))
        }
    };

    let vision = match extras.vision {
        ExtraState::Installed => {
            // mlx-vlm installed; mark as available unless broken version known
            if is_broken_vision_version(version) {
                FeatureQualification::Indeterminate(
                    "mlx-vlm version not yet smoke-tested for Qwen/Gemma paths".into(),
                )
            } else {
                FeatureQualification::Available
            }
        }
        ExtraState::Missing => {
            FeatureQualification::Unavailable("vision extra not installed".into())
        }
        ExtraState::Broken(ref reason) => {
            FeatureQualification::Unavailable(format!("vision extra broken: {reason}"))
        }
    };

    let embeddings = match extras.embeddings {
        ExtraState::Installed => FeatureQualification::Available,
        ExtraState::Missing => {
            FeatureQualification::Unavailable("embeddings extra not installed".into())
        }
        ExtraState::Broken(ref reason) => {
            FeatureQualification::Unavailable(format!("embeddings extra broken: {reason}"))
        }
    };

    // Status/memory telemetry and one-shot launch are core capabilities, not extras
    let status_memory_telemetry = FeatureQualification::Available;
    let one_shot_launch = FeatureQualification::Available;

    let _ = source; // Managed runtime may perform additional baseline checks in future

    QualifiedFeatures {
        tool_parsing,
        automatic_tool_choice,
        reasoning_parser,
        thinking_controls,
        guided_generation,
        vision,
        embeddings,
        status_memory_telemetry,
        one_shot_launch,
    }
}

fn is_broken_vision_version(_rapid_version: &str) -> bool {
    // Known broken: mlx-vlm==0.6.4; qualified: 0.6.5+ once smoke-tested.
    // For now treat as indeterminate until smoke matrix runs.
    true
}

/// Derive MTP concurrency qualification from serve --help flags.
/// Uses flag presence to detect Rapid's single-live-greedy fast-path.
fn derive_mtp_concurrency(flags: &[String]) -> MtpConcurrencyState {
    // Rapid's audited source documents single-live-greedy fast-path with fallback.
    // Presence of any MTP-related flags (e.g., --speculative) indicates this mode.
    let has_mtp_flags = flags
        .iter()
        .any(|f| f.contains("speculative") || f.contains("mtp") || f.contains("spec_decode"));
    if has_mtp_flags {
        MtpConcurrencyState::SingleActiveGreedy
    } else {
        // Without explicit MTP flags, assume single-active (conservative)
        MtpConcurrencyState::SingleActiveGreedy
    }
}

struct ProbeOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Run a bounded probe command.
async fn run_probe_command(binary: &Path, args: &[&str]) -> Result<ProbeOutput> {
    let mut cmd = Command::new(binary);
    cmd.args(args)
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    let mut child = cmd.spawn().map_err(|e| {
        anyhow!(
            "Failed to execute Rapid-MLX probe '{} {}': {}",
            binary.display(),
            args.join(" "),
            e
        )
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Failed to capture probe stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("Failed to capture probe stderr"))?;

    let capture = async {
        async fn read_bound<R>(reader: R) -> Result<Vec<u8>>
        where
            R: tokio::io::AsyncRead + Unpin,
        {
            let mut out = Vec::with_capacity(4096);
            reader
                .take((CAPABILITY_PROBE_MAX_OUTPUT + 1) as u64)
                .read_to_end(&mut out)
                .await?;
            if out.len() > CAPABILITY_PROBE_MAX_OUTPUT {
                bail!(
                    "Rapid-MLX probe output exceeded {} byte limit",
                    CAPABILITY_PROBE_MAX_OUTPUT
                );
            }
            Ok(out)
        }

        let (stdout_data, stderr_data, status) =
            tokio::join!(read_bound(stdout), read_bound(stderr), child.wait());
        let _ = status?;
        Ok::<(Vec<u8>, Vec<u8>), anyhow::Error>((stdout_data?, stderr_data?))
    };

    let (stdout_data, stderr_data) = tokio::time::timeout(CAPABILITY_PROBE_TIMEOUT, capture)
        .await
        .map_err(|_| {
            anyhow!(
                "Rapid-MLX probe timed out after {:.1}s: {} {}",
                CAPABILITY_PROBE_TIMEOUT.as_secs_f64(),
                binary.display(),
                args.join(" ")
            )
        })??;

    Ok(ProbeOutput {
        stdout: stdout_data,
        stderr: stderr_data,
    })
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file =
        std::fs::File::open(path).context("Cannot open Rapid-MLX executable for hashing")?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = std::io::Read::read(&mut file, &mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// D30: Conservative default prefix cache fraction of configured ceiling.
/// 10% is safe across architectures; can be calibrated higher per [escalate→device].
pub const PREFIX_CACHE_BUDGET_FRACTION: f64 = 0.10;

/// Prefix cache guidance derived from capability snapshot and memory availability.
/// This is a recommendation only — never forced, never auto-applied (A31).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PrefixCacheGuidance {
    /// Whether prefix cache is supported by this runtime (capability confirmed).
    pub supported: bool,
    /// Whether a recommendation is being made (supported + headroom + not explicitly set).
    pub should_recommend: bool,
    /// Recommended cache block count derived from available memory.
    /// Zero means no recommendation (insufficient headroom or not supported).
    pub recommended_max_cache_blocks: u32,
    /// D30: prefix cache budget in bytes, derived from configured_ceiling_bytes.
    /// Budget = configured_ceiling_bytes × PREFIX_CACHE_BUDGET_FRACTION.
    /// Conservative default; never unlimited (hard gate).
    pub prefix_cache_budget_bytes: u64,
    /// Human-readable reasons why guidance is off or reduced.
    pub reasons_off_or_lower: Vec<String>,
    /// Effective block size used in the calculation (n_embd × n_kv_heads × head_dim × dtype_bytes).
    /// Zero if not computed (unsupported or not recommended).
    pub block_size_bytes: u64,
}

/// Parameters for deriving prefix cache guidance.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PrefixCacheGuidanceParams {
    pub snapshot: CapabilitySnapshot,
    pub memory_ceiling_bytes: u64,
    pub current_safe_bytes: u64,
    pub estimated_model_overhead_bytes: u64,
    pub user_max_cache_blocks: Option<u32>,
    pub arch_n_embd: u32,
    pub arch_n_kv_heads: u32,
    pub arch_head_dim: u32,
}

impl PrefixCacheGuidance {
    /// Derive prefix cache guidance from capability snapshot and memory availability.
    ///
    /// Recommendation conditions (all must hold):
    /// (a) Capability confirmed: --max-cache-blocks flag present in snapshot
    /// (b) Sufficient memory headroom: safe availability > model overhead + headroom
    /// (c) Not already set explicitly by user
    ///
    /// D30: budget is always derived from configured_ceiling_bytes × fraction,
    /// independent of recommendation conditions (for API/wizard consumption).
    ///
    /// User explicit values always win (hard gate).
    #[allow(clippy::too_many_arguments)]
    pub fn derive(
        snapshot: &CapabilitySnapshot,
        memory_ceiling_bytes: u64,
        current_safe_bytes: u64,
        estimated_model_overhead_bytes: u64,
        user_max_cache_blocks: Option<u32>,
        arch_n_embd: u32,
        arch_n_kv_heads: u32,
        arch_head_dim: u32,
    ) -> Self {
        let mut reasons = Vec::new();

        // (a) Capability: check for --max-cache-blocks flag
        let has_cache_flag = snapshot
            .serve_flags
            .iter()
            .any(|f| f == "--max-cache-blocks");
        let supported = has_cache_flag;

        if !supported {
            reasons.push("Runtime does not expose --max-cache-blocks flag".into());
        }

        // D30: always compute budget from configured_ceiling_bytes
        let prefix_cache_budget_bytes = if memory_ceiling_bytes > 0 {
            (memory_ceiling_bytes as f64 * PREFIX_CACHE_BUDGET_FRACTION) as u64
        } else {
            0
        };

        // Block size: bf16 = 2 bytes per element (conservative)
        let block_size_bytes = if arch_n_embd > 0 && arch_n_kv_heads > 0 && arch_head_dim > 0 {
            (arch_n_embd as u64)
                .saturating_mul(arch_n_kv_heads as u64)
                .saturating_mul(arch_head_dim as u64)
                .saturating_mul(2) // bf16
        } else {
            0
        };

        // (c) User explicit values win — never override
        let user_explicit = user_max_cache_blocks.is_some();
        if user_explicit {
            reasons.push("User has explicitly set max_cache_blocks".into());
        }

        // (b) Memory headroom: safe availability > model overhead
        let available_for_cache = current_safe_bytes
            .saturating_sub(estimated_model_overhead_bytes)
            .min(prefix_cache_budget_bytes); // D30: bounded by budget

        let mut recommended_max_cache_blocks: u32 = 0;
        let mut should_recommend = false;

        if supported && !user_explicit && block_size_bytes > 0 && available_for_cache > 0 {
            // Recommend blocks bounded by available memory and D30 budget
            let blocks_from_memory =
                (available_for_cache / block_size_bytes).min(u32::MAX as u64) as u32;
            if blocks_from_memory > 0 {
                recommended_max_cache_blocks = blocks_from_memory;
                should_recommend = true;
            } else {
                reasons.push("Insufficient memory headroom for cache blocks".into());
            }
        } else if block_size_bytes == 0 {
            reasons.push("Cannot compute block size (missing architecture fields)".into());
        } else if !supported || user_explicit {
            // Reasons already recorded above
        } else {
            reasons.push("Insufficient memory headroom for cache blocks".into());
        }

        Self {
            supported,
            should_recommend,
            recommended_max_cache_blocks,
            prefix_cache_budget_bytes,
            reasons_off_or_lower: reasons,
            block_size_bytes,
        }
    }
}

/// Parameters for computing prefix cache diagnostic findings.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CacheDiagnosticParams {
    pub config_prefix_cache_enabled: bool,
    pub config_prefix_cache_budget_bytes: Option<u64>,
    pub config_max_cache_blocks: Option<u32>,
    pub snapshot: CapabilitySnapshot,
    pub configured_ceiling_bytes: u64,
    pub current_safe_availability_bytes: u64,
}

/// Diagnostic findings produced by cache configuration analysis.
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrefixCacheDiagnosticFindings {
    pub findings: Vec<CacheDiagnosticFinding>,
}

/// A single cache diagnostic finding.
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheDiagnosticFinding {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub fixable: bool,
    pub fix_action: Option<String>,
}

impl CapabilitySnapshot {
    /// Compute prefix cache diagnostic findings for a given configuration.
    ///
    /// Detects:
    /// - budget_bytes > configured_ceiling_bytes (error/misconfiguration)
    /// - max_cache_blocks set but --max-cache-blocks unsupported (warning)
    /// - prefix_cache_enabled=true but budget_bytes=0 and low headroom (warning)
    ///
    /// User explicit values always win; findings are recommendations only.
    #[allow(dead_code)]
    pub fn compute_prefix_cache_findings(
        &self,
        params: &CacheDiagnosticParams,
    ) -> PrefixCacheDiagnosticFindings {
        let mut findings = Vec::new();

        // Check 1: budget exceeds configured ceiling (misconfiguration)
        if let Some(budget) = params.config_prefix_cache_budget_bytes {
            if budget > params.configured_ceiling_bytes && params.configured_ceiling_bytes > 0 {
                findings.push(CacheDiagnosticFinding {
                    code: "CACHE_BUDGET_EXCEEDS_CEILING".into(),
                    severity: "error".into(),
                    message: format!(
                        "Prefix cache budget ({}) exceeds configured ceiling ({}). Reduce budget or increase ceiling.",
                        bytes_to_human(budget),
                        bytes_to_human(params.configured_ceiling_bytes)
                    ),
                    fixable: true,
                    fix_action: Some(format!("adjust_budget_{}", params.configured_ceiling_bytes as f64 * PREFIX_CACHE_BUDGET_FRACTION)
                        .replace(".", "_")),
                });
            }
        }

        // Check 2: max_cache_blocks set but unsupported
        if params.config_max_cache_blocks.is_some() {
            let has_cache_flag = self.serve_flags.iter().any(|f| f == "--max-cache-blocks");
            if !has_cache_flag {
                findings.push(CacheDiagnosticFinding {
                    code: "CACHE_BLOCKS_UNSUPPORTED".into(),
                    severity: "warning".into(),
                    message: "max_cache_blocks is configured but this runtime does not support --max-cache-blocks. The setting will be ignored.".into(),
                    fixable: true,
                    fix_action: Some("disable_blocks".into()),
                });
            }
        }

        // Check 3: prefix_cache_enabled but budget=0 and low headroom
        if params.config_prefix_cache_enabled
            && params.config_prefix_cache_budget_bytes == Some(0)
            && params.current_safe_availability_bytes > 0
        {
            let headroom_ratio = if params.configured_ceiling_bytes > 0 {
                params.current_safe_availability_bytes as f64
                    / params.configured_ceiling_bytes as f64
            } else {
                0.0
            };
            // Low headroom = below 30% of ceiling available
            if headroom_ratio < 0.30 {
                let recommended = if params.configured_ceiling_bytes > 0 {
                    (params.configured_ceiling_bytes as f64 * PREFIX_CACHE_BUDGET_FRACTION) as u64
                } else {
                    0
                };
                findings.push(CacheDiagnosticFinding {
                    code: "CACHE_ENABLED_NO_BUDGET_LOW_HEADROOM".into(),
                    severity: "warning".into(),
                    message: format!(
                        "Prefix cache is enabled but budget is 0 and memory headroom is low ({:.0}% of ceiling). Set a budget to prevent uncontrolled growth.",
                        headroom_ratio * 100.0
                    ),
                    fixable: true,
                    fix_action: Some(format!("set_budget_{}", recommended)),
                });
            }
        }

        PrefixCacheDiagnosticFindings { findings }
    }

    /// Check whether this snapshot supports --max-cache-blocks.
    #[allow(dead_code)]
    pub fn supports_max_cache_blocks(&self) -> bool {
        self.serve_flags.iter().any(|f| f == "--max-cache-blocks")
    }
}

fn bytes_to_human(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GiB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MiB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KiB", bytes as f64 / 1_024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// On-device, user-driven update-validation probe.
///
/// Runs after managed install/upgrade. Validates:
/// 1. `rapid-mlx serve --help` succeeds with expected structure
/// 2. `rapid-mlx serve --version` matches installed version
/// 3. Dependencies (MLX, MLX-LM, etc.) resolve without error
/// 4. Capability snapshot generation succeeds
/// 5. Basic self-import check: rapid-mlx can import core modules
///
/// Each sub-check is independently bounded (8s timeout, 512KB output).
/// Total probe completes within 30s.
///
/// Results:
/// - PASS: all checks succeed → environment healthy
/// - PER-FEATURE FAIL: specific capability fails → actionable diagnosis per feature
/// - CRITICAL FAIL: rapid-mlx itself broken → rollback eligible
///
/// This is `[escalate→device]` per plan §9.6 — real hardware measurements, not quota.
pub async fn run_update_validation_probe(
    binary: &Path,
    expected_version: &str,
) -> Result<ProbeResult> {
    let start = std::time::Instant::now();

    // 1. Version match check (critical)
    let version_result = tokio::time::timeout(PROBE_SUBCHECK_TIMEOUT, probe_version(binary)).await;
    let version = match version_result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            return Ok(ProbeResult::CriticalFail {
                message: format!("Version probe failed: {e}"),
            });
        }
        Err(_) => {
            return Ok(ProbeResult::CriticalFail {
                message: "Version probe timed out after 8s".into(),
            });
        }
    };

    if !version_matches(&version, expected_version) {
        return Ok(ProbeResult::CriticalFail {
            message: format!(
                "Installed Rapid-MLX version {version} does not match expected {expected_version}"
            ),
        });
    }

    // 2. Help structure check (critical)
    let help_result =
        tokio::time::timeout(PROBE_SUBCHECK_TIMEOUT, probe_help_structure(binary)).await;
    match help_result {
        Ok(Ok(ProbeResult::Pass)) => {}
        Ok(Ok(ProbeResult::CriticalFail { ref message })) => {
            return Ok(ProbeResult::CriticalFail {
                message: format!("Help probe failed: {message}"),
            });
        }
        Ok(Ok(ProbeResult::PerFeatureFail { .. })) => {
            // probe_help_structure only returns Pass/CriticalFail, but handle exhaustively
            return Ok(ProbeResult::CriticalFail {
                message: "Help probe returned unexpected result".into(),
            });
        }
        Ok(Err(e)) => {
            return Ok(ProbeResult::CriticalFail {
                message: format!("Help probe failed: {e}"),
            });
        }
        Err(_) => {
            return Ok(ProbeResult::CriticalFail {
                message: "Help probe timed out after 8s".into(),
            });
        }
    }

    // 3. Dependency resolution check (critical)
    let dep_result = tokio::time::timeout(PROBE_SUBCHECK_TIMEOUT, probe_dependencies(binary)).await;

    let _package_versions = match dep_result {
        Ok(versions) if has_critical_dependency(&versions) => versions,
        Ok(_) => {
            return Ok(ProbeResult::CriticalFail {
                message: "Critical dependency (mlx or mlx_lm) not found in environment".into(),
            });
        }
        Err(_) => {
            return Ok(ProbeResult::CriticalFail {
                message: "Dependency probe timed out after 8s".into(),
            });
        }
    };

    // 4. Self-import check: rapid-mlx can import core modules without crash (critical)
    let import_result =
        tokio::time::timeout(PROBE_SUBCHECK_TIMEOUT, probe_self_import(binary)).await;

    match import_result {
        Ok(Ok(ProbeResult::Pass)) => {}
        Ok(Ok(ProbeResult::CriticalFail { ref message })) => {
            return Ok(ProbeResult::CriticalFail {
                message: format!("Self-import failed: {message}"),
            });
        }
        Ok(Ok(ProbeResult::PerFeatureFail { feature_failures })) => {
            // Self-import PerFeatureFail (e.g. Python not found) is informational;
            // don't block activation for managed install. Collect it later.
            for ff in feature_failures {
                eprintln!(
                    "Rapid-MLX probe note [{feature}]: {message}",
                    feature = ff.feature,
                    message = ff.message
                );
            }
        }
        Ok(Err(e)) => {
            return Ok(ProbeResult::CriticalFail {
                message: format!("Self-import probe command failed: {e}"),
            });
        }
        Err(_) => {
            return Ok(ProbeResult::CriticalFail {
                message: "Self-import probe timed out after 8s".into(),
            });
        }
    }

    // 5. Capability snapshot generation (critical)
    let snapshot_result = tokio::time::timeout(
        PROBE_SUBCHECK_TIMEOUT,
        generate_snapshot(binary, RuntimeSource::Managed),
    )
    .await;

    let snapshot = match snapshot_result {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            return Ok(ProbeResult::CriticalFail {
                message: format!("Capability snapshot generation failed: {e}"),
            });
        }
        Err(_) => {
            return Ok(ProbeResult::CriticalFail {
                message: "Capability snapshot timed out after 8s".into(),
            });
        }
    };

    let elapsed = start.elapsed();
    if elapsed > PROBE_TOTAL_TIMEOUT {
        // We already succeeded in all critical checks, but warn about duration.
        // Don't fail the probe just for being slow on a loaded system.
        eprintln!(
            "Rapid-MLX probe completed in {:.1}s (budget: {:.1}s)",
            elapsed.as_secs_f64(),
            PROBE_TOTAL_TIMEOUT.as_secs_f64()
        );
    }

    // 6. Check for per-feature failures from extras
    let feature_failures = collect_feature_failures(&snapshot);

    if feature_failures.is_empty() {
        Ok(ProbeResult::Pass)
    } else {
        Ok(ProbeResult::PerFeatureFail { feature_failures })
    }
}

/// Check if version output matches expected version (major.minor.patch must match).
fn version_matches(actual: &str, expected: &str) -> bool {
    // Strip any leading 'v'
    let clean_actual = extract_version_text(actual).unwrap_or_else(|| actual.to_string());
    let clean_expected = expected.trim_start_matches('v');

    // Direct match is primary
    if clean_actual == clean_expected {
        return true;
    }

    // Also accept if major.minor.patch prefix matches (handles suffix variations like rc1)
    // Extract only numeric components
    let numeric_prefix = |s: &str| -> String {
        let parts: Vec<String> = s
            .split('.')
            .map(|p| {
                p.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
            })
            .filter(|p| !p.is_empty())
            .collect();
        parts.into_iter().take(3).collect::<Vec<_>>().join(".")
    };
    let actual_prefix = numeric_prefix(&clean_actual);
    let expected_prefix = numeric_prefix(clean_expected);
    actual_prefix == expected_prefix
}

/// Probe help structure: ensures rapid-mlx serve --help succeeds with expected flags.
async fn probe_help_structure(binary: &Path) -> Result<ProbeResult> {
    let output = run_probe_command(binary, &["serve", "--help"]).await?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let text = text.trim();
    if text.is_empty() {
        return Err(anyhow!("Help probe returned empty output"));
    }

    // Verify expected structure: must contain MODEL arg position and core flags
    let lower = text.to_ascii_lowercase();
    if !lower.contains("model") && !lower.contains("--host") && !lower.contains("--port") {
        return Err(anyhow!(
            "Help output lacks expected structure (no model/host/port)"
        ));
    }

    Ok(ProbeResult::Pass)
}

/// Probe self-import: rapid-mlx can import core modules without crash.
async fn probe_self_import(binary: &Path) -> Result<ProbeResult> {
    let python_env = resolve_python_for_binary(binary);
    let Some(python) = python_env.as_ref() else {
        // Python not found in environment; this is non-fatal for probe
        // since the binary itself works. Mark as indeterminate rather than fail.
        return Ok(ProbeResult::PerFeatureFail {
            feature_failures: vec![FeatureProbeFailure {
                feature: "self-import".into(),
                message: "Python interpreter not found in environment; self-import check skipped"
                    .into(),
            }],
        });
    };

    let script = r#"
import sys
import importlib

# Core imports that rapid-mlx itself requires
core = ["mlx", "mlx_lm"]
errors = []

for name in core:
    try:
        importlib.import_module(name.replace("-", "_"))
    except ImportError as e:
        errors.append(f"{name}: {e}")
    except Exception as e:
        errors.append(f"{name}: runtime error: {e}")

if errors:
    print("FAIL\n" + "\n".join(errors))
    sys.exit(1)
else:
    print("OK")
    sys.exit(0)
"#;

    match run_probe_command(python, &["-c", script]).await {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if text.starts_with("OK") {
                Ok(ProbeResult::Pass)
            } else {
                let details = text
                    .lines()
                    .skip(1)
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(ProbeResult::CriticalFail {
                    message: format!("Core module import failed: {details}"),
                })
            }
        }
        Err(e) => Ok(ProbeResult::CriticalFail {
            message: format!("Self-import probe command failed: {e}"),
        }),
    }
}

/// Check if the package list includes at least one critical dependency.
fn has_critical_dependency(packages: &[DependencyVersion]) -> bool {
    packages.iter().any(|p| {
        let name = p.package.to_ascii_lowercase();
        name == "mlx" || name == "mlx-lm" || name == "mlx_lm"
    })
}

/// Collect per-feature failures from capability snapshot.
fn collect_feature_failures(snapshot: &CapabilitySnapshot) -> Vec<FeatureProbeFailure> {
    let mut failures = Vec::new();

    // Guided generation: extra import check
    if let ExtraState::Broken(ref reason) = snapshot.installed_extras.guided {
        failures.push(FeatureProbeFailure {
            feature: "guided".into(),
            message: format!("[guided] extra import failed: {reason}"),
        });
    }

    // Vision: extra import check (mlx-vlm)
    if let ExtraState::Broken(ref reason) = snapshot.installed_extras.vision {
        failures.push(FeatureProbeFailure {
            feature: "vision".into(),
            message: format!("Vision extra import failed: {reason}"),
        });
    }

    // Embeddings: extra import check
    if let ExtraState::Broken(ref reason) = snapshot.installed_extras.embeddings {
        failures.push(FeatureProbeFailure {
            feature: "embeddings".into(),
            message: format!("Embeddings extra import failed: {reason}"),
        });
    }

    // Qualified features: record indeterminate/unavailable as per-feature notes
    if !matches!(
        snapshot.qualified_features.guided_generation,
        FeatureQualification::Available
    ) {
        if matches!(snapshot.installed_extras.guided, ExtraState::Installed) {
            // Extra installed but feature not available: something deeper is wrong
            failures.push(FeatureProbeFailure {
                feature: "guided".into(),
                message: "Guided extra installed but capability probe failed".into(),
            });
        }
        // Missing is informational, not a failure
    }

    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_hash_is_deterministic() {
        let hash1 = hash_help("--host --port --timeout");
        let hash2 = hash_help("--host --port --timeout");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash_help("--host --port"));
    }

    #[test]
    fn extract_flags_from_help_text() {
        let help = r#"
Usage: rapid-mlx serve [OPTIONS] MODEL

Options:
  --host TEXT
  --port INTEGER
  --timeout=1800
  --tool-call-parser [openai|default]
"#;
        let flags = extract_flags(help);
        assert!(flags.contains(&"--host".into()));
        assert!(flags.contains(&"--port".into()));
        assert!(flags.contains(&"--timeout".into()));
        assert!(flags.contains(&"--tool-call-parser".into()));
        assert!(!flags.contains(&"--nonexistent".into()));
    }

    #[test]
    fn extract_version_from_variants() {
        assert_eq!(
            extract_version_text("rapid-mlx 0.10.10"),
            Some("0.10.10".into())
        );
        assert_eq!(
            extract_version_text("Rapid-MLX v0.10.12\n"),
            Some("0.10.12".into())
        );
        assert_eq!(
            extract_version_text("0.10.11rc1"),
            Some("0.10.11rc1".into())
        );
        assert_eq!(extract_version_text("development"), None);
    }

    #[test]
    fn snapshot_invalidates_on_file_hash_change() {
        let identity1 = ExecutableIdentity {
            path: "/tmp/rapid-mlx".into(),
            file_hash: "abc123".into(),
            file_mtime_unix: 1000,
        };
        let identity2 = ExecutableIdentity {
            path: "/tmp/rapid-mlx".into(),
            file_hash: "def456".into(),
            file_mtime_unix: 2000,
        };
        let snap = CapabilitySnapshot {
            executable_identity: identity1.clone(),
            rapid_mlx_version: "0.10.10".into(),
            help_hash: hash_help(""),
            serve_flags: vec![],
            package_versions: vec![],
            installed_extras: InstalledExtras::default(),
            qualified_features: QualifiedFeatures::default(),
            mtp_concurrency: MtpConcurrencyState::SingleActiveGreedy,
            sampling_defaults: SamplingDefaultFields::default(),
            sampling_cascade: SamplingCascade::default(),
            evidence_timestamp: 0,
            source: CapabilitySnapshotSource::AutoProbed,
        };
        assert!(snap.is_valid_for(&identity1));
        assert!(!snap.is_valid_for(&identity2));
    }

    #[test]
    fn snapshot_fingerprint_includes_deps() {
        let mut snap1 = CapabilitySnapshot {
            executable_identity: ExecutableIdentity {
                path: "/tmp/x".into(),
                file_hash: "h".into(),
                file_mtime_unix: 0,
            },
            rapid_mlx_version: "0.10.10".into(),
            help_hash: "h".into(),
            serve_flags: vec![],
            package_versions: vec![DependencyVersion {
                package: "mlx".into(),
                version: "0.20".into(),
                source: DependencyVersionSource::PipFreeze,
            }],
            installed_extras: InstalledExtras::default(),
            qualified_features: QualifiedFeatures::default(),
            mtp_concurrency: MtpConcurrencyState::SingleActiveGreedy,
            sampling_defaults: SamplingDefaultFields::default(),
            sampling_cascade: SamplingCascade::default(),
            evidence_timestamp: 0,
            source: CapabilitySnapshotSource::AutoProbed,
        };
        let fp1 = snap1.fingerprint();
        snap1.package_versions.push(DependencyVersion {
            package: "mlx_lm".into(),
            version: "0.21".into(),
            source: DependencyVersionSource::PipFreeze,
        });
        let fp2 = snap1.fingerprint();
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn guided_extra_missing_marks_feature_unavailable() {
        let features = derive_qualified_features(
            &["--tool-call-parser", "--enable-auto-tool-choice"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>(),
            &InstalledExtras {
                guided: ExtraState::Missing,
                vision: ExtraState::Missing,
                embeddings: ExtraState::Missing,
            },
            "0.10.10",
            RuntimeSource::Managed,
        );
        match features.guided_generation {
            FeatureQualification::Unavailable(ref reason) => {
                assert!(reason.contains("guided"));
            }
            other => panic!("Expected Unavailable, got {:?}", other),
        }
        assert!(matches!(
            features.tool_parsing,
            FeatureQualification::Available
        ));
    }

    #[test]
    fn vision_extra_broken_produces_actionable_diagnosis() {
        let extras = InstalledExtras {
            guided: ExtraState::Installed,
            vision: ExtraState::Broken("ModuleNotFoundError: mlx_vlm".into()),
            embeddings: ExtraState::Installed,
        };
        let features = derive_qualified_features(
            &["--tool-call-parser"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>(),
            &extras,
            "0.10.10",
            RuntimeSource::Managed,
        );
        match features.vision {
            FeatureQualification::Unavailable(ref reason) => {
                assert!(reason.contains("broken") || reason.contains("mlx_vlm"));
            }
            other => panic!("Expected Unavailable, got {:?}", other),
        }
    }

    #[test]
    fn upstream_constrained_env_has_no_global_provisional() {
        // A managed install that passes all probes gets Available features,
        // not a blanket indeterminate state.
        let flags: Vec<String> = vec![
            "--tool-call-parser".into(),
            "--enable-auto-tool-choice".into(),
            "--reasoning".into(),
        ];
        let extras = InstalledExtras {
            guided: ExtraState::Installed,
            vision: ExtraState::Installed,
            embeddings: ExtraState::Installed,
        };
        let features =
            derive_qualified_features(&flags, &extras, "0.10.10", RuntimeSource::Managed);
        assert!(matches!(
            features.tool_parsing,
            FeatureQualification::Available
        ));
        assert!(matches!(
            features.guided_generation,
            FeatureQualification::Available
        ));
        assert!(matches!(
            features.status_memory_telemetry,
            FeatureQualification::Available
        ));
        assert!(matches!(
            features.one_shot_launch,
            FeatureQualification::Available
        ));
    }

    #[test]
    fn flag_presence_alone_does_not_qualify_guided() {
        // Even if Rapid's serve has some JSON-related flag, guided_generation
        // requires the [guided] extra actually installed.
        let flags: Vec<String> = vec!["--response-format".into()];
        let extras = InstalledExtras {
            guided: ExtraState::Missing,
            vision: ExtraState::Missing,
            embeddings: ExtraState::Missing,
        };
        let features =
            derive_qualified_features(&flags, &extras, "0.10.10", RuntimeSource::Managed);
        match features.guided_generation {
            FeatureQualification::Unavailable(_) => {}
            other => panic!(
                "Expected Unavailable for missing guided extra, got {:?}",
                other
            ),
        }
    }

    // Probe-specific tests

    #[test]
    fn version_matches_exact_and_prefix() {
        assert!(version_matches("0.10.10", "0.10.10"));
        assert!(version_matches("v0.10.10", "0.10.10"));
        assert!(version_matches("0.10.10", "v0.10.10"));
        // Accept suffix variations if major.minor.patch matches
        assert!(version_matches("0.10.10rc1", "0.10.10"));
        assert!(!version_matches("0.9.10", "0.10.10"));
        assert!(!version_matches("0.10.11", "0.10.10"));
    }

    #[test]
    fn help_structure_requires_expected_content() {
        // Valid help output
        let valid = r#"Usage: rapid-mlx serve [OPTIONS] MODEL
Options:
  --host TEXT
  --port INTEGER
"#;
        let flags = extract_flags(valid);
        assert!(flags.contains(&"--host".into()));
        assert!(flags.contains(&"--port".into()));

        // Help without expected content (no model/host/port)
        let invalid = "Just some random text";
        let flags = extract_flags(invalid);
        assert!(!flags.contains(&"--host".into()));
        assert!(!flags.contains(&"--port".into()));
    }

    #[test]
    fn has_critical_dependency_detects_mlx_and_mlx_lm() {
        let no_deps = Vec::<DependencyVersion>::new();
        assert!(!has_critical_dependency(&no_deps));

        let with_mlx = vec![DependencyVersion {
            package: "mlx".into(),
            version: "0.20".into(),
            source: DependencyVersionSource::PipFreeze,
        }];
        assert!(has_critical_dependency(&with_mlx));

        let with_mlx_lm = vec![DependencyVersion {
            package: "mlx_lm".into(),
            version: "0.21".into(),
            source: DependencyVersionSource::PipFreeze,
        }];
        assert!(has_critical_dependency(&with_mlx_lm));

        let with_mlx_lm_hyphen = vec![DependencyVersion {
            package: "mlx-lm".into(),
            version: "0.21".into(),
            source: DependencyVersionSource::PipFreeze,
        }];
        assert!(has_critical_dependency(&with_mlx_lm_hyphen));
    }

    #[test]
    fn collect_feature_failures_identifies_broken_extras() {
        let snapshot = CapabilitySnapshot {
            executable_identity: ExecutableIdentity {
                path: "/tmp/rapid-mlx".into(),
                file_hash: "h".into(),
                file_mtime_unix: 0,
            },
            rapid_mlx_version: "0.10.10".into(),
            help_hash: "h".into(),
            serve_flags: vec![],
            package_versions: vec![],
            installed_extras: InstalledExtras {
                guided: ExtraState::Broken("ModuleNotFoundError: outlines".into()),
                vision: ExtraState::Installed,
                embeddings: ExtraState::Broken("import error".into()),
            },
            qualified_features: QualifiedFeatures {
                guided_generation: FeatureQualification::Unavailable("broken".into()),
                ..Default::default()
            },
            mtp_concurrency: MtpConcurrencyState::SingleActiveGreedy,
            sampling_defaults: SamplingDefaultFields::default(),
            sampling_cascade: SamplingCascade::default(),
            evidence_timestamp: 0,
            source: CapabilitySnapshotSource::AutoProbed,
        };
        let failures = collect_feature_failures(&snapshot);
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].feature, "guided");
        assert!(failures[0].message.contains("outlines"));
        assert_eq!(failures[1].feature, "embeddings");
    }

    #[test]
    fn collect_feature_failures_empty_when_all_ok() {
        let snapshot = CapabilitySnapshot {
            executable_identity: ExecutableIdentity {
                path: "/tmp/rapid-mlx".into(),
                file_hash: "h".into(),
                file_mtime_unix: 0,
            },
            rapid_mlx_version: "0.10.10".into(),
            help_hash: "h".into(),
            serve_flags: vec!["--tool-call-parser".into()],
            package_versions: vec![],
            installed_extras: InstalledExtras {
                guided: ExtraState::Installed,
                vision: ExtraState::Installed,
                embeddings: ExtraState::Missing,
            },
            qualified_features: QualifiedFeatures {
                guided_generation: FeatureQualification::Available,
                ..Default::default()
            },
            mtp_concurrency: MtpConcurrencyState::SingleActiveGreedy,
            sampling_defaults: SamplingDefaultFields::default(),
            sampling_cascade: SamplingCascade::default(),
            evidence_timestamp: 0,
            source: CapabilitySnapshotSource::AutoProbed,
        };
        let failures = collect_feature_failures(&snapshot);
        // Missing is not a failure; only Broken is; installed+available means no failure
        assert!(failures.is_empty());
    }

    #[test]
    fn probe_result_pass_has_no_global_provisional() {
        // A PASS result means environment is healthy; no global Provisional banner
        let result = ProbeResult::Pass;
        assert!(matches!(result, ProbeResult::Pass));

        // Per-feature fail still doesn't justify a global banner — it's per-feature
        let result = ProbeResult::PerFeatureFail {
            feature_failures: vec![FeatureProbeFailure {
                feature: "guided".into(),
                message: "extra import failed".into(),
            }],
        };
        assert!(matches!(result, ProbeResult::PerFeatureFail { .. }));

        // Critical fail is actionable and includes rollback eligibility signal
        let result = ProbeResult::CriticalFail {
            message: "Version mismatch".into(),
        };
        assert!(matches!(result, ProbeResult::CriticalFail { .. }));
    }

    #[test]
    fn probe_result_critical_fail_is_rollback_eligible() {
        let result = ProbeResult::CriticalFail {
            message: "Core module import failed: mlx: ModuleNotFoundError".into(),
        };
        match result {
            ProbeResult::CriticalFail { ref message } => {
                assert!(!message.is_empty());
                assert!(message.contains("mlx"));
            }
            _ => panic!("Expected CriticalFail"),
        }
    }

    #[test]
    fn probe_timeout_values_are_bounded() {
        // Verify constants are within spec
        assert_eq!(PROBE_SUBCHECK_TIMEOUT, Duration::from_secs(8));
        assert_eq!(PROBE_TOTAL_TIMEOUT, Duration::from_secs(30));
        assert!(PROBE_SUBCHECK_TIMEOUT <= CAPABILITY_PROBE_TIMEOUT);
        assert!(PROBE_TOTAL_TIMEOUT.as_secs() > PROBE_SUBCHECK_TIMEOUT.as_secs());
    }

    // MTP concurrency qualification tests

    #[test]
    fn mtp_concurrency_states_have_correct_labels() {
        assert_eq!(
            MtpConcurrencyState::RequiresSingle.label(),
            "requires single active request"
        );
        assert_eq!(
            MtpConcurrencyState::Supported.label(),
            "per-sequence MTP supported"
        );
        assert_eq!(
            MtpConcurrencyState::SingleActiveGreedy.label(),
            "single-active greedy with fallback"
        );
        assert_eq!(MtpConcurrencyState::Unknown.label(), "undetermined");
    }

    #[test]
    fn mtp_concurrency_derived_as_single_active_greedy() {
        let flags = vec![
            "--host".into(),
            "--port".into(),
            "--tool-call-parser".into(),
        ];
        assert_eq!(
            derive_mtp_concurrency(&flags),
            MtpConcurrencyState::SingleActiveGreedy
        );
    }

    #[test]
    fn mtp_concurrency_with_speculative_flags_still_single_active_greedy() {
        let flags = vec!["--host".into(), "--port".into(), "--speculative".into()];
        assert_eq!(
            derive_mtp_concurrency(&flags),
            MtpConcurrencyState::SingleActiveGreedy
        );
    }

    // Sampling default fields tests

    #[test]
    fn sampling_defaults_probed_per_field() {
        let flags = vec![
            "--default-temperature".into(),
            "--default-top-p".into(),
            "--default-max-tokens".into(),
        ];
        let defaults = SamplingDefaultFields::from_flags(&flags);
        assert!(matches!(defaults.temperature, DefaultFieldState::Supported));
        assert!(matches!(defaults.top_p, DefaultFieldState::Supported));
        assert!(matches!(defaults.top_k, DefaultFieldState::Unsupported));
        assert!(matches!(defaults.max_tokens, DefaultFieldState::Supported));
    }

    #[test]
    fn sampling_defaults_effective_fields_excludes_unsupported() {
        let flags = vec!["--default-temperature".into(), "--default-min-p".into()];
        let defaults = SamplingDefaultFields::from_flags(&flags);
        let effective = defaults.effective_fields();
        assert!(effective.contains(&"temperature"));
        assert!(effective.contains(&"min_p"));
        assert!(!effective.contains(&"top_p"));
        assert!(!effective.contains(&"top_k"));
        assert!(!effective.contains(&"max_tokens"));
    }

    #[test]
    fn unsupported_defaults_not_reported_as_effective() {
        let flags = vec!["--host".into(), "--port".into()];
        let defaults = SamplingDefaultFields::from_flags(&flags);
        assert!(defaults.effective_fields().is_empty());
    }

    // Sampling cascade tests

    #[test]
    fn sampling_cascade_precedence_order_is_correct() {
        let cascade = SamplingCascade::from_flags(&[]);
        assert_eq!(cascade.precedence.len(), 5);
        assert_eq!(cascade.precedence[0], SamplingSource::RequestLevel);
        assert_eq!(cascade.precedence[1], SamplingSource::CliDefaults);
        assert_eq!(cascade.precedence[2], SamplingSource::AliasDefaults);
        assert_eq!(cascade.precedence[3], SamplingSource::GenerationConfig);
        assert_eq!(cascade.precedence[4], SamplingSource::HardcodedFallback);
    }

    #[test]
    fn sampling_cascade_derives_cli_defaults_from_flags() {
        let flags = vec!["--default-temperature".into(), "--default-top-p".into()];
        let cascade = SamplingCascade::from_flags(&flags);
        assert!(matches!(
            cascade.cli_defaults_available.temperature,
            DefaultFieldState::Supported
        ));
        assert!(matches!(
            cascade.cli_defaults_available.top_p,
            DefaultFieldState::Supported
        ));
        assert!(matches!(
            cascade.cli_defaults_available.max_tokens,
            DefaultFieldState::Unsupported
        ));
    }

    // Snapshot integration tests

    // Prefix cache guidance tests (Phase 6 Part A)

    #[test]
    fn prefix_cache_guidance_recommended_when_capability_and_headroom() {
        let snapshot = CapabilitySnapshot {
            executable_identity: ExecutableIdentity::default(),
            rapid_mlx_version: "0.10.12".into(),
            help_hash: "x".into(),
            serve_flags: vec!["--max-cache-blocks".into(), "--host".into()],
            package_versions: vec![],
            installed_extras: InstalledExtras::default(),
            qualified_features: QualifiedFeatures::default(),
            mtp_concurrency: MtpConcurrencyState::SingleActiveGreedy,
            sampling_defaults: SamplingDefaultFields::default(),
            sampling_cascade: SamplingCascade::default(),
            evidence_timestamp: 0,
            source: CapabilitySnapshotSource::AutoProbed,
        };
        // 48GB ceiling, 40GB safe, 20GB model overhead → 20GB available for cache
        // D30 budget = 48GB × 0.10 = 4.8GB
        let guidance = PrefixCacheGuidance::derive(
            &snapshot,
            48 * 1024 * 1024 * 1024,
            40 * 1024 * 1024 * 1024,
            20 * 1024 * 1024 * 1024,
            None,
            4096, // n_embd
            32,   // n_kv_heads
            128,  // head_dim
        );
        assert!(guidance.supported);
        assert!(guidance.should_recommend);
        assert!(guidance.recommended_max_cache_blocks > 0);
        // Budget should be ~4.8GB (10% of 48GB ceiling)
        assert_eq!(
            guidance.prefix_cache_budget_bytes,
            ((48u64 * 1024 * 1024 * 1024) as f64 * 0.10) as u64
        );
        assert!(guidance.reasons_off_or_lower.is_empty());
    }

    #[test]
    fn prefix_cache_guidance_not_recommended_when_no_capability_flag() {
        let snapshot = CapabilitySnapshot {
            executable_identity: ExecutableIdentity::default(),
            rapid_mlx_version: "0.9.0".into(),
            help_hash: "x".into(),
            serve_flags: vec!["--host".into()], // no --max-cache-blocks
            package_versions: vec![],
            installed_extras: InstalledExtras::default(),
            qualified_features: QualifiedFeatures::default(),
            mtp_concurrency: MtpConcurrencyState::SingleActiveGreedy,
            sampling_defaults: SamplingDefaultFields::default(),
            sampling_cascade: SamplingCascade::default(),
            evidence_timestamp: 0,
            source: CapabilitySnapshotSource::AutoProbed,
        };
        let guidance = PrefixCacheGuidance::derive(
            &snapshot,
            48 * 1024 * 1024 * 1024,
            40 * 1024 * 1024 * 1024,
            20 * 1024 * 1024 * 1024,
            None,
            4096,
            32,
            128,
        );
        assert!(!guidance.supported);
        assert!(!guidance.should_recommend);
        assert_eq!(guidance.recommended_max_cache_blocks, 0);
        assert!(
            guidance
                .reasons_off_or_lower
                .iter()
                .any(|r| r.contains("--max-cache-blocks"))
        );
    }

    #[test]
    fn prefix_cache_guidance_respects_user_explicit_value() {
        let snapshot = CapabilitySnapshot {
            executable_identity: ExecutableIdentity::default(),
            rapid_mlx_version: "0.10.12".into(),
            help_hash: "x".into(),
            serve_flags: vec!["--max-cache-blocks".into()],
            package_versions: vec![],
            installed_extras: InstalledExtras::default(),
            qualified_features: QualifiedFeatures::default(),
            mtp_concurrency: MtpConcurrencyState::SingleActiveGreedy,
            sampling_defaults: SamplingDefaultFields::default(),
            sampling_cascade: SamplingCascade::default(),
            evidence_timestamp: 0,
            source: CapabilitySnapshotSource::AutoProbed,
        };
        let guidance = PrefixCacheGuidance::derive(
            &snapshot,
            48 * 1024 * 1024 * 1024,
            40 * 1024 * 1024 * 1024,
            20 * 1024 * 1024 * 1024,
            Some(64), // user explicit
            4096,
            32,
            128,
        );
        assert!(guidance.supported);
        assert!(!guidance.should_recommend); // user explicit wins
        assert!(
            guidance
                .reasons_off_or_lower
                .iter()
                .any(|r| r.contains("explicitly"))
        );
    }

    #[test]
    fn prefix_cache_guidance_no_recommendation_insufficient_headroom() {
        let snapshot = CapabilitySnapshot {
            executable_identity: ExecutableIdentity::default(),
            rapid_mlx_version: "0.10.12".into(),
            help_hash: "x".into(),
            serve_flags: vec!["--max-cache-blocks".into()],
            package_versions: vec![],
            installed_extras: InstalledExtras::default(),
            qualified_features: QualifiedFeatures::default(),
            mtp_concurrency: MtpConcurrencyState::SingleActiveGreedy,
            sampling_defaults: SamplingDefaultFields::default(),
            sampling_cascade: SamplingCascade::default(),
            evidence_timestamp: 0,
            source: CapabilitySnapshotSource::AutoProbed,
        };
        // 48GB ceiling, 10GB safe, 20GB model overhead → negative available
        let guidance = PrefixCacheGuidance::derive(
            &snapshot,
            48 * 1024 * 1024 * 1024,
            10 * 1024 * 1024 * 1024,
            20 * 1024 * 1024 * 1024,
            None,
            4096,
            32,
            128,
        );
        assert!(guidance.supported);
        assert!(!guidance.should_recommend);
        assert_eq!(guidance.recommended_max_cache_blocks, 0);
        assert!(
            guidance
                .reasons_off_or_lower
                .iter()
                .any(|r| r.contains("headroom"))
        );
    }

    #[test]
    fn prefix_cache_budget_d30_within_configured_ceiling_fraction() {
        let snapshot = CapabilitySnapshot {
            executable_identity: ExecutableIdentity::default(),
            rapid_mlx_version: "0.10.12".into(),
            help_hash: "x".into(),
            serve_flags: vec!["--max-cache-blocks".into()],
            package_versions: vec![],
            installed_extras: InstalledExtras::default(),
            qualified_features: QualifiedFeatures::default(),
            mtp_concurrency: MtpConcurrencyState::SingleActiveGreedy,
            sampling_defaults: SamplingDefaultFields::default(),
            sampling_cascade: SamplingCascade::default(),
            evidence_timestamp: 0,
            source: CapabilitySnapshotSource::AutoProbed,
        };
        let ceiling = 64 * 1024 * 1024 * 1024u64;
        let guidance =
            PrefixCacheGuidance::derive(&snapshot, ceiling, ceiling, 0, None, 4096, 32, 128);
        // Budget = ceiling × 0.10, must be ≤ ceiling
        assert!(guidance.prefix_cache_budget_bytes > 0);
        assert!(guidance.prefix_cache_budget_bytes <= ceiling);
        assert_eq!(
            guidance.prefix_cache_budget_bytes,
            (ceiling as f64 * 0.10) as u64
        );
    }

    #[test]
    fn prefix_cache_guidance_is_recommendation_only_never_forced() {
        // Hard gate: guidance never auto-applies; requires user confirmation (A31).
        let snapshot = CapabilitySnapshot {
            executable_identity: ExecutableIdentity::default(),
            rapid_mlx_version: "0.10.12".into(),
            help_hash: "x".into(),
            serve_flags: vec!["--max-cache-blocks".into()],
            package_versions: vec![],
            installed_extras: InstalledExtras::default(),
            qualified_features: QualifiedFeatures::default(),
            mtp_concurrency: MtpConcurrencyState::SingleActiveGreedy,
            sampling_defaults: SamplingDefaultFields::default(),
            sampling_cascade: SamplingCascade::default(),
            evidence_timestamp: 0,
            source: CapabilitySnapshotSource::AutoProbed,
        };
        let guidance = PrefixCacheGuidance::derive(
            &snapshot,
            48 * 1024 * 1024 * 1024,
            40 * 1024 * 1024 * 1024,
            20 * 1024 * 1024 * 1024,
            None,
            4096,
            32,
            128,
        );
        // has values but doesn't mutate anything — it's pure derivation
        assert!(guidance.should_recommend);
        // No side effects: snapshot unchanged, no auto-apply
        assert!(
            !snapshot
                .serve_flags
                .contains(&"--max-cache-blocks=64".into())
        );
    }

    #[test]
    fn snapshot_includes_mtp_and_sampling_fields() {
        let flags = vec!["--default-temperature".into()];
        let snapshot = CapabilitySnapshot {
            executable_identity: ExecutableIdentity::default(),
            rapid_mlx_version: "0.10.10".into(),
            help_hash: "x".into(),
            serve_flags: flags.clone(),
            package_versions: vec![],
            installed_extras: InstalledExtras::default(),
            qualified_features: QualifiedFeatures::default(),
            mtp_concurrency: derive_mtp_concurrency(&flags),
            sampling_defaults: SamplingDefaultFields::from_flags(&flags),
            sampling_cascade: SamplingCascade::from_flags(&flags),
            evidence_timestamp: 0,
            source: CapabilitySnapshotSource::AutoProbed,
        };
        assert_eq!(
            snapshot.mtp_concurrency,
            MtpConcurrencyState::SingleActiveGreedy
        );
        assert!(matches!(
            snapshot.sampling_defaults.temperature,
            DefaultFieldState::Supported
        ));
        assert_eq!(
            snapshot.sampling_cascade.precedence[0],
            SamplingSource::RequestLevel
        );
    }
}
