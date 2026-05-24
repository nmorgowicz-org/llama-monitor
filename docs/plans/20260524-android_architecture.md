# Android Architecture for Llama-Monitor

Date: 2026-05-24
Author: Iris (assisting Nick)
Status: Draft / initial architecture

## 1. Goals

- Provide a secure, battery-efficient Android client for llama-monitor.
- Run on Samsung Galaxy Z Fold 6:
  - Folded (narrow front panel): compact, glanceable monitoring.
  - Unfolded (inner screen): full dashboard experience.
- Primary role:
  - Thin client that:
    - Directly accesses llama-server (e.g., http://192.168.2.16:8001) for inference metrics and chat.
    - Uses the remote agent (on “ryne”) for system/GPU metrics and agent lifecycle.
  - Displays llama-server metrics, GPU stats, system metrics, logs, and chat.
- Non-goals (for now):
  - Running a local llama-server on the phone.
  - Full parity with desktop features (e.g., heavy SSH workflows).

Key constraints:
- Must not sacrifice security.
- Must be robust against Android’s aggressive background limits.
- Must avoid Playwright (Ubuntu 26.04 issues) in our tooling.

## 2. High-Level Architecture

We will NOT do a full 1:1 desktop port. We will build:

- A Rust library (shared with llama-monitor) compiled to Android via NDK.
- A thin Android shell:
  - Activity with WebView.
  - Foreground service to keep monitoring alive.
- A remote agent on “ryne” (managed via ssh nick@ryne) that:
  - Exposes system/GPU metrics over HTTPS/mTLS + bearer token.
  - Is installed/started/updated via SSH.
- llama-server (e.g., http://192.168.2.16:8001):
  - Directly accessed by Android for inference metrics and chat.
  - No SSH or extra security wrapping; treated as trusted on the network.

Core idea:
- Android app = secure viewer + controller.
- Heavy lifting (GPU polling, system telemetry, SSH management) lives on ryne (or other remote hosts).

Components:

- Rust core (shared):
  - Llama metrics poller (HTTP).
  - WebSocket push layer.
  - Auth, TLS, chat storage, config.
  - Remote agent protocol (HTTP/WS over TLS).
- Android-specific:
  - JNI bridge to Activity.
  - WebView hosting the existing web UI.
  - Foreground service for background monitoring.
  - Android notifications for alerts.
- Remote agent:
  - Runs on ryne via llama-monitor --agent.
  - Exposes:
    - /health (public).
    - /metrics, /metrics/system, /metrics/gpu, /info (authenticated: mTLS + bearer token).
  - Accessed over HTTPS/mTLS with:
    - mTLS mandatory (client cert + CA).
    - Bearer token required for all endpoints except /health.
  - SSH is used only for control (install/start/stop/update/remove), not for ongoing metrics.

## 3. Security Model

Assumptions:
- We trust our devices (phone, Mac, etc.).
- We do not trust the network (public Wi-Fi, etc.).
- We must protect:
  - Llama-server access.
  - Chat history.
  - Agent control endpoints.

Design:

- Transport:
  - All external communication over TLS (rustls).
  - mTLS is mandatory between clients and the remote agent (already enforced by llama-monitor).
- Authentication:
  - Remote agent uses:
    - mTLS (client certificate + CA).
    - Bearer token (auto-generated or configured).
  - llama-server (e.g., http://192.168.2.16:8001): no auth from llama-monitor; treated as trusted on the network.
- For Android:
  - Store:
    - Bearer token and client certificate material in AndroidKeyStore-backed secure storage.
    - Never log or broadcast tokens.
- SSH:
  - Used by the dashboard for control:
    - Install/update the agent binary.
    - Start/stop/remove the agent.
    - Read the agent’s auth token.
    - Trust/verify the SSH host key.
  - For Android:
    - Option A (preferred): Use SSH directly (via ssh2 crate or equivalent) to manage the remote agent, then use HTTPS/mTLS for metrics.
    - Option B (if too heavy): Use a relay (e.g., your Mac/home server) that performs SSH to ryne; Android talks HTTPS to that relay.
    - In both cases, metrics traffic is over HTTPS/mTLS + bearer token.
- Data at rest:
  - Chat history:
    - Store in encrypted SQLite (rusqlite + SQLCipher or similar).
  - Config:
    - Store in Android’s private app directory.
- Network:
  - No public exposure:
    - Android client talks directly to ryne’s agent over HTTPS/mTLS.
    - No public endpoints, no open relay.
    - Use certificate pinning (optional but desirable) to prevent MITM.
  - llama-server:
    - Treated as trusted on the network; no additional auth from llama-monitor.

Security priorities:
- No plaintext endpoints.
- No hard-coded secrets.
- No unnecessary open ports on ryne.

## 4. Android System Monitoring (On-Device)

From research:

- Android limits direct access to low-level metrics:
  - CPU frequency/usage:
    - Partially available via /proc/stat, /sys/devices/system/cpu.
    - Often restricted by SELinux and vendor policies.
  - GPU stats:
    - Not reliably exposed; vendor-specific; many require root.
  - Thermal:
    - /sys/class/thermal:
      - Often readable for some zones.
      - Frequently restricted; values may be coarse or filtered.
  - Battery:
    - BatteryManager provides:
      - Level, voltage, temperature (coarse), charging status, health.
      - This is safe and reliable.

Conclusion:
- Realistic on-device metrics (no root):
  - Battery level, charging status, approximate temperature.
  - Basic CPU load (if /proc/stat accessible).
- Not realistic:
  - Accurate GPU stats.
  - Detailed thermal telemetry.
  - Deep motherboard / hardware metrics.

For llama-monitor use case:
- We are primarily monitoring remote systems (ryne), not the phone.
- On-device metrics are optional and limited:
  - Show:
    - Battery and charging status.
    - Approximate thermal state.
    - Basic load (if allowed).
  - Use them mainly for:
    - Context (e.g., “phone is hot” while streaming telemetry).
    - Power management decisions in the app.

Implementation:
- Use Android APIs:
  - BatteryManager for battery.
  - ThermalLogger / BatteryManager for thermal (where available).
- Avoid root or vendor hacks.

## 5. Cross-Compilation Strategy

We must compile Rust for Android (aarch64-linux-android) without breaking existing desktop builds.

Approach:

- Use:
  - cargo-ndk or a dedicated NDK toolchain config.
- Steps:
  - Install Android NDK.
  - Configure .cargo/config.toml with target-specific linker.
  - Build a static/shared library (cdylib) for Android.
  - Keep desktop binary as is.

Key crates:
- warp:
  - Pure Rust; compiles for Android.
- rusqlite:
  - Needs SQLite bundled or via system; we’ll bundle.
- rustls:
  - Pure Rust; good fit for Android.
- ssh2:
  - Heavy (vendored OpenSSL); required for full SSH control.
  - Options:
    - Include it for Android (preferred for consistency with desktop).
    - Or feature-gate it and use a relay for SSH initially.
  - Use cargo features to control inclusion on Android builds.
- sysinfo:
  - Works on Android; can be used for limited metrics.

Design rules:
- Use #[cfg(target_os = "android")] guards:
  - Exclude desktop-only components (tray, GPU backends).
  - Enable Android-specific behavior (service, WebView integration).

## 6. WebView and UI

We will reuse the existing web UI as much as possible.

Architecture:

- Android app:
  - Starts a local warp server (on 127.0.0.1) or serves from assets.
  - WebView loads:
    - http://127.0.0.1:<local_port> or
    - file:///android_asset/index.html (if we embed).
- Behavior:
  - WebSocket connects to:
    - Local Rust process (for live updates).
  - Rust process talks to:
    - llama-server directly for inference metrics and chat.
    - Remote agent on ryne via HTTPS/mTLS + bearer token for system/GPU metrics.

Foldable UX (Z Fold 6):

- Folded (front panel):
  - Compact, single-column view.
  - Show:
    - Connection status.
    - GPU summary (from ryne).
    - Current load (tokens/s, VRAM usage).
    - Alerts.
- Unfolded (inner screen):
  - Multi-panel layout:
    - Left: metrics, logs.
    - Right: chat, controls.
  - Use responsive CSS:
    - Adjust breakpoints for large screens.
    - Use existing UI components; enhance layouts.

Implementation:
- Detect screen size in JS:
  - Apply different layout classes.
- Optionally use Android foldable APIs via JNI:
  - Adjust UI when hinge angle changes.

Security:
- Ensure CSP allows only:
  - Trusted CDNs.
  - No open redirects.
- WebView:
  - Disable JavaScript access to unsafe origins.
  - Use secure context only.

## 7. Background Execution and Services

Android aggressively limits background processes. We must use a foreground service.

Design:

- Foreground service:
  - Persistent notification:
    - “Llama-Monitor active: connected to ryne.”
  - Keeps Rust runtime alive.
  - Continues:
    - Polling llama-server metrics.
    - Maintaining WebSocket to WebView.
- Behavior:
  - When app is in background:
    - Reduce polling frequency.
    - Keep connection to ryne alive.
  - When app is in foreground:
    - Increase polling frequency.
    - Stream live updates.

Battery considerations:
- Use adaptive polling:
  - Idle: every 10–30 seconds.
  - Active: every 1–5 seconds.
- Avoid continuous max-intensity polling.

## 8. Remote Agent on “ryne”

We assume:
- ssh nick@ryne is the primary remote host.
- llama-monitor --agent runs there.
- Latest merge to main fixes remote agent issues.

Role:
- Expose:
  - Llama-server metrics.
  - GPU stats.
  - System telemetry.
  - Logs.
- Accept:
  - Secure connections from:
    - Android app.
    - Mac desktop client.
    - Other trusted clients.

Security:
- TLS (rustls) with:
  - Proper certificate (CA-signed or self-signed with pinning).
- Auth:
  - API tokens or mTLS.
- No public exposure:
  - Bind to 127.0.0.1 or use SSH tunnel if needed.

Android integration:
- Android app:
  - Connects to ryne’s agent over HTTPS/mTLS + bearer token.
  - Uses tokens and client certs stored securely.
  - Either:
    - Uses SSH directly (via ssh2 crate) to manage the remote agent.
    - Or uses a relay to perform SSH on its behalf.

## 9. Phased Implementation Plan

Phase 1: Foundation
- Set up Android project:
  - Rust library (cdylib) for Android.
  - Basic Gradle build.
- Cross-compile:
  - Ensure core crates (warp, rusqlite, rustls, sysinfo) compile.
- Implement:
  - Minimal JNI bridge.
  - WebView loading existing UI from local server.

Phase 2: Remote Agent Integration
- Integrate connection to ryne’s agent:
  - TLS, auth, token storage.
  - Llama metrics polling.
- Stream metrics to WebView via WebSocket.
- Ensure UI displays remote telemetry correctly.

Phase 3: Android UX and Foldable Support
- Implement foreground service:
  - Persistent notification.
  - Background monitoring.
- Add responsive layouts:
  - Folded vs unfolded.
- Integrate on-device metrics:
  - Battery, thermal (basic).

Phase 4: Security Hardening
- Add:
  - Encrypted SQLite for chat history.
  - Certificate pinning (optional).
  - Token security via AndroidKeyStore.
- Review:
  - No plaintext endpoints.
  - No hard-coded secrets.

Phase 5: Advanced Features
- Add:
  - Widgets.
  - Lock-screen widgets.
  - Quick settings toggles.
- Enhance SSH support (if not already included).
- Add advanced security features (certificate pinning, etc.).

## 10. Risks and Mitigations

- Build complexity:
  - Risk: Cross-compilation failures, especially with ssh2/OpenSSL.
  - Mitigation: Feature-gate SSH; use cargo-ndk; iterate carefully.
- WebView security:
  - Risk: CSP/origin issues.
  - Mitigation: Lock down allowed origins; test extensively.
- Background execution:
  - Risk: Android killing our process.
  - Mitigation: Use foreground service; persistent notification.
- No local GPU telemetry:
  - Risk: Users expect GPU monitoring on phone.
  - Mitigation: Make it clear GPU stats come from ryne’s agent; design UI to show source.

## 11. Next Steps

- Confirm:
  - This architecture and security model are acceptable.
- Decide:
  - Initial auth method (tokens vs mTLS).
  - Whether to use HTTPS-only for Android or include SSH.
- Begin:
  - Setting up Android project and cross-compilation.
  - Adjusting llama-monitor code with Android-specific guards.

End of document.