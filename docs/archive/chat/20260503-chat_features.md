# Chat Feature Improvements

**Date:** 2026-05-03  
**Scope:** Export enhancements, pin/favorite tabs, message edit/regenerate gaps, persona system  
**Agent note:** Each section is self-contained and can be implemented independently.

---

## 1 — Export Improvements

### Current State

`exportChatTab()` and `importChatTab()` already exist in `static/js/features/chat-render.js`.

- **Export**: Markdown only. Downloads `<tab-name>.md` with messages formatted as `**You**\n\ncontent\n\n---\n\n**Assistant**\n\ncontent`.
- **Import**: Accepts `.json` (expects array with one tab object) or `.md` (parses the above format, appends messages to the active tab).
- **Entry point**: `chat-export-btn` button exists in `static/index.html` chat header. It is wired but only triggers Markdown export.

### What to Add

#### A. JSON Export

Add a second export format: a single-tab JSON file that round-trips through `importChatTab()` perfectly.

In `chat-render.js`, update `exportChatTab()` to accept a `format` argument:

```js
export function exportChatTab(format = 'md') {
    const tab = activeChatTab();
    if (!tab) return;

    if (format === 'json') {
        const data = JSON.stringify([normalizeTabForSave(tab)], null, 2);
        const blob = new Blob([data], { type: 'application/json' });
        const a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = `${tab.name.replace(/[^a-z0-9]/gi, '-').toLowerCase()}.json`;
        a.click();
        URL.revokeObjectURL(a.href);
        return;
    }

    // existing Markdown path unchanged
    const md = tab.messages
        .filter(m => m.role !== 'system' && !m.compaction_marker)
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

Import `normalizeTabForSave` from `chat-state.js` at the top of `chat-render.js` (it is already exported).

#### B. Export Format Picker

Replace the single `chat-export-btn` click handler with a small two-option dropdown. The simplest implementation: clicking the export button opens a tiny `<div class="chat-export-menu">` absolutely positioned below it with two items: "Save as Markdown" and "Save as JSON". Clicking either calls `exportChatTab('md')` or `exportChatTab('json')` and closes the menu. Clicking anywhere else dismisses it.

In `static/index.html`, add after `chat-export-btn`:
```html
<div id="chat-export-menu" class="chat-export-menu hidden">
  <button data-export-format="md">Save as Markdown</button>
  <button data-export-format="json">Save as JSON</button>
</div>
```

Wire in `chat-render.js` `initChat()`:
```js
document.getElementById('chat-export-btn')?.addEventListener('click', e => {
    e.stopPropagation();
    document.getElementById('chat-export-menu')?.classList.toggle('hidden');
});
document.getElementById('chat-export-menu')?.addEventListener('click', e => {
    const fmt = e.target.dataset.exportFormat;
    if (fmt) { exportChatTab(fmt); document.getElementById('chat-export-menu').classList.add('hidden'); }
});
document.addEventListener('click', () => document.getElementById('chat-export-menu')?.classList.add('hidden'));
```

CSS (add to `static/css/chat.css`):
```css
.chat-export-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 0;
    z-index: 200;
    min-width: 160px;
    box-shadow: 0 4px 12px rgba(0,0,0,0.3);
}
.chat-export-menu button {
    display: block;
    width: 100%;
    padding: 7px 14px;
    text-align: left;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 0.82rem;
    cursor: pointer;
}
.chat-export-menu button:hover { background: var(--surface-3); }
```

The parent button row element already needs `position: relative` — add it if not present.

---

## 2 — Pin / Favorite Tab

### Design

A tab can be "pinned." Pinned tabs:
- Always sort before unpinned tabs in the tab strip
- Show a small filled pin icon (or a subtle dot) in the tab
- Are excluded from any future "auto-close oldest tab" logic

Pins persist to disk with the tab. No server-side changes required in Option A (pin is just a field on the tab JSON). In Option B it is a column on the `tabs` table.

### Data Model

Add `pinned: false` to `newChatTab()` defaults in `chat-state.js`:

```js
export function newChatTab(name = 'New Chat') {
    return {
        // ...existing fields...
        pinned: false,
        // ...
    };
}
```

Add to `normalizeChatTab()`:
```js
pinned: tab.pinned ?? false,
```

### Toggle Function (chat-state.js)

```js
export function togglePinTab(id) {
    const tab = chat.tabs.find(t => t.id === id);
    if (!tab) return;
    tab.pinned = !tab.pinned;
    // Re-sort: pinned tabs first, then by their existing relative position
    const pinned = chat.tabs.filter(t => t.pinned);
    const unpinned = chat.tabs.filter(t => !t.pinned);
    chat.tabs = [...pinned, ...unpinned];
    chatViewBindings.renderChatTabs?.();
    scheduleChatPersist();
}
```

Export `togglePinTab` and add it to the public API.

### Drag-to-Reorder Interaction

Pinned tabs should only be reorderable among other pinned tabs; dragging a pinned tab into the unpinned section (or vice versa) should be a no-op. In the existing drag drop handler in `chat-render.js` (`renderChatTabs` drag events), add a guard:

```js
// in the dragover/drop handler, after resolving draggedTab and targetTab:
if (draggedTab.pinned !== targetTab.pinned) return;   // can't cross the pin boundary
```

### UI Changes (chat-render.js — renderChatTabs)

In the tab HTML template, add the pin button alongside the existing edit pencil icon:

```js
const pinnedIcon = tab.pinned
    ? `<svg class="chat-tab-pin-icon pinned" ...filled pin SVG...></svg>`
    : `<svg class="chat-tab-pin-icon" ...outline pin SVG...></svg>`;

// Add to tab innerHTML:
`<button class="chat-tab-pin-btn" data-tab-pin="${tab.id}" title="${tab.pinned ? 'Unpin' : 'Pin'}">${pinnedIcon}</button>`
```

Wire the click handler in the delegated event listener already in `initChatTabs` / `renderChatTabs`:
```js
if (e.target.closest('[data-tab-pin]')) {
    const id = e.target.closest('[data-tab-pin]').dataset.tabPin;
    togglePinTab(id);
    return;
}
```

CSS (add to `static/css/chat.css`):
```css
.chat-tab-pin-btn {
    background: none; border: none; padding: 0 2px; cursor: pointer;
    display: flex; align-items: center;
}
.chat-tab-pin-icon {
    width: 11px; height: 11px;
    opacity: 0;
    transition: opacity 0.15s;
    color: var(--text-muted);
}
.chat-tab:hover .chat-tab-pin-icon,
.chat-tab-pin-icon.pinned { opacity: 1; }
.chat-tab-pin-icon.pinned { color: var(--color-primary); }
```

Visual separator: after rendering all tabs, if there are both pinned and unpinned tabs, insert a 1 px vertical divider element between the two groups:
```js
const firstUnpinned = tabsEl.querySelector('.chat-tab:not(.chat-tab-pinned)');
if (firstUnpinned && tabsEl.querySelector('.chat-tab-pinned')) {
    const sep = document.createElement('div');
    sep.className = 'chat-tab-pin-sep';
    tabsEl.insertBefore(sep, firstUnpinned);
}
```

```css
.chat-tab-pin-sep {
    width: 1px; align-self: stretch; margin: 4px 2px;
    background: var(--border); flex-shrink: 0;
}
```

Add `.chat-tab-pinned` class to pinned tabs in `renderChatTabs` so the separator logic and drag guard can use it.

---

## 3 — Message Edit / Regenerate

### Current State (do not re-implement these)

The following already work correctly in `static/js/features/chat-render.js`:

| Feature | Status | Notes |
|---|---|---|
| Copy message | ✅ Working | Copies `.innerText` of message body |
| Regenerate last response | ✅ Working | `regenerateFromMessage()` — truncates to last user msg, re-sends |
| Edit any message (save in place) | ✅ Working | Inline textarea, Save/Cancel buttons |
| Edit last user message + Resend | ✅ Working | Shows "Resend" only when message is the final user turn |
| Delete message | ✅ Working | `deleteMessage()` with confirm dialog |
| Variant navigation (← →) | ✅ Working | `navigateVariant()`, `_variants` array, `_variantIndex` |
| Generate new variant | ✅ Working | Right arrow on last variant triggers regenerate |

### What Is Missing

#### A. Edit + Branch for Mid-Conversation User Messages

Currently, editing a user message that is **not** the last user turn shows only "Save" — it edits the content in place but does not re-send. This means the subsequent assistant messages now respond to a prompt they never saw, silently inconsistent.

**Fix**: Show a "Resend from here" button for **all** user messages (not just the last one). When clicked, truncate everything after that user message and re-send.

Change in `editMessageContent()` in `chat-render.js`:

```js
// Before: only shows Resend for last user message
const isLastUserMsg = msg.role === 'user' &&
    tab.messages.slice(msgIdx + 1).every(m => m.role !== 'user');

const resendBtn = isLastUserMsg
    ? `<button ...>Resend</button>`
    : '';

// After: show Resend for ALL user messages
const resendBtn = msg.role === 'user'
    ? `<button class="chat-edit-btn chat-edit-btn-resend" data-chat-edit="resend">Resend from here</button>`
    : '';
```

The `resendMessageEdit()` function already handles this correctly — it truncates `tab.messages` to `msgIdx + 1` and calls `sendChatResend`. No changes needed there.

Label the button "Resend from here" (not just "Resend") to make it clear that subsequent messages will be dropped.

#### B. Regenerate from Any Point

The current `regenerateFromMessage()` always finds `findLast('user')`, which is always the most recent user message regardless of which assistant message the button was clicked on. This means clicking Regenerate on an old assistant message regenerates from the wrong user turn.

**Fix**: Find the user message immediately preceding the clicked assistant message, not the last one globally.

```js
function regenerateFromMessage(btn) {
    const msgEl = btn.closest('.chat-message');
    const msgIdx = parseInt(msgEl.dataset.msgIdx);
    const tab = activeChatTab();
    if (!tab || isNaN(msgIdx)) return;

    const msg = tab.messages[msgIdx];
    if (!msg || msg.role !== 'assistant') return;

    // Find the user message immediately before this assistant message (not findLast globally)
    let userMsgIdx = -1;
    for (let i = msgIdx - 1; i >= 0; i--) {
        if (tab.messages[i].role === 'user') { userMsgIdx = i; break; }
    }
    if (userMsgIdx === -1) return;

    tab.messages = tab.messages.slice(0, userMsgIdx + 1);
    tab.updated_at = Date.now();
    scheduleChatPersist();

    getTransport()?.sendChatResend(tab);
}
```

Apply the same fix to `navigateVariant()` — the "generate new variant" path there also uses `findLast('user')` and has the same bug.

#### C. Visual Cue for Existing Variants

When a message has `_variants.length > 1`, the variant navigation arrows are already rendered. But before the user has generated any variants, there is no hint that the feature exists. No change needed here — the regenerate button tooltip ("Regenerate") is sufficient. The variant arrows appear naturally once the first regeneration happens. This is acceptable UX for a playground tool.

---

## 4 — Persona System

### Current State (do not re-implement)

- `static/js/features/chat-templates.js`: template manager modal, built-in templates, user-created templates, `applyTemplate()`, `{{char}}`/`{{user}}` substitution
- `/api/templates` CRUD endpoints (Rust): list, create, update, delete
- Template manager accessible via a button in chat settings panel (3-step access: open settings → scroll to System Prompt section → click Templates button)
- All built-in templates are roleplay characters; no neutral "task" personas exist

### What to Add

#### A. Non-Roleplay Default Personas

Add to the `BUILTIN_TEMPLATES` array in `chat-templates.js`. These use no `{{char}}`/`{{user}}` substitution — just a plain system prompt:

```js
{ id: 'builtin-coder',
  name: 'Coder',
  prompt: 'You are a senior software engineer. Give precise, working code with minimal explanation unless asked. Prefer idiomatic solutions. Point out potential bugs or issues in the user\'s code when you see them.' },

{ id: 'builtin-reviewer',
  name: 'Code Reviewer',
  prompt: 'You are a thorough code reviewer. Identify bugs, security issues, and style problems. Be specific — reference line numbers or variable names when possible. Suggest concrete fixes, not just observations.' },

{ id: 'builtin-writer',
  name: 'Writing Editor',
  prompt: 'You are a skilled writing editor. Improve clarity, flow, and precision. Preserve the author\'s voice. When rewriting, show the revised version first, then briefly explain what changed and why.' },

{ id: 'builtin-brainstorm',
  name: 'Brainstorm Partner',
  prompt: 'You are a creative brainstorming partner. Generate diverse ideas, challenge assumptions, and build on the user\'s thinking. Ask clarifying questions when the goal is unclear. Think out loud.' },

{ id: 'builtin-analyst',
  name: 'Analyst',
  prompt: 'You are a precise analytical assistant. Break down complex topics into structured components. Use numbered lists, tables, or headers when they aid clarity. Cite your reasoning. Flag when you are uncertain.' },

{ id: 'builtin-concise',
  name: 'Concise Assistant',
  prompt: 'You are a helpful, concise assistant. Answer directly. No preamble, no filler phrases, no restating the question. If the answer is short, keep it short.' },
```

Place these at the top of `BUILTIN_TEMPLATES` (before the roleplay characters) so they appear first in the list.

#### B. Persona Quick-Switch Chips

A slim row of persona chips appears just above the chat input bar, below the message list. This is the same visual area as the existing "suggested prompts" row but persists across messages.

**Placement in `static/index.html`**: Add between the messages area and the input bar:

```html
<div id="chat-persona-strip" class="chat-persona-strip">
  <!-- populated by JS -->
</div>
```

**Rendering** (add `renderPersonaStrip()` to `chat-render.js`):

```js
export async function renderPersonaStrip() {
    const strip = document.getElementById('chat-persona-strip');
    if (!strip) return;
    const tab = activeChatTab();
    const templates = await loadTemplates();   // from chat-templates.js
    // Show the 5 most recently used, or fall back to the first 5 builtins
    const recent = JSON.parse(localStorage.getItem('llama-persona-recent') || '[]');
    const ordered = [
        ...recent.map(id => templates.find(t => t.id === id)).filter(Boolean),
        ...templates.filter(t => !recent.includes(t.id)),
    ].slice(0, 5);

    const activeId = tab?._activeTemplateId || null;
    strip.innerHTML = ordered.map(t => `
      <button class="chat-persona-chip ${t.id === activeId ? 'active' : ''}"
              data-persona-id="${escapeAttr(t.id)}"
              title="${escapeAttr(t.prompt.slice(0, 120))}">
        ${escapeHtml(t.name)}
      </button>`).join('') +
      `<button class="chat-persona-chip chat-persona-chip-more" data-persona-action="open-manager" title="Browse all personas">
         <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
           <circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5" cy="12" r="1"/>
         </svg>
       </button>`;
}
```

Wire chip clicks in the delegated event listener in `initChat()`:

```js
document.getElementById('chat-persona-strip')?.addEventListener('click', e => {
    const chip = e.target.closest('[data-persona-id]');
    if (chip) {
        applyPersona(chip.dataset.personaId);
        return;
    }
    if (e.target.closest('[data-persona-action="open-manager"]')) {
        openTemplateManager();
    }
});
```

```js
async function applyPersona(templateId) {
    const tab = activeChatTab();
    if (!tab) return;
    const templates = await loadTemplates();
    const t = templates.find(x => x.id === templateId);
    if (!t) return;

    tab.system_prompt = t.prompt;
    tab._activeTemplateId = templateId;

    // Update recent list
    const recent = JSON.parse(localStorage.getItem('llama-persona-recent') || '[]');
    const updated = [templateId, ...recent.filter(id => id !== templateId)].slice(0, 10);
    localStorage.setItem('llama-persona-recent', JSON.stringify(updated));

    renderPersonaStrip();
    showToast(`Persona: ${t.name}`, 'success');
    scheduleChatPersist();
}
```

`applyPersona` is separate from the existing `applyTemplate` in chat-templates.js (which also updates the template manager UI). Either merge them or call `applyTemplate` internally — just avoid duplicating the `tab.system_prompt = t.prompt` logic.

#### C. Persona Indicator in Tab Header

When a tab has an active persona, show a small label below the tab name so the strip is scannable at a glance.

In `renderChatTabs()` in `chat-render.js`, add a persona subtitle to the tab HTML:

```js
const personaLabel = tab._activeTemplateId
    ? `<span class="chat-tab-persona">${escapeHtml(getTemplateNameById(tab._activeTemplateId))}</span>`
    : '';

// Add to tab innerHTML after the tab name span:
// <span class="chat-tab-name">...</span>
// <span class="chat-tab-persona">Coder</span>   ← new
```

`getTemplateNameById(id)` is a sync helper that looks up the cached template list (templates are already loaded into a module-level `_userTemplates` / merged with `BUILTIN_TEMPLATES` in `chat-templates.js`).

CSS:
```css
.chat-tab-persona {
    display: block;
    font-size: 0.65rem;
    color: var(--color-primary);
    opacity: 0.75;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100px;
    line-height: 1;
    margin-top: 1px;
}
```

#### D. Persist Active Persona with Tab

`_activeTemplateId` is a runtime-only field. To survive page reload, persist it as `active_template_id` in the tab JSON.

In `chat-state.js`, `newChatTab()` add:
```js
active_template_id: null,
```

In `normalizeChatTab()`:
```js
active_template_id: tab.active_template_id ?? null,
```

Change all references from `tab._activeTemplateId` to `tab.active_template_id` (no underscore prefix — it is persisted now).

In Rust `src/web/mod.rs` `ChatTab` struct, add:
```rust
#[serde(rename = "activeTemplateId", default)]
pub active_template_id: Option<String>,
```

#### E. SillyTavern Link

Add a quiet footnote at the bottom of the System Prompt settings panel in the chat settings sidebar. This is the natural place — a user who wants more than a system prompt is the user who wants SillyTavern.

In `static/index.html`, at the bottom of the system-prompt settings section (after the explicit mode toggle):

```html
<p class="chat-sillytavern-note">
  For advanced roleplay and character management, see
  <a href="https://github.com/SillyTavern/SillyTavern" target="_blank" rel="noopener">SillyTavern</a>.
</p>
```

CSS:
```css
.chat-sillytavern-note {
    font-size: 0.72rem;
    color: var(--text-muted);
    margin-top: 12px;
    line-height: 1.5;
}
.chat-sillytavern-note a { color: var(--text-muted); text-decoration: underline; opacity: 0.7; }
.chat-sillytavern-note a:hover { opacity: 1; }
```

---

## CSS File Summary

All new CSS belongs in `static/css/chat.css` unless noted. No new CSS files needed.

New classes introduced:
- `.chat-export-menu` — export format picker dropdown
- `.chat-tab-pin-btn`, `.chat-tab-pin-icon`, `.chat-tab-pin-sep` — pin feature
- `.chat-persona-strip`, `.chat-persona-chip` — persona quick-switch row
- `.chat-tab-persona` — persona label in tab header
- `.chat-msg-highlight` — brief yellow-glow animation on search result jump (add keyframe)
- `.chat-sillytavern-note` — footnote in settings panel

---

## Implementation Order

If implementing sequentially, suggested order:

1. **Non-roleplay default personas** (A under §4) — pure data, no UI change, 30 min
2. **Mid-conversation edit + branch** (A under §3) — one-line change in `editMessageContent`, 15 min
3. **Regenerate from correct user message** (B under §3) — five-line fix, 15 min
4. **JSON export** (A under §1) — small function addition, 30 min
5. **Export format picker** (B under §1) — small UI addition, 45 min
6. **Pin/favorite** (§2) — new field, sort logic, UI, 2–3 hours
7. **Persona chips** (B–D under §4) — new UI strip, local-storage recents, 3–4 hours
8. **SillyTavern link** (E under §4) — two lines of HTML + CSS, 5 min
