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

    // 2) Pattern match (e.g. /chat/:id)
    // Build regex per key: segment starting with ':' becomes ([^/]+), others are literal.
    for (const key of Object.keys(this.routes)) {
      if (key.includes(':')) {
        const parts = key.split('/').filter(Boolean);
        const pathParts = path.split('/').filter(Boolean);
        if (parts.length !== pathParts.length) continue;

        const segments = [];
        for (let i = 0; i < parts.length; i++) {
          if (parts[i].startsWith(':')) {
            segments.push('([^/]+)');
          } else {
            segments.push(this._escapeRegex(parts[i]));
          }
        }
        const re = new RegExp('^/' + segments.join('/') + '$');
        if (re.test(path)) {
          this.routes[key](path);
          return;
        }
      }
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
