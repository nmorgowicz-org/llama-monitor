#[cfg(target_os = "windows")]
use serde::Deserialize;

#[cfg(target_os = "windows")]
pub const SENSOR_BRIDGE_LOCAL_PORT: u16 = 7780;
#[cfg(target_os = "windows")]
const SENSOR_BRIDGE_TASK_NAME: &str = "LlamaMonitorSensorBridge";

/// Sensor reading from sensor_bridge output
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
fn get_sensor_bridge_path() -> Option<std::path::PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|x| x.to_path_buf()))?;
    let primary = exe_dir.join("sensor_bridge.exe");
    if primary.exists() {
        return Some(primary);
    }
    let legacy = exe_dir.join("bin").join("sensor_bridge.exe");
    if legacy.exists() {
        return Some(legacy);
    }
    None
}

#[cfg(target_os = "windows")]
pub fn is_sensor_bridge_available() -> bool {
    get_sensor_bridge_path().is_some()
}

#[cfg(target_os = "windows")]
pub fn is_lhm_installed() -> bool {
    is_sensor_bridge_available()
}

#[cfg(target_os = "windows")]
pub fn is_lhm_available() -> bool {
    is_sensor_bridge_available()
}

/// Check whether the sensor_bridge scheduled task exists in the task scheduler.
#[cfg(target_os = "windows")]
pub fn is_local_sensor_bridge_service_installed() -> bool {
    std::process::Command::new("schtasks")
        .args(["/Query", "/TN", SENSOR_BRIDGE_TASK_NAME])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Probe 127.0.0.1:7780 with a short timeout to check if sensor_bridge server is running.
#[cfg(target_os = "windows")]
pub fn is_local_sensor_bridge_running() -> bool {
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    let addr: SocketAddr = match format!("127.0.0.1:{SENSOR_BRIDGE_LOCAL_PORT}").parse() {
        Ok(a) => a,
        Err(_) => return false,
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

/// Poll the local sensor_bridge HTTP server for a CPU temperature reading.
/// Returns (temp_celsius, available).
#[cfg(target_os = "windows")]
fn poll_local_sensor_bridge_temp() -> (f32, bool) {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    let addr: SocketAddr = match format!("127.0.0.1:{SENSOR_BRIDGE_LOCAL_PORT}").parse() {
        Ok(a) => a,
        Err(_) => return (0.0, false),
    };

    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(300)) {
        Ok(s) => s,
        Err(_) => return (0.0, false),
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));

    let _ = stream.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");

    let mut response = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => response.extend_from_slice(&buf[..n]),
        }
    }

    let response_str = String::from_utf8_lossy(&response);
    let body = match response_str.split("\r\n\r\n").nth(1) {
        Some(b) => b,
        None => return (0.0, false),
    };

    let readings: Vec<SensorReading> = match serde_json::from_str(body.trim()) {
        Ok(r) => r,
        Err(_) => return (0.0, false),
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
pub fn get_lhm_cpu_temp() -> (f32, bool) {
    poll_local_sensor_bridge_temp()
}

#[cfg(target_os = "windows")]
pub fn is_lhm_running() -> bool {
    is_local_sensor_bridge_running()
}

/// Install sensor_bridge as a SYSTEM scheduled task via an elevated PowerShell script.
/// The UAC prompt appears on the user's desktop. This function returns immediately
/// (does not wait for UAC approval — the caller should poll `is_local_sensor_bridge_running()`).
#[cfg(target_os = "windows")]
pub fn install_local_sensor_bridge() -> Result<(), String> {
    let bridge_path = get_sensor_bridge_path()
        .ok_or_else(|| "sensor_bridge.exe not found next to llama-monitor.exe".to_string())?;

    // Single-quote-escape the path for PowerShell string literal
    let bridge_path_str = bridge_path.to_string_lossy().replace('\'', "''");

    let script = format!(
        r#"$ErrorActionPreference = 'Stop'
$bridge = '{bridge_path_str}'
$action = New-ScheduledTaskAction -Execute $bridge -Argument '--server'
$trigger = New-ScheduledTaskTrigger -AtStartup
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit 0 -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)
try {{ Unregister-ScheduledTask -TaskName '{SENSOR_BRIDGE_TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue }} catch {{}}
Register-ScheduledTask -TaskName '{SENSOR_BRIDGE_TASK_NAME}' -Action $action -Trigger $trigger -RunLevel Highest -User 'SYSTEM' -Settings $settings -Force | Out-Null
Start-ScheduledTask -TaskName '{SENSOR_BRIDGE_TASK_NAME}'
"#
    );

    let script_path = std::env::temp_dir().join("llama_monitor_sb_install.ps1");
    std::fs::write(&script_path, &script)
        .map_err(|e| format!("Failed to write install script: {e}"))?;

    let script_path_str = script_path.to_string_lossy().replace('\'', "''");

    std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("Start-Process powershell.exe -Verb RunAs -ArgumentList '-NoProfile -ExecutionPolicy Bypass -File \"{script_path_str}\"'"),
        ])
        .spawn()
        .map_err(|e| format!("Failed to launch UAC prompt: {e}"))?;

    // Drop a standalone uninstall script next to sensor_bridge.exe so users can
    // remove the service without needing to open the dashboard.
    if let Some(dir) = bridge_path.parent() {
        let uninstall_path = dir.join("uninstall_sensor_bridge.ps1");
        let _ = std::fs::write(
            &uninstall_path,
            "# Llama Monitor Sensor Bridge Uninstall\r\n\
# Right-click this file and choose \"Run with PowerShell\" to remove the service.\r\n\
if (-NOT ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] \"Administrator\")) {\r\n\
    Start-Process powershell.exe -Verb RunAs -ArgumentList \"-NoProfile -ExecutionPolicy Bypass -File `\"$PSCommandPath`\"\"\r\n\
    exit\r\n\
}\r\n\
Stop-ScheduledTask -TaskName 'LlamaMonitorSensorBridge' -ErrorAction SilentlyContinue\r\n\
Unregister-ScheduledTask -TaskName 'LlamaMonitorSensorBridge' -Confirm:$false -ErrorAction SilentlyContinue\r\n\
Write-Host 'Sensor Bridge service removed.'\r\n\
Read-Host 'Press Enter to close'\r\n",
        );
    }

    Ok(())
}

/// Stop and remove the sensor_bridge scheduled task via an elevated PowerShell script.
#[cfg(target_os = "windows")]
pub fn uninstall_local_sensor_bridge() -> Result<(), String> {
    let script = format!(
        r#"$ErrorActionPreference = 'SilentlyContinue'
Stop-ScheduledTask -TaskName '{SENSOR_BRIDGE_TASK_NAME}' -ErrorAction SilentlyContinue
Unregister-ScheduledTask -TaskName '{SENSOR_BRIDGE_TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue
"#
    );

    let script_path = std::env::temp_dir().join("llama_monitor_sb_uninstall.ps1");
    std::fs::write(&script_path, &script)
        .map_err(|e| format!("Failed to write uninstall script: {e}"))?;

    let script_path_str = script_path.to_string_lossy().replace('\'', "''");

    std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("Start-Process powershell.exe -Verb RunAs -ArgumentList '-NoProfile -ExecutionPolicy Bypass -File \"{script_path_str}\"'"),
        ])
        .spawn()
        .map_err(|e| format!("Failed to launch UAC prompt: {e}"))?;

    Ok(())
}

#[cfg(target_os = "windows")]
pub async fn ensure_lhm_available() -> Result<(), String> {
    if is_sensor_bridge_available() {
        return Ok(());
    }
    Err("Sensor bridge not available".to_string())
}

#[cfg(target_os = "windows")]
pub async fn start_lhm() -> Result<(), String> {
    Ok(())
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
