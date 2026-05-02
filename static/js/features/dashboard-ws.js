// ── Dashboard WebSocket Transport ──────────────────────────────────────────────
// WebSocket creation, onmessage dispatch, and dashboard update logic.
// Imports state from app-state.js and render functions from dashboard-render.js.

import { formatMetricAge, formatMetricNumber } from '../core/format.js';
import * as state from '../core/app-state.js';
import {
    setChipState,
    setCardState,
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
    renderRequestStats,
    renderGenerationDetailItems,
    renderDecodingConfig,
    renderCapabilityPopover,
    updateMetricDelta,
    setEmptyState,
    renderGpuCard,
    renderSystemCard,
    setMetricSectionVisibility,
} from './dashboard-render.js';
import { animateNumber } from './animate.js';
import { setRemoteAgentStatus } from './remote-agent.js';
import { activeChatTab } from './chat-state.js';
import { hideConnectingState, switchView } from './setup-view.js';

// Local state — not shared across modules
let lastGpuData = {};
// ── Cached DOM elements (populated at init time to avoid repeated queries) ──
let cachedElements = null;

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
        // Context
        mContextValue: document.getElementById('m-context-value'),
        mContextPct: document.getElementById('m-context-pct'),
        mContextDelta: document.getElementById('m-context-delta'),
        mContextMax: document.getElementById('m-context-max'),
        mContextBar: document.getElementById('m-context-bar'),
        mContextState: document.getElementById('m-context-state'),
        mContextEmpty: document.getElementById('m-context-empty'),
        mContextAge: document.getElementById('m-context-age'),
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
        // Context
        mCtxFill: document.getElementById('m-ctx-fill'),
        mCtxValue: document.getElementById('m-ctx'),
        mCtxDetails: document.getElementById('m-ctx-details'),
        mCtxState: document.getElementById('m-context-state'),
        mCtxPeakFill: document.getElementById('m-ctx-peak-fill'),
        mCtxLiveLabel: document.getElementById('m-ctx-live-label'),
        mCtxLiveDetail: document.getElementById('m-ctx-live-detail'),
        mCtxPeakDetail: document.getElementById('m-ctx-peak-detail'),
        mGenEmpty: document.getElementById('m-generation-empty'),
        mSlotsState: document.getElementById('m-slots-state'),
        mActivityState: document.getElementById('m-activity-state'),
        // Logs
        logPanel: document.getElementById('log-panel'),
    };
}

// ── WebSocket setup ───────────────────────────────────────────────────────────

export function initWebSocket() {
    const ws = new WebSocket(
        (location.protocol === 'https:' ? 'wss://' : 'ws://') + location.host + '/ws'
    );

    ws.onmessage = e => {
        const d = JSON.parse(e.data);
        updateDashboard(d);
    };

    ws.onerror = e => console.error('WebSocket error:', e);

    ws.onclose = () => {
        ensureCachedElements();
        if (cachedElements.statusText) cachedElements.statusText.textContent = 'Disconnected';
        state.sessionState.prevLogLen = 0;
    };

    return ws;
}

// ── Main dashboard update (replaces ws.onmessage in app.js) ──────────────────

function updateDashboard(d) {
    // Ensure DOM elements are cached (avoids repeated queries on every WS message)
    ensureCachedElements();

    // Store for use by status alert and other components
    state.setWsData(d);

    // Endpoint health strip
    updateEndpointStrip(d);

    // Agent status
    updateAgentStatus(d);

    // Attach/Detach buttons and server header
    updateAttachDetach(d);

    // Server state
    updateServerState(d);

    // Inference metrics
    updateInferenceMetrics(d);

    // GPU card
    updateGpuCard(d);

    // System card
    updateSystemCard(d);

    // Logs
    updateLogs(d);

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
        let statusText = 'OK';

        if (d.endpoint_kind === 'Local') {
            modeClass = 'local';
            modeText = 'Local';
            if (!d.capabilities.system || !d.capabilities.gpu) {
                statusClass = 'warning';
                statusText = 'Limited';
            }
        } else if (d.endpoint_kind === 'Remote') {
            modeClass = 'remote';
            modeText = 'Remote';
            if (!d.capabilities.inference) {
                statusClass = 'error';
                statusText = 'Error';
            } else {
                statusClass = 'warning';
                statusText = 'Inference only';
            }
        }

        if (d.capabilities.inference && !d.capabilities.host_metrics) {
            statusClass = 'warning';
            statusText = 'Inference only';
        }

        if (endpointModeEl) {
            endpointModeEl.textContent = modeText;
            endpointModeEl.className = 'endpoint-mode ' + modeClass;
        }
        if (endpointUrlEl) {
            endpointUrlEl.textContent = d.active_session_endpoint || d.active_session_id || 'No session';
        }
        if (endpointStatusEl) {
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

    const agentStatus = d.remote_agent_connected ? 'connected' : 'disconnected';
    const remoteAgentHealthReachable = d.remote_agent_health_reachable !== false;
    const firewallBlocked = d.remote_agent_connected && !remoteAgentHealthReachable;

    agentStatusEl.className = 'agent-status ' + (firewallBlocked ? 'firewall-blocked' : agentStatus);

    const textEl = agentStatusEl.querySelector('.agent-text');
    const fixBtn = agentStatusEl.querySelector('.btn-agent-fix');
    if (textEl) {
        if (firewallBlocked) {
            textEl.textContent = 'Firewall blocked';
        } else if (d.remote_agent_connected) {
            textEl.textContent = 'Remote Agent';
        } else {
            textEl.textContent = 'No Remote Agent';
        }
    }
    if (fixBtn) {
        const hasRemoteEndpoint = d.session_mode === 'attach' && d.endpoint_kind === 'Remote';
        const needsFix = hasRemoteEndpoint && (!d.remote_agent_connected || firewallBlocked);
        fixBtn.style.display = needsFix ? '' : 'none';
        fixBtn.title = firewallBlocked ? 'Repair remote agent connectivity' : 'Set up remote agent';
    }

    if (d.remote_agent_connected && !remoteAgentHealthReachable) {
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

        if (typeof state.view !== 'undefined' && state.view === 'setup') {
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
    state.sessionState.serverRunning = d.server_running;
    const ce = cachedElements;

    const dot = ce.statusDot;
    const txt = ce.statusText;
    const btnStart = ce.btnStart;
    const btnStop = ce.btnStop;

    dot.className = 'status-dot ' + (state.sessionState.serverRunning ? 'running' : 'stopped');
    txt.textContent = state.sessionState.serverRunning ? 'Running' : 'Stopped';

    const localRunning = d.local_server_running || false;
    if (btnStart) btnStart.disabled = localRunning;
    if (btnStop) btnStop.disabled = !localRunning;

    state.lastServerState = d.server_running;
    state.lastLlamaMetrics = d.llama;
    state.lastSystemMetrics = d.system || null;
    state.lastCapabilities = d.capabilities || null;
    state.lastGpuMetrics = d.gpu || {};
}

// ── Inference metrics ────────────────────────────────────────────────────────

function updateInferenceMetrics(d) {
    const l = state.lastLlamaMetrics;
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
        updateMetricDelta(promptDeltaEl, state.prevValues.prompt, promptDisplayRate, 1);
        animateNumber(promptEl, state.prevValues.prompt, promptDisplayRate, 300, 1, ' t/s');
        state.prevValues.prompt = promptDisplayRate;

        if (promptDisplayRate > state.monitorState.speedMax.prompt) {
            state.monitorState.speedMax.prompt = promptDisplayRate;
        }
        if (promptMaxEl && state.monitorState.speedMax.prompt > 0) {
            promptMaxEl.textContent = 'peak ' + state.monitorState.speedMax.prompt.toFixed(0);
        }
        const promptPct = Math.max((promptDisplayRate / state.monitorState.speedMax.prompt) * 100, 4);
        if (promptBar) promptBar.style.width = promptPct + '%';
    } else {
        promptEl.textContent = '\u2014';
        if (promptMaxEl) promptMaxEl.textContent = '';
        if (promptBar) promptBar.style.width = '0%';
    }

    // Generation throughput
    if (genDisplayRate > 0) {
        updateMetricDelta(genDeltaEl, state.prevValues.generation, genDisplayRate, 1);
        animateNumber(genEl, state.prevValues.generation, genDisplayRate, 300, 1, ' t/s');
        state.prevValues.generation = genDisplayRate;

        if (genDisplayRate > state.monitorState.speedMax.generation) {
            state.monitorState.speedMax.generation = genDisplayRate;
        }
        if (genMaxEl && state.monitorState.speedMax.generation > 0) {
            genMaxEl.textContent = 'peak ' + state.monitorState.speedMax.generation.toFixed(0);
        }
        const genPct = Math.max((genDisplayRate / state.monitorState.speedMax.generation) * 100, 4);
        if (genBar) genBar.style.width = genPct + '%';
    } else {
        genEl.textContent = '\u2014';
        if (genMaxEl) genMaxEl.textContent = '';
        if (genBar) genBar.style.width = '0%';
    }

    // Sparklines
    pushSparklinePoint('prompt', promptDisplayRate);
    pushSparklinePoint('generation', genDisplayRate);
    renderSparkline('m-prompt-spark', state.metricSeries.prompt, 'prompt', false);
    renderSparkline('m-gen-spark', state.metricSeries.generation, 'generation', false);

    // Throughput ratio
    const ratioBar = ce.mRatioBar;
    const ratioValue = ce.mRatioValue;
    if (promptDisplayRate > 0 && genDisplayRate > 0) {
        const ratio = promptDisplayRate / genDisplayRate;
        const ratioPct = Math.min((ratio / 50) * 100, 100);
        if (ratioBar) ratioBar.style.width = ratioPct + '%';
        if (ratioValue) ratioValue.textContent = ratio.toFixed(1) + ':1';
    } else {
        if (ratioBar) ratioBar.style.width = '0%';
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
    renderRequestStats();
    renderDecodingConfig(l, hasActiveEndpoint, generationActive);
    renderLiveSparkline('m-live-output-spark', state.metricSeries.liveOutput);

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
    renderCapabilityPopover(d, l, generationAvailable, !!(l?.context_live_tokens_available || l?.kv_cache_tokens_available));

    // Metric section visibility
    const hostMetricsVisible = d.host_metrics_available === true;
    const systemVisible = hostMetricsVisible && !!d.capabilities?.system;
    const gpuVisible = hostMetricsVisible && !!d.capabilities?.gpu;
    setMetricSectionVisibility('gpu-card', gpuVisible, 'gpu-section');
    setMetricSectionVisibility('system-card', systemVisible, 'system-section');
}

// ── Context metrics ──────────────────────────────────────────────────────────

function updateContextMetrics(d, l, hasActiveEndpoint) {
    const ce = cachedElements;
    const ctxFill = ce.mCtxFill;
    const ctxValue = ce.mCtxValue;
    const ctxDetails = ce.mCtxDetails;
    const ctxState = ce.mCtxState;
    const ctxPeakFill = ce.mCtxPeakFill;
    const ctxLiveLabel = ce.mCtxLiveLabel;
    const ctxLiveDetail = ce.mCtxLiveDetail;
    const ctxPeakDetail = ce.mCtxPeakDetail;
    const contextCapacity = l?.context_capacity_tokens || l?.kv_cache_max || 0;
    const contextLive = l?.context_live_tokens || l?.kv_cache_tokens || 0;
    const contextPeak = l?.context_high_water_tokens || l?.kv_cache_high_water || 0;
    const contextLiveAvailable = !!(l?.context_live_tokens_available || l?.kv_cache_tokens_available);
    const peakPct = contextCapacity > 0 && contextPeak > 0 ? Math.min(100, Math.max(2, (contextPeak / contextCapacity) * 100)) : 0;
    const contextCard = ce.contextCard;

    setEmptyState(ce.mContextEmpty, !hasActiveEndpoint);
    if (ctxPeakFill) ctxPeakFill.style.width = peakPct + '%';
    if (ctxPeakDetail) ctxPeakDetail.textContent = contextPeak > 0 ? formatMetricNumber(contextPeak) + ' peak' : '\u2014';

    if (l && contextCapacity > 0 && contextLiveAvailable) {
        const pct = ((contextLive / contextCapacity) * 100);
        const severity = pct >= 95 ? 'critical' : pct >= 80 ? 'warning' : '';
        setCardState(contextCard, severity === 'critical' ? 'live' : 'idle');
        setChipState(ctxState, 'live', severity || 'live');
        if (ctxLiveLabel) ctxLiveLabel.textContent = 'Live usage';
        if (ctxLiveDetail) ctxLiveDetail.textContent = formatMetricNumber(contextLive) + ' live';

        animateNumber(ctxValue, state.prevValues.contextPct, pct, 300, 1, '%');
        state.prevValues.contextPct = pct;

        if (ctxFill) {
            ctxFill.style.width = pct + '%';
            ctxFill.className = 'context-progress-fill ' + severity;
        }

        if (ctxDetails) ctxDetails.textContent = formatMetricNumber(contextLive) + ' / ' + formatMetricNumber(contextCapacity);

    } else if (l && contextCapacity > 0) {
        setCardState(contextCard, 'unavailable');
        setChipState(ctxState, 'capacity', 'idle');
        if (ctxFill) {
            ctxFill.style.width = '0%';
            ctxFill.className = 'context-progress-fill unavailable';
        }
        if (ctxLiveLabel) ctxLiveLabel.textContent = 'Live usage';
        if (ctxLiveDetail) ctxLiveDetail.textContent = 'not exposed by llama-server';
        if (ctxValue) ctxValue.textContent = 'peak observed only';
        if (ctxDetails) {
            const detailParts = ['capacity ' + formatMetricNumber(contextCapacity)];
            if (contextPeak > 0) {
                detailParts.push('peak ' + formatMetricNumber(contextPeak));
            }
            ctxDetails.textContent = detailParts.join(' · ');
        }
    } else {
        setCardState(contextCard, !hasActiveEndpoint ? 'dormant' : 'unavailable');
        setChipState(ctxState, 'unknown', 'idle');
        if (ctxLiveDetail) ctxLiveDetail.textContent = '\u2014';
        if (ctxPeakDetail) ctxPeakDetail.textContent = '\u2014';
        if (ctxFill) {
            ctxFill.style.width = '0%';
            ctxFill.className = 'context-progress-fill';
        }
        if (ctxPeakFill) ctxPeakFill.style.width = '0%';
        if (ctxValue) ctxValue.textContent = '\u2014';
        if (ctxDetails) ctxDetails.textContent = '';
    }
}

// ── GPU card ─────────────────────────────────────────────────────────────────

function updateGpuCard(d) {
    const gpuVisible = d.host_metrics_available === true && !!d.capabilities?.gpu;
    lastGpuData = d.gpu || {};
    state.lastGpuData = lastGpuData;

    renderGpuCard(d.gpu || {}, gpuVisible);
}

// ── System card ──────────────────────────────────────────────────────────────

function updateSystemCard(d) {
    const systemVisible = d.host_metrics_available === true && !!d.capabilities?.system;

    renderSystemCard(state.lastSystemMetrics, systemVisible);
}

// ── Logs ─────────────────────────────────────────────────────────────────────

function updateLogs(d) {
    const logs = d.logs || [];

    if (logs.length !== state.sessionState.prevLogLen) {
        const el = cachedElements.logPanel;
        const wasAtBottom = el && (el.scrollHeight - el.scrollTop - el.clientHeight < 40);

        if (el) {
            el.textContent = logs.join('\n');
            if (wasAtBottom) el.scrollTop = el.scrollHeight;
        }

        state.sessionState.prevLogLen = logs.length;
    }
}

// ── Badges ───────────────────────────────────────────────────────────────────

function updateBadges(d) {
    const isAttached = d.session_mode === 'attach' && d.active_session_endpoint;

    // Server badge
    const badgeParts = [];
    if (isAttached) {
        const gpuEntries = Object.entries(d.gpu || {});
        if (gpuEntries.length > 0) badgeParts.push('GPU ' + Math.max(...gpuEntries.map(([,m]) => m.temp)).toFixed(0) + 'C');
    }
    const ce = cachedElements;
    const badgeServer = ce.badgeServer;
    if (badgeServer) badgeServer.textContent = badgeParts.length ? ' ' + badgeParts.join(' \u00b7 ') : '';

    // Chat badge
    const badgeChat = ce.badgeChat;
    const tab = activeChatTab();
    const msgCount = tab ? tab.messages.filter(m => m.role !== 'system').length : 0;
    if (badgeChat) {
        if (msgCount > 0) {
            badgeChat.textContent = ' ' + msgCount + ' msg';
            badgeChat.style.display = '';
        } else {
            badgeChat.textContent = '';
            badgeChat.style.display = 'none';
        }
    }

    // Logs badge
    const badgeLogs = ce.badgeLogs;
    const logs = d.logs || [];
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
