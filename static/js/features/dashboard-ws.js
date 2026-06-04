// ── Dashboard WebSocket Transport ──────────────────────────────────────────────
// WebSocket creation, onmessage dispatch, and dashboard update logic.
// Imports state from app-state.js and render functions from dashboard-render.js.
//
// POWER OPTIMIZATION: Page Visibility API throttling.
// When the tab is hidden, we skip the dashboard update entirely — no need to
// parse JSON and write DOM if the user isn't looking at it. The WebSocket
// still stays connected and receives data, but we discard the messages until
// the tab becomes visible again. This saves ~100+ DOM writes per tick.

import { formatMetricAge, formatMetricNumber } from '../core/format.js';
import { deriveTelemetryGrade, gradeLabel, gradeStatusClass, gradeActionCopy } from '../features/telemetry-grade.js';
import {
    sessionState,
    prevValues,
    metricSeries,
    liveOutputTracker,
    metricCapabilities,
    requestActivity,
    recentTasks,
    slotSnapshots,
    setWsData,
    setLastServerState,
    setLastLlamaMetrics,
    setLastSystemMetrics,
    setLastGpuMetrics,
    setLastCapabilities,
    setLastGpuData,
    lastLlamaMetrics,
    lastSystemMetrics,
    wsData,
    currentPollInterval,
    monitorState,
    setupViewState,
} from '../core/app-state.js';
import {
    setChipState,
    setCardState,
    setEmptyState,
    pushSparklinePoint,
    renderSparkline,
    renderLiveSparkline,
    updateLiveOutputEstimate,
    updateRequestActivity,
    renderRecentTask,
    renderActivityRail,
    renderSlotGrid,
    getPrimarySlot,
    renderSlotUtilization,
    renderBatchEfficiency,
    renderRequestStats,
    renderGenerationDetailItems,
    renderDecodingConfig,
    formatParamCount,
    renderCapabilityPopover,
    updateMetricDelta,
    setMetricSectionVisibility,
    renderGpuCard,
    renderSystemCard,
} from './dashboard-render.js';
import { animateNumber } from './animate.js';
import { refreshChatTelemetry } from './chat-params.js';
import { updateContextCard, updateContextCardFromChatTabs } from './context-card.js';
import { refreshTopCockpit } from './nav.js';
import { activeChatTab } from './chat-state.js';
import { setRemoteAgentStatus } from './remote-agent.js';
import { hideConnectingState, switchView } from './setup-view.js';

// ── Cached DOM elements (populated at init time to avoid repeated queries) ──
let cachedElements = null;
let dashboardSocket = null;
let overlayStateObserver = null;

// ── Badge change detection — skip DOM writes when badge content is unchanged ──
let prevBadgeState = { server: null, chat: null, logs: null };

// ── Power optimization: Page Visibility API throttling ─────────────────────────
// When the tab is hidden, skip all dashboard updates (no DOM writes needed) and
// pause all infinite CSS animations (70+) via the power-saver body class.
// The GPU poller interval (in main.rs) automatically tracks the configured
// ws_push_interval_ms setting, so when the user selects "Battery Saver" the
// GPU hardware reads slow down proportionally.
// When the tab becomes visible, do one full update to refresh stale state.
let isTabVisible = true;

function isElementActuallyVisible(el) {
    if (!el) return false;
    const style = getComputedStyle(el);
    return style.display !== 'none' && style.visibility !== 'hidden';
}

function hasBlockingOverlayOpen() {
    const candidates = [
        ...document.querySelectorAll(
            '.modal-overlay.open, .modal-overlay.active, .modal-overlay[style*="display: block"], .modal-overlay[style*="display:block"], .keyboard-shortcut-overlay.open, .keyboard-shortcut-overlay.active, #release-notes-panel.open'
        ),
        document.getElementById('command-palette-overlay'),
    ].filter(isElementActuallyVisible);

    return candidates.length > 0;
}

function isBackgroundUiSuspended() {
    return !isTabVisible || hasBlockingOverlayOpen();
}

function syncBackgroundPowerState() {
    document.body.classList.toggle('power-saver', !isTabVisible);
    document.body.classList.toggle('background-paused', hasBlockingOverlayOpen());
}

function ensureOverlayStateObserver() {
    if (overlayStateObserver || !document.body) return;
    let scheduled = false;
    const scheduleSync = () => {
        if (scheduled) return;
        scheduled = true;
        requestAnimationFrame(() => {
            scheduled = false;
            syncBackgroundPowerState();
        });
    };

    overlayStateObserver = new MutationObserver(scheduleSync);
    overlayStateObserver.observe(document.body, {
        subtree: true,
        attributes: true,
        attributeFilter: ['class', 'style', 'aria-hidden', 'inert'],
    });
    scheduleSync();
}

function sendWsClientState() {
    if (!dashboardSocket || dashboardSocket.readyState !== WebSocket.OPEN) return;
    try {
        dashboardSocket.send(JSON.stringify({
            type: 'client-visibility',
            visible: isTabVisible,
        }));
    } catch {
        // Ignore transient send failures; the next state change will retry.
    }
}

function currentMonitorTab() {
    const activePage = document.querySelector('.page.active');
    if (!activePage || !activePage.id) return 'server';
    return activePage.id.replace(/^page-/, '');
}

function isMonitorViewActive() {
    return setupViewState.view === 'monitor';
}

document.addEventListener('visibilitychange', () => {
    const wasVisible = isTabVisible;
    isTabVisible = !document.hidden;

    // Toggle background power classes to pause/resume animation work
    syncBackgroundPowerState();
    sendWsClientState();

    if (!wasVisible && isTabVisible) {
        // Tab just became visible — schedule a refresh so stale data gets updated
        requestAnimationFrame(() => {
            if (wsData) {
                updateDashboard(wsData);
            }
        });
    }
});

function ensureCachedElements() {
    if (cachedElements) return;
    cachedElements = {
        // Inference metrics
        mPrompt: document.getElementById('m-prompt'),
        mGen: document.getElementById('m-gen'),
        mPromptDelta: document.getElementById('m-prompt-delta'),
        mGenDelta: document.getElementById('m-gen-delta'),
        mPromptMax: document.getElementById('m-prompt-max'),
        mGenMax: document.getElementById('m-gen-max'),
        mPromptBar: document.getElementById('m-prompt-bar'),
        mGenBar: document.getElementById('m-gen-bar'),
        mThroughputState: document.getElementById('m-throughput-state'),
        mThroughputAge: document.getElementById('m-throughput-age'),
        mThroughputEmpty: document.getElementById('m-throughput-empty'),
        mGenState: document.getElementById('m-generation-state'),
        mGenMain: document.getElementById('m-generation-main'),
        mGenSub: document.getElementById('m-generation-sub'),
        mGenDetails: document.getElementById('m-generation-details'),
        mGenRing: document.getElementById('m-generation-ring'),
        mLiveVelocity: document.getElementById('m-live-velocity'),
        mStagePrompt: document.getElementById('m-stage-prompt'),
        mStageOutput: document.getElementById('m-stage-output'),
        mRatioBar: document.getElementById('m-throughput-ratio-bar'),
        mRatioValue: document.getElementById('m-throughput-ratio'),
        // Cards
        throughputCard: document.querySelector('.widget-speed'),
        generationCard: document.querySelector('.widget-generation'),
        contextCard: document.querySelector('.widget-context'),
        mContextState: document.getElementById('m-context-state'),
        mContextEmpty: document.getElementById('m-context-empty'),
        // Badges
        badgeServer: document.getElementById('badge-server'),
        badgeChat: document.getElementById('badge-chat'),
        badgeLogs: document.getElementById('badge-logs'),
        // Endpoint
        endpointMode: document.getElementById('endpoint-mode'),
        endpointUrl: document.getElementById('endpoint-url'),
        endpointStatus: document.getElementById('endpoint-status'),
        // Agent
        agentStatus: document.getElementById('agent-status'),
        agentLatency: document.getElementById('agent-latency'),
        // Status
        statusText: document.getElementById('status-text'),
        // Server state
        serverHeader: document.getElementById('server-header'),
        btnAttach: document.getElementById('btn-attach'),
        btnDetach: document.getElementById('btn-detach'),
        btnDetachTop: document.getElementById('btn-detach-top'),
        historicBadge: document.getElementById('inference-historic-badge'),
        statusDot: document.getElementById('status-dot'),
        btnStart: document.getElementById('btn-start'),
        btnStop: document.getElementById('btn-stop'),
        mGenEmpty: document.getElementById('m-generation-empty'),
        mSlotsState: document.getElementById('m-slots-state'),
        mActivityState: document.getElementById('m-activity-state'),
        // Logs
        logPanel: document.getElementById('log-panel'),
    };
}

// ── WebSocket setup ───────────────────────────────────────────────────────────

export function initWebSocket() {
    ensureOverlayStateObserver();
    const ws = new WebSocket(
        (location.protocol === 'https:' ? 'wss://' : 'ws://') + location.host + '/ws'
    );
    dashboardSocket = ws;

ws.onmessage = e => {
    const d = JSON.parse(e.data);
    // Keep wsData current even when tab is hidden (needed for refresh on show)
    setWsData(d);
    if (!isTabVisible) return; // skip DOM writes while tab is hidden
    updateDashboard(d);
};

    ws.onopen = () => {
        sendWsClientState();
    };

// Poll context card after chat tabs load
window.onChatTabsLoaded = () => {
    setTimeout(() => {
        updateContextCardFromChatTabs();
    }, 500);
};

    ws.onerror = e => console.error('WebSocket error:', e);

    ws.onclose = () => {
        if (dashboardSocket === ws) dashboardSocket = null;
        ensureCachedElements();
        if (cachedElements.statusText) cachedElements.statusText.textContent = 'Disconnected';
        sessionState.prevLogLen = 0;
    };

    return ws;
}

// ── Main dashboard update (replaces ws.onmessage in app.js) ──────────────────

function updateDashboard(d) {
    // Ensure DOM elements are cached (avoids repeated queries on every WS message)
    ensureCachedElements();

    // Store for use by status alert and other components
    setWsData(d);

    // Derive and store telemetry grade for consumption by all dashboard components
    const grade = deriveTelemetryGrade(d);
    window.__telemetryGrade = grade;
    const inMonitorView = isMonitorViewActive();
    const activeTab = currentMonitorTab();

    // Attach/Detach buttons and server header
    updateAttachDetach(d);

    // Keep session/runtime state current even when the setup flow is on-screen.
    // The setup path and blocking modal flows still need state sync, but they
    // do not need the hidden background surfaces to keep re-rendering 2x/sec.
    updateServerState(d);

    if (!inMonitorView || hasBlockingOverlayOpen()) {
        return;
    }

    // Endpoint health strip
    updateEndpointStrip(d);

    // Agent status
    updateAgentStatus(d);

    // Inference metrics
    updateInferenceMetrics(d);
    if (activeTab === 'chat') {
        refreshChatTelemetry();
    }
    refreshTopCockpit();

    // GPU card
    if (activeTab === 'server') updateGpuCard(d);

    // System card
    if (activeTab === 'server') updateSystemCard(d);

    // Logs
    if (activeTab === 'logs') updateLogs(d);

    // Badges
    updateBadges(d);
}

// ── Endpoint health strip ────────────────────────────────────────────────────

function updateEndpointStrip(d) {
    const ce = cachedElements;
    const endpointModeEl = ce.endpointMode;
    const endpointUrlEl = ce.endpointUrl;
    const endpointStatusEl = ce.endpointStatus;

    if (d.capabilities && d.endpoint_kind) {
        let modeClass = 'unknown';
        let modeText = 'Unknown';
        let statusClass = 'ok';
        let statusText = 'Full telemetry';

        if (d.endpoint_kind === 'Local') {
            modeClass = 'local';
            modeText = 'Local';
        } else if (d.endpoint_kind === 'Remote') {
            modeClass = 'remote';
            modeText = 'Remote';
        }

        const grade = window.__telemetryGrade || 'local_full';
        statusClass = gradeStatusClass(grade);
        statusText = gradeLabel(grade);

        if (endpointModeEl) {
            endpointModeEl.textContent = modeText;
            endpointModeEl.className = 'endpoint-mode ' + modeClass;
        }
        if (endpointUrlEl) {
            endpointUrlEl.textContent = d.active_session_endpoint || d.active_session_id || 'No session';
        }
        if (endpointStatusEl) {
            // eslint-disable-next-line no-unsanitized/property -- statusClass and statusText are hardcoded string enums set in this function
            endpointStatusEl.innerHTML = '<span class="status-dot ' + statusClass + '"></span>' + statusText;
        }
    }
}

// ── Agent status ─────────────────────────────────────────────────────────────

function updateAgentStatus(d) {
    const ce = cachedElements;
    const agentStatusEl = ce.agentStatus;
    const agentLatencyEl = ce.agentLatency;

    if (!agentStatusEl) return;

    const showAgent = d.session_mode === 'attach' && d.endpoint_kind === 'Remote';
    agentStatusEl.style.display = showAgent ? '' : 'none';

    const grade = window.__telemetryGrade || 'local_full';

    // Badge class
    let badgeClass = 'agent-status';
    if (grade === 'remote_agent_connected') badgeClass += ' connected';
    else if (grade === 'remote_agent_connecting') badgeClass += ' update-available';
    else if (grade === 'remote_agent_update_available') badgeClass += ' update-available';
    else if (grade === 'remote_agent_firewall_blocked') badgeClass += ' firewall-blocked';
    else if (grade === 'remote_error') badgeClass += ' disconnected';
    else badgeClass += ' disconnected';

    // Text
    let agentText = 'Remote Agent';
    if (grade === 'remote_agent_connecting') agentText = 'Connecting...';
    else if (grade === 'remote_agent_update_available') agentText = 'Update Available';
    else if (grade === 'remote_agent_firewall_blocked') agentText = 'Firewall blocked';
    else if (grade === 'remote_agent_degraded') agentText = 'Degraded';
    else if (grade === 'remote_partial_sensors') agentText = 'Partial sensors';
    else if (grade === 'remote_error') agentText = 'Agent error';
    else if (grade === 'remote_inference_only') agentText = 'No Remote Agent';

    // Tooltip
    let tooltipText = 'Connected';
    let tooltipClass = 'connected';
    if (grade === 'remote_agent_connecting') { tooltipText = 'Connecting'; tooltipClass = 'warning'; }
    else if (grade === 'remote_agent_update_available') { tooltipText = 'Update available'; tooltipClass = 'warning'; }
    else if (grade === 'remote_agent_firewall_blocked') { tooltipText = 'Firewall blocked'; tooltipClass = 'warning'; }
    else if (grade === 'remote_agent_degraded') { tooltipText = 'Degraded compatibility'; tooltipClass = 'warning'; }
    else if (grade === 'remote_partial_sensors') { tooltipText = 'Partial sensor coverage'; tooltipClass = 'warning'; }
    else if (grade === 'remote_inference_only') { tooltipText = 'Inference only'; tooltipClass = 'disconnected'; }
    else if (grade === 'remote_error') { tooltipText = 'Connection failed'; tooltipClass = 'disconnected'; }

    // Fix button — show for any non-fully-connected state
    const needsFix = !['remote_agent_connected', 'local_full'].includes(grade);
    // Fix button copy
    let fixCopy = '\u26a1 Fix';
    let fixTitle = 'Set up remote agent';
    if (grade === 'remote_agent_connecting') { fixCopy = '\u23F3 Wait'; fixTitle = 'Remote agent activity is already in progress'; }
    else if (grade === 'remote_agent_update_available') { fixCopy = '\u26a1 Upgrade'; fixTitle = 'Upgrade remote agent to latest version'; }
    else if (grade === 'remote_agent_firewall_blocked') { fixTitle = 'Repair remote agent connectivity'; }
    else if (grade === 'remote_agent_degraded') { fixCopy = '\u26a1 Upgrade'; fixTitle = 'Upgrade agent for full compatibility'; }

    // Tooltip details — include action copy
    let details = '';
    if (d.remote_agent_version) details = 'Running v' + d.remote_agent_version;
    if (d.remote_agent_url) details += (details ? ' | ' : '') + d.remote_agent_url;
    const actionCopy = gradeActionCopy(grade);
    if (actionCopy) details += (details ? '\n' : '') + actionCopy;

    agentStatusEl.className = badgeClass;

    const textEl = agentStatusEl.querySelector('.agent-text');
    const fixBtn = agentStatusEl.querySelector('.btn-agent-fix');
    if (textEl) {
        textEl.textContent = agentText;
    }
    if (fixBtn) {
        fixBtn.style.display = needsFix ? '' : 'none';
        fixBtn.textContent = fixCopy;
        fixBtn.title = fixTitle;
    }

    // Update tooltip
    const tooltipStatus = document.getElementById('agent-tooltip-status');
    const tooltipDetails = document.getElementById('agent-tooltip-details');
    if (tooltipStatus) {
        tooltipStatus.textContent = tooltipText;
        tooltipStatus.className = 'agent-tooltip-status ' + tooltipClass;
    }
    if (tooltipDetails) {
        tooltipDetails.textContent = details;
    }

    // Telemetry grade chip
    const gradeChip = document.getElementById('telemetry-grade-chip');
    if (gradeChip) {
        const chipGrade = window.__telemetryGrade || 'local_full';
        gradeChip.textContent = gradeLabel(chipGrade);
        gradeChip.className = 'telemetry-grade-chip grade-' + gradeStatusClass(chipGrade);
        // Hide chip when agent is fully healthy — the endpoint-status pill already
        // shows "Full telemetry" and the badge label alone is sufficient.
        gradeChip.style.display = (d.endpoint_kind === 'Remote' && chipGrade !== 'remote_agent_connected') ? '' : 'none';
    }

    if (grade === 'remote_agent_firewall_blocked') {
        setRemoteAgentStatus('Agent connected but HTTP is not reachable (firewall blocked)', 'warning');
    }

    if (agentLatencyEl) {
        agentLatencyEl.textContent = '';
    }
}

// ── Attach/Detach buttons ────────────────────────────────────────────────────

function updateAttachDetach(d) {
    const ce = cachedElements;
    const serverHeader = ce.serverHeader;
    const btnAttach = ce.btnAttach;
    const btnDetach = ce.btnDetach;
    const btnDetachTop = ce.btnDetachTop;
    const historicBadge = ce.historicBadge;

    const isAttach = d.session_mode === 'attach' && d.active_session_endpoint;

    if (isAttach) {
        if (serverHeader) serverHeader.style.display = 'none';
        btnAttach.style.display = 'none';
        btnDetach.style.display = 'inline-block';
        if (btnDetachTop) btnDetachTop.style.display = 'inline-block';

        if (typeof setupViewState !== 'undefined' && setupViewState.view === 'setup') {
            // TODO: import from setup-view.js when that module is extracted
            hideConnectingState();
            switchView('monitor');
        }
    } else {
        if (serverHeader) serverHeader.style.display = '';
        btnAttach.style.display = 'inline-block';
        btnDetach.style.display = 'none';
        if (btnDetachTop) btnDetachTop.style.display = 'none';
    }

    if (historicBadge) {
        historicBadge.style.display = isAttach ? 'none' : 'inline-block';
    }
}

// ── Server state ─────────────────────────────────────────────────────────────

function updateServerState(d) {
    sessionState.serverRunning = d.server_running;
    const ce = cachedElements;

    const dot = ce.statusDot;
    const txt = ce.statusText;
    const btnStart = ce.btnStart;
    const btnStop = ce.btnStop;

    dot.className = 'status-dot ' + (sessionState.serverRunning ? 'running' : 'stopped');
    txt.textContent = sessionState.serverRunning ? 'Running' : 'Stopped';

    const localRunning = d.local_server_running || false;
    if (btnStart) btnStart.disabled = localRunning;
    if (btnStop) btnStop.disabled = !localRunning;

    const btnSwitchModel = document.getElementById('btn-switch-model');
    if (btnSwitchModel) btnSwitchModel.style.display = localRunning ? '' : 'none';

    setLastServerState(d.server_running);
    setLastLlamaMetrics(d.llama);
    setLastSystemMetrics(d.system || null);
    setLastCapabilities(d.capabilities || null);
    setLastGpuMetrics(d.gpu || {});

    // Sync session port/endpoint from WebSocket data (replaces removed HTTP poll)
    const ep = d.active_session_endpoint;
    if (ep) {
        try {
            const url = new URL(ep);
            const port = parseInt(url.port) || 8080;
            if (sessionState.activeSessionPort !== port) {
                sessionState.activeSessionPort = port;
            }
            // Keep the endpoint input in sync
            const endpointInput = document.getElementById('server-endpoint');
            if (endpointInput && endpointInput.value !== ep) {
                endpointInput.value = ep;
            }
        } catch {
            // ignore parse errors
        }
    }
}

// ── Inference metrics ────────────────────────────────────────────────────────

function updateInferenceMetrics(d) {
    const l = lastLlamaMetrics;
    const hasActiveEndpoint = !!d.active_session_id;
    const ce = cachedElements;

    const promptEl = ce.mPrompt;
    const genEl = ce.mGen;
    const promptMaxEl = ce.mPromptMax;
    const genMaxEl = ce.mGenMax;
    const promptBar = ce.mPromptBar;
    const genBar = ce.mGenBar;
    const throughputState = ce.mThroughputState;
    const throughputAge = ce.mThroughputAge;
    const throughputCard = ce.throughputCard;
    const generationCard = ce.generationCard;
    const contextCard = ce.contextCard;
    const promptDeltaEl = ce.mPromptDelta;
    const genDeltaEl = ce.mGenDelta;

    const promptRate = l?.prompt_tokens_per_sec || 0;
    const genRate = l?.generation_tokens_per_sec || 0;
    const promptDisplayRate = promptRate > 0 ? promptRate : (l?.last_prompt_tokens_per_sec || 0);
    const genDisplayRate = genRate > 0 ? genRate : (l?.last_generation_tokens_per_sec || 0);
    const promptAgeMs = l?.last_prompt_throughput_unix_ms || 0;
    const genAgeMs = l?.last_generation_throughput_unix_ms || 0;
    const latestThroughputMs = Math.max(promptAgeMs, genAgeMs);
    const throughputActive = promptRate > 0 || genRate > 0;

    setCardState(throughputCard, !hasActiveEndpoint ? 'dormant' : throughputActive ? 'live' : 'idle');
    setEmptyState(ce.mThroughputEmpty, !hasActiveEndpoint);
    setChipState(throughputState, throughputActive ? 'live' : 'idle', throughputActive ? 'live' : 'idle');

    if (throughputAge) {
        throughputAge.textContent = formatMetricAge(latestThroughputMs);
    }

    // Prompt throughput
    if (promptDisplayRate > 0) {
        updateMetricDelta(promptDeltaEl, prevValues.prompt, promptDisplayRate, 1);
        animateNumber(promptEl, prevValues.prompt, promptDisplayRate, 300, 1, ' t/s');
        prevValues.prompt = promptDisplayRate;

        if (promptDisplayRate > monitorState.speedMax.prompt) {
            monitorState.speedMax.prompt = promptDisplayRate;
        }
        if (promptMaxEl && monitorState.speedMax.prompt > 0) {
            promptMaxEl.textContent = 'peak ' + monitorState.speedMax.prompt.toFixed(0);
        }
        const promptPct = Math.max((promptDisplayRate / monitorState.speedMax.prompt) * 100, 4);
        if (promptBar) promptBar.style.transform = 'scaleX(' + (promptPct / 100) + ')';
    } else {
        promptEl.textContent = '\u2014';
        if (promptMaxEl) promptMaxEl.textContent = '';
        if (promptBar) promptBar.style.transform = 'scaleX(0)';
    }

    // Generation throughput
    if (genDisplayRate > 0) {
        updateMetricDelta(genDeltaEl, prevValues.generation, genDisplayRate, 1);
        animateNumber(genEl, prevValues.generation, genDisplayRate, 300, 1, ' t/s');
        prevValues.generation = genDisplayRate;

        if (genDisplayRate > monitorState.speedMax.generation) {
            monitorState.speedMax.generation = genDisplayRate;
        }
        if (genMaxEl && monitorState.speedMax.generation > 0) {
            genMaxEl.textContent = 'peak ' + monitorState.speedMax.generation.toFixed(0);
        }
        const genPct = Math.max((genDisplayRate / monitorState.speedMax.generation) * 100, 4);
        if (genBar) genBar.style.transform = 'scaleX(' + (genPct / 100) + ')';
    } else {
        genEl.textContent = '\u2014';
        if (genMaxEl) genMaxEl.textContent = '';
        if (genBar) genBar.style.transform = 'scaleX(0)';
    }

    // Sparklines
    pushSparklinePoint('prompt', promptDisplayRate);
    pushSparklinePoint('generation', genDisplayRate);
    renderSparkline('m-prompt-spark', metricSeries.prompt, 'prompt', false);
    renderSparkline('m-gen-spark', metricSeries.generation, 'generation', false);

    // Throughput ratio
    const ratioBar = ce.mRatioBar;
    const ratioValue = ce.mRatioValue;
    if (promptDisplayRate > 0 && genDisplayRate > 0) {
        const ratio = promptDisplayRate / genDisplayRate;
        const ratioPct = Math.min((ratio / 50) * 100, 100);
        if (ratioBar) ratioBar.style.transform = 'scaleX(' + (ratioPct / 100) + ')';
        if (ratioValue) ratioValue.textContent = ratio.toFixed(1) + ':1';
    } else {
        if (ratioBar) ratioBar.style.transform = 'scaleX(0)';
        if (ratioValue) ratioValue.textContent = '\u2014';
    }

    // Generation progress
    const generationState = ce.mGenState;
    const generationMain = ce.mGenMain;
    const generationSub = ce.mGenSub;
    const generationDetails = ce.mGenDetails;
    const generationRing = ce.mGenRing;
    const liveVelocity = ce.mLiveVelocity;
    const promptStage = ce.mStagePrompt;
    const outputStage = ce.mStageOutput;
    const generated = l?.slot_generation_tokens || 0;
    const remaining = l?.slot_generation_remaining || 0;
    const generationAvailable = !!l?.slot_generation_available;
    const generationActive = !!l?.slot_generation_active || (l?.slots_processing || 0) > 0;
    const slotLimit = getPrimarySlot(l)?.output_limit || 0;
    const generationTotal = l?.slot_generation_limit || slotLimit || (generated + remaining);
    const generationPct = generationTotal > 0 ? Math.min(100, Math.max(2, (generated / generationTotal) * 100)) : 0;
    const taskId = generationActive ? l?.active_task_id : l?.last_task_id;
    const nowMs = Date.now();
    const liveOutputRate = updateLiveOutputEstimate(taskId, generated, generationActive, nowMs);

    updateRequestActivity(taskId, generationActive, generated, nowMs);
    renderActivityRail(generationActive);
    renderRecentTask();
    renderSlotGrid(l, hasActiveEndpoint);
    renderSlotUtilization(l);
    renderBatchEfficiency(l);
    renderRequestStats();
    renderDecodingConfig(l, hasActiveEndpoint, generationActive);
    renderLiveSparkline('m-live-output-spark', metricSeries.liveOutput);

    setCardState(generationCard, !hasActiveEndpoint ? 'dormant' : generationActive ? 'live' : generationAvailable ? 'idle' : 'unavailable');
    setEmptyState(ce.mGenEmpty, !hasActiveEndpoint);
    setChipState(generationState, generationActive ? 'generating' : 'idle', generationActive ? 'live' : 'idle');
    setChipState(ce.mSlotsState, generationActive ? 'active' : 'idle', generationActive ? 'live' : 'idle');
    setChipState(ce.mActivityState, generationActive ? 'active' : 'idle', generationActive ? 'live' : 'idle');
    if (generationRing) generationRing.style.setProperty('--progress', generationPct.toFixed(2));
    if (liveVelocity) {
        liveVelocity.textContent = liveOutputRate > 0 ? liveOutputRate.toFixed(1) + ' t/s' : (generationActive ? 'warming' : 'retained');
    }
    if (promptStage && outputStage) {
        const useThroughputFallback = !generationAvailable;
        const isPromptPhase = useThroughputFallback
            ? !!(l?.prompt_throughput_active && !l?.generation_throughput_active)
            : (generated <= 1);
        const isOutputPhase = useThroughputFallback
            ? !!(l?.generation_throughput_active)
            : (generated > 1);
        promptStage.classList.toggle('active', generationActive && isPromptPhase);
        outputStage.classList.toggle('active', generationActive && isOutputPhase);
        promptStage.classList.toggle('idle', !generationActive && !isOutputPhase);
        outputStage.classList.toggle('idle', !generationActive && !isPromptPhase);
    }
    if (generationAvailable) {
        if (generationMain) generationMain.textContent = formatMetricNumber(generated) + ' output tokens';
        if (generationSub) generationSub.textContent = formatMetricNumber(remaining) + ' remaining';
        if (generationDetails) {
            const detailParts = [];
            if (taskId !== null && taskId !== undefined) detailParts.push('task ' + taskId);
            if (generationTotal > 0) {
                const maxStr = generationTotal >= 1000 ? Math.round(generationTotal / 1000) + 'k' : formatMetricNumber(generationTotal);
                detailParts.push('max ' + maxStr);
            }
            detailParts.push(formatMetricNumber(remaining) + ' left');
            renderGenerationDetailItems(generationDetails, detailParts);
        }
    } else {
        if (generationMain) generationMain.textContent = generationActive ? 'working' : '\u2014';
        if (generationSub) generationSub.textContent = 'output budget';
        renderGenerationDetailItems(generationDetails, []);
    }

    // Context metrics
    updateContextMetrics(d, l, hasActiveEndpoint);

    // Capability popover
    renderCapabilityPopover(d, l, generationAvailable, !!(l?.context_live_tokens_available || l?.kv_cache_tokens_available || (l?.context_capacity_tokens || 0) > 0));

    // Metric section visibility
    const hostMetricsVisible = d.host_metrics_available === true;
    const systemVisible = hostMetricsVisible && !!d.capabilities?.system;
    const gpuVisible = hostMetricsVisible && !!d.capabilities?.gpu;
    setMetricSectionVisibility('gpu-card', gpuVisible, 'gpu-section');
    setMetricSectionVisibility('system-card', systemVisible, 'system-section');
}

// ── Context metrics ──────────────────────────────────────────────────────────

function updateContextMetrics(d, l, hasActiveEndpoint) {
    updateContextCard(d, l, hasActiveEndpoint);
}

// ── GPU card ─────────────────────────────────────────────────────────────────

function updateGpuCard(d) {
    const gpuVisible = d.host_metrics_available === true && !!d.capabilities?.gpu;
    setLastGpuData(d.gpu || {});

    renderGpuCard(d.gpu || {}, gpuVisible, window.__telemetryGrade);
}

// ── System card ──────────────────────────────────────────────────────────────

function updateSystemCard(d) {
    const systemVisible = d.host_metrics_available === true && !!d.capabilities?.system;

    renderSystemCard(lastSystemMetrics, systemVisible, window.__telemetryGrade);
}

// ── Logs ─────────────────────────────────────────────────────────────────────

function updateLogs(d) {
    const logs = d.logs || [];
    const isAttached = d.session_mode === 'attach' && d.active_session_endpoint;
    const emptyState = document.getElementById('logs-empty-state');
    const logsPage = document.getElementById('page-logs');

    if (logs.length !== sessionState.prevLogLen) {
        const el = cachedElements.logPanel;
        const wasAtBottom = el && (el.scrollHeight - el.scrollTop - el.clientHeight < 40);

        if (el) {
            el.textContent = logs.join('\n');
            if (wasAtBottom) el.scrollTop = el.scrollHeight;
        }

        sessionState.prevLogLen = logs.length;
    }

    // Show empty state whenever there are no logs (attach or spawn mode).
    if (emptyState) {
        if (logs.length === 0) {
            emptyState.classList.add('visible');
            logsPage?.classList.add('logs-empty-mode');
        } else {
            emptyState.classList.remove('visible');
            logsPage?.classList.remove('logs-empty-mode');
        }
    }
}

// ── Badges ───────────────────────────────────────────────────────────────────

function updateBadges(d) {
    const isAttached = d.session_mode === 'attach' && d.active_session_endpoint;
    const ce = cachedElements;

    // Server badge \u2014 GPU temp changes infrequently; skip write when unchanged
    const gpuEntries = Object.entries(d.gpu || {});
    const serverText = (isAttached && gpuEntries.length > 0)
        ? ' GPU ' + Math.max(...gpuEntries.map(([,m]) => m.temp)).toFixed(0) + 'C'
        : '';
    if (serverText !== prevBadgeState.server) {
        prevBadgeState.server = serverText;
        const badgeServer = ce.badgeServer;
        if (badgeServer) badgeServer.textContent = serverText;
    }

    // Chat badge \u2014 message count rarely changes between ticks
    const tab = activeChatTab();
    const msgCount = tab ? tab.messages.filter(m => m.role !== 'system').length : 0;
    if (msgCount !== prevBadgeState.chat) {
        prevBadgeState.chat = msgCount;
        const badgeChat = ce.badgeChat;
        if (badgeChat) {
            if (msgCount > 0) {
                badgeChat.textContent = ' ' + msgCount + ' msg';
                badgeChat.style.display = '';
            } else {
                badgeChat.textContent = '';
                badgeChat.style.display = 'none';
            }
        }
    }

    // Logs badge \u2014 count only changes when new logs arrive
    const logs = d.logs || [];
    if (logs.length !== prevBadgeState.logs) {
        prevBadgeState.logs = logs.length;
        const badgeLogs = ce.badgeLogs;
        if (badgeLogs) {
            if (logs.length > 0) {
                badgeLogs.textContent = ' ' + logs.length;
                badgeLogs.style.display = '';
            } else {
                badgeLogs.textContent = '';
                badgeLogs.style.display = 'none';
            }
        }
    }
}
