# Native Windows Validation Receipt — 2026-08-17

- **Branch:** `feat/rapid-mlx-integration`
- **Commit:** `a97a8cc`
- **Host:** native Windows workstation `ryne`, x86_64
- **Checkout:** `D:\scripts\claude\local-llm-foundry`

## Passed on the synchronized build

- `cargo build --release` completed successfully.
- Release outputs contain both `target/release/local-llm-foundry.exe` and
  `target/release/llama-monitor.exe`.
- Canonical and legacy executable smoke checks completed; the legacy entrypoint
  emitted the expected compatibility notice and the canonical help surface was
  available.
- Fresh disposable headless startup remained alive for the probe interval and
  created only the canonical `local-llm-foundry` application root, including
  auth tokens, database/WAL files, certificates, settings, and model metadata.
- Legacy-only disposable startup remained alive for the probe interval,
  preserved the legacy sentinel, and did not create a canonical root.
- Disposable migration API qualification passed status, preview, authenticated
  queueing, copy-first restart execution, receipt creation, request cleanup,
  and legacy-root retention using `--migration-test-root`.
- Disposable rollback qualification passed rollback preview, authenticated
  rollback queueing, restart cleanup of the canonical root, legacy-root
  retention, and sentinel preservation.
- Native Windows release preflight passed with the installed .NET 10 SDK;
  the sensor bridge built successfully with two existing compiler warnings.
- Temporary canonical and legacy ZIP inspection passed: each archive contains
  its expected executable, `sensor_bridge.exe`, and x64 `WebView2Loader.dll`.
- Release-built Playwright UI validation passed with 281 tests and 5
  intentional/platform skips; the Rapid-MLX scenarios remain correctly
  Apple-Silicon/macOS gated and the SSH integration remains environment-gated.
- Tray/WebView2 source review found and corrected the compact popover's stale
  `llama-monitor` heading; the visible tray menu and runtime guidance already
  use `Local LLM Foundry`.
- Manual tray/WebView2 acceptance passed on the final Windows build: tray
  open/close, branded popover, native header dragging without text selection or
  mouse reattachment, dashboard/logs menu actions, and full app quit were
  verified. Resizing is intentionally disabled pending a future native
  borderless-window implementation.

All disposable roots were created below the Windows temporary directory and
were not connected to the user's real profile or model inventory.

## Environment limitation

`scripts/release-preflight.sh` is a Linux cross-toolchain preflight. The Windows
machine's `bash` launcher resolves to WSL2, and WSL2 is unavailable because the
Virtual Machine Platform feature is disabled. This does not invalidate the
native Windows build or application probes; the cross-platform release preflight
remains a separate CI/Linux gate.

## Still open

- Interrupted migration resume journal matrix.
- Sensor-bridge runtime receipt.
- Updater replacement/checksum/archive extraction.
- Remote-agent install/update/uninstall and mixed-version task cleanup.
- Native ZIP/package/icon manifest receipts beyond the temporary archive
  inspection.
- Fresh remote CI on the final branch and screenshot-parity acceptance.
