use warp::Filter;

use crate::inference::rapid_mlx::capabilities::CacheDiagnosticParams;
use crate::memory_availability::MemoryAvailabilityState;
use crate::state::{DoctorFinding, DoctorFindingType, DoctorSeverity, FixAction};

use super::common::{ApiCtx, check_api_token, unauthorized_api_token, with_app_config};

pub fn routes(
    ctx: ApiCtx,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let state = ctx.state;
    let config = ctx.config;

    api_doctor_findings(state.clone(), config.clone())
        .map(|reply| Box::new(reply) as Box<dyn warp::reply::Reply>)
        .boxed()
}

fn api_doctor_findings(
    state: crate::state::AppState,
    app_config: std::sync::Arc<crate::config::AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "doctor" / "findings")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, cfg: std::sync::Arc<crate::config::AppConfig>| {
                let state = state.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }

                    let mut findings = Vec::new();

                    // Collect cache-related findings from active Rapid-MLX preset
                    findings.extend(collect_cache_findings(&state).await);

                    // Collect reclaim guidance findings when memory pressure is high
                    findings.extend(collect_reclaim_findings().await);

                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "ok": true,
                            "findings": findings
                        }),
                    )))
                }
            },
        )
}

async fn collect_cache_findings(state: &crate::state::AppState) -> Vec<DoctorFinding> {
    let mut findings = Vec::new();

    // Get active session and its preset config
    let active_session_id = state.active_session_id.lock().unwrap().clone();
    if active_session_id.is_empty() {
        return findings;
    }

    let preset = {
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.iter().find(|s| s.id == active_session_id);
        let Some(session) = session else {
            return findings;
        };
        if session.preset_id.is_empty() {
            return findings;
        }
        let preset_id = session.preset_id.clone();
        drop(sessions);

        let presets = state.presets.lock().unwrap();
        presets
            .iter()
            .find(|p| p.id == preset_id)
            .and_then(|p| p.rapid_mlx.as_ref())
            .cloned()
    };

    let Some(config) = preset else {
        return findings;
    };

    // Get capability snapshot if Rapid-MLX is configured
    let snapshot =
        match crate::inference::rapid_mlx::capabilities::generate_snapshot_from_discovery().await {
            Ok(s) => s,
            Err(_) => return findings,
        };

    // Get memory availability snapshot
    let mem_snapshot = crate::memory_availability::build_snapshot();

    // Compute cache diagnostic findings
    let params = CacheDiagnosticParams {
        config_prefix_cache_enabled: config.prefix_cache_enabled,
        config_prefix_cache_budget_bytes: config.prefix_cache_budget_bytes,
        config_max_cache_blocks: config.max_cache_blocks,
        snapshot: snapshot.clone(),
        configured_ceiling_bytes: mem_snapshot.configured_ceiling_bytes,
        current_safe_availability_bytes: mem_snapshot.current_safe_availability_bytes,
    };

    let cache_findings = snapshot.compute_prefix_cache_findings(&params);

    // Convert to DoctorFinding format
    for finding in cache_findings.findings {
        let severity = match finding.severity.as_str() {
            "error" => DoctorSeverity::Issue,
            "warning" => DoctorSeverity::Warning,
            _ => DoctorSeverity::Warning,
        };

        let fix = if finding.fixable {
            match finding.fix_action.as_deref() {
                Some("disable_blocks") => Some(FixAction::DisableMaxCacheBlocks),
                Some("disable_prefix_cache") => Some(FixAction::DisablePrefixCache),
                Some(prefix)
                    if prefix.starts_with("adjust_budget_")
                        || prefix.starts_with("set_budget_") =>
                {
                    let bytes_str = prefix
                        .strip_prefix("adjust_budget_")
                        .or_else(|| prefix.strip_prefix("set_budget_"))
                        .unwrap_or("0")
                        .parse::<u64>()
                        .unwrap_or(0);
                    if prefix.starts_with("adjust_budget_") {
                        Some(FixAction::AdjustPrefixCacheBudget(bytes_str))
                    } else {
                        Some(FixAction::SetPrefixCacheBudget(bytes_str))
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        findings.push(DoctorFinding {
            finding_type: DoctorFindingType::Cache,
            severity,
            message: finding.message,
            section: "cache".to_string(),
            fix,
        });
    }

    findings
}

async fn collect_reclaim_findings() -> Vec<DoctorFinding> {
    let mut findings = Vec::new();

    // Build current memory availability snapshot
    let snapshot = crate::memory_availability::build_snapshot();

    // Only suggest reclaim when state indicates memory pressure
    let should_suggest = matches!(
        snapshot.state,
        MemoryAvailabilityState::Unsafe | MemoryAvailabilityState::AfterClosingApps
    );

    if !should_suggest {
        return findings;
    }

    // Compute reclaim guidance
    let guidance = crate::system::compute_reclaim_guidance(&snapshot);

    // Add reclaim suggestion findings
    if !guidance.available_actions.is_empty() {
        findings.push(DoctorFinding {
            finding_type: DoctorFindingType::Cache,
            severity: match snapshot.state {
                MemoryAvailabilityState::Unsafe => DoctorSeverity::Issue,
                MemoryAvailabilityState::AfterClosingApps => DoctorSeverity::Warning,
                _ => DoctorSeverity::Warning,
            },
            message: format!(
                "Memory pressure detected ({:?}). {} {}",
                snapshot.state,
                guidance.conservative_estimate,
                if guidance.available_actions.len() > 1 {
                    format!("({} actions available)", guidance.available_actions.len())
                } else {
                    "Reclaim action available.".to_string()
                }
            ),
            section: "memory".to_string(),
            fix: guidance
                .available_actions
                .first()
                .map(|_| FixAction::ReclaimBackendAllocatorCache),
        });
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(
        has_cache_flag: bool,
    ) -> crate::inference::rapid_mlx::capabilities::CapabilitySnapshot {
        crate::inference::rapid_mlx::capabilities::CapabilitySnapshot {
            serve_flags: if has_cache_flag {
                vec![
                    "--host".into(),
                    "--port".into(),
                    "--max-cache-blocks".into(),
                ]
            } else {
                vec!["--host".into(), "--port".into()]
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn cache_finding_budget_exceeds_ceiling() {
        let snapshot = make_snapshot(true);
        let params = CacheDiagnosticParams {
            config_prefix_cache_enabled: true,
            config_prefix_cache_budget_bytes: Some(50_000_000_000), // 50 GB
            config_max_cache_blocks: None,
            snapshot: snapshot.clone(),
            configured_ceiling_bytes: 40_000_000_000, // 40 GB
            current_safe_availability_bytes: 30_000_000_000,
        };
        let findings = snapshot.compute_prefix_cache_findings(&params);
        assert!(
            findings
                .findings
                .iter()
                .any(|f| f.code == "CACHE_BUDGET_EXCEEDS_CEILING"),
            "Should detect budget > ceiling"
        );
        let finding = findings
            .findings
            .iter()
            .find(|f| f.code == "CACHE_BUDGET_EXCEEDS_CEILING")
            .unwrap();
        assert_eq!(finding.severity, "error");
        assert!(finding.fixable);
    }

    #[tokio::test]
    async fn cache_finding_blocks_unsupported() {
        let snapshot = make_snapshot(false); // no --max-cache-blocks flag
        let params = CacheDiagnosticParams {
            config_prefix_cache_enabled: false,
            config_prefix_cache_budget_bytes: None,
            config_max_cache_blocks: Some(200), // user set it
            snapshot: snapshot.clone(),
            configured_ceiling_bytes: 40_000_000_000,
            current_safe_availability_bytes: 30_000_000_000,
        };
        let findings = snapshot.compute_prefix_cache_findings(&params);
        assert!(
            findings
                .findings
                .iter()
                .any(|f| f.code == "CACHE_BLOCKS_UNSUPPORTED"),
            "Should detect unsupported max_cache_blocks"
        );
        let finding = findings
            .findings
            .iter()
            .find(|f| f.code == "CACHE_BLOCKS_UNSUPPORTED")
            .unwrap();
        assert_eq!(finding.severity, "warning");
        assert!(finding.fixable);
        assert_eq!(finding.fix_action, Some("disable_blocks".into()));
    }

    #[tokio::test]
    async fn cache_finding_enabled_no_budget_low_headroom() {
        let snapshot = make_snapshot(true);
        let params = CacheDiagnosticParams {
            config_prefix_cache_enabled: true,
            config_prefix_cache_budget_bytes: Some(0), // enabled but zero budget
            config_max_cache_blocks: None,
            snapshot: snapshot.clone(),
            configured_ceiling_bytes: 40_000_000_000,
            current_safe_availability_bytes: 8_000_000_000, // 20% headroom (< 30%)
        };
        let findings = snapshot.compute_prefix_cache_findings(&params);
        assert!(
            findings
                .findings
                .iter()
                .any(|f| f.code == "CACHE_ENABLED_NO_BUDGET_LOW_HEADROOM"),
            "Should detect enabled cache with no budget and low headroom"
        );
        let finding = findings
            .findings
            .iter()
            .find(|f| f.code == "CACHE_ENABLED_NO_BUDGET_LOW_HEADROOM")
            .unwrap();
        assert_eq!(finding.severity, "warning");
        assert!(finding.fixable);
        assert!(
            finding
                .fix_action
                .as_ref()
                .unwrap()
                .starts_with("set_budget_")
        );
    }

    #[tokio::test]
    async fn cache_finding_no_issues_when_within_limits() {
        let snapshot = make_snapshot(true);
        let params = CacheDiagnosticParams {
            config_prefix_cache_enabled: true,
            config_prefix_cache_budget_bytes: Some(4_000_000_000), // within ceiling
            config_max_cache_blocks: Some(200),                    // supported
            snapshot: snapshot.clone(),
            configured_ceiling_bytes: 40_000_000_000,
            current_safe_availability_bytes: 30_000_000_000, // healthy headroom
        };
        let findings = snapshot.compute_prefix_cache_findings(&params);
        assert!(
            findings.findings.is_empty(),
            "Should have no findings when configuration is healthy: {:?}",
            findings.findings
        );
    }

    #[test]
    fn supports_max_cache_blocks_detects_flag() {
        let with_flag = make_snapshot(true);
        let without_flag = make_snapshot(false);
        assert!(with_flag.supports_max_cache_blocks());
        assert!(!without_flag.supports_max_cache_blocks());
    }
}
