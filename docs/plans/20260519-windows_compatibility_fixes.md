# Windows Compatibility Fixes

**Date:** 2026-05-19
**Status:** Active
**Priority:** High
**Branch:** `feature/windows-tray-webview-and-compat`

## Context

Development has been Mac-focused for several months. This document catalogs all Windows-specific gaps discovered during a comprehensive codebase audit on 2026-05-19. The primary goal of this branch is to implement the Windows tray WebView popover (see `20260505-windows_tray_webview.md`), but several additional Windows compatibility issues were found and are tracked here for systematic resolution across potentially multiple context compaction sessions.

---

## Issues Catalog

### Issue W-01: `wry` excluded from Windows (BLOCKS tray WebView)

**File:** `Cargo.toml:56-57`
**Status:** In progress — covered by `20260505-windows_tray_webview.md` Phase 1

```toml
# Current — Windows excluded:
[target.'cfg(not(target_os = "windows"))'.dependencies]
    wry = { version = "0.55", default-features = false, features = ["os-webview"], optional = true }

# Fix — move to universal:
[dependencies]
    wry = { version = "0.55", default-features = false, features = ["os-webview"], optional = true }
```

The `os-webview` feature enables WebView2 on Windows automatically. This is the root blocker for the Windows tray WebView.

**Severity:** CRITICAL — blocks WebView popover on Windows

---

### Issue W-02: `tray.rs` platform guards exclude Windows from WebView popover path

**File:** `src/tray.rs` — ~20 `#[cfg]` guard updates needed
**Status:** In progress — covered by `20260505-windows_tray_webview.md` Phases 2–9

The main changes needed (see the detailed plan for exact lines):
- Remove `not(target_os = "windows")` from all `webview-popover` feature guards
- Remove Windows-specific context menu code (`WindowsTrayMenu`, `handle_windows_menu`, `format_rate`)
- Extend `open_popover` / `close_popover` / `resize_popover` to include Windows
- Update `run_tray` channel creation and `TrayApp` init

**Severity:** CRITICAL — Windows users see static text menu instead of live metrics popover

---

### Issue W-03: `winit` dependency missing explicit Windows features

**File:** `Cargo.toml:51`
**Status:** Low risk — verify at build time

```toml
winit = { version = "0.31.0-beta.2", default-features = false, features = ["x11", "wayland"], optional = true }
```

The `x11` and `wayland` features are Linux-specific. On Windows, `winit`'s Win32 backend is compiled automatically when the build target is Windows, even with `default-features = false`. This is by design in winit — the platform backend is selected at compile time based on target, not a feature flag.

**Action:** Run `cargo check --target x86_64-pc-windows-msvc` to confirm compilation succeeds before declaring this resolved. If winit throws a `no backend selected` error, add an explicit `windows` feature when one becomes available in the beta. Currently no such feature exists in 0.31.0-beta.2.

**Severity:** LOW — likely compiles fine, but must be verified

---

### Issue W-04: `harden_file_permissions` is a no-op on Windows

**File:** `src/config.rs:16-32`
**Status:** Open

```rust
pub(crate) fn harden_file_permissions(path: &std::path::Path) {
    if !path.exists() { return; }
    #[cfg(unix)]
    { /* sets 0o600 */ }
    #[cfg(not(unix))]
    { let _ = path; }  // ← complete no-op on Windows
}
```

Called for these sensitive files in `main.rs:71-78`:
- `encryption-key`
- `api-token`
- `db-admin-token`
- `tls-config.json`
- `auth-config.json`
- `ui-settings-file`
- `sessions-file`
- `ssh-known-hosts-file`

On Windows, any local user can read these files if they have filesystem access to `%APPDATA%\llama-monitor\`.

**Fix options (in priority order):**

**Option A (recommended):** Use `icacls` via `std::process::Command` to restrict ACLs:
```rust
#[cfg(windows)]
{
    let path_str = path.to_string_lossy();
    // Remove all inherited permissions, grant only current user
    let _ = std::process::Command::new("icacls")
        .args([&*path_str, "/inheritance:r", "/grant:r", &format!("{}:F", username)])
        .output();
}
```
Problem: Getting the current username on Windows requires `GetUserNameW` or env var `USERNAME`. The `USERNAME` env var is simpler but not guaranteed in service contexts.

**Option B (safe default):** Document the limitation. Add a startup log warning on Windows that file permissions cannot be automatically hardened and users should restrict access to `%APPDATA%\llama-monitor\` manually.

**Option C (medium effort):** Add the `windows-acl` or `winapi` crate dependency to set DACLs programmatically.

**Recommended approach for this branch:** Option B (log the warning) as a low-risk immediate fix. Option A or C as a follow-up in a dedicated security hardening PR.

**Severity:** MEDIUM — security gap on multi-user Windows systems; low risk for typical single-user local deployments

---

### Issue W-05: Signal handling on Windows — `ctrl_c` only

**File:** `src/main.rs:781-787`
**Status:** Acceptable as-is for now

```rust
#[cfg(windows)]
{
    tokio::signal::ctrl_c().await.ok();
}
```

On Windows, `tokio::signal::ctrl_c()` handles:
- Ctrl+C in a console window
- `GenerateConsoleCtrlEvent` from parent processes

It does NOT handle:
- Windows Service `SERVICE_CONTROL_STOP` (service stop commands)
- Task Scheduler termination events

For a desktop tray app (the primary Windows deployment scenario), `ctrl_c` is sufficient. If llama-monitor is ever packaged as a Windows Service, this needs the `windows-service` crate.

**Action:** No change needed for tray app deployment. Document this limitation if Windows Service packaging is added later.

**Severity:** LOW for tray app; HIGH if packaged as Windows Service

---

### Issue W-06: `open_dashboard` uses `cmd.exe /C start`

**File:** `src/tray.rs:667-675`
**Status:** Will be deleted as part of W-02 (Phase 4 of tray WebView plan)

```rust
#[cfg(target_os = "windows")]
fn open_dashboard(port: u16) {
    let url = format!("http://127.0.0.1:{port}");
    if let Err(e) = std::process::Command::new("cmd.exe")
        .args(["/C", "start", "", &url])
        .spawn()
    { ... }
}
```

This code will be deleted when the Windows context menu is removed. If a "Open in Browser" action is needed in the future (e.g., a right-click context menu), this pattern is correct for Windows — `start ""` with a URL opens the default browser.

**Severity:** LOW — correctly implemented; will be removed as planned

---

### Issue W-07: GPU metrics — no Windows-native GPU detection

**File:** `src/gpu/mod.rs`
**Status:** Open — post-tray-webview work

The GPU backend auto-detection uses external CLI tools:
- `nvidia-smi` — works on Windows if NVIDIA drivers installed (NVIDIA adds it to PATH)
- `rocm-smi` — works on Windows if ROCm Windows installed
- `mactop` — macOS only (correctly excluded)

Missing on Windows:
- **Intel GPU / Arc:** No detection path. Intel's GPU tools (`igd-smi` or DirectX queries) aren't implemented.
- **AMD via DirectX:** ROCm is not universally installed on Windows; DirectX/DXGI could provide VRAM and utilization data natively.
- **NVIDIA via NVML:** The `nvml-wrapper` crate provides native NVIDIA monitoring without requiring `nvidia-smi` in PATH.

**Near-term fix:** Document that GPU monitoring on Windows requires `nvidia-smi` or `rocm-smi` in PATH. Add this to the Windows setup docs.

**Long-term:** Consider a `wmi`-based GPU discovery path since `wmi` is already a Windows dependency. `Win32_VideoController` provides GPU name and VRAM; DXGI can provide utilization.

**Severity:** MEDIUM — GPU metrics work for most NVIDIA Windows users; Intel GPU users get no data

---

### Issue W-08: `ssh2` with `vendored-openssl` on Windows MSVC

**File:** `Cargo.toml:39`
**Status:** Monitor — test at build time

```toml
ssh2 = { version = "0.9", features = ["vendored-openssl"] }
```

`vendored-openssl` compiles OpenSSL from source, which requires:
- A C compiler (MSVC or MinGW)
- Perl (for OpenSSL's `Configure` script)
- NASM (for assembler optimizations)

On Windows with the MSVC toolchain, OpenSSL compilation can fail if Perl is not in PATH. The cross-compilation targets in `.github/` likely use `x86_64-pc-windows-gnu` (MinGW) which is more forgiving.

**Status:** Non-issue. CI uses `x86_64-pc-windows-gnu` (MinGW cross-compile from Linux), confirmed in `.github/workflows/release.yml`. MinGW's bundled toolchain handles the OpenSSL build without requiring a separate Perl install.

**Severity:** LOW — resolved by CI target choice

---

### Issue W-09: Sensor bridge architecture — documentation gaps

**File:** `docs/reference/windows-sensor-bridge-implementation.md`
**Status:** Mostly documented — minor gaps

The sensor bridge doc exists and is comprehensive. Gaps:
1. Doesn't mention that `sensor_bridge.exe` must be placed next to `llama-monitor.exe` in the distribution
2. Doesn't document the HTTP server mode (the current `lhm.rs` polls `http://127.0.0.1:7780/` — not the stdout-based approach the doc describes)
3. The actual `sensor_bridge/Program.cs` in the repo runs as an HTTP server (not stdout), but the doc shows stdout-based code — **these are inconsistent**

**Action:** Reconcile the doc with the actual `sensor_bridge/Program.cs` implementation. The HTTP server mode in the actual code is more robust (no process-spawn overhead per poll).

**Severity:** LOW — documentation inconsistency, not a runtime bug

---

## Implementation Order for This Branch

Given the scope, work is prioritized as follows:

### Phase A: Tray WebView (primary goal, this session) ✅ COMPLETE

Follow `docs/plans/20260505-windows_tray_webview.md` phases 1–9 in order:
1. ✅ Cargo.toml: move `wry` to universal dep (W-01)
2. ✅ tray.rs: remove `not(target_os = "windows")` guards (W-02)
3. ✅ tray.rs: delete `WindowsTrayMenu` and related code (~110 lines removed)
4. ✅ tray.rs: extend popover functions to Windows via `not(target_os = "linux")` guards
5. ✅ Verify cross-compile: `cargo check --target x86_64-pc-windows-gnu` — passed

### Phase B: Windows permission warning (W-04, low effort) ✅ COMPLETE

✅ Added startup `eprintln!` on Windows in main.rs warning about file permission hardening gap.

### Phase C: Winit features verification (W-03) ✅ COMPLETE

✅ Confirmed: winit-win32 backend auto-selected when targeting Windows, even with
`default-features = false`. Cross-compile check passed with winit-win32 v0.31.0-beta.2.

### Phase D: Documentation updates ✅ COMPLETE

- ✅ Rewrote `docs/reference/windows-sensor-bridge-implementation.md` to match the
  actual HTTP server implementation (sensor_bridge is a persistent server on :7780,
  not a stdout-based one-shot process as the old doc described)
- ✅ Added Windows GPU source table (nvidia-smi, rocm-smi, WMI fallback) to the doc

### Phase E: Remaining implementation ✅ COMPLETE

- ✅ W-04: Windows ACL hardening — implemented in `config.rs` via `icacls /inheritance:r
  /grant:r <user>:(F)`; handles domain vs local accounts; logs warning on failure
- ✅ W-07: Intel/unknown GPU via WMI — new `src/gpu/wmi_gpu.rs` using
  `Win32_VideoController`; surfaces GPU name and VRAM; auto-selected on Windows
  when nvidia-smi/rocm-smi not found; reports name="wmi"
- W-05: Windows Service packaging — **dedicated plan written** at
  `docs/plans/20260519-windows_service_packaging.md`; deferred as a separate feature
  (different deployment model, not a compatibility gap in the tray app)

---

## Cross-Compile Verification Command

```bash
# CI uses MinGW GNU target — use the same locally:
rustup target add x86_64-pc-windows-gnu

# Check compile (no linker needed for check):
cargo check --target x86_64-pc-windows-gnu
```

Run this after every tray.rs change to catch cfg guard issues early.

---

## Files Modified in This Branch

| File | Changes | Issue |
|------|---------|-------|
| `Cargo.toml` | Move `wry` to universal dep | W-01 |
| `src/tray.rs` | Remove Windows exclusions, delete context menu code, extend popover | W-02 |
| `src/main.rs` | Add Windows file permission warning at startup | W-04 |
| `docs/reference/windows-sensor-bridge-implementation.md` | Reconcile with HTTP server implementation | W-09 |
| `AGENTS.md` | Add multi-platform compatibility mandate | — |
