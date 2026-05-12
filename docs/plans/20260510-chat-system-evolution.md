# Chat System Evolution
**Date:** 2026-05-10
**Supersedes:** `docs/plans/20260503-chat_storage_rework.md` (deleted)
**Status:** Planned
**Branch target:** `feature/chat-guided-generations` → `main`

---

## Executive Summary

Three connected work streams that together replace the overflowing horizontal tab bar with a
Discord-style session sidebar, reduce storage write amplification, and add full-text search across
conversations.

1. **Phase 0 — Navigation cleanup** (~1 hr): Remove `Sessions` and `Models` from the left icon
   rail. Both open modals that are already reachable from the Server tab top bar.
2. **Phase 1 — Chat session sidebar** (2–3 days): Replace `#chat-tab-bar` (horizontal, overflows
   silently) with a 240 px panel inside the Chat view. Discord-style list, pinned section, recency
   groups, context-pressure bars, rename/pin/delete/export context menu.
3. **Phase 2 — SQLite storage** (2–3 days): Replace the single `chat-tabs.json` full-overwrite
   approach with `rusqlite` (bundled). FTS5 full-text search, row-level writes, atomic transactions,
   zero system dependencies. The session sidebar needs cross-session search; the Rust rewrite is
   happening anyway.
4. **Phase 3 — Frontend API adapters** (1–2 days): Lazy-load messages (only active session),
   per-tab debounced persistence, `chat-search.js`.

---

## What Changed Since May 3

| Change | Impact |
|---|---|
| `explicit_level: u8` (3-state) replaces `explicit_mode: bool` | SQLite schema uses `INTEGER 0/1/2` |
| `context_notes: Vec<ContextNote>` on ChatTab | `context_notes TEXT JSON` column in `tabs` |
| `sidebar_width: u32` | `sidebar_width INTEGER` column in `tabs` |
| `auto_compact_summarize`, `compact_mode` | Two new columns in `tabs` |
| Message `_variants` / `_variantIndex` | `variants TEXT` + `variant_index INTEGER` in `messages` |
| Tab overflow is an active UX pain point | Drives session sidebar priority |

---

## ASCII Wireframes

### Full layout — chat view active, panel expanded

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  LLAMA MONITOR                                           v0.x.x  ▣ idle     │  ← nav cockpit
├──────────────────────┬──────────────────────────────────────────────────────┤
│ ▣ Server             │  Conversations     [+  New Chat]                      │
│ ● Chat               │ ┌──────────────────────────────────┐                 │
│   Logs               │ │ 🔍 Search conversations…          │                 │
│   Settings           │ └──────────────────────────────────┘                 │
│                      │  📌 PINNED                                            │
│                      │  ┌──────────────────────────────────────────────────┐│
│                      │  │ [R]  Rinn — Explicit Roleplay  🔥  · 213 msgs    ││
│                      │  │      Coder persona             ░░░░████████  82% ││
│                      │  └──────────────────────────────────────────────────┘│
│                      │  TODAY                                                │
│                      │  ┌──────────────────────────────────────────────────┐│
│                      │  │ [A]  Aria — sci-fi worldbuilding    · 44 msgs    ││  ← active (indigo)
│                      │  │      StoryTeller                 ░░░░░░░█  27%   ││
│                      │  └──────────────────────────────────────────────────┘│
│                      │  ┌──────────────────────────────────────────────────┐│
│                      │  │ [G]  General Questions              · 8 msgs     ││
│                      │  │      Default                     ░░░░░░░░   4%   ││
│                      │  └──────────────────────────────────────────────────┘│
│                      │  YESTERDAY                                            │
│                      │  ┌──────────────────────────────────────────────────┐│
│                      │  │ [C]  Code Review Session            · 61 msgs    ││
│                      │  │      Coder                       ░░░░████  60%   ││
│                      │  └──────────────────────────────────────────────────┘│
│                      │                              ◀ collapse               │
├──────────────────────┴─────────────────────────────────────────────────────┤│
│  [Behavior] [Settings] [Style] [Compact]         AI: Aria   You: Nick  🔓   ││  ← chat header
├────────────────────────────────────────────────────────────────────────────┤│
│                                                                             ││  ← messages
│  …                                                                          ││
│                                                                             ││
├────────────────────────────────────────────────────────────────────────────┤│
│  ┌──────────────────────────────────────────────────────────────────────┐  ││  ← input bar
│  │  Message Aria…                                                    ↵  │  ││
│  └──────────────────────────────────────────────────────────────────────┘  ││
└───────────────────────────────────────────────────────────────────────────────┘
208px fixed   240px session panel               flex: 1 chat content
```

### Session item states

```
DEFAULT:
  ┌──────────────────────────────────────────────┐
  │ [G]  General Questions              · 8 msgs │
  │      Default persona             ░░░░░░   3% │
  └──────────────────────────────────────────────┘
  (no border, no background)

HOVER:
  ┌──────────────────────────────────────────────┐
  │ [G]  General Questions   [📌] [⋯]  · 8 msgs │  ← action buttons revealed
  │      Default persona             ░░░░░░   3% │
  └──────────────────────────────────────────────┘
  (rgba(255,255,255,0.05) background)

ACTIVE:
  ┌──────────────────────────────────────────────┐
▌ │ [A]  Aria — sci-fi worldbuilding    · 44 msgs│  ← 2px indigo left border
  │      StoryTeller                 ░░░░░░█ 27% │
  └──────────────────────────────────────────────┘
  (rgba(99,102,241,0.12) background + 2px var(--color-primary) left border)

CONTEXT PRESSURE COLORS (ctx bar fill):
  < 50%  →  var(--color-success)   teal
  50-75% →  var(--color-warning)   amber
  75-90% →  #f97316                orange
  ≥ 90%  →  var(--color-error)     rose
```

---

## Phase 0: Navigation Cleanup

**Files:** `static/index.html`, `static/js/features/nav.js`
**Risk:** Very low

### `static/index.html` — exact lines to remove

Remove these two `<button>` elements (currently lines ~153–160):

```html
        <button class="sidebar-btn" data-tab="sessions" id="sidebar-btn-sessions" title="Sessions">
            <span class="sidebar-icon">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><path d="M8 21h8M12 17v4"/></svg>
            </span>
            <span class="sidebar-label">Sessions</span>
        </button>
        <button class="sidebar-btn" data-tab="models" id="sidebar-btn-models" title="Models">
            <span class="sidebar-icon">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/></svg>
            </span>
            <span class="sidebar-label">Models</span>
        </button>
```

The `#session-modal` and `#models-modal` divs, and the JS modules `sessions.js` / `models.js`,
remain **untouched** — they are still opened via `#session-open-btn` / `#models-open-btn` on the
Server tab top bar.

### `static/js/features/nav.js` — `switchTab` cleanup

The current `switchTab` function has explicit branches for `'sessions'` and `'models'` in the
"no `page` div" path (lines 16–22). After removing those buttons from HTML, those branches are dead
code. Remove them:

**Before (lines 9–24):**
```js
    // Handle modal tabs (no corresponding page div)
    if (!page) {
        document.querySelectorAll('.sidebar-btn').forEach(b => b.classList.remove('active'));
        const sidebarButton = document.querySelector(`.sidebar-btn[data-tab="${name}"]`);
        if (sidebarButton) sidebarButton.classList.add('active');

        // Trigger modal open handlers
        if (name === 'settings') {
            window.openSettingsModal?.();
        } else if (name === 'models') {
            window.openModelsModal?.();
        } else if (name === 'sessions') {
            window.openSessionModal?.();
        }
        return;
    }
```

**After:**
```js
    // Handle modal tabs (no corresponding page div, e.g. settings)
    if (!page) {
        document.querySelectorAll('.sidebar-btn').forEach(b => b.classList.remove('active'));
        const sidebarButton = document.querySelector(`.sidebar-btn[data-tab="${name}"]`);
        if (sidebarButton) sidebarButton.classList.add('active');
        if (name === 'settings') window.openSettingsModal?.();
        return;
    }
```

---

## Phase 1: Chat Session Sidebar

**Files:** `static/index.html`, `static/css/layout.css`, `static/css/chat.css`,
`static/js/features/nav.js`, `static/js/bootstrap.js`, `static/js/features/chat-state.js`,
`static/js/features/chat-render.js`, `static/js/features/chat-sessions-sidebar.js` (new)

### 1.1 HTML structure — `static/index.html`

**Step A — Remove the tab bar.** These lines are currently at the top of `#page-chat` (~line 608):

```html
            <!-- Tab bar -->
            <div class="chat-tab-bar" id="chat-tab-bar">
                <button class="chat-tab-add" id="chat-tab-add-btn" title="New chat tab">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                        <path d="M12 5v14M5 12h14"/>
                    </svg>
                </button>
                <div class="chat-tab-trash-wrapper" id="chat-tab-trash-wrapper">
                    <button class="chat-tab-trash-btn" id="chat-tab-trash-btn" title="Trash">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m3 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6h14z"/>
                        </svg>
                    </button>
                </div>
            </div>
            <div class="chat-tab-trash-dropdown" id="chat-tab-trash-dropdown">
            </div>
```

Delete all of the above. The trash is replaced by the delete action in the session item context menu.

**Step B — Add session panel and wrap existing content.**

After the opening `<div class="page chat-page" id="page-chat">`, add the session panel and wrap
all remaining children in `.chat-main-area`:

```html
        <div class="page chat-page" id="page-chat">

            <!-- Session sidebar — slide in when Chat nav is active -->
            <aside class="chat-sessions-panel" id="chat-sessions-panel" aria-label="Conversations">
                <div class="csp-header">
                    <span class="csp-title">Conversations</span>
                    <button class="csp-collapse-btn" id="csp-collapse-btn" title="Collapse">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                            <path d="M15 18l-6-6 6-6"/>
                        </svg>
                    </button>
                </div>
                <div class="csp-actions">
                    <button class="csp-new-btn" id="csp-new-btn">
                        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M12 5v14M5 12h14"/></svg>
                        New Chat
                    </button>
                </div>
                <div class="csp-search-wrap">
                    <input class="csp-search" id="csp-search" type="search"
                           placeholder="Search conversations…" autocomplete="off">
                </div>
                <div class="csp-list" id="csp-list"></div>
                <div class="csp-trash-strip hidden" id="csp-trash-strip"></div>
            </aside>

            <!-- Main chat area (header + messages + input) -->
            <div class="chat-main-area" id="chat-main-area">

            <!-- … all the existing #page-chat children go here … -->
            <!-- chat-header, chat-messages, chat-input-area, context-sidebar, etc. -->

            </div><!-- /chat-main-area -->
        </div><!-- /page-chat -->
```

The `#chat-tab-trash-dropdown` floating div that was after the tab bar is no longer needed and
should be deleted. Trash is handled via the context menu and `#csp-trash-strip`.

### 1.2 Layout — `static/css/layout.css`

Three changes:

**Change 1 — Make `#page-chat` a flex row** (the global `.page.active` rule uses `display: flex;
flex-direction: column`; override it for the chat page):

```css
/* Chat page uses row layout to accommodate session panel */
#page-chat {
  flex-direction: row;
}
```

**Change 2 — Session panel (slide via width, NOT display).**

`display: none → display: flex` transitions don't animate. The panel must always be `display: flex`
and use `width: 0 → width: 240px` to animate:

```css
.chat-sessions-panel {
  width: 0;
  min-width: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  background: var(--surface-card-base);
  border-right: 1px solid rgba(255, 255, 255, 0.06);
  transition: width 220ms cubic-bezier(0.16, 1, 0.3, 1);
}

.chat-sessions-panel.visible {
  width: 240px;
  min-width: 240px;
}

/* Collapsed state: width snaps to 0 (panel is hidden) */
.chat-sessions-panel.visible.collapsed {
  width: 0;
  min-width: 0;
}

[data-theme="light"] .chat-sessions-panel {
  background: var(--surface-card-elevated);
  border-right-color: rgba(0, 0, 0, 0.08);
}

@media (prefers-reduced-motion: reduce) {
  .chat-sessions-panel { transition: none; }
}
```

**Change 3 — Chat main area fills remaining space:**

```css
.chat-main-area {
  flex: 1;
  min-width: 0;       /* prevent flex overflow */
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
```

### 1.3 Session panel styles — `static/css/chat.css`

Add a new section after the existing chat header styles. These rules are **in addition to** the
existing `.chat-tab-*` rules, which stay in place during Phase 1 (they are cleaned up in Phase 3
once the tab bar is confirmed gone everywhere):

```css
/* ── Chat Sessions Panel (.csp-*) ──────────────────────────────────────── */

.csp-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 12px 8px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  flex-shrink: 0;
}

.csp-title {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--color-text-secondary);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.csp-collapse-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 120ms ease, color 120ms ease;
}
.csp-collapse-btn:hover {
  background: rgba(255, 255, 255, 0.07);
  color: var(--color-text-primary);
}

.csp-actions {
  padding: 10px 8px 4px;
  flex-shrink: 0;
}

.csp-new-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 8px 12px;
  background: rgba(99, 102, 241, 0.1);
  border: 1px solid rgba(99, 102, 241, 0.22);
  border-radius: var(--radius-sm);
  color: var(--color-primary);
  font-size: var(--text-sm);
  font-weight: 500;
  cursor: pointer;
  transition: background 140ms ease, border-color 140ms ease;
}
.csp-new-btn:hover {
  background: rgba(99, 102, 241, 0.18);
  border-color: rgba(99, 102, 241, 0.38);
}

.csp-search-wrap {
  padding: 4px 8px 6px;
  flex-shrink: 0;
}

.csp-search {
  width: 100%;
  padding: 6px 10px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.07);
  border-radius: var(--radius-sm);
  color: var(--color-text-primary);
  font-size: var(--text-sm);
  outline: none;
  transition: border-color 140ms ease;
  box-sizing: border-box;
}
.csp-search:focus {
  border-color: rgba(99, 102, 241, 0.4);
}
.csp-search::placeholder { color: var(--color-text-muted); }

.csp-list {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding-bottom: 8px;
}
.csp-list::-webkit-scrollbar { width: 3px; }
.csp-list::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.12); border-radius: 2px; }

.csp-section-header {
  padding: 10px 10px 3px;
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.07em;
  text-transform: uppercase;
  color: var(--color-text-muted);
  opacity: 0.65;
  user-select: none;
}

.csp-item {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px 10px;
  margin: 1px 4px;
  border-radius: var(--radius-sm);
  border-left: 2px solid transparent;
  cursor: pointer;
  transition: background 100ms ease, border-color 100ms ease;
}
.csp-item:hover {
  background: rgba(255, 255, 255, 0.04);
}
.csp-item.active {
  background: rgba(99, 102, 241, 0.1);
  border-left-color: var(--color-primary);
}

.csp-item-avatar {
  width: 30px;
  height: 30px;
  min-width: 30px;
  border-radius: 8px;
  background: hsl(var(--avatar-hue, 240), 45%, 28%);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: 700;
  color: rgba(255,255,255,0.9);
  margin-top: 1px;
}

.csp-item-body {
  flex: 1;
  min-width: 0;
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
  font-size: 0.67rem;
  color: var(--color-info);
  opacity: 0.7;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 90px;
}

.csp-item-explicit[data-level="1"]::after { content: ' 🔓'; font-size: 0.6rem; opacity: 0.8; }
.csp-item-explicit[data-level="2"]::after { content: ' 🔥'; font-size: 0.6rem; opacity: 0.85; }

.csp-item-count {
  font-size: 0.64rem;
  color: var(--color-text-muted);
  margin-left: auto;
  white-space: nowrap;
  flex-shrink: 0;
}

.csp-item-ctx-bar {
  height: 3px;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 2px;
  margin-top: 5px;
  overflow: hidden;
}
.csp-item-ctx-fill {
  height: 100%;
  border-radius: 2px;
  background: var(--color-success);
  transition: width 500ms ease, background 500ms ease;
}
.csp-item[data-ctx="medium"] .csp-item-ctx-fill { background: var(--color-warning); }
.csp-item[data-ctx="high"]   .csp-item-ctx-fill { background: #f97316; }
.csp-item[data-ctx="critical"] .csp-item-ctx-fill { background: var(--color-error); }

.csp-item-actions {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  display: none;
  align-items: center;
  gap: 2px;
  background: var(--surface-card-base);
  padding: 2px;
  border-radius: var(--radius-sm);
}
.csp-item:hover .csp-item-actions { display: flex; }

.csp-item-action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
  line-height: 1;
  transition: background 100ms ease, color 100ms ease;
}
.csp-item-action-btn:hover {
  background: rgba(255, 255, 255, 0.08);
  color: var(--color-text-primary);
}
.csp-item-action-btn.delete:hover {
  background: rgba(244, 63, 94, 0.12);
  color: #f43f5e;
}

/* Context menu */
.csp-context-menu {
  position: fixed;
  z-index: 500;
  min-width: 160px;
  background: var(--surface-card-elevated);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: var(--radius-base);
  box-shadow: var(--shadow-surface);
  padding: 4px 0;
  outline: none;
}
.csp-context-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 14px;
  font-size: var(--text-sm);
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: background 80ms ease;
  white-space: nowrap;
}
.csp-context-menu-item:hover {
  background: rgba(255, 255, 255, 0.05);
  color: var(--color-text-primary);
}
.csp-context-menu-item.danger:hover {
  background: rgba(244, 63, 94, 0.08);
  color: #f43f5e;
}
.csp-context-menu-separator {
  height: 1px;
  background: rgba(255, 255, 255, 0.06);
  margin: 4px 0;
}

/* Trash undo strip */
.csp-trash-strip {
  padding: 8px 10px;
  border-top: 1px solid rgba(255, 255, 255, 0.05);
  font-size: var(--text-sm);
  color: var(--color-text-secondary);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex-shrink: 0;
}
.csp-trash-strip.hidden { display: none; }
.csp-trash-undo-btn {
  border: none;
  background: rgba(99, 102, 241, 0.12);
  color: var(--color-primary);
  border-radius: var(--radius-sm);
  padding: 3px 8px;
  font-size: var(--text-sm);
  cursor: pointer;
}
.csp-trash-undo-btn:hover { background: rgba(99, 102, 241, 0.22); }

/* Entry animation for new items */
@keyframes csp-item-enter {
  from { opacity: 0; transform: translateX(-6px); }
  to   { opacity: 1; transform: translateX(0); }
}
.csp-item-new { animation: csp-item-enter 180ms ease forwards; }

@media (prefers-reduced-motion: reduce) {
  .csp-item-new { animation: none; }
}
```

### 1.4 New JS module — `static/js/features/chat-sessions-sidebar.js`

Create this file. It depends on `chat-state.js` functions and uses the `chatViewBindings` injection
pattern to avoid circular imports with `chat-render.js`.

```js
// ── Chat Sessions Sidebar ─────────────────────────────────────────────────────
// Renders and manages the left session panel inside #page-chat.
// Activated when the Chat nav item is selected; hidden otherwise.

import { chat } from '../core/app-state.js';
import { switchChatTab, closeChatTab, addChatTab, renameChatTab,
         togglePinTab, activeChatTab } from './chat-state.js';

const CSP_COLLAPSED_KEY = 'csp-collapsed';

// ── Lifecycle ──────────────────────────────────────────────────────────────────

export function initChatSessionsSidebar() {
    const newBtn    = document.getElementById('csp-new-btn');
    const collapseBtn = document.getElementById('csp-collapse-btn');
    const searchEl  = document.getElementById('csp-search');

    newBtn?.addEventListener('click', () => addChatTab());
    collapseBtn?.addEventListener('click', toggleSessionPanelCollapse);

    searchEl?.addEventListener('input', () => {
        const q = searchEl.value.trim().toLowerCase();
        _applySearchFilter(q);
    });

    // Close context menus on outside click
    document.addEventListener('click', (e) => {
        if (!e.target.closest('.csp-context-menu')) _dismissContextMenu();
    });
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') _dismissContextMenu();
    });

    // Restore collapsed state
    if (localStorage.getItem(CSP_COLLAPSED_KEY) === 'true') {
        const panel = document.getElementById('chat-sessions-panel');
        panel?.classList.add('collapsed');
    }
}

// ── Show / Hide (called from nav.js) ──────────────────────────────────────────

export function showSessionPanel() {
    const panel = document.getElementById('chat-sessions-panel');
    if (!panel) return;
    panel.classList.add('visible');
}

export function hideSessionPanel() {
    const panel = document.getElementById('chat-sessions-panel');
    if (!panel) return;
    panel.classList.remove('visible');
}

export function toggleSessionPanelCollapse() {
    const panel = document.getElementById('chat-sessions-panel');
    if (!panel) return;
    const collapsed = panel.classList.toggle('collapsed');
    localStorage.setItem(CSP_COLLAPSED_KEY, collapsed.toString());

    const icon = document.querySelector('#csp-collapse-btn svg');
    if (icon) {
        // Flip chevron direction
        icon.style.transform = collapsed ? 'rotate(180deg)' : '';
    }
}

// ── Render ─────────────────────────────────────────────────────────────────────

export function renderChatSessionsSidebar() {
    const list = document.getElementById('csp-list');
    if (!list) return;

    const groups = _groupTabsByRecency(chat.tabs || []);
    const activeId = chat.activeTabId;

    const sections = [
        { key: 'pinned',    label: '📌 Pinned' },
        { key: 'today',     label: 'Today' },
        { key: 'yesterday', label: 'Yesterday' },
        { key: 'week',      label: 'This Week' },
        { key: 'older',     label: 'Older' },
    ];

    const frag = document.createDocumentFragment();

    for (const { key, label } of sections) {
        const tabs = groups[key];
        if (!tabs || tabs.length === 0) continue;

        const hdr = document.createElement('div');
        hdr.className = 'csp-section-header';
        hdr.textContent = label;
        frag.appendChild(hdr);

        for (const tab of tabs) {
            frag.appendChild(_buildSessionItem(tab, tab.id === activeId));
        }
    }

    list.innerHTML = '';
    list.appendChild(frag);

    _applySearchFilter(document.getElementById('csp-search')?.value.trim().toLowerCase() || '');
}

export function updateSessionItem(tabId) {
    const list = document.getElementById('csp-list');
    const existing = list?.querySelector(`.csp-item[data-tab-id="${tabId}"]`);
    if (!existing) return;

    const tab = (chat.tabs || []).find(t => t.id === tabId);
    if (!tab) { existing.remove(); return; }

    const isActive = tab.id === chat.activeTabId;
    const fresh = _buildSessionItem(tab, isActive);
    existing.replaceWith(fresh);
}

// ── Item builder ──────────────────────────────────────────────────────────────

function _buildSessionItem(tab, isActive) {
    const el = document.createElement('div');
    const ctxPct = tab.lastCtxPct || 0;
    const ctxLevel = ctxPct >= 90 ? 'critical' : ctxPct >= 75 ? 'high' : ctxPct >= 50 ? 'medium' : 'low';
    const msgCount = (tab.messages || []).filter(m => m.role !== 'system').length;
    const initial = (tab.name || '?').charAt(0).toUpperCase();
    const hue = _avatarHue(tab.id);

    el.className = 'csp-item' + (isActive ? ' active' : '');
    el.dataset.tabId = tab.id;
    el.dataset.ctx = ctxLevel;
    el.setAttribute('role', 'button');
    el.setAttribute('tabindex', '0');
    el.setAttribute('aria-current', isActive ? 'true' : 'false');
    el.style.setProperty('--avatar-hue', hue);

    // Persona label (async lookup)
    let personaHtml = '';
    if (tab.active_template_id) {
        personaHtml = `<span class="csp-item-persona" data-template-id="${escapeAttr(tab.active_template_id)}">…</span>`;
    } else {
        personaHtml = `<span class="csp-item-persona">Default</span>`;
    }

    // eslint-disable-next-line no-unsanitized/property
    el.innerHTML = `
      <div class="csp-item-avatar" style="--avatar-hue:${hue}">
        <span>${escapeHtml(initial)}</span>
      </div>
      <div class="csp-item-body">
        <div class="csp-item-name">${escapeHtml(tab.name || 'Untitled')}</div>
        <div class="csp-item-meta">
          ${personaHtml}
          <span class="csp-item-explicit" data-level="${tab.explicit_level || 0}"></span>
          <span class="csp-item-count">${msgCount ? `${msgCount} msg${msgCount !== 1 ? 's' : ''}` : ''}</span>
        </div>
        <div class="csp-item-ctx-bar">
          <div class="csp-item-ctx-fill" style="width:${ctxPct.toFixed(1)}%"></div>
        </div>
      </div>
      <div class="csp-item-actions">
        <button class="csp-item-action-btn" data-action="pin" title="${tab.pinned ? 'Unpin' : 'Pin'}">
          ${tab.pinned ? '📌' : '⊙'}
        </button>
        <button class="csp-item-action-btn" data-action="more" title="More">⋯</button>
      </div>
    `;

    // Resolve persona name async
    if (tab.active_template_id) {
        _resolvePersonaLabel(el, tab.active_template_id);
    }

    // Click — switch tab
    el.addEventListener('click', (e) => {
        const action = e.target.closest('[data-action]')?.dataset.action;
        if (action) return; // handled below
        switchChatTab(tab.id);
        renderChatSessionsSidebar(); // refresh active state
    });

    // Keyboard activation
    el.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            switchChatTab(tab.id);
            renderChatSessionsSidebar();
        }
    });

    // Action buttons
    el.addEventListener('click', (e) => {
        const btn = e.target.closest('[data-action]');
        if (!btn) return;
        e.stopPropagation();
        const action = btn.dataset.action;
        if (action === 'pin') {
            togglePinTab(tab.id);
            renderChatSessionsSidebar();
        } else if (action === 'more') {
            _showContextMenu(tab, btn);
        }
    });

    return el;
}

// ── Context menu ──────────────────────────────────────────────────────────────

let _activeMenu = null;

function _showContextMenu(tab, anchorEl) {
    _dismissContextMenu();

    const menu = document.createElement('div');
    menu.className = 'csp-context-menu';
    menu.setAttribute('role', 'menu');
    menu.setAttribute('tabindex', '-1');

    const items = [
        { label: 'Rename',          action: 'rename' },
        { label: tab.pinned ? 'Unpin' : 'Pin', action: 'pin' },
        { label: 'Export JSON',     action: 'export-json' },
        { label: 'Export Markdown', action: 'export-md' },
        { label: 'Duplicate',       action: 'duplicate' },
        { separator: true },
        { label: 'Delete',          action: 'delete', danger: true },
    ];

    for (const item of items) {
        if (item.separator) {
            const sep = document.createElement('div');
            sep.className = 'csp-context-menu-separator';
            menu.appendChild(sep);
            continue;
        }
        const el = document.createElement('div');
        el.className = 'csp-context-menu-item' + (item.danger ? ' danger' : '');
        el.textContent = item.label;
        el.setAttribute('role', 'menuitem');
        el.addEventListener('click', (e) => {
            e.stopPropagation();
            _dismissContextMenu();
            _handleContextAction(tab, item.action);
        });
        menu.appendChild(el);
    }

    document.body.appendChild(menu);
    _activeMenu = menu;

    // Position near anchor
    const rect = anchorEl.getBoundingClientRect();
    const menuW = 170;
    const left = Math.min(rect.right + 4, window.innerWidth - menuW - 8);
    menu.style.left = left + 'px';
    menu.style.top  = rect.top + 'px';
    menu.focus();
}

function _dismissContextMenu() {
    _activeMenu?.remove();
    _activeMenu = null;
}

function _handleContextAction(tab, action) {
    switch (action) {
        case 'rename': {
            const newName = prompt('Rename conversation:', tab.name);
            if (newName && newName.trim()) {
                renameChatTab(tab.id, newName.trim());
                renderChatSessionsSidebar();
            }
            break;
        }
        case 'pin':
            togglePinTab(tab.id);
            renderChatSessionsSidebar();
            break;
        case 'export-json':
            window.exportChatTab?.('json');
            break;
        case 'export-md':
            window.exportChatTab?.('md');
            break;
        case 'duplicate': {
            // Phase 1: duplicate by creating a new tab with same name + " (copy)"
            import('./chat-state.js').then(({ newChatTab, addChatTab: add }) => {
                const copy = { ...tab, id: crypto.randomUUID(), name: tab.name + ' (copy)',
                               messages: [...(tab.messages || [])], created_at: Date.now(),
                               updated_at: Date.now() };
                chat.tabs.push(copy);
                switchChatTab(copy.id);
                renderChatSessionsSidebar();
                import('./chat-state.js').then(m => m.scheduleChatPersist());
            });
            break;
        }
        case 'delete':
            closeChatTab(tab.id);
            renderChatSessionsSidebar();
            break;
    }
}

// ── Search filter ─────────────────────────────────────────────────────────────

function _applySearchFilter(q) {
    const list = document.getElementById('csp-list');
    if (!list) return;

    list.querySelectorAll('.csp-item').forEach(el => {
        if (!q) { el.style.display = ''; return; }
        const name = el.querySelector('.csp-item-name')?.textContent.toLowerCase() || '';
        const persona = el.querySelector('.csp-item-persona')?.textContent.toLowerCase() || '';
        el.style.display = (name.includes(q) || persona.includes(q)) ? '' : 'none';
    });

    list.querySelectorAll('.csp-section-header').forEach(hdr => {
        // Hide section header if all items in section are hidden
        let next = hdr.nextElementSibling;
        let allHidden = true;
        while (next && !next.classList.contains('csp-section-header')) {
            if (next.classList.contains('csp-item') && next.style.display !== 'none') {
                allHidden = false; break;
            }
            next = next.nextElementSibling;
        }
        hdr.style.display = allHidden ? 'none' : '';
    });
}

// ── Grouping & utilities ──────────────────────────────────────────────────────

function _groupTabsByRecency(tabs) {
    const now = Date.now();
    const d  = (ms) => Math.floor(ms / 86400000);
    const today = d(now);

    const groups = { pinned: [], today: [], yesterday: [], week: [], older: [] };
    for (const tab of tabs) {
        if (tab.pinned) { groups.pinned.push(tab); continue; }
        const dayDiff = today - d(tab.updated_at || tab.created_at || now);
        if (dayDiff === 0)      groups.today.push(tab);
        else if (dayDiff === 1) groups.yesterday.push(tab);
        else if (dayDiff <= 7)  groups.week.push(tab);
        else                    groups.older.push(tab);
    }
    return groups;
}

function _avatarHue(id) {
    // Deterministic hue from UUID string
    let h = 0;
    for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) & 0xFFFF;
    return h % 360;
}

async function _resolvePersonaLabel(itemEl, templateId) {
    const span = itemEl.querySelector('.csp-item-persona');
    if (!span) return;
    const templates = await window.loadTemplates?.();
    const tmpl = templates?.find(t => t.id === templateId);
    span.textContent = tmpl?.name || '';
}

function escapeAttr(s) {
    return String(s).replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}
```

### 1.5 `static/js/features/nav.js` — show/hide session panel on tab switch

After the `switchTab` cleanup in Phase 0, add panel show/hide calls. The full revised `switchTab`:

```js
import { showSessionPanel, hideSessionPanel } from './chat-sessions-sidebar.js';

export function switchTab(name) {
    const page = document.getElementById('page-' + name);

    // Handle modal tabs (no corresponding page div, e.g. settings)
    if (!page) {
        document.querySelectorAll('.sidebar-btn').forEach(b => b.classList.remove('active'));
        const sidebarButton = document.querySelector(`.sidebar-btn[data-tab="${name}"]`);
        if (sidebarButton) sidebarButton.classList.add('active');
        if (name === 'settings') window.openSettingsModal?.();
        return;
    }

    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    document.querySelectorAll('.sidebar-btn').forEach(b => b.classList.remove('active'));

    page.classList.add('active');

    const sidebarButton = document.querySelector(`.sidebar-btn[data-tab="${name}"]`);
    if (sidebarButton) sidebarButton.classList.add('active');

    // Show session panel only when Chat is active
    if (name === 'chat') {
        showSessionPanel();
    } else {
        hideSessionPanel();
    }
}
```

### 1.6 `static/js/bootstrap.js` — wire up session sidebar

**Add import** (near the top, alongside other feature imports):
```js
import { initChatSessionsSidebar, renderChatSessionsSidebar } from './features/chat-sessions-sidebar.js';
```

**Replace the trash button block** (currently lines 66–101, binding `#chat-tab-add-btn` and the
`#chat-tab-trash-btn` dropdown). These elements are being removed from HTML. Replace with:

```js
// Bind session sidebar new-chat button (also wired in initChatSessionsSidebar via #csp-new-btn)
// Nothing needed here — initChatSessionsSidebar() handles its own bindings.
```

**Add init call** after `initChatRender()`:

```js
// Phase 6b: Chat rendering, templates, and params (after state/transport)
initChatRender();
initChatSessionsSidebar();   // ← ADD THIS LINE

// Bind chat scroll button
document.getElementById('chat-scroll-bottom')?.addEventListener('click', () => chatScroll(true));
```

**Remove the trash-related event listeners** (the three `document.addEventListener` blocks for the
dropdown at lines 83–101). They reference `#chat-tab-trash-btn` and `#chat-tab-trash-dropdown`
which no longer exist.

### 1.7 `static/js/features/chat-state.js` — call sidebar render

The `chatViewBindings` object currently drives rendering via the injection pattern. Add a
`renderChatSessionsSidebar` slot and wire it:

**In the `chatViewBindings` object** (currently lines 11–23), add one entry:
```js
const chatViewBindings = {
    renderChatTabs: null,
    renderChatSessionsSidebar: null,   // ← ADD
    renderChatMessages: null,
    // … rest unchanged …
};
```

**In `addChatTab()`** — currently just creates a tab and calls `scheduleChatPersist`. Add sidebar
render:
```js
export function addChatTab() {
    const tab = newChatTab(`Chat ${chat.tabs.length + 1}`);
    chat.tabs.push(tab);
    switchChatTab(tab.id);
    chatViewBindings.renderChatSessionsSidebar?.();   // ← ADD
    scheduleChatPersist();
}
```

**In `closeChatTab()`** — after the existing `renderChatTabs` call:
```js
    chatViewBindings.renderChatTabs?.();
    chatViewBindings.renderChatSessionsSidebar?.();   // ← ADD
    chatViewBindings.renderChatMessages?.();
    scheduleChatPersist();
```

**In `switchChatTab()`** — after setting `chat.activeTabId`:
```js
export function switchChatTab(id) {
    if (chat.busy) return;
    chat.activeTabId = id;
    chatViewBindings.renderChatTabs?.();
    chatViewBindings.renderChatSessionsSidebar?.();   // ← ADD (marks new active item)
    chatViewBindings.renderChatMessages?.();
    // … rest unchanged …
}
```

**In `togglePinTab()`**, `renameChatTab()`, `restoreTabFromTrash()` — add
`chatViewBindings.renderChatSessionsSidebar?.()` alongside every `renderChatTabs` call.

**In `initChatTabs()`** — add sidebar render after `renderChatTabs`:
```js
    chatViewBindings.renderChatTabs?.();
    chatViewBindings.renderChatSessionsSidebar?.();   // ← ADD
    chatViewBindings.renderChatMessages?.();
```

**Wire the binding in `initChatRender()`** (in `chat-render.js`, at the `registerChatViewBindings`
call near the bottom). Add:
```js
    registerChatViewBindings({
        // … existing bindings …
        renderChatSessionsSidebar,
    });
```

For this to work, `chat-render.js` must import `renderChatSessionsSidebar`:
```js
import { renderChatSessionsSidebar } from './chat-sessions-sidebar.js';
```

### 1.8 `static/js/features/chat-render.js` — guard `renderChatTabs()`

Add a guard at the top of `renderChatTabs()` so it silently no-ops once `#chat-tab-bar` is removed
from the DOM:

```js
export function renderChatTabs() {
    ensureChatElements();
    const bar = chatTabBarEl;
    if (!bar) return;   // session sidebar has taken over — tab bar removed from HTML
    // … rest of existing function unchanged …
}
```

Also guard `updateTabBarOverflowMask()`:
```js
export function updateTabBarOverflowMask() {
    const bar = document.getElementById('chat-tab-bar');
    if (!bar) return;
    bar.classList.toggle('no-overflow', bar.scrollWidth <= bar.clientWidth);
}
```

---

## Phase 2: SQLite Storage

**Files:** `Cargo.toml`, `src/chat_storage.rs` (new), `src/state.rs`, `src/main.rs`,
`src/web/api.rs`

Skip per-tab JSON sharding (old Option A) and go straight to SQLite:
- Cross-session search requires FTS5 — can't do that with flat files
- `rusqlite` bundled = no system dependency, works on all platforms
- Atomic transactions are safer than the tmp-rename trick
- WAL mode means reads never block writes

### 2.1 `Cargo.toml`

Under `[dependencies]`, add:
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
```

`bundled` compiles SQLite 3.x from source. Adds ~10 s to a clean release build; no-op on rebuilds.

### 2.2 Schema

Full DDL (embed as a constant `SCHEMA_SQL` in `src/chat_storage.rs`):

```sql
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS tabs (
    id                     TEXT    PRIMARY KEY,
    name                   TEXT    NOT NULL,
    system_prompt          TEXT    NOT NULL DEFAULT '',
    ai_name                TEXT,
    user_name              TEXT,
    explicit_level         INTEGER NOT NULL DEFAULT 0,
    active_template_id     TEXT,
    auto_compact           INTEGER NOT NULL DEFAULT 1,
    auto_compact_summarize INTEGER NOT NULL DEFAULT 0,
    compact_mode           TEXT    NOT NULL DEFAULT 'percent',
    compact_threshold      REAL    NOT NULL DEFAULT 0.8,
    model_params           TEXT    NOT NULL DEFAULT '{}',
    context_notes          TEXT    NOT NULL DEFAULT '[]',
    sidebar_width          INTEGER NOT NULL DEFAULT 280,
    tab_order              INTEGER NOT NULL DEFAULT 0,
    pinned                 INTEGER NOT NULL DEFAULT 0,
    last_ctx_pct           REAL,
    total_input_tokens     INTEGER NOT NULL DEFAULT 0,
    total_output_tokens    INTEGER NOT NULL DEFAULT 0,
    created_at             INTEGER NOT NULL,
    updated_at             INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    tab_id                    TEXT    NOT NULL REFERENCES tabs(id) ON DELETE CASCADE,
    role                      TEXT    NOT NULL CHECK(role IN ('user','assistant','system')),
    content                   TEXT    NOT NULL,
    timestamp_ms              INTEGER NOT NULL DEFAULT 0,
    input_tokens              INTEGER,
    output_tokens             INTEGER,
    cumulative_input_tokens   INTEGER,
    cumulative_output_tokens  INTEGER,
    compaction_marker         INTEGER NOT NULL DEFAULT 0,
    variants                  TEXT,     -- JSON array (_variants); NULL if none
    variant_index             INTEGER,  -- _variantIndex
    seq                       INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_tab ON messages(tab_id, seq);

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    content,
    content='messages',
    content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;
CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content)
        VALUES ('delete', old.id, old.content);
END;
CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content)
        VALUES ('delete', old.id, old.content);
    INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;
```

### 2.3 `src/chat_storage.rs` — full module

```rust
use std::path::PathBuf;
use anyhow::{Result, Context};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

const SCHEMA_SQL: &str = include_str!("../migrations/chat_schema.sql");
// OR embed inline as a const &str — either approach works.

// ── Data types ────────────────────────────────────────────────────────────────

/// Lightweight row returned by list_tabs — no messages loaded.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TabMeta {
    pub id: String,
    pub name: String,
    pub explicit_level: u8,
    pub active_template_id: Option<String>,
    pub pinned: bool,
    pub tab_order: i64,
    pub last_ctx_pct: Option<f64>,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub message_count: i64,   // COUNT(*) from messages
    pub created_at: i64,
    pub updated_at: i64,
}

/// Full tab row including metadata fields for save/load.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatTabRow {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
    pub ai_name: Option<String>,
    pub user_name: Option<String>,
    pub explicit_level: u8,
    pub active_template_id: Option<String>,
    pub auto_compact: bool,
    pub auto_compact_summarize: bool,
    pub compact_mode: String,
    pub compact_threshold: f64,
    pub model_params: serde_json::Value,
    pub context_notes: serde_json::Value,
    pub sidebar_width: u32,
    pub tab_order: i64,
    pub pinned: bool,
    pub last_ctx_pct: Option<f64>,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub messages: Vec<MessageRow>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageRow {
    #[serde(default)]
    pub id: i64,
    pub tab_id: String,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub timestamp_ms: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cumulative_input_tokens: Option<i64>,
    pub cumulative_output_tokens: Option<i64>,
    #[serde(default)]
    pub compaction_marker: bool,
    pub variants: Option<serde_json::Value>,
    pub variant_index: Option<i64>,
    pub seq: i64,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub tab_id: String,
    pub tab_name: String,
    pub message_id: i64,
    pub role: String,
    pub snippet: String,
    pub timestamp_ms: Option<i64>,
}

// ── Storage ───────────────────────────────────────────────────────────────────

pub struct ChatStorage {
    conn: std::sync::Mutex<Connection>,
}

impl ChatStorage {
    pub fn open(db_path: &PathBuf) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("opening chat.db at {}", db_path.display()))?;
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(Self { conn: std::sync::Mutex::new(conn) })
    }

    // ── Migration ─────────────────────────────────────────────────────────────

    /// Import legacy chat-tabs.json. Renames it to .bak when done.
    /// No-op if legacy file doesn't exist (already migrated).
    pub fn migrate_from_legacy(&self, legacy_path: &PathBuf) -> Result<()> {
        if !legacy_path.exists() { return Ok(()); }
        let raw = std::fs::read_to_string(legacy_path)?;
        let tabs: Vec<serde_json::Value> = serde_json::from_str(&raw)
            .context("parsing legacy chat-tabs.json")?;

        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;

        for (order, tab) in tabs.iter().enumerate() {
            let id = tab["id"].as_str().unwrap_or_default().to_string();
            if id.is_empty() { continue; }

            tx.execute(
                "INSERT OR REPLACE INTO tabs (
                    id, name, system_prompt, ai_name, user_name,
                    explicit_level, active_template_id,
                    auto_compact, auto_compact_summarize, compact_mode, compact_threshold,
                    model_params, context_notes, sidebar_width,
                    tab_order, pinned, last_ctx_pct,
                    total_input_tokens, total_output_tokens,
                    created_at, updated_at
                ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
                params![
                    id,
                    tab["name"].as_str().unwrap_or("Untitled"),
                    tab["system_prompt"].as_str().unwrap_or(""),
                    tab["ai_name"].as_str(),
                    tab["user_name"].as_str(),
                    tab["explicit_level"].as_i64().or_else(|| {
                        tab["explicit_mode"].as_bool().map(|b| if b { 1 } else { 0 })
                    }).unwrap_or(0) as i64,
                    tab["active_template_id"].as_str(),
                    tab["auto_compact"].as_bool().unwrap_or(true) as i64,
                    tab["auto_compact_summarize"].as_bool().unwrap_or(false) as i64,
                    tab["compact_mode"].as_str().unwrap_or("percent"),
                    tab["compact_threshold"].as_f64().unwrap_or(0.8),
                    tab["model_params"].to_string(),
                    tab["context_notes"].to_string(),
                    tab["sidebar_width"].as_i64().unwrap_or(280),
                    order as i64,
                    tab["pinned"].as_bool().unwrap_or(false) as i64,
                    tab["lastCtxPct"].as_f64(),
                    tab["totalInputTokens"].as_i64().unwrap_or(0),
                    tab["totalOutputTokens"].as_i64().unwrap_or(0),
                    tab["created_at"].as_i64().unwrap_or(0),
                    tab["updated_at"].as_i64().unwrap_or(0),
                ],
            )?;

            if let Some(msgs) = tab["messages"].as_array() {
                for (seq, msg) in msgs.iter().enumerate() {
                    tx.execute(
                        "INSERT INTO messages (tab_id, role, content, timestamp_ms,
                             input_tokens, output_tokens, compaction_marker, seq)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                        params![
                            id,
                            msg["role"].as_str().unwrap_or("user"),
                            msg["content"].as_str().unwrap_or(""),
                            msg["timestamp_ms"].as_i64().unwrap_or(0),
                            msg["input_tokens"].as_i64(),
                            msg["output_tokens"].as_i64(),
                            msg["compaction_marker"].as_bool().unwrap_or(false) as i64,
                            seq as i64,
                        ],
                    )?;
                }
            }
        }
        tx.commit()?;
        std::fs::rename(legacy_path, legacy_path.with_extension("json.bak"))?;
        Ok(())
    }

    // ── Tab CRUD ──────────────────────────────────────────────────────────────

    pub fn list_tabs(&self) -> Result<Vec<TabMeta>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT t.id, t.name, t.explicit_level, t.active_template_id,
                    t.pinned, t.tab_order, t.last_ctx_pct,
                    t.total_input_tokens, t.total_output_tokens,
                    COUNT(m.id) as message_count,
                    t.created_at, t.updated_at
             FROM tabs t
             LEFT JOIN messages m ON m.tab_id = t.id AND m.compaction_marker = 0
             GROUP BY t.id
             ORDER BY t.tab_order ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TabMeta {
                id:                   row.get(0)?,
                name:                 row.get(1)?,
                explicit_level:       row.get::<_, i64>(2)? as u8,
                active_template_id:   row.get(3)?,
                pinned:               row.get::<_, i64>(4)? != 0,
                tab_order:            row.get(5)?,
                last_ctx_pct:         row.get(6)?,
                total_input_tokens:   row.get(7)?,
                total_output_tokens:  row.get(8)?,
                message_count:        row.get(9)?,
                created_at:           row.get(10)?,
                updated_at:           row.get(11)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    pub fn get_tab(&self, id: &str) -> Result<ChatTabRow> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, system_prompt, ai_name, user_name,
                    explicit_level, active_template_id,
                    auto_compact, auto_compact_summarize, compact_mode, compact_threshold,
                    model_params, context_notes, sidebar_width,
                    tab_order, pinned, last_ctx_pct,
                    total_input_tokens, total_output_tokens,
                    created_at, updated_at
             FROM tabs WHERE id = ?1"
        )?;
        let mut tab = stmt.query_row(params![id], |row| {
            Ok(ChatTabRow {
                id:                    row.get(0)?,
                name:                  row.get(1)?,
                system_prompt:         row.get(2)?,
                ai_name:               row.get(3)?,
                user_name:             row.get(4)?,
                explicit_level:        row.get::<_, i64>(5)? as u8,
                active_template_id:    row.get(6)?,
                auto_compact:          row.get::<_, i64>(7)? != 0,
                auto_compact_summarize: row.get::<_, i64>(8)? != 0,
                compact_mode:          row.get(9)?,
                compact_threshold:     row.get(10)?,
                model_params:          serde_json::from_str(&row.get::<_, String>(11)?).unwrap_or_default(),
                context_notes:         serde_json::from_str(&row.get::<_, String>(12)?).unwrap_or_default(),
                sidebar_width:         row.get::<_, i64>(13)? as u32,
                tab_order:             row.get(14)?,
                pinned:                row.get::<_, i64>(15)? != 0,
                last_ctx_pct:          row.get(16)?,
                total_input_tokens:    row.get(17)?,
                total_output_tokens:   row.get(18)?,
                created_at:            row.get(19)?,
                updated_at:            row.get(20)?,
                messages:              vec![],
            })
        })?;

        tab.messages = self._load_messages_locked(&conn, id)?;
        Ok(tab)
    }

    pub fn create_tab(&self, tab: &ChatTabRow) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO tabs (id, name, system_prompt, ai_name, user_name,
                 explicit_level, active_template_id,
                 auto_compact, auto_compact_summarize, compact_mode, compact_threshold,
                 model_params, context_notes, sidebar_width,
                 tab_order, pinned, last_ctx_pct,
                 total_input_tokens, total_output_tokens,
                 created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
            params![
                tab.id, tab.name, tab.system_prompt, tab.ai_name, tab.user_name,
                tab.explicit_level as i64, tab.active_template_id,
                tab.auto_compact as i64, tab.auto_compact_summarize as i64,
                tab.compact_mode, tab.compact_threshold,
                serde_json::to_string(&tab.model_params)?,
                serde_json::to_string(&tab.context_notes)?,
                tab.sidebar_width as i64,
                tab.tab_order, tab.pinned as i64, tab.last_ctx_pct,
                tab.total_input_tokens, tab.total_output_tokens,
                tab.created_at, tab.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_tab_meta(&self, tab: &ChatTabRow) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tabs SET
                name=?2, system_prompt=?3, ai_name=?4, user_name=?5,
                explicit_level=?6, active_template_id=?7,
                auto_compact=?8, auto_compact_summarize=?9, compact_mode=?10, compact_threshold=?11,
                model_params=?12, context_notes=?13, sidebar_width=?14,
                pinned=?15, last_ctx_pct=?16,
                total_input_tokens=?17, total_output_tokens=?18,
                updated_at=?19
             WHERE id=?1",
            params![
                tab.id, tab.name, tab.system_prompt, tab.ai_name, tab.user_name,
                tab.explicit_level as i64, tab.active_template_id,
                tab.auto_compact as i64, tab.auto_compact_summarize as i64,
                tab.compact_mode, tab.compact_threshold,
                serde_json::to_string(&tab.model_params)?,
                serde_json::to_string(&tab.context_notes)?,
                tab.sidebar_width as i64,
                tab.pinned as i64, tab.last_ctx_pct,
                tab.total_input_tokens, tab.total_output_tokens,
                tab.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete_tab(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM tabs WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn reorder_tabs(&self, ordered_ids: &[String]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        for (i, id) in ordered_ids.iter().enumerate() {
            tx.execute("UPDATE tabs SET tab_order = ?1 WHERE id = ?2", params![i as i64, id])?;
        }
        tx.commit()?;
        Ok(())
    }

    // ── Message CRUD ──────────────────────────────────────────────────────────

    fn _load_messages_locked(&self, conn: &Connection, tab_id: &str) -> Result<Vec<MessageRow>> {
        let mut stmt = conn.prepare(
            "SELECT id, tab_id, role, content, timestamp_ms,
                    input_tokens, output_tokens,
                    cumulative_input_tokens, cumulative_output_tokens,
                    compaction_marker, variants, variant_index, seq
             FROM messages WHERE tab_id = ?1 ORDER BY seq"
        )?;
        let rows = stmt.query_map(params![tab_id], |row| {
            Ok(MessageRow {
                id:                      row.get(0)?,
                tab_id:                  row.get(1)?,
                role:                    row.get(2)?,
                content:                 row.get(3)?,
                timestamp_ms:            row.get(4)?,
                input_tokens:            row.get(5)?,
                output_tokens:           row.get(6)?,
                cumulative_input_tokens: row.get(7)?,
                cumulative_output_tokens:row.get(8)?,
                compaction_marker:       row.get::<_, i64>(9)? != 0,
                variants:                row.get::<_, Option<String>>(10)?.and_then(|s| serde_json::from_str(&s).ok()),
                variant_index:           row.get(11)?,
                seq:                     row.get(12)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    pub fn append_message(&self, msg: &MessageRow) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO messages (tab_id, role, content, timestamp_ms,
                 input_tokens, output_tokens,
                 cumulative_input_tokens, cumulative_output_tokens,
                 compaction_marker, variants, variant_index, seq)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,
                 COALESCE((SELECT MAX(seq)+1 FROM messages WHERE tab_id=?1), 0))",
            params![
                msg.tab_id, msg.role, msg.content, msg.timestamp_ms,
                msg.input_tokens, msg.output_tokens,
                msg.cumulative_input_tokens, msg.cumulative_output_tokens,
                msg.compaction_marker as i64,
                msg.variants.as_ref().map(|v| v.to_string()),
                msg.variant_index,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Replace all messages for a tab (used during compaction).
    pub fn replace_messages(&self, tab_id: &str, messages: &[MessageRow]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM messages WHERE tab_id = ?1", params![tab_id])?;
        for (seq, msg) in messages.iter().enumerate() {
            tx.execute(
                "INSERT INTO messages (tab_id, role, content, timestamp_ms,
                     input_tokens, output_tokens, compaction_marker, seq)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    tab_id, msg.role, msg.content, msg.timestamp_ms,
                    msg.input_tokens, msg.output_tokens,
                    msg.compaction_marker as i64, seq as i64,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // ── Full-text search ──────────────────────────────────────────────────────

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT t.id, t.name, m.id, m.role,
                    snippet(messages_fts, 0, '<mark>', '</mark>', '…', 24),
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
                tab_id:       row.get(0)?,
                tab_name:     row.get(1)?,
                message_id:   row.get(2)?,
                role:         row.get(3)?,
                snippet:      row.get(4)?,
                timestamp_ms: row.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }
}
```

### 2.4 `src/state.rs` — add `chat_storage`

In the `AppState` struct (currently line 357), add one field:

```rust
pub struct AppState {
    // … all existing fields unchanged …
    pub chat_storage: Arc<crate::chat_storage::ChatStorage>,
}
```

Update `AppState::new()` to accept a `ChatStorage` argument:

```rust
impl AppState {
    pub fn new(
        presets: Vec<ModelPreset>,
        paths: AppPaths,
        gpu_env: GpuEnv,
        ui_settings: UiSettings,
        chat_storage: Arc<crate::chat_storage::ChatStorage>,   // ← ADD
    ) -> Self {
        // … all existing field inits unchanged …
        Self {
            // … existing fields …
            chat_storage,
        }
    }
}
```

Update all `AppState::new(…)` call sites (including test helpers in the same file) to pass
`chat_storage`. The test helpers can use a temp file:
```rust
let cs = Arc::new(ChatStorage::open(&PathBuf::from(":memory:")).unwrap());
```

### 2.5 `src/main.rs` — initialize and migrate

Before the `AppState::new()` call:

```rust
use crate::chat_storage::ChatStorage;

// Construct chat storage (runs schema migrations on open)
let chat_db_path = config_dir.join("chat.db");
let chat_storage = Arc::new(
    ChatStorage::open(&chat_db_path)
        .context("opening chat.db")?
);

// One-time migration from legacy chat-tabs.json
let legacy = config_dir.join("chat-tabs.json");
if let Err(e) = chat_storage.migrate_from_legacy(&legacy) {
    eprintln!("[warn] chat legacy migration failed: {e}");
}

let state = AppState::new(initial_presets, paths, gpu_env, ui_settings, chat_storage);
```

### 2.6 `src/web/api.rs` — replace 2 endpoints with 9

The current `api_get_chat_tabs` (line 1806) and `api_put_chat_tabs` (line 1832) read/write the flat
JSON file via `CONFIG_DIR` / `chat_tabs_path()`. These are replaced entirely.

**Route registration** — in the route construction block (currently lines 519–569), replace:
```rust
let get_chat_tabs = api_get_chat_tabs();
let put_chat_tabs = api_put_chat_tabs();
```
with the nine new route builders (each takes `Arc<ChatStorage>` as a cloned argument).

**New handler pattern** — all handlers take the shared `ChatStorage`. Since these replace the
stateless file-I/O handlers, they need `Arc<ChatStorage>` passed via closure capture or a warp
`and_then` filter. The cleanest approach that fits the existing warp style:

```rust
fn with_chat_storage(
    storage: Arc<ChatStorage>,
) -> impl Filter<Extract = (Arc<ChatStorage>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || storage.clone())
}
```

Then each handler:

```rust
// GET /api/chat/tabs — metadata only (no messages)
fn api_list_tabs(
    storage: Arc<ChatStorage>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs")
        .and(warp::get())
        .and(with_chat_storage(storage))
        .and_then(|store: Arc<ChatStorage>| async move {
            match store.list_tabs() {
                Ok(tabs) => Ok::<_, warp::Rejection>(warp::reply::json(&tabs)),
                Err(e)   => {
                    eprintln!("list_tabs error: {e}");
                    Ok(warp::reply::json(&Vec::<TabMeta>::new()))
                }
            }
        })
}

// POST /api/chat/tabs — create new tab
fn api_create_tab(
    storage: Arc<ChatStorage>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs")
        .and(warp::post())
        .and(warp::body::json::<ChatTabRow>())
        .and(with_chat_storage(storage))
        .and_then(|mut tab: ChatTabRow, store: Arc<ChatStorage>| async move {
            if tab.id.is_empty() { tab.id = uuid::Uuid::new_v4().to_string(); }
            tab.created_at = chrono::Utc::now().timestamp_millis();
            tab.updated_at = tab.created_at;
            match store.create_tab(&tab) {
                Ok(_)  => Ok::<_, warp::Rejection>(warp::reply::json(&tab)),
                Err(e) => Ok(warp::reply::json(&serde_json::json!({"ok":false,"error":e.to_string()}))),
            }
        })
}

// GET /api/chat/tabs/:id — full tab with messages
fn api_get_tab(
    storage: Arc<ChatStorage>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String)
        .and(warp::get())
        .and(with_chat_storage(storage))
        .and_then(|id: String, store: Arc<ChatStorage>| async move {
            match store.get_tab(&id) {
                Ok(tab) => Ok::<_, warp::Rejection>(warp::reply::json(&tab)),
                Err(e)  => Ok(warp::reply::json(&serde_json::json!({"ok":false,"error":e.to_string()}))),
            }
        })
}

// PUT /api/chat/tabs/:id — full save (meta + replace messages)
fn api_put_tab(
    storage: Arc<ChatStorage>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String)
        .and(warp::put())
        .and(warp::body::json::<ChatTabRow>())
        .and(with_chat_storage(storage))
        .and_then(|id: String, mut tab: ChatTabRow, store: Arc<ChatStorage>| async move {
            tab.id = id;
            tab.updated_at = chrono::Utc::now().timestamp_millis();
            let messages = std::mem::take(&mut tab.messages);
            let msg_rows: Vec<crate::chat_storage::MessageRow> = messages.into_iter()
                .enumerate()
                .map(|(seq, m)| crate::chat_storage::MessageRow { seq: seq as i64, tab_id: tab.id.clone(), ..m })
                .collect();
            let result = store.update_tab_meta(&tab)
                .and_then(|_| store.replace_messages(&tab.id, &msg_rows));
            match result {
                Ok(_)  => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok":true}))),
                Err(e) => Ok(warp::reply::json(&serde_json::json!({"ok":false,"error":e.to_string()}))),
            }
        })
}

// DELETE /api/chat/tabs/:id
fn api_delete_tab(
    storage: Arc<ChatStorage>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String)
        .and(warp::delete())
        .and(with_chat_storage(storage))
        .and_then(|id: String, store: Arc<ChatStorage>| async move {
            match store.delete_tab(&id) {
                Ok(_)  => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok":true}))),
                Err(e) => Ok(warp::reply::json(&serde_json::json!({"ok":false,"error":e.to_string()}))),
            }
        })
}

// PATCH /api/chat/tabs/:id/meta — metadata only, no messages
fn api_patch_tab_meta(
    storage: Arc<ChatStorage>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String / "meta")
        .and(warp::patch())
        .and(warp::body::json::<ChatTabRow>())
        .and(with_chat_storage(storage))
        .and_then(|id: String, mut tab: ChatTabRow, store: Arc<ChatStorage>| async move {
            tab.id = id;
            tab.updated_at = chrono::Utc::now().timestamp_millis();
            match store.update_tab_meta(&tab) {
                Ok(_)  => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok":true}))),
                Err(e) => Ok(warp::reply::json(&serde_json::json!({"ok":false,"error":e.to_string()}))),
            }
        })
}

// POST /api/chat/tabs/:id/messages — append one or more messages
fn api_append_messages(
    storage: Arc<ChatStorage>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String / "messages")
        .and(warp::post())
        .and(warp::body::json::<serde_json::Value>())
        .and(with_chat_storage(storage))
        .and_then(|id: String, body: serde_json::Value, store: Arc<ChatStorage>| async move {
            let msgs = body["messages"].as_array().cloned().unwrap_or_default();
            let mut last_id = 0i64;
            for msg_val in msgs {
                let msg: crate::chat_storage::MessageRow = serde_json::from_value(msg_val)
                    .unwrap_or_else(|_| crate::chat_storage::MessageRow {
                        tab_id: id.clone(), role: "user".into(), content: "".into(),
                        id: 0, timestamp_ms: 0, input_tokens: None, output_tokens: None,
                        cumulative_input_tokens: None, cumulative_output_tokens: None,
                        compaction_marker: false, variants: None, variant_index: None, seq: 0,
                    });
                let mut m = msg;
                m.tab_id = id.clone();
                match store.append_message(&m) {
                    Ok(row_id) => last_id = row_id,
                    Err(e) => eprintln!("append_message error: {e}"),
                }
            }
            Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok":true,"last_id":last_id})))
        })
}

// PATCH /api/chat/tabs/order
fn api_reorder_tabs(
    storage: Arc<ChatStorage>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / "order")
        .and(warp::patch())
        .and(warp::body::json::<serde_json::Value>())
        .and(with_chat_storage(storage))
        .and_then(|body: serde_json::Value, store: Arc<ChatStorage>| async move {
            let ids: Vec<String> = body["tab_order"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            match store.reorder_tabs(&ids) {
                Ok(_)  => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({"ok":true}))),
                Err(e) => Ok(warp::reply::json(&serde_json::json!({"ok":false,"error":e.to_string()}))),
            }
        })
}

// GET /api/chat/search?q=…&limit=50
fn api_chat_search(
    storage: Arc<ChatStorage>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    #[derive(serde::Deserialize)]
    struct SearchParams { q: String, #[serde(default = "default_limit")] limit: usize }
    fn default_limit() -> usize { 50 }

    warp::path!("api" / "chat" / "search")
        .and(warp::get())
        .and(warp::query::<SearchParams>())
        .and(with_chat_storage(storage))
        .and_then(|p: SearchParams, store: Arc<ChatStorage>| async move {
            match store.search(&p.q, p.limit) {
                Ok(results) => Ok::<_, warp::Rejection>(warp::reply::json(&results)),
                Err(e) => {
                    eprintln!("search error: {e}");
                    Ok(warp::reply::json(&Vec::<crate::chat_storage::SearchResult>::new()))
                }
            }
        })
}
```

**Route table summary:**

| Method | Path | Handler |
|--------|------|---------|
| GET | `/api/chat/tabs` | `api_list_tabs` — `Vec<TabMeta>`, no messages |
| POST | `/api/chat/tabs` | `api_create_tab` — returns full `ChatTabRow` with assigned id |
| GET | `/api/chat/tabs/:id` | `api_get_tab` — full tab + messages |
| PUT | `/api/chat/tabs/:id` | `api_put_tab` — replace meta + all messages |
| PATCH | `/api/chat/tabs/:id/meta` | `api_patch_tab_meta` — meta only |
| POST | `/api/chat/tabs/:id/messages` | `api_append_messages` — append `{ messages: [] }` |
| DELETE | `/api/chat/tabs/:id` | `api_delete_tab` — cascade deletes messages |
| PATCH | `/api/chat/tabs/order` | `api_reorder_tabs` — `{ tab_order: ["id1",…] }` |
| GET | `/api/chat/search` | `api_chat_search` — `?q=…&limit=50` |

---

## Phase 3: Frontend API Adapters

### 3.1 `static/js/features/chat-state.js` — lazy load & per-tab persist

Replace `initChatTabs`:

```js
export async function initChatTabs() {
    try {
        const resp = await fetch('/api/chat/tabs');
        const metas = await resp.json();   // Vec<TabMeta>
        chat.tabs = metas.map(m => ({
            ...m,
            messages: null,
            _loaded: false,
            model_params: m.model_params || defaultModelParams(),
            context_notes: m.context_notes || [],
        }));
    } catch {
        chat.tabs = [];
    }

    if (chat.tabs.length === 0) {
        await addChatTab();
        return;
    }

    chat.activeTabId = chat.tabs[0].id;
    await _loadTabMessages(chat.activeTabId);

    chatViewBindings.renderChatSessionsSidebar?.();
    chatViewBindings.renderChatMessages?.();
    chatViewBindings.loadChatNames?.();
    chatViewBindings.updateExplicitToggleUI?.();
    chatViewBindings.syncMessageLimitInput?.();
    chatViewBindings.syncCompactSettingsUI?.(activeChatTab());
    chatViewBindings.refreshChatTelemetry?.();
    refreshTopCockpit();
}
```

Add `_loadTabMessages` (private helper):

```js
async function _loadTabMessages(id) {
    const tab = chat.tabs.find(t => t.id === id);
    if (!tab || tab._loaded) return;
    try {
        const resp = await fetch(`/api/chat/tabs/${id}`);
        const full = await resp.json();
        Object.assign(tab, full);
        tab._loaded = true;
    } catch (e) {
        console.error('[chat-state] _loadTabMessages failed:', e);
        tab.messages = [];
        tab._loaded = true;
    }
}
```

Update `switchChatTab` to lazy-load before rendering:

```js
export async function switchChatTab(id) {
    if (chat.busy) return;
    chat.activeTabId = id;
    await _loadTabMessages(id);
    chatViewBindings.renderChatTabs?.();
    chatViewBindings.renderChatSessionsSidebar?.();
    chatViewBindings.renderChatMessages?.();
    window.renderPersonaStrip?.();
    chatViewBindings.loadChatNames?.();
    chatViewBindings.updateExplicitToggleUI?.();
    chatViewBindings.syncMessageLimitInput?.();
    chatViewBindings.syncCompactSettingsUI?.(activeChatTab());
    chatViewBindings.updateCtxPressureBar?.(0);
    chatViewBindings.refreshChatTelemetry?.();
    refreshTopCockpit();
}
```

Note: `switchChatTab` becomes `async`. Update all callers to `await` or fire-and-forget as needed.

Replace `addChatTab` with a POST-based version:

```js
export async function addChatTab() {
    const tab = newChatTab(`Chat ${chat.tabs.length + 1}`);
    try {
        const resp = await fetch('/api/chat/tabs', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(tab),
        });
        const created = await resp.json();
        Object.assign(tab, created);
    } catch (e) {
        console.error('[chat-state] addChatTab POST failed:', e);
    }
    tab._loaded = true;
    chat.tabs.push(tab);
    await switchChatTab(tab.id);
    chatViewBindings.renderChatSessionsSidebar?.();
}
```

Replace `closeChatTab` with a DELETE-based version:

```js
export async function closeChatTab(id) {
    const tabIdx = chat.tabs.findIndex(t => t.id === id);
    if (tabIdx === -1) return;
    if (chat.tabs.length === 1) return;

    const [tab] = chat.tabs.splice(tabIdx, 1);
    chat.tabTrash.push({ tab, trashedAt: Date.now() });

    if (chat.activeTabId === id) {
        chat.activeTabId = chat.tabs[chat.tabs.length - 1]?.id ?? null;
        if (chat.activeTabId) await _loadTabMessages(chat.activeTabId);
    }

    chatViewBindings.renderChatTabs?.();
    chatViewBindings.renderChatSessionsSidebar?.();
    chatViewBindings.renderChatMessages?.();

    fetch(`/api/chat/tabs/${id}`, { method: 'DELETE' }).catch(e =>
        console.error('[chat-state] DELETE tab failed:', e));

    showToastWithActions('Tab deleted', 'info', '', [{
        id: 'undo', label: 'Undo', primary: true,
        handler: () => restoreTabFromTrash(id),
    }]);
}
```

Replace `scheduleChatPersist` / `persistChatTabs` with per-tab versions:

```js
export function scheduleChatPersist(tab) {
    // Fallback: if called without argument (old callers), persist active tab
    const t = tab || activeChatTab();
    if (!t) return;
    t._dirty = true;
    clearTimeout(t._persistTimer);
    t._persistTimer = setTimeout(() => _persistTab(t), 500);
}

async function _persistTab(tab) {
    if (!tab._dirty) return;
    tab._dirty = false;
    try {
        const body = normalizeTabForSave(tab);
        await fetch(`/api/chat/tabs/${tab.id}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body),
        });
    } catch (e) { console.error('[chat-state] PUT tab failed:', e); }
}
```

Update tab-order persistence:

```js
export async function persistTabOrder() {
    await fetch('/api/chat/tabs/order', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tab_order: chat.tabs.map(t => t.id) }),
    }).catch(e => console.error('[chat-state] PATCH order failed:', e));
}
```

Call `persistTabOrder()` from the drag-drop handler in `chat-sessions-sidebar.js` (Phase 1 sidebar
drag-to-reorder, if implemented) or from any existing reorder handler.

### 3.2 `static/js/features/chat-render.js` — `data-msg-id` attribute

The message renderer currently sets `data-msg-idx` (array index within `tab.messages`) on each
`.chat-message` element. This attribute is used in 9+ places — edit, copy, regenerate, fix-last,
variant cycling — and **must not be removed or renamed**. It will continue to work after Phase 3.

**Add `data-msg-id` as a second, separate attribute** alongside the existing `data-msg-idx`. This
is the SQLite `messages.id` rowid, returned by the Phase 2 API on `GET /api/chat/tabs/:id`. It is
only needed for search result jump-to-message:

```js
// KEEP the existing line:
wrapper.dataset.msgIdx = idx;

// ADD this line immediately after (msg.db_id populated by Phase 2 API response):
wrapper.dataset.msgId = msg.db_id ?? '';
```

The `msg.db_id` field will be `undefined` until Phase 2 is live, so `data-msg-id=""` is a safe
no-op placeholder during Phase 1.

Remove `updateTabBarOverflowMask()` entirely once Phase 1 tab bar removal is confirmed stable.

### 3.3 `static/js/features/chat-search.js` (new file)

The existing Option B search spec from the May 3 plan is verbatim correct for Phase 3. The search
input in the session panel `#csp-search` handles name filtering (Phase 1, inline in
`chat-sessions-sidebar.js`). Full FTS5 cross-session search is a separate flow triggered by a
dedicated search button — this second feature is `chat-search.js`:

Key exports: `initChatSearch()`, `openSearch()`, `closeSearch()`.

Search results replace the session list while active. Clicking a result calls
`switchChatTab(tabId)`, waits for messages to load, then scrolls to the element with
`data-msg-id="${messageId}"` and adds `chat-msg-highlight` class (remove after 2 s).

The 300 ms debounce fires `GET /api/chat/search?q=…` and renders result cards with `<mark>`-tagged
snippets (FTS5 `snippet()` output). The snippet content is trusted because it comes from the SQLite
FTS engine's own snippet wrapper; actual message content is stored verbatim (HTML-escaped at render
time, never at storage time).

---

## Migration Safety

- `chat-tabs.json` is renamed to `chat-tabs.json.bak` after successful import — user can manually
  restore by renaming back.
- If `chat.db` already exists AND `.json` is absent (re-run after crash), `migrate_from_legacy` is
  a no-op.
- WAL mode: concurrent reads never block during a write (important: the Rust server may receive tab
  API calls while a background poll task reads from SQLite).
- The `REFERENCES tabs(id) ON DELETE CASCADE` constraint means deleting a tab also deletes all its
  messages in one statement — no orphan cleanup needed.

---

## Implementation Order

### Week 1 — UX first

| Day | Work | Files |
|-----|------|-------|
| 1 | Phase 0: remove Sessions+Models nav buttons; clean `switchTab` | `index.html`, `nav.js` |
| 1 | Phase 1a: HTML — add `#chat-sessions-panel`, wrap chat content in `.chat-main-area`, remove `#chat-tab-bar` | `index.html` |
| 2 | Phase 1b: CSS — `#page-chat` flex-row, session panel width animation, `.csp-*` styles | `layout.css`, `chat.css` |
| 3 | Phase 1c: `chat-sessions-sidebar.js` — full implementation | new file |
| 3 | Phase 1d: Wire sidebar into `nav.js`, `bootstrap.js`, `chat-state.js`, `chat-render.js` | 4 files |

End of Week 1: sidebar renders from existing `chat.tabs` data (still backed by old `chat-tabs.json`
API). All session management works in new sidebar. Horizontal tab bar is gone.

### Week 2 — Storage

| Day | Work | Files |
|-----|------|-------|
| 4 | Phase 2a: `src/chat_storage.rs` — schema, open, migrate, CRUD | new file |
| 4 | Phase 2b: `Cargo.toml` + integrate `ChatStorage` into `AppState`, run migration | `Cargo.toml`, `state.rs`, `main.rs` |
| 5 | Phase 2c: All 9 new API endpoints | `api.rs` |
| 6 | Phase 3a: `chat-state.js` lazy load, per-tab persist, new API calls | `chat-state.js` |
| 7 | Phase 3b: `chat-search.js` FTS5 search UI | new file |

---

## File Impact Summary

### New files

| File | Purpose |
|------|---------|
| `src/chat_storage.rs` | SQLite storage layer |
| `static/js/features/chat-sessions-sidebar.js` | Session panel module |
| `static/js/features/chat-search.js` | FTS5 cross-session search |

### Modified files

| File | Change |
|------|--------|
| `static/index.html` | Remove Sessions/Models buttons; remove `#chat-tab-bar`; add `#chat-sessions-panel`; wrap chat content in `.chat-main-area` |
| `static/css/layout.css` | `#page-chat { flex-direction: row; }` + `.chat-sessions-panel` width-based slide animation + `.chat-main-area` |
| `static/css/chat.css` | New `.csp-*` rules section (old `.chat-tab-*` rules stay until Phase 3 cleanup) |
| `static/js/features/nav.js` | `switchTab` — remove dead `models`/`sessions` branches; add `showSessionPanel`/`hideSessionPanel` calls |
| `static/js/bootstrap.js` | Add `initChatSessionsSidebar()` call; remove trash-btn event bindings |
| `static/js/features/chat-state.js` | Add `renderChatSessionsSidebar` binding; update `addChatTab`, `closeChatTab`, `switchChatTab`, `scheduleChatPersist`, `initChatTabs` |
| `static/js/features/chat-render.js` | Guard `renderChatTabs()` + `updateTabBarOverflowMask()`; add `renderChatSessionsSidebar` to bindings; add `data-msg-id` attribute alongside existing `data-msg-idx` (do not remove `data-msg-idx`) |
| `src/web/api.rs` | Replace 2 flat-file endpoints with 9 SQLite-backed endpoints |
| `src/state.rs` | Add `chat_storage: Arc<ChatStorage>` to `AppState` |
| `src/main.rs` | Open `ChatStorage`, run migration, pass to `AppState::new` |
| `Cargo.toml` | Add `rusqlite = { version = "0.31", features = ["bundled"] }` |

### Deleted (Phase 3 cleanup)

| Symbol | In |
|--------|----|
| `renderChatTabs()` | `chat-render.js` — replaced by session sidebar |
| `updateTabBarOverflowMask()` | `chat-render.js` |
| All `.chat-tab`, `.chat-tab-bar`, `.chat-tab-*` CSS | `chat.css` |
| `GET /api/chat/tabs` full-array (old) | `api.rs` |
| `PUT /api/chat/tabs` full-array (old) | `api.rs` |
| `chat_tabs_path()` + `CONFIG_DIR` | `api.rs` (if nothing else uses them) |

---

## Risk & Mitigation

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| SQLite migration loses tabs | Low | `.json.bak` kept; migration is idempotent |
| Width-animation not smooth | Very Low | `width: 0 → 240px` + `overflow: hidden` always animates; DO NOT use `display: none → flex` |
| `switchChatTab` is now async — callers expect sync | Medium | Audit all callers; most fire-and-forget is fine; `chat.busy` guard prevents concurrent calls |
| Session panel layout breaks on narrow viewports | Medium | Test at 1280px; add `@media (max-width: 1100px) { .chat-sessions-panel.visible { width: 180px; } }` |
| `bundled` SQLite increases compile time | Low | ~10 s on clean build; CI caches deps |
| Drag-to-reorder in vertical list | Medium | Same JS drag API as horizontal tab bar, different axis — tested pattern |
| FTS5 snippet returns `<mark>` tags rendered via `innerHTML` | Accepted | Local-only app; message content is stored raw (escaped at render) |

---

## Design Tokens Used

All new components use existing tokens from `tokens.css`:
- `--color-primary` (indigo `#6366f1`), `--color-primary-light`
- `--color-info`, `--color-success`, `--color-warning`, `--color-error`
- `--surface-card-base`, `--surface-card-elevated`
- `--radius-sm` (8 px), `--radius-base`
- `--text-sm`, `--font-body`
- `--shadow-surface`
- `--color-text-primary`, `--color-text-secondary`, `--color-text-muted`
- `--border-subtle`

---

*Related: `docs/plans/20260510-explicit_persona_enhancements.md`, `docs/plans/20260508-chat-guided-generation-decisions.md`*
