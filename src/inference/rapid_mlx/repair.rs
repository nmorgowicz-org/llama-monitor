//! Bounded, local-source MTP sidecar repair jobs.
//!
//! The repair executable is deliberately kept outside Rust: MLX tensor
//! loading and NuSLERP must stay in the same Python environment as Rapid-MLX.
//! Rust owns the product boundary—path validation, lifecycle, cancellation,
//! output limits, and the managed sidecar destination.

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::{Notify, Semaphore};

use super::sidecar_inventory::{SidecarEntry, discover_sidecars, sidecar_dir_for_trunk};

const MAX_JOBS: usize = 16;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const REPAIR_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const SCRIPT_NAME: &str = "repair-mtp-mlx.py";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairRequest {
    pub target: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_format: String,
    #[serde(default)]
    pub recipe: Option<String>,
    /// BF16/HF source for the guarded `build-mtp-head.py` extraction path.
    #[serde(default)]
    pub bf16_source: Option<String>,
    #[serde(default)]
    pub bf16_revision: Option<String>,
    #[serde(default)]
    pub python: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairJobSnapshot {
    pub job_id: String,
    pub status: String,
    pub phase: String,
    pub message: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

struct RuntimeJob {
    snapshot: Arc<Mutex<RepairJobSnapshot>>,
    cancel: Arc<Notify>,
}

static JOBS: LazyLock<Mutex<BTreeMap<String, RuntimeJob>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static JOB_GATE: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));

pub fn start_job(request: RepairRequest, scripts_dir: &Path) -> Result<RepairJobSnapshot> {
    let target = existing_directory(&request.target, "target")?;
    let source = request
        .source
        .as_deref()
        .map(|value| existing_directory(value, "source"))
        .transpose()?;
    let recipe = request
        .recipe
        .as_deref()
        .map(|value| existing_file(value, "recipe"))
        .transpose()?;
    let bf16_source = request
        .bf16_source
        .as_deref()
        .map(validate_bf16_source)
        .transpose()?;
    let bf16_revision = request
        .bf16_revision
        .as_deref()
        .map(validate_revision)
        .transpose()?;
    if source.is_none() && recipe.is_none() && bf16_source.is_none() {
        bail!("one of source, recipe, or bf16Source is required");
    }
    if [source.is_some(), recipe.is_some(), bf16_source.is_some()]
        .into_iter()
        .filter(|present| *present)
        .count()
        > 1
    {
        bail!("source, recipe, and bf16Source are mutually exclusive");
    }
    if bf16_source
        .as_deref()
        .is_some_and(|value| !Path::new(value).is_absolute())
        && bf16_revision.is_none()
    {
        bail!("bf16Revision is required for a Hugging Face BF16 source");
    }
    let source_format = if request.source_format.trim().is_empty() {
        "mlx".to_string()
    } else {
        request.source_format.to_ascii_lowercase()
    };
    if source_format != "mlx" && source_format != "hf" {
        bail!("sourceFormat must be mlx or hf");
    }
    let script = resolve_script(
        scripts_dir,
        if bf16_source.is_some() {
            "build-mtp-head.py"
        } else {
            SCRIPT_NAME
        },
    )?;
    let python = request.python.unwrap_or_else(|| "python3".to_string());
    validate_executable_name(&python)?;
    let output_dir = sidecar_dir_for_trunk(&target);
    let output = output_dir.join("mtp.safetensors");
    let job_id = format!("mtp-repair-{:032x}", rand::random::<u128>());
    let snapshot = Arc::new(Mutex::new(RepairJobSnapshot {
        job_id: job_id.clone(),
        status: "queued".into(),
        phase: "queued".into(),
        message: "Waiting for the MLX repair worker".into(),
        result: None,
        error: None,
    }));
    let cancel = Arc::new(Notify::new());
    {
        let mut jobs = JOBS
            .lock()
            .map_err(|_| anyhow!("repair job registry poisoned"))?;
        jobs.retain(|_, job| {
            job.snapshot
                .lock()
                .map(|snapshot| {
                    !matches!(
                        snapshot.status.as_str(),
                        "completed" | "failed" | "cancelled"
                    )
                })
                .unwrap_or(false)
        });
        if jobs.len() >= MAX_JOBS {
            bail!("too many retained MTP repair jobs");
        }
        jobs.insert(
            job_id.clone(),
            RuntimeJob {
                snapshot: snapshot.clone(),
                cancel: cancel.clone(),
            },
        );
    }

    let task_snapshot = snapshot.clone();
    tokio::spawn(async move {
        let permit = match JOB_GATE.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(error) => {
                set_failed(
                    &task_snapshot,
                    format!("repair worker unavailable: {error}"),
                );
                return;
            }
        };
        set_running(&task_snapshot, "inspecting", "Reading MTP tensor headers");
        let mut command = Command::new(&python);
        if let Some(bf16_source) = bf16_source {
            command
                .arg(script)
                .arg("--bf16-source")
                .arg(bf16_source)
                .arg("--mlx-model")
                .arg(&target)
                .arg("--out")
                .arg(&output_dir)
                .arg("--python")
                .arg(&python);
            if let Some(revision) = bf16_revision {
                command.arg("--revision").arg(revision);
            }
        } else {
            command
                .arg(script)
                .arg("repair")
                .arg("--target")
                .arg(&target);
            if let Some(source) = source {
                command.arg("--source").arg(source);
                command.arg("--source-format").arg(&source_format);
            }
            if let Some(recipe) = recipe {
                command.arg("--recipe").arg(recipe);
            }
            command.arg("--output").arg(&output);
        }
        let result = run_bounded(command, cancel.clone()).await;
        drop(permit);
        match result {
            Ok(stdout) => {
                let report = if stdout.trim().is_empty() {
                    std::fs::read_to_string(output_dir.join("provenance.json"))
                        .map_err(|error| error.to_string())
                } else {
                    Ok(stdout)
                };
                match report.and_then(|value| {
                    serde_json::from_str::<serde_json::Value>(&value)
                        .map_err(|error| error.to_string())
                }) {
                    Ok(value) => set_completed(&task_snapshot, value),
                    Err(error) => set_failed(
                        &task_snapshot,
                        format!("repair returned invalid JSON: {error}"),
                    ),
                }
            }
            Err(RunError::Cancelled) => set_cancelled(&task_snapshot),
            Err(RunError::Failed(error)) => set_failed(&task_snapshot, error),
        }
    });
    snapshot
        .lock()
        .map(|value| value.clone())
        .map_err(|_| anyhow!("repair job snapshot poisoned"))
}

pub fn get_job(job_id: &str) -> Option<RepairJobSnapshot> {
    JOBS.lock()
        .ok()?
        .get(job_id)?
        .snapshot
        .lock()
        .ok()
        .map(|value| value.clone())
}

pub fn list_jobs() -> Vec<RepairJobSnapshot> {
    JOBS.lock()
        .ok()
        .map(|jobs| {
            jobs.values()
                .filter_map(|job| job.snapshot.lock().ok().map(|v| v.clone()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn cancel_job(job_id: &str) -> Result<RepairJobSnapshot> {
    let jobs = JOBS
        .lock()
        .map_err(|_| anyhow!("repair job registry poisoned"))?;
    let job = jobs
        .get(job_id)
        .ok_or_else(|| anyhow!("repair job not found"))?;
    let current = job
        .snapshot
        .lock()
        .map_err(|_| anyhow!("repair job snapshot poisoned"))?
        .clone();
    if matches!(current.status.as_str(), "queued" | "running") {
        job.cancel.notify_waiters();
    }
    Ok(current)
}

pub fn list_sidecars() -> Result<Vec<SidecarEntry>> {
    discover_sidecars()
}

fn resolve_script(scripts_dir: &Path, name: &str) -> Result<PathBuf> {
    let configured = scripts_dir.join(name);
    if configured.is_file() {
        return Ok(configured);
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join(name);
    if repository.is_file() {
        return Ok(repository);
    }
    bail!(
        "Rapid-MLX repair script not installed: {}",
        configured.display()
    )
}

fn validate_bf16_source(raw: &str) -> Result<String> {
    if raw.trim().is_empty() || raw.chars().any(char::is_control) {
        bail!("bf16Source is invalid");
    }
    if Path::new(raw).is_absolute() {
        return Ok(existing_directory(raw, "bf16Source")?
            .to_string_lossy()
            .into_owned());
    }
    if !crate::hf::validate_hf_repo_id(raw.trim()) {
        bail!("bf16Source must be an absolute local directory or owner/repo");
    }
    Ok(raw.trim().to_string())
}

fn validate_revision(raw: &str) -> Result<String> {
    let value = raw.trim();
    if !(7..=128).contains(&value.len())
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        bail!("revision must be a stable 7-128 character identifier");
    }
    Ok(value.to_string())
}

fn existing_directory(raw: &str, label: &str) -> Result<PathBuf> {
    let path = validate_path(raw, label)?;
    let canonical = path
        .canonicalize()
        .with_context(|| format!("cannot resolve {label} path"))?;
    if !canonical.is_dir() {
        bail!("{label} must be a directory");
    }
    Ok(canonical)
}

fn existing_file(raw: &str, label: &str) -> Result<PathBuf> {
    let path = validate_path(raw, label)?;
    let canonical = path
        .canonicalize()
        .with_context(|| format!("cannot resolve {label} path"))?;
    if !canonical.is_file() {
        bail!("{label} must be a regular file");
    }
    Ok(canonical)
}

fn validate_path(raw: &str, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(raw.trim());
    if raw.trim().is_empty() || path.is_relative() || raw.chars().any(char::is_control) {
        bail!("{label} must be an absolute local path");
    }
    Ok(path)
}

fn validate_executable_name(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        bail!("python executable is invalid");
    }
    Ok(())
}

enum RunError {
    Cancelled,
    Failed(String),
}

async fn drain(mut reader: impl AsyncRead + Unpin) -> Vec<u8> {
    let mut output = Vec::new();
    let mut buffer = [0u8; 8192];
    while let Ok(count) = reader.read(&mut buffer).await {
        if count == 0 {
            break;
        }
        if output.len() < MAX_OUTPUT_BYTES {
            output.extend_from_slice(&buffer[..count.min(MAX_OUTPUT_BYTES - output.len())]);
        }
    }
    output
}

async fn run_bounded(
    mut command: Command,
    cancel: Arc<Notify>,
) -> std::result::Result<String, RunError> {
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| RunError::Failed(format!("could not start repair: {error}")))?;
    let stdout = child.stdout.take().map(drain);
    let stderr = child.stderr.take().map(drain);
    let Some(stdout) = stdout else {
        return Err(RunError::Failed("repair stdout unavailable".into()));
    };
    let Some(stderr) = stderr else {
        return Err(RunError::Failed("repair stderr unavailable".into()));
    };
    let stdout_task = tokio::spawn(stdout);
    let stderr_task = tokio::spawn(stderr);
    let mut wait_task = tokio::spawn(async move {
        tokio::time::timeout(REPAIR_TIMEOUT, child.wait())
            .await
            .map_err(|_| RunError::Failed("MTP repair timed out after two hours".into()))?
            .map_err(|error| RunError::Failed(format!("repair process wait failed: {error}")))
    });
    let status = tokio::select! {
        status = &mut wait_task => status.map_err(|error| RunError::Failed(format!("repair wait task failed: {error}")))??,
        _ = cancel.notified() => {
            // `kill_on_drop(true)` terminates the child when the wait task is
            // aborted. This avoids borrowing the same child through both
            // `wait()` and the cancellation arm.
            //
            // The task is intentionally detached after abort; its child is
            // owned by the task and is dropped immediately by Tokio.
            // (The handle is consumed by this select arm.)
            wait_task.abort();
            stdout_task.abort();
            stderr_task.abort();
            return Err(RunError::Cancelled);
        }
    };
    let stdout = stdout_task.await.unwrap_or_default();
    let stderr = stderr_task.await.unwrap_or_default();
    if !status.success() {
        let detail = String::from_utf8_lossy(&stderr)
            .trim()
            .chars()
            .take(2000)
            .collect::<String>();
        return Err(RunError::Failed(if detail.is_empty() {
            format!("repair process exited with {status}")
        } else {
            detail
        }));
    }
    Ok(String::from_utf8_lossy(&stdout).trim().to_string())
}

fn set_running(snapshot: &Arc<Mutex<RepairJobSnapshot>>, phase: &str, message: &str) {
    if let Ok(mut value) = snapshot.lock() {
        value.status = "running".into();
        value.phase = phase.into();
        value.message = message.into();
    }
}

fn set_completed(snapshot: &Arc<Mutex<RepairJobSnapshot>>, result: serde_json::Value) {
    if let Ok(mut value) = snapshot.lock() {
        value.status = "completed".into();
        value.phase = "completed".into();
        value.message =
            "MTP sidecar candidate created; served requalification is still required".into();
        value.result = Some(result);
    }
}

fn set_failed(snapshot: &Arc<Mutex<RepairJobSnapshot>>, error: String) {
    if let Ok(mut value) = snapshot.lock() {
        value.status = "failed".into();
        value.phase = "failed".into();
        value.message = "MTP sidecar repair failed; no usable candidate was registered".into();
        value.error = Some(error);
    }
}

fn set_cancelled(snapshot: &Arc<Mutex<RepairJobSnapshot>>) {
    if let Ok(mut value) = snapshot.lock() {
        value.status = "cancelled".into();
        value.phase = "cancelled".into();
        value.message = "MTP sidecar repair cancelled".into();
    }
}

#[allow(dead_code)]
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{validate_bf16_source, validate_executable_name, validate_path, validate_revision};

    #[test]
    fn repair_paths_are_absolute_and_non_control() {
        assert!(validate_path("relative/model", "target").is_err());
        assert!(validate_path("/tmp/model\n", "target").is_err());
        assert!(validate_path("/tmp/model", "target").is_ok());
    }

    #[test]
    fn repair_python_override_rejects_control_input() {
        assert!(validate_executable_name("python3\n").is_err());
        assert!(validate_executable_name("python3").is_ok());
    }

    #[test]
    fn bf16_hf_sources_require_valid_repo_and_revision_shape() {
        assert!(validate_bf16_source("nightmedia/brainwaves").is_ok());
        assert!(validate_bf16_source("not-a-repo").is_err());
        assert!(validate_revision("0123456789abcdef").is_ok());
        assert!(validate_revision("main").is_err());
    }
}
