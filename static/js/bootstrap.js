// ── Bootstrap ─────────────────────────────────────────────────────────────────
// Module entrypoint. Loaded as type="module" from index.html.
//
// Phase 1: This file coexists with the legacy app.js (classic script).
// The legacy script provides all inline-handler functions on window.
// This bootstrap initializes the new module system and will gradually
// replace the legacy script as features are extracted.

import { escapeHtml } from './core/format.js';
import './compat/globals.js'; // Set window.escapeHtml, window.formatMetricNumber
import { initDashboardRender } from './features/dashboard-render.js';
import { initWebSocket } from './features/dashboard-ws.js';
import { initFileBrowser } from './features/file-browser.js';
import { initPresets } from './features/presets.js';
import { initSessions } from './features/sessions.js';
import { initAttachDetach } from './features/attach-detach.js';
import { initRemoteAgent } from './features/remote-agent.js';
import { initChatState } from './features/chat-state.js';
import { initChatTransport } from './features/chat-transport.js';
import { initChatRender } from './features/chat-render.js';
import { initChatTemplates } from './features/chat-templates.js';
import { initChatParams } from './features/chat-params.js';
import { initLHM } from './features/lhm.js';
import { initSetupView } from './features/setup-view.js';
import { initUpdates } from './features/updates.js';
import { initShortcuts } from './features/shortcuts.js';
import { initNav } from './features/nav.js';
import { initAnimate } from './features/animate.js';
import { initSettings } from './features/settings.js';
import { initUserMenu } from './features/user-menu.js';
import { initConfig } from './features/config.js';
import { initModels } from './features/models.js';
import { initSensorBridge } from './features/sensor-bridge.js';
import { initToast } from './features/toast.js';

// Verify module loading works — if this fails, the page is broken.
console.log('[bootstrap] Module entrypoint loaded');

// Make escapeHtml available on window for inline handlers (Phase 1 compat).
// The authoritative implementation is in format.js — this replaces the 3
// duplicates in app.js.
window.escapeHtml = escapeHtml;

// Phase 1: Initialize rendering functions, then WebSocket.
// dashboard-render provides rendering functions on window.*.
// dashboard-ws calls those functions via window.*.
initDashboardRender();
initWebSocket();

// Phase 4: Initialize extracted features — puts inline-handler functions on window.
initFileBrowser();
initPresets();
initSessions();
initAttachDetach();
initRemoteAgent();

// Phase 6a: Chat state before transport (transport imports from state)
initChatState();
initChatTransport();

// Phase 6b: Chat rendering, templates, and params (after state/transport)
initChatRender();
initChatTemplates();
initChatParams();

// Phase 7: LHM, setup view, updates, shortcuts
initLHM();
initSetupView();
initUpdates();
initShortcuts();

// Phase 8: Nav, animate, settings, user menu, config, models, sensor bridge, toast
initNav();
initAnimate();
initSettings();
initUserMenu();
initConfig();
initModels();
initSensorBridge();
initToast();

// Service worker registration
navigator.serviceWorker.register('/sw.js').catch(() => {});

// Signal that all modules are loaded and initialized
document.documentElement.classList.add('modules-ready');
