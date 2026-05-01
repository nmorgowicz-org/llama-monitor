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
    // Put on window for inline handlers
    window.loadSessions = loadSessions;
    window.renderSessionList = renderSessionList;
    window.deleteSession = deleteSession;
    window.switchSession = switchSession;
    window.openSessionModal = openSessionModal;
    window.showNewSessionForm = showNewSessionForm;
    window.showSessionsList = showSessionsList;
    window.updateSessionModalMode = updateSessionModalMode;
    window.closeSessionModal = closeSessionModal;
    window.saveSession = saveSession;
    window.quickAttachSession = quickAttachSession;
    window.quickStartSession = quickStartSession;
    window.updateActiveSessionInfo = updateActiveSessionInfo;

    // Mode change listener
    const modeSelect = document.getElementById('modal-session-mode');
    if (modeSelect) {
        modeSelect.addEventListener('change', updateSessionModalMode);
    }

    // Initial load
    loadSessions();

    // Poll active session info
    setInterval(updateActiveSessionInfo, 2000);
}
