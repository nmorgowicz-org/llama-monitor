# Chat Evolution — Adaptive Layout & UI Completion

**Date:** 2026-05-10
**Last updated:** 2026-05-18
**Status:** In Progress — Tasks 0–9 remaining; all targeted for `feature/chat-system-evolution`
**Branch:** `feature/chat-system-evolution`
**Scope:** CSS refactor, preset modal, file action consolidation, shell width architecture, toolbar density, name/font demotion, chat focus mode, draggable sidebar, popover hardening

---

## Purpose of This Document

This is the single living plan for all remaining UI work on the `feature/chat-system-evolution` branch. It supersedes both the original `20260510-adaptive-layout-enhancements.md` planning doc and the archived `20260505-modal_ui_improvements.md`. An AI agent reading this document with no prior context should be able to pick up and continue development from any task without needing to read anything else.

---

## What Has Already Been Done (Context)

The following work is **complete** and must not be re-implemented:

- **Chat system evolution** — The post-evolution DOM structure is live. `.chat-sessions-panel` + `.chat-main-area` replaced the old `#chat-tab-bar`. The `.sidebar-nav` now has four tabs only: Server, Chat, Logs, Settings.
- **Premium modal styling** — 10 of 11 modals have glass-morphism treatment. See `docs/plans/20260505-modal_ui_improvements.md` for the complete list. The only remaining modal is `#preset-modal`.
- **Security** — API token auth, origin validation, rate limiting, agent mTLS. Do not touch auth flows.
- **Release notes slide panel** — `.slide-panel` CSS is in `layout.css`.
- **File browser modal** — Full premium CSS in `chat.css`.

---

## App Architecture Reference

> Read this section before touching any files. This app is unlike most web projects.

### Runtime

- **Backend:** Rust (Tauri-style binary), serves static files and a REST/WebSocket API.
- **Frontend:** Vanilla JavaScript — no React, Vue, or Angular. All UI is direct DOM manipulation and CSS class toggling.
- **Static assets root:** `static/`
- **Entry point:** `static/index.html`
- **Compact/embedded variant:** `static/compact.html` — do not touch in this plan.

### Key frontend files

| File | Purpose |
|------|---------|
| `static/index.html` | Main app shell — all HTML, all modal markup, all button IDs |
| `static/js/bootstrap.js` | App initialization, Escape key handler (search `keydown`), suggestion parsing |
| `static/js/features/nav.js` | Tab switching (`switchTab` at top of file), sidebar collapse toggle, sidebar localStorage |
| `static/js/features/chat-params.js` | Chat header button wiring, export/import/persona, resize handle, compact, telemetry |
| `static/js/features/chat-sessions-sidebar.js` | Session panel show/hide/render |
| `static/js/features/chat-render.js` | Message rendering, `exportChatTab`, `importChatTab` |
| `static/js/features/chat-notes.js` | Chat notes sidebar — has its own resize handle (reference for Phase 5) |
| `static/js/features/settings.js` | Settings modal wiring, `default_sidebar_width` persistence |
| `static/js/core/app-state.js` | All runtime state — `chat`, `monitorState`, `wsData`, `lastLlamaMetrics` |

### Current DOM structure (post-evolution — this is the live state)

```
<body>
  ├── .endpoint-health-strip          ← TOP STATUS BAR (endpoint URL, health, agent, latency)
  ├── .top-nav-bar                    ← TITLE BAR (logo, h1, cockpit metrics button, user menu)
  ├── .sidebar-nav                    ← LEFT NAV (Server, Chat, Logs, Settings, collapse btn)
  └── .content-area                   ← ALL PAGE CONTENT
        └── .page.chat-page           ← CHAT (flex-direction: row)
              ├── .chat-sessions-panel   ← 240px Discord-style session list
              │     ├── .csp-header
              │     ├── .csp-actions
              │     ├── .csp-search-wrap / .csp-search
              │     └── .csp-list
              └── .chat-main-area        ← flex:1, all chat content
                    ├── #chat-header
                    │     ├── .chat-header-left  (telemetry trigger, Behavior, Model, Style, Compact, name pills, explicit toggle)
                    │     └── .chat-header-right (font controls, Export, Import, Persona)
                    ├── .chat-telemetry-inline-host (hidden by default, shown when pinned)
                    ├── #ctx-pressure-bar
                    ├── .chat-messages (with .chat-sidebar overlay for notes)
                    └── #chat-input-row
```

### Current chat header buttons (exact IDs, left to right)

**Left cluster (`.chat-header-left`):**
- `#chat-telemetry-btn` — telemetry popup trigger
- `#btn-behavior` — Behavior panel
- `#btn-model-params` — Model params panel
- `#btn-chat-style` — Style panel
- `#btn-compact` — Compact context
- `.chat-name-inputs` → `.chat-name-pill` × 2 (AI + You name inputs with `#chat-ai-name`, `#chat-user-name`)
- `#chat-explicit-toggle-footer` — explicit content toggle (lock/unlock/fire icon)

**Right cluster (`.chat-header-right`):**
- `.chat-font-controls` → `#chat-font-decrease`, `#chat-font-value` (span), `#chat-font-increase`
- `#chat-export-btn` + `#chat-export-menu` dropdown (Save as Markdown / Save as JSON)
- `#chat-import-btn`
- `#chat-persona-btn` + `#chat-persona-menu` dropdown

### CSS files and current state

| File | Lines | Purpose / Notes |
|------|-------|-----------------|
| `tokens.css` | ~70 | Design tokens — CSS custom properties. Complete. Use `var(--*)` always. |
| `base.css` | ~85 | Body, reset, base grid. 0% light theme coverage. |
| `layout.css` | ~1000+ | Sidebar widths (hardcoded 208px/68px), page layout, slide-panel. 0% light theme on layout rules. |
| `cards-inference.css` | ~1682 | Inference dashboard widget cards. 0% light theme. |
| `agent-modal.css` | ~1065 | Remote agent modal — premium, complete. |
| `cards-hardware.css` | ~1733 | Hardware dashboard cards. 0% light theme. |
| `components.css` | ~462 | Base modal shell (flat), button base styles, `.modal-section` (flat). Override point for all modals. |
| `chat.css` | **9381** | Chat UI + modal premium styles for export, config, file-browser, template-manager. **Needs refactor — see Task 0.** |
| `chat-guided-generation.css` | — | Suggestions + quick guide styles. |
| `auth.css` | — | Login/auth shell (`.auth-shell`, `.auth-shell-card`). Do not modify. |
| `setup-view.css` | ~1468 | Setup view + premium modals for analytics, user-preferences, shortcuts, models. |
| `settings-modal.css` | ~1327 | Settings modal — premium, complete. |
| `logs.css` | ~270 | Logs page. 0% light theme. |
| `modal-premium.css` | **DOES NOT EXIST YET** | Will be created in Task 0. |

### CSS import order in index.html (current)

```html
tokens.css → base.css → layout.css → cards-inference.css → agent-modal.css
→ cards-hardware.css → components.css → chat.css → chat-guided-generation.css
→ auth.css → setup-view.css → settings-modal.css → logs.css
```

`modal-premium.css` must be added **after `components.css`** (which it overrides) and **before `setup-view.css``. Exact insertion:

```html
<link rel="stylesheet" href="/css/components.css">
<link rel="stylesheet" href="/css/modal-premium.css">   ← ADD HERE
<link rel="stylesheet" href="/css/chat.css">
```

### Sidebar layout state

- Expanded width: `208px` (hardcoded in `layout.css` — `grep '208px'`)
- Collapsed width: `68px` (hardcoded in `layout.css` — `grep '68px'`)
- Body class when collapsed: `body.sidebar-collapsed`
- LocalStorage key: `sidebarCollapsed` (string `'true'`/`'false'`) — set in `nav.js`

### Existing localStorage keys (do not reuse these)

| Key | Owner | Purpose |
|-----|-------|---------|
| `sidebarCollapsed` | `nav.js` | Main nav collapsed state |
| `llama_monitor_sidebar_width` | `chat-notes.js` | Chat notes sidebar width |
| `uiSettings.theme` | `user-menu.js` | Dark/light theme |
| `llama-monitor-chat-focus-mode` | *(to be created)* | Focus mode persistence |
| `appNavWidth` | *(to be created)* | Main nav drag-resize width |

### State management

- All runtime state in `static/js/core/app-state.js`.
- Primary chat state: `export const chat = { busy, tabs, activeTabId, activeTabIdx, ... }`.
- No reactive framework. UI updated by calling render functions directly.
- State classes applied to `body` (e.g., `body.sidebar-collapsed`). All new state classes must follow this same pattern.

---

## Outstanding Tasks — Execution Order

Execute in this order. Later tasks build on earlier ones where noted.

| # | Task | Depends on | Effort |
|---|------|-----------|--------|
| 0 | CSS refactor — extract `modal-premium.css` | — | ~2h |
| 1 | Preset modal premium | Task 0 | ~2h |
| 2 | File action consolidation (Export+Import → File) | — | ~2h |
| 3 | Phase 1: Shell width architecture (CSS vars + ResizeObserver) | — | ~3h |
| 4 | Phase 2: Toolbar density tiers | Task 3 | ~3h |
| 5 | Phase 4: Name/font demotion | Task 4 | ~1h |
| 6 | Phase 7: Chat Focus Mode | Task 3 (soft) | ~5h |
| 7 | Phase 5: Draggable main sidebar | Task 3 | ~3h |
| 8 | Phase 6: Popover hardening | Task 4 | ~1h |

---

## Task 0: CSS Refactor — Extract `modal-premium.css`

**Why:** `chat.css` has grown to 9,381 lines because modal premium styles were added opportunistically during feature work. Modal glass-morphism styles have no semantic relationship to chat UI. The file must be split before adding more CSS in Tasks 1, 4, 5, 6, and 8.

**What to extract from `chat.css`:**

Search for these selectors in `chat.css` and move the entire block (from the selector through its final closing brace) to `modal-premium.css`:

1. `#export-modal` — glass shell, ambient glow, entrance animation, all section styles, light theme overrides
2. `#config-modal` — glass shell, ambient glow, entrance animation, all section styles, light theme overrides
3. `#file-browser-modal` — glass shell, ambient glow, entrance animation, all section/entry styles, light theme overrides
4. `.template-manager-modal` — glass shell, ambient glow, entrance animation, all internal section styles, light theme overrides

Also move any `@keyframes` in `chat.css` that are only used by those modals (e.g., `export-modal-entrance`, `config-modal-entrance`). Shared keyframes used by chat UI too should stay.

**How:**

1. Create `static/css/modal-premium.css` with a header comment identifying it as the shared premium modal styles file.
2. Cut each modal block from `chat.css` and paste into `modal-premium.css`. Do not modify the CSS rules, just relocate.
3. Add `<link rel="stylesheet" href="/css/modal-premium.css">` to `static/index.html` after the `components.css` line.
4. Build and verify all four modals still render correctly.

**Verification:** Open each of the four modals in the running app and confirm glass treatment, entrance animation, and hover states are unchanged.

---

## Task 1: Preset Modal Premium

**File to edit:** `static/css/modal-premium.css` (created in Task 0)
**Modal HTML:** `#preset-modal` in `static/index.html` (search `id="preset-modal"`)
**Sections:** 9 `details.modal-section` elements — Model & Memory, Context & KV Cache, Batching & Slots, Generation, GPU Distribution, Threading, Rope Scaling, Speculative Decoding, Advanced.

**What to add:**

Apply the exact same pattern used for `#config-modal`. The config modal is the best reference because it also uses `details.modal-section` elements. Copy its structure and adapt the modal ID. Key rules needed:

```css
/* 1. Glass shell */
#preset-modal .modal { /* glass bg, backdrop-filter, border, box-shadow, entrance animation */ }
#preset-modal .modal::before { /* top-edge gradient line */ }
#preset-modal .modal::after { /* breathing gradient border */ }
#preset-modal.open .modal::after,
#preset-modal[style*="display: block"] .modal::after { animation: modal-border-breathe 3s ease-in-out infinite; }

/* 2. Ambient glow orb */
#preset-modal::before { /* radial gradient, blur, ambient-drift animation */ }

/* 3. Entrance animation — define a keyframe named preset-modal-entrance */
@keyframes preset-modal-entrance { /* scale(0.95) translateY(12px) → scale(1) translateY(0) */ }

/* 4. Header */
#preset-modal .modal-header { /* gradient bg, border-bottom */ }
#preset-modal .modal-header h2 { /* gradient text fill */ }

/* 5. Widget-card for all 9 details.modal-section */
#preset-modal details.modal-section { /* widget-card treatment with 3D depth */ }
#preset-modal details.modal-section::before { /* top gradient accent line */ }
#preset-modal details.modal-section::after { /* border-glow mask, opacity 0 → 1 on hover */ }
#preset-modal details.modal-section:hover { /* elevation + border-color change */ }

/* 6. Per-section accent colors via nth-of-type (9 accents) */
/* Model & Memory: teal, Context: cyan, Batching: green, Generation: indigo,
   GPU: amber, Threading: purple, Rope: rose, Speculative: sky, Advanced: orange */

/* 7. Staggered section entrance animations (animation-delay increments of 0.04s) */
#preset-modal details.modal-section:nth-of-type(1) { animation-delay: 0.04s; }
/* ... through :nth-of-type(9) { animation-delay: 0.36s; } */

/* 8. Elevated form controls */
#preset-modal .modal-field input,
#preset-modal .modal-field select { /* gradient bg, hover glow, focus ring */ }

/* 9. Light theme overrides */
[data-theme="light"] #preset-modal .modal { /* light bg, light border, light shadow */ }
[data-theme="light"] #preset-modal details.modal-section { /* light card bg */ }
```

**Reference:** Copy from `#config-modal` in `chat.css` (or in `modal-premium.css` after Task 0). The config modal has the most similar structure (also uses `details.modal-section`).

**Reduced motion:**
```css
@media (prefers-reduced-motion: reduce) {
  #preset-modal .modal,
  #preset-modal .modal::after,
  #preset-modal details.modal-section {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

---

## Task 2: File Action Consolidation

**Goal:** Replace the separate `#chat-export-btn` and `#chat-import-btn` buttons with a single `#chat-file-btn` button and a unified dropdown. Saves one toolbar pill, reduces visual clutter, groups file management as a utility action.

### HTML changes (`static/index.html`)

Find the current export + import block in `#chat-header .chat-header-right` (search `chat-export-btn`). Replace:

```html
<!-- REMOVE THIS BLOCK: -->
<div class="chat-header-dropdown">
  <button class="chat-header-btn" id="chat-export-btn" ...>...</button>
  <div id="chat-export-menu" class="chat-export-menu hidden">
    <button data-export-format="md">Save as Markdown</button>
    <button data-export-format="json">Save as JSON</button>
  </div>
</div>
<button class="chat-header-btn" id="chat-import-btn" ...>...</button>

<!-- REPLACE WITH: -->
<div class="chat-header-dropdown">
  <button class="chat-header-btn" id="chat-file-btn" title="File actions — save or import conversation">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
      <polyline points="14 2 14 8 20 8"/>
      <line x1="12" y1="18" x2="12" y2="12"/>
      <line x1="9" y1="15" x2="15" y2="15"/>
    </svg>
    <span class="chat-header-label">File</span>
  </button>
  <div id="chat-file-menu" class="chat-export-menu hidden">
    <button data-export-format="md">Save as Markdown</button>
    <button data-export-format="json">Save as JSON</button>
    <button id="chat-file-import-item">Import conversation</button>
  </div>
</div>
```

### JS changes (`static/js/features/chat-params.js`)

Find the export button wiring block (search `chat-export-btn`) and the import button wiring (search `chat-import-btn`). Replace with:

```javascript
// File button — unified export/import dropdown
const fileBtn = document.getElementById('chat-file-btn');
const fileMenu = document.getElementById('chat-file-menu');
if (fileBtn && fileMenu) {
  fileBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    fileMenu.classList.toggle('hidden');
  });
  fileMenu.querySelectorAll('[data-export-format]').forEach(item => {
    item.addEventListener('click', () => {
      exportChatTab(item.dataset.exportFormat);
      fileMenu.classList.add('hidden');
    });
  });
  document.getElementById('chat-file-import-item')?.addEventListener('click', () => {
    importChatTab();
    fileMenu.classList.add('hidden');
  });
  document.addEventListener('click', (e) => {
    if (!e.target.closest('#chat-file-btn') && !e.target.closest('#chat-file-menu')) {
      fileMenu.classList.add('hidden');
    }
  });
}
```

Note: `exportChatTab` and `importChatTab` are imported from `chat-render.js` at the top of `chat-params.js` — they are already imported, just change the wiring.

### CSS changes (`static/css/chat.css`)

Search for `.chat-export-menu` — the existing dropdown menu CSS already works. The `#chat-file-menu` uses the same class so no new CSS is needed. If there are any rules targeting `#chat-export-btn` or `#chat-import-btn` by ID, rename them to `#chat-file-btn`.

---

## Task 3: Phase 1 — Shell Width Architecture

**Goal:** Replace hardcoded sidebar pixel values with CSS custom properties and introduce a `ResizeObserver`-based width density system. This is the foundation that Tasks 4, 5, 7, and 8 build on.

### Step 1: CSS variables in `layout.css`

Find the current hardcoded sidebar widths (search `208px` and `68px` in `layout.css`). Replace with CSS variables. Add to the `:root` block at the top of `layout.css`:

```css
:root {
  --sidebar-width-expanded: 208px;
  --sidebar-width-collapsed: 68px;
  --sidebar-width-active: 208px; /* updated by JS on drag resize */
}
```

Then replace all occurrences of `208px` (in sidebar and content-area rules) with `var(--sidebar-width-expanded)` and all `68px` with `var(--sidebar-width-collapsed)`. The `.content-area` margin and width rules that mirror the sidebar values must also use these variables:

```css
.content-area {
  margin-left: var(--sidebar-width-expanded);
  width: calc(100% - var(--sidebar-width-expanded));
}
body.sidebar-collapsed .content-area {
  margin-left: var(--sidebar-width-collapsed);
  width: calc(100% - var(--sidebar-width-collapsed));
}
```

### Step 2: Width density classes

Add to `layout.css` — these will be set by the JS observer in Step 3:

```css
/* Width density classes — applied to body by chat-width-observer.js */
/* Default / comfortable: no class needed (normal styles apply) */

body.shell-width-tight .chat-main-area {
  /* Tighter layout — enforced by descendant rules in Tasks 4 and 5 */
}

body.shell-width-very-tight .chat-main-area {
  /* Most compact layout — enforced by descendant rules in Tasks 4 and 5 */
}
```

The actual compaction rules go in `chat.css` in Task 4. This file just needs the class stubs documented.

### Step 3: Create `static/js/features/chat-width-observer.js`

```javascript
// Observes .chat-main-area width and applies density classes to body.
// Integration: imported and called from bootstrap.js after DOM ready.

const THRESHOLDS = {
  tight: 700,     // px — tune visually during implementation
  veryTight: 520, // px — tune visually during implementation
};

let observer = null;
let overridden = false; // true when Focus Mode pins density to comfortable

export function initChatWidthObserver() {
  const target = document.getElementById('chat-main-area');
  if (!target) return;
  observer = new ResizeObserver(entries => {
    if (overridden) return;
    const width = entries[0].contentRect.width;
    applyDensityClass(classForWidth(width));
  });
  observer.observe(target);
}

export function applyDensityClass(cls) {
  document.body.classList.remove('shell-width-tight', 'shell-width-very-tight');
  if (cls) document.body.classList.add(cls);
}

export function pinComfortableDensity() {
  overridden = true;
  applyDensityClass(''); // comfortable — no class
}

export function unpinDensity() {
  overridden = false;
  // Re-evaluate immediately
  const target = document.getElementById('chat-main-area');
  if (target) {
    const width = target.getBoundingClientRect().width;
    applyDensityClass(classForWidth(width));
  }
}

function classForWidth(width) {
  if (width < THRESHOLDS.veryTight) return 'shell-width-very-tight';
  if (width < THRESHOLDS.tight) return 'shell-width-tight';
  return ''; // comfortable
}
```

### Step 4: Wire into `bootstrap.js`

Add import at top of `bootstrap.js`:
```javascript
import { initChatWidthObserver } from './features/chat-width-observer.js';
```

Call after DOM is ready (find the existing init block in bootstrap.js):
```javascript
initChatWidthObserver();
```

### Threshold tuning guidance

After wiring, test with the browser at these states and adjust `THRESHOLDS` until toolbar clipping is prevented:
- Full-width browser, sidebar expanded
- ~1000px wide browser, sidebar expanded
- ~800px wide browser, sidebar expanded
- ~800px wide browser, sidebar collapsed
- Any width, Focus Mode active (comfortable always)

---

## Task 4: Phase 2 — Toolbar Density Tiers

**Goal:** Make the chat header respond progressively to the density classes set in Task 3.

**File:** `static/css/chat.css` — add at end of the file under a clearly labeled `/* === TOOLBAR DENSITY SYSTEM === */` comment.

### Tier: tight (`body.shell-width-tight`)

```css
body.shell-width-tight #chat-header {
  gap: 4px; /* reduce from default */
}
body.shell-width-tight .chat-header-left,
body.shell-width-tight .chat-header-right {
  gap: 3px;
  flex-wrap: wrap;
  min-width: 0;
}
body.shell-width-tight .chat-header-btn {
  padding: 4px 7px; /* reduce from default */
}
body.shell-width-tight .chat-name-input {
  width: 60px; /* reduce from 80px */
}
body.shell-width-tight .chat-font-controls {
  gap: 2px;
}
```

### Tier: very tight (`body.shell-width-very-tight`)

```css
/* Hide labels on secondary controls */
body.shell-width-very-tight .chat-name-label {
  display: none;
}
body.shell-width-very-tight .chat-font-value {
  display: none;
}
body.shell-width-very-tight .chat-name-input {
  width: 48px;
}
/* File button label hidden — icon only */
body.shell-width-very-tight #chat-file-btn .chat-header-label {
  display: none;
}
/* Persona label hidden — icon only */
body.shell-width-very-tight #chat-persona-btn .chat-header-label {
  display: none;
}
body.shell-width-very-tight .chat-header-btn {
  padding: 4px 5px;
}
```

### Priority preservation rule

The following buttons must remain **fully visible at all density tiers** — never hide or icon-only demote these:
- `#btn-behavior` (Behavior)
- `#btn-model-params` (Model)
- `#btn-chat-style` (Style)
- `#btn-compact` (Compact)

These are primary authoring controls. The file, persona, font, and name controls are secondary and may be demoted.

---

## Task 5: Phase 4 — Name/Font Demotion

This task is part of the density system (builds on Task 4). Additional name and font compaction rules beyond what Task 4 introduces.

**File:** `static/css/chat.css` — add under the density system comment block.

### Progressive name input compaction

```css
/* tight: shrink but keep labels */
body.shell-width-tight .chat-name-pill {
  padding: 2px 5px;
}

/* very-tight: pill shrinks to minimal footprint */
body.shell-width-very-tight .chat-name-pill {
  padding: 1px 3px;
}
body.shell-width-very-tight .chat-name-input {
  width: 44px;
  font-size: 11px;
}
```

### Font control preservation rule

Font controls must remain reachable at all tiers. Under very-tight, the `+`/`-` buttons remain visible even if the percentage value is hidden. Do not hide the entire `.chat-font-controls` block at any tier.

### Explicit toggle

The explicit toggle button (`#chat-explicit-toggle-footer`) should remain visible at all tiers — it is a core content control, not a utility.

---

## Task 6: Phase 7 — Chat Focus Mode

**Goal:** An immersive mode that hides the structural app chrome (status bar, title bar, sidebar nav) and gives the chat full screen space. The session panel stays visible so the user can switch chats without exiting.

This task is **independent** of Tasks 3–5. It can be implemented before or after the density system. If implemented before Task 3, the `pinComfortableDensity` / `unpinDensity` calls should be no-ops (check for function existence before calling).

### What Focus Mode hides (animated, not `display:none`)

| Element | Behavior |
|---------|---------|
| `.endpoint-health-strip` | Collapses to height 0, opacity 0 |
| `.top-nav-bar` | Collapses to height 0, opacity 0 |
| `.sidebar-nav` | Collapses to width 0, opacity 0 |
| `.content-area` | `margin-left: 0` to fill freed space |

### What Focus Mode keeps (untouched)

- `.chat-sessions-panel` — user must be able to switch chats without exiting
- All `#chat-header` controls — including the Focus Mode button itself
- `.chat-telemetry-inline-host` — if user had it pinned
- `.chat-messages`, `.chat-sidebar`, `#chat-input-row`

### Step 1: HTML additions to `static/index.html`

**Focus Mode button** — add as last button in `.chat-header-right`, after `#chat-persona-btn`:

```html
<button id="chat-focus-mode-btn"
        class="chat-header-btn"
        title="Focus Mode (⌘⇧F)"
        aria-pressed="false"
        aria-label="Toggle Focus Mode">
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor"
       stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
    <path d="M1 5V1h4M9 1h4v4M13 9v4H9M5 13H1V9"/>
  </svg>
  <span class="chat-header-label">Focus</span>
</button>
```

**Exit beacon + pill** — add immediately before `</body>`:

```html
<div id="focus-mode-exit-beacon" role="button" tabindex="-1" aria-label="Exit Focus Mode">
  <div id="focus-mode-exit-pill">Focus Mode — click to exit &nbsp;×</div>
</div>
```

### Step 2: CSS additions to `static/css/layout.css`

Add at the end of `layout.css` under a `/* === CHAT FOCUS MODE === */` comment:

```css
/* Animated collapse transitions */
.endpoint-health-strip,
.top-nav-bar {
  max-height: 200px;
  overflow: hidden;
  opacity: 1;
  transition: max-height 0.28s cubic-bezier(0.4, 0, 0.2, 1),
              opacity 0.22s ease;
}

body.chat-focus-mode .endpoint-health-strip,
body.chat-focus-mode .top-nav-bar {
  max-height: 0;
  opacity: 0;
  pointer-events: none;
}

body.chat-focus-mode .sidebar-nav {
  width: 0 !important;
  opacity: 0;
  pointer-events: none;
  overflow: hidden;
  transition: width 0.28s cubic-bezier(0.4, 0, 0.2, 1),
              opacity 0.22s ease;
}

body.chat-focus-mode .content-area {
  margin-left: 0 !important;
  width: 100% !important;
  transition: margin-left 0.28s cubic-bezier(0.4, 0, 0.2, 1),
              width 0.28s cubic-bezier(0.4, 0, 0.2, 1);
}

/* Exit beacon */
#focus-mode-exit-beacon {
  display: none;
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: 6px;
  z-index: 9999;
  cursor: pointer;
  background: transparent;
  pointer-events: none;
}

body.chat-focus-mode #focus-mode-exit-beacon {
  display: block;
  pointer-events: all;
}

#focus-mode-exit-pill {
  position: fixed;
  top: 0;
  left: 50%;
  transform: translateX(-50%) translateY(-100%);
  background: var(--surface-card, rgba(30, 30, 40, 0.92));
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border: 1px solid var(--border-subtle, rgba(255,255,255,0.1));
  border-top: none;
  border-radius: 0 0 12px 12px;
  padding: 6px 16px 8px;
  font-size: 12px;
  color: var(--text-secondary, rgba(255,255,255,0.6));
  letter-spacing: 0.03em;
  white-space: nowrap;
  z-index: 10000;
  opacity: 0;
  transition: transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1),
              opacity 0.18s ease;
  pointer-events: none;
  user-select: none;
}

#focus-mode-exit-beacon:hover #focus-mode-exit-pill,
#focus-mode-exit-pill:hover {
  transform: translateX(-50%) translateY(0);
  opacity: 1;
  pointer-events: all;
}

/* Reduced motion */
@media (prefers-reduced-motion: reduce) {
  .endpoint-health-strip,
  .top-nav-bar,
  body.chat-focus-mode .sidebar-nav,
  body.chat-focus-mode .content-area,
  #focus-mode-exit-pill {
    transition: none !important;
  }
}
```

Add to `static/css/chat.css` under the density system comment block:

```css
/* Focus Mode button active state */
#chat-focus-mode-btn.active {
  color: var(--color-primary, #a78bfa);
  background: var(--color-primary-muted, rgba(167, 139, 250, 0.12));
}
#chat-focus-mode-btn.active svg {
  stroke: var(--color-primary, #a78bfa);
}
```

### Step 3: Create `static/js/features/chat-focus-mode.js`

```javascript
import { pinComfortableDensity, unpinDensity } from './chat-width-observer.js';

const STORAGE_KEY = 'llama-monitor-chat-focus-mode';
let focusModeActive = false;

export function initChatFocusMode() {
  // Create beacon + pill if not already in DOM (they should be in index.html)
  const beacon = document.getElementById('focus-mode-exit-beacon');
  const pill = document.getElementById('focus-mode-exit-pill');
  if (beacon) beacon.addEventListener('click', exitFocusMode);
  if (pill) pill.addEventListener('click', exitFocusMode);

  // Restore persisted state
  if (localStorage.getItem(STORAGE_KEY) === '1') {
    enterFocusMode(true); // silent=true skips toast on restore
  }
}

export function enterFocusMode(silent = false) {
  focusModeActive = true;
  document.body.classList.add('chat-focus-mode');
  updateFocusModeButton();
  localStorage.setItem(STORAGE_KEY, '1');
  // Pin density to comfortable so focus mode is never accidentally cramped
  if (typeof pinComfortableDensity === 'function') pinComfortableDensity();
  if (!silent) {
    // Show a brief toast hinting how to exit
    // Use the existing toast system — search bootstrap.js or toast.js for showToast
    showFocusToast();
  }
}

export function exitFocusMode() {
  focusModeActive = false;
  document.body.classList.remove('chat-focus-mode');
  updateFocusModeButton();
  localStorage.removeItem(STORAGE_KEY);
  if (typeof unpinDensity === 'function') unpinDensity();
}

export function toggleFocusMode() {
  focusModeActive ? exitFocusMode() : enterFocusMode();
}

export function isFocusModeActive() {
  return focusModeActive;
}

function updateFocusModeButton() {
  const btn = document.getElementById('chat-focus-mode-btn');
  if (!btn) return;
  btn.classList.toggle('active', focusModeActive);
  btn.setAttribute('aria-pressed', String(focusModeActive));
  btn.setAttribute('title', focusModeActive ? 'Exit Focus Mode (⌘⇧F)' : 'Focus Mode (⌘⇧F)');
}

function showFocusToast() {
  // Find the toast function — it is exported from toast.js and called elsewhere in the codebase.
  // Search bootstrap.js or other feature files for: showToast( or toast( to find the exact call signature.
  // The toast should read: "Focus Mode active — hover top edge or press ⌘⇧F to exit"
  // Use a 4000ms duration so it disappears before becoming intrusive.
}
```

**Note on `showFocusToast`:** The exact toast API must be determined by reading `static/js/features/toast.js` or searching for `showToast` in the codebase. Fill in the call with the correct signature.

### Step 4: Wire into `bootstrap.js`

Add imports:
```javascript
import { initChatFocusMode, toggleFocusMode, isFocusModeActive } from './features/chat-focus-mode.js';
```

In the init block, call:
```javascript
initChatFocusMode();
```

In the `keydown` listener (find the existing `keydown` event listener at the bottom of `bootstrap.js`), add before the Escape handler or alongside other keyboard shortcuts:
```javascript
if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'F') {
  e.preventDefault();
  toggleFocusMode();
  return;
}
```

Wire the button:
```javascript
document.getElementById('chat-focus-mode-btn')
  ?.addEventListener('click', toggleFocusMode);
```

### Step 5: Auto-exit on tab switch in `nav.js`

In `nav.js`, find `switchTab` (at the top of the file). Add after the function signature, inside the body, before any layout changes:
```javascript
import { isFocusModeActive, exitFocusMode } from './chat-focus-mode.js';

export function switchTab(name) {
  if (name !== 'chat' && isFocusModeActive()) {
    exitFocusMode();
  }
  // ... rest of existing switchTab logic
}
```

### Focus Mode state diagram

```
[Normal View]
  → user clicks #chat-focus-mode-btn OR presses Cmd/Ctrl+Shift+F
[Focus Mode active]
  - body.chat-focus-mode applied
  - .endpoint-health-strip and .top-nav-bar collapse (max-height 0)
  - .sidebar-nav width → 0
  - .content-area expands to fill viewport
  - .chat-sessions-panel STAYS VISIBLE (user can switch chats)
  - density pinned to comfortable
  - localStorage set
  - "hover top edge" toast shown
  → hover top edge → pill appears → click to exit
  → Cmd/Ctrl+Shift+F → exit
  → click #chat-focus-mode-btn → exit
  → switchTab to non-chat → auto-exit (does NOT re-enter on return to chat)
[Normal View restored]
  - body.chat-focus-mode removed
  - all elements animate back
  - density observer resumes
  - localStorage cleared
```

---

## Task 7: Phase 5 — Draggable Main Sidebar

**Goal:** Allow the user to drag-resize the main left navigation sidebar. Width persists across sessions. Works correctly with the CSS variable system from Task 3.

**Note:** This task requires Task 3 to be complete first (CSS variables must exist).

### HTML addition to `static/index.html`

Inside `.sidebar-nav`, add a drag handle as the last child before the closing `</div>`:

```html
<div class="sidebar-resize-handle" id="sidebar-resize-handle" title="Drag to resize sidebar"></div>
```

### CSS additions to `static/css/layout.css`

```css
.sidebar-resize-handle {
  position: absolute;
  top: 0;
  right: -3px;
  width: 6px;
  height: 100%;
  cursor: col-resize;
  z-index: 10;
  background: transparent;
  transition: background 0.15s ease;
}
.sidebar-resize-handle:hover,
.sidebar-nav.is-resizing .sidebar-resize-handle {
  background: var(--color-primary, rgba(167, 139, 250, 0.4));
}

/* sidebar-nav needs position: relative for the handle */
.sidebar-nav {
  position: relative; /* add if not already set */
}
```

### JS — add to `static/js/features/nav.js`

```javascript
const NAV_WIDTH_KEY = 'appNavWidth';
const NAV_MIN_WIDTH = 140;
const NAV_MAX_WIDTH = 320;
const NAV_COLLAPSE_SNAP = 100; // drag below this → collapse

export function initSidebarResize() {
  const handle = document.getElementById('sidebar-resize-handle');
  const nav = document.getElementById('sidebar-nav');
  if (!handle || !nav) return;

  // Restore saved width
  const saved = parseInt(localStorage.getItem(NAV_WIDTH_KEY), 10);
  if (saved && saved >= NAV_MIN_WIDTH) {
    setNavWidth(saved);
  }

  let startX = 0;
  let startWidth = 0;

  handle.addEventListener('mousedown', (e) => {
    e.preventDefault();
    startX = e.clientX;
    startWidth = nav.getBoundingClientRect().width;
    nav.classList.add('is-resizing');
    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
  });

  function onMouseMove(e) {
    const delta = e.clientX - startX;
    const newWidth = Math.max(NAV_MIN_WIDTH, Math.min(NAV_MAX_WIDTH, startWidth + delta));
    setNavWidth(newWidth);
  }

  function onMouseUp() {
    nav.classList.remove('is-resizing');
    document.removeEventListener('mousemove', onMouseMove);
    document.removeEventListener('mouseup', onMouseUp);
    const currentWidth = nav.getBoundingClientRect().width;
    if (currentWidth < NAV_COLLAPSE_SNAP) {
      // Snap to collapsed
      collapseSidebar();
    } else {
      localStorage.setItem(NAV_WIDTH_KEY, Math.round(currentWidth));
    }
  }
}

function setNavWidth(px) {
  document.documentElement.style.setProperty('--sidebar-width-active', `${px}px`);
  document.documentElement.style.setProperty('--sidebar-width-expanded', `${px}px`);
}
```

Call `initSidebarResize()` from within `nav.js`'s own init or export it and call from `bootstrap.js`.

### Width persistence reconciliation

The existing `default_sidebar_width` in `src/state.rs` is the **chat notes sidebar width** (saved per session, server-side). The `SIDEBAR_STORAGE_KEY = 'llama_monitor_sidebar_width'` in `chat-notes.js` is the **chat notes sidebar width**. The new `appNavWidth` localStorage key is completely separate and must not conflict with either.

---

## Task 8: Phase 6 — Popover Hardening

**Goal:** Ensure all popovers/menus remain within the viewport at tight widths and in focus mode.

**File:** `static/css/chat.css`

### Known issues to fix

**1. Telemetry popover min-width**

Search `chat.css` for the telemetry popover rule containing `min-width` near `520px`. Reduce it:
```css
/* Find: #chat-telemetry-popover or .chat-telemetry-popover */
/* Change min-width from 520px to a more flexible value: */
min-width: min(520px, calc(100vw - 32px));
max-width: calc(100vw - 32px);
overflow-x: hidden;
```

**2. Persona menu right-edge clipping**

Search `chat.css` for `.chat-persona-menu` positioning rules. Ensure it has:
```css
.chat-persona-menu {
  /* existing rules plus: */
  max-width: min(320px, calc(100vw - 16px));
  right: 0; /* anchor to right edge of button, not left */
}
```

**3. File dropdown (new `#chat-file-menu`)**

The `.chat-export-menu` CSS already exists. Add a guard:
```css
.chat-export-menu {
  /* existing rules plus: */
  max-width: min(220px, calc(100vw - 16px));
}
```

**4. General popover guard**

Add to `chat.css`:
```css
/* Prevent any chat popover from rendering outside viewport */
.chat-header-dropdown > div,
.chat-telemetry-popover,
.chat-persona-menu {
  box-sizing: border-box;
}
```

---

## Validation Checklist

Work through this checklist in order after all tasks are complete.

### CSS refactor (Task 0)
- [ ] `modal-premium.css` exists and is linked in `index.html`
- [ ] `#export-modal` opens with glass treatment and entrance animation
- [ ] `#config-modal` opens with glass treatment and entrance animation
- [ ] `#file-browser-modal` opens with glass treatment and entrance animation
- [ ] `.template-manager-modal` opens with glass treatment and entrance animation
- [ ] `chat.css` no longer contains those four modal blocks (verify by grep)
- [ ] No visual regression on any other modal

### Preset modal (Task 1)
- [ ] `#preset-modal` opens with glass shell, ambient glow, breathing border
- [ ] All 9 sections have widget-card treatment with per-section accent colors
- [ ] Sections have staggered entrance animation
- [ ] Form controls (inputs, selects) have elevated treatment
- [ ] Light theme looks correct
- [ ] `prefers-reduced-motion` removes animations

### File consolidation (Task 2)
- [ ] Single `#chat-file-btn` appears in header (not two separate buttons)
- [ ] Dropdown shows: Save as Markdown, Save as JSON, Import conversation
- [ ] Each dropdown item triggers the correct action
- [ ] Dropdown closes on outside click
- [ ] No console errors

### Shell width architecture (Task 3)
- [ ] `layout.css` has CSS variables `--sidebar-width-expanded` and `--sidebar-width-collapsed`
- [ ] No hardcoded `208px` or `68px` values remain in sidebar/content-area rules
- [ ] `chat-width-observer.js` exists and is imported in `bootstrap.js`
- [ ] Resizing the browser window triggers density class changes on `body` (verify in devtools)
- [ ] At ~650px content width: `body.shell-width-tight` is present
- [ ] At ~480px content width: `body.shell-width-very-tight` is present
- [ ] Sidebar expand/collapse correctly updates `content-area` width

### Toolbar density (Tasks 4 + 5)
- [ ] At comfortable width: full labels, normal padding, no clipping
- [ ] At tight width: buttons compact, name inputs shrink, no `Persona` clipping
- [ ] At very-tight width: secondary labels hidden, font % hidden, name labels hidden
- [ ] Primary buttons (Behavior, Model, Style, Compact) always remain fully visible
- [ ] Font +/- buttons always remain visible
- [ ] Name inputs always remain accessible
- [ ] Explicit toggle always remains visible

### Chat Focus Mode (Task 6)
- [ ] `#chat-focus-mode-btn` visible in right cluster of chat header
- [ ] Clicking button enters focus mode with smooth animation
- [ ] `.endpoint-health-strip` collapses (height 0)
- [ ] `.top-nav-bar` collapses (height 0)
- [ ] `.sidebar-nav` collapses (width 0)
- [ ] `.content-area` fills full viewport width
- [ ] `.chat-sessions-panel` remains visible — session switching works in focus mode
- [ ] `#chat-focus-mode-btn` shows active (purple) state while in focus mode
- [ ] Hover over top edge of viewport reveals exit pill with animation
- [ ] Clicking exit pill exits focus mode with smooth animation
- [ ] Cmd/Ctrl+Shift+F enters focus mode
- [ ] Cmd/Ctrl+Shift+F again exits focus mode
- [ ] Pressing Escape does NOT exit focus mode (Escape is for modals)
- [ ] Switching to Server/Logs/Settings tab auto-exits focus mode
- [ ] Returning to Chat tab does NOT re-enter focus mode automatically
- [ ] Focus mode state survives page reload
- [ ] Toast hint appears on entry (not on restore)
- [ ] `prefers-reduced-motion`: transitions instant, no animation
- [ ] All chat controls function normally inside focus mode
- [ ] Context notes sidebar (`.chat-sidebar`) works normally inside focus mode
- [ ] Pinned telemetry inline (`#chat-telemetry-inline-host`) visible inside focus mode

### Draggable sidebar (Task 7)
- [ ] Resize handle visible on right edge of sidebar nav
- [ ] Handle cursor changes to `col-resize` on hover
- [ ] Dragging changes sidebar width live
- [ ] Width respects min (140px) and max (320px) constraints
- [ ] Dragging below ~100px snaps sidebar to collapsed state
- [ ] Width persists in `appNavWidth` localStorage key across reloads
- [ ] Expand/collapse toggle still works correctly after a resize
- [ ] Width observer re-evaluates density after a resize

### Popover hardening (Task 8)
- [ ] Telemetry popover does not overflow viewport at tight widths
- [ ] Persona menu anchors to right and does not clip off screen
- [ ] File dropdown does not clip
- [ ] All popovers tested at comfortable, tight, and very-tight density tiers
- [ ] All popovers tested with focus mode active (full width)

### Cross-cutting
- [ ] No JS console errors on any code path
- [ ] Light theme: new elements in Tasks 2, 6, 7 have light theme overrides
- [ ] `prefers-reduced-motion` coverage for all new animations (Tasks 1, 6, and toolbar density transitions)
- [ ] Focus Mode in light theme looks correct

---

## Risks and Mitigations

**Risk: `max-height` animation for `.endpoint-health-strip` / `.top-nav-bar` is imprecise.**
Mitigation: Set `max-height: 200px` conservatively. If flicker appears, switch to `transform: translateY(-110%) → translateY(0)` (Option B in the original planning doc) for those two elements while keeping `max-height` for `.sidebar-nav`.

**Risk: `content-area` transition conflicts with existing sidebar collapse animation.**
Mitigation: Scope the focus-mode transition to only `body.chat-focus-mode .content-area`. The existing collapse CSS targets `body.sidebar-collapsed .content-area` — different class, no conflict.

**Risk: ResizeObserver fires rapidly during drag resize and causes density class churn.**
Mitigation: The `THRESHOLDS` in `chat-width-observer.js` use tier bands, not continuous values. A `4px` jitter at a threshold boundary would at most toggle one class. If jitter is visible, add a debounce (10–20ms) on the observer callback.

**Risk: Draggable sidebar conflicts with persisted `default_sidebar_width` in `src/state.rs`.**
Mitigation: `default_sidebar_width` in state.rs is the notes sidebar width (per session, server-persisted). `appNavWidth` in localStorage is the main nav width (client-only). Different names, different storage mechanisms — no conflict. Do not rename either.

**Risk: Focus Mode FOUC (flash of unstyled content) on page reload.**
Mitigation: In `initChatFocusMode()`, read localStorage synchronously and call `enterFocusMode(true)` (silent) before the first paint. Because `bootstrap.js` runs at DOMContentLoaded, this happens early enough to prevent flash.

**Risk: CSS variable changes to `.sidebar-nav` break the existing collapse animation.**
Mitigation: The existing collapse animation uses a CSS `transition` on `width`. After Task 3, width is set via `var(--sidebar-width-expanded)` and `var(--sidebar-width-collapsed)`, and the transition still applies — the browser interpolates between custom property values normally.

---

## Open Implementation Decisions

These are not blockers but should be decided during implementation:

- **Focus Mode toast:** Exact `showToast` call signature — read `static/js/features/toast.js` to determine (duration, message, type).
- **Density thresholds:** `700px` (tight) and `520px` (very-tight) are starting values. Tune visually by testing with sidebar expanded and browser at 900px, 800px, 700px.
- **`prefers-reduced-motion` for density tiers:** Currently Tasks 4 and 5 add no animations — only `display`/sizing changes. If transitions are added to button padding changes in the future, add reduced-motion coverage then.
- **Focus Mode in `compact.html`:** Out of scope. `compact.html` is already minimal.
- **Session modal premium styling (`#session-modal`):** Intentionally deferred — low priority, not in this branch.
