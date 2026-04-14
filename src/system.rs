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

#[cfg(not(target_os = "windows"))]
fn get_cpu_name() -> String {
    "Unknown CPU".to_string()
}

fn get_cpu_temp(sys: &System) -> (f32, bool) {
    if sys.cpus().is_empty() {
        return (0.0, false);
    }
    (0.0, false)
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

#[cfg(not(target_os = "windows"))]
fn get_motherboard() -> String {
    "N/A".to_string()
}
