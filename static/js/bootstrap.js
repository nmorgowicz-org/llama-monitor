// ── Bootstrap ─────────────────────────────────────────────────────────────────
// Module entrypoint. Loaded as type="module" from index.html.
// Single authoritative startup path — no legacy app.js or init-state.js.

import './compat/globals.js'; // Set window.escapeHtml, window.formatMetricNumber

import { initDashboardRender } from './features/dashboard-render.js';
import { initWebSocket } from './features/dashboard-ws.js';
import { initPresets } from './features/presets.js';
import { initSessions } from './features/sessions.js';
import { addChatTab, autoResizeChatInput, initChatState, initChatTabs } from './features/chat-state.js';
import { chatScroll, initChatRender } from './features/chat-render.js';
import { initAttachDetach } from './features/attach-detach.js';
import { initRemoteAgent } from './features/remote-agent.js';
import { initChatTransport } from './features/chat-transport.js';
import { initChatTemplates } from './features/chat-templates.js';
import { initChatParams } from './features/chat-params.js';
import { initSetupView } from './features/setup-view.js';
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

// Set app version in sidebar (lightweight, no module dependency)
(function() {
    const el = document.getElementById('app-version');
    if (el && typeof APP_VERSION !== 'undefined') {
        el.textContent = `v${APP_VERSION}`;
    }
})();

// Phase 1: Initialize rendering functions, then WebSocket.
initDashboardRender();
initWebSocket();

// Phase 4: Initialize extracted features.
initPresets();
initSessions();
initAttachDetach();
initRemoteAgent();

// Phase 6a: Chat state before transport (transport imports from state)
initChatState();
initChatTransport();

// Phase 6b: Chat rendering, templates, and params (after state/transport)
initChatRender();

// Bind chat scroll button
document.getElementById('chat-scroll-bottom')?.addEventListener('click', () => chatScroll(true));

// Bind chat tab add button
document.getElementById('chat-tab-add-btn')?.addEventListener('click', addChatTab);
initChatTemplates();
initChatParams();

// Resize chat input to fit content
autoResizeChatInput();

// Phase 7: setup view, updates, shortcuts (LHM is deferred)
initSetupView();
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

// ── Deferred feature initialization ──────────────────────────────────────────
// These features are loaded on first use to reduce startup cost.

const deferredInits = {
    updates: null,
    lhm: null,
};

function once(key, loader) {
    if (!deferredInits[key]) {
        deferredInits[key] = loader();
    }
    return deferredInits[key];
}

function ensureUpdates() {
    return once('updates', async () => {
        const mod = await import('./features/updates.js');
        mod.initUpdates();
        return mod;
    });
}

function scheduleDeferredUpdateCheck() {
    const runCheck = () => {
        ensureUpdates().then(mod => mod.checkForUpdate());
    };

    if (document.visibilityState === 'visible') {
        if (requestIdleCallback) {
            requestIdleCallback(runCheck, { timeout: 3000 });
        } else {
            setTimeout(runCheck, 1500);
        }
        return;
    }

    const onVisible = () => {
        if (document.visibilityState === 'visible') {
            document.removeEventListener('visibilitychange', onVisible);
            runCheck();
        }
    };
    document.addEventListener('visibilitychange', onVisible);
}

scheduleDeferredUpdateCheck();

// LHM: defer until LHM show button is clicked (in settings modal)
(function() {
    function ensureInit() {
        return once('lhm', () => import('./features/lhm.js').then(mod => {
            mod.initLHM();
            return mod;
        }));
    }
    // Wire LHM show button (data-lhm-action="show" is in settings modal)
    document.addEventListener('click', (e) => {
        if (e.target.closest('[data-lhm-action="show"]')) {
            ensureInit().then(mod => {
                if (typeof mod.showLHMNotification === 'function') mod.showLHMNotification();
            });
        }
    });
})();

// Signal that all modules are loaded and initialized
document.documentElement.classList.add('modules-ready');

// Initialize chat tabs (async — fetches tabs from API)
initChatTabs().catch(err => console.error('[bootstrap] initChatTabs failed:', err));
