#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemMetrics {
    pub cpu_name: String,
    pub cpu_temp: f32,
    pub cpu_load: u32,
    pub cpu_clock_mhz: u32,
    pub ram_total_gb: f64,
    pub ram_used_gb: f64,
}

pub fn get_system_metrics() -> SystemMetrics {
    let cpu_name = get_cpu_name();
    let cpu_temp = get_cpu_temp();
    let cpu_load = get_cpu_load();
    let cpu_clock_mhz = get_cpu_clock();
    let (ram_total_gb, ram_used_gb) = get_ram_info();

    SystemMetrics {
        cpu_name,
        cpu_temp,
        cpu_load,
        cpu_clock_mhz,
        ram_total_gb,
        ram_used_gb,
    }
}

fn get_cpu_name() -> String {
    #[cfg(target_os = "windows")]
    {
        get_cpu_name_windows()
    }
    #[cfg(target_os = "linux")]
    {
        get_cpu_name_linux()
    }
    #[cfg(target_os = "macos")]
    {
        get_cpu_name_macos()
    }
}

#[cfg(target_os = "windows")]
fn get_cpu_name_windows() -> String {
    use std::process::Command;
    if let Ok(output) = Command::new("wmic")
        .args(["cpu", "get", "name", "/value"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(name) = line.strip_prefix("Name=") {
                return name.trim().to_string();
            }
        }
    }
    "Unknown CPU".to_string()
}

#[cfg(target_os = "linux")]
fn get_cpu_name_linux() -> String {
    use std::fs;
    if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
        for line in content.lines() {
            if let Some(name) = line.strip_prefix("model name\t: ") {
                return name.trim().to_string();
            }
            if let Some(name) = line.strip_prefix("Processor\t: ") {
                return name.trim().to_string();
            }
        }
    }
    "Unknown CPU".to_string()
}

#[cfg(target_os = "macos")]
fn get_cpu_name_macos() -> String {
    use std::process::Command;
    if let Ok(output) = Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
    {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    "Unknown CPU".to_string()
}

fn get_cpu_temp() -> f32 {
    #[cfg(target_os = "windows")]
    {
        get_cpu_temp_windows()
    }
    #[cfg(target_os = "linux")]
    {
        get_cpu_temp_linux()
    }
    #[cfg(target_os = "macos")]
    {
        get_cpu_temp_macos()
    }
}

#[cfg(target_os = "windows")]
fn get_cpu_temp_windows() -> f32 {
    use std::process::Command;
    if let Ok(output) = Command::new("wmic")
        .args([
            "/namespace:\\\\root\\wmi",
            "path",
            "MsAcpi_ThermalZoneTemperature",
            "get",
            "CurrentTemperature",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Ok(temp) = line.trim().parse::<u32>() {
                return (temp as f32 - 2731.5) / 10.0;
            }
        }
    }
    0.0
}

#[cfg(target_os = "linux")]
fn get_cpu_temp_linux() -> f32 {
    use std::fs;
    let thermal_paths = [
        "/sys/class/thermal/thermal_zone0/temp",
        "/sys/class/hwmon/hwmon0/temp1_input",
    ];
    for path in thermal_paths {
        if let Ok(content) = fs::read_to_string(path)
            && let Ok(temp) = content.trim().parse::<u32>()
        {
            return temp as f32 / 1000.0;
        }
    }
    0.0
}

#[cfg(target_os = "macos")]
fn get_cpu_temp_macos() -> f32 {
    use std::process::Command;
    if let Ok(output) = Command::new("sysctl")
        .args(["-n", "hw.sensors.cpu.temp"])
        .output()
    {
        let temp = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Ok(val) = temp.parse::<f32>() {
            return val;
        }
    }
    0.0
}

fn get_cpu_load() -> u32 {
    #[cfg(target_os = "windows")]
    {
        get_cpu_load_windows()
    }
    #[cfg(target_os = "linux")]
    {
        get_cpu_load_linux()
    }
    #[cfg(target_os = "macos")]
    {
        get_cpu_load_macos()
    }
}

#[cfg(target_os = "windows")]
fn get_cpu_load_windows() -> u32 {
    use std::process::Command;
    if let Ok(output) = Command::new("wmic")
        .args(["cpu", "get", "loadpercentage", "/value"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Ok(load) = line.trim().parse::<u32>() {
                return load;
            }
        }
    }
    0
}

#[cfg(target_os = "linux")]
fn get_cpu_load_linux() -> u32 {
    let content = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let mut lines = content.lines().filter(|l| l.starts_with("cpu "));
    if let Some(line) = lines.next() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            let idle: u64 = parts[4].parse().unwrap_or(0);
            let total: u64 = parts[1..]
                .iter()
                .map(|&s| s.parse::<u64>().unwrap_or(0))
                .sum();
            if total > 0 {
                return ((total - idle) as f64 / total as f64 * 100.0) as u32;
            }
        }
    }
    0
}

#[cfg(target_os = "macos")]
fn get_cpu_load_macos() -> u32 {
    use std::process::Command;
    if let Ok(output) = Command::new("top").args(["-l", "1", "-n", "0"]).output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(stats) = line.strip_prefix("CPU usage: ")
                && let Some(load) = stats.split(',').find_map(|s| {
                    s.trim()
                        .strip_prefix("Load: ")
                        .or_else(|| s.trim().strip_prefix("cpu "))
                })
                && let Ok(l) = load.trim().parse::<f32>()
            {
                return l as u32;
            }
        }
    }
    0
}

fn get_cpu_clock() -> u32 {
    #[cfg(target_os = "windows")]
    {
        get_cpu_clock_windows()
    }
    #[cfg(target_os = "linux")]
    {
        get_cpu_clock_linux()
    }
    #[cfg(target_os = "macos")]
    {
        get_cpu_clock_macos()
    }
}

#[cfg(target_os = "windows")]
fn get_cpu_clock_windows() -> u32 {
    use std::process::Command;
    if let Ok(output) = Command::new("wmic")
        .args(["cpu", "get", "maxclockspeed", "/value"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Ok(clock) = line.trim().parse::<u32>() {
                return clock;
            }
        }
    }
    0
}

#[cfg(target_os = "linux")]
fn get_cpu_clock_linux() -> u32 {
    use std::fs;
    if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
        for line in content.lines() {
            if let Some(freq) = line.strip_prefix("cpu MHz\t: ")
                && let Ok(mhz) = freq.trim().parse::<f64>()
            {
                return mhz as u32;
            }
            if let Some(freq) = line.strip_prefix("clock")
                && let Some(colon) = freq.find(":")
                && let Ok(mhz) = freq[colon + 1..]
                    .trim()
                    .trim_end_matches("MHz")
                    .parse::<f64>()
            {
                return mhz as u32;
            }
        }
    }
    0
}

#[cfg(target_os = "macos")]
fn get_cpu_clock_macos() -> u32 {
    use std::process::Command;
    if let Ok(output) = Command::new("sysctl")
        .args(["-n", "hw.cpufrequency"])
        .output()
    {
        let freq = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Ok(val) = freq.parse::<u64>() {
            return (val / 1_000_000) as u32;
        }
    }
    0
}

fn get_ram_info() -> (f64, f64) {
    #[cfg(target_os = "windows")]
    {
        get_ram_info_windows()
    }
    #[cfg(target_os = "linux")]
    {
        get_ram_info_linux()
    }
    #[cfg(target_os = "macos")]
    {
        get_ram_info_macos()
    }
}

#[cfg(target_os = "windows")]
fn get_ram_info_windows() -> (f64, f64) {
    use std::process::Command;
    if let Ok(output) = Command::new("wmic")
        .args([
            "os",
            "get",
            "TotalVisibleMemorySize,FreePhysicalMemory",
            "/value",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut total_kb = 0u64;
        let mut free_kb = 0u64;
        for line in stdout.lines() {
            if let Some(val) = line.strip_prefix("FreePhysicalMemory=") {
                free_kb = val.trim().parse().unwrap_or(0);
            }
            if let Some(val) = line.strip_prefix("TotalVisibleMemorySize=") {
                total_kb = val.trim().parse().unwrap_or(0);
            }
        }
        let total_gb = total_kb as f64 / 1024.0 / 1024.0;
        let free_gb = free_kb as f64 / 1024.0 / 1024.0;
        (total_gb, total_gb - free_gb)
    } else {
        (0.0, 0.0)
    }
}

#[cfg(target_os = "linux")]
fn get_ram_info_linux() -> (f64, f64) {
    let content = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut total_kb = 0u64;
    let mut free_kb = 0u64;
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2
            && let Ok(val) = parts[1].parse::<u64>()
        {
            if line.starts_with("MemTotal:") {
                total_kb = val;
            }
            if line.starts_with("MemAvailable:") || line.starts_with("MemFree:") {
                free_kb = free_kb.max(val);
            }
        }
    }
    let total_gb = total_kb as f64 / 1024.0 / 1024.0;
    let free_gb = free_kb as f64 / 1024.0 / 1024.0;
    (total_gb, total_gb - free_gb)
}

#[cfg(target_os = "macos")]
fn get_ram_info_macos() -> (f64, f64) {
    use std::process::Command;
    if let Ok(output) = Command::new("sysctl")
        .args(["-n", "hw.memsize", "vm.page_free_count", "vm.page_size"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        if lines.len() >= 3
            && let (Ok(total), Ok(free_pages), Ok(page_size)) = (
                lines[0].trim().parse::<u64>(),
                lines[1].trim().parse::<u64>(),
                lines[2].trim().parse::<u64>(),
            )
        {
            let total_gb = total as f64 / 1024.0 / 1024.0 / 1024.0;
            let free_gb = (free_pages * page_size) as f64 / 1024.0 / 1024.0 / 1024.0;
            return (total_gb, free_gb);
        }
    }
    (0.0, 0.0)
}
