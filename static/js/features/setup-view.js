// ── Setup / Monitor View ──────────────────────────────────────────────────────
// View transitions, animations, quick stats, and view state initialization.

// ── View State ────────────────────────────────────────────────────────────────

const appState = {
    view: 'setup',
    sessionActive: false,
    lastSessionData: null
};

// ── View Switching ────────────────────────────────────────────────────────────

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

// ── Connecting State ──────────────────────────────────────────────────────────

function showConnectingState() {
    const connectingDots = document.getElementById('connecting-dots');
    if (connectingDots) connectingDots.style.display = '';
}

function hideConnectingState() {
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

// ── Session Data ──────────────────────────────────────────────────────────────

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

// ── Quick Stats ───────────────────────────────────────────────────────────────

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

// ── Initialization ────────────────────────────────────────────────────────────

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

// ── Public API ────────────────────────────────────────────────────────────────

export function initSetupView() {
    window.appState = appState;
    window.switchView = switchView;
    window.showConnectingState = showConnectingState;
    window.hideConnectingState = hideConnectingState;
    window.showFlashOverlay = showFlashOverlay;
    window.animateCardsEnter = animateCardsEnter;
    window.animateCardsExit = animateCardsExit;
    window.animateSetupCardsEnter = animateSetupCardsEnter;
    window.saveLastSessionData = saveLastSessionData;
    window.loadLastSessionData = loadLastSessionData;
    window.renderQuickStats = renderQuickStats;
    window.syncSetupPresetSelect = syncSetupPresetSelect;
    window.initViewState = initViewState;
}
