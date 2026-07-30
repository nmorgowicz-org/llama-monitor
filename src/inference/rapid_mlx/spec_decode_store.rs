//! Measured speculative-decoding verdicts, keyed by capability-snapshot fingerprint.
//!
//! [`CapabilitySnapshot`](super::capabilities::CapabilitySnapshot) is built by parsing
//! `rapid-mlx serve --help`, so it can only ever record what a flag *claims*. Whether
//! the speculative scheduler actually engages takes a live server, a real model, and
//! three behavioral gates — see `scripts/rapid-mlx-requalify-spec-decode.mjs`. This
//! module is where that measurement is kept so the snapshot can look one up instead of
//! inferring one.
//!
//! The join key is [`CapabilitySnapshot::fingerprint`], which hashes the install path,
//! the binary's own hash, the help text, and every dependency version. That makes a
//! stored verdict exact and non-portable in the same breath: it cannot be misapplied
//! to a different build, and it can never match on another machine. Knowledge that
//! *is* portable across machines belongs in the version priors in
//! [`super::capabilities`], not here.
//!
//! Scope: the gates measure the scheduler, not a model. A verdict says "this build's
//! scheduler engages at temperature > 0 / under a tool grammar / without changing
//! greedy output". The model the measurement ran against is recorded in
//! [`MeasuredSpecDecode::model`] so a reader can see what it was, but a verdict is not
//! a promise that some other model has a usable MTP head.

use crate::inference::rapid_mlx::capabilities::{
    CapabilitySnapshot, FeatureQualification, spec_decode_gate_names, version_matches,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Store file name, under the same managed-runtime root the updater writes to.
pub const VERDICT_FILE: &str = "spec-decode-verdicts.json";

/// Bump only for a format change that older readers cannot understand. A store
/// written by a newer schema is ignored rather than guessed at.
const SCHEMA_VERSION: u32 = 1;

/// Refuse absurdly large stores rather than reading them into memory: this file holds
/// one small record per installed runtime, so anything past this is not our file.
const MAX_STORE_BYTES: u64 = 1024 * 1024;

/// Overall outcome of a requalification run, mirroring the lane's `overall` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpecDecodeOutcome {
    /// Every gate that ran passed.
    Qualified,
    /// Gates ran cleanly and the scheduler still does not engage.
    StillBlocked,
    /// A gate could not be read: the positive control failed, or a run errored.
    Uninterpretable,
}

/// One gate's result, kept so a verdict can be explained rather than just obeyed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredGate {
    pub gate: String,
    pub verdict: String,
    pub reason: String,
}

/// A measurement of one rapid-mlx build's speculative scheduler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredSpecDecode {
    /// The [`CapabilitySnapshot::fingerprint`] this was measured against.
    pub fingerprint: String,
    /// Version string the lane read from the binary it tested.
    pub rapid_mlx_version: String,
    pub outcome: SpecDecodeOutcome,
    /// The lane's own judgement that this run may promote the capability: a full gate
    /// sweep, all passing. A partial sweep is evidence, not qualification.
    pub promotes_capability: bool,
    pub gates_run: Vec<String>,
    pub gate_results: Vec<MeasuredGate>,
    /// Model the gates ran against. Recorded for provenance, not as a claim about it.
    pub model: String,
    /// Parsers that were installed during the run. "Tools did not block speculation"
    /// means nothing if no tool parser was ever installed, so these travel with the
    /// verdict rather than being dropped on ingest.
    pub tool_call_parser: Option<String>,
    pub reasoning_parser: Option<String>,
    pub parser_source: String,
    /// Directory holding the lane's raw run output.
    pub receipts: PathBuf,
    /// The lane's `generated_at`, ISO-8601. Not an expiry: the fingerprint changes if
    /// anything about the runtime changes, so a verdict is either exact or absent.
    pub measured_at: String,
}

impl MeasuredSpecDecode {
    /// Translate this measurement into a capability verdict.
    ///
    /// The only path to [`FeatureQualification::Available`] in the whole codebase runs
    /// through here, and it requires the lane's own `promotes_capability`: a full sweep
    /// with every gate passing.
    pub fn qualification(&self) -> FeatureQualification {
        match self.outcome {
            SpecDecodeOutcome::Qualified if self.promotes_capability => {
                FeatureQualification::Available
            }
            // Passing gates on a partial sweep is the most dangerous shape a result can
            // take: it looks like success. Say which gates were skipped.
            SpecDecodeOutcome::Qualified => {
                let skipped: Vec<&str> = spec_decode_gate_names()
                    .into_iter()
                    .filter(|name| !self.gates_run.iter().any(|run| run == name))
                    .collect();
                FeatureQualification::Indeterminate(format!(
                    "Every gate that ran passed on Rapid-MLX {version}, but the sweep was \
                     partial: {skipped} did not run",
                    version = self.rapid_mlx_version,
                    skipped = if skipped.is_empty() {
                        "some gates".to_string()
                    } else {
                        skipped.join(", ")
                    },
                ))
            }
            SpecDecodeOutcome::StillBlocked => FeatureQualification::Unavailable(format!(
                "Measured on this build: {reasons}",
                reasons = self.blocked_reasons(),
            )),
            // A run that could not be read settles nothing, so it must not read as a
            // negative result either.
            SpecDecodeOutcome::Uninterpretable => FeatureQualification::Indeterminate(format!(
                "The last requalification run on this build could not be read: {reasons}",
                reasons = self.blocked_reasons(),
            )),
        }
    }

    /// Reasons from the gates that did not pass, joined for display.
    fn blocked_reasons(&self) -> String {
        let failing: Vec<String> = self
            .gate_results
            .iter()
            .filter(|result| result.verdict != "pass")
            .map(|result| {
                format!(
                    "{gate} {verdict} — {reason}",
                    gate = result.gate,
                    verdict = result.verdict,
                    reason = result.reason
                )
            })
            .collect();
        if failing.is_empty() {
            "no gate reported a reason".to_string()
        } else {
            failing.join("; ")
        }
    }
}

/// On-disk shape. A map rather than a single record because one machine can have
/// several managed runtimes installed side by side, each with its own fingerprint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct StoreFile {
    schema_version: u32,
    verdicts: BTreeMap<String, MeasuredSpecDecode>,
}

/// Reader/writer for the measured-verdict store at a given managed-runtime root.
///
/// Takes its root as a parameter rather than resolving the config directory itself,
/// matching the updater and keeping the store testable in a tempdir.
#[derive(Debug, Clone)]
pub struct SpecDecodeVerdictStore {
    path: PathBuf,
}

impl SpecDecodeVerdictStore {
    /// Store held directly under `root`.
    pub fn at(root: &Path) -> Self {
        Self {
            path: root.join(VERDICT_FILE),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The measured verdict for this exact runtime, if one has ever been recorded.
    ///
    /// A missing, unreadable, oversized, or future-schema store returns `None`. Absence
    /// of a measurement is the normal case — it downgrades what can be claimed, it is
    /// not an error to report.
    pub fn verdict_for(&self, fingerprint: &str) -> Option<MeasuredSpecDecode> {
        self.load().ok()?.verdicts.get(fingerprint).cloned()
    }

    /// The most recent measurement recorded against some *other* runtime.
    ///
    /// Only meaningful when [`Self::verdict_for`] came back empty: it answers "you have
    /// swept this lane before, but not on what is installed now". That is the shape the
    /// upstream-changelog case takes — a new version lands, its fingerprint is new, and
    /// the old verdict silently stops applying. Without this the app would go quiet at
    /// exactly the moment the answer might have changed.
    ///
    /// Ordered by `measured_at` string comparison, which is correct because the lane
    /// writes ISO-8601 UTC. Ties fall to whichever the map yields last; a tie means two
    /// sweeps in the same millisecond and either is equally true.
    pub fn superseded_verdict(&self, fingerprint: &str) -> Option<MeasuredSpecDecode> {
        let file = self.load().ok()?;
        file.verdicts
            .values()
            .filter(|verdict| verdict.fingerprint != fingerprint)
            .max_by(|a, b| a.measured_at.cmp(&b.measured_at))
            .cloned()
    }

    /// Record a measurement against a fingerprint, replacing any earlier one.
    ///
    /// Later measurements win outright: a build that was blocked and now qualifies (or
    /// the reverse) has genuinely changed behavior, and keeping the older reading would
    /// mean serving a verdict we know to be superseded.
    pub fn record(&self, verdict: MeasuredSpecDecode) -> Result<()> {
        let mut file = self.load().unwrap_or_default();
        file.schema_version = SCHEMA_VERSION;
        file.verdicts.insert(verdict.fingerprint.clone(), verdict);
        self.write(&file)
    }

    /// Parse a `requalification.json` written by
    /// `scripts/rapid-mlx-requalify-spec-decode.mjs` and store it against `snapshot`.
    ///
    /// Every refusal below exists because the alternative is a verdict that reads as
    /// authoritative while describing something else: another build, or a different
    /// definition of what qualification means.
    pub fn ingest_requalification_report(
        &self,
        snapshot: &CapabilitySnapshot,
        report_path: &Path,
    ) -> Result<MeasuredSpecDecode> {
        let raw = std::fs::read_to_string(report_path).with_context(|| {
            format!(
                "Cannot read requalification report {}",
                report_path.display()
            )
        })?;
        let report: LaneReport = serde_json::from_str(&raw).with_context(|| {
            format!("{} is not a requalification report", report_path.display())
        })?;

        // A verdict that is not pinned to the build it was measured on is worthless —
        // the lane refuses to run without a version for the same reason.
        if !version_matches(&report.rapid_mlx_version, &snapshot.rapid_mlx_version) {
            bail!(
                "Report was measured on Rapid-MLX {measured}, but this snapshot describes \
                 {installed}; a verdict cannot be transferred between builds",
                measured = report.rapid_mlx_version.trim(),
                installed = snapshot.rapid_mlx_version,
            );
        }

        // If the lane and the app disagree about which gates constitute qualification,
        // then `promotes_capability` does not mean what this code thinks it means.
        let expected: Vec<String> = spec_decode_gate_names()
            .into_iter()
            .map(str::to_string)
            .collect();
        if report.gates_defined != expected {
            bail!(
                "Report defines gates {found:?} but this build qualifies on {expected:?}; \
                 the lane and SPEC_DECODE_GATES have drifted apart",
                found = report.gates_defined,
            );
        }

        let outcome = match report.overall.as_str() {
            "qualified" => SpecDecodeOutcome::Qualified,
            "still-blocked" => SpecDecodeOutcome::StillBlocked,
            "uninterpretable" => SpecDecodeOutcome::Uninterpretable,
            other => bail!("Report has unrecognised overall verdict {other:?}"),
        };

        // The lane computes this as "qualified AND full sweep". Recompute rather than
        // trusting the field, so a hand-edited report cannot promote the capability.
        let promotes_capability =
            outcome == SpecDecodeOutcome::Qualified && report.gates_run == expected;

        let verdict = MeasuredSpecDecode {
            fingerprint: snapshot.fingerprint(),
            rapid_mlx_version: report.rapid_mlx_version.trim().to_string(),
            outcome,
            promotes_capability,
            gates_run: report.gates_run,
            gate_results: report
                .results
                .into_iter()
                .map(|result| MeasuredGate {
                    gate: result.gate,
                    verdict: result.verdict,
                    reason: result.reason,
                })
                .collect(),
            model: report.model,
            tool_call_parser: report.tool_call_parser,
            reasoning_parser: report.reasoning_parser,
            parser_source: report.parser_source,
            receipts: report_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| report_path.to_path_buf()),
            measured_at: report.generated_at,
        };
        self.record(verdict.clone())?;
        Ok(verdict)
    }

    fn load(&self) -> Result<StoreFile> {
        let meta = std::fs::metadata(&self.path)?;
        if meta.len() > MAX_STORE_BYTES {
            bail!(
                "{} is {} bytes, too large to be a verdict store",
                self.path.display(),
                meta.len()
            );
        }
        let raw = std::fs::read_to_string(&self.path)?;
        let file: StoreFile = serde_json::from_str(&raw)?;
        if file.schema_version > SCHEMA_VERSION {
            bail!(
                "{} was written by schema {} but this build understands {SCHEMA_VERSION}",
                self.path.display(),
                file.schema_version
            );
        }
        Ok(file)
    }

    /// Write through a temp file in the same directory, so a crash mid-write leaves the
    /// previous store intact rather than a truncated one that would parse as empty.
    fn write(&self, file: &StoreFile) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Cannot create verdict store directory {}", parent.display())
            })?;
        }
        let temp = self.path.with_extension("json.tmp");
        let body = serde_json::to_string_pretty(file)?;
        std::fs::write(&temp, format!("{body}\n"))
            .with_context(|| format!("Cannot write {}", temp.display()))?;
        std::fs::rename(&temp, &self.path)
            .with_context(|| format!("Cannot install {}", self.path.display()))?;
        Ok(())
    }
}

/// Where this process keeps its store. Set once from the resolved config directory so
/// that `--config-dir` is honoured; a snapshot can be generated from several places and
/// none of them have the app config in hand.
static STORE_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Point the process store at `config_dir`. Called once during startup; later calls are
/// ignored rather than fought over, since the first caller is the one holding the
/// resolved configuration.
pub fn set_store_root(config_dir: &Path) {
    let _ = STORE_ROOT.set(config_dir.join(MANAGED_SUBDIR));
}

/// Subdirectory the managed runtimes live under, matching the updater's layout: a
/// verdict is about an installed runtime, so it belongs beside them.
const MANAGED_SUBDIR: &str = "runtimes/rapid-mlx";

/// The store for this process.
///
/// Falls back to the same platform default `AppConfig` uses when startup has not
/// registered a root — a snapshot generated before then still reads a real store rather
/// than silently finding nothing.
pub fn process_store() -> SpecDecodeVerdictStore {
    let root = STORE_ROOT
        .get()
        .cloned()
        .unwrap_or_else(|| default_config_dir().join(MANAGED_SUBDIR));
    SpecDecodeVerdictStore::at(&root)
}

fn default_config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        dirs::config_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
            .join("llama-monitor")
    }
    #[cfg(not(windows))]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("llama-monitor")
    }
}

/// The lane's `requalification.json`, narrowed to the fields a verdict needs.
#[derive(Debug, Deserialize)]
struct LaneReport {
    rapid_mlx_version: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    tool_call_parser: Option<String>,
    #[serde(default)]
    reasoning_parser: Option<String>,
    #[serde(default)]
    parser_source: String,
    #[serde(default)]
    gates_run: Vec<String>,
    #[serde(default)]
    gates_defined: Vec<String>,
    overall: String,
    #[serde(default)]
    results: Vec<LaneGateResult>,
    #[serde(default)]
    generated_at: String,
}

#[derive(Debug, Deserialize)]
struct LaneGateResult {
    #[serde(default)]
    gate: String,
    verdict: String,
    #[serde(default)]
    reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::rapid_mlx::capabilities::ExecutableIdentity;

    fn snapshot(version: &str) -> CapabilitySnapshot {
        CapabilitySnapshot {
            executable_identity: ExecutableIdentity {
                path: "/opt/rapid-mlx/bin/rapid-mlx".into(),
                file_hash: "abc123".into(),
                file_mtime_unix: 1,
            },
            rapid_mlx_version: version.to_string(),
            serve_flags: vec!["--speculative".into()],
            ..Default::default()
        }
    }

    fn report_json(overall: &str, gates_run: &[&str], version: &str) -> String {
        let gates: Vec<String> = spec_decode_gate_names()
            .into_iter()
            .map(|name| format!("\"{name}\""))
            .collect();
        let run: Vec<String> = gates_run.iter().map(|name| format!("\"{name}\"")).collect();
        let results: Vec<String> = gates_run
            .iter()
            .map(|name| {
                let verdict = if overall == "qualified" {
                    "pass"
                } else {
                    "blocked"
                };
                format!(
                    "{{\"gate\":\"{name}\",\"verdict\":\"{verdict}\",\"reason\":\"{name} reason\"}}"
                )
            })
            .collect();
        format!(
            "{{\"rapid_mlx_version\":\"{version}\",\"model\":\"/models/trunk\",\
             \"tool_call_parser\":\"qwen3\",\"reasoning_parser\":\"qwen3\",\
             \"parser_source\":\"explicit flags\",\"gates_run\":[{run}],\
             \"gates_defined\":[{gates}],\"overall\":\"{overall}\",\
             \"promotes_capability\":true,\"results\":[{results}],\
             \"generated_at\":\"2026-07-30T09:00:00.000Z\"}}",
            run = run.join(","),
            gates = gates.join(","),
            results = results.join(","),
        )
    }

    fn write_report(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("requalification.json");
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn absent_store_yields_no_verdict_rather_than_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        assert!(store.verdict_for("any-fingerprint").is_none());
    }

    #[test]
    fn full_passing_sweep_is_the_only_route_to_available() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        let snap = snapshot("0.12.0");
        let all: Vec<&str> = spec_decode_gate_names();
        let path = write_report(dir.path(), &report_json("qualified", &all, "0.12.0"));

        let verdict = store.ingest_requalification_report(&snap, &path).unwrap();
        assert!(verdict.promotes_capability);
        assert_eq!(verdict.qualification(), FeatureQualification::Available);
        // Round-trips through disk under the snapshot's own fingerprint.
        assert_eq!(
            store.verdict_for(&snap.fingerprint()).unwrap(),
            verdict,
            "a recorded verdict must be readable back under the fingerprint it was keyed by"
        );
    }

    #[test]
    fn a_verdict_is_keyed_to_one_runtime_and_invisible_to_others() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        let snap = snapshot("0.12.0");
        let all: Vec<&str> = spec_decode_gate_names();
        let path = write_report(dir.path(), &report_json("qualified", &all, "0.12.0"));
        store.ingest_requalification_report(&snap, &path).unwrap();

        // Same version, different install. The fingerprint hashes the binary itself, so
        // a measurement must not leak across runtimes — that is what keeps this store
        // from becoming a portable claim it has no evidence for.
        let mut other = snapshot("0.12.0");
        other.executable_identity.file_hash = "def456".into();
        assert_ne!(snap.fingerprint(), other.fingerprint());
        assert!(store.verdict_for(&other.fingerprint()).is_none());
    }

    #[test]
    fn partial_passing_sweep_is_indeterminate_and_names_the_skipped_gates() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        let snap = snapshot("0.12.0");
        let names = spec_decode_gate_names();
        let path = write_report(dir.path(), &report_json("qualified", &names[..1], "0.12.0"));

        let verdict = store.ingest_requalification_report(&snap, &path).unwrap();
        assert!(
            !verdict.promotes_capability,
            "a partial sweep is evidence, not qualification, even with every gate passing"
        );
        match verdict.qualification() {
            FeatureQualification::Indeterminate(reason) => {
                assert!(
                    reason.contains(names[1]),
                    "must name the skipped gate: {reason}"
                );
                assert!(
                    reason.contains(names[2]),
                    "must name the skipped gate: {reason}"
                );
            }
            other => panic!("expected Indeterminate, got {other:?}"),
        }
    }

    #[test]
    fn a_hand_edited_promotes_capability_flag_cannot_promote() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        let snap = snapshot("0.12.0");
        let names = spec_decode_gate_names();
        // The fixture always writes promotes_capability: true; this run swept one gate.
        let path = write_report(dir.path(), &report_json("qualified", &names[..1], "0.12.0"));
        let verdict = store.ingest_requalification_report(&snap, &path).unwrap();
        assert!(!verdict.promotes_capability);
    }

    #[test]
    fn still_blocked_is_unavailable_and_carries_the_gate_reasons() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        let snap = snapshot("0.12.0");
        let all: Vec<&str> = spec_decode_gate_names();
        let path = write_report(dir.path(), &report_json("still-blocked", &all, "0.12.0"));

        let verdict = store.ingest_requalification_report(&snap, &path).unwrap();
        match verdict.qualification() {
            FeatureQualification::Unavailable(reason) => {
                assert!(reason.contains("sampled reason"), "{reason}");
            }
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[test]
    fn an_unreadable_run_is_indeterminate_not_a_negative_result() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        let snap = snapshot("0.12.0");
        let all: Vec<&str> = spec_decode_gate_names();
        let path = write_report(dir.path(), &report_json("uninterpretable", &all, "0.12.0"));

        let verdict = store.ingest_requalification_report(&snap, &path).unwrap();
        assert!(matches!(
            verdict.qualification(),
            FeatureQualification::Indeterminate(_)
        ));
    }

    #[test]
    fn a_report_from_another_build_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        let all: Vec<&str> = spec_decode_gate_names();
        let path = write_report(dir.path(), &report_json("qualified", &all, "0.13.0"));

        let error = store
            .ingest_requalification_report(&snapshot("0.12.0"), &path)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("0.13.0") && error.contains("0.12.0"),
            "{error}"
        );
    }

    #[test]
    fn a_lane_that_defines_different_gates_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        let body = report_json("qualified", &["sampled"], "0.12.0")
            .replace("\"gates_defined\":[", "\"gates_defined\":[\"invented\",");

        let path = write_report(dir.path(), &body);
        let error = store
            .ingest_requalification_report(&snapshot("0.12.0"), &path)
            .unwrap_err()
            .to_string();
        assert!(error.contains("drifted apart"), "{error}");
    }

    #[test]
    fn a_later_measurement_supersedes_an_earlier_one() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        let snap = snapshot("0.12.0");
        let all: Vec<&str> = spec_decode_gate_names();

        let blocked = write_report(dir.path(), &report_json("still-blocked", &all, "0.12.0"));
        store
            .ingest_requalification_report(&snap, &blocked)
            .unwrap();
        let qualified = write_report(dir.path(), &report_json("qualified", &all, "0.12.0"));
        store
            .ingest_requalification_report(&snap, &qualified)
            .unwrap();

        assert_eq!(
            store
                .verdict_for(&snap.fingerprint())
                .unwrap()
                .qualification(),
            FeatureQualification::Available
        );
    }

    #[test]
    fn a_sweep_of_an_older_runtime_is_reported_as_superseded() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        let all: Vec<&str> = spec_decode_gate_names();

        let old = snapshot("0.11.1");
        let path = write_report(dir.path(), &report_json("still-blocked", &all, "0.11.1"));
        store.ingest_requalification_report(&old, &path).unwrap();

        // The upgrade: same install path, new binary, so a new fingerprint.
        let mut new = snapshot("0.12.0");
        new.executable_identity.file_hash = "newhash".into();
        assert!(store.verdict_for(&new.fingerprint()).is_none());

        let superseded = store.superseded_verdict(&new.fingerprint()).unwrap();
        assert_eq!(superseded.rapid_mlx_version, "0.11.1");
        assert_eq!(superseded.outcome, SpecDecodeOutcome::StillBlocked);
    }

    #[test]
    fn the_runtimes_own_verdict_is_never_reported_as_superseded() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        let snap = snapshot("0.12.0");
        let all: Vec<&str> = spec_decode_gate_names();
        let path = write_report(dir.path(), &report_json("still-blocked", &all, "0.12.0"));
        store.ingest_requalification_report(&snap, &path).unwrap();

        assert!(
            store.superseded_verdict(&snap.fingerprint()).is_none(),
            "a runtime's own measurement is current, not superseded"
        );
    }

    #[test]
    fn the_newest_of_several_older_sweeps_wins() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        let all: Vec<&str> = spec_decode_gate_names();

        for (version, hash, when) in [
            ("0.10.0", "h10", "2026-05-01T00:00:00.000Z"),
            ("0.11.1", "h11", "2026-07-30T09:00:00.000Z"),
        ] {
            let mut snap = snapshot(version);
            snap.executable_identity.file_hash = hash.into();
            let body = report_json("still-blocked", &all, version)
                .replace("2026-07-30T09:00:00.000Z", when);
            let path = write_report(dir.path(), &body);
            store.ingest_requalification_report(&snap, &path).unwrap();
        }

        let mut current = snapshot("0.12.0");
        current.executable_identity.file_hash = "h12".into();
        assert_eq!(
            store
                .superseded_verdict(&current.fingerprint())
                .unwrap()
                .rapid_mlx_version,
            "0.11.1"
        );
    }

    #[test]
    fn a_future_schema_store_is_ignored_rather_than_guessed_at() {
        let dir = tempfile::tempdir().unwrap();
        let store = SpecDecodeVerdictStore::at(dir.path());
        std::fs::write(
            store.path(),
            format!(
                "{{\"schema_version\":{},\"verdicts\":{{}}}}",
                SCHEMA_VERSION + 1
            ),
        )
        .unwrap();
        assert!(store.verdict_for("any").is_none());
    }
}
