# Local LLM Foundry 2.0 — Native Windows Validation Handoff

Status: native Windows Rust/JS/UI validation completed 2026-08-12; Windows core/config captures pass, while Rapid-MLX wizard/preset captures remain platform-gated; remaining markers are application-home, tray/WebView2, packaging, screenshot parity, and fresh CI evidence

The Windows checkout now has the portability and test-fixture fixes required for
the native MSVC build. Commit `3069a1b` is clean and validated with Rust 1.97.1:
native clippy, the full Rust suite, release build, GNU cross-target check and
clippy, JavaScript validation, lint, rebrand validation, and release-contract
validation all pass. After allowing the pinned Playwright Chromium executable
through Portmaster, the complete release-built native UI suite passed (270
passed, 5 intentional skips, 0 failures). WSL2 release preflight remains a
separate environment marker, and the disposable application-home, tray/
WebView2, sensor bridge, updater, remote-agent, and package checks still need
their explicit receipts before the Windows phase is fully closed.

This document is the operational handoff for a fresh Codex session running on
the Windows development machine. The authoritative implementation and release
decisions remain in
[`20260811-local_llm_foundry-rebrand.md`](20260811-local_llm_foundry-rebrand.md).
This handoff records only the Windows-specific setup, evidence contract, and
return markers needed to close Phases 11–14.

## Screenshot parity scope (2026-08-12)

Screenshot captures have two different evidence classes and must not be mixed:

- **Cross-platform UI evidence:** configuration and core captures run on
  Windows at the shared `1440x900` viewport. These exercise the release-built
  frontend and are valid for typography, wrapping, spacing, overflow, theme,
  and accessibility comparison against macOS.
- **Rapid-MLX product evidence:** local Rapid-MLX execution, live telemetry,
  runtime-manager behavior, and platform availability are Apple-Silicon/macOS
  only. The `rapid-mlx-live` scenario correctly skips on Windows when
  `/api/llama-binary/platform-info` reports Rapid-MLX unavailable.

The current Rapid wizard/preset scenarios also consult that real platform-info
gate. A Windows `wizard-rapidmlx` probe therefore produced a llama.cpp fallback
or timed out before opening the Rapid wizard, and the Windows `presets` group
did not render the seeded Rapid card. Those are **not** Windows Rapid visual
receipts and must not be compared as if they were. Deterministic DOM-only
scenarios such as the synthetic Rapid dashboard cards may be captured on
Windows when they do not cross the platform gate; label them as UI fixture
evidence, not runtime evidence.

The capture harness now marks Rapid-dependent scenarios explicitly and skips
them before creating a temporary application home, starting the server, or
launching Chromium unless the host is `darwin/arm64`. The skip is printed as a
visible `SKIP` line and exits successfully, so a Windows capture group remains
useful for the non-Rapid scenarios without spending minutes on impossible
browser timeouts. The Playwright suite deliberately keeps its mocked Rapid UI
tests cross-platform; only executable/runtime evidence is platform-gated.

For the final parity pass, capture the complete Rapid-MLX group on macOS and
compare only the shared, non-runtime surfaces across Windows and macOS. Do not
add a test-only platform override merely to manufacture Windows Rapid runtime
receipts; if cross-platform Rapid UI coverage becomes a release requirement,
introduce an explicit fixture-mode contract and keep it separate from live
runtime scenarios.

## Starting context

- Repository: `nmorgowicz-org/local-llm-foundry`
- Local checkout branch: `feat/rapid-mlx-integration`
- Current validated commit: `3069a1b`; verify with `git log -1` before testing.
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

The latest completed PR run is `31627911658`. CodeQL, lint, and Windows-target
clippy passed, but the Rust `check` job failed before UI qualification. One
Rapid-MLX compatibility probe hit a transient Linux `ETXTBSY` (“Text file
busy”) spawn error:

1. `inference::rapid_mlx::compatibility::tests::live_probe_qualifies_healthy_external_profiles_without_global_warning`
   — temporary fixture executable returned `ETXTBSY` at spawn.

The source fix adds a bounded retry for `ETXTBSY`; the exact test passed five
times locally, and the full Rust suite passed 1,238 tests. The earlier UI fix
also passed 15/15 focused release-built repeats and a complete release-built
suite (270 passed, 5 skipped). These are local receipts, not a substitute for
a fresh GitHub run. Do not remove or restore `ready-to-test` based only on
native Windows results.

## Windows prerequisites

Install or verify:

- Git for Windows and a PowerShell 7 session.
- Rust stable 1.97.1, Cargo, and the `x86_64-pc-windows-gnu` target required by
  the release workflow. The native binary was built with MSVC; the GNU target
  is retained for cross-target parity.
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
  green on the current commit (270 passed, 5 intentional skips). The remaining
  Windows closure markers are the native runtime/package matrix and a fresh CI
  run on the current commit.
- A fresh CI run after the fix is green; the transient failure from run
  `31627911658` is not merely hidden by retries.
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
