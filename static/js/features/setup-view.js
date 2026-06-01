// ── Setup / Monitor View ──────────────────────────────────────────────────────
// View transitions, animations, quick stats, and view state initialization.

import { setupViewState, chat } from '../core/app-state.js';
import { doAttachFromSetup } from './attach-detach.js';

function setAttachButtonLabel(button, label) {
    if (!button) return;
    const icon = document.createElement('span');
    icon.className = 'btn-icon';
    icon.textContent = '⚡';
    button.replaceChildren(icon, document.createTextNode(` ${label}`));
}

// ── View Switching ────────────────────────────────────────────────────────────

export function switchView(targetView) {
    if (setupViewState.view === 'transitioning') return;

    const previousView = setupViewState.view;
    setupViewState.view = 'transitioning';

    if (targetView === 'setup' && previousView === 'monitor') {
        savePreviousPosition();
    }

    const currentViewEl = document.getElementById('view-' + previousView);
    const targetViewEl = document.getElementById('view-' + targetView);
    const setupStrip = document.getElementById('endpoint-strip-setup');
    const monitorStrip = document.getElementById('endpoint-strip-monitor');

    if (!currentViewEl || !targetViewEl) {
        setupViewState.view = targetView;
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
                setupViewState.view = 'monitor';
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
                setupViewState.view = 'setup';
            }, 400);
        }, 600);
    }
}

// ── Connecting State ──────────────────────────────────────────────────────────

export function showConnectingState() {
    const connectingDots = document.getElementById('connecting-dots');
    if (connectingDots) connectingDots.style.display = '';
}

export function hideConnectingState() {
    const connectingDots = document.getElementById('connecting-dots');
    if (connectingDots) connectingDots.style.display = 'none';
}

// ── Animations ────────────────────────────────────────────────────────────────

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

// ── Recent Sessions ───────────────────────────────────────────────────────────

export async function loadRecentSessions() {
    try {
        const headers = window.authHeaders || (() => ({ 'Authorization': 'Bearer ' + (localStorage.getItem('llama-monitor-api-token') || '') }));
        const resp = await fetch('/api/sessions/recent', { headers: headers() });
        if (!resp.ok) return;
        const data = await resp.json();
        renderRecentEndpoints(data.sessions, data.active_session_id);
    } catch (e) {
        // Silent fail — first-run users won't have this endpoint
    }
}

export function renderRecentEndpoints(sessions, activeId) {
    const list = document.getElementById('setup-endpoint-list');
    const container = document.getElementById('setup-recent-endpoints');
    const spawnList = document.getElementById('setup-spawn-session-list');
    const spawnContainer = document.getElementById('setup-recent-spawn-sessions');
    const attachBtn = document.getElementById('setup-attach-btn');
    const lastSession = setupViewState.lastSessionData || loadLastSessionData();
    if (!list || !container || !spawnList || !spawnContainer) return;

    const allSessions = Array.isArray(sessions) ? sessions : [];
    const attachSessions = allSessions.filter(session => !!session.mode?.Attach);
    const spawnSessions = allSessions.filter(session => !!session.mode?.Spawn);

    if (!allSessions.length) {
        container.style.display = 'none';
        spawnContainer.style.display = 'none';
        setAttachButtonLabel(attachBtn, 'Attach');
        return;
    }

    container.style.display = attachSessions.length ? '' : 'none';
    spawnContainer.style.display = spawnSessions.length ? '' : 'none';
    setAttachButtonLabel(attachBtn, attachSessions.length ? 'Reconnect Manually' : 'Attach');
    list.innerHTML = '';
    spawnList.innerHTML = '';

    const buildCard = (session) => {
        const card = document.createElement('div');
        card.className = 'setup-endpoint-card';
        if (activeId && session.id === activeId) {
            card.classList.add('is-active-session');
        }

        let endpoint = '';
        let apiKey = '';
        if (session.mode && session.mode.Attach) {
            endpoint = session.mode.Attach.endpoint;
            apiKey = session.mode.Attach.api_key || '';
        } else if (session.mode && session.mode.Spawn) {
            endpoint = 'http://127.0.0.1:' + session.mode.Spawn.port;
            apiKey = session.mode.Spawn.api_key || '';
        }

        const statusClass = session.status === 'Running' ? 'status-running' :
                            session.status === 'Error' ? 'status-error' : 'status-disconnected';

        const lastConnected = session.last_connected_at ? formatRelativeTime(session.last_connected_at * 1000) : 'Never';
        const connectCount = session.connect_count || 0;

        const statusDot = document.createElement('div');
        statusDot.className = 'setup-endpoint-status ' + statusClass;

        const infoWrap = document.createElement('div');
        infoWrap.className = 'setup-endpoint-info';

        const nameEl = document.createElement('div');
        nameEl.className = 'setup-endpoint-name';
        nameEl.textContent = session.name || endpoint || 'Unnamed';

        const endpointEl = document.createElement('div');
        endpointEl.className = 'setup-endpoint-url';
        endpointEl.textContent = endpoint;
        endpointEl.title = endpoint;

        const metaEl = document.createElement('div');
        metaEl.className = 'setup-endpoint-meta';
        const metaParts = [];
        if (activeId && session.id === activeId) metaParts.push('Active workspace');
        else if (session.status === 'Running') metaParts.push('Last seen running');
        else if (session.status === 'Disconnected') metaParts.push('Ready to reconnect');
        else if (session.status === 'Error') metaParts.push(session.last_error || 'Needs attention');
        if (lastSession?.endpoint && endpoint && lastSession.endpoint === endpoint && lastSession.telemetryLabel) {
            metaParts.push(lastSession.telemetryLabel);
        }
        if (lastConnected !== 'Never') metaParts.push(lastConnected);
        if (connectCount > 0) metaParts.push(connectCount + 'x');
        if (session.mode?.Spawn) {
            metaParts.unshift('Local spawn session');
            if (session.mode.Spawn.bind_host === '0.0.0.0') metaParts.push('LAN visible');
            if (session.mode.Spawn.api_key) metaParts.push('API key saved');
        }
        let meta = metaParts.join(' · ');
        if (!meta) meta = 'Saved endpoint';
        metaEl.textContent = meta;

        infoWrap.appendChild(nameEl);
        infoWrap.appendChild(endpointEl);
        infoWrap.appendChild(metaEl);

        const connectBtn = document.createElement('button');
        connectBtn.className = 'setup-endpoint-connect';
        connectBtn.textContent = activeId && session.id === activeId
            ? 'Resume'
            : (session.last_connected_at ? 'Reconnect' : 'Connect');

        card.appendChild(statusDot);
        card.appendChild(infoWrap);
        card.appendChild(connectBtn);

        const doConnect = () => {
            const urlInput = document.getElementById('setup-endpoint-url');
            if (urlInput) urlInput.value = endpoint;
            const apiKeyInput = document.getElementById('setup-endpoint-api-key');
            if (apiKeyInput) apiKeyInput.value = apiKey;
            doAttachFromSetup();
        };
        connectBtn.addEventListener('click', (e) => { e.stopPropagation(); doConnect(); });
        card.addEventListener('click', doConnect);

        return card;
    };

    attachSessions.forEach(session => list.appendChild(buildCard(session)));
    spawnSessions.forEach(session => spawnList.appendChild(buildCard(session)));
}

function formatRelativeTime(ts) {
    const diff = Date.now() - ts;
    const seconds = Math.floor(diff / 1000);
    if (seconds < 60) return 'Just now';
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return minutes + 'm ago';
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return hours + 'h ago';
    const days = Math.floor(hours / 24);
    return days + 'd ago';
}

// ── Session Data ──────────────────────────────────────────────────────────────

export function saveLastSessionData(data) {
    const payload = { ...data, timestamp: Date.now() };
    localStorage.setItem('llama-monitor-last-session', JSON.stringify(payload));
    setupViewState.lastSessionData = payload;
}

export function loadLastSessionData() {
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

// ── Previous Position ─────────────────────────────────────────────────────────

export function savePreviousPosition() {
    const activePage = document.querySelector('.page.active');
    const navTab = activePage?.id?.replace('page-', '') || 'server';
    const chatTabId = navTab === 'chat' ? chat.activeTabId : null;
    const scrollPosition = activePage?.scrollTop || 0;

    const position = {
        view: setupViewState.view,
        navTab,
        chatTabId,
        scrollPosition,
        timestamp: Date.now(),
    };

    localStorage.setItem('llama-monitor-previous-position', JSON.stringify(position));
    setupViewState.previousPosition = position;
}

export function loadPreviousPosition() {
    try {
        const raw = localStorage.getItem('llama-monitor-previous-position');
        if (!raw) return null;
        const data = JSON.parse(raw);
        if (Date.now() - data.timestamp > 24 * 60 * 60 * 1000) {
            localStorage.removeItem('llama-monitor-previous-position');
            return null;
        }
        return data;
    } catch {
        return null;
    }
}

export function clearPreviousPosition() {
    localStorage.removeItem('llama-monitor-previous-position');
    setupViewState.previousPosition = null;
}

export async function restorePreviousPosition() {
    const position = loadPreviousPosition();
    if (!position) return;

    // Switch to saved nav tab
    if (position.navTab && position.navTab !== 'server') {
        const { switchTab } = await import('./nav.js');
        switchTab(position.navTab);
    }

    // Switch to saved chat tab
    if (position.chatTabId && position.navTab === 'chat') {
        const { switchChatTab } = await import('./chat-state.js');
        switchChatTab(position.chatTabId);
    }

    // Restore scroll position
    const activePage = document.querySelector('.page.active');
    if (activePage && position.scrollPosition > 0) {
        activePage.scrollTop = position.scrollPosition;
    }

    clearPreviousPosition();
}

// ── Quick Stats ───────────────────────────────────────────────────────────────

export function renderQuickStats() {
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

export function syncSetupPresetSelect() {
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

    // Show/hide the quick-start area depending on whether any real presets exist
    const quickStart = document.getElementById('setup-card-quick-start');
    if (quickStart) {
        const hasPresets = options.length > 0 && [...options].some(o => o.value !== '');
        quickStart.style.display = hasPresets ? '' : 'none';
    }
}

// ── Initialization ────────────────────────────────────────────────────────────

export function initViewState() {
    if (document.body.classList.contains('setup-active')) return; // already initialized
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
    loadRecentSessions();
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initSetupView() {
    // Initialize view state immediately — defensive functions return early if DOM not ready
    initViewState();
}
