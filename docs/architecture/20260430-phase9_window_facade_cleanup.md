# Phase 9: Remove Window Facade and Inline Handlers

## Goal

Eliminate the `window.*` compatibility layer by converting all inline HTML handlers to `addEventListener` wiring in JS modules. This removes the global namespace pollution and completes the modular architecture.

## Current State (updated 2026-05-01)

After Phase 8, all feature code is in ES modules. The modules register functions on `window.*` so that inline HTML handlers (e.g., `onclick="switchTab('chat')"` in `index.html`) can call them. This creates a large compatibility surface:

- `bootstrap.js` imports all modules, each `initXxx()` puts functions on `window.*`
- `app.js` (legacy shim) calls 11 `init*()` functions not yet wired into bootstrap.js on DOMContentLoaded
- Inline handlers in `index.html` call `window.*` functions directly — **175 total** (up from estimated 150)
- **24 feature modules** (up from estimated 20) need conversion

## Approach

### Step 1: Audit inline handlers

Find all inline handlers in `static/index.html` and map each to its source module:

```bash
grep -n 'onclick\|onchange\|oninput\|onkeydown\|onkeyup\|onfocus\|onblur' static/index.html
```

For each handler, note:
- The HTML element (id, class, or selector)
- The function it calls
- The module that provides that function

### Step 2: Convert each handler to addEventListener

For each inline handler, add an equivalent `addEventListener` call in the appropriate module's init function. For example:

**Before (HTML):**
```html
<button onclick="switchTab('chat')">Chat</button>
```

**After (HTML):**
```html
<button data-tab="chat">Chat</button>
```

**After (nav.js):**
```js
document.querySelectorAll('[data-tab]').forEach(btn => {
    btn.addEventListener('click', () => switchTab(btn.dataset.tab));
});
```

### Step 3: Remove window exports

Once all inline handlers are converted, remove the `window.xxx = xxx` assignments from each module's `initXxx()` function. The functions are only called from within the module system.

### Step 4: Remove the legacy shim

Delete `static/app.js` entirely. The shim's DOMContentLoaded calls can be moved into `bootstrap.js` as a single `DOMContentLoaded` listener that calls the module init functions that need DOM-ready timing.

### Step 5: Consolidate init timing

Currently, some modules bind events on import (IIFE), and the shim calls init functions on DOMContentLoaded. Consolidate: all DOM-dependent init runs in `bootstrap.js` under a single `DOMContentLoaded` listener.

## Modules to Convert (estimated from grep)

| Module | Approx. inline handlers | Notes |
|--------|------------------------|-------|
| nav.js | ~5 | switchTab, toggleSidebarCollapse |
| settings.js | ~15 | saveSettings, open/close modals, applySettings |
| user-menu.js | ~10 | toggleUserMenu, openUserProfile, toggleTheme, etc. |
| config.js | ~5 | openConfigModal, saveConfig, usePathServerBinary |
| models.js | ~3 | openModelsModal, closeModelsModal, refreshModels |
| presets.js | ~8 | open/close presets modal, save/load presets |
| sessions.js | ~10 | spawn/attach session controls |
| attach-detach.js | ~3 | attach/detach buttons |
| file-browser.js | ~5 | file browser actions |
| remote-agent.js | ~25 | agent setup, SSH guide, install/start/stop |
| chat-state.js | ~8 | new/close/switch tabs, rename, clear |
| chat-transport.js | ~3 | sendChat, stopChat, sendSuggestedPrompt |
| chat-render.js | ~15 | copy, edit, delete, navigate variants |
| chat-templates.js | ~10 | template manager, explicit mode |
| chat-params.js | ~12 | param changes, compact, font, style |
| lhm.js | ~3 | LHM install/start/uninstall |
| setup-view.js | ~5 | view switching, quick stats |
| updates.js | ~5 | update check, release notes, self-update |
| shortcuts.js | ~1 | keyboard shortcuts modal |
| dashboard-ws.js | ~5 | dashboard actions |

**Total: ~150 inline handlers to convert**

## Exit Criteria

- No inline handlers remain in `static/index.html`
- No `window.xxx = xxx` assignments in any module
- `static/app.js` is deleted
- `bootstrap.js` has a single DOMContentLoaded listener that calls all init functions
- Build passes, clippy clean, 138 tests pass

## Risk Assessment

**High risk** — touching 150+ inline handlers across 20 modules. A missed handler is a silent UI regression.

**Mitigation:**
1. Run the audit first (Step 1) and create a checklist
2. Convert one module at a time, verifying in browser after each
3. Keep the inline handlers and window exports as a fallback until all conversions are verified
4. Run the Playwright smoke test after each module conversion

## Estimated Effort

This is a mechanical but tedious refactor. Each handler conversion is ~5 minutes (find, convert, verify). At 150 handlers, that's ~12.5 hours of focused work. Recommended to spread across multiple sessions.
