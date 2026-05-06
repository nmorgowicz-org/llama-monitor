# Chat

The chat tab provides multi-tab streaming conversations with the connected llama.cpp server, per-tab configuration, and real-time telemetry.

## Tab Management

- **Multi-tab** — Parallel conversations with independent system prompts, model parameters, and message history
- **Pin tabs** — Pinned tabs stay at the front and persist across sessions
- **Keyboard switching** — Ctrl+1–9 by position, Ctrl+Shift+←/→ to cycle
- **Rename** — Custom tab names persist in `chat-tabs.json`
- **Maximum 10 tabs** — Old inactive tabs are auto-pruned

## Messaging

- **Streaming** — Real-time SSE streaming from `/v1/chat/completions`
- **Reasoning blocks** — Thinking/reasoning content rendered in expandable blocks
- **Markdown rendering** — Full Markdown with syntax-highlighted code blocks (highlight.js, atom-one-dark theme)
- **Code block headers** — Language label, line count, and copy button per block
- **Token estimates** — Input shows `~N tok` with color warnings at 800+ (yellow) and 1500+ (red) tokens
- **Smart scroll** — Auto-scroll only when near bottom; scroll-to-bottom button with unread count badge
- **History pagination** — Long conversations render only the most recent N messages (default 15); "Load More" reveals older batches

### Message Actions

| Action | Description |
|--------|-------------|
| **Edit** | Edit any user message (not just the last one) and regenerate from that point |
| **Regenerate** | Re-send from any user message to get a different response |
| **Copy** | Copy message text to clipboard |
| **Export** | Download entire chat history as formatted JSON |
| **Import** | Import conversations from `.json` (full tab restore) or `.md` (append messages to active tab) |

## System Prompts & Personas

- **Custom system prompts** — Per-tab system prompt with live editing
- **Template library** — Pre-built persona templates with policy management
- **Persona strip** — Click persona chips to switch conversation style; persists per-tab via `active_template_id`
- **Explicit mode** — Toggle for uncensored content on models that require guardrail override

## Model Parameters

Per-tab controls for generation behavior. An active-params dot indicator appears when non-defaults are set.

| Parameter | Description |
|-----------|-------------|
| Temperature | Randomness (0.0–2.0) |
| top_p | Nucleus sampling threshold |
| top_k | Top-k sampling |
| min_p | Minimum probability threshold |
| repeat_penalty | Repetition avoidance |
| max_tokens | Output length limit |

## Context Compaction

Recover from full context windows by summarizing earlier conversation into a tombstone message.

- **Manual compaction** — Click Compact to summarize messages above the threshold
- **Auto-compaction** — Per-tab threshold control; compacts automatically when context pressure exceeds the limit
- **Multi-compact safe** — Tombstones are preserved across re-compactions
- **Context ring** — Live context pressure indicator in the telemetry rail

## Chat Telemetry

Real-time metrics for the active chat tab, accessible via the telemetry toggle in the chat header.

### Summary Rail (always visible)
- **State chip** — Current generation state (idle, prompting, generating)
- **Prompt/Output stage** — Visual indicator of current processing phase
- **Throughput bars** — Live prompt (P) and generation (G) token speeds with mini progress bars
- **Live rate** — Current generation rate in tokens/sec
- **Context ring** — Current tab context pressure with percentage

### Expanded Detail Panel
- **Throughput grid** — Detailed prompt and generation speed metrics
- **Sparkline** — Throughput history chart
- **Task metadata** — Task ID, context usage, model info
- **Slot tiles** — Per-slot status with generation progress
- **Activity timeline** — 5-minute rolling window of recent tasks

### Popup Mode
The telemetry panel can float as a popover or pin inline below the chat toolbar. The popup mode includes a pin toggle to switch between floating and inline layouts.

![Chat Telemetry](../screenshots/03b-chat-telemetry.png)
![Chat Telemetry Pinned](../screenshots/03c-chat-telemetry-pinned.png)

## Data Flow

```
User message → /v1/chat/completions (SSE stream) → Browser renders tokens live
                                                    ↓
                                            WebSocket metrics (500ms) → Telemetry rail updates
```

## Persistence

Chat tabs, messages, system prompts, and model parameters persist to `~/.config/llama-monitor/chat-tabs.json`. Data is saved on every change (debounced) and survives app restarts.
