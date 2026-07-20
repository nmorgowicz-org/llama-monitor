use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use super::mactop_cache::{self, MactopCacheEntry};
use super::{GpuBackend, GpuMetrics};

#[derive(Deserialize)]
struct MactopOutput {
    soc_metrics: SocMetrics,
    memory: MemoryMetrics,
    // gpu_usage is redundant with soc_metrics.gpu_active
}

#[derive(Deserialize)]
struct SocMetrics {
    #[serde(default)]
    gpu_power: f64,
    #[serde(default)]
    cpu_power: f64,
    #[serde(default)]
    total_power: f64,
    #[serde(default)]
    gpu_freq_mhz: f64,
    #[serde(default)]
    gpu_temp: f64,
    #[serde(default)]
    gpu_active: f64,
    #[serde(default)]
    cpu_temp: f64,
    #[serde(default)]
    dram_read_bw_gbs: f64,
    #[serde(default)]
    dram_write_bw_gbs: f64,
    /// Current P-cluster frequency (MHz)
    #[serde(default)]
    p_cluster_freq_mhz: f64,
    /// Current S-cluster frequency (MHz)
    #[serde(default)]
    s_cluster_freq_mhz: f64,
    /// Current E-cluster frequency (MHz)
    #[serde(default)]
    e_cluster_freq_mhz: f64,
    /// P-cluster utilization (%)
    #[serde(default)]
    p_cluster_active: f64,
    /// S-cluster utilization (%)
    #[serde(default)]
    s_cluster_active: f64,
    /// E-cluster utilization (%)
    #[serde(default)]
    e_cluster_active: f64,
}

#[derive(Deserialize)]
struct MemoryMetrics {
    total: u64, // bytes
    used: u64,  // bytes
}

pub struct AppleBackend {
    last_cpu_temp: Mutex<f32>,
}

impl Default for AppleBackend {
    fn default() -> Self {
        AppleBackend {
            last_cpu_temp: Mutex::new(0.0),
        }
    }
}
impl AppleBackend {
    pub fn new() -> Self {
        AppleBackend {
            last_cpu_temp: Mutex::new(0.0),
        }
    }
}

impl GpuBackend for AppleBackend {
    fn read_metrics(&self) -> Result<BTreeMap<String, GpuMetrics>> {
        let output = Command::new("mactop")
            .args(["--headless", "--count", "1", "--format", "json"])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "mactop failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let mut mactop_vec: Vec<MactopOutput> = serde_json::from_slice(&output.stdout)
            .map_err(|e| anyhow::anyhow!("Failed to parse mactop JSON: {}", e))?;
        let mactop_output = mactop_vec
            .pop()
            .ok_or_else(|| anyhow::anyhow!("mactop returned empty JSON array"))?;

        // Cache CPU/SoC temperature for cpu_temp()
        if mactop_output.soc_metrics.cpu_temp > 0.0
            && let Ok(mut t) = self.last_cpu_temp.lock()
        {
            *t = mactop_output.soc_metrics.cpu_temp as f32;
        }

        // Populate shared mactop cache for system.rs to read cluster freq / power / load
        let soc = &mactop_output.soc_metrics;
        mactop_cache::set_cache(MactopCacheEntry {
            power_total_w: soc.total_power as f32,
            power_cpu_w: soc.cpu_power as f32,
            power_gpu_w: soc.gpu_power as f32,
            p_cluster_freq_mhz: soc.p_cluster_freq_mhz as u32,
            s_cluster_freq_mhz: soc.s_cluster_freq_mhz as u32,
            e_cluster_freq_mhz: soc.e_cluster_freq_mhz as u32,
            p_cluster_active: soc.p_cluster_active as f32,
            s_cluster_active: soc.s_cluster_active as f32,
            e_cluster_active: soc.e_cluster_active as f32,
        });

        // Convert bytes to MB
        let vram_total_mb = mactop_output.memory.total / (1024 * 1024);
        let vram_used_mb = mactop_output.memory.used / (1024 * 1024);

        // Estimate memory clock from DRAM bandwidth
        // Approximate: MCLK = (dram_bw_gbs * 1000) / 8 / 2 (DDR)
        let mclk_mhz = (soc.dram_read_bw_gbs + soc.dram_write_bw_gbs) * 1000.0 / 16.0;

        let metrics = GpuMetrics {
            temp: soc.gpu_temp as f32,
            load: soc.gpu_active as u32,
            power_consumption: soc.gpu_power as f32,
            power_limit: 0, // Not available from mactop
            vram_used: vram_used_mb as u64,
            vram_total: vram_total_mb as u64,
            sclk_mhz: soc.gpu_freq_mhz as u32,
            mclk_mhz: mclk_mhz as u32,
            metal_gpu_limit_mb: Some(read_iogpu_wired_limit_mb()),
        };

        let mut map = BTreeMap::new();
        map.insert(format!("GPU0 {}", detect_chip_name()), metrics);
        Ok(map)
    }

    fn cpu_temp(&self) -> Option<f32> {
        let t = *self.last_cpu_temp.lock().ok()?;
        if t > 0.0 { Some(t) } else { None }
    }

    fn name(&self) -> &str {
        "apple"
    }
}

fn detect_chip_name() -> &'static str {
    static CHIP_NAME: OnceLock<String> = OnceLock::new();
    CHIP_NAME.get_or_init(|| {
        std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Apple Silicon".to_string())
    })
}

/// Read `iogpu.wired_limit_mb` from the kernel each call.
/// Returns 0 if unset (system default: ~66% for ≤36 GB RAM, ~75% for larger).
/// Not cached — the value changes when the user applies the Metal GPU limit tweak.
/// The sysctl call takes ~10–50 µs, negligible against the metrics poll interval.
pub fn read_iogpu_wired_limit_mb() -> u64 {
    std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "iogpu.wired_limit_mb"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

/// Maximum fraction of total RAM allowed for the wired limit.
/// Rationale: on Apple Silicon with unified memory, the GPU wired pool is
/// carved from the same physical memory used by the OS, CPU, and all apps.
/// Allowing the wired limit to consume too much of RAM causes:
/// - System UI freezes and launch delays (kernel cannot wire critical structures)
/// - MLX process OOM kills under high KV pressure (no room for active weights)
/// - Aggressive swap thrashing (compressor exhaustion)
///
/// Apple's own sysctl default is ~66% for ≤36 GB RAM and ~75% for larger.
/// We permit up to 88% as the hard ceiling. This accommodates the user-verified
/// M5 Max path (57,344 MiB on 64 GB = 87.5%) while preserving ~12% for OS/kernel
/// and non-wired app memory. Going above 90% would risk kernel wiring failures
/// under sustained GPU pressure.
const WIRED_LIMIT_MAX_FRACTION: f64 = 0.88;

/// Compute the maximum allowed wired limit in MiB for this machine.
/// Returns None if total RAM cannot be determined.
/// The bound is floor(total_ram_miB × 0.88) to protect OS stability.
pub fn wired_limit_max_mb(total_ram_bytes: u64) -> Option<u64> {
    if total_ram_bytes == 0 {
        return None;
    }
    let total_ram_mb = total_ram_bytes / (1024 * 1024);
    let max_mb = (total_ram_mb as f64 * WIRED_LIMIT_MAX_FRACTION) as u64;
    Some(max_mb.max(1))
}

/// Compute the RAM-relative safe default wired limit when sysctl is unset (0).
/// Matches Apple's behavior: ~75% of total RAM on Apple Silicon.
/// This is the configured_ceiling_bytes default used by MemoryAvailabilitySnapshot.
pub fn wired_limit_safe_default_mb(total_ram_bytes: u64) -> Option<u64> {
    if total_ram_bytes == 0 {
        return None;
    }
    let total_ram_mb = total_ram_bytes / (1024 * 1024);
    Some((total_ram_mb as f64 * 0.75) as u64)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WiredLimitError {
    /// The requested value exceeds the RAM-relative safe maximum.
    ExceedsRamBound { requested_mb: u64, max_mb: u64 },
    /// The sysctl write succeeded but readback shows actual < requested.
    /// This indicates kernel rejection or partial application.
    ReadbackMismatch { requested_mb: u64, actual_mb: u64 },
    /// The sysctl command failed (requires root, permission denied, etc.).
    SysctlFailed { reason: String },
    /// The value is already at the requested setting.
    AlreadySet { current_mb: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WiredLimitSetResult {
    /// Whether the operation completed successfully.
    pub success: bool,
    /// The actual value now in effect, in MiB.
    pub actual_mb: u64,
    /// The previous value before this operation, in MiB.
    pub previous_mb: u64,
    /// Error details if success is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<WiredLimitError>,
}

/// Set the `iogpu.wired_limit_mb` sysctl with full hardening:
/// - Bounded: refused if above RAM-relative max (88% of total RAM)
/// - Readback-verified: actual value verified >= requested after write
/// - Reversible: stores previous value for restore
/// - Restart-aware: value may reset on reboot (documented, not claimed persistent)
/// - Persistence-qualified: on verified M5 Max, persists across reboot;
///   cross-version behavior not claimed without evidence
///
/// Requires elevated privileges (admin password via osascript).
pub fn set_iogpu_wired_limit_mb(requested_mb: u64, total_ram_bytes: u64) -> WiredLimitSetResult {
    // 1. Bounded: enforce RAM-relative safe maximum
    let max_mb = match wired_limit_max_mb(total_ram_bytes) {
        Some(m) => m,
        None => {
            return WiredLimitSetResult {
                success: false,
                actual_mb: 0,
                previous_mb: 0,
                error: Some(WiredLimitError::SysctlFailed {
                    reason: "Cannot determine total RAM".into(),
                }),
            };
        }
    };

    if requested_mb > max_mb {
        return WiredLimitSetResult {
            success: false,
            actual_mb: read_iogpu_wired_limit_mb(),
            previous_mb: read_iogpu_wired_limit_mb(),
            error: Some(WiredLimitError::ExceedsRamBound {
                requested_mb,
                max_mb,
            }),
        };
    }

    // 2. Capture previous value before mutation
    let previous_mb = read_iogpu_wired_limit_mb();

    // 3. No-op if already at requested value
    if previous_mb == requested_mb {
        return WiredLimitSetResult {
            success: true,
            actual_mb: previous_mb,
            previous_mb,
            error: Some(WiredLimitError::AlreadySet {
                current_mb: previous_mb,
            }),
        };
    }

    // 4. Write via osascript for elevated privileges
    let script = format!(
        r#"do shell script "sysctl -w iogpu.wired_limit_mb={}" with administrator privileges"#,
        requested_mb
    );
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output();

    let write_ok = match output {
        Ok(o) if o.status.success() => true,
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            // User cancelled the auth dialog
            if stderr.contains("cancelled") || stderr.contains("canceled") {
                return WiredLimitSetResult {
                    success: false,
                    actual_mb: previous_mb,
                    previous_mb,
                    error: Some(WiredLimitError::SysctlFailed {
                        reason: "Authorization cancelled by user".into(),
                    }),
                };
            }
            return WiredLimitSetResult {
                success: false,
                actual_mb: previous_mb,
                previous_mb,
                error: Some(WiredLimitError::SysctlFailed {
                    reason: format!("sysctl write failed: {}", stderr.trim()),
                }),
            };
        }
        Err(e) => {
            return WiredLimitSetResult {
                success: false,
                actual_mb: previous_mb,
                previous_mb,
                error: Some(WiredLimitError::SysctlFailed {
                    reason: format!("Could not run sysctl: {}", e),
                }),
            };
        }
    };

    if !write_ok {
        return WiredLimitSetResult {
            success: false,
            actual_mb: previous_mb,
            previous_mb,
            error: Some(WiredLimitError::SysctlFailed {
                reason: "sysctl write returned non-zero".into(),
            }),
        };
    }

    // 5. Readback verification: confirm actual >= requested
    let actual_mb = read_iogpu_wired_limit_mb();
    if actual_mb < requested_mb {
        // Readback shows value not fully applied — leave kernel intact (it's at actual_mb).
        // The previous value was already superseded by write; current effective is actual_mb.
        return WiredLimitSetResult {
            success: false,
            actual_mb,
            previous_mb,
            error: Some(WiredLimitError::ReadbackMismatch {
                requested_mb,
                actual_mb,
            }),
        };
    }

    WiredLimitSetResult {
        success: true,
        actual_mb,
        previous_mb,
        error: None,
    }
}

/// Restore the wired limit to a previous value or to the macOS default (0).
/// Passing `restore_to = 0` clears any explicit limit, returning to Apple's
/// built-in default (~66% or ~75% depending on RAM size).
///
/// Restart-aware: macOS does not guarantee `iogpu.wired_limit_mb` persists
/// across reboot. If the user's system (e.g. verified M5 Max) shows persistence,
/// that is an empirical observation, not a universal guarantee.
/// MLX reads this sysctl at Metal device init; a new value takes effect only
/// after the MLX/Rapid process restarts.
#[allow(dead_code)]
pub fn restore_iogpu_wired_limit_mb(
    restore_to_mb: u64,
    total_ram_bytes: u64,
) -> WiredLimitSetResult {
    if restore_to_mb == 0 {
        // Clearing to default
        set_iogpu_wired_limit_mb(0, total_ram_bytes)
    } else {
        set_iogpu_wired_limit_mb(restore_to_mb, total_ram_bytes)
    }
}

/// Documented behavior notes (for API responses and frontend teaching):
/// - Session-only: The sysctl change is active for the current boot. On some
///   macOS versions (verified on M5 Max), it persists across reboot; on others,
///   it resets to the Apple default. Do not rely on persistence without testing
///   on the target machine and macOS version.
/// - MLX restart required: MLX queries `iogpu.wired_limit_mb` at Metal device
///   initialization. An existing MLX/Rapid process does NOT pick up a new value.
///   Restart the model runtime after changing this limit.
/// - M5 Max 57344 MiB path: User-verified as reboot-persistent on their M5 Max.
///   Mechanism not formally documented by Apple; do not generalize to other
///   configurations without evidence.
pub fn wired_limit_behavior_notes() -> &'static str {
    "The iogpu.wired_limit_mb sysctl is session-only by design. On some macOS \
     versions (verified M5 Max), it persists across reboot; this is not guaranteed \
     on all machines. MLX reads the value at device init; restart the runtime \
     after changing this limit for it to take effect."
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 32 GiB RAM system
    const RAM_32GB_BYTES: u64 = 32 * 1024 * 1024 * 1024;
    /// 64 GiB RAM system (M5 Max class)
    const RAM_64GB_BYTES: u64 = 64 * 1024 * 1024 * 1024;

    #[test]
    fn wired_limit_max_mb_32gb() {
        let max = wired_limit_max_mb(RAM_32GB_BYTES).unwrap();
        let expected = (32_768_f64 * 0.88) as u64;
        assert_eq!(max, expected);
        assert!(max > 0);
    }

    #[test]
    fn wired_limit_max_mb_64gb() {
        let max = wired_limit_max_mb(RAM_64GB_BYTES).unwrap();
        let expected = (65_536_f64 * 0.88) as u64;
        assert_eq!(max, expected);
        assert!(max > 0);
    }

    #[test]
    fn wired_limit_max_mb_zero_ram() {
        assert_eq!(wired_limit_max_mb(0), None);
    }

    #[test]
    fn wired_limit_safe_default_mb_64gb() {
        let default_mb = wired_limit_safe_default_mb(RAM_64GB_BYTES).unwrap();
        let expected = (65_536_f64 * 0.75) as u64;
        assert_eq!(default_mb, expected);
    }

    #[test]
    fn wired_limit_safe_default_mb_zero_ram() {
        assert_eq!(wired_limit_safe_default_mb(0), None);
    }

    #[test]
    fn m5_max_57344_path_within_bounds() {
        let max = wired_limit_max_mb(RAM_64GB_BYTES).unwrap();
        assert!(
            57_344 <= max,
            "M5 Max verified path 57344 must be within bounds (max={})",
            max
        );
    }

    #[test]
    fn set_exceeds_ram_bound_rejected() {
        let max = wired_limit_max_mb(RAM_32GB_BYTES).unwrap();
        let result = set_iogpu_wired_limit_mb(max + 1000, RAM_32GB_BYTES);
        assert!(!result.success);
        match result.error {
            Some(WiredLimitError::ExceedsRamBound {
                requested_mb,
                max_mb,
            }) => {
                assert_eq!(requested_mb, max + 1000);
                assert_eq!(max_mb, max);
            }
            other => panic!("Expected ExceedsRamBound, got {:?}", other),
        }
    }

    #[test]
    fn set_within_bounds_allows_m5_max_value() {
        let result = set_iogpu_wired_limit_mb(57_344, RAM_64GB_BYTES);
        if let Some(ref err) = result.error {
            match err {
                WiredLimitError::ExceedsRamBound { .. } => {
                    panic!("M5 Max path incorrectly rejected by bounds check")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn restore_to_zero_clears_to_default() {
        let result = restore_iogpu_wired_limit_mb(0, RAM_32GB_BYTES);
        if let Some(ref err) = result.error {
            match err {
                WiredLimitError::ExceedsRamBound { .. } => {
                    panic!("Restore to 0 should not trigger bounds error")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn restore_to_previous_value_is_valid() {
        let result = restore_iogpu_wired_limit_mb(49_152, RAM_64GB_BYTES);
        if let Some(ref err) = result.error {
            match err {
                WiredLimitError::ExceedsRamBound { .. } => {
                    panic!("Restore to valid value rejected by bounds")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn behavior_notes_contain_required_info() {
        let notes = wired_limit_behavior_notes();
        assert!(
            notes.contains("session-only") || notes.contains("not guaranteed"),
            "Notes must indicate session-only/non-persistent behavior"
        );
        assert!(
            notes.contains("restart") || notes.contains("MLX reads"),
            "Notes must indicate MLX restart requirement"
        );
    }

    #[test]
    fn wired_limit_error_serializes() {
        let err = WiredLimitError::ExceedsRamBound {
            requested_mb: 100_000,
            max_mb: 50_000,
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("exceeds_ram_bound"));

        let err = WiredLimitError::ReadbackMismatch {
            requested_mb: 50_000,
            actual_mb: 0,
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("readback_mismatch"));
    }

    #[test]
    fn wired_limit_set_result_serializes() {
        let result = WiredLimitSetResult {
            success: true,
            actual_mb: 49_152,
            previous_mb: 0,
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains(r#""success":true"#));
        assert!(json.contains(r#""actual_mb":49152"#));
        assert!(json.contains(r#""previous_mb":0"#));
        assert!(!json.contains("error"));
    }

    #[test]
    fn wired_limit_set_result_with_error_serializes() {
        let result = WiredLimitSetResult {
            success: false,
            actual_mb: 0,
            previous_mb: 49_152,
            error: Some(WiredLimitError::ReadbackMismatch {
                requested_mb: 60_000,
                actual_mb: 0,
            }),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains(r#""success":false"#));
        assert!(json.contains("readback_mismatch"));
    }
}
