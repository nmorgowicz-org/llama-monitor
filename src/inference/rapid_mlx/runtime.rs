use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSource {
    Managed,
    Homebrew,
    Pip,
    Pipx,
    Custom,
    PathUnknown,
}

/// On-device update-validation probe result tiers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeResult {
    /// All checks passed; environment is healthy.
    Pass,
    /// Rapid-MLX itself is broken; managed install is rollback-eligible.
    CriticalFail {
        /// Concrete, actionable message about what failed.
        message: String,
    },
    /// Baseline passes but specific optional capability(s) failed.
    PerFeatureFail {
        /// Actionable per-feature diagnoses; never a global banner.
        feature_failures: Vec<FeatureProbeFailure>,
    },
}

/// Actionable diagnosis for a single optional capability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureProbeFailure {
    pub feature: String,
    pub message: String,
}

impl Default for ProbeResult {
    fn default() -> Self {
        Self::CriticalFail {
            message: "Not probed".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMetadata {
    pub executable_path: std::path::PathBuf,
    pub source: RuntimeSource,
    pub version: String,
    /// Capability snapshot for this runtime (auto-generated; may be None if not yet probed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_snapshot: Option<crate::inference::rapid_mlx::capabilities::CapabilitySnapshot>,
    /// Resolved dependency receipt for managed installs. Records the exact packages
    /// that were installed with this runtime environment. Never hand-curated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_receipt: Option<ResolvedDependencyReceipt>,
    /// Last on-device probe result (user-driven, post-install/upgrade).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_probe_result: Option<ProbeResult>,
}

/// Resolved receipt of dependency installation for a managed environment.
/// Preserves the exact packages installed from Rapid's upstream contract + supported extras.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedDependencyReceipt {
    pub rapid_mlx_version: String,
    /// Exact resolved versions of all installed packages.
    pub packages: Vec<ResolvedPackage>,
    /// When this receipt was generated.
    pub installed_at: u64,
    /// Whether this environment is known-good and eligible for rollback.
    pub rollback_eligible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedPackage {
    pub name: String,
    pub version: String,
}

impl Default for RuntimeMetadata {
    fn default() -> Self {
        Self {
            executable_path: std::path::PathBuf::new(),
            source: RuntimeSource::PathUnknown,
            version: String::new(),
            capability_snapshot: None,
            resolved_receipt: None,
            last_probe_result: None,
        }
    }
}
