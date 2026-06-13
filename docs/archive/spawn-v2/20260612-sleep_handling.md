# Sleep Handling & Low-Power Mode — Implementation & Architecture Plan

Date: 2026-06-12
Status: Draft (for implementation by future agents)

Purpose:
- Define how llama-monitor can enter a "sleep / low-power" mode where:
  - The llama-server continues running at full capability.
  - The llama-monitor backend and browser UI drastically reduce or stop all non-essential work (telemetry, GPU reads, WebSocket broadcasts, animations, etc.).
- Define how this integrates with:
  - Manual user control (quick, discoverable, premium UX).
  - Auto-sleep (tab hidden, user idle).
  - Browser close / reopen semantics (so returning users see their running server and chats without confusion).

This document must be treated as canonical context for any future agent implementing or modifying sleep handling.

--------------------------------------------------------------------------------
1. High-Level Concept

We are introducing a sleep handling model with three operational modes:

- Active:
  - Normal operation.
  - Full telemetry, GPU monitoring, WebSocket pushes, live UI, sparklines, animations.

- Idle:
  - Intermediate mode.
  - Reduced telemetry, slower polling, fewer UI updates.
  - Still provides "dashboard-lite" visibility.

- Sleep:
  - Minimal activity.
  - llama-server remains fully alive and externally usable.
  - llama-monitor backend:
    - Keeps HTTP server alive.
    - Keeps llama-server child, log capture, death watcher.
    - Pauses or drastically throttles:
      - GPU metrics polling.
      - System metrics polling.
      - Llama metrics polling.
      - Remote agent polling.
      - WebSocket broadcasts (long intervals).
  - Frontend (if open):
    - Freezes live telemetry and visualizations.
    - Minimal redraws.
  - Used when:
    - The user is not in the llama-monitor UI.
    - They are busy with other apps.
    - They are explicitly running llama-server externally.

Crucial rule:
- Sleep mode NEVER kills, restarts, or detaches the llama-server.
- Sleep mode NEVER shuts down the backend HTTP listener or the tokio runtime.

--------------------------------------------------------------------------------
2. Goals

- Resource savings:
  - Reduce CPU wakeups and memory traffic when the UI is not actively observed.
- Compatibility:
  - Fully compatible with external consumers of llama-server (other apps, scripts).
- UX:
  - Simple, premium control for toggling sleep / idle manually.
  - Automatic transitions based on tab visibility / user inactivity.
  - On browser reopen: user sees their running server and prior chats, without needing to re-spawn or reattach manually.

- Implementation constraints:
  - No invasive refactors.
  - Reuse existing hooks:
    - Active session flags
    - Page Visibility API
    - Existing "Battery Saver" refresh rate logic
    - Existing WebSocket client-visibility message

--------------------------------------------------------------------------------
3. What Must Always Stay Alive (in all modes)

These components are essential for stability, safety, and external access:

- Always ON:
  - Tokio runtime and main event loop.
  - Warp HTTP/TLS server (listening on configured port).
    - Must always accept:
      - New browser connections.
      - External clients using llama-server endpoints.
  - Llama-server child:
    - Child process.
    - stdout/stderr capture.
    - Death watcher (detects crashes/OOM/signals).
  - Chat DB:
    - Read/write for chat message streaming and persistence.
  - Session management:
    - Persistence of sessions (30s autosave).
    - Hourly DB maintenance, daily backups, ACME renewal.
  - Basic health:
    - A very lightweight health awareness so:
      - We know if llama-server is alive or dead.
      - We can show this when the user returns.

Rationale:
- If any of these are turned off, we risk:
  - Silently killing llama-server.
  - Losing the ability to detect crashes.
  - Breaking external clients.
  - Confusing the user when they return.

--------------------------------------------------------------------------------
4. What Can Be Frozen or Throttled in Sleep Mode

These components are candidates for sleep-mode control:

- GPU metrics poller:
  - Currently: every ~200–500 ms, invokes nvidia-smi/rocm-smi/sysctl/etc.
  - In Sleep: skip or slow drastically (e.g., every 10–30s or only on request).
  - Reason: highest CPU wake / power impact.

- System metrics poller:
  - Currently: 5s interval via sysinfo.
  - In Sleep: extend to 10–30s, or skip unless Active.

- Llama metrics poller:
  - Currently: every N seconds, GETs /health, /metrics, /slots, /v1/models.
  - Already gated: if no active_session_id → sleep until notified.
  - In Sleep: even with an active session, either:
    - Pause polling, or
    - Use a very slow interval (e.g., 10–30s).
  - Wake on:
    - Explicit telemetry request.
    - New WebSocket connection from browser.

- Remote agent poller:
  - Similar to llama poller.
  - In Sleep: throttle or pause.

- WebSocket broadcast:
  - Currently: configurable (default 500ms), already slows to ≥5000ms when tab is hidden.
  - In Sleep: further clamp to 10–30s, send minimal heartbeat instead of full telemetry payload.

- Frontend live telemetry:
  - Sparklines, GPU charts, throughput numbers, logs:
    - Stop or severely throttle updates in Sleep.
  - Retain connection and minimal presence.

- Periodic, low-cost tasks (safe to leave as-is):
  - Session autosave (30s).
  - DB maintenance (1h).
  - DB backup (24h).
  - ACME renewal (24h).
  - These are rare and cheap; no need to complicate with sleep mode.

--------------------------------------------------------------------------------
5. Core Architecture: sleep_mode Flag

We'll introduce a centralized sleep mode mechanism in AppState so all components can consult it.

5.1 AppState extension

Add:

- pub sleep_mode: Arc<tokio::sync::watch::Sender<bool>>
  - true: sleep mode is active
  - false: normal mode

- pub sleep_mode_config: Arc<Mutex<SleepModeConfig>>
  - Config struct controlling behavior:

struct SleepModeConfig {
    pub auto_sleep_when_all_hidden: bool,      // auto-enter when no visible clients
    pub auto_sleep_idle_secs: Option<u64>,     // auto-enter after X seconds of global inactivity (e.g., 600s)
    pub sleep_gpu_interval_secs: u64,          // interval for GPU poller when asleep
    pub sleep_sys_interval_secs: u64,          // interval for sys poller when asleep
    pub sleep_llama_interval_secs: u64,        // llama poller interval when asleep
    pub sleep_ws_interval_ms: u64,             // WS broadcast interval when asleep
}

- These can be persisted via ui-settings.json (or a new sleep-config section).

5.2 Wake-on-activity

Define wake conditions:

- New WebSocket connection from browser.
- Any API call that implies "user wants full telemetry" (e.g., /api/gpu, /api/metrics).
- A user-initiated action (chat send, session start, manual mode change).
- Frontend explicitly sends a "wake" message (e.g., when tab becomes visible).

Implementation pattern:

- In affected routes:
  - If sleep_mode is true and the request is from the UI or a telemetry-aware client:
    - Set sleep_mode = false.

- In WebSocket handler:
  - On open:
    - If there are connections, disable sleep_mode (or re-evaluate).
  - On close:
    - If no connections left:
      - Optionally re-enable sleep_mode based on config.

- Llama-server itself:
  - Its presence does not prevent sleep; we want sleep while it's running.

5.3 Interaction with llama_poll_notify

We already use llama_poll_notify to pause the llama metrics and remote agent pollers when no active session is set.

For sleep mode:

- Use the same notify mechanism:
  - When sleep_mode becomes true:
    - Call llama_poll_notify.notify_waiters() so pollers can re-evaluate and enter sleep.
  - When sleep_mode becomes false:
    - Call llama_poll_notify again to wake them.

- Each poller loop top:
  - Check:
    - active_session_id?
    - sleep_mode?
  - Decide whether to run or sleep.

--------------------------------------------------------------------------------
6. Per-Component Behavior

6.1 GPU Poller (src/main.rs)

Current behavior:
- Runs in its own thread:
  - If active_session_uses_local_metrics(): read_metrics()
  - Uses interval tied to ws_push_interval_ms.

Sleep-mode behavior:
- At top of loop:
  - let asleep = state.sleep_mode.borrow_and_update();
  - if asleep:
      thread::sleep(Duration::from_secs(cfg.sleep_gpu_interval_secs));
      continue;
- Otherwise, proceed as normal.

6.2 System Metrics Poller (src/main.rs)

Similar pattern:

- At top of loop:
  - let asleep = state.sleep_mode.borrow_and_update();
  - if asleep:
      thread::sleep(Duration::from_secs(cfg.sleep_sys_interval_secs));
      continue;
- Else proceed as normal.

6.3 Llama Metrics Poller (src/llama/poller.rs)

Current behavior:
- Gated: if !active_session_id → await llama_poll_notify.

Sleep-mode behavior:
- In main loop:
  - If asleep:
    - Use longer interval or await llama_poll_notify.
  - If awake:
    - Use normal interval.
- On sleep_mode change:
  - notify_waiters() to let it re-check.

6.4 Remote Agent Poller (src/agent.rs)

Same as llama metrics:
- If asleep:
  - Use longer interval or await notify.
- If awake:
  - Normal.

6.5 WebSocket Broadcast (src/web/ws.rs)

Current behavior:
- Adapts interval based on client-visible.

Sleep-mode behavior:
- When asleep:
  - Clamp interval to cfg.sleep_ws_interval_ms (e.g., 10_000 or 15_000).
  - Optionally:
    - Send a reduced payload (heartbeat + session status + critical flags only).
- When awake:
  - Use configured interval and full payload.

Implementation notes:
- In per-connection loop:
  - Before sleep(interval_ms):
    - let asleep = state.sleep_mode.borrow_and_update();
    - if asleep && effective_interval_ms < cfg.sleep_ws_interval_ms:
        effective_interval_ms = cfg.sleep_ws_interval_ms;

6.6 Backend auto-sleep timer

Add a background task that auto-manages sleep_mode based on activity:

Logic:

- Track:
  - Number of WebSocket connections.
  - Last API request timestamp that implies user presence.
  - Optionally: whether any connection reported "visible" in last X seconds.

Pseudocode:

- Interval: e.g., 30–60s.
- If:
  - sleep_mode is false, AND
  - no WebSocket connections (or all report hidden), AND
  - last_activity_elapsed > cfg.auto_sleep_idle_secs
- Then:
  - sleep_mode = true.

On new activity (WS open, API use, chat send):
- sleep_mode = false.

This ensures:
- User closes tab, walks away:
  - After idle timeout → app goes to sleep.
- User returns:
  - Opens tab → auto-wake.

--------------------------------------------------------------------------------
7. Frontend: Visibility, Sleep, and Behavior

We'll integrate sleep handling into:
- The existing Page Visibility API logic.
- A new sleep-mode message type over WebSocket.

7.1 Existing behavior (summary)

- On tab hidden:
  - isTabVisible = false.
  - body.power-saver added.
  - Skips updateDashboard() DOM writes.
  - Sends { type: 'client-visibility', visible: false }.
- On tab visible:
  - isTabVisible = true.
  - Removes body.power-saver.
  - Runs one full updateDashboard().
  - Sends { type: 'client-visibility', visible: true }.
- Backend currently uses this to slow pushes when hidden.

7.2 New client-visibility semantics

We extend the client-visibility message to include a mode:

{
  type: 'client-visibility',
  visible: true | false,
  mode: 'active' | 'idle' | 'sleep'
}

Rules (frontend):

- active:
  - Tab is visible AND user has interacted recently.
- idle:
  - Tab visible but no activity for X seconds (configurable).
- sleep:
  - Tab hidden OR user explicitly in sleep mode.

Implementation (frontend, in dashboard-ws.js or a dedicated power-saver.js):

- Track:
  - isTabVisible (from visibilitychange).
  - lastInteractionTime (from mouse/keyboard/touch).
- Decide mode:
  - If !isTabVisible:
    - mode = 'sleep'
  - Else if (now - lastInteractionTime) > idle_timeout:
    - mode = 'idle'
  - Else:
    - mode = 'active'
- Send client-visibility with current mode.
- On mode change to 'sleep'/'idle':
  - Optionally add class:
    - body.power-saver (sleep)
    - body.power-idle (idle)

Backend (ws.rs) behavior:

- Maintain per-connection mode.
- When deciding interval:
  - If any connection is 'active':
    - Prefer normal interval.
  - Else if any connection is 'idle':
    - Use intermediate interval.
  - Else (all 'sleep' or no connections):
    - Use sleep interval.

This avoids forcing sleep while someone is watching.

7.3 Backend sleep mode broadcast

The backend should broadcast the current sleep mode to the frontend:

- In the WS payload, include:
  - sleep_mode: true | false

Frontend can:

- Freeze telemetry:
  - If sleep_mode is true:
    - Do not update GPU/telemetry panels.
    - Do not render sparkline points.
    - Reduce animation complexity.
  - If sleep_mode is false:
    - Restore normal updates.

7.4 Chat streaming and generation

Rules:

- If a generation is in progress (model is generating tokens via streaming):
  - Treat this as “Active”:
    - Do not auto-sleep.
    - Frontend can mark mode as 'active'.
- This is detected via:
  - wsData indicating generation_tokens_per_sec > 0, or equivalent flags.

--------------------------------------------------------------------------------
8. UI/UX: Manual Control of Sleep Mode

We need an elegant way for users to:
- Manually enter/exit sleep mode.
- Configure auto-sleep policies.
- Understand what is happening without reading docs.

8.1 Primary control: Nav cockpit pill (EXTEND existing)

Rationale:
- The nav-cockpit is already:
  - Always visible.
  - Associated with system/telemetry status.
- Extending it keeps UI clean and premium.

Behavior:

- Replace/enrich the existing state pill with a mode pill:
  - "Active"
  - "Low Power" (sleep mode engaged)
  - "Auto" (auto-sleep enabled, system-managed)

- Interaction:
  - Click cycles: Active → Low Power → Auto → Active
  - Tooltip:
    - Active: "Full telemetry and UI updates."
    - Low Power: "Telemetry minimized; llama-server stays running."
    - Auto: "Automatically enables Low Power when hidden or idle."

- Styling:
  - Active: neutral/green accent.
  - Low Power: amber/yellow accent.
  - Auto: blue accent.

- Accessibility:
  - Use aria-label:
    - "Connection mode: Low Power. Telemetry minimized."
  - Keyboard-focusable.

8.2 Secondary control: Settings → Performance tab

In existing Settings → Performance section (where "Dashboard Refresh Rate" already lives), add:

- "Sleep Mode" section:
  - Toggle:
    - "Enable Auto Sleep"
  - Options:
    - "Auto-sleep when tab is hidden" (default on).
    - "Auto-sleep after no activity for:" [3 / 5 / 10 / 30 minutes].
  - Short explanation:
    - "Sleep Mode minimizes telemetry and UI activity while keeping your llama-server running."

This is where power users can tune behavior without cluttering the main UI.

8.3 Optional: Tray popover / system tray

For desktop-centric workflows, add:

- One row in the tray popover (compact.html):
  - "Low Power Mode: [On/Off]"
- This:
  - Calls a backend endpoint: POST /api/sleep-mode/toggle.
- Optional, for future polish.

8.4 Backend endpoints for manual control

Add endpoints:

- GET /api/sleep-mode
  - Returns current mode and config summary.
- POST /api/sleep-mode/toggle
  - Toggles sleep_mode on/off.
  - Requires api-token.
- POST /api/sleep-mode/set
  - Body:
    - { "enabled": true | false }

This lets:
- Frontend cockpit pill control it.
- Tray popover control it.
- External scripts control it.

--------------------------------------------------------------------------------
9. Browser Close / Reopen Behavior

An important design requirement: when a user closes their browser, sleeps, and later returns to llama-monitor (e.g., http://localhost:7778), the experience must be coherent, aware of the running llama-server, and respect auth if enabled.

9.1 Current behavior (summary)

- Backend:
  - Serves same index.html on "/".
  - No special logic to detect running llama-server or suggest resume.
  - Active session is kept in memory.

- Frontend (bootstrap + nav):
  - Always starts in setup view.
  - initChatTabs:
    - Loads existing chat tabs from backend.
  - initSetupView:
    - Shows setup screen.
    - loadRecentSessions:
      - Loads /api/sessions/recent.
      - If there are attach sessions and server is reachable, marks them as "Running" and shows "Reconnect/Resume".
  - initAttachDetachButtons:
    - Calls /api/sessions/active.
    - If status == "Running", calls switchView('monitor').
  - WebSocket (dashboard-ws.js):
    - On open, reads:
      - session_mode, server_running.
    - If attach mode + server_running + view == 'setup' → switchView('monitor').

Result:

- If llama-server is running and there is an active session with status Running:
  - On reopen → monitor view is restored.
- If llama-server is running but no active session (or inactive):
  - On reopen → setup view is shown.
  - Recent endpoints appear; user must manually "Reconnect/Resume".
- Chat tabs are loaded into memory but may be hidden (setup view).
- Auth:
  - With auth:
    - Form auth: login screen → then normal flow.
    - Basic auth: browser prompts → then normal flow.

This is mostly OK, but insufficient for a clean "sleep → wake → resume" experience.

9.2 Required behavior for sleep mode

We must ensure:

- When the user:
  - Has llama-server running,
  - Uses sleep mode or closes the browser,
  - Then returns,
- They see:
  - That their server is alive.
  - That their chats exist.
  - A straightforward path back into their prior context.

9.3 Backend: restore hint endpoint

Add an endpoint to let the frontend decide how to handle reopen:

- GET /api/sessions/restore-hint

Response (simplified):

{
  "server_running": true,
  "has_active_session": true,
  "active_session_id": "abc123",
  "active_session_status": "Running",
  "has_chat_tabs": true,
  "suggested_action": "resume_active" | "suggest_recent_attach" | "none"
}

Logic:

- If:
  - server_running == true
  - active_session exists
  - active_session_status == "Running"
- Then:
  - suggested_action: "resume_active"

- Else if:
  - server_running == true (from a recent attach),
  - no active session,
- Then:
  - suggested_action: "suggest_recent_attach"

- Else:
  - suggested_action: "none"

This is read-only, metadata-only, cheap.

9.4 Frontend: reopen-aware restore

On page load, after auth (if any) and before fully committing to setup view:

1) Call /api/sessions/restore-hint

2) Behavior:

- If suggested_action == "resume_active":
  - Immediately switchView('monitor').
  - Restore chat tabs.
  - Optionally restore previous nav/last tab (via restorePreviousPosition).

- If suggested_action == "suggest_recent_attach":
  - Show setup view.
  - At top, show a subtle banner or card:
    - "We detected a running server from your recent session. Resume?"
  - On click:
    - Attach to that session.
    - Restore chats.

- If suggested_action == "none":
  - Default setup view behavior.

This ensures:
- When llama-server was running and the user closes their browser:
  - They return to a familiar context without needing to re-attach.
- This integrates cleanly with sleep mode:
  - Sleep mode only reduces activity while llama-server runs.
  - When the user returns, we restore context automatically.

9.5 Auth and reopen

Rules:

- With auth enabled:
  - On reopen:
    - Prompt for auth (form or basic).
    - After login:
      - Run same restore-hint flow as above.
- With auth disabled:
  - Same restore-hint flow, no prompt.
- No behavior should change depending on auth beyond the initial login step.
- Sleep mode does not interfere with auth:
  - It only affects telemetry, UI, and pollers.

--------------------------------------------------------------------------------
10. Auto-Sleep Conditions (When Sleep Activates Automatically)

Sleep mode can activate automatically based on system conditions.

10.1 All clients hidden or no clients

Trigger:
- No WebSocket connections, or
- All WebSocket clients have sent client-visibility with mode 'sleep' or visible: false for more than X seconds (e.g., 60s).

Action:
- Backend sets sleep_mode = true.

Rationale:
- No one is actively looking at llama-monitor.
- Llama-server can keep running; we save resources.

10.2 Idle timeout

Trigger:
- sleep_mode_config.auto_sleep_idle_secs is set (e.g., 600 seconds).
- No user activity for that duration:
  - No new messages.
  - No WebSocket interactions implying UI presence.
  - No telemetry API calls from browser.

Action:
- sleep_mode = true.

Rationale:
- User left their desk or is busy elsewhere.

10.3 Wake-up on return

Sleep mode must clear automatically when:

- A new WebSocket connection is established from browser.
- An existing client sends client-visibility with visible: true / mode: 'active'.
- A new message is sent through llama-server via llama-monitor.
- User toggles cockpit pill to Active.

Implementation:
- In the relevant handlers:
  - Set sleep_mode = false.
  - If needed: llama_poll_notify.notify_waiters().

--------------------------------------------------------------------------------
11. Persistence and Configuration

Sleep mode preferences should be persisted (so they survive reloads) and tunable.

11.1 Where to store

Extend ui-settings.json with a sleep_mode section:

{
  "sleep_mode": {
    "auto_sleep_when_all_hidden": true,
    "auto_sleep_idle_secs": 600,
    "sleep_gpu_interval_secs": 15,
    "sleep_sys_interval_secs": 15,
    "sleep_llama_interval_secs": 15,
    "sleep_ws_interval_ms": 10000
  }
}

- Load on startup.
- Update via PUT /api/settings or a dedicated endpoint.
- Use defaults if missing.

11.2 Recommended defaults

- auto_sleep_when_all_hidden: true
- auto_sleep_idle_secs: 600
- sleep_gpu_interval_secs: 15
- sleep_sys_interval_secs: 15
- sleep_llama_interval_secs: 15
- sleep_ws_interval_ms: 10000

These are conservative:
- Still allow the backend to detect if the server is down.
- Avoid excessive wakeups.

--------------------------------------------------------------------------------
12. Implementation Phases (Recommended)

Implement incrementally. Each phase should be self-contained and fully backward-compatible.

Phase 1: Backend sleep_mode flag + core guards

- Add:
  - sleep_mode (watch channel)
  - sleep_mode_config
- Guard:
  - GPU poller
  - System metrics poller
  - Llama metrics poller
  - Remote agent poller
- Integrate:
  - llama_poll_notify usage for sleep/wake.
- Add:
  - /api/sleep-mode/toggle and /api/sleep-mode/set

Phase 2: WebSocket + frontend visibility integration

- Extend client-visibility handling:
  - Use modes active/idle/sleep.
- Adjust intervals:
  - Based on mode and sleep_mode flag.
- Frontend:
  - Add lastInteractionTime tracking.
  - Adjust modes in dashboard-ws.js or power-saver.js.
- Backend broadcasts sleep_mode in WS payload.
- Frontend uses sleep_mode:
  - To throttle UI updates, sparklines, telemetry rendering.

Phase 3: Manual UI controls

- Extend nav-cockpit pill:
  - Support Active / Low Power / Auto modes.
  - Wire to /api/sleep-mode endpoints.
- Add controls to Settings → Performance:
  - Auto-sleep toggles and timeouts.

Phase 4: Browser reopen-aware restore

- Implement:
  - /api/sessions/restore-hint.
  - Frontend reopen-aware behavior:
    - Resume active session.
    - Or suggest resume for a running attach session.
- Integrate:
  - With auth flow (form auth / basic auth).

Phase 5: Polish and optional enhancements

- Add tray popover sleep toggle.
- Tune intervals and timeouts based on telemetry.
- Optional: small "Sleep Mode" indicator in status bar for clarity.

--------------------------------------------------------------------------------
13. Security and Constraints

- Sleep mode:
  - Must not weaken or bypass auth.
  - Must not expose internal endpoints when asleep.
- Endpoints:
  - /api/sleep-mode/*:
    - Require api-token.
- llama-server:
  - No extra permissions or exposures while asleep.
- Rate limiting:
  - Any new wake-up or sleep toggle endpoints should respect existing cooldowns / rate limits to avoid abuse.

--------------------------------------------------------------------------------
14. Notes for Future Agents

- Treat this plan as authoritative.
- Any deviation (e.g., removing sleep from a critical path, killing llama-server, changing restore behavior) must be justified and documented.
- If extending features:
  - Check how they interact with sleep_mode.
  - Ensure no regressions:
    - llama-server always stays alive unless explicitly killed.
    - User can always reconnect and resume without confusion.
- When implementing:
  - Run existing e2e tests.
  - Ensure auth_routing tests still pass.
  - Add 1–2 focused tests:
    - sleep_mode toggles correctly.
    - restore-hint returns correct action.
