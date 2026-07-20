#![allow(clippy::collapsible_if)]

use crate::inference::rapid_mlx::runtime::RuntimeSource;
use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub const CAPABILITY_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);
pub const CAPABILITY_PROBE_MAX_OUTPUT: usize = 512 * 1024;

/// Source of a capability snapshot: automated discovery vs. manual override.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySnapshotSource {
    /// Automatically generated from live probing of this exact executable.
    AutoProbed,
    /// Manually overridden for a known-incompatible or known-safe runtime.
    ManualOverride,
}

/// Exact identity of the executable that a snapshot describes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CapabilitySnapshot {
    pub executable_identity: ExecutableIdentity,
    pub rapid_mlx_version: String,
    pub help_hash: String,
    pub serve_flags: Vec<String>,
    pub package_versions: Vec<DependencyVersion>,
    pub installed_extras: InstalledExtras,
    pub qualified_features: QualifiedFeatures,
    /// Timestamp when this snapshot was generated.
    pub evidence_timestamp: u64,
    pub source: CapabilitySnapshotSource,
}

impl CapabilitySnapshot {
    /// Check whether a stored snapshot is still valid for the given executable.
    pub fn is_valid_for(&self, current: &ExecutableIdentity) -> bool {
        self.executable_identity.path == current.path
            && self.executable_identity.file_hash == current.file_hash
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
            help_hash: "x".into(),
            serve_flags: vec![],
            package_versions: vec![],
            installed_extras: InstalledExtras::default(),
            qualified_features: QualifiedFeatures::default(),
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
}
