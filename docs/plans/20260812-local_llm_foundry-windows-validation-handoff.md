# Local LLM Foundry 2.0 — Native Windows Validation Handoff

Status: prepared 2026-08-12; native execution is not yet complete

This document is the operational handoff for a fresh Codex session running on
the Windows development machine. The authoritative implementation and release
decisions remain in
[`20260811-local_llm_foundry-rebrand.md`](20260811-local_llm_foundry-rebrand.md).
This handoff records only the Windows-specific setup, evidence contract, and
return markers needed to close Phases 11–14.

## Starting context

- Repository: `nmorgowicz-org/local-llm-foundry`
- Local checkout branch: `feat/rapid-mlx-integration`
- Current validated commit: checkout the pushed branch and record its exact SHA with `git log -1`; it includes the UI qualification fixes described below.
- Pull request: #314
- Product: Local LLM Foundry 2.0
- Canonical Windows application home: `%APPDATA%\\local-llm-foundry\\`
- Legacy Windows home: `%APPDATA%\\llama-monitor\\`
- Canonical executable: `local-llm-foundry.exe`
- Supported 2.0.x legacy executable alias: `llama-monitor.exe`
- Do not delete or silently move the legacy root or external model trees.

Before starting, confirm the checkout is clean and at the intended commit. Do
not reset or discard unrelated user changes. Read `AGENTS.md`,
`docs/agents/platform-details.md`, and
`docs/reference/windows-support.md` before running tests.

## Current CI state that must not be misreported as green

The latest completed PR run is `31615465442`. Windows-target clippy, Windows
GNU release smoke, Linux/macOS release smoke, lint, and CodeQL passed. Its UI
job failed with 268 passed, 5 skipped, and 2 failed:

1. `core/rapid-preset-visibility.spec.js:99` — Rapid-MLX control reachability
   timed out while repeatedly navigating sections/opening details.
2. `core/kv-usecase.spec.js:7` — the test selected a use-case before async
   bootstrap had bound its card listeners, leaving the default `q8_0` value.

The downloaded report is on the development Mac at
`/tmp/local-llm-foundry-ci-31615465442/playwright-report/`. If it is needed on
Windows, copy it as an investigation artifact; it is not source evidence.
The source fix adds an explicit `modules-ready` barrier to the affected tests
and visits each Rapid editor section once instead of repeating animated nav
clicks for every control. Focused release-built repeats passed 15/15 (five
repetitions of both affected specs), and the complete release-built suite passed
270 tests with 5 skipped. These are local receipts, not a substitute for a
fresh GitHub run. Do not remove or restore `ready-to-test` based only on native
Windows results.

## Windows prerequisites

Install or verify:

- Git for Windows and a PowerShell 7 session.
- Rust stable, Cargo, and the `x86_64-pc-windows-gnu` target required by the
  release workflow. If the native toolchain is MSVC, retain the GNU target for
  parity with CI and document which toolchain produced each binary.
- Node.js/npm and the repository UI dependencies.
- A Chromium installation usable by Playwright.
- WebView2 Runtime for tray/popover validation.
- Optional hardware tools only when testing those paths: `nvidia-smi`,
  `rocm-smi`, LibreHardwareMonitor/PawnIO, and `winget`.

Record versions and paths in the machine manifest before testing:

```powershell
Get-ComputerInfo | Select-Object WindowsProductName, WindowsVersion, OsBuildNumber
git --version
rustc -Vv
cargo -V
rustup target list --installed
node --version
npm --version
npx playwright --version
Get-Command llama-server.exe -ErrorAction SilentlyContinue
Get-Command nvidia-smi -ErrorAction SilentlyContinue
Get-Command rocm-smi -ErrorAction SilentlyContinue
```

## Build and static validation

From the repository root, run the mandatory project checks in this order. Use
`rtk` when it is installed; otherwise run the same command without the `rtk`
prefix and record that fact.

```powershell
rustup target add x86_64-pc-windows-gnu
cargo check --target x86_64-pc-windows-gnu
cargo clippy --target x86_64-pc-windows-gnu -- -D warnings
cargo test
npm run validate-js
npm run lint
git diff --check
cargo build --release
cargo fmt -- --check
```

The cross-target check is necessary but not native execution evidence. Preserve
the raw stdout/stderr and exit code for every command. If a command modifies
generated files, stop, inspect the diff, and commit only when the current phase
explicitly owns those files.

## Disposable native application-home matrix

Use a disposable `%APPDATA%` root for every scenario. Never point a migration
test at the real user profile. A convenient PowerShell setup is:

```powershell
$runRoot = Join-Path $env:TEMP ('foundry-windows-' + [guid]::NewGuid())
$env:APPDATA = Join-Path $runRoot 'AppData'
New-Item -ItemType Directory -Force $env:APPDATA | Out-Null
$binary = (Resolve-Path '.\\target\\release\\local-llm-foundry.exe').Path
```

For explicit-root tests, prefer the CLI override so the target is unambiguous:

```powershell
& $binary --headless --config-dir (Join-Path $env:APPDATA 'local-llm-foundry') --port 17778
```

Capture the resolved paths from startup logs and the relevant API response. Do
not infer success from a process that merely remains running.

Required scenarios:

1. Fresh install: canonical root is created, tokens/keys are created only in
   the canonical root, and no legacy root is invented.
2. Legacy-only root: old root remains effective until an authenticated preview
   and explicit execute confirmation; no best-effort move occurs at startup.
3. Both roots identical: preview reports the deterministic classification and
   execute is copy-first, journaled, verified, and restart-safe.
4. Both roots conflicting, partial, empty, and permission-denied: each returns
   the documented classification/error without deleting data or creating
   unrelated destination state.
5. Interrupted migration: stop after each journal checkpoint, restart, verify
   resume/rollback, and retain the receipt.
6. `--clear-auth-config`: verify only the documented auth material is cleared
   and the encryption key/database/model data remain intact.

For each case, retain a before/after tree manifest, hashes for files involved,
raw logs, API request/response receipts with secrets redacted, exit codes, and
the final classification.

## Windows-specific runtime checks

Validate the following on the native machine:

- `%APPDATA%\\local-llm-foundry\\logs`, certificates, encryption key, database,
  updater staging, and managed binaries use the canonical root.
- The legacy root remains readable and effective during the compatibility
  window; explicit migration does not silently delete it.
- `local-llm-foundry.exe --version`, `--help`, `--headless`, and `--agent`
  produce expected terminal output from PowerShell/cmd. Explorer launch is a
  GUI-subsystem path and must log diagnostics rather than depend on stdout.
- `llama-monitor.exe` remains a functional 2.0.x compatibility alias where the
  release package supplies it.
- `llama-server.exe`, `taskkill`, `where`, `nvidia-smi`, `rocm-smi`, `icacls`,
  `schtasks`, and `winget` helpers do not open unwanted console windows.
- WebView2 tray/popover opens, closes, resizes, and receives IPC messages.
  Missing WebView2 must surface actionable guidance; do not treat an automatic
  installer attempt as proof of success.
- Sensor Bridge status, PawnIO missing state, scheduled-task registration,
  reboot persistence, and UAC prompts behave as documented.
- GPU metrics report the correct source/degraded state. Intel limitations must
  remain explicit rather than showing fabricated values.
- Self-update stages a detached helper, waits for the old process to exit,
  replaces the executable, and restarts it. Verify both canonical and legacy
  asset selection/checksum paths.
- Remote-agent install/update/uninstall uses canonical paths while accepting
  supported legacy controller/agent/task names.

## Packaging and release checks

Run the release preflight and inspect the generated archives without publishing:

```powershell
bash scripts/release-preflight.sh
npm run validate-rebrand
npm run validate-release-contract
```

If Bash is unavailable, run these from Git Bash or WSL and record the
environment. Native Windows archive inspection must still be performed in
PowerShell:

```powershell
Get-ChildItem .\\dist -Recurse
Get-FileHash .\\dist\\*.zip -Algorithm SHA256
Expand-Archive .\\dist\\local-llm-foundry-windows-x86_64.zip .\\artifacts\\canonical
Expand-Archive .\\dist\\llama-monitor-windows-x86_64.zip .\\artifacts\\legacy
```

Verify the canonical ZIP contains `local-llm-foundry.exe`,
`sensor_bridge.exe`, and `WebView2Loader.dll`. Verify the legacy ZIP contains
`llama-monitor.exe`, `sensor_bridge.exe`, and `WebView2Loader.dll`. Confirm the
checksums manifest has exact entries for all four canonical and four legacy
2.0.x filenames. Do not test 2.1 canonical-only behavior by deleting aliases
from a 2.0 artifact; use the dedicated 2.1 fixture.

## Evidence package and completion gates

Write evidence under a new machine-local or committed Phase 12 receipt
directory, never over an older receipt:

```text
docs/plans/evidence/20260811-local-llm-foundry/phase-12/windows/
  machine-manifest.txt
  command-results/
  path-matrix.tsv
  migration-receipts/
  runtime-checks.tsv
  package-manifest.tsv
  screenshots/
  SHA256SUMS
```

Every receipt must include date/time, Windows version, architecture, toolchain,
commit SHA, command, exit code, raw output path, and a pass/fail result. Redact
tokens, private paths, hostnames, and user identifiers before committing.

Native Windows work can close its markers only when all of these are true:

- The full disposable application-home matrix passes without data loss.
- Native executable, tray/WebView2, sensor bridge, updater, remote-agent, and
  package checks have raw receipts.
- The mandatory Rust/JS checks and the complete release-built UI suite are
  green on the current commit.
- A fresh CI run after the fix is green; the two failures from run
  `31615465442` are not merely hidden by retries.
- Phase 12 evidence is complete and the plan status table is updated before
  Phase 13 screenshot capture or public 2.0 release work begins.

## Stop conditions

Stop and report instead of improvising if WebView2/driver installation needs
credentials, signing keys are unavailable, the repository checkout is dirty,
the migration classification is ambiguous, a checksum/archive differs from
the release contract, a legacy client cannot consume the bridge asset, or a
native failure cannot be reproduced with a disposable root. Do not delete
legacy data, alter the public release, or waive a failing gate.

## Authoritative references

- [`20260811-local_llm_foundry-rebrand.md`](20260811-local_llm_foundry-rebrand.md)
- [`../../agents/platform-details.md`](../../agents/platform-details.md)
- [`../../reference/windows-support.md`](../../reference/windows-support.md)
- [`../../reference/cross-compilation.md`](../../reference/cross-compilation.md)
- [`20260622-windows_build_toolchain.md`](20260622-windows_build_toolchain.md)
- [`evidence/20260811-local-llm-foundry/phase-11/README.md`](evidence/20260811-local-llm-foundry/phase-11/README.md)
