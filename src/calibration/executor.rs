//! Bounded single-trial Calibration executor.
//!
//! This is deliberately narrower than the final Quick/Balanced search. It
//! proves the lifecycle boundary first: preflight, one managed sibling
//! `llama-bench` run, durable journal transitions, cancellation, and a
//! redacted receipt. It never stops an active server or mutates a preset.

use super::candidates::{balanced_candidates, quick_candidates};
use super::jobs::{
    JournalEvent, JournalEventKind, append_event, mark_recovered_crash, read_events,
    recover_snapshot, suspected_crash_trials, write_snapshot,
};
use super::paths::{RegularFileError, require_regular_file};
use super::{
    CalibrationApplyRecord, CalibrationBudget, CalibrationCandidate, CalibrationCandidateResult,
    CalibrationFingerprint, CalibrationJobSnapshot, CalibrationJobState, CalibrationMeasurement,
    CalibrationReceipt, CalibrationWorkload, LlamaCppCalibrationPatch, StartCalibrationRequest,
    TrialStatus,
};
use crate::config::AppConfig;
use crate::inference::InferenceBackend;
use crate::llama::bench_runner::{SweepPoint, llama_bench_path, run_sweep_with_tokens};
use crate::presets::{self, ModelPreset};
use crate::state::AppState;
use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Notify, Semaphore};

const METHOD_VERSION: &str = "calibration-v1-single-trial";
const CONFIRMATION: &str = "CALIBRATE";
const MAX_DIAGNOSTICS: usize = 24;

static JOBS: LazyLock<Mutex<BTreeMap<String, RuntimeJob>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static JOB_GATE: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));

#[derive(Clone)]
struct RuntimeJob {
    snapshot: Arc<Mutex<CalibrationJobSnapshot>>,
    cancel: Arc<AtomicBool>,
    cancel_notify: Arc<Notify>,
    snapshot_path: PathBuf,
    journal_path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CalibrationPreflight {
    pub preset_id: String,
    pub preset_fingerprint: String,
    pub backend: InferenceBackend,
    pub model_identity: String,
    pub server_identity: String,
    pub bench_identity: String,
    pub planned_trials: u32,
    pub candidate_ids: Vec<String>,
    pub requires_server_stop: bool,
    pub supported_budget: &'static str,
    pub requested_budget: CalibrationBudget,
    pub confirmation: &'static str,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct CalibrationPreflightRequest {
    pub preset_id: String,
    pub workload: CalibrationWorkload,
    pub budget: CalibrationBudget,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct ApplyCalibrationRequest {
    pub target_preset_id: String,
    pub expected_target_fingerprint: String,
    pub candidate_id: Option<String>,
    #[serde(default = "default_create_derived")]
    pub create_derived: bool,
    pub exact_confirmation: Option<String>,
    #[serde(default = "default_validate_after_apply")]
    pub validate_after_apply: bool,
}

fn default_create_derived() -> bool {
    true
}

fn default_validate_after_apply() -> bool {
    true
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplyCalibrationResult {
    pub preset_id: String,
    pub derived: bool,
    pub candidate_id: String,
    pub before_fingerprint: String,
    pub after_fingerprint: String,
    pub validation: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct RollbackCalibrationRequest {
    pub expected_target_fingerprint: String,
    pub exact_confirmation: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RollbackCalibrationResult {
    pub preset_id: String,
    pub before_fingerprint: String,
    pub after_fingerprint: String,
}

pub fn confirmation_phrase() -> &'static str {
    CONFIRMATION
}

pub fn preset_fingerprint(preset: &ModelPreset) -> Result<String> {
    let encoded = serde_json::to_vec(preset).context("serialize preset fingerprint")?;
    let digest = Sha256::digest(encoded);
    let encoded_digest = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!("sha256:{encoded_digest}"))
}

pub fn preflight(
    config: &AppConfig,
    state: &AppState,
    preset_id: &str,
    workload: &CalibrationWorkload,
    budget: CalibrationBudget,
) -> Result<CalibrationPreflight> {
    let preset = find_preset(state, preset_id)?;
    if preset.backend != InferenceBackend::LlamaCpp {
        bail!("Calibration v1 supports llama.cpp presets only");
    }
    if *state.local_server_running.lock().unwrap() || *state.server_running.lock().unwrap() {
        bail!("Stop the active inference server before starting Calibration");
    }

    let model = resolve_model_path(config, &preset.model_path)?;
    let bench = llama_bench_path(&config.llama_server_path);
    if !bench.is_file() {
        bail!("Managed llama-bench is unavailable beside the configured llama-server");
    }

    let fingerprint = preset_fingerprint(&preset)?;
    validate_workload(workload)?;
    let candidates = match &budget {
        CalibrationBudget::Quick => quick_candidates(&preset, workload, None)?,
        CalibrationBudget::Balanced => balanced_candidates(&preset, workload, None)?,
        CalibrationBudget::Thorough => {
            bail!("Thorough Calibration is not available in the bounded 2.0 release")
        }
    };
    let candidate_ids = candidates
        .into_iter()
        .map(|candidate| candidate.id)
        .collect::<Vec<_>>();
    Ok(CalibrationPreflight {
        preset_id: preset.id.clone(),
        preset_fingerprint: fingerprint,
        backend: preset.backend,
        model_identity: library_relative_identity(config, &model),
        server_identity: config
            .llama_server_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "llama-server".into()),
        bench_identity: bench
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "llama-bench".into()),
        planned_trials: candidate_ids.len() as u32,
        candidate_ids,
        requires_server_stop: true,
        supported_budget: match budget {
            CalibrationBudget::Quick => "quick_single_trial",
            CalibrationBudget::Balanced => "balanced_bounded_plan",
            CalibrationBudget::Thorough => unreachable!(),
        },
        requested_budget: budget,
        confirmation: CONFIRMATION,
    })
}

pub fn start(
    config: Arc<AppConfig>,
    state: AppState,
    request: StartCalibrationRequest,
) -> Result<CalibrationJobSnapshot> {
    match request.budget {
        CalibrationBudget::Quick => {}
        CalibrationBudget::Balanced => bail!(
            "Balanced Calibration planning is available, but executor repetitions and pick verification are still gated"
        ),
        CalibrationBudget::Thorough => {
            bail!("Thorough Calibration is not available in the bounded 2.0 release")
        }
    }
    if request.exact_confirmation.as_deref() != Some(CONFIRMATION) {
        bail!("Exact confirmation CALIBRATE is required to start the bounded trial");
    }
    if request.allow_stop_active_server {
        bail!("Calibration cannot stop or restart an active server yet");
    }
    validate_workload(&request.workload)?;

    let preset = find_preset(&state, &request.preset_id)?;
    if preset.backend != InferenceBackend::LlamaCpp {
        bail!("Calibration v1 supports llama.cpp presets only");
    }
    let fingerprint = preset_fingerprint(&preset)?;
    if !request.expected_preset_fingerprint.is_empty()
        && request.expected_preset_fingerprint != fingerprint
    {
        bail!("Preset changed since preflight; refresh before starting Calibration");
    }
    let preflight = preflight(
        &config,
        &state,
        &request.preset_id,
        &request.workload,
        request.budget.clone(),
    )?;
    let candidates = match &request.budget {
        CalibrationBudget::Quick => quick_candidates(&preset, &request.workload, None)?,
        CalibrationBudget::Balanced => balanced_candidates(&preset, &request.workload, None)?,
        CalibrationBudget::Thorough => unreachable!(),
    };
    let model_path = resolve_model_path(&config, &preset.model_path)?;
    let bench_path = llama_bench_path(&config.llama_server_path);
    let id = crate::config::generate_random_token()
        .chars()
        .take(24)
        .collect::<String>();
    let job_dir = config.app_paths.calibration_jobs_dir().join(&id);
    let snapshot_path = job_dir.join("snapshot.json");
    let journal_path = job_dir.join("journal.jsonl");
    let snapshot = CalibrationJobSnapshot {
        id: id.clone(),
        state: CalibrationJobState::Queued,
        phase: "queued".into(),
        completed_trials: 0,
        planned_trials: preflight.planned_trials,
        diagnostics: vec!["Preset and active session remain unchanged during the trial".into()],
        receipt_id: None,
    };
    write_snapshot(&snapshot_path, &snapshot)?;
    append_event(
        &journal_path,
        &JournalEvent::new(JournalEventKind::JobCreated, None),
    )?;

    let runtime = RuntimeJob {
        snapshot: Arc::new(Mutex::new(snapshot.clone())),
        cancel: Arc::new(AtomicBool::new(false)),
        cancel_notify: Arc::new(Notify::new()),
        snapshot_path,
        journal_path,
    };
    {
        let mut jobs = JOBS
            .lock()
            .map_err(|_| anyhow!("Calibration registry unavailable"))?;
        if jobs.values().any(|job| {
            job.snapshot.lock().ok().is_some_and(|snapshot| {
                matches!(
                    snapshot.state,
                    CalibrationJobState::Queued
                        | CalibrationJobState::Running
                        | CalibrationJobState::Cancelling
                )
            })
        }) {
            bail!("Another Calibration job is already active");
        }
        jobs.insert(id.clone(), runtime.clone());
    }

    tokio::spawn(run_job(
        id,
        config,
        preset,
        request.workload,
        candidates,
        model_path,
        bench_path,
        fingerprint,
        request.budget,
        runtime,
    ));
    Ok(snapshot)
}

pub fn get(config: &AppConfig, id: &str) -> Result<Option<CalibrationJobSnapshot>> {
    if let Some(runtime) = JOBS
        .lock()
        .map_err(|_| anyhow!("Calibration registry unavailable"))?
        .get(id)
        .cloned()
    {
        return Ok(runtime
            .snapshot
            .lock()
            .ok()
            .map(|snapshot| snapshot.clone()));
    }
    let job_dir = config.app_paths.calibration_jobs_dir().join(id);
    let path = job_dir.join("snapshot.json");
    let Some(mut snapshot) = recover_snapshot(&path)? else {
        return Ok(None);
    };
    let journal = read_events(&job_dir.join("journal.jsonl"))?;
    let suspected = suspected_crash_trials(&journal);
    if !suspected.is_empty() {
        mark_recovered_crash(&mut snapshot, suspected.len());
        write_snapshot(&path, &snapshot)?;
    }
    Ok(Some(snapshot))
}

pub fn get_receipt(config: &AppConfig, id: &str) -> Result<Option<CalibrationReceipt>> {
    let path = config
        .app_paths
        .calibration_receipts_dir()
        .join(format!("{id}.json"));
    if !path.exists() {
        return Ok(None);
    }
    let encoded = fs::read(path)?;
    Ok(Some(
        serde_json::from_slice(&encoded).context("Calibration receipt is invalid")?,
    ))
}

pub fn cancel(config: &AppConfig, id: &str) -> Result<Option<CalibrationJobSnapshot>> {
    let runtime = JOBS
        .lock()
        .map_err(|_| anyhow!("Calibration registry unavailable"))?
        .get(id)
        .cloned();
    let Some(runtime) = runtime else {
        return get(config, id);
    };
    runtime.cancel.store(true, Ordering::Release);
    runtime.cancel_notify.notify_waiters();
    update_snapshot(&runtime, |snapshot| {
        if matches!(
            snapshot.state,
            CalibrationJobState::Queued | CalibrationJobState::Running
        ) {
            snapshot.state = CalibrationJobState::Cancelling;
            snapshot.phase = "cancelling".into();
        }
    })?;
    Ok(runtime
        .snapshot
        .lock()
        .ok()
        .map(|snapshot| snapshot.clone()))
}

#[allow(clippy::too_many_arguments)]
async fn run_job(
    id: String,
    config: Arc<AppConfig>,
    preset: ModelPreset,
    workload: super::CalibrationWorkload,
    candidates: Vec<CalibrationCandidate>,
    model_path: PathBuf,
    bench_path: PathBuf,
    fingerprint: String,
    budget: CalibrationBudget,
    runtime: RuntimeJob,
) {
    let _permit = match JOB_GATE.clone().acquire_owned().await {
        Ok(permit) => permit,
        Err(_) => {
            let _ = fail_job(&runtime, "Calibration executor gate is unavailable");
            return;
        }
    };
    if runtime.cancel.load(Ordering::Acquire) {
        let _ = finish_cancelled(&runtime);
        return;
    }
    if let Err(error) = transition(&runtime, CalibrationJobState::Running, "trial") {
        let _ = fail_job(
            &runtime,
            &format!("Calibration journal could not be prepared: {error}"),
        );
        return;
    }

    let mut candidate_results = Vec::new();
    for candidate in candidates {
        if runtime.cancel.load(Ordering::Acquire) {
            let _ = append_event(
                &runtime.journal_path,
                &JournalEvent::new(JournalEventKind::JobCancelled, Some(candidate.id)),
            );
            let _ = finish_cancelled(&runtime);
            return;
        }
        let candidate_id = candidate.id.clone();
        if let Err(error) = append_event(
            &runtime.journal_path,
            &JournalEvent::new(JournalEventKind::TrialPlanned, Some(candidate_id.clone())),
        )
        .and_then(|_| {
            append_event(
                &runtime.journal_path,
                &JournalEvent::new(JournalEventKind::TrialStarted, Some(candidate_id.clone())),
            )
        }) {
            let _ = fail_job(
                &runtime,
                &format!("Calibration journal could not be prepared: {error}"),
            );
            return;
        }
        let mut candidate_preset = preset.clone();
        apply_patch_to_preset(&mut candidate_preset, &candidate.typed_patch);
        let ngl = candidate_preset.gpu_layers.unwrap_or(99);
        let flash_attn = matches!(candidate_preset.flash_attn.trim(), "on" | "1" | "true");
        let ctk = non_empty_or(&candidate_preset.ctk, "q8_0");
        let ctv = non_empty_or(&candidate_preset.ctv, "q8_0");
        let batch_size = if candidate_preset.batch_size == 0 {
            2048
        } else {
            candidate_preset.batch_size
        };
        let ubatch_size = if candidate_preset.ubatch_size == 0 {
            512
        } else {
            candidate_preset.ubatch_size
        };
        let depths = vec![workload.minimum_context.max(1)];
        let model_path_string = model_path.to_string_lossy().into_owned();
        let bench = run_sweep_with_tokens(
            &bench_path,
            &config.llama_server_cwd,
            &model_path_string,
            ngl,
            flash_attn,
            &ctk,
            &ctv,
            batch_size,
            ubatch_size,
            &depths,
            candidate_preset.n_cpu_moe,
            workload.prompt_tokens,
            workload.generation_tokens,
        );
        let result = tokio::select! {
            result = bench => Some(result),
            _ = runtime.cancel_notify.notified() => None,
        };
        if runtime.cancel.load(Ordering::Acquire) {
            let _ = finish_cancelled(&runtime);
            return;
        }
        let measurement = match result {
            Some(Ok(points)) => measurement_from_points(points),
            Some(Err(error)) => CalibrationMeasurement {
                trial_id: candidate_id.clone(),
                status: Some(TrialStatus::Error),
                bounded_diagnostics: vec![sanitize_error(&error)],
                ..Default::default()
            },
            None => {
                let _ = finish_cancelled(&runtime);
                return;
            }
        };
        candidate_results.push(CalibrationCandidateResult {
            candidate,
            measurement,
        });
        let _ = append_event(
            &runtime.journal_path,
            &JournalEvent::new(JournalEventKind::TrialFinished, Some(candidate_id)),
        );
        let _ = update_snapshot(&runtime, |snapshot| {
            snapshot.completed_trials = snapshot.completed_trials.saturating_add(1);
        });
    }

    let completed_trials = candidate_results.len() as u32;
    let selected_candidate = select_winner(&candidate_results);
    let measurement = candidate_results
        .iter()
        .find(|result| result.candidate.id == "baseline")
        .map(|result| result.measurement.clone())
        .unwrap_or_default();
    let receipt = CalibrationReceipt {
        schema_version: super::CALIBRATION_SCHEMA_VERSION,
        method_version: METHOD_VERSION.into(),
        job_id: id.clone(),
        fingerprint: CalibrationFingerprint {
            baseline_config_hash: fingerprint.clone(),
            workload,
            ..CalibrationFingerprint::current(InferenceBackend::LlamaCpp, Default::default())
        },
        measurement,
        budget,
        candidate_results,
        selected_candidate,
        preset_id: preset.id.clone(),
        preset_fingerprint: fingerprint.clone(),
        apply_history: Vec::new(),
    };
    let receipt_path = config
        .app_paths
        .calibration_receipts_dir()
        .join(format!("{id}.json"));
    if let Err(error) = write_receipt(&receipt_path, &receipt) {
        let _ = fail_job(
            &runtime,
            &format!(
                "Calibration receipt could not be written: {}",
                sanitize_error(&error)
            ),
        );
        return;
    }
    let _ = update_snapshot(&runtime, |snapshot| {
        snapshot.state = CalibrationJobState::Complete;
        snapshot.phase = "complete".into();
        snapshot.completed_trials = completed_trials;
        snapshot.receipt_id = Some(id);
    });
}

const APPLY_CONFIRMATION: &str = "APPLY_CALIBRATION";
const ROLLBACK_CONFIRMATION: &str = "ROLLBACK_CALIBRATION";

pub fn apply_confirmation_phrase() -> &'static str {
    APPLY_CONFIRMATION
}

pub fn rollback_confirmation_phrase() -> &'static str {
    ROLLBACK_CONFIRMATION
}

pub fn apply(
    config: &AppConfig,
    state: &AppState,
    job_id: &str,
    request: ApplyCalibrationRequest,
) -> Result<ApplyCalibrationResult> {
    if request.exact_confirmation.as_deref() != Some(APPLY_CONFIRMATION) {
        bail!("Exact confirmation APPLY_CALIBRATION is required");
    }
    let receipt_path = config
        .app_paths
        .calibration_receipts_dir()
        .join(format!("{job_id}.json"));
    let receipt_bytes =
        fs::read(&receipt_path).map_err(|_| anyhow!("Calibration receipt not found"))?;
    let mut receipt: CalibrationReceipt =
        serde_json::from_slice(&receipt_bytes).context("Calibration receipt is invalid")?;
    let candidate_id = request
        .candidate_id
        .clone()
        .or_else(|| receipt.selected_candidate.clone())
        .ok_or_else(|| anyhow!("Calibration receipt has no measured winner"))?;
    let candidate = receipt
        .candidate_results
        .iter()
        .find(|result| result.candidate.id == candidate_id)
        .ok_or_else(|| anyhow!("Calibration candidate not found"))?;
    if candidate.measurement.status != Some(TrialStatus::Ok) {
        bail!("Only a valid measured Calibration candidate may be applied");
    }
    let source_id = if request.target_preset_id.is_empty() {
        receipt.preset_id.clone()
    } else {
        request.target_preset_id.clone()
    };
    let mut presets = state
        .presets
        .lock()
        .map_err(|_| anyhow!("Preset store unavailable"))?
        .clone();
    let original_presets = presets.clone();
    let source_index = presets
        .iter()
        .position(|preset| preset.id == source_id)
        .ok_or_else(|| anyhow!("Target preset not found"))?;
    let before = preset_fingerprint(&presets[source_index])?;
    if !request.expected_target_fingerprint.is_empty()
        && request.expected_target_fingerprint != before
    {
        bail!("Preset changed since Calibration; refresh before applying");
    }
    if source_id != receipt.preset_id && before != receipt.preset_fingerprint {
        bail!("Target preset does not match the Calibration source");
    }

    let mut updated = presets[source_index].clone();
    apply_patch_to_preset(&mut updated, &candidate.candidate.typed_patch);
    updated.backend = InferenceBackend::LlamaCpp;
    crate::inference::launch::validate_preset_backend_config(&updated)?;
    let derived = request.create_derived;
    let target_id = if derived {
        let id = crate::config::generate_random_token()
            .chars()
            .take(24)
            .collect::<String>();
        updated.id = id.clone();
        updated.name = format!("{} (Calibrated)", updated.name);
        id
    } else {
        updated.id.clone()
    };
    let rollback_id = crate::config::generate_random_token()
        .chars()
        .take(32)
        .collect::<String>();
    let rollback_path = config
        .app_paths
        .calibration_apply_backups_dir()
        .join(format!("{rollback_id}.json"));
    write_rollback_backup(&rollback_path, &presets[source_index])?;
    if derived {
        presets.push(updated.clone());
    } else {
        presets[source_index] = updated.clone();
    }
    presets::save_presets(&config.presets_file, &presets).context("save applied preset")?;
    let after = preset_fingerprint(&updated)?;
    let record = CalibrationApplyRecord {
        target_preset_id: target_id.clone(),
        candidate_id: candidate_id.clone(),
        derived,
        before_fingerprint: before.clone(),
        after_fingerprint: after.clone(),
        timestamp_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis()),
        validation: "not_run".into(),
        rollback_id,
    };
    receipt.apply_history.push(record);
    if let Err(error) = write_receipt(&receipt_path, &receipt) {
        // Restore the previous preset file/state if the receipt audit record
        // cannot be persisted; an apply without an audit trail is unsafe.
        let _ = presets::save_presets(&config.presets_file, &original_presets);
        return Err(error.context("persist Calibration apply history"));
    }
    *state.presets.lock().unwrap() = presets;
    Ok(ApplyCalibrationResult {
        preset_id: target_id,
        derived,
        candidate_id,
        before_fingerprint: before,
        after_fingerprint: after,
        validation: "not_run".into(),
    })
}

pub fn rollback(
    config: &AppConfig,
    state: &AppState,
    job_id: &str,
    request: RollbackCalibrationRequest,
) -> Result<RollbackCalibrationResult> {
    if request.exact_confirmation.as_deref() != Some(ROLLBACK_CONFIRMATION) {
        bail!("Exact confirmation ROLLBACK_CALIBRATION is required");
    }
    let receipt_path = config
        .app_paths
        .calibration_receipts_dir()
        .join(format!("{job_id}.json"));
    let bytes = fs::read(&receipt_path).map_err(|_| anyhow!("Calibration receipt not found"))?;
    let mut receipt: CalibrationReceipt =
        serde_json::from_slice(&bytes).context("Calibration receipt is invalid")?;
    let record = receipt
        .apply_history
        .last()
        .cloned()
        .ok_or_else(|| anyhow!("Calibration receipt has no applied preset"))?;
    if record.validation == "rolled_back" {
        bail!("Calibration apply has already been rolled back");
    }
    let mut presets = state
        .presets
        .lock()
        .map_err(|_| anyhow!("Preset store unavailable"))?
        .clone();
    let index = presets
        .iter()
        .position(|preset| preset.id == record.target_preset_id)
        .ok_or_else(|| anyhow!("Applied preset no longer exists"))?;
    let current = preset_fingerprint(&presets[index])?;
    if !request.expected_target_fingerprint.is_empty()
        && request.expected_target_fingerprint != current
    {
        bail!("Preset changed since Calibration apply; refresh before rolling back");
    }
    if current != record.after_fingerprint {
        bail!("Applied preset changed since Calibration apply; rollback is unsafe");
    }
    let backup_path = config
        .app_paths
        .calibration_apply_backups_dir()
        .join(format!("{}.json", record.rollback_id));
    let backup = read_rollback_backup(&backup_path)?;
    let before_fingerprint = preset_fingerprint(&backup)?;
    if record.derived {
        presets.remove(index);
    } else {
        presets[index] = backup;
    }
    presets::save_presets(&config.presets_file, &presets).context("save Calibration rollback")?;
    let restored = if record.derived {
        before_fingerprint
    } else {
        preset_fingerprint(&presets[index])?
    };
    receipt.apply_history.push(CalibrationApplyRecord {
        validation: "rolled_back".into(),
        ..record.clone()
    });
    write_receipt(&receipt_path, &receipt)?;
    *state.presets.lock().unwrap() = presets;
    Ok(RollbackCalibrationResult {
        preset_id: record.target_preset_id,
        before_fingerprint: current,
        after_fingerprint: restored,
    })
}

fn write_rollback_backup(path: &Path, preset: &ModelPreset) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    let encoded = serde_json::to_vec(preset).context("serialize Calibration rollback backup")?;
    let mut file = fs::File::create(&temporary)?;
    file.write_all(&encoded)?;
    file.sync_all()?;
    drop(file);
    crate::config::harden_file_permissions(&temporary);
    fs::rename(&temporary, path)?;
    crate::config::harden_file_permissions(path);
    Ok(())
}

fn read_rollback_backup(path: &Path) -> Result<ModelPreset> {
    let bytes = fs::read(path).map_err(|_| anyhow!("Calibration rollback backup not found"))?;
    serde_json::from_slice(&bytes).context("Calibration rollback backup is invalid")
}

/// Apply a measured candidate, then run one bounded real llama-bench check. If
/// the check is not valid, restore the exact prior source preset (or remove the
/// derived preset) before returning an error. The active inference session is
/// never touched by this validation.
pub async fn apply_with_validation(
    config: &AppConfig,
    state: &AppState,
    job_id: &str,
    request: ApplyCalibrationRequest,
) -> Result<ApplyCalibrationResult> {
    let source_id = if request.target_preset_id.is_empty() {
        get_receipt(config, job_id)?
            .ok_or_else(|| anyhow!("Calibration receipt not found"))?
            .preset_id
    } else {
        request.target_preset_id.clone()
    };
    let before = find_preset(state, &source_id)?;
    let derived = request.create_derived;
    let mut result = apply(config, state, job_id, request.clone())?;
    if !request.validate_after_apply {
        return Ok(result);
    }

    let applied = find_preset(state, &result.preset_id)?;
    let receipt = get_receipt(config, job_id)?
        .ok_or_else(|| anyhow!("Calibration receipt not found after apply"))?;
    let workload = receipt.fingerprint.workload;
    let model_path = resolve_model_path(config, &applied.model_path)?;
    let bench_path = llama_bench_path(&config.llama_server_path);
    let ngl = applied.gpu_layers.unwrap_or(99);
    let flash_attn = matches!(applied.flash_attn.trim(), "on" | "1" | "true");
    let ctk = non_empty_or(&applied.ctk, "q8_0");
    let ctv = non_empty_or(&applied.ctv, "q8_0");
    let batch_size = if applied.batch_size == 0 {
        2048
    } else {
        applied.batch_size
    };
    let ubatch_size = if applied.ubatch_size == 0 {
        512
    } else {
        applied.ubatch_size
    };
    let depth = workload.minimum_context.clamp(1, 4096);
    let measurement = match run_sweep_with_tokens(
        &bench_path,
        &config.llama_server_cwd,
        &model_path.to_string_lossy(),
        ngl,
        flash_attn,
        &ctk,
        &ctv,
        batch_size,
        ubatch_size,
        &[depth],
        applied.n_cpu_moe,
        workload.prompt_tokens.min(512),
        workload.generation_tokens.min(256),
    )
    .await
    {
        Ok(points) => measurement_from_points(points),
        Err(error) => CalibrationMeasurement {
            trial_id: result.candidate_id.clone(),
            status: Some(TrialStatus::Error),
            bounded_diagnostics: vec![sanitize_error(&error)],
            ..Default::default()
        },
    };
    let validation = if measurement.status == Some(TrialStatus::Ok) {
        "passed"
    } else {
        "failed_rolled_back"
    };
    update_apply_validation(config, job_id, validation)?;
    if validation != "passed" {
        rollback_immediate(config, state, &result.preset_id, derived, &before)?;
        bail!("Post-apply Calibration validation failed; the preset was rolled back")
    }
    result.validation = validation.into();
    Ok(result)
}

fn update_apply_validation(config: &AppConfig, job_id: &str, validation: &str) -> Result<()> {
    let path = config
        .app_paths
        .calibration_receipts_dir()
        .join(format!("{job_id}.json"));
    let bytes = fs::read(&path)?;
    let mut receipt: CalibrationReceipt = serde_json::from_slice(&bytes)?;
    if let Some(record) = receipt.apply_history.last_mut() {
        record.validation = validation.into();
    }
    write_receipt(&path, &receipt)
}

fn rollback_immediate(
    config: &AppConfig,
    state: &AppState,
    applied_id: &str,
    derived: bool,
    before: &ModelPreset,
) -> Result<()> {
    let mut presets = state
        .presets
        .lock()
        .map_err(|_| anyhow!("Preset store unavailable"))?
        .clone();
    if derived {
        presets.retain(|preset| preset.id != applied_id);
    } else if let Some(slot) = presets.iter_mut().find(|preset| preset.id == applied_id) {
        *slot = before.clone();
    } else {
        bail!("Applied preset disappeared during validation")
    }
    presets::save_presets(&config.presets_file, &presets)
        .context("persist Calibration rollback")?;
    *state.presets.lock().unwrap() = presets;
    Ok(())
}

fn select_winner(results: &[CalibrationCandidateResult]) -> Option<String> {
    results
        .iter()
        .filter(|result| result.measurement.status == Some(TrialStatus::Ok))
        .max_by(|left, right| {
            median(&left.measurement.tg_tps_samples)
                .partial_cmp(&median(&right.measurement.tg_tps_samples))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|result| result.candidate.id.clone())
}

/// Select the measured Balanced candidates that are eligible for a second,
/// independent verification pass. Baseline is excluded because it is the
/// control; only successful finite decode measurements can consume the two
/// verification slots. Ordering is deterministic for equal medians.
#[allow(dead_code)]
fn select_verification_candidates(
    results: &[CalibrationCandidateResult],
) -> Vec<CalibrationCandidate> {
    let mut eligible = results
        .iter()
        .filter(|result| {
            result.candidate.id != "baseline"
                && result.measurement.status == Some(TrialStatus::Ok)
                && !median(&result.measurement.tg_tps_samples).is_nan()
        })
        .collect::<Vec<_>>();
    eligible.sort_by(|left, right| {
        median(&right.measurement.tg_tps_samples)
            .total_cmp(&median(&left.measurement.tg_tps_samples))
            .then_with(|| left.candidate.id.cmp(&right.candidate.id))
    });
    eligible
        .into_iter()
        .take(super::candidates::BALANCED_MAX_VERIFICATION_CANDIDATES)
        .map(|result| result.candidate.clone())
        .collect()
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[sorted.len() / 2]
}

fn find_preset(state: &AppState, id: &str) -> Result<ModelPreset> {
    state
        .presets
        .lock()
        .map_err(|_| anyhow!("Preset store unavailable"))?
        .iter()
        .find(|preset| preset.id == id)
        .cloned()
        .ok_or_else(|| anyhow!("Preset not found"))
}

fn validate_workload(workload: &super::CalibrationWorkload) -> Result<()> {
    if workload.prompt_tokens == 0 || workload.prompt_tokens > 4096 {
        bail!("Quick Calibration prompt length must be between 1 and 4096 tokens");
    }
    if workload.generation_tokens == 0 || workload.generation_tokens > 4096 {
        bail!("Quick Calibration generation length must be between 1 and 4096 tokens");
    }
    if workload.parallel_requests != 1 {
        bail!("Quick Calibration supports one request only");
    }
    if workload.minimum_context == 0 || workload.minimum_context > 131_072 {
        bail!("Quick Calibration context must be between 1 and 131072 tokens");
    }
    Ok(())
}

fn resolve_model_path(config: &AppConfig, value: &str) -> Result<PathBuf> {
    let raw = PathBuf::from(value);
    let path = if raw.is_absolute() {
        raw
    } else {
        config
            .models_dir
            .clone()
            .unwrap_or_else(|| config.app_paths.models_dir())
            .join(raw)
    };
    let canonical = require_regular_file(&path).map_err(|error| match error {
        RegularFileError::NotFound | RegularFileError::Canonicalize => {
            anyhow!("Calibration model is unavailable")
        }
        RegularFileError::Symlink | RegularFileError::NotRegular => {
            anyhow!("Calibration requires a regular, non-symlink model file")
        }
    })?;
    let root = config
        .models_dir
        .clone()
        .unwrap_or_else(|| config.app_paths.models_dir())
        .canonicalize()
        .map_err(|_| anyhow!("Calibration model library is unavailable"))?;
    if canonical.strip_prefix(root).is_err() {
        bail!("Calibration model must be inside the configured model library");
    }
    if canonical
        .extension()
        .and_then(|ext| ext.to_str())
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("gguf"))
    {
        bail!("Calibration requires a GGUF model");
    }
    Ok(canonical)
}

fn library_relative_identity(config: &AppConfig, model: &Path) -> String {
    let root = config
        .models_dir
        .clone()
        .unwrap_or_else(|| config.app_paths.models_dir());
    root.canonicalize()
        .ok()
        .and_then(|root| {
            model
                .strip_prefix(root)
                .ok()
                .map(|path| path.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| {
            model
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

fn measurement_from_points(points: Vec<SweepPoint>) -> CalibrationMeasurement {
    let mut pp = Vec::new();
    let mut tg = Vec::new();
    for point in points {
        if point.pp_tps.is_finite() && point.pp_tps > 0.0 {
            pp.push(point.pp_tps);
        }
        if point.tg_tps.is_finite() && point.tg_tps > 0.0 {
            tg.push(point.tg_tps);
        }
    }
    let effective = tg.clone();
    CalibrationMeasurement {
        trial_id: "baseline".into(),
        status: Some(if pp.is_empty() && tg.is_empty() {
            TrialStatus::Implausible
        } else {
            TrialStatus::Ok
        }),
        pp_tps_samples: pp,
        tg_tps_samples: tg,
        ttft_ms_samples: Vec::new(),
        effective_tps_samples: effective,
        wall_time_ms: 0,
        memory_peak_bytes: None,
        bounded_diagnostics: Vec::new(),
    }
}

fn non_empty_or(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.into()
    } else {
        value.into()
    }
}

fn sanitize_error(error: &dyn std::fmt::Display) -> String {
    error
        .to_string()
        .lines()
        .take(4)
        .map(|line| {
            let line = line.rsplit_once('/').map_or(line, |(_, tail)| tail);
            line.rsplit_once('\\').map_or(line, |(_, tail)| tail)
        })
        .collect::<Vec<_>>()
        .join(" | ")
        .chars()
        .take(512)
        .collect()
}

fn write_receipt(path: &Path, receipt: &CalibrationReceipt) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    let encoded = serde_json::to_vec_pretty(receipt)?;
    fs::write(&temporary, encoded)?;
    fs::rename(&temporary, path)?;
    crate::config::harden_file_permissions(path);
    Ok(())
}

fn transition(runtime: &RuntimeJob, state: CalibrationJobState, phase: &str) -> Result<()> {
    update_snapshot(runtime, |snapshot| {
        snapshot.state = state;
        snapshot.phase = phase.into();
    })
}

fn fail_job(runtime: &RuntimeJob, message: &str) -> Result<()> {
    update_snapshot(runtime, |snapshot| {
        snapshot.state = CalibrationJobState::Failed;
        snapshot.phase = "failed".into();
        snapshot.diagnostics.push(message.to_string());
        snapshot.diagnostics.truncate(MAX_DIAGNOSTICS);
    })
}

fn finish_cancelled(runtime: &RuntimeJob) -> Result<()> {
    update_snapshot(runtime, |snapshot| {
        snapshot.state = CalibrationJobState::Cancelled;
        snapshot.phase = "cancelled".into();
    })
}

fn update_snapshot<F>(runtime: &RuntimeJob, update: F) -> Result<()>
where
    F: FnOnce(&mut CalibrationJobSnapshot),
{
    let mut snapshot = runtime
        .snapshot
        .lock()
        .map_err(|_| anyhow!("Calibration job unavailable"))?;
    update(&mut snapshot);
    write_snapshot(&runtime.snapshot_path, &snapshot)
}

pub fn apply_patch_to_preset(preset: &mut ModelPreset, patch: &LlamaCppCalibrationPatch) {
    if let Some(value) = patch.gpu_layers {
        preset.gpu_layers = Some(value);
    }
    if let Some(value) = patch.context_size {
        preset.context_size = value;
    }
    if let Some(value) = patch.threads {
        preset.threads = Some(value);
    }
    if let Some(value) = patch.threads_batch {
        preset.threads_batch = Some(value);
    }
    if let Some(value) = patch.ctk.as_ref() {
        preset.ctk = value.clone();
    }
    if let Some(value) = patch.ctv.as_ref() {
        preset.ctv = value.clone();
    }
    if let Some(value) = patch.batch_size {
        preset.batch_size = value;
    }
    if let Some(value) = patch.ubatch_size {
        preset.ubatch_size = value;
    }
    if let Some(value) = patch.flash_attn {
        preset.flash_attn = if value { "on" } else { "off" }.into();
    }
    if let Some(value) = patch.n_cpu_moe {
        preset.n_cpu_moe = Some(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_updates_only_typed_llama_fields() {
        let mut preset = ModelPreset::default();
        apply_patch_to_preset(
            &mut preset,
            &LlamaCppCalibrationPatch {
                context_size: Some(8192),
                flash_attn: Some(true),
                ..Default::default()
            },
        );
        assert_eq!(preset.context_size, 8192);
        assert_eq!(preset.flash_attn, "on");
    }

    #[test]
    fn measurement_rejects_empty_or_non_finite_points() {
        let measurement = measurement_from_points(vec![SweepPoint {
            depth: 1,
            pp_tps: f64::NAN,
            tg_tps: 0.0,
        }]);
        assert_eq!(measurement.status, Some(TrialStatus::Implausible));
    }

    #[test]
    fn verification_selection_is_bounded_and_deterministic() {
        let candidate = |id: &str, tg: f64| CalibrationCandidateResult {
            candidate: CalibrationCandidate {
                id: id.into(),
                typed_patch: LlamaCppCalibrationPatch::default(),
                capability_evidence: Vec::new(),
                predicted_memory_bytes: None,
            },
            measurement: CalibrationMeasurement {
                status: Some(TrialStatus::Ok),
                tg_tps_samples: vec![tg],
                ..Default::default()
            },
        };
        let selected = select_verification_candidates(&[
            candidate("baseline", 100.0),
            candidate("b", 120.0),
            candidate("a", 120.0),
            candidate("c", 130.0),
        ]);
        assert_eq!(
            selected
                .iter()
                .map(|candidate| candidate.id.as_str())
                .collect::<Vec<_>>(),
            ["c", "a"]
        );
    }

    #[test]
    fn rollback_backup_round_trips_a_preset() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("apply-backups").join("one.json");
        let mut preset = ModelPreset::default();
        preset.id = "source".into();
        preset.name = "Source".into();
        preset.api_key = Some("secret".into());
        write_rollback_backup(&path, &preset).expect("write backup");
        let restored = read_rollback_backup(&path).expect("read backup");
        assert_eq!(restored.id, preset.id);
        assert_eq!(restored.name, preset.name);
        assert_eq!(restored.api_key, preset.api_key);
    }
}
