//! Pure analysis of measured Calibration rows.
//!
//! This module never launches a process and never treats predicted values as
//! measurements. Invalid rows are excluded from winners and Pareto results.

use super::{CalibrationCandidateResult, TrialStatus};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CandidateAnalysis {
    pub candidate_id: String,
    pub status: Option<TrialStatus>,
    pub median_pp_tps: f64,
    pub median_tg_tps: f64,
    pub median_effective_tps: f64,
    pub spread_tg_tps: f64,
    pub sample_count: usize,
    pub confidence: AnalysisConfidence,
    pub noise_warning: bool,
    pub baseline_delta_tg_tps: Option<f64>,
    pub context_size: Option<u64>,
    pub pareto: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisConfidence {
    #[default]
    None,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CalibrationAnalysis {
    pub candidates: Vec<CandidateAnalysis>,
    pub pareto_candidate_ids: Vec<String>,
    pub fastest_candidate: Option<String>,
    pub balanced_candidate: Option<String>,
    pub max_context_candidate: Option<String>,
    pub warnings: Vec<String>,
    pub main_effects: Vec<MainEffect>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MainEffect {
    pub factor: String,
    pub level: String,
    pub sample_count: usize,
    pub median_effective_tps: f64,
    pub delta_from_factor_mean: f64,
}

pub fn analyze(results: &[CalibrationCandidateResult]) -> CalibrationAnalysis {
    let baseline = results
        .iter()
        .find(|result| result.candidate.id == "baseline" && valid(&result.measurement));
    let baseline_tg = baseline.map(|result| median(&result.measurement.tg_tps_samples));
    let mut candidates = results
        .iter()
        .map(|result| CandidateAnalysis {
            candidate_id: result.candidate.id.clone(),
            status: result.measurement.status,
            median_pp_tps: median(&result.measurement.pp_tps_samples),
            median_tg_tps: median(&result.measurement.tg_tps_samples),
            median_effective_tps: median(&result.measurement.effective_tps_samples),
            spread_tg_tps: relative_spread(&result.measurement.tg_tps_samples),
            sample_count: valid_sample_count(&result.measurement.tg_tps_samples),
            confidence: confidence(&result.measurement.tg_tps_samples),
            noise_warning: relative_spread(&result.measurement.tg_tps_samples) > 0.15,
            baseline_delta_tg_tps: baseline_tg
                .map(|value| median(&result.measurement.tg_tps_samples) - value),
            context_size: result.candidate.typed_patch.context_size,
            pareto: false,
        })
        .collect::<Vec<_>>();

    let pareto_ids = pareto_frontier(&candidates);
    for candidate in &mut candidates {
        candidate.pareto = pareto_ids.iter().any(|id| id == &candidate.candidate_id);
    }
    let fastest = best_by(&candidates, |candidate| candidate.median_tg_tps);
    let balanced = best_by(&candidates, |candidate| candidate.median_effective_tps);
    let max_context = candidates
        .iter()
        .filter(|candidate| valid_analysis(candidate) && candidate.context_size.is_some())
        .max_by(|left, right| {
            left.context_size
                .cmp(&right.context_size)
                .then_with(|| {
                    left.median_effective_tps
                        .total_cmp(&right.median_effective_tps)
                })
                .then_with(|| right.candidate_id.cmp(&left.candidate_id))
        })
        .map(|candidate| candidate.candidate_id.clone());

    let warnings = candidates
        .iter()
        .filter(|candidate| candidate.noise_warning)
        .map(|candidate| format!("{} has high decode noise", candidate.candidate_id))
        .collect();
    let main_effects = main_effects(results);
    CalibrationAnalysis {
        candidates,
        pareto_candidate_ids: pareto_ids,
        fastest_candidate: fastest,
        balanced_candidate: balanced,
        max_context_candidate: max_context,
        warnings,
        main_effects,
    }
}

fn main_effects(results: &[CalibrationCandidateResult]) -> Vec<MainEffect> {
    let mut grouped: BTreeMap<(String, String), Vec<f64>> = BTreeMap::new();
    for result in results.iter().filter(|result| valid(&result.measurement)) {
        let value = median(&result.measurement.effective_tps_samples);
        if value <= 0.0 {
            continue;
        }
        for (factor, level) in factor_levels(&result.candidate.typed_patch) {
            grouped.entry((factor, level)).or_default().push(value);
        }
    }
    let factor_means = grouped.iter().fold(
        BTreeMap::<String, Vec<f64>>::new(),
        |mut means, ((factor, _), values)| {
            means
                .entry(factor.clone())
                .or_default()
                .push(median(values));
            means
        },
    );
    grouped
        .into_iter()
        .map(|((factor, level), values)| {
            let median_effective_tps = median(&values);
            let mean = factor_means
                .get(&factor)
                .map(|values| values.iter().sum::<f64>() / values.len() as f64)
                .unwrap_or(0.0);
            MainEffect {
                factor,
                level,
                sample_count: values.len(),
                median_effective_tps,
                delta_from_factor_mean: median_effective_tps - mean,
            }
        })
        .collect()
}

fn factor_levels(patch: &super::LlamaCppCalibrationPatch) -> Vec<(String, String)> {
    let mut levels = Vec::new();
    if let Some(value) = patch.context_size {
        levels.push(("context_size".into(), value.to_string()));
    }
    if let Some(value) = patch.batch_size {
        levels.push(("batch_size".into(), value.to_string()));
    }
    if let Some(value) = patch.ubatch_size {
        levels.push(("ubatch_size".into(), value.to_string()));
    }
    if let Some(value) = patch.threads {
        levels.push(("threads".into(), value.to_string()));
    }
    if let Some(value) = patch.flash_attn {
        levels.push(("flash_attn".into(), value.to_string()));
    }
    levels
}

fn pareto_frontier(candidates: &[CandidateAnalysis]) -> Vec<String> {
    candidates
        .iter()
        .filter(|candidate| valid_analysis(candidate))
        .filter(|candidate| {
            !candidates.iter().any(|other| {
                valid_analysis(other)
                    && other.candidate_id != candidate.candidate_id
                    && other.median_effective_tps >= candidate.median_effective_tps
                    && other.context_size.unwrap_or(0) >= candidate.context_size.unwrap_or(0)
                    && (other.median_effective_tps > candidate.median_effective_tps
                        || other.context_size.unwrap_or(0) > candidate.context_size.unwrap_or(0))
            })
        })
        .map(|candidate| candidate.candidate_id.clone())
        .collect()
}

fn best_by<F>(candidates: &[CandidateAnalysis], score: F) -> Option<String>
where
    F: Fn(&CandidateAnalysis) -> f64,
{
    candidates
        .iter()
        .filter(|candidate| valid_analysis(candidate))
        .max_by(|left, right| {
            score(left)
                .total_cmp(&score(right))
                .then_with(|| right.candidate_id.cmp(&left.candidate_id))
        })
        .map(|candidate| candidate.candidate_id.clone())
}

fn valid(result: &super::CalibrationMeasurement) -> bool {
    result.status == Some(TrialStatus::Ok)
        && result
            .tg_tps_samples
            .iter()
            .any(|v| v.is_finite() && *v > 0.0)
}

fn valid_analysis(candidate: &CandidateAnalysis) -> bool {
    candidate.status == Some(TrialStatus::Ok)
        && candidate.median_tg_tps.is_finite()
        && candidate.median_tg_tps > 0.0
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect::<Vec<_>>();
    if sorted.is_empty() {
        return 0.0;
    }
    sorted.sort_by(f64::total_cmp);
    sorted[sorted.len() / 2]
}

fn relative_spread(values: &[f64]) -> f64 {
    let center = median(values);
    if center <= 0.0 {
        return 0.0;
    }
    let deviations = values
        .iter()
        .filter(|value| value.is_finite() && **value > 0.0)
        .map(|value| (*value - center).abs())
        .collect::<Vec<_>>();
    median(&deviations) / center
}

fn valid_sample_count(values: &[f64]) -> usize {
    values
        .iter()
        .filter(|value| value.is_finite() && **value > 0.0)
        .count()
}

fn confidence(values: &[f64]) -> AnalysisConfidence {
    match valid_sample_count(values) {
        0 => AnalysisConfidence::None,
        1 => AnalysisConfidence::Low,
        2..=3 => AnalysisConfidence::Medium,
        _ => {
            if relative_spread(values) <= 0.10 {
                AnalysisConfidence::High
            } else {
                AnalysisConfidence::Medium
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::LlamaCppCalibrationPatch;

    fn row(
        id: &str,
        tg: &[f64],
        effective: &[f64],
        context: Option<u64>,
    ) -> CalibrationCandidateResult {
        CalibrationCandidateResult {
            candidate: crate::calibration::CalibrationCandidate {
                id: id.into(),
                typed_patch: LlamaCppCalibrationPatch {
                    context_size: context,
                    ..Default::default()
                },
                capability_evidence: Vec::new(),
                predicted_memory_bytes: None,
            },
            measurement: crate::calibration::CalibrationMeasurement {
                trial_id: id.into(),
                status: Some(TrialStatus::Ok),
                tg_tps_samples: tg.into(),
                effective_tps_samples: effective.into(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn labels_deltas_and_pareto_are_measured_and_deterministic() {
        let analysis = analyze(&[
            row("baseline", &[10.0, 12.0], &[8.0], Some(4096)),
            row("fast", &[20.0, 21.0], &[15.0], Some(4096)),
            row("context", &[14.0, 15.0], &[11.0], Some(8192)),
            row("bad", &[f64::NAN], &[f64::NAN], Some(65536)),
        ]);
        assert_eq!(analysis.fastest_candidate.as_deref(), Some("fast"));
        assert_eq!(analysis.balanced_candidate.as_deref(), Some("fast"));
        assert_eq!(analysis.max_context_candidate.as_deref(), Some("context"));
        assert_eq!(analysis.pareto_candidate_ids, ["fast", "context"]);
        assert_eq!(analysis.candidates[1].baseline_delta_tg_tps, Some(9.0));
        assert!(!analysis.candidates[3].pareto);
        assert!(
            analysis
                .main_effects
                .iter()
                .any(|effect| effect.factor == "context_size" && effect.level == "8192")
        );
    }
}
