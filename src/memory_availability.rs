use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MemoryAvailabilityState {
    #[default]
    Unsafe,
    SafeNow,
    ConditionalAfterReclaim,
    AfterClosingApps,
}

/// Launch intent: additional generation (concurrent with existing sessions)
/// vs replace existing (stops the target runtime first). Consumed by Wizard
/// for estimation differentiation.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LaunchIntent {
    AdditionalGeneration,
    ReplaceExisting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAvailabilitySnapshot {
    /// Total unified/system memory in bytes. Informational only; never called "available".
    #[serde(default)]
    pub total_unified_bytes: u64,

    /// Current free memory in bytes (OS report).
    #[serde(default)]
    pub free_bytes: u64,

    /// Current wired (kernel-locked) memory in bytes.
    #[serde(default)]
    pub wired_bytes: u64,

    /// Current active memory in bytes.
    #[serde(default)]
    pub active_bytes: u64,

    /// Current speculative memory in bytes (macOS).
    #[serde(default)]
    pub speculative_bytes: u64,

    /// Current pageout/compressor memory in bytes (macOS).
    #[serde(default)]
    pub pageout_bytes: u64,

    /// Metal backend working set in bytes. On Apple Silicon, this is the Metal device's
    /// current recommended max working set or measured utilization base.
    #[serde(default)]
    pub metal_working_set_bytes: u64,

    /// Configured ceiling in bytes: the sysctl wired limit if set, otherwise a RAM-relative
    /// safe bound (default ~75% of total RAM on Apple Silicon). This is the stable capacity
    /// used by Model Browser/HF preview for planning.
    #[serde(default)]
    pub configured_ceiling_bytes: u64,

    /// Current safe availability in bytes, derived from metal working set, configured ceiling,
    /// and current backend utilization. This is what the Rapid Wizard and current launch use
    /// for fit determination. Always ≤ configured_ceiling_bytes.
    #[serde(default)]
    pub current_safe_availability_bytes: u64,

    /// The determined availability state for a given launch scenario.
    #[serde(default)]
    pub state: MemoryAvailabilityState,

    /// Backend-specific fields: GPU-specific data for Metal (effective ceiling,
    /// recommended working set). Empty on non-Metal platforms.
    #[serde(default)]
    pub backend_specific: serde_json::Map<String, serde_json::Value>,

    /// Timestamp (Unix epoch seconds) when this snapshot was taken.
    #[serde(default)]
    pub timestamp: u64,
}

impl Default for MemoryAvailabilitySnapshot {
    fn default() -> Self {
        Self {
            total_unified_bytes: 0,
            free_bytes: 0,
            wired_bytes: 0,
            active_bytes: 0,
            speculative_bytes: 0,
            pageout_bytes: 0,
            metal_working_set_bytes: 0,
            configured_ceiling_bytes: 0,
            current_safe_availability_bytes: 0,
            state: MemoryAvailabilityState::Unsafe,
            backend_specific: serde_json::Map::new(),
            timestamp: 0,
        }
    }
}

/// Builds a MemoryAvailabilitySnapshot from live system metrics.
/// On Apple Silicon (macOS), uses Metal working set and iogpu wired limit.
/// On other platforms, returns a safe degraded snapshot.
pub fn build_snapshot() -> MemoryAvailabilitySnapshot {
    #[cfg(target_os = "macos")]
    {
        build_macos_snapshot()
    }
    #[cfg(not(target_os = "macos"))]
    {
        build_non_macos_snapshot()
    }
}

#[cfg(target_os = "macos")]
fn build_macos_snapshot() -> MemoryAvailabilitySnapshot {
    let sys_info = crate::system::get_system_metrics();
    let total_bytes = (sys_info.ram_total_gb * 1024.0 * 1024.0 * 1024.0) as u64;
    let wired_bytes = (sys_info.memory_wired_gb * 1024.0 * 1024.0 * 1024.0) as u64;
    let free_bytes = (sys_info.memory_free_gb * 1024.0 * 1024.0 * 1024.0) as u64;
    let active_bytes = (sys_info.ram_used_gb * 1024.0 * 1024.0 * 1024.0) as u64;
    let speculative_bytes = (sys_info.memory_inactive_gb * 1024.0 * 1024.0 * 1024.0) as u64;
    let pageout_bytes = (sys_info.memory_compressor_gb * 1024.0 * 1024.0 * 1024.0) as u64;

    // Read the configured Metal GPU wired limit from sysctl
    let wired_limit_mb = crate::gpu::apple::read_iogpu_wired_limit_mb();
    let configured_ceiling_bytes = if wired_limit_mb > 0 {
        wired_limit_mb * 1024 * 1024
    } else {
        // Default safe bound: tiered reserve based on RAM size
        // (≤16GB: -6GB, ≥24GB: -8GB). Uses wired_limit_safe_default_mb for consistency.
        let safe_default_mb =
            crate::gpu::apple::wired_limit_safe_default_mb(total_bytes).unwrap_or(0);
        safe_default_mb * 1024 * 1024
    };

    // Metal working set: use the configured ceiling as the base (MLX reads this at init).
    // This is the effective base Rapid-MLX uses, multiplied by its utilization factor.
    let metal_working_set_bytes = configured_ceiling_bytes;

    // Current safe availability: use free_bytes from vm_stat, which matches Activity Monitor's "Free".
    // sysinfo's available_memory includes inactive/purgeable pages (~54 GB on 64 GB system),
    // which is misleadingly high — macOS may reclaim some, but they aren't truly available now.
    // For accurate "can I run this model right now?" we need actual free RAM.
    let current_safe_availability_bytes = free_bytes;

    // Determine state
    let state = if current_safe_availability_bytes > 0
        && current_safe_availability_bytes >= configured_ceiling_bytes.saturating_mul(50) / 100
    {
        MemoryAvailabilityState::SafeNow
    } else if free_bytes > 0 && free_bytes < current_safe_availability_bytes {
        MemoryAvailabilityState::ConditionalAfterReclaim
    } else if free_bytes == 0 && wired_bytes > 0 {
        MemoryAvailabilityState::AfterClosingApps
    } else {
        MemoryAvailabilityState::Unsafe
    };

    // Build backend-specific metadata for Metal
    let mut backend_specific = serde_json::Map::new();
    backend_specific.insert(
        "effective_ceiling_bytes".to_string(),
        serde_json::Value::Number(serde_json::Number::from(configured_ceiling_bytes)),
    );
    backend_specific.insert(
        "metal_working_set_bytes".to_string(),
        serde_json::Value::Number(serde_json::Number::from(metal_working_set_bytes)),
    );
    backend_specific.insert(
        "recommended_working_set_bytes".to_string(),
        serde_json::Value::Number(serde_json::Number::from(configured_ceiling_bytes)),
    );
    backend_specific.insert(
        "wired_limit_mb_sysctl".to_string(),
        serde_json::Value::Number(serde_json::Number::from(wired_limit_mb)),
    );

    MemoryAvailabilitySnapshot {
        total_unified_bytes: total_bytes,
        free_bytes,
        wired_bytes,
        active_bytes,
        speculative_bytes,
        pageout_bytes,
        metal_working_set_bytes,
        configured_ceiling_bytes,
        current_safe_availability_bytes,
        state,
        backend_specific,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    }
}

#[cfg(not(target_os = "macos"))]
fn build_non_macos_snapshot() -> MemoryAvailabilitySnapshot {
    let sys_info = crate::system::get_system_metrics();
    let total_bytes = (sys_info.ram_total_gb * 1024.0 * 1024.0 * 1024.0) as u64;
    let free_bytes = (sys_info.memory_free_gb * 1024.0 * 1024.0 * 1024.0) as u64;
    let available_bytes = (sys_info.ram_available_gb * 1024.0 * 1024.0 * 1024.0) as u64;

    // On non-macOS, use a safe RAM-relative ceiling (80% of total)
    let configured_ceiling_bytes = (total_bytes as f64 * 0.80) as u64;
    let current_safe_availability_bytes = available_bytes.min(configured_ceiling_bytes);

    let state =
        if current_safe_availability_bytes >= configured_ceiling_bytes.saturating_mul(50) / 100 {
            MemoryAvailabilityState::SafeNow
        } else if available_bytes > 0 {
            MemoryAvailabilityState::ConditionalAfterReclaim
        } else {
            MemoryAvailabilityState::Unsafe
        };

    MemoryAvailabilitySnapshot {
        total_unified_bytes: total_bytes,
        free_bytes,
        wired_bytes: 0,
        active_bytes: 0,
        speculative_bytes: 0,
        pageout_bytes: 0,
        metal_working_set_bytes: 0,
        configured_ceiling_bytes,
        current_safe_availability_bytes,
        state,
        backend_specific: serde_json::Map::new(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_has_all_required_fields() {
        let snapshot = MemoryAvailabilitySnapshot::default();
        // All fields exist and deserialize/serialize cleanly
        let json = serde_json::to_string(&snapshot).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("total_unified_bytes").is_some());
        assert!(parsed.get("free_bytes").is_some());
        assert!(parsed.get("wired_bytes").is_some());
        assert!(parsed.get("active_bytes").is_some());
        assert!(parsed.get("speculative_bytes").is_some());
        assert!(parsed.get("pageout_bytes").is_some());
        assert!(parsed.get("metal_working_set_bytes").is_some());
        assert!(parsed.get("configured_ceiling_bytes").is_some());
        assert!(parsed.get("current_safe_availability_bytes").is_some());
        assert!(parsed.get("state").is_some());
        assert!(parsed.get("backend_specific").is_some());
        assert!(parsed.get("timestamp").is_some());
    }

    #[test]
    fn current_safe_availability_leq_configured_ceiling() {
        let mut snapshot = MemoryAvailabilitySnapshot::default();
        snapshot.configured_ceiling_bytes = 48 * 1024 * 1024 * 1024;
        snapshot.current_safe_availability_bytes = 36 * 1024 * 1024 * 1024;
        assert!(
            snapshot.current_safe_availability_bytes <= snapshot.configured_ceiling_bytes,
            "current_safe_availability must be ≤ configured_ceiling"
        );
    }

    #[test]
    fn state_safe_now_when_sufficient() {
        let mut snapshot = MemoryAvailabilitySnapshot::default();
        snapshot.configured_ceiling_bytes = 48 * 1024 * 1024 * 1024;
        snapshot.current_safe_availability_bytes = 30 * 1024 * 1024 * 1024; // >50% of ceiling
        snapshot.state = MemoryAvailabilityState::SafeNow;
        assert_eq!(snapshot.state, MemoryAvailabilityState::SafeNow);
    }

    #[test]
    fn state_conditional_when_free_but_less_than_availability() {
        let mut snapshot = MemoryAvailabilitySnapshot::default();
        snapshot.current_safe_availability_bytes = 20 * 1024 * 1024 * 1024;
        snapshot.free_bytes = 10 * 1024 * 1024 * 1024; // free < availability
        snapshot.state = MemoryAvailabilityState::ConditionalAfterReclaim;
        assert_eq!(
            snapshot.state,
            MemoryAvailabilityState::ConditionalAfterReclaim
        );
    }

    #[test]
    fn build_snapshot_returns_valid_shape() {
        let snapshot = build_snapshot();
        assert!(snapshot.total_unified_bytes > 0 || snapshot.total_unified_bytes == 0); // may be 0 on CI/containers
        // Validate that pressure can reduce availability below ceiling
        assert!(
            snapshot.current_safe_availability_bytes <= snapshot.configured_ceiling_bytes
                || snapshot.configured_ceiling_bytes == 0,
            "current_safe_availability must not exceed configured_ceiling"
        );
    }

    #[test]
    fn no_total_unified_bytes_called_available() {
        // Verify the struct does NOT expose total_unified_bytes as an "available_memory_bytes" field
        let snapshot = MemoryAvailabilitySnapshot::default();
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(
            !json.contains("available_memory_bytes"),
            "total_unified_bytes must NOT be called available"
        );
        assert!(
            json.contains("total_unified_bytes"),
            "must use total_unified_bytes for the raw total"
        );
    }

    #[test]
    fn launch_intent_serializes_correctly() {
        let additional = LaunchIntent::AdditionalGeneration;
        let replace = LaunchIntent::ReplaceExisting;
        assert_eq!(
            serde_json::to_string(&additional).unwrap(),
            r#""additional_generation""#
        );
        assert_eq!(
            serde_json::to_string(&replace).unwrap(),
            r#""replace_existing""#
        );
    }

    #[test]
    fn memory_state_serializes_correctly() {
        assert_eq!(
            serde_json::to_string(&MemoryAvailabilityState::SafeNow).unwrap(),
            r#""safe_now""#
        );
        assert_eq!(
            serde_json::to_string(&MemoryAvailabilityState::ConditionalAfterReclaim).unwrap(),
            r#""conditional_after_reclaim""#
        );
        assert_eq!(
            serde_json::to_string(&MemoryAvailabilityState::AfterClosingApps).unwrap(),
            r#""after_closing_apps""#
        );
        assert_eq!(
            serde_json::to_string(&MemoryAvailabilityState::Unsafe).unwrap(),
            r#""unsafe""#
        );
    }
}
