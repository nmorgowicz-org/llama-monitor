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
    }
  },
};

export default Router;
