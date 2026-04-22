# Implementation Plan: UI/UX and Monitoring Improvements

**Date:** 2026-04-20  
**Source:** docs/20260419-ui-ux-and-monitoring-roadmap.md  
**Status:** Ready for implementation  
**Target Context Window:** ~131,000 tokens per iteration

---

## Overview

This document breaks down the roadmap into small, testable, and achievable tasks. Each task is designed to be completed within a single commit following the Conventional Commits format (e.g., `feat:`, `fix:`, `chore:`).

The roadmap centers on making the local vs. remote monitoring mode **explicit** and **obvious** to users, with clear capability indicators and appropriate fallback messaging.

---

## Phase 1: Backend Capability Model

**Goal:** Add backend capability flags to explicitly indicate what metrics are available and why.

### Task 1.1: Create `MetricsCapabilities` struct

**File:** `src/state.rs`  
**Size:** ~50 lines  
**Acceptance:** Unit tests pass, compiles without warnings

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsCapabilities {
    pub inference: bool,
    pub system: bool,
    pub gpu: bool,
    pub cpu_temperature: bool,
    pub memory: bool,
    pub host_metrics: bool,
    pub tray: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum EndpointKind {
    Local,
    Remote,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum SessionKind {
    Spawn,
    Attach,
    None,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum TrayMode {
    Desktop,
    Headless,
    Failed,
}
```

**Test:**
```bash
cargo test -- metrics_capabilities
cargo clippy -- -D warnings
```

### Task 1.2: Create `AvailabilityReason` enum

**File:** `src/state.rs`  
**Size:** ~30 lines  
**Acceptance:** All reasons documented and testable

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AvailabilityReason {
    Available,
    RemoteEndpoint,
    NoDisplay,
    TrayUnavailable,
    SensorUnavailable,
    BackendUnavailable,
    CommandMissing,
    PermissionDenied,
    MetricsUnreachable,
    NotApplicable,
}
```

**Test:**
```bash
cargo test -- availability_reason
```

### Task 1.3: Add capability fields to `AppState`

**File:** `src/state.rs`  
**Size:** ~30 lines  
**Acceptance:** AppState compiles, field is accessible

- Add `capabilities: Arc<Mutex<MetricsCapabilities>>`
- Add `endpoint_kind: Arc<Mutex<EndpointKind>>`
- Add `session_kind: Arc<Mutex<SessionKind>>`
- Add `tray_mode: Arc<Mutex<TrayMode>>`

**Test:**
```bash
cargo build
```

### Task 1.4: Implement capability calculation logic

**File:** `src/state.rs`  
**Size:** ~80 lines  
**Acceptance:** All 4 tests pass

```rust
impl AppState {
    fn calculate_capabilities(&self) -> MetricsCapabilities {
        // Logic based on session mode and endpoint locality
    }
    
    fn calculate_availability_reasons(&self) -> (AvailabilityReason, AvailabilityReason, AvailabilityReason) {
        // Return (system_reason, gpu_reason, cpu_temp_reason)
    }
}
```

**Tests:**
- Local spawn → all metrics available
- Local attach → inference only
- Remote attach → inference only
- Headless mode → tray unavailable

**Test:**
```bash
cargo test -- calculate_capabilities
cargo test -- calculate_availability_reasons
```

### Task 1.5: Update `AppState::new()` to initialize capabilities

**File:** `src/state.rs`  
**Size:** ~20 lines  
**Acceptance:** New instance has correct initial state

- Initialize `capabilities` with defaults
- Initialize `endpoint_kind` based on first session
- Initialize `session_kind` and `tray_mode`

**Test:**
```bash
cargo test -- state_new_initializes_capabilities
```

### Task 1.6: Add capability API endpoint

**File:** `src/web/api.rs`  
**Size:** ~40 lines  
**Acceptance:** GET `/api/capabilities` returns JSON

```rust
fn api_get_capabilities(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "capabilities")
        .and(warp::get())
        .map(move || {
            let caps = state.capabilities.lock().unwrap().clone();
            let endpoint_kind = state.endpoint_kind.lock().unwrap().clone();
            let session_kind = state.session_kind.lock().unwrap().clone();
            warp::reply::json(&serde_json::json!({
                "capabilities": caps,
                "endpoint_kind": endpoint_kind,
                "session_kind": session_kind
            }))
        })
}
```

**Test:**
```bash
curl -s http://localhost:7778/api/capabilities | jq .
cargo test -- api_get_capabilities
```

### Task 1.7: Add availability reasons to state WebSocket messages

**File:** `src/web/ws.rs`  
**Size:** ~20 lines  
**Acceptance:** WS messages include availability data

Update JSON payload:
```json
{
  "capabilities": {...},
  "availability": {
    "system_reason": "available",
    "gpu_reason": "available",
    "cpu_temp_reason": "available"
  }
}
```

**Test:**
```bash
cargo test -- ws_capabilities_payload
```

### Task 1.8: Add endpoint locality helper to API

**File:** `src/web/api.rs`  
**Size:** ~20 lines  
**Acceptance:** Utility function tested

```rust
fn get_endpoint_kind(endpoint: &str) -> EndpointKind {
    // Use existing endpoint_is_local() logic
}
```

**Test:**
```bash
cargo test -- get_endpoint_kind
```

---

## Phase 2: UI Updates - Main Dashboard

**Goal:** Update the web dashboard to render based on capabilities, not just data presence.

### Task 2.1: Add endpoint health strip to main dashboard

**File:** `static/index.html`  
**Size:** ~40 lines  
**Acceptance:** Strip appears at top, shows mode

Add HTML structure:
```html
<div class="endpoint-health-strip">
    <span class="endpoint-mode" id="endpoint-mode">Loading...</span>
    <span class="endpoint-url" id="endpoint-url">...</span>
    <span class="endpoint-status" id="endpoint-status">...</span>
</div>
```

**Styles in `static/style.css`:**
- Fixed position top, 32px height
- Compact badges for status indicators
- Color-coded: green (OK), yellow (degraded), red (error)

**Test:** Visual inspection

### Task 2.2: Update dashboard to hide unavailable sections

**File:** `static/app.js`  
**Size:** ~60 lines  
**Acceptance:** GPU section hidden for remote

- When `capabilities.gpu === false`, hide GPU table and section
- When `capabilities.system === false`, hide system table and section
- Show "Remote endpoint - host metrics unavailable" message

**Implementation:**
```javascript
function updateDashboardCapabilities(caps) {
    if (!caps.gpu) {
        document.querySelector('.gpu-section')?.style.display = 'none';
    }
    // ... etc
}
```

**Test:**
```bash
# Manual test: Attach to remote endpoint, verify GPU section hidden
```

### Task 2.3: Add capability-aware empty states

**File:** `static/app.js`  
**Size:** ~50 lines  
**Acceptance:** Missing metrics show reason, not blank

Replace "—" with explainers:
- GPU backend not detected → "GPU metrics unavailable"
- CPU temp missing → "Temperature sensor unavailable"
- Remote attach → "Host metrics unavailable for remote endpoint"

**Test:** Verify each case shows appropriate message

### Task 2.4: Add severity indicators to metrics

**File:** `static/style.css`  
**Size:** ~40 lines  
**Acceptance:** 3 severity levels defined

```css
/* Severity levels */
.severity-normal { border-left: 3px solid #a3be8c; }
.severity-warning { border-left: 3px solid #ebcb8b; }
.severity-critical { border-left: 3px solid #bf616a; }

/* Status dots */
.status-dot-normal::before { content: "●"; color: #a3be8c; }
.status-dot-warning::before { content: "●"; color: #ebcb8b; }
.status-dot-critical::before { content: "●"; color: #bf616a; }
```

**Test:** Verify visual appearance

### Task 2.5: Implement context usage display

**File:** `static/app.js`  
**Size:** ~30 lines  
**Acceptance:** Context shows raw + percentage

Always display: `Context  12,384 / 65,536 (18.9%)`  
Never show percentage alone

**Test:** Verify format matches design

### Task 2.6: Add mode badge to dashboard header

**File:** `static/index.html`  
**Size:** ~20 lines  
**Acceptance:** Badge shows "Spawn" or "Attach"

Add small badge next to status:
```html
<span class="mode-badge" id="mode-badge">Spawn</span>
```

**Test:** Verify spawns show "Spawn", attaches show "Attach"

---

## Phase 3: UI Updates - Tray Dropdown

**Goal:** Update tray to use capability flags and auto-size based on visible metrics.

### Task 3.1: Update compact HTML structure

**File:** `static/compact.html`  
**Size:** ~50 lines  
**Acceptance:** Sections conditionally rendered

- Add `data-capability="gpu"` attributes to sections
- Add `data-capability="system"` to CPU/RAM sections
- Add `#availability-note` for remote/headless messages

**Test:** HTML validates

### Task 3.2: Update compact.js capability handling

**File:** `static/app.js`  
**Size:** ~100 lines  
**Acceptance:** Tray respects capabilities

- When `capabilities.gpu === false`, hide GPU sections
- When `capabilities.system === false`, hide CPU/RAM
- Always show inference metrics
- Show "Host metrics unavailable for remote endpoint" when needed

**Test:** Manual inspection of tray on remote endpoint

### Task 3.3: Update tray auto-height logic

**File:** `src/tray.rs`  
**Size:** ~40 lines  
**Acceptance:** Dropdown scales with content

- Count visible sections in compact.html
- Calculate height based on visible content only
- Clamp within min/max bounds (96px - 520px)

**Test:**
```bash
cargo build --release
# Test: Attach to remote, verify compact dropdown is small
# Test: Local spawn, verify dropdown includes GPU/CPU sections
```

### Task 3.4: Add endpoint info to tray tooltip

**File:** `src/tray.rs`  
**Size:** ~30 lines  
**Acceptance:** Tooltip shows local/remote mode

```rust
fn build_tooltip(...) -> String {
    let mode = if local_metrics_available { "Local" } else { "Remote" };
    format!("{} - llama-monitor", mode)
}
```

**Test:** Check tray icon tooltip text

---

## Phase 4: CLI Explicit Headless Mode

**Goal:** Add `--headless` and `--no-tray` flags with clear semantics.

### Task 4.1: Add CLI flags to `AppArgs`

**File:** `src/cli.rs`  
**Size:** ~20 lines  
**Acceptance:** `cargo run -- --headless` works

```rust
#[arg(long)]
pub headless: bool,

#[arg(long)]
pub no_tray: bool,
```

**Test:**
```bash
cargo run -- --help | grep headless
cargo run -- --help | grep no-tray
```

### Task 4.2: Update `should_start_tray()` logic

**File:** `src/main.rs`  
**Size:** ~30 lines  
**Acceptance:** CLI flags override detection

```rust
fn should_start_tray(args: &AppArgs) -> bool {
    if args.headless || args.no_tray {
        return false;
    }
    
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
    }
    
    #[cfg(not(target_os = "linux"))]
    true
}
```

**Tests:**
- `cargo run -- --headless` → no tray
- `cargo run -- --no-tray` → no tray
- `cargo run` on Linux without DISPLAY → no tray

### Task 4.3: Add log messages for mode

**File:** `src/main.rs`  
**Size:** ~20 lines  
**Acceptance:** Logs clearly state selected mode

```rust
if args.headless {
    println!("[info] Headless mode enabled (no tray, no desktop UI)");
} else if args.no_tray {
    println!("[info] Tray disabled via --no-tray");
} else if should_start_tray() {
    println!("[info] Tray enabled (desktop mode)");
} else {
    println!("[info] Tray disabled (no graphical session)");
}
```

**Test:**
```bash
cargo run -- --headless 2>&1 | grep "Headless"
cargo run -- --no-tray 2>&1 | grep "Tray disabled"
```

### Task 4.4: Update tray startup failure message

**File:** `src/main.rs`  
**Size:** ~15 lines  
**Acceptance:** Clear log on failure

```rust
if let Err(e) = crate::tray::run_tray(state, port) {
    eprintln!("[warn] Tray unavailable: {e}");
    eprintln!("[info] Continuing in headless mode with web/API server");
    park_forever();
}
```

**Test:** Simulate tray failure by breaking compilation

---

## Phase 5: Remote Metrics Agent Design (Planning Only)

**Goal:** Design the architecture for future remote metrics collection.

### Task 5.1: Draft remote agent API spec

**File:** `docs/20260420-remote-agent-api.md`  
**Size:** ~150 lines  
**Acceptance:** Draft reviewed and approved

Define:
- `GET /metrics/system` → CPU/RAM data
- `GET /metrics/gpu` → GPU metrics
- `GET /metrics/temperatures` → Temp data
- Authentication requirement
- Minimal schema

**Output:** Markdown doc in docs/

### Task 5.2: Design UI affordance for remote agent

**File:** `static/index.html`  
**Size:** ~30 lines  
**Acceptance:** Visual indicator planned

- Add "Remote Agent Connected" badge
- Show latency and connection status
- Optional: "Connect Agent" modal for setup

**Note:** Implementation deferred to future phase

---

## Phase 6: Testing & Validation

**Goal:** Ensure all features work correctly across different scenarios.

### Task 6.1: Add integration tests for capabilities

**File:** `tests/integration/capabilities.rs`  
**Size:** ~100 lines  
**Acceptance:** All test cases pass

```rust
#[test]
fn local_spawn_has_all_metrics() { ... }

#[test]
fn local_attach_has_inference_only() { ... }

#[test]
fn remote_attach_has_inference_only() { ... }

#[test]
fn headless_mode_disables_tray() { ... }
```

**Test:**
```bash
cargo test --test capabilities
```

### Task 6.2: Browser-based UI tests

**File:** `tests/ui/capability-rendering.test.js`  
**Size:** ~150 lines  
**Acceptance:** Tests pass in headless Chrome

```javascript
test('GPU section hidden for remote endpoint', async () => {
    // Attach to remote, verify GPU table is hidden
});

test('CPU temp shows sensor unavailable when missing', async () => {
    // Mock system without temp, verify message shown
});
```

**Test:**
```bash
npm test
# or
cargo run -- --port 9999 &
# Manual inspection
```

### Task 6.3: Visual QA checklist

**File:** `docs/20260420-visual-qa-checklist.md`  
**Size:** ~50 lines  
**Acceptance:** All 14 checks pass

1. ✅ Local endpoint with all metrics
2. ✅ Remote endpoint with inference-only
3. ✅ Idle server with no generation
4. ✅ Server unreachable state
5. ✅ High context usage warning
6. ✅ Missing GPU backend
7. ✅ Missing CPU temperature
8. ✅ Narrow browser width
9. ✅ Tray with inference-only
10. ✅ Tray with full hardware
11. ✅ Dark mode contrast
12. ✅ Reduced motion mode
13. ✅ No text overlap/truncation
14. ✅ Charts don't grow unbounded

**Output:** Markdown checklist in docs/

---

## Phase 7: Documentation

**Goal:** Update docs to reflect new behavior.

### Task 7.1: Update README with capability info

**File:** `README.md`  
**Size:** ~50 lines  
**Acceptance:** README reflects capability model

- Add "Monitoring Modes" section
- Explain local vs. remote metric availability
- Clarify headless mode usage

**Test:** Readability check

### Task 7.2: Document CLI flags

**File:** `docs/cli-flags.md`  
**Size:** ~50 lines  
**Acceptance:** All flags documented

- `--headless`
- `--no-tray`
- `--gpu-backend`
- All existing flags

**Test:** Check all flags explained

### Task 7.3: Update API docs for new endpoints

**File:** `docs/api.md`  
**Size:** ~30 lines  
**Acceptance:** `/api/capabilities` documented

Add:
- Endpoint: `GET /api/capabilities`
- Response schema
- Example outputs for each mode

**Test:** API documentation clarity

---

## Implementation Order

**Recommended sequence:**

1. **Phase 1** (Backend) - 7 tasks
2. **Phase 2** (Main UI) - 6 tasks  
3. **Phase 3** (Tray) - 4 tasks
4. **Phase 4** (CLI) - 4 tasks
5. **Phase 5** (Planning) - 2 tasks
6. **Phase 6** (Testing) - 3 tasks
7. **Phase 7** (Docs) - 3 tasks

**Total:** 29 tasks  
**Estimated commits:** 29 (one per task)  
**Total lines of code:** ~1,200

---

## Acceptance Criteria Summary

After implementation, the app should:

1. ✅ Show local vs. remote mode explicitly in dashboard
2. ✅ Hide unavailable metric sections (no empty tables)
3. ✅ Show concise availability reasons for missing metrics
4. ✅ Tray dropdown auto-sizes based on visible metrics
5. ✅ Context usage always shows `used / max (pct%)` format
6. ✅ CLI flags `--headless` and `--no-tray` work correctly
7. ✅ All metrics sections respect capability flags
8. ✅ Tray dropdown never shows full dashboard content
9. ✅ Tests cover capability logic and UI rendering
10. ✅ Documentation updated for new behavior

---

## Notes for Implementation

1. **Test early, test often:** Each task should have a test or manual verification step
2. **One commit per task:** Follow conventional commit format strictly
3. **Backward compatibility:** New API responses include old fields + new capability fields
4. **Graceful degradation:** If a metric is temporarily unavailable, show "—" not error
5. **Design consistency:** Use the same color/system for all severity indicators
6. **Tray is secondary:** Don't spend too much time perfecting tray until main UI is solid
7. **Remote agent is future:** Phase 5 is design only; implement when requirements are solid

---

## Future Work (Not in this implementation)

1. Remote metrics agent implementation
2. Sparkline charts for historical data
3. Heatmaps for temperature trends
4. GPU utilization history
5. Alert threshold configuration
6. Export metrics to Prometheus
7. Multi-server monitoring dashboard
8. Session history and comparison
9. GPU memory leak detection
10. Automatic preset optimization

---

**Document version:** 1.0  
**Last updated:** 2026-04-20  
**Next review date:** 2026-05-01
