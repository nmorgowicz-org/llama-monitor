# Windows Native Tray Roadmap

Date: 2026-04-22

## Context

The Windows release asset previously shipped a lone `llama-monitor.exe` that linked the `wry` WebView stack. On Windows, `wry` uses WebView2 and imports `WebView2Loader.dll`. Because Windows resolves imported DLLs before Rust `main()` runs, a missing loader DLL prevents every code path from launching, including:

- `llama-monitor.exe --version`
- `llama-monitor.exe --agent`
- SSH install/start health checks
- Any future headless repair flow

The immediate release goal is to keep the Windows SSH install/start flow reliable while still providing a useful tray presence. The current recommendation is to avoid WebView2 for the Windows tray path and use native Windows tray/menu functionality first, then evaluate a richer native UI after the current branch is merged.

## Packaging Decision

Do not add separate Windows executables to the release page for this phase.

The preferred near-term release shape is still a single Windows executable:

- `llama-monitor-windows-x86_64.exe`
- Built with `--no-default-features --features native-tray`
- Built with `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C target-feature=+crt-static"`
- Does not import `WebView2Loader.dll`, `libgcc`, or `winpthread`

This keeps the SSH installer simple: download one EXE, place it under `%APPDATA%\llama-monitor\bin\llama-monitor.exe`, start with Task Scheduler, and verify `/health`.

If a future Windows desktop build needs a WebView2 popover, prefer a ZIP/installer that includes required runtime-adjacent files. Do not silently ship a lone EXE that has a hard DLL dependency.

## Phase 1: Native Tray Icon And Menu

### Goal

Provide a functional Windows tray experience without WebView2.

The Windows tray should use:

- `tray-icon` for native tray icon/menu integration.
- `winit` for the event loop needed by `tray-icon`.
- No `wry`.
- No `WebView2Loader.dll`.

### Expected User Experience

The Windows tray should provide:

- A tray icon with tooltip status.
- Left/right click opens a native context menu.
- Menu action: `Open Dashboard`.
- Menu action: `Quit`.
- Read-only metric rows:
  - endpoint/session mode
  - prompt throughput
  - generation throughput
  - active request count or slot activity
  - host metric availability

The menu is intentionally simple. It is a control/status surface, not a miniature dashboard.

### Implementation Notes

For the AI agent implementing or maintaining this:

1. Start in `src/tray.rs`.
2. Keep the existing macOS/Linux WebView popover behind `#[cfg(all(not(target_os = "windows"), feature = "webview-popover"))]`.
3. Add a Windows-specific native menu path behind `#[cfg(target_os = "windows")]`.
4. Use `tray_icon::menu::{Menu, MenuItem, MenuEvent, PredefinedMenuItem}`.
5. Store cloned `MenuItem`s in `TrayApp` so their text can be updated every tick.
6. On `Open Dashboard`, launch:
   - `cmd.exe /C start "" "http://127.0.0.1:{port}"`
7. For tooltip/menu data, use the existing `TrayState::get_metrics()` output. Do not add new polling loops.
8. Keep metric text compact and stable. Native menus are not good at dense live telemetry.

### Acceptance Criteria

- `cargo check` passes with default features.
- `cargo check --no-default-features --features native-tray` passes.
- Windows release build passes:

  ```bash
  CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C target-feature=+crt-static" \
    cargo build --release --target x86_64-pc-windows-gnu --no-default-features --features native-tray
  ```

- Windows PE dependency check shows no `WebView2Loader.dll`, `libgcc`, or `winpthread` import:

  ```bash
  x86_64-w64-mingw32-objdump -p target/x86_64-pc-windows-gnu/release/llama-monitor.exe
  ```

- SSH remote checks work:
  - `llama-monitor.exe --version`
  - `llama-monitor.exe --agent --agent-host 0.0.0.0 --agent-port 7779`
  - `GET http://<windows-host>:7779/health`

## Phase 2: Rich Native Tray Panel

### Goal

Replace the Windows native menu with a richer native tray panel that can approach or exceed the current HTML/WebView compact popover experience without requiring WebView2.

The strongest candidate is `egui`/`eframe`.

### Why egui

`egui` is immediate-mode, Rust-native, and well suited to telemetry dashboards:

- Fast custom drawing for gauges, sparklines, activity strips, and compact metric cards.
- Good single-binary story compared with WebView2.
- No HTML/CSS/browser runtime.
- Easier to make dense, responsive data presentation than a native menu.
- Works across Windows/macOS/Linux, making it a candidate to replace the current WebView popover everywhere.

The current WebView compact UI is useful because it reuses existing HTML/CSS, but a purpose-built `egui` panel could be cleaner and more stable:

- No browser embedding dependency.
- No CSS sizing surprises.
- Direct drawing for high-frequency telemetry.
- Shared Rust state formatting instead of duplicating presentation in JS.

### Libraries To Evaluate

Primary:

- `egui`
- `eframe`
- `egui_extras`
- `egui_plot` if lightweight plots are needed

Alternatives:

- `native-windows-gui`: more native-looking on Windows, less compelling for rich charts.
- Raw Win32 via `windows`: maximum control, highest maintenance cost.
- `slint`: polished declarative UI, but introduces another UI language/runtime model.
- `iced`: good application toolkit, likely heavier than needed for tray popover.

### Proposed Architecture

Create a cross-platform native tray panel module:

- `src/tray/native_panel.rs`
- `src/tray/model.rs`
- `src/tray/windows.rs` if Windows-specific anchoring is needed

Use a shared presentation model:

```rust
struct TraySnapshot {
    endpoint_label: String,
    session_label: String,
    prompt_tokens_per_sec: f64,
    generation_tokens_per_sec: f64,
    requests_processing: u32,
    slots_processing: u32,
    slots_idle: u32,
    context_live_tokens: Option<u64>,
    context_capacity_tokens: Option<u64>,
    host_metrics_available: bool,
    cpu_load_percent: Option<f32>,
    cpu_temp_c: Option<f32>,
    gpu_rows: Vec<TrayGpuRow>,
}
```

The model should be built from existing app state:

- `AppState::current_endpoint_kind()`
- `AppState::current_session_kind()`
- `AppState::host_metrics_available()`
- `state.system_metrics`
- `state.gpu_metrics`
- `state.llama_metrics`

### Rich Panel UI Scope

Initial `egui` panel should include:

- Header: endpoint/session state.
- Throughput row:
  - prompt tok/s
  - generation tok/s
  - active/deferred requests where available
- Context row:
  - live context tokens
  - capacity
  - percentage bar
- Slot row:
  - processing/idle count
  - active task id if available
- Host row:
  - CPU load/temp
  - memory used/total if available
- GPU rows:
  - name
  - temp
  - load
  - VRAM percent

Design guidance:

- Favor dense, premium dashboard presentation over native menu semantics.
- Use direct drawing for bars and sparklines.
- Avoid large empty panels when remote host metrics are unavailable.
- Keep a compact footprint; target around 280-360px wide.

### Cross-Platform Cutover Option

If the `egui` panel is cleaner than the current WebView popover, plan a follow-up cutover:

1. Keep `static/compact.html` as a fallback for one release.
2. Add `native-tray-panel` feature for all desktop OSes.
3. Run macOS/Linux/Windows manual tray QA.
4. Remove `wry` from default builds once the native panel reaches feature parity.
5. Keep the full browser dashboard unchanged.

This could produce a better long-term architecture: the full web UI remains the main product surface, while the tray becomes a lightweight native dashboard with no embedded-browser dependency.

### Suggested Task Breakdown

1. Add `TraySnapshot` model and tests for formatting.
2. Implement Windows native menu as Phase 1.
3. Add a spike branch with `egui`/`eframe`.
4. Build a standalone native panel window from static mock data.
5. Wire the panel to `TraySnapshot`.
6. Implement tray anchoring and show/hide behavior on Windows.
7. Add macOS and Linux support.
8. Compare native panel against current WebView popover.
9. Decide whether to make native panel the default and remove `wry`.

## Current Branch Priority

For the current branch/PR, keep the scope narrow:

- Finish SSH install/update/start/stop reliability.
- Keep Windows release artifact as a single EXE.
- Add simple native Windows tray menu.
- Defer rich native tray panel work until after merge.

