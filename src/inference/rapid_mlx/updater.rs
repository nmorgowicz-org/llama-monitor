use crate::config::harden_file_permissions;
use crate::inference::rapid_mlx::compatibility::{
    CompatibilityProfile, CompatibilityState, MINIMUM_VERIFIED_VERSION,
    probe_published_managed_release,
};
use crate::inference::rapid_mlx::runtime::RuntimeSource;
use anyhow::{Context, Result, anyhow, bail};
use rand::TryRng;
use rand::rngs::SysRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::fs;
use std::future::Future;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::{Semaphore, TryAcquireError};

const MANAGED_RELATIVE_ROOT: &str = "runtimes/rapid-mlx";
const ENVIRONMENTS_DIR: &str = "environments";
const POINTER_FILE: &str = "current.json";
const MANIFEST_FILE: &str = "manifest.json";
const COMPLETION_FILE: &str = ".complete";
const MANIFEST_SCHEMA_VERSION: u32 = 1;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MAX_COMMAND_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RuntimeInventoryEntry {
    pub environment_id: String,
    pub version: String,
    pub release_channel: ManagedReleaseChannel,
    pub executable_path: PathBuf,
    pub active: bool,
    pub rollback_candidate: bool,
    pub complete: bool,
}

impl Default for RuntimeInventoryEntry {
    fn default() -> Self {
        Self {
            environment_id: String::new(),
            version: String::new(),
            release_channel: ManagedReleaseChannel::Stable,
            executable_path: PathBuf::new(),
            active: false,
            rollback_candidate: false,
            complete: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct ManagedRuntimeStatus {
    pub supported: bool,
    pub installer_available: bool,
    pub mutation_in_progress: bool,
    pub active: Option<RuntimeInventoryEntry>,
    pub rollback_available: bool,
    pub inventory: Vec<RuntimeInventoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ExternalRuntimeStatus {
    pub source: RuntimeSource,
    pub version: String,
    pub executable_path: PathBuf,
    pub managed: bool,
    pub mutable: bool,
}

impl Default for ExternalRuntimeStatus {
    fn default() -> Self {
        Self {
            source: RuntimeSource::PathUnknown,
            version: String::new(),
            executable_path: PathBuf::new(),
            managed: false,
            mutable: false,
        }
    }
}

impl ExternalRuntimeStatus {
    /// Convert discovery output into report-only API data. No external source is mutable.
    pub fn report(source: RuntimeSource, version: impl Into<String>, path: PathBuf) -> Self {
        Self {
            source,
            version: version.into(),
            executable_path: path,
            managed: source == RuntimeSource::Managed,
            mutable: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct RuntimeMutationResult {
    pub active: RuntimeInventoryEntry,
    pub previous_environment_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagedReleaseChannel {
    Stable,
    Prerelease,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManagedReleaseSelection {
    version: String,
    channel: ManagedReleaseChannel,
    provenance: PublishedReleaseProvenance,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
struct PublishedReleaseProvenance;

impl ManagedReleaseSelection {
    /// Create an opaque selection from freshly rediscovered, immutable,
    /// non-draft release metadata. Request JSON cannot construct this type.
    pub(crate) fn from_published_release(
        version: impl Into<String>,
        channel: ManagedReleaseChannel,
    ) -> Result<Self> {
        let selection = Self {
            version: version.into(),
            channel,
            provenance: PublishedReleaseProvenance,
        };
        validate_release_selection(&selection)?;
        Ok(selection)
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn channel(&self) -> ManagedReleaseChannel {
        self.channel
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ActivePointer {
    schema_version: u32,
    active_environment_id: String,
    previous_environment_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct EnvironmentManifest {
    schema_version: u32,
    environment_id: String,
    version: String,
    binary_relative_path: String,
    binary_sha256: String,
    runtime_source: RuntimeSource,
    compatibility_state: String,
    release_channel: ManagedReleaseChannel,
}

trait RuntimeProbe: Send + Sync {
    fn probe<'a>(
        &'a self,
        binary: &'a Path,
        allow_prerelease: bool,
    ) -> Pin<Box<dyn Future<Output = Result<CompatibilityProfile>> + Send + 'a>>;
}

struct CompatibilityProbe;

impl RuntimeProbe for CompatibilityProbe {
    fn probe<'a>(
        &'a self,
        binary: &'a Path,
        allow_prerelease: bool,
    ) -> Pin<Box<dyn Future<Output = Result<CompatibilityProfile>> + Send + 'a>> {
        Box::pin(probe_published_managed_release(binary, allow_prerelease))
    }
}

pub struct RapidMlxRuntimeManager {
    root: PathBuf,
    uv_program: PathBuf,
    command_timeout: Duration,
    mutation_gate: Arc<Semaphore>,
    runtime_probe: Arc<dyn RuntimeProbe>,
    platform_supported: bool,
    #[cfg(test)]
    fail_retention_cleanup: bool,
}

impl RapidMlxRuntimeManager {
    pub fn new(config_root: &Path) -> Result<Self> {
        Self::with_uv(config_root, PathBuf::from("uv"))
    }

    pub fn with_uv(config_root: &Path, uv_program: PathBuf) -> Result<Self> {
        let root = prepare_managed_root(config_root)?;
        Ok(Self {
            root,
            uv_program,
            command_timeout: DEFAULT_COMMAND_TIMEOUT,
            mutation_gate: Arc::new(Semaphore::new(1)),
            runtime_probe: Arc::new(CompatibilityProbe),
            platform_supported: local_mutations_supported(),
            #[cfg(test)]
            fail_retention_cleanup: false,
        })
    }

    pub fn managed_root(&self) -> &Path {
        &self.root
    }

    pub fn parse_stable_version(version: &str) -> Result<(u64, u64, u64)> {
        parse_stable_version(version)
    }

    pub fn validate_published_version(version: &str, channel: ManagedReleaseChannel) -> Result<()> {
        let parsed = parse_managed_version(version)?;
        if parsed.prerelease != (channel == ManagedReleaseChannel::Prerelease) {
            bail!("Rapid-MLX release channel does not match its exact version");
        }
        Ok(())
    }

    pub fn status(&self) -> Result<ManagedRuntimeStatus> {
        let pointer = self.load_pointer()?;
        let mut inventory = self.inventory()?;
        let active_id = pointer
            .as_ref()
            .map(|item| item.active_environment_id.as_str());
        let previous_id = pointer
            .as_ref()
            .and_then(|item| item.previous_environment_id.as_deref());
        for item in &mut inventory {
            item.active = active_id == Some(item.environment_id.as_str());
            item.rollback_candidate = previous_id == Some(item.environment_id.as_str());
        }
        let active = inventory.iter().find(|item| item.active).cloned();
        Ok(ManagedRuntimeStatus {
            supported: self.platform_supported,
            installer_available: which::which(&self.uv_program).is_ok(),
            mutation_in_progress: self.mutation_gate.available_permits() == 0,
            rollback_available: inventory.iter().any(|item| item.rollback_candidate),
            active,
            inventory,
        })
    }

    pub fn inventory(&self) -> Result<Vec<RuntimeInventoryEntry>> {
        let environments = checked_existing_child(&self.root, Path::new(ENVIRONMENTS_DIR), true)?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(environments).context("Cannot read managed runtime inventory")? {
            let entry = entry.context("Cannot inspect managed runtime inventory")?;
            let file_type = entry
                .file_type()
                .context("Cannot inspect runtime environment")?;
            if file_type.is_symlink() || !file_type.is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            if validate_environment_id(&id).is_err() {
                continue;
            }
            if let Ok((manifest, binary)) = self.validate_environment(&id) {
                entries.push(RuntimeInventoryEntry {
                    environment_id: id,
                    version: manifest.version,
                    release_channel: manifest.release_channel,
                    executable_path: binary,
                    active: false,
                    rollback_candidate: false,
                    complete: true,
                });
            }
        }
        entries.sort_by(|left, right| left.environment_id.cmp(&right.environment_id));
        Ok(entries)
    }

    pub async fn install_release(
        &self,
        release: ManagedReleaseSelection,
    ) -> Result<RuntimeMutationResult> {
        self.stage_and_activate(release).await
    }

    pub async fn upgrade_release(
        &self,
        release: ManagedReleaseSelection,
    ) -> Result<RuntimeMutationResult> {
        self.stage_and_activate(release).await
    }

    pub async fn repair_release(
        &self,
        release: ManagedReleaseSelection,
    ) -> Result<RuntimeMutationResult> {
        self.ensure_platform()?;
        let _permit = self.try_mutation_permit()?;
        let pointer = self
            .load_pointer()?
            .ok_or_else(|| anyhow!("No active managed Rapid-MLX runtime is available to repair"))?;
        let (manifest, _) = self.validate_environment(&pointer.active_environment_id)?;
        validate_release_selection(&release)?;
        if release.version != manifest.version || release.channel != manifest.release_channel {
            bail!("Rediscovered Rapid-MLX release does not match the active managed runtime");
        }
        self.stage_and_activate_locked(release).await
    }

    #[cfg(test)]
    async fn install(&self, exact_version: &str) -> Result<RuntimeMutationResult> {
        self.install_release(test_stable_release(exact_version)?)
            .await
    }

    #[cfg(test)]
    async fn upgrade(&self, exact_version: &str) -> Result<RuntimeMutationResult> {
        self.upgrade_release(test_stable_release(exact_version)?)
            .await
    }

    pub async fn rollback(&self) -> Result<RuntimeMutationResult> {
        self.ensure_platform()?;
        let _permit = self.try_mutation_permit()?;
        let pointer = self
            .load_pointer()?
            .ok_or_else(|| anyhow!("No active managed Rapid-MLX runtime is available"))?;
        let previous = pointer
            .previous_environment_id
            .as_deref()
            .ok_or_else(|| anyhow!("No previous known-good Rapid-MLX runtime is available"))?;
        let (manifest, binary) = self.validate_environment(previous)?;
        let profile = self
            .runtime_probe
            .probe(
                &binary,
                manifest.release_channel == ManagedReleaseChannel::Prerelease,
            )
            .await
            .map_err(|_| {
                anyhow!("The previous Rapid-MLX runtime failed compatibility revalidation")
            })?;
        require_verified_profile(&manifest.version, &profile)?;
        let next = ActivePointer {
            schema_version: MANIFEST_SCHEMA_VERSION,
            active_environment_id: previous.to_string(),
            previous_environment_id: Some(pointer.active_environment_id.clone()),
        };
        let response = RuntimeMutationResult {
            active: self.entry_for(&next.active_environment_id, true, false)?,
            previous_environment_id: next.previous_environment_id.clone(),
        };
        self.write_pointer(&next)?;
        Ok(response)
    }

    async fn stage_and_activate(
        &self,
        release: ManagedReleaseSelection,
    ) -> Result<RuntimeMutationResult> {
        self.ensure_platform()?;
        let _permit = self.try_mutation_permit()?;
        self.stage_and_activate_locked(release).await
    }

    async fn stage_and_activate_locked(
        &self,
        release: ManagedReleaseSelection,
    ) -> Result<RuntimeMutationResult> {
        let parsed_release = validate_release_selection(&release)?;
        let exact_version = release.version.as_str();
        let environment_id = unique_environment_id(exact_version, &self.root)?;
        let environment = create_checked_child(
            &self.root,
            &Path::new(ENVIRONMENTS_DIR).join(&environment_id),
        )?;
        let result = async {
            let environment_relative = relative_to_root(&self.root, &environment)?;
            let tool_dir = create_checked_child(&self.root, &environment_relative.join("tool"))?;
            let bin_dir = create_checked_child(&self.root, &environment_relative.join("bin"))?;
            let cache_dir = checked_existing_child(&self.root, Path::new("uv-cache"), true)?;
            let python_dir = checked_existing_child(&self.root, Path::new("uv-python"), true)?;

            self.run_uv_install(exact_version, &tool_dir, &bin_dir, &cache_dir, &python_dir)
                .await?;
            let binary_relative = managed_tool_binary_relative();
            let binary = checked_existing_child(&tool_dir, &binary_relative, false)?;
            if !binary.is_file() {
                bail!("The staged Rapid-MLX runtime did not provide its expected executable");
            }
            let binary_relative = relative_to_root(&environment, &binary)?;
            let binary_sha256 = sha256_file(&binary)?;

            let profile = self
                .runtime_probe
                .probe(&binary, parsed_release.prerelease)
                .await
                .map_err(|_| {
                    anyhow!("The staged Rapid-MLX runtime failed compatibility validation")
                })?;
            require_verified_profile(exact_version, &profile)?;
            let manifest = EnvironmentManifest {
                schema_version: MANIFEST_SCHEMA_VERSION,
                environment_id: environment_id.clone(),
                version: exact_version.to_string(),
                binary_relative_path: path_to_manifest_string(&binary_relative)?,
                binary_sha256,
                runtime_source: RuntimeSource::Managed,
                compatibility_state: CompatibilityState::Verified.label().to_string(),
                release_channel: release.channel,
            };
            let environment = checked_existing_child(
                &self.root,
                &Path::new(ENVIRONMENTS_DIR).join(&environment_id),
                true,
            )?;
            atomic_json_write(&environment.join(MANIFEST_FILE), &manifest, true)?;
            let completion_path = environment.join(COMPLETION_FILE);
            let completion = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&completion_path)
                .context("Cannot create managed runtime completion marker")?;
            completion.sync_all()?;

            self.validate_environment(&environment_id)?;
            let old = self.load_pointer()?;
            let pointer = ActivePointer {
                schema_version: MANIFEST_SCHEMA_VERSION,
                active_environment_id: environment_id.clone(),
                previous_environment_id: old.map(|item| item.active_environment_id),
            };
            // Build the response before activation so no later failure can cause
            // cleanup to remove the newly active environment.
            let active = self.entry_for(&environment_id, true, false)?;
            let response = RuntimeMutationResult {
                active,
                previous_environment_id: pointer.previous_environment_id.clone(),
            };
            self.write_pointer(&pointer)?;
            Ok(response)
        }
        .await;

        match result {
            Ok(result) => {
                // current.json is the commit boundary. Retention is maintenance:
                // it must never turn a committed activation into a false failure.
                let _ = self.remove_unretained_environments();
                Ok(result)
            }
            Err(error) => {
                let _ = self.remove_inactive_environment(&environment_id);
                Err(error)
            }
        }
    }

    async fn run_uv_install(
        &self,
        exact_version: &str,
        tool_dir: &Path,
        bin_dir: &Path,
        cache_dir: &Path,
        python_dir: &Path,
    ) -> Result<()> {
        let requirement = format!("rapid-mlx=={exact_version}");
        let mut command = Command::new(&self.uv_program);
        command
            .args([
                OsStr::new("--no-config"),
                OsStr::new("tool"),
                OsStr::new("install"),
                OsStr::new(&requirement),
                OsStr::new("--link-mode"),
                OsStr::new("copy"),
                OsStr::new("--no-progress"),
                OsStr::new("--no-color"),
            ])
            .env_clear()
            .env("UV_TOOL_DIR", tool_dir)
            .env("UV_TOOL_BIN_DIR", bin_dir)
            .env("UV_CACHE_DIR", cache_dir)
            .env("UV_PYTHON_INSTALL_DIR", python_dir)
            .env("UV_NO_CONFIG", "1")
            .env("UV_NO_PROGRESS", "1")
            .env("NO_COLOR", "1")
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in ["PATH", "SSL_CERT_FILE", "SSL_CERT_DIR"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        configure_process_group(&mut command);
        let output = run_bounded_command(command, self.command_timeout)
            .await
            .map_err(|error| anyhow!("Managed Rapid-MLX installation failed: {error}"))?;
        if !output.status.success() {
            bail!(
                "Managed Rapid-MLX installation failed with status {}",
                output.status
            );
        }
        Ok(())
    }

    fn validate_environment(&self, id: &str) -> Result<(EnvironmentManifest, PathBuf)> {
        validate_environment_id(id)?;
        let relative = Path::new(ENVIRONMENTS_DIR).join(id);
        let environment = checked_existing_child(&self.root, &relative, true)?;
        let complete = checked_existing_child(&self.root, &relative.join(COMPLETION_FILE), false)?;
        let completion_metadata = fs::metadata(&complete)?;
        if !completion_metadata.is_file() || completion_metadata.len() != 0 {
            bail!("Managed runtime completion marker is invalid");
        }
        let manifest_path =
            checked_existing_child(&self.root, &relative.join(MANIFEST_FILE), false)?;
        let metadata = fs::metadata(&manifest_path)?;
        if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES {
            bail!("Managed runtime manifest is invalid");
        }
        let manifest: EnvironmentManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
        if manifest.schema_version != MANIFEST_SCHEMA_VERSION
            || manifest.environment_id != id
            || manifest.runtime_source != RuntimeSource::Managed
            || manifest.compatibility_state != CompatibilityState::Verified.label()
        {
            bail!("Managed runtime manifest does not match its environment");
        }
        let parsed = parse_managed_version(&manifest.version)?;
        if parsed.prerelease != (manifest.release_channel == ManagedReleaseChannel::Prerelease) {
            bail!("Managed runtime manifest release channel is invalid");
        }
        let binary_relative = PathBuf::from(&manifest.binary_relative_path);
        validate_relative_path(&binary_relative)?;
        let binary = checked_existing_child(&environment, &binary_relative, false)?;
        if !binary.is_file() {
            bail!("Managed runtime executable is not a regular file");
        }
        if manifest.binary_sha256.len() != 64
            || !manifest
                .binary_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || sha256_file(&binary)? != manifest.binary_sha256
        {
            bail!("Managed runtime executable integrity validation failed");
        }
        Ok((manifest, binary))
    }

    fn entry_for(
        &self,
        id: &str,
        active: bool,
        rollback_candidate: bool,
    ) -> Result<RuntimeInventoryEntry> {
        let (manifest, binary) = self.validate_environment(id)?;
        Ok(RuntimeInventoryEntry {
            environment_id: id.to_string(),
            version: manifest.version,
            release_channel: manifest.release_channel,
            executable_path: binary,
            active,
            rollback_candidate,
            complete: true,
        })
    }

    fn load_pointer(&self) -> Result<Option<ActivePointer>> {
        let path = self.root.join(POINTER_FILE);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
            Ok(metadata) => {
                if metadata.file_type().is_symlink()
                    || !metadata.is_file()
                    || metadata.len() > MAX_MANIFEST_BYTES
                {
                    bail!("Managed runtime activation pointer is invalid");
                }
                let pointer: ActivePointer = serde_json::from_slice(&fs::read(&path)?)?;
                if pointer.schema_version != MANIFEST_SCHEMA_VERSION {
                    bail!("Managed runtime activation pointer has an unsupported schema");
                }
                validate_environment_id(&pointer.active_environment_id)?;
                if let Some(previous) = &pointer.previous_environment_id {
                    validate_environment_id(previous)?;
                }
                Ok(Some(pointer))
            }
        }
    }

    fn write_pointer(&self, pointer: &ActivePointer) -> Result<()> {
        validate_environment_id(&pointer.active_environment_id)?;
        self.validate_environment(&pointer.active_environment_id)?;
        if let Some(previous) = &pointer.previous_environment_id {
            validate_environment_id(previous)?;
            self.validate_environment(previous)?;
        }
        atomic_json_write(&self.root.join(POINTER_FILE), pointer, false)
    }

    fn remove_inactive_environment(&self, id: &str) -> Result<()> {
        validate_environment_id(id)?;
        if let Some(pointer) = self.load_pointer()?
            && (pointer.active_environment_id == id
                || pointer.previous_environment_id.as_deref() == Some(id))
        {
            bail!("Refusing to remove a retained managed runtime environment");
        }
        let relative = Path::new(ENVIRONMENTS_DIR).join(id);
        let path = match checked_existing_child(&self.root, &relative, true) {
            Ok(path) => path,
            Err(_error) if !self.root.join(&relative).exists() => return Ok(()),
            Err(error) => return Err(error),
        };
        fs::remove_dir_all(path).context("Cannot remove inactive managed runtime environment")
    }

    fn remove_unretained_environments(&self) -> Result<()> {
        #[cfg(test)]
        if self.fail_retention_cleanup {
            bail!("Injected retention cleanup failure");
        }
        let Some(pointer) = self.load_pointer()? else {
            return Ok(());
        };
        let environments = checked_existing_child(&self.root, Path::new(ENVIRONMENTS_DIR), true)?;
        for entry in fs::read_dir(environments).context("Cannot inspect managed runtimes")? {
            let entry = entry.context("Cannot inspect managed runtime")?;
            let file_type = entry
                .file_type()
                .context("Cannot inspect managed runtime")?;
            if file_type.is_symlink() || !file_type.is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            if id == pointer.active_environment_id
                || pointer.previous_environment_id.as_deref() == Some(id.as_str())
            {
                continue;
            }
            // Only validated, complete environments participate in retention
            // pruning. Incomplete stage directories are cleaned by their owner.
            if self.validate_environment(&id).is_ok() {
                self.remove_inactive_environment(&id)?;
            }
        }
        Ok(())
    }

    fn try_mutation_permit(&self) -> Result<tokio::sync::OwnedSemaphorePermit> {
        self.mutation_gate
            .clone()
            .try_acquire_owned()
            .map_err(|error| match error {
                TryAcquireError::NoPermits => {
                    anyhow!("Another managed Rapid-MLX runtime operation is already in progress")
                }
                TryAcquireError::Closed => {
                    anyhow!("Managed Rapid-MLX runtime manager is unavailable")
                }
            })
    }

    fn ensure_platform(&self) -> Result<()> {
        if !self.platform_supported {
            bail!("Managed Rapid-MLX runtime changes require macOS on Apple Silicon");
        }
        Ok(())
    }
}

fn local_mutations_supported() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
}

#[cfg(unix)]
fn managed_tool_binary_relative() -> PathBuf {
    PathBuf::from("rapid-mlx/bin/rapid-mlx")
}

#[cfg(windows)]
fn managed_tool_binary_relative() -> PathBuf {
    PathBuf::from("rapid-mlx/Scripts/rapid-mlx.exe")
}

fn require_verified_profile(requested: &str, profile: &CompatibilityProfile) -> Result<()> {
    if profile.state != CompatibilityState::Verified || profile.version != requested {
        bail!("The staged Rapid-MLX runtime did not verify as the exact requested release");
    }
    Ok(())
}

fn parse_stable_version(version: &str) -> Result<(u64, u64, u64)> {
    let parsed = parse_managed_version(version)?;
    if parsed.prerelease {
        bail!("Rapid-MLX version must be an exact stable major.minor.patch release");
    }
    Ok(parsed.numbers)
}

struct ParsedManagedVersion {
    numbers: (u64, u64, u64),
    prerelease: bool,
}

fn parse_managed_version(version: &str) -> Result<ParsedManagedVersion> {
    if version.is_empty() || version.trim() != version || version.contains('+') {
        bail!("Rapid-MLX version must be an exact published package version");
    }
    let mut cursor = 0;
    let bytes = version.as_bytes();
    let major = parse_version_number(bytes, &mut cursor)?;
    require_version_byte(bytes, &mut cursor, b'.')?;
    let minor = parse_version_number(bytes, &mut cursor)?;
    require_version_byte(bytes, &mut cursor, b'.')?;
    let patch = parse_version_number(bytes, &mut cursor)?;
    let numbers = (major, minor, patch);
    if numbers < MINIMUM_VERIFIED_VERSION {
        bail!("Rapid-MLX version 0.10.9 or newer is required");
    }
    let suffix = &version[cursor..];
    let suffix = suffix.strip_prefix('-').unwrap_or(suffix);
    let prerelease = !suffix.is_empty();
    if prerelease {
        let lowercase = suffix.to_ascii_lowercase();
        let recognized = ["a", "alpha", "b", "beta", "rc", "dev"]
            .iter()
            .any(|prefix| lowercase.starts_with(prefix));
        if !recognized
            || !suffix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            bail!("Rapid-MLX prerelease version is not an exact supported package version");
        }
    }
    Ok(ParsedManagedVersion {
        numbers,
        prerelease,
    })
}

fn parse_version_number(bytes: &[u8], cursor: &mut usize) -> Result<u64> {
    let start = *cursor;
    while bytes.get(*cursor).is_some_and(u8::is_ascii_digit) {
        *cursor += 1;
    }
    if start == *cursor || (*cursor - start > 1 && bytes[start] == b'0') {
        bail!("Rapid-MLX version must use canonical numeric components");
    }
    Ok(std::str::from_utf8(&bytes[start..*cursor])?.parse()?)
}

fn require_version_byte(bytes: &[u8], cursor: &mut usize, expected: u8) -> Result<()> {
    if bytes.get(*cursor) != Some(&expected) {
        bail!("Rapid-MLX version must include major.minor.patch");
    }
    *cursor += 1;
    Ok(())
}

fn validate_release_selection(release: &ManagedReleaseSelection) -> Result<ParsedManagedVersion> {
    let _ = release.provenance;
    let parsed = parse_managed_version(&release.version)?;
    RapidMlxRuntimeManager::validate_published_version(&release.version, release.channel)?;
    Ok(parsed)
}

#[cfg(test)]
fn test_stable_release(version: &str) -> Result<ManagedReleaseSelection> {
    ManagedReleaseSelection::from_published_release(version, ManagedReleaseChannel::Stable)
}

fn prepare_managed_root(config_root: &Path) -> Result<PathBuf> {
    fs::create_dir_all(config_root).context("Cannot create the application config directory")?;
    let config_metadata = fs::symlink_metadata(config_root)?;
    if config_metadata.file_type().is_symlink() || !config_metadata.is_dir() {
        bail!("Application config root must be a non-symlink directory");
    }
    let canonical_config = config_root.canonicalize()?;
    let root = create_checked_child(&canonical_config, Path::new(MANAGED_RELATIVE_ROOT))?;
    create_checked_child(&root, Path::new(ENVIRONMENTS_DIR))?;
    create_checked_child(&root, Path::new("uv-cache"))?;
    create_checked_child(&root, Path::new("uv-python"))?;
    Ok(root)
}

fn create_checked_child(root: &Path, relative: &Path) -> Result<PathBuf> {
    validate_relative_path(relative)?;
    let canonical_root = root.canonicalize()?;
    let mut current = canonical_root.clone();
    for component in relative.components() {
        if component == Component::CurDir {
            continue;
        }
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                bail!("Managed runtime path contains a symlink or non-directory component")
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&current)?,
            Err(error) => return Err(error.into()),
        }
        current = current.canonicalize()?;
        require_inside(&current, &canonical_root)?;
    }
    Ok(current)
}

fn checked_existing_child(root: &Path, relative: &Path, directory: bool) -> Result<PathBuf> {
    validate_relative_path(relative)?;
    let canonical_root = root.canonicalize()?;
    let mut current = canonical_root.clone();
    let components: Vec<_> = relative.components().collect();
    for (index, component) in components.iter().enumerate() {
        if *component == Component::CurDir {
            continue;
        }
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() {
            bail!("Managed runtime path must not contain symlinked components");
        }
        if index + 1 < components.len() && !metadata.is_dir() {
            bail!("Managed runtime path contains a non-directory component");
        }
    }
    let canonical = current.canonicalize()?;
    require_inside(&canonical, &canonical_root)?;
    if directory && !canonical.is_dir() {
        bail!("Managed runtime path must be a directory");
    }
    Ok(canonical)
}

fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || path.to_string_lossy().starts_with(['/', '\\'])
    {
        bail!("Managed runtime path must be a traversal-free relative path");
    }
    Ok(())
}

fn require_inside(path: &Path, root: &Path) -> Result<()> {
    if path == root || path.starts_with(root) {
        Ok(())
    } else {
        bail!("Managed runtime path escapes its canonical root")
    }
}

fn relative_to_root(root: &Path, path: &Path) -> Result<PathBuf> {
    Ok(path
        .strip_prefix(root)
        .context("Managed runtime path escapes its canonical root")?
        .to_path_buf())
}

fn validate_environment_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
    {
        bail!("Managed runtime environment ID is invalid");
    }
    Ok(())
}

fn unique_environment_id(version: &str, root: &Path) -> Result<String> {
    for _ in 0..8 {
        let mut random = [0_u8; 12];
        SysRng
            .try_fill_bytes(&mut random)
            .map_err(|_| anyhow!("Secure randomness is unavailable"))?;
        let suffix: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let id = format!("{version}-{suffix}");
        if !root.join(ENVIRONMENTS_DIR).join(&id).exists() {
            return Ok(id);
        }
    }
    bail!("Could not allocate a unique managed runtime environment")
}

fn path_to_manifest_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("Managed runtime path is not valid UTF-8"))
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).context("Cannot read managed runtime executable")?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .context("Cannot hash managed runtime executable")?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn atomic_json_write<T: Serialize>(path: &Path, value: &T, harden: bool) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Managed runtime file has no parent"))?;
    let root = parent.canonicalize()?;
    let metadata = fs::symlink_metadata(parent)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!("Managed runtime file parent is invalid");
    }
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| anyhow!("Managed runtime filename is invalid"))?;
    let temp = root.join(format!(".{file_name}.tmp"));
    if fs::symlink_metadata(&temp).is_ok() {
        bail!("Managed runtime temporary file already exists");
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&temp, bytes)?;
    if harden {
        harden_file_permissions(&temp);
    }
    fs::rename(&temp, path)?;
    if harden {
        harden_file_permissions(path);
    }
    Ok(())
}

#[derive(Debug)]
struct BoundedCommandOutput {
    status: ExitStatus,
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.as_std_mut().process_group(0);
}

#[cfg(windows)]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_process_tree(pid: u32) {
    // The child is placed in a dedicated process group whose ID is its PID.
    // SAFETY: kill is called with a validated child PID and the constant SIGKILL.
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
    }
}

#[cfg(windows)]
fn terminate_process_tree(_pid: u32) {}

async fn terminate_and_reap(child: &mut tokio::process::Child, pid: u32) {
    terminate_process_tree(pid);
    let _ = child.start_kill();
    let _ = child.wait().await;
}

async fn run_bounded_command(command: Command, timeout: Duration) -> Result<BoundedCommandOutput> {
    run_bounded_command_inner(command, timeout, None).await
}

async fn run_bounded_command_inner(
    mut command: Command,
    timeout: Duration,
    ready_path: Option<&Path>,
) -> Result<BoundedCommandOutput> {
    let mut child = command.spawn().context("Could not start uv")?;
    let pid = child
        .id()
        .ok_or_else(|| anyhow!("Could not identify the uv process"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Could not capture uv stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("Could not capture uv stderr"))?;
    let (output_tx, mut output_rx) = tokio::sync::mpsc::channel(2);
    let stdout_tx = output_tx.clone();
    tokio::spawn(async move {
        let _ = stdout_tx.send(read_bounded(stdout).await).await;
    });
    let stderr_tx = output_tx.clone();
    tokio::spawn(async move {
        let _ = stderr_tx.send(read_bounded(stderr).await).await;
    });
    drop(output_tx);

    if let Some(ready_path) = ready_path {
        while !ready_path.exists() {
            if child.try_wait()?.is_some() {
                return Err(anyhow!("uv exited before startup completed"));
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);
    let mut status = None;
    let mut completed_readers = 0;
    loop {
        tokio::select! {
            _ = &mut deadline => {
                terminate_and_reap(&mut child, pid).await;
                return Err(anyhow!("uv timed out before completing"));
            }
            result = output_rx.recv(), if completed_readers < 2 => {
                match result {
                    Some(Ok(())) => completed_readers += 1,
                    Some(Err(error)) => {
                        terminate_and_reap(&mut child, pid).await;
                        return Err(error);
                    }
                    None => {
                        terminate_and_reap(&mut child, pid).await;
                        return Err(anyhow!("uv output capture ended unexpectedly"));
                    }
                }
            }
            result = child.wait(), if status.is_none() => {
                status = Some(result.context("Could not wait for uv")?);
            }
        }
        if completed_readers == 2
            && let Some(status) = status
        {
            return Ok(BoundedCommandOutput { status });
        }
    }
}

async fn read_bounded<R: tokio::io::AsyncRead + Unpin>(reader: R) -> Result<()> {
    let mut bytes = Vec::with_capacity(8192);
    reader
        .take((MAX_COMMAND_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > MAX_COMMAND_OUTPUT_BYTES {
        bail!("uv output exceeded its safety limit");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::rapid_mlx::compatibility::ServeCapabilities;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct FakeProbe {
        fail: AtomicBool,
    }

    impl RuntimeProbe for FakeProbe {
        fn probe<'a>(
            &'a self,
            binary: &'a Path,
            _allow_prerelease: bool,
        ) -> Pin<Box<dyn Future<Output = Result<CompatibilityProfile>> + Send + 'a>> {
            Box::pin(async move {
                if self.fail.load(Ordering::SeqCst) {
                    bail!("fixture probe failure at {}", binary.display());
                }
                let version = binary
                    .ancestors()
                    .nth(4)
                    .and_then(Path::file_name)
                    .and_then(OsStr::to_str)
                    .and_then(|id| id.rsplit_once('-').map(|(version, _)| version))
                    .unwrap()
                    .to_string();
                Ok(CompatibilityProfile {
                    state: CompatibilityState::Verified,
                    version,
                    capabilities: ServeCapabilities::verified_baseline(),
                })
            })
        }
    }

    #[cfg(unix)]
    fn fixture_manager() -> (tempfile::TempDir, RapidMlxRuntimeManager, Arc<FakeProbe>) {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let uv = temp.path().join("fake-uv");
        fs::write(
            &uv,
            "#!/bin/sh\ncase \"$*\" in *--no-config*--link-mode\\ copy*--no-progress*--no-color*) ;; *) exit 41 ;; esac\n[ -z \"$HOME\" ] || exit 42\n[ -n \"$UV_CACHE_DIR\" ] && [ -n \"$UV_PYTHON_INSTALL_DIR\" ] || exit 43\nmkdir -p \"$UV_TOOL_DIR/rapid-mlx/bin\" \"$UV_TOOL_BIN_DIR\"\nprintf '#!/bin/sh\\nexit 0\\n' > \"$UV_TOOL_DIR/rapid-mlx/bin/rapid-mlx\"\nchmod +x \"$UV_TOOL_DIR/rapid-mlx/bin/rapid-mlx\"\nln -s \"$UV_TOOL_DIR/rapid-mlx/bin/rapid-mlx\" \"$UV_TOOL_BIN_DIR/rapid-mlx\"\n",
        )
        .unwrap();
        fs::set_permissions(&uv, fs::Permissions::from_mode(0o755)).unwrap();
        let probe = Arc::new(FakeProbe {
            fail: AtomicBool::new(false),
        });
        let mut manager = RapidMlxRuntimeManager::with_uv(temp.path(), uv).unwrap();
        manager.runtime_probe = probe.clone();
        manager.platform_supported = true;
        (temp, manager, probe)
    }

    #[test]
    fn stable_version_parser_is_exact_and_forward_compatible() {
        assert_eq!(parse_stable_version("0.10.9").unwrap(), (0, 10, 9));
        assert_eq!(parse_stable_version("0.10.10").unwrap(), (0, 10, 10));
        assert_eq!(parse_stable_version("12.34.56").unwrap(), (12, 34, 56));
        for invalid in [
            "0.10.8",
            "v0.10.10",
            "0.10.10rc1",
            "0.10.10+local",
            "0.10",
            "01.10.10",
            " 0.10.10",
        ] {
            assert!(parse_stable_version(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn published_prerelease_selection_is_explicit_and_exact() {
        let selected = ManagedReleaseSelection::from_published_release(
            "0.10.11rc1",
            ManagedReleaseChannel::Prerelease,
        )
        .unwrap();
        let parsed = validate_release_selection(&selected).unwrap();
        assert!(parsed.prerelease);
        assert!(
            ManagedReleaseSelection::from_published_release(
                "0.10.11rc1",
                ManagedReleaseChannel::Stable,
            )
            .is_err()
        );
        assert!(
            ManagedReleaseSelection::from_published_release(
                "0.10.11+local",
                ManagedReleaseChannel::Prerelease,
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn activation_is_atomic_and_failed_probe_preserves_active() {
        let (_temp, manager, probe) = fixture_manager();
        let first = manager.install("0.10.9").await.unwrap();
        assert!(
            first
                .active
                .executable_path
                .ends_with("tool/rapid-mlx/bin/rapid-mlx")
        );
        assert!(
            fs::symlink_metadata(
                first
                    .active
                    .executable_path
                    .ancestors()
                    .nth(4)
                    .unwrap()
                    .join("bin/rapid-mlx")
            )
            .unwrap()
            .file_type()
            .is_symlink()
        );
        let pointer_before = fs::read(manager.root.join(POINTER_FILE)).unwrap();
        assert_eq!(first.active.version, "0.10.9");
        probe.fail.store(true, Ordering::SeqCst);
        assert!(manager.upgrade("0.10.10").await.is_err());
        assert_eq!(
            fs::read(manager.root.join(POINTER_FILE)).unwrap(),
            pointer_before
        );
        assert_eq!(manager.status().unwrap().active.unwrap().version, "0.10.9");
        assert_eq!(manager.inventory().unwrap().len(), 1);
        assert_eq!(
            fs::read_dir(manager.root.join(ENVIRONMENTS_DIR))
                .unwrap()
                .count(),
            1
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn pointer_write_failure_is_precommit_and_cleans_stage() {
        let (_temp, manager, _probe) = fixture_manager();
        manager.install("0.10.9").await.unwrap();
        let pointer_before = fs::read(manager.root.join(POINTER_FILE)).unwrap();
        fs::write(manager.root.join(".current.json.tmp"), b"occupied").unwrap();
        let error = manager.upgrade("0.10.10").await.unwrap_err();
        assert!(error.to_string().contains("temporary file"));
        assert_eq!(
            fs::read(manager.root.join(POINTER_FILE)).unwrap(),
            pointer_before
        );
        assert_eq!(
            fs::read_dir(manager.root.join(ENVIRONMENTS_DIR))
                .unwrap()
                .count(),
            1
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn postcommit_retention_failure_still_reports_activation_success() {
        let (_temp, mut manager, _probe) = fixture_manager();
        manager.install("0.10.9").await.unwrap();
        manager.install("0.10.10").await.unwrap();
        manager.fail_retention_cleanup = true;
        let activated = manager.install("0.10.11").await.unwrap();
        assert_eq!(activated.active.version, "0.10.11");
        assert_eq!(manager.status().unwrap().active.unwrap().version, "0.10.11");
        assert_eq!(manager.inventory().unwrap().len(), 3);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn repair_isolated_and_rollback_swaps_known_good_environments() {
        let (_temp, manager, _probe) = fixture_manager();
        let first = manager.install("0.10.9").await.unwrap();
        let repaired = manager
            .repair_release(test_stable_release("0.10.9").unwrap())
            .await
            .unwrap();
        assert_eq!(repaired.active.version, "0.10.9");
        assert_ne!(repaired.active.environment_id, first.active.environment_id);
        assert_eq!(
            repaired.previous_environment_id.as_deref(),
            Some(first.active.environment_id.as_str())
        );
        let upgraded = manager.upgrade("0.10.10").await.unwrap();
        let rolled_back = manager.rollback().await.unwrap();
        assert_eq!(
            rolled_back.active.environment_id,
            repaired.active.environment_id
        );
        assert_eq!(
            rolled_back.previous_environment_id.as_deref(),
            Some(upgraded.active.environment_id.as_str())
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn repair_requires_rediscovered_exact_release_proof() {
        let (_temp, manager, _probe) = fixture_manager();
        let installed = manager.install("0.10.9").await.unwrap();
        let pointer_before = fs::read(manager.root.join(POINTER_FILE)).unwrap();
        let error = manager
            .repair_release(test_stable_release("0.10.10").unwrap())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("does not match"));
        assert_eq!(
            fs::read(manager.root.join(POINTER_FILE)).unwrap(),
            pointer_before
        );
        assert_eq!(manager.inventory().unwrap().len(), 1);
        assert_eq!(
            manager.status().unwrap().active.unwrap().environment_id,
            installed.active.environment_id
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn explicitly_published_prerelease_can_activate() {
        let (_temp, manager, _probe) = fixture_manager();
        let result = manager
            .install_release(
                ManagedReleaseSelection::from_published_release(
                    "0.10.11rc1",
                    ManagedReleaseChannel::Prerelease,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(result.active.version, "0.10.11rc1");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn successful_activation_retains_only_active_and_previous() {
        let (_temp, manager, _probe) = fixture_manager();
        let first = manager.install("0.10.9").await.unwrap();
        let second = manager.install("0.10.10").await.unwrap();
        let third = manager.install("0.10.11").await.unwrap();
        let inventory = manager.inventory().unwrap();
        assert_eq!(inventory.len(), 2);
        assert!(
            !inventory
                .iter()
                .any(|item| item.environment_id == first.active.environment_id)
        );
        assert!(
            inventory
                .iter()
                .any(|item| item.environment_id == second.active.environment_id)
        );
        assert!(
            inventory
                .iter()
                .any(|item| item.environment_id == third.active.environment_id)
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn binary_hash_tampering_blocks_inventory_and_rollback() {
        let (_temp, manager, _probe) = fixture_manager();
        let first = manager.install("0.10.9").await.unwrap();
        let second = manager.install("0.10.10").await.unwrap();
        fs::write(&first.active.executable_path, b"tampered").unwrap();
        assert!(
            !manager
                .inventory()
                .unwrap()
                .iter()
                .any(|item| item.environment_id == first.active.environment_id)
        );
        let pointer_before = fs::read(manager.root.join(POINTER_FILE)).unwrap();
        assert!(manager.rollback().await.is_err());
        assert_eq!(
            fs::read(manager.root.join(POINTER_FILE)).unwrap(),
            pointer_before
        );
        assert_eq!(
            manager.status().unwrap().active.unwrap().environment_id,
            second.active.environment_id
        );
    }

    fn snapshot_tree(root: &Path) -> Vec<(PathBuf, bool, u64)> {
        fn visit(root: &Path, current: &Path, output: &mut Vec<(PathBuf, bool, u64)>) {
            let mut entries: Vec<_> = fs::read_dir(current)
                .unwrap()
                .map(|entry| entry.unwrap())
                .collect();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let metadata = fs::symlink_metadata(entry.path()).unwrap();
                output.push((
                    entry.path().strip_prefix(root).unwrap().to_path_buf(),
                    metadata.is_dir(),
                    metadata.len(),
                ));
                if metadata.is_dir() {
                    visit(root, &entry.path(), output);
                }
            }
        }
        let mut output = Vec::new();
        visit(root, root, &mut output);
        output
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unsupported_platform_rejects_all_mutations_without_side_effects() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let uv = temp.path().join("must-not-run-uv");
        let invoked = temp.path().join("uv-invoked");
        fs::write(
            &uv,
            format!("#!/bin/sh\ntouch '{}'\nexit 99\n", invoked.display()),
        )
        .unwrap();
        fs::set_permissions(&uv, fs::Permissions::from_mode(0o755)).unwrap();
        let mut manager = RapidMlxRuntimeManager::with_uv(&config, uv).unwrap();
        manager.platform_supported = false;
        let before = snapshot_tree(&manager.root);
        let release = test_stable_release("0.10.10").unwrap();

        for error in [
            manager.install_release(release.clone()).await.unwrap_err(),
            manager.upgrade_release(release.clone()).await.unwrap_err(),
            manager.repair_release(release).await.unwrap_err(),
            manager.rollback().await.unwrap_err(),
        ] {
            assert!(
                error.to_string().contains("require macOS on Apple Silicon"),
                "{error:#}"
            );
        }
        assert!(!invoked.exists());
        assert!(!manager.root.join(POINTER_FILE).exists());
        assert_eq!(snapshot_tree(&manager.root), before);
    }

    #[cfg(unix)]
    #[test]
    fn managed_paths_reject_traversal_and_symlink_components() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        assert!(validate_relative_path(Path::new("../escape")).is_err());
        let root = temp.path().join("root");
        fs::create_dir(&root).unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, root.join("linked")).unwrap();
        assert!(checked_existing_child(&root, Path::new("linked"), true).is_err());
        assert!(create_checked_child(&root, Path::new("linked/child")).is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn concurrent_mutation_fails_fast() {
        let (_temp, manager, _probe) = fixture_manager();
        let held = manager.mutation_gate.clone().try_acquire_owned().unwrap();
        let error = manager.install("0.10.10").await.unwrap_err();
        assert!(error.to_string().contains("already in progress"));
        drop(held);
        assert!(manager.install("0.10.10").await.is_ok());
    }

    #[cfg(unix)]
    fn process_exists(pid: i32) -> bool {
        // SAFETY: signal 0 only checks whether the fixture PID still exists.
        unsafe { libc::kill(pid, 0) == 0 }
    }

    #[cfg(unix)]
    async fn assert_fixture_descendant_reaped(pid_path: &Path) {
        let pid: i32 = fs::read_to_string(pid_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        for _ in 0..20 {
            if !process_exists(pid) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            !process_exists(pid),
            "fixture descendant {pid} survived cleanup"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn command_timeout_terminates_and_reaps_process_group() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("timeout-uv");
        let pid_path = temp.path().join("descendant.pid");
        fs::write(&script, "#!/bin/sh\nsleep 60 &\necho $! > \"$1\"\nwait\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let mut command = Command::new(script);
        command
            .arg(&pid_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_process_group(&mut command);
        let error = run_bounded_command_inner(command, Duration::from_millis(75), Some(&pid_path))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert_fixture_descendant_reaped(&pid_path).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn output_overflow_terminates_and_reaps_process_group() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("overflow-uv");
        let pid_path = temp.path().join("descendant.pid");
        fs::write(
            &script,
            "#!/bin/sh\nsleep 60 &\necho $! > \"$1\"\nwhile :; do printf x; done\n",
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let mut command = Command::new(script);
        command
            .arg(&pid_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_process_group(&mut command);
        let error = run_bounded_command_inner(command, Duration::from_secs(2), Some(&pid_path))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("safety limit"));
        assert_fixture_descendant_reaped(&pid_path).await;
    }

    /// Opt-in release qualification for the production uv workflow. This is ignored in
    /// normal and CI suites because it downloads two published runtime environments.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[tokio::test]
    #[ignore = "requires uv, network access, and published Rapid-MLX packages"]
    async fn real_uv_install_upgrade_rollback_and_reactivate() {
        let uv = which::which("uv").expect("uv must be installed for release qualification");
        let temp = tempfile::tempdir().unwrap();
        let manager = RapidMlxRuntimeManager::with_uv(temp.path(), uv).unwrap();

        let installed = manager.install("0.10.9").await.unwrap();
        assert_eq!(installed.active.version, "0.10.9");

        let upgraded = manager.upgrade("0.10.10").await.unwrap();
        assert_eq!(upgraded.active.version, "0.10.10");

        let rolled_back = manager.rollback().await.unwrap();
        assert_eq!(rolled_back.active.version, "0.10.9");

        let reactivated = manager.rollback().await.unwrap();
        assert_eq!(reactivated.active.version, "0.10.10");
        assert_eq!(manager.inventory().unwrap().len(), 2);
    }
}
