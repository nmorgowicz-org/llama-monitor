# Multi-Platform Compatibility (Full Reference)

Targets: macOS, Linux, Windows. All three are first-class.

## Platform Rules (Summary)

1. Never add platform-specific code without a Windows equivalent, or an explicit `#[cfg(target_os = "windows")]` stub.
2. Run cross-compile check before every PR:
   ```bash
   rustup target add x86_64-pc-windows-gnu
   cargo check --target x86_64-pc-windows-gnu
   ```
3. Removing `not(target_os = "windows")` guards is a common task. When extending a feature, audit these in affected files.
4. New `#[cfg]` guards must be justified with inline comment.
5. Test tooltip and fallback paths on all platforms; log a warning, never swallow errors on Windows.
6. `winit`: uses `default-features = false` with `x11` and `wayland`. Win32 backend selected automatically on Windows; no platform-specific feature flags.
7. `wry` is universal — must not be re-scoped to `not(target_os = "windows")`.
8. File permission hardening: `harden_file_permissions()` is a no-op on Windows (known gap, W-04).
9. GPU metrics on Windows: `nvidia-smi`/`rocm-smi` in PATH; Intel GPU not yet implemented.

## Windows-Specific Architecture

| Feature | Windows Implementation | Status |
|---------|----------------------|--------|
| Tray popover | WebView2 via `wry` | In progress |
| CPU temperature | `sensor_bridge.exe` HTTP sidecar | Working (elevated privileges required) |
| GPU metrics | `nvidia-smi` / `rocm-smi` in PATH | Working (NVIDIA/AMD) |
| File permissions | No-op | Known gap (W-04) |
| Signal handling | `tokio::signal::ctrl_c()` | Sufficient |
| Dashboard open | `cmd.exe /C start <url>` | Working |

## Related Documents
- `docs/plans/20260505-windows_tray_webview.md`
- `docs/plans/20260519-windows_compatibility_fixes.md`
- `docs/reference/windows-sensor-bridge-implementation.md`
