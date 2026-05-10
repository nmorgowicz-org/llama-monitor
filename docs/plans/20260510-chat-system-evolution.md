# Chat System Evolution
**Date:** 2026-05-10  
**Supersedes:** `docs/plans/20260503-chat_storage_rework.md`  
**Status:** Planned  
**Branch target:** `feature/chat-guided-generations` → `main`

---

## Executive Summary

The May 3 storage plan is still correct directionally, but the system has grown substantially since then (explicit mode v2, guided generation, persona-specific policies, context notes). This document expands the scope to three connected work streams:

1. **Navigation cleanup** — Remove `Sessions` and `Models` from the left sidebar. They are modal-based stubs that belong on the Server tab.
2. **Chat session sidebar** — Replace the overflowing horizontal tab bar with a persistent session panel inside the Chat view (Discord-style). This is the UX priority.
3. **Storage evolution** — Skip Option A (per-tab JSON) and implement SQLite directly. The chat session sidebar needs cross-session search; the Rust rewrite is happening anyway; bundled SQLite adds zero system dependencies.

---

## What Changed Since May 3

| Change | Impact on old plan |
|---|---|
| `explicit_level: u8` (3-state) replaces `explicit_mode: bool` | Schema update in SQLite schema |
| Per-persona `explicit_policies` with L1/L2 content | No schema impact (frontend-only) |
| `context_notes: Vec<ContextNote>` added to ChatTab | SQLite messages table gains a `context_notes` JSON column |
| `sidebar_width: u32` added to ChatTab | Add column to tabs table |
| `auto_compact_summarize`, `compact_mode` added | Add columns to tabs table |
| Tab overflow is an active pain point for users with many personas | Drives session sidebar priority |
| `visible_message_limit`, `stream_timeout` in model_params | Capture in `model_params` JSON blob |
| Message `_variants` / `_variantIndex` for regeneration | Add `variants` JSON column to messages table |

---

## Goals

### Must Have
- [ ] Sessions panel in chat view — list all conversations, grouped by recency
- [ ] Sessions panel — pinned conversations always at top
- [ ] Sessions panel — persona + explicit level visible per item
- [ ] Sessions panel — context pressure indicator per item
- [ ] Sessions panel — rename, delete, pin from context menu
- [ ] Sessions panel — collapsible (remember state in localStorage)
- [ ] Remove `Sessions` and `Models` nav items from left sidebar
- [ ] Per-tab file writes (no more full-array rewrite)
- [ ] Lazy-load messages (only active session loaded into memory)
- [ ] Migration from legacy `chat-tabs.json` on first start

### Should Have
- [ ] Sessions panel — search by session name
- [ ] Sessions panel — time-grouping (Today / Yesterday / This Week / Older)
- [ ] Sessions panel — drag to reorder
- [ ] Message-level persistence (append rather than full rewrite)
- [ ] Full-text search across all conversations (FTS5)

### Nice to Have
- [ ] Sessions panel — persona avatar (color + initial)
- [ ] Sessions panel — last message preview snippet
- [ ] Export/import from session panel context menu
- [ ] Bulk delete from trash view

---

## Architecture Overview

### Layout Change

**Current:**
```
[48px icon rail] [flex: 1 — chat view (tab bar + messages + input)]
```

**After:**
```
[48px icon rail] [240px session panel — visible when chat is active] [flex: 1 — chat content]
```

The session panel behaves like Discord's channel list: it lives to the right of the icon rail, only when the Chat nav item is active. On all other views (Server, Logs, Settings) the content area is unchanged.

### Navigation

**Current left sidebar items:** Server · Chat · Logs · Sessions · Models · Settings  
**After:** Server · Chat · Logs · Settings

Sessions and Models are modals triggered from the Server tab top bar — they don't need primary nav placement.

### Storage

**Current:** Single `chat-tabs.json` file, full overwrite on every change.

**After:** SQLite database at `~/.config/llama-monitor/chat.db`.
- `tabs` table — one row per conversation, all metadata
- `messages` table — one row per message, indexed by tab + sequence
- `messages_fts` virtual table — FTS5 full-text search over message content
- Migration from `chat-tabs.json` runs once on startup

---

## Phase 0: Navigation Cleanup

**Effort:** ~1 hour  
**Risk:** Very low

### `static/index.html`

Remove the two sidebar buttons:
```html
<!-- DELETE these two buttons from .sidebar-nav -->
<button class="sidebar-btn" data-tab="sessions" title="Sessions">…</button>
<button class="sidebar-btn" data-tab="models" title="Models">…</button>
```

The corresponding modals (`#session-modal`, `#models-modal`) and their JS (`sessions.js`, `models.js`) remain untouched — they are still triggered from the Server tab top bar via `#session-open-btn` etc.

### `static/js/features/nav.js`

Remove the `'sessions'` and `'models'` entries from any array or switch statement that maps `data-tab` values to behavior. The modal-opening logic in `sessions.js` / `models.js` uses direct button bindings and is unaffected.

### `static/css/layout.css`

If there are sidebar-specific sizing or ordering rules for the removed tabs, clean them up.

---

## Phase 1: Chat Session Sidebar

**Effort:** 2–3 days  
**Risk:** Medium (significant HTML/CSS/JS surgery, but storage unchanged in this phase)

### 1.1 Layout HTML — `static/index.html`

Add the session panel as a sibling to `.page` inside `.content-area`. It lives between the icon rail and the page:

```html
<div class="content-area">
  <div class="sidebar-nav" id="sidebar-nav">…</div>

  <!-- NEW: visible only when chat tab is active -->
  <aside class="chat-sessions-panel" id="chat-sessions-panel" aria-label="Conversations">
    <div class="csp-header">
      <span class="csp-title">Conversations</span>
      <button class="csp-collapse-btn" id="csp-collapse-btn" title="Collapse sidebar">
        <svg>…chevron-left…</svg>
      </button>
    </div>

    <div class="csp-actions">
      <button class="csp-new-btn" id="csp-new-btn">
        <svg>…plus…</svg>
        New Chat
      </button>
    </div>

    <div class="csp-search-wrap">
      <input class="csp-search" id="csp-search" type="search" placeholder="Search conversations…" autocomplete="off">
    </div>

    <div class="csp-list" id="csp-list">
      <!-- Rendered by chat-sessions-sidebar.js -->
    </div>

    <div class="csp-trash-strip" id="csp-trash-strip">
      <!-- "X deleted" undo strip rendered when trash is non-empty -->
    </div>
  </aside>

  <div class="page chat-page" id="page-chat">
    <!-- Existing chat content; remove .chat-tab-bar from here -->
    …
  </div>
  …other pages…
</div>
```

Remove `<div class="chat-tab-bar" id="chat-tab-bar">…</div>` from `#page-chat`. The horizontal tab bar is fully replaced by the session panel.

### 1.2 Session Panel Layout — `static/css/layout.css`

```css
/* The session panel slides in when the chat view is active */
.chat-sessions-panel {
  width: 240px;
  min-width: 180px;
  max-width: 320px;
  flex-shrink: 0;
  display: none;               /* hidden by default */
  flex-direction: column;
  background: var(--surface-card-base);
  border-right: 1px solid rgba(255, 255, 255, 0.06);
  overflow: hidden;
  transition: width 180ms ease, opacity 180ms ease;
}

/* Visible when chat nav is active */
.chat-sessions-panel.visible {
  display: flex;
}

/* Collapsed state — icon-only rail */
.chat-sessions-panel.collapsed {
  width: 0;
  opacity: 0;
  pointer-events: none;
}

/* Light theme */
[data-theme="light"] .chat-sessions-panel {
  background: var(--surface-card-elevated);
  border-right-color: rgba(0, 0, 0, 0.08);
}
```

### 1.3 Session Item Structure

Each conversation in the list is a `.csp-item` element:

```html
<div class="csp-item" data-tab-id="{id}" role="button" tabindex="0">
  <div class="csp-item-avatar" style="--avatar-hue: {hue}">
    <span class="csp-item-avatar-initial">{initial}</span>
  </div>
  <div class="csp-item-body">
    <div class="csp-item-name">{name}</div>
    <div class="csp-item-meta">
      <span class="csp-item-persona">{persona_name}</span>
      <!-- explicit badge: 🔓 level 1, 🔥 level 2, nothing at 0 -->
      <span class="csp-item-explicit" data-level="{level}"></span>
      <span class="csp-item-count">{msg_count}</span>
    </div>
    <div class="csp-item-ctx-bar">
      <div class="csp-item-ctx-fill" style="width: {ctx_pct}%"></div>
    </div>
  </div>
  <div class="csp-item-actions">
    <!-- revealed on hover -->
    <button class="csp-item-pin" title="Pin">📌</button>
    <button class="csp-item-more" title="More">⋯</button>
  </div>
</div>
```

**Avatar color**: hash the tab `id` to a hue (0–360) so each conversation gets a consistent color.

**Context bar**: a 4px tall strip at the bottom of the item, colored by pressure:
- < 50%: `var(--color-success)` (teal)
- 50–75%: `var(--color-warning)` (amber)  
- 75–90%: orange
- ≥ 90%: `var(--color-error)` (rose)

### 1.4 Session Grouping

The list is divided into sections by `updated_at`:

```
📌 Pinned          (pinned: true, any date)
   ──────
   Today           (updated today)
   ──────
   Yesterday
   ──────
   This Week       (last 7 days)
   ──────
   Older
```

Each section header is a `.csp-section-header` row. Sections without items are hidden.

### 1.5 New JS Module — `static/js/features/chat-sessions-sidebar.js`

```js
// Key exports:
export function initChatSessionsSidebar()    // bind events, init search
export function renderChatSessionsSidebar()  // rebuild entire list from chat.tabs
export function updateSessionItem(tabId)     // refresh one item (after rename, ctx change)
export function toggleSessionPanelCollapse() // persist collapsed state to localStorage

// Internal helpers:
function buildSessionItem(tab)              // returns DOM element
function groupTabsByRecency(tabs)           // returns { pinned, today, yesterday, week, older }
function avatarHue(id)                      // deterministic hue from UUID
function updateSectionVisibility()          // hide empty section headers
```

**Wiring into existing system:**

1. Call `initChatSessionsSidebar()` from `bootstrap.js` after `initChatTabs()`.
2. Call `renderChatSessionsSidebar()` from `registerChatViewBindings` alongside `renderChatTabs()` (which will be removed once the sidebar is stable).
3. In `switchChatTab(id)`: call `updateSessionItem(prevId)` and `updateSessionItem(id)` (mark active).
4. In `nav.js` `switchTab('chat')`: add `document.getElementById('chat-sessions-panel').classList.add('visible')` and remove it on other tab switches.

### 1.6 Context Menu (`.csp-item-more`)

Clicking the `⋯` button opens a small floating menu with:
- Rename
- Pin / Unpin
- Export (JSON / Markdown)
- Duplicate
- ─────
- Delete

Implement as a `.csp-context-menu` div positioned absolutely, dismissed on outside click or Escape.

### 1.7 Session Panel CSS — `static/css/chat.css` (new section)

Key styling rules:

```css
/* ── Chat Sessions Panel ──────────────────────────── */
.csp-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 12px 8px;
  border-bottom: 1px solid rgba(255,255,255,0.06);
}

.csp-title {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--color-text-secondary);
  letter-spacing: 0.03em;
  text-transform: uppercase;
}

.csp-new-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  width: calc(100% - 16px);
  margin: 10px 8px 4px;
  padding: 8px 12px;
  background: rgba(99, 102, 241, 0.12);
  border: 1px solid rgba(99, 102, 241, 0.25);
  border-radius: var(--radius-sm);
  color: var(--color-primary);
  font-size: var(--text-sm);
  font-weight: 500;
  cursor: pointer;
  transition: background 150ms ease, border-color 150ms ease;
}
.csp-new-btn:hover {
  background: rgba(99, 102, 241, 0.2);
  border-color: rgba(99, 102, 241, 0.4);
}

.csp-search {
  width: calc(100% - 16px);
  margin: 6px 8px;
  padding: 6px 10px;
  background: rgba(255,255,255,0.05);
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: var(--radius-sm);
  color: var(--color-text-primary);
  font-size: var(--text-sm);
}

.csp-list {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 0 0 8px;
}

.csp-section-header {
  padding: 10px 10px 4px;
  font-size: 0.7rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--color-text-muted);
  opacity: 0.7;
}

.csp-item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px 10px;
  margin: 1px 4px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 120ms ease;
  position: relative;
}
.csp-item:hover {
  background: rgba(255,255,255,0.05);
}
.csp-item.active {
  background: rgba(99, 102, 241, 0.12);
  border-left: 2px solid var(--color-primary);
  padding-left: 8px;
}

.csp-item-avatar {
  width: 30px;
  height: 30px;
  flex-shrink: 0;
  border-radius: 8px;
  background: hsl(var(--avatar-hue, 240), 50%, 30%);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: 700;
  color: #fff;
  margin-top: 1px;
}

.csp-item-name {
  font-size: var(--text-sm);
  font-weight: 500;
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  line-height: 1.3;
}

.csp-item-meta {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 2px;
}

.csp-item-persona {
  font-size: 0.68rem;
  color: var(--color-info);
  opacity: 0.75;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 90px;
}

.csp-item-explicit[data-level="1"]::after { content: ' 🔓'; font-size: 0.6rem; }
.csp-item-explicit[data-level="2"]::after { content: ' 🔥'; font-size: 0.6rem; }

.csp-item-count {
  font-size: 0.65rem;
  color: var(--color-text-muted);
  margin-left: auto;
  white-space: nowrap;
}

.csp-item-ctx-bar {
  height: 3px;
  background: rgba(255,255,255,0.06);
  border-radius: 2px;
  margin-top: 4px;
  overflow: hidden;
}
.csp-item-ctx-fill {
  height: 100%;
  border-radius: 2px;
  background: var(--color-success);
  transition: width 600ms ease, background 600ms ease;
}

/* Context pressure colors */
.csp-item[data-ctx-level="medium"] .csp-item-ctx-fill { background: var(--color-warning); }
.csp-item[data-ctx-level="high"]   .csp-item-ctx-fill { background: #f97316; }
.csp-item[data-ctx-level="critical"] .csp-item-ctx-fill { background: var(--color-error); }

.csp-item-actions {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  display: none;
  gap: 2px;
}
.csp-item:hover .csp-item-actions { display: flex; }

/* Animations */
@keyframes csp-item-enter {
  from { opacity: 0; transform: translateX(-8px); }
  to   { opacity: 1; transform: translateX(0); }
}
.csp-item-new {
  animation: csp-item-enter 200ms ease forwards;
}

@media (prefers-reduced-motion: reduce) {
  .csp-item-new { animation: none; }
  .chat-sessions-panel { transition: none; }
}
```

### 1.8 Transition of `renderChatTabs()`

During Phase 1, `renderChatTabs()` in `chat-render.js` is kept alive but its output div (`#chat-tab-bar`) is removed from the HTML. Add a guard:

```js
export function renderChatTabs() {
    const bar = document.getElementById('chat-tab-bar');
    if (!bar) return;   // session sidebar has taken over
    // …existing render code…
}
```

Once the session sidebar is stable and tested, delete `renderChatTabs()`, `updateTabBarOverflowMask()`, and all `.chat-tab` CSS rules.

---

## Phase 2: Storage Evolution — SQLite

**Effort:** 2–3 days  
**Risk:** Medium  
**Recommendation:** Implement SQLite directly (skip per-tab JSON sharding). Rationale:
- Cross-session search is necessary for the session panel to be genuinely useful
- API layer rewrite is identical whether backed by JSON or SQL
- `rusqlite` with `bundled` feature compiles SQLite from source — no system dependency
- Atomic transactions are safer than the tmp-rename trick under concurrent writes

### 2.1 `Cargo.toml`

```toml
[dependencies]
rusqlite = { version = "0.31", features = ["bundled"] }
```

### 2.2 Schema — `src/chat_storage.rs`

```sql
CREATE TABLE IF NOT EXISTS tabs (
    id                    TEXT PRIMARY KEY,
    name                  TEXT NOT NULL,
    system_prompt         TEXT NOT NULL DEFAULT '',
    ai_name               TEXT,
    user_name             TEXT,
    explicit_level        INTEGER NOT NULL DEFAULT 0,  -- 0/1/2
    active_template_id    TEXT,
    auto_compact          INTEGER NOT NULL DEFAULT 1,
    auto_compact_summarize INTEGER NOT NULL DEFAULT 0,
    compact_mode          TEXT NOT NULL DEFAULT 'percent',
    compact_threshold     REAL NOT NULL DEFAULT 0.8,
    model_params          TEXT NOT NULL DEFAULT '{}',  -- JSON blob
    context_notes         TEXT NOT NULL DEFAULT '[]',  -- JSON blob: Vec<ContextNote>
    sidebar_width         INTEGER NOT NULL DEFAULT 280,
    tab_order             INTEGER NOT NULL DEFAULT 0,
    pinned                INTEGER NOT NULL DEFAULT 0,
    last_ctx_pct          REAL,
    total_input_tokens    INTEGER NOT NULL DEFAULT 0,
    total_output_tokens   INTEGER NOT NULL DEFAULT 0,
    created_at            INTEGER NOT NULL,
    updated_at            INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    tab_id                TEXT NOT NULL REFERENCES tabs(id) ON DELETE CASCADE,
    role                  TEXT NOT NULL CHECK(role IN ('user','assistant','system')),
    content               TEXT NOT NULL,
    timestamp_ms          INTEGER NOT NULL DEFAULT 0,
    input_tokens          INTEGER,
    output_tokens         INTEGER,
    cumulative_input_tokens  INTEGER,
    cumulative_output_tokens INTEGER,
    compaction_marker     INTEGER NOT NULL DEFAULT 0,
    variants              TEXT,              -- JSON: _variants array (null if none)
    variant_index         INTEGER,           -- _variantIndex
    seq                   INTEGER NOT NULL   -- insertion order within tab
);

CREATE INDEX IF NOT EXISTS idx_messages_tab ON messages(tab_id, seq);

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    content,
    content='messages',
    content_rowid='id'
);

-- Keep FTS in sync
CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;
CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content) VALUES ('delete', old.id, old.content);
END;
CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content) VALUES ('delete', old.id, old.content);
    INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;
```

### 2.3 Rust Module — `src/chat_storage.rs`

```rust
use std::path::PathBuf;
use anyhow::Result;
use rusqlite::{Connection, params};

pub struct ChatStorage {
    pub conn: std::sync::Mutex<Connection>,
}

impl ChatStorage {
    pub fn open(db_path: &PathBuf) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Self::migrate(&conn)?;
        Ok(Self { conn: std::sync::Mutex::new(conn) })
    }

    fn migrate(conn: &Connection) -> Result<()> {
        conn.execute_batch(SCHEMA_SQL)?;   // SCHEMA_SQL = the CREATE TABLE statements above
        Ok(())
    }

    /// One-time import from legacy chat-tabs.json.
    /// Renames old file to chat-tabs.json.bak when done.
    pub fn migrate_from_legacy(&self, legacy_path: &PathBuf) -> Result<()> {
        if !legacy_path.exists() { return Ok(()); }
        let raw = std::fs::read_to_string(legacy_path)?;
        let tabs: Vec<serde_json::Value> = serde_json::from_str(&raw)?;
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        for (order, tab) in tabs.iter().enumerate() {
            // INSERT INTO tabs … (map JSON fields to columns)
            // INSERT INTO messages … for each message in tab["messages"]
        }
        tx.commit()?;
        std::fs::rename(legacy_path, legacy_path.with_extension("json.bak"))?;
        Ok(())
    }

    // --- Tab operations ---

    pub fn list_tabs(&self) -> Result<Vec<TabMeta>> { … }

    pub fn get_tab(&self, id: &str) -> Result<ChatTabRow> { … }

    pub fn create_tab(&self, tab: &ChatTabRow) -> Result<()> { … }

    pub fn update_tab_meta(&self, tab: &ChatTabRow) -> Result<()> { … }

    pub fn delete_tab(&self, id: &str) -> Result<()> { … }

    pub fn reorder_tabs(&self, ordered_ids: &[String]) -> Result<()> { … }

    // --- Message operations ---

    pub fn load_messages(&self, tab_id: &str) -> Result<Vec<MessageRow>> { … }

    pub fn append_message(&self, tab_id: &str, msg: &MessageRow) -> Result<i64> { … }

    pub fn update_message(&self, msg: &MessageRow) -> Result<()> { … }

    pub fn delete_messages_after(&self, tab_id: &str, seq: i64) -> Result<()> { … }

    pub fn compact_tab(&self, tab_id: &str, new_messages: &[MessageRow]) -> Result<()> {
        // Wraps DELETE + batch INSERT in a transaction
    }

    // --- Search ---

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> { … }
}
```

**TabMeta** (index-only, no messages — used for session panel):
```rust
pub struct TabMeta {
    pub id: String,
    pub name: String,
    pub explicit_level: u8,
    pub active_template_id: Option<String>,
    pub pinned: bool,
    pub tab_order: i64,
    pub last_ctx_pct: Option<f32>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub message_count: u64,     // computed with COUNT(*)
    pub created_at: u64,
    pub updated_at: u64,
}
```

### 2.4 AppState Integration — `src/state.rs`

Add `chat_storage` to AppState:
```rust
pub struct AppState {
    // …existing fields…
    pub chat_storage: Arc<ChatStorage>,
}
```

Initialize in `src/main.rs` (or wherever AppState is constructed):
```rust
let db_path = config_dir.join("chat.db");
let chat_storage = Arc::new(ChatStorage::open(&db_path)?);
// Run legacy migration (no-op if already done)
chat_storage.migrate_from_legacy(&config_dir.join("chat-tabs.json"))?;
```

### 2.5 New API Endpoints — `src/web/api.rs`

Replace the two existing chat-tab endpoints with a full REST set:

| Method | Path | Handler | Notes |
|---|---|---|---|
| `GET` | `/api/chat/tabs` | `api_list_tabs` | Returns `Vec<TabMeta>` — metadata only, **no messages** |
| `POST` | `/api/chat/tabs` | `api_create_tab` | Body: `ChatTab` (sans id). Returns created `TabMeta` with new id. |
| `GET` | `/api/chat/tabs/:id` | `api_get_tab` | Returns full tab with messages |
| `PUT` | `/api/chat/tabs/:id` | `api_update_tab` | Full tab save (metadata + messages replaced) |
| `PATCH` | `/api/chat/tabs/:id/meta` | `api_update_tab_meta` | Metadata only (name, params, etc.) — no messages |
| `POST` | `/api/chat/tabs/:id/messages` | `api_append_message` | Append single message after streaming completes |
| `DELETE` | `/api/chat/tabs/:id` | `api_delete_tab` | Delete tab + cascade messages |
| `PATCH` | `/api/chat/tabs/order` | `api_reorder_tabs` | Body: `{ tab_order: ["id1","id2",…] }` |
| `GET` | `/api/chat/search` | `api_chat_search` | Query: `?q=…&limit=50`. Returns `Vec<SearchResult>` |

**Backward compat:** Keep old `GET /api/chat/tabs` returning full tabs (loading messages for all tabs) for one release cycle — it can stay behind a feature flag or just be removed once the JS is updated.

### 2.6 Migration Safety

- `chat-tabs.json` is renamed to `chat-tabs.json.bak` after import — user can manually restore if needed.
- If `chat.db` already exists (re-run after crash), `migrate_from_legacy` is a no-op because `.bak` file has no import target.
- WAL journal mode means concurrent reads during write are safe.

---

## Phase 3: Frontend API Adapters

**Effort:** 1–2 days  
**Risk:** Low (API shape is well-defined)

### 3.1 `static/js/features/chat-state.js`

#### `initChatTabs()` — lazy loading

```js
export async function initChatTabs() {
    const resp = await fetch('/api/chat/tabs');
    const metas = await resp.json();   // TabMeta[]

    chat.tabs = metas.map(m => ({
        ...m,
        messages: null,    // null = not yet loaded
        _loaded: false,
        model_params: m.model_params || defaultModelParams(),
    }));

    chat.activeTabId = chat.tabs[0]?.id ?? null;

    if (chat.tabs.length === 0) {
        await addChatTab();   // create first tab
        return;
    }

    if (chat.activeTabId) {
        await _loadTabMessages(chat.activeTabId);
    }
}
```

#### `_loadTabMessages(id)` — on-demand message load

```js
async function _loadTabMessages(id) {
    const tab = chat.tabs.find(t => t.id === id);
    if (!tab || tab._loaded) return;
    const resp = await fetch(`/api/chat/tabs/${id}`);
    const full = await resp.json();
    Object.assign(tab, full);
    tab._loaded = true;
}
```

Call `_loadTabMessages` at the start of `switchChatTab(id)` before rendering.

#### `addChatTab()` — POST, use returned id

```js
export async function addChatTab() {
    const newTab = newChatTabDefaults();
    const resp = await fetch('/api/chat/tabs', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(newTab),
    });
    const created = await resp.json();
    Object.assign(newTab, created);
    newTab._loaded = true;
    chat.tabs.push(newTab);
    switchChatTab(newTab.id);
    renderChatSessionsSidebar();
}
```

#### `closeChatTab(id)` — DELETE

```js
export async function closeChatTab(id) {
    // Move to trash (existing behavior)
    const tab = chat.tabs.find(t => t.id === id);
    if (tab) chat.tabTrash.push({ tab, trashedAt: Date.now() });

    chat.tabs = chat.tabs.filter(t => t.id !== id);

    // Backend delete
    await fetch(`/api/chat/tabs/${id}`, { method: 'DELETE' });

    // Switch to another tab
    if (chat.activeTabId === id) {
        chat.activeTabId = chat.tabs[0]?.id ?? null;
        if (chat.activeTabId) await _loadTabMessages(chat.activeTabId);
    }

    renderChatSessionsSidebar();
    renderChatMessages();
}
```

#### `scheduleChatPersist(tab)` — per-tab debounced save

```js
export function scheduleChatPersist(tab) {
    if (!tab) return;
    tab._dirty = true;
    clearTimeout(tab._persistTimer);
    tab._persistTimer = setTimeout(() => _persistTab(tab), 500);
}

async function _persistTab(tab) {
    if (!tab._dirty) return;
    tab._dirty = false;
    await fetch(`/api/chat/tabs/${tab.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(_normalizeTabForSave(tab)),
    });
}
```

Update all `scheduleChatPersist()` callsites to pass the tab: `scheduleChatPersist(activeChatTab())`.

#### `persistTabOrder()` — PATCH order

```js
async function persistTabOrder() {
    await fetch('/api/chat/tabs/order', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tab_order: chat.tabs.map(t => t.id) }),
    });
}
```

Call this from the drag-reorder drop handler (already exists in `chat-render.js`).

### 3.2 Message-Level Persistence (post-stream)

After `finalizeAssistantMessage()` in `chat-transport.js`, instead of calling `scheduleChatPersist()` for the whole tab, append only the two new messages (user + assistant) to the backend:

```js
// In _doSendChat(), after streaming completes:
await fetch(`/api/chat/tabs/${tab.id}/messages`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
        messages: [userMsg, assistantMsg],
    }),
});
// Still call scheduleChatPersist(tab) for metadata (tokenCounts, lastCtxPct)
scheduleChatPersist(tab);
```

This is an optimization — for Phase 3 it's acceptable to keep `PUT /api/chat/tabs/:id` as the write path and add message-level appending as a follow-up.

### 3.3 `static/js/features/chat-render.js`

- Remove or guard `renderChatTabs()` as described in §1.8.
- Remove `updateTabBarOverflowMask()`.
- Add `data-msg-id="{row_id}"` to each `.chat-message` element (the SQLite `messages.id` rowid, returned with tab messages). This enables `jumpToSearchResult()`.

### 3.4 Search Integration — `static/js/features/chat-search.js` (new)

The search input in the session panel header triggers a `/api/chat/search?q=…` request. Results replace the session list with a results view. Clicking a result calls `switchChatTab(tabId)` and scrolls to `data-msg-id`.

This is identical to the Option B search UI described in the May 3 plan — use that spec verbatim.

---

## Implementation Order

### Week 1 — UX First, Storage Later

| Day | Work | Files |
|---|---|---|
| Day 1 | Phase 0: Remove Sessions + Models nav items | `index.html`, `nav.js`, `layout.css` |
| Day 1 | Phase 1a: HTML structure — add `#chat-sessions-panel`, remove `#chat-tab-bar` | `index.html` |
| Day 2 | Phase 1b: CSS — layout, panel, item styles, animations | `layout.css`, `chat.css` |
| Day 3 | Phase 1c: `chat-sessions-sidebar.js` — render, group, search, context menu | new file |
| Day 3 | Phase 1d: Wire sidebar into nav.js, chat-state.js, bootstrap.js | `nav.js`, `bootstrap.js` |

At end of Week 1: sidebar renders from existing `chat.tabs` data (still backed by old `chat-tabs.json` API). All session management works in the new sidebar. Horizontal tab bar is gone.

### Week 2 — Storage

| Day | Work | Files |
|---|---|---|
| Day 4 | Phase 2a: `src/chat_storage.rs` — schema, open, migrate, CRUD | new file |
| Day 4 | Phase 2b: Integrate ChatStorage into AppState, run migration on startup | `src/state.rs`, `src/main.rs` |
| Day 5 | Phase 2c: New API endpoints in `api.rs` | `src/web/api.rs` |
| Day 6 | Phase 3a: Update `chat-state.js` — lazy load, per-tab persist, new APIs | `chat-state.js` |
| Day 6 | Phase 3b: Remove old `GET/PUT /api/chat/tabs` (full-array) endpoints | `src/web/api.rs` |
| Day 7 | Phase 3c: `chat-search.js` — search UI wired to FTS5 endpoint | new file |

---

## File Impact Summary

### New Files
| File | Purpose |
|---|---|
| `src/chat_storage.rs` | SQLite storage layer |
| `static/js/features/chat-sessions-sidebar.js` | Session panel UI module |
| `static/js/features/chat-search.js` | Cross-session search |

### Modified Files
| File | Change |
|---|---|
| `static/index.html` | Remove Sessions/Models nav buttons; add `#chat-sessions-panel`; remove `#chat-tab-bar` |
| `static/css/layout.css` | 3-column layout when chat active; session panel show/hide |
| `static/css/chat.css` | Session panel styles; remove old tab-bar styles |
| `static/js/features/chat-state.js` | Lazy load, per-tab persist, new API calls |
| `static/js/features/chat-render.js` | Guard/remove `renderChatTabs()`; add `data-msg-id` to messages |
| `static/js/features/nav.js` | Show/hide session panel on chat nav activation |
| `static/js/bootstrap.js` | Call `initChatSessionsSidebar()` |
| `src/web/api.rs` | Replace 2 endpoints with 9; wire to ChatStorage |
| `src/state.rs` | Add `chat_storage: Arc<ChatStorage>` |
| `src/main.rs` (or equivalent) | Construct ChatStorage, run legacy migration |
| `Cargo.toml` | Add `rusqlite = { version = "0.31", features = ["bundled"] }` |

### Deleted (after Phase 3 stabilizes)
| File / Symbol | Reason |
|---|---|
| `renderChatTabs()` in `chat-render.js` | Replaced by session sidebar |
| `updateTabBarOverflowMask()` in `chat-render.js` | No longer needed |
| All `.chat-tab`, `.chat-tab-bar`, `.chat-tab-*` CSS rules | Replaced by `.csp-*` rules |
| `GET /api/chat/tabs` (full-array) | Replaced by metadata-only version |
| `PUT /api/chat/tabs` (full-array) | Replaced by per-tab `PUT /api/chat/tabs/:id` |

---

## Risk & Mitigation

| Risk | Likelihood | Mitigation |
|---|---|---|
| SQLite migration loses tabs | Low | Migration is idempotent; original `.json.bak` kept as fallback |
| Session panel layout breaks on narrow viewports | Medium | Test at 1024px; add responsive collapse at `< 1200px` |
| Drag-to-reorder in sidebar (vs. horizontal bar) | Medium | Implement as list drag; same JS technique, different axis |
| Performance with many sessions | Low | FTS5 and tab_order index; panel renders lazily |
| Bundled SQLite compile time increase | Low | `rusqlite` bundled adds ~10s to clean release build |
| Breaking change for existing `PUT /api/chat/tabs` callers | None | No external callers; this is a local app |

---

## Design Tokens Used

All new components use existing design tokens from `tokens.css`:
- Colors: `--color-primary` (indigo), `--color-info` (cyan), `--color-success/warning/error`
- Surfaces: `--surface-card-base`, `--surface-card-elevated`
- Radii: `--radius-sm` (8px)
- Typography: `--text-sm`, `--font-body`
- Shadows: `--shadow-surface`
- Gaps: `--gap-xs`, `--gap-sm`

Animations: use `@keyframes csp-item-enter` (new) for item insertions; reuse `card-float` timing vars for panel transitions.

---

*Supersedes: `docs/plans/20260503-chat_storage_rework.md`*  
*Related: `docs/plans/20260510-explicit_persona_enhancements.md`, `docs/plans/20260508-chat-guided-generation-decisions.md`*
