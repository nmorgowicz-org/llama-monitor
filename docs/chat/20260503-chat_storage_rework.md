# Chat Storage Rework

**Date:** 2026-05-03  
**Status:** Option A targeted for implementation; Option B deferred

---

## Problem Statement

All chat tabs are stored in a single flat JSON file (`chat_tabs.json` in the config dir). Every PUT to `/api/chat/tabs` serializes **all** tabs and overwrites the entire file. This causes:

- **Write amplification**: One new message in one tab rewrites every message in every tab
- **Load-time cost**: All tab history is loaded into memory and sent over the wire on startup, even for tabs not being viewed
- **Risk surface**: A crash mid-write corrupts every tab simultaneously
- **Scalability ceiling**: Long conversations hit noticeable latency when the file grows beyond ~2 MB

---

## Option A — Per-Tab JSON Sharding

**Implement now. No new dependencies.**

### Design

- One JSON file per tab: `chat-{uuid}.json` (full tab object including messages)
- One index file: `chat-index.json` containing only the tab order and lightweight metadata (id, name, created\_at, updated\_at, message count) — **no messages**
- On startup: load `chat-index.json` to build tab list; lazy-load `chat-{uuid}.json` only when the tab is first activated or needs its messages

### File Layout

```
~/.config/llama-monitor/           (or platform config dir)
  chat-index.json                  ← tab list, metadata, order
  chat-12345678-xxxx.json          ← full tab including messages
  chat-87654321-yyyy.json
  ...
```

`chat-index.json` schema:
```json
{
  "version": 1,
  "tab_order": ["12345678-xxxx", "87654321-yyyy"],
  "tabs": {
    "12345678-xxxx": {
      "id": "12345678-xxxx",
      "name": "Chat 1",
      "created_at": 1746300000000,
      "updated_at": 1746312345678,
      "message_count": 42,
      "last_ctx_pct": 23.4
    }
  }
}
```

`chat-{uuid}.json` schema: identical to the current per-tab object in `chat_tabs.json`, including `messages[]`, `model_params`, `system_prompt`, etc.

### Rust — Backend Changes

**File:** `src/web/mod.rs` (or `src/chat_storage.rs` — create new module)

#### New module: `src/chat_storage.rs`

```rust
use std::path::{Path, PathBuf};
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub struct ChatStorage {
    dir: PathBuf,
}

impl ChatStorage {
    pub fn new(config_dir: &Path) -> Self {
        Self { dir: config_dir.to_path_buf() }
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join("chat-index.json")
    }

    fn tab_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("chat-{}.json", id))
    }

    /// Load tab index (metadata only, no messages)
    pub fn load_index(&self) -> Result<ChatIndex> { ... }

    /// Save index (call after tab order/metadata changes)
    pub fn save_index(&self, index: &ChatIndex) -> Result<()> { ... }

    /// Load a single tab including all messages
    pub fn load_tab(&self, id: &str) -> Result<ChatTab> { ... }

    /// Save a single tab (messages included)
    pub fn save_tab(&self, tab: &ChatTab) -> Result<()> { ... }

    /// Delete tab file (called on tab close)
    pub fn delete_tab(&self, id: &str) -> Result<()> { ... }

    /// One-time migration: read old chat_tabs.json, write sharded files, rename old file
    pub fn migrate_from_legacy(&self) -> Result<()> { ... }
}
```

**Migration logic** (run once on startup before serving any requests):

```rust
pub fn migrate_from_legacy(&self) -> Result<()> {
    let legacy = self.dir.join("chat_tabs.json");
    if !legacy.exists() { return Ok(()); }

    let data = std::fs::read_to_string(&legacy)?;
    let tabs: Vec<ChatTab> = serde_json::from_str(&data)?;

    let mut index = ChatIndex { version: 1, tab_order: vec![], tabs: HashMap::new() };
    for tab in &tabs {
        self.save_tab(tab)?;
        index.tab_order.push(tab.id.clone());
        index.tabs.insert(tab.id.clone(), TabMeta::from(tab));
    }
    self.save_index(&index)?;
    std::fs::rename(&legacy, self.dir.join("chat_tabs.json.bak"))?;
    Ok(())
}
```

#### API endpoint changes (`src/web/mod.rs`)

Current endpoints:
- `GET /api/chat/tabs` → returns all tabs with messages (full array)
- `PUT /api/chat/tabs` → receives full array, overwrites file

**New endpoints to implement** (keep old ones for backward-compat during transition, then remove):

```
GET  /api/chat/tabs          → returns index only (metadata, no messages)
GET  /api/chat/tabs/:id      → returns single tab with messages
PUT  /api/chat/tabs/:id      → saves single tab (partial update OK)
POST /api/chat/tabs          → creates new tab, returns it with id
DELETE /api/chat/tabs/:id    → deletes tab file, removes from index
PATCH /api/chat/tabs/order   → body: { tab_order: ["id1", "id2"] } — reorders index
```

For backward compatibility during the JS migration, the old `GET /api/chat/tabs` can return the full array by loading all tab files — but should be deprecated once the JS is updated.

### JavaScript — Frontend Changes

**File:** `static/js/features/chat-state.js`

#### Startup: `initChatTabs()`

Currently fetches all tabs at once. New behavior:
1. `GET /api/chat/tabs` → receives index (metadata only)
2. Build `chat.tabs` array from index metadata (no messages yet)
3. Set `chat.activeTabId = index.tab_order[0]`
4. `GET /api/chat/tabs/:activeId` → load active tab's messages
5. Render

```js
export async function initChatTabs() {
    const indexResp = await fetch('/api/chat/tabs');
    const index = await indexResp.json();          // { tab_order, tabs: { id: meta } }

    chat.tabs = index.tab_order.map(id => ({
        ...index.tabs[id],
        messages: null,    // null = not yet loaded
        _loaded: false,
    }));
    chat.activeTabId = chat.tabs[0]?.id ?? null;

    if (chat.activeTabId) {
        await loadTabMessages(chat.activeTabId);
    }
    // ... render, bindings, etc.
}
```

#### Lazy loading: `loadTabMessages(id)`

```js
export async function loadTabMessages(id) {
    const tab = chat.tabs.find(t => t.id === id);
    if (!tab || tab._loaded) return;
    const resp = await fetch(`/api/chat/tabs/${id}`);
    const full = await resp.json();
    Object.assign(tab, full);
    tab._loaded = true;
}
```

Call `loadTabMessages` in `switchChatTab` before rendering messages.

#### Persistence: `scheduleChatPersist(tab)`

Change from persisting all tabs to persisting only the dirty tab:

```js
export function scheduleChatPersist(tab) {
    tab._dirty = true;
    clearTimeout(tab._persistTimer);
    tab._persistTimer = setTimeout(() => persistTab(tab), 500);
}

async function persistTab(tab) {
    if (!tab._dirty) return;
    tab._dirty = false;
    await fetch(`/api/chat/tabs/${tab.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(normalizeTabForSave(tab)),
    });
}
```

Update all callers of `scheduleChatPersist()` to pass the tab: `scheduleChatPersist(tab)`.

#### Tab creation / deletion

- `addChatTab()`: POST `/api/chat/tabs` with initial tab object, use returned id
- `closeChatTab(id)`: DELETE `/api/chat/tabs/:id`

#### Reorder persistence

Drag-to-reorder already reorders `chat.tabs` in memory. Add a PATCH call on drop:

```js
async function persistTabOrder() {
    await fetch('/api/chat/tabs/order', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tab_order: chat.tabs.map(t => t.id) }),
    });
}
```

### Migration Path

1. On first server start after upgrade:
   - Rust `migrate_from_legacy()` runs, shards `chat_tabs.json` into per-tab files, renames old file to `.bak`
   - New endpoints are live immediately
2. Old `GET /api/chat/tabs` (full array) is kept as a fallback for 1 release, then removed
3. No user action required

### Atomic Writes

To prevent partial writes corrupting a tab file, write to a `.tmp` file then rename:

```rust
pub fn save_tab(&self, tab: &ChatTab) -> Result<()> {
    let path = self.tab_path(&tab.id);
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_string_pretty(tab)?;
    std::fs::write(&tmp, &data)?;
    std::fs::rename(&tmp, &path)?;  // atomic on same filesystem
    Ok(())
}
```

---

## Option B — SQLite Migration

**Deferred. Implement after Option A is stable.**

### Why SQLite

- Row-level writes: one INSERT/UPDATE per message, not a full file rewrite
- Indexed queries: full-text search, date range, role filter — all O(log n)
- Single file: easier backup, no index/shard sync to maintain
- Lazy loading is free: `SELECT * FROM messages WHERE tab_id = ? ORDER BY rowid`

### Schema

```sql
CREATE TABLE tabs (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    system_prompt TEXT,
    ai_name     TEXT,
    user_name   TEXT,
    explicit_mode INTEGER NOT NULL DEFAULT 0,
    auto_compact  INTEGER NOT NULL DEFAULT 1,
    compact_mode  TEXT NOT NULL DEFAULT 'percent',
    compact_threshold REAL NOT NULL DEFAULT 0.8,
    auto_compact_summarize INTEGER NOT NULL DEFAULT 0,
    model_params  TEXT NOT NULL DEFAULT '{}',   -- JSON blob
    tab_order   INTEGER NOT NULL DEFAULT 0,
    last_ctx_pct REAL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE messages (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    tab_id      TEXT NOT NULL REFERENCES tabs(id) ON DELETE CASCADE,
    role        TEXT NOT NULL CHECK(role IN ('user','assistant','system')),
    content     TEXT NOT NULL,
    timestamp_ms INTEGER,
    input_tokens INTEGER,
    output_tokens INTEGER,
    compaction_marker INTEGER NOT NULL DEFAULT 0,
    seq         INTEGER NOT NULL   -- insertion order within tab
);

CREATE INDEX idx_messages_tab ON messages(tab_id, seq);
CREATE VIRTUAL TABLE messages_fts USING fts5(
    content,
    content='messages',
    content_rowid='id'
);
```

### Rust Dependencies

Add to `Cargo.toml`:
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
```

`bundled` compiles SQLite from source — no system library dependency, works cross-platform.

### Key Rust Operations

```rust
// Load tab list (no messages)
fn list_tabs(conn: &Connection) -> Result<Vec<TabMeta>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, created_at, updated_at FROM tabs ORDER BY tab_order"
    )?;
    // ...
}

// Load messages for a tab (lazy)
fn load_tab_messages(conn: &Connection, tab_id: &str) -> Result<Vec<Message>> {
    let mut stmt = conn.prepare(
        "SELECT role, content, timestamp_ms, input_tokens, output_tokens, compaction_marker
         FROM messages WHERE tab_id = ?1 ORDER BY seq"
    )?;
    // ...
}

// Append a single message (called after each response)
fn append_message(conn: &Connection, tab_id: &str, msg: &Message) -> Result<()> {
    conn.execute(
        "INSERT INTO messages (tab_id, role, content, timestamp_ms, input_tokens, output_tokens, seq)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, (SELECT COALESCE(MAX(seq)+1, 0) FROM messages WHERE tab_id = ?1))",
        params![tab_id, msg.role, msg.content, msg.timestamp_ms, msg.input_tokens, msg.output_tokens]
    )?;
    Ok(())
}

// Full-text search
fn search_messages(conn: &Connection, query: &str) -> Result<Vec<SearchResult>> {
    let mut stmt = conn.prepare(
        "SELECT m.tab_id, m.id, m.role, snippet(messages_fts, 0, '<b>', '</b>', '…', 20) as snippet
         FROM messages_fts
         JOIN messages m ON m.id = messages_fts.rowid
         WHERE messages_fts MATCH ?1
         ORDER BY rank"
    )?;
    // ...
}
```

### Migration from Option A (or legacy JSON)

Run once on startup:

```rust
fn migrate_from_json(conn: &mut Connection, storage: &ChatStorage) -> Result<()> {
    let index = storage.load_index()?;
    let tx = conn.transaction()?;
    for (order, id) in index.tab_order.iter().enumerate() {
        let tab = storage.load_tab(id)?;
        // INSERT into tabs ...
        for (seq, msg) in tab.messages.iter().enumerate() {
            // INSERT into messages ...
        }
    }
    tx.commit()?;
    // rename JSON files to .bak
    Ok(())
}
```

### New API Endpoints for Option B

Same as Option A, but backed by SQL queries instead of file I/O.

Add search endpoint:
```
GET /api/chat/search?q=<query>&tab=<id|all>
→ [{ tab_id, tab_name, message_id, role, snippet, timestamp_ms }]
```

### Frontend Search UI

#### Entry point

A magnifying glass icon button sits at the right end of the chat panel header (same row as the export/clear/compact buttons). Clicking it toggles the search bar open; pressing Escape or clicking it again closes it and restores normal view.

#### Search bar

```html
<div id="chat-search-bar" class="chat-search-bar hidden">
  <input id="chat-search-input" class="chat-search-input" type="search"
         placeholder="Search conversations…" autocomplete="off">
  <span id="chat-search-count" class="chat-search-count"></span>
  <button id="chat-search-close" class="chat-search-close" title="Close search">✕</button>
</div>
```

- Auto-focuses on open
- 300 ms debounce before firing request
- While typing: count label shows "Searching…"; after response: "12 results" or "No results"
- Scope toggle (optional, phase 2): "This chat" / "All chats" radio — default "All chats"

#### Results panel

Replaces the message list area while search is active. Each result is a compact card:

```html
<div class="chat-search-result" data-tab-id="..." data-msg-idx="...">
  <div class="chat-search-result-header">
    <span class="chat-search-result-tab">Rinn</span>
    <span class="chat-search-result-role">assistant</span>
    <span class="chat-search-result-date">May 2, 2026 · 3:14 PM</span>
  </div>
  <div class="chat-search-result-snippet">…context around the <mark>matched term</mark>…</div>
</div>
```

- Results are ordered by SQLite FTS5 `rank` (relevance)
- Max 50 results rendered; a "Show more" button loads the next page
- Clicking a result: calls `switchChatTab(tabId)`, waits for messages to load, scrolls to the message element with `data-msg-idx` matching the result, adds a brief highlight animation class (`chat-msg-highlight`) that fades after 2 s

#### JavaScript module additions

New file: `static/js/features/chat-search.js`

```js
import { activeChatTab, switchChatTab } from './chat-state.js';
import { renderChatMessages } from './chat-render.js';

let _debounceTimer = null;

export function initChatSearch() {
    const btn = document.getElementById('chat-search-btn');
    const bar = document.getElementById('chat-search-bar');
    const input = document.getElementById('chat-search-input');
    const closeBtn = document.getElementById('chat-search-close');

    btn?.addEventListener('click', () => toggleSearch());
    closeBtn?.addEventListener('click', () => closeSearch());
    input?.addEventListener('input', () => {
        clearTimeout(_debounceTimer);
        _debounceTimer = setTimeout(() => runSearch(input.value.trim()), 300);
    });
    document.addEventListener('keydown', e => {
        if (e.key === 'Escape') closeSearch();
    });
}

function toggleSearch() {
    const bar = document.getElementById('chat-search-bar');
    if (bar.classList.contains('hidden')) {
        bar.classList.remove('hidden');
        document.getElementById('chat-search-input')?.focus();
    } else {
        closeSearch();
    }
}

function closeSearch() {
    document.getElementById('chat-search-bar')?.classList.add('hidden');
    document.getElementById('chat-search-results')?.classList.add('hidden');
    document.getElementById('chat-messages')?.classList.remove('hidden');
    document.getElementById('chat-search-input').value = '';
}

async function runSearch(query) {
    if (!query) { closeSearch(); return; }
    const countEl = document.getElementById('chat-search-count');
    countEl.textContent = 'Searching…';

    const resp = await fetch(`/api/chat/search?q=${encodeURIComponent(query)}`);
    const results = await resp.json();     // [{ tab_id, tab_name, message_id, role, snippet, timestamp_ms }]

    countEl.textContent = results.length ? `${results.length} result${results.length !== 1 ? 's' : ''}` : 'No results';
    renderSearchResults(results);
}

function renderSearchResults(results) {
    const msgList = document.getElementById('chat-messages');
    let panel = document.getElementById('chat-search-results');
    if (!panel) {
        panel = document.createElement('div');
        panel.id = 'chat-search-results';
        panel.className = 'chat-search-results';
        msgList.parentNode.insertBefore(panel, msgList);
    }
    msgList.classList.add('hidden');
    panel.classList.remove('hidden');

    panel.innerHTML = results.map(r => `
      <div class="chat-search-result" data-tab-id="${escapeAttr(r.tab_id)}" data-msg-id="${r.message_id}">
        <div class="chat-search-result-header">
          <span class="chat-search-result-tab">${escapeHtml(r.tab_name)}</span>
          <span class="chat-search-result-role">${escapeHtml(r.role)}</span>
          <span class="chat-search-result-date">${new Date(r.timestamp_ms).toLocaleString()}</span>
        </div>
        <div class="chat-search-result-snippet">${r.snippet}</div>
      </div>`).join('');

    panel.querySelectorAll('.chat-search-result').forEach(el => {
        el.addEventListener('click', () => jumpToResult(el.dataset.tabId, parseInt(el.dataset.msgId)));
    });
}

async function jumpToResult(tabId, messageId) {
    await switchChatTab(tabId);
    // After tab switch, scroll to the message. The backend returns message rowid;
    // find the DOM element with matching data-msg-id (add this attribute to chat-render.js).
    const msgEl = document.querySelector(`.chat-message[data-msg-id="${messageId}"]`);
    if (msgEl) {
        msgEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
        msgEl.classList.add('chat-msg-highlight');
        setTimeout(() => msgEl.classList.remove('chat-msg-highlight'), 2000);
    }
    closeSearch();
}
```

#### Required changes in `chat-render.js`

- Add `data-msg-id="${msg.db_id || ''}"` to each rendered `.chat-message` element (the SQLite `messages.id` rowid, returned with tab messages in Option B)
- Import and call `initChatSearch()` from `initChat()`

#### Rust search endpoint

```rust
// GET /api/chat/search?q=<query>&limit=<n>
async fn chat_search(
    Query(params): Query<SearchParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let conn = state.db.lock().unwrap();
    let results = search_messages(&conn, &params.q, params.limit.unwrap_or(50))
        .unwrap_or_default();
    Json(results)
}

#[derive(Deserialize)]
struct SearchParams {
    q: String,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct SearchResult {
    tab_id: String,
    tab_name: String,
    message_id: i64,
    role: String,
    snippet: String,
    timestamp_ms: Option<i64>,
}

fn search_messages(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, m.id, m.role,
                snippet(messages_fts, 0, '<mark>', '</mark>', '…', 24) as snippet,
                m.timestamp_ms
         FROM messages_fts
         JOIN messages m ON m.id = messages_fts.rowid
         JOIN tabs t ON t.id = m.tab_id
         WHERE messages_fts MATCH ?1
           AND m.compaction_marker = 0
         ORDER BY rank
         LIMIT ?2"
    )?;
    let rows = stmt.query_map(params![query, limit as i64], |row| {
        Ok(SearchResult {
            tab_id: row.get(0)?,
            tab_name: row.get(1)?,
            message_id: row.get(2)?,
            role: row.get(3)?,
            snippet: row.get(4)?,
            timestamp_ms: row.get(5)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}
```

Note: The `snippet()` function returns raw HTML with `<mark>` tags — the frontend renders this as `innerHTML`. The snippet content comes from the SQLite FTS engine which never contains user-controlled HTML in its snippet wrapper; the message content itself must be escaped before insertion into FTS (handle at INSERT time in Rust using `html_escape::encode_text` or equivalent, or accept that snippet output is trusted given the local-only deployment model).

### Write Pattern Changes

Instead of persisting the full tab on every message:
- After user sends: `INSERT INTO messages` for user message
- After assistant finishes: `INSERT INTO messages` for assistant message + `UPDATE tabs SET updated_at = ?`
- Compaction: `DELETE FROM messages WHERE tab_id = ? AND seq < ?` then `INSERT INTO messages` for compaction tombstone
- Tab metadata change (rename, param change): `UPDATE tabs SET ... WHERE id = ?`

---

## Decision Matrix

| Concern | Option A (JSON Sharding) | Option B (SQLite) |
|---|---|---|
| Write amplification | Eliminated (per-tab files) | Eliminated (row-level) |
| New dependency | None | `rusqlite` (bundled) |
| Lazy loading | Yes (per-tab HTTP) | Yes (SQL query) |
| Atomic writes | Rename trick | Transactions |
| Full-text search | No | Yes |
| Cross-tab search | No | Yes |
| Migration complexity | Low | Medium |
| Implementation time | ~1 day | ~2–3 days |
| Risk | Low | Medium |

**Recommendation**: Ship Option A now to eliminate the most painful write-amplification problem. Option B is the correct long-term answer if search or cross-tab analytics are wanted.
