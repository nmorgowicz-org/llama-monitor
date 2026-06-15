# Native MCP Support

Date: 2026-06-15
Status: Proposal / ready to implement

Goal: Add first-class MCP (Model Context Protocol) support to llama-monitor's own chat UI, so users can connect any MCP server (web search, memory, compression, file tools, etc.) and have tool calls resolved transparently mid-conversation — without touching a second UI or doing any manual wiring.

---

## 1. Problem / Motivation

llama-server ships a built-in web UI with an MCP proxy (`--ui-mcp-proxy`, already being passed). That proxy only works inside the llama-server web UI, not in llama-monitor's own chat. Telling users to open a second browser tab and configure MCPs there is bad UX.

The ideal experience: you paste a URL, name the server, optionally add a token, and from that moment every chat in llama-monitor has access to those tools — with zero other steps.

---

## 2. Architecture Overview

```
User types message
      │
      ▼
[JS: _doSendChat]
  - Fetches tool list from /api/mcp/tools (if MCPs enabled)
  - Injects "tools" array into the completions request
  - Streams response
      │
      ├─ finish_reason = "stop"  ──────────────────► render reply, done
      │
      └─ finish_reason = "tool_calls"
           │
           ▼
     [JS: tool call loop]
       - Collects tool_call objects from the delta stream
       - POSTs each to /api/mcp/call (Rust proxy)
             │
             ▼
         [Rust: McpClient]
           - Routes to correct server by server_id
           - Calls MCP server's tools/call
           - Returns JSON result
             │
             ▼
       - Appends assistant message (with tool_calls) + tool result messages
       - Re-sends full message history to completions endpoint
       - Loops until finish_reason = "stop"
```

The Rust backend is a **thin proxy** for MCP calls — it holds the auth tokens, prevents CORS issues, and abstracts the MCP transport. All the tool call orchestration logic (the loop, message injection, render) lives in the **JS frontend**, matching the existing pattern where _doSendChat owns the chat lifecycle.

---

## 3. MCP Protocol Basics (what the Rust client needs to speak)

MCP uses JSON-RPC 2.0. Two transports matter:

### 3a. Streamable HTTP (current standard)
- Single endpoint URL (e.g. `https://host/mcp`)
- All calls are `POST` with `Content-Type: application/json`
- Discovery: `POST /mcp` body `{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"llama-monitor","version":"1.0"}}}`
- Tool list: `POST /mcp` body `{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}`
- Tool call: `POST /mcp` body `{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"tool_name","arguments":{...}}}`
- Response is JSON (not streaming for tool calls)
- Some servers require `Mcp-Session-Id` header returned from initialize (track per-server)

### 3b. SSE transport (legacy, still common)
- Two endpoints: GET `{url}/sse` for server→client events, POST `{url}/messages` for client→server calls
- The GET returns an event stream; the first event contains an endpoint URL for POSTing
- Substantially more complex; implement only if needed

**Recommendation**: Implement HTTP transport first. It covers the user's dockermisc1 stack (hindsight, headroom, searxng/crawl4ai). Add SSE later if a specific server requires it.

---

## 4. Data Model

### 4a. Rust: `McpServerConfig` (add to `src/state.rs`)

```rust
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct McpServerConfig {
    /// Stable client-generated UUID. Used to route /api/mcp/call requests.
    pub id: String,
    /// Human label shown in the UI.
    pub name: String,
    /// Base MCP endpoint URL, e.g. "https://dockermisc1:3001/mcp"
    pub url: String,
    /// Optional Bearer token. Stored encrypted (same pattern as other tokens in config).
    pub auth_token: Option<String>,
    /// Transport type: "http" (default) or "sse"
    pub transport: String,
    /// Whether this server participates in chat tool injection.
    pub enabled: bool,
}
```

### 4b. Rust: Add to `UiSettings` in `src/state.rs` (after line 251, before `SleepModeConfig`)

```rust
/// MCP servers available to llama-monitor's chat UI.
#[serde(default)]
pub mcp_servers: Vec<McpServerConfig>,
```

Because `UiSettings` is already serialized/deserialized with `serde(default)`, adding this field is backwards-compatible — old `ui-settings.json` files simply get an empty vec on load.

**Auth token encryption**: On save via `PUT /api/settings`, detect any `McpServerConfig.auth_token` that doesn't start with `enc:`, encrypt it with `encrypt_value()` (same AES-256-GCM path used for TLS/ACME credentials in `src/config.rs`). On load via `GET /api/settings`, strip the `enc:` prefix for the full settings endpoint (used only server-side). The masked public endpoint (`GET /api/settings`, not `/full`) must redact `auth_token` to `"***"` (same pattern as `mask_remote_agent_token` in `src/web/api.rs:7300`).

### 4c. JS: tab-level MCP toggle

Add `mcp_enabled: true` to the chat tab schema (default true). This is an in-memory field — not persisted to the DB. Lets users disable tools for a specific conversation (e.g. creative writing where tool calls would be disruptive).

---

## 5. Rust Backend: New MCP Routes

All three routes live in `src/web/api.rs`. Register them in the main route combinator near the bottom of `build_routes()`.

### 5a. `GET /api/mcp/tools`

Returns the merged tool list from all **enabled** MCP servers. The frontend calls this before each `_doSendChat` invocation that has MCPs active.

```
GET /api/mcp/tools
Authorization: Bearer {api_token}

Response 200:
{
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "mcp__{server_id}__{tool_name}",   // namespaced to avoid collisions
        "description": "...",
        "parameters": { ... }   // JSON Schema from MCP tools/list
      },
      "_mcp_server_id": "abc-uuid",
      "_mcp_tool_name": "tool_name"
    }
  ],
  "servers": [
    { "id": "abc-uuid", "name": "Hindsight", "ok": true, "tool_count": 7 },
    { "id": "def-uuid", "name": "Headroom", "ok": false, "error": "timeout" }
  ]
}
```

Tool names are namespaced as `mcp__{server_id}__{tool_name}` so they don't collide across servers and the call router can extract both pieces directly.

**Implementation notes**:
- Fetch all enabled servers in parallel (join/select_all).
- Per-server timeout: 5 seconds.
- Servers that fail are reported in `servers[*].error` but don't block the others.
- Cache the tool list per-server in `AppState` for 60 seconds to avoid hammering MCPs on every message. Invalidate the cache when settings are saved.

Add to `AppState` in `src/state.rs`:
```rust
pub mcp_tools_cache: Arc<Mutex<HashMap<String, (Vec<serde_json::Value>, std::time::Instant)>>>,
```

### 5b. `POST /api/mcp/call`

Executes a single tool call against the appropriate MCP server.

```
POST /api/mcp/call
Content-Type: application/json
Authorization: Bearer {api_token}

Body:
{
  "server_id": "abc-uuid",
  "tool_name": "recall",
  "arguments": { "query": "what is the user's name?" }
}

Response 200:
{
  "content": [
    { "type": "text", "text": "The user's name is Nick." }
  ]
}

Response 4xx/5xx:
{
  "error": "MCP server returned error: ...",
  "is_tool_error": true   // tells the JS to inject as tool result with isError=true
}
```

**Implementation notes**:
- Look up server by `server_id` in `ui_settings.mcp_servers`.
- Decrypt `auth_token` before use.
- Build HTTP POST with JSON-RPC 2.0 `tools/call` method.
- Set timeout to 30 seconds (tools like web search can be slow).
- Return MCP's `result.content` array directly.
- On MCP-level errors (JSON-RPC error object), return `{ "error": "...", "is_tool_error": true }` with HTTP 200 so the JS can inject the error as a tool result and let the model recover gracefully rather than aborting the loop.

### 5c. `POST /api/mcp/test`

Tests connectivity to a server URL without saving it. Used by the UI when the user clicks "Test connection".

```
POST /api/mcp/test
Content-Type: application/json
Authorization: Bearer {api_token}

Body:
{
  "url": "https://dockermisc1:3001/mcp",
  "auth_token": "optionaltoken"
}

Response 200:
{
  "ok": true,
  "tool_count": 12,
  "tools": ["recall", "retain", "reflect", ...]   // first 20 tool names for preview
}

// or on failure:
{
  "ok": false,
  "error": "Connection refused"
}
```

---

## 6. JS Frontend Changes

### 6a. New module: `static/js/features/mcp-client.js`

A thin module that owns MCP state and exposes helpers to `chat-transport.js`.

```js
// Cached tool list: { tools: [...], servers: [...], fetchedAt: timestamp }
let _mcpToolCache = null;
const CACHE_TTL_MS = 60_000;

export async function getMcpTools() {
  if (_mcpToolCache && Date.now() - _mcpToolCache.fetchedAt < CACHE_TTL_MS) {
    return _mcpToolCache;
  }
  const resp = await fetch('/api/mcp/tools', { headers: window.authHeaders?.() });
  if (!resp.ok) return { tools: [], servers: [] };
  const data = await resp.json();
  _mcpToolCache = { ...data, fetchedAt: Date.now() };
  return data;
}

export function invalidateMcpCache() {
  _mcpToolCache = null;
}

export async function callMcpTool(serverId, toolName, args) {
  const resp = await fetch('/api/mcp/call', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...window.authHeaders?.() },
    body: JSON.stringify({ server_id: serverId, tool_name: toolName, arguments: args }),
  });
  return await resp.json();
}

// Parse "mcp__{server_id}__{tool_name}" back to parts
export function parseMcpToolName(name) {
  const m = name.match(/^mcp__([^_]+(?:_[^_]+)*)__(.+)$/);
  if (!m) return null;
  return { serverId: m[1], toolName: m[2] };
}

export function hasMcpServers(settings) {
  return (settings?.mcp_servers || []).some(s => s.enabled);
}
```

### 6b. Changes to `static/js/features/chat-transport.js`

All changes are inside `_doSendChat`. The core flow is extended after building `messages` and before calling `fetch('/api/chat')`.

**Step 1: Inject tools** (add after line ~545 where `messages` array is assembled)

```js
import { getMcpTools, callMcpTool, parseMcpToolName, hasMcpServers } from './mcp-client.js';

// MCP tool injection
let mcpToolsData = null;
const tabMcpEnabled = tab.mcp_enabled !== false;  // default true
if (tabMcpEnabled && hasMcpServers(window._appSettings)) {
  mcpToolsData = await getMcpTools();
}
const mcpTools = mcpToolsData?.tools || [];
```

**Step 2: Add `tools` to the completions payload** (inside the `JSON.stringify` body)

```js
body: JSON.stringify({
  messages,
  stream: true,
  temperature: params.temperature,
  // ... other params ...
  ...(mcpTools.length > 0 ? { tools: mcpTools, tool_choice: 'auto' } : {}),
}),
```

**Step 3: Handle `finish_reason: "tool_calls"` in the streaming loop**

After the existing streaming loop, add detection of tool call finish. The current loop accumulates `delta.content` into `msgContent`. Extend it to also accumulate `delta.tool_calls` into a `toolCallAccumulator`.

```js
// In the streaming loop, alongside delta.content handling:
if (delta.tool_calls) {
  for (const tc of delta.tool_calls) {
    const idx = tc.index ?? 0;
    if (!toolCallAccumulator[idx]) {
      toolCallAccumulator[idx] = { id: tc.id || '', type: 'function', function: { name: '', arguments: '' } };
    }
    if (tc.id) toolCallAccumulator[idx].id = tc.id;
    if (tc.function?.name) toolCallAccumulator[idx].function.name += tc.function.name;
    if (tc.function?.arguments) toolCallAccumulator[idx].function.arguments += tc.function.arguments;
  }
}
```

After the streaming loop, check for tool calls:

```js
const finishReason = lastFinishReason;  // captured from the stream
if (finishReason === 'tool_calls' && toolCallAccumulator.length > 0 && mcpTools.length > 0) {
  // Don't finalize the message yet — enter the tool call loop
  const loopResult = await _runMcpToolLoop({
    tab, messages, toolCallAccumulator, mcpTools, params,
    onToolStart: (toolName) => { /* update indicator */ },
    onToolDone: (toolName, result) => { /* update indicator */ },
  });
  // loopResult contains the final assistant content
  msgContent = loopResult.finalContent;
  tokenUsage = loopResult.tokenUsage;
}
```

**Step 4: `_runMcpToolLoop` function** (new private function in chat-transport.js)

```js
async function _runMcpToolLoop({ tab, messages, toolCallAccumulator, mcpTools, params, onToolStart, onToolDone }) {
  const MAX_ITERATIONS = 8;  // prevent infinite loops
  let iteration = 0;
  let currentMessages = [...messages];
  let finalContent = '';
  let tokenUsage = null;

  while (iteration < MAX_ITERATIONS) {
    iteration++;

    // 1. Append assistant turn with tool_calls
    const assistantMsg = {
      role: 'assistant',
      content: null,  // null when tool_calls are present (OpenAI spec)
      tool_calls: toolCallAccumulator,
    };
    currentMessages.push(assistantMsg);

    // 2. Execute each tool call in parallel (or sequential if preferred)
    const toolResults = await Promise.all(
      toolCallAccumulator.map(async (tc) => {
        const parsed = parseMcpToolName(tc.function.name);
        if (!parsed) return null;

        onToolStart?.(tc.function.name);
        let args;
        try { args = JSON.parse(tc.function.arguments || '{}'); } catch { args = {}; }

        const result = await callMcpTool(parsed.serverId, parsed.toolName, args);
        onToolDone?.(tc.function.name, result);

        // Format as OpenAI tool result message
        const contentStr = result.error
          ? `Error: ${result.error}`
          : (Array.isArray(result.content)
              ? result.content.map(c => c.text ?? JSON.stringify(c)).join('\n')
              : JSON.stringify(result));

        return {
          role: 'tool',
          tool_call_id: tc.id,
          content: contentStr,
        };
      })
    );

    // Append tool result messages (filter nulls)
    currentMessages.push(...toolResults.filter(Boolean));

    // 3. Re-send to completions endpoint and stream the next response
    const { content, nextToolCalls, usage, finishReason } = await _streamCompletions({
      messages: currentMessages,
      tools: mcpTools,
      params,
    });

    tokenUsage = usage;

    if (finishReason !== 'tool_calls' || !nextToolCalls.length) {
      // Model is done calling tools — this is the final response
      finalContent = content;
      break;
    }

    // Model wants to call more tools — loop
    toolCallAccumulator = nextToolCalls;
  }

  if (iteration >= MAX_ITERATIONS) {
    finalContent += '\n\n[Tool call limit reached]';
  }

  return { finalContent, tokenUsage };
}
```

`_streamCompletions` is a helper that does the fetch+stream and returns `{ content, nextToolCalls, usage, finishReason }` — essentially the inner streaming logic factored out from `_doSendChat`. Extract it as a shared private function.

### 6c. Tool call rendering in `static/js/features/chat-render.js`

Tool calls should be shown inline during execution and as a collapsed summary in the final message.

**During execution** (live indicator): When a tool call starts, inject a temporary "thinking" element next to the response bubble:

```html
<div class="mcp-tool-call-live">
  <span class="mcp-tool-icon">⚙</span>
  <span class="mcp-tool-name">recall</span>
  <span class="mcp-tool-spinner">…</span>
</div>
```

**In the final saved message**, store tool call metadata as `message.mcp_tool_calls: [{name, args_summary, result_summary}]`. Render as a collapsed disclosure:

```html
<details class="mcp-tool-calls-summary">
  <summary>3 tools used</summary>
  <div class="mcp-tool-call-entry">
    <span class="mcp-tool-name">recall</span>
    <code class="mcp-tool-args">{"query": "user name"}</code>
    <div class="mcp-tool-result">The user's name is Nick.</div>
  </div>
  ...
</details>
```

This keeps the conversation readable while making tool usage inspectable.

### 6d. Settings panel: `static/js/features/settings.js`

Add a new "MCP Servers" section to the existing settings page (after the current Remote Agent section, around line 1600).

**Section layout**:
```
MCP Servers
  ┌─────────────────────────────────────────────────┐
  │ [+ Add Server]                                  │
  │                                                 │
  │  • Hindsight     https://dockermisc1:3001/mcp   │
  │    7 tools  ●    [Edit] [Delete]                │
  │                                                 │
  │  • Headroom      https://dockermisc1:3002/mcp   │
  │    4 tools  ○    [Edit] [Delete]  (disabled)    │
  └─────────────────────────────────────────────────┘
```

**Add/Edit modal** (reuse the existing `<dialog>` pattern from other modals):
```
Name:        [Hindsight Memory          ]
URL:         [https://dockermisc1:3001/mcp]
Auth Token:  [••••••••••••  ] [Show]
Transport:   (•) HTTP  ( ) SSE
Enabled:     [✓]
             [Test Connection]  [Save]
```

When "Test Connection" is clicked:
1. POST to `/api/mcp/test`
2. Show spinner
3. On success: show "✓ Connected — 12 tools available: recall, retain, reflect, ..."
4. On failure: show "✗ Error: Connection refused"

**How settings are wired**: `settings.js` already calls `GET /api/settings` on load and `PUT /api/settings` on save. The `mcp_servers` array will be included in both payloads automatically, since `UiSettings` is fully serialized. The JS needs to:
- Render the server list from `settings.mcp_servers`
- Generate UUIDs client-side for new servers (`crypto.randomUUID()`)
- Track the full list in memory and write it back on save

`window._appSettings` must be kept fresh — it's already updated after settings saves. `mcp-client.js` reads `window._appSettings.mcp_servers` to decide whether to fetch tools.

### 6e. Chat toolbar: MCP toggle

Add a small tool indicator to the chat input area toolbar (next to the existing model params button). When MCPs are configured:

- **Idle**: pill showing `⚙ 3 tools` (count of available tools)
- **During tool call**: animated pill showing `⚙ Searching…`
- **Disabled for this tab**: grayed pill `⚙ off`

Clicking the pill opens a popover with:
- List of active servers and tool counts
- Toggle to enable/disable MCP for this tab
- Quick link to "Configure MCP servers" (opens settings)

---

## 7. File Change Summary

| File | Change |
|------|--------|
| `src/state.rs` | Add `McpServerConfig` struct; add `mcp_servers: Vec<McpServerConfig>` to `UiSettings`; add `mcp_tools_cache` to `AppState` |
| `src/web/api.rs` | Add `GET /api/mcp/tools`, `POST /api/mcp/call`, `POST /api/mcp/test` handlers; add masking of `auth_token` in `mask_remote_agent_token`; encrypt token on PUT /api/settings |
| `src/llama/server.rs` | Update `--webui-mcp-proxy` → `--ui-mcp-proxy` (line 428, deprecation fix) |
| `static/js/features/mcp-client.js` | **New file**: tool cache, `getMcpTools`, `callMcpTool`, `parseMcpToolName` |
| `static/js/features/chat-transport.js` | Tool injection in `_doSendChat`; `_runMcpToolLoop`; `_streamCompletions` extraction |
| `static/js/features/chat-render.js` | Tool call live indicator; collapsed tool call disclosure in final message |
| `static/js/features/settings.js` | MCP Servers section + add/edit modal |
| `static/index.html` | Add MCP settings section HTML, tool indicator HTML, modal HTML |

---

## 8. Implementation Tasks (ordered)

### Phase 1 — Backend foundation (can be done independently)

**Task 1.1** — Add `McpServerConfig` struct and `mcp_servers` field to `UiSettings` in `src/state.rs`.
- Add `McpServerConfig` struct with fields: `id: String`, `name: String`, `url: String`, `auth_token: Option<String>`, `transport: String` (default `"http"`), `enabled: bool` (default `true`).
- Add `#[serde(default)] pub mcp_servers: Vec<McpServerConfig>` to `UiSettings`.
- Add `pub mcp_tools_cache: Arc<Mutex<HashMap<String, (Vec<serde_json::Value>, std::time::Instant)>>>` to `AppState` and initialize it in `AppState::new()`.

**Task 1.2** — Implement `McpClient` helper in a new `src/mcp/mod.rs`.
- `async fn fetch_tools(url: &str, token: Option<&str>) -> Result<Vec<serde_json::Value>>` — sends JSON-RPC `tools/list`, returns MCP tool definitions.
- `async fn call_tool(url: &str, token: Option<&str>, name: &str, args: serde_json::Value) -> Result<serde_json::Value>` — sends `tools/call`, returns `result.content`.
- HTTP only (transport="http"). POST with `Content-Type: application/json`. Auth via `Authorization: Bearer {token}` header if token present.
- Use existing `reqwest` client pattern from `src/web/api.rs` (`build_upstream_client`).

**Task 1.3** — Add `GET /api/mcp/tools` route in `src/web/api.rs`.
- Requires auth (same `check_api_token` pattern).
- Reads `state.ui_settings.lock().mcp_servers`, filters `enabled: true`.
- Calls `mcp::fetch_tools` for each in parallel (tokio join, 5s timeout per server).
- Namespaces tool names as `mcp__{server_id}__{tool_name}`.
- Caches per-server in `mcp_tools_cache` for 60 seconds.
- Returns merged tool list + per-server status.

**Task 1.4** — Add `POST /api/mcp/call` route.
- Requires auth.
- Accepts `{ server_id, tool_name, arguments }` JSON body.
- Looks up server in `ui_settings.mcp_servers` by id.
- Decrypts `auth_token` if present (same `decrypt_value()` from `src/config.rs`).
- Calls `mcp::call_tool` with 30s timeout.
- Returns `{ content: [...] }` on success, `{ error: "...", is_tool_error: true }` on MCP error.

**Task 1.5** — Add `POST /api/mcp/test` route.
- No state change — just connectivity check.
- Accepts `{ url, auth_token? }`.
- Calls `mcp::fetch_tools`, returns `{ ok: true, tool_count, tools: [first 20 names] }` or `{ ok: false, error }`.

**Task 1.6** — Mask and encrypt `auth_token` in settings endpoints.
- In `mask_remote_agent_token` (or create a parallel `mask_mcp_tokens` function), redact `auth_token` to `"***"` for the public `GET /api/settings`.
- In `PUT /api/settings` handler, before saving, encrypt any `mcp_servers[*].auth_token` values not already prefixed with `enc:`.
- Register all new routes in the main route combinator at the bottom of `build_routes()`.

### Phase 2 — Frontend: settings UI

**Task 2.1** — Add MCP Servers section HTML to `static/index.html`.
- Section with a server list container (`<div id="mcp-servers-list">`).
- "+ Add Server" button.
- Add/Edit `<dialog id="mcp-server-modal">` with fields: Name, URL, Auth Token (with show/hide toggle), Transport radio (HTTP/SSE), Enabled checkbox, Test Connection button, Save/Cancel.

**Task 2.2** — Wire up settings.js to render and manage MCP servers.
- On settings load (`loadSettings()`), call `renderMcpServerList(settings.mcp_servers)`.
- `renderMcpServerList`: for each server, render a row with name, URL (truncated), tool count badge (fetched from cache), enabled toggle, Edit/Delete buttons.
- `openMcpServerModal(server?)`: open the dialog, pre-populate fields if editing.
- "Test Connection": POST to `/api/mcp/test`, show inline status.
- "Save": validate URL is non-empty, generate `crypto.randomUUID()` for new servers, update local settings object, call existing `saveSettings()` flow, call `invalidateMcpCache()`.

### Phase 3 — Frontend: chat integration

**Task 3.1** — Create `static/js/features/mcp-client.js`.
- Exactly as specified in §6a above.
- Export: `getMcpTools`, `callMcpTool`, `parseMcpToolName`, `hasMcpServers`, `invalidateMcpCache`.

**Task 3.2** — Extract `_streamCompletions` from `_doSendChat` in `chat-transport.js`.
- Factor out the inner `fetch('/api/chat')` + streaming loop into a reusable `async function _streamCompletions({ messages, tools, params })` that returns `{ content, thinkContent, nextToolCalls, finishReason, usage }`.
- `_doSendChat` calls it for the initial request.
- `_runMcpToolLoop` calls it for subsequent iterations.

**Task 3.3** — Add `_runMcpToolLoop` to `chat-transport.js`.
- Exactly as specified in §6b above.
- MAX_ITERATIONS = 8.
- Execute tool calls in parallel (Promise.all).
- Append `mcp_tool_calls` metadata to the final message object for rendering.

**Task 3.4** — Hook MCP into `_doSendChat`.
- After message assembly (~line 545), call `getMcpTools()` if MCPs enabled.
- Inject `tools` into completions payload.
- After streaming, if `finishReason === 'tool_calls'`, call `_runMcpToolLoop`.

**Task 3.5** — Tool call rendering in `chat-render.js`.
- Live indicator: when `onToolStart` fires, inject `<div class="mcp-tool-call-live">` element near the active response bubble. Remove/replace when `onToolDone` fires.
- Final render: if `message.mcp_tool_calls?.length`, render a `<details class="mcp-tool-calls-summary">` disclosure before the message content.

**Task 3.6** — MCP toolbar indicator in `index.html` + wiring in a new `mcp-indicator.js` or inline in `chat-state.js`.
- Pill element next to model params button in the chat input toolbar.
- Shows "⚙ N tools" when MCPs are active, "⚙ Calling…" during tool execution, hidden when no MCPs configured.
- Click → popover with server list + tab-level toggle.

---

## 9. UX Principles (follow these during implementation)

- **Zero friction for common case**: if a server is configured and enabled, tools just work — no per-chat config required.
- **Non-destructive when tools fail**: if an MCP server is unreachable at tool-call time, append a tool result message saying `Error: server unavailable` and let the model respond gracefully. Never hard-crash the chat.
- **Transparent, not intrusive**: tool call activity is shown, but compactly. The collapsed `<details>` disclosure keeps the conversation readable.
- **Encrypted at rest**: auth tokens go through `encrypt_value()` before disk. The public settings API never returns the plaintext token — only `"***"`.
- **Disable cleanly**: the tab-level `mcp_enabled` toggle and the global `enabled` per-server give two levels of opt-out without touching config files.

---

## 10. Testing Checklist

- [ ] Settings: add server, save, reload — server appears with correct fields
- [ ] Settings: auth token is redacted to `***` in GET /api/settings response
- [ ] Test Connection button succeeds against a real MCP server
- [ ] Test Connection shows correct error for bad URL
- [ ] Tool list fetched and injected correctly (`tools` key in completions request)
- [ ] Single tool call resolved and final reply renders correctly
- [ ] Multi-tool call loop (model calls >1 tool) terminates correctly
- [ ] MAX_ITERATIONS guard fires after 8 rounds without infinite loop
- [ ] Tool call disclosure in final message is collapsed by default, expandable
- [ ] Live tool indicator appears/disappears correctly during call
- [ ] Disabling a server in settings removes its tools from next injection (cache invalidated)
- [ ] Tab-level MCP toggle disables tool injection for that tab only
- [ ] Chat works normally (no regression) when no MCP servers configured
- [ ] Remote/Attach session: MCP tools work identically (same /api/mcp routes regardless of session mode)

---

## 11. MCP Config Auto-Discovery & Import

Instead of requiring users to manually enter server URLs, llama-monitor should scan well-known config files from other AI tools the user already has installed and offer to import them in one click.

### 11a. Config file locations and formats

The Rust backend reads these paths at import-scan time (all paths relative to the user's home directory):

| Tool | Path | MCP key | Server format |
|------|------|---------|---------------|
| Claude Code | `~/.claude/settings.json` | `mcpServers` | object: `{ url?, command?, args?, env?, type? }` |
| Claude Code (local) | `~/.claude/settings.local.json` | `mcpServers` | same |
| OpenCode | `~/.config/opencode/opencode.json` | `mcp` | object: `{ type: "remote"\|"local", url?, command?, environment?, headers? }` |
| Kilo | `~/.config/kilo/kilo.json` | `mcp` | same as OpenCode |
| Zed | `~/.config/zed/settings.json` | `context_servers` | object: `{ command, args?, env? }` (stdio only) |

(Codex at `~/.codex/config.toml` only exposes internal bundled MCPs in TOML `[mcp_servers.*]` sections — skip it.)

**Real example from this machine** (OpenCode / Kilo, `~/.config/opencode/opencode.json`):
```json
{
  "searxng":   { "type": "local",  "command": ["/opt/homebrew/bin/mcp-searxng", ...] },
  "crawl4ai":  { "type": "remote", "url": "http://192.168.10.65:11235/mcp/sse" },
  "huggingface":{ "type": "remote", "url": "http://192.168.10.65:8808/mcp" },
  "camofox":   { "type": "remote", "url": "http://192.168.10.65:9378/mcp",
                 "headers": { "Authorization": "Bearer camofox-mcp-local-key-..." } },
  "hindsight": { "type": "remote", "url": "http://192.168.10.65:8888/mcp/nick-hermes/" }
}
```

### 11b. What can be imported

Only **remote (HTTP/SSE) servers** are directly importable — llama-monitor doesn't spawn child processes, so stdio-based `local` servers can't be used. The import UI marks local servers as "not supported (stdio)" and skips them unless that limitation is removed later.

Auth token extraction:
- OpenCode/Kilo: `headers.Authorization` → strip `"Bearer "` prefix → becomes `auth_token`
- Claude Code: `env.{TOOLNAME}_API_KEY` or any env var containing "key" or "token" → heuristically extract
- Transport detection: URL ending in `/sse` → `transport: "sse"`, otherwise `transport: "http"`

### 11c. Backend: `GET /api/mcp/discover`

New route, no body required. Returns discovered servers from all installed tools.

```
GET /api/mcp/discover
Authorization: Bearer {api_token}

Response 200:
{
  "sources": [
    {
      "tool": "opencode",
      "config_path": "/Users/nick/.config/opencode/opencode.json",
      "servers": [
        {
          "name": "crawl4ai",
          "url": "http://192.168.10.65:11235/mcp/sse",
          "auth_token": null,
          "transport": "sse",
          "importable": true,
          "already_configured": false
        },
        {
          "name": "camofox",
          "url": "http://192.168.10.65:9378/mcp",
          "auth_token": "camofox-mcp-local-key-2026-iris-secured",
          "transport": "http",
          "importable": true,
          "already_configured": false
        },
        {
          "name": "searxng",
          "url": null,
          "auth_token": null,
          "transport": null,
          "importable": false,
          "skip_reason": "stdio (local) — not supported"
        }
      ]
    },
    {
      "tool": "claude-code",
      "config_path": "/Users/nick/.claude/settings.json",
      "servers": []   // empty in this case
    }
  ]
}
```

**Implementation** (new function `mcp::discover_external_servers(home_dir: &Path) -> Vec<DiscoverySource>`):
- Try each config path; silently skip if file doesn't exist or fails to parse.
- Deduplicate by URL across sources (mark `already_configured: true` if URL already in `ui_settings.mcp_servers`).
- Never error — always return whatever was found.

### 11d. Backend: `POST /api/mcp/import`

Imports a selected subset of discovered servers into `ui_settings.mcp_servers`.

```
POST /api/mcp/import
Content-Type: application/json
Authorization: Bearer {api_token}

Body:
{
  "servers": [
    { "name": "crawl4ai", "url": "...", "auth_token": null, "transport": "sse" },
    { "name": "camofox",  "url": "...", "auth_token": "...", "transport": "http" }
  ]
}

Response 200:
{ "imported": 2, "skipped_duplicates": 0 }
```

Handler:
- Generate a new UUID for each server.
- Encrypt `auth_token` before appending to `ui_settings.mcp_servers`.
- Save settings atomically.
- Invalidate `mcp_tools_cache`.

### 11e. Frontend: import flow

**Trigger**: When the MCP Servers settings section is empty (no servers configured), automatically show a callout:

```
┌──────────────────────────────────────────────────────────┐
│  ✦ Found MCP servers in your other AI tools              │
│    Claude Code, OpenCode, Kilo detected                  │
│                         [Review & Import →]              │
└──────────────────────────────────────────────────────────┘
```

If servers are already configured, show a smaller "Import from other tools" link below the server list.

**Import modal** (new `<dialog id="mcp-import-modal">`):

```
Import MCP Servers

From OpenCode  /Users/nick/.config/opencode/opencode.json
  [✓] crawl4ai        http://192.168.10.65:11235/mcp/sse    (SSE)
  [✓] huggingface     http://192.168.10.65:8808/mcp          (HTTP)
  [✓] camofox         http://192.168.10.65:9378/mcp          (HTTP, token found)
  [✓] hindsight       http://192.168.10.65:8888/mcp/...      (HTTP)
  [ ] searxng         [stdio — not supported]

From Claude Code  /Users/nick/.claude/settings.json
  (no remote servers found)

                              [Import Selected (4)]  [Cancel]
```

After import:
- POST to `/api/mcp/import` with selected servers.
- Close modal.
- Refresh server list — newly imported servers appear immediately.
- Show toast: "4 MCP servers imported. Testing connections…" then run quick connectivity checks in the background and update each row's status badge.

**Task additions** (append to Phase 1 / Phase 2 task list):

- **Task 1.7** — Implement `mcp::discover_external_servers`: parse OpenCode, Kilo, Claude Code config files; return `Vec<DiscoverySource>`.
- **Task 1.8** — Add `GET /api/mcp/discover` route.
- **Task 1.9** — Add `POST /api/mcp/import` route.
- **Task 2.3** — Add import callout + import modal to `index.html` and wire in `settings.js`: call `GET /api/mcp/discover` when opening settings, render modal, POST to `/api/mcp/import` on confirm.

---

## 12. Notes / Open Questions

- **SSE transport**: Not implemented in Phase 1. If a user has an SSE-only MCP, they'll see "Test Connection" fail with a descriptive error. Add SSE support as a follow-up.
- **Tool approval / confirmation**: Some users may want a "confirm before calling" gate for sensitive tools (e.g. file write, email send). Not in scope now — add a per-server `require_confirm: bool` setting later.
- **Streaming tool results**: Some MCP servers support streaming content in tool responses. Not needed now; the non-streaming path covers all current use cases.
- **`--ui-mcp-proxy` in llama-server**: This is the webui's own MCP proxy, separate from ours. Keep passing it (it's harmless and enables the webui's own MCP config for users who open that). But our `/api/mcp/*` routes are independent of it.
- **Context size pressure**: Injecting many tools adds tokens to every request. Consider a per-server `max_tools` filter and/or a relevance-based tool selector for users with large MCP deployments. Not needed for a focused set of 5–10 tools.
