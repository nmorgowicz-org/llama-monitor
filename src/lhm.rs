#[cfg(target_os = "windows")]
use std::path::Path;

#[cfg(target_os = "windows")]
use wmi::{Variant, WMIConnection};

#[cfg(target_os = "windows")]
use std::collections::HashMap;

#[cfg(target_os = "windows")]
pub async fn ensure_lhm_available() -> Result<(), String> {
    // First check if process is running (portable mode)
    if is_lhm_running() {
        eprintln!("[LHM] Process is already running");
        return Ok(());
    }
    
    // Check if binary exists at portable location
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let lhm_exe = std::path::PathBuf::from(&local_app_data)
            .join("LibreHardwareMonitor")
            .join("LibreHardwareMonitor.exe");
        
        if lhm_exe.exists() {
            eprintln!("[LHM] Binary found at {}, starting...", lhm_exe.display());
            std::process::Command::new(&lhm_exe)
                .arg("-s")  // Silent mode
                .spawn()
                .map_err(|e| format!("Failed to start LHM: {}", e))?;
            
            // Give it time to start
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            
            // Minimize if it came up
            minimize_lhm().ok();
            
            return Ok(());
        }
    }
    
    eprintln!("[LHM] Checking WMI Sensor class...");
    match WMIConnection::new() {
        Ok(wmi) => {
            match wmi
                .raw_query::<HashMap<String, Variant>>("SELECT * FROM Sensor")
                .map_err(|_| "WMI query failed".to_string())
            {
                Ok(results) => {
                    if !results.is_empty() {
                        eprintln!("[LHM] WMI Sensor class available");
                        return Ok(());
                    }
                    eprintln!("[LHM] WMI Sensor class not available");
                }
                Err(_) => {
                    eprintln!("[LHM] WMI Sensor query failed");
                }
            }
        }
        Err(_) => {
            eprintln!("[LHM] Failed to connect to WMI");
        }
    }

    eprintln!("[LHM] Checking WMI Hardware class...");
    match WMIConnection::new() {
        Ok(wmi) => match wmi.raw_query::<HashMap<String, Variant>>("SELECT * FROM Hardware") {
            Ok(results) => {
                if !results.is_empty() {
                    eprintln!("[LHM] WMI Hardware class available");
                    return Ok(());
                }
                eprintln!("[LHM] WMI Hardware class not available");
            }
            Err(_) => {
                eprintln!("[LHM] WMI Hardware query failed");
            }
        },
        Err(_) => {
            eprintln!("[LHM] Failed to connect to WMI");
        }
    }

    eprintln!("[LHM] Checking LibreHardwareMonitorService...");
    if let Ok(output) = std::process::Command::new("sc")
        .args(["query", "LibreHardwareMonitorService"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("RUNNING") || stdout.contains("STARTED") {
            eprintln!("[LHM] LibreHardwareMonitorService is running");
            return Ok(());
        }
        eprintln!(
            "[LHM] LibreHardwareMonitorService not running: {}",
            stdout.trim()
        );
    }

    eprintln!("[LHM] Checking registry key...");
    if let Ok(output) = std::process::Command::new("reg")
        .args(["query", "HKLM\\SOFTWARE\\LibreHardwareMonitor"])
        .output()
    {
        if output.status.success() {
            eprintln!("[LHM] Registry key exists");
            return Ok(());
        }
        eprintln!("[LHM] Registry key not found");
    }

    Err("No LHM installation found".to_string())
}

#[cfg(target_os = "windows")]
pub fn is_lhm_available() -> bool {
    // First check if process is running (portable mode)
    if is_lhm_running() {
        eprintln!("[LHM] Process is running");
        return true;
    }
    
    // Check for service installation
    if let Ok(output) = std::process::Command::new("reg")
        .args(["query", "HKLM\\SOFTWARE\\LibreHardwareMonitor"])
        .output()
        && output.status.success()
    {
        eprintln!("[LHM] Registry key exists but service not running");
        return false;
    }

    false
}

#[cfg(target_os = "windows")]
pub fn get_lhm_cpu_temp() -> (f32, bool) {
    match WMIConnection::new() {
        Ok(wmi) => {
            match wmi.raw_query::<HashMap<String, Variant>>(
                "SELECT Value FROM Sensor WHERE SensorType = 'Temperature'",
            ) {
                Ok(results) => {
                    for row in &results {
                        if let Some(Variant::R4(val)) = row.get("Value") {
                            eprintln!("[LHM] CPU temperature from LHM: {}C", val);
                            return (*val, true);
                        }
                        if let Some(Variant::R8(val)) = row.get("Value") {
                            eprintln!("[LHM] CPU temperature from LHM: {}C", val);
                            return (*val as f32, true);
                        }
                    }
                    eprintln!("[LHM] No temperature sensor found in WMI");
                }
                Err(e) => eprintln!("[LHM] WMI query failed: {}", e),
            }
        }
        Err(e) => eprintln!("[LHM] Failed to connect to WMI: {}", e),
    }
    (0.0, false)
}

#[cfg(target_os = "windows")]
pub fn is_lhm_running() -> bool {
    use sysinfo::System;
    
    let sys = System::new_all();
    
    // Try exact match first, then partial match
    for process in sys.processes().values() {
        if let Some(name) = process.name().to_str() {
            let name_lower = name.to_lowercase();
            if name_lower.contains("librehardwaremonitor") {
                eprintln!("[LHM] Found running process: {}", name);
                return true;
            }
        }
    }
    false
}

#[cfg(target_os = "windows")]
pub fn minimize_lhm() -> Result<(), String> {
    use std::process::Command;

    let powershell_script = r#"
        $process = Get-Process | Where-Object { $_.ProcessName -eq 'LibreHardwareMonitor' -or $_.Path -like '*LibreHardwareMonitor*' } | Select-Object -First 1
        if ($process) {
            $window = $process.MainWindowHandle
            if ($window -ne [IntPtr]::Zero) {
                [user32.dll]::ShowWindow($window, 7)
            }
        }
    "#;

    match Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Command", powershell_script])
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("Failed to minimize LHM: {}", stderr))
            }
        }
        Err(e) => Err(format!("Failed to run minimize command: {}", e)),
    }
}

#[cfg(target_os = "windows")]
pub fn configure_lhm_auto_minimize() -> Result<(), String> {
    use std::process::Command;

    let powershell_script = r#"
        $regPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
        $appName = "LibreHardwareMonitor"
        $appPath = "$env:LOCALAPPDATA\LibreHardwareMonitor\LibreHardwareMonitor.exe"
        
        if (Test-Path $appPath) {
            Set-ItemProperty -Path $regPath -Name $appName -Value $appPath
            Write-Host "Configured LHM to auto-start minimized"
        } else {
            Write-Host "LHM not found at $appPath"
        }
    "#;

    match Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Command", powershell_script])
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("Failed to configure LHM auto-start: {}", stderr))
            }
        }
        Err(e) => Err(format!("Failed to run config command: {}", e)),
    }
}

#[cfg(target_os = "windows")]
pub async fn download_and_install_lhm() -> Result<(), String> {
    use reqwest::Client;
    use std::fs;
    use zip::ZipArchive;

    eprintln!("[LHM] Starting download_and_install_lhm()");

    eprintln!("[LHM] Fetching latest release info from GitHub...");

    let client = Client::new();
    eprintln!("[LHM] Created HTTP client");

    eprintln!("[LHM] Making GitHub API request...");
    let release = client
        .get("https://api.github.com/repos/LibreHardwareMonitor/LibreHardwareMonitor/releases/latest")
        .header("User-Agent", "llama-monitor")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch LHM release info: {}", e))?;
    eprintln!("[LHM] GitHub API request completed");

    eprintln!("[LHM] Parsing JSON response...");
    let release_json: serde_json::Value = release
        .json()
        .await
        .map_err(|e| format!("Failed to parse LHM release JSON: {}", e))?;
    eprintln!("[LHM] JSON parsing completed");

    eprintln!(
        "[LHM] Latest release: {}",
        release_json["tag_name"].as_str().unwrap_or("unknown")
    );

    eprintln!("[LHM] Processing release assets...");
    let assets = release_json["assets"]
        .as_array()
        .ok_or("No assets in release")?;
    eprintln!("[LHM] Found {} assets", assets.len());

    eprintln!("[LHM] Searching for LibreHardwareMonitor.zip...");
    let zip_url = assets
        .iter()
        .find(|a| a["name"].as_str() == Some("LibreHardwareMonitor.zip"))
        .ok_or("LibreHardwareMonitor.zip not found in latest release")?["browser_download_url"]
        .as_str()
        .ok_or("browser_download_url not found")?;
    eprintln!("[LHM] Found download URL: {}", zip_url);

    eprintln!("[LHM] Starting download from: {}", zip_url);
    let zip_response = client
        .get(zip_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download LHM: {}", e))?;
    eprintln!("[LHM] Download response received");

    eprintln!("[LHM] Reading response bytes...");
    let zip_bytes = zip_response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read LHM ZIP: {}", e))?;
    eprintln!("[LHM] Downloaded {} bytes", zip_bytes.len());

    let install_dir = std::env::var("LOCALAPPDATA")
        .map_err(|_| "LOCALAPPDATA not set".to_string())?
        .to_string();
    let lhm_dir = Path::new(&install_dir).join("LibreHardwareMonitor");

    eprintln!("[LHM] Installing to: {}", lhm_dir.display());

    eprintln!("[LHM] Creating installation directory...");
    fs::create_dir_all(&lhm_dir)
        .map_err(|e| format!("Failed to create install directory: {}", e))?;
    eprintln!("[LHM] Directory created successfully");

    eprintln!("[LHM] Creating progress file...");
    let progress_file = lhm_dir.join("install_progress.txt");
    fs::write(&progress_file, "downloading: 0%")
        .map_err(|e| format!("Failed to create progress file: {}", e))?;
    eprintln!("[LHM] Progress file created");

    eprintln!("[LHM] Preparing ZIP archive...");
    let zip_reader = std::io::Cursor::new(zip_bytes);
    let mut archive =
        ZipArchive::new(zip_reader).map_err(|e| format!("Failed to extract LHM ZIP: {}", e))?;
    eprintln!("[LHM] Archive ready, {} files to extract", archive.len());

    eprintln!("[LHM] Starting extraction...");

    let total_files = archive.len();
    for i in 0..total_files {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;
        let path = lhm_dir.join(file.mangled_name());

        if file.is_dir() {
            fs::create_dir_all(&path)
                .map_err(|e| format!("Failed to create directory {}: {}", path.display(), e))?;
        } else {
            let parent: &Path = path.parent().unwrap();
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
            let mut output = fs::File::create(&path)
                .map_err(|e| format!("Failed to create file {}: {}", path.display(), e))?;
            std::io::copy(&mut file, &mut output)
                .map_err(|e| format!("Failed to write file {}: {}", path.display(), e))?;
        }

        fs::write(
            &progress_file,
            format!(
                "extracting: {}%",
                ((i + 1) as f64 / total_files as f64 * 100.0).round() as u8
            ),
        )
        .map_err(|e| format!("Failed to update progress: {}", e))?;
    }
    eprintln!(
        "[LHM] Extraction complete, {} files extracted",
        archive.len()
    );

    let lhm_exe = lhm_dir.join("LibreHardwareMonitor.exe");
    if !lhm_exe.exists() {
        return Err("LibreHardwareMonitor.exe not found after extraction".to_string());
    }

    eprintln!("[LHM] Verifying installation...");
    let lhm_exe = lhm_dir.join("LibreHardwareMonitor.exe");
    eprintln!("[LHM] LHM executable path: {}", lhm_exe.display());

    if !lhm_exe.exists() {
        return Err("LibreHardwareMonitor.exe not found after extraction".to_string());
    }
    eprintln!("[LHM] Executable verified");

    eprintln!("[LHM] Running LHM installer (-s flag) with UAC elevation...");
    let lhm_exe_str = lhm_exe.to_string_lossy().replace("\\", "\\\\");
    let powershell_cmd = format!(
        "Start-Process '{}' -ArgumentList '-s' -Verb RunAs -Wait",
        lhm_exe_str
    );
    eprintln!("[LHM] PowerShell command: {}", powershell_cmd);

    let output = std::process::Command::new("powershell")
        .arg("-Command")
        .arg(&powershell_cmd)
        .output()
        .map_err(|e| format!("Failed to run LHM installer: {}", e))?;
    eprintln!(
        "[LHM] Installer command completed, success: {}",
        output.status.success()
    );

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("[LHM] Installer stderr: {}", stderr);
        fs::write(&progress_file, "failed")
            .map_err(|e| format!("Failed to update progress: {}", e))?;
        return Err(format!("LHM installer failed: {}", stderr));
    }

    fs::write(&progress_file, "completed")
        .map_err(|e| format!("Failed to update progress: {}", e))?;

    eprintln!("[LHM] Verifying LHM is running...");
    if is_lhm_running() {
        eprintln!("[LHM] LHM is running, minimizing window...");
        match minimize_lhm() {
            Ok(()) => eprintln!("[LHM] LHM window minimized successfully"),
            Err(e) => eprintln!("[LHM] Failed to minimize LHM: {}", e),
        }

        eprintln!("[LHM] Configuring auto-start with Windows...");
        match configure_lhm_auto_minimize() {
            Ok(()) => eprintln!("[LHM] Auto-start configured successfully"),
            Err(e) => eprintln!("[LHM] Failed to configure auto-start: {}", e),
        }
    } else {
        eprintln!("[LHM] Warning: LHM process not detected after installation");
    }

    eprintln!("[LHM] LHM installer completed successfully");
    eprintln!("[LHM] download_and_install_lhm() finished successfully");

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn uninstall_lhm() -> Result<(), String> {
    use std::fs;
    use std::process::Command;

    let install_dir = std::env::var("LOCALAPPDATA")
        .map_err(|_| "LOCALAPPDATA not set".to_string())?
        .to_string();
    let lhm_dir = Path::new(&install_dir).join("LibreHardwareMonitor");

    eprintln!("[LHM] Checking installation directory: {}", lhm_dir.display());

    if !lhm_dir.exists() {
        return Err("LHM not installed".to_string());
    }

    eprintln!("[LHM] Stopping LHM process...");
    let powershell_stop = r#"
        $process = Get-Process | Where-Object { $_.ProcessName -eq 'LibreHardwareMonitor' -or $_.Path -like '*LibreHardwareMonitor*' } | Select-Object -First 1
        if ($process) {
            Stop-Process -Id $process.Id -Force
            Start-Sleep -Seconds 2
        }
    "#;

    match Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Command", powershell_stop])
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("[LHM] Failed to stop LHM process: {}", stderr);
            }
        }
        Err(e) => eprintln!("[LHM] Failed to run stop command: {}", e),
    }

    eprintln!("[LHM] Removing auto-start registry entry...");
    let powershell_registry = r#"
        $regPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
        if (Test-Path $regPath) {
            Remove-ItemProperty -Path $regPath -Name "LibreHardwareMonitor" -Force -ErrorAction SilentlyContinue
        }
    "#;

    match Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Command", powershell_registry])
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("[LHM] Failed to remove registry entry: {}", stderr);
            }
        }
        Err(e) => eprintln!("[LHM] Failed to run registry command: {}", e),
    }

    eprintln!("[LHM] Deleting installation directory...");
    match fs::remove_dir_all(&lhm_dir) {
        Ok(()) => {
            eprintln!("[LHM] Directory deleted successfully");
        }
        Err(e) => {
            eprintln!("[LHM] Failed to delete directory: {}", e);
            return Err(format!("Failed to delete LHM directory: {}", e));
        }
    }

    eprintln!("[LHM] Uninstallation complete");
    Ok(())
}
