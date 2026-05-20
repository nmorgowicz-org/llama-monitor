# Windows Sensor Bridge Implementation

## Overview

On Windows, CPU temperature cannot be read by Rust code without a kernel driver. The solution is a small C# sidecar service (`sensor_bridge.exe`) that uses the `LibreHardwareMonitorLib` NuGet package to read kernel-level sensor data and serve it as JSON over a local HTTP server. The Rust app polls `http://127.0.0.1:7780/` on a short interval to fetch the latest readings.

**Data sources collected:**

| Metric | Source |
|--------|--------|
| CPU temperature | `sensor_bridge.exe` via LibreHardwareMonitorLib → `http://127.0.0.1:7780/` |
| CPU load % | `sysinfo` crate |
| CPU clock speed (MHz) | WMI `Win32_PerfFormattedData_Counters_ProcessorInformation` |
| CPU model name | WMI `Win32_Processor` |
| Motherboard name/model | WMI `Win32_BaseBoard` |
| RAM usage | `sysinfo` crate |
| GPU name + VRAM | WMI `Win32_VideoController` (fallback when nvidia-smi/rocm-smi unavailable) |
| GPU temp/utilization | `nvidia-smi` (NVIDIA) or `rocm-smi` (AMD); not available for Intel via WMI |

---

## Architecture

```
llama-monitor.exe
  │  polls every 500 ms
  ↓
http://127.0.0.1:7780/    ← sensor_bridge.exe (persistent HTTP server)
  │  responds with cached JSON
  │  updates cache every 5 seconds via LHM
  ↓
LibreHardwareMonitor kernel driver
  (reads CPU Package temp, motherboard sensors)
```

The sensor bridge runs as a **persistent background process** — it does not exit after one request. It maintains a 5-second refresh timer for sensor data and serves cached readings to any client that connects. This avoids the overhead of spawning a new process per poll.

---

## sensor_bridge.exe

### Source

`sensor_bridge/Program.cs` in the repository root.

### What it does

1. Opens `LibreHardwareMonitor.Hardware.Computer` with CPU and Motherboard sensors enabled
2. Starts a 5-second refresh timer via `System.Threading.Timer`
3. Binds `HttpListener` to `http://127.0.0.1:7780/`
4. On each GET request, returns the latest sensor JSON (does not re-read hardware on each request)
5. Handles port conflicts by deleting stale netsh URL reservations and killing duplicate sensor_bridge processes

### JSON response format

```json
[
  {
    "hardware": "Intel Core i9-14900K",
    "subhardware": null,
    "name": "CPU Package",
    "type": "Temperature",
    "value": 52.0
  },
  {
    "hardware": "Intel Core i9-14900K",
    "subhardware": "Core #0",
    "name": "Temperature",
    "type": "Temperature",
    "value": 48.0
  }
]
```

Fields:
- `hardware` — top-level hardware name (CPU model, motherboard model)
- `subhardware` — sub-component name, or `null` if a top-level sensor
- `name` — sensor name (e.g., "CPU Package", "Core #1")
- `type` — sensor type string (e.g., "Temperature", "Load", "Clock")
- `value` — current reading as a float, or `null` if unavailable

### Building sensor_bridge.exe

Requires .NET SDK 8.0+ and the `LibreHardwareMonitorLib` NuGet package. Build from the `sensor_bridge/` directory:

```bash
dotnet publish -c Release -r win-x64 --self-contained true \
  -p:PublishSingleFile=true -o ./publish
```

This produces a single self-contained `sensor_bridge.exe` (~30 MB) in `./publish`. No .NET runtime installation required on the target machine.

### Distribution

`sensor_bridge.exe` must be placed **next to** `llama-monitor.exe` in the installation directory. The release workflow (`release.yml`) already handles this:

```yaml
cp sensor_bridge/publish/sensor_bridge.exe windows-bundle/sensor_bridge.exe
```

The Rust code in `src/lhm.rs` locates the binary relative to the current executable:

```rust
let bridge_path = std::env::current_exe()
    .ok()
    .and_then(|p| p.parent().map(|d| d.join("sensor_bridge.exe")));
```

---

## Rust Integration

**Files:** `src/lhm.rs`, `src/lhm_persistence.rs`, `static/js/windows-lhm.js`

### Lifecycle managed by `src/lhm.rs`

| Function | Purpose |
|----------|---------|
| `is_sensor_bridge_available()` | Checks if `sensor_bridge.exe` exists next to the binary |
| `is_lhm_available()` | Alias for `is_sensor_bridge_available()` |
| `is_local_sensor_bridge_running()` | Probes `http://127.0.0.1:7780/` for a live response |
| `poll_local_sensor_bridge_temp()` | GET `http://127.0.0.1:7780/`, parse JSON, return CPU Package temp |
| `install_local_sensor_bridge()` | Runs UAC-elevated PowerShell to register a Windows Scheduled Task that starts `sensor_bridge.exe` at system startup as SYSTEM |
| `uninstall_local_sensor_bridge()` | Removes the scheduled task |

### Temperature polling (`src/system.rs`)

```rust
#[cfg(target_os = "windows")]
{
    let (temp, available) = crate::lhm::get_lhm_cpu_temp();
    metrics.cpu_temp = temp;
    metrics.cpu_temp_available = available;
}
```

Called on every system metrics poll (every 5 seconds by default). If `sensor_bridge` is not running or returns no data, `available` is `false` and temperature is not shown in the UI.

### Frontend (`static/js/windows-lhm.js`)

The frontend provides a UI for managing the sensor bridge installation in the Settings modal. It calls `/api/sensor-bridge/*` endpoints to:
- Check if sensor_bridge.exe is present
- Check if the service is running
- Install / uninstall the scheduled task

---

## Elevation and Permissions

The LibreHardwareMonitor kernel driver requires **Administrator privileges** to load. The sensor bridge handles this by running as SYSTEM via a Windows Scheduled Task (installed by `install_local_sensor_bridge()` using UAC-elevated PowerShell).

When running `sensor_bridge.exe` directly (e.g., for testing), launch the terminal as Administrator. Without elevation, the sensor reading array will be empty (`[]`).

---

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| CPU temp shows unavailable | sensor_bridge not running | Install via Settings → Sensor Bridge tab |
| `sensor_bridge.exe` returns `[]` | Not running as Administrator | Run terminal as Admin; or install as scheduled task |
| Port 7780 conflict | Another process or stale URL reservation | sensor_bridge auto-resolves via netsh cleanup and process kill on startup |
| "sensor_bridge.exe not found" | Binary missing from install dir | Re-run the installer or copy `sensor_bridge.exe` next to `llama-monitor.exe` |
| Sensor bridge crashes immediately | Missing .NET runtime | Use the self-contained build (`--self-contained true`); no .NET required |

---

**Last updated:** 2026-05-19
