# 2026-04-23: Tool-Call Blocked State Detection

## Problem

When a model is executing an agent task (running `cargo test`, `cargo build`, shell commands, etc.), the llama-server metrics go static. The slot shows `is_processing: true` but `n_decoded` stops increasing and `has_next_token` becomes `false`. The UI shows no activity — throughput drops to zero, generation card freezes — and there's no visibility into what the model is doing.

This happens because the model has emitted a tool call and is blocked waiting for the application to inject the tool result back into the context. During this window, the model cannot produce tokens, but the slot remains busy.

## Detection Heuristic

**llama-server does not expose a tool-calling state** through any HTTP endpoint (`/slots`, `/metrics`, `/health`). The `is_processing` boolean is the only state indicator, and it cannot distinguish between normal generation, tool call output, or waiting for a tool result.

However, we can **infer** a blocked state from the existing metrics:

```
is_processing == true          // slot is still holding a task
output_active == false         // has_next_token is false (not generating)
n_decoded > 0                  // has generated tokens this turn
n_decoded stagnant for N polls // not increasing over consecutive polls
```

This pattern differs from:
- **Idle** — `is_processing == false`
- **Active generation** — `output_active == true`, `n_decoded` increasing
- **Prompt ingest** — `output_active == true`, `n_decoded <= 1`

**Threshold:** 3+ consecutive polls (≈3 seconds at 1s poll interval) with stagnant `n_decoded` while `is_processing == true` and `output_active == false`.

## ⚠️ Disabled — Unreliable

**Blocked detection is currently disabled** because we cannot reliably distinguish "blocked on tool call" from "processing big context" using only `n_decoded` stagnation. Both states show:
- `is_processing == true`
- `output_active == false`
- `n_decoded` stagnant

Big context processing can take 30-60+ seconds, making any threshold either too sensitive (false positives during context processing) or too insensitive (missing actual tool calls).

**Re-enable when llama-server exposes a tool-calling state** in `/slots` (e.g., a `tool_calling` boolean or `state` enum that distinguishes between prompt processing, generation, and tool waiting).

**Current status:** The backend code is disabled (commented out in `src/llama/poller.rs`), but the frontend UI code remains intact (it will simply never see `tool_calling_blocked: true`). The `LlamaMetrics` struct still has `tool_calling_blocked`, `blocked_duration_sec`, and `blocked_task_id` fields for when detection is re-enabled.

## Backend Changes

### New Fields in `LlamaMetrics`

```rust
pub tool_calling_blocked: bool,       // inferred from slot metrics
pub blocked_duration_sec: u64,        // how long blocked (accumulated across polls)
pub blocked_task_id: Option<u64>,     // the task_id of the blocked slot
```

### Detection Logic in Poller

In `src/llama/poller.rs`, after fetching `/slots` metrics, add a per-slot tracker:

```rust
struct SlotBlockTracker {
    last_decoded: u32,
    stagnant_polls: u32,
    blocked_since: Option<Instant>,
}
```

On each poll:
1. If `is_processing && !output_active && n_decoded > 0 && n_decoded == last_decoded`:
   - Increment `stagnant_polls`
   - If `stagnant_polls >= 3`, set `blocked_since` if not already set
2. If `output_active || n_decoded != last_decoded`:
   - Reset `stagnant_polls` to 0, clear `blocked_since`
3. If `!is_processing`:
   - Full reset

Set `tool_calling_blocked = blocked_since.is_some()` and `blocked_duration_sec` from the elapsed time.

### WebSocket Serialization

Add fields to the WebSocket metrics payload so the frontend receives:
```json
{
  "tool_calling_blocked": true,
  "blocked_duration_sec": 12,
  "blocked_task_id": 42
}
```

## Frontend Integration

### Placement: Throughput Card

The throughput card (`static/index.html` lines 176-216) is the natural home for this indicator. When the model is blocked on tool execution, **throughput is impacted directly** — both prompt and generation rates drop to zero. Showing the blocked state here connects the cause (tool calling) to the effect (zero throughput).

### Visual Design

#### 1. Live Chip — New `blocked` State

The throughput card's live chip (`#m-throughput-state`) already supports `live`, `idle`, `warning`, and `critical` states. Add a new `blocked` variant:

```css
.metric-live-chip.blocked {
  color: #ebcb8b;
  border-color: rgba(235, 203, 139, 0.36);
  background: rgba(235, 203, 139, 0.08);
  animation: chip-blocked-pulse 2s ease-in-out infinite;
}

.metric-live-chip.blocked::before {
  background: #ebcb8b;
  box-shadow: 0 0 10px rgba(235, 203, 139, 0.6);
  animation: dot-pulse 1s ease-in-out infinite;
}

@keyframes chip-blocked-pulse {
  0%, 100% { box-shadow: 0 0 0 rgba(235, 203, 139, 0); }
  50% { box-shadow: 0 0 16px rgba(235, 203, 139, 0.2); }
}
```

This reuses the existing amber/warning palette (`#ebcb8b`) and the `dot-pulse` animation already used for live indicators.

#### 2. Blocked Duration Bar — Between Speed Bars and Sparkline

Insert a new inline indicator between the generation speed bar and the sparkline pair:

```html
<div class="blocked-indicator" id="m-throughput-blocked" style="display: none;">
  <span class="blocked-icon" aria-hidden="true">
    <svg width="14" height="14" viewBox="0 0 14 14">
      <circle cx="7" cy="7" r="6" fill="none" stroke="#ebcb8b" stroke-width="1.5"
              stroke-dasharray="30 10" stroke-linecap="round">
        <animateTransform attributeName="transform" type="rotate"
                          from="0 7 7" to="360 7 7" dur="2.4s" repeatCount="indefinite"/>
      </circle>
    </svg>
  </span>
  <span class="blocked-text">tool calling — throughput suspended</span>
  <span class="blocked-timer" id="m-blocked-timer">0s</span>
</div>
```

```css
.blocked-indicator {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 999px;
  background: linear-gradient(135deg, rgba(235, 203, 139, 0.1), rgba(235, 203, 139, 0.04));
  border: 1px solid rgba(235, 203, 139, 0.22);
  font-family: var(--font-mono);
  font-size: var(--text-2xs);
  font-weight: 700;
  opacity: 0;
  transform: translateY(-4px);
  transition: opacity 300ms ease, transform 300ms ease;
}

.blocked-indicator.visible {
  opacity: 1;
  transform: translateY(0);
}

.blocked-indicator .blocked-text {
  color: #f4db9f;
  flex: 1;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.blocked-indicator .blocked-timer {
  color: #ebcb8b;
  font-variant-numeric: tabular-nums;
  min-width: 28px;
  text-align: right;
}
```

The spinning dashed circle (`stroke-dasharray="30 10"`) mirrors the generation card's token-flow animation aesthetic — a subtle motion indicator that communicates "work is happening, but not here."

#### 3. Card-Level `is-blocked` State

Add a new card-level state class for the throughput card:

```css
.widget-card.is-blocked {
  border-color: rgba(235, 203, 139, 0.24);
  box-shadow:
    0 10px 34px rgba(0, 0, 0, 0.48),
    0 0 36px rgba(235, 203, 139, 0.1),
    inset 0 1px 0 rgba(255, 255, 255, 0.06);
}

.widget-card.is-blocked::after {
  opacity: 1;
  background: linear-gradient(135deg, rgba(235, 203, 139, 0.32), transparent 42%, rgba(235, 203, 139, 0.16));
  animation: card-edge-breathe 2.8s ease-in-out infinite;
}
```

This mirrors the `.is-live` card state but uses the amber palette, giving the card a warm glow that signals "something is happening, but it's not normal throughput."

#### 4. Speed Bar Dimming

When blocked, the prompt and generation speed bars should dim to communicate that throughput is suspended:

```css
.widget-speed.is-blocked .speed-bar {
  opacity: 0.3;
  filter: grayscale(0.6);
  transition: opacity 400ms ease, filter 400ms ease;
}

.widget-speed.is-blocked .speed-bar::after {
  animation: none; /* stop shimmer */
}
```

### Generation Card — Third Stage Segment

The generation card already has a two-segment stage indicator (`static/index.html` lines 225-228):

```html
<div class="generation-stage" id="m-generation-stage">
  <span class="stage-segment" id="m-stage-prompt">Prompt ingest</span>
  <span class="stage-segment" id="m-stage-output">Output generation</span>
</div>
```

Add a third segment:

```html
<span class="stage-segment" id="m-stage-toolcall">Tool calling</span>
```

```css
.stage-segment.toolcall {
  color: #f4db9f;
  background: linear-gradient(135deg, rgba(235, 203, 139, 0.22), rgba(235, 203, 139, 0.12));
  box-shadow: 0 0 18px rgba(235, 203, 139, 0.16);
  animation: stage-toolcall-pulse 2s ease-in-out infinite;
}

@keyframes stage-toolcall-pulse {
  0%, 100% { box-shadow: 0 0 18px rgba(235, 203, 139, 0.16); }
  50% { box-shadow: 0 0 28px rgba(235, 203, 139, 0.32); }
}
```

### JS Rendering Logic

In `static/app.js` throughput rendering section (around line 4750):

```javascript
const isBlocked = l?.tool_calling_blocked || false;
const blockedSec = l?.blocked_duration_sec || 0;

// Card state: blocked overrides idle
setCardState(throughputCard,
  !hasActiveEndpoint ? 'dormant' : isBlocked ? 'blocked' : throughputActive ? 'live' : 'idle'
);

// Chip state
setChipState(throughputState,
  throughputActive ? 'live' : isBlocked ? 'blocked' : 'idle',
  throughputActive ? 'live' : isBlocked ? 'blocked' : 'idle'
);

// Blocked indicator
const blockedEl = document.getElementById('m-throughput-blocked');
const blockedTimer = document.getElementById('m-blocked-timer');
if (blockedEl) {
  blockedEl.classList.toggle('visible', isBlocked);
  if (blockedTimer) blockedTimer.textContent = `${blockedSec}s`;
}
```

In the generation card section (around line 4851):

```javascript
const toolCallStage = document.getElementById('m-stage-toolcall');
if (toolCallStage) {
  toolCallStage.classList.toggle('toolcall', isBlocked);
  toolCallStage.classList.toggle('idle', !isBlocked && !generationActive);
}
// Adjust existing stage logic to not activate output during tool calling
promptStage.classList.toggle('active', generationActive && generated <= 1 && !isBlocked);
outputStage.classList.toggle('active', generationActive && generated > 1 && !isBlocked);
```

### Sparkline Blocked Annotation

When blocked, annotate the sparkline with a vertical dashed line at the current position:

```javascript
// In renderSparkline(), when isBlocked:
const blockedLine = document.createElementNS('http://www.w3.org/2000/svg', 'line');
blockedLine.setAttribute('x1', '120');
blockedLine.setAttribute('y1', '0');
blockedLine.setAttribute('x2', '120');
blockedLine.setAttribute('y2', '28');
blockedLine.setAttribute('stroke', '#ebcb8b');
blockedLine.setAttribute('stroke-width', '1');
blockedLine.setAttribute('stroke-dasharray', '3 3');
blockedLine.setAttribute('opacity', '0.5');
svg.appendChild(blockedLine);
```

This visually marks where throughput dropped to zero, creating a "wall" effect that communicates the suspension point.

## State Machine

```
                    ┌──────────────┐
                    │     IDLE     │
                    │ is_processing│
                    │    == false  │
                    └──────┬───────┘
                           │ slot starts processing
                    ┌──────▼───────┐
               ┌────│ PROMPT INGEST│────┐
               │    │ n_decoded≤1  │    │
               │    └──────┬───────┘    │
               │           │ n_decoded > 1
               │    ┌──────▼───────┐    │
               │    │  GENERATING  │    │
               │    │ output_active│    │
               │    │   == true    │    │
               │    └──────┬───────┘    │
               │           │ output_active == false,
               │           │ n_decoded stagnant ≥ 3 polls
               │    ┌──────▼───────┐    │
               │    │  BLOCKED     │    │
               │    │ tool calling │    │
               │    │ (inferred)   │    │
               │    └──────┬───────┘    │
               │           │ output resumes OR slot idle
               │           ▼             │
               │    ┌──────┐             │
               └────│ DONE │◄────────────┘
                    └──────┘
```

## Design Rationale

### Why the throughput card?

The blocked state is fundamentally a throughput story. When the model is tool-calling, both prompt and generation rates drop to zero. The throughput card already tracks these rates with sparklines, bars, and live chips. Adding the blocked indicator here:

1. **Connects cause to effect** — the user sees zero throughput and immediately understands why
2. **Uses existing visual language** — the amber/warning palette, chip states, and card-level glow are all established patterns
3. **Avoids overcrowding the generation card** — the generation card is already dense with ring progress, stage segments, velocity estimates, sparklines, token flow, and detail chips

### Animation choices

| Element | Animation | Rationale |
|---------|-----------|-----------|
| Live chip dot | `dot-pulse` (existing) | Consistent with live indicator pattern |
| Chip glow | `chip-blocked-pulse` (new) | Subtle breathing glow, not alarming |
| Spinning circle | SVG `animateTransform` rotate | Communicates "work happening elsewhere" |
| Card edge | `card-edge-breathe` (existing) | Reuses the same pattern as `is-live` |
| Speed bars | Dimmed + shimmer stopped | Visual de-emphasis: throughput is paused |
| Sparkline wall | Static dashed line | Marks the suspension point |

### Color choices

Amber (`#ebcb8b` / `#f4db9f`) is used because:
- It's the existing **warning** palette in the app (see `.metric-live-chip.warning`)
- It signals "attention needed" without the urgency of red/critical
- It's distinct from teal (live), green (generation), and blue (prompt)
- It matches the `is-unavailable` card border tone

## Open Questions

1. **Tool name detection** — llama-server doesn't expose which tool is being called. Could we parse the last generated text for tool call patterns (e.g., JSON with `"name"` and `"arguments"` fields) to show "calling `cargo_test`" instead of generic "tool calling"?

2. **Multiple blocked slots** — If multiple slots are blocked simultaneously, should we show a count?

3. **Timeout escalation** — If blocked duration exceeds a threshold (e.g., 60s), should the chip escalate from `blocked` (amber) to `critical` (red) to signal a potential hang?

4. **Historical tracking** — Should we track total blocked time per session as a metric, similar to how we track peak throughput?

## References

- Detection analysis: this document
- Agent fix: `src/agent.rs` — `resolve_windows_appdata` and `default_start_command_for_os_with`
- Metrics parsing: `src/llama/metrics.rs` lines 146-220 (slot parsing), 115-143 (Prometheus)
- Poller: `src/llama/poller.rs`
- Throughput card: `static/index.html` lines 176-216, `static/app.js` lines 4726-4811
- Generation card: `static/index.html` lines 219-249, `static/app.js` lines 4813-4874
- CSS: `static/style.css` lines 950-976 (card states), 1066-1090 (chip states), 1190-1223 (speed bars), 1394-1415 (stage segments)
