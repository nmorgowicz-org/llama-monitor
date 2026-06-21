# Low-Power Mode With Logging (3-Tier Design)

**Date:** 2026-06-19
**Status:** Proposed
**Priority:** Medium
**Scope:** Sleep/low-power modes, WS broadcast, poller intervals, API endpoints, UI settings

This plan assumes the API refactor end-state from
`docs/plans/20260616-api_rs_refactor.md`.

## Problem

Currently:

- `sleep_mode` is binary (on/off).
- When asleep, the WebSocket broadcast sends only a minimal heartbeat (T-049) and drops:
  - console/server logs
  - llama metrics
  - GPU/system metrics
- There is no intermediate mode that:
  - Throttles or skips metrics to save resources
  - Still streams llama-server console logs so the user can monitor loading, context, or errors

This is undesirable because:
- Users want low-power behavior when away.
- Users also want to watch console logs even while metrics are reduced.
- Today, turning on sleep mode cuts logs — so users either keep everything full or lose visibility into server output.

## Goal

Introduce three tiers of operation instead of two:

1. **Idle (normal):** full metrics, full logs, normal intervals.
2. **LowPower (new, middle-ground):** metrics throttled/old, console logs still streamed, moderate WS interval.
3. **Asleep (full sleep):** minimal heartbeat, no logs, slow intervals (current asleep behavior).

## Non-Goals

- Do not change route paths.
- Do not alter security/auth rules.
- Do not introduce new concepts into the upstream/inference layer.
- Do not rework all UI code; specify only the interfaces the UI must respect.

## Modes

### Idle (Normal)

- All metrics: live.
- All logs: streamed.
- WS interval: current normal value.
- GPU/sys/llama pollers: normal intervals.
- Triggered:
  - Default at startup
  - On user activity or tab visibility
  - Immediately on reconnect if user was idle but not long-enough to qualify for full sleep

### LowPower (New)

Intention: metrics are cheap and throttled, but console logs are visible in real time.

- Metrics:
  - GPU, system, llama metrics: use same slow intervals as current asleep mode.
  - WS payload may still include metrics, but they are stale-throttled rather than live.
- Logs:
  - Still streamed to WS clients.
  - This is the key change: `server_logs` are included even in low-power.
- WS interval:
  - Faster than asleep, slower than full: e.g., 2–3 seconds (configurable or fixed).
- Triggered:
  - Automatically after a short idle period (configurable, e.g., 60–120 seconds).
  - Or explicitly by user (UI toggle for "low-power").
  - On reconnect when tabs open but user hasn't been active and some idle time has passed (optional, implementation-dependent).

### Asleep (Full Sleep)

Intention: system is effectively idle; minimize everything.

- Metrics:
  - Throttled to slowest intervals.
  - May be excluded from WS payload.
- Logs:
  - Not streamed (kept only in buffer in case they're needed later).
- WS payload:
  - Minimal heartbeat with only essential flags:
    - `sleep_mode` / `power_mode`
    - `sleep_mode_manual` / `power_mode_manual`
    - `server_running`, `local_server_running`
    - `active_session_id`, `active_session_status`
- Triggered:
  - Automatically when all tabs are hidden and configured interval passes.
  - Automatically when idle beyond a longer threshold (e.g., 10 minutes), if configured.
  - Explicitly by user (current sleep toggle behavior becomes "asleep").

## Design (Post-Refactor Layout)

Based on the end-state of the `api_rs_refactor.md` plan:

- The `api/upstream.rs` currently contains sleep mode endpoints; after refactor completion they should move out.
- The natural home for power/sleep-related routes is:
  - `api/sessions.rs` (close to session/monitoring), or
  - A new `api/power.rs` if it grows; for now, sessions is fine.

We will design as if:

- `AppState` lives in `src/state.rs`.
- WS broadcast lives in `src/web/ws.rs`.
- Power/sleep endpoints live under the API module tree (post-refactor).

### State Changes (`src/state.rs`)

Replace the binary `sleep_mode` flag with a 3-state enum.

Current:

- `pub sleep_mode: Arc<AtomicBool>`
- `pub sleep_mode_manual: Arc<AtomicBool>`
- `pub sleep_mode_config: Arc<Mutex<SleepModeConfig>>`
- `pub sleep_notify: Arc<tokio::sync::Notify>`

Proposed:

- Introduce:

  - `PowerMode` enum:
    - `Idle`
    - `LowPower`
    - `Asleep`

- Use:
  - `pub power_mode: Arc<AtomicU8>` (or a wrapper type) instead of `sleep_mode`.
  - `pub power_mode_manual: Arc<AtomicBool>` (true if user explicitly set current mode).
  - `pub sleep_notify: Arc<tokio::sync::Notify>` (reuse or rename if desired).

Or, to minimize atomic complexity, we can:

- Keep it as:
  - `pub sleep_mode: Arc<AtomicBool>` — high-level asleep/not-asleep
  - `pub low_power_mode: Arc<AtomicBool>` — new intermediate mode
  - `pub power_mode_manual: Arc<AtomicBool>` — true when user set mode manually
- Derive effective mode:
  - If `sleep_mode`:
    - → `Asleep`
  - Else if `low_power_mode`:
    - → `LowPower`
  - Else:
    - → `Idle`
- This avoids introducing complex atomic-enum patterns while remaining straightforward.

Either approach is acceptable; the second is simpler and avoids heavy changes during the refactor.

The rest of this plan assumes this derived 3-tier behavior.

### Config Changes (`SleepModeConfig`)

Update `SleepModeConfig` (in `src/state.rs`, persisted via `ui-settings.json`):

Add fields for the low-power tier and transition thresholds:

- Keep:
  - `auto_sleep_when_all_hidden: bool`
  - `auto_sleep_idle_secs: Option<u64>` (for Asleep)
  - `sleep_gpu_interval_secs: u64`
  - `sleep_sys_interval_secs: u64`
  - `sleep_llama_interval_secs: u64`
  - `sleep_ws_interval_ms: u64`

- Add:
  - `low_power_enabled: bool` (default `true`: allow low-power mode)
  - `low_power_idle_secs: Option<u64>` (time before switching to LowPower when idle)
  - `low_power_ws_interval_ms: u64` (WS interval in LowPower, e.g., 2000–3000 ms)
  - `low_power_keep_logs: bool` (default `true`: keep streaming logs in LowPower)

Backward compatibility:

- All new fields use `#[serde(default)]` with safe defaults.
- Existing configs automatically get Idle/LowPower/Asleep behavior via derived fields without requiring migration.

### WS Broadcast (`src/web/ws.rs`)

Key change: when in LowPower, keep logs in the payload.

Current asleep logic:

- If asleep:
  - Slow interval.
  - Minimal payload (no logs, no llama/gpu/system).

New logic (conceptual):

- Determine effective mode:
  - `Asleep` if `sleep_mode == true`
  - `LowPower` if `!sleep_mode && low_power_mode == true`
  - `Idle` otherwise

- Interval:
  - Idle: use normal WS interval.
  - LowPower:
    - Use `low_power_ws_interval_ms` (e.g., 2–3s), clamped reasonably.
  - Asleep:
    - Use current slow asleep interval (e.g., 10s).

- Payload:

  - Idle:
    - Full payload (current behavior): metrics + logs + everything.

  - LowPower:
    - Include:
      - `logs`
      - Basic server/session status
      - Optional: last known llama/gpu/system values (throttled, but acceptable)
    - This preserves console logs visibility while the system is on reduced polling.

  - Asleep:
    - Same minimal heartbeat as current behavior:
      - `sleep_mode`, `power_mode`, `server_running`, `local_server_running`,
      - `active_session_id`, `active_session_status`
    - No logs.

Implementation notes:

- Introduce a helper function:
  - `fn effective_power_mode(state: &AppState) -> PowerMode` (or equivalent)
- Use it in:
  - WS interval selection
  - WS payload selection
  - Any sleep/asleep-related guards

This keeps the logic centralized.

### Pollers and Background Loops

Use the same mode to slow or skip metrics without affecting log capture.

- `src/main.rs` (GPU and system-metrics pollers):
  - If Asleep:
    - Use slow intervals (current behavior).
  - If LowPower:
    - Use same slow intervals as Asleep (metrics throttled).
  - Idle:
    - Normal.

- `src/llama/poller.rs` and `src/agent.rs` (llama metrics):
  - If Asleep:
    - Use slow llama poll interval.
  - If LowPower:
    - Also use slow interval.
  - Idle:
    - Normal.

Log capture (`push_log`):

- Always runs.
- Not gated by any mode.
- LowPower only affects whether logs are included in the WS broadcast.

### Auto-Transition Logic (`src/main.rs`)

Current: auto-sleep after long idle.

New: two-step transition:

- Introduce a small background loop (or adjust existing) to manage transitions:

  - If:
    - `low_power_enabled`
    - idle time >= `low_power_idle_secs`
    - not currently Asleep
    - not streaming
    - not manually set to Idle (or allow override)
  - Then:
    - Enter LowPower mode: `low_power_mode = true`

  - If:
    - idle time >= `auto_sleep_idle_secs`
    - and not streaming
    - and not actively connected/visible
  - Then:
    - Enter Asleep mode: `sleep_mode = true`

On user activity, visibility, or reconnect:

- Immediately clear LowPower.
- If auto-sleep only (not manual), clear Asleep.
- Keep any manual flags respected.

Design constraint:

- This should not introduce heavy logic into the middle of the refactor; just extend the existing auto-sleep loop with a second threshold.

### API Endpoints

After the refactor, sleep-mode endpoints will move out of `api/upstream.rs`. They should expose the new mode rather than only `sleep_mode`.

Recommended location: `api/sessions.rs` or a new `api/power.rs`.

Endpoints (paths unchanged conceptually; implementation may adapt):

- `GET /api/sleep-mode`
  - Returns:
    - `enabled` (legacy field: true if asleep)
    - `mode` (new: "idle" | "low-power" | "asleep")
    - `manual` (user set)
    - `config` (current config)

- `POST /api/sleep-mode/toggle`
  - Toggles asleep (backward compatible):
    - If Asleep → Idle
    - If not Asleep → Asleep (skips LowPower on manual toggle for simplicity)
  - Marks as manual when used.

- `POST /api/sleep-mode/set`
  - Accepts:
    - `enabled` (legacy bool)
    - `mode` (new optional: "idle" | "low-power" | "asleep")
  - Behavior:
    - If `mode` is provided:
      - Set exactly that mode.
    - Else if only `enabled`:
      - Treat `enabled=true` → Asleep
      - `enabled=false` → Idle (for backward compatibility).

This:

- Preserves existing clients.
- Gives the UI new control.
- Is straightforward to implement post-refactor in a dedicated module.

### Backward Compatibility

Must not break existing clients:

- Keep:
  - `sleep_mode` top-level behavior (asleep/not asleep)
  - Existing JSON shapes as much as possible
- New fields are additive:
  - `low_power_mode` (bool)
  - `mode` (string)
  - Config additions with `#[serde(default)]`

If any existing code checks `sleep_mode.load()`:

- Where it guards "is system in full sleep?" keep as is.
- Where it guards "is system reduced-power?" treat both `Asleep` and `LowPower` as reduced (for interval logic).

## Summary of Behavior (End-State)

- Idle:
  - Full metrics, full logs.
- LowPower (new):
  - Metrics: throttled
  - Logs: streamed
  - Intended: user idle, tab possibly open, or low-power requested.
- Asleep:
  - Metrics: throttled/omitted
  - Logs: not streamed
  - Intended: fully away, all tabs hidden or long idle.

This gives:

- A safe, low-impact mode for extended inactivity (Asleep).
- A practical mode for when the user wants to reduce metrics impact but keep an eye on console logs (LowPower).
- Clear, localized hooks for all behavior once the API refactor is complete.

## Impact on Ongoing Refactor

Minimal and safe:

- No route path changes.
- All changes are additive on top of existing sleep-mode hooks.
- After refactor:
  - Move sleep-mode/power-mode endpoints out of `api/upstream.rs` into their proper module (e.g., `api/sessions.rs` or `api/power.rs`).
  - Use centralized `effective_power_mode` in WS and pollers.
  - Keep `push_log` unconditional.
