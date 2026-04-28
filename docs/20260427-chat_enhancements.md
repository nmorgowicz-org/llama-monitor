# Chat Enhancements — Implementation Plan

**Date:** 2026-04-27
**Status:** Planned
**Priority:** Medium

---

## Problem

Current chat implementation has limitations:
- No streaming — client waits for full response before displaying
- No abort capability — can't stop generation mid-stream
- Uses HTTP POST with manual SSE parsing (broken)

## Proposed Solution

Replace HTTP POST chat endpoint with WebSocket-based bidirectional communication.

## Architecture

### Current Flow
```
Client → POST /api/chat → Dashboard → POST llama-server → Full Response → Client
```

### Proposed Flow
```
Client ↔ WebSocket /ws ↔ Dashboard ↔ HTTP llama-server → Streaming chunks → Client
```

## Implementation Requirements

### 1. Server-Side (`src/web/ws.rs`)

**Current state:** Read-only WebSocket (metrics push only)
```rust
// Current: server → client only
while let Some(_msg) = ws_rx.next().await {}
```

**Required changes:**
- Handle incoming client messages (chat requests)
- Route messages to appropriate handlers (chat vs. control)
- Maintain separate WebSocket connections for metrics vs. chat
- Or add message type discrimination (e.g., `{ "type": "chat", "message": [...] }`)

**Estimated effort:** 2-3 hours

### 2. Server-Side (`src/web/api.rs`)

**Remove:** `api_chat()` POST endpoint
**Add:** Chat handling in WebSocket message router

**Streaming logic:**
- Forward llama-server SSE chunks to client via WebSocket
- Handle client disconnect → cancel upstream request (Tokio auto-cancellation)
- No manual abort needed — connection drop cancels task

### 3. Client-Side (`static/app.js`)

**Current:** `fetch()` POST with broken SSE parsing
**Required:** WebSocket-based chat

**Changes:**
- Create separate WebSocket connection for chat (or reuse existing with message types)
- Send chat requests via WebSocket
- Handle streaming chunks as they arrive
- Implement stop button (close WebSocket → server cancels upstream)

**Estimated effort:** 1-2 hours

### 4. Message Protocol

Add message type discrimination to WebSocket:
```json
// Client → Server
{ "type": "chat", "messages": [...], "stream": true }

// Server → Client (streaming)
{ "type": "chat_chunk", "delta": { "content": "..." } }

// Server → Client (complete)
{ "type": "chat_done" }

// Server → Client (error)
{ "type": "chat_error", "error": "..." }
```

## Benefits

- ✅ Real-time streaming (tokens appear as generated)
- ✅ Proper abort (close WebSocket → upstream cancels)
- ✅ Single connection for all communication
- ✅ No SSE/POST limitations

## Risks

- Breaking change to WebSocket protocol
- Requires careful testing of metrics + chat coexistence
- Client-side reconnection logic needed

## Alternative Approaches Considered

1. **SSE with GET** — Can't send chat history in URL
2. **Server-Sent Events with fetch()** — Browsers don't support POST with SSE
3. **Long-polling** — Latency, complexity, no streaming

## Timeline

Estimated total: 4-6 hours of implementation + testing

---

## Notes

- Current WebSocket infrastructure exists but is read-only
- Tokio's automatic task cancellation on disconnect makes abort trivial
- No additional dependencies needed — `warp` + `tokio` already in use
