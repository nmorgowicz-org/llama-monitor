// ── Dashboard Render ─────────────────────────────────────────────────────────
// Rendering functions extracted from legacy app.js for dashboard-ws.js.

        '>': '&gt;',
        '"': '&quot;',
        "'": '&#39;'
    })[char]);
}

function setChipState(el, label, state) {
    if (!el) return;
    el.textContent = label;
    el.className = 'metric-live-chip ' + (state || '');
}

function setCardState(card, state) {
    if (!card) return;
    card.classList.remove('is-live', 'is-idle', 'is-unavailable', 'is-dormant');
    if (state) card.classList.add('is-' + state);
}

function pushSparklinePoint(name, value) {
    window.metricSeries[name].push(Number.isFinite(value) ? value : 0);
    const limit = name === 'liveOutput' ? 90 : 40;
    if (window.metricSeries[name].length > limit) {
        window.metricSeries[name].shift();
    }
}

function renderSparkline(id, points, className, isBlocked) {
    const svg = document.getElementById(id);
    if (!svg || !points || points.length < 2) return;
    const width = 120;
    const height = 28;
    const max = Math.max(...points, 1);
    const step = width / (points.length - 1);
    const path = points.map((value, index) => {
        const x = index * step;
        const y = height - ((value / max) * (height - 4)) - 2;
        return (index === 0 ? 'M' : 'L') + x.toFixed(2) + ' ' + y.toFixed(2);
    }).join(' ');
    const wallLine = isBlocked ? '<line x1="120" y1="0" x2="120" y2="28" stroke="#ebcb8b" stroke-width="1" stroke-dasharray="3 3" opacity="0.5"/>' : '';
    svg.innerHTML = '<path class="sparkline-fill ' + className + '" d="' + path + ' L 120 28 L 0 28 Z"></path><path class="sparkline-line ' + className + '" d="' + path + '"></path>' + wallLine;
}

function renderLiveSparkline(id, points) {
    const svg = document.getElementById(id);
    if (!svg) return;
    if (!points || points.length < 2) {
        svg.innerHTML = '';
        return;
    }
    const width = 120;
    const height = 28;
    const max = Math.max(...points, 1);
    const step = width / (points.length - 1);
    let peak = { value: -1, x: 0, y: height - 2 };
    const path = points.map((value, index) => {
        const x = index * step;
        const y = height - ((value / max) * (height - 6)) - 3;
        if (value > peak.value) peak = { value, x, y };
        return (index === 0 ? 'M' : 'L') + x.toFixed(2) + ' ' + y.toFixed(2);
    }).join(' ');
    svg.innerHTML = [
        '<path class="sparkline-fill live-output" d="' + path + ' L 120 28 L 0 28 Z"></path>',
        '<path class="sparkline-line live-output" d="' + path + '"></path>',
        '<circle class="sparkline-peak live-output" cx="' + peak.x.toFixed(2) + '" cy="' + peak.y.toFixed(2) + '" r="2.6"></circle>'
    ].join('');
}

function getTaskKey(taskId, active) {
    if (taskId !== null && taskId !== undefined) return String(taskId);
    return active ? 'active-unknown' : null;
}

function updateLiveOutputEstimate(taskId, decoded, active, nowMs) {
    const tracker = window.liveOutputTracker;
    const taskChanged = tracker.taskId !== taskId;
    if (taskChanged) {
        tracker.taskId = taskId;
        tracker.previousDecoded = Number.isFinite(decoded) ? decoded : null;
        tracker.previousMs = nowMs;
        tracker.latestRate = 0;
        tracker.rates = [];
        window.metricSeries.liveOutput = [];
        pushSparklinePoint('liveOutput', 0);
        return 0;
    }

    let rate = 0;
    if (active && Number.isFinite(decoded) && Number.isFinite(tracker.previousDecoded) && tracker.previousMs) {
        const deltaTokens = Math.max(0, decoded - tracker.previousDecoded);
        const deltaSeconds = (nowMs - tracker.previousMs) / 1000;
        rate = deltaSeconds > 0 ? deltaTokens / deltaSeconds : 0;
    }

    if (active && rate > 0) {
        tracker.rates.push(rate);
        if (tracker.rates.length > 6) tracker.rates.shift();
    } else if (!active) {
        tracker.rates = [];
    }

    const smoothedRate = tracker.rates.length
        ? tracker.rates.reduce((sum, value) => sum + value, 0) / tracker.rates.length
        : 0;

    tracker.previousDecoded = Number.isFinite(decoded) ? decoded : tracker.previousDecoded;
    tracker.previousMs = nowMs;
    tracker.latestRate = smoothedRate;
    pushSparklinePoint('liveOutput', smoothedRate);
    return smoothedRate;
}

function updateRequestActivity(taskId, active, outputTokens, nowMs) {
    const taskKey = getTaskKey(taskId, active);
    let openSegment = window.requestActivity.find(segment => !segment.endedAtMs);

    if (active && taskKey) {
        if (!openSegment || openSegment.taskKey !== taskKey) {
            if (openSegment) {
                openSegment.endedAtMs = nowMs;
                openSegment.outputTokens = outputTokens || openSegment.outputTokens || 0;
                window.recentTasks.unshift(openSegment);
            }
            window.requestActivity.push({
                taskKey,
                taskId,
                startedAtMs: nowMs,
                firstOutputAtMs: outputTokens > 0 ? nowMs : null,
                endedAtMs: null,
                state: 'active',
                outputTokens: outputTokens || 0
            });
        } else {
            if (!openSegment.firstOutputAtMs && outputTokens > 0) {
                openSegment.firstOutputAtMs = nowMs;
            }
            openSegment.outputTokens = outputTokens || openSegment.outputTokens || 0;
        }
    } else if (openSegment) {
        openSegment.endedAtMs = nowMs;
        openSegment.state = 'complete';
        if (!openSegment.firstOutputAtMs && outputTokens > 0) {
            openSegment.firstOutputAtMs = nowMs;
        }
        openSegment.outputTokens = outputTokens || openSegment.outputTokens || 0;
        window.recentTasks.unshift(openSegment);
    }

    const cutoff = nowMs - (10 * 60 * 1000);
    window.requestActivity = window.requestActivity
        .filter(segment => !segment.endedAtMs || segment.endedAtMs >= cutoff)
        .slice(-100);
    window.recentTasks = window.recentTasks.slice(0, 8);
}

function formatDuration(ms) {
    if (!Number.isFinite(ms) || ms <= 0) return '~0.0s';
    if (ms < 1000) return '~' + Math.round(ms) + 'ms';
    return '~' + (ms / 1000).toFixed(1) + 's';
}

function renderRecentTask() {
    const el = document.getElementById('m-recent-task');
    if (!el) return;
    const task = window.recentTasks[0];
    if (!task || !task.endedAtMs) {
        el.style.display = 'none';
        el.textContent = '';
        return;
    }
    const durationMs = task.endedAtMs - task.startedAtMs;
    const rate = durationMs > 0 ? (task.outputTokens / (durationMs / 1000)) : 0;
    const id = task.taskId !== null && task.taskId !== undefined ? task.taskId : 'unknown';
    el.style.display = '';
    el.textContent = 'Last task ' + id + ' · ' + formatMetricNumber(task.outputTokens || 0) + ' output tokens · ' + formatDuration(durationMs) + ' · ~' + rate.toFixed(1) + ' t/s estimated';
}

function renderActivityRail(active) {
    const rail = document.getElementById('m-activity-rail');
    if (!rail) return;
    const now = Date.now();
    const windowMs = 5 * 60 * 1000;
    const segments = window.requestActivity.slice(-28);
    if (!segments.length) {
        rail.innerHTML = '<span class="activity-empty">No recent tasks</span>';
        return;
    }
    rail.innerHTML = segments.map(segment => {
        let start = Math.max(0, Math.min(100, ((segment.startedAtMs - (now - windowMs)) / windowMs) * 100));
        const endMs = segment.endedAtMs || now;
        const minWidth = segment.endedAtMs ? 3 : 8;
        let width = Math.max(minWidth, Math.min(100 - start, ((endMs - segment.startedAtMs) / windowMs) * 100));
        if (start + width > 100) {
            start = Math.max(0, 100 - width);
        }
        const firstOutputAtMs = segment.firstOutputAtMs || (segment.outputTokens > 0 ? endMs : null);
        const phaseTotalMs = Math.max(1, endMs - segment.startedAtMs);
        const promptPct = firstOutputAtMs
            ? Math.max(12, Math.min(72, ((firstOutputAtMs - segment.startedAtMs) / phaseTotalMs) * 100))
            : 100;
        const generationPct = firstOutputAtMs ? Math.max(18, 100 - promptPct) : 0;
        const duration = formatDuration(endMs - segment.startedAtMs);
        const task = segment.taskId !== null && segment.taskId !== undefined ? segment.taskId : 'unknown';
        const cls = segment.endedAtMs ? 'complete' : active ? 'active' : 'complete';
        const title = 'task ' + task + ' · ' + duration + ' · ' + formatMetricNumber(segment.outputTokens || 0) + ' output tokens';
        const phases = [
            '<span class="activity-phase prompt" style="width:' + promptPct.toFixed(2) + '%"></span>',
            generationPct > 0 ? '<span class="activity-phase generation" style="width:' + generationPct.toFixed(2) + '%"></span>' : '',
            segment.endedAtMs ? '<span class="activity-marker" aria-hidden="true"></span>' : ''
        ].join('');
        return '<span class="activity-segment ' + cls + '" style="left:' + start.toFixed(2) + '%;width:' + width.toFixed(2) + '%" tabindex="0" title="' + title + '">' + phases + '</span>';
    }).join('');
}

function renderSlotGrid(l, hasActiveEndpoint) {
    const grid = document.getElementById('m-slot-grid');
    if (!grid) return;
    if (!hasActiveEndpoint || !l) {
        grid.innerHTML = '<div class="slot-tile idle"><div class="slot-tile-top"><span>slots</span><strong>waiting</strong></div><div class="slot-tile-task">attach endpoint</div><div class="slot-tile-meta"><span>0 output</span><span>ctx unknown</span></div></div>';
        return;
    }
    const slotSnapshots = Array.isArray(l.slots) ? l.slots : [];
    if (slotSnapshots.length > 0) {
        grid.innerHTML = slotSnapshots.map(slot => {
            const busy = !!slot.is_processing;
            const task = busy && slot.id_task !== null && slot.id_task !== undefined ? 'task ' + slot.id_task : 'idle';
            const output = slot.output_available ? formatMetricNumber(slot.output_tokens || 0) + ' output' : 'output unknown';
            const ctx = slot.n_ctx > 0 ? formatMetricNumber(slot.n_ctx) + ' ctx' : 'ctx unknown';
            return '<div class="slot-tile ' + (busy ? 'busy' : 'idle') + '">' +
                '<div class="slot-tile-top"><span>slot ' + escapeHtml(slot.id ?? '?') + '</span><strong>' + (busy ? 'active' : 'idle') + '</strong></div>' +
                '<div class="slot-tile-task">' + escapeHtml(task) + '</div>' +
                '<div class="slot-tile-meta"><span>' + output + '</span><span>' + ctx + '</span></div>' +
            '</div>';
        }).join('');
        return;
    }
    const processing = l?.slots_processing || 0;
    const idle = l?.slots_idle || 0;
    const total = Math.max(1, processing + idle);
    const activeTask = l?.active_task_id;
    const capacity = l?.context_capacity_tokens || l?.kv_cache_max || 0;
    const generated = l?.slot_generation_tokens || 0;
    const tiles = [];

    for (let index = 0; index < total; index += 1) {
        const busy = index < processing;
        const task = busy && activeTask !== null && activeTask !== undefined ? 'task ' + activeTask : 'idle';
        const output = busy || total === 1 ? formatMetricNumber(generated) + ' output' : '0 output';
        const slotCapacity = capacity > 0 ? Math.round(capacity / total) : 0;
        const ctx = slotCapacity > 0 ? formatMetricNumber(slotCapacity) + ' ctx' : 'ctx unknown';
        tiles.push(
            '<div class="slot-tile ' + (busy ? 'busy' : 'idle') + '">' +
                '<div class="slot-tile-top"><span>slot ' + index + '</span><strong>' + (busy ? 'active' : 'idle') + '</strong></div>' +
                '<div class="slot-tile-task">' + task + '</div>' +
                '<div class="slot-tile-meta"><span>' + output + '</span><span>' + ctx + '</span></div>' +
            '</div>'
        );
    }

    grid.innerHTML = tiles.join('');
}

function getPrimarySlot(l) {
    const slots = Array.isArray(l?.slots) ? l.slots : [];
    return slots.find(slot => slot.is_processing) || slots[0] || null;
}

function renderSlotUtilization(l) {
    const utilBar = document.getElementById('m-slot-util-bar');
    const utilValue = document.getElementById('m-slot-util');
    if (!l || !l.slots_processing !== undefined && l.slots_idle !== undefined) {
        if (utilBar) utilBar.style.width = '0%';
        if (utilValue) utilValue.textContent = '\u2014';
        return;
    }
    const processing = l.slots_processing || 0;
    const idle = l.slots_idle || 0;
    const total = processing + idle;
    if (total === 0) {
        if (utilBar) utilBar.style.width = '0%';
        if (utilValue) utilValue.textContent = '\u2014';
        return;
    }
    const utilPct = Math.round((processing / total) * 100);
    if (utilBar) utilBar.style.width = utilPct + '%';
    if (utilValue) utilValue.textContent = utilPct + '%';
}

function renderRequestStats() {
    const reqCount = document.getElementById('m-req-count');
    const reqAvg = document.getElementById('m-req-avg');
    const now = Date.now();
    const windowMs = 10 * 60 * 1000;
    const segments = window.requestActivity.filter(s => (s.endedAtMs || now) >= now - windowMs);
    const completed = segments.filter(s => s.endedAtMs);
    if (reqCount) reqCount.textContent = formatMetricNumber(completed.length);
    if (reqAvg && completed.length > 0) {
        const totalDuration = completed.reduce((sum, s) => sum + (s.endedAtMs - s.startedAtMs), 0);
        const avgDuration = totalDuration / completed.length;
        reqAvg.textContent = formatDuration(avgDuration);
    } else if (reqAvg) {
        reqAvg.textContent = '\u2014';
    }
}

function renderSamplerParamsInline(slot) {
    const el = document.getElementById('m-sampler-params-inline');
    if (!el || !slot || !slot.sampler_config) {
        el.innerHTML = '';
        return;
    }
    const samplerItems = slot.sampler_config || [];
    const priorityKeys = ['top_k', 'top_p', 'min_p', 'temperature', 'dry', 'xtc'];
    const priorityItems = samplerItems.filter(item => priorityKeys.includes(item.label));
    if (priorityItems.length === 0) {
        el.innerHTML = '';
        return;
    }
    el.innerHTML = priorityItems.slice(0, 4).map(item => {
        const displayValue = formatConfigValue(item.value);
        return '<span class="config-kv"><span>' + escapeHtml(item.label) + '</span><strong>' + escapeHtml(displayValue) + '</strong></span>';
    }).join('');
}

function formatConfigValue(value) {
    const num = parseFloat(value);
    if (!Number.isNaN(num) && Number.isFinite(num)) {
        const rounded = Math.round(num * 100) / 100;
        return String(rounded);
    }
    return String(value);
}

function renderConfigItems(id, items, emptyText) {
    const el = document.getElementById(id);
    if (!el) return;
    if (!items || !items.length) {
        el.innerHTML = '<span class="config-empty">' + emptyText + '</span>';
        return;
    }
    el.innerHTML = items.map(item => {
        const displayValue = formatConfigValue(item.value);
        return '<span class="config-kv"><span>' + escapeHtml(item.label) + '</span><strong>' + escapeHtml(displayValue) + '</strong></span>';
    }).join('');
}

function renderGenerationDetailItems(el, parts) {
    if (!el) return;
    el.innerHTML = parts
        .filter(Boolean)
        .map(part => '<span class="generation-detail-chip">' + escapeHtml(part) + '</span>')
        .join('');
}

function renderDecodingConfig(l, hasActiveEndpoint) {
    const slot = getPrimarySlot(l);
    const specChip = document.getElementById('m-speculative-chip');
    const decodingState = document.getElementById('m-decoding-state');
    const hasConfig = !!slot && ((slot.sampler_stack || []).length > 0 || (slot.speculative_config || []).length > 0);

    setChipState(decodingState, hasConfig ? 'config' : 'waiting', hasConfig ? 'live' : 'idle');

    if (!hasActiveEndpoint || !slot) {
        if (specChip) specChip.textContent = 'Attach an endpoint for decoding config';
        renderConfigItems('m-speculative-config', [], 'Speculative config unavailable');
        renderSamplerParamsInline(null);
        return;
    }

    if (specChip) {
        const specType = slot.speculative_type || 'configuration';
        const nMax = (slot.speculative_config || []).find(item => item.label === 'n_max');
        if (slot.speculative_enabled || (slot.speculative_config || []).length > 0) {
            specChip.textContent = 'Speculative · ' + specType + (nMax ? ' · n_max ' + nMax.value : '');
            specChip.classList.add('enabled');
        } else {
            specChip.textContent = 'Speculative decoding not enabled';
            specChip.classList.remove('enabled');
        }
    }

    renderConfigItems('m-speculative-config', slot.speculative_config || [], 'Configuration only appears when exposed');

    renderSamplerParamsInline(slot);
}

function renderCapabilityPopover(d, l, generationAvailable, contextLiveAvailable) {
    const popover = document.getElementById('capability-popover');
    if (!popover) return;
    const hasInference = !!d.capabilities?.inference;
    const slotsAvailable = !!l && ((l.slots_processing || 0) + (l.slots_idle || 0) > 0);
    const metricsAvailable = !!l && (
        (l.prompt_tokens_total || 0) > 0 ||
        (l.generation_tokens_total || 0) > 0 ||
        (l.context_high_water_tokens || 0) > 0 ||
        (l.last_generation_throughput_unix_ms || 0) > 0 ||
        (l.last_prompt_throughput_unix_ms || 0) > 0
    );
    const rows = [
        ['Inference', hasInference ? 'live' : 'unavailable', hasInference],
        ['Slots', slotsAvailable ? 'live' : 'waiting', slotsAvailable],
        ['Metrics', metricsAvailable ? 'live' : 'waiting', metricsAvailable],
        ['Generation progress', generationAvailable ? 'live' : 'not exposed', generationAvailable],
        ['Throughput', metricsAvailable ? 'retained avg + live estimate' : 'waiting', metricsAvailable],
        ['Context capacity', (l?.context_capacity_tokens || 0) > 0 ? 'live' : 'waiting', (l?.context_capacity_tokens || 0) > 0],
        ['Context usage', contextLiveAvailable ? 'live' : 'not exposed', contextLiveAvailable],
        ['Host metrics', d.host_metrics_available ? 'live' : 'unavailable', !!d.host_metrics_available],
        ['Remote agent', d.remote_agent_connected ? 'connected' : 'disconnected', !!d.remote_agent_connected]
    ];
    popover.innerHTML = rows.map(([label, value, ok]) => {
        return '<span class="capability-row"><span class="capability-led ' + (ok ? 'ok' : 'muted') + '"></span><span>' + label + '</span><strong>' + value + '</strong></span>';
    }).join('');
}

function updateMetricDelta(el, previous, current, decimals = 1) {
    if (!el || !Number.isFinite(previous) || previous <= 0 || !Number.isFinite(current)) return;
    const delta = current - previous;
    if (Math.abs(delta) < 0.05) return;
    el.textContent = (delta > 0 ? '+' : '') + delta.toFixed(decimals);
    el.className = 'metric-delta ' + (delta > 0 ? 'positive' : 'negative') + ' show';
    window.clearTimeout(el._hideTimer);
    el._hideTimer = window.setTimeout(() => {
        el.classList.remove('show');
    }, 900);
}

function setEmptyState(el, show) {
    if (!el) return;
    el.classList.toggle('visible', !!show);
}

// Store previous values for animation
window.prevValues = {
    prompt: 0,
    generation: 0,
    contextPct: 0
};

window.metricSeries = {
    prompt: [],
    generation: [],
    liveOutput: []
};

window.slotSnapshots = new Map();
window.requestActivity = [];
window.recentTasks = [];
window.metricCapabilities = {};
window.liveOutputTracker = {
    taskId: null,
    previousDecoded: null,
    previousMs: null,
    latestRate: 0,
    rates: []
};


let remoteAgentInProgress = false;

let remoteAgentSshConnection = null;
let latestSshHostKey = null;



let lastServerState = null;

let lastLlamaMetrics = null;

let lastSystemMetrics = null;

let lastGpuMetrics = null;

let lastCapabilities = null;

let currentPollInterval = 5000;



   let presets = [];

    let serverRunning = false;

    let prevLogLen = 0;

    

    let sessions = [];

    const latestVer = data.latest_release?.tag_name || data.release?.tag_name || 'Not checked';

    const installedVer = data.installed_version || (data.installed ? 'Unknown' : 'Not installed');

    document.getElementById('remote-agent-latest-version').textContent = latestVer;

    document.getElementById('remote-agent-installed-version').textContent = installedVer;

    versionsEl.style.display = '';

    const isInstalled = data.installed || false;

    const isRunning = data.running || false;

    const isUpdateAvailable = data.update_available || false;

    const updateIndicator = document.getElementById('remote-agent-update-indicator');

    if (updateIndicator) {
        updateIndicator.style.display = isUpdateAvailable ? 'inline' : 'none';
        updateIndicator.textContent = '● Update available';
        updateIndicator.style.color = '#ebcb8b';
    }

    const buttonsEl = document.getElementById('remote-agent-buttons');

    if (buttonsEl) {

        const installBtn = document.getElementById('btn-remote-agent-install');

        const startBtn = document.getElementById('btn-remote-agent-start');

        const updateBtn = document.getElementById('btn-remote-agent-update');

        const stopBtn = document.getElementById('btn-remote-agent-stop');

        const restartBtn = document.getElementById('btn-remote-agent-restart');

        const removeBtn = document.getElementById('btn-remote-agent-remove');

        if (installBtn) installBtn.style.display = isInstalled ? 'none' : '';

        if (startBtn) startBtn.style.display = isRunning ? 'none' : '';

        if (updateBtn) {
            if (isUpdateAvailable) {
                updateBtn.style.display = '';
                updateBtn.textContent = 'Update Agent';
            } else if (isRunning) {
                updateBtn.textContent = 'Restart';
                updateBtn.style.display = '';
            } else {
                updateBtn.style.display = 'none';
            }
        }

        if (stopBtn) stopBtn.style.display = isRunning ? '' : 'none';

        if (restartBtn) restartBtn.style.display = isRunning ? '' : 'none';

        if (removeBtn) removeBtn.style.display = (isInstalled || data.managed_task_installed) ? '' : 'none';

        if (isRunning && isUpdateAvailable) {
            document.getElementById('remote-agent-status-indicator').textContent = '● Update available';
            document.getElementById('remote-agent-status-indicator').style.color = '#ebcb8b';
        } else if (isRunning) {
            document.getElementById('remote-agent-status-indicator').textContent = '● Ready';
            document.getElementById('remote-agent-status-indicator').style.color = '#a3be8b';
        } else {
            document.getElementById('remote-agent-status-indicator').textContent = '● Not running';
            document.getElementById('remote-agent-status-indicator').style.color = '#8899aa';
        }

    }

}

function showRemoteAgentFirewall(showAlert = true) {

    const firewallEl = document.getElementById('remote-agent-firewall');

    if (firewallEl) {

        firewallEl.style.display = '';

    }

    if (showAlert) {
        showToast('Firewall blocked - Agent HTTP access is not reachable', 'error');
    }

}

function openFirewallHelp() {
    openConfigModal();

    const panel = document.getElementById('remote-agent-panel');
    if (panel) panel.open = true;

    const agentUrlInput = document.getElementById('set-remote-agent-url');
    if (agentUrlInput && !agentUrlInput.value.trim()) {
        agentUrlInput.value = inferredAgentUrl();
    }

    const sshTargetInput = document.getElementById('set-remote-agent-ssh-target');
    if (sshTargetInput && !sshTargetInput.value.trim()) {
        sshTargetInput.value = remoteEndpointHost();
    }

    const firewallEl = document.getElementById('remote-agent-firewall');

    if (firewallEl && firewallEl.style.display === 'none') {
        firewallEl.style.display = '';
    }

    setRemoteAgentStatus(
        'Configure the remote agent for this host, then use <strong>Install & Start</strong> or <strong>Start Agent</strong>. If the agent starts but remains unreachable, open TCP port <strong>7779</strong> on the remote machine.',
        'info'
    );

    setTimeout(() => {
        if (firewallEl) firewallEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
        sshTargetInput?.focus();
        sshTargetInput?.select();
    }, 50);

}

function addTimelineItem(message, status) {

    const timelineEl = document.getElementById('remote-agent-timeline');

    const itemsEl = document.getElementById('remote-agent-timeline-items');

    if (!timelineEl || !itemsEl) return;

    timelineEl.style.display = '';

    const timestamp = new Date().toLocaleTimeString();

    const item = document.createElement('div');

    item.className = 'remote-agent-timeline-item ' + status;

    item.innerHTML = '<span class="timestamp">[' + timestamp + ']</span>' + message;

    itemsEl.appendChild(item);

    itemsEl.scrollTop = itemsEl.scrollHeight;

}

function clearTimeline() {

    const itemsEl = document.getElementById('remote-agent-timeline-items');

    if (itemsEl) {

        itemsEl.innerHTML = '';

    }

    const timelineEl = document.getElementById('remote-agent-timeline');

    if (timelineEl) {

        timelineEl.style.display = 'none';

    }

}

let fbTargetId = '';

let fbFilter = '';

let fbCurrentPath = '';



function openFileBrowser(targetId, filter) {

    fbTargetId = targetId;

    fbFilter = filter === 'dir' ? '' : (filter || '');

    const modal = document.getElementById('file-browser-modal');

    // If target already has a path, start there; otherwise home

    const current = document.getElementById(targetId).value;

    let startPath = '';

    if (current) {

        // Use parent directory of current value

        const parts = current.split('/');

        parts.pop();

        startPath = parts.join('/') || '/';

    }

    // Show/hide "Select This Folder" for dir-mode

    const selectBtn = modal.querySelector('.btn-modal-save');

    selectBtn.style.display = filter === 'dir' ? '' : 'none';

    modal.classList.add('open');

    fileBrowserGo(startPath);

}



function closeFileBrowser() {

    document.getElementById('file-browser-modal').classList.remove('open');

}



document.getElementById('file-browser-modal').addEventListener('click', e => {

    if (e.target === e.currentTarget) closeFileBrowser();

});



async function fileBrowserGo(path) {

    const entriesEl = document.getElementById('fb-entries');

    entriesEl.innerHTML = '<div class="fb-empty">Loading...</div>';

    const params = new URLSearchParams();

    if (path) params.set('path', path);

    if (fbFilter) params.set('filter', fbFilter);

    try {

        const resp = await fetch('/api/browse?' + params);

        const data = await resp.json();

        if (data.error) {

            entriesEl.innerHTML = '<div class="fb-empty">' + data.error + '</div>';

            return;

        }

        fbCurrentPath = data.path;

        document.getElementById('fb-path-input').value = data.path;

        if (data.entries.length === 0) {

            entriesEl.innerHTML = '<div class="fb-empty">Empty directory</div>';

            return;

        }

        entriesEl.innerHTML = data.entries.map(e => {

            const escapeJsString = (s) => s.replace(/\\/g, '\\\\').replace(/'/g, "\\'");

            if (e.is_dir) {

                return '<div class="fb-entry fb-entry-dir" onclick="fileBrowserGo(\'' + escapeJsString(e.path) + '\')">' +

                    '<span class="fb-entry-icon">\u{1F4C1}</span>' +

                    '<span class="fb-entry-name">' + e.name + '</span></div>';

            } else {

                return '<div class="fb-entry fb-entry-file fb-match" onclick="fileBrowserSelect(\'' + escapeJsString(e.path) + '\')">' +

                    '<span class="fb-entry-icon">\u{1F4C4}</span>' +

                    '<span class="fb-entry-name">' + e.name + '</span>' +

                    '<span class="fb-entry-size">' + e.size_display + '</span></div>';

            }

        }).join('');

    } catch (err) {

        entriesEl.innerHTML = '<div class="fb-empty">Error: ' + err.message + '</div>';

    }

}



function fileBrowserUp() {

    if (fbCurrentPath && fbCurrentPath !== '/') {

        const parts = fbCurrentPath.split('/');

        parts.pop();

        fileBrowserGo(parts.join('/') || '/');

    }

}



function fileBrowserSelect(path) {

    document.getElementById(fbTargetId).value = path || fbCurrentPath;

    document.getElementById(fbTargetId).dispatchEvent(new Event('input', { bubbles: true }));

    closeFileBrowser();

}



// Close file browser on Escape

document.addEventListener('keydown', e => {

    if (e.key === 'Escape' && document.getElementById('file-browser-modal').classList.contains('open')) {

        closeFileBrowser();

        e.stopImmediatePropagation();

    }

}, true);



// --- Preset Selection ---



document.getElementById('preset-select').addEventListener('change', () => saveSettings());



// --- Toast Notifications ---

const TOAST_AUTO_DISMISS = 3500;

function showToast(title, type = 'error', message = '') {

    const container = document.getElementById('toast-container');

    const toast = document.createElement('div');

    toast.className = 'toast toast-' + type;

    let content = '';

    if (type === 'progress') {

        content = '<div class="toast-content"><div class="toast-progress-bar"><div class="toast-progress-fill" style="width:0%"></div></div></div>';

    } else {

        const iconMap = {
            success: 'success',
            error: 'error',
            warning: 'warning',
            info: 'info'
        };

        const iconType = iconMap[type] || 'info';

        content = `
            <div class="toast-icon ${type}">${getToastIcon(iconType)}</div>
            <div class="toast-content">
                ${title ? '<div class="toast-title">' + escapeHtml(title) + '</div>' : ''}
                ${message ? '<div class="toast-message">' + escapeHtml(message) + '</div>' : ''}
            </div>
            <button class="toast-close" onclick="this.parentElement.remove()">&times;</button>
        `;

    }

    toast.innerHTML = content;

    container.appendChild(toast);

    requestAnimationFrame(() => { toast.classList.add('show'); });

    if (type === 'progress') {

        return toast;

    } else {

        setTimeout(() => {

            toast.classList.remove('show');

            setTimeout(() => toast.remove(), 300);

        }, TOAST_AUTO_DISMISS);

        return null;

    }

}

function getToastIcon(type) {


// Render GPU card
function renderGpuCard(gpuMap, visible) {
    var card = document.getElementById('gpu-card');
    var emptyEl = document.getElementById('gpu-empty');
    var deviceName = document.getElementById('gpu-device-name');
    var tempGauge = document.getElementById('gpu-temp-gauge');
    var tempValue = document.getElementById('gpu-temp-value');
    var stateChip = document.getElementById('gpu-state');

    if (!card || !visible) {
        if (card) setCardState(card, 'dormant');
        return;
    }

    var entries = Object.entries(gpuMap);
    if (entries.length === 0) {
        setCardState(card, 'unavailable');
        setEmptyState(emptyEl, true);
        return;
    }

    lastGpuData = gpuMap;
    setEmptyState(emptyEl, false);

    // Use first GPU (most common case)
    var _loop = entries[0];
    var name = _loop[0];
    var m = _loop[1];

    setCardState(card, 'live');
    setChipState(stateChip, 'live', 'live');

    // Device name
    if (deviceName) deviceName.textContent = name;

    // Temperature gauge
    var temp = Math.round(m.temp);
    var tempPct = Math.min(100, (temp / 100) * 100);
    var tempColor = getTempSeverityColor(temp);
    var isTempHot = temp >= 75;
    if (tempGauge) {
        tempGauge.style.setProperty('--pct', tempPct.toFixed(1));
        tempGauge.style.setProperty('--gauge-color', tempColor);
        tempGauge.classList.toggle('is-warming', isTempHot);
    }
    if (tempValue) tempValue.textContent = temp + '\u00B0';

    // Push history
    pushGpuHistory('load', m.load);
    pushGpuHistory('power', m.power_consumption);
    var vramPct = m.vram_total > 0 ? (m.vram_used / m.vram_total) * 100 : 0;
    pushGpuHistory('vramPct', vramPct);
    pushGpuHistory('sclk', m.sclk_mhz);
    pushGpuHistory('mclk', m.mclk_mhz);

    // Load
    var loadViz = document.getElementById('gpu-load-viz');
    var loadVal = document.getElementById('gpu-load-value');
    var loadStyle = vizPrefs.gpu.load;
    var loadHot = m.load >= 90;
    var loadColor = getSeverityColor(m.load);
    if (loadStyle === 'ring') renderHwRing(loadViz, m.load, loadHot);
    else if (loadStyle === 'sparkline') renderHwSparkline(loadViz, gpuHistory.load);
    else renderHwBar(loadViz, m.load, loadHot);
    renderHwMetricSparkline('gpu-load-spark', gpuHistory.load, loadColor, loadStyle !== 'sparkline');
    if (loadVal) loadVal.textContent = m.load + '%';

    // Power
    var powerViz = document.getElementById('gpu-power-viz');
    var powerVal = document.getElementById('gpu-power-value');
    var powerBlock = document.getElementById('gpu-power-block');
    var powerPct = m.power_limit > 0 ? (m.power_consumption / m.power_limit) * 100 : 0;
    var isCapped = m.power_consumption >= m.power_limit && m.power_limit > 0;
    var powerStyle = vizPrefs.gpu.power;
    var powerColor = getSeverityColor(powerPct);
    if (powerBlock) powerBlock.classList.toggle('hw-power-capped', isCapped);
    if (powerStyle === 'ring') renderHwRing(powerViz, powerPct, isCapped);
    else if (powerStyle === 'sparkline') renderHwSparkline(powerViz, gpuHistory.power);
    else renderHwBar(powerViz, powerPct, isCapped);
    renderHwMetricSparkline('gpu-power-spark', gpuHistory.power, powerColor, powerStyle !== 'sparkline');
    if (powerVal) powerVal.textContent = m.power_consumption.toFixed(1) + 'W' + (isCapped ? '!' : '') + ' / ' + m.power_limit + 'W';

    // VRAM
    var vramViz = document.getElementById('gpu-vram-viz');
    var vramVal = document.getElementById('gpu-vram-value');
    var vramStyle = vizPrefs.gpu.vram;
    var vramGb = m.vram_total > 0 ? (m.vram_used / 1024).toFixed(1) : '0';
    var vramTotalGb = m.vram_total > 0 ? (m.vram_total / 1024).toFixed(0) : '0';
    var vramColor = getSeverityColor(vramPct);
    if (vramStyle === 'ring') renderHwRing(vramViz, vramPct, vramPct >= 90);
    else if (vramStyle === 'sparkline') renderHwSparkline(vramViz, gpuHistory.vramPct);
    else if (vramStyle === 'stacked') renderHwStacked(vramViz, vramPct);
    else renderHwBar(vramViz, vramPct, vramPct >= 90);
    renderHwMetricSparkline('gpu-vram-spark', gpuHistory.vramPct, vramColor, vramStyle !== 'sparkline');
    if (vramVal) vramVal.textContent = vramGb + ' / ' + vramTotalGb + ' GB';

    // Clocks
    var clocksViz = document.getElementById('gpu-clocks-viz');
    var clocksVal = document.getElementById('gpu-clocks-value');
    var clocksStyle = vizPrefs.gpu.clocks;
    if (clocksStyle === 'ring') {
        renderHwDualRing(clocksViz, m.sclk_mhz, m.mclk_mhz);
        if (clocksVal) clocksVal.textContent = '';
    } else if (clocksStyle === 'chips') {
        renderHwChips(clocksViz, ['SCLK ' + m.sclk_mhz + 'MHz', 'MCLK ' + m.mclk_mhz + 'MHz']);
        if (clocksVal) clocksVal.textContent = '';
    } else {
        if (clocksViz) clocksViz.innerHTML = '';
        if (clocksVal) clocksVal.textContent = m.sclk_mhz + ' / ' + m.mclk_mhz + ' MHz';
    }
}

// Render System card
function renderSystemCard(sys, visible) {
    var card = document.getElementById('system-card');
    var emptyEl = document.getElementById('sys-empty');
    var deviceName = document.getElementById('sys-device-name');
    var tempGauge = document.getElementById('sys-temp-gauge');
    var tempValue = document.getElementById('sys-temp-value');
    var stateChip = document.getElementById('sys-state');

    if (!card || !visible) {
        if (card) setCardState(card, 'dormant');
        return;
    }

    if (!sys) {
        setCardState(card, 'unavailable');
        setEmptyState(emptyEl, true);
        return;
    }

    setEmptyState(emptyEl, false);
    setCardState(card, 'live');
    setChipState(stateChip, 'live', 'live');

    // Device name: CPU model + motherboard
    var parts = [];
    if (sys.cpu_name) parts.push(sys.cpu_name);
    if (sys.motherboard && sys.motherboard !== 'Unknown Motherboard') parts.push(sys.motherboard);
    if (deviceName) deviceName.textContent = parts.join(' / ') || 'System';

    // Temperature
    var hasTemp = sys.cpu_temp_available && sys.cpu_temp > 0;
    var sysTemp = hasTemp ? Math.round(sys.cpu_temp) : 0;
    var tempPct = Math.min(100, (sysTemp / 100) * 100);
    var tempColor = getTempSeverityColor(sysTemp);
    var isTempHot = sysTemp >= 75;
    if (tempGauge) {
        tempGauge.style.setProperty('--pct', hasTemp ? tempPct.toFixed(1) : '0');
        tempGauge.style.setProperty('--gauge-color', tempColor);
        tempGauge.classList.toggle('is-warming', isTempHot);
    }
    if (tempValue) tempValue.textContent = hasTemp ? sysTemp + '\u00B0' : '\u2014';

    // Show temp unavailable badge when connected to remote agent without temp data
    var tempBadge = document.getElementById('sys-temp-unavailable-badge');
    if (tempBadge) {
        var isRemoteAgent = appState.wsData && appState.wsData.endpoint_kind === 'Remote';
        if (!hasTemp && isRemoteAgent) {
            var reason = sys.cpu_temp_available
                ? 'Sensor returned no data'
                : 'sensor_bridge not installed';
            tempBadge.style.display = 'inline-flex';
            tempBadge.title = 'CPU temperature unavailable: ' + reason;
            tempBadge.querySelector('.hw-temp-badge-text').textContent = 'No temp data';
        } else {
            tempBadge.style.display = 'none';
        }
    }

    // Show sensor_bridge setup callout on Windows when temp is unavailable
    var sbSetup = document.getElementById('sensor-bridge-setup-callout');
    var setupAvailable = lastCapabilities && lastCapabilities.sensor_bridge_setup_available;
    if (sbSetup) {
        sbSetup.style.display = (setupAvailable && !hasTemp) ? '' : 'none';
    }

    // Push history
    if (sys.cpu_load > 0) pushSysHistory('cpuLoad', sys.cpu_load);
    var ramPct = sys.ram_total_gb > 0 ? (sys.ram_used_gb / sys.ram_total_gb) * 100 : 0;
    if (sys.ram_total_gb > 0) pushSysHistory('ramPct', ramPct);

    // CPU Load
    var loadViz = document.getElementById('sys-load-viz');
    var loadVal = document.getElementById('sys-load-value');
    var loadStyle = vizPrefs.system.load;
    var cpuLoad = sys.cpu_load || 0;
    var loadHot = cpuLoad >= 90;
    var loadColor = getSeverityColor(cpuLoad);
    if (loadStyle === 'ring') renderHwRing(loadViz, cpuLoad, loadHot);
    else if (loadStyle === 'sparkline') renderHwSparkline(loadViz, sysHistory.cpuLoad);
    else renderHwBar(loadViz, cpuLoad, loadHot);
    renderHwMetricSparkline('sys-load-spark', sysHistory.cpuLoad, loadColor, loadStyle !== 'sparkline');
    if (loadVal) loadVal.textContent = cpuLoad > 0 ? cpuLoad + '%' : '\u2014';

    // RAM
    var ramViz = document.getElementById('sys-ram-viz');
    var ramVal = document.getElementById('sys-ram-value');
    var ramStyle = vizPrefs.system.ram;
    var ramColor = getSeverityColor(ramPct);
    if (ramStyle === 'ring') renderHwRing(ramViz, ramPct, ramPct >= 90);
    else if (ramStyle === 'sparkline') renderHwSparkline(ramViz, sysHistory.ramPct);
    else if (ramStyle === 'stacked') renderHwStacked(ramViz, ramPct);
    else renderHwBar(ramViz, ramPct, ramPct >= 90);
    renderHwMetricSparkline('sys-ram-spark', sysHistory.ramPct, ramColor, ramStyle !== 'sparkline');
    if (ramVal) ramVal.textContent = sys.ram_total_gb > 0 ? sys.ram_used_gb.toFixed(1) + ' / ' + sys.ram_total_gb.toFixed(0) + ' GB' : '\u2014';

    // Clock
    var clockViz = document.getElementById('sys-clock-viz');
    var clockVal = document.getElementById('sys-clock-value');
    var clockStyle = vizPrefs.system.clock;
    var clockMhz = sys.cpu_clock_mhz || 0;
    if (clockMhz > 0) pushSysHistory('cpuClock', clockMhz);
    if (clockStyle === 'ring') {
        renderHwClockRing(clockViz, clockMhz);
        if (clockVal) clockVal.textContent = '';
    } else if (clockStyle === 'chip') {
        var display = formatClockReadout(clockMhz);
        renderHwChips(clockViz, [clockMhz > 0 ? display.value + ' ' + display.unit : '\u2014']);
        if (clockVal) clockVal.textContent = '';
    } else {
        if (clockViz) clockViz.innerHTML = '';
        if (clockVal) clockVal.textContent = clockMhz > 0 ? clockMhz + ' MHz' : '\u2014';
    }
}

function setMetricSectionVisibility(cardId, visible, sectionId) {
    const card = document.getElementById(cardId);
    if (!card) return;
    const section = sectionId ? document.getElementById(sectionId) : card.closest('.metric-section');
    if (section) section.style.display = visible ? '' : 'none';
}

ws.onmessage = e => {

    const d = JSON.parse(e.data);
    appState.wsData = d; // Store for use by status alert and other components

    // Update endpoint health strip
    const endpointModeEl = document.getElementById('endpoint-mode');
    const endpointUrlEl = document.getElementById('endpoint-url');
    const endpointStatusEl = document.getElementById('endpoint-status');
    const agentStatusEl = document.getElementById('agent-status');
    const agentLatencyEl = document.getElementById('agent-latency');

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

// ── Public API ────────────────────────────────────────────────────────────────

export function initDashboardRender() {
    // Put rendering functions on window for dashboard-ws.js
    window.setChipState = setChipState;
    window.setCardState = setCardState;
    window.pushSparklinePoint = pushSparklinePoint;
    window.updateLiveOutputEstimate = updateLiveOutputEstimate;
    window.updateRequestActivity = updateRequestActivity;
    window.renderRecentTask = renderRecentTask;
    window.renderActivityRail = renderActivityRail;
    window.renderSlotGrid = renderSlotGrid;
    window.getPrimarySlot = getPrimarySlot;
    window.renderSlotUtilization = renderSlotUtilization;
    window.renderRequestStats = renderRequestStats;
    window.renderGenerationDetailItems = renderGenerationDetailItems;
    window.renderDecodingConfig = renderDecodingConfig;
    window.renderCapabilityPopover = renderCapabilityPopover;
    window.updateMetricDelta = updateMetricDelta;
    window.setEmptyState = setEmptyState;
    window.renderGpuCard = renderGpuCard;
    window.renderSystemCard = renderSystemCard;
    window.setMetricSectionVisibility = setMetricSectionVisibility;
    window.renderHwBar = renderHwBar;
    window.renderHwRing = renderHwRing;
    window.renderHwSparkline = renderHwSparkline;
    window.renderHwMetricSparkline = renderHwMetricSparkline;
    window.renderHwStacked = renderHwStacked;
    window.renderHwChips = renderHwChips;
    window.renderHwDualRing = renderHwDualRing;
    window.renderHwClockRing = renderHwClockRing;
    window.buildSparklineSVG = buildSparklineSVG;
    window.pushGpuHistory = pushGpuHistory;
    window.pushSysHistory = pushSysHistory;
    window.loadVizPrefs = loadVizPrefs;
    window.saveVizPrefs = saveVizPrefs;
    window.toggleVizSwitcher = toggleVizSwitcher;
    window.selectVizStyle = selectVizStyle;
    window.resetVizPrefs = resetVizPrefs;
    window.getSeverityColor = getSeverityColor;
    window.getTempSeverityColor = getTempSeverityColor;
}
