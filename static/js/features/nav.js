// ── Navigation ────────────────────────────────────────────────────────────────
// Tab switching and sidebar collapse.

import { chat, contextCapacityTokens, lastLlamaMetrics, metricSeries, setWsData, wsData } from '../core/app-state.js';
import { chatScroll } from './chat-render.js';
import { showSessionPanel, hideSessionPanel } from './chat-sessions-sidebar.js';
import { isFocusModeActive, exitFocusMode } from './chat-focus-mode.js';
import { renderCapabilityPopover } from './dashboard-render.js';
import { showToast } from './toast.js';

export function switchTab(name) {
    if (name !== 'chat' && isFocusModeActive()) exitFocusMode();

    const page = document.getElementById('page-' + name);

    // Handle modal tabs (no corresponding page div)
    if (!page) {
        document.querySelectorAll('.sidebar-btn').forEach(b => b.classList.remove('active'));
        const sidebarButton = document.querySelector(`.sidebar-btn[data-tab="${name}"]`);
        if (sidebarButton) sidebarButton.classList.add('active');
        if (name === 'settings') window.openSettingsModal?.();
        return;
    }

    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    document.querySelectorAll('.sidebar-btn').forEach(b => b.classList.remove('active'));

    page.classList.add('active');

    const sidebarButton = document.querySelector(`.sidebar-btn[data-tab="${name}"]`);
    if (sidebarButton) sidebarButton.classList.add('active');

    if (name === 'chat') {
        showSessionPanel();
    } else {
        hideSessionPanel();
    }

    // Scroll chat to bottom when entering chat page (no tab switch = no re-render)
    if (name === 'chat') {
        setTimeout(() => chatScroll(true), 50);
    }
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

// ── Endpoint status popover ──────────────────────────────────────────────────

function initEndpointStatus() {
    const endpointStatus = document.getElementById('endpoint-status');
    const endpointStatusWrap = endpointStatus?.closest('.endpoint-status-wrap');
    const popover = document.getElementById('capability-popover');
    if (!endpointStatus || !endpointStatusWrap || !popover) return;

    function positionPopover() {
        const rect = endpointStatusWrap.getBoundingClientRect();
        popover.style.top = (rect.bottom + 8) + 'px';
        popover.style.left = Math.min(rect.left, window.innerWidth - 370) + 'px';
    }

    endpointStatus.addEventListener('click', event => {
        event.stopPropagation();
        const open = endpointStatusWrap.classList.toggle('open');
        endpointStatus.setAttribute('aria-expanded', open ? 'true' : 'false');
        if (open) {
            renderCapabilityPopover(wsData, wsData?.llama);
            positionPopover();
        }
    });

    document.addEventListener('click', event => {
        if (!event.target.closest('.endpoint-status-wrap')) {
            endpointStatusWrap.classList.remove('open');
            endpointStatus.setAttribute('aria-expanded', 'false');
        }
    });
}

function deriveTabCtxPct(tab, capacity) {
    if (!tab || !capacity) return 0;
    const asst = (tab.messages || []).filter(m => m.role === 'assistant' && !m.compaction_marker);
    if (!asst.length) return tab.last_ctx_pct || 0;
    // Use tab-level cumulative totals (most accurate); fall back to summing message fields.
    const totalInput = tab.total_input_tokens
        || asst.reduce((sum, m) => sum + (m.input_tokens || 0), 0);
    const totalOutput = tab.total_output_tokens
        || asst.reduce((sum, m) => sum + (m.output_tokens || 0), 0);
    return Math.min(200, (totalInput + totalOutput) / capacity * 100);
}

function buildCockpitSparkline(points) {
    if (!points || points.length < 2) return '';
    const width = 120;
    const height = 28;
    const max = Math.max(...points, 1);
    const step = width / (points.length - 1);
    const currentValue = points[points.length - 1];
    const currentX = width;
    const currentY = height - ((currentValue / max) * (height - 6)) - 3;
    const path = points.map((value, index) => {
        const x = index * step;
        const y = height - ((value / max) * (height - 6)) - 3;
        return (index === 0 ? 'M' : 'L') + x.toFixed(2) + ' ' + y.toFixed(2);
    }).join(' ');
    return [
        '<path class="sparkline-fill live-output" d="' + path + ' L 120 28 L 0 28 Z" fill="currentColor"></path>',
        '<path class="sparkline-line live-output" d="' + path + '"></path>',
        '<line class="sparkline-current-trace live-output" x1="' + Math.max(currentX - 8, 0).toFixed(2) + '" y1="' + currentY.toFixed(2) + '" x2="' + currentX.toFixed(2) + '" y2="' + currentY.toFixed(2) + '"></line>',
        '<circle class="sparkline-current-halo live-output" cx="' + currentX.toFixed(2) + '" cy="' + currentY.toFixed(2) + '" r="7.4"></circle>',
        '<circle class="sparkline-current live-output" cx="' + currentX.toFixed(2) + '" cy="' + currentY.toFixed(2) + '" r="3.6"></circle>',
        '<circle class="sparkline-current-core live-output" cx="' + currentX.toFixed(2) + '" cy="' + currentY.toFixed(2) + '" r="1.2"></circle>',
    ].join('');
}

function buildIdleCockpitPoints() {
    return [0.18, 0.42, 0.3, 0.54, 0.36, 0.62, 0.4, 0.5, 0.34, 0.46];
}

export function refreshTopCockpit() {
    const cockpit = document.getElementById('nav-cockpit');
    if (!cockpit) return;

    const stateEl = document.getElementById('nav-cockpit-state');
    const throughputEl = document.getElementById('nav-cockpit-throughput');
    const specEl = document.getElementById('nav-cockpit-spec');
    const contextEl = document.getElementById('nav-cockpit-context');
    const gpuEl = document.getElementById('nav-cockpit-gpu');
    const sparkEl = document.getElementById('nav-cockpit-spark');

    const hasActiveEndpoint = !!wsData?.active_session_id;
    const l = hasActiveEndpoint ? lastLlamaMetrics : null;
    const promptRate = l?.prompt_tokens_per_sec || 0;
    const genRate = l?.generation_tokens_per_sec || 0;
    const promptDisplayRate = promptRate > 0 ? promptRate : (l?.last_prompt_tokens_per_sec || 0);
    const genDisplayRate = genRate > 0 ? genRate : (l?.last_generation_tokens_per_sec || 0);
    const generationActive = !!l?.slot_generation_active || (l?.slots_processing || 0) > 0 || genRate > 0;

    // T-057: cockpit pill modes: Active / Low Power (manual) / Low Power (auto) / idle
    const isSleeping = wsData?.sleep_mode === true;
    const isManualSleep = isSleeping && wsData?.sleep_mode_manual === true;
    let label = 'idle';
    let stateClass = 'idle';

    if (isSleeping) {
        label = 'low power';
        stateClass = 'sleep';
    } else if (!hasActiveEndpoint) {
        label = 'attach';
    } else if (promptRate > 0 && genRate <= 0) {
        label = 'prompting';
        stateClass = 'live';
    } else if (generationActive) {
        label = 'generating';
        stateClass = 'live';
    }

    if (stateEl) {
        stateEl.textContent = label;
        stateEl.className = 'metric-live-chip nav-cockpit-state ' + stateClass;
    }
    cockpit.classList.toggle('is-live', stateClass === 'live');
    cockpit.classList.toggle('is-idle', stateClass !== 'live' && stateClass !== 'sleep');
    cockpit.classList.toggle('is-low-power', stateClass === 'sleep');
    cockpit.classList.toggle('is-low-power-manual', isManualSleep);
    cockpit.classList.toggle('has-session', hasActiveEndpoint);

    // Keep Low Power pill tooltip and label in sync with actual mode
    const sleepPill = document.getElementById('nav-sleep-pill');
    const sleepPillLabel = document.getElementById('nav-sleep-pill-label');
    if (sleepPillLabel) {
        sleepPillLabel.textContent = isSleeping ? 'Sleeping' : 'Low Power';
    }
    if (sleepPill) {
        const unavailable = sleepPill.getAttribute('data-unavailable') === 'true';
        sleepPill.disabled = !hasActiveEndpoint || unavailable;
        sleepPill.setAttribute('aria-pressed', isSleeping ? 'true' : 'false');
        if (unavailable) {
            sleepPill.setAttribute('title', 'Low Power control is not available on this server.');
            sleepPill.setAttribute('aria-label', 'Low Power unavailable');
        } else if (!hasActiveEndpoint) {
            sleepPill.setAttribute('title', 'Low Power is available after a local server session is active.');
            sleepPill.setAttribute('aria-label', 'Low Power unavailable until a local server session is active');
        } else if (isManualSleep) {
            sleepPill.setAttribute(
                'title',
                'Low Power: ON (manual) — llama-server stays running; click to resume full telemetry.'
            );
            sleepPill.setAttribute('aria-label', 'Turn Low Power off');
        } else if (isSleeping) {
            sleepPill.setAttribute(
                'title',
                'Low Power: ON (auto) — Idle timeout triggered; click to resume full telemetry.'
            );
            sleepPill.setAttribute('aria-label', 'Turn Low Power off');
        } else {
            sleepPill.setAttribute(
                'title',
                'Low Power: OFF — llama-server keeps running with minimal overhead; click to reduce telemetry.'
            );
            sleepPill.setAttribute('aria-label', 'Turn Low Power on');
        }
    }

    if (throughputEl) {
        throughputEl.textContent = 'P ' + (promptDisplayRate > 0 ? promptDisplayRate.toFixed(0) : '—') + ' · G ' + (genDisplayRate > 0 ? genDisplayRate.toFixed(0) : '—');
    }

    if (specEl) {
        const tpd = l?.tokens_per_decode ?? 0;
        if (tpd > 1.05) {
            specEl.textContent = tpd.toFixed(2) + '× S';
            specEl.classList.remove('hidden');
        } else {
            specEl.classList.add('hidden');
        }
    }

    const capacity = hasActiveEndpoint ? (contextCapacityTokens || l?.context_capacity_tokens || l?.kv_cache_max || 0) : 0;
    let worstCtx = 0;
    if (capacity > 0) {
        worstCtx = (chat.tabs || []).reduce((max, tab) => Math.max(max, deriveTabCtxPct(tab, capacity)), 0);
    }
    if (contextEl) {
        contextEl.textContent = 'Ctx ' + (worstCtx > 0 ? Math.round(worstCtx) + '%' : '—');
        contextEl.title = worstCtx > 0 ? 'Highest chat context pressure across tabs' : 'No live context pressure available';
    }

    const gpuEntries = Object.values(wsData?.gpu || {});
    const hottestGpu = gpuEntries.length > 0
        ? Math.max(...gpuEntries.map(m => Number(m?.temp) || 0))
        : 0;
    if (gpuEl) {
        gpuEl.textContent = 'GPU ' + (hottestGpu > 0 ? hottestGpu.toFixed(0) + 'C' : '—');
    }

    if (sparkEl) {
        const promptPoints = hasActiveEndpoint ? (metricSeries.prompt || []) : [];
        const genPoints = hasActiveEndpoint ? (metricSeries.generation || []) : [];
        const livePoints = hasActiveEndpoint ? (metricSeries.liveOutput || []) : [];
        const maxLen = Math.max(promptPoints.length, genPoints.length);
        const points = [];
        for (let i = 0; i < maxLen; i += 1) {
            const p = promptPoints[promptPoints.length - maxLen + i] || 0;
            const g = genPoints[genPoints.length - maxLen + i] || 0;
            points.push(Math.max(p, g));
        }
        const fallbackPoints = livePoints.filter(value => value > 0);
        let displayPoints = points.some(value => value > 0) ? points : fallbackPoints;
        if (displayPoints.length < 2) {
            displayPoints = buildIdleCockpitPoints();
        }
        // eslint-disable-next-line no-unsanitized/property -- SVG markup is generated internally from numeric series values only
        sparkEl.innerHTML = displayPoints.length >= 2 ? buildCockpitSparkline(displayPoints) : '';
    }
}

// ── Sidebar drag-resize ───────────────────────────────────────────────────────

const SIDEBAR_RESIZE_KEY = 'appNavWidth';
const SIDEBAR_MIN = 140;
const SIDEBAR_MAX = 320;

function setSidebarWidth(px) {
    const clamped = Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, px));
    document.documentElement.style.setProperty('--sidebar-width-expanded', clamped + 'px');
    localStorage.setItem(SIDEBAR_RESIZE_KEY, clamped);
}

function initSidebarResize() {
    const handle = document.getElementById('sidebar-resize-handle');
    const sidebar = document.getElementById('sidebar-nav');
    if (!handle || !sidebar) return;

    const saved = Number(localStorage.getItem(SIDEBAR_RESIZE_KEY));
    if (saved >= SIDEBAR_MIN && saved <= SIDEBAR_MAX) {
        setSidebarWidth(saved);
    }

    let startX = 0;
    let startWidth = 0;

    handle.addEventListener('mousedown', e => {
        if (e.button !== 0) return;
        e.preventDefault();
        startX = e.clientX;
        startWidth = sidebar.getBoundingClientRect().width;
        sidebar.classList.add('is-resizing');
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
    });

    document.addEventListener('mousemove', e => {
        if (!sidebar.classList.contains('is-resizing')) return;
        setSidebarWidth(startWidth + (e.clientX - startX));
    });

    document.addEventListener('mouseup', () => {
        if (!sidebar.classList.contains('is-resizing')) return;
        sidebar.classList.remove('is-resizing');
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
    });
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initNav() {
    // Bind sidebar tab switching
    document.querySelectorAll('.sidebar-btn[data-tab]').forEach(btn => {
        btn.addEventListener('click', () => switchTab(btn.dataset.tab));
    });

    // Bind sidebar collapse
    const collapseBtn = document.getElementById('sidebar-collapse-btn');
    if (collapseBtn) {
        collapseBtn.addEventListener('click', toggleSidebarCollapse);
    }

    // Bind nav logo (prevent default link navigation)
    const navLogo = document.getElementById('nav-logo');
    if (navLogo) {
        navLogo.addEventListener('click', event => event.preventDefault());
    }

    const cockpit = document.getElementById('nav-cockpit');
    if (cockpit) {
        cockpit.addEventListener('click', () => switchTab('server'));
        cockpit.addEventListener('keydown', (e) => {
            if (e.key !== 'Enter' && e.key !== ' ') return;
            e.preventDefault();
            switchTab('server');
        });
    }

    const sleepPill = document.getElementById('nav-sleep-pill');
    if (sleepPill) {
        sleepPill.addEventListener('click', async (e) => {
            e.stopPropagation();

            // If previously marked unavailable (e.g., 404), do nothing further
            if (sleepPill.getAttribute('data-unavailable') === 'true') return;

            // Avoid accidental double-taps
            if (sleepPill.getAttribute('data-disabled') === 'true') return;
            sleepPill.setAttribute('data-disabled', 'true');

            const wasSleeping = wsData?.sleep_mode === true;
            console.warn('[sleep] toggle click: wasSleeping=', wasSleeping, 'wsData.sleep_mode_manual=', wsData?.sleep_mode_manual);

            try {
                const auth = window.authHeaders ? window.authHeaders() : {};
                const res = await fetch('/api/sleep-mode/toggle', {
                    method: 'POST',
                    headers: {
                        ...auth,
                        'Content-Type': 'application/json',
                    },
                });

                if (!res.ok) {
                    if (res.status === 404) {
                        showToast('Low Power control is not available on this server.', 'info');
                        // Disable pill to avoid repeated failed clicks
                        sleepPill.setAttribute('data-unavailable', 'true');
                        sleepPill.disabled = true;
                        sleepPill.style.opacity = '0.4';
                        sleepPill.style.cursor = 'not-allowed';
                        sleepPill.setAttribute(
                            'title',
                            'Low Power control is not available on this server.'
                        );
                    } else {
                        showToast('Failed to toggle Low Power mode.', 'error');
                    }
                    return;
                }

                // Attempt to read the new mode from the response
                let nextSleeping = wasSleeping; // fallback
                try {
                    const data = await res.json();
                    if (data != null && typeof data.sleep_mode === 'boolean') {
                        nextSleeping = data.sleep_mode;
                    } else if (data != null && typeof data.enabled === 'boolean') {
                        nextSleeping = data.enabled;
                    }
                } catch (_) {
                    // If not JSON, assume toggled
                    nextSleeping = !wasSleeping;
                }

                console.warn('[sleep] API response: nextSleeping=', nextSleeping);
                // Reflect change in local state immediately (WS will confirm)
                setWsData({
                    ...(wsData || {}),
                    sleep_mode: nextSleeping,
                    sleep_mode_manual: nextSleeping,
                });
                console.warn('[sleep] optimistic wsData set, calling refreshTopCockpit');
                refreshTopCockpit();
                sleepPill.setAttribute('aria-pressed', nextSleeping ? 'true' : 'false');

                if (nextSleeping) {
                    showToast('Low Power: ON — llama-server keeps running; telemetry paused.', 'success');
                } else {
                    showToast('Low Power: OFF — full telemetry resumed.', 'success');
                }
            } catch (err) {
                showToast('Low Power toggle failed (network error).', 'error');
            } finally {
                // Allow re-toggling after a short debounce (unless marked unavailable)
                if (sleepPill.getAttribute('data-unavailable') !== 'true') {
                    setTimeout(() => {
                        sleepPill.removeAttribute('data-disabled');
                    }, 600);
                }
            }
        });
    }

    restoreSidebarState();
    initSidebarResize();
    initEndpointStatus();
    refreshTopCockpit();
}
