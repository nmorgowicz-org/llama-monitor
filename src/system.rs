#[cfg(target_os = "macos")]
use crate::gpu::mactop_cache;
use sysinfo::System;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemMetrics {
    #[serde(default)]
    pub cpu_name: String,
    #[serde(default)]
    pub cpu_temp: f32,
    #[serde(default)]
    pub cpu_temp_available: bool,
    #[serde(default)]
    pub cpu_load: u32,
    #[serde(default)]
    pub cpu_clock_mhz: u32,
    #[serde(default)]
    pub ram_total_gb: f64,
    #[serde(default)]
    pub ram_used_gb: f64,
    #[serde(default)]
    pub ram_available_gb: f64,
    #[serde(default)]
    pub memory_pressure_level: String,
    #[serde(default)]
    pub memory_pressure_source: String,
    #[serde(default)]
    pub memory_pressure_score: f64,
    #[serde(default)]
    pub memory_free_gb: f64,
    #[serde(default)]
    pub memory_compressor_gb: f64,
    #[serde(default)]
    pub memory_compressed_gb: f64,
    /// S-core count. On Apple Silicon this is derived from the first L2-group size of
    /// perflevel1 when the chip has two sub-clusters inside the secondary perf level.
    /// Returns 0 on non-macOS or when only one perf-level cluster exists.
    #[serde(default)]
    pub s_cores: u32,
    /// Human-readable name for the P-core cluster from `hw.perflevel0.name` (e.g. "Super").
    #[serde(default)]
    pub p_cluster_name: String,
    /// Human-readable name for the secondary cluster from `hw.perflevel1.name` (e.g. "Performance").
    #[serde(default)]
    pub secondary_cluster_name: String,
    /// Wired (kernel-locked) memory in GB. Non-compressible; includes GPU framebuffers
    /// and kernel allocations. On Apple Silicon this is the Metal iogpu budget floor.
    #[serde(default)]
    pub memory_wired_gb: f64,
    /// Purgeable memory in GB (macOS only). File-backed pages the kernel can drop
    /// instantly without writing to disk — effectively free on demand.
    #[serde(default)]
    pub memory_purgeable_gb: f64,
    /// Inactive memory in GB (macOS only). Candidate pages for compression or eviction.
    #[serde(default)]
    pub memory_inactive_gb: f64,
    /// Cross-platform estimate of memory the OS can reclaim without killing a process.
    #[serde(default)]
    pub memory_reclaimable_gb: f64,
    /// Swap/pagefile currently in use, where the platform exposes it.
    #[serde(default)]
    pub swap_used_gb: f64,
    #[serde(default)]
    pub swapins: u64,
    #[serde(default)]
    pub swapouts: u64,
    #[serde(default)]
    pub swapins_delta: u64,
    #[serde(default)]
    pub swapouts_delta: u64,
    #[serde(default)]
    pub memory_psi_some_avg10: f64,
    #[serde(default)]
    pub memory_psi_full_avg10: f64,
    #[serde(default)]
    pub memory_pressure_advice: String,
    #[serde(default)]
    pub motherboard: String,
    /// Performance-core count (Apple Silicon only; 0 = unknown/not applicable).
    #[serde(default)]
    pub p_cores: u32,
    /// Efficiency-core count (Apple Silicon only; 0 = unknown/not applicable).
    #[serde(default)]
    pub e_cores: u32,
    /// Total system power drawn (Watts). Populated on macOS via mactop.
    #[serde(default)]
    pub power_total_w: f32,
    /// CPU power drawn (Watts). Populated on macOS via mactop.
    #[serde(default)]
    pub power_cpu_w: f32,
    /// GPU power drawn (Watts). Populated on macOS via mactop.
    #[serde(default)]
    pub power_gpu_w: f32,
    /// P-cluster current frequency (MHz). Populated on macOS via mactop.
    #[serde(default)]
    pub p_cluster_freq_mhz: u32,
    /// S-cluster current frequency (MHz). Populated on macOS via mactop.
    #[serde(default)]
    pub s_cluster_freq_mhz: u32,
    /// E-cluster current frequency (MHz). Populated on macOS via mactop.
    #[serde(default)]
    pub e_cluster_freq_mhz: u32,
    /// P-cluster utilization (%). Populated on macOS via mactop.
    #[serde(default)]
    pub p_cluster_active: f32,
    /// S-cluster utilization (%). Populated on macOS via mactop.
    #[serde(default)]
    pub s_cluster_active: f32,
    /// E-cluster utilization (%). Populated on macOS via mactop.
    #[serde(default)]
    pub e_cluster_active: f32,
    // Default: all new fields are 0/false for non-macOS builds
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            cpu_name: String::new(),
            cpu_temp: 0.0,
            cpu_temp_available: false,
            cpu_load: 0,
            cpu_clock_mhz: 0,
            ram_total_gb: 0.0,
            ram_used_gb: 0.0,
            ram_available_gb: 0.0,
            memory_pressure_level: String::new(),
            memory_pressure_source: String::new(),
            memory_pressure_score: 0.0,
            memory_free_gb: 0.0,
            memory_compressor_gb: 0.0,
            memory_compressed_gb: 0.0,
            s_cores: 0,
            p_cluster_name: String::new(),
            secondary_cluster_name: String::new(),
            memory_wired_gb: 0.0,
            memory_purgeable_gb: 0.0,
            memory_inactive_gb: 0.0,
            memory_reclaimable_gb: 0.0,
            swap_used_gb: 0.0,
            swapins: 0,
            swapouts: 0,
            swapins_delta: 0,
            swapouts_delta: 0,
            memory_psi_some_avg10: 0.0,
            memory_psi_full_avg10: 0.0,
            memory_pressure_advice: String::new(),
            motherboard: String::new(),
            p_cores: 0,
            e_cores: 0,
            power_total_w: 0.0,
            power_cpu_w: 0.0,
            power_gpu_w: 0.0,
            p_cluster_freq_mhz: 0,
            s_cluster_freq_mhz: 0,
            e_cluster_freq_mhz: 0,
            p_cluster_active: 0.0,
            s_cluster_active: 0.0,
            e_cluster_active: 0.0,
        }
    }
}

pub fn get_system_metrics() -> SystemMetrics {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    let cpu_name = get_cpu_name();
    let (cpu_temp, cpu_temp_available) = get_cpu_temp(&sys);

    // On Apple Silicon, use mactop cache for accurate real-time clock, load, and power.
    // The GPU poller populates this every ~500ms, so data is fresh.
    let (
        cpu_load,
        cpu_clock_mhz,
        power_total_w,
        power_cpu_w,
        power_gpu_w,
        p_cluster_freq_mhz,
        s_cluster_freq_mhz,
        e_cluster_freq_mhz,
        p_cluster_active,
        s_cluster_active,
        e_cluster_active,
    ) = {
        #[cfg(target_os = "macos")]
        {
            if let Some(cache) = mactop_cache::get_cache() {
                // Weighted average of cluster utilization based on core counts
                let (p_cores, s_cores_raw, _e_cores_raw, _, _) = get_core_counts();
                let s_cores = if s_cores_raw > 0 {
                    s_cores_raw
                } else {
                    p_cores
                };
                let total_cores = p_cores + s_cores;
                if total_cores > 0 {
                    let weighted_load = (cache.p_cluster_active * p_cores as f32
                        + cache.s_cluster_active * s_cores as f32)
                        / total_cores as f32;
                    let cpu_load = weighted_load as u32;

                    // Use P-cluster frequency as the "main" clock (it's the one doing heavy work)
                    let cpu_clock_mhz = cache.p_cluster_freq_mhz;
                    (
                        cpu_load,
                        cpu_clock_mhz,
                        cache.power_total_w,
                        cache.power_cpu_w,
                        cache.power_gpu_w,
                        cache.p_cluster_freq_mhz,
                        cache.s_cluster_freq_mhz,
                        cache.e_cluster_freq_mhz,
                        cache.p_cluster_active,
                        cache.s_cluster_active,
                        cache.e_cluster_active,
                    )
                } else {
                    // Fallback if core counts are unknown
                    let cpu_load = get_cpu_load(&sys);
                    let cpu_clock_mhz = get_cpu_clock(&sys);
                    (
                        cpu_load,
                        cpu_clock_mhz,
                        0.0,
                        0.0,
                        0.0,
                        0,
                        0,
                        0,
                        0.0,
                        0.0,
                        0.0,
                    )
                }
            } else {
                // Cache not yet populated — fallback to sysinfo
                let cpu_load = get_cpu_load(&sys);
                let cpu_clock_mhz = get_cpu_clock(&sys);
                (
                    cpu_load,
                    cpu_clock_mhz,
                    0.0,
                    0.0,
                    0.0,
                    0,
                    0,
                    0,
                    0.0,
                    0.0,
                    0.0,
                )
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            (
                get_cpu_load(&sys),
                get_cpu_clock(&sys),
                0.0,
                0.0,
                0.0,
                0,
                0,
                0,
                0.0,
                0.0,
                0.0,
            )
        }
    };

    let (ram_total_gb, ram_used_gb, ram_available_gb) = get_ram_info(&sys);
    let memory_pressure = get_memory_pressure(ram_total_gb, ram_available_gb);
    let motherboard = get_motherboard();
    let (p_cores, s_cores, e_cores, p_cluster_name, secondary_cluster_name) = get_core_counts();

    SystemMetrics {
        cpu_name,
        cpu_temp,
        cpu_temp_available,
        cpu_load,
        cpu_clock_mhz,
        ram_total_gb,
        ram_used_gb,
        ram_available_gb,
        memory_pressure_level: memory_pressure.level,
        memory_pressure_source: memory_pressure.source,
        memory_pressure_score: memory_pressure.score,
        memory_free_gb: memory_pressure.free_gb,
        memory_compressor_gb: memory_pressure.compressor_gb,
        memory_compressed_gb: memory_pressure.compressed_gb,
        memory_wired_gb: memory_pressure.wired_gb,
        memory_purgeable_gb: memory_pressure.purgeable_gb,
        memory_inactive_gb: memory_pressure.inactive_gb,
        memory_reclaimable_gb: memory_pressure.reclaimable_gb,
        swap_used_gb: memory_pressure.swap_used_gb,
        swapins: memory_pressure.swapins,
        swapouts: memory_pressure.swapouts,
        swapins_delta: memory_pressure.swapins_delta,
        swapouts_delta: memory_pressure.swapouts_delta,
        memory_psi_some_avg10: memory_pressure.psi_some_avg10,
        memory_psi_full_avg10: memory_pressure.psi_full_avg10,
        memory_pressure_advice: memory_pressure.advice,
        motherboard,
        p_cores,
        s_cores,
        e_cores,
        p_cluster_name,
        secondary_cluster_name,
        power_total_w,
        power_cpu_w,
        power_gpu_w,
        p_cluster_freq_mhz,
        s_cluster_freq_mhz,
        e_cluster_freq_mhz,
        p_cluster_active,
        s_cluster_active,
        e_cluster_active,
    }
}

#[cfg(target_os = "macos")]
fn get_core_counts() -> (u32, u32, u32, String, String) {
    fn sysctl_u32(key: &str) -> u32 {
        std::process::Command::new("sysctl")
            .args(["-n", key])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }
    fn sysctl_str(key: &str) -> String {
        std::process::Command::new("sysctl")
            .args(["-n", key])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }

    let level_count = sysctl_u32("hw.perflevelcount");
    let level_count = if level_count > 0 { level_count } else { 2 };

    let mut total_p = 0u32;
    let mut total_s = 0u32;
    let mut total_e = 0u32;
    let mut first_name = String::new();
    let mut second_name = String::new();

    for i in 0..level_count {
        let cores = sysctl_u32(&format!("hw.perflevel{i}.physicalcpu"));
        let name = sysctl_str(&format!("hw.perflevel{i}.name"));
        let name_lower = name.to_lowercase();

        if first_name.is_empty() {
            first_name = name.clone();
        } else if second_name.is_empty() {
            second_name = name.clone();
        }

        if name_lower.contains("super") {
            total_p += cores;
        } else if name_lower.contains("performance") {
            total_s += cores;
        } else if name_lower.contains("efficiency") || name_lower.contains("e-core") {
            total_e += cores;
        } else {
            // Fallback: treat unrecognized as efficiency
            total_e += cores;
        }
    }

    // Backward compatibility: if "s" is nonzero (secondary/performance cluster),
    // it is used for the S-row in the dashboard; p_cores includes all top-tier
    // (Super or performance-only on older chips).
    // On older 2-level chips (e.g. M3 Max), names are typically
    // "Performance" (perflevel0) and "Efficiency" (perflevel1), so
    // total_p = 0 and total_s will hold the P-cores; adjust for that case:
    if total_p == 0 && total_s > 0 && total_e > 0 {
        total_p = total_s;
        total_s = 0;
    }

    (total_p, total_s, total_e, first_name, second_name)
}

#[cfg(not(target_os = "macos"))]
fn get_core_counts() -> (u32, u32, u32, String, String) {
    (0, 0, 0, String::new(), String::new())
}

#[cfg(target_os = "windows")]
fn get_cpu_name() -> String {
    use std::collections::HashMap;
    use wmi::{Variant, WMIConnection};

    if let Ok(wmi) = WMIConnection::new() {
        let results: Vec<HashMap<String, Variant>> =
            match wmi.raw_query::<HashMap<String, Variant>>("SELECT Name FROM Win32_Processor") {
                Ok(r) => r,
                Err(_) => return "Unknown CPU".to_string(),
            };
        for row in &results {
            if let Some(Variant::String(name)) = row.get("Name") {
                return name.clone();
            }
        }
    }

    "Unknown CPU".to_string()
}

#[cfg(target_os = "linux")]
fn get_cpu_name() -> String {
    use std::fs;

    if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
        for line in content.lines() {
            if let Some(cpu_name) = line.strip_prefix("model name\t: ") {
                return cpu_name.to_string();
            }
            if let Some(cpu_name) = line.strip_prefix("Processor\t: ") {
                return cpu_name.to_string();
            }
        }
    }

    "Unknown CPU".to_string()
}

#[cfg(target_os = "macos")]
fn get_cpu_name() -> String {
    use std::process::Command;

    if let Ok(output) = Command::new("sysctl")
        .arg("-n")
        .arg("machdep.cpu.brand_string")
        .output()
        && output.status.success()
    {
        return String::from_utf8_lossy(&output.stdout).trim().to_string();
    }

    "Unknown CPU".to_string()
}

fn get_cpu_temp(sys: &System) -> (f32, bool) {
    if sys.cpus().is_empty() {
        return (0.0, false);
    }

    #[cfg(target_os = "windows")]
    {
        use crate::lhm;

        if lhm::is_lhm_available() {
            return lhm::get_lhm_cpu_temp();
        }
        (0.0, false)
    }

    #[cfg(target_os = "linux")]
    {
        use std::fs;

        let components = sysinfo::Components::new_with_refreshed_list();

        if !components.is_empty()
            && let Some(comp) = components.iter().next()
        {
            return (comp.temperature().unwrap_or(0.0), true);
        }

        let temp_paths = [
            "/sys/class/thermal/thermal_zone0/temp",
            "/sys/class/hwmon/hwmon0/temp1_input",
            "/sys/class/hwmon/hwmon0/device/temp1_input",
            "/sys/class/hwmon/hwmon1/temp1_input",
            "/sys/class/hwmon/hwmon2/temp1_input",
        ];

        for path in temp_paths {
            if let Ok(content) = fs::read_to_string(path)
                && let Ok(temp_milli) = content.trim().parse::<i32>()
            {
                return (temp_milli as f32 / 1000.0, true);
            }
        }

        (0.0, false)
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let sensors = ["hw.sensors.cpu0.temp0", "hw.acpi.thermal.cpu0.temperature"];

        for sensor in sensors {
            if let Ok(output) = Command::new("sysctl").arg("-n").arg(sensor).output()
                && output.status.success()
                && let Ok(temp) = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<f32>()
            {
                return (temp, true);
            }
        }

        (0.0, false)
    }
}

fn get_cpu_load(sys: &System) -> u32 {
    if sys.cpus().is_empty() {
        return 0;
    }

    (sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32) as _
}

#[cfg(target_os = "linux")]
fn get_cpu_clock(sys: &System) -> u32 {
    use std::fs;

    if sys.cpus().is_empty() {
        return 0;
    }

    let mut freq_samples_mhz = Vec::new();

    for cpu in 0..sys.cpus().len() {
        let base = format!("/sys/devices/system/cpu/cpu{cpu}/cpufreq");
        for candidate in ["scaling_cur_freq", "cpuinfo_cur_freq"] {
            let path = format!("{base}/{candidate}");
            if let Ok(raw) = fs::read_to_string(&path)
                && let Ok(khz) = raw.trim().parse::<u64>()
                && khz > 0
            {
                freq_samples_mhz.push((khz / 1000) as u32);
                break;
            }
        }
    }

    if !freq_samples_mhz.is_empty() {
        let sum: u64 = freq_samples_mhz.iter().map(|&v| v as u64).sum();
        return (sum / freq_samples_mhz.len() as u64) as u32;
    }

    if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
        let mhz_values: Vec<u32> = cpuinfo
            .lines()
            .filter_map(|line| line.split_once(':'))
            .filter(|(key, _)| key.trim() == "cpu MHz")
            .filter_map(|(_, value)| value.trim().parse::<f64>().ok())
            .filter(|mhz| *mhz > 0.0)
            .map(|mhz| mhz.round() as u32)
            .collect();

        if !mhz_values.is_empty() {
            let sum: u64 = mhz_values.iter().map(|&v| v as u64).sum();
            return (sum / mhz_values.len() as u64) as u32;
        }
    }

    sys.cpus().iter().map(|c| c.frequency()).max().unwrap_or(0) as u32
}

#[cfg(target_os = "windows")]
fn get_cpu_clock(sys: &System) -> u32 {
    use std::collections::HashMap;
    use wmi::{Variant, WMIConnection};

    if sys.cpus().is_empty() {
        return 0;
    }

    fn variant_to_u32(value: &Variant) -> Option<u32> {
        match value {
            Variant::UI4(v) => Some(*v),
            Variant::I4(v) => (*v).try_into().ok(),
            Variant::UI8(v) => (*v).try_into().ok(),
            Variant::I8(v) => (*v).try_into().ok(),
            Variant::R4(v) => {
                if *v > 0.0 {
                    Some(v.round() as u32)
                } else {
                    None
                }
            }
            Variant::R8(v) => {
                if *v > 0.0 {
                    Some(v.round() as u32)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    if let Ok(wmi) = WMIConnection::new() {
        let base_mhz = wmi
            .raw_query::<HashMap<String, Variant>>("SELECT CurrentClockSpeed FROM Win32_Processor")
            .ok()
            .map(|results| {
                results
                    .iter()
                    .filter_map(|row| row.get("CurrentClockSpeed").and_then(variant_to_u32))
                    .filter(|mhz| *mhz > 0)
                    .collect::<Vec<_>>()
            })
            .and_then(|mhz_values| {
                if mhz_values.is_empty() {
                    None
                } else {
                    let sum: u64 = mhz_values.iter().map(|&v| v as u64).sum();
                    Some((sum / mhz_values.len() as u64) as u32)
                }
            });

        let perf_pct = wmi
            .raw_query::<HashMap<String, Variant>>(
                "SELECT Name,PercentProcessorPerformance FROM Win32_PerfFormattedData_Counters_ProcessorInformation WHERE Name='_Total'",
            )
            .ok()
            .and_then(|results| {
                results.iter().find_map(|row| {
                    row.get("PercentProcessorPerformance")
                        .and_then(variant_to_u32)
                        .filter(|pct| *pct > 0)
                })
            });

        if let (Some(base_mhz), Some(perf_pct)) = (base_mhz, perf_pct) {
            let effective = ((base_mhz as f64) * (perf_pct as f64 / 100.0)).round() as u32;
            if effective > 0 {
                return effective;
            }
        }

        if let Some(base_mhz) = base_mhz {
            return base_mhz;
        }
    }

    sys.cpus().iter().map(|c| c.frequency()).max().unwrap_or(0) as u32
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
fn get_cpu_clock(sys: &System) -> u32 {
    if sys.cpus().is_empty() {
        return 0;
    }

    sys.cpus().iter().map(|c| c.frequency()).max().unwrap_or(0) as u32
}

fn get_ram_info(sys: &System) -> (f64, f64, f64) {
    let total_gb = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let used_gb = sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let available_gb = sys.available_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    (total_gb, used_gb, available_gb)
}

#[derive(Default)]
struct MemoryPressure {
    level: String,
    source: String,
    score: f64,
    free_gb: f64,
    compressor_gb: f64,
    compressed_gb: f64,
    wired_gb: f64,
    purgeable_gb: f64,
    inactive_gb: f64,
    reclaimable_gb: f64,
    swap_used_gb: f64,
    swapins: u64,
    swapouts: u64,
    swapins_delta: u64,
    swapouts_delta: u64,
    psi_some_avg10: f64,
    psi_full_avg10: f64,
    advice: String,
}

#[cfg(target_os = "macos")]
fn get_memory_pressure(total_ram_gb: f64, _available_ram_gb: f64) -> MemoryPressure {
    let output = std::process::Command::new("vm_stat").output();
    let Ok(output) = output else {
        return MemoryPressure::default();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    parse_macos_vm_stat(&text, total_ram_gb)
}

#[cfg(target_os = "macos")]
fn parse_macos_vm_stat(text: &str, total_ram_gb: f64) -> MemoryPressure {
    use std::sync::{Mutex, OnceLock};

    static LAST_SWAP: OnceLock<Mutex<(u64, u64)>> = OnceLock::new();

    let mut page_size = 16_384_u64;
    let mut free_pages = 0_u64;
    let mut wired_pages = 0_u64;
    let mut compressor_pages = 0_u64;
    let mut compressed_pages = 0_u64;
    let mut purgeable_pages = 0_u64;
    let mut inactive_pages = 0_u64;
    let mut swapins = 0_u64;
    let mut swapouts = 0_u64;

    for line in text.lines() {
        if let Some(size) = line
            .strip_prefix("Mach Virtual Memory Statistics: (page size of ")
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|value| value.parse::<u64>().ok())
        {
            page_size = size;
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value
            .trim()
            .trim_end_matches('.')
            .replace(',', "")
            .parse::<u64>()
            .unwrap_or(0);
        match key.trim() {
            "Pages free" => free_pages = value,
            "Pages wired down" => wired_pages = value,
            "Pages occupied by compressor" => compressor_pages = value,
            "Pages stored in compressor" => compressed_pages = value,
            "Pages purgeable" => purgeable_pages = value,
            "Pages inactive" => inactive_pages = value,
            "Swapins" => swapins = value,
            "Swapouts" => swapouts = value,
            _ => {}
        }
    }

    let page_gb = page_size as f64 / 1024.0 / 1024.0 / 1024.0;
    let free_gb = free_pages as f64 * page_gb;
    let wired_gb = wired_pages as f64 * page_gb;
    let compressor_gb = compressor_pages as f64 * page_gb;
    let compressed_gb = compressed_pages as f64 * page_gb;
    let purgeable_gb = purgeable_pages as f64 * page_gb;
    let inactive_gb = inactive_pages as f64 * page_gb;
    let reclaimable_gb = purgeable_gb + inactive_gb;
    let compressor_ratio = if total_ram_gb > 0.0 {
        compressor_gb / total_ram_gb
    } else {
        0.0
    };
    let pressure_score = if total_ram_gb > 0.0 {
        let free_pressure = (1.0 - (free_gb / total_ram_gb)).clamp(0.0, 1.0) * 45.0;
        let compressor_pressure = (compressor_ratio / 0.30).clamp(0.0, 1.0) * 45.0;
        let swap_pressure = if swapouts > 0 { 10.0 } else { 0.0 };
        (free_pressure + compressor_pressure + swap_pressure).min(100.0)
    } else {
        0.0
    };
    let (swapins_delta, swapouts_delta) = {
        let mut last = LAST_SWAP
            .get_or_init(|| Mutex::new((swapins, swapouts)))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let delta = (
            swapins.saturating_sub(last.0),
            swapouts.saturating_sub(last.1),
        );
        *last = (swapins, swapouts);
        delta
    };
    let level = if free_gb < 0.5 || compressor_ratio >= 0.30 || swapouts_delta > 0 {
        "critical"
    } else if free_gb < 1.5 || compressor_ratio >= 0.18 {
        "warning"
    } else {
        "ok"
    }
    .to_string();
    let advice = if wired_gb > total_ram_gb * 0.55 {
        "Wired memory is high; prefer mmap-enabled presets and disable mlock.".to_string()
    } else if reclaimable_gb > 1.0 && free_gb < 1.5 {
        "Reclaimable cache is available; Free Memory can help if sudo is already authorized."
            .to_string()
    } else if compressor_ratio >= 0.18 || swapouts_delta > 0 {
        "Reduce context, batch, or parallel slots; macOS is compressing or swapping memory."
            .to_string()
    } else {
        "Memory pressure is normal.".to_string()
    };

    MemoryPressure {
        level,
        source: "vm_stat".to_string(),
        score: pressure_score,
        free_gb,
        wired_gb,
        compressor_gb,
        compressed_gb,
        purgeable_gb,
        inactive_gb,
        reclaimable_gb,
        swap_used_gb: 0.0,
        swapins,
        swapouts,
        swapins_delta,
        swapouts_delta,
        psi_some_avg10: 0.0,
        psi_full_avg10: 0.0,
        advice,
    }
}

#[cfg(target_os = "linux")]
fn get_memory_pressure(total_ram_gb: f64, available_ram_gb: f64) -> MemoryPressure {
    use std::fs;

    fn kb_to_gb(kb: u64) -> f64 {
        kb as f64 / 1024.0 / 1024.0
    }

    let mut mem_free_kb = 0_u64;
    let mut cached_kb = 0_u64;
    let mut sreclaimable_kb = 0_u64;
    let mut mlocked_kb = 0_u64;
    let mut unevictable_kb = 0_u64;
    let mut swap_total_kb = 0_u64;
    let mut swap_free_kb = 0_u64;

    if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            let mut parts = line.split_whitespace();
            let key = parts.next().unwrap_or("").trim_end_matches(':');
            let value = parts
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            match key {
                "MemFree" => mem_free_kb = value,
                "Cached" => cached_kb = value,
                "SReclaimable" => sreclaimable_kb = value,
                "Mlocked" => mlocked_kb = value,
                "Unevictable" => unevictable_kb = value,
                "SwapTotal" => swap_total_kb = value,
                "SwapFree" => swap_free_kb = value,
                _ => {}
            }
        }
    }

    let (psi_some_avg10, psi_full_avg10) = fs::read_to_string("/proc/pressure/memory")
        .ok()
        .map(|psi| parse_linux_psi(&psi))
        .unwrap_or((0.0, 0.0));
    let available_ratio = if total_ram_gb > 0.0 {
        available_ram_gb / total_ram_gb
    } else {
        1.0
    };
    let swap_used_gb = kb_to_gb(swap_total_kb.saturating_sub(swap_free_kb));
    let pressure_score = ((1.0 - available_ratio).clamp(0.0, 1.0) * 55.0
        + (psi_some_avg10 / 20.0).clamp(0.0, 1.0) * 30.0
        + (psi_full_avg10 / 5.0).clamp(0.0, 1.0) * 15.0)
        .min(100.0);
    let level = if psi_full_avg10 >= 1.0
        || psi_some_avg10 >= 20.0
        || (available_ratio < 0.05 && swap_used_gb > 0.0)
    {
        "critical"
    } else if psi_some_avg10 >= 5.0 || available_ratio < 0.10 {
        "warning"
    } else {
        "ok"
    }
    .to_string();
    let reclaimable_gb = kb_to_gb(cached_kb + sreclaimable_kb);
    let pinned_gb = kb_to_gb(mlocked_kb + unevictable_kb);
    let advice = if psi_full_avg10 >= 1.0 || psi_some_avg10 >= 5.0 {
        "Linux PSI reports memory stalls; reduce context, batch, or parallel slots.".to_string()
    } else if available_ratio < 0.10 {
        "Available memory is low; lower model footprint or stop other large processes.".to_string()
    } else {
        "Memory pressure is normal.".to_string()
    };

    MemoryPressure {
        level,
        source: "linux_psi".to_string(),
        score: pressure_score,
        free_gb: kb_to_gb(mem_free_kb),
        wired_gb: pinned_gb,
        reclaimable_gb,
        swap_used_gb,
        psi_some_avg10,
        psi_full_avg10,
        advice,
        ..Default::default()
    }
}

#[cfg(target_os = "linux")]
fn parse_linux_psi(text: &str) -> (f64, f64) {
    fn avg10(line: &str) -> f64 {
        line.split_whitespace()
            .find_map(|part| part.strip_prefix("avg10="))
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.0)
    }

    let mut some = 0.0;
    let mut full = 0.0;
    for line in text.lines() {
        if line.starts_with("some ") {
            some = avg10(line);
        } else if line.starts_with("full ") {
            full = avg10(line);
        }
    }
    (some, full)
}

#[cfg(target_os = "windows")]
fn get_memory_pressure(total_ram_gb: f64, available_ram_gb: f64) -> MemoryPressure {
    use std::collections::HashMap;
    use wmi::{Variant, WMIConnection};

    fn variant_to_u64(value: &Variant) -> Option<u64> {
        match value {
            Variant::UI4(v) => Some(*v as u64),
            Variant::I4(v) => (*v).try_into().ok(),
            Variant::UI8(v) => Some(*v),
            Variant::I8(v) => (*v).try_into().ok(),
            Variant::String(v) => v.parse().ok(),
            _ => None,
        }
    }

    let mut free_phys_gb = available_ram_gb;
    let mut swap_used_gb = 0.0;
    if let Ok(wmi) = WMIConnection::new()
        && let Ok(rows) = wmi.raw_query::<HashMap<String, Variant>>(
            "SELECT FreePhysicalMemory,TotalVirtualMemorySize,FreeVirtualMemory FROM Win32_OperatingSystem",
        )
        && let Some(row) = rows.first()
    {
        if let Some(kb) = row.get("FreePhysicalMemory").and_then(variant_to_u64) {
            free_phys_gb = kb as f64 / 1024.0 / 1024.0;
        }
        let total_virtual = row
            .get("TotalVirtualMemorySize")
            .and_then(variant_to_u64)
            .unwrap_or(0);
        let free_virtual = row
            .get("FreeVirtualMemory")
            .and_then(variant_to_u64)
            .unwrap_or(0);
        swap_used_gb = total_virtual.saturating_sub(free_virtual) as f64 / 1024.0 / 1024.0;
    }

    let available_ratio = if total_ram_gb > 0.0 {
        free_phys_gb / total_ram_gb
    } else {
        1.0
    };
    let pressure_score = ((1.0 - available_ratio).clamp(0.0, 1.0) * 100.0).min(100.0);
    let level = if available_ratio < 0.05 {
        "critical"
    } else if available_ratio < 0.10 || pressure_score >= 90.0 {
        "warning"
    } else {
        "ok"
    }
    .to_string();
    let advice = if level == "critical" || level == "warning" {
        "Windows reports low available memory; reduce context, batch, or stop large processes."
            .to_string()
    } else {
        "Memory pressure is normal.".to_string()
    };

    MemoryPressure {
        level,
        source: "windows_wmi".to_string(),
        score: pressure_score,
        free_gb: free_phys_gb,
        swap_used_gb,
        advice,
        ..Default::default()
    }
}

#[cfg(all(
    not(target_os = "macos"),
    not(target_os = "linux"),
    not(target_os = "windows")
))]
fn get_memory_pressure(_total_ram_gb: f64, _available_ram_gb: f64) -> MemoryPressure {
    MemoryPressure::default()
}

#[cfg(target_os = "windows")]
fn get_motherboard() -> String {
    use std::collections::HashMap;
    use wmi::{Variant, WMIConnection};
    if let Ok(wmi) = WMIConnection::new() {
        let results: Vec<HashMap<String, Variant>> = match wmi
            .raw_query::<HashMap<String, Variant>>("SELECT Product FROM Win32_BaseBoard")
        {
            Ok(r) => r,
            Err(_) => {
                eprintln!("[system] WMI query failed for Win32_BaseBoard");
                return "Unknown Motherboard".to_string();
            }
        };
        for row in &results {
            if let Some(Variant::String(product)) = row.get("Product") {
                return product.clone();
            }
        }
    }

    "Unknown Motherboard".to_string()
}

#[cfg(target_os = "linux")]
fn get_motherboard() -> String {
    use std::fs;

    let paths = [
        "/sys/class/dmi/id/product_name",
        "/sys/class/dmi/id/board_name",
        "/sys/class/dmi/id/vendor",
    ];

    for path in paths {
        if let Ok(content) = fs::read_to_string(path) {
            let value = content.trim();
            if !value.is_empty() && value != "To Be Filled By O.E.M." {
                return value.to_string();
            }
        }
    }

    "Unknown Motherboard".to_string()
}

#[cfg(target_os = "macos")]
fn get_motherboard() -> String {
    use std::process::Command;

    if let Ok(output) = Command::new("sysctl").arg("-n").arg("hw.model").output()
        && output.status.success()
    {
        let model = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !model.is_empty() {
            return model;
        }
    }

    "Unknown Motherboard".to_string()
}
