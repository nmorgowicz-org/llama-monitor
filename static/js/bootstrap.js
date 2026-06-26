// ── Bootstrap ─────────────────────────────────────────────────────────────────
// Module entrypoint. Loaded as type="module" from index.html.
// Single authoritative startup path — no legacy app.js or init-state.js.

import './compat/globals.js'; // Set window.escapeHtml, window.formatMetricNumber

import { initDashboardRender } from './features/dashboard-render.js';
import { initWebSocket } from './features/dashboard-ws.js';
import { initPresets } from './features/presets.js';
import { activeChatTab, addChatTab, autoResizeChatInput, initChatState, initChatTabs, restoreTabFromTrash, switchChatTab } from './features/chat-state.js';
import { chatScroll, initChatRender } from './features/chat-render.js';
import { initChatSessionsSidebar, renderChatSessionsSidebar } from './features/chat-sessions-sidebar.js';
import { initChatSearch } from './features/chat-search.js';
import { initAttachDetach } from './features/attach-detach.js';
import { initRemoteAgent } from './features/remote-agent.js';
import { initChatTransport } from './features/chat-transport.js';
import { initChatTemplates } from './features/chat-templates.js';
import { initChatParams } from './features/chat-params.js';
import { initSetupView, switchView } from './features/setup-view.js';
import { initShortcuts } from './features/shortcuts.js';
import { initNav, switchTab } from './features/nav.js';
import { initChatWidthObserver } from './features/chat-width-observer.js';
import { initChatFocusMode, toggleFocusMode } from './features/chat-focus-mode.js';
import { initChatHistoryQA } from './features/chat-history-qa.js';
import { initAnimate } from './features/animate.js';
import { initSettings, openSettingsModal } from './features/settings.js';
import { initUserMenu } from './features/user-menu.js';
import { initConfig } from './features/config.js';
import { initModels } from './features/models.js';
import { initSensorBridge } from './features/sensor-bridge.js';
import { initToast } from './features/toast.js';
import { initNetworkDetection } from './features/network-detection.js';
import { initContextSidebar } from './features/chat-notes.js';
import { initSuggestionsDropdown, closeSuggestionsDropdown } from './features/chat-suggestions.js';
import { initQuickGuide, closeQuickGuide } from './features/chat-quick-guide.js';
import { initDbAdmin } from './features/db-admin.js';
import { initAuthGate, logoutCurrentUser } from './features/auth.js';
import { deriveTelemetryGrade, gradeLabel, gradeStatusClass, gradeActionCopy } from './features/telemetry-grade.js';
import { initReplyPlanUpdates } from './features/chat-reply-plan.js';
import { initCommandPalette } from './features/workspace-command-palette.js';
import { initSpawnWizard, openSpawnWizard } from './features/spawn-wizard.js';
import { initTunePanel } from './features/tune-panel.js';
import { initLlamaUpdater } from './features/llama-updater.js';
import { HF_DISCOVER_CATEGORIES } from './features/hf-browse.js';
import { initGlobalTooltip } from './core/tooltip.js';
import Router from './features/router.js';

// Verify module loading works — if this fails, the page is broken.
console.log('[bootstrap] Module entrypoint loaded');

// Helper for modules: returns headers object with Authorization if token is available.
window.authHeaders = function(extra = {}) {
    const headers = { ...extra };
    if (window.__API_TOKEN && !headers['Authorization']) {
        headers['Authorization'] = `Bearer ${window.__API_TOKEN}`;
    }
    return headers;
};

// fetch wrapper that shows "Authentication required" on 401.
// Use as: const res = await authFetch(url, options);
window.authFetch = async function(url, options = {}) {
    const res = await fetch(url, options);
    if (res.status === 401) {
        const toast = (await import('./features/toast.js')).showToast;
        if (typeof toast === 'function') {
            toast('Authentication required', 'error', 'API token is missing or invalid.');
        }
    }
    return res;
};

// Set app version in sidebar (lightweight, no module dependency)
(function() {
    const el = document.getElementById('app-version');
    if (el && typeof APP_VERSION !== 'undefined') {
        el.textContent = `v${APP_VERSION}`;
    }
})();

window.logoutCurrentUser = logoutCurrentUser;

async function initializeApp() {
    const auth = await initAuthGate();
    if (!auth.ready) return;

    try {
        const res = await fetch('/api/internal/api-token');
        if (res.ok) {
            const data = await res.json();
            if (data.token) {
                window.__API_TOKEN = data.token;
            }
        }
    } catch {
        // Non-critical: continue without token if fetch fails.
    }

    initGlobalTooltip();

    // Phase 1: Initialize rendering functions, then WebSocket.
    initDashboardRender();
    initWebSocket();

    // Phase 4: Initialize extracted features.
    initPresets();
    initAttachDetach();
    initRemoteAgent();

    // Phase 6a: Chat state before transport (transport imports from state)
    initChatState();
    initChatTransport();

    // Phase 6b: Chat rendering, templates, and params (after state/transport)
    initChatRender();
    initChatSessionsSidebar();
    initChatSearch();

    // Bind chat scroll button
    document.getElementById('chat-scroll-bottom')?.addEventListener('click', () => chatScroll(true));

    initChatTemplates();
    initChatParams();

    // Resize chat input to fit content
    autoResizeChatInput();

    // Phase 7: setup view, updates, shortcuts (LHM is deferred)
    initSetupView();
    initShortcuts();

    // Phase 8: Nav, animate, settings, user menu, config, models, sensor bridge, toast
    initNav();
    initChatWidthObserver();
    initChatFocusMode();
    initChatHistoryQA();
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
    initReplyPlanUpdates();

    // Phase 10: Database administration
    initDbAdmin();

    // Phase 11: Command palette
    initCommandPalette();

    // Phase 12: Spawn wizard, tune panel, binary updater
    initSpawnWizard();
    initTunePanel();
    initLlamaUpdater();

    // Initialize chat tabs only after token bootstrap and feature init complete.
    await initChatTabs();

    // Phase 13: Router (must run after all navigation functions are available)
    if (typeof history !== 'undefined' && history.pushState) {
      Router.register('/', () => switchView('setup'));
      Router.register('/spawn', () => openSpawnWizard());
      Router.register('/chat', () => switchTab('chat'));
      Router.register('/logs', () => switchTab('logs'));
      Router.register('/settings', () => openSettingsModal());
      Router.register('/chat/:id', path => {
        const id = path.split('/chat/')[1];
        if (id && typeof switchChatTab === 'function') {
          switchTab('chat');
          switchChatTab(id);
        } else {
          switchTab('chat');
        }
      });

      Router.init();
    }
}

initializeApp().catch(err => console.error('[bootstrap] initializeApp failed:', err));

// Mutual exclusion: opening one guided panel closes the other.
window.addEventListener('suggestionsOpened', () => closeQuickGuide());
window.addEventListener('quickGuideOpened', () => closeSuggestionsDropdown());

// Wire up guided generation event handlers
document.getElementById('context-sidebar-toggle')?.addEventListener('click', (e) => {
    e.stopPropagation();
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

function getTopmostDismissibleOverlay() {
    const candidates = [
        ...document.querySelectorAll('.modal-overlay.open, .modal-overlay.active, .keyboard-shortcut-overlay.open, #release-notes-panel.open'),
        document.getElementById('command-palette-overlay'),
    ].filter(el => {
        const style = getComputedStyle(el);
        return style.display !== 'none' && style.visibility !== 'hidden';
    });

    candidates.sort((a, b) => {
        const aZ = Number.parseInt(getComputedStyle(a).zIndex, 10) || 0;
        const bZ = Number.parseInt(getComputedStyle(b).zIndex, 10) || 0;
        return bZ - aZ;
    });

    return candidates[0] || null;
}

document.addEventListener('keydown', (e) => {
    if (!e.defaultPrevented && !e.repeat && !e.isComposing && (e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'F') {
        e.preventDefault();
        toggleFocusMode();
        return;
    }

    if (e.key !== 'Escape' || e.defaultPrevented || e.repeat || e.isComposing) return;

    const overlay = getTopmostDismissibleOverlay();
    if (!overlay) return;

    const closeButton = overlay.querySelector('.modal-close, .shortcuts-close, .cat-mgr-close, .modal-close-btn')
        || (overlay.id === 'release-notes-panel' ? document.getElementById('release-notes-close') : null);

    if (!closeButton) return;

    e.preventDefault();
    e.stopPropagation();
    closeButton.click();
});

// Handle suggestion selection
function parseSuggestionText(text) {
    const [rawTitle, ...rest] = (text || '').split('\n');
    return {
        title: (rawTitle || '').trim(),
        description: rest.join('\n').trim(),
    };
}

function detectSuggestionInputStyle(tab) {
    const recentUserMessages = (tab?.messages || [])
        .filter(msg => msg.role === 'user' && msg.content?.trim())
        .slice(-5)
        .map(msg => msg.content.trim());

    if (recentUserMessages.length === 0) return 'instruction';

    const joined = recentUserMessages.join('\n').toLowerCase();
    const firstPersonMatches = joined.match(/\b(i|i'm|i’d|i'll|me|my|mine)\b/g) || [];
    const directiveMatches = joined.match(/\b(write|continue|have|make|let|show|focus|use|keep|add|rewrite|respond)\b/g) || [];

    if (directiveMatches.length >= firstPersonMatches.length && directiveMatches.length >= 2) {
        return 'instruction';
    }
    if (firstPersonMatches.length >= 3) {
        return 'first_person';
    }
    return 'third_person';
}

function buildSuggestionDraft(text, tab) {
    const { title, description } = parseSuggestionText(text);
    const beat = [title, description].filter(Boolean).join('. ');
    const style = detectSuggestionInputStyle(tab);

    if (style === 'first_person') {
        return `Use this beat for my next first-person turn:\n${beat}\n\nKeep my established POV, tense, and voice. Expand it into a natural, detailed continuation.`;
    }
    if (style === 'third_person') {
        return `Use this beat for the next scene continuation:\n${beat}\n\nKeep the established third-person POV, tense, and tone. Expand it into a natural, detailed continuation.`;
    }
    return `Use this next beat:\n${beat}\n\nKeep the current POV, tense, and tone. Expand it into a natural, detailed continuation.`;
}

function buildSuggestionSendMessage(text, tab) {
    const { title, description } = parseSuggestionText(text);
    const beat = description ? `${title}. ${description}` : title;
    const style = detectSuggestionInputStyle(tab);

    if (style === 'first_person') {
        return `Continue from my perspective using this beat: ${beat}. Keep my established voice, POV, and tense.`;
    }
    if (style === 'third_person') {
        return `Continue the scene using this beat: ${beat}. Keep the established third-person style, tone, and tense.`;
    }
    return `Use this next beat: ${beat}. Keep the current POV, tense, and tone.`;
}

window.addEventListener('suggestionSelected', async (e) => {
    const { text, mode = 'send' } = e.detail;
    const tab = activeChatTab();
    const input = document.getElementById('chat-input');

    if (mode === 'send') {
        const [{ sendChatWithContent }] = await Promise.all([
            import('./features/chat-transport.js'),
        ]);
        if (tab) {
            await sendChatWithContent(buildSuggestionSendMessage(text, tab));
        }
        return;
    }

    if (input && tab) {
        const draft = buildSuggestionDraft(text, tab);
        if (input.value.trim()) {
            const separator = input.value.endsWith('\n') ? '' : '\n';
            input.value = `${input.value}${separator}${draft}`;
        } else {
            input.value = draft;
        }
        input.dataset.suggestionDraft = 'true';
        input.focus();
        input.setSelectionRange(input.value.length, input.value.length);
        autoResizeChatInput();
        window.dispatchEvent(new CustomEvent('replyPlanChanged'));
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
