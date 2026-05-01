// ── Sessions ───────────────────────────────────────────────────────────────────
// Session CRUD: load, switch, delete, modal management.

// ── Load ───────────────────────────────────────────────────────────────────────

export async function loadSessions() {
    try {
        const resp = await fetch('/api/sessions');
        window.sessions = await resp.json();
        renderSessionList();

        const lastAttach = window.sessions
            .filter(s => s.mode && s.mode.Attach)
            .sort((a, b) => b.last_active - a.last_active)[0];

        if (lastAttach) {
            const endpointInput = document.getElementById('server-endpoint');
            if (endpointInput) {
                endpointInput.value = lastAttach.mode.Attach.endpoint;
                endpointInput.dataset.preserved = '1';
                if (window.saveSettings) window.saveSettings();
            }
        }
    } catch (err) {
        console.error('Failed to load sessions:', err);
    }
}

// ── Render ─────────────────────────────────────────────────────────────────────

export function renderSessionList() {
    const list = document.getElementById('sessions-list');
    const empty = document.getElementById('sessions-empty');
    if (!list) return;

    if (window.sessions.length === 0) {
        list.innerHTML = '';
        if (empty) empty.style.display = 'block';
        return;
    }
    if (empty) empty.style.display = 'none';

    list.innerHTML = window.sessions.map(s => {
        const is_active = s.id === window.activeSessionId;
        const isAttach = s.mode && s.mode.Attach;
        const isSpawn = s.mode && s.mode.Spawn;
        const modeText = isSpawn ? 'Spawn' : 'Attach';
        const modeIcon = isSpawn ? '🖥' : '🔗';
        const endpoint = isAttach ? s.mode.Attach.endpoint : '';
        const port = isSpawn ? s.mode.Spawn.port : '';
        const presetId = s.preset_id || '';
        const presetObj = window.presets.find(p => p.id === presetId);
        const presetName = presetObj ? presetObj.name : (isSpawn ? '(no preset)' : '');
        const statusText = s.status === 'Running' ? 'Running' :
                           s.status === 'Stopped' ? 'Stopped' :
                           s.status === 'Disconnected' ? 'Disconnected' : (s.status || '');

        const name = window.escapeHtml(s.name);
        const detailText = modeText + (port ? ' : ' + port : '') + (isSpawn && presetName ? ' · ' + window.escapeHtml(presetName) : '') + (endpoint ? ' · ' + window.escapeHtml(endpoint) : '');
        const statusHtml = statusText ? '<span class="session-item-status">' + window.escapeHtml(statusText) + '</span>' : '';

        let actionsHtml = '';
        if (isAttach) {
            actionsHtml += `<button class="btn-sm btn-preset" data-action="connect" data-endpoint="${window.escapeHtml(endpoint)}">Connect</button>`;
        }
        if (isSpawn) {
            actionsHtml += `<button class="btn-sm btn-preset" data-action="start" data-session-id="${window.escapeHtml(s.id)}">Start</button>`;
        }
        actionsHtml += `<button class="btn-sm btn-preset btn-preset-delete" data-action="delete" data-session-id="${window.escapeHtml(s.id)}">✕</button>`;

        return `<div class="session-item${is_active ? ' active' : ''}">` +
            `<div class="session-item-main" data-session-id="${window.escapeHtml(s.id)}">` +
            '<span class="session-item-icon">' + modeIcon + '</span>' +
            '<div class="session-item-info">' +
            '<span class="session-item-name">' + name + '</span>' +
            '<span class="session-item-detail">' + detailText + '</span>' +
            '</div>' +
            statusHtml +
            '</div>' +
            '<div class="session-item-actions">' +
            actionsHtml +
            '</div>' +
            '</div>';
    }).join('');
}

// ── Quick actions ──────────────────────────────────────────────────────────────

export function quickAttachSession(endpoint) {
    const serverEndpoint = document.getElementById('server-endpoint');
    if (serverEndpoint) serverEndpoint.value = endpoint;
    localStorage.setItem('llama-monitor-last-endpoint', endpoint);
    closeSessionModal();
    if (window.showConnectingState) window.showConnectingState();
    if (window.doAttach) window.doAttach();
}

export function quickStartSession(sessionId) {
    closeSessionModal();
    switchSession(sessionId);
    if (window.showConnectingState) window.showConnectingState();
    if (window.doStart) window.doStart();
}

// ── CRUD ───────────────────────────────────────────────────────────────────────

export async function deleteSession(sessionId) {
    if (!confirm('Delete this session?')) return;
    try {
        const resp = await fetch('/api/sessions/' + encodeURIComponent(sessionId), { method: 'DELETE' });
        const data = await resp.json();
        if (data.ok) {
            window.showToast('Session deleted', 'success');
            loadSessions();
        } else {
            window.showToast('Delete failed: ' + (data.error || 'unknown'), 'error');
        }
    } catch (e) {
        window.showToast('Delete failed: ' + e.message, 'error');
    }
}

export async function switchSession(sessionId) {
    try {
        const resp = await fetch('/api/sessions/active', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ id: sessionId })
        });
        const data = await resp.json();
        if (data.ok) {
            window.activeSessionId = sessionId;
            renderSessionList();
            window.showToast('Switched to session', 'success');
            if (window.loadPresets) window.loadPresets();
        } else {
            window.showToast('Failed to switch session: ' + data.error, 'error');
        }
    } catch (err) {
        window.showToast('Failed to switch session: ' + err.message, 'error');
    }
}

// ── Modal ──────────────────────────────────────────────────────────────────────

export function openSessionModal() {
    const modal = document.getElementById('session-modal');
    const title = document.getElementById('session-modal-title');
    title.textContent = 'Sessions';
    modal.classList.add('open');
    showSessionsList();
}

export function showNewSessionForm() {
    document.getElementById('sessions-list-view').style.display = 'none';
    document.getElementById('sessions-new-form').style.display = 'block';
    const newBtn = document.getElementById('btn-new-session');
    if (newBtn) newBtn.style.display = 'none';
    document.getElementById('session-form').reset();
    document.getElementById('modal-session-mode').value = 'spawn';
    updateSessionModalMode();
}

export function showSessionsList() {
    document.getElementById('sessions-list-view').style.display = 'block';
    document.getElementById('sessions-new-form').style.display = 'none';
    const newBtn = document.getElementById('btn-new-session');
    if (newBtn) newBtn.style.display = 'inline-block';
    renderSessionList();
}

export function updateSessionModalMode() {
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
        input.value = window.activeSessionPort || 8001;
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

export function closeSessionModal() {
    document.getElementById('session-modal').classList.remove('open');
}

export function saveSession(event) {
    event.preventDefault();

    const mode = document.getElementById('modal-session-mode').value;
    const name = document.getElementById('modal-session-name').value.trim();

    if (!name) {
        window.showToast('Please enter a session name', 'error');
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
        window.showToast('Please enter an endpoint', 'error');
        return;
    }

    if (mode === 'spawn' && !modalPresetId && !presetId) {
        window.showToast('Select a model preset before creating a spawn session', 'error');
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
            window.showToast(mode === 'attach' ? 'Attached to endpoint' : 'Session created', 'success');
        } else {
            window.showToast('Failed to create session: ' + data.error, 'error');
        }
    })
    .catch(err => window.showToast('Failed to create session: ' + err.message, 'error'));
}

// ── Active session info ────────────────────────────────────────────────────────

export async function updateActiveSessionInfo() {
    try {
        const resp = await fetch('/api/sessions/active');
        const data = await resp.json();
        if (data && data.mode) {
            const modeParts = data.mode.split(':');
            if (modeParts[0] === 'Spawn') {
                window.activeSessionPort = parseInt(modeParts[1]) || 8080;
            } else if (modeParts[0] === 'Attach') {
                const endpoint = modeParts.slice(1).join(':');
                try {
                    const url = new URL(endpoint);
                    window.activeSessionPort = parseInt(url.port) || 8080;
                } catch(e) {
                    window.activeSessionPort = 8080;
                }
                const endpointInput = document.getElementById('server-endpoint');
                if (endpointInput && endpointInput.value !== endpoint) {
                    endpointInput.value = endpoint;
                    if (window.saveSettings) window.saveSettings();
                }
            }
        }
    } catch (err) {
        console.error('Failed to update active session info:', err);
    }
}

// ── Init ───────────────────────────────────────────────────────────────────────

export function initSessions() {
    // Bind session modal buttons
    document.getElementById('session-modal-close')?.addEventListener('click', closeSessionModal);
    document.getElementById('session-modal-cancel')?.addEventListener('click', closeSessionModal);
    document.getElementById('btn-new-session')?.addEventListener('click', showNewSessionForm);
    document.getElementById('session-create-first')?.addEventListener('click', showNewSessionForm);
    document.getElementById('session-browse-model-btn')?.addEventListener('click', () => window.openFileBrowser('modal-session-model-path', 'gguf'));

    // Bind session form submit
    const sessionForm = document.getElementById('session-form');
    if (sessionForm) sessionForm.addEventListener('submit', saveSession);

    // Bind sidebar sessions button
    document.getElementById('sidebar-btn-sessions')?.addEventListener('click', openSessionModal);

    // Bind nav new session button
    document.getElementById('nav-new-session-btn')?.addEventListener('click', openSessionModal);

    // Bind setup view link
    document.getElementById('setup-browse-sessions-link')?.addEventListener('click', (e) => {
        e.preventDefault();
        openSessionModal();
    });

    // Event delegation for dynamically generated session list
    const sessionsList = document.getElementById('sessions-list');
    if (sessionsList) {
        sessionsList.addEventListener('click', (e) => {
            const btn = e.target.closest('[data-action]');
            if (btn) {
                e.stopPropagation();
                const action = btn.dataset.action;
                if (action === 'connect') {
                    quickAttachSession(btn.dataset.endpoint);
                } else if (action === 'start') {
                    quickStartSession(btn.dataset.sessionId);
                } else if (action === 'delete') {
                    deleteSession(btn.dataset.sessionId);
                }
                return;
            }

            const itemMain = e.target.closest('.session-item-main');
            if (itemMain) {
                switchSession(itemMain.dataset.sessionId);
                return;
            }
        });
    }

    // Mode change listener
    const modeSelect = document.getElementById('modal-session-mode');
    if (modeSelect) {
        modeSelect.addEventListener('change', updateSessionModalMode);
    }

    // Keep on window for cross-module calls
    window.updateActiveSessionInfo = updateActiveSessionInfo;

    // Initial load
    loadSessions();

    // Poll active session info
    setInterval(updateActiveSessionInfo, 2000);
}
