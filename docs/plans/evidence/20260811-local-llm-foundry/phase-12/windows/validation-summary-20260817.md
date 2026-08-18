# Native Windows Validation Receipt — 2026-08-17

- **Branch:** `feat/rapid-mlx-integration`
- **Commit:** `e11af59`
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
- Tray/WebView2 and sensor-bridge runtime receipts.
- Updater replacement/checksum/archive extraction.
- Remote-agent install/update/uninstall and mixed-version task cleanup.
- Native ZIP/package/icon manifest receipts.
- Fresh remote CI on the final branch and screenshot-parity acceptance.
