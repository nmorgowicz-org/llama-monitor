// ── Chat State & Persistence ─────────────────────────────────────────────────
// Tab collection, active tab, busy flags, persistence scheduling, tab CRUD.

const CHAT_TABS_PERSIST_DEBOUNCE_MS = 500;

// ── Tab Accessors ──────────────────────────────────────────────────────────────

export function activeChatTab() {
    return window.chatTabs.find(t => t.id === window.activeChatTabId) ?? null;
}

// ── Tab Creation ───────────────────────────────────────────────────────────────

export function newChatTab(name = 'New Chat') {
    return {
        id: crypto.randomUUID(),
        name,
        system_prompt: 'You are {{char}}, a helpful, concise assistant. You are talking to {{user}}. Provide clear, accurate answers.',
        ai_name: '',
        user_name: '',
        explicit_mode: false,
        auto_compact: true,
        messages: [],
        totalInputTokens: 0,
        totalOutputTokens: 0,
        model_params: {
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            min_p: 0.01,
            repeat_penalty: 1.0,
            max_tokens: null,
            stream_timeout: 120,
        },
        created_at: Date.now(),
        updated_at: Date.now(),
    };
}

function normalizeChatTab(tab) {
    return {
        ...tab,
        auto_compact: tab.auto_compact ?? true,
    };
}

// ── Tab Initialization ────────────────────────────────────────────────────────

export async function initChatTabs() {
    try {
        const resp = await fetch('/api/chat/tabs');
        const data = await resp.json();
        window.chatTabs = data.length ? data.map(normalizeChatTab) : [newChatTab('Chat 1')];
    } catch {
        window.chatTabs = [newChatTab('Chat 1')];
    }
    window.activeChatTabId = window.chatTabs[0].id;

    // Render (legacy — Phase 6b)
    if (typeof window.renderChatTabs === 'function') window.renderChatTabs();
    if (typeof window.renderChatMessages === 'function') window.renderChatMessages();

    // Load UI state from tab
    if (typeof window.loadChatNames === 'function') window.loadChatNames();
    if (typeof window.populateTemplatesDropdown === 'function') window.populateTemplatesDropdown();
    if (typeof window.updateExplicitToggleUI === 'function') window.updateExplicitToggleUI();
    if (typeof window.updateParamsDirtyIndicator === 'function') window.updateParamsDirtyIndicator();
    if (typeof window.syncMessageLimitInput === 'function') window.syncMessageLimitInput();
    if (typeof window.syncCompactSettingsUI === 'function') window.syncCompactSettingsUI(activeChatTab());

    // Show welcome tip on first visit
    if (!localStorage.getItem('llama-monitor-chat-welcomed')) {
        localStorage.setItem('llama-monitor-chat-welcomed', 'true');
        setTimeout(() => {
            if (typeof window.showToast === 'function') {
                window.showToast('Tip: try a suggested prompt below to get started', 'info');
            }
        }, 800);
    }
}

// ── Tab CRUD ───────────────────────────────────────────────────────────────────

export function addChatTab() {
    const tab = newChatTab(`Chat ${window.chatTabs.length + 1}`);
    window.chatTabs.push(tab);
    switchChatTab(tab.id);
    scheduleChatPersist();
}

export function closeChatTab(id) {
    if (window.chatTabs.length === 1) return;
    window.chatTabs = window.chatTabs.filter(t => t.id !== id);
    if (window.activeChatTabId === id) {
        window.activeChatTabId = window.chatTabs[window.chatTabs.length - 1].id;
    }
    if (typeof window.renderChatTabs === 'function') window.renderChatTabs();
    if (typeof window.renderChatMessages === 'function') window.renderChatMessages();
    scheduleChatPersist();
}

export function switchChatTab(id) {
    if (window.chatBusy) return;
    window.activeChatTabId = id;
    if (typeof window.renderChatTabs === 'function') window.renderChatTabs();
    if (typeof window.renderChatMessages === 'function') window.renderChatMessages();
    if (typeof window.loadChatNames === 'function') window.loadChatNames();
    if (typeof window.updateExplicitToggleUI === 'function') window.updateExplicitToggleUI();
    if (typeof window.syncMessageLimitInput === 'function') window.syncMessageLimitInput();
    if (typeof window.syncCompactSettingsUI === 'function') window.syncCompactSettingsUI(activeChatTab());
    if (typeof window.updateCtxPressureBar === 'function') window.updateCtxPressureBar(0);
}

export function renameChatTab(id, newName) {
    const tab = window.chatTabs.find(t => t.id === id);
    if (tab) {
        tab.name = newName.trim() || tab.name;
        if (typeof window.renderChatTabs === 'function') window.renderChatTabs();
        scheduleChatPersist();
    }
}

export function clearChat() {
    const tab = activeChatTab();
    if (!tab) return;
    tab.messages = [];
    tab.updated_at = Date.now();
    if (typeof window.renderChatMessages === 'function') window.renderChatMessages();
    if (typeof window.updateChatTabBadge === 'function') window.updateChatTabBadge();
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
        if (typeof window.renderChatMessages === 'function') window.renderChatMessages();
    }
}

// ── Persistence ────────────────────────────────────────────────────────────────

export function normalizeTabForSave(tab) {
    const t = { ...tab };
    delete t.totalInputTokens;
    delete t.totalOutputTokens;
    t.messages = (t.messages || []).map(m => {
        const msg = { ...m };
        delete msg.cumulativeInputTokens;
        delete msg.cumulativeOutputTokens;
        return msg;
    });
    return t;
}

export function scheduleChatPersist() {
    window.chatTabsDirty = true;
    clearTimeout(window.chatPersistTimer);
    window.chatPersistTimer = setTimeout(persistChatTabs, CHAT_TABS_PERSIST_DEBOUNCE_MS);
}

export function markChatTabsDirty() {
    window.chatTabsDirty = true;
}

export async function persistChatTabs() {
    if (!window.chatTabsDirty) return;
    try {
        const tabsToSave = window.chatTabs.map(normalizeTabForSave);
        const totalMessages = tabsToSave.reduce((sum, t) => sum + (t.messages?.length || 0), 0);
        if (totalMessages === 0 && tabsToSave.length > 0) {
            return;
        }
        await fetch('/api/chat/tabs', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(tabsToSave),
        });
    } catch (e) { console.error('persistChatTabs error:', e); }
}

export function flushChatPersist() {
    clearTimeout(window.chatPersistTimer);
    if (window.chatTabs && window.chatTabs.length) {
        fetch('/api/chat/tabs', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(window.chatTabs.map(normalizeTabForSave)),
            keepalive: true,
        });
    }
}

// ── Busy UI ────────────────────────────────────────────────────────────────────

export function setChatBusyUI(busy) {
    const sendBtn = document.getElementById('btn-send');
    if (busy) {
        // Use window.* to avoid circular import (stopChat/sendChat are in chat-transport)
        sendBtn.onclick = () => window.stopChat();
        sendBtn.innerHTML = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
             <rect x="6" y="6" width="12" height="12" rx="2"/>
           </svg>`;
        sendBtn.classList.add('btn-chat-send-stop');
        sendBtn.title = 'Stop generating';
    } else {
        sendBtn.onclick = () => window.sendChat();
        sendBtn.innerHTML = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
             <path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z"/>
           </svg>`;
        sendBtn.classList.remove('btn-chat-send-stop');
        sendBtn.title = 'Send message';
    }

    const input = document.getElementById('chat-input');
    if (input) input.disabled = busy;
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
    // Put on window for inline handlers
    window.activeChatTab = activeChatTab;
    window.initChatTabs = initChatTabs;
    window.newChatTab = newChatTab;
    window.addChatTab = addChatTab;
    window.closeChatTab = closeChatTab;
    window.switchChatTab = switchChatTab;
    window.renameChatTab = renameChatTab;
    window.clearChat = clearChat;
    window.substituteNames = substituteNames;
    window.updateChatName = updateChatName;
    window.scheduleChatPersist = scheduleChatPersist;
    window.persistChatTabs = persistChatTabs;
    window.flushChatPersist = flushChatPersist;
    window.markChatTabsDirty = markChatTabsDirty;
    window.setChatBusyUI = setChatBusyUI;
    window.autoResizeChatInput = autoResizeChatInput;

    // beforeunload flush
    window.addEventListener('beforeunload', flushChatPersist);
}
