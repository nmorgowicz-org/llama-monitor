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

Requires .NET SDK 10.0 and the `LibreHardwareMonitorLib` NuGet package. Build from the `sensor_bridge/` directory:

```bash
dotnet publish sensor_bridge/sensor_bridge.csproj -c Release -r win-x64 \
  --self-contained true -p:PublishSingleFile=true \
  -p:IncludeNativeLibrariesForSelfExtract=true
```

The project file (`sensor_bridge/sensor_bridge.csproj`) pins `net10.0`, `RuntimeIdentifier win-x64`, `SelfContained true`, and `PublishSingleFile true`, so the flags above are redundant but are kept explicit in CI for clarity.

This produces a single self-contained `sensor_bridge.exe` in `./publish`. **No .NET runtime installation is required on the target machine.** The runtime is bundled into the executable. `LibreHardwareMonitorLib 0.9.6` ships a `net8.0` assembly that is forward-compatible with net10.0 — no special steps required.

The CI release workflow (`release.yml`) builds with `--self-contained true` explicitly alongside the csproj settings.

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

## Driver requirement — PawnIO

`LibreHardwareMonitorLib 0.9.6` uses the **PawnIO** kernel driver to access hardware sensors. PawnIO is a separately-installed, signed kernel driver; it is not bundled inside `sensor_bridge.exe`.

**If the PawnIO driver is not present on the target machine, the bridge process will start and respond on port 7780, but will return no temperature readings (`[]`).** The `/api/sensor-bridge/status` endpoint returns a `pawnio` boolean so the dashboard can distinguish "bridge running but driver missing" from "bridge not running". When the bridge runs but PawnIO is absent, the dashboard surfaces a "driver missing" message with a link to pawnio.eu.

### One-click setup (recommended)

The **Settings → Sensor Bridge** install flow (which already registers the scheduled task) now also installs PawnIO inside the same elevated PowerShell session — a single UAC prompt handles everything:

```powershell
# Idempotent: guarded by sc query PawnIO before running
winget install -e --id namazso.PawnIO --silent `
  --accept-package-agreements --accept-source-agreements --disable-interactivity
```

A failed winget install is non-fatal; the scheduled task is still registered and the dashboard will report the missing driver state rather than silently showing no data.

### Manual install

If winget is unavailable, download and run the official PawnIO installer from [pawnio.eu](https://pawnio.eu) or the [namazso/PawnIO.Setup](https://github.com/namazso/PawnIO.Setup) GitHub releases page.

### Legacy note — WinRing0 and Defender (builds ≤ 0.9.4 only)

Older LHM builds (0.9.4 and earlier) extracted a `WinRing0x64.sys` driver to disk at runtime. Since approximately March 2025, Microsoft Defender flags WinRing0 (CVE-2020-14979) and may quarantine the extracted `.sys` file, silently disabling temperature readings. **This does not apply to 0.9.6**, which is a PawnIO build and does not use WinRing0.

Do not chase prerelease versions (0.9.5/0.9.7-pre): those carry documented regressions (AMD Family 10h temporarily disabled; missing Nuvoton sensors). Stay on stable 0.9.6.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| CPU temp shows unavailable | sensor_bridge not running | Install via Settings → Sensor Bridge tab |
| Bridge running but no temperature | PawnIO driver not installed | Use one-click setup or install PawnIO manually from pawnio.eu |
| `sensor_bridge.exe` returns `[]` | Not running as Administrator | Run terminal as Admin; or install as scheduled task |
| Port 7780 conflict | Another process or stale URL reservation | sensor_bridge auto-resolves via netsh cleanup and process kill on startup |
| "sensor_bridge.exe not found" | Binary missing from install dir | Re-run the installer or copy `sensor_bridge.exe` next to `llama-monitor.exe` |
| Sensor bridge crashes immediately | Missing .NET runtime | Use the self-contained build (current default); no .NET required |

---

**Last updated:** 2026-06-22
