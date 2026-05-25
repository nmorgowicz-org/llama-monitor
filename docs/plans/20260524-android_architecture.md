# Android Architecture for Llama-Monitor

**Date:** 2026-05-24  
**Author:** Claude (initial analysis), opencode/qwen36-27b (code scan verification and corrections)  
**Status:** Draft — Code scan verified and corrected  
**Branch:** feat/android-compatibility  
**Last Verified:** 2026-05-24 (code scan against `feat/android-compatibility` branch)

---

## 1. System Overview

llama-monitor is a **monitoring-first web dashboard** for llama.cpp servers, implemented as:

- A **Rust binary** serving a warp HTTP/WebSocket server on configurable port (default 7778)
- A **vanilla JS web UI** (ES modules, no framework, no bundler) embedded in the binary at compile time via `build.rs` → `include_str!` macros generating `src/gen/static_assets.rs` and `src/gen/routes.rs`
- **Platform-specific metric backends** for GPU, CPU, and system telemetry
- A **remote agent protocol** (HTTPS/mTLS + bearer token) for attaching to remote GPU machines

The binary does NOT use Electron, Tauri, or any JavaScript runtime. The tray icon and optional desktop WebView popover are behind Cargo feature flags (`native-tray`, `webview-popover`) that are stripped at compile time.

**Runtime data flow:**
```
llama-server (HTTP :8001)
        ↓ polling (500ms)
[Rust: LlamaPoller task]
        ↓ Arc<Mutex<AppState>>
[Rust: SystemPoller task (5s)]  [Rust: GpuPoller task (5s)]
        ↓
[Rust: WebSocket pusher (200ms–10s, default 500ms)]
        ↓ JSON over ws://localhost:PORT/ws
[Browser / WebView: vanilla JS dashboard-ws.js → DOM updates]
```

**Remote agent mode adds:**
```
ryne:7779 (llama-monitor --agent, HTTPS/mTLS)
        ↓ polling (5s) via reqwest/rustls
[Rust: RemoteAgentPoller task]
        ↓ merged into same AppState
```

**Static asset pipeline:**
```
static/
  index.html, js/**/*.js, css/**/*.css, manifest.json
        ↓ build.rs (runs on host at compile time)
src/gen/static_assets.rs   — include_str! constants
src/gen/routes.rs           — warp route composition
        ↓ compiled into binary / cdylib
```

---

## 2. Dependency Audit: Android Compatibility Reality

### 2.1 Feature Flag Structure

**Current (verified 2026-05-24):**
```toml
[features]
default = ["native-tray", "webview-popover"]
native-tray = ["dep:tray-icon", "dep:winit"]
webview-popover = ["dep:wry"]
```

**Proposed addition:**
```toml
[features]
default = ["native-tray", "webview-popover", "ssh-control"]
native-tray = ["dep:tray-icon", "dep:winit"]
webview-popover = ["dep:wry"]
ssh-control = ["dep:ssh2"]
android = []

[dependencies]
ssh2 = { version = "0.9", features = ["vendored-openssl"], optional = true }
```

Building with `--no-default-features` removes `wry`, `winit`, `tray-icon`, and all their transitive platform-specific deps (`libappindicator`, `muda`, `objc2`, `webkit2gtk`, etc.).

**Note on CLI parsing:** The `clap` crate (v4) is always compiled regardless of features. On Android in cdylib mode, CLI parsing is dead weight. Consider adding a `cli` feature to gate it:
```toml
[features]
cli = ["dep:clap"]
```
This is deferred to Phase 2+ as it requires refactoring `src/cli.rs` and `src/main.rs`.

### 2.2 Platform-Conditional Dependencies: Not a Problem on Android

| Crate | Gate | Android status |
|---|---|---|
| `gtk 0.18` | `cfg(target_os = "linux")` | ✅ Excluded — `target_os = "android"` ≠ `"linux"` |
| `mac-notification-sys 0.6` | `cfg(target_os = "macos")` | ✅ Excluded |
| `wmi 0.18.4` | `cfg(windows)` | ✅ Excluded |

`cfg(target_os = "linux")` does **not** match `aarch64-linux-android`. This is a fundamental Rust target triple distinction — `aarch64-linux-android` has `target_os = "android"`, not `"linux"`.

### 2.3 Real Remaining Issues After `--no-default-features`

| Crate | Issue | Severity | Solution |
|---|---|---|---|
| `ssh2 0.9.5` + `vendored-openssl` | Links vendored OpenSSL C; NDK integration non-trivial | **High** | Feature-gate `ssh2` under new `ssh-control` feature; Android build uses `--no-default-features --features android` |
| `rusqlite 0.39` + `bundled` | Bundles SQLite C source | **Medium** | Solvable — SQLite cross-compiles cleanly with NDK; `bundled` feature is designed for this |
| `dirs 6.0.0` | Returns empty paths on Android | **Low** | Single `#[cfg(target_os = "android")]` path override |
| `sysinfo 0.39` | Reduced metrics on Android | **Low** | Use as-is; returns what Android allows |
| `tempfile 3.27` | Depends on `rustix 1.1.4` which has unresolved Android compile errors (`linux_raw_sys` refs on bionic libc). PR #1577 to fix this has been open since Feb 2026 with no merge. | **High** | Use `#[cfg(target_os = "android")]` alternative: create temp files manually in app's private storage (`filesDir`) instead of relying on `tempfile` crate |

**Additional tempfile note:** 10+ uses found in `src/agent.rs` alone (lines 1979, 2001, 2094, 2154, 2157, 2235, 2340, 2467, etc.). Most are in remote agent install flow — consider if this flow is needed on Android.

**tempfile usage audit (10+ uses found):**
- `src/agent.rs`: lines 1979, 2001, 2094, 2154, 2157, 2235, 2340, 2467 — remote agent install flow, cert handling
- `src/chat_storage.rs`: line 1258 — test code only
- `src/acme.rs`: line 408 — ACME cert renewal

Most uses are in the remote agent install flow, which may not be needed on Android (Android is a metrics consumer, not the agent host).
| `src/main.rs` | Desktop entry with tray/GUI setup | **Medium** | Android uses cdylib JNI entry; `main.rs` excluded from cdylib target |
| `src/system.rs` | Platform-specific commands/paths | **Medium** | Add `#[cfg(target_os = "android")]` backends |
| `src/gpu/mod.rs` | No Android GPU backend | **Medium** | Add `AndroidGpuBackend` (dummy initially) |

### 2.4 What Compiles Cleanly for Android (Majority of the Codebase)

| Component | Crates | Status |
|---|---|---|
| Async runtime | `tokio 1.52` | ✅ Android supported |
| HTTP server | `warp 0.4` | ✅ Pure Rust |
| HTTP client | `reqwest 0.13` (rustls, `default-features = false`) | ✅ Pure Rust TLS |
| TLS | `rustls 0.23`, `tokio-rustls 0.26` | ✅ Pure Rust |
| HTTPS | `hyper-rustls 0.27` (ring feature) | ✅ Pure Rust |
| Cert generation | `rcgen 0.14` | ✅ Pure Rust |
| Crypto | `aes-gcm 0.10`, `sha1/sha2 0.11`, `hkdf 0.13` | ✅ Pure Rust |
| Password hashing | `argon2 0.6.0-rc.8` | ✅ Pure Rust |
| Randomness | `rand 0.10`, `getrandom` | ✅ Android supported |
| Serialization | `serde 1`, `serde_json 1` | ✅ Pure Rust |
| System info | `sysinfo 0.39` | ✅ Explicit Android support |
| Security headers | `warp-helmet 1.0` | ✅ Pure Rust |
| Build script | `build.rs` | ✅ Runs on host, not cross-compiled |
| Chat storage | `rusqlite 0.39` + `bundled` | ✅ Works with NDK |
| GUI stack | `wry`, `winit`, `tray-icon`, `gtk` | ✅ All excluded |

---

## 3. Proposed Architecture

### 3.1 Guiding Principle

**Android is a first-class platform, not a port.** Per the MANDATORY multi-platform rule in AGENTS.md, Android joins macOS, Linux, and Windows as a supported target. Every Android-specific code path requires a `#[cfg(target_os = "android")]` guard with an inline comment explaining the gap, following the same pattern as existing Windows gaps (W-04 file permissions, W-05 service signals, etc.).

### 3.2 High-Level Components

```
┌──────────────────────────────────────────────────────────────┐
│                      Android App (APK)                       │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Kotlin: MainActivity + LlamaMonitorService          │   │
│  │  - Activity lifecycle, WebView hosting               │   │
│  │  - Foreground service (keeps Rust runtime alive)     │   │
│  │  - Notifications with live metrics summary           │   │
│  │  - AndroidKeyStore for secret storage                │   │
│  │  - Foldable state detection (androidx.window)        │   │
│  │  - Adaptive polling (screen/battery callbacks)       │   │
│  └──────────────┬──────────────────────────┬────────────┘   │
│                 │ JNI                       │ loopback HTTP  │
│  ┌──────────────▼────────────┐  ┌──────────▼────────────┐   │
│  │  Rust cdylib (libllama.so)│  │  Android WebView      │   │
│  │                           │  │                       │   │
│  │  - Tokio runtime          │  │  http://127.0.0.1:    │   │
│  │  - Warp HTTP server       │◄─┤  PORT                 │   │
│  │    127.0.0.1:PORT         │  │                       │   │
│  │  - WebSocket push         │  │  Existing JS/CSS/HTML │   │
│  │  - Llama metrics poller   │  │  zero changes needed  │   │
│  │  - Remote agent client    │  │  for Phase 1          │   │
│  │  - Chat storage (SQLite)  │  └───────────────────────┘   │
│  │  - Auth/TLS (rustls)      │                              │
│  │  - Android metric backends│                              │
│  └───────────────────────────┘                              │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  APK Assets: static/index.html, js/**/*.js, css/**   │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
          │ HTTPS/mTLS + bearer token       │ HTTP direct
          ▼                                 ▼
   Remote Agent (ryne)               llama-server :8001
   llama-monitor --agent             inference metrics + chat
```

### 3.3 Architecture Decision: Localhost Warp Server (Not JNI Bridge)

Two approaches were considered:

**Option A: JNI bridge only** — WebView uses `addJavascriptInterface` to call Kotlin → Rust JNI.  
**Option B: Localhost warp server** — Rust runs warp on `127.0.0.1:PORT`; WebView connects exactly as a desktop browser.

**Decision: Option B.**

Rationale:
1. **Zero web UI changes** — `dashboard-ws.js` connects to `location.host`. The WebView loads `http://127.0.0.1:PORT/` directly from the warp server (not `file://`), so `location.host` resolves to `127.0.0.1:PORT`. WebSocket and all `fetch` calls work without modification.
2. **Reuses full existing message protocol** — JSON push schema, capability negotiation, availability enums, telemetry grades all work as-is.
3. **Single code path** — Warp handlers, auth, and push logic serve Android identically to desktop.
4. **Debuggable** — Chrome DevTools remote debugging works on Android WebView; the HTTP server can be hit directly with `adb forward`.
5. **No new IPC layer** — JNI bridge is needed only for lifecycle (start/stop), secret management, and battery callbacks. Not for data.

Trade-off: ~2-5MB RAM overhead for loopback. Acceptable on Z Fold 6 (12GB RAM).

**Important:** Loading from `file://` (APK assets) is NOT viable — `file://` URLs have no `host:port` in the origin, so `location.host` is empty and `ws:///ws` is an invalid WebSocket URL. Loading from the warp server avoids this entirely.

### 3.4 Web UI Asset Strategy

**Strategy 2 is the recommended approach from Phase 1.** Use the existing `include_str!` mechanism in the cdylib (same as desktop). Assets compiled into `libllama.so`. The warp server serves them via the existing `static_routes()` function. The WebView loads `http://127.0.0.1:PORT/` directly from the warp server.

Benefits:
- **Zero JS changes** — `location.host` resolves correctly; WebSocket and `fetch` work as-is
- **Consistent with desktop** — same asset pipeline, same `build.rs` generation
- **Single code path** — `gen/routes.rs` compiles fine in a cdylib; warp serves assets identically
- **No APK assets management** — no separate asset copy step in Gradle build

The APK only needs the Kotlin app shell (Activity, Service, JNI declarations) and the `libllama.so` cdylib. All web assets flow through the warp server.

---

## 4. Existing Code Reusability Analysis

### 4.1 Modules That Require Zero Changes

| Module | Purpose |
|---|---|
| HTTP server / route setup | warp routes, static asset serving |
| WebSocket handler + push | JSON push, interval management |
| Llama metrics poller | HTTP polling to llama-server |
| Remote agent client | HTTPS/mTLS polling to ryne |
| Auth (token, form, basic) | All auth logic unchanged |
| TLS / rcgen | Certificate generation, rustls |
| Chat storage | rusqlite + SQLite (compiles with NDK) |
| Session management | Session state machine |
| Capability calculation | Derives capabilities from session type |
| REST API handlers | All /api/* handlers |
| SQLite backup logic | Online backup API |
| Generated static routes | Regenerated by build.rs |

### 4.2 Modules That Need Android Backends

| Module | Current behavior | Android change |
|---|---|---|
| `src/system.rs` | CPU name/temp/load/clock via sysinfo + platform commands (sysctl, WMI, /proc) | Add `#[cfg(target_os = "android")]` branch: `/proc/stat`, `/sys/class/thermal`, `/proc/cpuinfo`, BatteryManager JNI callbacks |

**Critical: All four functions below require `#[cfg(target_os = "android")]` guards:**

| Function (line) | Current variants | Android fallback |
|---|---|---|
| `get_cpu_name()` (line 47) | `[windows]`, `[linux]`, `[macos]` | `/proc/cpuinfo` `Hardware:` field |
| `get_cpu_temp()` (line 101) | `[windows]`, `[linux]`, `[macos]` | BatteryManager + thermal zone enumeration |
| `get_cpu_clock()` (line 177) | `[linux]`, `[windows]`, `[not(linux, windows)]` | Falls into catch-all (sysinfo only) |
| `get_motherboard()` (line 321) | `[windows]`, `[linux]`, `[macos]` | `sysinfo::System::get()` for device info |
| `src/gpu/mod.rs` | Detects NVIDIA/AMD/Apple backend | Add `#[cfg(target_os = "android")]` branch; returns `AndroidGpuBackend` (dummy Phase 1, Adreno/Mali Phase 5) |
| `src/certs.rs` | `certs_dir()` (line 22) derives path from `dirs::home_dir() + ".config/llama-monitor/certs"` | **Must also be overridden for Android** — add `#[cfg(target_os = "android")]` arm that reads from `android::paths::files_dir()` instead of `dirs::home_dir()`; this is separate from the `config.rs` override |
| `src/config.rs` | Uses `dirs` crate for config path | Add `#[cfg(target_os = "android")]` path override using JNI-provided `filesDir` |
| `src/gpu/mod.rs` | `is_apple_silicon()` calls `Command::new("sysctl")` before the backend dispatch | Add `#[cfg(target_os = "android")] { return false; }` early-exit at the top of `is_apple_silicon()` — without this, `sysctl` is spawned as a subprocess on Android (fails harmlessly but wastes time) |
| `src/lhm.rs` | Windows sensor bridge polling | Already conditional on Windows; not compiled on Android |
| `src/tray.rs` | Desktop tray management | Already feature-gated; excluded by `--no-default-features` |
| `src/main.rs` | Binary entry point | Android uses cdylib JNI; `main.rs` not in cdylib target |

### 4.3 New Android-Specific Files Required

| File | Purpose |
|---|---|
| `src/android/mod.rs` | JNI entry points: `startServer`, `stopServer`, `isRunning`, `setPollInterval` |
| `src/android/battery.rs` | JNI receiver for BatteryManager data from Kotlin |
| `src/android/thermal.rs` | Read `/sys/class/thermal/thermal_zone*/temp` |
| `src/android/paths.rs` | Android config/data directory resolution |
| `src/android/keystore.rs` | JNI bridge to AndroidKeyStore via Kotlin |
| `src/gpu/android.rs` | Android GPU backend (dummy → Adreno → Mali) |
| `android/app/src/main/java/…/MainActivity.kt` | Activity: WebView host, service binding |
| `android/app/src/main/java/…/LlamaMonitorService.kt` | Foreground service |
| `android/app/src/main/java/…/RustLib.kt` | JNI declarations |
| `android/app/src/main/java/…/KeystoreHelper.kt` | EncryptedSharedPreferences wrapper |
| `android/app/src/main/res/xml/network_security_config.xml` | Allow cleartext to 127.0.0.1 |
| `android/app/build.gradle` | Gradle build; cargo-ndk integration |

### 4.4 SSH Control on Android

`ssh2 0.9.5` is the sole user of OpenSSL in the dependency tree (via `vendored-openssl` feature on `libssh2-sys` → `openssl-sys`). Three options:

1. **Feature-gate (Phase 1-3):** Android build skips SSH control. Android is a metrics consumer of the already-running remote agent. Agent management done from desktop only.
2. **Pure-Rust SSH (`russh` crate) (Phase 4 evaluation):** Eliminates OpenSSL from the entire project — desktop and Android. Worth evaluating as an architectural improvement.
3. **Relay pattern:** Android sends control commands to a relay endpoint on the Mac; Mac performs SSH to ryne.

**Decision: Option 1 for Phase 1-3; evaluate Option 2 for Phase 4.**

Cargo.toml change:
```toml
[features]
default = ["native-tray", "webview-popover", "ssh-control"]
native-tray    = ["dep:tray-icon", "dep:winit"]
webview-popover = ["dep:wry"]
ssh-control    = ["dep:ssh2"]   # NEW: isolates OpenSSL
android        = []              # NEW: marker for Android-specific code

[dependencies]
ssh2 = { version = "0.9", features = ["vendored-openssl"], optional = true }
```

Android build command: `--no-default-features --features android`

---

## 5. Data Flow on Android

### 5.1 Metrics Push Pipeline

```
Android Foreground Service
   │
   ├─ Rust Tokio runtime (libllama.so)
   │     ├─ LlamaPoller (500ms)
   │     │     └─ GET llama-server:8001 /metrics /slots /v1/models
   │     │
   │     ├─ RemoteAgentPoller (5s)
   │     │     └─ HTTPS/mTLS → ryne:7779
   │     │           /api/metrics (GPU + system from ryne)
   │     │           /api/info (agent version)
   │     │           /health (reachability)
   │     │
   │     ├─ AndroidSystemPoller (5s) [NEW]
   │     │     ├─ /proc/stat → CPU load
   │     │     ├─ /proc/cpuinfo → CPU name, clock
   │     │     ├─ /sys/class/thermal/ → thermal zones (partial)
   │     │     └─ JNI callback → BatteryManager data
   │     │
   │     ├─ AndroidGpuPoller (5s) [NEW]
   │     │     └─ Phase 1: availability = BackendUnavailable
   │     │        Phase 5: kgsl sysfs / Mali sysfs
   │     │
   │     └─ WebSocket pusher
   │           └─ ws://127.0.0.1:PORT/ws
   │
   └─ Android WebView
         └─ dashboard-ws.js: updateDashboard(d)
               └─ Renders sparklines, GPU cards, chat, logs
```

### 5.2 Availability Codes for Android Metrics

The existing `Availability` enum covers all Android cases:

| Metric | Android code | Reason |
|---|---|---|
| GPU (Phase 1-4) | `BackendUnavailable` | No driver-level access without root |
| GPU clock (Phase 5, Adreno) | `Available` or `SensorUnavailable` | Firmware-dependent |
| CPU temperature | `Available` (partial) or `SensorUnavailable` | SELinux may block thermal zones |
| CPU load | `Available` | `/proc/stat` generally readable |
| CPU clock | `Available` (partial) | cpufreq sysfs often accessible |
| Memory | `Available` | sysinfo MemTotal/MemAvailable |
| Battery | `Available` | BatteryManager unrestricted |
| Remote agent metrics | `Available` when connected | Same as desktop |

Never return `null` silently. Always set the `availability` field to the correct enum value so the UI can render the correct card state (`is-unavailable`, `is-dormant`, etc.).

### 5.3 Adaptive Polling

The Kotlin foreground service communicates device state to the Rust runtime via `RustLib.setPollInterval(ms)`:

| Device state | Poll interval |
|---|---|
| Screen on, charging | 500ms (default) |
| Screen on, battery | 1000ms |
| Screen off | 10000ms |
| Battery saver mode | 30000ms |
| App backgrounded (JS Page Visibility) | 5000ms (JS signals server) |

---

## 6. UI/UX Architecture

### 6.1 The Web UI's Actual Mobile Readiness

The frontend is significantly more mobile-ready than a naive assessment might suggest.

**Already present in the codebase:**
- `<meta name="viewport" content="width=device-width, initial-scale=1">`
- `<meta name="apple-mobile-web-app-capable" content="yes">`
- `<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">`
- `/manifest.json` (PWA manifest)
- Responsive CSS breakpoints: 1200px, 980px, 860px, 820px, 768px, 720px, 640px
- `ResizeObserver` density classes: `shell-width-very-tight` (< 520px), `shell-width-tight` (< 920px), `shell-width-snug` (< 1050px) — applied to `<body>` dynamically
- `@media (prefers-reduced-motion: reduce)` applied across all 159+ animations
- `backdrop-filter: blur(16px) saturate(180%)` on cards (GPU-composited)
- IndexedDB for chat state via `chat-state.js`

**Missing — requires addition:**
- Touch event handlers for drag interactions (sidebar resize handle, chat tab reorder)
- `-webkit-overflow-scrolling: touch` on remaining scrollable containers (already present in `chat.css` and `chat-guided-generation.css`; still needed on `.content-area`, `.log-panel`, `.settings-modal-body`, `.chat-sessions-list`)
- Touch targets sized to 44×44dp minimum (`@media (pointer: coarse)`) — not present anywhere in the CSS
- Safe area insets (`env(safe-area-inset-*)`) for punch-hole display — not present
- `visualViewport` resize listener for soft keyboard avoidance in chat — not present (only `window.innerHeight` is used)
- Swipe gestures for sidebar open/close — not present
- Long-press for context menus (currently right-click / `contextmenu` event only) — not present
- `prefers-color-scheme` CSS `@media` rules for automatic dark/light switch (JS detection via `window.matchMedia` already present in `user-menu.js`; CSS-level rules still missing)
- Passive touch event listener discipline: drag-prevention handlers use `{ passive: false }` (required for `preventDefault()`); all others use `{ passive: true }` to avoid scroll jank
- Mobile URL input optimization: endpoint/URL fields in setup should use `type="url"` + `inputmode="url"` to get the correct mobile keyboard and suppress autocorrect

### 6.2 Z Fold 6 Form Factor

**Device dimensions (Z Fold 6):**
- **Folded (cover display):** ~390×748dp portrait (Snapdragon 8 Gen 3, Adreno 750)
- **Unfolded (main display):** ~932×932dp landscape/tablet

**How the existing layout classes map:**

| Physical state | CSS viewport | Density class triggered | Layout result |
|---|---|---|---|
| Folded portrait | ~390px wide | `shell-width-tight` | Sidebar collapsed (68px), stacked inference cards |
| Folded landscape | ~748px wide | `shell-width-tight` | Partial sidebar, 2-col inference grid |
| Unfolded landscape | ~932px wide | `shell-width-snug` | Expanded sidebar, full 3-col dashboard |
| Unfolded portrait | ~692px wide | `shell-width-tight` | Similar to folded landscape |

The `inference-grid` collapses at `max-width: 860px` — this means folded portrait and folded landscape both get the stacked layout, which is correct behavior.

**Note:** A `max-width: min(420px, 92vw)` rule already exists in `chat.css` for a specific chat element — this is NOT the global layout breakpoint described below. The following new rule is still needed in `layout.css` to hide the sidebar on the folded cover display.

**New CSS breakpoint for folded cover (add to `layout.css`):**
```css
/* Folded cover display: max 420px */
@media (max-width: 420px) {
  .sidebar-nav {
    display: none; /* Full-width content on cover panel */
  }
  .content-area {
    margin-left: 0;
    width: 100%;
  }
  .dashboard-header {
    padding: 12px 16px;
  }
  .view {
    padding: 12px;
    gap: var(--gap-md);
  }
}
```

**Foldable hinge detection via Kotlin → WebView:**
```kotlin
// In MainActivity, using androidx.window:window-java
windowInfoTracker.windowLayoutInfo(this).collect { info ->
    val fold = info.displayFeatures
        .filterIsInstance<FoldingFeature>()
        .firstOrNull()
    val state = when {
        fold == null -> "flat"
        fold.state == FoldingFeature.State.HALF_OPENED -> "half"
        fold.isSeparating -> "unfolded"
        else -> "flat"
    }
    webView.evaluateJavascript(
        "window.dispatchEvent(new CustomEvent('foldstatechange'," +
        "{ detail: { state: '$state' } }))", null
    )
}
```

In JS (`bootstrap.js` or new `android-foldable.js`):
```javascript
window.addEventListener('foldstatechange', (e) => {
    document.body.classList.remove('is-folded', 'is-unfolded', 'is-half');
    document.body.classList.add(`is-${e.detail.state}`);
});
```

**Two-panel unfolded layout (new CSS):**
```css
body.is-unfolded #page-chat {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--gap-lg);
}

body.is-unfolded #page-server .dashboard-grid {
  /* Full 12-column grid available; all 3 inference cards visible */
  grid-template-columns: repeat(12, minmax(0, 1fr));
}
```

Must have `[data-theme="light"]` overrides for all new selectors (AGENTS.md requirement).

### 6.3 Touch Handling

**Sidebar toggle:** Tap works via native `click` event propagation — no changes needed. Drag-to-resize needs touch handlers mirroring the existing mouse handlers:
```javascript
// Existing: mousedown/mousemove/mouseup on .sidebar-resize-handle
// Add: touchstart/touchmove/touchend on same element
resizeHandle.addEventListener('touchstart', (e) => {
    startX = e.touches[0].clientX;
    startWidth = sidebar.offsetWidth;
    e.preventDefault();
}, { passive: false });
resizeHandle.addEventListener('touchmove', (e) => {
    const dx = e.touches[0].clientX - startX;
    sidebar.style.width = `${Math.max(68, Math.min(300, startWidth + dx))}px`;
    e.preventDefault();
}, { passive: false });
```

**Scrollable containers (add to existing CSS):**

`-webkit-overflow-scrolling: touch` is already present in `chat.css` (covers `.chat-messages`) and `chat-guided-generation.css`. Add only the remaining containers:

```css
/* Already present in chat.css — do NOT duplicate */
/* .chat-messages { -webkit-overflow-scrolling: touch; } */

/* Add these to layout.css or a new android.css partial */
.content-area,
.log-panel,
.settings-modal-body,
.chat-sessions-list {
  -webkit-overflow-scrolling: touch;
  overscroll-behavior-y: contain;
}

.chat-tab-bar {
  -webkit-overflow-scrolling: touch;
  overscroll-behavior-x: contain;
  scroll-snap-type: x proximity;
}
```

**Touch targets (`@media (pointer: coarse)`):**
```css
@media (pointer: coarse) {
  .sidebar-nav-item {
    min-height: 44px;
  }
  .btn-send,
  .btn-chat-tab-close,
  .user-menu-trigger,
  .chat-control-btn {
    min-height: 44px;
    min-width: 44px;
  }
}
```

**Swipe gesture for sidebar:**
```javascript
// Touch left-edge swipe on main content → open sidebar
let touchStartX = 0;
document.addEventListener('touchstart', (e) => {
    touchStartX = e.touches[0].clientX;
}, { passive: true });
document.addEventListener('touchend', (e) => {
    const dx = e.changedTouches[0].clientX - touchStartX;
    if (touchStartX < 30 && dx > 60) sidebar.classList.remove('collapsed');
    if (dx < -60 && !sidebar.classList.contains('collapsed')) {
        sidebar.classList.add('collapsed');
    }
}, { passive: true });
```

**Long-press for context menus:**
```javascript
// In chat-sidebar.js — add to session list items
let longPressTimer = null;
item.addEventListener('touchstart', () => {
    longPressTimer = setTimeout(() => openContextMenu(item), 500);
}, { passive: true });   // passive: true — no preventDefault needed
item.addEventListener('touchend', () => clearTimeout(longPressTimer), { passive: true });
item.addEventListener('touchmove', () => clearTimeout(longPressTimer), { passive: true });
```

**Passive event listener discipline (applies to all touch handlers added for Android):**
- Sidebar resize drag handlers: `{ passive: false }` — they call `e.preventDefault()` to suppress scroll during drag
- Swipe detection (touchstart/touchend): `{ passive: true }` — no scroll suppression needed
- Long-press timers: `{ passive: true }` — no scroll suppression needed
- Chrome logs a warning and scroll jank occurs if `preventDefault()` is called in a passive listener; the reverse (missing `passive: false`) silently blocks native scroll

**Mobile URL input optimization (add to connection setup form):**
```html
<!-- Connection setup endpoint fields -->
<input type="url" inputmode="url" autocorrect="off" autocapitalize="none"
       placeholder="http://192.168.2.16:8001">
```
Applies to: llama-server endpoint, remote agent URL. Without this, the iOS/Android soft keyboard shows the generic text keyboard and autocorrect mangles URLs.

**Chat keyboard avoidance:**
```javascript
// In chat input initialization
if (window.visualViewport) {
    window.visualViewport.addEventListener('resize', () => {
        const chatMessages = document.getElementById('chat-messages');
        if (chatMessages) chatMessages.scrollTop = chatMessages.scrollHeight;
    });
}
```

**Safe area insets (add to `tokens.css`):**
```css
:root {
  --safe-top: env(safe-area-inset-top, 0px);
  --safe-bottom: env(safe-area-inset-bottom, 0px);
  --safe-left: env(safe-area-inset-left, 0px);
  --safe-right: env(safe-area-inset-right, 0px);
}

.top-nav {
  padding-top: calc(var(--gap-sm) + var(--safe-top));
}

.chat-input-area {
  padding-bottom: calc(var(--gap-sm) + var(--safe-bottom));
}
```

### 6.4 WebView Configuration

```kotlin
webView.settings.apply {
    javaScriptEnabled = true
    domStorageEnabled = true          // localStorage used by UI settings
    databaseEnabled = true
    allowFileAccessFromFileURLs = false
    allowUniversalAccessFromFileURLs = false
    cacheMode = WebSettings.LOAD_DEFAULT
    mediaPlaybackRequiresUserGesture = false
    useWideViewPort = true
    loadWithOverviewMode = true
    setSupportZoom(true)
    builtInZoomControls = false
    displayZoomControls = false
}
// Chrome DevTools remote debugging for debug builds
if (BuildConfig.DEBUG) {
    WebView.setWebContentsDebuggingEnabled(true)
}
```

**Network security config** (`res/xml/network_security_config.xml`):
```xml
<network-security-config>
    <base-config cleartextTrafficPermitted="false" />
    <!-- Allow cleartext to localhost (Rust warp server) -->
    <domain-config cleartextTrafficPermitted="true">
        <domain includeSubdomains="false">127.0.0.1</domain>
    </domain-config>
    <!-- Allow cleartext to local llama-server (trusted LAN) -->
    <!-- Configure per-user in settings; add known LAN ranges or specific IPs -->
</network-security-config>
```

CSP in `warp-helmet` configuration must allow `ws://127.0.0.1:PORT` for WebSocket connections when the `android` feature is active.

### 6.5 Dark/Light Theme

The existing theme system uses `[data-theme="light"]` on `<html>` with localStorage persistence. Add `prefers-color-scheme` detection on initial load (JS):

```javascript
// In theme initialization — add before existing localStorage check
if (!localStorage.getItem('llm-theme')) {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    document.documentElement.setAttribute('data-theme', prefersDark ? 'dark' : 'light');
}
window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
    if (!localStorage.getItem('llm-theme')) {
        document.documentElement.setAttribute('data-theme', e.matches ? 'dark' : 'light');
    }
});
```

The dark theme (`--color-bg: #0f1115`) is excellent for OLED — true black backgrounds save measurable battery on the Z Fold 6's AMOLED displays.

### 6.6 Performance Considerations for Mobile WebView

**`backdrop-filter`:** The card glassmorphism (`backdrop-filter: blur(16px) saturate(180%)`) is GPU-composited. Z Fold 6 handles this fine, but budget devices will not:
```css
@media (pointer: coarse) and (max-resolution: 2.5dppx) {
  .widget-card {
    backdrop-filter: none;
    background: var(--surface-card-base);
  }
}
```

**Tokio thread pool:** Limit worker threads on Android to avoid competing with the foreground app:
```rust
#[cfg(target_os = "android")]
let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(2)
    .enable_all()
    .build()?;
```

**SVG sparklines and CSS animations** are lightweight (no canvas, no WebGL) and perform well on Chromium/Android WebView without changes.

---

## 7. Metrics Backend Strategy

### 7.1 The Existing Platform Detection Pattern

The GPU backend uses a `detect_backend(force: &str)` function dispatching to `RocmBackend`, `NvidiaBackend`, `AppleBackend`, or `DummyBackend`. The `DummyBackend` returns empty metrics with appropriate `Availability` codes — already the correct pattern for unsupported platforms.

The system metrics backend does platform dispatch inline via `#[cfg]` blocks. Both patterns are already established; Android adds new arms to each.

### 7.2 Android CPU Metrics

| Metric | Source | Reliability |
|---|---|---|
| CPU model name | `/proc/cpuinfo` `Hardware:` or `model name:` | ✅ Reliable |
| CPU core count | `sysinfo::System::cpus().len()` | ✅ Reliable |
| CPU load % | `/proc/stat` via `sysinfo` | ✅ Reliable |
| CPU frequency (current) | `/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq` | ⚠️ SELinux-dependent |
| CPU frequency (max) | `/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq` | ⚠️ Same |

`sysinfo 0.39` handles `/proc/stat` on Android. Use it directly for load. For frequency:
```rust
#[cfg(target_os = "android")]
fn read_cpu_freq_mhz() -> Option<u32> {
    std::fs::read_to_string(
        "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq"
    )
    .ok()
    .and_then(|s| s.trim().parse::<u32>().ok())
    .map(|khz| khz / 1000)
    .or_else(read_cpuinfo_mhz)
}
```

### 7.3 Android Thermal Metrics

| Source | Reliability |
|---|---|
| `/sys/class/thermal/thermal_zone*/temp` | ⚠️ Some zones readable, SELinux blocks others |
| BatteryManager.EXTRA_TEMPERATURE | ✅ Always available (tenths of °C) |
| Android ThermalManager API (API 29+) | ⚠️ Returns normalized STATUS enum (0-6), not raw °C |

Strategy: enumerate thermal zones at startup; record which are readable; report those with correct names; set `SensorUnavailable` for the rest. Always include battery temperature (via JNI BatteryManager callback) as the one guaranteed thermal reading.

### 7.4 Android GPU Metrics (Phase 5)

GPU metrics on Android without root are vendor-specific. The Z Fold 6 uses Snapdragon 8 Gen 3 with **Adreno 750**.

**Adreno (Qualcomm) sysfs paths:**
- `/sys/class/kgsl/kgsl-3d0/gpu_clock` — current GPU clock in Hz
- `/sys/class/kgsl/kgsl-3d0/gpubusy` — busy/total ticks
- `/sys/class/kgsl/kgsl-3d0/devfreq/cur_freq` — devfreq current frequency
- `/sys/class/kgsl/kgsl-3d0/max_gpuclk` — max clock

All accessed via standard file reads; `PermissionDenied` caught silently and mapped to `SensorUnavailable`.

**ARM Mali (Samsung Exynos variants):**
- `/sys/class/misc/mali0/device/utilization` — GPU utilization % (Samsung)
- `/sys/kernel/gpu/gpu_freq` — current clock (Samsung-specific)
- Highly firmware-dependent; treat all as best-effort.

**GPU VRAM on Android:** The Z Fold 6 uses unified LPDDR5X memory — there is no dedicated GPU VRAM. Report `availability = NotApplicable` for VRAM on Android, with a UI note that GPU and CPU share system memory. Show total RAM instead.

### 7.5 Battery as a First-Class Metric

Battery is a new metric class not present on any desktop platform. Add to the WebSocket push payload:

```rust
#[cfg(target_os = "android")]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct BatteryMetrics {
    pub level_percent: u8,
    pub is_charging: bool,
    pub temperature_celsius: f32,  // BatteryManager.EXTRA_TEMPERATURE / 10.0
    pub voltage_mv: u32,
    pub health: String,            // "good" | "overheat" | "dead" | "cold"
    pub plugged: String,           // "ac" | "usb" | "wireless" | "none"
}
```

Serialize as `battery` field in the top-level WebSocket push object. The JS handler in `dashboard-ws.js` renders it into the system metrics card if present.

### 7.6 Platform Metric Coverage Comparison

| Metric | macOS | Linux | Windows | Android |
|---|---|---|---|---|
| CPU model | sysctl | /proc/cpuinfo | WMI | /proc/cpuinfo |
| CPU load % | sysinfo | sysinfo | sysinfo | sysinfo |
| CPU clock | sysctl | /proc/cpuinfo | WMI | /sys/…/cpufreq |
| CPU temp | sysctl / mactop | /sys/class/thermal | sensor_bridge (LHM) | /sys/class/thermal (partial) |
| GPU name | mactop | nvidia-smi / rocm-smi | nvidia-smi / WMI | sysfs Phase 5 |
| GPU temp | mactop | nvidia-smi / rocm-smi | nvidia-smi / LHM | sysfs Phase 5, limited |
| GPU load | mactop | nvidia-smi / rocm-smi | nvidia-smi | sysfs Phase 5, limited |
| GPU VRAM | mactop (unified) | nvidia-smi / rocm-smi | nvidia-smi | NotApplicable (unified RAM) |
| RAM used/total | sysinfo | sysinfo | sysinfo | sysinfo |
| Battery | N/A | N/A | N/A | BatteryManager ✅ |

---

## 8. Security Model

### 8.1 Alignment with Existing Architecture

The existing security model (documented in AGENTS.md §Security Requirements) applies unchanged on Android. All token comparisons use `subtle::ConstantTimeEq`. All randomness uses `getrandom::getrandom()`. All user-data API endpoints require `api-token` Bearer auth. Destructive operations require `db-admin-token`.

### 8.2 Secret Storage: AndroidKeyStore

All secrets use AndroidKeyStore-backed EncryptedSharedPreferences:

| Secret | Android Storage |
|---|---|
| Bearer token (api-token) | EncryptedSharedPreferences via AndroidKeyStore |
| Remote agent token | EncryptedSharedPreferences |
| Client TLS certificate private key | AndroidKeyStore key entry |
| Server CA certificate (public) | EncryptedSharedPreferences |

```kotlin
// KeystoreHelper.kt
class KeystoreHelper(context: Context) {
    private val masterKey = MasterKey.Builder(context)
        .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
        .build()
    private val prefs = EncryptedSharedPreferences.create(
        context, "llama_monitor_secrets", masterKey,
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
    )
    fun put(key: String, value: String) = prefs.edit().putString(key, value).apply()
    fun get(key: String): String? = prefs.getString(key, null)
    fun delete(key: String) = prefs.edit().remove(key).apply()
}
```

The Kotlin wrapper is exposed to Rust via JNI (`get_secret`, `put_secret`, `delete_secret` in `src/android/keystore.rs`).

### 8.3 Data at Rest

Chat database lives in `/data/data/com.llamamonitor.app/files/llama-monitor/chat.db` — app-private storage, mode 0600, inaccessible to other apps without root. This is equivalent to the Unix `harden_file_permissions()` hardening. The existing `ChatStorage::backup()` API works unchanged on Android.

SQLite encryption (SQLCipher): defer to Phase 4. App-private storage provides adequate protection for Phase 1-3.

### 8.4 Certificate Pinning

Implement TOFU (Trust On First Use) for the remote agent's HTTPS certificate, extending the existing `ssh-known-hosts.json` model to HTTPS:

**Flow:**
1. First connection: store agent's CA fingerprint
2. Subsequent connections: verify fingerprint matches
3. Mismatch: prompt user to re-scan and trust new key (like SSH host key verification)

This is architecturally identical to the SSH host key model already implemented.

### 8.5 LAN llama-server Security

Per the existing security model: `http://192.168.2.16:8001` is treated as trusted on the local network. No auth wrapping from llama-monitor. The `network_security_config.xml` must explicitly permit cleartext to the configured llama-server address. This should be a user-configurable setting, not hardcoded.

---

## 9. Cross-Compilation Strategy

### 9.1 Target

Primary: `aarch64-linux-android` (arm64-v8a, Android API 30+)

Minimum Android API: **30 (Android 11)**. Rationale: API 30 provides reliable `backdrop-filter` in WebView, ThermalManager API, and better Chromium engine features. The Z Fold 6 ships Android 14 (API 34).

### 9.2 Toolchain

```bash
# Install NDK r27c (LTS)
ANDROID_NDK_ROOT=/opt/android-ndk/r27c

# Add Rust target
rustup target add aarch64-linux-android

# Install cargo-ndk
cargo install cargo-ndk --locked
```

**`.cargo/config.toml` addition** (alongside existing macOS/Linux targets):
```toml
[target.aarch64-linux-android]
linker = "/opt/android-ndk/r27c/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android30-clang"
ar     = "/opt/android-ndk/r27c/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
```

In practice, `cargo-ndk` sets these from `ANDROID_NDK_ROOT` automatically. The config.toml entry is a CI fallback.

### 9.3 Build Command

```bash
cargo ndk \
  --target aarch64-linux-android \
  --platform 30 \
  -o android/app/src/main/jniLibs \
  build \
  --release \
  --no-default-features \
  --features android
```

Output: `android/app/src/main/jniLibs/arm64-v8a/libllama.so`

### 9.4 Dependency Build Analysis

| Crate | Android cross-compile | Resolution |
|---|---|---|
| `tokio 1.52` | ✅ Works | `tokio::signal` SIGTERM works on Android; primary shutdown via JNI |
| `warp 0.4` | ✅ Works | Pure Rust hyper-based |
| `reqwest 0.13` | ✅ Works | `default-features = false`, uses rustls |
| `rustls 0.23` | ✅ Works | Pure Rust |
| `serde/serde_json` | ✅ Works | Pure Rust |
| `argon2 0.6.0-rc.8` | ✅ Works | Pure Rust |
| `rand 0.10` | ✅ Works | `getrandom` supports aarch64-linux-android |
| `sysinfo 0.39` | ✅ Works | Explicit Android support in changelog |
| `rusqlite 0.39` + `bundled` | ✅ Works with NDK | `cc` crate picks up NDK C compiler from env; SQLite is pure ANSI C |
| `dirs 6.0.0` | ⚠️ Empty paths | Override with `#[cfg(target_os = "android")]` path function |
| `ssh2 0.9.5` | ✅ Feature-gated out | `--no-default-features` excludes `ssh-control` feature |
| `wry/winit/gtk/tray-icon` | ✅ All excluded | `--no-default-features` + platform cfg gates |
| `tempfile 3.27` | ❌ Compile error | Depends on `rustix 1.1.4` which fails on Android (unresolved `linux_raw_sys` refs); PR #1577 open since Feb 2026, not merged. Use `#[cfg(target_os = "android")]` alternative |

**Additional note:** 10+ tempfile uses in agent.rs (remote agent install flow). Consider feature-gating this flow on Android.
| `clap 4` | N/A — not in cdylib | CLI parsing not needed in library mode |

### 9.5 rusqlite Bundled on Android: The Mechanics

`rusqlite` with `bundled` feature includes `libsqlite3-sys` with `bundled` enabled, which uses the `cc` crate to compile SQLite's amalgamation (single-file ANSI C) from source. The `cc` crate reads `CC` and `AR` from environment variables that `cargo-ndk` sets automatically:

```
CC  = aarch64-linux-android30-clang
AR  = llvm-ar
CFLAGS = --target=aarch64-linux-android30 --sysroot=...
```

SQLite's amalgamation has zero OS-specific code beyond standard POSIX — it compiles cleanly for Android. This is a tested, documented path in the rusqlite ecosystem.

---

## 10. JNI Bridge Design

### 10.1 Rust Entry Points

```rust
// src/android/mod.rs
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint, jlong};

static SHUTDOWN_TOKEN: OnceLock<CancellationToken> = OnceLock::new();
static IS_RUNNING: AtomicBool = AtomicBool::new(false);
static POLL_INTERVAL_TX: OnceLock<watch::Sender<u64>> = OnceLock::new();
static BATTERY_STATE: OnceLock<Mutex<BatteryMetrics>> = OnceLock::new();

#[cfg(feature = "android")]
#[no_mangle]
pub extern "C" fn Java_com_llamamonitor_app_RustLib_startServer(
    env: JNIEnv, _class: JClass,
    port: jint,
    config_dir: JString,
    agent_url: JString,
    agent_token: JString,
) -> jint {
    let config_dir: String = env.get_string(&config_dir)
        .expect("config_dir must be valid UTF-8").into();
    // Set Android paths before anything else
    android::paths::set_files_dir(PathBuf::from(&config_dir));

    let token = CancellationToken::new();
    SHUTDOWN_TOKEN.set(token.clone()).ok();
    IS_RUNNING.store(true, Ordering::Relaxed);

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)  // Limit on Android
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(async {
            run_headless(port as u16, token).await;
        });
        IS_RUNNING.store(false, Ordering::Relaxed);
    });
    0
}

#[cfg(feature = "android")]
#[no_mangle]
pub extern "C" fn Java_com_llamamonitor_app_RustLib_stopServer(
    _env: JNIEnv, _class: JClass
) {
    if let Some(token) = SHUTDOWN_TOKEN.get() {
        token.cancel();
    }
}

#[cfg(feature = "android")]
#[no_mangle]
pub extern "C" fn Java_com_llamamonitor_app_RustLib_setPollInterval(
    _env: JNIEnv, _class: JClass, interval_ms: jlong
) {
    if let Some(tx) = POLL_INTERVAL_TX.get() {
        let _ = tx.send(interval_ms as u64);
    }
}

#[cfg(feature = "android")]
#[no_mangle]
pub extern "C" fn Java_com_llamamonitor_app_RustLib_isRunning(
    _env: JNIEnv, _class: JClass
) -> jboolean {
    IS_RUNNING.load(Ordering::Relaxed) as jboolean
}

#[cfg(feature = "android")]
#[no_mangle]
pub extern "C" fn Java_com_llamamonitor_app_RustLib_updateBatteryMetrics(
    _env: JNIEnv, _class: JClass,
    level: jint, temp_tenths: jint,
    is_charging: jboolean, voltage_mv: jint, health: jint
) {
    if let Some(state) = BATTERY_STATE.get() {
        let mut m = state.lock().unwrap();
        m.level_percent = level as u8;
        m.temperature_celsius = temp_tenths as f32 / 10.0;
        m.is_charging = is_charging != 0;
        m.voltage_mv = voltage_mv as u32;
    }
}
```

### 10.2 Kotlin Declarations

```kotlin
// RustLib.kt
object RustLib {
    init { System.loadLibrary("llama") }

    external fun startServer(
        port: Int, configDir: String,
        agentUrl: String?, agentToken: String?
    ): Int
    external fun stopServer()
    external fun setPollInterval(intervalMs: Long)
    external fun isRunning(): Boolean
    external fun updateBatteryMetrics(
        level: Int, tempTenths: Int,
        isCharging: Boolean, voltageMv: Int, health: Int
    )
}
```

---

## 10A. Signal Handling on Android

### 10A.1 The Problem

The existing shutdown handler in `src/main.rs:762-785` uses Unix signals:

```rust
#[cfg(unix)]
{
    let mut sigint = tokio::signal::unix::signal(SignalKind::interrupt()).unwrap();
    let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = sigint.recv() => {},
        _ = sigterm.recv() => {},
    }
}
```

**Android is Unix-like but signal handling is limited in apps.** The foreground service lifecycle handles shutdown via JNI `stopServer()` call, not Unix signals.

### 10A.2 Solution

Android uses the JNI `stopServer()` entry point to trigger shutdown:

```rust
// src/android/mod.rs
#[cfg(feature = "android")]
#[no_mangle]
pub extern "C" fn Java_com_llamamonitor_app_RustLib_stopServer(
    _env: JNIEnv, _class: JClass
) {
    if let Some(token) = SHUTDOWN_TOKEN.get() {
        token.cancel();
    }
}
```

The Rust side should ignore Unix signals entirely on Android and rely solely on the cancellation token.

### 10A.3 Code Change Required

**Critical:** `#[cfg(unix)]` matches Android (Android is Unix-like), so the existing signal handler **compiles fine** for Android — but **panics at runtime** when Tokio tries to register Unix signals that do not function in Android app processes. This is a runtime crash, not a compile error, and will not be caught by `cargo check`. It must be fixed before the first Android test run.

```rust
// In src/main.rs shutdown handler
#[cfg(all(unix, not(target_os = "android")))]
{
    // Existing Unix signal handling
}

#[cfg(target_os = "android")]
{
    // Wait for JNI shutdown token cancellation
    SHUTDOWN_TOKEN.get().unwrap().cancelled().await;
}
```

---

## 10B. Config Path Injection Complexity

### 10B.1 The Problem

The document states: *"Override with `#[cfg(target_os = "android")]` path function using JNI-provided `filesDir`"*

**Reality:** This is more complex. The config path comes from CLI parsing (`args.config_dir`), which doesn't exist in cdylib mode:

```rust
// src/config.rs:458-461
let config_dir = args.config_dir.unwrap_or_else(|| {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("llama-monitor")
});
```

### 10B.2 Solution

The JNI `startServer` function must inject the config path directly into `AppState` initialization, not just override `dirs::home_dir()`:

```rust
// src/android/mod.rs
#[no_mangle]
pub extern "C" fn Java_com_llamamonitor_app_RustLib_startServer(
    env: JNIEnv, _class: JClass,
    port: jint,
    config_dir: JString,  // ← Inject path directly
    agent_url: JString,
    agent_token: JString,
) -> jint {
    let config_dir: String = env.get_string(&config_dir)
        .expect("config_dir must be valid UTF-8").into();
    
    // Set Android paths before anything else
    android::paths::set_files_dir(PathBuf::from(&config_dir));
    
    // ... continue with server startup
}
```

### 10B.3 Complete Path Override

All paths derived from `config_dir` must use the injected value:
- `chat.db` — chat database
- `presets.json` — model presets
- `templates.json` — persona templates
- `ui-settings.json` — UI settings
- `sessions.json` — session state
- `auth-config.json` — auth configuration
- `tls-config.json` — TLS configuration
- `agent-tokens.json` — agent token management
- `encryption-key` — at-rest encryption key
- `api-token` — API bearer token
- `db-admin-token` — DB admin token

---

## 10C. File Permissions Hardening on Android

### 10C.1 Current State

`harden_file_permissions()` in `src/config.rs` has Unix (`#[cfg(unix)]`) and Windows (`#[cfg(windows)]`) paths only.

### 10C.2 Android Behavior

Android uses app sandboxing — files in `/data/data/package/files/` are already inaccessible to other apps without root. The Unix permission bits are less relevant but still useful for defense-in-depth.

### 10C.3 Recommendation

Keep the Unix path active (Android is Unix-like) but log a warning that app sandboxing provides the primary protection:

```rust
#[cfg(target_os = "android")]
fn harden_file_permissions(path: &Path) {
    // App sandboxing provides primary protection
    // Set permissions as defense-in-depth
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(mut perms) = std::fs::metadata(path).map(|m| m.permissions()) {
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}
```

---

## 11. Foreground Service and Background Execution

### 11.1 Service Architecture

Android will kill any background process not protected by a foreground service. The Rust tokio runtime must live inside a foreground service:

```kotlin
// LlamaMonitorService.kt
class LlamaMonitorService : Service() {

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(NOTIFICATION_ID, buildNotification())

        if (!RustLib.isRunning()) {
            val ks = KeystoreHelper(this)
            RustLib.startServer(
                port = 17778,
                configDir = filesDir.absolutePath + "/llama-monitor",
                agentUrl = ks.get("agent_url"),
                agentToken = ks.get("agent_token")
            )
        }

        registerBatteryReceiver()
        registerScreenReceiver()
        return START_STICKY
    }

    override fun onDestroy() {
        RustLib.stopServer()
        unregisterReceivers()
        super.onDestroy()
    }

    private val batteryReceiver = object : BroadcastReceiver() {
        override fun onReceive(ctx: Context, intent: Intent) {
            val level = intent.getIntExtra(BatteryManager.EXTRA_LEVEL, 0)
            val temp  = intent.getIntExtra(BatteryManager.EXTRA_TEMPERATURE, 0)
            val plugged = intent.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0)
            val health  = intent.getIntExtra(BatteryManager.EXTRA_HEALTH, 0)
            val voltage = intent.getIntExtra(BatteryManager.EXTRA_VOLTAGE, 0)
            RustLib.updateBatteryMetrics(level, temp, plugged != 0, voltage, health)
            // Slow polling when on battery
            RustLib.setPollInterval(if (plugged != 0) 500L else 2000L)
        }
    }

    private val screenReceiver = object : BroadcastReceiver() {
        override fun onReceive(ctx: Context, intent: Intent) {
            when (intent.action) {
                Intent.ACTION_SCREEN_OFF -> RustLib.setPollInterval(10_000L)
                Intent.ACTION_SCREEN_ON  -> RustLib.setPollInterval(500L)
            }
        }
    }

    private fun buildNotification(): Notification {
        createNotificationChannel()
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Llama Monitor")
            .setContentText("Monitoring active")
            .setSmallIcon(R.drawable.ic_notification)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setContentIntent(openAppPendingIntent())
            .addAction(R.drawable.ic_stop, "Stop",
                PendingIntent.getService(this, 0,
                    Intent(this, LlamaMonitorService::class.java)
                        .setAction(ACTION_STOP),
                    PendingIntent.FLAG_IMMUTABLE))
            .build()
    }
}
```

**AndroidManifest.xml permissions:**
```xml
<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC" />
<uses-permission android:name="android.permission.POST_NOTIFICATIONS" />
<uses-permission android:name="android.permission.INTERNET" />
<uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />

<service
    android:name=".LlamaMonitorService"
    android:foregroundServiceType="dataSync"
    android:exported="false" />
```

### 11.2 Doze Mode Handling

Android Doze mode (screen off + unplugged + stationary) limits network access and defers background work. For llama-monitor:
- Remote agent polling can be deferred — metrics are non-critical when screen is off
- Use `JobScheduler` with `setRequiredNetworkType(NETWORK_TYPE_ANY)` for periodic sync during Doze maintenance windows
- The foreground service with `dataSync` type has an exemption for active data connections

---

## 12. CI/CD Integration

### 12.0 Runner Repo: Current State (as of 2026-05-24)

The `llama-monitor-runner` repo (`ghcr.io/nmorgowicz-org/llama-monitor-runner:latest`) is the custom ARC runner image for all llama-monitor CI. Understanding its current state is required context before implementing §12.1–12.4.

**What is already in the runner image:**

| Component | Status | Detail |
|---|---|---|
| Rust stable toolchain | ✅ Installed (UID 1000) | `RUSTUP_HOME=/opt/rustup`, `CARGO_HOME=/opt/cargo` |
| `aarch64-unknown-linux-gnu` Rust target | ✅ Pre-installed | Linux ARM64 cross-compilation (via cross-rs) |
| `aarch64-apple-darwin` Rust target | ✅ Pre-installed | macOS ARM64 via osxcross |
| `x86_64-pc-windows-gnu` Rust target | ✅ Pre-installed | Windows cross-compilation |
| `rust-src` component | ✅ Pre-installed | Prevents concurrent rustup download races |
| `cross-rs` (from git) | ✅ Installed | Used for Linux ARM64 and Windows builds |
| `aarch64-linux-gnu` Ubuntu sysroot | ✅ Present | GTK/WebKit ARM64 headers — for **Linux**, NOT Android |
| `unzip` | ✅ Installed | In the apt package list |
| `curl` | ✅ Installed | Used throughout; `wget` is **not** available |
| Android NDK | ❌ Not present | Must be added to Dockerfile |
| `aarch64-linux-android` Rust target | ❌ Not pre-installed | Must be added to Dockerfile |
| `cargo-ndk` | ❌ Not installed | Must be added to Dockerfile |
| Android SDK (`ANDROID_HOME`) | ❌ Not present | Required only for `gradlew assembleRelease` (APK assembly) |

**Critical distinction:** `aarch64-unknown-linux-gnu` (Linux ARM64) and `aarch64-linux-android` (Android ARM64) are different Rust targets despite sharing hardware. The Ubuntu `aarch64-linux-gnu` sysroot already in the Dockerfile provides GTK/WebKit headers for Linux cross-builds via cross-rs — it is **not** usable as an Android NDK sysroot.

**Runner scale sets in use:**

| Scale set name | Runners | CPU | Memory | Build cache |
|---|---|---|---|---|
| `arc-llama-monitor` | 0–4 | 2–4 cores | 2–5 GiB | `/var/cache/builds` (20 GiB emptyDir) |
| `arc-llama-monitor-fast` | 0–2 | 4–8 cores | 4–8 GiB | `/var/cache/builds` (30 GiB emptyDir) |

`CARGO_HOME` is overridden at pod runtime to `/var/cache/builds/cargo` by the pod spec env. CI jobs should use `CARGO_TARGET_DIR: /var/cache/builds/target` for Rust build artifacts — `/cache/target` does not exist in the pod spec.

**Image build pipeline:**

- Trigger: push to `Dockerfile` or `.github/workflows/runner-image.yml`; weekly Sunday 03:00 UTC; `workflow_dispatch`
- Build runner: `arc-general-docker` (stock ARC runner with DinD — the custom image is not used to build itself)
- Push targets: `ghcr.io/nmorgowicz-org/llama-monitor-runner:latest` + `:<sha>`
- Base image: `ghcr.io/nmorgowicz-org/osxcross-base:darwin25.1` (private)
- Tag cleanup: keeps 5 most recent versions (cleanup job runs on `arc-general`)

### 12.1 Extending the arc-runner Image (Precise Dockerfile Diff)

The following changes to `llama-monitor-runner/Dockerfile` add Android NDK, the Android Rust target, and `cargo-ndk`. All insertions are relative to the current `main` branch.

**Step 1 — NDK installation (as `root`, after the existing aarch64 sysroot COPY/symlink section, before the runner agent install block):**

```dockerfile
# ── 1c. Android NDK r27c ──────────────────────────────────────────────────────
# Owned by UID 1000 so cargo-ndk (running as the runner user) can read the toolchain.
ARG NDK_VERSION=r27c
RUN curl -fsSL \
      "https://dl.google.com/android/repository/android-ndk-${NDK_VERSION}-linux.zip" \
      -o /tmp/android-ndk.zip \
    && unzip -q /tmp/android-ndk.zip -d /opt \
    && mv "/opt/android-ndk-${NDK_VERSION}" /opt/android-ndk \
    && rm /tmp/android-ndk.zip \
    && chown -R 1000:1000 /opt/android-ndk

ENV ANDROID_NDK_ROOT=/opt/android-ndk
```

**Step 2 — Android Rust target (add `aarch64-linux-android` to the existing `rustup target add` block, as `USER 1000:1000`):**

```dockerfile
# Targets: native + macOS + Android (cross-rs handles Linux ARM64 and Windows at runtime)
RUN rustup target add \
    x86_64-unknown-linux-gnu \
    aarch64-unknown-linux-gnu \
    x86_64-pc-windows-gnu \
    aarch64-apple-darwin \
    aarch64-linux-android && \
    rustup component add rust-src
```

**Step 3 — `cargo-ndk` (add to the existing `cargo install cross` line, as `USER 1000:1000`):**

```dockerfile
# Install cross-rs and cargo-ndk
RUN cargo install cross --git https://github.com/cross-rs/cross \
    && cargo install cargo-ndk --locked
```

**Image size impact:** NDK r27c is ~1.3 GB zip → ~3.5 GB extracted (full multi-architecture toolchain). Estimated runner image size increase: ~3.5 GB (current image ~6–7 GB; new total ~10 GB). If image size becomes a concern, the NDK can be trimmed post-extract to remove non-ARM64 prebuilt binaries (reduces to ~800 MB–1 GB), but this is deferred to Phase 2.

**Triggering a rebuild:** Merging the Dockerfile changes to `llama-monitor-runner/main` triggers the `Build Runner Image` workflow automatically (push path filter on `Dockerfile`). Android CI jobs cannot run until `:latest` is updated with the new image.

### 12.2 CI Check Job (No NDK Required)

**Important:** The existing CI has complex path-filtering and concurrency rules. Android jobs must integrate with existing `paths-filter` logic and respect the `ready-to-test` label gating.

Add to `.github/workflows/ci.yml`:
```yaml
check-android:
  name: Check Android compatibility
  runs-on: arc-llama-monitor
  steps:
    - uses: actions/checkout@v4
    - name: cargo check for Android
      run: |
        cargo check \
          --target aarch64-linux-android \
          --no-default-features \
          --features android
```

`cargo check` resolves the dependency graph and type-checks without invoking the C linker. With `aarch64-linux-android` pre-installed in the image (per §12.1 Step 2), no `rustup target add` step is needed. `cargo check` for a non-native target does not invoke the NDK linker — it catches `#[cfg]` mistakes and Android-incompatible type usage without requiring the NDK at check time.

### 12.3 Release Build Job

Add to `.github/workflows/release.yml`:
```yaml
build-android:
  name: Build Android (aarch64)
  runs-on: arc-llama-monitor
  steps:
    - uses: actions/checkout@v4

    - name: Build Android cdylib
      run: |
        cargo ndk \
          --target aarch64-linux-android \
          --platform 30 \
          -o android/app/src/main/jniLibs \
          build --release \
          --no-default-features --features android
      env:
        ANDROID_NDK_ROOT: /opt/android-ndk          # set by Dockerfile ENV; listed explicitly for clarity
        CARGO_TARGET_DIR: /var/cache/builds/target  # pod build cache mount

    - name: Build APK
      run: cd android && ./gradlew assembleRelease
      env:
        ANDROID_HOME: /opt/android-sdk

    - uses: actions/upload-artifact@v4
      with:
        name: llama-monitor-android-aarch64
        path: android/app/build/outputs/apk/release/*.apk
```

**Android SDK gap:** `ANDROID_HOME` for `gradlew assembleRelease` requires Android SDK build tools (`aapt2`, `d8`, `zipalign`), which are **not** in the runner image. Two options for Phase 1:

- **Option A (recommended):** Split into two jobs — `build-android-lib` on `arc-llama-monitor` (produces `libllama.so` as a workflow artifact) and `build-android-apk` on `ubuntu-latest` GitHub-hosted runner (which has Android SDK pre-installed at `/usr/local/lib/android/sdk`) downloading the `.so` artifact and running `gradlew`.
- **Option B (Phase 2+):** Add Android SDK command-line tools to the runner image (~300 MB + platform packages). Appropriate once the APK build is stable and worth the added image maintenance.

### 12.4 AGENTS.md Update Required

Per the MANDATORY multi-platform rule, the pre-PR checklist in AGENTS.md must be updated to include:

**Exact text to add to AGENTS.md §Multi-Platform Compatibility:**

```markdown
| `aarch64-linux-android` | Android | ✅ Excluded |
```

**Exact text to add to pre-PR checklist:**

```markdown
# After any changes to src/, Cargo.toml, or any #[cfg] guards:
cargo check --target aarch64-linux-android --no-default-features --features android
```

**Exact text to add to Windows-specific Architecture table (for reference):**

```markdown
| Feature | Android Implementation | Status |
|---------|----------------------|--------|
| Tray popover | WebView2 via `wry` | Working — uses same WebView as Windows |
| CPU temperature | BatteryManager + thermal zones | Working — partial (SELinux may block some zones) |
| GPU metrics | Adreno kgsl sysfs / Mali sysfs | Working for Adreno (Phase 5) |
| File permissions | App sandboxing + Unix mode bits | Working — app sandboxing is primary protection |
| Signal handling | JNI stopServer() call | Working — Unix signals ignored |
| Dashboard open | WebView loads http://127.0.0.1:PORT | Working |
```

---

## 13. Phased Implementation Plan

> **Timeline note (2026-05-24):** Revised for 100% AI-agent development with Claude Code + Codex running in parallel. Debug cycles that bottleneck a single agent are absorbed — one agent compiles while another pre-writes the next fix. The bottleneck shifts from "waiting for compilation" to "waiting for physical device feedback."

### Phase 1: Build Infrastructure and Skeleton (Week 1)

**Revised from Weeks 1-2.** Waves A (Rust backends), B (JNI), and C (Gradle/Kotlin) overlap aggressively. One agent iterates on `cargo ndk build` errors while others pre-write fixes for known issues (tempfile/rustix workaround, rusqlite+NDK linking flags, JNI symbol naming). The 5-7 debug cycles that would take days for a single agent compress to hours when agents pipeline ahead of each other. Runner image rebuild is the only hard blocker (~1-2 hours).

1. Add `ssh-control` and `android` features to `Cargo.toml`; move `ssh2` under `ssh-control`
2. Create `src/android/mod.rs` — JNI entry points (start/stop/status/setPollInterval)
3. Create `src/android/paths.rs` — Android config dir override with direct JNI injection (see §10B)
4. Add `#[cfg(target_os = "android")]` arm to `certs_dir()` in `src/certs.rs` — reads from `android::paths::files_dir()` instead of `dirs::home_dir()` (see §4.2; this is separate from the config.rs override)
5. Add `#[cfg(target_os = "android")]` temp file alternative for all 10+ tempfile uses (avoids `tempfile` → `rustix` compile error)
6. Add `#[cfg(target_os = "android")]` GPU stub to `src/gpu/mod.rs` (DummyAndroidBackend) AND add early-exit to `is_apple_silicon()` before the `Command::new("sysctl")` call
7. Add `#[cfg(target_os = "android")]` guards to all 4 platform functions in `src/system.rs` (see §4.2 table)
8. Add signal handling override for Android (see §10A) — **this is a RUNTIME PANIC without the fix**, not just a compile issue; highest priority in Phase 1
9. Set up `android/` Gradle project with Activity + WebView layout (WebView loads `http://127.0.0.1:PORT/` from warp server, not `file://`)
10. Implement `LlamaMonitorService` (foreground service, notification)
11. Implement `KeystoreHelper.kt`
12. Add `cargo-ndk` task to Gradle; create `.cargo/config.toml` (file doesn't exist yet)
13. Add `check-android` job to `.github/workflows/ci.yml`
14. Update `AGENTS.md` pre-PR checklist with Android check command (exact text in §12.4)
15. Merge NDK + `aarch64-linux-android` + `cargo-ndk` changes to `llama-monitor-runner` and trigger image rebuild (prerequisite for jobs in steps 13 and Phase 1 release build)

**Note on multi-client enrollment (see §20):** Phase 1 delivers Android as a single secondary client. The full self-service enrollment API (letting Android pair without manual SSH) is tracked in §20 and targeted for Phase 4 alongside other security hardening. For Phase 1 testing, Android's CA can be manually deployed to RYNE.

**Before merging:** Run `cargo check --target x86_64-pc-windows-gnu` (always required), `cargo check --target aarch64-linux-android`, `cargo clippy -- -D warnings`, `cargo fmt`. Update `docs/reference/` with Android build instructions.

**Deliverable:** APK installs on Z Fold 6. Dashboard shows with remote agent metrics from ryne. Local Android metrics show correct `availability` codes (no silent nulls).

### Phase 2: Android System Metrics (Week 2)

**Revised from Weeks 3-4.** With Phase 1 collapsing to 1 week, Phase 2 starts immediately. Multiple agents write battery.rs, thermal.rs, BatteryMetrics struct, JS handlers, and adaptive polling in parallel. The only sequential dependency is validating JNI data passing end-to-end, which takes one feedback round.

1. Implement `src/android/battery.rs` — `updateBatteryMetrics` JNI receiver
2. Implement `src/android/thermal.rs` — enumerate `/sys/class/thermal/`, filter readable
3. Add `BatteryMetrics` struct; serialize as `battery` field in WebSocket push
4. Add JS handler for `data.battery` in `dashboard-ws.js`
5. Add battery row to System card in `dashboard-render.js`
6. Implement `[data-theme="light"]` overrides for battery UI elements
7. Implement adaptive polling (screen on/off broadcast receivers → `setPollInterval` JNI)
8. Update `js-module-baseline.json` if new JS modules added

**Deliverable:** System card shows CPU load, battery level/temp/charging state; polling adapts to screen state.

### Phase 3: Touch Interaction and Foldable UX (Weeks 3-5)

**Revised from Weeks 5-8.** CSS work collapses to a day with parallel agents. Touch JS and foldable detection written in parallel. Gesture thresholds start with proven Android conventions (500ms long-press, 60px swipe) requiring only 1 tuning round. Kotlin foldable detection written defensively with null handling for non-foldable devices.

1. Add `max-width: 420px` CSS breakpoint for folded cover display
2. Add touch drag handlers for sidebar resize
3. Add `-webkit-overflow-scrolling: touch` to all scrollable containers
4. Add `@media (pointer: coarse)` rules for 44dp minimum touch targets
5. Add `env(safe-area-inset-*)` CSS variables
6. Implement foldable state detection in Kotlin; dispatch `foldstatechange` event to WebView
7. Add `body.is-unfolded` two-panel CSS layout
8. Add swipe gesture for sidebar open/close
9. Add long-press context menus in chat sidebar
10. Implement `visualViewport` resize listener for keyboard avoidance
11. Add `prefers-color-scheme` observation in theme initialization
12. Add backdrop-filter disable media query for low-end devices
13. Run Playwright UI tests; update selectors if needed

**Deliverable:** App fully touch-operable; adapts to folded/unfolded states; chat input works with soft keyboard.

### Phase 4: Security Hardening and SSH Control Evaluation (Week 6)

**Revised from Weeks 7-8.** Security review items parallelized across agents. russh evaluation scoped to evaluation only — migration becomes Phase 6 if viable. AndroidKeyStore verification remains a human gate but runs in parallel with all other work.

1. Verify AndroidKeyStore integration in production — **human gate, runs in parallel, blocks sign-off**
2. Implement TOFU certificate pinning for remote agent
3. Add agent setup UI flow for Android (simplified settings wizard)
4. Evaluate `russh` as pure-Rust SSH replacement — **evaluation only; migration deferred to Phase 6 if viable**
5. Security review pass per AGENTS.md security checklist

**Deliverable:** Production-grade secret storage; certificate pinning; security review complete.

### Phase 5: Android GPU Metrics (Week 7, verification deferred)

**Revised from Weeks 9+.** Adreno and Mali sysfs readers written in parallel; verification deferred to post-merge.

1. Implement `src/gpu/android.rs` with Adreno kgsl sysfs reader — **AI writes code, paths unverified**
2. Add Mali sysfs reader for Exynos variants — **AI writes code, paths unverified**
3. ~~Test on Z Fold 6~~ → **Deferred to post-merge human task**
4. Add GPU clock to GPU dashboard card
5. Document Android GPU availability gaps in `docs/reference/android.md`

### Revised Timeline Summary

| Phase | Original | Revised | Delta | Primary Reason |
|-------|----------|---------|-------|----------------|
| Phase 1: Build Infrastructure | Weeks 1-2 | **Week 1** | -1 | Multiple agents pipeline debug cycles |
| Phase 2: System Metrics | Weeks 3-4 | **Week 2** | -2 | Parallel agents; starts immediately after Phase 1 |
| Phase 3: Touch & Foldable UX | Weeks 5-8 | **Weeks 3-5** | -3 | CSS parallelized; gesture thresholds use proven conventions |
| Phase 4: Security Hardening | Weeks 7-8 | **Week 6** | -2 | Security review items parallelized |
| Phase 5: GPU Metrics | Weeks 9+ | **Week 7** | deferred | Parallel Adreno/Mali readers; verification deferred |
| **Total** | **~9 weeks** | **~7 weeks** | **-22%** | Parallel huge-model agents absorb debug loops |

**Key assumptions:**
- Claude Code + Codex running concurrently; agents pipeline ahead of each other
- One person providing physical device feedback for touch gestures (async, ~1 day turnaround)
- Phase 1 → Phase 2 dependency is hard but Phase 1 is fast enough
- Gesture thresholds start with proven Android conventions (500ms long-press, 60px swipe)
- AndroidKeyStore verification remains a human gate but runs in parallel

---

## 14. Risks and Mitigations

| Risk | Probability | Mitigation |
|---|---|---|
| `tempfile` → `rustix` compile error on Android | High | Use `#[cfg(target_os = "android")]` alternative: create temp files manually in app's private storage; do not rely on `tempfile` crate for Android target |
| `rusqlite` + `bundled` fails NDK cross-compile | Low | Use specific NDK version r27c (tested); document in AGENTS.md |
| Tokio worker panics on missing Android syscall | Low | Limit `worker_threads(2)`; test on API 30+ |
| WebView blocks `ws://127.0.0.1` | Medium | Set `network_security_config.xml` correctly; test on API 30 emulator |
| `backdrop-filter` causes thermal throttle | Medium | Add `@media (pointer: coarse)` disable rule; Z Fold 6 Adreno 750 handles it |
| Adreno kgsl sysfs paths vary by firmware | High | Return `SensorUnavailable` gracefully; never crash on missing paths |
| `ssh-control` feature accidentally included in Android build | Medium | `cargo check --target aarch64-linux-android` in CI catches this |
| Foreground service killed by Doze mode | Medium | Use `foregroundServiceType="dataSync"`; reduce polling intervals; test on Doze-enabled device |
| warp port conflict (17778 already in use) | Low | Pick random port at startup; communicate to WebView via JNI |
| JNI global reference leaks → memory growth | **High** | Use `jni-rs` `GlobalRef` wrappers; profile with Android Studio memory profiler |
| New `#[cfg(target_os = "android")]` breaks Windows build | Medium | Mandatory `cargo check --target x86_64-pc-windows-gnu` in pre-PR checklist |
| `dirs 6` empty path not caught → chat DB in wrong location | **High** | `src/android/paths.rs` override; add test asserting non-empty config path on Android |
| Runner image rebuild required before any Android CI runs | Medium | Merge `llama-monitor-runner` Dockerfile changes first; coordinate with Phase 1 step 12 |
| **Signal handling RUNTIME PANIC on Android** | **High** | **`#[cfg(unix)]` matches Android and compiles fine but panics at runtime; fix in Phase 1 task 8 before any device test (see §10A)** |
| **Config path injection fails** | **Medium** | **Inject path directly via JNI; don't rely on `dirs::home_dir()` (see §10B)** |
| **`certs_dir()` uses wrong path on Android** | **High** | **`src/certs.rs:22` has its own `dirs::home_dir()` call independent of config.rs; both must be overridden (see §4.2)** |
| **tempfile usage more extensive than expected** | **Medium** | **Audit all 10+ uses; many in remote agent install flow (may not be needed on Android)** |
| **Android client cert not trusted by RYNE** | **High** | **Multi-client enrollment API not yet implemented; Android CA must be manually deployed for Phase 1 testing (see §20)** |
| **`sysctl` subprocess spawned on Android** | **Low** | **`is_apple_silicon()` in gpu/mod.rs calls `Command::new("sysctl")` without Android guard; add early-exit (see §4.2)** |
| **Touch drag handlers block scroll (missing passive flag)** | **Medium** | **All new touch handlers must specify `passive: false` or `passive: true` explicitly; wrong passive setting causes scroll jank or Chrome warnings** |

---

## 15. Files Changed Summary

**Note:** `.cargo/config.toml` is listed under "Modified Files" but is actually a **new file** that doesn't exist yet. It must be created as part of Phase 1.

### New Files
```
src/android/mod.rs          — JNI entry points
src/android/battery.rs      — BatteryManager JNI receiver
src/android/thermal.rs      — /sys/class/thermal reader
src/android/paths.rs        — Android config directory
src/android/keystore.rs     — AndroidKeyStore JNI bridge
src/android/tempfile.rs     — Android temp file alternative (avoids rustix compile error)
src/gpu/android.rs          — Android GPU backend

.cargo/config.toml          — Android linker config (NEW — does not exist yet)

android/ (entire Gradle project)
├── app/src/main/java/com/llamamonitor/app/
│   ├── MainActivity.kt
│   ├── LlamaMonitorService.kt
│   ├── RustLib.kt
│   └── KeystoreHelper.kt
├── app/src/main/res/xml/network_security_config.xml
├── app/src/main/AndroidManifest.xml
├── app/build.gradle
└── settings.gradle

docs/reference/android.md   — Android build and deployment reference
```

### Modified Files
```
Cargo.toml                   — ssh-control + android features; ssh2 optional
src/certs.rs                 — certs_dir() Android path override (NEW — see §4.2)
src/gpu/mod.rs               — Android backend dispatch + is_apple_silicon() early-exit
src/system.rs                — Android metric collection paths (4 functions need guards)
src/config.rs                — Android paths override
src/agent.rs                 — #[cfg(target_os = "android")] temp file alternative (avoids tempfile→rustix)
src/main.rs                  — Android signal handling override — RUNTIME PANIC without this fix (see §10A)
static/js/features/dashboard-ws.js     — Handle data.battery field
static/js/features/dashboard-render.js — Render battery in system card
static/css/layout.css        — max-width: 420px global breakpoint for folded display (NOT the chat.css component rule)
static/css/tokens.css        — env(safe-area-inset-*) variables
static/js/                   — passive touch event listeners; type="url" form inputs; visualViewport listener
.github/workflows/ci.yml     — check-android job
.github/workflows/release.yml — build-android job
AGENTS.md                   — Android in multi-platform checklist; Android pre-PR check
```

### New Files (not modifications)
```
.cargo/config.toml           — aarch64-linux-android linker config (NEW — does not exist yet)
```

### llama-monitor-runner Changes Required (separate repo)
```
Dockerfile                   — NDK r27c install; aarch64-linux-android target; cargo-ndk
```

---

## 16. Decisions Required

1. **SSH control for Phase 4:** Relay vs. evaluate `russh` (pure-Rust SSH2)? `russh` would eliminate OpenSSL from the entire project.
2. **App package name:** `com.llamamonitor.app` or another namespace?
3. **Distribution method:** Sideload (`.apk` artifact from CI) vs. future Play Store consideration? Given the app's nature, sideload is appropriate.
4. **SSH known-hosts + TOFU cert pinning:** Extend the existing `ssh-known-hosts.json` model to HTTPS certs for the remote agent, or use a separate cert store?
5. **APK build job split (Phase 1):** Confirm Option A (split `build-android-lib` on `arc-llama-monitor` + `build-android-apk` on `ubuntu-latest`) vs. adding Android SDK to the runner image now.
6. **NDK image size:** Accept ~3.5 GB increase to runner image, or implement a post-extract trim to reduce to ~800 MB?
7. **`tempfile` replacement strategy:** Which code paths use `tempfile`? Audit and add `#[cfg(target_os = "android")]` alternatives for each (manual temp file creation in app's private storage).
8. **Remote agent install flow on Android:** Most `tempfile` uses are in the remote agent install flow. Is this flow needed on Android at all? If not, can it be feature-gated out?
9. **Signal handling:** Confirm JNI `stopServer()` for shutdown instead of Unix signals (see §10A).
10. **Config path injection:** Confirm direct JNI injection of config path instead of overriding `dirs::home_dir()` (see §10B).
11. **File permissions:** Accept app sandboxing as primary protection with Unix mode bits as defense-in-depth (see §10C)?
12. **CLI feature gating:** Add `cli` feature to gate `clap` crate on Android? Deferred to Phase 2+.

---

## 17. Pre-Phase 1 Verification Checklist

Complete these steps before starting Phase 1 implementation:

### 17.1 Codebase Preparation

- [ ] **Merge `llama-monitor-runner` Dockerfile changes** (NDK, Android target, cargo-ndk)
- [ ] **Create `.cargo/config.toml`** (file doesn't exist yet)
- [ ] **Add `ssh-control` and `android` features to Cargo.toml**
- [ ] **Verify Windows still compiles**: `cargo check --target x86_64-pc-windows-gnu --no-default-features`
- [ ] **Audit all `tempfile` uses** — determine which are needed for Android vs. can be feature-gated out

### 17.2 Toolchain Setup

```bash
# Install NDK r27c (LTS)
export ANDROID_NDK_ROOT=/opt/android-ndk/r27c

# Add Rust target
rustup target add aarch64-linux-android

# Install cargo-ndk
cargo install cargo-ndk --locked
```

### 17.3 Build Verification

```bash
# Test Android check (should fail until Phase 1 work begins)
cargo check --target aarch64-linux-android --no-default-features --features android

# Test with no features (should work after ssh-control feature added)
cargo check --target aarch64-linux-android --no-default-features
```

---

## 18. Code Scan Verification (2026-05-24)

This document was verified against the codebase on 2026-05-24. The following findings update the original analysis:

| Original Claim | Verified Status | Notes |
|---|---|---|
| Feature flags exclude GUI with `--no-default-features` | ✅ Verified | Cargo.toml lines 6-9 |
| `cfg(target_os = "linux")` doesn't match Android | ✅ Verified | Standard Rust target behavior |
| `ssh2` is sole OpenSSL user | ✅ Verified | Dependency tree confirmed |
| `tempfile` → `rustix` compile error | ✅ Verified | 10+ uses in `src/agent.rs` |
| No `src/android/` directory | ✅ Verified | Glob returned nothing |
| No `android/` Gradle project | ✅ Verified | Glob returned nothing |
| GPU backend uses `detect_backend()` | ✅ Verified | `src/gpu/mod.rs:43` |
| Windows compiles with `--no-default-features` | ✅ Verified | `cargo check` succeeds |

### Findings That Corrected the Document

1. **tempfile usage is more extensive** — 10+ uses in agent.rs alone, not just "tempfile crate"
2. **Signal handling needs addressing** — Unix signals don't work in Android apps; need JNI shutdown
3. **Config path injection is complex** — CLI parsing doesn't exist in cdylib mode
4. **`.cargo/config.toml` doesn't exist** — needs to be created
5. **File permissions hardening** — needs Android-specific handling

### Findings That Added to the Document

1. **Section 10A**: Signal handling on Android
2. **Section 10B**: Config path injection complexity
3. **Section 10C**: File permissions hardening on Android
4. **Section 17**: Pre-Phase 1 verification checklist
5. **Section 18**: This verification section

---

## 19. Updated mTLS and Remote Agent Analysis (2026-05-24)

This section incorporates findings from the updated `tls-architecture.md` and `remote-agent.md` reference documents, reflecting recent bugfixes and architectural clarifications.

### 19.1 CA Versioning and Auto-Rotation

**New finding:** The `.ca-v2` sentinel file mechanism ensures backward compatibility:

- A `.ca-v2` sentinel file is written alongside every correctly-formatted CA
- On startup, if the sentinel is absent (old installation or first run), all CA and leaf certs are deleted and regenerated with the correct CA DN
- This ensures that old certs with an ambiguous subject (`CN=rcgen self signed cert`) are rotated out automatically without user intervention

**Android impact:** The auto-rotation mechanism works unchanged on Android since it's based on file I/O in the config directory. The sentinel file will be created at `~/.config/llama-monitor/certs/.ca-v2`.

### 19.2 Multi-CA Support

**New finding:** The agent supports multiple independent CAs:

```
~/.config/llama-monitor/certs/
├── ca.pem                    # Primary CA
└── cas/
    ├── <instance-id>.pem    # Per-dashboard CA certificates
    └── ...
```

**Android impact:** The multi-CA support works unchanged — the agent loads trust anchors from both the legacy single CA and all `.pem` files in the `cas/` directory. This is critical for multi-dashboard setups where a single agent serves multiple dashboards.

### 19.3 Agent Token Management

**Updated understanding:** The `agent-tokens.json` file enables multi-client setups:

```json
{
  "tokens": ["<token1>", "<token2>"]
}
```

- The primary agent token is automatically ensured in this file on startup
- Any token in the list is accepted for authenticated agent endpoints
- Enables multiple dashboards to poll the same agent

**Android impact:** Android will be a metrics consumer, so it will use the primary `api-token` file. The `agent-tokens.json` mechanism is primarily for agent-side multi-client support.

### 19.4 Protocol Versioning

**New finding:** The remote agent protocol uses versioning:

- Current protocol version: **1.0.0**
- The dashboard enforces a minimum protocol version when polling the agent
- Below minimum version: degraded compatibility mode with `protocol_too_old` flag
- Missing version: treated as protocol too old (older agents)

**Android impact:** The protocol versioning ensures backward compatibility. Android will enforce the same minimum version as other platforms.

### 19.5 Agent States and Indicators

**Updated states for Android dashboard:**

| State | Description | Android Behavior |
|-------|-------------|------------------|
| Connected | Agent reachable, health checks succeed | Normal operation |
| Firewall blocked | Agent started but HTTP endpoint unreachable | Show "Fix" button with firewall guidance |
| Update Available | Agent version older than latest release | Show "Upgrade" button |
| Protocol too old | Agent below minimum protocol version | Enter degraded mode |

**Android-specific consideration:** The "Firewall blocked" state is more likely on Android due to mobile network configurations. The UI guidance should include mobile network troubleshooting.

### 19.6 Certificate Pinning for Remote Agents

**Updated TOFU implementation:**

```
Remote Agent Certificate Pinning Flow:
1. First connection: store agent's CA fingerprint
2. Subsequent connections: verify fingerprint matches
3. Mismatch: prompt user to re-scan and trust new key (like SSH host key verification)
```

**Android impact:** Extend the existing `ssh-known-hosts.json` model to HTTPS certificates. The verification flow is architecturally identical to SSH host key verification.

### 19.7 Endpoint Authentication

**Updated authentication requirements:**

| Token Type | Endpoints |
|------------|-----------|
| `api-token` | Standard operations (attach, DB queries, TLS/ACME, most remote-agent endpoints) |
| `db-admin-token` | Elevated operations (install/remove on remote-agent endpoints) |

**Android impact:** Authentication works unchanged — Android uses the same token management as other platforms.

### 19.8 Remote Agent Config File

**New finding:** The dashboard writes `remote-agent-config.json` during install:

```json
{
  "api_token": "<dashboard-api-token>"
}
```

- Written with restrictive permissions (0600 on Unix/macOS)
- Allows agent (or SSH-managed operations) to authenticate to dashboard endpoints

**Android impact:** Not directly applicable since Android is a metrics consumer, not the agent host. The config file is written to the remote agent host.

---

---

## 20. Multi-Client Remote Agent Enrollment (Security Bug)

**Added:** 2026-05-25  
**Priority:** Phase 4 (full implementation); Phase 1 workaround documented below  
**Status:** Bug — multi-client infrastructure exists but enrollment is manual-only

### 20.1 Current State

The agent already supports multiple trust anchors via the `cas/` directory:
```
~/.config/llama-monitor/certs/
├── ca.pem                          # Legacy single CA (primary dashboard)
└── cas/
    ├── <instance-id-1>.pem        # Per-dashboard CA (SHA1 of CA pubkey, hex)
    └── <instance-id-2>.pem        # Second dashboard CA, etc.
```

The agent loads all `.pem` files from `cas/` at startup and builds a combined TLS trust store. Multiple independent dashboards CAN connect to the same agent IF their CAs are pre-loaded. Token auth is also multi-client: `agent-tokens.json` accepts any token in the array.

**The gap:** There is no self-service enrollment API. A second client (Android, second Mac, etc.) must:
1. Generate its CA + client cert
2. Manually copy its CA `.pem` to `cas/` on the agent host (requires SSH or filesystem access to RYNE)
3. Restart the agent (CAs are loaded once at startup — no dynamic reload)

This works for technical users managing multiple desktop dashboards. It is a blocking UX problem for Android users who may not have SSH access to RYNE.

### 20.2 What's Needed

Four additions to make multi-client enrollment self-service:

**A. Dynamic CA reload without restart**

When a new `.pem` file is added to `cas/`, the agent should reload its trust store without dropping existing connections. Use `tokio::fs::watch` or a timed reload (e.g., check `cas/` every 60 seconds for new files).

**B. Client CA registration endpoint**

```
POST /api/agent/trust-ca
Authorization: Bearer <db-admin-token>
Content-Type: application/json

{ "ca_pem": "-----BEGIN CERTIFICATE-----\n..." }
```

- Requires `db-admin-token` (elevated privilege — same as destructive operations)
- Validates the PEM is a valid CA certificate
- Writes to `cas/<instance-id>.pem` (instance-id = first 16 hex chars of SHA1(pubkey))
- Triggers dynamic reload
- Returns `{ "instance_id": "<hex>", "trusted": true }`

This endpoint allows the dashboard UI on Mac to push Android's CA to RYNE on behalf of the user — no SSH required.

**C. Secure Android pairing flow (UI)**

```
Android App (first run)          Mac Dashboard             RYNE Agent
      │                               │                        │
      │  1. Generate CA + client cert │                        │
      │  2. Display QR code with:     │                        │
      │     - CA public cert (PEM)    │                        │
      │     - Preferred agent URL     │                        │
      │                               │                        │
      │          3. Mac scans QR      │                        │
      │          4. POST /api/agent/  │                        │
      │             trust-ca with     │                        │
      │             db-admin-token    │──────────────────────▶│
      │                               │  5. CA written to cas/ │
      │                               │  6. Trust store reload  │
      │                               │                        │
      │◀─────────────────────────────────────────────────────│
      │  7. Android connects directly (mTLS succeeds)          │
```

The QR code only contains the Android CA public cert (not private key). The Mac dashboard acts as the enrollment relay, authenticating to the agent with its existing `db-admin-token`.

**D. Per-client token issuance (Phase 5+ consideration)**

Currently all clients share tokens from `agent-tokens.json`. Per-client tokens with individual revocation would allow removing one client without affecting others. This is a larger architecture change — deferred until multiple Android clients exist in practice.

### 20.3 Phase 1 Workaround

For Phase 1 development and testing, Android connects via the manual process:

```bash
# On RYNE (as the agent host user):
mkdir -p ~/.config/llama-monitor/certs/cas/

# On Android (or from adb shell):
# 1. Start llama-monitor Android app once — this generates the CA at:
#    /data/data/com.llamamonitor.app/files/llama-monitor/certs/ca.pem

# 2. Pull the CA from the device:
adb pull /data/data/com.llamamonitor.app/files/llama-monitor/certs/ca.pem /tmp/android-ca.pem

# 3. Deploy to RYNE:
scp /tmp/android-ca.pem nick@ryne:~/.config/llama-monitor/certs/cas/android-<date>.pem

# 4. Restart the agent:
ssh nick@ryne "systemctl restart llama-monitor-agent"
# (or however the agent is managed on RYNE)
```

Document this procedure in `docs/reference/android.md` alongside the Phase 1 deliverables.

### 20.4 Agent Restart Gap

The dynamic CA reload (§20.2-A) is required before the QR pairing flow works in production. Without it, the `POST /api/agent/trust-ca` endpoint writes the file but the new CA only takes effect after an agent restart — defeating the self-service UX goal.

The reload should be implemented as: after writing a new CA file, rebuild the `ServerConfig` and hot-swap it into the warp server's TLS acceptor. This requires plumbing a `watch::Receiver<Arc<ServerConfig>>` through the agent's TLS setup, which is a moderate but self-contained change.

### 20.5 Files Impacted (Phase 4)

```
src/agent.rs        — POST /api/agent/trust-ca handler; dynamic ServerConfig reload
src/certs.rs        — watch-based CA directory monitor; build_combined_tls_config()
android/app/…/      — QR code generation + display on first-run screen
static/js/          — Enrollment relay UI: scan QR → POST to agent → confirm
docs/reference/     — android.md Phase 1 manual enrollment procedure
```

---

*End of document.*
