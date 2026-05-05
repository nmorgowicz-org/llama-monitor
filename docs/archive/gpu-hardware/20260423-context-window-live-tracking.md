# 2026-04-24: Live Context Window Tracking

## The Problem

The Context Window card in llama-monitor is largely static. It shows context capacity and a peak-observed watermark, but live usage (`context_live_tokens`) only appears when llama-server exposes `n_tokens`, `n_past`, `n_ctx_used`, or `n_cache_tokens` in the `/slots` response — which varies by llama.cpp version and is often absent.

When live usage is unavailable, the card shows a flat bar and the label "not exposed by llama-server". This is the default experience for most setups.

The goal of this document is to explain:
1. What data llama.cpp/OpenAI endpoint actually provides
2. How openclaw and opencode extract it
3. What llama-monitor needs to do differently
4. How to make the Context Window card premium and animated

---

## Data Sources: What llama.cpp Exposes

### Source A — `/slots` polling (every 500ms, already implemented)

**IMPORTANT: This build does NOT expose live context usage in `/slots`.**

The `/slots` JSON endpoint returns an array of slot objects. On this build, each slot only contains:

```json
{
  "id": 0,
  "n_ctx": 212992,           // ← context window capacity for this slot (always present)
  "is_processing": true,
  "speculative": ...,
  "id_task": ...,
  "params": ...,
  "next_token": ...
}
```

**Fields that are NOT present on this build:**
- `n_past` — not exposed
- `n_tokens` — not exposed
- `n_ctx_used` — not exposed
- `n_cache_tokens` — not exposed

**This means passive monitoring of another client's context usage (e.g., watching openclaw's session) is not possible.** The server simply does not expose a live "tokens currently in context" value through any endpoint.

The only useful field from `/slots` on this build is `n_ctx` (capacity), which is always available.

### Source B — `/metrics` (Prometheus endpoint)

**This build also does NOT expose KV cache metrics in `/metrics`.**

The Prometheus metrics endpoint does not include:
- `kv_cache_usage_ratio` — not present
- `kv_cache_tokens` — not present

The only relevant metric is `n_tokens_max: 37` — a high-water mark (largest context observed since server start), not a live value. This tells you the biggest context that's ever been used, not the current state.

### Source C — `/v1/chat/completions` with `stream_options` (not yet implemented)

The OpenAI-compatible streaming endpoint supports an optional request field:

```json
{
  "messages": [...],
  "stream": true,
  "stream_options": { "include_usage": true }
}
```

When `include_usage` is true, llama.cpp appends a final SSE chunk **before** `data: [DONE]` that contains the exact token count for the completed request:

```
data: {"id":"...","object":"chat.completion.chunk","choices":[],"usage":{"prompt_tokens":7981,"completion_tokens":451,"total_tokens":8432}}

data: [DONE]
```

- `prompt_tokens` — tokens consumed by the input (all messages in the conversation)
- `completion_tokens` — tokens generated in this response
- `total_tokens` — `prompt_tokens + completion_tokens` = tokens now occupying the context window

This is the mechanism openclaw and opencode use. It is reliable, always present when requested, and gives a precise snapshot of context state at the end of each turn regardless of what `/slots` exposes.

**However, this only works for requests that llama-monitor itself makes.** If openclaw is driving the model, llama-monitor cannot see this data because it's only sent to the client that made the request.

### Source D — non-streaming response body (bonus, same mechanism)

For non-streaming calls (`stream: false`), llama.cpp always includes a `usage` object at the top level of the response JSON:

```json
{
  "choices": [...],
  "usage": {
    "prompt_tokens": 18,
    "completion_tokens": 20,
    "total_tokens": 38
  }
}
```

No special configuration needed for non-streaming — the field is always there.

**Same limitation: only available to the client that made the request.**

---

## How openclaw Gets This

openclaw's model config has a flag:

```json
{
  "compat": {
    "supportsUsageInStreaming": true
  }
}
```

When this flag is set for a provider, openclaw injects `"stream_options": {"include_usage": true}` into every streaming request. The streaming response handler reads every SSE chunk, and when it finds one where `choices` is empty but `usage` is present, it extracts `prompt_tokens`, `completion_tokens`, and `total_tokens`. These get stored on the session transcript and surfaced in the UI as:

```
↑43.5k  ↓899  R529.6k  33% ctx
```

- `↑43.5k` — prompt tokens (input)
- `↓899` — completion tokens (output)
- `R529.6k` — total context window capacity (from model config's `contextWindow`)
- `33% ctx` — `total_tokens / contextWindow * 100`

The context window size comes from the **model config** (manually set), not from the API response. This is the one gap in the openclaw approach — it requires knowing `n_ctx` ahead of time.

---

## How opencode Gets This

opencode does the same thing but in a TUI (terminal UI). It:

1. Sends `stream_options: { include_usage: true }` with every streaming request
2. Listens for the usage chunk in the SSE stream
3. Accumulates `total_tokens` across all turns in the session
4. Formats and displays: `87.3K (42%)` in the status bar

The context window size comes from the model's known `contextWindow` setting in its config.

---

## The Advantage llama-monitor Has

Unlike openclaw and opencode, llama-monitor **already knows `n_ctx`** from the `/slots` polling. `context_capacity_tokens` is populated before any chat request is made. We do not need a separate model config — we can use the value we already have.

This means:
- `context_capacity_tokens` = denominator for `%` — from `/slots` ✓
- `total_tokens` after each chat turn = from `stream_options` — **needs implementation**

---

## Critical Limitation: Passive Monitoring Is Not Possible

**After thorough testing, we confirmed that passive context monitoring of another client's session (e.g., watching openclaw while it drives the model) is not possible on this llama.cpp build.**

The server does not expose any endpoint that provides a live "tokens currently in context" value. The only passively observable data is:

- `n_tokens_max` — a high-water mark that only ever goes up, tells you the biggest context that's ever been used since server start, not the current state
- `n_ctx: 212992` — capacity, always available

**The `stream_options` approach is only valid when llama-monitor's own built-in chat tab is being used.** For the passive monitoring use case (watching while openclaw works), you're blocked at the llama.cpp layer.

---

## Gap Analysis: What's Missing

| Capability | Status |
|---|---|
| Context capacity (`n_ctx`) | ✓ Already tracked via `/slots` |
| Live context usage (`n_past`) | ✗ Not exposed by this build |
| Per-turn token count (stream) | ✗ Not implemented — no `stream_options` injection |
| SSE usage chunk parsing | ✗ Frontend silently drops usage chunks |
| Chat UI token annotation | ✗ No per-message context footprint shown |
| Context growth sparkline | ✗ No history tracked |
| Pressure color coding | ✗ No warning levels |
| Passive monitoring of other clients | ✗ Not possible — no endpoint |

---

## Implementation Plan

### 1. Backend: Inject `stream_options` (Rust, `src/web/api.rs`)

The `api_chat` handler currently forwards the request body as opaque bytes. It needs to:
1. Parse the body as JSON
2. If `stream: true`, add `stream_options: { include_usage: true }` (only if not already set)
3. Re-serialize and forward

```rust
// In api_chat, before forwarding the body:
let body_json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
let body_to_send = if body_json.get("stream").and_then(|v| v.as_bool()).unwrap_or(false) {
    let mut modified = body_json.clone();
    if modified.get("stream_options").is_none() {
        modified["stream_options"] = serde_json::json!({ "include_usage": true });
    }
    serde_json::to_vec(&modified).unwrap_or(body.to_vec())
} else {
    body.to_vec()
};
```

The backend currently uses `resp.bytes().await` which buffers the full response. This must remain as-is for the proxy to work — the frontend's `getReader()` will read the buffered SSE content chunk-by-chunk as normal. No streaming architecture changes are required.

### 2. Frontend: Parse the Usage Chunk (`static/app.js`, `sendChat`)

The current SSE parser drops chunks with no `choices[0].delta`:

```js
const delta = obj.choices && obj.choices[0] && obj.choices[0].delta;
if (!delta) continue;  // ← usage chunk is dropped here
```

Add a usage capture branch before the `continue`:

```js
// Check for usage chunk (choices is empty, usage is present)
if (obj.usage) {
    window.lastChatUsage = {
        promptTokens: obj.usage.prompt_tokens || 0,
        completionTokens: obj.usage.completion_tokens || 0,
        totalTokens: obj.usage.total_tokens || 0,
        timestamp: Date.now(),
    };
}
const delta = obj.choices && obj.choices[0] && obj.choices[0].delta;
if (!delta) continue;
```

After `sendChat` completes, append context info to the assistant message element using `window.lastChatUsage` and `window.prevValues.contextCapacity` (already tracked from WebSocket).

### 3. Frontend: Context History Ring Buffer (`static/app.js`)

To power the sparkline, maintain a rolling history array:

```js
// Initialize once (alongside other prevValues setup)
window.contextHistory = [];  // array of { tokens, pct, ts }

// In the WebSocket update handler, after computing contextLive/contextCapacity:
if (contextLiveAvailable && contextCapacity > 0) {
    const pct = (contextLive / contextCapacity) * 100;
    window.contextHistory.push({ tokens: contextLive, pct, ts: Date.now() });
    if (window.contextHistory.length > 120) window.contextHistory.shift();  // 60s at 500ms
}
```

Also push from `window.lastChatUsage` whenever a chat completes:

```js
// After sendChat resolves
if (window.lastChatUsage && contextCapacity > 0) {
    const pct = (window.lastChatUsage.totalTokens / contextCapacity) * 100;
    window.contextHistory.push({
        tokens: window.lastChatUsage.totalTokens,
        pct,
        ts: Date.now(),
        source: 'chat',   // mark as chat-derived vs slot-derived
    });
}
```

### 4. Frontend: Annotate Chat Messages

After each assistant response, inject a small token footer into `msgEl`:

```js
// After streaming loop completes, if we have usage data:
if (window.lastChatUsage && window.lastChatUsage.totalTokens > 0) {
    const cap = window.prevValues.contextCapacity || 0;
    const total = window.lastChatUsage.totalTokens;
    const pct = cap > 0 ? Math.round((total / cap) * 100) : null;
    const label = formatMetricNumber(total) + ' ctx' + (pct !== null ? ' · ' + pct + '%' : '');
    const footerEl = document.createElement('div');
    footerEl.className = 'msg-ctx-footer';
    footerEl.textContent = label;
    msgEl.appendChild(footerEl);
}
```

---

## Context Window Card — Premium UI Redesign

### Current State

```
CONTEXT WINDOW              [live]
Live usage            8.2k live
[████████░░░░░░░░░░░░░░░░░░░░░]
Peak observed                  —
[░░░░░░░░░░░░░░░░░░░░░░░░░░░░░]
            72%
       8.2k / 128.0k
```

### Target State

```
CONTEXT WINDOW              [72% · comfortable]
Live usage                     8.2k / 128k
[████████████████░░░░░░░░░░░░░] 72%
[╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲╱╲] ← 60s sparkline, color-coded
──────────────────────────────────
Peak observed         12.1k (9.5%)  ← from /slots high-water
Last turn         ↑6.1k + ↓451 = 8.2k  ← from stream_options
Est. turns left               ~14  ← (cap - live) / avg_per_turn
```

### Pressure Levels

| Usage % | Chip label | Color | Border behavior |
|---|---|---|---|
| < 60% | comfortable | teal (normal live) | normal |
| 60–80% | moderate | amber | soft glow |
| 80–95% | pressure | orange | breathing edge |
| > 95% | critical | red | breathing edge + pulse |

These reuse the existing chip state system (`setChipState`) and card state classes (`is-live`, `is-blocked`, `is-critical`).

### Sparkline

The context history ring buffer feeds `renderSparkline()` directly. Color the line by pressure level:
- Green segment: pct < 60
- Amber segment: pct 60–80
- Orange segment: pct 80–95
- Red segment: pct > 95

The sparkline gives the card continuous motion even when the server is idle (turns taken earlier are visible in the trace).

### Animated Fill

The progress bar fill already uses `animateNumber` for the percentage. Add a CSS transition on the bar width:

```css
.context-progress-fill {
    transition: width 600ms cubic-bezier(0.4, 0, 0.2, 1),
                background-color 400ms ease;
}
```

When pressure crosses a threshold, the color transition is smooth rather than a snap.

### "Last Turn" Row

A third data row below the peak row shows the most recent chat usage, sourced from `window.lastChatUsage`:

```
Last turn     ↑6.1k prompt + ↓451 gen = 8.2k total
```

This is the exact number from `stream_options` — shown grayed-out when no chat has been sent, updated after each response.

### "Est. turns remaining" Stat

If we know the average tokens-per-turn from the history buffer, we can estimate:

```js
const avgPerTurn = contextHistory.length > 1
    ? (contextHistory[contextHistory.length - 1].tokens - contextHistory[0].tokens) / contextHistory.length
    : null;
const turnsLeft = avgPerTurn > 0 ? Math.floor((contextCapacity - contextLive) / avgPerTurn) : null;
```

Show as `~14 turns` or `—` if no history yet. This is the most useful forward-looking stat for anyone running a long agentic session.

---

## Fallback Behavior

The new `stream_options` source is additive. The existing priority stack becomes:

| Rank | Source | When available |
|---|---|---|
| 1 | `stream_options` `total_tokens` (chat-derived, per-turn) | After each chat completion |
| 2 | Prometheus `n_tokens_max` (peak watermark only) | Always |

**Note:** `/slots` `n_past` / `n_tokens` is no longer a fallback on this build because the server doesn't expose these fields.

When only source 1 is available (no chat sent yet), the card shows the chat-derived value with label "last turn" rather than "live usage", and the chip reads `chat` instead of `live`. The sparkline fills in from chat events only (one point per turn rather than 2/sec), which is sparser but still useful.

When neither source is available (no chat sent yet), the existing "not exposed by llama-server" message remains.

---

## CSS Additions

```css
/* Pressure color palette for context fill */
.context-progress-fill.moderate {
    background: linear-gradient(90deg, #ebcb8b 0%, #f4db9f 100%);
}
.context-progress-fill.pressure {
    background: linear-gradient(90deg, #d08770 0%, #e0a070 100%);
}
.context-progress-fill.critical {
    background: linear-gradient(90deg, #bf616a 0%, #d57a81 100%);
    animation: bar-shimmer 1.5s linear infinite;
}

/* Chat message context footer */
.msg-ctx-footer {
    font-size: 0.72rem;
    color: #4c566a;
    margin-top: 6px;
    letter-spacing: 0.03em;
}
.msg-ctx-footer.moderate { color: #c5a958; }
.msg-ctx-footer.pressure { color: #c07050; }
.msg-ctx-footer.critical { color: #bf616a; }

/* Context sparkline container */
.context-sparkline {
    margin: 4px 0 6px;
    height: 24px;
    opacity: 0.75;
}

/* Third data row: last turn */
.context-last-turn {
    font-size: 0.78rem;
    color: #616e88;
    margin-top: 4px;
    display: flex;
    justify-content: space-between;
}
```

---

## File Map

| File | Change |
|---|---|
| `src/web/api.rs` | Inject `stream_options: { include_usage: true }` in `api_chat` |
| `static/app.js` | Parse usage chunk in SSE loop; maintain `contextHistory` ring buffer; annotate chat messages; update context card render |
| `static/index.html` | Add sparkline container, last-turn row, est-turns-left stat to context card HTML |
| `static/app.css` | Pressure color classes, chat footer styles, sparkline container |

No Rust struct changes needed. No new API endpoints. No WebSocket protocol changes.

---

## Testing

1. Start llama-server with any model, attach llama-monitor
2. Open the chat panel, send a message
3. Verify: final SSE chunk before `[DONE]` contains `usage` field (check browser DevTools → Network → EventStream)
4. Verify: context card shows updated value after response
5. Verify: chat message has footer with token count and percentage
6. Send multiple turns — verify sparkline grows and `est. turns left` decreases
7. Run to 80%+ context — verify chip transitions from `comfortable` → `moderate` → `pressure` with appropriate colors

</parameter>
<parameter=filePath>
/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/20260424-context-window-live-tracking.md