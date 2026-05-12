// ── Chat State & Persistence ─────────────────────────────────────────────────
// Tab collection, active tab, busy flags, persistence scheduling, tab CRUD.

import { chat } from '../core/app-state.js';
import { refreshTopCockpit } from './nav.js';
import { showToast, showToastWithActions } from './toast.js';

const CHAT_TABS_PERSIST_DEBOUNCE_MS = 500;
const TRASH_AUTO_PURGE_MS = 24 * 60 * 60 * 1000; // 24 hours
const TRASH_PURGE_CHECK_INTERVAL_MS = 60 * 60 * 1000; // check every hour
const chatViewBindings = {
    renderChatTabs: null,
    renderChatMessages: null,
    loadChatNames: null,
    updateExplicitToggleUI: null,
    updateParamsDirtyIndicator: null,
    syncMessageLimitInput: null,
    syncCompactSettingsUI: null,
    updateCtxPressureBar: null,
    refreshChatTelemetry: null,
    updateChatTabBadge: null,
    checkAutoCompact: null,
};

export function registerChatViewBindings(bindings) {
    Object.assign(chatViewBindings, bindings);
}

export function getChatViewBindings() {
    return chatViewBindings;
}

// ── Tab Accessors ──────────────────────────────────────────────────────────────

export function activeChatTab() {
    return chat.tabs.find(t => t.id === chat.activeTabId) ?? null;
}

// ── Tab Creation ───────────────────────────────────────────────────────────────

export function newChatTab(name = 'New Chat') {
    return {
        id: crypto.randomUUID(),
        name,
        system_prompt: 'You are {{char}}, a helpful, concise assistant. You are talking to {{user}}. Provide clear, accurate answers.',
        active_template_id: '',
        ai_name: '',
        user_name: '',
        explicit_level: 0,
        auto_compact: true,
        messages: [],
        totalInputTokens: 0,
        totalOutputTokens: 0,
        lastCtxPct: 0,
        model_params: {
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            min_p: 0.01,
            repeat_penalty: 1.0,
            max_tokens: null,
            stream_timeout: 120,
        },
        context_notes: [],
        context_custom_sections: [],
        sidebar_width: 280,
        quick_guide_draft: '',
        quick_guide_active: '',
        quick_guide_pending: '',
        armed_story_beats: [],
        created_at: Date.now(),
        updated_at: Date.now(),
        pinned: false,
    };
}

function normalizeChatTab(tab) {
    const messages = tab.messages || [];
    const totalInputTokens = messages.reduce((sum, m) => sum + (m.input_tokens || 0), 0);
    const totalOutputTokens = messages.reduce((sum, m) => sum + (m.output_tokens || 0), 0);
    let explicitLevel = tab.explicit_level ?? tab.explicitLevel ?? 0;
    if (tab.explicit_mode !== undefined && tab.explicit_level === undefined) {
        explicitLevel = tab.explicit_mode ? 1 : 0;
    }
    return {
        ...tab,
        explicit_level: explicitLevel,
        active_template_id: tab.active_template_id ?? '',
        auto_compact: tab.auto_compact ?? true,
        lastCtxPct: tab.lastCtxPct ?? 0,
        totalInputTokens: tab.totalInputTokens ?? totalInputTokens,
        totalOutputTokens: tab.totalOutputTokens ?? totalOutputTokens,
        context_notes: tab.context_notes ?? [],
        context_custom_sections: tab.context_custom_sections ?? [],
        sidebar_width: tab.sidebar_width ?? 280,
        quick_guide_draft: tab.quick_guide_draft ?? '',
        quick_guide_active: '',
        quick_guide_pending: '',
        armed_story_beats: tab.armed_story_beats ?? [],
        pinned: tab.pinned ?? false,
    };
}

// ── Tab Initialization ────────────────────────────────────────────────────────

export async function initChatTabs() {
    try {
        const resp = await fetch('/api/chat/tabs');
        const data = await resp.json();
        chat.tabs = data.length ? data.map(normalizeChatTab) : [newChatTab('Chat 1')];
    } catch {
        chat.tabs = [newChatTab('Chat 1')];
    }
    chat.activeTabId = chat.tabs[0].id;

    // Render (legacy — Phase 6b)
    chatViewBindings.renderChatTabs?.();
    chatViewBindings.renderChatMessages?.();

    // Load UI state from tab
    chatViewBindings.loadChatNames?.();
    chatViewBindings.updateExplicitToggleUI?.();
    chatViewBindings.updateParamsDirtyIndicator?.();
    chatViewBindings.syncMessageLimitInput?.();
    chatViewBindings.syncCompactSettingsUI?.(activeChatTab());
    chatViewBindings.refreshChatTelemetry?.();
    chatViewBindings.updatePersonaMenuName?.();
    refreshTopCockpit();

   // Trigger context card update - mark that chat tabs loaded so dashboard can poll
     if (typeof window.onChatTabsLoaded === 'function') {
         window.onChatTabsLoaded();
     }

     // Show chat tip only when user is on monitor view with an active chat session
     const { setupViewState } = await import('../core/app-state.js');
     if (setupViewState.view === 'monitor') {
         const activeTab = activeChatTab();
         if (activeTab && activeTab.messages.length > 0) {
             if (!localStorage.getItem('llama-monitor-chat-tips-seen')) {
                 localStorage.setItem('llama-monitor-chat-tips-seen', 'true');
                 setTimeout(() => {
                     showToast('Tip: try a suggested prompt below to get started', 'info');
                 }, 800);
             }
         }
     }
}

// ── Tab CRUD ───────────────────────────────────────────────────────────────────

export function addChatTab() {
    const tab = newChatTab(`Chat ${chat.tabs.length + 1}`);
    chat.tabs.push(tab);
    switchChatTab(tab.id);
    scheduleChatPersist();
}

export function closeChatTab(id) {
    const tabIdx = chat.tabs.findIndex(t => t.id === id);
    if (tabIdx === -1) return;
    if (chat.tabs.length === 1) return;

    const [tab] = chat.tabs.splice(tabIdx, 1);
    chat.tabTrash.push({ tab, trashedAt: Date.now() });

    if (chat.activeTabId === id) {
        chat.activeTabId = chat.tabs[chat.tabs.length - 1].id;
    }

    chatViewBindings.renderChatTabs?.();
    chatViewBindings.renderChatMessages?.();
    scheduleChatPersist();

    showToastWithActions('Tab deleted', 'info', '', [
        {
            id: 'undo',
            label: 'Undo',
            primary: true,
            handler: () => restoreTabFromTrash(id),
        },
    ]);
}

export function restoreTabFromTrash(id) {
    const trashIdx = chat.tabTrash.findIndex(t => t.tab.id === id);
    if (trashIdx === -1) return;

    const [trashEntry] = chat.tabTrash.splice(trashIdx, 1);
    chat.tabs.push(normalizeChatTab(trashEntry.tab));
    chat.activeTabId = trashEntry.tab.id;

    chatViewBindings.renderChatTabs?.();
    chatViewBindings.renderChatMessages?.();
    chatViewBindings.loadChatNames?.();
    chatViewBindings.updateExplicitToggleUI?.();
    chatViewBindings.syncMessageLimitInput?.();
    chatViewBindings.syncCompactSettingsUI?.(activeChatTab());
    chatViewBindings.refreshChatTelemetry?.();
    chatViewBindings.updatePersonaMenuName?.();
    refreshTopCockpit();
    scheduleChatPersist();
}

export function switchChatTab(id) {
    if (chat.busy) return;
    chat.activeTabId = id;
    chatViewBindings.renderChatTabs?.();
    chatViewBindings.renderChatMessages?.();
    chatViewBindings.loadChatNames?.();
    chatViewBindings.updateExplicitToggleUI?.();
    chatViewBindings.syncMessageLimitInput?.();
    chatViewBindings.syncCompactSettingsUI?.(activeChatTab());
    chatViewBindings.updateCtxPressureBar?.(0);
    chatViewBindings.refreshChatTelemetry?.();
    chatViewBindings.updatePersonaMenuName?.();
    refreshTopCockpit();
    window.dispatchEvent(new CustomEvent('activeTabChanged', {
        detail: { tabId: id },
    }));
}

export function renameChatTab(id, newName) {
    const tab = chat.tabs.find(t => t.id === id);
    if (tab) {
        tab.name = newName.trim() || tab.name;
        chatViewBindings.renderChatTabs?.();
        scheduleChatPersist();
    }
}

export function togglePinTab(id) {
    const tab = chat.tabs.find(t => t.id === id);
    if (!tab) return;
    tab.pinned = !tab.pinned;
    const pinned = chat.tabs.filter(t => t.pinned);
    const unpinned = chat.tabs.filter(t => !t.pinned);
    chat.tabs = [...pinned, ...unpinned];
    chatViewBindings.renderChatTabs?.();
    scheduleChatPersist();
}

export function clearChat() {
    const tab = activeChatTab();
    if (!tab) return;
    tab.messages = [];
    tab.updated_at = Date.now();
    chatViewBindings.renderChatMessages?.();
    chatViewBindings.updateChatTabBadge?.();
    chatViewBindings.refreshChatTelemetry?.();
    refreshTopCockpit();
    scheduleChatPersist();
}

// ── Tab Field Updates ─────────────────────────────────────────────────────────

export function substituteNames(prompt, aiName, userName) {
    if (!prompt) return prompt;
    let p = prompt;
    p = p.replace(/\{\{char\}\}/gi, aiName || 'AI');
    p = p.replace(/\{\{user\}\}/gi, userName || 'User');
    return p;
}

export function updateChatName(field, value) {
    const tab = activeChatTab();
    if (tab) {
        tab[field] = value.trim();
        scheduleChatPersist();
        chatViewBindings.renderChatMessages?.();
    }
}

// ── Persistence ────────────────────────────────────────────────────────────────

export function normalizeTabForSave(tab) {
    const t = { ...tab };
    delete t.explicit_mode;
    delete t.explicitLevel;
    delete t._quickGuideInstruction;
    delete t.quick_guide_pending;
    t.messages = (t.messages || []).map(m => {
        const msg = { ...m };
        delete msg.cumulativeInputTokens;
        delete msg.cumulativeOutputTokens;
        return msg;
    });
    return t;
}

export function scheduleChatPersist() {
    chat.tabsDirty = true;
    clearTimeout(chat.persistTimer);
    chat.persistTimer = setTimeout(persistChatTabs, CHAT_TABS_PERSIST_DEBOUNCE_MS);
}

export function purgeOldTrash() {
    const cutoff = Date.now() - TRASH_AUTO_PURGE_MS;
    const before = chat.tabTrash.length;
    chat.tabTrash = chat.tabTrash.filter(entry => entry.trashedAt > cutoff);
    const purged = before - chat.tabTrash.length;
    if (purged > 0) {
        scheduleChatPersist();
    }
    return purged;
}

export function markChatTabsDirty() {
    chat.tabsDirty = true;
}

export async function persistChatTabs() {
    if (!chat.tabsDirty) return;
    try {
        const tabsToSave = chat.tabs.map(normalizeTabForSave);
        const totalMessages = tabsToSave.reduce((sum, t) => sum + (t.messages?.length || 0), 0);
        if (totalMessages === 0 && tabsToSave.length > 0) {
            return;
        }
        const response = await fetch('/api/chat/tabs', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(tabsToSave),
        });
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
        }
        chat.tabsDirty = false;
    } catch (e) {
        console.error('persistChatTabs error:', e);
        throw e;
    }
}

export function flushChatPersist() {
    clearTimeout(chat.persistTimer);
    if (chat.tabs && chat.tabs.length) {
        fetch('/api/chat/tabs', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(chat.tabs.map(normalizeTabForSave)),
            keepalive: true,
        });
    }
}

// ── Busy UI ────────────────────────────────────────────────────────────────────

// Getter for transport functions — avoids circular import (chat-state ↔ chat-transport)
let _getTransport = null;
export function setTransportGetter(getter) {
    _getTransport = getter;
}

export function setChatBusyUI(busy) {
    const sendBtn = document.getElementById('btn-send');
    const transport = _getTransport ? _getTransport() : null;
    if (busy) {
        sendBtn.onclick = () => transport?.stopChat();
        sendBtn.innerHTML = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
             <rect x="6" y="6" width="12" height="12" rx="2"/>
           </svg>`;
        sendBtn.classList.add('btn-chat-send-stop');
        sendBtn.title = 'Stop generating';
    } else {
        sendBtn.onclick = () => transport?.sendChat();
        sendBtn.innerHTML = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
             <path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z"/>
           </svg>`;
        sendBtn.classList.remove('btn-chat-send-stop');
        sendBtn.title = 'Send message';
    }

    const input = document.getElementById('chat-input');
    if (input) input.disabled = busy;
    chatViewBindings.refreshChatTelemetry?.();
    refreshTopCockpit();
}

// ── Input ──────────────────────────────────────────────────────────────────────

export function autoResizeChatInput() {
    const input = document.getElementById('chat-input');
    if (!input) return;
    input.style.height = 'auto';
    input.style.height = Math.min(input.scrollHeight, 200) + 'px';
}

// ── Init ───────────────────────────────────────────────────────────────────────

export function initChatState() {
    window.addEventListener('beforeunload', flushChatPersist);
    chat.trashPurgeTimer = setInterval(purgeOldTrash, TRASH_PURGE_CHECK_INTERVAL_MS);
    purgeOldTrash();
}
