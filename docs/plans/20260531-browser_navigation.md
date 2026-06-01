# Browser Navigation (Back/Forward) — Concept Plan

**Status:** Concept — to be implemented after `feature/spawn-llama-server-v2` merges  
**Branch target:** `main` post-merge

---

## Problem

The app is a single-page application served at `http://localhost:7778`. The URL never changes, so:
- Browser back/forward leaves the app entirely
- No deep-linkable URLs (can't bookmark a chat session, share a settings link, etc.)
- Refreshing any in-app state loses position

---

## Approach: History API + warp catch-all

Use the browser's `history.pushState` / `popstate` for clean URLs. Requires a one-line warp change so unknown paths still return `index.html` rather than 404.

### Rust change (warp)

Add a catch-all at the end of the route chain in `src/web/server.rs` (or wherever routes are built):

```rust
// Must come last — serves index.html for any GET not matched by API/asset routes
let spa_fallback = warp::get()
    .and(warp::path::full())
    .and_then(serve_index_html);
```

This is intentionally the **last** route registered so API routes and static assets still take priority.

---

## URL Map

| Path | View |
|---|---|
| `/` | Dashboard (metrics, running model status) |
| `/spawn` | Spawn wizard — step 1 (model select) |
| `/settings` | Settings panel |
| `/chat` | Chat session list / new chat |
| `/chat/:id` | Specific chat session |
| `/models` | Model browser / download manager |
| `/logs` | Server logs (if applicable) |

Wizard sub-steps (`/spawn/hardware`, `/spawn/review`) are optional — the wizard is a linear flow so back within it could just use the Back button rather than the browser back button. Keeping the URL at `/spawn` throughout is fine initially.

---

## JS Router (minimal, ~60 lines)

No framework needed. A small router object:

```javascript
const Router = {
  routes: {},        // { '/path': handlerFn }
  register(path, fn) { this.routes[path] = fn; },
  navigate(path, replace = false) {
    if (replace) history.replaceState({path}, '', path);
    else history.pushState({path}, '', path);
    this._dispatch(path);
  },
  _dispatch(path) {
    // exact match, then prefix match, then fallback to '/'
    const handler = this.routes[path]
      ?? Object.entries(this.routes).find(([k]) => path.startsWith(k))?.[1]
      ?? this.routes['/'];
    handler?.(path);
  },
  init() {
    window.addEventListener('popstate', e => this._dispatch(e.state?.path ?? location.pathname));
    this._dispatch(location.pathname);
  }
};
```

Each view registers itself:
```javascript
Router.register('/spawn',    () => openSpawnWizard());
Router.register('/settings', () => openSettings());
Router.register('/chat',     (path) => openChat(path.split('/')[2] ?? null));
Router.register('/',         () => showDashboard());
```

Navigation calls replace `showTab()` / `openModal()` style calls with `Router.navigate('/spawn')`.

---

## Phased Rollout

**Phase 1 — Top-level views** (low risk)
- warp catch-all
- Router init + register dashboard, settings, spawn entry, models
- Replace existing view-switch calls with `Router.navigate`

**Phase 2 — Chat sessions**
- `/chat/:id` deep links
- Session restore on page load from URL

**Phase 3 — Wizard sub-steps** (optional / nice-to-have)
- `/spawn`, `/spawn/hardware`, `/spawn/review`
- Lets users copy a link mid-wizard (low value, low priority)

---

## What This Does NOT Do

- No server-side rendering — the server always returns `index.html`, state is client-only
- No query-string state persistence (model params, filter state) — that's a separate concern
- No animated transitions — those can be layered on independently

---

## Estimated Effort

| Phase | Rust | JS | Risk |
|---|---|---|---|
| 1 | ~5 lines | ~100 lines | Low |
| 2 | 0 | ~50 lines | Low–Medium (session ID handling) |
| 3 | 0 | ~30 lines | Low |
