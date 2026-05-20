// ── Network Detection ──────────────────────────────────────────────────────────
// Detects slow network conditions via the Network Information API and suggests
// adjusting the polling rate. Also monitors for connection changes.

import { showToast } from './toast.js';

let initialized = false;
let currentEffectiveType = null;
let hasShownSuggestion = false;

// ── Network Quality Detection ─────────────────────────────────────────────────

// Mapping from effective connection type to recommended polling interval
const NETWORK_INTERVAL_MAP = {
    'slow-2g': 5000,
    '2g': 5000,
    '3g': 2000,
    '4g': 500,
};

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

    // Don't suggest if already in auto mode or using a slow preset
    if (currentInterval === 'auto') return;
    const currentMs = parseInt(currentInterval) || 500;
    if (currentMs > 500) return;

    hasShownSuggestion = true;
    const label = getNetworkLabel(networkInfo);
    showToast(
        `Slow network detected (${label}). Switching to Auto mode in Settings → Performance will adjust polling automatically.`,
        'warning',
        8000
    );
}

// ── Monitoring ────────────────────────────────────────────────────────────────

let lastAutoInterval = null;

function onNetworkChange() {
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

    // Periodic check (every 30s) for browsers that don't fire change events
    setInterval(onNetworkChange, 30000);
}
