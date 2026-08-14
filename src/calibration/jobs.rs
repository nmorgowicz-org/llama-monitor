//! Durable job journal primitives for Calibration.
//!
//! The executor and HTTP layer are intentionally separate. These helpers make
//! every state transition durable before a trial can start, and classify a
//! started-without-finished trial as a suspected crash on recovery.

use super::{
    CalibrationCandidateResult, CalibrationJobManifest, CalibrationJobSnapshot, CalibrationJobState,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_JOURNAL_LINE_BYTES: usize = 32 * 1024;
const MAX_TRIAL_RESULT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalEventKind {
    #[default]
    JobCreated,
    TrialPlanned,
    TrialStarted,
    TrialFinished,
    JobCancelled,
    JobFailed,
    JobResumed,
    TrialAbandoned,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct JournalEvent {
    pub schema_version: u32,
    pub timestamp_unix_ms: u128,
    pub kind: JournalEventKind,
    pub trial_id: Option<String>,
    pub detail: Option<String>,
}

impl JournalEvent {
    pub fn new(kind: JournalEventKind, trial_id: Option<String>) -> Self {
        Self {
            schema_version: super::CALIBRATION_SCHEMA_VERSION,
            timestamp_unix_ms: now_ms(),
            kind,
            trial_id,
            detail: None,
        }
    }
}

pub fn append_event(path: &Path, event: &JournalEvent) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let encoded = serde_json::to_vec(event).context("serialize calibration journal event")?;
    if encoded.len() > MAX_JOURNAL_LINE_BYTES {
        bail!("calibration journal event exceeds the bounded output limit");
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(&encoded)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    crate::config::harden_file_permissions(path);
    Ok(())
}

pub fn read_events(path: &Path) -> Result<Vec<JournalEvent>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path)?;
    let mut events = Vec::new();
    for (line_number, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.len() > MAX_JOURNAL_LINE_BYTES {
            bail!(
                "calibration journal line {} exceeds the bounded output limit",
                line_number + 1
            );
        }
        events.push(
            serde_json::from_str(&line)
                .with_context(|| format!("parse calibration journal line {}", line_number + 1))?,
        );
    }
    Ok(events)
}

pub fn suspected_crash_trials(events: &[JournalEvent]) -> BTreeSet<String> {
    let mut started = BTreeSet::new();
    for event in events {
        let Some(trial_id) = event.trial_id.as_deref() else {
            continue;
        };
        match event.kind {
            JournalEventKind::TrialStarted => {
                started.insert(trial_id.to_string());
            }
            JournalEventKind::TrialFinished | JournalEventKind::TrialAbandoned => {
                started.remove(trial_id);
            }
            _ => {}
        }
    }
    started
}

pub fn write_snapshot(path: &Path, snapshot: &CalibrationJobSnapshot) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let temporary = path.with_extension("json.tmp");
    let encoded = serde_json::to_vec_pretty(snapshot).context("serialize calibration snapshot")?;
    let mut file = File::create(&temporary)?;
    file.write_all(&encoded)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path)?;
    crate::config::harden_file_permissions(path);
    Ok(())
}

pub fn recover_snapshot(path: &Path) -> Result<Option<CalibrationJobSnapshot>> {
    if !path.exists() {
        return Ok(None);
    }
    let encoded = fs::read(path)?;
    let snapshot: CalibrationJobSnapshot = serde_json::from_slice(&encoded)
        .with_context(|| format!("parse calibration snapshot {}", path.display()))?;
    Ok(Some(snapshot))
}

pub fn write_manifest(path: &Path, manifest: &CalibrationJobManifest) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let encoded = serde_json::to_vec_pretty(manifest).context("serialize Calibration manifest")?;
    let temporary = path.with_extension("json.tmp");
    let mut file = File::create(&temporary)?;
    file.write_all(&encoded)?;
    file.sync_all()?;
    drop(file);
    crate::config::harden_file_permissions(&temporary);
    fs::rename(&temporary, path)?;
    crate::config::harden_file_permissions(path);
    Ok(())
}

pub fn read_manifest(path: &Path) -> Result<Option<CalibrationJobManifest>> {
    if !path.exists() {
        return Ok(None);
    }
    let encoded = fs::read(path)?;
    Ok(Some(serde_json::from_slice(&encoded).with_context(
        || format!("parse Calibration manifest {}", path.display()),
    )?))
}

pub fn mark_recovered_crash(snapshot: &mut CalibrationJobSnapshot, trial_count: usize) {
    if matches!(snapshot.state, CalibrationJobState::Running) {
        snapshot.state = CalibrationJobState::Failed;
        snapshot.phase = "recovered_suspected_crash".into();
        snapshot
            .diagnostics
            .push(format!("Recovered {} unfinished trial(s) as suspected crash; explicit confirmation is required before retry", trial_count));
    }
}

pub fn append_trial_result(path: &Path, result: &CalibrationCandidateResult) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let encoded = serde_json::to_vec(result).context("serialize Calibration trial result")?;
    if encoded.len() > MAX_TRIAL_RESULT_BYTES {
        bail!("Calibration trial result exceeds bounded output limit");
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(&encoded)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}

pub fn read_trial_results(path: &Path) -> Result<Vec<CalibrationCandidateResult>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path)?;
    let mut results = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.len() > MAX_TRIAL_RESULT_BYTES {
            bail!(
                "Calibration trial result line {} exceeds bounded output limit",
                index + 1
            );
        }
        results.push(
            serde_json::from_str(&line)
                .with_context(|| format!("parse Calibration trial result {}", index + 1))?,
        );
    }
    Ok(results)
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |value| value.as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn journal_round_trip_and_crash_classification() {
        let temp = tempdir().expect("tempdir");
        let journal = temp.path().join("jobs").join("journal.jsonl");
        append_event(
            &journal,
            &JournalEvent::new(JournalEventKind::TrialStarted, Some("a".into())),
        )
        .expect("append started");
        append_event(
            &journal,
            &JournalEvent::new(JournalEventKind::TrialStarted, Some("b".into())),
        )
        .expect("append started");
        append_event(
            &journal,
            &JournalEvent::new(JournalEventKind::TrialFinished, Some("a".into())),
        )
        .expect("append finished");

        let events = read_events(&journal).expect("read events");
        assert_eq!(events.len(), 3);
        assert_eq!(
            suspected_crash_trials(&events)
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["b".to_string()]
        );
    }

    #[test]
    fn snapshot_write_is_atomic_and_recovery_marks_running_jobs() {
        let temp = tempdir().expect("tempdir");
        let snapshot_path = temp.path().join("jobs").join("snapshot.json");
        let mut snapshot = CalibrationJobSnapshot {
            id: "job-1".into(),
            state: CalibrationJobState::Running,
            phase: "trial".into(),
            completed_trials: 1,
            planned_trials: 2,
            diagnostics: Vec::new(),
            receipt_id: None,
        };
        write_snapshot(&snapshot_path, &snapshot).expect("write snapshot");
        assert!(!snapshot_path.with_extension("json.tmp").exists());
        let recovered = recover_snapshot(&snapshot_path)
            .expect("recover snapshot")
            .expect("snapshot exists");
        assert_eq!(recovered.id, "job-1");
        mark_recovered_crash(&mut snapshot, 1);
        assert_eq!(snapshot.state, CalibrationJobState::Failed);
        assert!(snapshot.phase.contains("suspected_crash"));
    }

    #[test]
    fn trial_results_round_trip_for_resume() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("results.jsonl");
        let result = CalibrationCandidateResult {
            candidate: Default::default(),
            measurement: Default::default(),
        };
        append_trial_result(&path, &result).expect("append result");
        assert_eq!(
            read_trial_results(&path).expect("read results"),
            vec![result]
        );
    }
}
