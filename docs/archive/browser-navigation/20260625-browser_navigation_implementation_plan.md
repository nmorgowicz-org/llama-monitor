# Browser Navigation (Back/Forward) — Implementation Plan

**Status:** Implementation-ready  
**Branch target:** `main`  
**Based on:** `docs/plans/20260531-browser_navigation.md` (concept)  
**Created:** 2026-06-25

This document is the authoritative implementation spec. It must be detailed enough that an AI agent with no project context can implement it.

---

## Objectives

Implement browser-style navigation so that:

- Browser Back/Forward works within the app (does not exit the page).
- URLs are stable and meaningful (e.g., `/chat`, `/chat/:id`, `/logs`).
- Deep links and bookmarks work for top-level views and chat sessions.
- Existing navigation behavior and APIs are preserved (no UX breakages).

Non-goals:

- No server-side rendering.
- No new client-side frameworks (React/Vue/etc.).
- No query-string state persistence for advanced filters (separate concern).
- No new animations tied to routing.

---

## High-Level Approach

- Backend: Add a SPA fallback route so any GET path that is not `/api/*` or an existing static asset returns `index.html` with the same auth, CSP, and injected metadata as the current root route.
- Frontend: Introduce a minimal Router (History API–based) that:
  - Keeps URL in sync with navigation.
  - Hooks into `popstate` for Back/Forward.
  - Delegates to existing navigation functions; never reimplements them.

The system is strictly a thin routing layer over existing behavior.

---

## Backend Changes (Rust)

### Files

- `src/web/mod.rs`
- (Do not modify other web/routing files.)

### Goal

Add a catch-all SPA fallback that:

- Is tried only after all existing API and static asset routes.
- Only responds to GET.
- Respects auth (same guard as the existing index route).
- Reuses the same HTML/nonce/version/platform injection as the current root route.

### Current behavior (as of June 2026)

- `build_routes()` in `src/web/mod.rs` (around line 127) wires:
  - Index route: exact match at `/` using `index_route()` (around line 342).
  - Compact route: `/compact` (separate, its own CSP).
  - Static assets via generated `gen_routes::static_routes()`.
  - All `/api/*` routes via `api_routes()`.
- `handle_rejection()` returns JSON 404 for unmatched paths.
- `auth_guard()` (around line 198–237) already protects API and index routes.

Do not change these. The SPA fallback plugs into this existing chain.

### Required changes

1. Implement `spa_fallback_handler`:

   - It should:
     - Read environment and configuration.
     - Return the same injected `index.html` as `index_route` does:
       - Version injection
       - Platform annotation
       - Per-request CSP nonce
     - Be idempotent and share as much code as possible with the existing index route.

   Implementation guidance:
   - Either:
     - Extract the index-generation logic into a small shared function (preferred), or
     - Call the existing index-route logic from the fallback (if structurally convenient).
   - The handler signature must match:

     ```rust
     async fn spa_fallback_handler(
         _path: warp::path::FullPath
     ) -> Result<impl warp::Reply, warp::Rejection> {
         // Reuse index_route logic: generate HTML + CSP.
     }
     ```

2. Add the fallback route in `build_routes()` near the end:

   - Place it:
     - After index, compact, static, and API routes.
     - Before `.recover(handle_rejection)`.
   - Wrap it in the same `auth_guard()` used by existing index/API routes so:
     - Non-authenticated users cannot bypass login via `/chat` etc.

   Conceptual placement:

   ```rust
   let spa_fallback = warp::get()
       .and(warp::path::full())
       .and_then(spa_fallback_handler);

   // Combine:
   let routes = index_or_static_or_api_or_compact
       .or(spa_fallback)
       .with(auth_guard())
       .recover(handle_rejection);
   ```

   Adjust the exact `.or()` tree so that:
   - `/api/*` never falls to `spa_fallback`.
   - Static assets (`gen_routes::static_routes()`) never fall to it.
   - Only arbitrary GET paths like `/chat`, `/logs`, `/chat/someid`, etc. are caught.

3. Constraints:

   - Must not break:
     - Existing route smoke tests (in `src/web/api/mod.rs` or tests).
     - `/compact` route (must keep its own CSP).
     - Any `/api/*` endpoint.
   - Must not change behavior of:
     - `/` (root)
     - `/compact`
     - WebSocket at `/ws`

### Acceptance criteria

- Navigating to:
  - `/chat`
  - `/logs`
  - `/spawn`
  - `/settings`
  - `/chat/some-id-123`
  - Random paths like `/x/y/z`
  All:
  - Return 200 + SPA shell (same `index.html` template) when authenticated.
  - Respect existing auth: prompt/401 behavior when not authenticated, same as now.
- All existing `/api/*` routes:
  - Still return their JSON responses, not the SPA shell.
- Route tests:
  - Still pass with same 401/4xx semantics.
- No new logging or performance regressions.

---

## Frontend Changes (JavaScript)

### Core Principles

- DO NOT:
  - Rewrite existing navigation functions.
  - Introduce a new framework (React/Vue/Svelte/etc.).
  - Change how `switchView`, `switchTab`, or chat/session modules internally manage state.
- DO:
  - Add a small router that:
    - Uses `pushState`/`popstate`.
    - Maps URLs to existing functions.
  - Gradually replace direct calls to navigation functions with `Router.navigate(...)`.

### Files

- Create:
  - `static/js/features/router.js`
- Modify:
  - `static/js/bootstrap.js`
  - Any file that currently calls:
    - `switchView(...)`
    - `switchTab(...)`
    - `openSettingsModal()`
    - `openSpawnWizard()`
    - `switchChatTab(...)` for high-level navigation (see “Integration” below).

### 1. Router Implementation (`static/js/features/router.js`)

Create a minimal router with:

- `register(path, handler)`
- `navigate(path, { push? })` using `pushState` by default
- `popstate` listener
- Initial dispatch on load based on `location.pathname`
- Pattern matching for `/chat/:id`

The router must be safe for unknown paths: fallback to `/` route.

Implementation (baseline):

```js
// static/js/features/router.js

const Router = {
  routes: {},

  register(path, handler) {
    this.routes[path] = handler;
  },

  navigate(path, options = {}) {
    const push = options.push !== false;
    if (push) {
      history.pushState({ path }, '', path);
    } else {
      history.replaceState({ path }, '', path);
    }
    this._dispatch(path);
  },

  init() {
    const initialPath = location.pathname || '/';
    history.replaceState({ path: initialPath }, '', initialPath);
    window.addEventListener('popstate', () => {
      const path = location.pathname || '/';
      this._dispatch(path);
    });
    this._dispatch(initialPath);
  },

  _dispatch(path) {
    // 1) Exact match
    const exact = this.routes[path];
    if (exact) {
      exact(path);
      return;
    }

    // 2) Pattern match (e.g., /chat/:id)
    for (const key of Object.keys(this.routes)) {
      if (key.includes(':')) {
        const pattern = key
          .replace(/\//g, '\\/')
          .replace(/:[^/]+/g, '[^/]+');
        const re = new RegExp('^' + pattern + '$');
        if (re.test(path)) {
          this.routes[key](path);
          return;
        }
      }
    }

    // 3) Fallback to root route if defined
    if (this.routes['/']) {
      this.routes['/'](path);
      return;
    }
  }
};
```

Constraints:

- Must:
  - Avoid `eval`, `innerHTML`, and unsafe usage.
  - Treat path segments as untrusted (no direct `querySelector` with user-supplied path without sanitization).

### 2. Register Routes

At application initialization time (after modules are imported), register routes that map to existing functions.

Exact mapping:

- `/` → setup view (home)
  - Handler: `() => switchView('setup')`
- `/spawn` → spawn wizard
  - Handler: `() => openSpawnWizard()`
- `/chat` → conversations
  - Handler: `() => switchTab('chat')`
- `/logs` → logs page
  - Handler: `() => switchTab('logs')`
- `/settings` → settings modal
  - Handler: `() => openSettingsModal()`
- `/chat/:id` → specific chat session
  - Handler:
    - Extract id: `const id = path.split('/chat/')[1];`
    - If id is present and `switchChatTab` is available, call `switchChatTab(id)`.

Example (to be placed during bootstrap initialization):

```js
// After imports and before main UI init
Router.register('/', () => switchView('setup'));
Router.register('/spawn', () => openSpawnWizard());
Router.register('/chat', () => switchTab('chat'));
Router.register('/logs', () => switchTab('logs'));
Router.register('/settings', () => openSettingsModal());
Router.register('/chat/:id', path => {
  const id = path.split('/chat/')[1];
  if (id && typeof switchChatTab === 'function') switchChatTab(id);
});

Router.init();
```

Implementation notes:

- `switchView` lives in `setup-view.js`.
- `switchTab` lives in `nav.js`.
- `openSpawnWizard` lives in `spawn-wizard.js`.
- `openSettingsModal` lives in `settings.js`.
- `switchChatTab` lives in `chat-state.js`.

### 3. Bootstrap Integration (`static/js/bootstrap.js`)

Changes required:

- Import `router.js` as an ES module.
- Ensure `Router.init()` and registrations run:
  - Only after other feature modules are loaded.
  - Only in contexts where `window.history` and `pushState` are available.

Implementation guidance:

- Near the end of `bootstrap.js`’s initialization (after `initNav()` and similar), add:

  - `import Router from './features/router.js';` or however router is exported.
  - Register the routes using the examples above.

- Ensure:
  - No changes to existing auth or view-loading behavior.
  - No hard page reloads are introduced here.

### 4. Integration: Replace Direct Calls

Objective:

Make all major navigational actions go through `Router.navigate(...)` so the URL is kept in sync.

This is the bulk of Phase 1 work.

Rules:

- DO NOT:
  - Change the behavior of existing functions (e.g., `switchView`, `switchTab`).
  - Remove early guards, checks, or auth logic.

DO:
- Find direct calls to:
  - `switchView('setup')`
  - `switchView('monitor')` (map to `/` or appropriate route)
  - `switchTab('chat')`
  - `switchTab('logs')`
  - `openSpawnWizard()`
  - `openSettingsModal()`
- Replace them with Router calls:
  - `Router.navigate('/');` for setup view.
  - `Router.navigate('/spawn');` for spawn wizard.
  - `Router.navigate('/chat');` for chat.
  - `Router.navigate('/logs');` for logs.
  - `Router.navigate('/settings');` for settings.

Scope:

- Search in all `static/js/features/*.js` files.
- Do not replace:
  - Calls inside `router.js`.
  - Test-only harness code or comment-only usages.
- Be careful not to change unrelated logic on those lines.

Acceptance criteria:

- Clicking sidebar tabs (Server/Chat/Logs/Settings) updates URL.
- Opening spawn wizard updates URL to `/spawn`.
- Opening settings modal updates URL to `/settings`.
- Using Back/Forward:
  - Moves between views using those same routes.
  - Does not navigate away from the page (no blank/exit).

---

## Phase 2 — Chat Session Deep Links

Goal:

Make `/chat/:id` work so that:

- Opening `/chat/ABC123` in a browser:
  - Ensures monitor/chat view is active.
  - Loads and focuses the session with id `ABC123`.
- Chat session switching inside the app updates URL.

Steps:

- Extend router behavior for `/chat/:id`:

  - Ensure:
    - `switchView('monitor')` / equivalent is invoked if necessary.
    - `switchTab('chat')` is called.
    - `switchChatTab(id)` is called with the parsed id.

- Update major places that call `switchChatTab(id)` directly:
  - Replace with `Router.navigate('/chat/' + id);` where appropriate (e.g., clicking session items in the sessions sidebar).

Constraints:

- Validate/sanitize the id:
  - Do NOT allow HTML entities or weird characters.
  - Simple rule: if `id` contains `[<>"]` or starts with `//` or contains `..`, treat as invalid and fallback to generic chat view.

Acceptance criteria:

- Pasting `/chat/real-tab-id` into the browser:
  - Opens the dashboard in chat page and selects that tab.
- Using Back/Forward when switching between chat sessions respects history.

---

## Phase 3 — Wizard Sub-Steps (Optional)

Only if desired; low priority.

- Add:
  - `/spawn` as base.
  - Optionally `/spawn/hardware`, `/spawn/review`.
- When the wizard moves steps, call `Router.navigate('/spawn/...', {push: true})`.

No strict acceptance criteria required; keep in sync with wizard UX.

---

## Security and Safety Constraints

Apply these rules globally:

- Auth:
  - SPA fallback must respect `auth_guard()`.
- Secrets:
  - Do not log tokens, URLs with tokens, or sensitive config.
- Inputs:
  - Never use path segments directly in:
    - `innerHTML`
    - `insertAdjacentHTML`
    - Unescaped `document.write`
  - Prefer `textContent` and attribute-safe APIs.
- CSP:
  - No `'unsafe-inline'` for scripts.
- Rust:
  - No direct file system manipulation of live SQLite.
  - Use `#[serde(default)]` where applicable if new structs are added (none required here).

---

## Pre-PR Validation Checklist

Before marking implementation as ready:

- [ ] Rust:
  - [ ] `cargo build --release` passes.
  - [ ] `cargo clippy -- -D warnings` passes.
  - [ ] `cargo test` passes (no known test failures).
  - [ ] SPA fallback:
    - [ ] Tested with:
      - `/chat`
      - `/logs`
      - `/settings`
      - `/chat/someid`
      - `/x/y/z`
    - [ ] All return SPA shell and respect auth.
    - [ ] Existing `/api/*` endpoints remain unaffected.

- [ ] JS:
  - [ ] All modules using `switchView`/`switchTab`/`openSpawnWizard`/`openSettingsModal` updated.
  - [ ] `Router.init()` runs once during bootstrap.
  - [ ] No console errors related to router or popstate.
  - [ ] `npm run validate-js` passes.
  - [ ] `npm run lint` passes.

- [ ] E2E / UI:
  - [ ] Run Playwright:
    - [ ] `cd tests/ui && CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 npm test`
    - [ ] No new failures (allow updating tests if absolutely required; no silent breakage).
  - [ ] Browser Back/Forward:
    - [ ] Confirmed functional in main flows (chat, logs, spawn, settings).
  - [ ] Deep links:
    - [ ] `/chat` opens chat view.
    - [ ] `/logs` opens logs.
    - [ ] (Phase 2) `/chat/:id` opens specific session.

- [ ] Docs:
  - [ ] Update:
    - [ ] `docs/reference/chat.md` (if relevant).
    - [ ] `docs/reference/dashboard.md` (mention navigation and URLs).
  - [ ] No screenshots committed to `docs/screenshots/` unless referenced.

---

## Notes for an AI Agent Implementing This

- Treat existing navigation functions as stable:
  - `switchView`
  - `switchTab`
  - `openSettingsModal`
  - `openSpawnWizard`
  - `switchChatTab`
- Your job is to:
  - Add the router.
  - Hook it into bootstrap.
  - Make sure every major navigation action goes through the router.
  - Ensure the backend serves SPA shell for arbitrary paths, respecting auth.
- Prefer small, focused changes; avoid sweeping refactors.
- If unsure, match the style and patterns in existing files.
- Always re-run the Pre-PR validation commands before considering the work complete.
