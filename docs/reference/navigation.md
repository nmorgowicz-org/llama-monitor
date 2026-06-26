# SPA Navigation and Routing

This document describes the single-page application (SPA) routing model, the client-side router, backend SPA guard, and vendor asset strategy for Llama Monitor.

## SPA Routing

The UI is now a single-page app: index.html is served once and a client-side router handles navigation between major views. The page is not reloaded when moving between Dashboard, Chat, Logs, Server, and Spawn.

Public SPA routes:

- /
  - Home view (dashboard).
- /chat
  - Chat workspace; shows the last active tab or a neutral empty state.
- /chat/:id
  - Deep link to a specific conversation (by tab ID).
- /logs
  - Logs view.
- /server
  - Server tab.
- /spawn
  - Spawn wizard.

Behavior:

- Visiting these URLs from outside (bookmarks, browser history, external links) will:
  - Load the SPA shell.
  - Let the client-side router choose the correct view.
- The browser address bar always reflects the current view.
- Browser Back/Forward work via the router.

Deep linking to conversations:

- /chat/:id loads that conversation if the ID is valid.
- The ID must:
  - Start with "tab_"
  - Be 16–80 characters
  - Contain only letters, digits, underscores, or hyphens
- If the ID is invalid, the router treats it as suspicious:
  - Logs a warning.
  - Falls back to a safe chat view instead of loading the bogus ID.

## Client-Side Router

Implemented in: static/js/features/router.js

Key responsibilities:

- Manage URL and view in sync (no page reload).
- Intercept browser navigation.
- Provide helpers used by other frontend modules.

Key API:

- Router.navigate(path):
  - Primary way to change view and URL.
  - Triggers navigation hooks and updates the URL.
  - Used by top nav and modals.
- Router.updateUrlWithoutDispatch(path):
  - Updates the URL to reflect current view/tab without re-running full navigation.
  - Used by:
    - bootstrap.js when selecting an initial route based on hash/path.
    - chat-state.js when switching conversations (switchChatTab).
  - Ensures URL and current view are always in sync without side effects.

Pattern routes:

- Routes like /chat/:id are supported via precomputed regex patterns.
- Validation logic for :id is enforced in the pattern handler (see deep linking rules above).

Important:

- Frontend code should:
  - Use Router.navigate() for user-facing navigation.
  - Use Router.updateUrlWithoutDispatch() for internal URL sync (e.g., chat tab switching).
  - Avoid calling history.replaceState() or history.pushState() directly unless absolutely necessary.

## Backend SPA Guard

The backend uses an SPA-friendly index_route for unknown paths.

Implemented in: src/web/mod.rs

Rules:

- For GET requests that are:
  - Not under /api/*
  - Not WebSocket
- The server decides between:
  - Serving the SPA shell, or
  - Returning 404 (asset-like paths)

SPA guard invariant:

- All SPA routes must have no dot in the last path segment.
- If the last segment contains a dot, the path is treated as asset-like:
  - Example: /chat.js, /vendor/js/highlight.min.js
  - Returns 404 if not handled by another route.
- Otherwise, the SPA shell is returned so the client router can interpret the path.

This invariant must be preserved:
- New static or API paths should not conflict with this rule.
- If a new SPA route is added with a dot (e.g., /files/a.md), the guard must be updated accordingly.

## Vendor Assets and CDN

The app no longer relies on external CDNs for core scripting libraries.

Migration:

- Libraries previously loaded from cdn.jsdelivr.net:
  - marked
  - DOMPurify
  - highlight.js
- Are now self-hosted under:
  - /vendor/ (e.g., /vendor/js/marked.min.js)

How it works:

- Dependencies declared in package.json.
- A script scripts/vendor-copy.mjs:
  - Copies dist files from node_modules into static/vendor/
  - Is run as part of the asset build step.
- Renovate can manage version updates via package.json.

Security and CSP impact:

- Global CSP (non-index routes):
  - script-src is 'self' only; no CDN allowed.
- index.html:
  - Still allows cdn.jsdelivr.net in script-src for backward compatibility with SRI-pinned scripts.
  - No strict-dynamic directive (it was disabling host allowlists and blocking scripts).
- Benefits:
  - Fewer external dependencies.
  - Avoid CDN MIME-type or availability issues.
  - Simpler, stricter CSP.

## Browser History and Modals

The router is responsible for keeping history and modals consistent:

- When navigating away from a view, modals (e.g., Settings) are closed:
  - Navigation triggers an onBeforeDispatch hook that dismisses open modals.
- Escape key:
  - Closes the Settings modal instead of triggering browser-back.
- Chat tab switching:
  - Uses Router.updateUrlWithoutDispatch() to sync URL without causing full navigation or re-opening modals.

This ensures:
- Back/Forward history behaves predictably.
- Modals don't remain open after navigation.
