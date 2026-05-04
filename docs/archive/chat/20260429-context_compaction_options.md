# Chat Context Compaction — 2026-04-29

## Problem

Long conversations fill the model's context window. Once the KV cache is exhausted llama-server rejects new requests or silently truncates the prompt. The current UI shows `N% ctx` in each message footer, but does nothing to prevent the user from hitting the wall.

Goal: let the user recover from (or pre-empt) a full context window without having to start a new tab.

---

## Current State — What Already Exists

These are the hooks a future implementation can build directly on.

**`tab.messages`** (`static/app.js`) — array of `{ role, content, timestamp_ms, input_tokens, output_tokens }`. The system prompt lives at index 0 with `role: 'system'`. This is the only source of truth; it is what gets sent to the model on every turn.

**`tab.visible_message_limit`** — already controls how many messages are *rendered*. Compaction is different: it controls what is *sent to the model*. Do not conflate them.

**Context % in message metadata** — `finalizeAssistantMessage()` (`app.js`) writes `ctx%` into each completed assistant message via the token metadata. The raw value comes from the SSE response's `timings` or from the `X-Cache-Tokens`-style fields depending on the llama-server version. Look for the `ctx` field in `parts.push(...)` inside `finalizeAssistantMessage`. This is the trigger signal for auto-compaction.

**`/api/chat` route** (`src/web/api.rs`) — proxies to `llama-server /v1/chat/completions`. It sends `tab.messages` (minus `visible_message_limit` filtering — all messages go to the model today). A summarization call for Option B would hit this same endpoint.

**`scheduleChatPersist()`** — debounced persistence for `tab`. Any mutation to `tab.messages` must be followed by this call.

---

## Shared Concepts (Both Options)

### What "compact" means structurally

Compaction always:
1. Preserves the system prompt (`messages[0]` where `role === 'system'`, if present)
2. Removes some number of the oldest non-system messages
3. Inserts a compaction tombstone so the user and future agents know it happened
4. Persists the result

The tombstone is a lightweight message injected at the boundary:
```js
{
  role: 'system',
  content: '[Context compacted — N messages summarized. Conversation continues below.]',
  compaction_marker: true,   // custom field, ignored by the model
  timestamp_ms: Date.now(),
}
```

This gives the UI something to render distinctively (a horizontal rule or badge) and tells a future reader the history is not complete.

### How many messages to keep

The "keep tail" length should be configurable but default to **10 messages** (5 exchanges). This is enough to preserve the immediate working context. The compacted block is everything before that tail (after the system prompt).

### The trigger signal

Auto-compaction needs a reliable ctx% value. The best place to read it is inside `finalizeAssistantMessage()`, immediately after the token metadata is written. At that point `tab.lastCtxPct` (a field to add) can be updated. A post-finalize check then decides whether to compact:

```js
if (tab.auto_compact && tab.lastCtxPct >= tab.compact_threshold) {
    compactChatTab(tab);
}
```

`compact_threshold` defaults to `0.80` (80%). Store both on the tab object so they persist per-tab.

---

## Option A — Truncation

Drop the oldest messages outright. No model call. Instant.

### Core function

```js
function compactChatTab(tab, keepTail = 10) {
    const msgs = tab.messages;
    const systemMsg = msgs[0]?.role === 'system' ? msgs[0] : null;
    const nonSystem = msgs.filter(m => m.role !== 'system');

    if (nonSystem.length <= keepTail) return; // nothing to do

    const dropped = nonSystem.length - keepTail;
    const kept = nonSystem.slice(-keepTail);

    const tombstone = {
        role: 'system',
        content: `[Context compacted — ${dropped} messages removed to free context space.]`,
        compaction_marker: true,
        timestamp_ms: Date.now(),
    };

    tab.messages = [
        ...(systemMsg ? [systemMsg] : []),
        tombstone,
        ...kept,
    ];
    tab.updated_at = Date.now();
    scheduleChatPersist();
    renderChatMessages();
}
```

### UI

**Manual trigger:** Add a "Compact" button to the chat header. Visually similar to the existing `#btn-model-params` button. Only active when `tab.messages.length > keepTail + 1`.

**Auto trigger:** Toggle + threshold slider in the system prompt panel (`#chat-system-panel`), alongside the existing message limit input. Store `tab.auto_compact` (bool) and `tab.compact_threshold` (0.5–0.95, default 0.8) on the tab.

**Tombstone rendering:** `buildMessageElement()` should detect `compaction_marker: true` and render a distinct divider instead of a normal bubble — a thin horizontal rule with centered text like "— context compacted —".

**Files to touch:**
- `static/app.js` — `compactChatTab()`, `finalizeAssistantMessage()` (read ctx%, fire auto-compact), `buildMessageElement()` (tombstone render), `setChatBusyUI()` or a new `initCompactBtn()`
- `static/index.html` — compact button in `.chat-header-left`, threshold controls in `#chat-system-panel`
- `static/css/chat.css` — tombstone divider style (`.chat-compact-marker`)

**Effort: ~3–4 hours**

### Tradeoff

The model has no memory of what was in the dropped messages. If the user references something from 20 turns ago, the model won't know what they mean. Acceptable for task-oriented sessions (code, analysis); poor for long conversational threads.

---

## Option B — Summarization-Based Compaction

Before discarding old messages, ask the model to summarize them. Inject the summary so the model retains the gist. Builds directly on Option A — the `compactChatTab()` structure stays, a summarization step is inserted before the drop.

### Flow

```
1. Identify the messages to drop (same tail logic as Option A)
2. Fire POST /api/chat with:
     - system: "You are a conversation summarizer. Be concise."
     - user:   <dropped messages formatted as a transcript>
             + "Summarize this conversation segment in 3-5 sentences,
                preserving key facts, decisions, and context the reader
                needs to continue the conversation."
3. Collect the summary (non-streaming, or streaming into a status area)
4. Build tombstone with summary embedded:
     content: `[Context compacted — ${dropped} messages summarized]\n\n${summary}`
5. Replace dropped messages with tombstone (same as Option A)
```

### Key design choices

**Same endpoint, different system prompt.** The summarization call goes to `/api/chat` like a normal message. The difference is that it uses a throwaway message array (not `tab.messages`) and the result is captured in JS, not appended to the tab. The existing `sendChat()` / SSE infrastructure can be reused with a `summarizeOnly: true` flag, or a new lighter `fetchSummary(messages)` async function that drives the SSE reader without touching the UI.

**Non-streaming preferred for summarization.** Streaming the summary into the tab creates a confusing in-progress state. Better to collect the full summary silently, then apply the compaction atomically. If the model supports it, pass `"stream": false` in the request body. If not, drive the SSE reader to completion in memory before committing.

**The transcript format.** Feed the dropped messages as a simple labeled transcript:
```
User: <content>
Assistant: <content>
User: <content>
...
```
Strip token metadata. Truncate individual messages longer than ~500 chars with `[truncated]` to avoid the summarization call itself consuming too much context.

**Model availability.** Option B requires the llama-server to be online and a model loaded. If `sendChat()` returns a network error or a 503, fall back to Option A silently (drop without summary) and note this in the tombstone: `[Context compacted — server unavailable for summarization; N messages dropped]`.

**Latency.** Summarizing 20+ messages of a technical conversation can take 5–30 seconds on a local model. Show a non-blocking status in the header during this time (e.g. the compact button enters a spinner state). Do not block the user from reading the thread while summarization runs. Do block sending a new message until compaction completes (set `chatBusy = true`).

**Files to touch (delta over Option A):**
- `static/app.js` — `fetchSummary(messages)` async function, modified `compactChatTab()` to await summary before committing, busy state during summarization
- `src/web/api.rs` — no changes needed; `/api/chat` already handles this
- `static/css/chat.css` — compact button spinner state (already exists as `.chat-send-spinner`)

**Effort: ~7–9 hours** on top of a working Option A

---

## Implementation Order

**Start with Option A.** The structural changes (tombstone message type, `compactChatTab()`, settings controls, auto-trigger in `finalizeAssistantMessage()`) are required by both options. Getting A working first validates the UX and the trigger mechanism before adding the async summarization layer.

**Upgrade path to B is additive.** `compactChatTab()` becomes `async` and gains a `summarize: bool` parameter. If false (or the server is unavailable), it behaves exactly as Option A. The tombstone format accommodates both cases. No rework required.

---

## Detailed Implementation Plan

### Phase 1: Backend — Add compaction fields to Rust structs

**Why first:** Without these fields, compaction settings won't survive page reloads. Serde silently drops unknown fields by default.

**File:** `src/web/api.rs`

1. **Add fields to `ChatTab` struct** (~line 76):
   ```rust
   #[serde(default)]
   pub auto_compact: Option<bool>,
   #[serde(default)]
   pub compact_threshold: Option<f32>,
   ```
2. **Add field to `ChatMessage` struct**:
   ```rust
   #[serde(default)]
   pub compaction_marker: Option<bool>,
   ```
3. **Update `normalizeTabForSave()`** (if it exists) to ensure these fields are not stripped during serialization.

### Phase 2: Option A — Truncation compaction (frontend)

**File:** `static/app.js`

1. **Implement `compactChatTab(tab, keepTail, summarize)`** — use the pseudocode from the "Option A" section above. Make it `async` from the start to accommodate Phase 3 later.

2. **Modify `finalizeAssistantMessage()`** (~line 6680):
   - After computing `ctxPct` (line 6760), store it: `tab.lastCtxPct = ctxPct`
   - After the function completes, add the auto-compaction guard:
     ```js
     if (tab.auto_compact && tab.lastCtxPct >= (tab.compact_threshold || 0.8) * 100) {
         await compactChatTab(activeTab(), null, false);
     }
     ```

3. **Modify `buildMessageElement()`** (~line 6571):
   - Add guard at top: if `msg.compaction_marker`, render a divider instead of a bubble
   - Return early with a `.chat-compact-marker` wrapper

4. **Add `compactionInProgress` flag** (~line 6167):
   - Do NOT reuse `chatBusy` — it blocks tab switching and reading
   - New flag only blocks sending: modify `sendChat()` (~line 6786) to also check `if (compactionInProgress) return`
   - Set `compactionInProgress = true` at start of `compactChatTab()`, `false` at end

**File:** `static/index.html`

5. **Add compact button** to `.chat-header-left` (near `#btn-model-params`)
6. **Add auto-compaction controls** to `#chat-system-panel` (~line 678): toggle + threshold slider, alongside the existing message limit input

**File:** `static/css/chat.css`

7. **Add `.chat-compact-marker` styles** — horizontal rule with centered text

### Phase 3: Option B — Summarization layer

**File:** `static/app.js`

1. **Implement `fetchSummary(messages)`** — new async function. Do NOT try to reuse `sendChat()` — it's too tightly coupled to UI operations (reads `#chat-input`, pushes to `tab.messages`, renders, always streams).
   - Build a throwaway messages array: system prompt + transcript
   - Call `/api/chat` via `fetch()`
   - Consume the SSE stream in memory (backend always streams — no non-streaming mode)
   - Return the full summary string
   - On network error / 503, return `null` (signals fallback to Option A)

2. **Transcript formatting helper** — convert dropped messages to:
   ```
   User: <content>
   Assistant: <content>
   ```
   Strip token metadata. Truncate messages >500 chars with `[truncated]`.

3. **Modify `compactChatTab()`** — when `summarize === true`:
   - Call `fetchSummary(droppedMessages)`
   - If result is non-null, embed in tombstone content
   - If `null`, fall back to Option A tombstone: `[Context compacted — server unavailable; N messages dropped]`
   - Show spinner on compact button during summarization (reuse `.chat-send-spinner` CSS)

### Phase 4: Polish

1. **Multi-compact support** — ensure compaction can run multiple times. Tombstones from previous compactions must be treated as regular system messages (kept, not re-summarized).
2. **Settings persistence validation** — verify `auto_compact` and `compact_threshold` survive page reload.
3. **UI edge cases** — what happens when all messages are compacted? When server is offline? When model is not loaded?

---

## Task List

### Backend

- [ ] Add `auto_compact: Option<bool>` to `ChatTab` struct
- [ ] Add `compact_threshold: Option<f32>` to `ChatTab` struct
- [ ] Add `compaction_marker: Option<bool>` to `ChatMessage` struct
- [ ] Add unit tests for serialization/deserialization of new fields

### Frontend — Option A

- [ ] Implement `compactChatTab(tab, keepTail, summarize)` (async, truncation logic)
- [ ] Add `compactionInProgress` flag (separate from `chatBusy`)
- [ ] Modify `sendChat()` to check `compactionInProgress`
- [ ] Store `tab.lastCtxPct` in `finalizeAssistantMessage()`
- [ ] Add auto-compaction guard in `finalizeAssistantMessage()`
- [ ] Modify `buildMessageElement()` to render tombstones
- [ ] Add compact button to `.chat-header-left`
- [ ] Add auto-compaction controls (toggle + slider) to `#chat-system-panel`
- [ ] Add `.chat-compact-marker` CSS styles

### Frontend — Option B

- [ ] Implement `fetchSummary(messages)` (new function, no `sendChat()` reuse)
- [ ] Implement transcript formatting helper
- [ ] Modify `compactChatTab()` to call `fetchSummary()` when `summarize === true`
- [ ] Add fallback to Option A on network error
- [ ] Add spinner state to compact button during summarization

### Polish

- [ ] Verify multi-compact works (tombstones preserved, not re-summarized)
- [ ] Verify settings survive page reload
- [ ] Handle edge cases: offline server, no model loaded, empty tab

---

## Open Questions for Implementer

- **keepTail default.** 10 messages (5 exchanges) is a reasonable start. Consider making it a global preference rather than per-tab.
- **Threshold default.** 80% is conservative. Users doing long coding sessions may want 90% or even manual-only. Expose as a per-tab setting.
- **Export.** `exportChatHistory()` (if it exists) should export the *full* pre-compaction history if possible, or at least note that compaction occurred. Currently `tab.messages` is the only record — consider whether a `tab.full_messages_archive` is worth maintaining for export purposes.
- **Context % source.** The current `ctxPct` computation uses cumulative token estimates, not actual KV cache measurements. This is consistent with the doc's fallback approach, but may not match actual context usage if the model was reloaded.
