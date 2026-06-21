# Windows Support Consolidation

**Date:** 2026-06-21
**Status:** Planned
**Priority:** High (P0 items are visible-quality / functional gaps)
**Supersedes:** `20260505-windows_tray_webview.md`, `20260519-windows_compatibility_fixes.md`,
`20260519-windows_service_packaging.md` (all three deleted; their completed work and
remaining open items are folded into this document).

---

## 0. How to read this document

This is a **fresh-start brief**. It assumes no prior context. It was written after a full
re-audit of the codebase on 2026-06-21, *after* the large `refactor/api-modules-v2` API
overhaul that split `src/web/api/` into per-domain modules. The three previous Windows plans
were written months earlier and referenced line numbers and an API layout that no longer
exist; do not trust their specifics. Everything below was re-verified against the current tree.

Work is grouped by priority. **P0** items are the difference between "compiles and runs on
Windows" (true today) and "feels like a real Windows app" (not true today). Do P0 first and in
order — items 1 and 2 are coupled and must land together.

Each item states: the problem, the exact files, the fix (with code), and how to verify.

---

## 1. Status: what is already DONE (do not redo)

The previous branch (`feature/windows-tray-webview-and-compat`) shipped a large amount of
Windows work that is **confirmed present and correct** in the current tree. Do not re-plan these:

| Area | Where | State |
|---|---|---|
| `wry` as a universal dependency (WebView2 on Windows) | `Cargo.toml:56` | Done — no `cfg(not(windows))` guard |
| Unified tray WebView popover (no more static context menu) | `src/tray.rs` | Done — popover path uses `cfg(not(target_os = "linux"))` |
| `winit` Win32 backend auto-selected | `Cargo.toml:55` | Done — verified compiles for `x86_64-pc-windows-gnu` |
| File-permission hardening via `icacls` | `src/config.rs:30-73` | Done — `/inheritance:r /grant:r <user>:(F)`, domain/local aware |
| WMI GPU discovery (Intel/AMD/unknown) | `src/gpu/wmi_gpu.rs`, `src/gpu/mod.rs:59-84` | Done — `Win32_VideoController`, name + VRAM, auto-fallback |
| Sensor bridge HTTP server (CPU temp) | `sensor_bridge/Program.cs`, `src/lhm.rs` | Done — persistent `HttpListener` on `:7780` |
| Sensor bridge install/uninstall (SYSTEM scheduled task, UAC) | `src/lhm.rs:157-238` | Done |
| Sensor bridge HTTP API | `src/web/api/sensor_bridge.rs` | Done — `/api/sensor-bridge/{status,install,uninstall}` |
| Self-update of `llama-monitor.exe` (in-place via detached `.bat`) | `src/agent.rs` (`install` mod, ~3850) | Done — `DETACHED_PROCESS`, waits for PID exit then `copy`/`start` |
| llama-server binary update on Windows | `src/web/api/llama_binary.rs:719` | Done — `.exe` name, rename-based install |
| Remote agent install/manage over SSH to Windows | `src/agent.rs` (extensive) | Done — `%APPDATA%` resolution, schtasks, cmd.exe quoting |
| `ssh2` vendored-openssl on MinGW | `Cargo.toml:39` | Non-issue — CI uses `x86_64-pc-windows-gnu` |

CI builds and ships a Windows bundle today: `.github/workflows/release.yml` produces
`llama-monitor-windows-x86_64.zip` containing `llama-monitor.exe` + `sensor_bridge.exe`.

---

## 2. P0 — Visible-quality and functional gaps (do these first)

### P0-1 + P0-2 are coupled — read both before starting

Today the app links as a **console subsystem** binary and several subprocesses are spawned
**without** `CREATE_NO_WINDOW`. Right now that's "invisible" because the parent console absorbs
the child output. The moment you hide the main console (P0-1), every un-flagged subprocess
spawn will **flash a black console window** — and the GPU pollers spawn `nvidia-smi`/`rocm-smi`
every ~500 ms, so the screen would strobe. **You must land P0-1 and P0-2 in the same change.**

---

### P0-1: The app opens a console window on launch

**Problem.** `src/main.rs` has no `windows_subsystem` attribute, so on Windows the tray app
launches with an attached console window — unacceptable for a GUI tray app. But the app also
relies on `println!`/`eprintln!` for logging, and `--headless`/`--agent` modes are legitimately
console/CLI use cases where we *want* stdout.

`windows_subsystem` is a **compile-time** attribute, so we cannot switch it per-CLI-flag at
runtime directly. The standard idiom is: compile as the **windows (GUI) subsystem**, then at
startup *re-attach* to the parent console if one exists (i.e. the user launched from `cmd`/
PowerShell), so CLI usage still prints.

**Files:** `src/main.rs` (top of file), plus a small Windows-only helper.

**Fix.**

1. Add the subsystem attribute at the very top of `src/main.rs` (above `#![recursion_limit]`):

```rust
// GUI subsystem on Windows so the tray app does not spawn a console window.
// We re-attach to the parent console at runtime (see attach_parent_console)
// so `--headless`/`--agent`/`--version` still print when run from a terminal.
#![cfg_attr(windows, windows_subsystem = "windows")]
```

2. Add a Windows-only console re-attach helper and call it as the **first line** of `main()`:

```rust
#[cfg(windows)]
fn attach_parent_console() {
    // ATTACH_PARENT_PROCESS = (DWORD)-1
    // Safe: AttachConsole is a no-op/failure when there is no parent console
    // (e.g. launched from Explorer), which is exactly what we want for the tray app.
    unsafe {
        // Declared locally to avoid pulling in the full `windows`/`winapi` crate
        // just for one symbol. Link against kernel32 (always present).
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn AttachConsole(dwProcessId: u32) -> i32;
        }
        const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}
```

Call it first in `main()`:

```rust
fn main() -> Result<()> {
    #[cfg(windows)]
    attach_parent_console();
    let args = cli::AppArgs::parse();
    // ...
```

**Caveats to handle / verify on real hardware:**
- When launched from a console, `AttachConsole` reuses the parent's console, but the shell has
  already returned its prompt. Output interleaves with the prompt. This is the well-known,
  accepted behavior of GUI-subsystem CLIs on Windows (same as e.g. `rustup`). Acceptable.
- `--version`/`--help` (clap) print and exit before much else; confirm they still appear when
  run from PowerShell. If clap output is swallowed, the re-attach is the fix.
- Consider adding a startup log line to a file as a fallback (see "Optional: file logging"
  under §4) so diagnostics survive even with no console.

**Verify:** Build the GUI-subsystem `.exe`. Double-click from Explorer → no console window, tray
icon appears. Run `llama-monitor.exe --version` from PowerShell → version prints.

---

### P0-2: Subprocess spawns flash console windows (`CREATE_NO_WINDOW`)

**Problem.** Once the main binary is GUI-subsystem (P0-1), any `std::process::Command`
(or tokio `Command`) spawned without the `CREATE_NO_WINDOW` (`0x0800_0000`) creation flag pops a
transient console window. The hot offenders (some run on a timer):

| File | Command | Frequency |
|---|---|---|
| `src/gpu/nvidia.rs:10` | `nvidia-smi` | **every GPU poll (~500 ms)** |
| `src/gpu/rocm.rs:11` | `rocm-smi` | **every GPU poll (~500 ms)** |
| `src/gpu/mod.rs:120-132` | `where` / `which` (`command_exists`) | startup + backend detect |
| `src/gpu/mod.rs:100` | `sysctl` | macOS only — N/A on Windows |
| `src/llama/server.rs:797` | `llama-server.exe` (the model server) | per model start |
| `src/llama/server.rs:1037` | `taskkill` | per model stop |
| `src/config.rs:55` | `icacls` | **8× at every startup** (one per secret file) |
| `src/lhm.rs:59` | `schtasks /Query` | status checks |
| `src/lhm.rs:180,226` | `powershell ... Start-Process -Verb RunAs` | install/uninstall (UAC — intentional window, leave as-is) |

Note `src/agent.rs` already uses `creation_flags(DETACHED_PROCESS)` in one place — the codebase
already knows the `std::os::windows::process::CommandExt` idiom; we just need to apply
`CREATE_NO_WINDOW` consistently.

**Fix — add one shared helper, then route all hidden spawns through it.**

Create `src/platform.rs` (new module; add `mod platform;` to `src/main.rs`):

```rust
//! Cross-platform process-spawn helpers.

/// Configure a `std::process::Command` so it never flashes a console window on
/// Windows. No-op on other platforms. Apply to every short-lived helper process
/// (nvidia-smi, rocm-smi, icacls, schtasks, where, taskkill, ...).
pub fn no_window(cmd: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Same for tokio's async `Command`.
pub fn no_window_tokio(cmd: &mut tokio::process::Command) -> &mut tokio::process::Command {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
```

Then apply at each site. Examples:

```rust
// src/gpu/nvidia.rs
let output = crate::platform::no_window(&mut std::process::Command::new("nvidia-smi"))
    .args([/* ... */])
    .output();
```

```rust
// src/llama/server.rs (the model server spawn at ~410-797)
crate::platform::no_window_tokio(&mut cmd);   // before cmd.spawn()
```

```rust
// src/config.rs harden_file_permissions — wrap the icacls Command
let result = crate::platform::no_window(&mut std::process::Command::new("icacls"))
    .args([/* ... */])
    .status();
```

Apply to **every** entry in the table above **except** the two `powershell ... -Verb RunAs`
calls in `src/lhm.rs` (those intentionally surface a UAC prompt; hiding their window is wrong).

> Edge case: `tokio::process::Command` does **not** expose `creation_flags` directly on all
> versions; it re-exports it via `std::os::windows::process::CommandExt` implemented for the
> inner type. Verify `no_window_tokio` compiles for the pinned tokio (`1.52`). If it does not,
> set the flag on the std `Command` before converting, or use `CommandExt` on the tokio type
> behind `#[cfg(windows)]use std::os::windows::process::CommandExt;`.

**Verify:** On Windows, start the app (GUI subsystem), start a model, let GPU polling run for a
minute → **zero** console flashes. UAC prompt still appears for sensor-bridge install.

---

### P0-3: Ship the sensor bridge self-contained on .NET 10 (no runtime dependency on the target)

**Decision (settled):** make `sensor_bridge.exe` a **self-contained, single-file, net10.0**
build so it runs on any Windows machine with zero .NET installed on the target. This bumps the
project to .NET 10 *and* bundles the runtime.

**Background — why this matters and why it "looked fine" before.**
The bridge is currently built **framework-dependent** (`--self-contained false`,
`release.yml:56`) targeting `net8.0` (`sensor_bridge/sensor_bridge.csproj`, no
`SelfContained`/`RuntimeIdentifier`). A framework-dependent `.exe` contains only our code, not
the runtime; at launch it looks for a compatible .NET runtime **already installed on the target
Windows machine**.

- This is why a developer's own Windows box (which had a .NET runtime) showed temperature data
  fine, including via remote-agent push — the remote flow only registers a scheduled task to run
  the pushed `sensor_bridge.exe` (`src/agent.rs:1664-1672`); it never installs a runtime. It
  "worked" because that machine happened to have .NET.
- On a **clean** Windows box with no .NET, the file exists (so `is_sensor_bridge_available()`
  returns `true`) but the process fails to start → CPU temperature is silently unavailable with
  no obvious cause. Since the whole point is pushing the bridge to arbitrary machines we don't
  control, we cannot assume a runtime is present.
- **The build runner's SDK version does not change what the target needs.** The target
  requirement is set by `<TargetFramework>`, not by which SDK compiled it. Roll-forward (running
  a net8.0 app on a machine that has only a newer major) is policy-dependent and not safe to
  rely on. Self-contained removes all of this guesswork.

**Changes:**

1. **`sensor_bridge/sensor_bridge.csproj`** — bump to net10.0 and pin self-contained single-file
   so local builds, the runner, and anyone reading the project all agree:

```xml
<PropertyGroup>
  <OutputType>Exe</OutputType>
  <TargetFramework>net10.0</TargetFramework>
  <ImplicitUsings>enable</ImplicitUsings>
  <Nullable>enable</Nullable>
  <RuntimeIdentifier>win-x64</RuntimeIdentifier>
  <SelfContained>true</SelfContained>
  <PublishSingleFile>true</PublishSingleFile>
  <IncludeNativeLibrariesForSelfExtract>true</IncludeNativeLibrariesForSelfExtract>
</PropertyGroup>
```

   > **net10.0 compatibility is confirmed safe (researched 2026-06-21).**
   > `LibreHardwareMonitorLib 0.9.6` ships a `net8.0` assembly (plus `netstandard2.0` and
   > `net472`), and NuGet explicitly lists `net8.0`, `net9.0`, **`net10.0`** as compatible. A
   > net10.0 project consumes the net8.0 assembly via forward-compat — no roll-forward hack
   > needed. The "needs .NET 10" chatter online refers to *host apps* (e.g. FanControl), not the
   > library itself. So the framework bump is low-risk; still smoke-test that the published exe
   > returns readings, not just that it builds.

**⚠️ Separate, higher-risk caveat — the WinRing0 kernel driver vs. single-file + Defender.**
This is independent of the .NET version and is the thing most likely to bite us, so verify it on
real hardware before trusting the self-contained build:

- LHM reads low-level sensors via an embedded kernel driver (`WinRing0x64.sys`). At runtime the
  library **extracts that `.sys` to disk** and loads it into the kernel.
- Since ~March 2025 **Microsoft Defender flags WinRing0** (CVE-2020-14979, "vulnerable driver" /
  HackTool) and may quarantine the `.sys` — which silently kills temperature readings even though
  `sensor_bridge.exe` itself runs.
- **`PublishSingleFile` makes this worse:** the bundle (including the native `.sys`) unpacks to a
  temp self-extract directory, and the driver gets materialized there — both a common Defender
  trigger and a possible driver-load/path problem. This interaction did **not** exist in the
  current loose framework-dependent build, so going single-file could *introduce* a regression on
  Defender-strict machines.
- It may not have surfaced for the original developer's box (Defender policy/version dependent),
  so "it worked for me on .NET 8" does **not** clear this for clean/managed Windows targets.

**Mitigations to evaluate (pick during implementation, don't pre-commit):**
  1. **Test single-file first.** If Defender leaves the extracted `.sys` alone on a stock,
     up-to-date Windows 11, single-file is fine and simplest — proceed.
  2. **If Defender quarantines it:** either (a) drop `PublishSingleFile` and ship a normal
     self-contained folder (the `.sys` sits next to the exe as a plain file, often handled better
     and easier to add an AV exclusion for), or (b) migrate to **namazso's PawnIO fork** of LHM,
     which uses a separately-installed *signed* driver (PawnIO) instead of an embedded `.sys`.
     PawnIO sidesteps both the extraction and the Defender flag, but it **requires PawnIO to be
     installed on the target**, which partially undoes the "works everywhere with zero setup"
     goal — so treat it as the fallback, not the default.
  3. Document an AV-exclusion path for managed/enterprise environments regardless of choice.

This caveat belongs in `docs/reference/windows-sensor-bridge-implementation.md` too (see §5).

2. **`.github/workflows/release.yml:54-56`** — build self-contained. With the csproj pinned, the
   flags are redundant but keep them explicit:

```yaml
- name: Build sensor bridge
  if: matrix.target == 'x86_64-pc-windows-gnu'
  run: dotnet publish sensor_bridge/sensor_bridge.csproj -c Release -r win-x64
       --self-contained true -p:PublishSingleFile=true
       -p:IncludeNativeLibrariesForSelfExtract=true -o sensor_bridge/publish
```

   Bundle grows by ~30 MB (the embedded runtime). Acceptable — it's the price of "works
   everywhere." The single resulting `sensor_bridge.exe` still ships next to `llama-monitor.exe`
   exactly as today (`release.yml:72`).

**Runner changes (`../llama-monitor-runner/Dockerfile`):**

The self-hosted runner that builds the Windows bundle already installs `dotnet-sdk-10.0`
(Dockerfile ~line 85), so the heavy lift is done. Specifics:

- **No new apt packages needed.** SDK 10 builds net10.0 natively, which actually *removes* the
  prior latent risk: a net8.0 project on an SDK-10-only image depends on the net8.0
  reference/targeting pack being restorable; net10.0 needs no such pack.
- **Self-contained cross-publish for `win-x64` from this Linux container works**, but it
  restores the Windows runtime packs (`Microsoft.NETCore.App.Runtime.win-x64`,
  `…Host.win-x64`) from NuGet at build time. The runner already has outbound network during
  builds (it fetches buildx, etc.), so no change — **but** if the runner is ever switched to
  offline/locked restore, these packs must be pre-warmed into the NuGet cache. Call this out so
  nobody is surprised by a restore failure in an air-gapped rebuild.
- **Stale comment:** the Dockerfile note "SDK 10 can still build net8.0 targets" becomes moot
  once the project is net10.0. Update or drop it to avoid implying we still target net8.0.
- No .NET *workload* (e.g. MAUI) is required — this is a plain console app; `dotnet-sdk-10.0`
  alone is sufficient.

**Verify:**
- On the runner: `dotnet publish ... -r win-x64 --self-contained true` succeeds and emits a
  single `sensor_bridge.exe`.
- On a Windows VM with **no .NET installed at all**, run that `sensor_bridge.exe` directly →
  it serves sensor JSON on `:7780` (this is the test that the old framework-dependent build
  would have failed).
- End-to-end: remote-agent push to a clean Windows box → temperature data appears in the
  dashboard without any manual .NET install on that box.

---

## 3. P1 — Correctness and platform-convention issues

### P1-1: Config directory ignores Windows convention and diverges from agent mode

**Problem.** `src/config.rs:463-466` hardcodes the config dir on **all** platforms:

```rust
let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
home.join(".config").join("llama-monitor")
```

On Windows this resolves to `C:\Users\<user>\.config\llama-monitor` — a Unix-ism that does not
belong on Windows. Worse, it **diverges from the rest of the codebase**:
- `src/agent.rs:354,459` use `dirs::config_dir()` → `%APPDATA%\llama-monitor` (Roaming).
- All remote-install / path-detection logic assumes `%APPDATA%\llama-monitor`
  (`src/agent.rs:1692, 2266`, etc.).

So on Windows the dashboard stores config under `.config\` while agent mode and every documented
path use `%APPDATA%\`. Same machine, two config dirs.

**Fix (with migration — do not silently strand existing data).**

1. Switch the default to the platform-correct location:

```rust
let config_dir = args.config_dir.unwrap_or_else(|| {
    // %APPDATA%\llama-monitor on Windows, ~/Library/Application Support/llama-monitor on
    // macOS, ~/.config/llama-monitor on Linux. Matches agent.rs and all remote paths.
    dirs::config_dir()
        .unwrap_or_else(|| {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".config")
        })
        .join("llama-monitor")
});
```

> ⚠️ On macOS, `dirs::config_dir()` is `~/Library/Application Support`, **not** `~/.config`.
> This changes the macOS default too. Since macOS is the primary platform, decide deliberately:
> either (a) special-case non-Windows to keep `~/.config` and only fix Windows, or (b) migrate
> all platforms. **Recommended: only change Windows** to minimize blast radius:

```rust
let config_dir = args.config_dir.unwrap_or_else(|| {
    #[cfg(windows)]
    {
        dirs::config_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
            .join("llama-monitor")
    }
    #[cfg(not(windows))]
    {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".config").join("llama-monitor")
    }
});
```

2. **Migration (Windows only).** On startup, if the new `%APPDATA%\llama-monitor` does not exist
   but the legacy `%USERPROFILE%\.config\llama-monitor` does, move it (or copy then mark). Add a
   one-shot migration in `main.rs` before `init_encryption_key`. Log clearly. Keep it best-effort
   (a failed migration must not block startup).

**Verify:** Fresh Windows run → config lands in `%APPDATA%\llama-monitor`. Existing
`.config\llama-monitor` install → data migrated, app finds previous presets/tokens. Local
dashboard and `--agent` mode now agree on the directory.

---

### P1-2: WMI `AdapterRAM` caps VRAM at ~4 GB

**Problem.** `src/gpu/wmi_gpu.rs:36-40` reads VRAM from `Win32_VideoController.AdapterRAM`, which
is a **UINT32** and wraps at 4 GiB. Any modern GPU with >4 GB VRAM reports a wrong, tiny, or
wrapped value. The code already comments on this but still uses it.

**Fix.** Read VRAM from the registry, which stores the true size as a QWORD:
`HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\<NNNN>`,
value `HardwareInformation.qwMemorySize` (REG_QWORD, bytes). Iterate the `0000`, `0001`, …
subkeys and match the adapter by `HardwareInformation.AdapterString`/`DriverDesc` to the WMI
`Name`. Fall back to `AdapterRAM` only when the registry value is missing.

This needs registry access. Options: add the `winreg` crate (small, MSVC+GNU compatible), or
query via the existing `wmi` connection if a better class is available (DXGI is more accurate
but needs FFI). **Recommended: `winreg`** for a contained, well-trodden fix.

**Verify:** On a Windows box with a >4 GB GPU (e.g. 8/12/16 GB card), the dashboard shows the
correct total VRAM instead of a wrapped value.

---

### P1-3: Tray WebView IPC and WebView2 runtime — verify on real hardware + fail gracefully

**Problem.** Two unverified assumptions remain from the original tray plan:
1. `src/tray.rs:397-409` assumes `wry`'s `with_ipc_handler` bridges `window.ipc.postMessage`
   on Windows via WebView2 automatically (there's a code comment admitting it's untested and a
   polyfill may be needed). The popover's resize/close messages depend on this.
2. `wry` requires the **WebView2 runtime**. It's present on Win11 and most updated Win10, but
   **not** guaranteed on LTSC/older images. `build_as_child` currently just logs and returns on
   failure (`src/tray.rs:414-420`), so the user gets a silent no-op popover with no explanation.

**Fix.**
1. On a real Windows machine, click the tray icon and confirm: popover opens, GPU sections
   appearing/disappearing resize it, clicking the icon again closes it. If IPC messages don't
   arrive, add the documented `with_initialization_script` polyfill that forwards
   `window.chrome.webview.postMessage`. (Keep the existing fallback comment honest — update it to
   say "verified" or ship the polyfill.)
2. When `build_as_child` fails, detect probable missing-WebView2 and surface a one-time tray
   notification / log line pointing to the Evergreen runtime, instead of a silent failure.
   Optionally detect WebView2 presence up front via its registry key under
   `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}`.

**Verify:** Popover works end-to-end on Win11. On a VM with WebView2 removed, the user sees a
clear message rather than a dead click.

---

## 4. P2 — Enhancements (optional, schedule after P0/P1)

### P2-1: GPU utilization/temperature on Windows for non-NVIDIA/AMD-CLI cards
WMI gives name + VRAM only — no load, no temp. Two complementary upgrades:
- **NVIDIA without `nvidia-smi` in PATH:** add the `nvml-wrapper` crate to read utilization,
  temp, power, and VRAM via NVML directly (also removes a per-poll subprocess spawn — see P0-2).
- **Intel/AMD utilization:** read GPU engine utilization from PDH performance counters
  (`\GPU Engine(*)\Utilization Percentage`) and/or DXGI `QueryVideoMemoryInfo` for live VRAM.
  No temp without vendor APIs; acceptable to leave temp blank.

### P2-2: Optional file logging
With the GUI subsystem (P0-1), users launched from Explorer have no console. Add an optional
rolling log file under the config dir (e.g. `%APPDATA%\llama-monitor\logs\`) so support can ask
for a log. Gate behind an env var or `--log-file` to avoid disk churn by default.

### P2-3: Windows Service mode (deferred — was `20260519-windows_service_packaging.md`)
Still **not implemented** and still genuinely low priority: the app is a desktop tray app, and
`--headless` already covers "run the web/API without a UI". Implement only if a real
boot-without-login deployment need appears. If so, the design from the old plan still holds:
- Add `windows-service = "0.7"` under `[target.'cfg(windows)'.dependencies]`.
- Add `--service` / `--install-service` / `--uninstall-service` flags (`#[cfg(windows)]`).
- **Prerequisite refactor:** extract the inlined startup in `main.rs` into a reusable
  `run_headless(args, app_config) -> Result<()>` that both `main()` and the SCM `service_main`
  call. This is the bulk of the work and the main reason to defer.
- SCM `service_main` reports StartPending → Running → StopPending and triggers the existing
  graceful-shutdown path (WAL checkpoint + sessions save) on `SERVICE_CONTROL_STOP`.
- Config-dir divergence (SYSTEM account vs user `%APPDATA%`) is solved by passing `--config-dir`
  explicitly; this also depends on P1-1 being done first.

> Note: the existing Windows shutdown handler (`src/main.rs:886-892`) uses
> `tokio::signal::ctrl_c()` only. That is correct and sufficient for the tray/headless app.
> It does **not** handle SCM stop events — only relevant if/when P2-3 is implemented.

---

## 5. Reference-doc changes required

Implementers of the items above **must** update these reference docs in the same PR:

1. **`docs/reference/windows-sensor-bridge-implementation.md`** — *correctness fix.*
   It currently claims `--self-contained true` (line ~87) while CI actually built
   `--self-contained false`, and it references `net8.0`. After P0-3 the truth is: self-contained,
   single-file, **net10.0**, no .NET runtime required on the target. Update the build command,
   the target-framework references, and keep the "no runtime required on target" wording (now
   actually true). Also add a **WinRing0 troubleshooting entry**: Microsoft Defender may
   flag/quarantine the extracted `WinRing0x64.sys` (CVE-2020-14979), making temperature silently
   unavailable; document the symptom, an AV-exclusion path, and the PawnIO-fork fallback (see the
   WinRing0 caveat under P0-3).

2. **New: `docs/reference/windows-support.md`** — *create this.*
   There is currently **no single Windows runtime reference**; knowledge is scattered across the
   (now-deleted) plans and code comments. Create a consolidated reference covering:
   - Config/data directory location (post P1-1) and the legacy-path migration.
   - The subprocess-window policy (P0-2): "all short-lived helper spawns go through
     `crate::platform::no_window`; the only intentional windows are UAC `RunAs` prompts."
   - The GUI-subsystem + `AttachConsole` behavior (P0-1) and how logging works.
   - GPU data sources on Windows (the table from the sensor-bridge doc: nvidia-smi/NVML, rocm-smi,
     WMI/registry VRAM, PDH utilization) and what each does/doesn't provide.
   - WebView2 runtime requirement and the failure-mode UX (P1-3).
   - The shipped bundle contents (`llama-monitor.exe`, `sensor_bridge.exe`) and where each file
     must live relative to the other.
   This doc becomes the canonical "how Windows works" page; link it from `AGENTS.md`.

3. **`docs/reference/binary-lifecycle.md`** — *minor.*
   It documents llama-server install/update generically. Add a short note that on Windows the
   `llama-monitor.exe` **self**-update cannot overwrite a running `.exe` in place and therefore
   uses the detached `.bat` helper (`src/agent.rs` install module), distinct from the
   rename-based llama-server update. One paragraph; the mechanism already exists in code.

---

## 6. Implementation order (single recommended sequence)

1. **P0-1 + P0-2 together** (GUI subsystem + `crate::platform::no_window` everywhere). Land as
   one change; they are only safe together. New file `src/platform.rs`.
2. **P0-3** (sensor bridge self-contained) + reconcile the sensor-bridge reference doc.
3. **P1-1** (config dir + Windows migration). Touches startup; run the full test suite after.
4. **P1-2** (VRAM via registry).
5. **P1-3** (WebView2 / IPC verification — needs real Windows hardware).
6. Author **`docs/reference/windows-support.md`** capturing the new behavior (steps 1-5).
7. P2 items as capacity allows. P2-3 (service mode) only on demand.

---

## 7. Verification matrix

CI cross-compiles from Linux (`x86_64-pc-windows-gnu`) and **cannot** exercise runtime behavior.
After each change, run the cheap check; the runtime checks require a Windows machine/VM.

```bash
rustup target add x86_64-pc-windows-gnu
cargo check --target x86_64-pc-windows-gnu     # cfg-guard + compile sanity after every edit
```

Manual (Windows 11 + a clean Windows 10 VM without .NET / without WebView2):

| Test | Expected |
|---|---|
| Double-click `.exe` from Explorer | No console window; tray icon appears (P0-1) |
| `llama-monitor.exe --version` from PowerShell | Version prints (P0-1 re-attach) |
| Run with a model loaded + GPU polling for 60 s | Zero console flashes (P0-2) |
| Sensor-bridge install (Settings) | UAC prompt appears (the one allowed window) |
| Clean VM (no .NET at all), run `sensor_bridge.exe` | Serves JSON on `:7780` — self-contained net10.0 (P0-3) |
| Fresh install | Config under `%APPDATA%\llama-monitor` (P1-1) |
| Upgrade over a `.config\llama-monitor` install | Data migrated; presets/tokens intact (P1-1) |
| GPU with >4 GB VRAM | Correct total VRAM shown (P1-2) |
| Tray icon click | Popover opens, resizes, closes via IPC (P1-3) |
| VM without WebView2 | Clear message instead of silent dead click (P1-3) |

---

## 8. New / modified files at a glance

| File | Change | Item |
|---|---|---|
| `src/main.rs` | `windows_subsystem` attr; `attach_parent_console()`; config-dir migration call | P0-1, P1-1 |
| `src/platform.rs` | **new** — `no_window` / `no_window_tokio` helpers | P0-2 |
| `src/gpu/nvidia.rs`, `src/gpu/rocm.rs`, `src/gpu/mod.rs` | route spawns through `no_window` | P0-2 |
| `src/llama/server.rs` | `no_window_tokio` on server spawn; `no_window` on `taskkill` | P0-2 |
| `src/config.rs` | `no_window` on `icacls`; Windows config-dir default | P0-2, P1-1 |
| `src/lhm.rs` | `no_window` on `schtasks` (NOT on `RunAs` powershell) | P0-2 |
| `.github/workflows/release.yml` | sensor bridge `--self-contained true` | P0-3 |
| `sensor_bridge/sensor_bridge.csproj` | bump to `net10.0`; pin `RuntimeIdentifier`/`SelfContained`/single-file | P0-3 |
| `../llama-monitor-runner/Dockerfile` | already has `dotnet-sdk-10.0`; update stale "builds net8.0" comment; ensure NuGet restore reachable | P0-3 |
| `src/gpu/wmi_gpu.rs` | registry VRAM read (add `winreg`) | P1-2 |
| `src/tray.rs` | IPC polyfill (if needed) + WebView2 failure messaging | P1-3 |
| `Cargo.toml` | maybe `winreg` (P1-2), `nvml-wrapper` (P2-1), `windows-service` (P2-3) | P1-2, P2 |
| `docs/reference/windows-sensor-bridge-implementation.md` | reconcile self-contained | §5 |
| `docs/reference/windows-support.md` | **new** consolidated Windows reference | §5 |
| `docs/reference/binary-lifecycle.md` | note Windows self-update `.bat` mechanism | §5 |
