// ── Bootstrap ─────────────────────────────────────────────────────────────────
// Module entrypoint. Loaded as type="module" from index.html.
//
// Phase 1: This file coexists with the legacy app.js (classic script).
// The legacy script provides all inline-handler functions on window.
// This bootstrap initializes the new module system and will gradually
// replace the legacy script as features are extracted.

import { escapeHtml } from './core/format.js';
import { initWebSocket } from './features/dashboard-ws.js';

// Verify module loading works — if this fails, the page is broken.
console.log('[bootstrap] Module entrypoint loaded');

// Make escapeHtml available on window for inline handlers (Phase 1 compat).
// The authoritative implementation is in format.js — this replaces the 3
// duplicates in app.js.
window.escapeHtml = escapeHtml;

// Phase 3: Initialize WebSocket (replaces ws creation in app.js).
// app.js still runs first and provides rendering functions on window.*.
// The dashboard-ws module calls those functions via window.*.
initWebSocket();
