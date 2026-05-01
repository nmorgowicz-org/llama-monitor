// ── Global Compatibility Facade ────────────────────────────────────────────────
// Functions used by inline handlers in index.html (onclick, oninput, etc.) must
// be reachable on window. This file imports the real implementations from
// feature modules and assigns them to window.
//
// During Phase 1, most functions are still in the legacy app.js (classic script).
// As features are extracted, their inline-handler functions are imported here.
//
// Goal for Phase 8: reduce this file to only the functions still needed by
// inline handlers, then remove inline handlers entirely and delete this file.

// Import the authoritative escapeHtml from format.js (replaces 3 duplicates in app.js)
import { escapeHtml, formatMetricNumber } from '../core/format.js';

// Attach to window — this is the ONLY place window assignments should happen
window.escapeHtml = escapeHtml;
window.formatMetricNumber = formatMetricNumber;

// ── Functions to be migrated as features are extracted ─────────────────────────
// Each function below is referenced from an inline handler in index.html.
// Once the owning feature is extracted, import it here and assign to window.
//
// The full inventory of 127 unique function names is tracked in:
// docs/20260430-appjs_refactor.md
//
// Examples (to be filled in as features are extracted):
//
// import { switchTab } from '../features/nav.js';
// window.switchTab = switchTab;
//
// import { sendChat } from '../features/chat-transport.js';
// window.sendChat = sendChat;
