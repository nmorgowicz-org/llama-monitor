// ── Context Window Card ──────────────────────────────────────────────────────
// Hybrid runtime/chat-derived context card with dual gauge/fleet views.

import { chat } from '../core/app-state.js';
import { escapeHtml, formatMetricNumber } from '../core/format.js';
import { setCardState, setChipState, setEmptyState } from './dashboard-render.js';
import { scheduleChatPersist } from './chat-state.js';

const STALE_CHAT_MS = 7 * 24 * 60 * 60 * 1000;
const MAX_VISIBLE_CHATS = 5;
const MAX_VISIBLE_FLEET_ROWS = 4;

let initialized = false;
let cached = null;
let currentView = 'gauge';
let expanded = false;
let lastDashboard = null;
let lastLlamaMetrics = null;

function ensureElements() {
    if (cached) return cached;
    cached = {
        card: document.querySelector('.widget-context'),
        stateChip: document.getElementById('m-context-state'),
        empty: document.getElementById('m-context-empty'),
        subtitle: document.getElementById('m-context-subtitle'),
        gaugeView: document.getElementById('context-view-gauge'),
        fleetView: document.getElementById('context-view-fleet'),
        gaugeToggle: document.getElementById('context-view-toggle-gauge'),
        fleetToggle: document.getElementById('context-view-toggle-fleet'),
        gaugeValue: document.getElementById('m-context-gauge-value'),
        gaugeSecondary: document.getElementById('m-context-gauge-secondary'),
        gaugeRing: document.getElementById('m-context-gauge-ring'),
        strip: document.getElementById('m-context-chat-strip'),
        stripMeta: document.getElementById('m-context-chat-strip-meta'),
        fleetSummary: document.getElementById('m-context-fleet-summary'),
        fleetRows: document.getElementById('m-context-fleet-rows'),
        fleetFooter: document.getElementById('m-context-fleet-footer'),
    };
    return cached;
}

function pctState(pct) {
    if (pct == null) return 'unknown';
    if (pct >= 90) return 'critical';
    if (pct >= 75) return 'warning';
    if (pct >= 50) return 'warm';
    return 'idle';
}

// Compute ctx% for a tab from its message history.
// Formula: (cumulative output tokens + last request's input tokens) / capacity.
// KV cache means each request's input_tokens is incremental, not the full context.
// All output tokens remain in the KV cache as conversation content grows.
function deriveCtxPctFromMessages(tab, capacity) {
    if (!capacity) return null;
    const messages = tab.messages || [];
    const asst = messages.filter(m => m.role === 'assistant' && (m.output_tokens || m.input_tokens));
    if (!asst.length) return null;
    const totalOutput = asst.reduce((sum, m) => sum + (m.output_tokens || 0), 0);
    const lastInput = asst.at(-1).input_tokens || 0;
    const ctxTokens = totalOutput + lastInput;
    return Math.min(100, Math.round((ctxTokens / capacity) * 100));
}

// Backfill lastCtxPct for tabs that don't have it persisted yet.
// Called when llama metrics arrive with a known capacity.
function backfillCtxPct(capacity) {
    let dirty = false;
    for (const tab of chat.tabs) {
        if (!tab.lastCtxPct) {
            const derived = deriveCtxPctFromMessages(tab, capacity);
            if (derived != null && derived > 0) {
                tab.lastCtxPct = derived;
                dirty = true;
            }
        }
    }
    if (dirty) scheduleChatPersist();
}

function deriveChatSummaries(capacity) {
    const now = Date.now();
    return chat.tabs
        .map(tab => {
            const messages = tab.messages || [];
            const lastTimestamp = messages.reduce((max, msg) => Math.max(max, msg.timestamp_ms || 0), 0)
                || tab.updated_at
                || tab.created_at
                || 0;

            // Prefer lastCtxPct (already computed / persisted), then derive from messages.
            let ctxPct = (typeof tab.lastCtxPct === 'number' && tab.lastCtxPct > 0)
                ? tab.lastCtxPct
                : (capacity ? deriveCtxPctFromMessages(tab, capacity) : null);

            return {
                id: tab.id,
                name: tab.name || 'Untitled chat',
                ctxPct,
                state: pctState(ctxPct),
                lastMessageTimestamp: lastTimestamp,
                isStale: lastTimestamp > 0 ? now - lastTimestamp > STALE_CHAT_MS : true,
                autoCompact: !!tab.auto_compact,
                inputTokens: tab.totalInputTokens || 0,
                outputTokens: tab.totalOutputTokens || 0,
                messageCount: messages.filter(msg => msg.role !== 'system').length,
            };
        })
        .sort((a, b) => b.lastMessageTimestamp - a.lastMessageTimestamp);
}

function deriveContextViewModel(d, l) {
    const capacityTokens = l?.context_capacity_tokens || l?.kv_cache_max || 0;
    const runtimeLiveTokens = l?.context_live_tokens || l?.kv_cache_tokens || 0;
    const runtimeLiveAvailable = !!(l?.context_live_tokens_available || l?.kv_cache_tokens_available);
    const chatSummaries = deriveChatSummaries(capacityTokens);
    const trackedChats = chatSummaries.filter(chatItem => chatItem.messageCount > 0);
    const nonStaleChats = trackedChats.filter(chatItem => !chatItem.isStale);
    const pressureKnown = trackedChats.filter(chatItem => chatItem.ctxPct != null);
    const activeChatCount = trackedChats.length;
    const pressuredChatCount = pressureKnown.filter(chatItem => (chatItem.ctxPct || 0) >= 75).length;
    // Show the most recently active chat in the gauge center (chatSummaries sorted by timestamp desc)
    const busiestChat = trackedChats[0] || null;
    const avgPct = pressureKnown.length ? pressureKnown.reduce((sum, item) => sum + item.ctxPct, 0) / pressureKnown.length : null;
    const maxPct = pressureKnown.length ? Math.max(...pressureKnown.map(item => item.ctxPct)) : null;
    const staleChatCount = trackedChats.filter(chatItem => chatItem.isStale).length;

    let mode = 'empty';
    if (capacityTokens > 0 && runtimeLiveAvailable) mode = 'live-runtime';
    else if (trackedChats.length > 0) mode = 'chat-derived';
    else if (capacityTokens > 0) mode = 'capacity-only';

    const runtimeLivePct = capacityTokens > 0 && runtimeLiveAvailable ? (runtimeLiveTokens / capacityTokens) * 100 : null;
    const primaryChats = nonStaleChats.length ? nonStaleChats : trackedChats.slice(0, 1);

    let note = null;
    if (mode === 'live-runtime') {
        note = `${formatMetricNumber(runtimeLiveTokens)} / ${formatMetricNumber(capacityTokens)} live`;
    } else if (mode === 'chat-derived') {
        note = pressureKnown.length > 0
            ? `${activeChatCount} chat${activeChatCount !== 1 ? 's' : ''} · ${pressuredChatCount} under pressure`
            : `${activeChatCount} chat${activeChatCount !== 1 ? 's' : ''} tracked`;
    } else if (mode === 'capacity-only') {
        note = `Context size ${formatMetricNumber(capacityTokens)}`;
    } else {
        note = 'Start a chat or attach to a server to track context pressure';
    }

    return {
        mode,
        viewMode: currentView,
        capacityTokens,
        runtimeLiveTokens: runtimeLiveAvailable ? runtimeLiveTokens : null,
        runtimeLivePct,
        activeChatCount,
        pressuredChatCount,
        busiestChat,
        chatSummaries,
        staleChatCount,
        aggregateChatPressure: { avgPct, maxPct },
        note,
        hasActiveEndpoint: !!(d?.active_session_endpoint),
        nonStaleChats,
        primaryChats,
    };
}

function renderOverflowButton(className, overflow) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = className;
    button.dataset.action = 'toggle-overflow';
    button.textContent = expanded ? 'Show less' : `+${overflow} more`;
    return button;
}

function renderChatStrip(model) {
    const { strip, stripMeta } = ensureElements();
    const source = model.nonStaleChats.length ? model.nonStaleChats : model.chatSummaries;
    const visible = expanded ? source : source.slice(0, MAX_VISIBLE_CHATS);
    const overflow = Math.max(0, source.length - visible.length);
    strip.innerHTML = '';

    if (visible.length === 0) {
        strip.innerHTML = '<div class="context-pill context-pill-empty">No tracked chats yet</div>';
        stripMeta.textContent = model.mode === 'capacity-only'
            ? `Capacity ${formatMetricNumber(model.capacityTokens)}`
            : 'Chat context appears after conversations begin';
        return;
    }

    for (const item of visible) {
        const pill = document.createElement('div');
        pill.className = `context-pill ${item.state}`;
        pill.innerHTML = `
            <span class="context-pill-name">${escapeHtml(item.name)}</span>
            <span class="context-pill-value">${escapeHtml(item.ctxPct != null ? Math.round(item.ctxPct) + '%' : '—')}</span>
        `;
        strip.appendChild(pill);
    }

    if (overflow > 0) {
        const more = renderOverflowButton('context-pill context-pill-overflow', overflow);
        strip.appendChild(more);
    }

    // Only show meta when it adds info the pills don't already convey
    stripMeta.textContent = model.staleChatCount > 0
        ? `${model.staleChatCount} stale chat${model.staleChatCount !== 1 ? 's' : ''}`
        : model.capacityTokens > 0
            ? `${formatMetricNumber(model.capacityTokens)} ctx window`
            : '';
}

const GAUGE_CIRCUMFERENCE = 402; // 2π × r64

function renderGaugeView(model) {
    const { gaugeValue, gaugeSecondary, gaugeRing } = ensureElements();
    const heroPct = model.mode === 'live-runtime'
        ? model.runtimeLivePct
        : model.mode === 'chat-derived'
            ? (model.busiestChat?.ctxPct ?? model.aggregateChatPressure.maxPct)
            : null;
    const displayPct = heroPct != null ? Math.max(0, Math.min(100, heroPct)) : 0;

    if (gaugeRing) {
        const offset = GAUGE_CIRCUMFERENCE * (1 - displayPct / 100);
        gaugeRing.style.strokeDashoffset = String(offset);
        const stateClass = `state-${pctState(heroPct)}`;
        const animClass = model.mode === 'live-runtime' ? 'live' : heroPct != null ? 'chat' : 'idle';
        gaugeRing.setAttribute('class', `context-gauge-ring ${stateClass} ${animClass}`);
    }

    if (model.mode === 'live-runtime') {
        gaugeValue.textContent = `${Math.round(displayPct)}%`;
        gaugeSecondary.textContent = `${formatMetricNumber(model.runtimeLiveTokens)} / ${formatMetricNumber(model.capacityTokens)} live`;
    } else if (model.mode === 'chat-derived') {
        gaugeValue.textContent = heroPct != null ? `${Math.round(displayPct)}%` : '—';
        gaugeSecondary.textContent = model.busiestChat?.name
            ?? `${model.activeChatCount} chat${model.activeChatCount !== 1 ? 's' : ''}`;
    } else if (model.mode === 'capacity-only') {
        gaugeValue.textContent = formatMetricNumber(model.capacityTokens);
        gaugeSecondary.textContent = 'Capacity';
    } else {
        gaugeValue.textContent = '—';
        gaugeSecondary.textContent = 'Attach to a server or start a chat';
    }

    renderChatStrip(model);
}

function renderFleetView(model) {
    const { fleetSummary, fleetRows, fleetFooter } = ensureElements();
    const source = model.nonStaleChats.length ? model.nonStaleChats : model.chatSummaries;
    const rows = expanded ? source : source.slice(0, MAX_VISIBLE_FLEET_ROWS);

    fleetSummary.textContent = model.mode === 'live-runtime'
        ? `${model.activeChatCount} chat${model.activeChatCount !== 1 ? 's' : ''} · runtime live`
        : model.mode === 'chat-derived'
            ? `${model.activeChatCount} chat${model.activeChatCount !== 1 ? 's' : ''} · ${model.pressuredChatCount} under pressure`
            : model.mode === 'capacity-only'
                ? `Capacity ${formatMetricNumber(model.capacityTokens)}`
                : 'No active chat context yet';

    if (rows.length === 0) {
        fleetRows.innerHTML = '<div class="context-fleet-empty">Start a chat to build context intelligence.</div>';
        fleetFooter.textContent = model.note || '';
        return;
    }

    // eslint-disable-next-line no-unsanitized/property
    fleetRows.innerHTML = rows.map(item => {
        const pct = item.ctxPct != null ? Math.round(item.ctxPct) : null;
        const state = escapeHtml(item.state);
        const pctLabel = escapeHtml(pct != null ? pct + '%' : '—');
        const width = escapeHtml(String(pct != null ? Math.min(100, pct) : 0));
        return `
            <div class="context-fleet-row ${state}">
                <div class="context-fleet-row-top">
                    <span class="context-fleet-name">${escapeHtml(item.name)}</span>
                    <span class="context-fleet-value">${pctLabel}</span>
                </div>
                <div class="context-fleet-bar">
                    <div class="context-fleet-fill ${state}" style="width:${width}%"></div>
                </div>
            </div>
        `;
    }).join('');

    const overflow = Math.max(0, source.length - rows.length);
    if (overflow > 0) {
        const more = renderOverflowButton('context-fleet-more', overflow);
        fleetRows.appendChild(more);
    }

    fleetFooter.textContent = model.mode === 'live-runtime'
        ? `${formatMetricNumber(model.runtimeLiveTokens)} / ${formatMetricNumber(model.capacityTokens)} live`
        : model.capacityTokens > 0
            ? `${formatMetricNumber(model.capacityTokens)} ctx window`
            : '';
}

function applyViewMode(viewMode) {
    currentView = viewMode;
    const { gaugeView, fleetView, gaugeToggle, fleetToggle, subtitle } = ensureElements();
    gaugeView.classList.toggle('active', viewMode === 'gauge');
    gaugeView.classList.toggle('hidden', viewMode !== 'gauge');
    fleetView.classList.toggle('active', viewMode === 'fleet');
    fleetView.classList.toggle('hidden', viewMode !== 'fleet');
    gaugeToggle.classList.toggle('active', viewMode === 'gauge');
    fleetToggle.classList.toggle('active', viewMode === 'fleet');
    gaugeToggle?.setAttribute('aria-selected', String(viewMode === 'gauge'));
    fleetToggle?.setAttribute('aria-selected', String(viewMode === 'fleet'));
    // Subtitle only makes sense in gauge view; hide it in fleet to avoid duplicate info
    if (subtitle) subtitle.style.display = viewMode === 'gauge' ? '' : 'none';
}

function saveViewPreference() {
    fetch('/api/settings')
        .then(resp => resp.ok ? resp.json() : null)
        .then(settings => {
            if (!settings) return;
            return fetch('/api/settings', {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ ...settings, context_card_view: currentView }),
            });
        })
        .catch(() => {});
}

function setViewMode(viewMode, { persist = false } = {}) {
    if (viewMode !== 'gauge' && viewMode !== 'fleet') return;
    applyViewMode(viewMode);
    if (persist) saveViewPreference();
}

function handleCardClick(event) {
    if (event.target instanceof Element && event.target.closest('[data-action="toggle-overflow"]')) {
        expanded = !expanded;
        return true;
    }
    return false;
}

export function initContextCard() {
    if (initialized) return;
    initialized = true;
    const { card, gaugeToggle, fleetToggle } = ensureElements();
    gaugeToggle?.addEventListener('click', () => setViewMode('gauge', { persist: true }));
    fleetToggle?.addEventListener('click', () => setViewMode('fleet', { persist: true }));
    card?.addEventListener('click', event => {
        if (handleCardClick(event)) {
            renderContextCard();
        }
    });
    applyViewMode(currentView);
}

export function setContextCardViewPreference(viewMode) {
    currentView = viewMode === 'fleet' ? 'fleet' : 'gauge';
    if (initialized) applyViewMode(currentView);
}

function renderContextCard() {
    const elements = ensureElements();
    const model = deriveContextViewModel(lastDashboard || {}, lastLlamaMetrics);
    const visibleSource = model.nonStaleChats.length ? model.nonStaleChats : model.chatSummaries;
    if (visibleSource.length <= Math.max(MAX_VISIBLE_CHATS, MAX_VISIBLE_FLEET_ROWS)) {
        expanded = false;
    }

    setEmptyState(elements.empty, model.mode === 'empty' && !model.hasActiveEndpoint);
    setCardState(elements.card, model.mode === 'empty' ? 'dormant' : model.mode === 'capacity-only' ? 'unavailable' : 'idle');
    setChipState(
        elements.stateChip,
        model.mode === 'live-runtime' ? 'live' : model.mode === 'chat-derived' ? 'derived' : model.mode === 'capacity-only' ? 'capacity' : 'idle',
        model.mode === 'live-runtime' ? pctState(model.runtimeLivePct) : model.mode === 'chat-derived' ? 'live' : 'idle'
    );
    elements.subtitle.textContent = model.note || '';

    renderGaugeView(model);
    renderFleetView(model);
    applyViewMode(currentView);
}

export function updateContextCard(d, l) {
    initContextCard();
    lastDashboard = d;
    lastLlamaMetrics = l;

    // When capacity is known, backfill lastCtxPct for any tab that doesn't have it.
    // This runs once per capacity value change (e.g., first connect after page load).
    const capacity = l?.context_capacity_tokens || l?.kv_cache_max || 0;
    if (capacity > 0) backfillCtxPct(capacity);

    renderContextCard();
}

// Called by chat-state after tabs load from disk so the card shows immediately.
export function updateContextCardFromChatTabs() {
    initContextCard();
    renderContextCard();
}
