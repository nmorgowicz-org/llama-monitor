//! Bounded single-trial Calibration executor.
//!
//! This is deliberately narrower than the final Quick/Balanced search. It
//! proves the lifecycle boundary first: preflight, one managed sibling
//! `llama-bench` run, durable journal transitions, cancellation, and a
//! redacted receipt. It never stops an active server or mutates a preset.

use super::jobs::{
    JournalEvent, JournalEventKind, append_event, mark_recovered_crash, read_events,
    recover_snapshot, suspected_crash_trials, write_snapshot,
};
use super::{
    CalibrationFingerprint, CalibrationJobSnapshot, CalibrationJobState, CalibrationMeasurement,
    CalibrationReceipt, LlamaCppCalibrationPatch, StartCalibrationRequest, TrialStatus,
};
use crate::config::AppConfig;
use crate::inference::InferenceBackend;
use crate::llama::bench_runner::{SweepPoint, llama_bench_path, run_sweep_with_tokens};
use crate::presets::ModelPreset;
use crate::state::AppState;
use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
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
    pub requires_server_stop: bool,
    pub supported_budget: &'static str,
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
        planned_trials: 1,
        requires_server_stop: true,
        supported_budget: "quick_single_trial",
    })
}

pub fn start(
    config: Arc<AppConfig>,
    state: AppState,
    request: StartCalibrationRequest,
) -> Result<CalibrationJobSnapshot> {
    if request.budget != super::CalibrationBudget::Quick {
        bail!("Only the bounded Quick single-trial Calibration is available yet");
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
    let preflight = preflight(&config, &state, &request.preset_id)?;
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
        model_path,
        bench_path,
        fingerprint,
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
    model_path: PathBuf,
    bench_path: PathBuf,
    fingerprint: String,
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
    if let Err(error) = transition(&runtime, CalibrationJobState::Running, "trial")
        .and_then(|_| {
            append_event(
                &runtime.journal_path,
                &JournalEvent::new(JournalEventKind::TrialPlanned, Some("baseline".into())),
            )
        })
        .and_then(|_| {
            append_event(
                &runtime.journal_path,
                &JournalEvent::new(JournalEventKind::TrialStarted, Some("baseline".into())),
            )
        })
    {
        let _ = fail_job(
            &runtime,
            &format!("Calibration journal could not be prepared: {error}"),
        );
        return;
    }

    let ngl = preset.gpu_layers.unwrap_or(99);
    let flash_attn = matches!(preset.flash_attn.trim(), "on" | "1" | "true");
    let ctk = non_empty_or(&preset.ctk, "q8_0");
    let ctv = non_empty_or(&preset.ctv, "q8_0");
    let batch_size = if preset.batch_size == 0 {
        2048
    } else {
        preset.batch_size
    };
    let ubatch_size = if preset.ubatch_size == 0 {
        512
    } else {
        preset.ubatch_size
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
        preset.n_cpu_moe,
        workload.prompt_tokens,
        workload.generation_tokens,
    );
    let result = tokio::select! {
        result = bench => Some(result),
        _ = runtime.cancel_notify.notified() => None,
    };
    if runtime.cancel.load(Ordering::Acquire) {
        let _ = append_event(
            &runtime.journal_path,
            &JournalEvent::new(JournalEventKind::JobCancelled, Some("baseline".into())),
        );
        let _ = finish_cancelled(&runtime);
        return;
    }
    match result {
        Some(Ok(points)) => {
            let measurement = measurement_from_points(points);
            let receipt = CalibrationReceipt {
                schema_version: super::CALIBRATION_SCHEMA_VERSION,
                method_version: METHOD_VERSION.into(),
                job_id: id.clone(),
                fingerprint: CalibrationFingerprint {
                    baseline_config_hash: fingerprint,
                    workload,
                    ..CalibrationFingerprint::current(
                        InferenceBackend::LlamaCpp,
                        Default::default(),
                    )
                },
                measurement,
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
            let _ = append_event(
                &runtime.journal_path,
                &JournalEvent::new(JournalEventKind::TrialFinished, Some("baseline".into())),
            );
            let _ = update_snapshot(&runtime, |snapshot| {
                snapshot.state = CalibrationJobState::Complete;
                snapshot.phase = "complete".into();
                snapshot.completed_trials = 1;
                snapshot.receipt_id = Some(id);
            });
        }
        Some(Err(error)) => {
            let _ = append_event(
                &runtime.journal_path,
                &JournalEvent::new(JournalEventKind::JobFailed, Some("baseline".into())),
            );
            let _ = fail_job(
                &runtime,
                &format!("Calibration trial failed: {}", sanitize_error(&error)),
            );
        }
        None => {
            let _ = finish_cancelled(&runtime);
        }
    }
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
    let metadata =
        fs::symlink_metadata(&path).map_err(|_| anyhow!("Calibration model is unavailable"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!("Calibration requires a regular, non-symlink model file");
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| anyhow!("Calibration model is unavailable"))?;
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
}
