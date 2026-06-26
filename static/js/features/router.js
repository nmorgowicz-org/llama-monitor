// ── Client-side router (History API) ──────────────────────────────────────────
//
// Minimal router to enable:
// - Back/Forward within the app
// - Stable, meaningful URLs (e.g. /chat, /logs, /settings, /chat/:id)
// - Deep-linking and bookmarks
//
// Delegates to existing navigation functions; never reimplements them.

import { chat } from '../core/app-state.js';

// Router is the single place that:
// - writes history (push/replace)
// - dispatches routes
// Other modules must not call history.* directly except via Router helpers.

const Router = {
  routes: {},
  // For pattern routes: store { re, params } so handler receives { path, params }.
  patternRoutes: [],
  // Optional hook run before every dispatch; used to dismiss open overlays
  // (settings/spawn modals) when navigating away so Back/Forward works.
  _beforeDispatch: null,

  // Register a callback invoked with the path about to be dispatched, before
  // the route handler runs.
  onBeforeDispatch(fn) {
    this._beforeDispatch = fn;
  },

  register(path, handler) {
    if (path.includes(':')) {
      // Precompute regex and param names once.
      const parts = path.split('/').filter(Boolean);
      const segments = [];
      const params = [];
      for (const p of parts) {
        if (p.startsWith(':')) {
          segments.push('([^/]+)');
          params.push(p.slice(1));
        } else {
          segments.push(this._escapeRegex(p));
        }
      }
      this.patternRoutes.push({
        re: new RegExp('^/' + segments.join('/') + '$'),
        params,
        handler,
      });
    } else {
      this.routes[path] = handler;
    }
  },

  navigate(path, options = {}) {
    const push = options.push !== false;
    const [pathOnly, hash] = path.split('#');
    const url = hash ? pathOnly + '#' + hash : pathOnly;

    // Avoid stacking duplicate history entries: re-navigating to the URL we're
    // already on (e.g. re-clicking the active tab) replaces instead of pushing,
    // so Back doesn't have to be pressed repeatedly to escape.
    const current = location.pathname + location.hash;
    if (push && url !== current) {
      history.pushState({ path: pathOnly }, '', url);
    } else {
      history.replaceState({ path: pathOnly }, '', url);
    }

    this._dispatch(pathOnly);
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

  // Update the URL to reflect the current underlying state without re-dispatching
  // the route handler. Used by chat-state.js and bootstrap.js to keep URLs
  // bookmarkable (e.g. /chat/<id>) while avoiding recursion through /chat/:id.
  updateUrlWithoutDispatch(path) {
    const [pathOnly] = (path || '/').split('#');
    try {
      history.replaceState({ path: pathOnly }, '', pathOnly);
    } catch {
      // If history API is blocked (e.g. file://), no-op.
    }
  },

  _dispatch(path) {
    // Let listeners dismiss overlays before the route handler runs.
    if (this._beforeDispatch) {
      try {
        this._beforeDispatch(path);
      } catch (e) {
        console.error('[router] beforeDispatch hook failed:', e);
      }
    }

    // 1) Exact match
    const exact = this.routes[path];
    if (exact) {
      exact(path);
      return;
    }

    // 2) Pattern match (e.g. /chat/:id) using precomputed regex.
    for (const pr of this.patternRoutes) {
      const m = pr.re.exec(path);
      if (!m) continue;
      const params = {};
      for (let i = 0; i < pr.params.length; i++) {
        params[pr.params[i]] = decodeURIComponent(m[i + 1]);
      }
      pr.handler(path, params);
      return;
    }

    // 3) Fallback to root route if defined
    if (this.routes['/']) {
      this.routes['/'](path);
    }
  },

  _escapeRegex(s) {
    return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  },
};

// Best-effort route for whatever top-level view is currently showing underneath
// any modal. Used when a modal closes so the URL returns to the underlying view
// without needing to re-dispatch (the underlying view never changed while the
// modal was open).
export function routeForCurrentView() {
  if (document.body.classList.contains('setup-active')) return '/';
  const page = document.querySelector('.page.active');
  switch (page && page.id) {
    case 'page-chat': {
      // Preserve the active session so closing a modal over a chat returns to
      // that exact conversation (bookmarkable / reload-safe).
      // Safe-read: chat may not be initialized in early boot edge cases.
      const id = (typeof chat !== 'undefined' && chat?.activeTabId) || null;
      return id ? '/chat/' + encodeURIComponent(id) : '/chat';
    }
    case 'page-logs':
      return '/logs';
    default:
      return '/server';
  }
}

export default Router;
