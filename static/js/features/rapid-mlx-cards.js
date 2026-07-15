const STALE_POLLS = 3;
const cardHistory = new Map();
let parkedLlamaCards = null;
let lastPollSequence = null;
let lastSessionId = null;

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

function metric(label, value) {
    const row = document.createElement('div');
    row.className = 'rapid-metric';
    const name = document.createElement('span');
    name.textContent = label;
    const result = document.createElement('strong');
    result.textContent = value;
    row.append(name, result);
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

function staleLabel(sampledAtMs, missing) {
    const timestamp = Number(sampledAtMs);
    const age = Number.isFinite(timestamp) && timestamp > 0
        ? Math.max(0, Math.floor((Date.now() - timestamp) / 1000)) + 's ago'
        : 'last sample';
    return 'stale · ' + age + ' · ' + missing + '/3';
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
        chip.className = 'metric-live-chip ' + (state === 'stale' ? 'idle' : 'live');
        chip.textContent = state;
        top.append(chip);
    }
    const body = document.createElement('div');
    body.className = 'rapid-metric-list';
    rows.forEach(row => body.append(row));
    element.append(top, body);
    return element;
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
        render: s => card('Inference throughput', [
            ...(present(s.prompt_tokens_per_second) ? [metric('Prompt', formatNumber(s.prompt_tokens_per_second, 1) + ' t/s')] : []),
            ...(present(s.generation_tokens_per_second) ? [metric('Generation', formatNumber(s.generation_tokens_per_second, 1) + ' t/s')] : []),
        ], 'live'),
    },
    {
        id: 'queue', order: 30,
        available: s => anyPresent(s, ['running_requests', 'waiting_requests']),
        render: s => card('Request queue', [
            ...(present(s.running_requests) ? [metric('Running', formatNumber(s.running_requests))] : []),
            ...(present(s.waiting_requests) ? [metric('Waiting', formatNumber(s.waiting_requests))] : []),
        ], Number(s.running_requests) > 0 ? 'active' : 'idle'),
    },
    {
        id: 'memory', order: 40,
        available: s => anyPresent(s, ['active_memory_bytes', 'peak_memory_bytes', 'cache_memory_bytes']),
        render: s => card('Metal runtime memory', [
            ...(present(s.active_memory_bytes) ? [metric('Active', formatBytes(s.active_memory_bytes))] : []),
            ...(present(s.peak_memory_bytes) ? [metric('Peak', formatBytes(s.peak_memory_bytes))] : []),
            ...(present(s.cache_memory_bytes) ? [metric('Cache', formatBytes(s.cache_memory_bytes))] : []),
        ]),
    },
    {
        id: 'cache', order: 50,
        available: s => !!s.cache_metrics,
        render: s => card('Prefix & cache state', [
            ...(present(s.global_cache_hit_rate) ? [metric('Hit rate', formatNumber(s.global_cache_hit_rate * 100, 1) + '%')] : []),
            ...(present(s.global_cache_entries) ? [metric('Entries', formatNumber(s.global_cache_entries))] : []),
            ...(present(s.cache_metrics?.current_memory_bytes) ? [metric('Memory', formatBytes(s.cache_metrics.current_memory_bytes))] : []),
            ...(Array.isArray(s.cache_metrics?.multimodal_cache_kinds)
                && s.cache_metrics.multimodal_cache_kinds.length > 0
                ? [metric('Multimodal cache', 'Available')]
                : []),
        ]),
    },
    {
        id: 'totals', order: 60,
        available: s => anyPresent(s, ['completed_requests_total', 'prompt_tokens_total', 'completion_tokens_total', 'steps_executed']),
        render: s => card('Cumulative totals', [
            ...(present(s.completed_requests_total) ? [metric('Requests', formatNumber(s.completed_requests_total))] : []),
            ...(present(s.prompt_tokens_total) ? [metric('Prompt tokens', formatNumber(s.prompt_tokens_total))] : []),
            ...(present(s.completion_tokens_total) ? [metric('Completion tokens', formatNumber(s.completion_tokens_total))] : []),
            ...(present(s.steps_executed) ? [metric('Steps', formatNumber(s.steps_executed))] : []),
        ]),
    },
    {
        id: 'activity', order: 70,
        available: s => Array.isArray(s.active_requests) && s.active_requests.some(request => request && typeof request === 'object' && (present(request.id) || present(request.request_id) || present(request.status))),
        render: s => card('Request activity', [metric('Recognized requests', formatNumber(s.active_requests.filter(request => request && typeof request === 'object' && (present(request.id) || present(request.request_id) || present(request.status))).length))], 'active'),
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
    grid.replaceChildren();
    if (!sample) {
        const loading = document.createElement('div');
        loading.className = 'rapid-telemetry-loading';
        loading.dataset.telemetryState = 'loading';
        loading.setAttribute('role', 'status');
        loading.setAttribute('aria-live', 'polite');
        loading.textContent = 'Connecting to Rapid-MLX telemetry…';
        grid.append(loading);
        return;
    }

    const isNewPoll = pollSequence !== lastPollSequence;
    lastPollSequence = pollSequence;
    const availabilitySample = pollFailed ? {
        health: sample.health,
        model: sample.model,
        uptime_seconds: sample.uptime_seconds,
        ready: sample.ready,
    } : sample;
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
        grid.append(element);
    });
}

export const RAPID_MLX_CARD_IDS = CARD_REGISTRY.map(cardDefinition => cardDefinition.id);
