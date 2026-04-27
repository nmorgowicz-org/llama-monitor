function switchTab(name) {
    const page = document.getElementById('page-' + name);
    if (!page) return;

    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));

    document.querySelectorAll('.sidebar-btn').forEach(b => b.classList.remove('active'));

    page.classList.add('active');

    const sidebarButton = Array.from(document.querySelectorAll('.sidebar-btn'))
        .find(button => button.getAttribute('onclick') === "switchTab('" + name + "')");
    if (sidebarButton) sidebarButton.classList.add('active');

}

function toggleSidebarCollapse() {
    const sidebar = document.getElementById('sidebar-nav');
    const icon = document.querySelector('.sidebar-collapse-icon');
    
    sidebar.classList.toggle('collapsed');
    document.body.classList.toggle('sidebar-collapsed');
    
    const isCollapsed = sidebar.classList.contains('collapsed');
    localStorage.setItem('sidebarCollapsed', isCollapsed.toString());
    
    if (icon) {
        icon.textContent = isCollapsed ? '▶' : '◀';
    }
}

// Restore sidebar state on page load
function restoreSidebarState() {
    const sidebar = document.getElementById('sidebar-nav');
    const icon = document.querySelector('.sidebar-collapse-icon');
    const isCollapsed = localStorage.getItem('sidebarCollapsed') === 'true';
    
    if (isCollapsed) {
        sidebar.classList.add('collapsed');
        document.body.classList.add('sidebar-collapsed');
        if (icon) icon.textContent = '▶';
    }
}

document.addEventListener('DOMContentLoaded', restoreSidebarState);

// Number counting animation for smooth value transitions
function animateNumber(element, from, to, duration = 300, decimals = 1, suffix = '') {
    if (!element) return;
    
    const startTime = performance.now();
    const diff = to - from;
    
    function update(currentTime) {
        const elapsed = currentTime - startTime;
        const progress = Math.min(elapsed / duration, 1);
        
        // Ease-out cubic
        const ease = 1 - Math.pow(1 - progress, 3);
        const current = from + (diff * ease);
        
        element.textContent = current.toFixed(decimals) + suffix;
        
        if (progress < 1) {
            requestAnimationFrame(update);
        }
    }
    
    requestAnimationFrame(update);
}

function formatMetricNumber(value) {
    if (!Number.isFinite(value)) return '0';
    return Math.round(value).toLocaleString();
}

function formatMetricAge(unixMs) {
    if (!unixMs) return 'no recent activity';
    const ageSeconds = Math.max(0, Math.floor((Date.now() - unixMs) / 1000));
    if (ageSeconds < 2) return 'updated just now';
    if (ageSeconds < 60) return 'updated ' + ageSeconds + 's ago';
    const ageMinutes = Math.floor(ageSeconds / 60);
    return 'updated ' + ageMinutes + 'm ago';
}

function escapeHtml(value) {
    return String(value ?? '').replace(/[&<>"']/g, char => ({
        '&': '&amp;',
        '<': '&lt;',
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

    let activeSessionId = 'default';

    let activeSessionPort = 8080;



// --- Settings Persistence (backend) ---



let settingsSaveTimer = null;

// Dirty state tracking for settings modal
let settingsIsDirty = false;

function markSettingsDirty() {
    settingsIsDirty = true;
    const indicator = document.querySelector('#settings-modal .dirty-indicator');
    if (indicator) indicator.classList.add('visible');
}

function clearSettingsDirty() {
    settingsIsDirty = false;
    const indicator = document.querySelector('#settings-modal .dirty-indicator');
    if (indicator) indicator.classList.remove('visible');
}

// Track dirty state on settings input changes
(function() {
    const settingsModal = document.getElementById('settings-modal');
    if (settingsModal) {
        settingsModal.addEventListener('input', markSettingsDirty);
        settingsModal.addEventListener('change', markSettingsDirty);
    }
})();

// Keyboard shortcuts for settings modal
document.addEventListener('keydown', (e) => {
    const modal = document.getElementById('settings-modal');
    if (!modal || !modal.classList.contains('open')) return;

    // Escape to close
    if (e.key === 'Escape') {
        e.preventDefault();
        closeSettingsModal();
    }

    // Cmd+S / Ctrl+S to save
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
        e.preventDefault();
        saveSettings();
    }
});



   function collectSettings() {

        const endpoint = document.getElementById('server-endpoint').value.trim();

        let port = 8001;

        if (endpoint) {

            try {

                const url = new URL(endpoint);

                port = parseInt(url.port) || 8001;

            } catch(e) {

                // invalid URL, use default

            }

        }

        return {

            preset_id: document.getElementById('preset-select').value,

            port: port,

            llama_server_path: document.getElementById('set-server-path').value,

            llama_server_cwd: document.getElementById('set-server-cwd').value,

            models_dir: '',

            server_endpoint: endpoint,

            remote_agent_url: document.getElementById('set-remote-agent-url')?.value.trim() || '',

            remote_agent_token: document.getElementById('set-remote-agent-token')?.value.trim() || '',

            remote_agent_ssh_autostart: !!document.getElementById('set-remote-agent-ssh-autostart')?.checked,

            remote_agent_ssh_target: document.getElementById('set-remote-agent-ssh-target')?.value.trim() || '',

            remote_agent_ssh_command: document.getElementById('set-remote-agent-ssh-command')?.value.trim() || '',

        };

    }



function saveSettings() {

    // Debounce: wait 400ms of inactivity before saving

    clearTimeout(settingsSaveTimer);

    // Ripple effect on save button
    const saveBtn = document.querySelector('#settings-modal .btn-modal-save');
    if (saveBtn) {
        const ripple = document.createElement('span');
        ripple.classList.add('ripple');
        const rect = saveBtn.getBoundingClientRect();
        const size = Math.max(rect.width, rect.height);
        ripple.style.width = ripple.style.height = size + 'px';
        ripple.style.left = (rect.width / 2 - size / 2) + 'px';
        ripple.style.top = (rect.height / 2 - size / 2) + 'px';
        saveBtn.appendChild(ripple);
        setTimeout(() => ripple.remove(), 500);

        // Success flash
        saveBtn.classList.add('success');
        saveBtn.textContent = '✓ Saved';
        setTimeout(() => {
            saveBtn.classList.remove('success');
            saveBtn.textContent = 'Save Settings';
        }, 1200);
    }

    // Clear dirty indicator
    clearSettingsDirty();

    settingsSaveTimer = setTimeout(() => {

        fetch('/api/settings', {

            method: 'PUT',

            headers: { 'Content-Type': 'application/json' },

            body: JSON.stringify(collectSettings()),

        }).catch(() => {});

    }, 400);

}



   function applySettings(s) {

        if (!s) return;

        if (s.port) {
            const portInput = document.getElementById('port');
            if (portInput) portInput.value = s.port;
        }

        if (s.llama_server_path !== undefined) {
            const serverPathInput = document.getElementById('set-server-path');
            if (serverPathInput) serverPathInput.value = s.llama_server_path;
        }

        if (s.llama_server_cwd !== undefined) {
            const serverCwdInput = document.getElementById('set-server-cwd');
            if (serverCwdInput) serverCwdInput.value = s.llama_server_cwd;
        }

        if (s.server_endpoint) {
            const endpointInput = document.getElementById('server-endpoint');
            if (endpointInput && !endpointInput.dataset.preserved) {
                endpointInput.value = s.server_endpoint;
            }
        }

        if (s.remote_agent_url !== undefined) {
            const el = document.getElementById('set-remote-agent-url');
            if (el) el.value = s.remote_agent_url;
        }

        if (s.remote_agent_token !== undefined) {
            const el = document.getElementById('set-remote-agent-token');
            if (el) el.value = s.remote_agent_token;
        }

        if (s.remote_agent_ssh_autostart !== undefined) {
            const el = document.getElementById('set-remote-agent-ssh-autostart');
            if (el) el.checked = !!s.remote_agent_ssh_autostart;
        }

        if (s.remote_agent_ssh_target !== undefined) {
            const el = document.getElementById('set-remote-agent-ssh-target');
            if (el) el.value = s.remote_agent_ssh_target;
        }

        if (s.remote_agent_ssh_command !== undefined) {
            const el = document.getElementById('set-remote-agent-ssh-command');
            if (el) el.value = s.remote_agent_ssh_command;
        }

    }



document.addEventListener('DOMContentLoaded', () => {

loadVizPrefs();

// Auto-save on any control bar change

document.getElementById('controls').addEventListener('input', saveSettings);

document.getElementById('controls').addEventListener('change', saveSettings);

// Do not auto-detect on SSH target input. SSH actions must remain explicit.
const sshTargetInput = document.getElementById('set-remote-agent-ssh-target');

if (sshTargetInput) {

    sshTargetInput.addEventListener('input', () => {

        remoteAgentSshConnection = null;
        clearRemoteAgentValidation();
        setRemoteAgentStatus('SSH target set. Click <strong>Check Host</strong>, <strong>Install & Start</strong>, or <strong>Start Agent</strong> when you are ready.', 'info');

    });

}

const sshGuideAuth = document.getElementById('ssh-guide-auth');
if (sshGuideAuth) {
    sshGuideAuth.addEventListener('change', updateSshGuideAuthFields);
}

const endpointStatus = document.getElementById('endpoint-status');
const endpointStatusWrap = endpointStatus?.closest('.endpoint-status-wrap');
if (endpointStatus && endpointStatusWrap) {
    endpointStatus.addEventListener('click', event => {
        event.stopPropagation();
        const open = endpointStatusWrap.classList.toggle('open');
        endpointStatus.setAttribute('aria-expanded', open ? 'true' : 'false');
    });
    document.addEventListener('click', event => {
        if (!event.target.closest('.endpoint-status-wrap')) {
            endpointStatusWrap.classList.remove('open');
            endpointStatus.setAttribute('aria-expanded', 'false');
        }
    });
}



// Load presets and populate dropdown

async function loadPresets(selectId) {

    const [presetsResp, settingsResp] = await Promise.all([

        fetch('/api/presets'),

        selectId === undefined ? fetch('/api/settings') : Promise.resolve(null),

    ]);

    presets = await presetsResp.json();

    const saved = settingsResp ? await settingsResp.json() : null;



    const sel = document.getElementById('preset-select');

    sel.innerHTML = '';

    presets.forEach(p => {

        const opt = document.createElement('option');

        opt.value = p.id;

        opt.textContent = p.name;

        sel.appendChild(opt);

    });



    const targetId = selectId ?? (saved?.preset_id || null);

    if (targetId && presets.find(p => p.id === targetId)) {

        sel.value = targetId;

    } else if (presets.length > 0) {

        sel.value = presets[0].id;

    }



    if (selectId === undefined && saved) applySettings(saved);

    saveSettings();

}



 // Initial load

    loadPresets();

    loadGpuEnv();

    // LHM auto-check (must be after DOM is ready)
    checkLHMAndPrompt();

});

    loadSessions().then(() => {

        updateActiveSessionInfo();

    });



// --- GPU Environment ---



async function loadGpuEnv() {

    try {

        const resp = await fetch('/api/gpu-env');

        const data = await resp.json();

        const env = data.env;

        const archs = data.architectures;

        const detected = data.detected;



        const sel = document.getElementById('gpu-env-arch');

        sel.innerHTML = '';

        archs.forEach(a => {

            const opt = document.createElement('option');

            opt.value = a.id;

            let label = a.name;

            if (detected && detected.arch === a.id) label += ' (detected)';

            opt.textContent = label;

            sel.appendChild(opt);

        });

        sel.value = env.arch;



        document.getElementById('gpu-env-devices').value = env.devices;

        document.getElementById('gpu-env-rocm-path').value = env.rocm_path || '/opt/rocm';



        const infoEl = document.getElementById('gpu-detected-info');

        const summaryInfo = document.getElementById('gpu-env-info');

        if (detected) {

            const source = detected.arch === 'apple' ? 'local macOS system profile' : detected.arch === 'nvidia' ? 'local nvidia-smi' : 'local rocminfo';

            infoEl.textContent = 'Local detection: ' + detected.count + 'x ' + detected.arch + ' (' + detected.names.join(', ') + ') via ' + source;

            summaryInfo.textContent = '\u2014 ' + detected.count + 'x ' + detected.arch;

        } else {

            infoEl.textContent = 'No local GPU detected via Apple Silicon, rocminfo, or nvidia-smi. Remote hosts need a remote agent.';

            summaryInfo.textContent = '';

        }

    } catch (err) {

        console.error('Failed to load GPU env:', err);

    }

}



// --- Config Modal ---



function openConfigModal() {

    closeSettingsModal();

    document.getElementById('config-modal').classList.add('open');

}



function closeConfigModal() {

    document.getElementById('config-modal').classList.remove('open');

}

// Remote Agent Setup Modal State
let remoteAgentSetupState = {
    sshHost: '',
    sshPort: '22',
    sshAuth: 'agent',
    sshPassword: '',
    sshKeyPath: '',
    latestVersion: null,
    installedVersion: null,
    hostKey: null
};

function openRemoteAgentSetup() {
    closeConfigModal();
    closeSettingsModal();
    
    const modal = document.getElementById('remote-agent-setup-modal');
    if (!modal) return;
    prepareAgentSetupFromEndpoint();
    
    // Get current endpoint URL
    const endpointUrl = document.getElementById('endpoint-url')?.textContent || '';
    const endpointEl = document.getElementById('agent-setup-endpoint-url');
    if (endpointEl) endpointEl.textContent = endpointUrl;
    
    // Infer SSH host from endpoint
    let inferredHost = '';
    try {
        const url = endpointUrl.includes('://') ? endpointUrl : 'http://' + endpointUrl;
        const hostname = new URL(url).hostname;
        inferredHost = hostname;
    } catch (_) {}
    
    // Pre-fill fields
    const sshHostInput = document.getElementById('agent-setup-ssh-host');
    const agentUrlInput = document.getElementById('agent-setup-agent-url');
    if (sshHostInput && !sshHostInput.value && inferredHost) {
        sshHostInput.value = inferredHost;
    }
    if (agentUrlInput && !agentUrlInput.value && inferredHost) {
        agentUrlInput.value = 'http://' + inferredHost + ':7779';
    }
    
    // Reset state
    document.getElementById('agent-setup-host-key')?.style.setProperty('display', 'none');
    document.getElementById('btn-agent-setup-trust')?.style.setProperty('display', 'none');
    document.getElementById('agent-setup-details-section')?.style.setProperty('display', '');
    document.getElementById('agent-setup-install-section')?.style.setProperty('display', '');
    document.getElementById('agent-setup-progress')?.style.setProperty('display', 'none');
    document.getElementById('agent-setup-status')?.style.setProperty('display', 'none');
    document.getElementById('btn-agent-setup-done')?.style.setProperty('display', 'none');
    document.getElementById('btn-agent-setup-install')?.style.setProperty('display', '');
    document.getElementById('btn-agent-setup-start')?.style.setProperty('display', 'none');
    document.getElementById('btn-agent-setup-stop')?.style.setProperty('display', 'none');
    document.getElementById('btn-agent-setup-remove')?.style.setProperty('display', 'none');
    
    // Check latest version
    checkRemoteAgentVersions();

    // Update status alert based on current agent state
    updateAgentSetupStatusAlert();

    modal.classList.add('open');
}

function updateAgentSetupStatusAlert() {
    const alert = document.getElementById('agent-setup-status-alert');
    const icon = document.getElementById('agent-setup-status-alert-icon');
    const title = document.getElementById('agent-setup-status-alert-title');
    const message = document.getElementById('agent-setup-status-alert-message');

    if (!alert) {
        console.log('[Agent] Status alert element NOT found');
        return;
    }

    const state = appState.wsData;
    console.log('[Agent] Status alert update:', { wsData: !!state, state });
    if (!state) {
        alert.style.display = 'none';
        return;
    }

    const isConnected = state.remote_agent_connected;
    const isFirewallBlocked = state.remote_agent_connected && !state.remote_agent_health_reachable;
    const hasRemoteEndpoint = state.session_mode === 'attach' && state.endpoint_kind === 'Remote';
    const sys = state.system || {};
    const hasCpuTemp = sys.cpu_temp_available && sys.cpu_temp > 0;

    if (!hasRemoteEndpoint) {
        alert.style.display = 'flex';
        alert.className = 'agent-setup-status-alert';
        icon.textContent = '\u2139\ufe0f';
        title.textContent = 'No Remote Endpoint';
        message.textContent = 'Configure a remote endpoint in Settings to enable agent management.';
        return;
    }

    if (isFirewallBlocked) {
        alert.style.display = 'flex';
        alert.className = 'agent-setup-status-alert warning';
        icon.textContent = '\u26a0\ufe0f';
        title.textContent = 'Firewall Blocking Agent';
        message.textContent = 'Agent running but HTTP port 7779 unreachable — check Windows Firewall inbound rules.';
        return;
    }

    if (!isConnected) {
        alert.style.display = 'flex';
        alert.className = 'agent-setup-status-alert';
        icon.textContent = '\ud83d\udd27';
        title.textContent = 'Agent Not Connected';
        message.textContent = 'Install or start the agent on the remote host to begin monitoring.';
        return;
    }

    // Connected — check for partial issues
    const issues = [];
    if (!hasCpuTemp) {
        issues.push(sys.cpu_temp_available
            ? 'CPU temp sensor returned no data'
            : 'sensor_bridge not installed (CPU temp unavailable)');
    }

    if (issues.length > 0) {
        alert.style.display = 'flex';
        alert.className = 'agent-setup-status-alert warning';
        icon.textContent = '\u26a0\ufe0f';
        title.textContent = 'Agent Running (with issues)';
        message.textContent = issues.join('. ') + '.';
        return;
    }

    // Fully healthy
    alert.style.display = 'flex';
    alert.className = 'agent-setup-status-alert success';
    icon.textContent = '\u2705';
    title.textContent = 'Agent Running';
    message.textContent = 'Remote agent is connected and reporting all metrics.';
}

function closeRemoteAgentSetup() {
    document.getElementById('remote-agent-setup-modal')?.classList.remove('open');
}

function updateSshSetupAuthFields() {
    const auth = document.getElementById('agent-setup-ssh-auth')?.value || 'agent';
    const passwordRow = document.getElementById('agent-setup-password-row');
    const keyRow = document.getElementById('agent-setup-key-row');
    if (passwordRow) passwordRow.style.display = auth === 'password' ? '' : 'none';
    if (keyRow) keyRow.style.display = auth === 'key' ? '' : 'none';
}

// Bind auth selector
document.addEventListener('DOMContentLoaded', () => {
    const authSelect = document.getElementById('agent-setup-ssh-auth');
    if (authSelect) {
        authSelect.addEventListener('change', updateSshSetupAuthFields);
    }
});

// Sensor bridge setup button handler
document.addEventListener('DOMContentLoaded', () => {
    const btn = document.getElementById('btn-sensor-bridge-setup');
    if (!btn) return;
    btn.addEventListener('click', async () => {
        btn.disabled = true;
        btn.textContent = 'Installing...';
        const callout = document.getElementById('sensor-bridge-setup-callout');
        try {
            const res = await fetch('/api/sensor-bridge/install', { method: 'POST' });
            const data = await res.json();
            if (!data.started) {
                btn.textContent = 'Setup';
                btn.disabled = false;
                if (callout) {
                    callout.innerHTML = '<span style="color:#bf616a;">Install failed: ' + (data.error || 'Unknown error') + '</span>';
                }
                return;
            }
            if (callout) {
                callout.innerHTML = '<span style="color:#a3be8c;">A UAC prompt will appear on your desktop \u2014 approve it to install the sensor service. This takes a few seconds.</span>';
            }
            // Poll for running status up to 30 seconds
            let elapsed = 0;
            const poll = setInterval(async () => {
                elapsed += 2000;
                try {
                    const s = await fetch('/api/sensor-bridge/status');
                    const sd = await s.json();
                    if (sd.running) {
                        clearInterval(poll);
                        if (callout) callout.style.display = 'none';
                    } else if (elapsed >= 30000) {
                        clearInterval(poll);
                        btn.textContent = 'Setup';
                        btn.disabled = false;
                        if (callout) {
                            callout.innerHTML = 'CPU temperature requires a one-time service install. <button id="btn-sensor-bridge-setup" style="margin-left:8px; padding:3px 10px; background:#5e81ac; border:none; border-radius:4px; color:#eceff4; cursor:pointer; font-size:12px;">Setup</button><span style="color:#ebcb8b; margin-left:8px;">Timed out \u2014 did you approve the UAC prompt?</span>';
                            // Re-bind the new button
                            const newBtn = document.getElementById('btn-sensor-bridge-setup');
                            if (newBtn) newBtn.addEventListener('click', () => btn.click());
                        }
                    }
                } catch (_) {}
            }, 2000);
        } catch (e) {
            btn.textContent = 'Setup';
            btn.disabled = false;
        }
    });
});

function collectRemoteAgentSetupConnection() {
    const hostInput = document.getElementById('agent-setup-ssh-host')?.value.trim() || '';
    const portInput = document.getElementById('agent-setup-ssh-port')?.value.trim() || '22';
    const auth = document.getElementById('agent-setup-ssh-auth')?.value || 'agent';
    const host = hostInput.includes('@') ? hostInput.split('@')[1] : hostInput;
    const username = hostInput.includes('@') ? hostInput.split('@')[0] : '';
    const connection = { host, username, port: parseInt(portInput, 10) };
    
    if (auth === 'password') {
        connection.password = document.getElementById('agent-setup-ssh-password')?.value || '';
    } else if (auth === 'key') {
        connection.private_key_path = document.getElementById('agent-setup-ssh-key-path')?.value.trim() || '';
    }
    
    return { auth, connection };
}

function sshTargetFromSetup() {
    const hostInput = document.getElementById('agent-setup-ssh-host')?.value.trim() || '';
    const portInput = document.getElementById('agent-setup-ssh-port')?.value.trim() || '22';
    const userHost = hostInput;
    const port = parseInt(portInput, 10);
    return port && port !== 22 ? 'ssh://' + userHost + ':' + port : userHost;
}

async function scanRemoteAgentHostKey() {
    const { connection } = collectRemoteAgentSetupConnection();
    if (!connection.host) {
        showAgentSetupStatus('Enter an SSH host first.', 'error');
        document.getElementById('agent-setup-ssh-host')?.focus();
        return;
    }
    
    const hostKeyEl = document.getElementById('agent-setup-host-key');
    const trustBtn = document.getElementById('btn-agent-setup-trust');
    if (hostKeyEl) {
        hostKeyEl.style.display = '';
        hostKeyEl.innerHTML = '<em>Scanning host key…</em>';
    }
    if (trustBtn) trustBtn.style.display = 'none';
    
    try {
        const resp = await fetch('/api/remote-agent/ssh/host-key', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                ssh_target: sshTargetFromSetup(),
                ssh_connection: connection
            })
        });
        const data = await resp.json();
        if (!data.ok) {
            remoteAgentSetupState.hostKey = null;
            if (hostKeyEl) hostKeyEl.innerHTML = '<em style="color:#bf616a;">Scan failed: ' + escapeHtml(data.error || 'unknown') + '</em>';
            return;
        }
        
        remoteAgentSetupState.hostKey = data.host_key;
        const trusted = data.host_key.trusted ? 'trusted' : 'not trusted yet';
        if (hostKeyEl) {
            hostKeyEl.innerHTML = [
                '<strong>Key:</strong> ' + escapeHtml(data.host_key.key_type),
                '<strong>Host:</strong> ' + escapeHtml(data.host_key.host + ':' + data.host_key.port),
                '<strong>Fingerprint:</strong> ' + escapeHtml(formatHostKey(data.host_key.key_hex)),
                '<strong>Status:</strong> ' + trusted
            ].join('<br>');
        }
        if (trustBtn) trustBtn.style.display = data.host_key.trusted ? 'none' : '';
        
        if (!data.host_key.trusted) {
            // Auto-advance to details section after scan
            document.getElementById('agent-setup-details-section')?.style.setProperty('display', '');
        }
    } catch (err) {
        remoteAgentSetupState.hostKey = null;
        if (hostKeyEl) hostKeyEl.innerHTML = '<em style="color:#bf616a;">Scan failed: ' + escapeHtml(err.message) + '</em>';
    }
}

async function trustRemoteAgentHostKey() {
    const { connection } = collectRemoteAgentSetupConnection();
    if (!remoteAgentSetupState.hostKey?.key_hex) {
        showAgentSetupStatus('Scan the host key before trusting it.', 'error');
        return;
    }
    
    const resp = await fetch('/api/remote-agent/ssh/trust', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            ssh_target: sshTargetFromSetup(),
            ssh_connection: connection,
            key_hex: remoteAgentSetupState.hostKey.key_hex
        })
    });
    const data = await resp.json();
    if (!data.ok) {
        showAgentSetupStatus('Failed to trust host key: ' + (data.error || 'unknown'), 'error');
        return;
    }
    
    const hostKeyEl = document.getElementById('agent-setup-host-key');
    if (hostKeyEl) {
        hostKeyEl.innerHTML += '<br><strong style="color:#95bc7a;">✓ Trusted for future operations</strong>';
    }
    document.getElementById('btn-agent-setup-trust')?.style.setProperty('display', 'none');
    showAgentSetupStatus('Host key trusted. You can now install and start the agent.', 'ok');
    
    // Auto-advance to install section
    document.getElementById('agent-setup-install-section')?.style.setProperty('display', '');
}

async function checkRemoteAgentVersions() {
    const latestEl = document.getElementById('agent-setup-latest-version');
    const installedEl = document.getElementById('agent-setup-installed-version');
    
    if (latestEl) latestEl.textContent = 'Checking…';
    
    try {
        const resp = await fetch('/api/remote-agent/releases/latest');
        const data = await resp.json();
        if (data.ok && data.release?.tag_name) {
            remoteAgentSetupState.latestVersion = data.release.tag_name;
            if (latestEl) latestEl.textContent = data.release.tag_name;
        } else {
            if (latestEl) latestEl.textContent = 'Unavailable';
        }
    } catch (_) {
        if (latestEl) latestEl.textContent = 'Unavailable';
    }
    
    // Check installed version
    const { connection } = collectRemoteAgentSetupConnection();
    if (!connection.host || !remoteAgentSetupState.hostKey) {
        if (installedEl) installedEl.textContent = '—';
        return;
    }
    
    try {
        const resp = await fetch('/api/remote-agent/detect', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                ssh_target: sshTargetFromSetup(),
                ssh_connection: connection,
                agent_url: document.getElementById('agent-setup-agent-url')?.value.trim() || null
            })
        });
        const data = await resp.json();
        if (data.ok) {
            remoteAgentSetupState.installedVersion = data.installed_version || null;
            if (installedEl) installedEl.textContent = data.installed_version || 'Not installed';
            if (data.installed_version) {
                document.getElementById('btn-agent-setup-install')?.style.setProperty('display', 'none');
                document.getElementById('btn-agent-setup-start')?.style.setProperty('display', data.reachable ? 'none' : '');
                document.getElementById('btn-agent-setup-stop')?.style.setProperty('display', data.reachable ? '' : 'none');
                document.getElementById('btn-agent-setup-remove')?.style.setProperty('display', '');
            }
        } else {
            remoteAgentSetupState.installedVersion = null;
            if (installedEl) installedEl.textContent = 'Not installed';
        }
    } catch (_) {
        remoteAgentSetupState.installedVersion = null;
        if (installedEl) installedEl.textContent = 'Checking…';
    }
}

function showAgentSetupProgress(message, percent) {
    const progressEl = document.getElementById('agent-setup-progress');
    const bar = document.getElementById('agent-setup-progress-bar');
    const text = document.getElementById('agent-setup-progress-text');
    
    progressEl.style.display = '';
    if (bar) bar.style.width = percent + '%';
    if (text) text.textContent = message;
    
    // Auto-scroll to keep progress visible
    setTimeout(() => {
        const statusEl = document.getElementById('agent-setup-status');
        if (statusEl && statusEl.offsetParent !== null) {
            statusEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
        } else if (progressEl && progressEl.offsetParent !== null) {
            progressEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
        }
    }, 100);
}

function hideAgentSetupProgress() {
    document.getElementById('agent-setup-progress')?.style.setProperty('display', 'none');
}

function showAgentSetupStatus(message, kind) {
    const el = document.getElementById('agent-setup-status');
    el.style.display = '';
    el.className = 'agent-setup-status ' + kind;
    el.innerHTML = message;
}

function remoteAgentSetupRequestPayload() {
    const { connection } = collectRemoteAgentSetupConnection();
    return {
        ssh_target: sshTargetFromSetup(),
        ssh_connection: connection,
        agent_url: document.getElementById('agent-setup-agent-url')?.value.trim() || inferredAgentUrl() || null
    };
}

function renderManagedAgentStatus(data) {
    const installedEl = document.getElementById('agent-setup-installed-version');
    if (installedEl) installedEl.textContent = data.installed_version || (data.installed ? 'Unknown' : 'Not installed');

    const installBtn = document.getElementById('btn-agent-setup-install');
    const startBtn = document.getElementById('btn-agent-setup-start');
    const stopBtn = document.getElementById('btn-agent-setup-stop');
    const removeBtn = document.getElementById('btn-agent-setup-remove');

    if (installBtn) {
        installBtn.style.display = data.installed && data.managed_task_matches && !data.update_available ? 'none' : '';
        installBtn.querySelector('.btn-icon')?.replaceChildren(document.createTextNode(data.installed ? '↻' : '⬇'));
    }
    if (startBtn) startBtn.style.display = data.running ? 'none' : '';
    if (stopBtn) stopBtn.style.display = data.running ? '' : 'none';
    if (removeBtn) removeBtn.style.display = data.installed || data.managed_task_installed ? '' : 'none';

    const managedLines = [
        '<strong>Install path:</strong> ' + escapeHtml(data.install_path || 'unknown'),
        '<strong>Installed:</strong> ' + (data.installed ? 'yes' : 'no'),
        '<strong>Running:</strong> ' + (data.running || data.reachable ? 'yes' : 'no'),
    ];
    if (data.installed_version) managedLines.push('<strong>Version:</strong> ' + escapeHtml(data.installed_version));
    if (data.managed_task_name) managedLines.push('<strong>Startup task:</strong> ' + escapeHtml(data.managed_task_name) + (data.managed_task_matches ? ' (healthy)' : ' (needs repair)'));
    if (data.managed_task_command && !data.managed_task_matches) managedLines.push('<strong>Task command:</strong> ' + escapeHtml(data.managed_task_command));
    showAgentSetupStatus(managedLines.join('<br>'), data.running || data.reachable ? 'ok' : 'info');
}

async function checkManagedRemoteAgent() {
    const { connection } = collectRemoteAgentSetupConnection();
    if (!connection.host) {
        prepareAgentSetupFromEndpoint();
    }
    if (!collectRemoteAgentSetupConnection().connection.host) {
        showAgentSetupStatus('Enter an SSH host first.', 'error');
        return { ok: false, error: 'No SSH host' };
    }

    showAgentSetupProgress('Checking managed agent…', 20);
    try {
        const resp = await fetch('/api/remote-agent/status', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(remoteAgentSetupRequestPayload())
        });
        const data = await resp.json();
        hideAgentSetupProgress();
        if (!data.ok) {
            showAgentSetupStatus('Status check failed: ' + escapeHtml(data.error || 'unknown'), 'error');
            return data;
        }
        renderManagedAgentStatus(data);
        return data;
    } catch (err) {
        hideAgentSetupProgress();
        showAgentSetupStatus('Status check failed: ' + escapeHtml(err.message), 'error');
        return { ok: false, error: err.message };
    }
}

async function installRemoteAgent() {
    const { connection } = collectRemoteAgentSetupConnection();
    if (!connection.host) {
        showAgentSetupStatus('Enter an SSH host first.', 'error');
        return;
    }
    
    // Auto-scan host key if not done
    if (!remoteAgentSetupState.hostKey) {
        showAgentSetupProgress('Scanning host key…', 5);
        try {
            const resp = await fetch('/api/remote-agent/ssh/host-key', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    ssh_target: sshTargetFromSetup(),
                    ssh_connection: connection
                })
            });
            const data = await resp.json();
            if (!data.ok) {
                showAgentSetupStatus('Failed to scan host key: ' + (data.error || 'unknown'), 'error');
                return;
            }
            remoteAgentSetupState.hostKey = data.host_key;
        } catch (err) {
            showAgentSetupStatus('Failed to scan host key: ' + err.message, 'error');
            return;
        }
    }
    
    // Auto-trust host key if not trusted
    if (!remoteAgentSetupState.hostKey.trusted) {
        showAgentSetupProgress('Trusting host key…', 10);
        try {
            const resp = await fetch('/api/remote-agent/ssh/trust', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    ssh_target: sshTargetFromSetup(),
                    ssh_connection: connection,
                    key_hex: remoteAgentSetupState.hostKey.key_hex
                })
            });
            const data = await resp.json();
            if (!data.ok) {
                showAgentSetupStatus('Failed to trust host key: ' + (data.error || 'unknown'), 'error');
                return;
            }
            remoteAgentSetupState.hostKey.trusted = true;
        } catch (err) {
            showAgentSetupStatus('Failed to trust host key: ' + err.message, 'error');
            return;
        }
    }
    
    showAgentSetupProgress('Detecting remote OS…', 15);
    
    try {
        // Detect remote OS first
        const detectResp = await fetch('/api/remote-agent/detect', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                ssh_target: sshTargetFromSetup(),
                ssh_connection: connection,
                agent_url: document.getElementById('agent-setup-agent-url')?.value.trim() || null
            })
        });
        const detectData = await detectResp.json();
        if (!detectData.ok) {
            showAgentSetupStatus('Failed to detect remote OS: ' + (detectData.error || 'unknown'), 'error');
            return;
        }
        
        const remoteOs = detectData.os || 'linux';
        const remoteArch = detectData.arch || 'x86_64';
        showAgentSetupProgress('Remote: ' + remoteOs + ' ' + remoteArch + '. Fetching release…', 15);
        
        // Use the matching_asset from detect response (already filtered by OS/arch)
        const asset = detectData.matching_asset;
        if (!asset) {
            showAgentSetupStatus('No compatible asset found for ' + remoteOs + ' ' + remoteArch + ': ' + (detectData.error || ''), 'error');
            return;
        }
        
        showAgentSetupProgress('Downloading ' + asset.name + '…', 20);
        
        const resp = await fetch('/api/remote-agent/install', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                ssh_target: sshTargetFromSetup(),
                ssh_connection: connection,
                asset: asset,
                install_path: detectData.install_path
            })
        });
        
        if (!resp.ok) {
            const text = await resp.text();
            showAgentSetupStatus('Install failed: HTTP ' + resp.status + ' - ' + text, 'error');
            return;
        }
        
        const data = await resp.json();
        if (!data.ok) {
            showAgentSetupStatus('Install failed: ' + (data.error || 'unknown'), 'error');
            return;
        }
        
        remoteAgentSetupState.installedVersion = remoteAgentSetupState.latestVersion;
        document.getElementById('agent-setup-installed-version').textContent = remoteAgentSetupState.installedVersion;
        document.getElementById('btn-agent-setup-install')?.style.setProperty('display', 'none');
        document.getElementById('btn-agent-setup-start')?.style.setProperty('display', '');
        
        showAgentSetupStatus('Agent installed successfully. Starting managed agent…', 'ok');
        await startRemoteAgent();
    } catch (err) {
        showAgentSetupStatus('Install failed: ' + err.message, 'error');
        hideAgentSetupProgress();
    }
}

async function startRemoteAgent() {
    const { connection } = collectRemoteAgentSetupConnection();
    if (!connection.host) {
        showAgentSetupStatus('Enter an SSH host first.', 'error');
        return;
    }
    
    showAgentSetupProgress('Starting agent…', 30);
    
    try {
        const resp = await fetch('/api/remote-agent/start', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                ssh_target: sshTargetFromSetup(),
                ssh_connection: connection
            })
        });
        const data = await resp.json();
        
        if (!data.ok) {
            showAgentSetupStatus('Start failed: ' + (data.error || 'unknown'), 'error');
            hideAgentSetupProgress();
            return;
        }
        
        showAgentSetupProgress('Agent started… verifying…', 80);
        
        // Wait a moment then verify
        await new Promise(r => setTimeout(r, 2000));
        
        const verifyResp = await fetch('/api/remote-agent/detect', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                ssh_target: sshTargetFromSetup(),
                ssh_connection: connection,
                agent_url: document.getElementById('agent-setup-agent-url')?.value.trim() || null
            })
        });
        const verifyData = await verifyResp.json();
        
        hideAgentSetupProgress();
        
        if (verifyData.ok && (verifyData.reachable || data.running)) {
            renderManagedAgentStatus({ ...verifyData, running: true });
            document.getElementById('btn-agent-setup-done').style.display = '';
        } else {
            showAgentSetupStatus('Agent started but verification failed. Check SSH logs.', 'error');
        }
    } catch (err) {
        showAgentSetupStatus('Start failed: ' + err.message, 'error');
        hideAgentSetupProgress();
    }
}

async function stopManagedRemoteAgent() {
    const { connection } = collectRemoteAgentSetupConnection();
    if (!connection.host) {
        prepareAgentSetupFromEndpoint();
    }
    if (!collectRemoteAgentSetupConnection().connection.host) {
        showAgentSetupStatus('Enter an SSH host first.', 'error');
        return;
    }

    showAgentSetupProgress('Stopping managed agent…', 30);
    try {
        const resp = await fetch('/api/remote-agent/stop', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(remoteAgentSetupRequestPayload())
        });
        const data = await resp.json();
        hideAgentSetupProgress();
        if (!data.ok) {
            showAgentSetupStatus('Stop failed: ' + escapeHtml(data.error || 'unknown'), 'error');
            return;
        }
        showAgentSetupStatus('Agent process stopped. The managed startup task remains installed.', 'ok');
        await checkManagedRemoteAgent();
    } catch (err) {
        hideAgentSetupProgress();
        showAgentSetupStatus('Stop failed: ' + escapeHtml(err.message), 'error');
    }
}

async function removeManagedRemoteAgent() {
    const { connection } = collectRemoteAgentSetupConnection();
    if (!connection.host) {
        prepareAgentSetupFromEndpoint();
    }
    if (!collectRemoteAgentSetupConnection().connection.host) {
        showAgentSetupStatus('Enter an SSH host first.', 'error');
        return;
    }

    const confirmed = window.confirm('Remove the managed remote agent from this host? This stops the process, deletes the startup task, and removes the managed binary.');
    if (!confirmed) return;

    showAgentSetupProgress('Removing managed agent…', 30);
    try {
        const resp = await fetch('/api/remote-agent/remove', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(remoteAgentSetupRequestPayload())
        });
        const data = await resp.json();
        hideAgentSetupProgress();
        if (!data.ok) {
            showAgentSetupStatus('Remove failed: ' + escapeHtml(data.error || 'unknown'), 'error');
            return;
        }
        showAgentSetupStatus('Managed agent removed from this host.', 'ok');
        document.getElementById('agent-setup-installed-version').textContent = 'Not installed';
        document.getElementById('btn-agent-setup-install')?.style.setProperty('display', '');
        document.getElementById('btn-agent-setup-start')?.style.setProperty('display', 'none');
        document.getElementById('btn-agent-setup-stop')?.style.setProperty('display', 'none');
        document.getElementById('btn-agent-setup-remove')?.style.setProperty('display', 'none');
    } catch (err) {
        hideAgentSetupProgress();
        showAgentSetupStatus('Remove failed: ' + escapeHtml(err.message), 'error');
    }
}

async function finishRemoteAgentSetup() {
    const agentUrlInput = document.getElementById('agent-setup-agent-url');
    const agentTokenInput = document.getElementById('agent-setup-agent-token');
    const sshHostInput = document.getElementById('agent-setup-ssh-host');
    const sshPortInput = document.getElementById('agent-setup-ssh-port');
    const sshAuthSelect = document.getElementById('agent-setup-ssh-auth');
    
    // Update settings
    const settings = {
        remote_agent_url: agentUrlInput?.value.trim() || '',
        remote_agent_token: agentTokenInput?.value.trim() || '',
        remote_agent_ssh_target: sshTargetFromSetup(),
        remote_agent_ssh_autostart: true
    };
    
    const { auth, connection } = collectRemoteAgentSetupConnection();
    if (auth === 'password') {
        settings.remote_agent_ssh_password = connection.password;
    } else if (auth === 'key') {
        settings.remote_agent_ssh_key_path = connection.private_key_path;
    }
    
    try {
        await fetch('/api/settings', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(settings)
        });
    } catch (_) {}
    
    closeRemoteAgentSetup();
    
    // Refresh the page state
    window.location.reload();
}

function openSettingsModal() {
    const modal = document.getElementById('settings-modal');
    if (!modal) return;
    modal.classList.remove('closing');
    modal.classList.add('open');
    // Reset dirty state on open
    clearSettingsDirty();
}

function closeSettingsModal() {
    const modal = document.getElementById('settings-modal');
    if (!modal) return;
    modal.classList.add('closing');
    setTimeout(() => {
        modal.classList.remove('open', 'closing');
        clearSettingsDirty();
    }, 260);
}

function toggleUserMenu(event) {
    event.preventDefault();
    event.stopPropagation();
    document.querySelector('.nav-user-menu')?.classList.toggle('open');
}

function closeUserMenu() {
    document.querySelector('.nav-user-menu')?.classList.remove('open');
}

document.addEventListener('click', event => {
    if (!event.target.closest('.nav-user')) {
        closeUserMenu();
    }
});

function openUserProfile(event) {
    event?.preventDefault();
    closeUserMenu();
    openUserPreferencesModal();
    showToast('Profile is local-only for now. Preferences are available here.', 'info');
}

function openUserPreferencesModal(event) {
    event?.preventDefault();
    closeUserMenu();
    document.getElementById('user-preferences-modal')?.classList.add('open');
}

function closeUserPreferencesModal() {
    document.getElementById('user-preferences-modal')?.classList.remove('open');
}

function saveUserPreferences() {
    const theme = document.getElementById('pref-theme-mode')?.value || 'dark';
    const fontScale = document.getElementById('pref-font-scale')?.value || '1';
    const spacingScale = document.getElementById('pref-spacing-scale')?.value || '1';

    applyThemePreference(theme);
    document.documentElement.style.fontSize = (Number(fontScale) * 16) + 'px';
    document.documentElement.style.setProperty('--gap-md', (Number(spacingScale) * 16) + 'px');

    localStorage.setItem('llama-monitor-preferences', JSON.stringify({
        theme,
        fontScale,
        spacingScale,
    }));

    closeUserPreferencesModal();
    showToast('Preferences saved', 'success');
}

function applyThemePreference(theme) {
    const effectiveTheme = theme === 'auto'
        ? (window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark')
        : theme;
    document.documentElement.dataset.theme = effectiveTheme;
}

function toggleTheme(event) {
    event?.preventDefault();
    closeUserMenu();
    const current = document.documentElement.dataset.theme === 'light' ? 'light' : 'dark';
    const next = current === 'light' ? 'dark' : 'light';
    document.documentElement.dataset.theme = next;
    const pref = document.getElementById('pref-theme-mode');
    if (pref) pref.value = next;
    showToast('Theme set to ' + next, 'success');
}

function openUserHelp(event) {
    event?.preventDefault();
    closeUserMenu();
    openKeyboardShortcutsModal();
}

function logoutUser(event) {
    event?.preventDefault();
    closeUserMenu();
    showToast('No signed-in account is configured for this local app.', 'info');
}

try {
    const savedPreferences = JSON.parse(localStorage.getItem('llama-monitor-preferences') || 'null');
    if (savedPreferences) {
        applyThemePreference(savedPreferences.theme || 'dark');
        if (savedPreferences.fontScale) {
            document.documentElement.style.fontSize = (Number(savedPreferences.fontScale) * 16) + 'px';
        }
        if (savedPreferences.spacingScale) {
            document.documentElement.style.setProperty('--gap-md', (Number(savedPreferences.spacingScale) * 16) + 'px');
        }
    }
} catch (_) {}

document.querySelectorAll('.settings-tab').forEach(tab => {
    tab.addEventListener('click', () => {
        const target = tab.dataset.tab;
        document.querySelectorAll('.settings-tab').forEach(t => t.classList.remove('active'));
        document.querySelectorAll('.settings-pane').forEach(p => p.classList.remove('active'));
        tab.classList.add('active');
        document.getElementById('settings-' + target)?.classList.add('active');
    });
});

function escapeHtml(value) {
    return String(value ?? '')
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

async function openModelsModal() {
    document.getElementById('models-modal')?.classList.add('open');
    await loadModels();
}

function closeModelsModal() {
    document.getElementById('models-modal')?.classList.remove('open');
}

async function loadModels() {
    const list = document.getElementById('models-list');
    const summary = document.getElementById('models-summary');
    if (!list || !summary) return;

    summary.textContent = 'Loading models...';
    list.innerHTML = '';

    try {
        const resp = await fetch('/api/models');
        const models = await resp.json();
        summary.textContent = models.length ? models.length + ' model' + (models.length === 1 ? '' : 's') + ' found' : 'No models found';
        list.innerHTML = models.length ? models.map(model => {
            const name = model.model_name || model.filename;
            const meta = [
                model.quant_type || 'unknown quant',
                model.size_display || '',
                model.is_split ? 'split model' : ''
            ].filter(Boolean).join(' · ');
            return '<div class="model-item">' +
                '<div><div class="model-name" title="' + escapeHtml(model.path) + '">' + escapeHtml(name) + '</div>' +
                '<div class="model-meta">' + escapeHtml(model.filename) + '</div></div>' +
                '<div class="model-meta">' + escapeHtml(meta) + '</div>' +
                '</div>';
        }).join('') : '<div class="model-item"><div class="model-name">No models discovered</div><div class="model-meta">Configure --models-dir or model paths in presets.</div></div>';
    } catch (err) {
        summary.textContent = 'Failed to load models';
        list.innerHTML = '<div class="model-item"><div class="model-name">Error</div><div class="model-meta">' + escapeHtml(err.message) + '</div></div>';
    }
}

async function refreshModels() {
    const summary = document.getElementById('models-summary');
    if (summary) summary.textContent = 'Refreshing...';
    try {
        const resp = await fetch('/api/models/refresh', { method: 'POST' });
        const data = await resp.json();
        if (!data.ok) showToast('Model refresh failed: ' + (data.error || 'unknown'), 'error');
    } catch (err) {
        showToast('Model refresh failed: ' + err.message, 'error');
    }
    await loadModels();
}



document.getElementById('config-modal').addEventListener('click', e => {

    if (e.target === e.currentTarget) closeConfigModal();

});


document.getElementById('session-modal').addEventListener('click', e => {

    if (e.target === e.currentTarget) closeSessionModal();

});



function saveConfig() {

    // Save server paths via settings

    clearTimeout(settingsSaveTimer);

    fetch('/api/settings', {

        method: 'PUT',

        headers: { 'Content-Type': 'application/json' },

        body: JSON.stringify(collectSettings()),

    }).catch(() => {});



    // Save GPU env

    const env = {

        arch: document.getElementById('gpu-env-arch').value,

        devices: document.getElementById('gpu-env-devices').value.trim(),

        rocm_path: document.getElementById('gpu-env-rocm-path').value.trim() || '/opt/rocm',

        extra_env: [],

    };

    fetch('/api/gpu-env', {

        method: 'PUT',

        headers: { 'Content-Type': 'application/json' },

        body: JSON.stringify(env),

    }).catch(() => {});



    closeConfigModal();

    showToast('Configuration saved', 'success');

}

function usePathServerBinary() {

    const input = document.getElementById('set-server-path');

    if (input) input.value = '';

    showToast('llama-server will be resolved from PATH', 'info');

}

function inferredAgentUrl() {

    const explicit = document.getElementById('set-remote-agent-url')?.value.trim();

    if (explicit) return explicit;

    const endpoint = document.getElementById('server-endpoint')?.value.trim();

    if (!endpoint) return '';

    try {

        const url = new URL(endpoint);

        return url.protocol + '//' + url.hostname + ':7779';

    } catch (_) {

        return '';

    }

}

function remoteEndpointHost() {
    const endpoint = document.getElementById('server-endpoint')?.value.trim();
    if (!endpoint) return '';

    try {
        const url = new URL(endpoint.includes('://') ? endpoint : 'http://' + endpoint);
        return url.hostname || '';
    } catch (_) {
        return '';
    }
}

function inferSshGuideDefaults() {
    const hostInput = document.getElementById('ssh-guide-host');
    const userInput = document.getElementById('ssh-guide-user');
    const portInput = document.getElementById('ssh-guide-port');
    const existingTarget = document.getElementById('set-remote-agent-ssh-target')?.value.trim();
    const endpointHost = remoteEndpointHost();

    if (hostInput && !hostInput.value.trim()) {
        hostInput.value = endpointHost || '127.0.0.1';
    }

    if (portInput && !portInput.value.trim()) {
        portInput.value = '22';
    }

    if (existingTarget && existingTarget.includes('@') && userInput && !userInput.value.trim()) {
        const afterScheme = existingTarget.replace(/^ssh:\/\//, '');
        userInput.value = afterScheme.split('@')[0] || '';
    }
}

function updateSshGuideAuthFields() {
    const auth = document.getElementById('ssh-guide-auth')?.value || 'agent';
    const passwordRow = document.getElementById('ssh-guide-password-row');
    const keyRow = document.getElementById('ssh-guide-key-row');
    const passphraseRow = document.getElementById('ssh-guide-passphrase-row');

    if (passwordRow) passwordRow.style.display = auth === 'password' ? '' : 'none';
    if (keyRow) keyRow.style.display = auth === 'key' ? '' : 'none';
    if (passphraseRow) passphraseRow.style.display = auth === 'key' ? '' : 'none';
}

function openSshSetupGuide() {
    const guide = document.getElementById('ssh-setup-guide');
    if (!guide) return;

    inferSshGuideDefaults();
    updateSshGuideAuthFields();
    previewSshSetupGuide();
    guide.style.display = '';
    guide.scrollIntoView({ behavior: 'smooth', block: 'center' });
}

function closeSshSetupGuide() {
    const guide = document.getElementById('ssh-setup-guide');
    if (guide) guide.style.display = 'none';
}

function collectSshGuideConnection() {
    const host = document.getElementById('ssh-guide-host')?.value.trim() || '';
    const username = document.getElementById('ssh-guide-user')?.value.trim() || '';
    const port = parseInt(document.getElementById('ssh-guide-port')?.value, 10) || 22;
    const auth = document.getElementById('ssh-guide-auth')?.value || 'agent';
    const connection = { host, username, port };

    if (auth === 'password') {
        connection.password = document.getElementById('ssh-guide-password')?.value || '';
    } else if (auth === 'key') {
        connection.private_key_path = document.getElementById('ssh-guide-key-path')?.value.trim() || '';
        connection.private_key_passphrase = document.getElementById('ssh-guide-key-passphrase')?.value || '';
    }

    return { auth, connection };
}

function sshTargetFromConnection(connection) {
    const userHost = connection.username ? connection.username + '@' + connection.host : connection.host;
    return connection.port && connection.port !== 22 ? 'ssh://' + userHost + ':' + connection.port : userHost;
}

function previewSshSetupGuide() {
    const plan = document.getElementById('ssh-guide-plan');
    if (!plan) return;

    const { auth, connection } = collectSshGuideConnection();
    if (!connection.host) {
        plan.textContent = 'Fill in the host details to preview the install/start plan.';
        return;
    }

    const target = sshTargetFromConnection(connection);
    const agentUrl = 'http://' + connection.host + ':7779';
    const authLabel = auth === 'password' ? 'password for this operation' : auth === 'key' ? 'private key file' : 'SSH agent or keychain';

    plan.innerHTML = [
        '<strong>SSH target:</strong> ' + escapeHtml(target),
        '<strong>Agent URL:</strong> ' + escapeHtml(agentUrl),
        '<strong>Auth:</strong> ' + escapeHtml(authLabel),
        '<strong>Install path:</strong> detected by OS; usually ~/.config/llama-monitor/bin/llama-monitor or %APPDATA%\\llama-monitor\\bin\\llama-monitor.exe',
        '<strong>Release source:</strong> latest llama-monitor GitHub release asset matching remote OS/architecture',
        '<strong>Remote command:</strong> default OS-specific agent start command unless you override it below'
    ].join('<br>');
}

function formatHostKey(keyHex) {
    return String(keyHex || '').match(/.{1,2}/g)?.join(':') || '';
}

async function scanSshHostKey() {
    const { connection } = collectSshGuideConnection();
    if (!connection.host) {
        showRemoteAgentValidation('Enter a remote SSH host first.', 'error');
        document.getElementById('ssh-guide-host')?.focus();
        return;
    }

    const hostKeyEl = document.getElementById('ssh-guide-host-key');
    const trustBtn = document.getElementById('btn-ssh-guide-trust');
    if (hostKeyEl) {
        hostKeyEl.style.display = '';
        hostKeyEl.textContent = 'Scanning host key...';
    }
    if (trustBtn) trustBtn.style.display = 'none';

    try {
        const resp = await fetch('/api/remote-agent/ssh/host-key', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                ssh_target: sshTargetFromConnection(connection),
                ssh_connection: connection
            })
        });
        const data = await resp.json();
        if (!data.ok) {
            latestSshHostKey = null;
            if (hostKeyEl) hostKeyEl.textContent = 'Host-key scan failed: ' + (data.error || 'unknown error');
            return;
        }

        latestSshHostKey = data.host_key;
        if (hostKeyEl) {
            hostKeyEl.innerHTML = [
                '<strong>Host key:</strong> ' + escapeHtml(data.host_key.key_type),
                '<strong>Host:</strong> ' + escapeHtml(data.host_key.host + ':' + data.host_key.port),
                '<strong>Fingerprint:</strong> ' + escapeHtml(formatHostKey(data.host_key.key_hex)),
                data.host_key.trusted ? '<strong>Status:</strong> trusted' : '<strong>Status:</strong> not trusted yet'
            ].join('<br>');
        }
        if (trustBtn) trustBtn.style.display = data.host_key.trusted ? 'none' : '';
    } catch (err) {
        latestSshHostKey = null;
        if (hostKeyEl) hostKeyEl.textContent = 'Host-key scan failed: ' + err.message;
    }
}

async function trustSshHostKey() {
    const { connection } = collectSshGuideConnection();
    if (!latestSshHostKey?.key_hex) {
        showRemoteAgentValidation('Scan the host key before trusting it.', 'error');
        return;
    }

    const resp = await fetch('/api/remote-agent/ssh/trust', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            ssh_target: sshTargetFromConnection(connection),
            ssh_connection: connection,
            key_hex: latestSshHostKey.key_hex
        })
    });
    const data = await resp.json();
    if (!data.ok) {
        showRemoteAgentValidation('Failed to trust host key: ' + (data.error || 'unknown error'), 'error');
        return;
    }

    clearRemoteAgentValidation();
    document.getElementById('btn-ssh-guide-trust')?.style.setProperty('display', 'none');
    const hostKeyEl = document.getElementById('ssh-guide-host-key');
    if (hostKeyEl) {
        hostKeyEl.innerHTML += '<br><strong>Status:</strong> trusted for future SSH operations';
    }
    setRemoteAgentStatus('SSH host key trusted. You can now click <strong>Check Host</strong>, <strong>Install & Start</strong>, or <strong>Start Agent</strong>.', 'ok');
}

function applySshSetupGuide() {
    const { connection } = collectSshGuideConnection();

    if (!connection.host) {
        showRemoteAgentValidation('Enter a remote SSH host first.', 'error');
        document.getElementById('ssh-guide-host')?.focus();
        return;
    }

    const target = sshTargetFromConnection(connection);
    const targetInput = document.getElementById('set-remote-agent-ssh-target');
    const agentUrlInput = document.getElementById('set-remote-agent-url');

    if (targetInput) targetInput.value = target;
    if (agentUrlInput && !agentUrlInput.value.trim()) {
        agentUrlInput.value = 'http://' + connection.host + ':7779';
    }

    remoteAgentSshConnection = connection;
    clearRemoteAgentValidation();
    setRemoteAgentStatus('Guided SSH settings are ready. Click <strong>Check Host</strong>, <strong>Install & Start</strong>, or <strong>Start Agent</strong> when you want to contact the remote machine.', 'info');
    saveSettings();
}

function remoteAgentSshPayload() {
    const sshTarget = document.getElementById('set-remote-agent-ssh-target')?.value.trim();
    const payload = { ssh_target: sshTarget };

    if (remoteAgentSshConnection && sshTarget === sshTargetFromConnection(remoteAgentSshConnection)) {
        payload.ssh_connection = remoteAgentSshConnection;
    }

    return payload;
}

function setRemoteAgentStatus(message, kind) {

    const el = document.getElementById('remote-agent-status');

    if (!el) return;

    el.style.color = kind === 'error' ? '#bf616a' : kind === 'ok' ? '#a3be8c' : '#9aa7b7';

    el.innerHTML = message;

}

function showRemoteAgentValidation(message, type) {

    const el = document.getElementById('remote-agent-validation');

    const msgEl = document.getElementById('remote-agent-validation-message');

    if (!el || !msgEl) return;

    el.className = 'remote-agent-validation ' + type;

    msgEl.textContent = message;

    el.style.display = '';

}

function clearRemoteAgentValidation() {

    const el = document.getElementById('remote-agent-validation');

    if (el) el.style.display = 'none';

}

function showRemoteAgentProgress(message, percent, total) {

    const progressEl = document.getElementById('remote-agent-progress');

    if (!progressEl) return;

    const progressBarContainer = document.getElementById('remote-agent-progress-bar-container');
    const progressBar = document.getElementById('remote-agent-progress-bar');
    const progressText = document.getElementById('remote-agent-progress-text');

    progressEl.style.display = '';

    if (progressBarContainer && progressBar && progressText) {
        progressBar.style.width = percent + '%';
        progressText.textContent = message + (total ? ' (' + percent + '%)' : '');
    }

}

function hideRemoteAgentProgress() {

    const progressEl = document.getElementById('remote-agent-progress');

    if (progressEl) progressEl.style.display = 'none';

}

function setRemoteAgentButtonsDisabled(disabled) {

    const detectBtn = document.getElementById('btn-remote-agent-detect');
    const latestBtn = document.getElementById('btn-remote-agent-latest');
    const installBtn = document.getElementById('btn-remote-agent-install');
    const startBtn = document.getElementById('btn-remote-agent-start');
    const updateBtn = document.getElementById('btn-remote-agent-update');
    const stopBtn = document.getElementById('btn-remote-agent-stop');
    const restartBtn = document.getElementById('btn-remote-agent-restart');
    const removeBtn = document.getElementById('btn-remote-agent-remove');

    if (detectBtn) detectBtn.disabled = disabled;
    if (latestBtn) latestBtn.disabled = disabled;
    if (installBtn) installBtn.disabled = disabled;
    if (startBtn) startBtn.disabled = disabled;
    if (updateBtn) updateBtn.disabled = disabled;
    if (stopBtn) stopBtn.disabled = disabled;
    if (restartBtn) restartBtn.disabled = disabled;
    if (removeBtn) removeBtn.disabled = disabled;

    remoteAgentInProgress = disabled;

}

function updateAgentStatusIndicator(connected, firewallBlocked) {

    const el = document.getElementById('agent-status');
    const menuDot = document.getElementById('agent-menu-dot');
    const menuSubtitle = document.getElementById('agent-menu-subtitle');

    if (menuDot) {
        menuDot.className = 'agent-menu-dot' + (connected ? (firewallBlocked ? ' warning' : ' connected') : '');
    }
    if (menuSubtitle) {
        if (firewallBlocked) {
            menuSubtitle.textContent = 'Agent started, HTTP blocked';
        } else if (connected) {
            menuSubtitle.textContent = 'Connected to remote metrics';
        } else {
            const host = remoteEndpointHost();
            menuSubtitle.textContent = host ? 'Manage agent for ' + host : 'No remote endpoint attached';
        }
    }

    if (!el) return;

    if (!connected) {
        el.style.display = 'none';
        return;
    }

    el.style.display = 'flex';
    const fixBtn = el.querySelector('.btn-agent-fix');

    if (firewallBlocked) {
        el.className = 'agent-status firewall-blocked';
        const indicator = el.querySelector('.agent-indicator');
        const textEl = el.querySelector('.agent-text');
        if (indicator) indicator.textContent = '⚠️';
        if (textEl) textEl.textContent = 'Firewall blocked';
        if (fixBtn) fixBtn.style.display = '';
    } else {
        el.className = 'agent-status connected';
        const indicator = el.querySelector('.agent-indicator');
        const textEl = el.querySelector('.agent-text');
        if (indicator) indicator.textContent = '●';
        if (textEl) textEl.textContent = 'Remote Agent';
        if (fixBtn) fixBtn.style.display = 'none';
    }

}

function toggleAgentMenu(event) {
    event.preventDefault();
    event.stopPropagation();
    document.getElementById('agent-menu-panel')?.classList.toggle('open');
}

function toggleAgentMenuFromBadge(event) {
    event.preventDefault();
    event.stopPropagation();
    closeAgentMenu();
    openRemoteAgentSetup();
}

function openRemoteAgentSetupFromBadge(event) {
    event.preventDefault();
    event.stopPropagation();
    // DEBUG: log state when Fix button is clicked
    console.log('[Agent] Fix button CLICKED:', {
        wsData: appState.wsData,
        session_mode: appState.wsData?.session_mode,
        endpoint_kind: appState.wsData?.endpoint_kind,
        remote_agent_connected: appState.wsData?.remote_agent_connected,
        remote_agent_health_reachable: appState.wsData?.remote_agent_health_reachable,
        cpu_temp_available: appState.wsData?.system?.cpu_temp_available,
    });
    closeAgentMenu();
    openRemoteAgentSetup();
}

function closeAgentMenu() {
    document.getElementById('agent-menu-panel')?.classList.remove('open');
}

document.addEventListener('click', event => {
    if (!event.target.closest('.agent-menu')) {
        closeAgentMenu();
    }
});

function prepareAgentSetupFromEndpoint() {
    const host = remoteEndpointHost();
    const sshHostInput = document.getElementById('agent-setup-ssh-host');
    const agentUrlInput = document.getElementById('agent-setup-agent-url');
    const configSshTarget = document.getElementById('set-remote-agent-ssh-target');
    const configAgentUrl = document.getElementById('set-remote-agent-url');

    if (sshHostInput && !sshHostInput.value.trim() && host) sshHostInput.value = host;
    if (agentUrlInput && !agentUrlInput.value.trim() && host) agentUrlInput.value = 'http://' + host + ':7779';
    if (configSshTarget && !configSshTarget.value.trim() && host) configSshTarget.value = host;
    if (configAgentUrl && !configAgentUrl.value.trim() && host) configAgentUrl.value = 'http://' + host + ':7779';
}

async function agentMenuCheck() {
    closeAgentMenu();
    openRemoteAgentSetup();
    await checkManagedRemoteAgent();
}

async function agentMenuInstallRepair() {
    closeAgentMenu();
    openRemoteAgentSetup();
    await installRemoteAgent();
}

async function agentMenuStart() {
    closeAgentMenu();
    openRemoteAgentSetup();
    await startRemoteAgent();
}

async function agentMenuStop() {
    closeAgentMenu();
    openRemoteAgentSetup();
    await stopManagedRemoteAgent();
}

async function agentMenuRemove() {
    closeAgentMenu();
    openRemoteAgentSetup();
    await removeManagedRemoteAgent();
}

function escapeHtml(value) {

    return String(value)

        .replace(/&/g, '&amp;')

        .replace(/</g, '&lt;')

        .replace(/>/g, '&gt;')

        .replace(/"/g, '&quot;')

        .replace(/'/g, '&#39;');

}

async function remoteAgentLatestRelease() {

    showRemoteAgentProgress('Checking latest release...', 100, 100);
    setRemoteAgentButtonsDisabled(true);

    try {

        const resp = await fetch('/api/remote-agent/releases/latest');

        const data = await resp.json();

        if (!data.ok) {

            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);
            setRemoteAgentStatus('Release check failed: ' + escapeHtml(data.error || 'unknown error'), 'error');
            return;

        }

        const assets = (data.release.assets || []).map(asset => escapeHtml(asset.name)).join('<br>');

        const latestEl = document.getElementById('remote-agent-latest-version');
        if (latestEl) latestEl.textContent = data.release.tag_name || 'Unknown';

        setRemoteAgentStatus('<strong>Latest release:</strong> ' + escapeHtml(data.release.tag_name) + '<br>' + assets, 'ok');

        setTimeout(() => {
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);
        }, 500);

    } catch (err) {

        hideRemoteAgentProgress();
        setRemoteAgentButtonsDisabled(false);
        setRemoteAgentStatus('Release check failed: ' + escapeHtml(String(err)), 'error');

    }

}

async function remoteAgentDetect(showProgress = false) {

    const sshTarget = document.getElementById('set-remote-agent-ssh-target')?.value.trim();

    if (!sshTarget) {

        clearRemoteAgentValidation();
        showRemoteAgentValidation('Enter an SSH target first.', 'error');
        document.getElementById('set-remote-agent-ssh-target').focus();
        return { ok: false, error: 'No SSH target' };

    }

    if (showProgress) {
        showRemoteAgentProgress('Detecting remote host...', 0, 100);
    } else {
        setRemoteAgentStatus('Detecting remote host...', 'info');
    }

    try {

        const resp = await fetch('/api/remote-agent/detect', {

            method: 'POST',

            headers: { 'Content-Type': 'application/json' },

            body: JSON.stringify({
                ...remoteAgentSshPayload(),
                agent_url: inferredAgentUrl() || null,
            }),

        });

        const data = await resp.json();

        const asset = data.matching_asset ? data.matching_asset.name : 'No matching asset';

        const archiveNote = data.matching_asset && data.matching_asset.archive ? ' (archive; extract before install)' : '';

        const installPath = data.install_path || 'unknown';

        const lines = [

            '<strong>' + escapeHtml(data.os) + ' / ' + escapeHtml(data.arch) + '</strong>',

            'Asset: ' + escapeHtml(asset) + escapeHtml(archiveNote),

            'Install: ' + escapeHtml(installPath),

            'Installed: ' + (data.installed ? 'yes' : 'no'),

            'Reachable: ' + (data.reachable ? 'yes' : 'no'),

        ];

        if (data.installed_version) {

            lines.push('Installed: v' + escapeHtml(data.installed_version));

        }

        if (data.managed_task_name) {

            lines.push('Startup task: ' + escapeHtml(data.managed_task_name) + (data.managed_task_matches ? ' (healthy)' : ' (needs repair)'));

        }

        if (data.update_available) {

            const latestVer = data.latest_release?.tag_name || 'unknown';

            lines.push('<strong style="color:#ebcb8b;">Update available: v' + escapeHtml(latestVer) + '</strong>');

        }

        if (data.error) lines.push('Issue: ' + escapeHtml(data.error));

        setRemoteAgentStatus(lines.join('<br>'), data.ok ? 'ok' : 'error');

        if (data.ok) {

            updateRemoteAgentPanelState(data);

        }

        clearRemoteAgentValidation();

        if (showProgress) {
            hideRemoteAgentProgress();
        }

        return data;

    } catch (err) {

        setRemoteAgentStatus('Detection failed: ' + escapeHtml(String(err)), 'error');

        if (showProgress) {
            hideRemoteAgentProgress();
        }

        return { ok: false, error: String(err) };

    }

}

async function remoteAgentInstall() {

    const sshTarget = document.getElementById('set-remote-agent-ssh-target')?.value.trim();

    if (!sshTarget) {

        clearRemoteAgentValidation();
        showRemoteAgentValidation('Enter an SSH target first.', 'error');
        document.getElementById('set-remote-agent-ssh-target').focus();
        return;

    }

    setRemoteAgentButtonsDisabled(true);
    clearRemoteAgentValidation();
    addTimelineItem('Installation started', 'pending');
    showRemoteAgentProgress('Detecting and installing agent...', 10, 100);

    try {

        const detectData = await remoteAgentDetect(true);

        if (!detectData.ok || !detectData.matching_asset) {

            addTimelineItem('Detection failed: ' + (detectData.error || 'unknown'), 'failed');
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);

            if (detectData.error) {
                showRemoteAgentValidation('Detection failed: ' + detectData.error, 'error');
            } else {
                setRemoteAgentStatus('Install failed: Detection failed', 'error');
            }

            return;

        }

        const resp = await fetch('/api/remote-agent/install', {

            method: 'POST',

            headers: { 'Content-Type': 'application/json' },

            body: JSON.stringify({
                ...remoteAgentSshPayload(),
                asset: detectData.matching_asset,
                install_path: detectData.install_path,
            }),

        });

        const data = await resp.json();

        if (!data.ok) {

            addTimelineItem('Installation failed: ' + (data.error || 'unknown'), 'failed');
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);
            setRemoteAgentStatus('Install failed: ' + escapeHtml(data.error || 'unknown'), 'error');
            return;

        }

        addTimelineItem('Installation completed', 'completed');
        showRemoteAgentProgress('Agent installed successfully', 100, 100);

        setTimeout(() => {
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);
            setRemoteAgentStatus('Agent installed successfully at ' + escapeHtml(data.install_path || 'unknown'), 'ok');
            updateRemoteAgentPanelState(data);
            remoteAgentStart();
        }, 500);

    } catch (err) {

        addTimelineItem('Installation error: ' + escapeHtml(String(err)), 'failed');
        hideRemoteAgentProgress();
        setRemoteAgentButtonsDisabled(false);
        setRemoteAgentStatus('Install failed: ' + escapeHtml(String(err)), 'error');

    }

}

async function remoteAgentStart() {

    const sshTarget = document.getElementById('set-remote-agent-ssh-target')?.value.trim();

    if (!sshTarget) {

        clearRemoteAgentValidation();
        showRemoteAgentValidation('Enter an SSH target first.', 'error');
        document.getElementById('set-remote-agent-ssh-target').focus();
        return;

    }

    setRemoteAgentButtonsDisabled(true);
    addTimelineItem('Start command sent', 'pending');
    showRemoteAgentProgress('Detecting and starting agent...', 10, 100);

    try {

        const detectData = await remoteAgentDetect(true);

        if (!detectData.ok) {

            addTimelineItem('Detection failed: ' + (detectData.error || 'unknown'), 'failed');
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);

            if (detectData.error) {
                showRemoteAgentValidation('Detection failed: ' + detectData.error, 'error');
            } else {
                setRemoteAgentStatus('Start failed: Detection failed', 'error');
            }

            return;

        }

        const installPath = detectData.install_path || '~/.config/llama-monitor/bin/llama-monitor';
        const startCommand = detectData.start_command || 'nohup ' + installPath + ' --agent --agent-host 0.0.0.0 --agent-port 7779 > ~/.config/llama-monitor/agent.log 2>&1 &';

        const resp = await fetch('/api/remote-agent/start', {

            method: 'POST',

            headers: { 'Content-Type': 'application/json' },

            body: JSON.stringify({
                ...remoteAgentSshPayload(),
                install_path: installPath,
                start_command: startCommand,
            }),

        });

        const data = await resp.json();

        if (!data.ok) {

            addTimelineItem('Start failed: ' + (data.error || 'unknown'), 'failed');
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);
            setRemoteAgentStatus('Start failed: ' + escapeHtml(data.error || 'unknown'), 'error');
            return;

        }

        addTimelineItem('Agent started', 'completed');
        showRemoteAgentProgress('Agent started successfully', 100, 100);

        setTimeout(() => {
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);

            let message = 'Agent started successfully';

            if (data.health_reachable) {

                message += ' and is reachable';

            } else {

                message += ', but HTTP is not reachable (firewall blocked)';

            }

            setRemoteAgentStatus(message, data.health_reachable ? 'ok' : 'warning');

            if (!data.health_reachable) {

                showRemoteAgentFirewall();

            }

            updateRemoteAgentPanelState(data);

        }, 500);

    } catch (err) {

        addTimelineItem('Start error: ' + escapeHtml(String(err)), 'failed');
        hideRemoteAgentProgress();
        setRemoteAgentButtonsDisabled(false);
        setRemoteAgentStatus('Start failed: ' + escapeHtml(String(err)), 'error');

    }

}

async function remoteAgentUpdate() {

    const sshTarget = document.getElementById('set-remote-agent-ssh-target')?.value.trim();

    if (!sshTarget) {

        clearRemoteAgentValidation();
        showRemoteAgentValidation('Enter an SSH target first.', 'error');
        document.getElementById('set-remote-agent-ssh-target').focus();
        return;

    }

    setRemoteAgentButtonsDisabled(true);
    addTimelineItem('Update started', 'pending');
    showRemoteAgentProgress('Stopping and updating agent...', 5, 100);

    try {

        await remoteAgentStop();

        showRemoteAgentProgress('Agent stopped, installing update...', 20, 100);

        await new Promise(resolve => setTimeout(resolve, 1000));

        const detectData = await remoteAgentDetect(true);

        if (!detectData.ok || !detectData.matching_asset) {

            addTimelineItem('Detection failed: ' + (detectData.error || 'unknown'), 'failed');
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);

            if (detectData.error) {
                showRemoteAgentValidation('Detection failed: ' + detectData.error, 'error');
            } else {
                setRemoteAgentStatus('Update failed: Detection failed', 'error');
            }

            return;

        }

        const resp = await fetch('/api/remote-agent/update', {

            method: 'POST',

            headers: { 'Content-Type': 'application/json' },

            body: JSON.stringify({
                ...remoteAgentSshPayload(),
                agent_url: inferredAgentUrl() || null,
            }),

        });

        const data = await resp.json();

        if (!data.ok) {

            addTimelineItem('Update failed: ' + (data.error || 'unknown'), 'failed');
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);
            setRemoteAgentStatus('Update failed: ' + escapeHtml(data.error || 'unknown'), 'error');
            return;

        }

        addTimelineItem('Update completed', 'completed');
        showRemoteAgentProgress('Agent updated successfully', 80, 100);

        setTimeout(() => {
            remoteAgentStart();
        }, 500);

    } catch (err) {

        addTimelineItem('Update error: ' + escapeHtml(String(err)), 'failed');
        hideRemoteAgentProgress();
        setRemoteAgentButtonsDisabled(false);
        setRemoteAgentStatus('Update failed: ' + escapeHtml(String(err)), 'error');

    }

}

async function remoteAgentStop() {

    const sshTarget = document.getElementById('set-remote-agent-ssh-target')?.value.trim();

    if (!sshTarget) {

        clearRemoteAgentValidation();
        showRemoteAgentValidation('Enter an SSH target first.', 'error');
        document.getElementById('set-remote-agent-ssh-target').focus();
        return { ok: false, error: 'No SSH target' };

    }

    setRemoteAgentButtonsDisabled(true);
    addTimelineItem('Stop command sent', 'pending');
    showRemoteAgentProgress('Stopping agent...', 0, 100);

    try {

        const resp = await fetch('/api/remote-agent/stop', {

            method: 'POST',

            headers: { 'Content-Type': 'application/json' },

            body: JSON.stringify(remoteAgentSshPayload()),

        });

        const data = await resp.json();

        if (!data.ok) {

            addTimelineItem('Stop failed: ' + (data.error || 'unknown'), 'failed');
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);
            setRemoteAgentStatus('Stop failed: ' + escapeHtml(data.error || 'unknown'), 'error');
            return data;

        }

        addTimelineItem('Agent stopped', 'completed');
        showRemoteAgentProgress('Agent stopped successfully', 100, 100);

        setTimeout(() => {
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);
            setRemoteAgentStatus('Agent stopped successfully', 'ok');
            updateRemoteAgentPanelState(data);
        }, 500);

        return data;

    } catch (err) {

        addTimelineItem('Stop error: ' + escapeHtml(String(err)), 'failed');
        hideRemoteAgentProgress();
        setRemoteAgentButtonsDisabled(false);
        setRemoteAgentStatus('Stop failed: ' + escapeHtml(String(err)), 'error');
        return { ok: false, error: String(err) };

    }

}

async function remoteAgentRestart() {

    setRemoteAgentButtonsDisabled(true);
    addTimelineItem('Restart started', 'pending');

    const stopResult = await remoteAgentStop();

    if (!stopResult.ok) {
        setRemoteAgentButtonsDisabled(false);
        return;
    }

    addTimelineItem('Restarting agent...', 'pending');

    setTimeout(() => {

        remoteAgentStart();

    }, 1000);

}

async function remoteAgentRemove() {

    const sshTarget = document.getElementById('set-remote-agent-ssh-target')?.value.trim();

    if (!sshTarget) {

        clearRemoteAgentValidation();
        showRemoteAgentValidation('Enter an SSH target first.', 'error');
        document.getElementById('set-remote-agent-ssh-target').focus();
        return;

    }

    if (!window.confirm('Remove the managed remote agent from this host? This stops the process, deletes the startup task, and removes the managed binary.')) {
        return;
    }

    setRemoteAgentButtonsDisabled(true);
    addTimelineItem('Remove command sent', 'pending');
    showRemoteAgentProgress('Removing managed agent...', 0, 100);

    try {

        const resp = await fetch('/api/remote-agent/remove', {

            method: 'POST',

            headers: { 'Content-Type': 'application/json' },

            body: JSON.stringify(remoteAgentSshPayload()),

        });

        const data = await resp.json();

        if (!data.ok) {

            addTimelineItem('Remove failed: ' + (data.error || 'unknown'), 'failed');
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);
            setRemoteAgentStatus('Remove failed: ' + escapeHtml(data.error || 'unknown'), 'error');
            return data;

        }

        addTimelineItem('Managed agent removed', 'completed');
        showRemoteAgentProgress('Managed agent removed', 100, 100);

        setTimeout(() => {
            hideRemoteAgentProgress();
            setRemoteAgentButtonsDisabled(false);
            setRemoteAgentStatus('Managed agent removed from this host.', 'ok');
            updateRemoteAgentPanelState({ installed: false, running: false, managed_task_installed: false });
        }, 500);

        return data;

    } catch (err) {

        addTimelineItem('Remove error: ' + escapeHtml(String(err)), 'failed');
        hideRemoteAgentProgress();
        setRemoteAgentButtonsDisabled(false);
        setRemoteAgentStatus('Remove failed: ' + escapeHtml(String(err)), 'error');
        return { ok: false, error: String(err) };

    }

}

function updateRemoteAgentPanelState(data) {

    const versionsEl = document.getElementById('remote-agent-versions');

    if (!versionsEl) return;

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

    const icons = {
        success: '✓',
        error: '✗',
        warning: '⚠',
        info: 'ℹ'
    };

    return icons[type] || 'ℹ';

}

function updateToastProgress(toastElement, percent, message) {

    if (!toastElement) return;

    const fill = toastElement.querySelector('.toast-progress-fill');

    const content = toastElement.querySelector('.toast-content');

    if (fill) fill.style.width = percent + '%';

    if (content && message) {

        content.innerHTML = '<div class="toast-title">' + escapeHtml(message) + '</div>';

    }

}

function showToastWithActions(title, type, message, actions = []) {

    const toast = showToast(title, type, message);

    if (!toast) return null;

    const toastEl = toast;

    setTimeout(() => {

        const actionsDiv = document.createElement('div');

        actionsDiv.className = 'toast-actions';

        actions.forEach(action => {

            const btn = document.createElement('button');

            btn.className = 'toast-action toast-action-' + (action.primary ? 'primary' : 'secondary');

            btn.textContent = action.label;

            btn.onclick = () => {

                if (action.callback) action.callback();

                toastEl.remove();

            };

            actionsDiv.appendChild(btn);

        });

        toastEl.appendChild(actionsDiv);

    }, 50);

    return toast;

}

function showToastProgress(title, type = 'info') {

    return showToast(title, 'progress');

}



// --- Preset Modal ---



function setVal(id, v) { document.getElementById(id).value = v ?? ''; }

function setChk(id, v) { document.getElementById(id).checked = !!v; }

function setOpt(id, v) { document.getElementById(id).value = v || ''; }

function numOrEmpty(id, v) { document.getElementById(id).value = v != null ? v : ''; }



function clearFieldErrors() {

    document.querySelectorAll('#preset-form .field-error').forEach(el => el.classList.remove('field-error'));

}



function openPresetModal(mode) {

    const modal = document.getElementById('preset-modal');

    const title = document.getElementById('modal-title');

    const form = document.getElementById('preset-form');

    form.reset();

    clearFieldErrors();



    if (mode === 'edit') {

        const id = document.getElementById('preset-select').value;

        const p = presets.find(pr => pr.id === id);

        if (!p) { showToast('No preset selected', 'warn'); return; }

        title.textContent = 'Edit Preset';

        setVal('modal-preset-id', p.id);

        // Model & Memory

        setVal('modal-name', p.name);

        setVal('modal-model-path', p.model_path);

        numOrEmpty('modal-gpu-layers', p.gpu_layers);

        setChk('modal-no-mmap', p.no_mmap);

        setChk('modal-mlock', p.mlock);

        // Context & KV

        setVal('modal-context-size', p.context_size || 128000);

        setVal('modal-ctk', p.ctk || 'q8_0');

        setVal('modal-ctv', p.ctv || 'f16');

        setOpt('modal-flash-attn', p.flash_attn);

        // Batching

        setVal('modal-batch-size', p.batch_size || 2048);

        setVal('modal-ubatch-size', p.ubatch_size || p.batch_size || 2048);

     setVal('modal-parallel-slots', p.parallel_slots || 1);

         // Generation

         numOrEmpty('modal-temperature', p.temperature);

         numOrEmpty('modal-top-p', p.top_p);

         numOrEmpty('modal-top-k', p.top_k);

         numOrEmpty('modal-min-p', p.min_p);

         numOrEmpty('modal-repeat-penalty', p.repeat_penalty);

         numOrEmpty('modal-n-cpu-moe', p.n_cpu_moe);

         // GPU

         setVal('modal-tensor-split', p.tensor_split);

        setOpt('modal-split-mode', p.split_mode);

        numOrEmpty('modal-main-gpu', p.main_gpu);

        // Threading

        numOrEmpty('modal-threads', p.threads);

        numOrEmpty('modal-threads-batch', p.threads_batch);

        // Rope

        setOpt('modal-rope-scaling', p.rope_scaling);

        numOrEmpty('modal-rope-freq-base', p.rope_freq_base);

        numOrEmpty('modal-rope-freq-scale', p.rope_freq_scale);

        // Spec decoding

        setChk('modal-ngram-spec', p.ngram_spec);

        numOrEmpty('modal-spec-ngram-size', p.spec_ngram_size);

        numOrEmpty('modal-draft-min', p.draft_min);

        numOrEmpty('modal-draft-max', p.draft_max);

        setVal('modal-draft-model', p.draft_model);

        // Advanced

        numOrEmpty('modal-seed', p.seed);

        setVal('modal-system-prompt-file', p.system_prompt_file);

        setVal('modal-extra-args', p.extra_args);

    } else {

        title.textContent = 'New Preset';

        setVal('modal-preset-id', '');

        setVal('modal-context-size', 128000);

        setVal('modal-ctk', 'q8_0');

        setVal('modal-ctv', 'f16');

     setVal('modal-batch-size', 2048);

         setVal('modal-ubatch-size', 2048);

         setVal('modal-parallel-slots', 1);

         setVal('modal-temperature', 1.0);

         setVal('modal-top-p', 0.95);

         numOrEmpty('modal-top-k', 40);

         numOrEmpty('modal-min-p', 0.01);

         numOrEmpty('modal-repeat-penalty', 1.0);

         numOrEmpty('modal-n-cpu-moe', 16);

     }



    modal.classList.add('open');

    // Scroll modal body to top

    const body = modal.querySelector('.modal-body');

    if (body) body.scrollTop = 0;

}



function closePresetModal() {

    const modal = document.getElementById('preset-modal');

    modal.classList.remove('open');

}



// Close modal on overlay click

document.getElementById('preset-modal').addEventListener('click', e => {

    if (e.target === e.currentTarget) closePresetModal();

});



// Close modals on Escape key

document.addEventListener('keydown', e => {

    if (e.key === 'Escape' && document.getElementById('config-modal').classList.contains('open')) {

        closeConfigModal();

    } else if (e.key === 'Escape' && document.getElementById('preset-modal').classList.contains('open')) {

        closePresetModal();

    }

});



function intOrNull(id) { const v = document.getElementById(id).value; return v !== '' ? parseInt(v) : null; }

function floatOrNull(id) { const v = document.getElementById(id).value; return v !== '' ? parseFloat(v) : null; }

function strVal(id) { return document.getElementById(id).value.trim(); }



async function savePreset(event) {

    event.preventDefault();

    clearFieldErrors();



    const id = document.getElementById('modal-preset-id').value;

    const preset = {

        // Model & Memory

        name: strVal('modal-name'),

        model_path: strVal('modal-model-path'),

        gpu_layers: intOrNull('modal-gpu-layers'),

        no_mmap: document.getElementById('modal-no-mmap').checked,

        mlock: document.getElementById('modal-mlock').checked,

        // Context & KV

        context_size: parseInt(document.getElementById('modal-context-size').value) || 128000,

        ctk: strVal('modal-ctk') || 'q8_0',

        ctv: strVal('modal-ctv') || 'f16',

        flash_attn: strVal('modal-flash-attn'),

        // Batching

        batch_size: parseInt(document.getElementById('modal-batch-size').value) || 2048,

        ubatch_size: parseInt(document.getElementById('modal-ubatch-size').value) || 2048,

      parallel_slots: parseInt(document.getElementById('modal-parallel-slots').value) || 1,

         // Generation

         temperature: floatOrNull('modal-temperature'),

         top_p: floatOrNull('modal-top-p'),

         top_k: floatOrNull('modal-top-k'),

         min_p: floatOrNull('modal-min-p'),

         repeat_penalty: floatOrNull('modal-repeat-penalty'),

         n_cpu_moe: intOrNull('modal-n-cpu-moe'),

         // GPU

         tensor_split: strVal('modal-tensor-split'),

        split_mode: strVal('modal-split-mode'),

        main_gpu: intOrNull('modal-main-gpu'),

        // Threading

        threads: intOrNull('modal-threads'),

        threads_batch: intOrNull('modal-threads-batch'),

        // Rope

        rope_scaling: strVal('modal-rope-scaling'),

        rope_freq_base: floatOrNull('modal-rope-freq-base'),

        rope_freq_scale: floatOrNull('modal-rope-freq-scale'),

        // Spec decoding

        ngram_spec: document.getElementById('modal-ngram-spec').checked,

        spec_ngram_size: intOrNull('modal-spec-ngram-size'),

        draft_min: intOrNull('modal-draft-min'),

        draft_max: intOrNull('modal-draft-max'),

        draft_model: strVal('modal-draft-model'),

        // Advanced

        seed: intOrNull('modal-seed'),

        system_prompt_file: strVal('modal-system-prompt-file'),

        extra_args: strVal('modal-extra-args'),

    };



    // Inline validation

    let valid = true;

    if (!preset.name) {

        document.getElementById('modal-name').classList.add('field-error');

        valid = false;

    }

    if (!preset.model_path) {

        document.getElementById('modal-model-path').classList.add('field-error');

        valid = false;

    }

    if (!valid) {

        showToast('Please fill in all required fields', 'error');

        return;

    }



    const saveBtn = document.getElementById('btn-modal-save');

    saveBtn.classList.add('saving');

    saveBtn.textContent = 'Saving...';



    try {

        let resp;

        let savedId;

        if (id) {

            resp = await fetch('/api/presets/' + encodeURIComponent(id), {

                method: 'PUT',

                headers: { 'Content-Type': 'application/json' },

                body: JSON.stringify(preset),

            });

            if (!resp.ok) {

                const err = await resp.text().catch(() => 'Unknown error');

                showToast('Save failed: ' + err, 'error');

                return;

            }

            savedId = id;

        } else {

            resp = await fetch('/api/presets', {

                method: 'POST',

                headers: { 'Content-Type': 'application/json' },

                body: JSON.stringify(preset),

            });

            if (!resp.ok) {

                const err = await resp.text().catch(() => 'Unknown error');

                showToast('Save failed: ' + err, 'error');

                return;

            }

            const data = await resp.json();

            savedId = data.id || null;

        }

        closePresetModal();

        await loadPresets(savedId);

        showToast('Preset saved', 'success');

    } catch (err) {

        showToast('Save failed: ' + err.message, 'error');

    } finally {
    }
}


async function copyPreset() {

    const id = document.getElementById('preset-select').value;

    const p = presets.find(pr => pr.id === id);

    if (!p) { showToast('No preset selected', 'warn'); return; }



    const copy = Object.assign({}, p);

    delete copy.id;

    copy.name = p.name + ' (copy)';



    try {

        const resp = await fetch('/api/presets', {

            method: 'POST',

            headers: { 'Content-Type': 'application/json' },

            body: JSON.stringify(copy),

        });

        if (!resp.ok) {

            const err = await resp.text().catch(() => 'Unknown error');

            showToast('Copy failed: ' + err, 'error');

            return;

        }

        const data = await resp.json();

        await loadPresets(data.preset?.id || null);

        showToast('Preset copied', 'success');

    } catch (err) {

        showToast('Copy failed: ' + err.message, 'error');

    }

}



async function deletePreset() {

    const id = document.getElementById('preset-select').value;

    const p = presets.find(pr => pr.id === id);

    if (!p) { showToast('No preset selected', 'warn'); return; }

    if (!confirm('Delete preset "' + p.name + '"?')) return;



    try {

        const resp = await fetch('/api/presets/' + encodeURIComponent(id), { method: 'DELETE' });

        if (!resp.ok) {

            const err = await resp.text().catch(() => 'Unknown error');

            showToast('Delete failed: ' + err, 'error');

            return;

        }

        await loadPresets();

        showToast('Preset deleted', 'success');

    } catch (err) {

        showToast('Delete failed: ' + err.message, 'error');

    }

}



async function resetPresets() {

    if (!confirm('Reset all presets to built-in defaults? Custom presets will be removed.')) return;

    try {

        const resp = await fetch('/api/presets/reset', { method: 'POST' });

        if (!resp.ok) {

            const err = await resp.text().catch(() => 'Unknown error');

            showToast('Reset failed: ' + err, 'error');

            return;

        }

        await loadPresets();

        showToast('Presets reset to defaults', 'success');

    } catch (err) {

        showToast('Reset failed: ' + err.message, 'error');

    }

}



// Clear field errors on input

['modal-name', 'modal-model-path'].forEach(id => {

    document.getElementById(id).addEventListener('input', function() {

        this.classList.remove('field-error');

    });

});



// --- End Preset Modal ---



function getConfig() {

    const id = document.getElementById('preset-select').value;

    const p = presets.find(pr => pr.id === id) || {};

    return {

        model_path: p.model_path || '',

        context_size: p.context_size || 128000,

        ctk: p.ctk || 'q8_0',

        ctv: p.ctv || 'f16',

        tensor_split: p.tensor_split || '',

      batch_size: p.batch_size || 2048,

         ubatch_size: p.ubatch_size || p.batch_size || 2048,

         no_mmap: !!p.no_mmap,

         port: parseInt(document.getElementById('port').value) || 8001,

         ngram_spec: !!p.ngram_spec,

         parallel_slots: p.parallel_slots || 1,

         // Generation

         temperature: p.temperature,

         top_p: p.top_p,

         top_k: p.top_k,

         min_p: p.min_p,

         repeat_penalty: p.repeat_penalty,

         n_cpu_moe: p.n_cpu_moe,

         gpu_layers: p.gpu_layers ?? null,

        mlock: !!p.mlock,

        flash_attn: p.flash_attn || '',

        split_mode: p.split_mode || '',

        main_gpu: p.main_gpu ?? null,

        threads: p.threads ?? null,

        threads_batch: p.threads_batch ?? null,

        rope_scaling: p.rope_scaling || '',

        rope_freq_base: p.rope_freq_base ?? null,

        rope_freq_scale: p.rope_freq_scale ?? null,

        draft_model: p.draft_model || '',

        draft_min: p.draft_min ?? null,

        draft_max: p.draft_max ?? null,

        spec_ngram_size: p.spec_ngram_size ?? null,

        seed: p.seed ?? null,

        system_prompt_file: p.system_prompt_file || '',

        extra_args: p.extra_args || '',

    };

}



async function doStart() {

    const config = getConfig();

    if (!config.model_path) {

        showToast('No model path set. Edit the preset to select a model.', 'error');

        return;

    }

    document.getElementById('btn-start').disabled = true;

    await doKillLlamaInternal();

    const resp = await fetch('/api/start', {

        method: 'POST',

        headers: {'Content-Type': 'application/json'},

        body: JSON.stringify(config),

    });

    const data = await resp.json();

    if (!data.ok) {
        showToast('Start failed: ' + (data.error || 'unknown'), 'error');
        hideConnectingState();
    } else {
        switchView('monitor');
        hideConnectingState();
    }

}



async function doKillLlamaInternal() {

    try {

        await fetch('/api/kill-llama', { method: 'POST' });

    } catch(e) {

        // Ignore errors from kill, just try to continue

    }

}



async function doAttach() {

    const endpointInput = document.getElementById('server-endpoint');

    const endpoint = endpointInput.value.trim();

    if (!endpoint) {

        showToast('Please enter a server endpoint', 'error');

        return;

    }

    const resp = await fetch('/api/attach', {

        method: 'POST',

        headers: {'Content-Type': 'application/json'},

        body: JSON.stringify({ endpoint }),

    });

    const data = await resp.json();

    if (!data.ok) {

        showToast('Attach failed: ' + (data.error || 'unknown'), 'error');
        hideConnectingState();

    } else {

        showToast('Attached to server', 'success');
        hideConnectingState();

        if (data.warning) {

            showToast(data.warning, 'warning');

        }

        // Hide server header immediately
        const serverHeader = document.getElementById('server-header');

        if (serverHeader) {

            serverHeader.style.display = 'none';

        }

        // Reset speed max values for new session
        window.speedMax = { prompt: 0, generation: 0 };

        switchView('monitor');

    }

    updateActiveSessionInfo();

}

async function doDetach() {

    const resp = await fetch('/api/detach', { method: 'POST' });

    const data = await resp.json();

    if (!data.ok) {

        showToast('Detach failed: ' + (data.error || 'unknown'), 'error');

    } else {

        showToast('Detached from server', 'success');
        saveLastSessionData({
            promptRate: window.speedMax.prompt > 0 ? window.speedMax.prompt + ' t/s' : '—',
            genRate: window.speedMax.generation > 0 ? window.speedMax.generation + ' t/s' : '—',
            sessionName: currentSessionId || '—'
        });

        // Immediately update button states without waiting for WebSocket
        const btnAttach = document.getElementById('btn-attach');

        const btnDetach = document.getElementById('btn-detach');

        const btnDetachTop = document.getElementById('btn-detach-top');

        if (btnAttach && btnDetach) {

            btnAttach.style.display = 'inline-block';

            btnDetach.style.display = 'none';

        }

        if (btnDetachTop) {

            btnDetachTop.style.display = 'none';

        }

        // Show server header
        const serverHeader = document.getElementById('server-header');

        if (serverHeader) {

            serverHeader.style.display = '';

        }

        // Show "Historic" badge on inference metrics
        const historicBadge = document.getElementById('inference-historic-badge');

        if (historicBadge) {

            historicBadge.style.display = 'inline-block';

        }

        // Reset speed max values on detach
        window.speedMax = { prompt: 0, generation: 0 };

        switchView('setup');

    }

    updateActiveSessionInfo();

}



async function doKillLlama() {

    if (!confirm('Kill all running llama-server processes?')) return;

    document.getElementById('btn-kill').disabled = true;

    try {

        const resp = await fetch('/api/kill-llama', { method: 'POST' });

        const data = await resp.json();

        if (!data.ok) showToast('Kill failed: ' + (data.error || 'unknown'), 'error');

        else showToast('llama-server killed', 'success');

    } catch (e) {

        showToast('Kill failed: ' + e.message, 'error');

    }

    document.getElementById('btn-kill').disabled = false;

}



async function doStop() {

    document.getElementById('btn-stop').disabled = true;

    await fetch('/api/stop', { method: 'POST' });

    await doKillLlamaInternal();

}



// --- Session Management ---



async function loadSessions() {

    try {

        const resp = await fetch('/api/sessions');

        sessions = await resp.json();

        renderSessionList();

        const lastAttach = sessions
            .filter(s => s.mode && s.mode.Attach)
            .sort((a, b) => b.last_active - a.last_active)[0];

        if (lastAttach) {
            const endpointInput = document.getElementById('server-endpoint');
            if (endpointInput) {
                endpointInput.value = lastAttach.mode.Attach.endpoint;
                endpointInput.dataset.preserved = '1';
                saveSettings();
            }
        }

    } catch (err) {

        console.error('Failed to load sessions:', err);

    }

}



function renderSessionList() {
    const list = document.getElementById('sessions-list');
    const empty = document.getElementById('sessions-empty');
    if (!list) return;

    if (sessions.length === 0) {
        list.innerHTML = '';
        if (empty) empty.style.display = 'block';
        return;
    }
    if (empty) empty.style.display = 'none';

    list.innerHTML = sessions.map(s => {
        const is_active = s.id === activeSessionId;
        const isAttach = s.mode && s.mode.Attach;
        const isSpawn = s.mode && s.mode.Spawn;
        const modeText = isSpawn ? 'Spawn' : 'Attach';
        const modeIcon = isSpawn ? '🖥' : '🔗';
        const endpoint = isAttach ? s.mode.Attach.endpoint : '';
        const port = isSpawn ? s.mode.Spawn.port : '';
        const presetId = s.preset_id || '';
        const presetObj = presets.find(p => p.id === presetId);
        const presetName = presetObj ? presetObj.name : (isSpawn ? '(no preset)' : '');
        const statusText = s.status === 'Running' ? 'Running' :
                           s.status === 'Stopped' ? 'Stopped' :
                           s.status === 'Disconnected' ? 'Disconnected' : (s.status || '');

        return '<div class="session-item' + (is_active ? ' active' : '') + '">' +
            '<div class="session-item-main" onclick="switchSession(\'' + s.id + '\')">' +
            '<span class="session-item-icon">' + modeIcon + '</span>' +
            '<div class="session-item-info">' +
            '<span class="session-item-name">' + s.name + '</span>' +
            '<span class="session-item-detail">' + modeText + (port ? ' : ' + port : '') + (isSpawn && presetName ? ' · ' + presetName : '') + (endpoint ? ' · ' + endpoint : '') + '</span>' +
            '</div>' +
            (statusText ? '<span class="session-item-status">' + statusText + '</span>' : '') +
            '</div>' +
            '<div class="session-item-actions">' +
            (isAttach ? '<button class="btn-sm btn-preset" onclick="event.stopPropagation(); quickAttachSession(\'' + endpoint + '\')">Connect</button>' : '') +
            (isSpawn ? '<button class="btn-sm btn-preset" onclick="event.stopPropagation(); quickStartSession(\'' + s.id + '\')">Start</button>' : '') +
            '<button class="btn-sm btn-preset btn-preset-delete" onclick="event.stopPropagation(); deleteSession(\'' + s.id + '\')">✕</button>' +
            '</div>' +
            '</div>';
    }).join('');
}

function quickAttachSession(endpoint) {
    const serverEndpoint = document.getElementById('server-endpoint');
    if (serverEndpoint) serverEndpoint.value = endpoint;
    localStorage.setItem('llama-monitor-last-endpoint', endpoint);
    closeSessionModal();
    showConnectingState();
    doAttach();
}

function quickStartSession(sessionId) {
    closeSessionModal();
    switchSession(sessionId);
    showConnectingState();
    doStart();
}

async function deleteSession(sessionId) {
    if (!confirm('Delete this session?')) return;
    try {
        const resp = await fetch('/api/sessions/' + encodeURIComponent(sessionId), { method: 'DELETE' });
        const data = await resp.json();
        if (data.ok) {
            showToast('Session deleted', 'success');
            loadSessions();
        } else {
            showToast('Delete failed: ' + (data.error || 'unknown'), 'error');
        }
    } catch (e) {
        showToast('Delete failed: ' + e.message, 'error');
    }
}



async function switchSession(sessionId) {

    try {

        const resp = await fetch('/api/sessions/active', {

            method: 'POST',

            headers: { 'Content-Type': 'application/json' },

            body: JSON.stringify({ id: sessionId })

        });

        const data = await resp.json();

        if (data.ok) {

            activeSessionId = sessionId;

            renderSessionList();

            showToast('Switched to session', 'success');

            // Reload presets for this session

            loadPresets();

        } else {

            showToast('Failed to switch session: ' + data.error, 'error');

        }

    } catch (err) {

        showToast('Failed to switch session: ' + err.message, 'error');

    }

}



function openSessionModal() {
    const modal = document.getElementById('session-modal');
    const title = document.getElementById('session-modal-title');
    title.textContent = 'Sessions';
    modal.classList.add('open');
    showSessionsList();
}

function showNewSessionForm() {
    document.getElementById('sessions-list-view').style.display = 'none';
    document.getElementById('sessions-new-form').style.display = 'block';
    const newBtn = document.getElementById('btn-new-session');
    if (newBtn) newBtn.style.display = 'none';
    document.getElementById('session-form').reset();
    document.getElementById('modal-session-mode').value = 'spawn';
    updateSessionModalMode();
}

function showSessionsList() {
    document.getElementById('sessions-list-view').style.display = 'block';
    document.getElementById('sessions-new-form').style.display = 'none';
    const newBtn = document.getElementById('btn-new-session');
    if (newBtn) newBtn.style.display = 'inline-block';
    renderSessionList();
}

function updateSessionModalMode() {
    const mode = document.getElementById('modal-session-mode')?.value || 'spawn';
    const label = document.getElementById('modal-session-port-label');
    const input = document.getElementById('modal-session-port');
    const spawnFields = document.getElementById('spawn-session-fields');
    if (!label || !input) return;

    if (mode === 'attach') {
        label.textContent = 'Endpoint';
        input.placeholder = 'http://127.0.0.1:8001';
        input.value = document.getElementById('server-endpoint')?.value || '';
        if (spawnFields) spawnFields.style.display = 'none';
    } else {
        label.textContent = 'Port';
        input.placeholder = '8001';
        input.value = activeSessionPort || 8001;
        if (spawnFields) {
            spawnFields.style.display = 'block';
            const presetSelect = document.getElementById('modal-session-preset');
            if (presetSelect) {
                presetSelect.innerHTML = '<option value="">(select a preset)</option>';
                const mainSelect = document.getElementById('preset-select');
                if (mainSelect) {
                    const options = mainSelect.querySelectorAll('option');
                    options.forEach(opt => {
                        if (opt.value) {
                            const clone = document.createElement('option');
                            clone.value = opt.value;
                            clone.textContent = opt.textContent;
                            presetSelect.appendChild(clone);
                        }
                    });
                }
            }
        }
    }
}

document.getElementById('modal-session-mode')?.addEventListener('change', updateSessionModalMode);



function closeSessionModal() {

    document.getElementById('session-modal').classList.remove('open');

}



function saveSession(event) {
    event.preventDefault();

    const mode = document.getElementById('modal-session-mode').value;
    const name = document.getElementById('modal-session-name').value.trim();

    if (!name) {
        showToast('Please enter a session name', 'error');
        return;
    }

    const target = document.getElementById('modal-session-port').value.trim();
    const presetId = document.getElementById('preset-select')?.value;
    const modalPresetId = document.getElementById('modal-session-preset')?.value;
    const endpoint = target || document.getElementById('server-endpoint')?.value.trim();
    const url = mode === 'attach' ? '/api/attach' : '/api/sessions/spawn';
    const payload = mode === 'attach'
        ? { endpoint }
        : {
            name,
            port: parseInt(target, 10) || 8001,
            preset_id: modalPresetId || presetId,
            model_path: (document.getElementById('modal-session-model-path')?.value || '').trim() || undefined,
            gpu_layers: document.getElementById('modal-session-gpu-layers')?.value ? parseInt(document.getElementById('modal-session-gpu-layers').value, 10) : undefined,
            context_size: document.getElementById('modal-session-context-size')?.value ? parseInt(document.getElementById('modal-session-context-size').value, 10) : undefined,
            no_mmap: document.getElementById('modal-session-no-mmap')?.checked || undefined,
            mlock: document.getElementById('modal-session-mlock')?.checked || undefined,
          };

    if (mode === 'attach' && !endpoint) {
        showToast('Please enter an endpoint', 'error');
        return;
    }

    if (mode === 'spawn' && !modalPresetId && !presetId) {
        showToast('Select a model preset before creating a spawn session', 'error');
        return;
    }

    fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
    })
    .then(r => r.json())
    .then(data => {
        if (data.ok) {
            closeSessionModal();
            loadSessions();
            updateActiveSessionInfo();
            showToast(mode === 'attach' ? 'Attached to endpoint' : 'Session created', 'success');
        } else {
            showToast('Failed to create session: ' + data.error, 'error');
        }
    })
    .catch(err => showToast('Failed to create session: ' + err.message, 'error'));
}



// WebSocket

const ws = new WebSocket((location.protocol === 'https:' ? 'wss://' : 'ws://') + location.host + '/ws');

// Initialize button states on page load
async function initAttachDetachButtons() {
    try {
        const resp = await fetch('/api/sessions/active');
        const data = await resp.json();
        const btnAttach = document.getElementById('btn-attach');
        const btnDetach = document.getElementById('btn-detach');
        if (data && data.mode && data.mode.startsWith('Attach:') && btnAttach && btnDetach) {
            btnAttach.style.display = 'none';
            btnDetach.style.display = 'inline-block';
        } else if (btnAttach && btnDetach) {
            btnAttach.style.display = 'inline-block';
            btnDetach.style.display = 'none';
        }
    } catch (err) {
        console.error('Failed to initialize attach/detach buttons:', err);
    }
}
initAttachDetachButtons();



async function updateActiveSessionInfo() {

    try {

        const resp = await fetch('/api/sessions/active');

        const data = await resp.json();

        if (data && data.mode) {

            const modeParts = data.mode.split(':');

            if (modeParts[0] === 'Spawn') {

                activeSessionPort = parseInt(modeParts[1]) || 8080;

            } else if (modeParts[0] === 'Attach') {

                const endpoint = modeParts.slice(1).join(':');

                try {

                    const url = new URL(endpoint);

                    activeSessionPort = parseInt(url.port) || 8080;

                } catch(e) {

                    activeSessionPort = 8080;

                }

                const endpointInput = document.getElementById('server-endpoint');

                if (endpointInput && endpointInput.value !== endpoint) {

                    endpointInput.value = endpoint;

                    saveSettings();

                }

            }

        }

    } catch (err) {

        console.error('Failed to update active session info:', err);

    }

}



setInterval(updateActiveSessionInfo, 2000);

/* ===== Hardware Card Rendering ===== */

// Severity color helper
function getSeverityColor(pct) {
    if (pct >= 95) return '#f43f5e';
    if (pct >= 80) return '#f59e0b';
    return '#10b981';
}

function getTempSeverityColor(temp) {
    if (temp >= 90) return '#f43f5e';
    if (temp >= 75) return '#f59e0b';
    return '#8fbcbb';
}

// Visualization rendering helpers
function setVizContent(container, html) {
    if (!container) return;
    container.innerHTML = html;
}

function swapVizContent(container, html) {
    if (!container) return;
    container.classList.add('viz-fade-out');
    setTimeout(function() {
        container.innerHTML = html;
        container.classList.remove('viz-fade-out');
        container.classList.add('viz-fade-in');
        setTimeout(function() { container.classList.remove('viz-fade-in'); }, 160);
    }, 120);
}

function renderHwBar(container, pct, isHot) {
    if (!container) return;
    const bgCls = isHot ? 'hw-bar-bg is-hot' : 'hw-bar-bg';
    setVizContent(container, '<div class="' + bgCls + '"><div class="hw-bar-fill" style="width:' + pct.toFixed(1) + '%;--bar-start:' + getSeverityColor(pct) + ';--bar-end:' + getSeverityColor(Math.min(pct + 15, 100)) + '"></div></div>');
}

function renderHwRing(container, pct, isHot) {
    if (!container) return;
    const cls = isHot ? 'hw-ring-viz is-warming' : 'hw-ring-viz';
    setVizContent(container, '<div class="' + cls + '" style="--pct:' + pct.toFixed(1) + ';--gauge-color:' + getSeverityColor(pct) + '"></div>');
}

function renderHwSparkline(container, history) {
    if (!container || !history || history.length < 2) {
        setVizContent(container, '');
        return;
    }
    const svg = buildSparklineSVG(history, 'hw-sparkline', '#8fbcbb');
    setVizContent(container, svg);
}

// Render inline sparkline below a bar metric (into existing SVG element)
function renderHwMetricSparkline(svgId, history, color, show) {
    const svg = document.getElementById(svgId);
    if (!svg) return;
    if (!show || !history || history.length < 2) {
        svg.style.visibility = (show && history && history.length >= 2) ? '' : 'hidden';
        return;
    }
    svg.style.visibility = '';
    const width = 120;
    const height = 28;
    const max = Math.max(...history, 1);
    const min = Math.min(...history, 0);
    const range = Math.max(max - min, 1);
    const step = width / (history.length - 1);
    const peakValue = Math.max(...history);
    const peakIndex = history.lastIndexOf(peakValue);
    const peakX = peakIndex * step;
    const peakY = height - (((peakValue - min) / range) * (height - 4)) - 2;
    const path = history.map((value, index) => {
        const x = index * step;
        const y = height - (((value - min) / range) * (height - 4)) - 2;
        return (index === 0 ? 'M' : 'L') + x.toFixed(2) + ' ' + y.toFixed(2);
    }).join(' ');
    svg.innerHTML =
        '<path class="sparkline-fill" d="' + path + ' L 120 28 L 0 28 Z" fill="' + color + '" opacity="0.16"></path>' +
        '<path class="sparkline-line" d="' + path + '" stroke="' + color + '" fill="none" stroke-width="2.4" vector-effect="non-scaling-stroke" stroke-linecap="round" stroke-linejoin="round" filter="drop-shadow(0 0 4px ' + color + ')"></path>' +
        '<circle class="sparkline-peak" cx="' + peakX.toFixed(2) + '" cy="' + peakY.toFixed(2) + '" r="2.3" fill="' + color + '" opacity="0.9"></circle>';
}

function renderHwStacked(container, pct) {
    if (!container) return;
    const isHot = pct >= 90;
    const bgCls = isHot ? 'hw-stacked-bg is-hot' : 'hw-stacked-bg';
    setVizContent(container, '<div class="' + bgCls + '"><div class="hw-stacked-fill" style="width:' + pct.toFixed(1) + '%;--bar-start:' + getSeverityColor(pct) + ';--bar-end:' + getSeverityColor(Math.min(pct + 15, 100)) + '"></div><div class="hw-stacked-free" style="width:' + (100 - pct).toFixed(1) + '%"></div></div>');
}

function renderHwChips(container, chips) {
    if (!container) return;
    setVizContent(container, '<div class="hw-chips">' + chips.map(function(c) { return '<span class="hw-chip">' + c + '</span>'; }).join('') + '</div>');
}

// Render dual-ring gauge (GPU clocks: SCLK inner, MCLK outer)
function formatClockReadout(mhz) {
    if (!Number.isFinite(mhz) || mhz <= 0) {
        return { value: '\u2014', unit: 'MHz', detail: '\u2014' };
    }
    if (mhz >= 1000) {
        var ghz = mhz >= 10000 ? (mhz / 1000).toFixed(1) : (mhz / 1000).toFixed(2);
        return { value: ghz, unit: 'GHz', detail: mhz + ' MHz' };
    }
    return { value: String(mhz), unit: 'MHz', detail: mhz + ' MHz' };
}

function computeClockBand(history, current) {
    var points = (history || []).filter(Number.isFinite);
    if (Number.isFinite(current) && current > 0) points.push(current);
    if (points.length === 0) {
        return { min: 0, max: 0, pct: 0, peakPct: 0, lowPct: 0 };
    }
    var min = Math.min.apply(null, points);
    var max = Math.max.apply(null, points);
    var span = Math.max(max - min, 1);
    var normalized = function(value) {
        if (!Number.isFinite(value)) return 0;
        return Math.max(0, Math.min(100, ((value - min) / span) * 100));
    };
    var pct = span <= 1 ? 100 : normalized(current);
    return {
        min: min,
        max: max,
        pct: pct,
        peakPct: normalized(max),
        lowPct: normalized(min)
    };
}

function renderHwDualRing(container, sclk, mclk) {
    if (!container) return;
    var sclkBand = computeClockBand(gpuHistory.sclk, sclk);
    var mclkBand = computeClockBand(gpuHistory.mclk, mclk);
    var sclkColor = getSeverityColor(sclkBand.pct);
    var mclkColor = '#60a5fa';
    var sclkPulse = (3.4 - Math.min(sclkBand.pct, 100) * 0.014).toFixed(2) + 's';
    var mclkPulse = (3.8 - Math.min(mclkBand.pct, 100) * 0.016).toFixed(2) + 's';
    setVizContent(container,
        '<div class="hw-clock-gpu-layout">' +
          '<div class="hw-clock-cluster hw-clock-gpu">' +
            '<div class="hw-clock-orbit outer" style="--pct:' + mclkBand.pct.toFixed(1) + ';--peak-pct:' + mclkBand.peakPct.toFixed(1) + ';--low-pct:' + mclkBand.lowPct.toFixed(1) + ';--orbit-color:' + mclkColor + ';--dot-radius:-78px;--pulse-duration:' + mclkPulse + ';">' +
              '<div class="hw-clock-orbit-track"></div>' +
              '<div class="hw-clock-orbit-fill"></div>' +
              '<div class="hw-clock-orbit-peak"></div>' +
              '<div class="hw-clock-orbit-low"></div>' +
              '<div class="hw-clock-orbit-dot"></div>' +
            '</div>' +
            '<div class="hw-clock-orbit inner" style="--pct:' + sclkBand.pct.toFixed(1) + ';--peak-pct:' + sclkBand.peakPct.toFixed(1) + ';--low-pct:' + sclkBand.lowPct.toFixed(1) + ';--orbit-color:' + sclkColor + ';--dot-radius:-55px;--pulse-duration:' + sclkPulse + ';">' +
              '<div class="hw-clock-orbit-track"></div>' +
              '<div class="hw-clock-orbit-fill"></div>' +
              '<div class="hw-clock-orbit-peak"></div>' +
              '<div class="hw-clock-orbit-low"></div>' +
              '<div class="hw-clock-orbit-dot"></div>' +
            '</div>' +
            '<div class="hw-clock-core">' +
              '<div class="hw-clock-unit">GPU</div>' +
              '<div class="hw-clock-band">Clocks</div>' +
            '</div>' +
          '</div>' +
          '<div class="hw-clock-gpu-readout">' +
            '<div class="hw-clock-meter">' +
              '<div class="hw-clock-meter-label">SCLK</div>' +
              '<div class="hw-clock-meter-bar" style="--pct:' + sclkBand.pct.toFixed(1) + ';--peak-pct:' + sclkBand.peakPct.toFixed(1) + ';--low-pct:' + sclkBand.lowPct.toFixed(1) + ';">' +
                '<div class="hw-clock-meter-fill" style="--pct:' + sclkBand.pct.toFixed(1) + ';--meter-color:' + sclkColor + ';--pulse-duration:' + sclkPulse + ';"></div>' +
                '<div class="hw-clock-meter-marker"></div>' +
                '<div class="hw-clock-meter-marker-low"></div>' +
              '</div>' +
              '<div class="hw-clock-meter-value">' + formatClockReadout(sclk).value + ' ' + formatClockReadout(sclk).unit + '</div>' +
              '<div class="hw-clock-meter-band">' + sclkBand.min + '-' + sclkBand.max + '</div>' +
            '</div>' +
            '<div class="hw-clock-meter">' +
              '<div class="hw-clock-meter-label">MCLK</div>' +
              '<div class="hw-clock-meter-bar" style="--pct:' + mclkBand.pct.toFixed(1) + ';--peak-pct:' + mclkBand.peakPct.toFixed(1) + ';--low-pct:' + mclkBand.lowPct.toFixed(1) + ';">' +
                '<div class="hw-clock-meter-fill" style="--pct:' + mclkBand.pct.toFixed(1) + ';--meter-color:' + mclkColor + ';--pulse-duration:' + mclkPulse + ';"></div>' +
                '<div class="hw-clock-meter-marker"></div>' +
                '<div class="hw-clock-meter-marker-low"></div>' +
              '</div>' +
              '<div class="hw-clock-meter-value">' + formatClockReadout(mclk).value + ' ' + formatClockReadout(mclk).unit + '</div>' +
              '<div class="hw-clock-meter-band">' + mclkBand.min + '-' + mclkBand.max + '</div>' +
            '</div>' +
          '</div>' +
        '</div>');
}

// Render single-ring gauge (System clock)
function renderHwClockRing(container, clock) {
    if (!container) return;
    var band = computeClockBand(sysHistory.cpuClock, clock);
    var display = formatClockReadout(clock);
    var color = getSeverityColor(band.pct);
    var pulse = (3.6 - Math.min(band.pct, 100) * 0.016).toFixed(2) + 's';
    var footerSpark = sysHistory.cpuClock.length > 1
        ? '<div class="hw-clock-footer sparkline-only"><div class="hw-clock-footer-spark">' + buildSparklineSVG(sysHistory.cpuClock, 'hw-clock-footer-spark', color) + '</div></div>'
        : '';
    setVizContent(container,
        '<div class="hw-clock-system-layout">' +
          '<div class="hw-clock-cluster hw-clock-system">' +
            '<div class="hw-clock-orbit outer" style="--pct:' + band.pct.toFixed(1) + ';--peak-pct:' + band.peakPct.toFixed(1) + ';--low-pct:' + band.lowPct.toFixed(1) + ';--orbit-color:' + color + ';--dot-radius:-61px;--pulse-duration:' + pulse + ';">' +
              '<div class="hw-clock-orbit-track"></div>' +
              '<div class="hw-clock-orbit-fill"></div>' +
              '<div class="hw-clock-orbit-peak"></div>' +
              '<div class="hw-clock-orbit-low"></div>' +
              '<div class="hw-clock-orbit-dot"></div>' +
            '</div>' +
            '<div class="hw-clock-core">' +
              '<div class="hw-clock-stack">' +
                '<div class="hw-clock-row"><span class="hw-clock-row-value">' + display.value + '</span><span class="hw-clock-unit">' + display.unit + '</span></div>' +
                '<div class="hw-clock-band">' + band.min + '-' + band.max + ' MHz</div>' +
              '</div>' +
            '</div>' +
          '</div>' +
          '<div class="hw-clock-meter">' +
            '<div class="hw-clock-meter-label">CLOCK</div>' +
            '<div class="hw-clock-meter-bar" style="--pct:' + band.pct.toFixed(1) + ';--peak-pct:' + band.peakPct.toFixed(1) + ';--low-pct:' + band.lowPct.toFixed(1) + ';">' +
              '<div class="hw-clock-meter-fill" style="--pct:' + band.pct.toFixed(1) + ';--meter-color:' + color + ';--pulse-duration:' + pulse + ';"></div>' +
              '<div class="hw-clock-meter-marker"></div>' +
              '<div class="hw-clock-meter-marker-low"></div>' +
            '</div>' +
            '<div class="hw-clock-meter-value">' + clock + '</div>' +
            '<div class="hw-clock-meter-band">' + band.min + '-' + band.max + '</div>' +
          '</div>' +
        '</div>' +
        footerSpark);
}

// Build sparkline SVG (reuses inference card pattern)
function buildSparklineSVG(points, cssClass, color) {
    var len = points.length;
    if (len < 2) return '';
    var w = 120, h = 24, pad = 2;
    var max = Math.max.apply(null, points);
    var min = Math.min.apply(null, points);
    var range = max - min || 1;
    var step = w / (len - 1);
    var pts = points.map(function(v, i) { return i * step + ',' + (h - pad - ((v - min) / range) * (h - pad * 2)); });
    var linePath = 'M' + pts.join(' L');
    var fillPath = linePath + ' L' + (len - 1) * step + ',' + h + ' L0,' + h + ' Z';
    var peakIdx = points.indexOf(max);
    var peakX = peakIdx * step;
    var peakY = h - pad - ((max - min) / range) * (h - pad * 2);
    return '<svg class="metric-sparkline ' + cssClass + '" viewBox="0 0 ' + w + ' ' + h + '" preserveAspectRatio="none" aria-hidden="true">' +
        '<defs><linearGradient id="hw-spark-grad-' + cssClass + '" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stop-color="' + color + '" stop-opacity="0.25"/><stop offset="100%" stop-color="' + color + '" stop-opacity="0.02"/></linearGradient></defs>' +
        '<path d="' + fillPath + '" fill="url(#hw-spark-grad-' + cssClass + ')"/>' +
        '<path d="' + linePath + '" fill="none" stroke="' + color + '" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>' +
        (len > 3 ? '<circle cx="' + peakX.toFixed(1) + '" cy="' + peakY.toFixed(1) + '" r="2" fill="' + color + '" opacity="0.8"/>' : '') +
        '</svg>';
}

// GPU metric history ring buffers
var gpuHistory = { load: [], power: [], vramPct: [], sclk: [], mclk: [] };
function pushGpuHistory(key, value) {
    if (!Number.isFinite(value)) return;
    gpuHistory[key].push(value);
    var limit = key === 'load' || key === 'power' || key === 'vramPct' ? 60 : 30;
    if (gpuHistory[key].length > limit) gpuHistory[key].shift();
}

// System metric history ring buffers
var sysHistory = { cpuLoad: [], ramPct: [], cpuClock: [] };
function pushSysHistory(key, value) {
    if (!Number.isFinite(value)) return;
    sysHistory[key].push(value);
    var limit = 60;
    if (sysHistory[key].length > limit) sysHistory[key].shift();
}

// Visualization preferences
var vizPrefs = {
    gpu: { load: 'bar', power: 'bar', vram: 'bar', clocks: 'ring' },
    system: { load: 'bar', ram: 'bar', clock: 'ring' }
};

function loadVizPrefs() {
    try {
        var gpuStr = localStorage.getItem('llama-monitor-gpu-viz');
        if (gpuStr) vizPrefs.gpu = JSON.parse(gpuStr);
        var sysStr = localStorage.getItem('llama-monitor-system-viz');
        if (sysStr) vizPrefs.system = JSON.parse(sysStr);
    } catch(e) {}
}

function saveVizPrefs(card) {
    try {
        var key = card === 'gpu' ? 'llama-monitor-gpu-viz' : 'llama-monitor-system-viz';
        localStorage.setItem(key, JSON.stringify(vizPrefs[card]));
    } catch(e) {}
}

function toggleVizSwitcher(card) {
    var prefix = card === 'gpu' ? 'gpu' : 'sys';
    var sw = document.getElementById(prefix + '-viz-switcher');
    if (!sw) return;
    var isOpen = sw.style.display !== 'none';
    // Close all switchers
    document.querySelectorAll('.viz-switcher').forEach(function(el) { el.style.display = 'none'; });
    if (!isOpen) {
        sw.style.display = 'flex';
        // Position relative to card
        var cardEl = document.getElementById(card === 'gpu' ? 'gpu-card' : 'system-card');
        if (cardEl) {
            var rect = cardEl.getBoundingClientRect();
            var parentRect = cardEl.parentElement.getBoundingClientRect();
            sw.style.top = (rect.height + 8) + 'px';
            sw.style.right = '0';
        }
    }
}

function selectVizStyle(card, metric, style) {
    vizPrefs[card][metric] = style;
    saveVizPrefs(card);
    // Update active state in switcher
    var prefix = card === 'gpu' ? 'gpu' : 'sys';
    var sw = document.getElementById(prefix + '-viz-switcher');
    if (sw) {
        sw.querySelectorAll('.viz-option').forEach(function(btn) {
            btn.classList.toggle('active', btn.getAttribute('data-style') === style && btn.closest('.viz-switcher-options').getAttribute('data-metric') === metric);
        });
    }
    // Fade out viz containers, then re-render
    var cardEl = document.getElementById(card === 'gpu' ? 'gpu-card' : 'system-card');
    if (cardEl) {
        cardEl.querySelectorAll('.hw-metric-viz').forEach(function(el) { el.classList.add('viz-fade-out'); });
        setTimeout(function() {
            if (card === 'gpu') renderGpuCard(lastGpuData || {}, !!lastGpuData && Object.keys(lastGpuData).length > 0);
            else renderSystemCard(lastSystemMetrics, !!lastSystemMetrics);
            cardEl.querySelectorAll('.hw-metric-viz').forEach(function(el) {
                el.classList.remove('viz-fade-out');
                el.classList.add('viz-fade-in');
                setTimeout(function() { el.classList.remove('viz-fade-in'); }, 160);
            });
        }, 120);
    }
}

function resetVizPrefs(card) {
    vizPrefs[card] = card === 'gpu'
        ? { load: 'bar', power: 'bar', vram: 'bar', clocks: 'chips' }
        : { load: 'bar', ram: 'bar', clock: 'chip' };
    saveVizPrefs(card);
    var prefix = card === 'gpu' ? 'gpu' : 'sys';
    var sw = document.getElementById(prefix + '-viz-switcher');
    if (sw) {
        sw.querySelectorAll('.viz-option').forEach(function(btn) {
            btn.classList.remove('active');
        });
        sw.querySelectorAll('.viz-option[data-style="bar"], .viz-option[data-style="chips"], .viz-option[data-style="chip"]').forEach(function(btn) {
            btn.classList.add('active');
        });
    }
    // Fade out viz containers, then re-render
    var cardEl = document.getElementById(card === 'gpu' ? 'gpu-card' : 'system-card');
    if (cardEl) {
        cardEl.querySelectorAll('.hw-metric-viz').forEach(function(el) { el.classList.add('viz-fade-out'); });
        setTimeout(function() {
            if (card === 'gpu') renderGpuCard(lastGpuData || {}, !!lastGpuData && Object.keys(lastGpuData).length > 0);
            else renderSystemCard(lastSystemMetrics, !!lastSystemMetrics);
            cardEl.querySelectorAll('.hw-metric-viz').forEach(function(el) {
                el.classList.remove('viz-fade-out');
                el.classList.add('viz-fade-in');
                setTimeout(function() { el.classList.remove('viz-fade-in'); }, 160);
            });
        }, 120);
    }
}

// Close switchers on outside click
document.addEventListener('click', function(e) {
    if (!e.target.closest('.viz-switcher') && !e.target.closest('.viz-gear-btn')) {
        document.querySelectorAll('.viz-switcher').forEach(function(el) { el.style.display = 'none'; });
    }
});

// Persist last GPU data for re-render after style switch
var lastGpuData = {};

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

    if (agentStatusEl) {
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
            // Only show Fix button when agent is NOT connected AND we have a remote endpoint configured
            // (i.e., there's something to fix). Don't show it when everything is working.
            const hasRemoteEndpoint = d.session_mode === 'attach' && d.endpoint_kind === 'Remote';
            const needsFix = hasRemoteEndpoint && (!d.remote_agent_connected || firewallBlocked);
            fixBtn.style.display = needsFix ? '' : 'none';
            fixBtn.title = firewallBlocked ? 'Repair remote agent connectivity' : 'Set up remote agent';
            // DEBUG: log Fix button state changes
            if (needsFix) {
                console.log('[Agent] Fix button SHOWN:', {
                    session_mode: d.session_mode,
                    endpoint_kind: d.endpoint_kind,
                    remote_agent_connected: d.remote_agent_connected,
                    remote_agent_health_reachable: d.remote_agent_health_reachable,
                    firewallBlocked,
                    hasRemoteEndpoint,
                    needsFix,
                    cpu_temp_available: d.system ? d.system.cpu_temp_available : 'N/A',
                });
            }
        }
        
        if (d.remote_agent_connected && !remoteAgentHealthReachable) {
            setRemoteAgentStatus('Agent connected but HTTP is not reachable (firewall blocked)', 'warning');
        }
        
        if (agentLatencyEl) {
            agentLatencyEl.textContent = '';
        }
    }

    // Update Attach/Detach button states and server header visibility based on session mode
    const serverHeader = document.getElementById('server-header');
    const btnAttach = document.getElementById('btn-attach');
    const btnDetach = document.getElementById('btn-detach');
    const btnDetachTop = document.getElementById('btn-detach-top');

    const isAttach = d.session_mode === 'attach' && d.active_session_endpoint;

    if (isAttach) {
        // Attached state: hide server header, show detach buttons
        if (serverHeader) serverHeader.style.display = 'none';
        btnAttach.style.display = 'none';
        btnDetach.style.display = 'inline-block';
        if (btnDetachTop) btnDetachTop.style.display = 'inline-block';

        // Switch to monitor view if not already there
        if (appState.view === 'setup') {
            hideConnectingState();
            switchView('monitor');
        }
    } else {
        // Not attached: show server header, hide detach buttons
        if (serverHeader) serverHeader.style.display = '';
        btnAttach.style.display = 'inline-block';
        btnDetach.style.display = 'none';
        if (btnDetachTop) btnDetachTop.style.display = 'none';
    }

    // Update "Historic" badge on inference metrics section
    const historicBadge = document.getElementById('inference-historic-badge');
    if (historicBadge) {
        historicBadge.style.display = isAttach ? 'none' : 'inline-block';
    }



    // Server state

    serverRunning = d.server_running;

    const dot = document.getElementById('status-dot');

    const txt = document.getElementById('status-text');

    dot.className = 'status-dot ' + (serverRunning ? 'running' : 'stopped');

    txt.textContent = serverRunning ? 'Running' : 'Stopped';

    const btnStart = document.getElementById('btn-start');

    const btnStop = document.getElementById('btn-stop');

    // Use local_server_running for Start/Stop buttons (independent of remote endpoint)
    const localRunning = d.local_server_running || false;

    if (btnStart) {

        btnStart.disabled = localRunning;

    }

    if (btnStop) {

        btnStop.disabled = !localRunning;

    }



    lastServerState = d.server_running;

    lastLlamaMetrics = d.llama;

    lastSystemMetrics = d.system || null;

    lastCapabilities = d.capabilities || null;

    lastGpuMetrics = d.gpu || {};



    // Inference metrics

    const l = lastLlamaMetrics;

    // Helper for availability-aware empty state
    function getEmptyStateMessage(reason, fallback) {
        if (reason === 'RemoteEndpoint') {
            return 'Host metrics unavailable for remote endpoint';
        }
        if (reason === 'SensorUnavailable') {
            return 'Temperature sensor unavailable';
        }
        if (reason === 'BackendUnavailable') {
            return 'GPU metrics unavailable';
        }
        return fallback || '\u2014';
    }

    const gpuReason = d.availability?.gpu || 'Available';
    const cpuTempReason = d.availability?.cpu_temp || 'Available';
    const systemReason = d.availability?.system || 'Available';

    // Speed metrics with adaptive bars (track max values seen)
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
    const hasActiveEndpoint = !!d.active_session_id;

    const promptRate = l?.prompt_tokens_per_sec || 0;
    const genRate = l?.generation_tokens_per_sec || 0;
    const promptDisplayRate = promptRate > 0 ? promptRate : (l?.last_prompt_tokens_per_sec || 0);
    const genDisplayRate = genRate > 0 ? genRate : (l?.last_generation_tokens_per_sec || 0);
    const promptAgeMs = l?.last_prompt_throughput_unix_ms || 0;
    const genAgeMs = l?.last_generation_throughput_unix_ms || 0;
    const latestThroughputMs = Math.max(promptAgeMs, genAgeMs);
    const throughputActive = promptRate > 0 || genRate > 0;

    setCardState(throughputCard, !hasActiveEndpoint ? 'dormant' : throughputActive ? 'live' : 'idle');
    setEmptyState(document.getElementById('m-throughput-empty'), !hasActiveEndpoint);
    setChipState(throughputState, throughputActive ? 'live' : 'idle', throughputActive ? 'live' : 'idle');

    if (throughputAge) {
        throughputAge.textContent = formatMetricAge(latestThroughputMs);
    }

    if (promptDisplayRate > 0) {
        updateMetricDelta(promptDeltaEl, window.prevValues.prompt, promptDisplayRate, 1);
        animateNumber(promptEl, window.prevValues.prompt, promptDisplayRate, 300, 1, ' t/s');
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

    if (genDisplayRate > 0) {
        updateMetricDelta(genDeltaEl, window.prevValues.generation, genDisplayRate, 1);
        animateNumber(genEl, window.prevValues.generation, genDisplayRate, 300, 1, ' t/s');
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

    pushSparklinePoint('prompt', promptDisplayRate);
    pushSparklinePoint('generation', genDisplayRate);
    renderSparkline('m-prompt-spark', window.metricSeries.prompt, 'prompt', false);
    renderSparkline('m-gen-spark', window.metricSeries.generation, 'generation', false);

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

    // Generation progress from /slots next_token metadata
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
    renderDecodingConfig(l, hasActiveEndpoint);
    renderLiveSparkline('m-live-output-spark', window.metricSeries.liveOutput);

    setCardState(generationCard, !hasActiveEndpoint ? 'dormant' : generationActive ? 'live' : generationAvailable ? 'idle' : 'unavailable');
    setEmptyState(document.getElementById('m-generation-empty'), !hasActiveEndpoint);
    setChipState(generationState, generationActive ? 'generating' : 'idle', generationActive ? 'live' : 'idle');
    setChipState(document.getElementById('m-slots-state'), generationActive ? 'active' : 'idle', generationActive ? 'live' : 'idle');
    setChipState(document.getElementById('m-activity-state'), generationActive ? 'active' : 'idle', generationActive ? 'live' : 'idle');
    if (generationRing) generationRing.style.setProperty('--progress', generationPct.toFixed(2));
    if (liveVelocity) {
        liveVelocity.textContent = liveOutputRate > 0 ? liveOutputRate.toFixed(1) + ' t/s' : (generationActive ? 'warming' : 'retained');
    }
    if (promptStage && outputStage) {
        // Use throughput as proxy for phase detection when next_token data isn't available
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

    // Context metrics with progress bar
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

    setEmptyState(document.getElementById('m-context-empty'), !hasActiveEndpoint);
    if (ctxPeakFill) ctxPeakFill.style.width = peakPct + '%';
    if (ctxPeakDetail) ctxPeakDetail.textContent = contextPeak > 0 ? formatMetricNumber(contextPeak) + ' peak' : '\u2014';

    if (l && contextCapacity > 0 && contextLiveAvailable) {
        const pct = ((contextLive / contextCapacity) * 100);
        const severity = pct >= 95 ? 'critical' : pct >= 80 ? 'warning' : '';
        setCardState(contextCard, severity === 'critical' ? 'live' : 'idle');
        setChipState(ctxState, 'live', severity || 'live');
        if (ctxLiveLabel) ctxLiveLabel.textContent = 'Live usage';
        if (ctxLiveDetail) ctxLiveDetail.textContent = formatMetricNumber(contextLive) + ' live';

        animateNumber(ctxValue, window.prevValues.contextPct, pct, 300, 1, '%');
        window.prevValues.contextPct = pct;
        
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
        if (ctxValue) ctxValue.textContent = getEmptyStateMessage(systemReason, '\u2014');
        if (ctxDetails) ctxDetails.textContent = '';
    }

    renderCapabilityPopover(d, l, generationAvailable, contextLiveAvailable);

    const hostMetricsVisible = d.host_metrics_available === true;
    const systemVisible = hostMetricsVisible && !!d.capabilities?.system;
    const gpuVisible = hostMetricsVisible && !!d.capabilities?.gpu;
    setMetricSectionVisibility('gpu-card', gpuVisible, 'gpu-section');
    setMetricSectionVisibility('system-card', systemVisible, 'system-section');

    // GPU card rendering
    renderGpuCard(d.gpu || {}, gpuVisible);

    // System card rendering
    renderSystemCard(lastSystemMetrics, systemVisible);


    // Logs

    const logs = d.logs || [];

    if (logs.length !== prevLogLen) {

        const el = document.getElementById('log-panel');

        const wasAtBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;

        el.textContent = logs.join('\n');

        if (wasAtBottom) el.scrollTop = el.scrollHeight;

        prevLogLen = logs.length;

    }



    // Tab badges - only show live metrics when session is active

    const badgeParts = [];

    const isAttached = d.session_mode === 'attach' && d.active_session_endpoint;

    if (isAttached) {

        const gpuEntries = Object.entries(d.gpu || {});

        if (gpuEntries.length > 0) badgeParts.push('GPU ' + Math.max(...gpuEntries.map(([,m]) => m.temp)).toFixed(0) + 'C');

    }

    document.getElementById('badge-server').textContent = badgeParts.length ? ' ' + badgeParts.join(' \u00b7 ') : '';

    const badgeChat = document.getElementById('badge-chat');

    if (chatHistory.length > 0) {

        badgeChat.textContent = ' ' + chatHistory.length + ' msg';

        badgeChat.style.display = '';

    } else {

        badgeChat.textContent = '';

        badgeChat.style.display = 'none';

    }

    const badgeLogs = document.getElementById('badge-logs');

    if (logs.length > 0) {

        badgeLogs.textContent = ' ' + logs.length;

        badgeLogs.style.display = '';

    } else {

        badgeLogs.textContent = '';

        badgeLogs.style.display = 'none';

    }

};

ws.onerror = e => console.error('WebSocket error:', e);

ws.onclose = () => { 

    document.getElementById('status-text').textContent = 'Disconnected'; 

    prevLogLen = 0;

};



// Markdown

if (typeof marked !== 'undefined') {

    marked.setOptions({ breaks: true, gfm: true });

}

function renderMd(src) {

    if (typeof marked !== 'undefined') {

        try { return marked.parse(src); } catch(_) {}

    }

    return src.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/\n/g,'<br>');

}



// Chat

let chatHistory = [];

let chatBusy = false;
document.getElementById('chat-input').addEventListener('keydown', e => {

    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendChat(); }

});



function clearChat() {

    chatHistory = [];

    document.getElementById('chat-messages').innerHTML = '';

}



function chatScroll() {

    const c = document.getElementById('chat-messages');

    c.scrollTop = c.scrollHeight;

}



function appendMsg(role, text) {

    const el = document.createElement('div');

    el.className = 'msg msg-' + role;

    el.textContent = text;

    document.getElementById('chat-messages').appendChild(el);

    chatScroll();

    return el;

}



async function sendChat() {

    if (chatBusy) return;

    const input = document.getElementById('chat-input');

    const text = input.value.trim();

    if (!text) return;

    input.value = '';



    chatHistory.push({ role: 'user', content: text });

    appendMsg('user', text);



    const url = '/api/sessions/active';



    chatBusy = true;

    document.getElementById('btn-send').disabled = true;



    let thinkEl = null;

    let thinkContent = '';

    const msgEl = appendMsg('assistant', '');

    let msgContent = '';



    try {

        const resp = await fetch(url, {

            method: 'GET',

            headers: { 'Content-Type': 'application/json' },

        });

             const sessionData = await resp.json();

        if (sessionData.error) {

            throw new Error(sessionData.error);

        }

        // Parse mode to get port (format: "Spawn:8001" or "Attach:http://...:8001")

        const modeParts = sessionData.mode.split(':');

        if (modeParts[0] === 'Spawn') {

            activeSessionPort = parseInt(modeParts[1]) || 8080;

        } else if (modeParts[0] === 'Attach') {

            // Extract port from endpoint URL

            try {

                const url = new URL(modeParts[1]);

                activeSessionPort = parseInt(url.port) || 8080;

            } catch(e) {

                // ignore, use default

            }

        }

        const chatUrl = '/api/chat'; // Port derived server-side from active session

        

        const chatResp = await fetch(chatUrl, {

            method: 'POST',

            headers: { 'Content-Type': 'application/json' },

            body: JSON.stringify({

                messages: chatHistory,

                stream: true,

                temperature: 1.0,

                top_p: 0.95,

                top_k: 40,

                min_p: 0.01,

                repeat_penalty: 1.0,

            }),

        });



        const reader = chatResp.body.getReader();

        const decoder = new TextDecoder();

        let buf = '';



        while (true) {

            const { done, value } = await reader.read();

            if (done) break;

            buf += decoder.decode(value, { stream: true });



            const lines = buf.split('\n');

            buf = lines.pop() || '';



            for (const line of lines) {

                if (!line.startsWith('data: ')) continue;

                const payload = line.slice(6).trim();

                if (payload === '[DONE]') continue;

                try {

                    const obj = JSON.parse(payload);

                    const delta = obj.choices && obj.choices[0] && obj.choices[0].delta;

                    if (!delta) continue;



                    // Reasoning / thinking content

                    const rc = delta.reasoning_content || '';

                    if (rc) {

                        thinkContent += rc;

                        if (!thinkEl) {

                            thinkEl = document.createElement('details');

                            thinkEl.className = 'msg msg-thinking';

                            thinkEl.innerHTML = '<summary>thinking...</summary><span></span>';

                            document.getElementById('chat-messages').insertBefore(thinkEl, msgEl);

                        }

                        thinkEl.querySelector('span').textContent = thinkContent;

                    }



                    // Regular content

                    const c = delta.content || '';

                    if (c) {

                        msgContent += c;

                        msgEl.innerHTML = renderMd(msgContent);

                    }

                } catch (_) {}

            }

            chatScroll();

        }

    } catch (err) {

        msgEl.textContent = '[error] ' + err.message;

        msgEl.style.color = '#bf616a';

    }



    if (msgContent) {

        chatHistory.push({ role: 'assistant', content: msgContent });

    }

    chatBusy = false;

    document.getElementById('btn-send').disabled = false;

}

if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js').catch(() => {});
}

// Show the LHM notification (triggered by user clicking the LHM button or first load)
async function showLHMNotification() {
    return new Promise(async (resolve) => {
        // Store resolve in window for inline onclick handlers
        window.lhmResolve = resolve;
        const overlay = document.createElement('div');
        overlay.className = 'notification-container';
        overlay.style.cssText = `
            position: fixed;
            top: 20px;
            right: 20px;
            width: 400px;
            background: #2e3440;
            border: 2px solid #ebcb8b;
            border-radius: 8px;
            box-shadow: 0 10px 30px rgba(0,0,0,0.5);
            z-index: 9999;
            padding: 20px;
            color: #d8dee9;
            animation: fadeIn 0.3s ease-out;
        `;
        
        overlay.innerHTML = `
            <div style="display:flex;justify-content:space-between;align-items:center;">
                <h3 style="margin:0 0 10px 0;color:#ebcb8b;">LibreHardwareMonitor Status</h3>
                <button onclick="this.closest('.notification-container').remove(); window.lhmResolve('cancel');" style="background:none;border:none;color:#d8dee9;cursor:pointer;font-size:20px;">&times;</button>
            </div>
            <p id="lhm-status-text" style="margin:0 0 15px 0;line-height:1.5;">Checking status...</p>
            <div id="lhm-buttons" style="display:flex;gap:10px;flex-direction:column;"></div>
        `;
        
        document.body.appendChild(overlay);
        
        // Check LHM status
        const lhmStatusEl = document.getElementById('lhm-status-text');
        const lhmButtonsEl = document.getElementById('lhm-buttons');
        
        try {
            const [statusResp, checkResp] = await Promise.all([
                fetch('/api/lhm/status').catch(() => null),
                fetch('/api/lhm/check').catch(() => null)
            ]);
            
            let isDisabled = false;
            let lhmAvailable = false;
            let lhmInstalled = false;
            
            if (statusResp && statusResp.ok) {
                const statusData = await statusResp.json();
                isDisabled = statusData.disabled || false;
            }
            
            if (checkResp && checkResp.ok) {
                const checkData = await checkResp.json();
                lhmAvailable = checkData.running || false;
                lhmInstalled = checkData.installed || false;
            }
            
            if (isDisabled) {
                lhmStatusEl.textContent = 'LibreHardwareMonitor is disabled. Enable it to monitor CPU temperatures.';
                lhmButtonsEl.innerHTML = `
                    <button id="btn-lhm-enable" style="flex:1;padding:10px;background:#a3be8c;border:none;border-radius:4px;cursor:pointer;font-weight:bold;">Enable Monitoring</button>
                `;
                lhmButtonsEl.querySelector('#btn-lhm-enable').onclick = async () => {
                    overlay.remove();
                    try {
                        const disableResp = await fetch('/api/lhm/disable', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ disabled: false })
                        });
                        if (disableResp.ok) {
                            showToast('LHM monitoring enabled', 'success');
                            setTimeout(() => location.reload(), 1500);
                        }
                    } catch (err) {
                        showToast('Failed to enable LHM: ' + err.message, 'error');
                    }
                };
            } else if (lhmAvailable) {
                // LHM is running
                lhmStatusEl.textContent = 'LibreHardwareMonitor is running. CPU temperature monitoring is active.';
                lhmButtonsEl.innerHTML = `
                    <button id="btn-lhm-uninstall" style="flex:1;padding:10px;background:#bf616a;border:none;border-radius:4px;cursor:pointer;font-weight:bold;">Uninstall LHM</button>
                `;
                lhmButtonsEl.querySelector('#btn-lhm-uninstall').onclick = async () => {
                    overlay.remove();
                    const uninstallConfirm = confirm('Are you sure you want to uninstall LibreHardwareMonitor? This will disable CPU temperature monitoring.');
                    if (uninstallConfirm) {
                        try {
                            const uninstallResp = await fetch('/api/lhm/uninstall', {
                                method: 'POST'
                            });
                            if (uninstallResp.ok) {
                                showToast('LHM uninstalled successfully', 'success');
                                setTimeout(() => location.reload(), 1500);
                            }
                        } catch (err) {
                            showToast('Failed to uninstall LHM: ' + err.message, 'error');
                        }
                    }
                };
            } else if (lhmInstalled) {
                // LHM is installed but not running - offer to start it
                lhmStatusEl.textContent = 'LibreHardwareMonitor is installed but not running. Start it to enable CPU temperature monitoring.';
                lhmButtonsEl.innerHTML = `
                    <button id="btn-lhm-start" style="flex:1;padding:10px;background:#a3be8c;border:none;border-radius:4px;cursor:pointer;font-weight:bold;">Start LHM</button>
                    <button id="btn-lhm-uninstall" style="flex:1;padding:10px;background:#bf616a;border:none;border-radius:4px;cursor:pointer;">Uninstall LHM</button>
                `;
                
                lhmButtonsEl.querySelector('#btn-lhm-start').onclick = async () => {
                    overlay.remove();
                    
                    // Show UAC warning modal
                    const warningOverlay = document.createElement('div');
                    warningOverlay.style.cssText = `
                        position: fixed;
                        top: 50%;
                        left: 50%;
                        transform: translate(-50%, -50%);
                        width: 450px;
                        background: #2e3440;
                        border: 2px solid #ebcb8b;
                        border-radius: 12px;
                        box-shadow: 0 20px 60px rgba(0,0,0,0.8);
                        z-index: 99999;
                        padding: 25px;
                        color: #d8dee9;
                    `;
                    
                    warningOverlay.innerHTML = `
                        <div style="display:flex;justify-content:center;align-items:center;margin-bottom:20px;">
                            <div style="width:48px;height:48px;background:#3b4252;border-radius:50%;display:flex;align-items:center;justify-content:center;margin-right:20px;">
                                <span style="font-size:24px;">⚠️</span>
                            </div>
                            <div>
                                <h2 style="margin:0 0 5px 0;color:#ebcb8b;">Administrator Access Required</h2>
                                <p style="margin:0;font-size:0.85rem;color:#a3be8c;">A Windows security prompt will appear</p>
                            </div>
                        </div>
                        
                        <div style="background:#3b4252;border-radius:8px;padding:15px;margin-bottom:20px;line-height:1.6;font-size:0.9rem;">
                            <p style="margin:0 0 10px 0;"><strong>What will happen:</strong></p>
                            <ul style="margin:0 0 10px 0;padding-left:20px;">
                                <li>Windows will show a UAC prompt asking for admin permission</li>
                                <li>Click "Yes" to allow LibreHardwareMonitor to start</li>
                                <li>LHM will run in the background (may briefly flash on screen)</li>
                                <li>After starting, the window will refresh automatically</li>
                            </ul>
                        </div>
                        
                        <div style="display:flex;gap:10px;">
                            <button id="btn-uac-yes" style="flex:1;padding:12px;background:#a3be8c;border:none;border-radius:6px;cursor:pointer;font-weight:bold;font-size:1rem;">Yes, Continue</button>
                            <button id="btn-uac-no" style="flex:1;padding:12px;background:#bf616a;border:none;border-radius:6px;cursor:pointer;font-size:1rem;">Cancel</button>
                        </div>
                        
                        <p style="margin-top:20px;font-size:0.75rem;color:#616e88;text-align:center;">
                            LibreHardwareMonitor needs admin access to read hardware sensors.
                        </p>
                    `;
                    
                    document.body.appendChild(warningOverlay);
                    
                    return new Promise((resolve) => {
                        warningOverlay.querySelector('#btn-uac-yes').onclick = () => {
                            warningOverlay.remove();
                            resolve(true);
                        };
                        warningOverlay.querySelector('#btn-uac-no').onclick = () => {
                            warningOverlay.remove();
                            resolve(false);
                        };
                    }).then(async (proceed) => {
                        if (!proceed) return;
                        
                        try {
                            const startResp = await fetch('/api/lhm/start', {
                                method: 'POST'
                            });
                            if (startResp.ok) {
                                showToast('LHM started successfully', 'success');
                                setTimeout(() => location.reload(), 2000);
                            } else {
                                const data = await startResp.json();
                                showToast('Failed to start LHM: ' + (data.error || 'Unknown error'), 'error');
                            }
                        } catch (err) {
                            showToast('Failed to start LHM: ' + err.message, 'error');
                        }
                    });
                };
                
                lhmButtonsEl.querySelector('#btn-lhm-uninstall').onclick = async () => {
                    overlay.remove();
                    const uninstallConfirm = confirm('Are you sure you want to uninstall LibreHardwareMonitor?');
                    if (uninstallConfirm) {
                        try {
                            const uninstallResp = await fetch('/api/lhm/uninstall', {
                                method: 'POST'
                            });
                            if (uninstallResp.ok) {
                                showToast('LHM uninstalled successfully', 'success');
                                setTimeout(() => location.reload(), 1500);
                            }
                        } catch (err) {
                            showToast('Failed to uninstall LHM: ' + err.message, 'error');
                        }
                    }
                };
            } else {
                // LHM not installed
                lhmStatusEl.textContent = 'CPU temperature monitoring requires LibreHardwareMonitor. Please install it to see CPU temperatures.';
                lhmButtonsEl.innerHTML = `
                    <button id="btn-lhm-install" style="flex:1;padding:10px;background:#a3be8c;border:none;border-radius:4px;cursor:pointer;font-weight:bold;">Install Automatically</button>
                    <button id="btn-lhm-cancel" style="flex:1;padding:10px;background:#bf616a;border:none;border-radius:4px;cursor:pointer;">Disable</button>
                `;
                
                lhmButtonsEl.querySelector('#btn-lhm-cancel').onclick = async () => {
                    overlay.remove();
                    try {
                        const disableResp = await fetch('/api/lhm/disable', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ disabled: true })
                        });
                        if (disableResp.ok) {
                            showToast('LHM monitoring disabled', 'success');
                            setTimeout(() => location.reload(), 1500);
                        }
                    } catch (err) {
                        showToast('Failed to disable LHM: ' + err.message, 'error');
                    }
                };
                
                lhmButtonsEl.querySelector('#btn-lhm-install').onclick = async () => {
                    console.log('[LHM UI] Install button clicked');
                    overlay.remove();
                    
                    // Show UAC warning modal
                    const warningOverlay = createUACWarningOverlay();
                    const userConfirmed = await showWarningModal(warningOverlay);
                    
                    if (!userConfirmed) {
                        resolve('cancel');
                        return;
                    }
                    
                    console.log('[LHM UI] User confirmed, starting installation...');
                    
                    const progressOverlay = document.createElement('div');
                    progressOverlay.style.cssText = `
                        position: fixed;
                        top: 50%;
                        left: 50%;
                        transform: translate(-50%, -50%);
                        width: 400px;
                        background: #2e3440;
                        border: 2px solid #88c0d0;
                        border-radius: 12px;
                        box-shadow: 0 20px 60px rgba(0,0,0,0.7);
                        z-index: 99999;
                        padding: 30px;
                        color: #d8dee9;
                        text-align: center;
                    `;
                    
                    progressOverlay.innerHTML = `
                        <div style="margin-bottom: 20px;">
                            <h3 style="margin: 0 0 10px 0; color: #88c0d0; font-size: 18px;">Installing LibreHardwareMonitor</h3>
                            <p style="margin: 0; color: #bf616a;">This will open a UAC prompt.</p>
                        </div>
                        <div id="progress-bar-container" style="width: 100%; height: 8px; background: #4c566a; border-radius: 4px; overflow: hidden; margin-bottom: 15px;">
                            <div id="progress-bar" style="width: 0%; height: 100%; background: #88c0d0; transition: width 0.3s ease;"></div>
                        </div>
                        <div id="progress-text" style="color: #bf616a; font-size: 14px;">Waiting for UAC...</div>
                        <div style="margin-top: 15px; font-size: 12px; color: #616e88;">
                            <span class="spinner" style="display: inline-block; width: 12px; height: 12px; border: 2px solid #616e88; border-top: 2px solid #88c0d0; border-radius: 50%; animation: spin 1s linear infinite; margin-right: 8px;"></span>
                            Please wait...
                        </div>
                        <style>
                            @keyframes spin { to { transform: rotate(360deg); } }
                        </style>
                    `;
                    
                    document.body.appendChild(progressOverlay);
                    
                    console.log('[LHM UI] Calling /api/lhm/install...');
                    try {
                        const response = await fetch('/api/lhm/install', {
                            method: 'POST',
                            headers: {
                                'Content-Type': 'application/json'
                            }
                        });
                        console.log('[LHM UI] /api/lhm/install response status:', response.status);
                        
                        if (response.ok) {
                            const data = await response.json();
                            console.log('[LHM UI] /api/lhm/install response:', data);
                            
                            const progressText = document.getElementById('progress-text');
                            const progressBar = document.getElementById('progress-bar');
                            
                            let attempts = 0;
                            const maxAttempts = 60;
                            
                            const checkProgress = async () => {
                                if (attempts >= maxAttempts) {
                                    if (progressOverlay) progressOverlay.remove();
                                    showToast('Installation timeout. Please check if LHM was installed.', 'error');
                                    return;
                                }
                                
                                  attempts++;
                                    console.log(`[LHM UI] Checking progress (attempt ${attempts})...`);
                                        
                                        try {
                                            const progressResp = await fetch('/api/lhm/progress');
                                            if (progressResp.ok) {
                                        const progressData = await progressResp.json();
                                        const progress = progressData.progress || '';
                                        
                                        console.log('[LHM UI] Progress:', progress);
                                        
                                        if (progressText) {
                                            let progressDisplay = progress;
                                            let progressBarWidth = '0%';
                                            
                                            if (progress.includes('downloading:')) {
                                                progressDisplay = 'Downloading...';
                                                const pct = progress.match(/(\d+)%/);
                                                if (pct) progressBarWidth = pct[1] + '%';
                                            } else if (progress.includes('extracting:')) {
                                                progressDisplay = progress;
                                                const pct = progress.match(/(\d+)%/);
                                                if (pct) progressBarWidth = pct[1] + '%';
                                            } else if (progress === 'completed') {
                                                progressDisplay = 'Installation complete! LHM is now running.';
                                                if (progressBar) progressBar.style.background = '#a3be8c';
                                            } else if (progress === 'failed') {
                                                progressDisplay = 'Installation failed!';
                                                if (progressBar) progressBar.style.background = '#bf616a';
                                            }
                                            
                                            progressText.textContent = progressDisplay;
                                            if (progressBar && progressBarWidth !== '0%') {
                                                progressBar.style.width = progressBarWidth;
                                            }
                                        }
                                        
                                        if (progress === 'completed' || progress === 'failed') {
                                            setTimeout(() => {
                                                if (progressOverlay) progressOverlay.remove();
                                                showToast('Installation ' + (progress === 'completed' ? 'complete! Reloading...' : 'failed'), progress === 'completed' ? 'success' : 'error');
                                                if (progress === 'completed') {
                                                    setTimeout(() => {
                                                        window.location.reload();
                                                    }, 2000);
                                                }
                                            }, 1500);
                                           } else {
                                                setTimeout(checkProgress, 500);
                                            }
                                        }
                                    } catch (err) {
                                    console.error('[LHM UI] Progress check error:', err);
                                    setTimeout(checkProgress, 500);
                                }
                            };
                            
                           setTimeout(checkProgress, 1000);
                        } else {
                            const data = await response.json();
                            console.error('[LHM UI] /api/lhm/install failed:', data);
                            if (progressOverlay) progressOverlay.remove();
                            showToast(`Installation failed: ${data.error || 'Unknown error'}`, 'error');
                        }
             } catch (err) {
                 console.error('[LHM UI] /api/lhm/install error:', err);
                 if (progressOverlay) progressOverlay.remove();
                 showToast(`Installation error: ${err.message}`, 'error');
             }
         };
       }
        } catch (err) {
            console.error('[LHM UI] Error checking LHM status:', err);
            lhmStatusEl.textContent = 'Error checking LHM status. Please try again.';
        }
      });
  }

// Create the UAC warning overlay
function createUACWarningOverlay() {
    const overlay = document.createElement('div');
    overlay.style.cssText = `
        position: fixed;
        top: 50%;
        left: 50%;
        transform: translate(-50%, -50%);
        width: 500px;
        background: #2e3440;
        border: 2px solid #ebcb8b;
        border-radius: 12px;
        box-shadow: 0 20px 60px rgba(0,0,0,0.8);
        z-index: 99999;
        padding: 30px;
        color: #d8dee9;
    `;
    
    overlay.innerHTML = `
        <div style="display:flex;justify-content:center;align-items:center;margin-bottom:20px;">
            <div style="width:48px;height:48px;background:#3b4252;border-radius:50%;display:flex;align-items:center;justify-content:center;margin-right:20px;">
                <span style="font-size:24px;">⚠️</span>
            </div>
            <div>
                <h2 style="margin:0 0 5px 0;color:#ebcb8b;">Administrator Access Required</h2>
                <p style="margin:0;font-size:0.85rem;color:#a3be8c;">This will open a Windows security prompt</p>
            </div>
        </div>
        
        <div style="background:#3b4252;border-radius:8px;padding:15px;margin-bottom:20px;line-height:1.6;font-size:0.9rem;">
            <p style="margin:0 0 10px 0;"><strong>What will happen:</strong></p>
            <ul style="margin:0 0 10px 0;padding-left:20px;">
                <li>Windows will show a UAC prompt asking for admin permission</li>
                <li>LibreHardwareMonitor will be downloaded (~5MB)</li>
                <li>It will be installed silently to your AppData folder</li>
                <li>After installation, the window will minimize automatically</li>
            </ul>
        </div>
        
        <div style="display:flex;gap:10px;">
            <button id="btn-warning-yes" style="flex:1;padding:12px;background:#a3be8c;border:none;border-radius:6px;cursor:pointer;font-weight:bold;font-size:1rem;">Yes, Continue</button>
            <button id="btn-warning-no" style="flex:1;padding:12px;background:#bf616a;border:none;border-radius:6px;cursor:pointer;font-size:1rem;">Cancel</button>
        </div>
        
        <p style="margin-top:20px;font-size:0.75rem;color:#616e88;text-align:center;">
            LibreHardwareMonitor needs admin access to read hardware sensors.
        </p>
    `;
    
    return overlay;
}

// Show the warning modal and return user's choice
function showWarningModal(overlay) {
    return new Promise((resolve) => {
        document.body.appendChild(overlay);
        
        overlay.querySelector('#btn-warning-yes').onclick = () => {
            overlay.remove();
            resolve(true);
        };
        
        overlay.querySelector('#btn-warning-no').onclick = () => {
            overlay.remove();
            resolve(false);
        };
    });
}

// LHM (LibreHardwareMonitor) integration - runs once on page load
async function checkLHMAndPrompt() {
    // Only run LHM checks on Windows
    if (navigator.platform.indexOf('Win') === -1) {
        return;
    }
    
    // Check server-side config file first
    let isDisabled = false;
    try {
        const statusResp = await fetch('/api/lhm/status');
        if (statusResp.ok) {
            const statusData = await statusResp.json();
            isDisabled = statusData.disabled;
        }
    } catch (err) {
        // Config doesn't exist or error - proceed normally
    }
    
    // Check if LHM is available
    let lhmAvailable = false;
    try {
        const checkResp = await fetch('/api/lhm/check');
        if (checkResp.ok) {
            const checkData = await checkResp.json();
            lhmAvailable = checkData.available || false;
        }
    } catch (err) {
        // API not available
    }
    
    // Update the system metrics table
    const sysRowsEl = document.getElementById('system-rows');
    if (sysRowsEl) {
        const isWindows = navigator.platform.indexOf('Win') !== -1;
        
        let tempColumn = '';
        if (isWindows) {
            if (lhmAvailable) {
                tempColumn = '<td class="value temp" id="lhm-temp-col">—</td>';
            } else if (isDisabled) {
                tempColumn = '<td class="value temp" id="lhm-temp-col"><button class="btn-lhm-inline need-attention" onclick="showLHMNotification()" title="Install LibreHardwareMonitor for CPU temp monitoring">&#9971;</button></td>';
            } else {
                tempColumn = '<td class="value temp" id="lhm-temp-col"><button class="btn-lhm-inline" onclick="showLHMNotification()" title="Install LibreHardwareMonitor for CPU temp monitoring">&#9971;</button></td>';
            }
        } else {
            tempColumn = '<td class="value temp">—</td>';
        }
        
        const currentRow = sysRowsEl.querySelector('tr');
        if (currentRow) {
            const existingCells = currentRow.querySelectorAll('td');
            if (existingCells.length >= 2) {
                existingCells[1].outerHTML = tempColumn;
            }
       }
    }
}

// Clean up LHM resolve function
window.lhmResolve = null;

// ============================================
// Keyboard Shortcuts Modal
// ============================================

function openKeyboardShortcutsModal() {
    document.getElementById('keyboard-shortcuts-modal').classList.add('open');
}

function closeKeyboardShortcutsModal() {
    document.getElementById('keyboard-shortcuts-modal').classList.remove('open');
}

// Show modal on ? key
document.addEventListener('keydown', e => {
    if (e.key === '?' && !e.ctrlKey && !e.altKey && !e.metaKey) {
        e.preventDefault();
        openKeyboardShortcutsModal();
    }
});

// Close modal on Escape key
document.addEventListener('keydown', e => {
    if (e.key === 'Escape' && document.getElementById('keyboard-shortcuts-modal').classList.contains('open')) {
        closeKeyboardShortcutsModal();
    }
});

// ============================================
// Setup / Monitor View Management
// ============================================

const appState = {
    view: 'setup',
    sessionActive: false,
    lastSessionData: null
};

function switchView(targetView) {
    if (appState.view === 'transitioning') return;
    appState.view = 'transitioning';

    const currentViewEl = document.getElementById('view-' + (appState.view === 'transitioning' ? 'setup' : appState.view));
    const targetViewEl = document.getElementById('view-' + targetView);
    const setupStrip = document.getElementById('endpoint-strip-setup');
    const monitorStrip = document.getElementById('endpoint-strip-monitor');

    if (!currentViewEl || !targetViewEl) {
        appState.view = targetView;
        return;
    }

    if (targetView === 'monitor') {
        currentViewEl.classList.add('exiting');
        setTimeout(() => {
            currentViewEl.style.display = 'none';
            currentViewEl.classList.remove('exiting');
            targetViewEl.style.display = '';
            targetViewEl.classList.add('entering');
            showFlashOverlay();
            animateCardsEnter();
            if (setupStrip) setupStrip.style.display = 'none';
            if (monitorStrip) monitorStrip.style.display = '';
            document.body.classList.remove('setup-active');
            setTimeout(() => {
                targetViewEl.classList.remove('entering');
                appState.view = 'monitor';
            }, 500);
        }, 400);
    } else {
        animateCardsExit();
        if (setupStrip) setupStrip.style.display = '';
        if (monitorStrip) monitorStrip.style.display = 'none';
        document.body.classList.add('setup-active');
        setTimeout(() => {
            currentViewEl.style.display = 'none';
            currentViewEl.classList.remove('exiting');
            targetViewEl.style.display = '';
            targetViewEl.classList.add('entering');
            animateSetupCardsEnter();
            setTimeout(() => {
                targetViewEl.classList.remove('entering');
                appState.view = 'setup';
            }, 400);
        }, 600);
    }
}

function showConnectingState() {
    const connectingDots = document.getElementById('connecting-dots');
    if (connectingDots) connectingDots.style.display = '';
}

function hideConnectingState() {
    const connectingDots = document.getElementById('connecting-dots');
    if (connectingDots) connectingDots.style.display = 'none';
}

function showFlashOverlay() {
    const existing = document.querySelector('.view-flash');
    if (existing) existing.remove();
    const flash = document.createElement('div');
    flash.className = 'view-flash';
    document.body.appendChild(flash);
    setTimeout(() => flash.remove(), 800);
}

function animateCardsEnter() {
    const cards = document.querySelectorAll('.view-monitor .widget-card');
    cards.forEach((card, i) => {
        card.classList.add('entrance');
        setTimeout(() => card.classList.add('active'), 120 * i);
    });
}

function animateCardsExit() {
    const cards = [...document.querySelectorAll('.view-monitor .widget-card')].reverse();
    cards.forEach((card, i) => {
        card.style.transition = `opacity 0.3s ease ${60 * i}ms, transform 0.3s ease ${60 * i}ms`;
        card.style.opacity = '0';
        card.style.transform = 'translateY(16px)';
    });
}

function animateSetupCardsEnter() {
    const cards = document.querySelectorAll('.view-setup .setup-card.entrance');
    cards.forEach((card, i) => {
        setTimeout(() => card.classList.add('active'), 80 * i);
    });
}

function doAttachFromSetup() {
    const input = document.getElementById('setup-endpoint-url');
    const url = input ? input.value.trim() : '';
    if (url) {
        const serverEndpoint = document.getElementById('server-endpoint');
        if (serverEndpoint) serverEndpoint.value = url;
        localStorage.setItem('llama-monitor-last-endpoint', url);
    }
    showConnectingState();
    doAttach();
}

function doStartFromSetup() {
    const select = document.getElementById('setup-preset-select');
    if (select) {
        const presetSelect = document.getElementById('preset-select');
        if (presetSelect) presetSelect.value = select.value;
    }
    showConnectingState();
    doStart();
}

function saveLastSessionData(data) {
    const payload = { ...data, timestamp: Date.now() };
    localStorage.setItem('llama-monitor-last-session', JSON.stringify(payload));
    appState.lastSessionData = payload;
}

function loadLastSessionData() {
    try {
        const raw = localStorage.getItem('llama-monitor-last-session');
        if (!raw) return null;
        const data = JSON.parse(raw);
        if (Date.now() - data.timestamp > 24 * 60 * 60 * 1000) {
            localStorage.removeItem('llama-monitor-last-session');
            return null;
        }
        return data;
    } catch {
        return null;
    }
}

function renderQuickStats() {
    const data = loadLastSessionData();
    const statsEl = document.getElementById('setup-stats');
    if (!statsEl) return;

    if (data) {
        const promptRate = document.getElementById('setup-last-prompt-rate');
        const genRate = document.getElementById('setup-last-gen-rate');
        const session = document.getElementById('setup-last-session');
        if (promptRate) promptRate.textContent = data.promptRate || '—';
        if (genRate) genRate.textContent = data.genRate || '—';
        if (session) session.textContent = data.sessionName || '—';
        statsEl.style.display = 'flex';
    } else {
        statsEl.style.display = 'none';
    }
}

function syncSetupPresetSelect() {
    const setupSelect = document.getElementById('setup-preset-select');
    const mainSelect = document.getElementById('preset-select');
    if (!setupSelect || !mainSelect) return;

    setupSelect.innerHTML = '';
    const options = mainSelect.querySelectorAll('option');
    options.forEach(opt => {
        const clone = document.createElement('option');
        clone.value = opt.value;
        clone.textContent = opt.textContent;
        setupSelect.appendChild(clone);
    });
    setupSelect.value = mainSelect.value;
}

// Initialize view state on load
function initViewState() {
    renderQuickStats();
    syncSetupPresetSelect();
    const lastEndpoint = localStorage.getItem('llama-monitor-last-endpoint');
    if (lastEndpoint) {
        const input = document.getElementById('setup-endpoint-url');
        if (input) input.value = lastEndpoint;
    }
    document.body.classList.add('setup-active');
    const setupView = document.getElementById('view-setup');
    const monitorView = document.getElementById('view-monitor');
    if (setupView) {
        setupView.style.display = '';
        setupView.classList.add('entering');
        setTimeout(() => setupView.classList.remove('entering'), 600);
    }
    if (monitorView) monitorView.style.display = 'none';
}

// Call init on DOM ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initViewState);
} else {
    initViewState();
}
