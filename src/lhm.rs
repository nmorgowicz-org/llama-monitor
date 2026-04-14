#[cfg(target_os = "windows")]
use std::path::Path;

#[cfg(target_os = "windows")]
use wmi::{Variant, WMIConnection};

#[cfg(target_os = "windows")]
use std::collections::HashMap;

#[cfg(target_os = "windows")]
pub async fn ensure_lhm_available() -> Result<(), String> {
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
pub async fn download_and_install_lhm() -> Result<(), String> {
    use reqwest::Client;
    use std::fs;
    use zip::ZipArchive;

    eprintln!("[LHM] Fetching latest release info from GitHub...");

    let client = Client::new();
    let release = client
        .get("https://api.github.com/repos/LibreHardwareMonitor/LibreHardwareMonitor/releases/latest")
        .header("User-Agent", "llama-monitor")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch LHM release info: {}", e))?;

    let release_json: serde_json::Value = release
        .json()
        .await
        .map_err(|e| format!("Failed to parse LHM release JSON: {}", e))?;

    eprintln!(
        "[LHM] Latest release: {}",
        release_json["tag_name"].as_str().unwrap_or("unknown")
    );

    let assets = release_json["assets"]
        .as_array()
        .ok_or("No assets in release")?;
    let zip_url = assets
        .iter()
        .find(|a| a["name"].as_str() == Some("LibreHardwareMonitor.zip"))
        .ok_or("LibreHardwareMonitor.zip not found in latest release")?["browser_download_url"]
        .as_str()
        .ok_or("browser_download_url not found")?;

    eprintln!("[LHM] Downloading LHM from: {}", zip_url);

    let zip_response = client
        .get(zip_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download LHM: {}", e))?;

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

    fs::create_dir_all(&lhm_dir)
        .map_err(|e| format!("Failed to create install directory: {}", e))?;

    let zip_reader = std::io::Cursor::new(zip_bytes);
    let mut archive =
        ZipArchive::new(zip_reader).map_err(|e| format!("Failed to extract LHM ZIP: {}", e))?;

    eprintln!("[LHM] Extracting {} files...", archive.len());
    for i in 0..archive.len() {
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
    }
    eprintln!("[LHM] Extraction complete");

    let lhm_exe = lhm_dir.join("LibreHardwareMonitor.exe");
    if !lhm_exe.exists() {
        return Err("LibreHardwareMonitor.exe not found after extraction".to_string());
    }

    eprintln!("[LHM] Running LHM installer (-s flag)...");
    let output = std::process::Command::new(&lhm_exe)
        .arg("-s")
        .output()
        .map_err(|e| format!("Failed to run LHM installer: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("LHM installer failed: {}", stderr));
    }

    eprintln!("[LHM] LHM installer completed successfully");

    Ok(())
}
