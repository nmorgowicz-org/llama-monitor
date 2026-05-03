// ── Chat Params ───────────────────────────────────────────────────────────────
// Model parameter panel, system prompt panel, style/font/enter-to-send controls,
// and compaction settings.

import { chat, lastLlamaMetrics } from '../core/app-state.js';
import {
    activeChatTab,
    registerChatViewBindings,
    scheduleChatPersist,
    updateChatName,
} from './chat-state.js';
import { exportChatTab, importChatTab, renderChatMessages } from './chat-render.js';
import { fetchSummary, sendChat } from './chat-transport.js';
import {
    applySystemPromptTemplate,
} from './chat-templates.js';
import { renderPersonaStrip } from './chat-render.js';
import {
    onSystemPromptChange,
    openTemplateManager,
    toggleExplicitMode,
    toggleSystemPromptPanel,
} from './chat-templates.js';
import { showToast, showToastWithActions } from './toast.js';

// Local state — previously on window, migrated to local variables
let chatFont = parseInt(localStorage.getItem('llama-monitor-chat-font') || '100');
let enterToSend = localStorage.getItem('llama-monitor-enter-to-send') !== 'false';
let paramToastTimer = null;

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
    scheduleChatPersist();
    clearTimeout(paramToastTimer);
    paramToastTimer = setTimeout(() => showToast('Parameter saved', 'success'), 2000);
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
    scheduleChatPersist();
    updateParamsDirtyIndicator();
    showToast('Parameters reset to defaults', 'success');
}

export function updateParamsDirtyIndicator() {
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
    const source = chat.tabs.find(t => t.id === sourceId);
    const target = activeChatTab();
    if (!source || !target || source.id === target.id) return;
    target.system_prompt = source.system_prompt;
    target.model_params = JSON.parse(JSON.stringify(source.model_params));
    target.updated_at = Date.now();
    scheduleChatPersist();
    syncParamPanelToTab();
    updateParamsDirtyIndicator();
    const indicator = document.getElementById('system-prompt-indicator');
    indicator.style.display = target.system_prompt ? 'inline' : 'none';
    document.getElementById('chat-system-input').value = target.system_prompt;
    showToast('Settings copied from "' + source.name + '"', 'success');
}

function showCopySettingsDropdown() {
    const target = activeChatTab();
    if (!target) return;
    const others = chat.tabs.filter(t => t.id !== target.id);
    if (others.length === 0) {
        showToast('No other tabs to copy from', 'info');
        return;
    }
    const toast = showToastWithActions(
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
    renderChatMessages();
    scheduleChatPersist();
}

function syncMessageLimitInput() {
    const tab = activeChatTab();
    const input = document.getElementById('chat-msg-limit');
    if (tab && input) input.value = tab.visible_message_limit || 15;
}

// ── Compaction ────────────────────────────────────────────────────────────────

// Estimate ctx% from message token metadata (mirrors deriveCtxPctFromMessages in context-card.js).
function estimateCtxPct(tab, capacity) {
    if (!capacity) return null;
    const asst = (tab.messages || []).filter(m => m.role === 'assistant' && !m.compaction_marker);
    if (!asst.length) return null;
    const totalOutput = asst.reduce((sum, m) => sum + (m.output_tokens || 0), 0);
    const lastInput = asst.at(-1).input_tokens || 0;
    return Math.min(200, (totalOutput + lastInput) / capacity * 100); // allow >100 so overflow is visible
}

// Calculate how many recent messages can be kept while staying within 65% of capacity.
// This leaves 35% for the system prompt, tombstone summary, and headroom for new responses.
// Falls back to the default if capacity is unknown or token metadata is sparse.
function calcKeepTailForCapacity(conversational, capacity) {
    if (!capacity || conversational.length === 0) return 15;
    const budget = Math.floor(capacity * 0.65);
    let tokensUsed = 0;
    let keep = 0;
    for (let i = conversational.length - 1; i >= 0; i--) {
        const m = conversational[i];
        // Use recorded tokens; fall back to rough char estimate (4 chars ≈ 1 token)
        const t = (m.input_tokens || 0) + (m.output_tokens || 0)
            || Math.ceil((m.content?.length || 0) / 4);
        if (tokensUsed + t > budget) break;
        tokensUsed += t;
        keep++;
    }
    // Must keep at least 1 and drop at least 1
    return Math.max(1, Math.min(keep, conversational.length - 1));
}

export async function compactChatTab(tab, keepTail = null, summarize = true) {
    const msgs = tab.messages;
    const systemMsg = msgs[0]?.role === 'system' && !msgs[0]?.compaction_marker ? msgs[0] : null;
    const tombstones = msgs.filter(m => m.compaction_marker);
    const conversational = msgs.filter(m => m.role !== 'system' && !m.compaction_marker);

    // Resolve keepTail: capacity-aware if not specified, so a small model gets a
    // smaller tail and the compacted conversation actually fits its context window.
    const capacity = lastLlamaMetrics?.context_capacity_tokens || lastLlamaMetrics?.kv_cache_max || 0;
    const resolvedKeepTail = keepTail ?? (capacity > 0
        ? calcKeepTailForCapacity(conversational, capacity)
        : 15);

    if (conversational.length <= resolvedKeepTail) return;

    chat.compactionInProgress = true;
    setCompactButtonBusy(true);

    const dropped = conversational.slice(0, conversational.length - resolvedKeepTail);
    const kept = conversational.slice(-resolvedKeepTail);
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
        const summary = await fetchSummary(dropped);
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
    scheduleChatPersist();
    renderChatMessages();
    setCompactButtonBusy(false);
    chat.compactionInProgress = false;

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

// Called after each response (via view binding) and after model switch.
// Fires compaction if auto_compact is on and the tab has hit its threshold.
export async function checkAutoCompact(tab) {
    if (!tab || !tab.auto_compact || chat.compactionInProgress || chat.busy) return;
    const capacity = lastLlamaMetrics?.context_capacity_tokens || lastLlamaMetrics?.kv_cache_max || 0;
    if (!capacity) return;

    const ctxPct = estimateCtxPct(tab, capacity);
    if (ctxPct === null) return;

    const mode = tab.compact_mode || 'percent';
    let shouldCompact = false;
    if (mode === 'optimized') {
        // Compact when fewer than 25k tokens remain
        const asstMsgs = (tab.messages || []).filter(m => m.role === 'assistant' && !m.compaction_marker);
        const totalOutput = asstMsgs.reduce((sum, m) => sum + (m.output_tokens || 0), 0);
        const lastInput = asstMsgs.at(-1)?.input_tokens || 0;
        shouldCompact = capacity - (totalOutput + lastInput) < 25_000;
    } else {
        const threshold = (tab.compact_threshold || 0.8) * 100;
        shouldCompact = ctxPct >= threshold;
    }

    if (shouldCompact) {
        await compactChatTab(tab, null, !!tab.auto_compact_summarize);
    }
}

function onAutoCompactChange(checked) {
    const tab = activeChatTab();
    if (!tab) return;
    tab.auto_compact = checked;
    tab.updated_at = Date.now();
    document.getElementById('compact-threshold-field').style.opacity = checked ? '1' : '0.4';
    const summarizeField = document.getElementById('compact-summarize-field');
    if (summarizeField) summarizeField.style.opacity = checked ? '1' : '0.4';
    scheduleChatPersist();
}

function onAutoCompactSummarizeChange(checked) {
    const tab = activeChatTab();
    if (!tab) return;
    tab.auto_compact_summarize = checked;
    tab.updated_at = Date.now();
    scheduleChatPersist();
}

function onCompactModeChange(mode) {
    const tab = activeChatTab();
    if (!tab) return;
    tab.compact_mode = mode;
    tab.updated_at = Date.now();
    scheduleChatPersist();
    syncCompactSettingsUI(tab);
}

function onCompactThresholdChange(value) {
    const tab = activeChatTab();
    if (!tab) return;
    tab.compact_threshold = value / 100;
    tab.updated_at = Date.now();
    document.getElementById('chat-compact-threshold-val').textContent = `${value}%`;
    scheduleChatPersist();
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
        const willDrop = Math.max(0, conversational.length - 15);
        btn.title = willDrop > 0
            ? `Compact context — will remove ${willDrop} oldest messages`
            : 'Compact context — nothing to remove yet';
    }
}

// ── Style / Font / Enter-to-send ──────────────────────────────────────────────

function applyChatStyle(style) {
    const page = document.getElementById('page-chat');
    if (page) {
        page.dataset.chatStyle = style;
    }
}

export { applyChatStyle };

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
    showToast(`Style: ${CHAT_STYLE_LABELS[style]}`, 'success');
}

function updateChatStyleLabel(style) {
    const label = document.getElementById('chat-style-label');
    if (label) label.textContent = CHAT_STYLE_LABELS[style] || 'Rounded';
}

function adjustChatFont(delta) {
    chatFont = Math.max(70, Math.min(150, chatFont + delta * 10));
    localStorage.setItem('llama-monitor-chat-font', chatFont);
    applyChatFontSize();
}

function applyChatFontSize() {
    const messages = document.getElementById('chat-messages');
    if (messages) {
        messages.style.setProperty('--chat-font-scale', chatFont / 100);
    }
    const inputRow = document.getElementById('chat-input-row');
    if (inputRow) {
        inputRow.style.setProperty('--chat-font-scale', chatFont / 100);
    }
    const label = document.getElementById('chat-font-value');
    if (label) label.textContent = chatFont + '%';
}

function onEnterToggleChange(checked) {
    enterToSend = checked;
    localStorage.setItem('llama-monitor-enter-to-send', checked ? 'true' : 'false');
    const prefCheckbox = document.getElementById('pref-enter-to-send');
    if (prefCheckbox) prefCheckbox.checked = checked;
}

export function getEnterToSend() {
    return enterToSend;
}

export function setEnterToSend(checked) {
    onEnterToggleChange(checked);
}

function initEnterToggle() {
    const toggle = document.getElementById('chat-enter-toggle-input');
    if (toggle) toggle.checked = enterToSend;
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
        if (e.key === 'Enter' && !e.shiftKey && enterToSend) {
            e.preventDefault();
            sendChat();
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
    initChatResizeHandle();

    // Bind chat header buttons
    document.getElementById('btn-system-prompt')?.addEventListener('click', toggleSystemPromptPanel);
    document.getElementById('btn-model-params')?.addEventListener('click', toggleModelParamsPanel);
    document.getElementById('btn-chat-style')?.addEventListener('click', toggleStylePanel);
    document.getElementById('btn-compact')?.addEventListener('click', onManualCompact);
    registerPersonaMenuBindings();
    registerTemplateMenuBindings();

    // Bind chat name inputs
    document.getElementById('chat-ai-name')?.addEventListener('input', (e) => updateChatName('ai_name', e.target.value));
    document.getElementById('chat-user-name')?.addEventListener('input', (e) => updateChatName('user_name', e.target.value));

    // Bind explicit toggle (footer)
    document.getElementById('chat-explicit-toggle-footer')?.addEventListener('click', toggleExplicitMode);

    // Bind font controls
    document.getElementById('chat-font-decrease')?.addEventListener('click', () => adjustChatFont(-1));
    document.getElementById('chat-font-increase')?.addEventListener('click', () => adjustChatFont(1));

    // Bind export button with dropdown menu
    const exportBtn = document.getElementById('chat-export-btn');
    const exportMenu = document.getElementById('chat-export-menu');
    if (exportBtn && exportMenu) {
        exportBtn.addEventListener('click', e => {
            e.stopPropagation();
            exportMenu.classList.toggle('hidden');
        });
        exportMenu.addEventListener('click', e => {
            const fmt = e.target.dataset.exportFormat;
            if (fmt) {
                exportChatTab(fmt);
                exportMenu.classList.add('hidden');
            }
        });
    }
    document.addEventListener('click', e => {
        if (!e.target.closest('#chat-export-btn') && !e.target.closest('#chat-export-menu')) {
            document.getElementById('chat-export-menu')?.classList.add('hidden');
        }
    });
    document.getElementById('chat-import-btn')?.addEventListener('click', importChatTab);

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
    document.getElementById('chat-template-select')?.addEventListener('change', (e) => applySystemPromptTemplate(e.target.value));
    document.getElementById('chat-template-mgmt-btn')?.addEventListener('click', openTemplateManager);
    document.getElementById('chat-explicit-toggle-settings')?.addEventListener('click', toggleExplicitMode);
    document.getElementById('chat-system-input')?.addEventListener('input', onSystemPromptChange);
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

    registerChatViewBindings({
        loadChatNames,
        syncCompactSettingsUI,
        syncMessageLimitInput,
        updateCtxPressureBar,
        updateParamsDirtyIndicator,
        checkAutoCompact,
    });
}

// ── Chat Input Resize Handle ─────────────────────────────────────────────────
let isResizing = false;
let inputRowEl = null;
let textareaEl = null;
let startY = 0;
let startHeight = 0;
const MIN_ROWS = 1;
const MAX_ROWS = 10;

function initChatResizeHandle() {
    const handle = document.getElementById('chat-resize-handle');
    if (!handle) return;
    
    inputRowEl = document.getElementById('chat-input-row');
    textareaEl = document.getElementById('chat-input');
    
    if (!inputRowEl || !textareaEl) return;
    
    // Restore saved height
    const savedHeight = localStorage.getItem('llama-monitor-input-height');
    if (savedHeight) {
        textareaEl.style.height = savedHeight;
        updateResizeHandleUI();
    }
    
    handle.addEventListener('mousedown', startResize);
    document.addEventListener('mousemove', doResize);
    document.addEventListener('mouseup', stopResize);
}

function startResize(e) {
    if (!textareaEl) return;
    isResizing = true;
    startY = e.clientY;
    startHeight = textareaEl.getBoundingClientRect().height;
    const handle = document.getElementById('chat-resize-handle');
    if (handle) handle.classList.add('active');
    e.preventDefault();
}

function doResize(e) {
    if (!isResizing || !textareaEl) return;
    const delta = e.clientY - startY;
    const minHeight = textareaEl.offsetHeight;
    const newHeight = Math.max(minHeight, startHeight + delta);
    const computedStyle = getComputedStyle(textareaEl);
    const padding = parseFloat(computedStyle.paddingTop) + parseFloat(computedStyle.paddingBottom);
    const border = parseFloat(computedStyle.borderTopWidth) + parseFloat(computedStyle.borderBottomWidth);
    const contentHeight = newHeight - padding - border;
    const lineHeight = parseFloat(computedStyle.lineHeight) || 24;
    const rows = Math.max(1, Math.round(contentHeight / lineHeight));
    textareaEl.style.height = newHeight + 'px';
    textareaEl.rows = Math.max(MIN_ROWS, Math.min(MAX_ROWS, rows));
    updateResizeHandleUI();
}

function stopResize() {
    if (!isResizing) return;
    isResizing = false;
    const handle = document.getElementById('chat-resize-handle');
    if (handle) handle.classList.remove('active');
    if (textareaEl) {
        localStorage.setItem('llama-monitor-input-height', textareaEl.style.height || '42px');
    }
}

function updateResizeHandleUI() {
    const handle = document.getElementById('chat-resize-handle');
    const hint = handle?.querySelector('.resize-hint');
    if (handle && textareaEl) {
        const height = textareaEl.getBoundingClientRect().height;
        const max = 200;
        const pct = Math.min(100, (height - 42) / (max - 42) * 100);
        handle.style.setProperty('--resize-pct', pct / 100);
    }
}

export function resetChatInputHeight() {
    if (textareaEl) {
        textareaEl.style.height = '';
        textareaEl.rows = 1;
        localStorage.removeItem('llama-monitor-input-height');
        updateResizeHandleUI();
    }
}

// ── Persona Menu Bindings ───────────────────────────────────────────────────

let personaMenuEl = null;
let personaMenuListEl = null;

export function registerPersonaMenuBindings() {
    const btn = document.getElementById('chat-persona-btn');
    const menu = document.getElementById('chat-persona-menu');
    const list = document.getElementById('chat-persona-menu-list');
    const name = document.getElementById('chat-persona-menu-name');
    
    personaMenuEl = menu;
    personaMenuListEl = list;
    
    if (!btn || !menu || !list || !name) return;
    
    btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const isVisible = !menu.classList.toggle('hidden');
        if (isVisible) {
            loadPersonaMenuItems();
        }
    });
    
    document.addEventListener('click', (e) => {
        if (!menu.contains(e.target)) {
            menu.classList.add('hidden');
        }
    });
}

async function loadPersonaMenuItems() {
    if (!personaMenuListEl) return;
    
    personaMenuListEl.innerHTML = '<div class="chat-persona-menu-loading">Loading personas...</div>';
    
    try {
        const response = await fetch('/api/templates');
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
        }
        
        const templates = await response.json();
        const personas = templates || [];
        
        if (personas.length === 0) {
            personaMenuListEl.innerHTML = '<div class="chat-persona-menu-loading">No personas found</div>';
            return;
        }
        
        personaMenuListEl.innerHTML = '';
        
        personas.forEach((persona) => {
            const item = document.createElement('button');
            item.className = 'chat-persona-menu-item';
            if (window.currentPersona && window.currentPersona.name === persona.name) {
                item.classList.add('active');
            }
            
            const icon = document.createElement('span');
            icon.className = 'chat-persona-menu-item-icon';
            icon.textContent = '🎭';
            
            const content = document.createElement('div');
            content.className = 'chat-persona-menu-item-content';
            
            const nameEl = document.createElement('div');
            nameEl.className = 'chat-persona-menu-item-name';
            nameEl.textContent = persona.name;
            content.appendChild(nameEl);
            
            if (persona.description) {
                const meta = document.createElement('div');
                meta.className = 'chat-persona-menu-item-meta';
                meta.textContent = persona.description.substring(0, 60);
                content.appendChild(meta);
            }
            
            item.appendChild(icon);
            item.appendChild(content);
            
            item.addEventListener('click', () => {
                window.currentPersona = persona;
                document.getElementById('chat-persona-menu-name').textContent = persona.name;
                document.getElementById('chat-persona-menu').classList.add('hidden');
                // Re-render persona chips to show active state
                renderPersonaStrip?.();
            });
            
            personaMenuListEl.appendChild(item);
        });
    } catch (err) {
        const errorEl = document.createElement('div');
        errorEl.className = 'chat-persona-menu-loading';
        errorEl.textContent = 'Error: ' + err.message;
        personaMenuListEl.appendChild(errorEl);
    }
}

export function setPersonaMenuActive(personaName) {
    const items = personaMenuListEl?.querySelectorAll('.chat-persona-menu-item');
    items?.forEach(item => {
        if (item.textContent.includes(personaName)) {
            item.classList.add('active');
        } else {
            item.classList.remove('active');
        }
    });
    const nameEl = document.getElementById('chat-persona-menu-name');
    if (nameEl && personaName) {
        nameEl.textContent = personaName;
    }
}

// ── Template Menu Bindings ──────────────────────────────────────────────────

let templateMenuEl = null;
let templateMenuListEl = null;

export function registerTemplateMenuBindings() {
    const btn = document.getElementById('chat-template-select');
    const menu = document.getElementById('chat-template-menu');
    const list = document.getElementById('chat-template-menu-list');
    
    templateMenuEl = menu;
    templateMenuListEl = list;
    
    if (!btn || !menu || !list) return;
    
    btn.addEventListener('change', (e) => {
        const templateName = e.target.value;
        if (templateName && window.setActiveTemplate) {
            window.setActiveTemplate(templateName);
        }
    });
    
    btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const isVisible = !menu.classList.toggle('hidden');
        if (isVisible) {
            loadTemplateMenuItems();
        }
    });
    
    document.addEventListener('click', (e) => {
        if (!menu.contains(e.target) && e.target.id !== 'chat-template-select') {
            menu.classList.add('hidden');
        }
    });
}

async function loadTemplateMenuItems() {
    if (!templateMenuListEl) return;
    
    templateMenuListEl.innerHTML = '<div class="chat-persona-menu-loading">Loading templates...</div>';
    
    try {
        const response = await fetch('/api/chat-templates');
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
        }
        
        const data = await response.json();
        const templates = data.templates || [];
        
        if (templates.length === 0) {
            templateMenuListEl.innerHTML = '<div class="chat-persona-menu-loading">No templates found</div>';
            return;
        }
        
        templateMenuListEl.innerHTML = '';
        
        templates.forEach((template) => {
            const item = document.createElement('button');
            item.className = 'chat-persona-menu-item';
            if (window.currentTemplate && window.currentTemplate.name === template.name) {
                item.classList.add('active');
            }
            
                const icon = document.createElement('span');
            icon.className = 'chat-persona-menu-item-icon';
            icon.textContent = '📝';
            
            const content = document.createElement('div');
            content.className = 'chat-persona-menu-item-content';
            
            const nameEl = document.createElement('div');
            nameEl.className = 'chat-persona-menu-item-name';
            nameEl.textContent = template.name;
            content.appendChild(nameEl);
            
            if (template.description) {
                const meta = document.createElement('div');
                meta.className = 'chat-persona-menu-item-meta';
                meta.textContent = template.description.substring(0, 60);
                content.appendChild(meta);
            }
            
            item.appendChild(icon);
            item.appendChild(content);
            
            item.addEventListener('click', async () => {
                try {
                    const res = await fetch(`/api/chat-templates/activate/${encodeURIComponent(template.name)}`, {
                        method: 'POST',
                    });
                    if (!res.ok) {
                        throw new Error(`HTTP ${res.status}`);
                    }
                    window.currentTemplate = template;
                    document.getElementById('chat-template-select').value = template.name;
                    document.getElementById('chat-template-menu').classList.add('hidden');
                    // Re-render to apply template
                    renderPersonaStrip?.();
                    window.setActiveTemplate?.(template.id);
                } catch (err) {
                    console.error('Failed to activate template:', err);
                    alert('Failed to activate template: ' + err.message);
                }
            });
            
            templateMenuListEl.appendChild(item);
        });
    } catch (err) {
        const errorEl = document.createElement('div');
        errorEl.className = 'chat-persona-menu-loading';
        errorEl.textContent = 'Error: ' + err.message;
        templateMenuListEl.appendChild(errorEl);
    }
}

export function setTemplateMenuActive(templateName) {
    const items = templateMenuListEl?.querySelectorAll('.chat-persona-menu-item');
    items?.forEach(item => {
        if (item.textContent.includes(templateName)) {
            item.classList.add('active');
        } else {
            item.classList.remove('active');
        }
    });
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}
