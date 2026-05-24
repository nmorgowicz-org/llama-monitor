# Android Architecture for Llama-Monitor

**Date:** 2026-05-24  
**Author:** Claude (full codebase analysis)  
**Status:** Draft  
**Branch:** feat/android-compatibility


---

## 1. System Overview

llama-monitor is a monitoring-first web dashboard for llama.cpp servers, implemented as a Rust binary serving a warp HTTP/WebSocket server with a vanilla JS web UI embedded at compile time via `build.rs`. Platform-specific metric backends provide GPU, CPU, and system telemetry. A remote agent protocol (HTTPS/mTLS + bearer token) enables attaching to remote GPU machines.

The binary does not use Electron, Tauri, or any JavaScript runtime. Tray icon and desktop WebView popover are behind Cargo feature flags stripped at compile time. On Android, the Rust core runs as a cdylib (`libllama.so`) inside a Kotlin foreground service, serving the web UI over a localhost warp server to an Android WebView.

---

## 2. Dependency Audit: Android Compatibility

### 2.1 Feature Flag Structure

Building with `--no-default-features` removes `wry`, `winit`, `tray-icon`, and all transitive platform-specific deps (`libappindicator`, `muda`, `objc2`, `webkit2gtk`, etc.). A new `android` feature flag gates Android-specific code.

### 2.2 Platform-Conditional Dependencies

| Crate | Gate | Android status |
|---|---|---|
| `gtk 0.18` | `cfg(target_os = "linux")` | Excluded — `target_os = "android"` ≠ `"linux"` |
| `mac-notification-sys 0.6` | `cfg(target_os = "macos")` | Excluded |
| `wmi 0.18.4` | `cfg(windows)` | Excluded |

### 2.3 Real Remaining Issues After `--no-default-features`

| Crate | Issue | Severity | Solution |
|---|---|---|---|
| `ssh2 0.9.5` + `vendored-openssl` | Links vendored OpenSSL C; NDK integration non-trivial | **High** | Feature-gate under `ssh-control`; Android uses `--no-default-features --features android` |
| `rusqlite 0.39` + `bundled` | Bundles SQLite C source | **Medium** | SQLite cross-compiles cleanly with NDK |
| `dirs 6.0.0` | Returns empty paths on Android | **Low** | `#[cfg(target_os = "android")]` path override |
| `sysinfo 0.39` | Reduced metrics on Android | **Low** | Use as-is; returns what Android allows |
| `tempfile 3.27` | Depends on `rustix 1.1.4` with unresolved Android compile errors | **High** | Manual temp file creation in app's private storage |
| `src/main.rs` | Desktop entry with tray/GUI setup | **Medium** | Android uses cdylib JNI entry; `main.rs` excluded |
| `src/system.rs` | Platform-specific commands/paths | **Medium** | Add `#[cfg(target_os = "android")]` backends |
| `src/gpu/mod.rs` | No Android GPU backend | **Medium** | Add `AndroidGpuBackend` (dummy initially) |

### 2.4 What Compiles Cleanly for Android

The majority of the codebase compiles without changes: tokio, warp, reqwest (rustls), rustls, hyper-rustls, rcgen, aes-gcm, sha1/sha2, hkdf, argon2, rand/getrandom, serde, sysinfo, warp-helmet, rusqlite+bundled, and the build script. All GUI stack crates (wry, winit, tray-icon, gtk) are excluded.

---

## 3. Proposed Architecture

### 3.1 Guiding Principle

Android is a first-class platform, not a port. Per the MANDATORY multi-platform rule in AGENTS.md, Android joins macOS, Linux, and Windows as a supported target. Every Android-specific code path requires a `#[cfg(target_os = "android")]` guard with an inline comment.

### 3.2 High-Level Components

The Android APK contains a Kotlin foreground service hosting the Rust cdylib and a WebView. The Rust cdylib runs a tokio runtime with warp HTTP server on `127.0.0.1:PORT`, WebSocket push, llama metrics polling, remote agent client, chat storage, auth/TLS, and Android metric backends. The WebView loads `http://127.0.0.1:PORT/` from the warp server, using the existing JS/CSS/HTML with zero changes for Phase 1. Communication between Kotlin and Rust is limited to lifecycle (start/stop), secret management, and battery callbacks via JNI.

### 3.3 Architecture Decision: Localhost Warp Server (Not JNI Bridge)

Two approaches were considered:
- **Option A:** JNI bridge only — WebView uses `addJavascriptInterface` to call Kotlin → Rust JNI
- **Option B:** Localhost warp server — Rust runs warp on `127.0.0.1:PORT`; WebView connects as a desktop browser

**Decision: Option B.**

Rationale:
1. **Zero web UI changes** — `dashboard-ws.js` connects to `location.host`; WebSocket and all `fetch` calls work without modification
2. **Reuses full existing message protocol** — JSON push schema, capability negotiation, telemetry grades all work as-is
3. **Single code path** — Warp handlers, auth, and push logic serve Android identically to desktop
4. **Debuggable** — Chrome DevTools remote debugging; `adb forward` for direct HTTP access
5. **No new IPC layer** — JNI bridge needed only for lifecycle, secret management, and battery callbacks

Trade-off: ~2-5MB RAM overhead for loopback. Acceptable on Z Fold 6 (12GB RAM).

Loading from `file://` (APK assets) is not viable — `file://` URLs have no `host:port` in the origin, making `ws:///ws` invalid.

### 3.4 Web UI Asset Strategy

Use the existing `include_str!` mechanism in the cdylib (same as desktop). Assets compiled into `libllama.so`, served via the existing `static_routes()` function. The WebView loads `http://127.0.0.1:PORT/` directly from the warp server.

Benefits:
- Zero JS changes — `location.host` resolves correctly
- Consistent with desktop — same asset pipeline, same `build.rs` generation
- Single code path — `gen/routes.rs` compiles fine in a cdylib
- No APK assets management — no separate asset copy step in Gradle build

### 3.5 SSH Control on Android

`ssh2 0.9.5` is the sole OpenSSL user. Decision: feature-gate SSH control out for Phase 1-3 (Android is a metrics consumer only; agent management from desktop). Evaluate `russh` (pure-Rust SSH) for Phase 4.

---

## 4. Existing Code Reusability

### 4.1 Modules That Require Zero Changes

HTTP server/route setup, WebSocket handler+push, llama metrics poller, remote agent client, auth (token/form/basic), TLS/rcgen, chat storage, session management, capability calculation, REST API handlers, SQLite backup logic, and generated static routes all compile and run unchanged on Android.

### 4.2 Modules That Need Android Backends

| Module | Android change |
|---|---|
| `src/system.rs` | Add `#[cfg(target_os = "android")]` branch: `/proc/stat`, `/sys/class/thermal`, `/proc/cpuinfo`, BatteryManager JNI callbacks |
| `src/gpu/mod.rs` | Add `#[cfg(target_os = "android")]` branch; returns `AndroidGpuBackend` (dummy Phase 1, Adreno/Mali Phase 5) |
| `src/config.rs` | Add `#[cfg(target_os = "android")]` path override using JNI-provided `filesDir` |
| `src/lhm.rs` | Already conditional on Windows; not compiled on Android |
| `src/tray.rs` | Already feature-gated; excluded by `--no-default-features` |
| `src/main.rs` | Android uses cdylib JNI; `main.rs` not in cdylib target |

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

---

## 5. Data Flow on Android

### 5.1 Metrics Push Pipeline

The Android foreground service runs the Rust tokio runtime with: LlamaPoller (500ms) polling llama-server; RemoteAgentPoller (5s) via HTTPS/mTLS; new AndroidSystemPoller (5s) reading `/proc/stat`, `/proc/cpuinfo`, thermal zones, and BatteryManager JNI callbacks; new AndroidGpuPoller (5s, dummy Phase 1, Adreno/Mali sysfs Phase 5); and the WebSocket pusher to `ws://127.0.0.1:PORT/ws`. The Android WebView runs `dashboard-ws.js` for DOM updates.

### 5.2 Availability Codes for Android Metrics

The existing `Availability` enum covers all Android cases. GPU (Phase 1-4): `BackendUnavailable`. CPU temperature: `Available` (partial) or `SensorUnavailable` (SELinux). CPU load/clock: `Available` (partial). Memory: `Available`. Battery: `Available`. Remote agent metrics: `Available` when connected. Never return `null` silently.

### 5.3 Adaptive Polling

The Kotlin foreground service communicates device state to the Rust runtime via `RustLib.setPollInterval(ms)`:

| Device state | Poll interval |
|---|---|
| Screen on, charging | 500ms (default) |
| Screen on, battery | 1000ms |
| Screen off | 10000ms |
| Battery saver mode | 30000ms |
| App backgrounded | 5000ms (JS signals server) |

---

## 6. UI/UX Architecture

### 6.1 Current Mobile Readiness (Verified Audit)

The frontend has basic responsive layout and PWA scaffolding, but **zero touch-specific interaction support**. All interactive gestures are mouse-only.

**Already present:** Viewport meta tags, PWA manifest, responsive CSS breakpoints (1200px–640px across 8 CSS files), ResizeObserver density classes, `prefers-reduced-motion` on 159+ animations, GPU-composited `backdrop-filter`, `prefers-color-scheme` detection on load, `-webkit-overflow-scrolling: touch` on 2 containers, IndexedDB chat state, `location.host` WebSocket connection.

**Verified missing — must be built from scratch:**

| Missing Feature | Details |
|---|---|
| Touch drag handlers | Sidebar resize uses only `mousedown`/`mousemove`/`mouseup`; no touch equivalents |
| Swipe gestures | No swipe or gesture code exists anywhere |
| Long-press context menus | Context menus use only `contextmenu` (right-click); no touch long-press |
| Safe area insets | No `env(safe-area-inset-*)` variables in any CSS file |
| Soft keyboard avoidance | No `visualViewport` reference anywhere |
| Coarse-pointer touch targets | Zero `@media (pointer: coarse)` rules in any of 15 CSS files |
| Folded cover breakpoint | No `max-width: 420px` media query; smallest existing is 520px |
| `-webkit-overflow-scrolling: touch` coverage | Only 2 containers; all scrollable containers need it |

### 6.2 Z Fold 6 Form Factor

Folded (cover display): ~390×748dp portrait. Unfolded (main display): ~932×932dp landscape/tablet. Existing layout classes map: folded portrait triggers `shell-width-very-tight` (< 520px), folded landscape triggers `shell-width-tight` (< 920px), unfolded landscape triggers `shell-width-snug` (< 1050px). New `@media (max-width: 420px)` breakpoint needed for folded cover. Kotlin→JS foldable state detection via `androidx.window` dispatches a `foldstatechange` CustomEvent.

### 6.3 Touch Handling

All touch features must be built from scratch: sidebar drag-to-resize (touchstart/touchmove/touchend), left-edge swipe for sidebar toggle, long-press context menus (500ms timer), soft keyboard avoidance (`visualViewport.resize`), scrollable container scrolling, coarse-pointer touch targets (44px min), and safe area insets. code examples.

### 6.4 WebView Configuration

JavaScript, DOM storage, and database enabled. No file URL access. Chrome DevTools remote debugging for debug builds. Network security config allows cleartext to 127.0.0.1. CSP must allow `ws://127.0.0.1:PORT` for WebSocket. Kotlin and XML details.

### 6.5 Theme and Performance

Dark theme is OLED-friendly (true black backgrounds). Add live `change` event listener for Android system theme switching. `backdrop-filter` disabled on budget devices via `@media (pointer: coarse) and (max-resolution: 2.5dppx)`. Tokio worker threads limited to 2 on Android. CSS and Rust details.

---

## 7. Metrics Backend Strategy

### 7.1 Existing Pattern

The GPU backend uses `detect_backend(force: &str)` dispatching to platform backends or `DummyBackend`. System metrics use inline `#[cfg]` blocks. Android adds new arms to each.

### 7.2 Android CPU Metrics

CPU model from `/proc/cpuinfo`, core count and load from sysinfo, frequency from `/sys/…/cpufreq` (SELinux-dependent). implementation.

### 7.3 Android Thermal Metrics

Thermal zones from `/sys/class/thermal/thermal_zone*/temp` (partial, SELinux-dependent). BatteryManager temperature always available. Android ThermalManager API (API 29+) returns normalized STATUS enum, not raw °C. Strategy: enumerate zones at startup, record readable ones, always include battery temperature.

### 7.4 Android GPU Metrics (Phase 5)

Adreno kgsl sysfs paths (`/sys/class/kgsl/kgsl-3d0/`) for Snapdragon devices. ARM Mali sysfs for Exynos variants (firmware-dependent, best-effort). GPU VRAM is `NotApplicable` on Android (unified LPDDR5X memory). sysfs paths.

### 7.5 Battery as a First-Class Metric

Battery is a new metric class not present on any desktop platform. the `BatteryMetrics` struct and serialization details.

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
| Battery | N/A | N/A | N/A | BatteryManager |

---

## 8. Security Model

### 8.1 Alignment with Existing Architecture

The existing security model (AGENTS.md §Security Requirements) applies unchanged. All token comparisons use `subtle::ConstantTimeEq`. All randomness uses `getrandom::getrandom()`. All user-data API endpoints require `api-token`. Destructive operations require `db-admin-token`.

### 8.2 Secret Storage

All secrets use AndroidKeyStore-backed EncryptedSharedPreferences (api-token, remote agent token, TLS cert private key, server CA cert). Kotlin implementation.

### 8.3 Data at Rest

Chat database in app-private storage (`/data/data/…/files/llama-monitor/chat.db`), mode 0600, inaccessible to other apps without root. SQLite encryption (SQLCipher) deferred to Phase 4.

### 8.4 Certificate Pinning

TOFU (Trust On First Use) for remote agent HTTPS certificate, extending the existing `ssh-known-hosts.json` model.

### 8.5 LAN llama-server Security

`http://192.168.x.x:8001` treated as trusted per existing model. `network_security_config.xml` permits cleartext to configured llama-server address (user-configurable, not hardcoded).

---

## 9. Cross-Compilation Strategy

### 9.1 Target and Toolchain

Primary: `aarch64-linux-android` (arm64-v8a, Android API 30+). Minimum API 30 (Android 11) for reliable `backdrop-filter`, ThermalManager API, and better Chromium features.

```bash
rustup target add aarch64-linux-android
cargo install cargo-ndk --locked
```

`.cargo/config.toml` linker config for `aarch64-linux-android` (cargo-ndk sets from `ANDROID_NDK_ROOT` automatically).

### 9.2 Build Command

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

### 9.3 Dependency Build Analysis

| Crate | Android cross-compile | Resolution |
|---|---|---|
| `tokio 1.52` | Works | `tokio::signal` SIGTERM works on Android |
| `warp 0.4` | Works | Pure Rust hyper-based |
| `reqwest 0.13` | Works | rustls, `default-features = false` |
| `rustls 0.23` | Works | Pure Rust |
| `serde/serde_json` | Works | Pure Rust |
| `argon2 0.6.0-rc.8` | Works | Pure Rust |
| `rand 0.10` | Works | `getrandom` supports aarch64-linux-android |
| `sysinfo 0.39` | Works | Explicit Android support |
| `rusqlite 0.39` + `bundled` | Works with NDK | `cc` crate picks up NDK C compiler |
| `dirs 6.0.0` | Empty paths | `#[cfg(target_os = "android")]` override |
| `ssh2 0.9.5` | Feature-gated out | `--no-default-features` excludes `ssh-control` |
| `wry/winit/gtk/tray-icon` | All excluded | `--no-default-features` + platform cfg gates |
| `tempfile 3.27` | Compile error | Manual temp file alternative |
| `clap 4` | N/A | Not in cdylib |

---

## 10. Foreground Service and Background Execution

Android requires a foreground service to keep the Rust runtime alive. The service starts the Rust cdylib, registers battery and screen broadcast receivers for adaptive polling, and shows an ongoing notification. Doze mode handling: remote agent polling can be deferred; foreground service with `dataSync` type has exemption for active data connections. Kotlin service implementation and manifest permissions.

---

## 11. CI/CD Integration

### 11.1 Runner Image Changes

The `llama-monitor-runner` image needs: Android NDK r27c, `aarch64-linux-android` Rust target, and `cargo-ndk`. Estimated image size increase: ~3.5 GB (current ~6-7 GB → ~10 GB). Dockerfile diff.

### 11.2 CI Jobs

- **`check-android`:** `cargo check --target aarch64-linux-android --no-default-features --features android` — no NDK required, catches `#[cfg]` mistakes
- **`build-android`:** `cargo ndk build --release` + `gradlew assembleRelease` — produces APK artifact

APK build job split (recommended): `build-android-lib` on `arc-llama-monitor` produces `libllama.so`; `build-android-apk` on `ubuntu-latest` (Android SDK pre-installed) runs `gradlew`.

### 11.3 AGENTS.md Update

Add `cargo check --target aarch64-linux-android --no-default-features --features android` to pre-PR checklist alongside existing Windows check.

---

## 12. Phased Implementation Plan

> **Timeline note (2026-05-24):** Revised for 100% AI-agent development with Claude Code + Codex running in parallel. Debug cycles that bottleneck a single agent are absorbed — one agent compiles while another pre-writes the next fix. The bottleneck shifts from "waiting for compilation" to "waiting for physical device feedback."

### Phase 1: Build Infrastructure and Skeleton (Week 1)

**Revised from Weeks 1-2.** Waves A (Rust backends), B (JNI), and C (Gradle/Kotlin) overlap aggressively.

1. Add `ssh-control` and `android` features to `Cargo.toml`; move `ssh2` under `ssh-control`
2. Create `src/android/mod.rs` — JNI entry points (start/stop/status/setPollInterval)
3. Create `src/android/paths.rs` — Android config dir override
4. Add `#[cfg(target_os = "android")]` temp file alternative (avoids `tempfile` → `rustix` compile error)
5. Add `#[cfg(target_os = "android")]` GPU stub to `src/gpu/mod.rs` (DummyAndroidBackend)
6. Add `#[cfg(target_os = "android")]` stub to `src/system.rs` (sysinfo-only; no platform commands)
7. Set up `android/` Gradle project with Activity + WebView layout
8. Implement `LlamaMonitorService` (foreground service, notification)
9. Implement `KeystoreHelper.kt`
10. Add `cargo-ndk` task to Gradle; configure `.cargo/config.toml`
11. Add `check-android` job to `.github/workflows/ci.yml`
12. Update `AGENTS.md` pre-PR checklist with Android check command
13. Merge NDK + `aarch64-linux-android` + `cargo-ndk` to `llama-monitor-runner`; trigger image rebuild

**Before merging:** `cargo check --target x86_64-pc-windows-gnu`, `cargo check --target aarch64-linux-android`, `cargo clippy -- -D warnings`, `cargo fmt`.

**Deliverable:** APK installs on Z Fold 6. Dashboard shows with remote agent metrics. Local Android metrics show correct `availability` codes.

### Phase 2: Android System Metrics (Week 2)

**Revised from Weeks 3-4.** Multiple agents write battery.rs, thermal.rs, BatteryMetrics struct, JS handlers, and adaptive polling in parallel.

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

**Revised from Weeks 5-8.** CSS work collapses to a day. Touch JS and foldable detection written in parallel.

#### 3A: CSS Mobile Foundations (Week 3, days 1-2)
1. Folded cover breakpoint — `@media (max-width: 420px)` in `layout.css`
2. Scrollable container scrolling — `-webkit-overflow-scrolling: touch` and `overscroll-behavior`
3. Coarse-pointer touch targets — `@media (pointer: coarse)` with 44px min targets
4. Safe area insets — `env(safe-area-inset-*)` variables in `tokens.css`
5. Backdrop-filter disable — budget device media query in `cards-inference.css`

#### 3B: Touch Interaction (Weeks 3-4)
6. Sidebar drag-to-resize — touch handlers in `nav.js`
7. Left-edge swipe for sidebar — document-level touch listeners in `nav.js`
8. Long-press context menus — 500ms timer in `chat-sessions-sidebar.js`
9. Soft keyboard avoidance — `visualViewport.resize` listener in chat input
10. Live theme switching — `prefers-color-scheme` change listener in `user-menu.js`

#### 3C: Foldable State Detection (Week 4)
11. Kotlin foldable detection — `androidx.window` → `foldstatechange` CustomEvent
12. JS foldable handler — `android-foldable.js` toggles body classes
13. Two-panel unfolded layout — CSS for `body.is-unfolded` grid layouts

#### 3D: Verification and Integration (Week 5)
14. Run Playwright UI tests; update selectors if element IDs changed
15. Run `npm run lint` and `npm run validate-js` on all modified JS files
16. Physical device validation on Z Fold 6; adjust thresholds as needed

**Deliverable:** App fully touch-operable; adapts to folded/unfolded states; chat input works with soft keyboard.

### Phase 4: Security Hardening and SSH Control Evaluation (Week 6)

**Revised from Weeks 7-8.** Security review items parallelized across agents.

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

## 13. Risks and Mitigations

| Risk | Probability | Mitigation |
|---|---|---|
| `tempfile` → `rustix` compile error on Android | High | `#[cfg(target_os = "android")]` alternative: manual temp file creation |
| `rusqlite` + `bundled` fails NDK cross-compile | Low | Use NDK r27c (tested); document in AGENTS.md |
| Tokio worker panics on missing Android syscall | Low | Limit `worker_threads(2)`; test on API 30+ |
| WebView blocks `ws://127.0.0.1` | Medium | `network_security_config.xml`; test on API 30 emulator |
| `backdrop-filter` causes thermal throttle | Medium | `@media (pointer: coarse)` disable rule |
| Adreno kgsl sysfs paths vary by firmware | High | Return `SensorUnavailable` gracefully; never crash |
| `ssh-control` feature accidentally included in Android build | Medium | `cargo check --target aarch64-linux-android` in CI |
| Foreground service killed by Doze mode | Medium | `foregroundServiceType="dataSync"`; reduce polling; test on Doze device |
| warp port conflict (17778 already in use) | Low | Pick random port at startup; communicate to WebView via JNI |
| JNI global reference leaks → memory growth | Medium | `jni-rs` `GlobalRef` wrappers; profile with Android Studio |
| New `#[cfg(target_os = "android")]` breaks Windows build | Medium | Mandatory `cargo check --target x86_64-pc-windows-gnu` in pre-PR checklist |
| `dirs 6` empty path not caught → chat DB in wrong location | Low | `src/android/paths.rs` override; test asserting non-empty path |
| Runner image rebuild required before any Android CI | Medium | Merge `llama-monitor-runner` Dockerfile changes first |

---

## 14. Files Changed Summary

### New Files
```
src/android/mod.rs          — JNI entry points
src/android/battery.rs      — BatteryManager JNI receiver
src/android/thermal.rs      — /sys/class/thermal reader
src/android/paths.rs        — Android config directory
src/android/keystore.rs     — AndroidKeyStore JNI bridge
src/android/tempfile.rs     — Android temp file alternative
src/gpu/android.rs          — Android GPU backend

android/app/src/main/java/com/llamamonitor/app/
├── MainActivity.kt
├── LlamaMonitorService.kt
├── RustLib.kt
└── KeystoreHelper.kt
android/app/src/main/res/xml/network_security_config.xml
android/app/src/main/AndroidManifest.xml
android/app/build.gradle
android/settings.gradle

docs/reference/android.md   — Android build and deployment reference
```

### Modified Files
```
Cargo.toml                   — ssh-control + android features; ssh2 optional
.cargo/config.toml           — aarch64-linux-android linker config
src/gpu/mod.rs               — Android backend dispatch
src/system.rs                — Android metric collection paths
src/config.rs                — Android paths override
src/agent.rs                 — Android temp file alternative
static/js/features/dashboard-ws.js     — Handle data.battery field
static/js/features/dashboard-render.js — Render battery in system card
static/js/features/nav.js              — Touch drag resize; left-edge swipe
static/js/features/chat-sessions-sidebar.js — Long-press context menus
static/js/features/chat-input.js       — visualViewport keyboard avoidance
static/js/features/user-menu.js        — Live prefers-color-scheme listener
static/js/features/android-foldable.js — Foldable state handler (new)
static/css/layout.css        — 420px breakpoint; pointer:coarse; overflow-scrolling
static/css/tokens.css        — env(safe-area-inset-*) variables
static/css/cards-inference.css — backdrop-filter disable for budget devices
.github/workflows/ci.yml     — check-android job
.github/workflows/release.yml — build-android job
AGENTS.md                   — Android in multi-platform checklist
```

### llama-monitor-runner Changes (separate repo)
```
Dockerfile                   — NDK r27c; aarch64-linux-android target; cargo-ndk
```

---

## 15. Decisions Required

1. **SSH control for Phase 4:** Relay vs. evaluate `russh` (pure-Rust SSH2)? `russh` would eliminate OpenSSL from the entire project.
2. **App package name:** `com.llamamonitor.app` or another namespace?
3. **Distribution method:** Sideload (`.apk` artifact from CI) vs. future Play Store consideration?
4. **SSH known-hosts + TOFU cert pinning:** Extend existing `ssh-known-hosts.json` model or use separate cert store?
5. **APK build job split (Phase 1):** Confirm Option A (split on `arc-llama-monitor` + `ubuntu-latest`) vs. adding Android SDK to runner image.
6. **NDK image size:** Accept ~3.5 GB increase or implement post-extract trim to ~800 MB?
7. **`tempfile` replacement strategy:** Which code paths use `tempfile`? Audit and add Android alternatives for each.

---

*End of document.*
