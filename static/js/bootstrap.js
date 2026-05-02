// ── Bootstrap ─────────────────────────────────────────────────────────────────
// Module entrypoint. Loaded as type="module" from index.html.
// Single authoritative startup path — no legacy app.js or init-state.js.

import { escapeHtml } from './core/format.js';
import * as state from './core/app-state.js';
import './compat/globals.js'; // Set window.escapeHtml, window.formatMetricNumber

// Stub functions for dead HTML references (analytics/export modals not yet implemented)
window.closeAnalyticsModal = () => {};
window.closeExportModal = () => {};
window.exportData = () => {};

// Initialize window.* state from app-state.js (replaces init-state.js classic script)
window.prevValues = state.prevValues;
window.metricSeries = state.metricSeries;
window.slotSnapshots = state.slotSnapshots;
window.requestActivity = state.requestActivity;
window.recentTasks = state.recentTasks;
window.metricCapabilities = state.metricCapabilities;
window.liveOutputTracker = state.liveOutputTracker;
window.lastServerState = state.lastServerState;
window.lastLlamaMetrics = state.lastLlamaMetrics;
window.lastSystemMetrics = state.lastSystemMetrics;
window.lastGpuMetrics = state.lastGpuMetrics;
window.lastCapabilities = state.lastCapabilities;
window.currentPollInterval = state.currentPollInterval;
window.lastGpuData = state.lastGpuData;
window.presets = state.presets;
window.sessions = state.sessions;
window.activeSessionId = state.activeSessionId;
window.activeSessionPort = state.activeSessionPort;
window.serverRunning = state.serverRunning;
window.prevLogLen = state.prevLogLen;
window.remoteAgentInProgress = state.remoteAgentInProgress;
window.remoteAgentSshConnection = state.remoteAgentSshConnection;
window.latestSshHostKey = state.latestSshHostKey;
window.settingsIsDirty = state.settingsIsDirty;
window.settingsSaveTimer = state.settingsSaveTimer;
window.chatTabs = state.chatTabs;
window.activeChatTabId = state.activeChatTabId;
window.chatBusy = state.chatBusy;
window.compactionInProgress = state.compactionInProgress;
window.unreadChatCount = state.unreadChatCount;
window.chatAbortController = state.chatAbortController;
window.chatTabsDirty = state.chatTabsDirty;
window.chatPersistTimer = state.chatPersistTimer;
window.chatInitialized = state.chatInitialized;
window.lhmResolve = state.lhmResolve;
window.enterToSend = localStorage.getItem('llama-monitor-enter-to-send') !== 'false';
window.chatFontSize = parseInt(localStorage.getItem('llama-monitor-chat-font') || '100');

import { initDashboardRender } from './features/dashboard-render.js';
import { initWebSocket } from './features/dashboard-ws.js';
import { initPresets } from './features/presets.js';
import { initSessions } from './features/sessions.js';
import { initAttachDetach } from './features/attach-detach.js';
import { initRemoteAgent } from './features/remote-agent.js';
import { initChatState, initChatTabs, autoResizeChatInput } from './features/chat-state.js';
import { initChatTransport } from './features/chat-transport.js';
import { initChatRender } from './features/chat-render.js';
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
document.getElementById('chat-scroll-bottom')?.addEventListener('click', () => window.chatScroll(true));

// Bind chat tab add button
document.getElementById('chat-tab-add-btn')?.addEventListener('click', () => window.addChatTab());
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
    fileBrowser: null,
    updates: null,
    lhm: null,
};

function once(key, loader) {
    if (!deferredInits[key]) {
        deferredInits[key] = loader();
    }
    return deferredInits[key];
}

function ensureFileBrowser() {
    return once('fileBrowser', async () => {
        const mod = await import('./features/file-browser.js');
        mod.initFileBrowser();
        return mod;
    });
}

function ensureUpdates() {
    return once('updates', async () => {
        const mod = await import('./features/updates.js');
        mod.initUpdates();
        return mod;
    });
}

window.openFileBrowser = (targetId, filter) => {
    ensureFileBrowser().then(mod => mod.openFileBrowser(targetId, filter));
};

function scheduleDeferredUpdateCheck() {
    const runCheck = () => {
        ensureUpdates().then(mod => mod.checkForUpdate());
    };

    if (document.visibilityState === 'visible') {
        if (window.requestIdleCallback) {
            window.requestIdleCallback(runCheck, { timeout: 3000 });
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

// Service worker registration
navigator.serviceWorker.register('/sw.js').catch(() => {});

// Signal that all modules are loaded and initialized
document.documentElement.classList.add('modules-ready');

// Initialize chat tabs (async — fetches tabs from API)
initChatTabs().catch(err => console.error('[bootstrap] initChatTabs failed:', err));
