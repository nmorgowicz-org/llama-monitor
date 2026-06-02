// ── Setup / Monitor View ──────────────────────────────────────────────────────
// View transitions, animations, quick stats, and view state initialization.

import { setupViewState, chat, sessionState } from '../core/app-state.js';
import { doAttachFromSetup } from './attach-detach.js';
import { escapeHtml } from '../core/format.js';

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
    // no-op: launch cards use CSS animation-delay via inline style
}

// ── Attach drawer toggle ──────────────────────────────────────────────────────

let _attachDrawerOpen = false;

export function toggleAttachDrawer(forceOpen) {
    const drawer = document.getElementById('setup-attach-drawer');
    if (!drawer) return;
    _attachDrawerOpen = forceOpen !== undefined ? forceOpen : !_attachDrawerOpen;
    drawer.classList.toggle('open', _attachDrawerOpen);
    const btn = document.getElementById('setup-attach-remote-btn');
    if (btn) {
        btn.style.background = _attachDrawerOpen ? 'var(--neutral-soft-bg-strong)' : '';
        btn.style.borderColor = _attachDrawerOpen ? 'var(--neutral-soft-border)' : '';
    }
}

// ── Launch Grid — Preset Cards ────────────────────────────────────────────────

function _visiblePresetsLocal(presets) {
    const user = presets.filter(p => !p.id.startsWith('default-'));
    return user.length > 0 ? user : presets;
}

export function renderLaunchGrid() {
    const grid = document.getElementById('setup-launch-grid');
    if (!grid) return;
    grid.innerHTML = '';

    const allPresets = sessionState.presets || [];
    const userPresets = allPresets.filter(p => !p.id.startsWith('default-'));
    const hasUserPresets = userPresets.length > 0;
    const presets = _visiblePresetsLocal(allPresets);
    const activePresetId = document.getElementById('preset-select')?.value || '';

    if (!hasUserPresets) {
        // No user presets: New Config goes first so it's the obvious CTA
        const newCard = _buildNewConfigCard(true);
        newCard.style.animationDelay = '0ms';
        grid.appendChild(newCard);
        presets.forEach((preset, i) => {
            const card = _buildLaunchCard(preset, activePresetId);
            card.style.animationDelay = `${(i + 1) * 55}ms`;
            grid.appendChild(card);
        });
    } else {
        // User has presets: show them first (leftmost), New Config goes last
        presets.forEach((preset, i) => {
            const card = _buildLaunchCard(preset, activePresetId);
            card.style.animationDelay = `${i * 55}ms`;
            grid.appendChild(card);
        });
        const newCard = _buildNewConfigCard(false);
        newCard.style.animationDelay = `${presets.length * 55}ms`;
        grid.appendChild(newCard);
    }
}

function _buildLaunchCard(preset, activePresetId) {
    const isExample = preset.id.startsWith('default-');
    const card = document.createElement('div');
    card.className = 'launch-card';
    card.dataset.presetId = preset.id;
    if (isExample) card.classList.add('launch-card--example');

    // Only show running if the server is actually live and this preset is the active one
    const isRunning = !isExample && sessionState.serverRunning && preset.id === activePresetId && activePresetId;
    if (isRunning) card.classList.add('launch-card--running');

    const modelFile = (preset.model_path || '').split(/[/\\]/).pop() ||
                      (preset.hf_repo ? preset.hf_repo.split('/').slice(-1)[0] : '');
    const hasModel = !!modelFile;

    const ctxK = preset.context_size ? Math.round(preset.context_size / 1024) : 128;
    const ctxDisplay = ctxK >= 1000 ? `${(ctxK / 1024).toFixed(1)}M ctx` : `${ctxK}k ctx`;
    const ctkDisplay = (preset.ctk || 'q8_0') + '/' + (preset.ctv || 'f16');

    if (isExample) {
        // Example card: dimmed, no edit button, use-wizard CTA only
        // eslint-disable-next-line no-unsanitized/property -- content sanitized via escapeHtml
        card.innerHTML = `
            <div class="launch-card-top">
                <div class="launch-card-name">${escapeHtml(preset.name)}</div>
                <span class="launch-card-example-badge">Example</span>
            </div>
            <div class="launch-card-model launch-card-model--empty">Configure a model to use this</div>
            <div class="launch-card-chips">
                <span class="launch-chip">${ctxDisplay}</span>
                <span class="launch-chip">${ctkDisplay}</span>
            </div>
            <div class="launch-card-actions">
                <button class="launch-card-btn-start launch-card-btn-start--configure" type="button">
                    + New Configuration
                </button>
            </div>
        `;
        card.querySelector('.launch-card-btn-start').addEventListener('click', () => {
            import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard());
        });
    } else {
        // eslint-disable-next-line no-unsanitized/property -- content sanitized via escapeHtml
        card.innerHTML = `
            <div class="launch-card-top">
                <div class="launch-card-name">${escapeHtml(preset.name)}</div>
                ${isRunning ? '<span class="launch-card-running-badge">● Running</span>' : ''}
            </div>
            <div class="launch-card-model ${hasModel ? '' : 'launch-card-model--empty'}">${escapeHtml(modelFile || 'No model configured')}</div>
            <div class="launch-card-chips">
                <span class="launch-chip">${ctxDisplay}</span>
                <span class="launch-chip">${ctkDisplay}</span>
                ${preset.ngram_spec ? '<span class="launch-chip launch-chip--accent">n-gram</span>' : ''}
            </div>
            <div class="launch-card-actions">
                <button class="launch-card-btn-edit" type="button">Edit</button>
                <button class="launch-card-btn-start ${hasModel ? '' : 'launch-card-btn-start--configure'}" type="button">
                    ${hasModel ? '▶ Start' : '⚙ Configure'}
                </button>
                <button class="launch-card-btn-trash" type="button" title="Delete preset">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                         stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M3 6h18"/>
                        <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                        <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/>
                        <line x1="10" y1="11" x2="10" y2="17"/>
                        <line x1="14" y1="11" x2="14" y2="17"/>
                    </svg>
                </button>
            </div>
        `;

        card.querySelector('.launch-card-btn-edit').addEventListener('click', () => {
            const mainSel = document.getElementById('preset-select');
            if (mainSel) mainSel.value = preset.id;
            import('./presets.js').then(({ openPresetModal }) => openPresetModal('edit'));
        });

        card.querySelector('.launch-card-btn-trash').addEventListener('click', async (e) => {
            e.stopPropagation();
            if (!confirm(`Delete preset "${preset.name}"? This cannot be undone.`)) return;
            try {
                const headers = window.authHeaders ? window.authHeaders() : {};
                const resp = await fetch(`/api/presets/${preset.id}`, { method: 'DELETE', headers });
                if (resp.ok) {
                    await import('./presets.js').then(({ loadPresets }) => loadPresets());
                    renderLaunchGrid();
                }
            } catch (err) {
                console.error('Delete preset failed:', err);
            }
        });

        card.querySelector('.launch-card-btn-start').addEventListener('click', () => {
            if (!hasModel) {
                import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard());
                return;
            }
            const setupSel = document.getElementById('setup-preset-select');
            if (setupSel) setupSel.value = preset.id;
            const mainSel = document.getElementById('preset-select');
            if (mainSel) mainSel.value = preset.id;
            import('./attach-detach.js').then(({ doStartFromSetup }) => doStartFromSetup());
        });
    }

    return card;
}

function _buildNewConfigCard(isPrimary = false) {
    const card = document.createElement('div');
    card.className = 'launch-card launch-card--new' + (isPrimary ? ' launch-card--new-primary' : '');
    // eslint-disable-next-line no-unsanitized/property -- static HTML, no user data
        card.innerHTML = `
        <div class="launch-card-new-icon">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        </div>
        <div class="launch-card-new-label">New Configuration</div>
        ${isPrimary ? '<div class="launch-card-new-hint">Set up your first local model</div>' : ''}
    `;
    card.addEventListener('click', () => {
        import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard());
    });
    return card;
}

export function updateRunningCardHighlight() {
    const activePresetId = document.getElementById('preset-select')?.value || '';
    document.querySelectorAll('.launch-card[data-preset-id]').forEach(card => {
        const isRunning = sessionState.serverRunning && card.dataset.presetId === activePresetId && activePresetId;
        card.classList.toggle('launch-card--running', !!isRunning);
        const badge = card.querySelector('.launch-card-running-badge');
        if (badge) badge.style.display = isRunning ? '' : 'none';
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
    // Don't clobber the Connect button label — it's fixed in the two-pane layout
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

    // Live health-check attach sessions that aren't already confirmed Running
    attachSessions.forEach((session, i) => {
        if (session.status === 'Running') return;
        const endpoint = session.mode?.Attach?.endpoint;
        if (!endpoint) return;
        const card = list.children[i];
        if (!card) return;
        const dot = card.querySelector('.setup-endpoint-status');
        if (!dot) return;
        const authHdrs = window.authHeaders ? window.authHeaders() : {};
        const checkUrl = '/api/sessions/check-endpoint?url=' + encodeURIComponent(endpoint);
        fetch(checkUrl, { headers: authHdrs, signal: AbortSignal.timeout(6000) })
            .then(r => r.ok ? r.json() : null)
            .then(data => { if (data?.reachable) dot.className = 'setup-endpoint-status status-running'; })
            .catch(() => {});
    });
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

    // Mirror the main select (already filtered to visible presets by presets.js)
    setupSelect.innerHTML = '';
    const options = mainSelect.querySelectorAll('option');
    options.forEach(opt => {
        const clone = document.createElement('option');
        clone.value = opt.value;
        clone.textContent = opt.textContent;
        setupSelect.appendChild(clone);
    });
    setupSelect.value = mainSelect.value;

    // Also re-render the launch grid when presets change
    renderLaunchGrid();
}

// ── Initialization ────────────────────────────────────────────────────────────

export function initViewState() {
    if (document.body.classList.contains('setup-active')) return; // already initialized
    renderQuickStats();
    syncSetupPresetSelect(); // also calls renderLaunchGrid
    const lastEndpoint = localStorage.getItem('llama-monitor-last-endpoint');
    if (lastEndpoint) {
        const input = document.getElementById('setup-endpoint-url');
        if (input) input.value = lastEndpoint;
    }

    // Bind models button
    document.getElementById('setup-models-btn')?.addEventListener('click', () => {
        import('./models.js').then(({ openModelsModal }) => openModelsModal());
    });

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
