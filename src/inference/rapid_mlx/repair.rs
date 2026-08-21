//! Bounded, local-source MTP sidecar repair jobs.
//!
//! The repair executable is deliberately kept outside Rust: MLX tensor
//! loading and NuSLERP must stay in the same Python environment as Rapid-MLX.
//! Rust owns the product boundary—path validation, lifecycle, cancellation,
//! output limits, and the managed sidecar destination.

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::{Notify, Semaphore};

use super::sidecar_inventory::{SidecarEntry, discover_sidecars, sidecar_dir_for_trunk};

/// Maximum number of queued, running, or recently completed jobs retained for
/// status inspection. Completed entries are pruned when a new job is queued.
const MAX_JOBS: usize = 16;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
/// Upper bound for one repair or requalification subprocess.
const REPAIR_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const SCRIPT_NAME: &str = "repair-mtp-mlx.py";
const VALIDATE_OPERATION: &str = "validate";
const REQUALIFY_OPERATION: &str = "requalify";
const REQUALIFY_SCRIPT_NAME: &str = "rapid-mlx-requalify-spec-decode.mjs";
const RECIPE_NAME: &str = "spec-decode-recipe.json";
const DEFAULT_REQUAL_TOKENS: u32 = 3;
const SCREEN_ESTIMATE_SECONDS: u64 = 5 * 60;
const FULL_ESTIMATE_SECONDS: u64 = 90 * 60;

fn default_operation() -> String {
    "repair".to_string()
}

/// Authenticated request describing one managed Rapid-MLX MTP operation.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairRequest {
    pub target: String,
    #[serde(default = "default_operation")]
    pub operation: String,
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
    /// Requested ceiling for the requalification lane. The observed K histogram
    /// remains authoritative and is intentionally not represented by this field.
    #[serde(default)]
    pub num_speculative_tokens: Option<u32>,
    #[serde(default)]
    pub disable_auto_k: bool,
    /// `screen` runs a short sampled live probe; `full` runs all qualification gates.
    #[serde(default)]
    pub requalification_mode: Option<String>,
}

/// Operator-facing state for a queued, running, or completed MTP job.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairJobSnapshot {
    pub job_id: String,
    pub status: String,
    pub phase: String,
    pub message: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    /// Unix start time and a bounded operator-facing estimate for long jobs.
    pub started_at_unix: u64,
    pub estimated_seconds: Option<u64>,
    pub completed_steps: u8,
    pub total_steps: u8,
}

struct RuntimeJob {
    snapshot: Arc<Mutex<RepairJobSnapshot>>,
    cancel: Arc<Notify>,
}

static JOBS: LazyLock<Mutex<BTreeMap<String, RuntimeJob>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static JOB_GATE: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));

/// Validate and enqueue a managed MTP repair, validation, or requalification job.
///
/// The returned snapshot is the initial queued state. The subprocess runs in a
/// background task, and callers should use [`get_job`] or [`list_jobs`] to
/// observe its terminal result. At most one MLX subprocess runs at a time;
/// [`MAX_JOBS`] bounds retained lifecycle state separately.
pub fn start_job(request: RepairRequest, scripts_dir: &Path) -> Result<RepairJobSnapshot> {
    let target = existing_directory(&request.target, "target")?;
    let operation = request.operation.trim().to_ascii_lowercase();
    if operation != "repair" && operation != VALIDATE_OPERATION && operation != REQUALIFY_OPERATION
    {
        bail!("operation must be repair, validate, or requalify");
    }
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
    if operation == VALIDATE_OPERATION {
        if source.is_some() || recipe.is_some() || bf16_source.is_some() || bf16_revision.is_some()
        {
            bail!("validation accepts only target and operation");
        }
    } else if operation == REQUALIFY_OPERATION {
        if source.is_some() || bf16_source.is_some() || bf16_revision.is_some() {
            bail!("requalification accepts only target, recipe, and depth settings");
        }
        if let Some(mode) = request.requalification_mode.as_deref()
            && !matches!(mode, "screen" | "full")
        {
            bail!("requalification mode must be screen or full");
        }
    } else if source.is_none() && recipe.is_none() && bf16_source.is_none() {
        bail!("one of source, recipe, or bf16Source is required");
    }
    if operation == "repair"
        && [source.is_some(), recipe.is_some(), bf16_source.is_some()]
            .into_iter()
            .filter(|present| *present)
            .count()
            > 1
    {
        bail!("source, recipe, and bf16Source are mutually exclusive");
    }
    if operation == "repair"
        && bf16_source
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
    let script_name = if operation == VALIDATE_OPERATION {
        SCRIPT_NAME
    } else if operation == REQUALIFY_OPERATION {
        REQUALIFY_SCRIPT_NAME
    } else if bf16_source.is_some() {
        "build-mtp-head.py"
    } else {
        SCRIPT_NAME
    };
    let script = resolve_script(scripts_dir, script_name)?;
    let python = request.python.unwrap_or_else(default_python);
    validate_executable_name(&python)?;
    let node = "node";
    let requal_recipe = if operation == REQUALIFY_OPERATION {
        Some(match recipe.clone() {
            Some(path) => path,
            None => resolve_script(scripts_dir, RECIPE_NAME)?,
        })
    } else {
        None
    };
    // Three draft tokens matches the short qualification recipe's conservative
    // default; the observed K histogram remains authoritative.
    let requal_tokens = request
        .num_speculative_tokens
        .unwrap_or(DEFAULT_REQUAL_TOKENS);
    let disable_auto_k = request.disable_auto_k;
    if operation == REQUALIFY_OPERATION && !(1..=8).contains(&requal_tokens) {
        bail!("numSpeculativeTokens must be between 1 and 8");
    }
    let requal_port = if operation == REQUALIFY_OPERATION {
        Some(ephemeral_port()?)
    } else {
        None
    };
    let job_id = format!("mtp-repair-{:032x}", rand::random::<u128>());
    let sidecar_dir = sidecar_dir_for_trunk(&target);
    let output_dir = if operation == REQUALIFY_OPERATION {
        let output = sidecar_dir.join("mtp.safetensors");
        if !output.is_file() {
            bail!(
                "no managed MTP sidecar exists for this trunk: {}",
                output.display()
            );
        }
        sidecar_dir.join(format!("requalification-{job_id}"))
    } else {
        sidecar_dir.clone()
    };
    let output = sidecar_dir.join("mtp.safetensors");
    if operation == VALIDATE_OPERATION {
        ensure_validation_candidate(&sidecar_dir, &output)?;
    }
    let requalification_mode = request
        .requalification_mode
        .clone()
        .unwrap_or_else(|| "full".to_string());
    let total_steps = if operation == REQUALIFY_OPERATION {
        if requalification_mode == "screen" {
            1
        } else {
            3
        }
    } else {
        1
    };
    let estimated_seconds = if operation == REQUALIFY_OPERATION {
        Some(if requalification_mode == "screen" {
            SCREEN_ESTIMATE_SECONDS
        } else {
            FULL_ESTIMATE_SECONDS
        })
    } else {
        None
    };
    let snapshot = Arc::new(Mutex::new(RepairJobSnapshot {
        job_id: job_id.clone(),
        status: "queued".into(),
        phase: "queued".into(),
        message: "Waiting for the MLX repair worker".into(),
        result: None,
        error: None,
        started_at_unix: unix_now(),
        estimated_seconds,
        completed_steps: 0,
        total_steps,
    }));
    let cancel = Arc::new(Notify::new());
    register_job(&job_id, snapshot.clone(), cancel.clone())?;

    let task_snapshot = snapshot.clone();
    let validation = operation == VALIDATE_OPERATION;
    let requalifying = operation == REQUALIFY_OPERATION;
    let operation_for_message = operation.clone();
    tokio::spawn(async move {
        let permit = tokio::select! {
            permit = JOB_GATE.clone().acquire_owned() => match permit {
            Ok(permit) => permit,
            Err(error) => {
                set_failed(
                    &task_snapshot,
                    format!("repair worker unavailable: {error}"),
                );
                return;
            }
        },
        _cancelled = cancel.notified() => {
                set_cancelled(&task_snapshot);
                return;
            }
        };
        if validation {
            set_running(
                &task_snapshot,
                "validating",
                "Checking MTP tensors and pre_fc_norm means",
            );
        } else if requalifying {
            set_running(
                &task_snapshot,
                "requalifying",
                "Running sampled, constrained, and parity gates",
            );
        } else {
            set_running(&task_snapshot, "inspecting", "Reading MTP tensor headers");
        }
        let command = match build_command(JobCommandSpec {
            operation: &operation_for_message,
            python: &python,
            node,
            script: &script,
            target: &target,
            source: source.as_deref(),
            source_format: &source_format,
            recipe: recipe.as_deref(),
            bf16_source: bf16_source.as_deref(),
            bf16_revision: bf16_revision.as_deref(),
            requal_recipe: requal_recipe.as_deref(),
            sidecar_dir: &sidecar_dir,
            output_dir: &output_dir,
            output: &output,
            requal_port,
            requal_tokens,
            requalification_mode: &requalification_mode,
            disable_auto_k,
        }) {
            Ok(command) => command,
            Err(error) => {
                drop(permit);
                set_failed(&task_snapshot, error.to_string());
                return;
            }
        };
        let result = run_bounded(
            command,
            cancel.clone(),
            sensitive_environment_values(),
            requalifying,
        )
        .await;
        drop(permit);
        match result {
            Ok(stdout) => {
                if requalifying {
                    let report_path = output_dir.join("requalification.json");
                    match ingest_requalification(&report_path, &sidecar_dir).await {
                        Ok(value) => set_completed(&task_snapshot, value, true),
                        Err(error) => set_failed(&task_snapshot, error.to_string()),
                    }
                    return;
                }
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
                    Ok(value) if validation => {
                        match promote_validated_sidecar(&output_dir, &output, value) {
                            Ok(promoted) => set_completed(&task_snapshot, promoted, false),
                            Err(error) => set_failed(&task_snapshot, error.to_string()),
                        }
                    }
                    Ok(value) => set_completed(&task_snapshot, value, false),
                    Err(error) => set_failed(
                        &task_snapshot,
                        format!("{operation_for_message} returned invalid JSON: {error}"),
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

/// Return a snapshot for one managed job, if it is still retained.
pub fn get_job(job_id: &str) -> Option<RepairJobSnapshot> {
    JOBS.lock()
        .ok()?
        .get(job_id)?
        .snapshot
        .lock()
        .ok()
        .map(|value| value.clone())
}

/// Return retained repair-job snapshots in job-id order.
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

/// Request cancellation of a queued or running job and return its current snapshot.
///
/// Cancellation is cooperative at the Rust boundary but terminates the managed
/// subprocess through `kill_on_drop(true)`. A latched `notify_one` signal makes
/// cancellation safe even when the worker has not acquired the single-job gate.
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
        job.cancel.notify_one();
    }
    Ok(current)
}

pub fn list_sidecars() -> Result<Vec<SidecarEntry>> {
    discover_sidecars()
}

fn register_job(
    job_id: &str,
    snapshot: Arc<Mutex<RepairJobSnapshot>>,
    cancel: Arc<Notify>,
) -> Result<()> {
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
    if jobs.contains_key(job_id) {
        bail!("could not allocate a unique repair job id; please retry");
    }
    jobs.insert(job_id.to_string(), RuntimeJob { snapshot, cancel });
    Ok(())
}

struct JobCommandSpec<'a> {
    operation: &'a str,
    python: &'a str,
    node: &'a str,
    script: &'a Path,
    target: &'a Path,
    source: Option<&'a Path>,
    source_format: &'a str,
    recipe: Option<&'a Path>,
    bf16_source: Option<&'a str>,
    bf16_revision: Option<&'a str>,
    requal_recipe: Option<&'a Path>,
    sidecar_dir: &'a Path,
    output_dir: &'a Path,
    output: &'a Path,
    requal_port: Option<u16>,
    requal_tokens: u32,
    requalification_mode: &'a str,
    disable_auto_k: bool,
}

fn build_command(spec: JobCommandSpec<'_>) -> Result<Command> {
    if spec.operation == VALIDATE_OPERATION {
        let mut command = Command::new(spec.python);
        command
            .arg(spec.script)
            .arg("validate")
            .arg("--sidecar")
            .arg(spec.output);
        return Ok(command);
    }

    if spec.operation == REQUALIFY_OPERATION {
        let recipe = spec
            .requal_recipe
            .ok_or_else(|| anyhow!("requalification recipe was not resolved"))?;
        let port = spec
            .requal_port
            .ok_or_else(|| anyhow!("requalification port was not allocated"))?;
        let mut command = Command::new(spec.node);
        command.arg(spec.script);
        if let Some(root) = spec.script.parent().and_then(Path::parent) {
            command.current_dir(root);
        }
        command
            .arg("--recipe")
            .arg(recipe)
            .arg("--model")
            .arg(spec.target)
            .arg("--speculative-model")
            .arg(spec.sidecar_dir)
            .arg("--out")
            .arg(spec.output_dir)
            .arg("--port")
            .arg(port.to_string())
            .arg("--num-speculative-tokens")
            .arg(spec.requal_tokens.to_string())
            .arg("--mode")
            .arg(spec.requalification_mode);
        if let Some(cache) = requalification_hf_cache() {
            command.arg("--hf-hub-cache").arg(cache);
        }
        if spec.disable_auto_k {
            command.arg("--disable-auto-k");
        }
        return Ok(command);
    }

    let mut command = Command::new(spec.python);
    if let Some(bf16_source) = spec.bf16_source {
        command
            .arg(spec.script)
            .arg("--bf16-source")
            .arg(bf16_source)
            .arg("--mlx-model")
            .arg(spec.target)
            .arg("--out")
            .arg(spec.output_dir)
            .arg("--python")
            .arg(spec.python);
        if let Some(revision) = spec.bf16_revision {
            command.arg("--revision").arg(revision);
        }
    } else {
        command
            .arg(spec.script)
            .arg("repair")
            .arg("--target")
            .arg(spec.target);
        if let Some(source) = spec.source {
            command
                .arg("--source")
                .arg(source)
                .arg("--source-format")
                .arg(spec.source_format);
        }
        if let Some(recipe) = spec.recipe {
            command.arg("--recipe").arg(recipe);
        }
        command.arg("--output").arg(spec.output);
    }
    Ok(command)
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

fn ensure_validation_candidate(output_dir: &Path, output: &Path) -> Result<()> {
    if !output.is_file() {
        bail!(
            "no managed MTP sidecar exists for this trunk: {}",
            output.display()
        );
    }
    let provenance_path = output_dir.join("provenance.json");
    let raw = std::fs::read_to_string(&provenance_path).with_context(|| {
        format!(
            "managed MTP sidecar is missing provenance: {}",
            provenance_path.display()
        )
    })?;
    let provenance: Value = serde_json::from_str(&raw).with_context(|| {
        format!(
            "managed MTP sidecar provenance is invalid: {}",
            provenance_path.display()
        )
    })?;
    let top_status = provenance.get("status").and_then(Value::as_str);
    let requalification_status = provenance
        .get("requalification")
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str);
    if top_status == Some("qualified") || requalification_status == Some("qualified") {
        bail!("refusing to overwrite qualified MTP provenance; requalification already passed");
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("cannot read sidecar digest: {}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = std::io::Read::read(&mut file, &mut buffer)
            .with_context(|| format!("cannot hash sidecar: {}", path.display()))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn promote_validated_sidecar(output_dir: &Path, output: &Path, report: Value) -> Result<Value> {
    if report.get("status").and_then(Value::as_str) != Some("validated") {
        bail!("validator did not return a validated report");
    }
    let validation = report
        .get("validation")
        .cloned()
        .ok_or_else(|| anyhow!("validator returned no validation report"))?;
    let all_positive = validation
        .get("all_positive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let means = validation
        .get("pre_fc_norm_means")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("validator returned no pre_fc_norm means"))?;
    if !all_positive
        || means.is_empty()
        || means.values().any(|value| {
            value
                .as_f64()
                .is_none_or(|mean| !mean.is_finite() || mean <= 0.0)
        })
    {
        bail!("validator returned a failed pre_fc_norm sign check");
    }
    let digest = sha256_file(output)?;
    if report.get("sha256").and_then(Value::as_str) != Some(digest.as_str()) {
        bail!("sidecar changed while validation was running; refusing promotion");
    }

    let provenance_path = output_dir.join("provenance.json");
    let raw = std::fs::read_to_string(&provenance_path).with_context(|| {
        format!(
            "cannot read sidecar provenance: {}",
            provenance_path.display()
        )
    })?;
    let mut provenance: Value = serde_json::from_str(&raw)
        .with_context(|| format!("invalid sidecar provenance: {}", provenance_path.display()))?;
    let object = provenance
        .as_object_mut()
        .ok_or_else(|| anyhow!("sidecar provenance must be a JSON object"))?;
    let requalification_status = object
        .get("requalification")
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str);
    if object.get("status").and_then(Value::as_str) == Some("qualified")
        || requalification_status == Some("qualified")
    {
        bail!("refusing to overwrite qualified MTP provenance");
    }
    object.insert("status".into(), Value::String("candidate".into()));
    object.insert("norm_check_passed".into(), Value::Bool(true));
    object.insert("sha256".into(), Value::String(digest));
    object.insert(
        "estimated_memory_bytes".into(),
        Value::Number(
            output
                .metadata()
                .with_context(|| format!("cannot stat sidecar: {}", output.display()))?
                .len()
                .into(),
        ),
    );
    let mut validation = validation;
    validation["status"] = Value::String("passed".into());
    object.insert("validation".into(), validation);
    object.insert("validated_at_unix".into(), Value::Number(unix_now().into()));

    let temporary = output_dir.join("provenance.json.validation-tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(&provenance)?)?;
    if let Err(error) = std::fs::rename(&temporary, &provenance_path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error).with_context(|| {
            format!(
                "cannot atomically promote sidecar provenance: {}",
                provenance_path.display()
            )
        });
    }
    Ok(provenance)
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

fn default_python() -> String {
    for executable in ["rapid-mlx", "vllm-mlx"] {
        if let Some(binary) = find_on_path(executable)
            && let Some(python) = python_for_binary(&binary)
        {
            return python.to_string_lossy().into_owned();
        }
    }
    "python3".to_string()
}

fn find_on_path(executable: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let candidate = dir.join(executable);
        candidate.is_file().then_some(candidate)
    })
}

fn python_for_binary(binary: &Path) -> Option<PathBuf> {
    if let Ok(first_line) = std::fs::read_to_string(binary)
        .map(|value| value.lines().next().unwrap_or_default().to_string())
        && let Some(interpreter) = first_line.strip_prefix("#!")
        && Path::new(interpreter.trim()).is_file()
    {
        return Some(PathBuf::from(interpreter.trim()));
    }
    let parent = binary.parent()?;
    if let Some(candidate) = ["python3", "python"]
        .iter()
        .map(|name| parent.join(name))
        .find(|candidate| candidate.is_file())
    {
        return Some(candidate);
    }
    parent.parent().and_then(|root| {
        ["python3", "python"]
            .iter()
            .map(|name| root.join("bin").join(name))
            .find(|candidate| candidate.is_file())
    })
}

fn sensitive_environment_values() -> Vec<String> {
    std::env::vars()
        .filter_map(|(name, value)| {
            let upper = name.to_ascii_uppercase();
            (upper.contains("TOKEN") || upper.contains("KEY") || upper.contains("SECRET"))
                .then_some(value)
        })
        .filter(|value| !value.is_empty())
        .collect()
}

enum RunError {
    Cancelled,
    Failed(String),
}

fn ephemeral_port() -> Result<u16> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .context("could not allocate a local requalification port")?;
    Ok(listener
        .local_addr()
        .context("could not read the local requalification port")?
        .port())
}

/// Use an existing external HF cache for qualification until the explicit model-root
/// migration moves it under Foundry's managed models directory. This is read-only: the
/// requalification lane never downloads into or mutates the external cache.
fn requalification_hf_cache() -> Option<PathBuf> {
    std::env::var_os("HF_HUB_CACHE")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .or_else(|| {
            dirs::home_dir()
                .map(|home| home.join(".cache/huggingface/hub"))
                .filter(|path| path.is_dir())
        })
}

async fn ingest_requalification(report_path: &Path, sidecar_dir: &Path) -> Result<Value> {
    if !report_path.is_file() {
        bail!(
            "requalification completed without a report: {}",
            report_path.display()
        );
    }
    let snapshot = super::capabilities::generate_snapshot_from_discovery().await?;
    let verdict = super::spec_decode_store::process_store()
        .ingest_requalification_report(&snapshot, report_path)?;
    let outcome = serde_json::to_value(verdict.outcome)?;
    let reason = if matches!(
        verdict.outcome,
        super::spec_decode_store::SpecDecodeOutcome::Qualified
    ) {
        "All requalification gates passed.".to_string()
    } else {
        verdict.reason()
    };
    let mut result = json!({
        "outcome": outcome,
        "promotesCapability": verdict.promotes_capability,
        "reason": reason,
        "rapidMlxVersion": verdict.rapid_mlx_version.clone(),
        "numSpeculativeTokens": verdict.num_speculative_tokens,
        "disableAutoK": verdict.disable_auto_k,
        "gatesRun": verdict.gates_run.clone(),
        "report": report_path,
    });
    if let Err(error) = update_requalification_provenance(sidecar_dir, &verdict, report_path) {
        result["provenanceWarning"] = Value::String(error.to_string());
    }
    Ok(result)
}

fn update_requalification_provenance(
    sidecar_dir: &Path,
    verdict: &super::spec_decode_store::MeasuredSpecDecode,
    report_path: &Path,
) -> Result<()> {
    let path = sidecar_dir.join("provenance.json");
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read sidecar provenance: {}", path.display()))?;
    let mut provenance: Value = serde_json::from_str(&raw)
        .with_context(|| format!("invalid sidecar provenance: {}", path.display()))?;
    let object = provenance
        .as_object_mut()
        .ok_or_else(|| anyhow!("sidecar provenance must be a JSON object"))?;
    let status = match verdict.outcome {
        super::spec_decode_store::SpecDecodeOutcome::Screened => "screened",
        super::spec_decode_store::SpecDecodeOutcome::Qualified => "qualified",
        super::spec_decode_store::SpecDecodeOutcome::StillBlocked => "still-blocked",
        super::spec_decode_store::SpecDecodeOutcome::Uninterpretable => "uninterpretable",
    };
    object.insert(
        "requalification".into(),
        json!({
            "status": status,
            "rapid_mlx_version": verdict.rapid_mlx_version.clone(),
            "num_speculative_tokens": verdict.num_speculative_tokens,
            "disable_auto_k": verdict.disable_auto_k,
            "measured_at": verdict.measured_at.clone(),
            "report": report_path,
            "reason": verdict.reason(),
        }),
    );
    let temporary = sidecar_dir.join("provenance.json.requalification-tmp");
    std::fs::write(
        &temporary,
        format!("{}\n", serde_json::to_string_pretty(&provenance)?),
    )?;
    if let Err(error) = std::fs::rename(&temporary, &path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error)
            .with_context(|| format!("cannot update sidecar provenance: {}", path.display()));
    }
    Ok(())
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
    secrets: Vec<String>,
    allow_still_blocked: bool,
) -> std::result::Result<String, RunError> {
    // Create the notification future before spawning. `cancel_job` uses
    // `notify_one`, which retains one permit when cancellation wins the race
    // with worker startup.
    let cancel_signal = cancel.notified();
    tokio::pin!(cancel_signal);
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
        _ = &mut cancel_signal => {
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
    if !status.success() && (!allow_still_blocked || status.code() != Some(20)) {
        let mut detail = String::from_utf8_lossy(&stderr).into_owned();
        for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
            detail = detail.replace(secret, "[REDACTED]");
        }
        let detail = detail.trim().chars().take(2000).collect::<String>();
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

fn set_completed(
    snapshot: &Arc<Mutex<RepairJobSnapshot>>,
    result: serde_json::Value,
    requalification: bool,
) {
    if let Ok(mut value) = snapshot.lock() {
        value.status = "completed".into();
        value.phase = "completed".into();
        value.message = if requalification {
            "MTP sidecar requalification completed; inspect the recorded outcome".into()
        } else {
            "MTP sidecar candidate created; served requalification is still required".into()
        };
        value.completed_steps = value.total_steps;
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
    use super::{
        JobCommandSpec, RepairRequest, RunError, build_command, promote_validated_sidecar,
        run_bounded, sha256_file, validate_bf16_source, validate_executable_name, validate_path,
        validate_revision,
    };
    use serde_json::json;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Notify;

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

    #[test]
    fn repair_request_defaults_to_repair_and_accepts_validation() {
        let repair: RepairRequest = serde_json::from_value(json!({"target":"/tmp/trunk"})).unwrap();
        assert_eq!(repair.operation, "repair");

        let validate: RepairRequest = serde_json::from_value(json!({
            "target":"/tmp/trunk",
            "operation":"validate"
        }))
        .unwrap();
        assert_eq!(validate.operation, "validate");
    }

    #[test]
    fn requalification_request_carries_safe_depth_settings() {
        let request: RepairRequest = serde_json::from_value(json!({
            "target": "/tmp/trunk",
            "operation": "requalify",
            "numSpeculativeTokens": 3,
            "disableAutoK": true
        }))
        .unwrap();
        assert_eq!(request.operation, "requalify");
        assert_eq!(request.num_speculative_tokens, Some(3));
        assert!(request.disable_auto_k);
    }

    #[test]
    fn command_builder_keeps_validation_on_the_python_lane() {
        let command = build_command(JobCommandSpec {
            operation: "validate",
            python: "python3",
            node: "node",
            script: Path::new("/tmp/repair-mtp-mlx.py"),
            target: Path::new("/tmp/model"),
            source: None,
            source_format: "mlx",
            recipe: None,
            bf16_source: None,
            bf16_revision: None,
            requal_recipe: None,
            sidecar_dir: Path::new("/tmp/sidecar"),
            output_dir: Path::new("/tmp/output"),
            output: Path::new("/tmp/sidecar/mtp.safetensors"),
            requal_port: None,
            requal_tokens: 3,
            requalification_mode: "screen",
            disable_auto_k: false,
        })
        .unwrap();
        assert_eq!(command.as_std().get_program(), "python3");
        let args: Vec<_> = command.as_std().get_args().collect();
        assert_eq!(args[0], "/tmp/repair-mtp-mlx.py");
        assert_eq!(args[1], "validate");
        assert_eq!(args[2], "--sidecar");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bounded_process_honors_cancellation() {
        let mut command = tokio::process::Command::new("sh");
        command.args(["-c", "sleep 30"]);
        let cancel = Arc::new(Notify::new());
        let task = tokio::spawn(run_bounded(command, cancel.clone(), Vec::new(), false));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        cancel.notify_one();
        assert!(matches!(task.await.unwrap(), Err(RunError::Cancelled)));
    }

    #[test]
    fn successful_validation_promotes_pending_provenance() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("mtp.safetensors");
        std::fs::write(&output, b"sidecar fixture").unwrap();
        let provenance = directory.path().join("provenance.json");
        std::fs::write(
            &provenance,
            serde_json::to_vec(&json!({
                "schema_version": 2,
                "status": "candidate",
                "repair_mode": "relocation",
                "norm_check_passed": false
            }))
            .unwrap(),
        )
        .unwrap();
        let digest = sha256_file(&output).unwrap();
        let promoted = promote_validated_sidecar(
            directory.path(),
            &output,
            json!({
                "status": "validated",
                "sha256": digest,
                "validation": {
                    "all_positive": true,
                    "pre_fc_norm_means": {
                        "pre_fc_norm_embedding.weight": 0.5,
                        "pre_fc_norm_hidden.weight": 0.6
                    }
                }
            }),
        )
        .unwrap();
        assert_eq!(promoted["status"], "candidate");
        assert_eq!(promoted["repair_mode"], "relocation");
        assert_eq!(promoted["norm_check_passed"], true);
        assert_eq!(promoted["validation"]["status"], "passed");
    }

    #[test]
    fn failed_validation_does_not_promote_provenance() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("mtp.safetensors");
        std::fs::write(&output, b"corrupt sidecar fixture").unwrap();
        let provenance = directory.path().join("provenance.json");
        let original = json!({
            "schema_version": 2,
            "status": "pending",
            "repair_mode": "relocation",
            "norm_check_passed": false
        });
        std::fs::write(&provenance, serde_json::to_vec(&original).unwrap()).unwrap();
        let result = promote_validated_sidecar(
            directory.path(),
            &output,
            json!({
                "status": "validated",
                "sha256": sha256_file(&output).unwrap(),
                "validation": {
                    "all_positive": false,
                    "pre_fc_norm_means": {
                        "pre_fc_norm_embedding.weight": -0.4,
                        "pre_fc_norm_hidden.weight": -0.4
                    }
                }
            }),
        );
        assert!(result.is_err());
        let unchanged: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&provenance).unwrap()).unwrap();
        assert_eq!(unchanged, original);
    }
}
