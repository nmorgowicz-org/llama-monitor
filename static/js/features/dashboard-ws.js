// ── Dashboard WebSocket Transport ──────────────────────────────────────────────
// WebSocket creation, onmessage dispatch, and dashboard update logic.
// Rendering functions (renderGpuCard, renderSlotGrid, etc.) still live in app.js
// and are called via window.* during this transition phase.

import { formatMetricAge } from '../core/format.js';

let lastGpuData = {};

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
        const statusText = document.getElementById('status-text');
        if (statusText) statusText.textContent = 'Disconnected';
        window.prevLogLen = 0;
    };

    return ws;
}

// ── Main dashboard update (replaces ws.onmessage in app.js) ──────────────────

function updateDashboard(d) {
    // Store for use by status alert and other components
    if (typeof window.appState !== 'undefined') {
        window.appState.wsData = d;
    }

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
    const endpointModeEl = document.getElementById('endpoint-mode');
    const endpointUrlEl = document.getElementById('endpoint-url');
    const endpointStatusEl = document.getElementById('endpoint-status');

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
    const agentStatusEl = document.getElementById('agent-status');
    const agentLatencyEl = document.getElementById('agent-latency');

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
        if (typeof window.setRemoteAgentStatus === 'function') {
            window.setRemoteAgentStatus('Agent connected but HTTP is not reachable (firewall blocked)', 'warning');
        }
    }

    if (agentLatencyEl) {
        agentLatencyEl.textContent = '';
    }
}

// ── Attach/Detach buttons ────────────────────────────────────────────────────

function updateAttachDetach(d) {
    const serverHeader = document.getElementById('server-header');
    const btnAttach = document.getElementById('btn-attach');
    const btnDetach = document.getElementById('btn-detach');
    const btnDetachTop = document.getElementById('btn-detach-top');
    const historicBadge = document.getElementById('inference-historic-badge');

    const isAttach = d.session_mode === 'attach' && d.active_session_endpoint;

    if (isAttach) {
        if (serverHeader) serverHeader.style.display = 'none';
        btnAttach.style.display = 'none';
        btnDetach.style.display = 'inline-block';
        if (btnDetachTop) btnDetachTop.style.display = 'inline-block';

        if (typeof window.appState !== 'undefined' && window.appState.view === 'setup') {
            if (typeof window.hideConnectingState === 'function') window.hideConnectingState();
            if (typeof window.switchView === 'function') window.switchView('monitor');
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
    window.serverRunning = d.server_running;

    const dot = document.getElementById('status-dot');
    const txt = document.getElementById('status-text');
    const btnStart = document.getElementById('btn-start');
    const btnStop = document.getElementById('btn-stop');

    dot.className = 'status-dot ' + (window.serverRunning ? 'running' : 'stopped');
    txt.textContent = window.serverRunning ? 'Running' : 'Stopped';

    const localRunning = d.local_server_running || false;
    if (btnStart) btnStart.disabled = localRunning;
    if (btnStop) btnStop.disabled = !localRunning;

    window.lastServerState = d.server_running;
    window.lastLlamaMetrics = d.llama;
    window.lastSystemMetrics = d.system || null;
    window.lastCapabilities = d.capabilities || null;
    window.lastGpuMetrics = d.gpu || {};
}

// ── Inference metrics ────────────────────────────────────────────────────────

function updateInferenceMetrics(d) {
    const l = window.lastLlamaMetrics;
    const hasActiveEndpoint = !!d.active_session_id;

    // Speed metrics
    if (!window.speedMax) {
        window.speedMax = { prompt: 0, generation: 0 };
    }

    const promptEl = document.getElementById('m-prompt');
    const genEl = document.getElementById('m-gen');
    const promptMaxEl = document.getElementById('m-prompt-max');
    const genMaxEl = document.getElementById('m-gen-max');
    const promptBar = document.getElementById('m-prompt-bar');
    const genBar = document.getElementById('m-gen-bar');
    const throughputState = document.getElementById('m-throughput-state');
    const throughputAge = document.getElementById('m-throughput-age');
    const throughputCard = document.querySelector('.widget-speed');
    const generationCard = document.querySelector('.widget-generation');
    const contextCard = document.querySelector('.widget-context');
    const promptDeltaEl = document.getElementById('m-prompt-delta');
    const genDeltaEl = document.getElementById('m-gen-delta');

    const promptRate = l?.prompt_tokens_per_sec || 0;
    const genRate = l?.generation_tokens_per_sec || 0;
    const promptDisplayRate = promptRate > 0 ? promptRate : (l?.last_prompt_tokens_per_sec || 0);
    const genDisplayRate = genRate > 0 ? genRate : (l?.last_generation_tokens_per_sec || 0);
    const promptAgeMs = l?.last_prompt_throughput_unix_ms || 0;
    const genAgeMs = l?.last_generation_throughput_unix_ms || 0;
    const latestThroughputMs = Math.max(promptAgeMs, genAgeMs);
    const throughputActive = promptRate > 0 || genRate > 0;

    window.setCardState(throughputCard, !hasActiveEndpoint ? 'dormant' : throughputActive ? 'live' : 'idle');
    window.setEmptyState(document.getElementById('m-throughput-empty'), !hasActiveEndpoint);
    window.setChipState(throughputState, throughputActive ? 'live' : 'idle', throughputActive ? 'live' : 'idle');

    if (throughputAge) {
        throughputAge.textContent = formatMetricAge(latestThroughputMs);
    }

    // Prompt throughput
    if (promptDisplayRate > 0) {
        window.updateMetricDelta(promptDeltaEl, window.prevValues.prompt, promptDisplayRate, 1);
        window.animateNumber(promptEl, window.prevValues.prompt, promptDisplayRate, 300, 1, ' t/s');
        window.prevValues.prompt = promptDisplayRate;

        if (promptDisplayRate > window.speedMax.prompt) {
            window.speedMax.prompt = promptDisplayRate;
        }
        if (promptMaxEl && window.speedMax.prompt > 0) {
            promptMaxEl.textContent = 'peak ' + window.speedMax.prompt.toFixed(0);
        }
        const promptPct = Math.max((promptDisplayRate / window.speedMax.prompt) * 100, 4);
        if (promptBar) promptBar.style.width = promptPct + '%';
    } else {
        promptEl.textContent = '\u2014';
        if (promptMaxEl) promptMaxEl.textContent = '';
        if (promptBar) promptBar.style.width = '0%';
    }

    // Generation throughput
    if (genDisplayRate > 0) {
        window.updateMetricDelta(genDeltaEl, window.prevValues.generation, genDisplayRate, 1);
        window.animateNumber(genEl, window.prevValues.generation, genDisplayRate, 300, 1, ' t/s');
        window.prevValues.generation = genDisplayRate;

        if (genDisplayRate > window.speedMax.generation) {
            window.speedMax.generation = genDisplayRate;
        }
        if (genMaxEl && window.speedMax.generation > 0) {
            genMaxEl.textContent = 'peak ' + window.speedMax.generation.toFixed(0);
        }
        const genPct = Math.max((genDisplayRate / window.speedMax.generation) * 100, 4);
        if (genBar) genBar.style.width = genPct + '%';
    } else {
        genEl.textContent = '\u2014';
        if (genMaxEl) genMaxEl.textContent = '';
        if (genBar) genBar.style.width = '0%';
    }

    // Sparklines
    window.pushSparklinePoint('prompt', promptDisplayRate);
    window.pushSparklinePoint('generation', genDisplayRate);
    window.renderSparkline('m-prompt-spark', window.metricSeries.prompt, 'prompt', false);
    window.renderSparkline('m-gen-spark', window.metricSeries.generation, 'generation', false);

    // Throughput ratio
    const ratioBar = document.getElementById('m-throughput-ratio-bar');
    const ratioValue = document.getElementById('m-throughput-ratio');
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
    const generationState = document.getElementById('m-generation-state');
    const generationMain = document.getElementById('m-generation-main');
    const generationSub = document.getElementById('m-generation-sub');
    const generationDetails = document.getElementById('m-generation-details');
    const generationRing = document.getElementById('m-generation-ring');
    const liveVelocity = document.getElementById('m-live-velocity');
    const promptStage = document.getElementById('m-stage-prompt');
    const outputStage = document.getElementById('m-stage-output');
    const generated = l?.slot_generation_tokens || 0;
    const remaining = l?.slot_generation_remaining || 0;
    const generationAvailable = !!l?.slot_generation_available;
    const generationActive = !!l?.slot_generation_active || (l?.slots_processing || 0) > 0;
    const slotLimit = window.getPrimarySlot(l)?.output_limit || 0;
    const generationTotal = l?.slot_generation_limit || slotLimit || (generated + remaining);
    const generationPct = generationTotal > 0 ? Math.min(100, Math.max(2, (generated / generationTotal) * 100)) : 0;
    const taskId = generationActive ? l?.active_task_id : l?.last_task_id;
    const nowMs = Date.now();
    const liveOutputRate = window.updateLiveOutputEstimate(taskId, generated, generationActive, nowMs);

    window.updateRequestActivity(taskId, generationActive, generated, nowMs);
    window.renderActivityRail(generationActive);
    window.renderRecentTask();
    window.renderSlotGrid(l, hasActiveEndpoint);
    window.renderSlotUtilization(l);
    window.renderRequestStats();
    window.renderDecodingConfig(l, hasActiveEndpoint);
    window.renderLiveSparkline('m-live-output-spark', window.metricSeries.liveOutput);

    window.setCardState(generationCard, !hasActiveEndpoint ? 'dormant' : generationActive ? 'live' : generationAvailable ? 'idle' : 'unavailable');
    window.setEmptyState(document.getElementById('m-generation-empty'), !hasActiveEndpoint);
    window.setChipState(generationState, generationActive ? 'generating' : 'idle', generationActive ? 'live' : 'idle');
    window.setChipState(document.getElementById('m-slots-state'), generationActive ? 'active' : 'idle', generationActive ? 'live' : 'idle');
    window.setChipState(document.getElementById('m-activity-state'), generationActive ? 'active' : 'idle', generationActive ? 'live' : 'idle');
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
        if (generationMain) generationMain.textContent = window.formatMetricNumber(generated) + ' output tokens';
        if (generationSub) generationSub.textContent = window.formatMetricNumber(remaining) + ' remaining';
        if (generationDetails) {
            const detailParts = [];
            if (taskId !== null && taskId !== undefined) detailParts.push('task ' + taskId);
            if (generationTotal > 0) {
                const maxStr = generationTotal >= 1000 ? Math.round(generationTotal / 1000) + 'k' : window.formatMetricNumber(generationTotal);
                detailParts.push('max ' + maxStr);
            }
            detailParts.push(window.formatMetricNumber(remaining) + ' left');
            window.renderGenerationDetailItems(generationDetails, detailParts);
        }
    } else {
        if (generationMain) generationMain.textContent = generationActive ? 'working' : '\u2014';
        if (generationSub) generationSub.textContent = 'output budget';
        window.renderGenerationDetailItems(generationDetails, []);
    }

    // Context metrics
    updateContextMetrics(d, l, hasActiveEndpoint);

    // Capability popover
    if (typeof window.renderCapabilityPopover === 'function') {
        window.renderCapabilityPopover(d, l, generationAvailable, !!(l?.context_live_tokens_available || l?.kv_cache_tokens_available));
    }

    // Metric section visibility
    const hostMetricsVisible = d.host_metrics_available === true;
    const systemVisible = hostMetricsVisible && !!d.capabilities?.system;
    const gpuVisible = hostMetricsVisible && !!d.capabilities?.gpu;
    if (typeof window.setMetricSectionVisibility === 'function') {
        window.setMetricSectionVisibility('gpu-card', gpuVisible, 'gpu-section');
        window.setMetricSectionVisibility('system-card', systemVisible, 'system-section');
    }
}

// ── Context metrics ──────────────────────────────────────────────────────────

function updateContextMetrics(d, l, hasActiveEndpoint) {
    const ctxFill = document.getElementById('m-ctx-fill');
    const ctxValue = document.getElementById('m-ctx');
    const ctxDetails = document.getElementById('m-ctx-details');
    const ctxState = document.getElementById('m-context-state');
    const ctxPeakFill = document.getElementById('m-ctx-peak-fill');
    const ctxLiveLabel = document.getElementById('m-ctx-live-label');
    const ctxLiveDetail = document.getElementById('m-ctx-live-detail');
    const ctxPeakDetail = document.getElementById('m-ctx-peak-detail');
    const contextCapacity = l?.context_capacity_tokens || l?.kv_cache_max || 0;
    const contextLive = l?.context_live_tokens || l?.kv_cache_tokens || 0;
    const contextPeak = l?.context_high_water_tokens || l?.kv_cache_high_water || 0;
    const contextLiveAvailable = !!(l?.context_live_tokens_available || l?.kv_cache_tokens_available);
    const peakPct = contextCapacity > 0 && contextPeak > 0 ? Math.min(100, Math.max(2, (contextPeak / contextCapacity) * 100)) : 0;
    const contextCard = document.querySelector('.widget-context');

    if (typeof window.setEmptyState === 'function') window.setEmptyState(document.getElementById('m-context-empty'), !hasActiveEndpoint);
    if (ctxPeakFill) ctxPeakFill.style.width = peakPct + '%';
    if (ctxPeakDetail) ctxPeakDetail.textContent = contextPeak > 0 ? window.formatMetricNumber(contextPeak) + ' peak' : '\u2014';

    if (l && contextCapacity > 0 && contextLiveAvailable) {
        const pct = ((contextLive / contextCapacity) * 100);
        const severity = pct >= 95 ? 'critical' : pct >= 80 ? 'warning' : '';
        window.setCardState(contextCard, severity === 'critical' ? 'live' : 'idle');
        window.setChipState(ctxState, 'live', severity || 'live');
        if (ctxLiveLabel) ctxLiveLabel.textContent = 'Live usage';
        if (ctxLiveDetail) ctxLiveDetail.textContent = window.formatMetricNumber(contextLive) + ' live';

        window.animateNumber(ctxValue, window.prevValues.contextPct, pct, 300, 1, '%');
        window.prevValues.contextPct = pct;

        if (ctxFill) {
            ctxFill.style.width = pct + '%';
            ctxFill.className = 'context-progress-fill ' + severity;
        }

        if (ctxDetails) ctxDetails.textContent = window.formatMetricNumber(contextLive) + ' / ' + window.formatMetricNumber(contextCapacity);

    } else if (l && contextCapacity > 0) {
        window.setCardState(contextCard, 'unavailable');
        window.setChipState(ctxState, 'capacity', 'idle');
        if (ctxFill) {
            ctxFill.style.width = '0%';
            ctxFill.className = 'context-progress-fill unavailable';
        }
        if (ctxLiveLabel) ctxLiveLabel.textContent = 'Live usage';
        if (ctxLiveDetail) ctxLiveDetail.textContent = 'not exposed by llama-server';
        if (ctxValue) ctxValue.textContent = 'peak observed only';
        if (ctxDetails) {
            const detailParts = ['capacity ' + window.formatMetricNumber(contextCapacity)];
            if (contextPeak > 0) {
                detailParts.push('peak ' + window.formatMetricNumber(contextPeak));
            }
            ctxDetails.textContent = detailParts.join(' · ');
        }
    } else {
        window.setCardState(contextCard, !hasActiveEndpoint ? 'dormant' : 'unavailable');
        window.setChipState(ctxState, 'unknown', 'idle');
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
    window.lastGpuData = lastGpuData;

    if (typeof window.renderGpuCard === 'function') {
        window.renderGpuCard(d.gpu || {}, gpuVisible);
    }
}

// ── System card ──────────────────────────────────────────────────────────────

function updateSystemCard(d) {
    const systemVisible = d.host_metrics_available === true && !!d.capabilities?.system;

    if (typeof window.renderSystemCard === 'function') {
        window.renderSystemCard(window.lastSystemMetrics, systemVisible);
    }
}

// ── Logs ─────────────────────────────────────────────────────────────────────

function updateLogs(d) {
    const logs = d.logs || [];

    if (logs.length !== window.prevLogLen) {
        const el = document.getElementById('log-panel');
        const wasAtBottom = el && (el.scrollHeight - el.scrollTop - el.clientHeight < 40);

        if (el) {
            el.textContent = logs.join('\n');
            if (wasAtBottom) el.scrollTop = el.scrollHeight;
        }

        window.prevLogLen = logs.length;
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
    const badgeServer = document.getElementById('badge-server');
    if (badgeServer) badgeServer.textContent = badgeParts.length ? ' ' + badgeParts.join(' \u00b7 ') : '';

    // Chat badge
    const badgeChat = document.getElementById('badge-chat');
    if (typeof window.activeChatTab === 'function') {
        const tab = window.activeChatTab();
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
    }

    // Logs badge
    const badgeLogs = document.getElementById('badge-logs');
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
