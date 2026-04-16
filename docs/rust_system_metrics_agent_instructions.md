# Agent Instructions: Rust System Metrics App with LHM Sidecar

## Overview

You are building a Windows system metrics application in Rust. The app collects the following data:

- Motherboard name and model
- CPU temperature (requires a C# sidecar binary)
- CPU load (usage %)
- Current CPU clock speed (MHz)
- RAM usage (% used and free GB)
- CPU model name

**Architecture summary:** Most metrics come from native Rust crates. CPU temperature cannot be read natively on Windows without a kernel driver. The solution is a small C# sidecar executable (`sensor_bridge.exe`) that uses the `LibreHardwareMonitorLib` NuGet package to read the kernel-level sensor data and output it as JSON to stdout. The Rust app spawns this sidecar as a child process and parses its output.

---

## Part 1: Prerequisites

### 1.1 Required Tooling

Ensure the following are installed before proceeding:

| Tool | Purpose | Install Command / URL |
|---|---|---|
| Rust + Cargo | Main app | https://rustup.rs |
| .NET SDK 8.0 or newer | Build the C# sidecar | https://dotnet.microsoft.com/download |
| Visual Studio Code or any editor | Optional but helpful | -- |

Verify installs:
```bash
rustc --version
cargo --version
dotnet --version
```

### 1.2 Required Privileges

The C# sidecar (`sensor_bridge.exe`) **must run as Administrator** to access hardware sensor data via the LibreHardwareMonitor kernel driver. Your Rust app must either:

- Be launched as Administrator, OR
- Use a manifest file to request elevation (see Part 4)

---

## Part 2: Build the C# Sidecar (`sensor_bridge`)

This is a standalone .NET console app. It reads all hardware sensor data and prints it as a JSON array to stdout, then exits.

### 2.1 Create the Project

```bash
mkdir sensor_bridge
cd sensor_bridge
dotnet new console
```

### 2.2 Add LibreHardwareMonitorLib

```bash
dotnet add package LibreHardwareMonitorLib
```

This NuGet package handles all kernel driver interaction internally. No separate driver install is needed.

### 2.3 Replace Program.cs

Replace the contents of `Program.cs` with the following:

```csharp
using LibreHardwareMonitor.Hardware;
using System.Text.Json;

class UpdateVisitor : IVisitor
{
    public void VisitComputer(IComputer computer)
    {
        computer.Traverse(this);
    }
    public void VisitHardware(IHardware hardware)
    {
        hardware.Update();
        foreach (var sub in hardware.SubHardware) sub.Accept(this);
    }
    public void VisitSensor(ISensor sensor) { }
    public void VisitParameter(IParameter parameter) { }
}

var computer = new Computer
{
    IsCpuEnabled = true,
    IsMotherboardEnabled = true,
    IsMemoryEnabled = true,
    IsGpuEnabled = false,   // set true if you want GPU temps later
    IsStorageEnabled = false
};

computer.Open();
computer.Accept(new UpdateVisitor());

var sensors = new List<object>();

foreach (var hardware in computer.Hardware)
{
    foreach (var subHardware in hardware.SubHardware)
    {
        foreach (var sensor in subHardware.Sensors)
        {
            sensors.Add(new
            {
                hardware = hardware.Name,
                subhardware = subHardware.Name,
                name = sensor.Name,
                type = sensor.SensorType.ToString(),
                value = sensor.Value
            });
        }
    }

    foreach (var sensor in hardware.Sensors)
    {
        sensors.Add(new
        {
            hardware = hardware.Name,
            subhardware = (string?)null,
            name = sensor.Name,
            type = sensor.SensorType.ToString(),
            value = sensor.Value
        });
    }
}

computer.Close();

Console.WriteLine(JsonSerializer.Serialize(sensors));
```

### 2.4 Build the Sidecar

```bash
dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -o ./publish
```

This produces a single `sensor_bridge.exe` in `./publish`. Copy this file into your Rust project's output directory or a `bin/` subfolder.

**Important:** `--self-contained true` means the .NET runtime is bundled. The user does not need .NET installed separately.

### 2.5 Verify the Sidecar Works

Run it manually as Administrator from a terminal:

```powershell
.\publish\sensor_bridge.exe
```

You should see JSON output like:

```json
[
  {"hardware":"Intel Core i9-14900K","subhardware":null,"name":"CPU Package","type":"Temperature","value":52.0},
  {"hardware":"Intel Core i9-14900K","subhardware":null,"name":"CPU Core #1","type":"Temperature","value":48.0},
  ...
]
```

If you see an empty array `[]`, the app is running without Administrator privileges. Re-run as Admin.

---

## Part 3: Build the Rust Application

### 3.1 Create the Project

```bash
cargo new system_metrics
cd system_metrics
```

### 3.2 Cargo.toml Dependencies

Replace the `[dependencies]` section in `Cargo.toml`:

```toml
[dependencies]
sysinfo = "0.30"
wmi = "0.13"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### 3.3 Project Structure

Place `sensor_bridge.exe` in a `bin/` folder inside your project:

```
system_metrics/
├── Cargo.toml
├── src/
│   └── main.rs
└── bin/
    └── sensor_bridge.exe
```

### 3.4 main.rs

```rust
use serde::Deserialize;
use sysinfo::{CpuExt, System, SystemExt};
use wmi::{COMLibrary, WMIConnection};

// WMI structs for motherboard and CPU model
#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct BaseBoard {
    Manufacturer: String,
    Product: String,
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct Processor {
    Name: String,
}

// Matches the JSON output from sensor_bridge.exe
#[derive(Deserialize, Debug)]
struct SensorReading {
    hardware: String,
    name: String,
    #[serde(rename = "type")]
    sensor_type: String,
    value: Option<f64>,
}

fn get_cpu_temperature() -> Option<f64> {
    // Locate sensor_bridge.exe relative to the current executable
    let exe_dir = std::env::current_exe()
        .ok()?
        .parent()?
        .to_path_buf();

    let bridge_path = exe_dir.join("bin").join("sensor_bridge.exe");

    if !bridge_path.exists() {
        eprintln!("sensor_bridge.exe not found at {:?}", bridge_path);
        return None;
    }

    let output = std::process::Command::new(&bridge_path)
        .output()
        .ok()?;

    if !output.status.success() {
        eprintln!("sensor_bridge.exe failed: {:?}", output.status);
        return None;
    }

    let json_str = String::from_utf8(output.stdout).ok()?;
    let readings: Vec<SensorReading> = serde_json::from_str(&json_str).ok()?;

    // Find the first CPU Package temperature reading
    readings
        .iter()
        .find(|r| r.sensor_type == "Temperature" && r.name.contains("Package"))
        .and_then(|r| r.value)
}

fn main() {
    // --- sysinfo: CPU load, clock, RAM ---
    let mut sys = System::new_all();
    sys.refresh_all();

    println!("=== CPU ===");
    for (i, cpu) in sys.cpus().iter().enumerate() {
        println!(
            "  Core {}: {:.1}% load @ {} MHz",
            i,
            cpu.cpu_usage(),
            cpu.frequency()
        );
    }

    // --- WMI: motherboard and CPU model ---
    println!("\n=== System Info ===");
    match COMLibrary::new() {
        Ok(com) => {
            match WMIConnection::new(com.into()) {
                Ok(wmi) => {
                    // CPU model
                    let cpus: Result<Vec<Processor>, _> = wmi.query();
                    match cpus {
                        Ok(list) => {
                            for cpu in &list {
                                println!("  CPU Model: {}", cpu.Name.trim());
                            }
                        }
                        Err(e) => eprintln!("  CPU WMI error: {}", e),
                    }

                    // Motherboard
                    let boards: Result<Vec<BaseBoard>, _> = wmi.query();
                    match boards {
                        Ok(list) => {
                            for board in &list {
                                println!(
                                    "  Motherboard: {} {}",
                                    board.Manufacturer.trim(),
                                    board.Product.trim()
                                );
                            }
                        }
                        Err(e) => eprintln!("  Motherboard WMI error: {}", e),
                    }
                }
                Err(e) => eprintln!("  WMI connection error: {}", e),
            }
        }
        Err(e) => eprintln!("  COM init error: {}", e),
    }

    // --- RAM ---
    println!("\n=== Memory ===");
    let total_gb = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let used_gb = sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let free_gb = total_gb - used_gb;
    let used_pct = (used_gb / total_gb) * 100.0;
    println!("  Total: {:.2} GB", total_gb);
    println!("  Used:  {:.2} GB ({:.1}%)", used_gb, used_pct);
    println!("  Free:  {:.2} GB", free_gb);

    // --- CPU Temperature via sidecar ---
    println!("\n=== Temperature ===");
    match get_cpu_temperature() {
        Some(temp) => println!("  CPU Package: {:.1} °C", temp),
        None => println!("  CPU temp unavailable (run as Administrator or check sensor_bridge.exe)"),
    }
}
```

### 3.5 Build and Run

```bash
cargo build --release
```

Copy `bin/sensor_bridge.exe` into the same `bin/` folder next to the compiled `system_metrics.exe`:

```
target/release/
├── system_metrics.exe
└── bin/
    └── sensor_bridge.exe
```

Run as Administrator:

```bash
.\target\release\system_metrics.exe
```

---

## Part 4: Elevation (Running as Administrator)

The app needs admin rights for temperature data. Two options:

### Option A: Manifest File (Recommended for Distribution)

Create `system_metrics.exe.manifest`:

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
```

Embed it using `mt.exe` (from Windows SDK) or add a `build.rs` in Rust using the `winres` crate:

```toml
# Cargo.toml
[build-dependencies]
winres = "0.1"
```

```rust
// build.rs
fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_manifest_file("system_metrics.exe.manifest");
        res.compile().unwrap();
    }
}
```

### Option B: Graceful Fallback (No Elevation Required)

If you don't want to force elevation, handle the `None` case from `get_cpu_temperature()` gracefully and display all other metrics normally. Temperature will simply show as unavailable when not elevated.

---

## Part 5: Troubleshooting

| Symptom | Likely Cause | Fix |
|---|---|---|
| `sensor_bridge.exe` outputs `[]` | Not running as Admin | Re-run terminal as Administrator |
| `sensor_bridge.exe not found` | Wrong path | Ensure `bin/sensor_bridge.exe` is next to your `.exe` |
| WMI motherboard returns empty | Rare on some boards | Try querying `Win32_ComputerSystem` instead for manufacturer |
| CPU temp shows `None` | Sidecar failed silently | Run `sensor_bridge.exe` manually and check output |
| `Generic failure` on WMI | LHM WMI bridge issue (irrelevant here) | Not applicable, sidecar approach bypasses WMI entirely |
| sysinfo shows 0 MHz clock | Needs a second `refresh_all()` call | Call `sys.refresh_all()` twice with a short sleep between |

---

## Part 6: File Checklist

Before shipping or testing, confirm these files exist:

- [ ] `sensor_bridge/publish/sensor_bridge.exe` (built in Part 2)
- [ ] `system_metrics/src/main.rs` (written in Part 3)
- [ ] `system_metrics/Cargo.toml` (with correct dependencies)
- [ ] `sensor_bridge.exe` copied into `target/release/bin/`
- [ ] App launched as Administrator

---

## Summary of Data Sources

| Metric | Source |
|---|---|
| CPU load % | `sysinfo` crate |
| CPU clock speed (MHz) | `sysinfo` crate |
| CPU model name | WMI `Win32_Processor` via `wmi` crate |
| Motherboard name/model | WMI `Win32_BaseBoard` via `wmi` crate |
| RAM total, used, free | `sysinfo` crate |
| CPU temperature | `sensor_bridge.exe` via `LibreHardwareMonitorLib` |
