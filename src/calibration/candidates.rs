//! Deterministic, capability-gated llama.cpp candidate planning.

use super::{CalibrationCandidate, CalibrationWorkload, LlamaCppCalibrationPatch};
use crate::inference::InferenceBackend;
use crate::inference::launch::{
    LocalLaunchRequest, request_from_preset, validate_preset_backend_config,
};
use crate::inference::llama_cpp_capabilities::CapabilitySnapshot;
use crate::presets::ModelPreset;
use anyhow::{Result, anyhow};

/// Build the bounded Quick candidate set without launching any process.
///
/// Every candidate is validated through the production preset/launch path.
/// Optional flags are only emitted when the exact runtime help snapshot proves
/// they exist; the baseline is always retained as the measured control.
pub fn quick_candidates(
    preset: &ModelPreset,
    workload: &CalibrationWorkload,
    capabilities: Option<&CapabilitySnapshot>,
) -> Result<Vec<CalibrationCandidate>> {
    if preset.backend != InferenceBackend::LlamaCpp {
        return Err(anyhow!("Calibration candidates require a llama.cpp preset"));
    }
    validate_preset_backend_config(preset)?;
    if workload.minimum_context == 0 {
        return Err(anyhow!("Calibration workload requires a positive context"));
    }

    let mut planned = vec![(
        "baseline".to_string(),
        LlamaCppCalibrationPatch::default(),
        vec!["baseline preset configuration".to_string()],
    )];

    if capability_has(capabilities, "-fa", "--flash-attn")
        && !matches!(preset.flash_attn.trim(), "on" | "1" | "true")
    {
        planned.push((
            "flash-attention".into(),
            LlamaCppCalibrationPatch {
                flash_attn: Some(true),
                ..Default::default()
            },
            vec!["llama-server help advertises flash attention".into()],
        ));
    }

    let current_batch = if preset.batch_size == 0 {
        2048
    } else {
        preset.batch_size
    };
    let current_ubatch = if preset.ubatch_size == 0 {
        512
    } else {
        preset.ubatch_size
    };
    if current_batch > 1024 && current_ubatch > 256 {
        planned.push((
            "bounded-batch".into(),
            LlamaCppCalibrationPatch {
                batch_size: Some(1024),
                ubatch_size: Some(256),
                ..Default::default()
            },
            vec!["bounded Quick batch alternative".into()],
        ));
    }

    let mut candidates = Vec::new();
    for (id, patch, evidence) in planned {
        let mut candidate_preset = preset.clone();
        super::executor::apply_patch_to_preset(&mut candidate_preset, &patch);
        validate_preset_backend_config(&candidate_preset)?;
        let launch = request_from_preset(&candidate_preset, None)?;
        if !matches!(launch, LocalLaunchRequest::LlamaCpp(_)) {
            return Err(anyhow!("Calibration candidate crossed backend boundary"));
        }
        candidates.push(CalibrationCandidate {
            id,
            typed_patch: patch,
            capability_evidence: evidence,
            predicted_memory_bytes: None,
        });
    }
    Ok(candidates)
}

fn capability_has(capabilities: Option<&CapabilitySnapshot>, short: &str, long: &str) -> bool {
    capabilities.is_some_and(|snapshot| {
        snapshot
            .serve_flags
            .iter()
            .any(|flag| flag == short || flag == long)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::llama_cpp_capabilities::{
        CacheCapabilities, CapabilitySnapshot, CapabilitySnapshotSource, ConcurrencyCapabilities,
        ContextCapabilities, EndpointCapabilities, ExecutableIdentity, SpeculationCapabilities,
        StreamingCapabilities, TemplateCapabilities, ToolCapabilities,
    };

    fn snapshot_with_flash() -> CapabilitySnapshot {
        CapabilitySnapshot {
            executable_identity: ExecutableIdentity {
                path: "llama-server".into(),
                file_hash: "hash".into(),
                file_mtime_unix: 0,
            },
            version_text: "test".into(),
            help_hash: "help".into(),
            serve_flags: vec!["-fa".into()],
            cache: CacheCapabilities::default(),
            context: ContextCapabilities::default(),
            concurrency: ConcurrencyCapabilities::default(),
            endpoints: EndpointCapabilities::default(),
            streaming: StreamingCapabilities::default(),
            templates: TemplateCapabilities::default(),
            tools: ToolCapabilities::default(),
            speculation: SpeculationCapabilities::default(),
            evidence_timestamp: 0,
            source: CapabilitySnapshotSource::ManualOverride,
        }
    }

    #[test]
    fn quick_candidates_are_deterministic_and_capability_gated() {
        let mut preset = ModelPreset::default();
        preset.batch_size = 2048;
        preset.ubatch_size = 512;
        let workload = CalibrationWorkload::default();
        let without = quick_candidates(&preset, &workload, None).expect("baseline candidates");
        assert_eq!(
            without.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["baseline", "bounded-batch"]
        );
        let with = quick_candidates(&preset, &workload, Some(&snapshot_with_flash()))
            .expect("capability candidates");
        assert_eq!(with[1].id, "flash-attention");
    }

    #[test]
    fn rapid_preset_is_rejected_before_candidate_generation() {
        let mut preset = ModelPreset::default();
        preset.backend = InferenceBackend::RapidMlx;
        assert!(quick_candidates(&preset, &CalibrationWorkload::default(), None).is_err());
    }
}
