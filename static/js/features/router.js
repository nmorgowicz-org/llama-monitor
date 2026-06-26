// ── Client-side router (History API) ──────────────────────────────────────────
//
// Minimal router to enable:
// - Back/Forward within the app
// - Stable, meaningful URLs (e.g. /chat, /logs, /settings, /chat/:id)
// - Deep-linking and bookmarks
//
// Delegates to existing navigation functions; never reimplements them.

const Router = {
  routes: {},
  // For pattern routes: store { re, params } so handler receives { path, params }.
  patternRoutes: [],

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

    if (push) {
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

  _dispatch(path) {
    // 1) Exact match
    const exact = this.routes[path];
    if (exact) {
      exact(path);
      return;
    }

    // 2) Pattern match (e.g. /chat/:id) using precomputed regex.
    const pathParts = path.split('/');
    for (const pr of this.patternRoutes) {
      if (pathParts.length !== (pr.params.length + 1)) continue;
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

export default Router;
