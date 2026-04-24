use sysinfo::System;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemMetrics {
    pub cpu_name: String,
    pub cpu_temp: f32,
    pub cpu_temp_available: bool,
    pub cpu_load: u32,
    pub cpu_clock_mhz: u32,
    pub ram_total_gb: f64,
    pub ram_used_gb: f64,
    pub motherboard: String,
}

pub fn get_system_metrics() -> SystemMetrics {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_name = get_cpu_name();
    let (cpu_temp, cpu_temp_available) = get_cpu_temp(&sys);
    let cpu_load = get_cpu_load(&sys);
    let cpu_clock_mhz = get_cpu_clock(&sys);
    let (ram_total_gb, ram_used_gb) = get_ram_info(&sys);
    let motherboard = get_motherboard();

    SystemMetrics {
        cpu_name,
        cpu_temp,
        cpu_temp_available,
        cpu_load,
        cpu_clock_mhz,
        ram_total_gb,
        ram_used_gb,
        motherboard,
    }
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

fn get_ram_info(sys: &System) -> (f64, f64) {
    let total_gb = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let used_gb = sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    (total_gb, used_gb)
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
