use sysinfo::System;

#[derive(Debug, Clone, serde::Serialize)]
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

    match Command::new("sysctl")
        .arg("-n")
        .arg("machdep.cpu.brand_string")
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                return String::from_utf8_lossy(&output.stdout).trim().to_string();
            }
        }
        Err(_) => {}
    }

    "Unknown CPU".to_string()
}

fn get_cpu_temp(sys: &System) -> (f32, bool) {
    if sys.cpus().is_empty() {
        return (0.0, false);
    }

    #[cfg(target_os = "windows")]
    {
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
            {
                if let Ok(temp) = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<f32>()
                {
                    return (temp, true);
                }
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

fn get_cpu_clock(sys: &System) -> u32 {
    if sys.cpus().is_empty() {
        return 0;
    }

    let max_freq = sys.cpus().iter().map(|c| c.frequency()).max().unwrap_or(0);
    max_freq as _
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

    match Command::new("sysctl").arg("-n").arg("hw.model").output() {
        Ok(output) => {
            if output.status.success() {
                let model = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !model.is_empty() {
                    return model;
                }
            }
        }
        Err(_) => {}
    }

    "Unknown Motherboard".to_string()
}
