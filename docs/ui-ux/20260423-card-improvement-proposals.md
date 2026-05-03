# 2026-04-23: Card Improvement Proposals

## Throughput Card — Blocked Indicator (Implemented)

### What You Should See

When the model is blocked on a tool call, the throughput card shows:

| Element | Normal State | Blocked State | Critical (>60s) |
|---------|-------------|---------------|-----------------|
| **Live chip** | Teal "live" or gray "idle" | Amber "blocked" with pulsing dot | Red "critical" with pulsing dot |
| **Card border** | Teal glow (live) or none (idle) | Amber glow with breathing edge | Amber glow with breathing edge |
| **Speed bars** | Full opacity, shimmer animation | Dimmed (30% opacity, grayscale) | Dimmed (30% opacity, grayscale) |
| **Blocked indicator** | Hidden (0 opacity) | Visible: spinning dashed circle + "tool calling — throughput suspended" + timer | Red styling + "potential hang — throughput suspended" |
| **Sparklines** | Normal | Dashed amber "wall" line at right edge | Dashed amber "wall" line at right edge |

### Why It Might Not Appear

The blocked detection has **two modes**:

1. **Primary (n_decoded available):** Requires `next_token[0].n_decoded` from `/slots`. Tracks stagnant output tokens while `is_processing=true` and `output_active=false`.
2. **Fallback (n_decoded not available):** Detects from throughput dropping to zero (`prompt_tps == 0` and `gen_tps == 0`) while `is_processing=true`.

**If your llama-server doesn't expose `next_token` data**, the fallback mode activates. It requires:
- `is_processing == true` (slot is busy)
- Both prompt and generation throughput at zero
- For 3+ consecutive polls (~3 seconds)

**To trigger it for testing:** Run a model with tool calling enabled, send a request that triggers a tool call, and watch the throughput card. When the tool executes, throughput drops to zero while the slot stays processing → blocked indicator appears.

### Prompt/Gen Ratio

The prompt/gen ratio bar at the bottom of the throughput card shows the ratio of prompt processing speed to generation speed. This is useful for understanding model efficiency but takes up vertical space. **It could be moved to the Context card or Decoding Config card** to make room for more blocked-state metrics.

---

## Slot Activity Card Proposals

### Current State

Grid of slot tiles showing:
- Slot ID (slot 0, slot 1, etc.)
- Busy/idle state
- Task ID
- Output tokens
- Context size
- Single utilization bar (% busy)

### Proposal 1: Blocked State per Slot (High Impact)

**What:** Highlight slot tiles in amber when that specific slot is blocked on tool calling.

**Visual:**
```
┌─────────────────────────┐
│ slot 0   ⚠ BLOCKED      │  ← amber border + glow
│ task 42                  │
│ 128 output   8k ctx      │
│ [amber mini bar]         │  ← blocked duration bar
└─────────────────────────┘
```

**Implementation:**
- Backend: Per-slot blocked tracking (extend `SlotBlockTracker` to track all slots, not just primary)
- Frontend: Add `blocked` class to `.slot-tile` with amber styling matching the throughput card
- CSS: Reuse `.slot-tile.busy` pattern but with amber palette

### Proposal 2: Per-Slot Throughput Sparkline (Medium Impact)

**What:** Tiny sparkline in each slot tile showing that slot's token generation rate over the last 20 polls.

**Visual:**
```
┌─────────────────────────┐
│ slot 0   ACTIVE          │
│ task 42                  │
│ 128 output   8k ctx      │
│ ╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲   │  ← 28px sparkline
└─────────────────────────┘
```

**Implementation:**
- Backend: Track per-slot `n_decoded` history in poller (array of last 20 values per slot)
- Frontend: Render sparkline in each tile using existing `renderSparkline()` function
- Trade-off: Adds complexity to backend state, but reuses existing sparkline rendering

### Proposal 3: Phase Breakdown Bar per Slot (Medium Impact)

**What:** Mini bar in each slot tile showing prompt vs generation time split for the current task (like the activity rail phases, but per-slot).

**Visual:**
```
┌─────────────────────────┐
│ slot 0   ACTIVE          │
│ task 42                  │
│ [████ blue ███ green ]   │  ← 60% prompt, 40% generation
│ 128 output   8k ctx      │
└─────────────────────────┘
```

**Implementation:**
- Frontend: Calculate from `firstOutputAtMs` and `endedAtMs` in request activity tracker
- CSS: Two-color gradient bar with blue (prompt) and green (generation) segments

### Proposal 4: Utilization History Sparkline (Low Impact)

**What:** Replace the static utilization % bar with a sparkline showing slot utilization over the last 60 seconds.

**Visual:**
```
Slot utilization
[╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲] 40%
```

**Implementation:**
- Frontend: Track utilization % history in an array, render as sparkline
- Backend: No changes needed

---

## Request Activity Card Proposals

### Current State

- Two stats: Requests (10m), Avg duration
- Activity rail: 28 recent tasks as horizontal bars with prompt/generation phase split

### Proposal 1: Blocked Phase Coloring (High Impact)

**What:** Add amber "blocked" segments to the activity rail segments, showing how much of each request was spent waiting for tool results.

**Visual:**
```
Current:  [████ blue ███ green ]
Proposed: [██ blue ███ green ███ amber ]
                        ← time blocked on tool call
```

**Implementation:**
- Backend: Track blocked time per request in the request activity tracker
- Frontend: Add `.activity-phase.blocked` CSS class with amber gradient
- CSS: `background: linear-gradient(90deg, rgba(235, 203, 139, 0.5), rgba(235, 203, 139, 0.8))`

### Proposal 2: Duration Sparkline (High Impact)

**What:** Replace the two static stats with a sparkline showing request duration over the last 28 requests.

**Visual:**
```
Request Activity
┌─────────────────────────────────────┐
│ Duration (last 28 requests)         │
│ [╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲]   │  ← duration sparkline
│ Peak: 12.4s   Avg: 3.2s             │
└─────────────────────────────────────┘
```

**Implementation:**
- Frontend: Track request durations in an array, render as sparkline
- Backend: No changes needed
- Color: Use amber for durations > 2x average, green for normal

### Proposal 3: Request Rate Heatmap (Medium Impact)

**What:** Small heatmap below the activity rail showing requests/minute over the last 10 minutes.

**Visual:**
```
[··▪▫▪▫▫▪▫▫▫▪▫▫▪▫▫▪▫▫▫▪▫▫▪▫▫▫▪▫▫]
  10m ago                              now
  (darker = more requests)
```

**Implementation:**
- Frontend: Bucket request timestamps into 1-minute intervals, render as colored cells
- CSS: 4 intensity levels using amber palette

### Proposal 4: Tokens-per-Request Distribution (Low Impact)

**What:** Simple histogram showing output token count distribution.

**Visual:**
```
Tokens/request
  0-100    [████████████████████████████] 42
  100-500  [██████████████████████] 31
  500-1k   [██████████] 12
  1k+      [████] 3
```

**Implementation:**
- Frontend: Bucket request output tokens into ranges, render as horizontal bars
- Backend: No changes needed

---

## Context Window Card Proposals

### Current Problem

The Context Window card is largely static because llama-server only exposes:
- `n_ctx` (context capacity)
- `n_tokens` / `n_past` / `n_ctx_used` / `n_cache_tokens` (live usage, when available)
- Peak observed (from Prometheus `n_tokens_max`)

When `n_tokens` isn't exposed, the card shows only the capacity bar with no movement.

### Proposal 1: Inferred Context Usage from Throughput (High Impact)

**What:** Infer context growth from cumulative token counts. Each request adds prompt tokens + generated tokens to the context window.

**Formula:**
```
estimated_context = prompt_tokens_total + generation_tokens_total - (tokens freed by completed requests)
```

**Visual:**
```
Context Window
Live usage (inferred)
[██████████████████████████░░░░░░░░] 72%
Peak observed
[████████████████████████████████░░] 91%
```

**Implementation:**
- Backend: Track cumulative tokens from Prometheus counters, estimate context usage
- Frontend: Show "inferred" label on the live usage bar
- Caveat: This is approximate — doesn't account for context sliding window or KV cache eviction

### Proposal 2: Context Utilization History Sparkline (High Impact)

**What:** Add a sparkline showing context usage over the last 60 polls (1 minute at 1s interval).

**Visual:**
```
Context Window
Live usage
[██████████████████████████░░░░░░░░] 72%
[╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲]  ← context usage over time
Peak: 8.2k / 8k ctx
```

**Implementation:**
- Frontend: Track `context_live_tokens` values in an array, render as sparkline
- Color: Green when < 70%, amber when 70-90%, red when > 90%
- Backend: No changes needed (already exposes `context_live_tokens`)

### Proposal 3: Request Context Footprint (Medium Impact)

**What:** Show the average context tokens consumed per request, with a trend indicator.

**Visual:**
```
Context Window
Live usage: 4.2k / 8k (52%)
Avg per request: 1.8k tokens ↗ +12%
Context turns: 3 active
```

**Implementation:**
- Backend: Track context tokens at request start vs end, calculate delta per request
- Frontend: Show as stat items with trend arrows
- Useful for understanding how much context each agent turn consumes

### Proposal 4: Context Pressure Indicator (Medium Impact)

**What:** Visual indicator of how close the model is to context limits, with color-coded warning levels.

**Visual:**
```
Context Window
┌─────────────────────────────────────┐
│ ████████████████████████░░░░░░░░ 72%│
│         ● COMFORTABLE               │  ← green dot, "comfortable" label
│ Peak: 7.4k / 8k                     │
│ Est. turns remaining: ~4            │  ← estimated before hitting limit
└─────────────────────────────────────┘
```

**Levels:**
- < 60%: Green "comfortable"
- 60-80%: Amber "moderate"
- 80-95%: Orange "pressure"
- > 95%: Red "critical — context nearly full"

**Implementation:**
- Frontend: Calculate from `context_live_tokens / context_capacity_tokens`
- CSS: Color-coded dot + label matching existing chip states
- "Est. turns remaining" = `(capacity - live) / avg_tokens_per_turn`

### Proposal 5: Move Prompt/Gen Ratio Here (Low Impact)

**What:** Move the prompt/gen ratio bar from the Throughput card to the Context card, since it's related to how efficiently the model uses its context.

**Rationale:** Frees up vertical space in the Throughput card for more blocked-state metrics, while giving the Context card more content.

---

## Priority Summary

| Priority | Proposal | Card | Effort | Impact |
|----------|----------|------|--------|--------|
| **P0** | Blocked phase coloring | Request Activity | Low | High — connects to tool-call detection |
| **P0** | Blocked state per slot | Slot Activity | Medium | High — shows which slot is blocked |
| **P1** | Duration sparkline | Request Activity | Low | High — shows latency trends |
| **P1** | Context utilization sparkline | Context Window | Low | High — gives movement to static card |
| **P1** | Context pressure indicator | Context Window | Low | High — warns about context limits |
| **P2** | Per-slot throughput sparkline | Slot Activity | Medium | Medium — per-slot visibility |
| **P2** | Inferred context usage | Context Window | Medium | Medium — works when n_tokens not exposed |
| **P2** | Request rate heatmap | Request Activity | Medium | Medium — shows request patterns |
| **P3** | Phase breakdown per slot | Slot Activity | Low | Low — nice-to-have |
| **P3** | Tokens/request distribution | Request Activity | Low | Low — historical insight |
| **P3** | Utilization history sparkline | Slot Activity | Low | Low — replaces static bar |

## Implementation Notes

### Shared Components

Several proposals reuse the same building blocks:
- **Sparkline rendering:** Existing `renderSparkline()` function can be reused for all sparkline proposals
- **Color palette:** Amber (`#ebcb8b`/`#f4db9f`) for blocked/warning, green (`#a3be8c`) for normal, red (`#bf616a`) for critical
- **CSS animations:** `dot-pulse`, `card-edge-breathe`, `bar-shimmer` are all reusable
- **Card state system:** `is-live`, `is-idle`, `is-blocked`, `is-dormant` pattern is consistent

### Backend Considerations

Most proposals are frontend-only, consuming existing metrics. The exceptions:
- **Per-slot blocked tracking:** Requires extending `SlotBlockTracker` to track all slots
- **Per-slot throughput history:** Requires tracking `n_decoded` history per slot in poller
- **Inferred context usage:** Requires tracking cumulative token deltas in poller
- **Request context footprint:** Requires tracking context tokens at request boundaries

### Frontend Layout

The detail grid (`inference-detail-grid`) has 3 cards in a row. Adding content to these cards should respect the existing `min-height` and `gap` constraints to avoid layout shifts. The blocked indicator in the throughput card uses `opacity: 0` / `max-height` collapse to avoid affecting layout when hidden.
