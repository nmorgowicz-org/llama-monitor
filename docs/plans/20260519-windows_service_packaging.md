# Windows Service Packaging

**Date:** 2026-05-19
**Status:** Planned
**Priority:** Low
**Prerequisite:** Windows tray WebView (branch `feature/windows-tray-webview-and-compat`, merged)

---

## Goal

Allow llama-monitor to run as a Windows Service (managed by the Service Control Manager) so it can start automatically at boot without requiring a user session. The tray UI would not be shown in service mode; the web UI remains accessible.

This is a distinct deployment model from the current tray app. The tray app (`--no-tray` optional) is the primary model and works correctly today. This plan adds an optional service mode.

---

## Current State

The app already has a headless mode (`--headless`) that skips the tray and runs only the web/API server. On Windows, the shutdown handler uses `tokio::signal::ctrl_c()` which handles Ctrl+C but does NOT handle:
- `SERVICE_CONTROL_STOP` from the Service Control Manager
- `SERVICE_CONTROL_SHUTDOWN` during system shutdown
- `SERVICE_CONTROL_PAUSE` / `SERVICE_CONTROL_CONTINUE`

The existing headless mode is the foundation for service mode; the only missing piece is the SCM integration.

**Relevant files:**
- `src/main.rs` — startup, headless check, shutdown handler
- `src/cli.rs` — CLI arg parsing (`--headless`, `--no-tray`)
- `Cargo.toml` — where the new dependency goes

---

## Implementation Plan

### Step 1: Add `windows-service` crate

In `Cargo.toml`, add under `[target.'cfg(windows)'.dependencies]`:

```toml
[target.'cfg(windows)'.dependencies]
    wmi = { version = "0.18.4", features = ["default"] }
    windows-service = "0.7"   # ← add this
```

The `windows-service` crate provides safe wrappers around Windows Service API.

### Step 2: Add `--service` CLI flag

In `src/cli.rs`, add to `AppArgs`:

```rust
/// Run as a Windows Service (service mode; implies --headless)
#[cfg(windows)]
#[arg(long, default_value_t = false)]
pub service: bool,
```

### Step 3: Add service entry point in `main.rs`

At the top of `main()`, before anything else, detect service mode and delegate:

```rust
#[cfg(windows)]
if args.service {
    return crate::service::run_as_service(args);
}
```

### Step 4: Create `src/service.rs` (Windows-only)

```rust
// src/service.rs — only compiled on Windows

use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode,
        ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

const SERVICE_NAME: &str = "LlamaMonitor";

define_windows_service!(ffi_service_main, service_main);

pub fn run_as_service(args: crate::cli::AppArgs) -> anyhow::Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
        .map_err(|e| anyhow::anyhow!("Failed to start service dispatcher: {e}"))?;
    Ok(())
}

fn service_main(_args: Vec<OsString>) {
    if let Err(e) = run_service() {
        eprintln!("[service] Fatal error: {e}");
    }
}

fn run_service() -> anyhow::Result<()> {
    // Channel to receive stop signal from SCM
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    let stop_tx = Arc::new(std::sync::Mutex::new(stop_tx));

    let status_handle = service_control_handler::register(
        SERVICE_NAME,
        move |control| match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = stop_tx.lock().unwrap().send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        },
    )?;

    // Report: Starting
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(10),
        process_id: None,
    })?;

    // --- Start the app (same as headless mode) ---
    // TODO: call the shared startup logic here, passing --headless equivalent
    // The cleanest approach is to extract the async startup into a function
    // `crate::run_headless(args) -> anyhow::Result<()>` that main() also calls.

    // Report: Running
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::ZERO,
        process_id: None,
    })?;

    // Block until stop signal
    stop_rx.recv().ok();

    // Report: Stopping
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StopPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(5),
        process_id: None,
    })?;

    // TODO: trigger graceful shutdown (WAL checkpoint, sessions save)

    Ok(())
}
```

### Step 5: Refactor `main.rs` to extract shared startup

The largest refactor: extract the startup logic that is currently inlined in `main()` into a function `run_headless(args, app_config) -> anyhow::Result<()>` so both `main()` and `service.rs` can call it. The tray-specific code stays in `main()`.

This is the most involved part of the implementation. Key extraction points:
- GPU/system poller spawning
- Warp server spawning  
- Sessions persistence timer
- DB maintenance tasks
- ACME renewal job
- Graceful shutdown hook (in service mode, shutdown is triggered by SCM signal, not SIGTERM)

### Step 6: Service installation

Two options for installation:

**Option A: PowerShell (manual, recommended for now)**
```powershell
# Install (run as Administrator)
New-Service -Name "LlamaMonitor" `
    -BinaryPathName "C:\path\to\llama-monitor.exe --service --port 7778" `
    -DisplayName "Llama Monitor" `
    -StartupType Automatic

# Start
Start-Service LlamaMonitor

# Stop  
Stop-Service LlamaMonitor

# Remove
Remove-Service LlamaMonitor   # Windows 10 1809+ / PowerShell 6+
# or: sc.exe delete LlamaMonitor
```

**Option B: Built-in `--install-service` / `--uninstall-service` flags**

Add CLI flags that call `windows_service::service_manager` to register/unregister the service programmatically. Requires admin privileges at install time.

```rust
#[cfg(windows)]
if args.install_service {
    return crate::service::install_service(&args);
}
#[cfg(windows)]
if args.uninstall_service {
    return crate::service::uninstall_service();
}
```

Use `windows_service::service_manager::ServiceManager` for this.

### Step 7: Service identity and permissions

The service needs to run as a specific account with access to the config directory. Options:
- **LocalSystem**: has full machine access, simple but overprivileged
- **LocalService**: limited rights, may not have access to config dir
- **NetworkService**: similar to LocalService but with network identity
- **Named account**: best practice, requires creating a dedicated user

For the initial implementation, `LocalSystem` is simplest. Document the trade-off.

The config dir on Windows is `%APPDATA%\llama-monitor\` which is user-specific. When running as LocalSystem, use `C:\ProgramData\llama-monitor\` instead. This means the service and tray app use different config directories unless `--config-dir` is specified explicitly.

**Recommended:** Add a `--config-dir` override to the CLI (check if it already exists in `src/cli.rs`) so both modes can be pointed at the same directory.

---

## Files to Create/Modify

| File | Change |
|------|--------|
| `Cargo.toml` | Add `windows-service = "0.7"` under `[target.'cfg(windows)'.dependencies]` |
| `src/cli.rs` | Add `--service`, `--install-service`, `--uninstall-service` flags (all `#[cfg(windows)]`) |
| `src/service.rs` | New file — Windows Service entry point and SCM handler |
| `src/main.rs` | Add service dispatch; extract `run_headless()` function |

---

## Testing

Since CI cross-compiles from Linux, service functionality cannot be tested in CI. Manual testing required on a Windows machine:

1. Build: `cargo build --release --target x86_64-pc-windows-gnu`
2. Copy to Windows machine
3. Open PowerShell as Administrator
4. Run: `.\llama-monitor.exe --install-service --port 7778`
5. Verify service appears in `services.msc`
6. Start: `Start-Service LlamaMonitor`
7. Verify web UI is accessible at `http://localhost:7778`
8. Stop: `Stop-Service LlamaMonitor`
9. Verify graceful shutdown (WAL checkpoint in logs)

---

## Risks

1. **Config dir divergence**: Service (SYSTEM account) vs tray app (user account) use different `%APPDATA%` paths. Mitigate with `--config-dir`.
2. **`main.rs` refactor scope**: Extracting `run_headless()` touches a large function. Run the full test suite after.
3. **`windows-service` version compatibility**: Verify the crate works with `x86_64-pc-windows-gnu` (MinGW) — some Windows crates only support MSVC.
4. **Sensor bridge in service context**: `sensor_bridge.exe` is currently expected next to the binary. In service mode, the binary lives in a system path; sensor_bridge must be co-located or the path must be configured.

---

## Non-Goals

- Running the tray UI from a service (not possible without a user session)
- Auto-start tray app at login (use Windows Startup folder or Task Scheduler for that, not SCM)
- Linux systemd service (already documented separately / use `--headless`)
