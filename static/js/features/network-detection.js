// ── Network Detection ──────────────────────────────────────────────────────────
// Detects slow network conditions via the Network Information API and suggests
// adjusting the polling rate. Also monitors for connection changes.

import { showToast, showToastWithActions } from './toast.js';
import { wsData } from '../core/app-state.js';

let initialized = false;
let currentEffectiveType = null;
let hasShownSuggestion = false;
let networkCheckTimer = null;

// ── Network Quality Detection ─────────────────────────────────────────────────

// Mapping from effective connection type to recommended polling interval
const NETWORK_INTERVAL_MAP = {
    'slow-2g': 5000,
    '2g': 5000,
    '3g': 2000,
    '4g': 500,
};

const CADENCE_OPTIONS = [
    { value: 'auto', label: 'Auto', interval: null },
    { value: '500', label: 'Smooth 500ms', interval: 500 },
    { value: '1000', label: 'Balanced 1s', interval: 1000 },
    { value: '2000', label: 'Battery Saver 2s', interval: 2000 },
    { value: '5000', label: 'Low Power 5s', interval: 5000 },
];

const PRESSURE_SAMPLE_MS = 1000;
const PRESSURE_DRIFT_MS = 350;
const PRESSURE_CONSECUTIVE_LIMIT = 4;

let cadenceInitialized = false;
let pressureTimer = null;
let pressureExpectedAt = 0;
let pressureConsecutive = 0;
let pressureSuggestionShown = false;

/**
 * Returns the recommended polling interval based on current network conditions.
 * Falls back to 500ms (Normal) if the Network Information API is unavailable.
 */
export function getAutoPollingInterval() {
    const info = getNetworkInfo();

    // If Data Saver is on, use a conservative rate
    if (info.saveData) {
        return 2000;
    }

    // Use effective connection type
    const mapped = NETWORK_INTERVAL_MAP[info.effectiveType];
    if (mapped) {
        return mapped;
    }

    // Fall back to RTT-based detection
    if (info.rtt !== null) {
        if (info.rtt > 500) return 5000;
        if (info.rtt > 300) return 2000;
        if (info.rtt > 100) return 1000;
        return 500;
    }

    // Default: Normal
    return 500;
}

function formatInterval(ms) {
    return ms >= 1000 ? `${ms / 1000}s` : `${ms}ms`;
}

function getCadenceSelectValue() {
    return document.getElementById('settings-ws-push-interval')?.value || '500';
}

function getEffectiveCadenceMs(value = getCadenceSelectValue()) {
    if (value === 'auto') return getAutoPollingInterval();
    return parseInt(value, 10) || 500;
}

function getCadenceLabel(value = getCadenceSelectValue()) {
    if (value === 'auto') return `Auto ${formatInterval(getAutoPollingInterval())}`;
    const option = CADENCE_OPTIONS.find(item => item.value === value);
    return option ? option.label.replace(/^([^ ]+) /, '$1 ') : formatInterval(getEffectiveCadenceMs(value));
}

async function applyMonitoringCadence(value, source = 'manual') {
    const select = document.getElementById('settings-ws-push-interval');
    if (select) {
        select.value = value;
    }

    const interval = getEffectiveCadenceMs(value);
    updateCadenceChip();
    updateCadenceMenu();

    try {
        await fetch('/api/settings', {
            method: 'PUT',
            headers: window.authHeaders
                ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                : { 'Content-Type': 'application/json' },
            body: JSON.stringify({ ws_push_interval_ms: interval }),
        });
        window.dispatchEvent(new CustomEvent('monitoring-cadence-changed', {
            detail: { value, interval, source },
        }));
    } catch {
        showToast('Cadence update failed', 'error', 'Dashboard refresh rate could not be saved.');
    }
}

function hasActiveEndpoint() {
    return !!(wsData?.active_session_endpoint || wsData?.server_running || wsData?.local_server_running);
}

function updateCadenceChip() {
    const chip = document.getElementById('nav-cadence-chip');
    const valueEl = document.getElementById('nav-cadence-value');
    if (!chip) return;

    chip.style.display = hasActiveEndpoint() ? 'inline-flex' : 'none';
    const value = getCadenceSelectValue();
    const effective = getEffectiveCadenceMs(value);
    if (valueEl) valueEl.textContent = value === 'auto' ? `Auto ${formatInterval(effective)}` : formatInterval(effective);
    chip.setAttribute(
        'title',
        `Dashboard refresh cadence: ${getCadenceLabel(value)}. Controls WebSocket updates and local GPU telemetry pressure.`
    );
}

function updateCadenceMenu() {
    const menu = document.getElementById('nav-cadence-menu');
    if (!menu) return;
    const value = getCadenceSelectValue();
    menu.querySelectorAll('[data-cadence-value]').forEach(button => {
        button.classList.toggle('is-active', button.dataset.cadenceValue === value);
    });
}

function positionCadenceMenu() {
    const chip = document.getElementById('nav-cadence-chip');
    const menu = document.getElementById('nav-cadence-menu');
    if (!chip || !menu) return;

    const rect = chip.getBoundingClientRect();
    const width = menu.offsetWidth || 190;
    const left = Math.max(8, Math.min(window.innerWidth - width - 8, rect.right - width));
    menu.style.top = `${rect.bottom + 8}px`;
    menu.style.left = `${left}px`;
}

function closeCadenceMenu() {
    const chip = document.getElementById('nav-cadence-chip');
    const menu = document.getElementById('nav-cadence-menu');
    if (!chip || !menu) return;
    chip.setAttribute('aria-expanded', 'false');
    menu.setAttribute('aria-hidden', 'true');
}

function toggleCadenceMenu() {
    const chip = document.getElementById('nav-cadence-chip');
    const menu = document.getElementById('nav-cadence-menu');
    if (!chip || !menu) return;

    const opening = menu.getAttribute('aria-hidden') !== 'false';
    chip.setAttribute('aria-expanded', String(opening));
    menu.setAttribute('aria-hidden', String(!opening));
    updateCadenceMenu();
    if (opening) positionCadenceMenu();
}

function getNetworkInfo() {
    const connection = navigator.connection || navigator.mozConnection || navigator.webkitConnection;
    if (!connection) {
        return {
            effectiveType: 'unknown',
            rtt: null,
            downlink: null,
            saveData: false,
        };
    }
    return {
        effectiveType: connection.effectiveType || 'unknown',
        rtt: connection.rtt ?? null,
        downlink: connection.downlink ?? null,
        saveData: connection.saveData ?? false,
    };
}

function isSlowNetwork(networkInfo) {
    return ['slow-2g', '2g', '3g'].includes(networkInfo.effectiveType) ||
           networkInfo.saveData ||
           (networkInfo.rtt !== null && networkInfo.rtt > 300);
}

function getNetworkLabel(networkInfo) {
    switch (networkInfo.effectiveType) {
        case 'slow-2g': return 'Very Slow (2G)';
        case '2g': return 'Slow (2G)';
        case '3g': return 'Moderate (3G)';
        case '4g': return 'Good (4G)';
        default: return networkInfo.effectiveType === 'unknown' ? 'Detected' : networkInfo.effectiveType;
    }
}

// ── UI Updates ────────────────────────────────────────────────────────────────

function updateNetworkIndicator(networkInfo) {
    const indicator = document.getElementById('network-status-indicator');
    const icon = document.getElementById('network-status-icon');
    const text = document.getElementById('network-status-text');

    if (!indicator || !icon || !text) return;

    const slow = isSlowNetwork(networkInfo);
    const autoMode = document.getElementById('settings-ws-push-interval')?.value === 'auto';
    const effectiveInterval = autoMode ? getAutoPollingInterval() : null;

    if (networkInfo.effectiveType === 'unknown' && networkInfo.rtt === null && !autoMode) {
        indicator.style.display = 'none';
        return;
    }

    indicator.style.display = 'flex';
    indicator.className = 'network-status-indicator ' + (slow ? 'slow' : 'good');

    icon.textContent = slow ? '\u26a0\ufe0f' : '\u2705';

    let detail = '';
    if (networkInfo.rtt !== null) {
        detail += ` ~${networkInfo.rtt}ms latency`;
    }
    if (networkInfo.downlink !== null) {
        detail += ` · ${networkInfo.downlink} Mbps`;
    }
    if (networkInfo.saveData) {
        detail += ' · Data Saver on';
    }
    if (autoMode && effectiveInterval) {
        detail += ` · Polling: ${effectiveInterval >= 1000 ? (effectiveInterval / 1000) + 's' : effectiveInterval + 'ms'}`;
    }

    text.textContent = '';
    const strong = document.createElement('strong');
    strong.textContent = getNetworkLabel(networkInfo) + (autoMode ? ' (Auto)' : '');
    text.appendChild(strong);
    if (detail) {
        text.appendChild(document.createTextNode(detail));
    }
}

function suggestPollingAdjustment(networkInfo) {
 if (hasShownSuggestion) return;

 const currentInterval = document.getElementById('settings-ws-push-interval')?.value;

 // Don't suggest if already in auto mode or using slow preset
 if (currentInterval === 'auto') return;
 const currentMs = parseInt(currentInterval) || 500;
 if (currentMs > 500) return;

 hasShownSuggestion = true;
 const label = getNetworkLabel(networkInfo);
 showToastWithActions(
  'Slow network detected',
  'warning',
  `${label}. Auto cadence can reduce dashboard traffic while the connection is constrained.`,
  [{
   id: 'auto',
   label: 'Use Auto',
   primary: true,
   handler: () => applyMonitoringCadence('auto', 'network-suggestion'),
  }],
  { duration: 9000 }
 );
}

// ── Monitoring ────────────────────────────────────────────────────────────────

let lastAutoInterval = null;

function onNetworkChange() {
    if (document.hidden) return;

    const networkInfo = getNetworkInfo();
    const wasSlow = isSlowNetwork({ effectiveType: currentEffectiveType || '4g' });
    const isNowSlow = isSlowNetwork(networkInfo);
    const autoMode = document.getElementById('settings-ws-push-interval')?.value === 'auto';

    currentEffectiveType = networkInfo.effectiveType;
    updateNetworkIndicator(networkInfo);

    // If in auto mode and network changed, apply new interval
    if (autoMode) {
        const newInterval = getAutoPollingInterval();
        if (newInterval !== lastAutoInterval) {
            lastAutoInterval = newInterval;
            // Silently update the backend without triggering a full save
            fetch('/api/settings', {
                method: 'PUT',
                headers: window.authHeaders
                    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                    : { 'Content-Type': 'application/json' },
                body: JSON.stringify({ ws_push_interval_ms: newInterval }),
            }).catch(() => {});
        }
    }

    if (isNowSlow && !wasSlow) {
        suggestPollingAdjustment(networkInfo);
    }
}

function startNetworkMonitoring() {
    if (networkCheckTimer) return;
    networkCheckTimer = setInterval(onNetworkChange, 30000);
}

function stopNetworkMonitoring() {
    if (!networkCheckTimer) return;
    clearInterval(networkCheckTimer);
    networkCheckTimer = null;
}


// ── Cadence controls ──────────────────────────────────────────────────────────

function initCadenceControls() {
    if (cadenceInitialized) return;
    cadenceInitialized = true;

    const chip = document.getElementById('nav-cadence-chip');
    const menu = document.getElementById('nav-cadence-menu');
    const select = document.getElementById('settings-ws-push-interval');

    chip?.addEventListener('click', event => {
        event.stopPropagation();
        toggleCadenceMenu();
    });

    menu?.addEventListener('click', event => {
        const button = event.target.closest('[data-cadence-value]');
        if (!button) return;
        applyMonitoringCadence(button.dataset.cadenceValue || '500', 'nav');
        closeCadenceMenu();
    });

    document.addEventListener('click', event => {
        if (!menu || menu.getAttribute('aria-hidden') === 'true') return;
        if (menu.contains(event.target) || chip?.contains(event.target)) return;
        closeCadenceMenu();
    });

    window.addEventListener('resize', () => {
        if (menu?.getAttribute('aria-hidden') === 'false') positionCadenceMenu();
    });

    window.addEventListener('monitoring-cadence-request', event => {
        applyMonitoringCadence(event.detail?.value || '500', event.detail?.source || 'request');
    });

    window.addEventListener('settings-applied', updateCadenceChip);
    window.addEventListener('monitoring-cadence-changed', updateCadenceChip);
    select?.addEventListener('change', () => setTimeout(updateCadenceChip, 0));

    setInterval(updateCadenceChip, 1000);
    updateCadenceChip();
    updateCadenceMenu();
}

// ── Browser pressure detection ────────────────────────────────────────────────

function shouldSuggestPressureCadence() {
    if (pressureSuggestionShown || document.hidden || !hasActiveEndpoint()) return false;
    const currentMs = getEffectiveCadenceMs();
    return currentMs < 2000;
}

function suggestPressureCadence(driftMs) {
    if (!shouldSuggestPressureCadence()) return;
    pressureSuggestionShown = true;
    showToastWithActions(
        'Dashboard updates may be stressing this browser',
        'warning',
        `UI timers are running about ${Math.round(driftMs)}ms late. Battery Saver cadence reduces dashboard and GPU telemetry refresh pressure.`,
        [{
            id: 'battery-saver',
            label: 'Use 2s',
            primary: true,
            handler: () => applyMonitoringCadence('2000', 'browser-pressure'),
        }],
        { duration: 12000 }
    );
}

function sampleBrowserPressure() {
    const now = performance.now();
    if (pressureExpectedAt > 0) {
        const drift = now - pressureExpectedAt;
        if (drift > PRESSURE_DRIFT_MS && shouldSuggestPressureCadence()) {
            pressureConsecutive += 1;
            if (pressureConsecutive >= PRESSURE_CONSECUTIVE_LIMIT) {
                suggestPressureCadence(drift);
            }
        } else {
            pressureConsecutive = Math.max(0, pressureConsecutive - 1);
        }
    }

    pressureExpectedAt = now + PRESSURE_SAMPLE_MS;
    pressureTimer = window.setTimeout(sampleBrowserPressure, PRESSURE_SAMPLE_MS);
}

function startBrowserPressureMonitoring() {
    if (pressureTimer) return;
    pressureExpectedAt = performance.now() + PRESSURE_SAMPLE_MS;
    pressureTimer = window.setTimeout(sampleBrowserPressure, PRESSURE_SAMPLE_MS);
}

// ── Init ──────────────────────────────────────────────────────────────────────

export function initNetworkDetection() {
    if (initialized) return;
    initialized = true;

    const connection = navigator.connection || navigator.mozConnection || navigator.webkitConnection;

    // Run initial check
    onNetworkChange();

    // Listen for network changes
    if (connection) {
        connection.addEventListener('change', onNetworkChange);
    }

    // Also check on online/offline events
    window.addEventListener('online', () => {
        setTimeout(onNetworkChange, 500);
    });
    window.addEventListener('offline', () => {
        const indicator = document.getElementById('network-status-indicator');
        const icon = document.getElementById('network-status-icon');
        const text = document.getElementById('network-status-text');
        if (indicator) {
            indicator.style.display = 'flex';
            indicator.className = 'network-status-indicator slow';
        }
        if (icon) icon.textContent = '\u274c';
        if (text) {
            text.textContent = '';
            const strong = document.createElement('strong');
            strong.textContent = 'Offline';
            text.appendChild(strong);
            text.appendChild(document.createTextNode(' \u2014 No network connection'));
        }
    });

    document.addEventListener('visibilitychange', () => {
        if (document.hidden) {
            stopNetworkMonitoring();
            return;
        }
        onNetworkChange();
        startNetworkMonitoring();
    });

    // Periodic check (every 30s) for browsers that don't fire change events
    if (!document.hidden) {
        startNetworkMonitoring();
    }
}
