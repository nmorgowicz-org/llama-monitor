#[cfg(target_os = "windows")]
use serde::Deserialize;

/// Sensor reading from sensor_bridge.exe output
#[cfg(target_os = "windows")]
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct SensorReading {
    hardware: String,
    subhardware: Option<String>,
    name: String,
    #[serde(rename = "type")]
    sensor_type: String,
    value: Option<f64>,
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub async fn ensure_lhm_available() -> Result<(), String> {
    if is_sensor_bridge_available() {
        return Ok(());
    }
    Err("Sensor bridge not available".to_string())
}

#[cfg(target_os = "windows")]
pub fn is_sensor_bridge_available() -> bool {
    let exe_dir = match std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|x| x.to_path_buf()))
    {
        Some(d) => d,
        None => return false,
    };
    let primary = exe_dir.join("sensor_bridge.exe");
    let legacy = exe_dir.join("bin").join("sensor_bridge.exe");
    primary.exists() || legacy.exists()
}

#[cfg(target_os = "windows")]
pub fn is_lhm_installed() -> bool {
    is_sensor_bridge_available()
}

#[cfg(target_os = "windows")]
pub fn is_lhm_available() -> bool {
    is_sensor_bridge_available()
}

#[cfg(target_os = "windows")]
pub async fn start_lhm() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn get_lhm_cpu_temp() -> (f32, bool) {
    let exe_dir = match std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|x| x.to_path_buf()))
    {
        Some(d) => d,
        None => {
            eprintln!("[LHM] Failed to get executable directory");
            return (0.0, false);
        }
    };

    let bridge_path = {
        let primary = exe_dir.join("sensor_bridge.exe");
        if primary.exists() {
            primary
        } else {
            exe_dir.join("bin").join("sensor_bridge.exe")
        }
    };

    if !bridge_path.exists() {
        eprintln!("sensor_bridge.exe not found at {:?}", bridge_path);
        return (0.0, false);
    }

    let temp_file = std::env::temp_dir().join("sensor_bridge_output.json");
    let _ = std::fs::remove_file(&temp_file);

    let result = std::process::Command::new(&bridge_path).output();

    let Ok(output) = result else {
        eprintln!("[LHM] Failed to spawn sensor_bridge.exe");
        return (0.0, false);
    };

    if !output.status.success() {
        eprintln!(
            "sensor_bridge.exe failed: {:?} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return (0.0, false);
    }

    let json_str = match std::fs::read_to_string(&temp_file).ok() {
        Some(s) => s,
        None => {
            eprintln!("[LHM] Failed to read temp file");
            return (0.0, false);
        }
    };

    let _ = std::fs::remove_file(&temp_file);

    let readings: Vec<SensorReading> = match serde_json::from_str(&json_str).ok() {
        Some(r) => r,
        None => {
            eprintln!("[LHM] Failed to parse sensor JSON");
            return (0.0, false);
        }
    };

    readings
        .iter()
        .find(|r| {
            if r.sensor_type != "Temperature" {
                return false;
            }
            let name = r.name.to_ascii_lowercase();
            let hardware = r.hardware.to_ascii_lowercase();
            let subhardware = r.subhardware.as_deref().unwrap_or("").to_ascii_lowercase();
            (name.contains("package")
                || name.contains("cpu")
                || name.contains("ccd")
                || name.contains("tdie")
                || name.contains("die"))
                && (hardware.contains("cpu")
                    || hardware.contains("ryzen")
                    || subhardware.contains("cpu")
                    || subhardware.contains("ryzen"))
        })
        .and_then(|r| r.value)
        .map(|v| (v as f32, true))
        .unwrap_or((0.0, false))
}

#[cfg(target_os = "windows")]
pub fn is_lhm_running() -> bool {
    true
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn minimize_lhm() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn configure_lhm_auto_minimize() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
pub async fn download_and_install_lhm() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn uninstall_lhm() -> Result<(), String> {
    Ok(())
}
