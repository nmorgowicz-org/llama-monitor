// ── Bootstrap ─────────────────────────────────────────────────────────────────
// Module entrypoint. Loaded as type="module" from index.html.
// Single authoritative startup path — no legacy app.js or init-state.js.

import './compat/globals.js'; // Set window.escapeHtml, window.formatMetricNumber

import { initDashboardRender } from './features/dashboard-render.js';
import { initWebSocket } from './features/dashboard-ws.js';
import { initPresets } from './features/presets.js';
import { initSessions } from './features/sessions.js';
import { addChatTab, autoResizeChatInput, initChatState, initChatTabs, restoreTabFromTrash } from './features/chat-state.js';
import { chatScroll, initChatRender, renderTrashDropdown } from './features/chat-render.js';
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
import { initNetworkDetection } from './features/network-detection.js';
import { initContextSidebar } from './features/chat-notes.js';
import { initSuggestionsDropdown } from './features/chat-suggestions.js';
import { initQuickGuide } from './features/chat-quick-guide.js';
import { initFixLastResponse } from './features/chat-fix-last.js';

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

// Bind trash dropdown toggle
document.getElementById('chat-tab-trash-btn')?.addEventListener('click', (e) => {
    e.stopPropagation();
    const dropdown = document.getElementById('chat-tab-trash-dropdown');
    if (!dropdown) return;
    const isOpen = dropdown.classList.contains('open');
    if (!isOpen) {
        renderTrashDropdown();
        const rect = e.currentTarget.getBoundingClientRect();
        dropdown.style.top = (rect.bottom + 4) + 'px';
        dropdown.style.right = (window.innerWidth - rect.right) + 'px';
    }
    dropdown.classList.toggle('open');
});

// Close trash dropdown when clicking outside
document.addEventListener('click', (e) => {
    const dropdown = document.getElementById('chat-tab-trash-dropdown');
    const trashBtn = document.getElementById('chat-tab-trash-btn');
    if (dropdown && dropdown.classList.contains('open') &&
        !trashBtn.contains(e.target) && !dropdown.contains(e.target)) {
        dropdown.classList.remove('open');
    }
});

// Event delegation for trash restore buttons
document.getElementById('chat-tab-trash-dropdown')?.addEventListener('click', (e) => {
    const restoreBtn = e.target.closest('[data-trash-restore]');
    if (restoreBtn) {
        e.stopPropagation();
        const tabId = restoreBtn.dataset.trashRestore;
        restoreTabFromTrash(tabId);
        document.getElementById('chat-tab-trash-dropdown')?.classList.remove('open');
    }
});

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
initNetworkDetection();

// Phase 9: Guided generation features
initContextSidebar();
initSuggestionsDropdown();
initQuickGuide();
initFixLastResponse();

// Wire up guided generation event handlers
document.getElementById('context-sidebar-toggle')?.addEventListener('click', () => {
    import('./features/chat-notes.js').then(({ toggleContextSidebar }) => toggleContextSidebar());
});

document.getElementById('suggestions-toggle')?.addEventListener('click', (e) => {
    e.stopPropagation();
    import('./features/chat-suggestions.js').then(({ toggleSuggestionsDropdown }) => toggleSuggestionsDropdown());
});

document.getElementById('quick-guide-toggle')?.addEventListener('click', (e) => {
    e.stopPropagation();
    import('./features/chat-quick-guide.js').then(({ toggleQuickGuide }) => toggleQuickGuide());
});

// Handle suggestion selection
window.addEventListener('suggestionSelected', (e) => {
    const { text, mode = 'replace' } = e.detail;
    const input = document.getElementById('chat-input');
    if (input) {
        if (mode === 'append' && input.value.trim()) {
            const separator = input.value.endsWith('\n') ? '' : '\n';
            input.value = `${input.value}${separator}${text}`;
        } else {
            input.value = text;
        }
        input.focus();
        input.setSelectionRange(input.value.length, input.value.length);
    }
});

// Handle quick guide submission as an immediate guided follow-up.
window.addEventListener('quickGuideSubmitted', async (e) => {
    const { instruction } = e.detail;
    const trimmedInstruction = instruction.trim();
    const [{ activeChatTab, scheduleChatPersist }, { sendQuickGuideReply }] = await Promise.all([
        import('./features/chat-state.js'),
        import('./features/chat-transport.js'),
    ]);

    const tab = activeChatTab();
    if (!tab) return;

    tab.quick_guide_active = trimmedInstruction;
    tab.quick_guide_draft = '';
    tab._quickGuideInFlight = !!trimmedInstruction;
    scheduleChatPersist();
    window.dispatchEvent(new CustomEvent('quickGuideStateChanged', {
        detail: { tabId: tab.id, guide: tab.quick_guide_active },
    }));

    if (trimmedInstruction) {
        const result = await sendQuickGuideReply();
        if (result?.message) {
            result.message._quickGuideMeta = {
                instruction: trimmedInstruction,
                transientUserPrompt: result.transientUserPrompt ?? null,
            };
            tab._quickGuideLastRun = {
                instruction: trimmedInstruction,
                targetRole: result.message.role,
                targetIndex: tab.messages.length - 1,
                appliedAt: Date.now(),
            };
        }
    }

    tab.quick_guide_active = '';
    tab._quickGuideInFlight = false;
    scheduleChatPersist();
    window.dispatchEvent(new CustomEvent('quickGuideStateChanged', {
        detail: { tabId: tab.id, guide: '' },
    }));
});

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
