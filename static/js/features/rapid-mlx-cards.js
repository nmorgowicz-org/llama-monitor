import { buildSparklineSVG, buildRingMarkup, updateMetricDelta } from './dashboard-render.js';

const STALE_POLLS = 3;
const SPARKLINE_HISTORY_LIMIT = 60;
const cardHistory = new Map();
let parkedLlamaCards = null;
let lastPollSequence = null;
let lastSessionId = null;
const throughputHistory = { promptTps: [], generationTps: [] };
const throughputPeaks = { promptTps: 0, generationTps: 0 };

function pushThroughputHistory(key, value) {
    if (!Number.isFinite(value)) return;
    const series = throughputHistory[key];
    series.push(value);
    if (series.length > SPARKLINE_HISTORY_LIMIT) series.shift();
    if (value > throughputPeaks[key]) throughputPeaks[key] = value;
}

const present = value => value !== null && value !== undefined;
const anyPresent = (sample, fields) => fields.some(field => present(sample?.[field]));

function normalizedProgress(value) {
    if (typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= 1) return value;
    if (!value || typeof value !== 'object') return null;
    const current = Number(value.current);
    const total = Number(value.total);
    if (!Number.isFinite(current) || !Number.isFinite(total) || current < 0 || total <= 0 || current > total) return null;
    return current / total;
}

function formatNumber(value, digits = 0) {
    return Number(value).toLocaleString(undefined, {
        minimumFractionDigits: digits,
        maximumFractionDigits: digits,
    });
}

function formatBytes(value) {
    if (!present(value)) return '';
    const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
    let amount = Number(value);
    let unit = 0;
    while (amount >= 1024 && unit < units.length - 1) {
        amount /= 1024;
        unit++;
    }
    return amount.toFixed(unit > 1 ? 1 : 0) + ' ' + units[unit];
}

function formatDuration(seconds) {
    const total = Math.max(0, Math.floor(Number(seconds)));
    const hours = Math.floor(total / 3600);
    const minutes = Math.floor((total % 3600) / 60);
    if (hours) return hours + 'h ' + minutes + 'm';
    return minutes ? minutes + 'm' : total + 's';
}

function metric(label, value, rawDelta) {
    const row = document.createElement('div');
    row.className = 'rapid-metric';
    const name = document.createElement('span');
    name.textContent = label;
    row.append(name);
    if (Number.isFinite(rawDelta)) {
        const wrap = document.createElement('span');
        wrap.className = 'speed-value-wrap';
        const result = document.createElement('strong');
        result.textContent = value;
        result.dataset.raw = String(rawDelta);
        const delta = document.createElement('span');
        delta.className = 'metric-delta';
        wrap.append(result, delta);
        row.append(wrap);
    } else {
        const result = document.createElement('strong');
        result.textContent = value;
        row.append(result);
    }
    return row;
}

function progressMetric(ratio) {
    const percent = Math.round(ratio * 100);
    const row = document.createElement('div');
    row.className = 'rapid-progress';
    const label = document.createElement('div');
    label.className = 'rapid-progress-label';
    label.textContent = 'Progress';
    const value = document.createElement('strong');
    value.textContent = percent + '%';
    label.append(value);
    const track = document.createElement('div');
    track.className = 'rapid-progress-track';
    track.setAttribute('role', 'progressbar');
    track.setAttribute('aria-label', 'Rapid-MLX live progress');
    track.setAttribute('aria-valuemin', '0');
    track.setAttribute('aria-valuemax', '100');
    track.setAttribute('aria-valuenow', String(percent));
    const fill = document.createElement('div');
    fill.className = 'rapid-progress-fill';
    fill.style.width = percent + '%';
    track.append(fill);
    row.append(label, track);
    return row;
}

// Pairs two sparklines side-by-side in the shared `.sparkline-pair` grid, mirroring
// llama.cpp's SPEED card layout (prompt/generation history shown together).
function sparklinePairRow(entries) {
    const row = document.createElement('div');
    row.className = 'sparkline-pair rapid-sparkline-pair';
    row.dataset.sparklineLabel = entries.map(entry => entry.label).join('+');
    entries.forEach(({ points, color }) => {
        const chart = document.createElement('div');
        chart.className = 'rapid-sparkline-chart';
        // eslint-disable-next-line no-unsanitized/property -- SVG path data built from numeric array values only, cssClass/color are call-site literals
        chart.innerHTML = buildSparklineSVG(points, 'rapid-sparkline', color);
        row.append(chart);
    });
    return row;
}

// Mirrors llama.cpp's SPEED card row: label + value + peak badge, then a glowing
// gradient throughput bar (relative to the session peak) beneath it.
function speedRow(label, value, unit, peak, barClass) {
    const row = document.createElement('div');
    row.className = 'rapid-speed-row';
    const header = document.createElement('div');
    header.className = 'speed-header';
    const name = document.createElement('span');
    name.className = 'speed-label';
    name.textContent = label;
    const wrap = document.createElement('span');
    wrap.className = 'speed-value-wrap';
    const result = document.createElement('span');
    result.className = 'speed-value';
    result.textContent = formatNumber(value, 1) + ' ' + unit;
    wrap.append(result);
    header.append(name, wrap);
    if (Number.isFinite(peak) && peak > 0) {
        const badge = document.createElement('span');
        badge.className = 'speed-peak-badge';
        badge.textContent = 'peak ' + formatNumber(peak, 1);
        header.append(badge);
    }
    const bg = document.createElement('div');
    bg.className = 'speed-bar-bg';
    const fill = document.createElement('div');
    fill.className = 'speed-bar ' + barClass;
    const ratio = Number.isFinite(peak) && peak > 0 ? Math.min(1, value / peak) : (value > 0 ? 1 : 0);
    fill.style.transform = 'scaleX(' + ratio.toFixed(4) + ')';
    bg.append(fill);
    row.append(header, bg);
    return row;
}

function hitRateRingRow(hitRatePct, hasData) {
    const row = document.createElement('div');
    row.className = 'rapid-ring-row' + (hasData ? '' : ' is-no-data');
    row.dataset.ringLabel = 'Hit rate';
    const viz = document.createElement('div');
    viz.className = 'rapid-ring-viz';
    if (hasData) {
        // eslint-disable-next-line no-unsanitized/property -- markup built from numeric pct and a hardcoded hex color literal only
        viz.innerHTML = buildRingMarkup(hitRatePct, '#88c0d0', false);
    }
    const text = document.createElement('div');
    text.className = 'rapid-ring-text';
    const name = document.createElement('span');
    name.textContent = 'Hit rate';
    const value = document.createElement('strong');
    value.textContent = hasData ? hitRatePct.toFixed(1) + '%' : 'No data yet';
    text.append(name, value);
    row.append(viz, text);
    return row;
}

function requestRailRow(activeRequests) {
    const row = document.createElement('div');
    row.className = 'rapid-rail-row';
    const rail = document.createElement('div');
    rail.className = 'rapid-rail';
    rail.setAttribute('role', 'list');
    rail.setAttribute('aria-label', 'Active requests');
    activeRequests.forEach(request => {
        const badge = document.createElement('span');
        const status = String(request.status || 'active').toLowerCase();
        badge.className = 'rapid-rail-badge rapid-rail-badge--' + status.replace(/[^a-z0-9-]/g, '-');
        badge.setAttribute('role', 'listitem');
        badge.title = (request.id || request.request_id || '') + (request.status ? ' · ' + request.status : '');
        badge.textContent = status;
        rail.append(badge);
    });
    row.append(rail);
    return row;
}

function memoryBarRow(activeBytes, peakBytes, cacheBytes) {
    const row = document.createElement('div');
    row.className = 'rapid-membar-row';
    const label = document.createElement('div');
    label.className = 'rapid-membar-label';
    const name = document.createElement('span');
    name.textContent = 'Active vs. session peak';
    const value = document.createElement('strong');
    value.textContent = formatBytes(activeBytes) + ' / ' + formatBytes(peakBytes);
    label.append(name, value);
    const track = document.createElement('div');
    track.className = 'rapid-membar-track';
    const activeRatio = peakBytes > 0 ? Math.min(1, activeBytes / peakBytes) : 0;
    const cacheRatio = peakBytes > 0 ? Math.min(1 - activeRatio, cacheBytes / peakBytes) : 0;
    const activeFill = document.createElement('div');
    activeFill.className = 'rapid-membar-fill rapid-membar-fill--active';
    activeFill.style.width = (activeRatio * 100) + '%';
    const cacheFill = document.createElement('div');
    cacheFill.className = 'rapid-membar-fill rapid-membar-fill--cache';
    cacheFill.style.width = (cacheRatio * 100) + '%';
    cacheFill.style.left = (activeRatio * 100) + '%';
    track.setAttribute('role', 'img');
    track.setAttribute('aria-label', 'Active memory ' + formatBytes(activeBytes) + ' of session peak ' + formatBytes(peakBytes));
    track.append(activeFill, cacheFill);
    row.append(label, track);
    return row;
}

function staleLabel(sampledAtMs, missing) {
    const timestamp = Number(sampledAtMs);
    const age = Number.isFinite(timestamp) && timestamp > 0
        ? Math.max(0, Math.floor((Date.now() - timestamp) / 1000)) + 's ago'
        : 'last sample';
    return 'stale · ' + age + ' · ' + missing + '/3';
}

// Maps a card's free-text state label to the shared `.metric-live-chip` vocabulary
// (already used by llama.cpp cards) instead of always rendering green "live" styling.
function chipClassForState(state) {
    if (state === 'degraded' || state === 'not ready') return 'critical';
    if (state === 'stale' || state === 'idle') return 'idle';
    return 'live';
}

function card(title, rows, state = '') {
    const element = document.createElement('section');
    element.className = 'widget-card rapid-telemetry-card';
    const top = document.createElement('div');
    top.className = 'metric-card-topline';
    const label = document.createElement('h3');
    label.className = 'widget-metric-label';
    label.textContent = title;
    top.append(label);
    if (state) {
        const chip = document.createElement('span');
        chip.className = 'metric-live-chip ' + chipClassForState(state);
        chip.textContent = state;
        top.append(chip);
    }
    const body = document.createElement('div');
    body.className = 'rapid-metric-list';
    rows.forEach(row => body.append(row));
    element.append(top, body);
    return element;
}

function syncAttributes(target, source) {
    for (const attribute of [...target.attributes]) {
        if (!source.hasAttribute(attribute.name)) target.removeAttribute(attribute.name);
    }
    for (const attribute of [...source.attributes]) {
        if (target.getAttribute(attribute.name) !== attribute.value) {
            target.setAttribute(attribute.name, attribute.value);
        }
    }
}

function syncText(target, source) {
    if (target.textContent !== source.textContent) target.textContent = source.textContent;
}

function syncProgressRow(target, source) {
    const targetLabel = target.querySelector('.rapid-progress-label');
    const sourceLabel = source.querySelector('.rapid-progress-label');
    if (targetLabel && sourceLabel) {
        const targetValue = targetLabel.querySelector('strong');
        const sourceValue = sourceLabel.querySelector('strong');
        if (targetValue && sourceValue) syncText(targetValue, sourceValue);
    }
    const targetTrack = target.querySelector('.rapid-progress-track');
    const sourceTrack = source.querySelector('.rapid-progress-track');
    if (targetTrack && sourceTrack) {
        syncAttributes(targetTrack, sourceTrack);
        const targetFill = targetTrack.querySelector('.rapid-progress-fill');
        const sourceFill = sourceTrack.querySelector('.rapid-progress-fill');
        if (targetFill && sourceFill && targetFill.style.width !== sourceFill.style.width) {
            targetFill.style.width = sourceFill.style.width;
        }
    }
}

function rowKey(row, index) {
    if (row.classList.contains('rapid-progress')) return 'progress';
    if (row.classList.contains('rapid-sparkline-row') || row.classList.contains('rapid-sparkline-pair')) {
        return 'sparkline:' + row.dataset.sparklineLabel;
    }
    if (row.classList.contains('rapid-speed-row')) return 'speed:' + row.querySelector('.speed-label')?.textContent;
    if (row.classList.contains('rapid-ring-row')) return 'ring:' + row.dataset.ringLabel;
    if (row.classList.contains('rapid-rail-row')) return 'rail';
    if (row.classList.contains('rapid-membar-row')) return 'membar';
    const label = row.querySelector(':scope > span')?.textContent;
    return label ? 'metric:' + label : 'row:' + index;
}

function syncCardBody(target, source) {
    const existing = new Map([...target.children].map((row, index) => [rowKey(row, index), row]));
    [...source.children].forEach((sourceRow, index) => {
        const key = rowKey(sourceRow, index);
        let targetRow = existing.get(key);
        if (!targetRow || targetRow.tagName !== sourceRow.tagName) {
            targetRow = sourceRow;
        } else if (sourceRow.classList.contains('rapid-progress')) {
            syncProgressRow(targetRow, sourceRow);
        } else if (sourceRow.classList.contains('rapid-sparkline-row') || sourceRow.classList.contains('rapid-sparkline-pair')) {
            const targetCharts = [...targetRow.querySelectorAll('.rapid-sparkline-chart')];
            const sourceCharts = [...sourceRow.querySelectorAll('.rapid-sparkline-chart')];
            if (targetCharts.length === sourceCharts.length) {
                targetCharts.forEach((targetChart, i) => {
                    const sourceChart = sourceCharts[i];
                    if (targetChart.innerHTML !== sourceChart.innerHTML) {
                        // eslint-disable-next-line no-unsanitized/property -- sourceChart.innerHTML was itself built via buildSparklineSVG, not from untrusted input
                        targetChart.innerHTML = sourceChart.innerHTML;
                    }
                });
            } else {
                targetRow = sourceRow;
            }
        } else if (sourceRow.classList.contains('rapid-speed-row')) {
            const targetValue = targetRow.querySelector('.speed-value');
            const sourceValue = sourceRow.querySelector('.speed-value');
            if (targetValue && sourceValue) syncText(targetValue, sourceValue);
            const targetBadge = targetRow.querySelector('.speed-peak-badge');
            const sourceBadge = sourceRow.querySelector('.speed-peak-badge');
            if (sourceBadge) {
                if (targetBadge) syncText(targetBadge, sourceBadge);
                else targetRow.querySelector('.speed-header')?.append(sourceBadge);
            } else {
                targetBadge?.remove();
            }
            const targetFill = targetRow.querySelector('.speed-bar');
            const sourceFill = sourceRow.querySelector('.speed-bar');
            if (targetFill && sourceFill && targetFill.style.transform !== sourceFill.style.transform) {
                targetFill.style.transform = sourceFill.style.transform;
            }
        } else if (sourceRow.classList.contains('rapid-ring-row')) {
            if (targetRow.className !== sourceRow.className) targetRow.className = sourceRow.className;
            const targetViz = targetRow.querySelector('.rapid-ring-viz');
            const sourceViz = sourceRow.querySelector('.rapid-ring-viz');
            if (targetViz && sourceViz && targetViz.innerHTML !== sourceViz.innerHTML) {
                // eslint-disable-next-line no-unsanitized/property -- sourceViz.innerHTML was itself built via buildRingMarkup, not from untrusted input
                targetViz.innerHTML = sourceViz.innerHTML;
            }
            const targetValue = targetRow.querySelector('.rapid-ring-text strong');
            const sourceValue = sourceRow.querySelector('.rapid-ring-text strong');
            if (targetValue && sourceValue) syncText(targetValue, sourceValue);
        } else if (sourceRow.classList.contains('rapid-rail-row')) {
            const targetRail = targetRow.querySelector('.rapid-rail');
            const sourceRail = sourceRow.querySelector('.rapid-rail');
            if (targetRail && sourceRail && targetRail.innerHTML !== sourceRail.innerHTML) {
                targetRail.replaceChildren(...sourceRail.childNodes);
            }
        } else if (sourceRow.classList.contains('rapid-membar-row')) {
            const targetValue = targetRow.querySelector('.rapid-membar-label strong');
            const sourceValue = sourceRow.querySelector('.rapid-membar-label strong');
            if (targetValue && sourceValue) syncText(targetValue, sourceValue);
            const targetTrack = targetRow.querySelector('.rapid-membar-track');
            const sourceTrack = sourceRow.querySelector('.rapid-membar-track');
            if (targetTrack && sourceTrack) {
                syncAttributes(targetTrack, sourceTrack);
                ['active', 'cache'].forEach(kind => {
                    const targetFill = targetTrack.querySelector('.rapid-membar-fill--' + kind);
                    const sourceFill = sourceTrack.querySelector('.rapid-membar-fill--' + kind);
                    if (targetFill && sourceFill) {
                        if (targetFill.style.width !== sourceFill.style.width) targetFill.style.width = sourceFill.style.width;
                        if (targetFill.style.left !== sourceFill.style.left) targetFill.style.left = sourceFill.style.left;
                    }
                });
            }
        } else {
            const targetLabel = targetRow.querySelector(':scope > span');
            const sourceLabel = sourceRow.querySelector(':scope > span');
            const targetValue = targetRow.querySelector('strong');
            const sourceValue = sourceRow.querySelector('strong');
            if (targetLabel && sourceLabel) syncText(targetLabel, sourceLabel);
            if (targetValue && sourceValue) {
                if (sourceValue.dataset.raw !== undefined) {
                    const previousRaw = Number(targetValue.dataset.raw);
                    const currentRaw = Number(sourceValue.dataset.raw);
                    const targetDelta = targetRow.querySelector('.metric-delta');
                    if (targetDelta && targetValue.dataset.raw !== undefined) {
                        updateMetricDelta(targetDelta, previousRaw, currentRaw, 0);
                    }
                    targetValue.dataset.raw = sourceValue.dataset.raw;
                }
                syncText(targetValue, sourceValue);
            }
        }
        const currentAtIndex = target.children[index];
        if (currentAtIndex !== targetRow) target.insertBefore(targetRow, currentAtIndex || null);
        existing.delete(key);
    });
    existing.forEach(row => row.remove());
}

function syncCard(target, source) {
    syncAttributes(target, source);
    const targetTopline = target.querySelector('.metric-card-topline');
    const sourceTopline = source.querySelector('.metric-card-topline');
    if (targetTopline && sourceTopline) {
        const targetHeading = targetTopline.querySelector('.widget-metric-label');
        const sourceHeading = sourceTopline.querySelector('.widget-metric-label');
        if (targetHeading && sourceHeading) {
            syncAttributes(targetHeading, sourceHeading);
            syncText(targetHeading, sourceHeading);
        }
        const sourceChip = sourceTopline.querySelector('.metric-live-chip');
        let targetChip = targetTopline.querySelector('.metric-live-chip');
        if (sourceChip) {
            if (!targetChip) {
                targetChip = sourceChip;
                targetTopline.append(targetChip);
            } else {
                syncAttributes(targetChip, sourceChip);
                syncText(targetChip, sourceChip);
            }
        } else {
            targetChip?.remove();
        }
    }
    const targetBody = target.querySelector('.rapid-metric-list');
    const sourceBody = source.querySelector('.rapid-metric-list');
    if (targetBody && sourceBody) syncCardBody(targetBody, sourceBody);
}

const CARD_REGISTRY = [
    {
        id: 'runtime', order: 10,
        available: s => present(s.model) && (present(s.health) || present(s.ready) || present(s.uptime_seconds)),
        render: s => card('Rapid-MLX runtime', [
            metric('Model', s.model || 'Loading model identity'),
            metric('State', s.telemetry_unavailable
                ? 'Telemetry unavailable'
                : String(s.health || 'Unknown').replace(/([a-z])([A-Z])/g, '$1 $2')),
            ...(present(s.uptime_seconds) ? [metric('Uptime', formatDuration(s.uptime_seconds))] : []),
        ], s.telemetry_unavailable || s.health === 'Degraded'
            ? 'degraded'
            : (s.ready === false ? 'not ready' : 'live')),
    },
    {
        id: 'throughput', order: 20,
        available: s => anyPresent(s, ['generation_tokens_per_second', 'prompt_tokens_per_second']),
        render: s => {
            const pairs = [];
            if (throughputHistory.promptTps.length >= 2) pairs.push({ label: 'Prompt t/s', points: throughputHistory.promptTps, color: '#8fbcbb' });
            if (throughputHistory.generationTps.length >= 2) pairs.push({ label: 'Generation t/s', points: throughputHistory.generationTps, color: '#88c0d0' });
            return card('Inference throughput', [
                ...(present(s.prompt_tokens_per_second)
                    ? [speedRow('Prompt', Number(s.prompt_tokens_per_second), 't/s', throughputPeaks.promptTps, 'prompt-bar')]
                    : []),
                ...(present(s.generation_tokens_per_second)
                    ? [speedRow('Generation', Number(s.generation_tokens_per_second), 't/s', throughputPeaks.generationTps, 'gen-bar')]
                    : []),
                ...(pairs.length > 0 ? [sparklinePairRow(pairs)] : []),
            ], Number(s.running_requests) > 0 ? 'generating' : 'live');
        },
    },
    {
        id: 'queue', order: 30,
        available: s => anyPresent(s, ['running_requests', 'waiting_requests']),
        render: s => {
            const recognized = Array.isArray(s.active_requests)
                ? s.active_requests.filter(request => request && typeof request === 'object' && (present(request.id) || present(request.request_id) || present(request.status)))
                : [];
            return card('Request queue', [
                ...(present(s.running_requests) ? [metric('Running', formatNumber(s.running_requests))] : []),
                ...(present(s.waiting_requests) ? [metric('Waiting', formatNumber(s.waiting_requests))] : []),
                ...(recognized.length > 0 ? [requestRailRow(recognized)] : []),
            ], Number(s.running_requests) > 0 ? 'active' : 'idle');
        },
    },
    {
        id: 'memory', order: 40,
        available: s => anyPresent(s, ['active_memory_bytes', 'peak_memory_bytes', 'cache_memory_bytes']),
        render: s => card('Metal runtime memory', [
            ...(present(s.active_memory_bytes) && present(s.peak_memory_bytes)
                ? [memoryBarRow(Number(s.active_memory_bytes), Number(s.peak_memory_bytes), Number(s.cache_memory_bytes) || 0)]
                : []),
            ...(present(s.active_memory_bytes) ? [metric('Active', formatBytes(s.active_memory_bytes))] : []),
            ...(present(s.peak_memory_bytes) ? [metric('Peak', formatBytes(s.peak_memory_bytes))] : []),
            ...(present(s.cache_memory_bytes) ? [metric('Cache', formatBytes(s.cache_memory_bytes))] : []),
        ]),
    },
    {
        id: 'cache', order: 50,
        available: s => !!s.cache_metrics || present(s.global_cache_hit_rate),
        render: s => {
            const hasHitRate = present(s.global_cache_hit_rate);
            return card('Prefix & cache state', [
                ...(hasHitRate
                    ? [hitRateRingRow(s.global_cache_hit_rate * 100, true)]
                    : []),
                ...(present(s.global_cache_entries) ? [metric('Entries', formatNumber(s.global_cache_entries))] : []),
                ...(present(s.cache_metrics?.current_memory_bytes) ? [metric('Memory', formatBytes(s.cache_metrics.current_memory_bytes))] : []),
                ...(Array.isArray(s.cache_metrics?.multimodal_cache_kinds)
                    && s.cache_metrics.multimodal_cache_kinds.length > 0
                    ? [metric('Multimodal cache', 'Available')]
                    : []),
            ]);
        },
    },
    {
        id: 'totals', order: 60,
        available: s => anyPresent(s, ['completed_requests_total', 'prompt_tokens_total', 'completion_tokens_total', 'steps_executed']),
        render: s => card('Cumulative totals', [
            ...(present(s.completed_requests_total) ? [metric('Requests', formatNumber(s.completed_requests_total), Number(s.completed_requests_total))] : []),
            ...(present(s.prompt_tokens_total) ? [metric('Prompt tokens', formatNumber(s.prompt_tokens_total), Number(s.prompt_tokens_total))] : []),
            ...(present(s.completion_tokens_total) ? [metric('Completion tokens', formatNumber(s.completion_tokens_total), Number(s.completion_tokens_total))] : []),
            ...(present(s.steps_executed) ? [metric('Steps', formatNumber(s.steps_executed), Number(s.steps_executed))] : []),
        ]),
    },
    {
        id: 'progress', order: 80,
        available: s => normalizedProgress(s.backend_details?.progress) !== null,
        render: s => {
            const ratio = normalizedProgress(s.backend_details.progress);
            return card('Live progress', [progressMetric(ratio)], 'active');
        },
    },
];

function parkLlamaCards() {
    if (parkedLlamaCards) return;
    parkedLlamaCards = [];
    for (const selector of ['#inference', '.inference-detail-grid']) {
        const container = document.querySelector(selector);
        if (!container) continue;
        const fragment = document.createDocumentFragment();
        while (container.firstChild) fragment.append(container.firstChild);
        parkedLlamaCards.push({ container, fragment });
    }
}

export function restoreLlamaCards() {
    document.getElementById('rapid-mlx-card-grid')?.remove();
    if (!parkedLlamaCards) return;
    parkedLlamaCards.forEach(({ container, fragment }) => container.append(fragment));
    parkedLlamaCards = null;
    cardHistory.clear();
    lastPollSequence = null;
    lastSessionId = null;
}

export function renderRapidMlxCards(
    sample,
    pollSequence = 0,
    pollFailed = false,
    sessionId = '',
    sampledAtUnixMs = null
) {
    parkLlamaCards();
    const sessionChanged = lastSessionId !== sessionId;
    if (sessionChanged) {
        cardHistory.clear();
        lastPollSequence = null;
        lastSessionId = sessionId;
        throughputHistory.promptTps.length = 0;
        throughputHistory.generationTps.length = 0;
    }
    if (sample && !pollFailed) {
        pushThroughputHistory('promptTps', Number(sample.prompt_tokens_per_second));
        pushThroughputHistory('generationTps', Number(sample.generation_tokens_per_second));
    }
    const section = document.getElementById('inference-section');
    if (!section) return;
    let grid = document.getElementById('rapid-mlx-card-grid');
    if (!grid) {
        grid = document.createElement('div');
        grid.id = 'rapid-mlx-card-grid';
        grid.className = 'inference-grid rapid-telemetry-grid';
        section.append(grid);
    }
    if (!sessionChanged && sample && pollSequence === lastPollSequence && grid.childElementCount > 0) return;
    if (!sample) {
        grid.querySelectorAll('[data-card-id]').forEach(element => element.remove());
        let loading = grid.querySelector('[data-telemetry-state="loading"]');
        if (!loading) {
            loading = document.createElement('div');
            loading.className = 'rapid-telemetry-loading';
            loading.dataset.telemetryState = 'loading';
            loading.setAttribute('role', 'status');
            loading.setAttribute('aria-live', 'polite');
            loading.textContent = 'Connecting to Rapid-MLX telemetry…';
            grid.append(loading);
        }
        return;
    }

    grid.querySelector('[data-telemetry-state="loading"]')?.remove();

    const isNewPoll = pollSequence !== lastPollSequence;
    lastPollSequence = pollSequence;
    const availabilitySample = pollFailed ? {
        health: sample.health,
        model: sample.model,
        uptime_seconds: sample.uptime_seconds,
        ready: sample.ready,
    } : sample;
    const renderedCardIds = new Set();
    CARD_REGISTRY.slice().sort((a, b) => a.order - b.order).forEach(definition => {
        let renderedSample = sample;
        let stale = false;
        let missing = 0;
        if (pollFailed && definition.id === 'runtime' && definition.available(sample)) {
            const prior = cardHistory.get(definition.id);
            if (isNewPoll && prior) prior.missing++;
            missing = Math.min(STALE_POLLS, prior?.missing || 1);
            renderedSample = { ...sample, telemetry_unavailable: true };
            cardHistory.set(definition.id, { sample, missing });
            stale = true;
        } else if (definition.available(availabilitySample)) {
            cardHistory.set(definition.id, { sample, missing: 0 });
        } else {
            const prior = cardHistory.get(definition.id);
            if (!prior) return;
            if (isNewPoll) prior.missing++;
            if (prior.missing >= STALE_POLLS) {
                cardHistory.delete(definition.id);
                return;
            }
            renderedSample = prior.sample;
            stale = true;
            missing = prior.missing;
        }
        const element = definition.render(renderedSample);
        element.dataset.cardId = definition.id;
        const heading = element.querySelector('.widget-metric-label');
        if (heading) {
            heading.id = 'rapid-card-heading-' + definition.id;
            element.setAttribute('aria-labelledby', heading.id);
        }
        if (stale) {
            element.classList.add('is-stale');
            let chip = element.querySelector('.metric-live-chip');
            if (!chip) {
                chip = document.createElement('span');
                chip.className = 'metric-live-chip idle';
                element.querySelector('.metric-card-topline')?.append(chip);
            }
            chip.setAttribute('role', 'status');
            const staleText = staleLabel(sampledAtUnixMs, missing);
            chip.textContent = definition.id === 'runtime' ? 'degraded · ' + staleText : staleText;
        }
        renderedCardIds.add(definition.id);
        const existing = grid.querySelector(`[data-card-id="${definition.id}"]`);
        if (existing) {
            syncCard(existing, element);
        } else {
            grid.append(element);
        }
    });
    grid.querySelectorAll('[data-card-id]').forEach(element => {
        if (!renderedCardIds.has(element.dataset.cardId)) element.remove();
    });
    CARD_REGISTRY.slice().sort((a, b) => a.order - b.order).forEach(definition => {
        const element = grid.querySelector(`[data-card-id="${definition.id}"]`);
        if (element) grid.append(element);
    });
}

export const RAPID_MLX_CARD_IDS = CARD_REGISTRY.map(cardDefinition => cardDefinition.id);
