# Power Optimization Plan

**Date:** 2026-06-04
**Issue:** Browser tab with llama-monitor is a serious battery burner on MacBook. Closing the tab resolves the drain.

---

## Root Cause Analysis

### Three Major Drain Categories

#### 1. WebSocket Push at 500ms with No Throttling (The Engine)

- Server pushes a **full JSON snapshot** of GPU metrics, CPU metrics, llama metrics, logs, session state, and capability info every **500ms** (2x/sec)
- GPU hardware poller reads at 500ms regardless of whether clients are connected or the tab is visible
- **No Page Visibility API integration** — neither frontend nor backend responds to `document.hidden`
- Default `ws_push_interval_ms` is 500ms; user can manually change but nothing does it automatically

#### 2. ~100+ DOM Writes Per Tick (The Amplifier)

Every 500ms WebSocket message triggers `updateDashboard()` which cascades through:

- 30+ `textContent` writes to metric elements
- 15+ `style.width` writes to progress bars (each triggering CSS transitions → layout recalc)
- 6+ SVG sparkline `innerHTML` rebuilds (full style recalc with gradient definitions)
- 2x `animateNumber` rAF loops at 60fps for 300ms — animations **overlap** since ticks are 500ms apart
- **Zero change detection** — every element's value gets written even when unchanged (only `updateLogs` has change detection)

#### 3. 70+ Infinite CSS Animations (The GPU Furnace)

70+ `@keyframes` with `animation-iteration-count: infinite` run simultaneously:

- **25+ `box-shadow` infinite animations** — `box-shadow` with blur radius forces GPU recalc every frame; animating the blur radius is even worse
- **10+ `filter: drop-shadow()` infinite animations** — filter recomputation every frame
- **SVG SMIL `<animate>` on `<feGaussianBlur>`** (index.html:84) — runs a Gaussian blur at 60fps forever on every sparkline's "current position" dot
- **`mesh-drift` animates `background-position`** — forces full repaint every frame
- **`tune-sweep` animates `left` and `width`** — forces layout recalculation every frame

### The Kill Chain (What Happens Every 500ms)

```
[Server: 500ms GPU poll] ──┐
                           ├──> [WebSocket: 500ms full JSON push]
[Server: 500ms WS push] ──┘       └──> [Browser: parses JSON]
                                                       └──> updateDashboard()
                                                               ├──> 30+ textContent writes
                                                               ├──> 15+ style.width writes (→ layout recalc)
                                                               ├──> 6+ SVG innerHTML rebuilds (→ full style recalc)
                                                               ├──> 2x animateNumber rAF loops (→ 60fps DOM writes)
                                                               └──> 70+ CSS infinite animations at 60fps
```

### Redundant Polling

- `sessions.js` polls `/api/sessions` via HTTP every 2s for data already in every WebSocket message

---

## Recommendations (10 Items)

### IMMEDIATE — High Impact, Low Effort

#### #1: Page Visibility API Throttling

**Problem:** Full dashboard updates run while tab is hidden. 500ms interval fires identically whether user can see the tab or not.

**Fix:** When `document.hidden` is true, automatically increase WebSocket push interval to 5000ms. Revert when visible.

**Files:**
- `static/js/features/dashboard-ws.js` — add visibility listener that adjusts `last_interval_ms`

**Expected impact:** ~80% reduction in DOM writes when tab is backgrounded (typical user frequently switches tabs).

#### #2: Change Detection on DOM Writes

**Problem:** Every element's value is written every tick even when unchanged. No `if (newVal === prevVal) return;` guards anywhere.

**Fix:** Add change detection before every DOM write in `updateDashboard()` cascade. Store last-known values and skip writes when data hasn't changed.

**Files:**
- `static/js/features/dashboard-ws.js` — add change detection wrapper
- `static/js/features/dashboard-render.js` — add change detection for GPU/system card renders

**Expected impact:** Eliminates most DOM work when server is idle (metrics stable between ticks).

#### #3: Kill SVG SMIL `<animate>` on `<feGaussianBlur>`

**Problem:** In `static/index.html:84`, `<animate attributeName="stdDeviation" values="1;2;1" dur="2.5s" repeatCount="indefinite"/>` runs a Gaussian blur at 60fps forever.

**Fix:** Remove the SVG SMIL animation. Replace with a CSS `transform: scale()` + `opacity` pulse on the dot element itself.

**Files:**
- `static/index.html` — remove `<animate>` tag from `#spark-pulse-glow` filter
- `static/css/cards-hardware.css` — add CSS animation for the dot element

**Expected impact:** Eliminates continuous GPU blur recalculation on every sparkline's current-position indicator.

#### #4: Remove Redundant `sessions.js` HTTP Polling

**Problem:** `sessions.js:437` polls `/api/sessions` via HTTP every 2s for data already included in every WebSocket message.

**Fix:** Remove the `setInterval(updateActiveSessionInfo, 2000)` call. The WebSocket delivers the same data.

**Files:**
- `static/js/features/sessions.js` — remove the polling interval; `updateActiveSessionInfo` should be called from the WebSocket message handler instead

**Expected impact:** Eliminates one HTTP roundtrip every 2 seconds.

### MEDIUM — High Impact, Medium Effort

#### #5: Replace `box-shadow` Infinite Animations with Opacity Alternatives

**Problem:** ~25 infinite `@keyframes` animate `box-shadow` with expanding blur radius. Each forces GPU recalculation every frame.

**Fix:** For each animated element:
1. Put the shadow on a pseudo-element (`::before` or `::after`)
2. Animate only `opacity` (compositing-only) on the pseudo-element

**Files:**
- `static/css/cards-hardware.css` — ~15 animations to refactor
- `static/css/cards-inference.css` — ~8 animations to refactor
- `static/css/chat.css` — ~6 animations to refactor

**Example transformation:**

Before:
```css
@keyframes hw-bar-glow {
  0%, 100% { box-shadow: 0 0 4px rgba(var(--accent), 0.3); }
  50%      { box-shadow: 0 0 12px rgba(var(--accent), 0.6); }
}
```

After:
```css
.hw-bar-glow::before {
  content: '';
  position: absolute;
  inset: 0;
  box-shadow: 0 0 12px rgba(var(--accent), 0.6);
  animation: opacity-pulse 2s ease-in-out infinite;
}

@keyframes opacity-pulse {
  0%, 100% { opacity: 0.3; }
  50%      { opacity: 1; }
}
```

**Expected impact:** Each reframed animation goes from "GPU recalc + paint every frame" to "compositing only, no paint."

#### #6: Replace `style.width` Transitions with `transform: scaleX()`

**Problem:** 15+ progress bars use `width` transitions which trigger layout recalculation on every update.

**Fix:** Apply `transform-origin: left center` to the fill elements and use `transform: scaleX(0-1)` instead of `width: 0%-100%`. Set the fill width to 100% always and scale it down.

**Files:**
- `static/css/cards-inference.css` — `.speed-bar`, `.context-fleet-fill`, throughput ratio bars
- `static/css/cards-hardware.css` — `.hw-stacked-fill`, hw bars
- `static/css/chat.css` — chat bars, compact progress
- `static/js/features/dashboard-ws.js` — change `style.width = pct + '%'` to `style.transform = 'scaleX(' + pct/100 + ')'`
- `static/js/features/dashboard-render.js` — same pattern
- `static/js/features/chat-params.js` — same pattern

**Expected impact:** Removes layout recalculation from every progress bar update (which fires every 500ms).

#### #7: Add Change Detection to SVG Rebuilds

**Problem:** 6+ sparkline SVGs rebuild from scratch every 500ms via `innerHTML`, including gradient definitions.

**Fix:** Only rebuild SVG when data actually changed meaningfully:
- For sparklines: only rebuild if the new data point differs from the last
- For GPU sparklines: only rebuild if load/power/VRAM values changed

**Files:**
- `static/js/features/dashboard-render.js` — `renderSparkline()`, `renderHwMetricSparkline()`
- `static/js/features/dashboard-ws.js` — add data comparison before calling render functions

**Expected impact:** Eliminates full SVG rebuild when metrics are stable (which is most ticks when server is idle).

#### #8: Fix `tune-sweep` Animation

**Problem:** `tune-sweep` in `tune-panel.css:151-155` animates `left` and `width` — both are layout-triggering properties.

**Fix:** Replace with `transform: translateX()` and `transform: scaleX()`.

**Files:**
- `static/css/tune-panel.css` — rewrite `@keyframes tune-sweep`

**Expected impact:** Changes from "layout recalc every frame" to "compositing only."

### LONG-TERM — Highest Impact, Highest Effort

#### #9: Make GPU Poller Interval Configurable

**Problem:** GPU poller is hardcoded at 500ms (`GPU_POLL_INTERVAL` in `src/main.rs:37`). Ignores `ws_push_interval_ms` setting. If WS is at 5s, the poller reads hardware 20x per second and throws away 19 readings.

**Fix:** Make `GPU_POLL_INTERVAL` track `ws_push_interval_ms`. When the user changes push interval, also adjust the GPU poller.

**Files:**
- `src/main.rs` — make poller interval dynamic based on `ui_settings.ws_push_interval_ms`
- `src/state.rs` — expose the setting to the poller

**Expected impact:** When push interval is high (battery saver mode), GPU hardware reads slow down proportionally.

#### #10: Frontend-Driven Push Rate

**Problem:** The server doesn't know the tab is hidden. Even with frontend-side throttling (#1), the server still pushes at full rate and the browser just discards the messages.

**Fix:** When the tab becomes hidden, the frontend sends a `PUT /api/settings` with `ws_push_interval_ms: 5000`. When visible, sends `ws_push_interval_ms: 500` (or the user's configured default).

**Files:**
- `static/js/features/dashboard-ws.js` — send settings update on visibility change
- `src/web/ws.rs` — ensure the interval respects runtime changes (it already does via `last_interval_ms`)

**Expected impact:** Server stops sending data when no one is watching. Reduces network traffic and server CPU.

---

## Implementation Order

1. #1 (Page Visibility API throttling) — fastest win, minimal risk
2. #2 (Change detection on DOM writes) — eliminates wasted work
3. #3 (Kill SVG SMIL animation) — single file change, high GPU impact
4. #4 (Remove redundant polling) — one line to delete
5. #5 (box-shadow → opacity) — moderate effort, preserves visual style
6. #6 (width → scaleX) — moderate effort, preserves visual style
7. #7 (SVG change detection) — reduces rebuild frequency
8. #8 (tune-sweep fix) — small, quick
9. #9 (GPU poller configurable) — server-side, higher risk
10. #10 (Frontend-driven push rate) — depends on #1 being stable first

---

## Styling Preservation Notes

The goal is **zero visual regression**. Every change preserves the current visual appearance:

- Replacing `box-shadow` animation with opacity-based pseudo-elements maintains the glow effect
- Replacing `width` transition with `scaleX` maintains the bar fill animation (the transition timing function must be preserved)
- Replacing `left` + `width` with `transform` maintains the sweep animation
- Removing SVG SMIL `<animate>` replaces the blur pulse with a CSS-equivalent scale + opacity pulse on the dot element
