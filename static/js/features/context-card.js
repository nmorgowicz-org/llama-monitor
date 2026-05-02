// ── Context Window Card ──────────────────────────────────────────────────────
// Hybrid runtime/chat-derived context card with dual gauge/fleet views.

import { chat } from '../core/app-state.js';
import { escapeHtml, formatMetricNumber } from '../core/format.js';
import { setCardState, setChipState, setEmptyState } from './dashboard-render.js';

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
        gaugeTrack: document.getElementById('m-context-gauge-track'),
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

function deriveChatSummaries() {
    const now = Date.now();
    return chat.tabs
        .map(tab => {
            const messages = tab.messages || [];
            const lastTimestamp = messages.reduce((max, msg) => Math.max(max, msg.timestamp_ms || 0), 0)
                || tab.updated_at
                || tab.created_at
                || 0;
            return {
                id: tab.id,
                name: tab.name || 'Untitled chat',
                ctxPct: typeof tab.lastCtxPct === 'number' && tab.lastCtxPct > 0 ? tab.lastCtxPct : null,
                state: pctState(typeof tab.lastCtxPct === 'number' ? tab.lastCtxPct : null),
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
    const chatSummaries = deriveChatSummaries();
    const trackedChats = chatSummaries.filter(chatItem => chatItem.messageCount > 0);
    const nonStaleChats = trackedChats.filter(chatItem => !chatItem.isStale);
    const pressureKnown = trackedChats.filter(chatItem => chatItem.ctxPct != null);
    const activeChatCount = trackedChats.length;
    const pressuredChatCount = pressureKnown.filter(chatItem => (chatItem.ctxPct || 0) >= 75).length;
    const busiestChat = pressureKnown.slice().sort((a, b) => (b.ctxPct || 0) - (a.ctxPct || 0))[0] || trackedChats[0] || null;
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
            ? `${activeChatCount} chats tracked · ${pressuredChatCount} under pressure`
            : `${activeChatCount} chats tracked · live runtime usage unavailable`;
    } else if (mode === 'capacity-only') {
        note = `Context size ${formatMetricNumber(capacityTokens)} · live usage unavailable`;
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
        aggregateChatPressure: {
            avgPct,
            maxPct,
        },
        note,
        hasActiveEndpoint: !!d.active_session_endpoint,
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
            <span class="context-pill-value">${item.ctxPct != null ? Math.round(item.ctxPct) + '%' : '—'}</span>
        `;
        strip.appendChild(pill);
    }

    if (overflow > 0) {
        const more = renderOverflowButton('context-pill context-pill-overflow', overflow);
        strip.appendChild(more);
    }

    stripMeta.textContent = model.staleChatCount > 0
        ? `${model.chatSummaries.length} chats tracked · ${model.staleChatCount} stale`
        : `${model.chatSummaries.length} chats tracked`;
}

function renderGaugeView(model) {
    const { gaugeValue, gaugeSecondary, gaugeRing, gaugeTrack } = ensureElements();
    const heroPct = model.mode === 'live-runtime'
        ? model.runtimeLivePct
        : model.mode === 'chat-derived'
            ? (model.busiestChat?.ctxPct ?? model.aggregateChatPressure.maxPct)
            : null;
    const displayPct = heroPct != null ? Math.max(0, Math.min(100, heroPct)) : 0;
    const ringPct = Math.max(0, Math.min(100, displayPct));
    const dash = 314;
    gaugeTrack.style.strokeDasharray = `${dash} ${dash}`;
    gaugeRing.style.strokeDasharray = `${(ringPct / 100) * dash} ${dash}`;
    gaugeRing.setAttribute('class', `context-gauge-ring ${pctState(heroPct)}`);

    if (model.mode === 'live-runtime') {
        gaugeValue.textContent = `${Math.round(displayPct)}%`;
        gaugeSecondary.textContent = `${formatMetricNumber(model.runtimeLiveTokens)} / ${formatMetricNumber(model.capacityTokens)} live`;
    } else if (model.mode === 'chat-derived') {
        gaugeValue.textContent = heroPct != null ? `${Math.round(displayPct)}%` : '—';
        gaugeSecondary.textContent = model.busiestChat
            ? `${model.busiestChat.name} · ${model.busiestChat.autoCompact ? 'auto-compact on' : 'manual compaction'}`
            : 'Based on tracked chats';
    } else if (model.mode === 'capacity-only') {
        gaugeValue.textContent = formatMetricNumber(model.capacityTokens);
        gaugeSecondary.textContent = 'Capacity only';
    } else {
        gaugeValue.textContent = '—';
        gaugeSecondary.textContent = 'Waiting for chat or runtime context';
    }

    renderChatStrip(model);
}

function renderFleetView(model) {
    const { fleetSummary, fleetRows, fleetFooter } = ensureElements();
    const source = model.nonStaleChats.length ? model.nonStaleChats : model.chatSummaries;
    const rows = expanded ? source : source.slice(0, MAX_VISIBLE_FLEET_ROWS);
    fleetSummary.textContent = model.mode === 'live-runtime'
        ? `${model.activeChatCount} chats · runtime live`
        : model.mode === 'chat-derived'
            ? `${model.activeChatCount} chats · ${model.pressuredChatCount} under pressure`
            : model.mode === 'capacity-only'
                ? `Capacity ${formatMetricNumber(model.capacityTokens)}`
                : 'No active chat context yet';

    if (rows.length === 0) {
        fleetRows.innerHTML = '<div class="context-fleet-empty">Start a chat to build context intelligence.</div>';
        fleetFooter.textContent = model.note || '';
        return;
    }

    fleetRows.innerHTML = rows.map(item => {
        const pct = item.ctxPct != null ? Math.round(item.ctxPct) : null;
        return `
            <div class="context-fleet-row ${item.state}">
                <div class="context-fleet-row-top">
                    <span class="context-fleet-name">${escapeHtml(item.name)}</span>
                    <span class="context-fleet-value">${pct != null ? pct + '%' : 'unknown'}</span>
                </div>
                <div class="context-fleet-bar">
                    <div class="context-fleet-fill ${item.state}" style="width:${pct != null ? Math.min(100, pct) : 8}%"></div>
                </div>
                <div class="context-fleet-meta">${item.autoCompact ? 'auto-compact' : 'manual'}${item.isStale ? ' · stale' : ''}</div>
            </div>
        `;
    }).join('');

    const overflow = Math.max(0, source.length - rows.length);
    if (overflow > 0) {
        const more = renderOverflowButton('context-fleet-more', overflow);
        fleetRows.appendChild(more);
    }

    if (model.mode === 'live-runtime') {
        fleetFooter.textContent = `${formatMetricNumber(model.runtimeLiveTokens)} / ${formatMetricNumber(model.capacityTokens)} live`;
    } else if (model.aggregateChatPressure.avgPct != null) {
        fleetFooter.textContent = `active chat pressure avg ${Math.round(model.aggregateChatPressure.avgPct)}%`;
    } else {
        fleetFooter.textContent = model.note || '';
    }
}

function applyViewMode(viewMode) {
    currentView = viewMode;
    const { gaugeView, fleetView, gaugeToggle, fleetToggle } = ensureElements();
    gaugeView.classList.toggle('active', viewMode === 'gauge');
    gaugeView.classList.toggle('hidden', viewMode !== 'gauge');
    fleetView.classList.toggle('active', viewMode === 'fleet');
    fleetView.classList.toggle('hidden', viewMode !== 'fleet');
    gaugeToggle.classList.toggle('active', viewMode === 'gauge');
    fleetToggle.classList.toggle('active', viewMode === 'fleet');
    gaugeToggle?.setAttribute('aria-selected', String(viewMode === 'gauge'));
    fleetToggle?.setAttribute('aria-selected', String(viewMode === 'fleet'));
}

function saveViewPreference() {
    fetch('/api/settings')
        .then(resp => resp.ok ? resp.json() : null)
        .then(settings => {
            if (!settings) return;
            return fetch('/api/settings', {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    ...settings,
                    context_card_view: currentView,
                }),
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
            updateContextCard(lastDashboard || {}, lastLlamaMetrics || null);
        }
    });
    applyViewMode(currentView);
}

export function setContextCardViewPreference(viewMode) {
    currentView = viewMode === 'fleet' ? 'fleet' : 'gauge';
    if (initialized) {
        applyViewMode(currentView);
    }
}

export function updateContextCard(d, l) {
    initContextCard();
    lastDashboard = d;
    lastLlamaMetrics = l;
    const elements = ensureElements();
    const model = deriveContextViewModel(d, l);
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
