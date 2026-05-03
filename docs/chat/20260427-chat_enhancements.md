# Chat Enhancements — Full Implementation Plan

**Date:** 2026-04-27  
**Status:** ✅ Implemented + Polished  
**Priority:** High  
**Completed:** 2026-04-28 — All 7 phases implemented. One residual bug found and fixed (dead `chatHistory` reference in sidebar badge updater at `app.js:6004`).  
**Polish:** 2026-04-28 — Added: visual feedback toasts, param reset button, system prompt templates, duplicate tab settings, import conversation, character counter, premium styling (micro-interactions, smooth panel transitions, enhanced focus states).  
**UX Overhaul:** 2026-04-28 — Labeled all header buttons, removed jargon ("llama-server" → plain language), added suggested prompts in empty state, hid advanced params behind toggle, added param tooltips, set safe defaults (temp 0.7, top_p 0.9, helpful assistant prompt), added one-time welcome tip.

---

## Overview

This document is a complete, sequential implementation guide for a full chat overhaul. The existing chat works at a basic level but is missing streaming transport reliability, multi-session tab management, and a modern premium UI. This plan covers every file that must change, every new data structure, and exact behavioral contracts. Implement in phase order; each phase is independently testable.

---

## Current State Audit

### What exists today

**Backend — `src/web/api.rs` (lines 1207–1293)**
- `POST /api/chat` endpoint: receives JSON body, derives llama-server URL from active session (secure), forwards via `reqwest`, returns SSE stream via `warp::sse::reply`
- SSE chunking: strips `data: ` prefix, forwards raw JSON payloads
- Cancellation: `tx.is_closed()` check stops forwarding when client disconnects

**Backend — `src/web/ws.rs` (lines 1–116)**
- `GET /ws` WebSocket: server → client metrics push at 500ms intervals
- Incoming client messages are **dropped**: `while let Some(_msg) = ws_rx.next().await {}`
- Single WS connection per browser tab serves all metrics

**Frontend — `static/app.js` (lines 6070–6385)**
- `chatHistory: []` — flat array, single global history
- `sendChat()`: fetch POST to `/api/chat`, `getReader()` streaming, SSE parsing via `buf.split('\n')`
- `stopChat()`: `AbortController.abort()` cancels the fetch
- `clearChat()`: wipes `chatHistory` array and DOM
- Markdown rendering via `marked.parse()` (CDN)
- Thinking/reasoning blocks via `<details>` with `delta.reasoning_content`
- Message counter badge on sidebar
- No persistence, no tabs, no system prompt, no parameter UI

**Frontend — `static/index.html` (lines 552–575)**
```html
<div class="page" id="page-chat">
  <div class="chat-messages" id="chat-messages"></div>
  <div class="chat-input-row">
    <div class="chat-controls">
      <button class="btn-chat-control" id="btn-clear" …>
      <button class="btn-chat-control btn-chat-stop" id="btn-stop" …>
    </div>
    <textarea id="chat-input" rows="2" …></textarea>
    <button class="btn-chat-send" id="btn-send" …>
  </div>
</div>
```

**Frontend — `static/css/chat.css` (lines 1–170)**
- `.chat-messages` / `.log-panel`: `min-height: 360px`, `overflow: auto`
- `.chat-input-row`: grid `auto / minmax(0,1fr) / auto`, glassmorphism border
- `.msg`: `padding: 12px 14px; border-radius: var(--radius-base)`
- `.msg-user`: `margin-left: 12%; background: rgba(99, 102, 241, 0.14)`
- `.msg-assistant`: `margin-right: 12%; background: rgba(255, 255, 255, 0.055)`
- Send button: `var(--gradient-primary)` indigo, 44×44px
- No avatar, no timestamp, no copy/regenerate, no typing indicator

### Problems to fix

1. SSE fetch streaming works but is fragile — server disconnects leave fetch hanging with no retry
2. Single global `chatHistory` — switching sessions corrupts context
3. No chat tabs — one conversation at a time
4. No system prompt support
5. No UI for model parameters (temperature, top_p, etc.)
6. No persistence — refresh loses entire conversation history
7. No copy-to-clipboard on messages
8. No regenerate last response
9. No typing/thinking indicator
10. No empty-state welcome UX
11. Message styling is minimal — no avatars, timestamps, or role labels

---

## Architecture Decision: Keep SSE, Fix the Transport

The original plan proposed migrating chat to WebSocket. After auditing the code, **keep `POST /api/chat` with SSE** for chat transport. Reasons:

- The SSE path in `api.rs` is correct and already handles cancellation via `tx.is_closed()`
- The client-side fetch streaming (`getReader()`) works; the "broken SSE" noted in the original draft was a stale observation
- Migrating to WebSocket would require adding chat session multiplexing to `ws.rs`, which introduces complexity (chat messages interspersed with metrics, connection state management across tab switches)
- The single architectural addition needed is robust reconnect + error recovery on the client, not a protocol change

The WebSocket (`/ws`) remains metrics-only. Chat uses SSE POST. This stays.

---

## Phase 1 — Backend: Multi-Tab Chat & System Prompt Support

### 1.1 Add `system_prompt` to chat request passthrough

**File: `src/web/api.rs`**

The current endpoint blindly forwards the request body to llama-server. No changes needed to support system prompt — callers include it in the `messages` array as `{"role": "system", "content": "..."}`. The frontend is responsible for prepending it. No backend changes in this phase.

### 1.2 Add `/api/chat/abort` endpoint

**File: `src/web/api.rs`**

Add a no-op endpoint for forward compatibility. SSE abort already works via `AbortController` on the client, but add this so future WebSocket migration has a clean hook:

```rust
fn api_chat_abort(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "abort")
        .and(warp::post())
        .and_then(move || {
            let _state = state.clone();
            async move {
                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok": true})))
            }
        })
}
```

Register it in `src/web/mod.rs` alongside the other `api_*` filters.

### 1.3 Add chat history persistence endpoints

**File: `src/web/api.rs`**

Add two new endpoints for saving and loading named chat tabs:

```
GET  /api/chat/tabs          → returns Vec<ChatTab>
PUT  /api/chat/tabs          → saves Vec<ChatTab> (full replace)
```

Define in `api.rs`:

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,      // "user" | "assistant" | "system"
    pub content: String,
    pub timestamp_ms: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ChatTab {
    pub id: String,                    // UUID v4
    pub name: String,                  // User-editable label
    pub system_prompt: String,         // Empty string = no system prompt
    pub messages: Vec<ChatMessage>,
    pub model_params: ChatModelParams,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ChatModelParams {
    pub temperature: f32,   // default: 1.0
    pub top_p: f32,         // default: 0.95
    pub top_k: u32,         // default: 40
    pub min_p: f32,         // default: 0.01
    pub repeat_penalty: f32, // default: 1.0
    pub max_tokens: Option<u32>, // default: None (model decides)
}
```

Persist to `~/.config/llama-monitor/chat-tabs.json` using the same file I/O pattern as `presets.json`. Use `AppState` or a standalone file path helper — do not add a new field to `AppState`; load/save on request.

Implementation pattern (follow existing preset file I/O exactly):

```rust
fn chat_tabs_path() -> std::path::PathBuf {
    // same pattern as config_dir() + "presets.json"
    crate::config::config_dir().join("chat-tabs.json")
}

fn api_get_chat_tabs() -> impl Filter<…> {
    warp::path!("api" / "chat" / "tabs")
        .and(warp::get())
        .and_then(|| async move {
            let path = chat_tabs_path();
            if path.exists() {
                let raw = tokio::fs::read_to_string(&path).await…;
                let tabs: Vec<ChatTab> = serde_json::from_str(&raw)…;
                Ok(warp::reply::json(&tabs))
            } else {
                Ok(warp::reply::json(&Vec::<ChatTab>::new()))
            }
        })
}

fn api_put_chat_tabs() -> impl Filter<…> {
    warp::path!("api" / "chat" / "tabs")
        .and(warp::put())
        .and(warp::body::json::<Vec<ChatTab>>())
        .and_then(|tabs: Vec<ChatTab>| async move {
            let path = chat_tabs_path();
            let json = serde_json::to_string_pretty(&tabs)…;
            tokio::fs::write(&path, json).await…;
            Ok(warp::reply::json(&serde_json::json!({"ok": true})))
        })
}
```

---

## Phase 2 — Frontend: Multi-Tab Chat State

### 2.1 Replace global chat state with tab map

**File: `static/app.js`** — replace the chat state block (currently lines 6072–6075):

**Remove:**
```js
let chatHistory = [];
let chatBusy = false;
let chatAbortController = null;
```

**Replace with:**
```js
// Chat tab management
const CHAT_TABS_PERSIST_DEBOUNCE_MS = 1500;

let chatTabs = [];           // Array of ChatTab objects (mirrors server schema)
let activeChatTabId = null;  // Currently visible tab ID
let chatBusy = false;        // True while a generation is in flight
let chatAbortController = null;
let chatPersistTimer = null; // Debounced save timer

// Accessors
function activeChatTab() {
    return chatTabs.find(t => t.id === activeChatTabId) ?? null;
}

function activeChatHistory() {
    const tab = activeChatTab();
    if (!tab) return [];
    // Strip system message from visible history (it's stored separately)
    return tab.messages.filter(m => m.role !== 'system');
}
```

### 2.2 Tab initialization and persistence

```js
async function initChatTabs() {
    try {
        const resp = await fetch('/api/chat/tabs');
        const data = await resp.json();
        chatTabs = data.length ? data : [newChatTab('Chat 1')];
    } catch {
        chatTabs = [newChatTab('Chat 1')];
    }
    activeChatTabId = chatTabs[0].id;
    renderChatTabs();
    renderChatMessages();
}

function newChatTab(name = 'New Chat') {
    return {
        id: crypto.randomUUID(),
        name,
        system_prompt: '',
        messages: [],
        model_params: {
            temperature: 1.0,
            top_p: 0.95,
            top_k: 40,
            min_p: 0.01,
            repeat_penalty: 1.0,
            max_tokens: null,
        },
        created_at: Date.now(),
        updated_at: Date.now(),
    };
}

function scheduleChatPersist() {
    clearTimeout(chatPersistTimer);
    chatPersistTimer = setTimeout(persistChatTabs, CHAT_TABS_PERSIST_DEBOUNCE_MS);
}

async function persistChatTabs() {
    try {
        await fetch('/api/chat/tabs', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(chatTabs),
        });
    } catch { /* silent — next persist will retry */ }
}
```

Call `initChatTabs()` once during app initialization (alongside `initWebSocket()`).

### 2.3 Tab CRUD operations

```js
function addChatTab() {
    const tab = newChatTab(`Chat ${chatTabs.length + 1}`);
    chatTabs.push(tab);
    switchChatTab(tab.id);
    scheduleChatPersist();
}

function closeChatTab(id) {
    if (chatTabs.length === 1) return; // Always keep one tab
    chatTabs = chatTabs.filter(t => t.id !== id);
    if (activeChatTabId === id) {
        activeChatTabId = chatTabs[chatTabs.length - 1].id;
    }
    renderChatTabs();
    renderChatMessages();
    scheduleChatPersist();
}

function switchChatTab(id) {
    if (chatBusy) return; // Don't switch while generating
    activeChatTabId = id;
    renderChatTabs();
    renderChatMessages();
}

function renameChatTab(id, newName) {
    const tab = chatTabs.find(t => t.id === id);
    if (tab) {
        tab.name = newName.trim() || tab.name;
        renderChatTabs();
        scheduleChatPersist();
    }
}
```

### 2.4 Update `sendChat()` to use active tab

Replace the entire `sendChat()` function:

```js
async function sendChat() {
    if (chatBusy) return;
    const tab = activeChatTab();
    if (!tab) return;

    const input = document.getElementById('chat-input');
    const text = input.value.trim();
    if (!text) return;
    input.value = '';
    autoResizeChatInput();

    const userMsg = {
        role: 'user',
        content: text,
        timestamp_ms: Date.now(),
    };
    tab.messages.push(userMsg);
    tab.updated_at = Date.now();

    renderChatMessages();

    const params = tab.model_params;
    const messages = [];
    if (tab.system_prompt) {
        messages.push({ role: 'system', content: tab.system_prompt });
    }
    messages.push(...tab.messages.map(m => ({ role: m.role, content: m.content })));

    chatBusy = true;
    setChatBusyUI(true);
    chatAbortController = new AbortController();

    // Append thinking block placeholder and assistant message element
    let thinkEl = null;
    let thinkContent = '';
    const msgEl = appendAssistantPlaceholder();
    let msgContent = '';

    try {
        const chatResp = await fetch('/api/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            signal: chatAbortController.signal,
            body: JSON.stringify({
                messages,
                stream: true,
                temperature: params.temperature,
                top_p: params.top_p,
                top_k: params.top_k,
                min_p: params.min_p,
                repeat_penalty: params.repeat_penalty,
                ...(params.max_tokens ? { max_tokens: params.max_tokens } : {}),
            }),
        });

        if (!chatResp.ok) {
            throw new Error(`HTTP ${chatResp.status}`);
        }

        const reader = chatResp.body.getReader();
        const decoder = new TextDecoder();
        let buf = '';

        while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            buf += decoder.decode(value, { stream: true });

            const lines = buf.split('\n');
            buf = lines.pop() ?? '';

            for (const line of lines) {
                if (!line.startsWith('data: ')) continue;
                const payload = line.slice(6).trim();
                if (payload === '[DONE]') continue;
                try {
                    const obj = JSON.parse(payload);
                    const delta = obj.choices?.[0]?.delta;
                    if (!delta) continue;

                    const rc = delta.reasoning_content ?? '';
                    if (rc) {
                        thinkContent += rc;
                        if (!thinkEl) {
                            thinkEl = appendThinkingBlock(msgEl);
                        }
                        thinkEl.querySelector('.chat-thinking-body').textContent = thinkContent;
                    }

                    const c = delta.content ?? '';
                    if (c) {
                        if (document.getElementById('chat-typing').style.display !== 'none') {
                            document.getElementById('chat-typing').style.display = 'none';
                        }
                        msgContent += c;
                        msgEl.querySelector('.chat-msg-body').innerHTML = renderMd(msgContent);
                    }
                } catch { /* malformed chunk — skip */ }
            }
            chatScroll();
        }

    } catch (err) {
        const body = msgEl.querySelector('.chat-msg-body');
        if (err.name === 'AbortError') {
            body.innerHTML = msgContent
                ? renderMd(msgContent)
                : '<span class="chat-stopped">[stopped]</span>';
        } else {
            body.innerHTML = `<span class="chat-error">[error] ${escapeHtml(err.message)}</span>`;
        }
    }

    if (msgContent) {
        tab.messages.push({
            role: 'assistant',
            content: msgContent,
            timestamp_ms: Date.now(),
        });
        tab.updated_at = Date.now();
        scheduleChatPersist();
    } else if (!tab.messages.at(-1)?.content) {
        // Remove empty assistant stub if no content arrived
        tab.messages.pop();
    }

    finalizeAssistantMessage(msgEl, msgContent);
    setChatBusyUI(false);
    chatBusy = false;
    chatAbortController = null;
    updateChatTabBadge();
}
```

### 2.5 Update `clearChat()` and `stopChat()`

```js
function clearChat() {
    const tab = activeChatTab();
    if (!tab) return;
    tab.messages = [];
    tab.updated_at = Date.now();
    renderChatMessages();
    updateChatTabBadge();
    scheduleChatPersist();
}

function stopChat() {
    if (chatAbortController) {
        chatAbortController.abort();
        chatAbortController = null;
    }
    chatBusy = false;
    setChatBusyUI(false);
}
```

### 2.6 `setChatBusyUI()` helper

```js
function setChatBusyUI(busy) {
    document.getElementById('btn-send').disabled = busy;
    const stopBtn = document.getElementById('btn-stop');
    if (stopBtn) stopBtn.style.display = busy ? 'flex' : 'none';

    const input = document.getElementById('chat-input');
    if (input) input.disabled = busy;

    const typing = document.getElementById('chat-typing');
    if (typing) typing.style.display = busy ? 'flex' : 'none';
}
```

---

## Phase 3 — Frontend: Full Chat UI Rebuild

### 3.1 Replace `#page-chat` HTML entirely

**File: `static/index.html`** — replace lines 552–575:

```html
<!-- Page: Chat -->
<div class="page" id="page-chat">

  <!-- Tab bar -->
  <div class="chat-tab-bar" id="chat-tab-bar">
    <!-- Rendered by renderChatTabs() -->
    <button class="chat-tab-add" onclick="addChatTab()" title="New chat tab">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
        <path d="M12 5v14M5 12h14"/>
      </svg>
    </button>
  </div>

  <!-- Chat header: system prompt + model params -->
  <div class="chat-header" id="chat-header">
    <div class="chat-header-left">
      <button class="chat-header-btn" id="btn-system-prompt"
              onclick="toggleSystemPromptPanel()" title="System prompt">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/>
        </svg>
        <span id="system-prompt-indicator" class="chat-header-badge" style="display:none;">S</span>
      </button>
      <button class="chat-header-btn" id="btn-model-params"
              onclick="toggleModelParamsPanel()" title="Model parameters">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M4 6h16M8 12h8M11 18h2"/>
        </svg>
      </button>
    </div>
    <div class="chat-header-right">
      <button class="chat-header-btn" onclick="exportChatTab()" title="Export conversation">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3"/>
        </svg>
      </button>
    </div>
  </div>

  <!-- System prompt panel (collapsed by default) -->
  <div class="chat-system-panel" id="chat-system-panel" style="display:none;">
    <label class="chat-system-label">System Prompt</label>
    <textarea id="chat-system-input" rows="3"
              placeholder="You are a helpful assistant…"
              oninput="onSystemPromptChange()"></textarea>
  </div>

  <!-- Model params panel (collapsed by default) -->
  <div class="chat-params-panel" id="chat-params-panel" style="display:none;">
    <div class="chat-params-grid">
      <label>Temperature
        <input type="range" id="param-temperature" min="0" max="2" step="0.05"
               oninput="onParamChange('temperature', +this.value)">
        <span id="param-temperature-val">1.0</span>
      </label>
      <label>Top P
        <input type="range" id="param-top-p" min="0" max="1" step="0.01"
               oninput="onParamChange('top_p', +this.value)">
        <span id="param-top-p-val">0.95</span>
      </label>
      <label>Top K
        <input type="range" id="param-top-k" min="0" max="200" step="1"
               oninput="onParamChange('top_k', +this.value)">
        <span id="param-top-k-val">40</span>
      </label>
      <label>Min P
        <input type="range" id="param-min-p" min="0" max="0.5" step="0.005"
               oninput="onParamChange('min_p', +this.value)">
        <span id="param-min-p-val">0.01</span>
      </label>
      <label>Repeat Penalty
        <input type="range" id="param-repeat-penalty" min="1" max="2" step="0.01"
               oninput="onParamChange('repeat_penalty', +this.value)">
        <span id="param-repeat-penalty-val">1.0</span>
      </label>
      <label>Max Tokens
        <input type="number" id="param-max-tokens" min="0" step="64"
               placeholder="Model default"
               oninput="onParamChange('max_tokens', this.value ? +this.value : null)">
      </label>
    </div>
  </div>

  <!-- Message thread -->
  <div class="chat-messages" id="chat-messages">
    <!-- Empty state rendered by renderChatMessages() -->
  </div>

  <!-- Typing indicator (hidden by default) -->
  <div class="chat-typing" id="chat-typing" style="display:none;">
    <div class="chat-typing-avatar">AI</div>
    <div class="chat-typing-dots">
      <span></span><span></span><span></span>
    </div>
  </div>

  <!-- Input area -->
  <div class="chat-input-row" id="chat-input-row">
    <div class="chat-controls">
      <button class="btn-chat-control" id="btn-clear" onclick="clearChat()" title="Clear chat">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m5 0V4a2 2 0 012-2h2a2 2 0 012 2v2"/>
        </svg>
      </button>
      <button class="btn-chat-control btn-chat-stop" id="btn-stop"
              onclick="stopChat()" title="Stop generating" style="display:none;">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="6" y="6" width="12" height="12" rx="2"/>
        </svg>
      </button>
    </div>
    <div class="chat-textarea-wrap">
      <textarea id="chat-input" rows="1"
                placeholder="Send a message… (Enter to send, Shift+Enter for newline)"
                oninput="autoResizeChatInput()"></textarea>
    </div>
    <button class="btn-chat-send" id="btn-send" onclick="sendChat()">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z"/>
      </svg>
    </button>
  </div>

</div>
```

### 3.2 Render functions

**File: `static/app.js`**

#### `renderChatTabs()`

```js
function renderChatTabs() {
    const bar = document.getElementById('chat-tab-bar');
    const addBtn = bar.querySelector('.chat-tab-add');
    // Remove existing tab buttons (keep the add button)
    bar.querySelectorAll('.chat-tab').forEach(el => el.remove());

    for (const tab of chatTabs) {
        const el = document.createElement('div');
        el.className = 'chat-tab' + (tab.id === activeChatTabId ? ' active' : '');
        el.dataset.tabId = tab.id;
        el.innerHTML = `
          <span class="chat-tab-name" ondblclick="startRenameTab('${tab.id}')">${escapeHtml(tab.name)}</span>
          <span class="chat-tab-count">${tab.messages.filter(m => m.role !== 'system').length || ''}</span>
          ${chatTabs.length > 1
            ? `<button class="chat-tab-close" onclick="closeChatTab('${tab.id}')" title="Close tab">×</button>`
            : ''}
        `;
        el.addEventListener('click', e => {
            if (e.target.classList.contains('chat-tab-close')) return;
            if (e.target.classList.contains('chat-tab-name') && e.detail === 2) return;
            switchChatTab(tab.id);
        });
        bar.insertBefore(el, addBtn);
    }
}
```

#### `renderChatMessages()`

```js
function renderChatMessages() {
    const container = document.getElementById('chat-messages');
    const tab = activeChatTab();

    if (!tab || tab.messages.filter(m => m.role !== 'system').length === 0) {
        container.innerHTML = `
          <div class="chat-empty">
            <div class="chat-empty-icon">
              <svg width="40" height="40" viewBox="0 0 24 24" fill="none"
                   stroke="currentColor" stroke-width="1.5" opacity="0.3">
                <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"/>
              </svg>
            </div>
            <p class="chat-empty-title">Start a conversation</p>
            <p class="chat-empty-hint">Messages sent here go to the active llama-server session.</p>
          </div>`;
        return;
    }

    container.innerHTML = '';
    for (const msg of tab.messages) {
        if (msg.role === 'system') continue;
        container.appendChild(buildMessageElement(msg));
    }

    chatScroll();
}
```

#### `buildMessageElement(msg)`

```js
function buildMessageElement(msg) {
    const isUser = msg.role === 'user';
    const wrapper = document.createElement('div');
    wrapper.className = `chat-message chat-message-${msg.role}`;

    const ts = msg.timestamp_ms
        ? new Date(msg.timestamp_ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
        : '';

    wrapper.innerHTML = `
      <div class="chat-avatar">${isUser ? 'You' : 'AI'}</div>
      <div class="chat-bubble">
        <div class="chat-msg-body">${isUser ? escapeHtml(msg.content) : renderMd(msg.content)}</div>
        <div class="chat-msg-footer">
          <span class="chat-msg-time">${ts}</span>
          <div class="chat-msg-actions">
            <button class="chat-action-btn" onclick="copyMessageContent(this)" title="Copy">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none"
                   stroke="currentColor" stroke-width="2">
                <rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/>
              </svg>
            </button>
            ${!isUser ? `<button class="chat-action-btn" onclick="regenerateFromMessage(this)" title="Regenerate">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none"
                   stroke="currentColor" stroke-width="2">
                <path d="M1 4v6h6M23 20v-6h-6"/><path d="M20.5 9A9 9 0 005.6 5.6L1 10m22 4l-4.6 4.4A9 9 0 013.5 15"/>
              </svg>
            </button>` : ''}
          </div>
        </div>
      </div>`;

    return wrapper;
}
```

#### `appendAssistantPlaceholder()` and `appendThinkingBlock()`

```js
function appendAssistantPlaceholder() {
    const container = document.getElementById('chat-messages');
    const wrapper = document.createElement('div');
    wrapper.className = 'chat-message chat-message-assistant chat-message-streaming';
    wrapper.innerHTML = `
      <div class="chat-avatar">AI</div>
      <div class="chat-bubble">
        <div class="chat-msg-body"><span class="chat-cursor">▋</span></div>
        <div class="chat-msg-footer">
          <span class="chat-msg-time"></span>
          <div class="chat-msg-actions"></div>
        </div>
      </div>`;
    container.appendChild(wrapper);
    chatScroll();
    return wrapper;
}

function appendThinkingBlock(afterEl) {
    const details = document.createElement('details');
    details.className = 'chat-thinking';
    details.innerHTML = `
      <summary class="chat-thinking-summary">
        <span class="chat-thinking-label">Thinking…</span>
      </summary>
      <div class="chat-thinking-body"></div>`;
    afterEl.parentElement.insertBefore(details, afterEl);
    return details;
}

function finalizeAssistantMessage(el, content) {
    el.classList.remove('chat-message-streaming');
    const body = el.querySelector('.chat-msg-body');
    if (content) {
        body.innerHTML = renderMd(content);
    }
    const time = el.querySelector('.chat-msg-time');
    if (time) {
        time.textContent = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }
    const actions = el.querySelector('.chat-msg-actions');
    if (actions && content) {
        actions.innerHTML = `
          <button class="chat-action-btn" onclick="copyMessageContent(this)" title="Copy">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none"
                 stroke="currentColor" stroke-width="2">
              <rect x="9" y="9" width="13" height="13" rx="2"/>
              <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/>
            </svg>
          </button>
          <button class="chat-action-btn" onclick="regenerateFromMessage(this)" title="Regenerate">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none"
                 stroke="currentColor" stroke-width="2">
              <path d="M1 4v6h6M23 20v-6h-6"/>
              <path d="M20.5 9A9 9 0 005.6 5.6L1 10m22 4l-4.6 4.4A9 9 0 013.5 15"/>
            </svg>
          </button>`;
    }
}
```

#### Copy, Regenerate, Export

```js
function copyMessageContent(btn) {
    const body = btn.closest('.chat-bubble').querySelector('.chat-msg-body');
    navigator.clipboard.writeText(body.innerText).then(() => {
        btn.classList.add('chat-action-btn-copied');
        setTimeout(() => btn.classList.remove('chat-action-btn-copied'), 1500);
    });
}

function regenerateFromMessage(btn) {
    if (chatBusy) return;
    const tab = activeChatTab();
    if (!tab) return;

    const msgEl = btn.closest('.chat-message');
    const allMsgs = Array.from(document.querySelectorAll('#chat-messages .chat-message'));
    const idx = allMsgs.indexOf(msgEl);

    const firstVisibleIdx = tab.messages.findIndex(m => m.role !== 'system');
    const cutAt = firstVisibleIdx + idx;
    tab.messages = tab.messages.slice(0, cutAt);
    tab.updated_at = Date.now();

    renderChatMessages();
    scheduleChatPersist();

    const lastUser = [...tab.messages].reverse().find(m => m.role === 'user');
    if (lastUser) {
        tab.messages = tab.messages.filter(m => m !== lastUser);
        document.getElementById('chat-input').value = lastUser.content;
        sendChat();
    }
}

function exportChatTab() {
    const tab = activeChatTab();
    if (!tab) return;
    const md = tab.messages
        .filter(m => m.role !== 'system')
        .map(m => `**${m.role === 'user' ? 'You' : 'Assistant'}**\n\n${m.content}`)
        .join('\n\n---\n\n');
    const blob = new Blob([md], { type: 'text/markdown' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `${tab.name.replace(/[^a-z0-9]/gi, '-').toLowerCase()}.md`;
    a.click();
    URL.revokeObjectURL(a.href);
}
```

#### System prompt and model param panels

```js
function toggleSystemPromptPanel() {
    const panel = document.getElementById('chat-system-panel');
    const visible = panel.style.display !== 'none';
    panel.style.display = visible ? 'none' : 'block';
    if (!visible) {
        const tab = activeChatTab();
        document.getElementById('chat-system-input').value = tab?.system_prompt ?? '';
    }
}

function onSystemPromptChange() {
    const tab = activeChatTab();
    if (!tab) return;
    tab.system_prompt = document.getElementById('chat-system-input').value;
    tab.updated_at = Date.now();
    const indicator = document.getElementById('system-prompt-indicator');
    indicator.style.display = tab.system_prompt ? 'inline' : 'none';
    scheduleChatPersist();
}

function toggleModelParamsPanel() {
    const panel = document.getElementById('chat-params-panel');
    const visible = panel.style.display !== 'none';
    panel.style.display = visible ? 'none' : 'block';
    if (!visible) syncParamPanelToTab();
}

function syncParamPanelToTab() {
    const tab = activeChatTab();
    if (!tab) return;
    const p = tab.model_params;
    const set = (id, val, displayId) => {
        const el = document.getElementById(id);
        if (el) { el.value = val ?? ''; }
        const disp = document.getElementById(displayId);
        if (disp) disp.textContent = val ?? '';
    };
    set('param-temperature', p.temperature, 'param-temperature-val');
    set('param-top-p', p.top_p, 'param-top-p-val');
    set('param-top-k', p.top_k, 'param-top-k-val');
    set('param-min-p', p.min_p, 'param-min-p-val');
    set('param-repeat-penalty', p.repeat_penalty, 'param-repeat-penalty-val');
    const maxTok = document.getElementById('param-max-tokens');
    if (maxTok) maxTok.value = p.max_tokens ?? '';
}

function onParamChange(key, value) {
    const tab = activeChatTab();
    if (!tab) return;
    tab.model_params[key] = value;
    tab.updated_at = Date.now();
    const map = {
        temperature: 'param-temperature-val',
        top_p: 'param-top-p-val',
        top_k: 'param-top-k-val',
        min_p: 'param-min-p-val',
        repeat_penalty: 'param-repeat-penalty-val',
    };
    const dispId = map[key];
    if (dispId) {
        const el = document.getElementById(dispId);
        if (el) el.textContent = value ?? '';
    }
    scheduleChatPersist();
}
```

#### Inline tab rename

```js
function startRenameTab(id) {
    const tabEl = document.querySelector(`.chat-tab[data-tab-id="${id}"] .chat-tab-name`);
    if (!tabEl) return;
    const orig = tabEl.textContent;
    tabEl.contentEditable = 'true';
    tabEl.focus();
    const range = document.createRange();
    range.selectNodeContents(tabEl);
    window.getSelection().removeAllRanges();
    window.getSelection().addRange(range);
    const finish = () => {
        tabEl.contentEditable = 'false';
        renameChatTab(id, tabEl.textContent || orig);
    };
    tabEl.addEventListener('blur', finish, { once: true });
    tabEl.addEventListener('keydown', e => {
        if (e.key === 'Enter') { e.preventDefault(); tabEl.blur(); }
        if (e.key === 'Escape') { tabEl.textContent = orig; tabEl.blur(); }
    }, { once: true });
}
```

#### Badge and auto-resize

```js
function updateChatTabBadge() {
    const tab = activeChatTab();
    const count = tab ? tab.messages.filter(m => m.role !== 'system').length : 0;
    const badge = document.getElementById('sidebar-badge-chat');
    if (badge) badge.textContent = count > 0 ? count : '';
}

function autoResizeChatInput() {
    const ta = document.getElementById('chat-input');
    if (!ta) return;
    ta.style.height = 'auto';
    ta.style.height = Math.min(ta.scrollHeight, 200) + 'px';
}
```

Update the keydown listener:
```js
document.getElementById('chat-input').addEventListener('keydown', e => {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendChat(); }
});
```

---

## Phase 4 — CSS: Premium Modern UI

**File: `static/css/chat.css`** — replace the entire chat section (currently lines 1–170) with the following. The existing `.msg`, `.msg-user`, `.msg-assistant` rules will be removed and replaced with the new premium UI styles.

### Tab bar

```css
/* ── Chat Tab Bar ───────────────────────────────────── */
.chat-tab-bar {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 0 var(--gap-md) 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  overflow-x: auto;
  scrollbar-width: none;
  flex-shrink: 0;
}
.chat-tab-bar::-webkit-scrollbar { display: none; }

.chat-tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px 7px;
  border-radius: 8px 8px 0 0;
  border: 1px solid transparent;
  border-bottom: none;
  cursor: pointer;
  white-space: nowrap;
  font-size: var(--text-sm);
  color: var(--text-muted);
  background: transparent;
  transition: color 0.15s, background 0.15s;
  user-select: none;
  position: relative;
  bottom: -1px;
}
.chat-tab:hover {
  color: var(--text-primary);
  background: rgba(255, 255, 255, 0.04);
}
.chat-tab.active {
  color: var(--text-primary);
  background: var(--color-bg);
  border-color: rgba(255, 255, 255, 0.06);
  border-bottom-color: var(--color-bg);
}
.chat-tab-name {
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.chat-tab-name[contenteditable="true"] {
  outline: 1px solid var(--color-primary);
  border-radius: 3px;
  padding: 0 2px;
  min-width: 40px;
}
.chat-tab-count {
  font-size: 10px;
  color: var(--text-muted);
  opacity: 0.6;
}
.chat-tab-close {
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: none;
  color: var(--text-muted);
  cursor: pointer;
  border-radius: 4px;
  font-size: 14px;
  line-height: 1;
  opacity: 0;
  transition: opacity 0.15s, background 0.15s;
  padding: 0;
}
.chat-tab:hover .chat-tab-close { opacity: 1; }
.chat-tab-close:hover { background: rgba(244, 63, 94, 0.15); color: #f43f5e; }

.chat-tab-add {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: 1px solid rgba(255, 255, 255, 0.06);
  background: rgba(255, 255, 255, 0.03);
  border-radius: 6px;
  cursor: pointer;
  color: var(--text-muted);
  transition: all 0.15s;
  flex-shrink: 0;
  margin-left: 4px;
}
.chat-tab-add:hover {
  background: rgba(255, 255, 255, 0.07);
  color: var(--text-primary);
}
```

### Chat header

```css
/* ── Chat Header ────────────────────────────────────── */
.chat-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px var(--gap-md);
  border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  flex-shrink: 0;
}
.chat-header-left,
.chat-header-right {
  display: flex;
  align-items: center;
  gap: 4px;
}
.chat-header-btn {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.03);
  color: var(--text-muted);
  cursor: pointer;
  font-size: var(--text-sm);
  transition: all 0.15s;
  position: relative;
}
.chat-header-btn:hover {
  background: rgba(255, 255, 255, 0.07);
  color: var(--text-primary);
}
.chat-header-badge {
  width: 16px;
  height: 16px;
  background: var(--color-primary);
  color: white;
  border-radius: 4px;
  font-size: 9px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
}
```

### System prompt panel

```css
/* ── System Prompt Panel ────────────────────────────── */
.chat-system-panel {
  padding: var(--gap-sm) var(--gap-md);
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  background: rgba(99, 102, 241, 0.04);
  flex-shrink: 0;
}
.chat-system-label {
  display: block;
  font-size: var(--text-sm);
  color: var(--text-muted);
  margin-bottom: 6px;
  font-weight: 500;
}
.chat-system-panel textarea {
  width: 100%;
  min-height: 64px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  padding: 8px 12px;
  font-family: var(--font-sans);
  font-size: var(--text-sm);
  background: rgba(255, 255, 255, 0.03);
  color: var(--text-primary);
  resize: vertical;
  line-height: 1.5;
  transition: border-color 0.2s;
}
.chat-system-panel textarea:focus {
  outline: none;
  border-color: rgba(99, 102, 241, 0.4);
}
```

### Model params panel

```css
/* ── Model Params Panel ─────────────────────────────── */
.chat-params-panel {
  padding: var(--gap-sm) var(--gap-md);
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  background: rgba(255, 255, 255, 0.02);
  flex-shrink: 0;
}
.chat-params-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: var(--gap-sm);
}
.chat-params-grid label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: var(--text-sm);
  color: var(--text-muted);
}
.chat-params-grid input[type="range"] {
  width: 100%;
  accent-color: var(--color-primary);
}
.chat-params-grid input[type="number"] {
  padding: 4px 8px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.03);
  color: var(--text-primary);
  font-size: var(--text-sm);
}
.chat-params-grid span {
  font-size: 11px;
  color: var(--color-primary);
  font-variant-numeric: tabular-nums;
}
```

### Message thread

```css
/* ── Chat Messages ──────────────────────────────────── */
.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: var(--gap-md) var(--gap-lg);
  display: flex;
  flex-direction: column;
  gap: var(--gap-md);
  scroll-behavior: smooth;
}
.chat-messages::-webkit-scrollbar { width: 4px; }
.chat-messages::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.08);
  border-radius: 2px;
}

.chat-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 60px 0;
  color: var(--text-muted);
  text-align: center;
}
.chat-empty-icon { opacity: 0.4; }
.chat-empty-title {
  font-size: var(--text-lg);
  font-weight: 600;
  color: var(--text-secondary);
}
.chat-empty-hint { font-size: var(--text-sm); }

.chat-message {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  max-width: 82%;
  animation: chat-msg-in 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}
@keyframes chat-msg-in {
  from { opacity: 0; transform: translateY(6px); }
  to   { opacity: 1; transform: translateY(0); }
}
.chat-message-user {
  align-self: flex-end;
  flex-direction: row-reverse;
}
.chat-message-assistant {
  align-self: flex-start;
}

.chat-avatar {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  font-weight: 700;
  flex-shrink: 0;
  letter-spacing: 0.02em;
}
.chat-message-user .chat-avatar {
  background: rgba(99, 102, 241, 0.25);
  color: var(--color-primary);
}
.chat-message-assistant .chat-avatar {
  background: rgba(16, 185, 129, 0.18);
  color: #10b981;
}

.chat-bubble {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}
.chat-msg-body {
  padding: 10px 14px;
  border-radius: 14px;
  font-size: var(--text-base);
  line-height: 1.6;
  word-break: break-word;
}
.chat-message-user .chat-msg-body {
  background: rgba(99, 102, 241, 0.18);
  border-bottom-right-radius: 4px;
  color: var(--text-primary);
}
.chat-message-assistant .chat-msg-body {
  background: rgba(255, 255, 255, 0.05);
  border-bottom-left-radius: 4px;
  color: var(--text-primary);
  border: 1px solid rgba(255, 255, 255, 0.06);
}
.chat-message-streaming .chat-msg-body {
  border-color: rgba(99, 102, 241, 0.2);
}
.chat-cursor {
  display: inline-block;
  animation: chat-cursor-blink 1s step-end infinite;
  color: var(--color-primary);
}
@keyframes chat-cursor-blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0; }
}

.chat-msg-footer {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 4px;
  opacity: 0;
  transition: opacity 0.15s;
}
.chat-message:hover .chat-msg-footer { opacity: 1; }
.chat-message-user .chat-msg-footer { flex-direction: row-reverse; }
.chat-msg-time {
  font-size: 11px;
  color: var(--text-muted);
  opacity: 0.6;
}
.chat-msg-actions { display: flex; gap: 4px; }
.chat-action-btn {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid rgba(255, 255, 255, 0.06);
  background: rgba(255, 255, 255, 0.04);
  border-radius: 6px;
  cursor: pointer;
  color: var(--text-muted);
  transition: all 0.15s;
  padding: 0;
}
.chat-action-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: var(--text-primary);
}
.chat-action-btn-copied {
  background: rgba(16, 185, 129, 0.15) !important;
  color: #10b981 !important;
  border-color: rgba(16, 185, 129, 0.2) !important;
}

.chat-thinking {
  margin-bottom: var(--gap-sm);
  max-width: 82%;
  align-self: flex-start;
}
.chat-thinking-summary {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
  cursor: pointer;
  list-style: none;
  font-size: var(--text-sm);
  color: var(--text-muted);
  background: rgba(255, 255, 255, 0.03);
  user-select: none;
  transition: background 0.15s;
}
.chat-thinking-summary:hover { background: rgba(255, 255, 255, 0.06); }
.chat-thinking-label { font-style: italic; }
.chat-thinking-body {
  margin-top: 6px;
  padding: 10px 14px;
  border-left: 2px solid rgba(255, 255, 255, 0.06);
  font-size: var(--text-sm);
  color: var(--text-muted);
  white-space: pre-wrap;
  line-height: 1.6;
  font-family: var(--font-mono);
}

.chat-stopped { color: var(--text-muted); font-style: italic; }
.chat-error   { color: var(--color-error); }

.chat-typing {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 var(--gap-lg);
  margin-bottom: 4px;
}
.chat-typing-avatar {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: rgba(16, 185, 129, 0.18);
  color: #10b981;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  font-weight: 700;
  flex-shrink: 0;
}
.chat-typing-dots {
  display: flex;
  gap: 5px;
  padding: 10px 14px;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 14px;
  border-bottom-left-radius: 4px;
  border: 1px solid rgba(255, 255, 255, 0.06);
}
.chat-typing-dots span {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--text-muted);
  animation: chat-typing-bounce 1.2s infinite;
}
.chat-typing-dots span:nth-child(2) { animation-delay: 0.15s; }
.chat-typing-dots span:nth-child(3) { animation-delay: 0.3s; }
@keyframes chat-typing-bounce {
  0%, 80%, 100% { transform: translateY(0); opacity: 0.4; }
  40%           { transform: translateY(-5px); opacity: 1; }
}
```

### Input area

```css
/* ── Chat Input ─────────────────────────────────────── */
.chat-input-row {
  display: grid;
  grid-template-columns: auto 1fr auto;
  gap: var(--gap-sm);
  padding: var(--gap-sm) var(--gap-md);
  background: rgba(255, 255, 255, 0.02);
  border-top: 1px solid rgba(255, 255, 255, 0.05);
  flex-shrink: 0;
}
.chat-textarea-wrap {
  display: flex;
  align-items: flex-end;
}
.chat-input-row textarea {
  width: 100%;
  min-height: 42px;
  max-height: 200px;
  resize: none;
  overflow-y: auto;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 10px;
  padding: 10px 14px;
  font-family: var(--font-sans);
  font-size: var(--text-base);
  line-height: 1.5;
  background: rgba(255, 255, 255, 0.03);
  color: var(--text-primary);
  transition: border-color 0.2s, box-shadow 0.2s;
  scrollbar-width: thin;
}
.chat-input-row textarea:focus {
  outline: none;
  border-color: rgba(99, 102, 241, 0.35);
  box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.08);
}
.chat-input-row textarea:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.chat-controls {
  display: flex;
  flex-direction: column;
  gap: 6px;
  justify-content: flex-end;
}
.btn-chat-control {
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: rgba(255, 255, 255, 0.04);
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  padding: 0;
}
.btn-chat-control:hover {
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-primary);
  border-color: rgba(255, 255, 255, 0.15);
  transform: translateY(-1px);
}
.btn-chat-control:active { transform: translateY(0); }
.btn-chat-stop {
  color: rgba(244, 63, 94, 0.8);
  border-color: rgba(244, 63, 94, 0.15);
}
.btn-chat-stop:hover {
  color: #f43f5e;
  background: rgba(244, 63, 94, 0.08);
  border-color: rgba(244, 63, 94, 0.3);
}
.btn-chat-send {
  width: 42px;
  height: 42px;
  align-self: flex-end;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 12px;
  border: none;
  background: var(--gradient-primary);
  color: white;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  padding: 0;
  box-shadow: 0 2px 12px rgba(99, 102, 241, 0.3);
}
.btn-chat-send:hover:not(:disabled) {
  transform: translateY(-2px);
  box-shadow: 0 6px 20px rgba(99, 102, 241, 0.45);
}
.btn-chat-send:active:not(:disabled) { transform: translateY(0); }
.btn-chat-send:disabled {
  opacity: 0.45;
  cursor: not-allowed;
  box-shadow: none;
}

[data-theme="light"] .chat-message-user .chat-msg-body {
  background: rgba(99, 102, 241, 0.12);
}
[data-theme="light"] .chat-message-assistant .chat-msg-body {
  background: rgba(0, 0, 0, 0.04);
  border-color: rgba(0, 0, 0, 0.06);
}
[data-theme="light"] .chat-tab.active { background: var(--color-bg); }
[data-theme="light"] .chat-tab-bar {
  border-bottom-color: rgba(0, 0, 0, 0.08);
}

@media (max-width: 600px) {
  .chat-message { max-width: 96%; }
  .chat-messages { padding: var(--gap-sm); }
  .chat-params-grid { grid-template-columns: 1fr 1fr; }
}
```

### Markdown prose reset inside assistant bubbles

```css
.chat-message-assistant .chat-msg-body p  { margin: 0 0 0.6em; }
.chat-message-assistant .chat-msg-body p:last-child { margin-bottom: 0; }
.chat-message-assistant .chat-msg-body pre {
  background: rgba(0, 0, 0, 0.3);
  border-radius: 8px;
  padding: 10px 14px;
  overflow-x: auto;
  font-size: var(--text-sm);
  margin: 0.5em 0;
}
.chat-message-assistant .chat-msg-body code:not(pre code) {
  background: rgba(0, 0, 0, 0.25);
  border-radius: 4px;
  padding: 1px 5px;
  font-size: 0.9em;
}
.chat-message-assistant .chat-msg-body ul,
.chat-message-assistant .chat-msg-body ol {
  padding-left: 1.4em;
  margin: 0.4em 0;
}
.chat-message-assistant .chat-msg-body blockquote {
  border-left: 3px solid rgba(99, 102, 241, 0.4);
  padding-left: 10px;
  margin-left: 0;
  color: var(--text-secondary);
}
.chat-message-assistant .chat-msg-body table {
  border-collapse: collapse;
  width: 100%;
  font-size: var(--text-sm);
}
.chat-message-assistant .chat-msg-body th,
.chat-message-assistant .chat-msg-body td {
  border: 1px solid rgba(255, 255, 255, 0.08);
  padding: 5px 10px;
  text-align: left;
}
.chat-message-assistant .chat-msg-body th {
  background: rgba(255, 255, 255, 0.04);
}
```

---

## Phase 5 — Layout: Make `#page-chat` a Proper Flex Column

**File: `static/css/layout.css`** — find the `.page` rule and ensure it has:

```css
.page {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
```

The `#page-chat` page must not scroll as a whole — the `.chat-messages` div scrolls internally. Verify `.content-area` sets a definite height so flex children can use `flex: 1` correctly.

---

## Phase 6 — Helpers and Utility Functions

**File: `static/app.js`** — add near the top of the chat section:

```js
function escapeHtml(str) {
    return str
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}
```

If `escapeHtml` already exists elsewhere in the file, do not duplicate it.

---

## Phase 7 — Initialization Wiring

**File: `static/app.js`** — in the main initialization block, add:

```js
initChatTabs();
```

Also call `autoResizeChatInput()` once on startup:

```js
document.addEventListener('DOMContentLoaded', () => {
    autoResizeChatInput();
});
```

---

## Phase 8 — Remove Dead Code

**File: `static/app.js`** — remove:

1. Old `appendMsg(role, text)` function — replaced by `buildMessageElement()` and `appendAssistantPlaceholder()`
2. Any reference to `chatHistory` as a global array
3. The `activeSessionPort` variable manipulation inside `sendChat()`
4. The GET `/api/sessions/active` fetch inside `sendChat()` — the server already resolves the active session

**File: `static/css/chat.css`** — remove:

- `.msg` rule (line ~150)
- `.msg-user` rule (line ~156)
- `.msg-assistant` rule (line ~161)

These are fully replaced by the `.chat-message-*` hierarchy.

---

## File Change Summary

| File | Type of change |
|------|---------------|
| `src/web/api.rs` | Add `api_chat_abort()`, `api_get_chat_tabs()`, `api_put_chat_tabs()`, `ChatTab` / `ChatMessage` / `ChatModelParams` structs, `chat_tabs_path()` |
| `src/web/mod.rs` | Register three new API filters |
| `static/index.html` | Replace `#page-chat` block entirely (lines 552–575) |
| `static/app.js` | Replace all chat state and functions; add tab management, persistence, render helpers, UI helpers |
| `static/css/chat.css` | Replace `.chat-messages` through `.msg-assistant` + add all new component rules |

---

## Behavioral Contracts

- **Tab switching** is blocked while `chatBusy = true`.
- **Close tab** is disabled when only one tab exists.
- **Chat history** is keyed to the tab object. Session switches do not clear tab histories.
- **System prompt** is prepended to every `messages` array sent to `/api/chat` but never shown in the visible thread.
- **Persistence** is debounced at 1.5s.
- **Abort** cancels the fetch. Partial content that arrived before abort is preserved as a valid assistant message.
- **Regenerate** removes the target assistant message and all subsequent messages, then re-sends the preceding user turn.
- **Export** downloads a `.md` file with user/assistant turns separated by `---`. System prompt is excluded.
- **Tab rename** via double-click triggers `contentEditable`. Enter or blur commits. Escape reverts.

---

## Implementation Progress

- [x] **Phase 1** — Backend: Add `/api/chat/abort` and persistence endpoints
- [x] **Phase 2** — Frontend: Replace global chat state with tab management
- [x] **Phase 3** — Frontend: Full UI rebuild with tabs, system prompt, model params
- [x] **Phase 4** — CSS: Premium modern UI styling in `chat.css`
- [x] **Phase 5** — Layout: Ensure proper flex structure in `layout.css`
- [x] **Phase 6** — Helpers: Add utility functions
- [x] **Phase 7** — Initialization: Wire everything up

## Testing Checklist

**Test environment:** Live llama-server at `http://192.168.2.16:8001`

- [ ] App loads with one default chat tab when no `chat-tabs.json` exists
- [ ] Sending a message streams tokens in real-time (cursor blink → token by token)
- [ ] Typing indicator appears when busy, disappears on first token
- [ ] Stop button cancels generation; partial content is preserved
- [ ] Clear chat empties the active tab's history and renders empty state
- [ ] Adding a second tab creates an independent history
- [ ] Switching between tabs shows each tab's own message history
- [ ] Closing a tab removes it; active tab shifts to the last remaining
- [ ] Renaming a tab via double-click persists after a page reload
- [ ] System prompt is sent to the model but not visible in the UI thread
- [ ] Model parameter sliders update values live; values are used on next send
- [ ] Copy button copies the message text to clipboard; button flashes green
- [ ] Regenerate removes the assistant message and re-sends the user turn
- [ ] Export downloads a markdown file with the full visible conversation
- [ ] Chat history persists across page reload (fetch from `/api/chat/tabs`)
- [ ] Light theme: message bubbles render with correct contrast
- [ ] Mobile (≤600px): messages fill 96% width; params panel is 2-column
- [ ] Markdown renders inside assistant bubbles: bold, code, lists, tables
- [ ] Thinking/reasoning blocks appear as collapsible `<details>` above the message
- [ ] Badge on sidebar chat tab reflects the correct message count
