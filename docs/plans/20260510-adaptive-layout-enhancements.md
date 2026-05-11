# Adaptive Layout Enhancements

**Date:** 2026-05-10
**Status:** Planning
**Priority:** High
**Scope:** Chat toolbar responsiveness, shell width model, left sidebar resizing, Chat Focus Mode
**Depends on:** `docs/plans/20260510-chat-system-evolution.md` should be implemented first. Several sections in this plan reference post-evolution DOM structure (`.chat-sessions-panel`, `.chat-main-area`). Implementing adaptive layout before the evolution is possible but will require re-validation of DOM targets after the evolution lands. See the App Architecture Reference section for pre- vs. post-evolution structural differences.

---

## Executive Summary

The layout architecture needs improvements across two dimensions: **width** and **height/immersion**.

**Width dimension:** The current chat toolbar is built as a desktop-first, non-wrapping row that only falls back to compact behavior at the mobile breakpoint. When the main left sidebar is expanded, the shell immediately loses 140px of width, but the chat header does not enter any intermediate compact state. The result is control clipping on medium-width desktop layouts, most visibly around the `Persona` button.

**Height/immersion dimension:** There is currently no way for a user engaged in active chat to remove the persistent structural chrome — the "Llama Monitor" title bar, cockpit metrics, and the endpoint/agent status strip — from view. These elements consume significant vertical space and visual attention that is irrelevant to the act of chatting. A **Chat Focus Mode** should allow the user to enter a full-screen-style chat experience: the app-level chrome collapses (top strip, title bar, left nav sidebar), leaving the chat session panel (for switching between conversations), the chat header controls, the message thread, and the input area. The goal is to maximize visible message content — the user should see more of their conversation at once, with the app chrome out of the way, while retaining the ability to switch chats without exiting Focus Mode.

Neither dimension should be fixed in isolation. Both share the same underlying philosophy: measure available space, respond progressively, and let the user control their own environment.

The long-term trigger for responsive density should be a `ResizeObserver` on the rendered content area or chat header, not viewport width guesses alone. Chat Focus Mode should layer on top of this system cleanly.

---

## Problem Statement

### Width failure mode

The chat header is rendered as a two-sided flex row in [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:626) and styled in [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:259). Both left and right clusters are simple non-wrapping flex containers in [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:269). There is no intermediate desktop compaction state.

The only meaningful responsive fallback happens at the mobile breakpoint in [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:2297), where labels, name inputs, and font controls are hidden outright. That breakpoint is too late for the actual failure case: desktop browser width reduced while the main sidebar remains open.

### Why the issue appears with the left sidebar open

The shell sidebar uses a hard-coded expanded width of `208px` in [static/css/layout.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/layout.css:685) and a collapsed width of `68px` in [static/css/layout.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/layout.css:710). The content area mirrors this in [static/css/layout.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/layout.css:874) and [static/css/layout.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/layout.css:885).

That means the expanded sidebar steals 140px of width before the chat header changes behavior at all.

### Why a local patch is not enough

The `Persona` button clipping is just the most obvious symptom. The actual problem is:

- the shell width model is binary and rigid
- the toolbar has no progressive density system
- several secondary controls permanently consume space
- popovers also assume generous minimum widths

If only the Persona button is patched, the next failure will show up in another header control or popover.

### Height/immersion failure mode

The full structural chrome of the app — the top strip, the title bar, and the sidebar nav — is always present, even when the user has no interest in switching tabs, checking server status, or navigating to another section. During an active chat session, this chrome:

- consumes vertical space that could display more messages
- introduces visual noise irrelevant to the current task
- creates a "tool" feeling rather than an "experience" feeling

There is no current mechanism to suppress this chrome or enter any kind of focused writing view. A premium 2026 chat interface should offer an immersive mode that feels as refined as entering a full-screen writing application.

---

## Goals

### Primary goals

- Prevent clipping of chat toolbar controls at intermediate desktop widths.
- Make the header react to actual available content width, not only viewport width.
- Preserve primary authoring actions longer than secondary utility actions.
- Support a future-proof shell that still behaves correctly if the sidebar becomes resizable.
- Provide a **Chat Focus Mode** that hides all non-chat structural chrome with a smooth, premium transition.
- Allow the user to toggle Focus Mode on/off without disrupting chat state.

### Secondary goals

- Reduce duplication in file actions by consolidating export/import.
- Demote low-priority controls under constrained widths.
- Improve consistency between shell layout state and chat layout state.
- Make Focus Mode an escape hatch that integrates naturally with the width density system.

### Non-goals

- Full visual redesign of the chat header.
- Reworking persona/template behavior beyond button/menu presentation.
- Replacing the current mobile layout strategy.
- A full-screen mode that removes browser chrome (that is OS-level and out of scope).

---

## App Architecture Reference

> This section exists because the implementing agent may not have prior context on the app. Read it before making any file changes.

This is a **Tauri/Rust desktop application** with a vanilla JavaScript frontend. There is no React, Vue, or Angular. All UI is managed through direct DOM manipulation and CSS class toggling. The frontend lives in `/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/`.

### HTML entry points

- `/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html` — main app shell
- `/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/compact.html` — compact/embedded variant (not the focus of this plan)

### Top-level layout layers (in DOM order)

The following elements are the ones this plan touches most. Understand their relationships before editing anything.

> ⚠️ **Two structural states exist depending on which feature branch has landed.** This plan is written to be implemented *after* `docs/plans/20260510-chat-system-evolution.md`. Verify which state the codebase is in before starting any work.

**Pre-evolution (current `feature/chat-guided-generations` branch):**

```
<body>
  ├── .endpoint-health-strip          ← TOP STATUS BAR (endpoint + agent info)
  │     ├── .endpoint-health-strip-setup   (setup state)
  │     └── .endpoint-health-strip-monitor (monitor state: URL, status, agent, latency, badges)
  ├── .top-nav-bar                    ← TITLE BAR (Llama Monitor logo, h1, cockpit metrics, user menu)
  ├── .main-layout                    ← WRAPPER containing sidebar + content
  │     ├── .sidebar-nav              ← LEFT NAVIGATION (Server, Chat, Logs, Settings buttons)
  │     └── .content-area             ← ALL PAGE CONTENT (tabs switch inside here)
  │           └── .page.chat-page     ← CHAT SECTION
  │                 ├── #chat-tab-bar         ← horizontal tab bar (REMOVED post-evolution)
  │                 ├── #chat-header
  │                 ├── #chat-telemetry-inline-host
  │                 ├── #ctx-pressure-bar
  │                 ├── .chat-messages (with .chat-sidebar overlay)
  │                 └── #chat-input-row
```

**Post-evolution (after `chat-system-evolution` plan lands — target structure for this plan):**

```
<body>
  ├── .endpoint-health-strip          ← TOP STATUS BAR (same as above)
  ├── .top-nav-bar                    ← TITLE BAR (same as above)
  ├── .main-layout
  │     ├── .sidebar-nav              ← LEFT NAVIGATION (Sessions/Models removed; Server, Chat, Logs, Settings remain)
  │     └── .content-area             ← ALL PAGE CONTENT
  │           └── .page.chat-page     ← CHAT SECTION (flex-direction: row post-evolution)
  │                 ├── .chat-sessions-panel  ← 240px Discord-style session list (replaces #chat-tab-bar)
  │                 │     ├── .csp-header (title + collapse btn)
  │                 │     ├── .csp-actions (New Chat btn)
  │                 │     ├── .csp-search (search input)
  │                 │     └── .csp-list (session items with ctx pressure bars)
  │                 └── .chat-main-area       ← flex:1, all remaining chat content
  │                       ├── #chat-header
  │                       ├── #chat-telemetry-inline-host
  │                       ├── #ctx-pressure-bar
  │                       ├── .chat-messages (with .chat-sidebar overlay)
  │                       └── #chat-input-row
```

**Key structural differences to keep in mind:**
- `#chat-tab-bar` is gone post-evolution; session switching happens via `.chat-sessions-panel`
- `#page-chat` becomes a flex row (session panel + main area side by side) instead of a flex column
- The `ResizeObserver` for width density should target `.chat-main-area` post-evolution, not `.content-area`
- `.sidebar-nav` no longer has Sessions or Models buttons post-evolution (they move to Server tab modals)

### Layout CSS variables (current state — pre-refactor)

Sidebar widths are currently hard-coded in [static/css/layout.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/layout.css:685):

- expanded: `208px` at line 685
- collapsed: `68px` at line 710

Content area margins mirror these at lines 874 and 885. This duplication is what Phase 1 of the width work replaces with CSS variables.

### State management

All runtime state lives in [static/js/core/app-state.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/core/app-state.js). The primary chat state object is:

```javascript
export const chat = {
  busy: boolean,
  tabs: ChatTab[],
  activeTabId: string,
  activeTabIdx: number,
  tabsDirty: boolean,
  persistTimer: number | null,
  tabTrash: [...],
  disableAutoScroll: boolean,
  // ... more
};
```

There is no Redux, Zustand, or any reactive framework. UI updates are triggered by calling render functions explicitly. The binding registration system in [static/js/features/chat-state.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-state.js:11) wires `chat-render.js` callbacks into `chat-state.js` without creating circular imports.

### CSS approach

Vanilla CSS with custom properties. Key files:

- [static/css/tokens.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/tokens.css) — design tokens
- [static/css/layout.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/layout.css) — sidebar, pages, nav
- [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css) — all chat-specific styles
- [static/css/chat-guided-generation.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat-guided-generation.css) — suggestions + quick guide

State classes are applied to `body` (e.g., `body.sidebar-collapsed`) and read by descendant selectors. Focus Mode should follow this exact same pattern.

### Sidebar collapse state

Toggled by [static/js/features/nav.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/nav.js:35). Adds/removes `body.sidebar-collapsed`. Persisted to `localStorage` key `sidebarCollapsed`.

### Persisted width state (pre-existing, do not conflict with)

- `default_sidebar_width` in [src/state.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/state.rs:127)
- settings UI for sidebar width in [static/js/features/settings.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/settings.js:64)
- chat notes sidebar width in [static/js/features/chat-notes.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-notes.js:58)

Use a distinct key (`app_nav_width`) for the main left nav. Do not reuse any existing key.

---

## Current State Inventory

### Chat header structure

The current chat header contains:

- Left cluster in [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:627)
  - telemetry trigger
  - `Behavior`
  - `Settings`
  - `Style`
  - `Compact`
  - optional `Fix`
  - AI/You name inputs
  - explicit toggle
- Right cluster in [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:705)
  - font controls
  - `Export`
  - `Import`
  - `Persona`

Key styling references:

- header shell: [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:259)
- left/right row containers: [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:269)
- header button sizing/padding: [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:479)
- desktop/mobile compaction rule: [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:2297)

### Main width consumers

These are the largest persistent width costs:

- font controls in [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:3131)
- font value minimum width in [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:3162)
- name pill cluster in [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:3171)
- fixed `80px` name input width in [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:3199)
- separate export/import/persona buttons in [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:715)

### Existing action wiring

Current event wiring already supports a menu-based file action pattern:

- export button + dropdown in [static/js/features/chat-params.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-params.js:823)
- import as separate direct action in [static/js/features/chat-params.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-params.js:844)
- persona button + menu in [static/js/features/chat-params.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-params.js:976)

### Sidebar state today

Sidebar collapse state is toggled in [static/js/features/nav.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/nav.js:35) and expressed as `body.sidebar-collapsed` in [static/js/features/nav.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/nav.js:39).

---

## Design Principles

### 1. Width should be measured, not guessed

The toolbar should respond to the real width of the content area or chat header. This matters because future width changes may come from:

- browser resizing
- sidebar expanded/collapsed state
- resizable sidebar width
- future shell additions
- Chat Focus Mode (sidebar is fully removed from flow)

### 2. Progressive density beats abrupt mode switches

The current model is effectively:

- full desktop
- then sudden mobile collapse

The new model should add at least two intermediate density tiers before mobile.

### 3. Preserve authoring actions longer than utility actions

Under constrained width, the user should keep access to active chat composition tools before file-management or display-tuning tools.

### 4. Shared shell state should drive shared layout behavior

The chat header should not invent its own unrelated width heuristics. The shell and chat should agree on width state.

### 5. Focus mode is user-controlled immersion, not a feature toggle

Chat Focus Mode is not a "feature on/off" switch. It is a deliberate shift in the user's experience. The interaction for entering and exiting focus mode must feel intentional and premium — not accidental — and the exit affordance must be discoverable without being intrusive.

### 6. Focus mode interacts with the width system correctly

When Focus Mode is active, the left nav sidebar is removed from layout flow, giving the chat area 140–208px of additional horizontal space. The `ResizeObserver` will detect this and would normally re-evaluate density. Focus Mode should pin density to `comfortable` regardless of measured width, since the intent is a premium immersive experience. The session panel (`.chat-sessions-panel`) remains in flow and continues to occupy its 240px — this is intentional and does not need special handling by the width observer.

---

## Proposed Architecture — Width Dimension

## 1. Width State Model

Introduce width-density classes derived from actual measured width.

### Recommended trigger

Use a `ResizeObserver` on one of:

- `.chat-main-area` — **preferred post-evolution** (measures only the actual chat writing area, excluding the session panel)
- `#chat-header` — acceptable alternative; same width as `.chat-main-area` in the post-evolution layout
- `.content-area` — **pre-evolution only**; post-evolution this includes the session panel width and will produce incorrect density thresholds

The observer should compute width and apply density classes to `body` or to the chat root, for example:

- `shell-width-comfortable`
- `shell-width-tight`
- `shell-width-very-tight`

Optional additional state:

- `shell-sidebar-expanded`
- `shell-sidebar-collapsed`
- `shell-sidebar-resizing`

### Why `ResizeObserver`

This is preferred over pure viewport media queries because it reacts to real available width after all shell deductions. It also naturally supports a future draggable main sidebar without needing hard-coded "if sidebar open and viewport less than X" rules.

### Focus Mode override

When `body.chat-focus-mode` is active:

- Remove all `shell-width-*` classes
- Apply `shell-width-comfortable` unconditionally
- Suspend the `ResizeObserver` from changing density classes
- Resume observer (without the override) when focus mode exits

This ensures a clean experience: focus mode is never accidentally cramped because the viewport itself happens to be narrow.

### Fallback

Media queries may still exist for coarse mobile behavior, but they should become the outermost guardrail, not the main desktop compaction mechanism.

---

## 2. Toolbar Density System

The chat header should get progressive density tiers.

### Density tier A: comfortable

Current full desktop experience remains roughly intact.

### Density tier B: tight

Apply modest compaction without changing control semantics:

- reduce button horizontal padding from current [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:483)
- reduce inter-control gaps from current [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:273)
- allow left and right groups to wrap
- add `min-width: 0` to wrapping groups
- keep button labels visible for primary actions
- shrink name input widths from [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:3200)

### Density tier C: very tight

Demote secondary content:

- hide `AI` / `You` labels while keeping inputs
- hide font percentage value first
- convert file actions to a single `File` button
- switch lower-priority buttons to icon-only as needed
- keep `Persona` visible longer than file utilities

### Density tier D: near-mobile desktop

If still constrained:

- collapse name editing further into a lighter affordance
- move font controls into a popover or existing `Style` surface
- keep only primary authoring actions directly exposed

### Mobile

The existing mobile behavior in [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:2297) can remain, but should be validated against the new density system so rules do not fight each other.

---

## 3. Toolbar Priority Model

The implementation should explicitly rank controls by survival priority.

### Priority 1: keep visible longest

- telemetry trigger
- `Behavior`
- `Settings`
- `Style`
- `Compact`
- `Persona`

### Priority 2: compress before removing

- AI/You names
- explicit toggle

### Priority 3: demote or consolidate early

- font controls
- export/import

### Priority 4: overflow candidates

- font controls
- file actions
- name editing affordances if space becomes extremely constrained

This priority order should be documented in code comments near any density logic so future changes do not regress it accidentally.

---

## 4. File Action Consolidation

### Proposed change

Replace separate `Export` and `Import` buttons with a single `File` button and dropdown.

### Why

- saves a full toolbar pill
- fits the existing dropdown interaction pattern
- lowers visual clutter
- treats file management as a grouped utility action rather than a primary composition action

### Menu contents

- `Save as Markdown`
- `Save as JSON`
- `Import conversation`

### Icon guidance

Do not use a disk icon by itself. A disk reads as save/export only.

Prefer:

- neutral file icon
- tray/archive icon
- import/export arrows combined with a file metaphor

### Implementation references

- existing export menu wiring: [static/js/features/chat-params.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-params.js:823)
- existing import action: [static/js/features/chat-params.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-params.js:844)

---

## 5. Name Editing Strategy

### Current issue

The name pills are visually strong but consume a lot of permanent horizontal space:

- two pills
- two labels
- two `80px` inputs
- explicit toggle in the same cluster

### Recommended progression

Tiered behavior under constrained width:

1. shrink input widths
2. hide `AI` / `You` labels
3. reduce pill padding
4. if still needed, replace full inline inputs with smaller chips or a dedicated `Names` affordance that opens inline editing

### Important constraint

Do not silently remove the feature. If inline editing is demoted, there must still be an obvious way to edit names.

---

## 6. Font Control Strategy

### Current issue

Font controls are useful but not core to active prompt composition, and they cost permanent header width.

### Recommended progression

1. keep current controls in comfortable width
2. hide the numeric percentage in tight widths
3. if needed, replace with a single compact button or move the controls into `Style`

The preferred "correct" destination is likely the `Style` surface if that panel already owns other appearance-related behavior.

---

## 7. Main Left Sidebar Resizing

### Recommendation

Add a draggable resize handle to the main left navigation shell.

### Why

- gives users direct control over tradeoff between nav readability and content width
- aligns with existing app behavior where the chat input already supports resizing
- works naturally with a `ResizeObserver`-driven width model

### Implementation model

Use the chat input resize handle in [static/js/features/chat-params.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-params.js:894) as interaction precedent, not as a copy-paste target.

The main nav should gain:

- visual resize handle along its right edge
- persistent width value
- min/max constraints
- collapse behavior that still snaps to collapsed width

### Proposed sidebar variables

Move shell layout off hard-coded widths and onto CSS variables, for example:

- `--sidebar-width-expanded`
- `--sidebar-width-collapsed`
- `--sidebar-width-active`

Then use those variables in:

- `.sidebar-nav`
- `.content-area`

instead of duplicating numeric values in [static/css/layout.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/layout.css:694) and [static/css/layout.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/layout.css:880).

### Persistence

Before implementation, reconcile the existing width-related state:

- `default_sidebar_width`
- any per-tab `sidebar_width`
- chat-notes sidebar width storage

The main left nav must use a distinct storage key and naming scheme so it does not conflict with the chat notes sidebar.

Suggested naming:

- `app_nav_width`
- `appNavWidth`

Do not reuse the chat notes sidebar width key.

---

## 8. Popover and Menu Width Audit

This width pass should also fix obvious popovers that assume too much space.

### Known issue

The telemetry popover has `min-width: 520px` in [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:359). That is likely to cause poor behavior on narrow desktop widths even after the toolbar itself is fixed.

### Additional checks

- persona menu anchoring at [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css:811)
- export/file dropdown alignment
- any right-edge clipping when menus are opened in tight density modes

---

## Proposed Architecture — Chat Focus Mode

## 9. Chat Focus Mode Overview

Chat Focus Mode is an immersive view that removes all structural application chrome and presents the chat interface at maximum size. It is analogous to a "distraction-free" mode in premium writing apps — but it retains session switching so the user is never locked into a single conversation.

### What stays visible

Everything that is intrinsic to the chat experience remains fully functional:

- `.chat-sessions-panel` — session list for switching between conversations (post-evolution). Pre-evolution equivalent: `#chat-tab-bar`. **This stays visible.** The user needs to be able to switch chats without exiting Focus Mode.
- `#chat-header` — all controls: Behavior, Settings, Style, Compact, Fix, name inputs, explicit toggle, font controls, export/import (or File), Persona, Focus Mode toggle button
- `#chat-telemetry-inline-host` — if the user has pinned telemetry inline, it remains
- `#ctx-pressure-bar` — context pressure indicator
- `.chat-messages` — the full message thread (the primary beneficiary of Focus Mode — more vertical space means more visible messages)
- `.chat-sidebar` — the context notes sidebar, if open (user controls this independently)
- `#chat-input-row` — input textarea, suggestions, quick guide, enter toggle, send button

### What gets hidden

The app-level chrome that is irrelevant to the chatting task:

- `.endpoint-health-strip` — the full top status bar (endpoint mode, URL, status, agent info, latency, badges). Defined in [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:59). **Hidden.**
- `.top-nav-bar` — the "Llama Monitor" title bar, cockpit metrics, user menu. Defined in [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:95). **Hidden.**
- `.sidebar-nav` — the entire left navigation sidebar (Server, Chat, Logs, Settings). Defined in [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:138). **Hidden.**

### What the layout becomes

When Focus Mode is active:
- `.sidebar-nav` collapses to width 0 and `.content-area` expands to fill the full viewport width
- `.endpoint-health-strip` and `.top-nav-bar` collapse to height 0, giving the chat full vertical space
- Inside `.content-area`, the session panel (`.chat-sessions-panel`) and `.chat-main-area` remain in their normal flex-row arrangement — the session panel just has more room since it no longer shares the viewport with app nav
- Net result: the user gets the full screen minus 240px (session panel) dedicated to the message thread and input

---

## 10. Focus Mode Toggle — Interaction Design

### Entry affordance

Add a **focus mode button** to the right cluster of `#chat-header`, as the last button in the row. This gives it the least intrusive position while remaining permanently accessible.

**Visual design:**
- Icon: a "maximize/expand corners" glyph or a "focus lens" symbol. Do not use a full-screen icon (which implies OS-level full screen). A good candidate is four outward-pointing corner arrows, similar to a content-expand icon. Lucide's `Expand` or `Maximize2` icons work well; if using inline SVG, draw a simple four-corner outward-arrow.
- Label: `Focus` at comfortable widths, icon-only at tight widths (priority 3 — it should still be accessible)
- Button ID: `#chat-focus-mode-btn`
- Apply the same button styling class used by Export/Import/Persona

**Keyboard shortcut:**
- `Ctrl+Shift+F` (Windows/Linux) / `Cmd+Shift+F` (Mac) — a deliberate chord that avoids browser conflicts
- Wire this in the same location as other keyboard shortcuts. Search [static/js/bootstrap.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/bootstrap.js) or [static/js/features/nav.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/nav.js) for existing keyboard event listeners and add the new handler there.

### Exit affordance — the critical UX requirement

Exiting focus mode must be discoverable without being visually loud. The wrong solution is an always-visible UI element that clutters the focus experience. The right solution is a **hover-reveal top edge beacon**.

**Recommended implementation:**

When `body.chat-focus-mode` is active:

1. Render a `#focus-mode-exit-beacon` element: a thin horizontal bar pinned to the very top of the viewport. It should be `height: 4px` and `width: 100%`, `position: fixed`, `top: 0`, `z-index` above all other content.
2. On hover of the beacon, animate a pill or banner downward into view: "Exit Focus Mode ✕" with a subtle backdrop. This pill should have an `opacity: 0` → `opacity: 1` + `translateY(-100%)` → `translateY(0)` transition, duration 200ms.
3. Clicking the beacon pill (or pressing the keyboard shortcut again, or pressing `Escape`) exits focus mode.

The beacon is `pointer-events: all` only during focus mode and `pointer-events: none` otherwise.

**Alternative exit methods (implement all three):**
1. Hover-reveal beacon (primary — most discoverable for new users)
2. Same keyboard shortcut as entry (primary — fastest for power users)
3. The `#chat-focus-mode-btn` in the chat header remains visible inside focus mode and can be clicked again to exit

**Do not** show a persistent "EXIT FOCUS MODE" button at all times — that defeats the purpose.

---

## 11. Focus Mode CSS Architecture

### Body class

Apply a single class to `<body>` when focus mode is active:

```css
body.chat-focus-mode
```

This follows the exact same pattern as `body.sidebar-collapsed` and keeps the selector surface clean.

### Hidden element transitions

The following should animate out with smooth transitions. Use `height + opacity + overflow: hidden` rather than `display: none` (which cannot animate) or `visibility: hidden` (which leaves layout ghost space). For elements with `overflow: hidden`, height must transition from its natural value to `0`.

However, because natural element heights are difficult to animate with pure CSS (you cannot transition `height: auto`), use one of two approaches:

**Option A (simpler): `max-height` animation**

```css
/* layout.css — add to existing rules */
.endpoint-health-strip,
.top-nav-bar {
  max-height: 200px; /* larger than actual height */
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

.sidebar-nav {
  /* existing width transition setup — currently hard-coded, to be replaced by Phase 1 CSS vars */
  transition: width 0.28s cubic-bezier(0.4, 0, 0.2, 1),
              opacity 0.22s ease;
  overflow: hidden;
}

body.chat-focus-mode .sidebar-nav {
  width: 0 !important;
  opacity: 0;
  pointer-events: none;
}

.content-area {
  transition: margin-left 0.28s cubic-bezier(0.4, 0, 0.2, 1);
}

body.chat-focus-mode .content-area {
  margin-left: 0 !important;
}
```

**Option B (smoother): `transform: translateY` + clip-path**

For the header strip and title bar, translate them upward out of the viewport rather than collapsing height. This avoids `max-height` imprecision and produces a more premium slide effect.

```css
.endpoint-health-strip,
.top-nav-bar {
  transform: translateY(0);
  opacity: 1;
  transition: transform 0.28s cubic-bezier(0.4, 0, 0.2, 1),
              opacity 0.2s ease;
}

body.chat-focus-mode .endpoint-health-strip,
body.chat-focus-mode .top-nav-bar {
  transform: translateY(-110%);
  opacity: 0;
  pointer-events: none;
  position: fixed; /* prevent layout reflow during animation */
  top: 0;
  left: 0;
  right: 0;
  z-index: -1;
}
```

**Recommendation: use Option A for simplicity on first implementation.** If the `max-height` flicker is noticeable during testing, switch the strip and title bar to Option B.

### Focus mode exit beacon

```css
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
```

The pill label should read: `Focus Mode — click to exit  ×`

Use a dim, secondary text style, not a bold call-to-action. The experience should feel like discovering a hidden affordance, not a warning banner.

### `prefers-reduced-motion` coverage

All transition properties introduced by Focus Mode must have a reduced-motion alternative:

```css
@media (prefers-reduced-motion: reduce) {
  .endpoint-health-strip,
  .top-nav-bar,
  .sidebar-nav,
  .content-area,
  #focus-mode-exit-pill {
    transition: none !important;
  }
}
```

---

## 12. Focus Mode JavaScript

Create a new file: [static/js/features/chat-focus-mode.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-focus-mode.js)

### Responsibilities

- Toggle `body.chat-focus-mode` on/off
- Update button state (`#chat-focus-mode-btn` active style)
- Persist state to localStorage
- Wire the exit beacon and pill click handler
- Wire keyboard shortcut
- Integrate with tab switching (auto-exit focus mode if user switches away from chat tab)
- Notify the width observer to override density class

### Suggested API surface

```javascript
// chat-focus-mode.js

const STORAGE_KEY = 'llama-monitor-chat-focus-mode';

let focusModeActive = false;

export function initChatFocusMode() {
  // Create beacon + pill DOM if not present
  // Restore state from localStorage
  // Wire beacon click → exitFocusMode()
  // Wire pill click → exitFocusMode()
  // Register keyboard listener
}

export function enterFocusMode() {
  focusModeActive = true;
  document.body.classList.add('chat-focus-mode');
  updateFocusModeButton();
  localStorage.setItem(STORAGE_KEY, '1');
  // Notify width observer to pin comfortable density
}

export function exitFocusMode() {
  focusModeActive = false;
  document.body.classList.remove('chat-focus-mode');
  updateFocusModeButton();
  localStorage.removeItem(STORAGE_KEY);
  // Notify width observer to resume normal density computation
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
```

### Keyboard shortcut wiring

Find the existing `keydown` listener in [static/js/bootstrap.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/bootstrap.js) or [static/js/features/nav.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/nav.js). Add:

```javascript
if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'F') {
  e.preventDefault();
  toggleFocusMode();
}
```

### Tab-switch auto-exit

In [static/js/features/nav.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/nav.js) `switchTab()`, add:

```javascript
if (name !== 'chat' && isFocusModeActive()) {
  exitFocusMode();
}
```

This prevents the awkward state where the user switches to "Server" and the sidebar is still hidden.

### Width observer integration

In the width observer module (to be created during Phase 1), before applying density classes, check:

```javascript
if (isFocusModeActive()) {
  applyDensityClass('comfortable');
  return; // skip further computation
}
```

---

## 13. Focus Mode Button in HTML

Add to the right cluster of `#chat-header` in [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html), after the Persona button:

```html
<button id="chat-focus-mode-btn"
        class="chat-header-btn"
        title="Focus Mode (⌘⇧F)"
        aria-pressed="false"
        aria-label="Toggle Focus Mode">
  <!-- SVG: four outward-pointing corner arrows -->
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor"
       stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
    <path d="M1 5V1h4M9 1h4v4M13 9v4H9M5 13H1V9"/>
  </svg>
  <span class="chat-header-btn-label">Focus</span>
</button>
```

Add the exit beacon and pill just before `</body>`:

```html
<div id="focus-mode-exit-beacon" role="button" tabindex="-1" aria-label="Exit Focus Mode">
  <div id="focus-mode-exit-pill">Focus Mode — click to exit &nbsp;×</div>
</div>
```

---

## 14. Focus Mode State Diagram

```
[Normal View]
  │
  ├── user clicks #chat-focus-mode-btn
  │   OR presses Ctrl/Cmd+Shift+F
  │
  ▼
[chat-focus-mode active]
  - body.chat-focus-mode applied
  - .endpoint-health-strip hidden (animated, height → 0)
  - .top-nav-bar hidden (animated, height → 0)
  - .sidebar-nav collapsed (width → 0, animated)
  - .content-area expands (margin-left → 0, animated)
  - .chat-sessions-panel REMAINS VISIBLE (user can still switch chats)
  - width observer pinned to 'comfortable'
  - localStorage persisted
  │
  ├── user hovers top edge → beacon pill appears
  │   user clicks beacon pill → exit
  ├── user presses Ctrl/Cmd+Shift+F → exit
  ├── user clicks #chat-focus-mode-btn → exit
  └── user switches to non-chat tab → auto-exit
  │
  ▼
[Normal View restored]
  - body.chat-focus-mode removed
  - .endpoint-health-strip, .top-nav-bar animate back in
  - .sidebar-nav restores its prior width (expanded or collapsed)
  - .content-area margin restores
  - width observer resumes normal density computation
  - localStorage cleared
```

---

## 15. Focus Mode Active Button State

The `#chat-focus-mode-btn` needs a visual active state to communicate that Focus Mode is currently on. Follow the same pattern as other header buttons that have active states in this codebase.

```css
/* In chat.css */
#chat-focus-mode-btn.active {
  color: var(--color-primary, #a78bfa);
  background: var(--color-primary-muted, rgba(167, 139, 250, 0.12));
}

#chat-focus-mode-btn.active svg {
  stroke: var(--color-primary, #a78bfa);
}
```

This gives the user a persistent in-header signal that focus mode is active, complementing the hover-reveal beacon.

---

## 16. Focus Mode Interactions with Other Panels

### Session panel (`.chat-sessions-panel`)

The session panel remains fully visible and interactive in Focus Mode. This is intentional — the user must be able to switch between conversations without exiting Focus Mode. The 240px it occupies is part of the retained chat interface, not chrome.

Post-evolution note: the session panel has its own collapse button (`.csp-collapse-btn`). The user can optionally collapse it from inside Focus Mode for even more space. Focus Mode does not force it open or closed — it simply leaves it in whatever state the user had it.

### Context notes sidebar (`.chat-sidebar`)

The context notes sidebar remains visible if the user has it open. Focus Mode does not touch `.chat-sidebar`. An open context notes sidebar in a narrow viewport may feel tight, but the user opened both intentionally. No special handling needed. The width observer is still pinned to `comfortable` and the sidebar width is user-adjustable via its existing resize handle.

---

## Proposed File Touches

Expected implementation files:

- [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html)
- [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css)
- [static/css/layout.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/layout.css)
- [static/js/features/nav.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/nav.js)
- [static/js/features/chat-params.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-params.js)
- [static/js/features/chat-focus-mode.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-focus-mode.js) ← **new file**
- [static/js/bootstrap.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/bootstrap.js)

Possible supporting files:

- [static/js/features/settings.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/settings.js)
- [src/state.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/state.rs)
- [src/web/api.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/api.rs)

---

## Implementation Phases

## Phase 1: Shell width architecture

- introduce CSS variables for main nav width
- move shell layout off duplicated hard-coded widths
- add JS width observer and density-class application
- preserve existing collapse/restore behavior

**Deliverable:** the app exposes robust shell width state before UI compaction rules are rewritten.

## Phase 2: Chat toolbar density system

- make header groups wrap-capable
- add density-specific compaction rules
- define control priority behavior in CSS and JS
- preserve primary actions while constraining secondary ones

**Deliverable:** the toolbar no longer clips with the sidebar open at medium desktop widths.

## Phase 3: File action consolidation

- merge export/import into a single `File` control
- update dropdown and event handling
- remove redundant toolbar surface

**Deliverable:** file actions are grouped and consume less header width.

## Phase 4: Name and font demotion

- shrink names progressively
- move or compact font controls
- validate no important affordance becomes undiscoverable

**Deliverable:** secondary width consumers no longer crowd primary authoring controls.

## Phase 5: Draggable main sidebar

- add resize handle and persistence
- integrate resize events with shell width observation
- validate collapse/expand interactions

**Deliverable:** users can tune shell width directly without breaking the toolbar.

## Phase 6: Popover hardening

- audit telemetry and persona popovers
- ensure anchored menus fit in tight layouts
- reduce excessive `min-width` rules where needed

**Deliverable:** narrow desktop behavior remains stable beyond the toolbar itself.

## Phase 7: Chat Focus Mode

This phase can be implemented independently of Phases 1–6. It does not require the width observer or CSS variables to exist first. Implement it against the current layout and let Phase 1 integration (pinning density to `comfortable`) be added when Phase 1 lands.

### Step 1 — HTML

- Add `#chat-focus-mode-btn` to the right cluster of `#chat-header` in `index.html`
- Add `#focus-mode-exit-beacon` and `#focus-mode-exit-pill` before `</body>`

### Step 2 — CSS

- Add `body.chat-focus-mode` rules to `layout.css`:
  - collapse `.endpoint-health-strip` and `.top-nav-bar` via max-height + opacity transition
  - collapse `.sidebar-nav` width to 0 with opacity transition
  - expand `.content-area` to margin-left: 0
  - **do not touch** `.chat-sessions-panel` — it remains visible so the user can switch chats
- Add exit beacon and pill styles to `layout.css`
- Add `#chat-focus-mode-btn.active` state to `chat.css`
- Add `prefers-reduced-motion` overrides

### Step 3 — JS

- Create `chat-focus-mode.js` with `initChatFocusMode`, `enterFocusMode`, `exitFocusMode`, `toggleFocusMode`, `isFocusModeActive`
- Wire button click → `toggleFocusMode()`
- Wire beacon + pill click → `exitFocusMode()`
- Wire keyboard shortcut in bootstrap.js or nav.js
- Add tab-switch auto-exit in nav.js `switchTab()`
- Restore state from localStorage on `initChatFocusMode()`
- Import and call `initChatFocusMode()` from bootstrap.js

**Deliverable:** users can enter and exit a clean, immersive chat view with smooth transitions and multiple discoverable exit paths.

---

## Validation Checklist

### Layout behavior

- expanded sidebar + medium browser width does not clip `Persona`
- collapsed sidebar still looks intentional and not over-compacted
- toolbar transitions smoothly between density tiers
- no overlap between wrapped header rows

### Functional behavior

- export still works for both Markdown and JSON
- import still works after consolidation
- persona menu remains reachable at all density tiers
- font controls remain reachable somewhere if removed from the main row
- AI/You name editing remains available at all density tiers

### Sidebar behavior

- resize handle respects min/max bounds
- resized width persists across reloads
- collapsing and re-expanding restores the prior expanded width
- width observer updates density classes correctly during live dragging

### Focus Mode behavior

- entering focus mode hides `.endpoint-health-strip`, `.top-nav-bar`, and `.sidebar-nav` smoothly
- `.content-area` expands to fill full viewport width
- `.chat-sessions-panel` remains visible and fully interactive — session switching works inside Focus Mode
- switching to a different session from the panel does not exit Focus Mode
- all chat controls remain usable inside focus mode
- hover over top edge reveals exit pill with animation
- clicking exit pill exits focus mode
- keyboard shortcut enters and exits focus mode
- pressing the shortcut again while focus mode is active exits
- `#chat-focus-mode-btn` shows active state while focus mode is on
- switching to a non-chat tab (Server, Logs, etc.) auto-exits focus mode
- focus mode state persists across page reload
- restoring focus mode on load animates correctly (no flash of unstyled content)
- context notes sidebar remains functional inside focus mode
- telemetry inline mode remains visible if user had it pinned

### Cross-cutting UI checks

- telemetry popover does not render off-screen in tight widths
- persona menu anchor remains inside viewport
- light theme rules still cover new states including focus mode
- any new motion introduced by resize affordances has `prefers-reduced-motion` coverage
- Focus Mode transition has `prefers-reduced-motion` coverage (instant toggle)
- focus mode beacon is not visible or interactive in normal mode

---

## Risks and Mitigations

### Risk: CSS rule conflicts between density tiers and mobile rules

**Mitigation:** keep density rules explicitly ordered and scoped; document precedence.

### Risk: `ResizeObserver` creates flicker or class churn during dragging

**Mitigation:** snap to a small number of tier thresholds rather than continuously mutating styles.

### Risk: sidebar resize state collides with existing width persistence

**Mitigation:** use distinct state names and storage keys for the main app nav.

### Risk: moving controls into popovers harms discoverability

**Mitigation:** preserve primary actions in the row and only demote utilities.

### Risk: Focus Mode `max-height` animation has visible overshoot or imprecise timing

**Mitigation:** set `max-height` value conservatively high (200px). If still imprecise, switch to Option B (translateY) for the strip and title bar, keeping Option A for the sidebar width.

### Risk: Focus Mode restore on page load causes visible FOUC (flash of unstyled content)

**Mitigation:** In `initChatFocusMode()`, read localStorage synchronously and apply `body.chat-focus-mode` before the first paint. Add a `data-focus-mode-init` attribute to `<body>` inline in the HTML as a placeholder if needed, then remove it after JS init.

### Risk: exit beacon is too subtle and users do not discover it

**Mitigation:** show a brief "Focus Mode active" toast on entry using the existing toast system, with a hint: "Hover top edge or press Ctrl+Shift+F to exit." This one-time hint (dismissed by the toast timeout) teaches the exit gesture without permanently cluttering the UI.

### Risk: auto-exit on tab switch breaks user intent

**Mitigation:** if the user re-enters the chat tab, focus mode should not automatically re-activate. Only re-activate if the user triggers it themselves. The auto-exit is one-directional.

---

## Recommended Threshold Strategy

Exact numbers should be tuned during implementation, but the approach should be threshold-based rather than pixel-perfect per-control hacks.

Suggested starting point:

- `comfortable`: current default layout
- `tight`: first compaction tier
- `very-tight`: second compaction tier

The thresholds should be based on measured content width and tuned visually in-browser with the sidebar:

- expanded
- collapsed
- actively resized
- focus mode active (always comfortable)

Avoid hard-coding thresholds that assume the sidebar is only ever `208px` or `68px`.

---

## Open Questions For Implementation

- Should font controls move entirely into `Style`, or remain as a compact popover?
- Should AI/You names become a dedicated `Names` button at very tight widths, or remain inline as tiny chips?
- Should the density classes live on `body`, `.content-area`, or the chat root?
- Should the main sidebar width be globally shared across all views, or only affect the current session shell?
- Should Focus Mode be available in `compact.html` as well, or only `index.html`? (Likely only `index.html` — the compact view is already minimal.)
- Should Focus Mode ever restore automatically on page load if it was active before a reload? (Recommended: yes, for consistency — user chose this layout intentionally.)
- Should there be a visual indicator other than the active button state while in focus mode? A subtle ambient glow or dim border on the chat area could reinforce the mode without being intrusive.

These are implementation decisions, not blockers. The architectural direction is already clear.

---

## Recommended Next Step

When implementation begins for the width enhancements, the agent should start by introducing the shell width model and density classes first. Do not begin with a Persona-only CSS patch. The correct sequence is:

1. shell width variables and observer
2. toolbar density behavior
3. utility control consolidation/demotion
4. resizable main sidebar
5. popover cleanup

For Chat Focus Mode, which is an independent feature:

1. Create `chat-focus-mode.js` module
2. Add button to HTML
3. Add CSS rules to `layout.css` and `chat.css`
4. Wire keyboard shortcut and tab-switch auto-exit
5. Test entry/exit transitions at multiple viewport sizes
6. Validate all chat functionality works inside focus mode

Focus Mode can ship before or after the width phases — it has no hard dependency on them.
