// ── Chat Params ───────────────────────────────────────────────────────────────
// Model parameter panel, system prompt panel, style/font/enter-to-send controls,
// and compaction settings.

import { activeChatTab } from './chat-state.js';

// ── Model params panel ────────────────────────────────────────────────────────

function toggleModelParamsPanel() {
    const panel = document.getElementById('chat-params-panel');
    const isOpen = panel.classList.toggle('open');
    if (isOpen) syncParamPanelToTab();
}

function syncParamPanelToTab() {
    const tab = activeChatTab();
    if (!tab) return;
    const p = tab.model_params;
    const set = (id, val, displayId) => {
        const el = document.getElementById(id);
        if (el) { el.value = val ?? ''; }
        const disp = document.getElementById(displayId);
        if (disp) disp.textContent = val ?? '';
    };
    set('param-temperature', p.temperature, 'param-temperature-val');
    set('param-top-p', p.top_p, 'param-top-p-val');
    set('param-top-k', p.top_k, 'param-top-k-val');
    set('param-min-p', p.min_p, 'param-min-p-val');
    set('param-repeat-penalty', p.repeat_penalty, 'param-repeat-penalty-val');
    const maxTok = document.getElementById('param-max-tokens');
    if (maxTok) maxTok.value = p.max_tokens ?? '';
    const streamTimeout = document.getElementById('param-stream-timeout');
    if (streamTimeout) streamTimeout.value = p.stream_timeout ?? 120;
}

function onParamChange(key, value) {
    const tab = activeChatTab();
    if (!tab) return;
    tab.model_params[key] = value;
    tab.updated_at = Date.now();
    const map = {
        temperature: 'param-temperature-val',
        top_p: 'param-top-p-val',
        top_k: 'param-top-k-val',
        min_p: 'param-min-p-val',
        repeat_penalty: 'param-repeat-penalty-val',
    };
    const dispId = map[key];
    if (dispId) {
        const el = document.getElementById(dispId);
        if (el) el.textContent = value ?? '';
    }
    window.scheduleChatPersist();
    clearTimeout(window.paramToastTimer);
    window.paramToastTimer = setTimeout(() => window.showToast('Parameter saved', 'success'), 2000);
    updateParamsDirtyIndicator();
}

function toggleAdvancedParams() {
    const panel = document.getElementById('chat-params-advanced');
    const chevron = document.querySelector('.chat-advanced-chevron');
    const isVisible = panel.style.display !== 'none';
    panel.style.display = isVisible ? 'none' : 'block';
    if (chevron) {
        chevron.style.transform = isVisible ? 'rotate(0deg)' : 'rotate(90deg)';
    }
}

function resetParamsToDefaults() {
    const tab = activeChatTab();
    if (!tab) return;
    tab.model_params = {
        temperature: 0.7,
        top_p: 0.9,
        top_k: 40,
        min_p: 0.01,
        repeat_penalty: 1.0,
        max_tokens: null,
        stream_timeout: 120,
    };
    tab.updated_at = Date.now();
    syncParamPanelToTab();
    window.scheduleChatPersist();
    updateParamsDirtyIndicator();
    window.showToast('Parameters reset to defaults', 'success');
}

function updateParamsDirtyIndicator() {
    const tab = activeChatTab();
    if (!tab) return;
    const p = tab.model_params;
    const isDirty = p.temperature !== 0.7 || p.top_p !== 0.9
        || p.top_k !== 40 || p.min_p !== 0.01
        || p.repeat_penalty !== 1.0 || (p.max_tokens && p.max_tokens !== 0)
        || p.stream_timeout !== 120;
    const btn = document.getElementById('btn-model-params');
    if (btn) btn.classList.toggle('has-active-params', isDirty);
}

// ── Copy settings between tabs ────────────────────────────────────────────────

function duplicateTabSettings(sourceId) {
    const source = window.chatTabs.find(t => t.id === sourceId);
    const target = activeChatTab();
    if (!source || !target || source.id === target.id) return;
    target.system_prompt = source.system_prompt;
    target.model_params = JSON.parse(JSON.stringify(source.model_params));
    target.updated_at = Date.now();
    window.scheduleChatPersist();
    syncParamPanelToTab();
    updateParamsDirtyIndicator();
    const indicator = document.getElementById('system-prompt-indicator');
    indicator.style.display = target.system_prompt ? 'inline' : 'none';
    document.getElementById('chat-system-input').value = target.system_prompt;
    window.showToast('Settings copied from "' + source.name + '"', 'success');
}

function showCopySettingsDropdown() {
    const target = activeChatTab();
    if (!target) return;
    const others = window.chatTabs.filter(t => t.id !== target.id);
    if (others.length === 0) {
        window.showToast('No other tabs to copy from', 'info');
        return;
    }
    const toast = window.showToastWithActions(
        'Copy settings from',
        'info',
        'Select a tab to copy its system prompt and parameters',
        others.map(t => ({
            label: t.name,
            primary: false,
            action: () => duplicateTabSettings(t.id),
        }))
    );
    if (!toast) return;
    setTimeout(() => toast.remove(), 8000);
}

// ── Message limit ─────────────────────────────────────────────────────────────

function onMessageLimitChange(value) {
    const tab = activeChatTab();
    if (!tab) return;
    const limit = Math.max(5, Math.min(200, value));
    tab.visible_message_limit = limit;
    tab.updated_at = Date.now();
    window.renderChatMessages();
    window.scheduleChatPersist();
}

function syncMessageLimitInput() {
    const tab = activeChatTab();
    const input = document.getElementById('chat-msg-limit');
    if (tab && input) input.value = tab.visible_message_limit || 15;
}

// ── Compaction ────────────────────────────────────────────────────────────────

async function compactChatTab(tab, keepTail = 10, summarize = true) {
    const msgs = tab.messages;
    const systemMsg = msgs[0]?.role === 'system' && !msgs[0]?.compaction_marker ? msgs[0] : null;
    const tombstones = msgs.filter(m => m.compaction_marker);
    const conversational = msgs.filter(m => m.role !== 'system' && !m.compaction_marker);

    if (conversational.length <= keepTail) return;

    window.compactionInProgress = true;
    setCompactButtonBusy(true);

    const dropped = conversational.slice(0, conversational.length - keepTail);
    const kept = conversational.slice(-keepTail);
    console.log('[COMPACT] starting — dropped:', dropped.length, 'kept:', kept.length, 'oldTombstones:', tombstones.length);

    let placeholderEl = null;
    if (summarize) {
        const chatMsgs = document.getElementById('chat-messages');
        if (chatMsgs) {
            placeholderEl = document.createElement('div');
            placeholderEl.className = 'chat-message chat-compact-marker compact-marker-summarizing';
            placeholderEl.dataset.compactState = 'loading';
            placeholderEl.innerHTML = `
              <div class="compact-marker-content">
                <div class="compact-marker-rule compact-marker-rule-left"></div>
                <div class="compact-marker-pill">
                  <span class="compact-summarizing-dots"><span></span><span></span><span></span></span>
                  <span class="compact-marker-label">Summarizing conversation…</span>
                </div>
                <div class="compact-marker-rule compact-marker-rule-right"></div>
              </div>`;
            chatMsgs.appendChild(placeholderEl);
            chatMsgs.scrollTop = chatMsgs.scrollHeight;
        }
    }

    let tombstoneContent;
    let isSummarized = false;

    if (summarize) {
        const summary = await window.fetchSummary(dropped);
        if (summary) {
            const ctxNote = tab.lastCtxPct > 0 ? ` · was ${tab.lastCtxPct}% ctx` : '';
            tombstoneContent = `[Context compacted — ${dropped.length} messages summarized${ctxNote}]\n\n${summary}`;
            isSummarized = true;
        } else {
            tombstoneContent = `[Context compacted — server unavailable for summarization; ${dropped.length} messages dropped]`;
        }
    } else {
        const ctxNote = tab.lastCtxPct > 0 ? ` · was ${tab.lastCtxPct}% ctx` : '';
        tombstoneContent = `[Context compacted — ${dropped.length} messages removed${ctxNote}]`;
    }

    const tombstone = {
        role: 'system',
        content: tombstoneContent,
        compaction_marker: true,
        timestamp_ms: Date.now(),
        summarized: isSummarized,
        dropped_count: dropped.length,
        dropped_preview: dropped.slice(0, 8).map(m => ({ role: m.role, snippet: m.content.slice(0, 80) })),
        tokens_freed_estimate: dropped.reduce((sum, m) => sum + Math.round((m.input_tokens || 0) + (m.output_tokens || 0)), 0),
        ctx_pct_before: tab.lastCtxPct || 0,
    };

    if (placeholderEl) placeholderEl.remove();

    tab.messages = [
        ...(systemMsg ? [systemMsg] : []),
        ...tombstones,
        tombstone,
        ...kept,
    ];
    const finalTombstones = tab.messages.filter(m => m.compaction_marker);
    console.log('[COMPACT] done — final:', tab.messages.length, 'tombstones:', finalTombstones.length);
    if (finalTombstones.length !== tombstones.length + 1) {
        console.warn('[COMPACT] MISMATCH — expected', tombstones.length + 1, 'got', finalTombstones.length);
        console.warn('[COMPACT] kept markers:', kept.filter(m => m.compaction_marker).length);
    }
    tab.updated_at = Date.now();
    window.scheduleChatPersist();
    window.renderChatMessages();
    setCompactButtonBusy(false);
    window.compactionInProgress = false;

    setTimeout(() => {
        const markers = document.querySelectorAll('.chat-compact-marker');
        if (markers.length > 0) {
            markers[markers.length - 1].scrollIntoView({ behavior: 'smooth', block: 'center' });
        }
    }, 80);
}

function setCompactButtonBusy(isBusy) {
    const btn = document.getElementById('btn-compact');
    if (!btn) return;
    btn.classList.toggle('chat-btn-busy', isBusy);
    btn.disabled = isBusy;
}

function onManualCompact() {
    const tab = activeChatTab();
    if (!tab) return;
    compactChatTab(tab);
}

function onAutoCompactChange(checked) {
    const tab = activeChatTab();
    if (!tab) return;
    tab.auto_compact = checked;
    tab.updated_at = Date.now();
    document.getElementById('compact-threshold-field').style.opacity = checked ? '1' : '0.4';
    const summarizeField = document.getElementById('compact-summarize-field');
    if (summarizeField) summarizeField.style.opacity = checked ? '1' : '0.4';
    window.scheduleChatPersist();
}

function onAutoCompactSummarizeChange(checked) {
    const tab = activeChatTab();
    if (!tab) return;
    tab.auto_compact_summarize = checked;
    tab.updated_at = Date.now();
    window.scheduleChatPersist();
}

function onCompactModeChange(mode) {
    const tab = activeChatTab();
    if (!tab) return;
    tab.compact_mode = mode;
    tab.updated_at = Date.now();
    window.scheduleChatPersist();
    syncCompactSettingsUI(tab);
}

function onCompactThresholdChange(value) {
    const tab = activeChatTab();
    if (!tab) return;
    tab.compact_threshold = value / 100;
    tab.updated_at = Date.now();
    document.getElementById('chat-compact-threshold-val').textContent = `${value}%`;
    window.scheduleChatPersist();
}

function updateCtxPressureBar(pct) {
    const bar = document.getElementById('ctx-pressure-bar');
    const fill = document.getElementById('ctx-pressure-fill');
    if (!bar || !fill) return;
    if (!pct || pct <= 0) { bar.style.display = 'none'; return; }
    bar.style.display = 'block';
    fill.style.width = Math.min(pct, 100) + '%';
    fill.className = 'ctx-pressure-fill' +
        (pct >= 90 ? ' ctx-pressure-critical' :
         pct >= 75 ? ' ctx-pressure-high' :
         pct >= 50 ? ' ctx-pressure-medium' : '');

    const textarea = document.getElementById('chat-input');
    if (textarea) {
        textarea.classList.remove('ctx-pressure-medium', 'ctx-pressure-high', 'ctx-pressure-critical');
        if (pct >= 90) textarea.classList.add('ctx-pressure-critical');
        else if (pct >= 75) textarea.classList.add('ctx-pressure-high');
        else if (pct >= 50) textarea.classList.add('ctx-pressure-medium');
    }
}

function syncCompactSettingsUI(tab) {
    const autoToggle = document.getElementById('chat-auto-compact');
    const thresholdSlider = document.getElementById('chat-compact-threshold');
    const thresholdVal = document.getElementById('chat-compact-threshold-val');
    const thresholdField = document.getElementById('compact-threshold-field');
    if (!autoToggle || !thresholdSlider) return;

    const isOn = !!tab?.auto_compact;
    const mode = tab?.compact_mode || 'percent';
    const isOptimized = mode === 'optimized';

    autoToggle.checked = isOn;

    document.getElementById('compact-mode-percent')?.classList.toggle('compact-mode-pill-active', !isOptimized);
    document.getElementById('compact-mode-optimized')?.classList.toggle('compact-mode-pill-active', isOptimized);
    document.getElementById('compact-mode-help-percent').style.display = isOptimized ? 'none' : '';
    document.getElementById('compact-mode-help-optimized').style.display = isOptimized ? '' : 'none';

    const modeField = document.getElementById('compact-mode-field');
    if (modeField) modeField.style.opacity = isOn ? '1' : '0.4';
    thresholdField.style.display = isOptimized ? 'none' : '';
    thresholdField.style.opacity = isOn ? '1' : '0.4';
    thresholdSlider.value = (tab?.compact_threshold || 0.8) * 100;
    thresholdVal.textContent = `${thresholdSlider.value}%`;

    const summarizeToggle = document.getElementById('chat-auto-compact-summarize');
    const summarizeField = document.getElementById('compact-summarize-field');
    if (summarizeToggle) summarizeToggle.checked = !!tab?.auto_compact_summarize;
    if (summarizeField) summarizeField.style.opacity = isOn ? '1' : '0.4';

    const btn = document.getElementById('btn-compact');
    if (btn && tab) {
        const conversational = tab.messages.filter(m => m.role !== 'system' && !m.compaction_marker);
        const willDrop = Math.max(0, conversational.length - 10);
        btn.title = willDrop > 0
            ? `Trim context — will remove ${willDrop} oldest messages`
            : 'Trim context — nothing to remove yet';
    }
}

// ── Style / Font / Enter-to-send ──────────────────────────────────────────────

function applyChatStyle(style) {
    const page = document.getElementById('page-chat');
    if (page) {
        page.dataset.chatStyle = style;
    }
}

const CHAT_STYLES = ['rounded', 'compact', 'minimal', 'bubbly'];
const CHAT_STYLE_LABELS = { rounded: 'Rounded', compact: 'Compact', minimal: 'Minimal', bubbly: 'Bubbly' };

function toggleStylePanel() {
    const panel = document.getElementById('chat-style-panel');
    const isOpen = panel.style.display !== 'none';
    panel.style.display = isOpen ? 'none' : 'block';
    if (!isOpen) {
        const current = document.getElementById('page-chat')?.dataset.chatStyle || 'rounded';
        panel.querySelectorAll('.chat-style-card').forEach(card => {
            card.classList.toggle('active', card.dataset.style === current);
        });
        document.getElementById('chat-system-panel').style.display = 'none';
        document.getElementById('chat-params-panel').style.display = 'none';
    }
}

function selectChatStyle(style) {
    applyChatStyle(style);
    localStorage.setItem('llama-monitor-chat-style', style);
    updateChatStyleLabel(style);
    const select = document.getElementById('pref-chat-style');
    if (select) select.value = style;
    document.getElementById('chat-style-panel').style.display = 'none';
    window.showToast(`Style: ${CHAT_STYLE_LABELS[style]}`, 'success');
}

function updateChatStyleLabel(style) {
    const label = document.getElementById('chat-style-label');
    if (label) label.textContent = CHAT_STYLE_LABELS[style] || 'Rounded';
}

function adjustChatFont(delta) {
    window.chatFontSize = Math.max(70, Math.min(150, window.chatFontSize + delta * 10));
    localStorage.setItem('llama-monitor-chat-font', window.chatFontSize);
    applyChatFontSize();
}

function applyChatFontSize() {
    const messages = document.getElementById('chat-messages');
    if (messages) {
        messages.style.setProperty('--chat-font-scale', window.chatFontSize / 100);
    }
    const label = document.getElementById('chat-font-value');
    if (label) label.textContent = window.chatFontSize + '%';
}

function onEnterToggleChange(checked) {
    window.enterToSend = checked;
    localStorage.setItem('llama-monitor-enter-to-send', checked ? 'true' : 'false');
    const prefCheckbox = document.getElementById('pref-enter-to-send');
    if (prefCheckbox) prefCheckbox.checked = checked;
}

function initEnterToggle() {
    const toggle = document.getElementById('chat-enter-toggle-input');
    if (toggle) toggle.checked = window.enterToSend;
}

function initChatStyle() {
    const savedChatStyle = localStorage.getItem('llama-monitor-chat-style') || 'rounded';
    applyChatStyle(savedChatStyle);
    const select = document.getElementById('pref-chat-style');
    if (select) select.value = savedChatStyle;
    updateChatStyleLabel(savedChatStyle);
}

function initChatInputHandler() {
    const input = document.getElementById('chat-input');
    if (!input) return;
    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && !e.shiftKey && window.enterToSend) {
            e.preventDefault();
            window.sendChat();
        }
    });
}

// ── Chat names ────────────────────────────────────────────────────────────────

function loadChatNames() {
    const tab = activeChatTab();
    if (!tab) return;

    const aiInput = document.getElementById('chat-ai-name');
    const userInput = document.getElementById('chat-user-name');

    if (aiInput) aiInput.value = tab.ai_name || '';
    if (userInput) userInput.value = tab.user_name || '';
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initChatParams() {
    // Call setup functions that bind DOM event listeners
    applyChatFontSize();
    initEnterToggle();
    initChatStyle();
    initChatInputHandler();

    // Bind chat header buttons
    document.getElementById('btn-system-prompt')?.addEventListener('click', () => window.toggleSystemPromptPanel());
    document.getElementById('btn-model-params')?.addEventListener('click', toggleModelParamsPanel);
    document.getElementById('btn-chat-style')?.addEventListener('click', toggleStylePanel);
    document.getElementById('btn-compact')?.addEventListener('click', onManualCompact);

    // Bind chat name inputs
    document.getElementById('chat-ai-name')?.addEventListener('input', (e) => window.updateChatName('ai_name', e.target.value));
    document.getElementById('chat-user-name')?.addEventListener('input', (e) => window.updateChatName('user_name', e.target.value));

    // Bind explicit toggle (footer)
    document.getElementById('chat-explicit-toggle-footer')?.addEventListener('click', () => window.toggleExplicitMode());

    // Bind font controls
    document.getElementById('chat-font-decrease')?.addEventListener('click', () => adjustChatFont(-1));
    document.getElementById('chat-font-increase')?.addEventListener('click', () => adjustChatFont(1));

    // Bind export button
    document.getElementById('chat-export-btn')?.addEventListener('click', () => window.exportChatTab());

    // Bind chat style cards (event delegation)
    const styleGrid = document.getElementById('chat-style-grid');
    if (styleGrid) {
        styleGrid.addEventListener('click', (e) => {
            const card = e.target.closest('.chat-style-card');
            if (card) selectChatStyle(card.dataset.style);
        });
    }

    // Bind system prompt panel
    document.getElementById('chat-copy-settings-btn')?.addEventListener('click', showCopySettingsDropdown);
    document.getElementById('chat-template-select')?.addEventListener('change', (e) => window.applySystemPromptTemplate(e.target.value));
    document.getElementById('chat-template-mgmt-btn')?.addEventListener('click', () => window.openTemplateManager());
    document.getElementById('chat-explicit-toggle-settings')?.addEventListener('click', () => window.toggleExplicitMode());
    document.getElementById('chat-system-input')?.addEventListener('input', () => window.onSystemPromptChange());
    document.getElementById('chat-msg-limit')?.addEventListener('input', (e) => onMessageLimitChange(+e.target.value));
    document.getElementById('chat-auto-compact')?.addEventListener('change', (e) => onAutoCompactChange(e.target.checked));
    document.getElementById('compact-mode-percent')?.addEventListener('click', () => onCompactModeChange('percent'));
    document.getElementById('compact-mode-optimized')?.addEventListener('click', () => onCompactModeChange('optimized'));
    document.getElementById('chat-compact-threshold')?.addEventListener('input', (e) => onCompactThresholdChange(+e.target.value));
    document.getElementById('chat-auto-compact-summarize')?.addEventListener('change', (e) => onAutoCompactSummarizeChange(e.target.checked));

    // Bind model params panel
    document.getElementById('chat-advanced-toggle')?.addEventListener('click', toggleAdvancedParams);
    document.getElementById('chat-reset-params-btn')?.addEventListener('click', resetParamsToDefaults);

    // Bind param sliders
    document.getElementById('param-temperature')?.addEventListener('input', (e) => onParamChange('temperature', +e.target.value));
    document.getElementById('param-top-p')?.addEventListener('input', (e) => onParamChange('top_p', +e.target.value));
    document.getElementById('param-top-k')?.addEventListener('input', (e) => onParamChange('top_k', +e.target.value));
    document.getElementById('param-min-p')?.addEventListener('input', (e) => onParamChange('min_p', +e.target.value));
    document.getElementById('param-repeat-penalty')?.addEventListener('input', (e) => onParamChange('repeat_penalty', +e.target.value));
    document.getElementById('param-max-tokens')?.addEventListener('input', (e) => onParamChange('max_tokens', e.target.value ? +e.target.value : null));
    document.getElementById('param-stream-timeout')?.addEventListener('input', (e) => onParamChange('stream_timeout', +e.target.value));

    // Bind enter toggle
    document.getElementById('chat-enter-toggle-input')?.addEventListener('change', (e) => onEnterToggleChange(e.target.checked));

    // Keep on window for cross-module calls
    window.applyChatStyle = applyChatStyle;
    window.updateParamsDirtyIndicator = updateParamsDirtyIndicator;
    window.syncMessageLimitInput = syncMessageLimitInput;
    window.updateCtxPressureBar = updateCtxPressureBar;
    window.syncCompactSettingsUI = syncCompactSettingsUI;
    window.loadChatNames = loadChatNames;
    window.updateChatName = updateChatName;
    window.toggleSystemPromptPanel = toggleSystemPromptPanel;
    window.onSystemPromptChange = onSystemPromptChange;
    window.toggleExplicitMode = toggleExplicitMode;
}
