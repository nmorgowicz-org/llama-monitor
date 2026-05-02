// ── Settings ──────────────────────────────────────────────────────────────────
// Settings modal: collect, save, apply, dirty tracking, and event bindings.

// ── Dirty tracking ────────────────────────────────────────────────────────────

function markSettingsDirty() {
    window.settingsIsDirty = true;
    clearTimeout(window.settingsSaveTimer);
}

function clearSettingsDirty() {
    window.settingsIsDirty = false;
}

// ── Collect / Save / Apply ────────────────────────────────────────────────────

function collectSettings() {
    const endpoint = document.getElementById('server-endpoint').value.trim();

    let port = 8001;
    if (endpoint) {
        try {
            const url = new URL(endpoint);
            port = parseInt(url.port) || 8001;
        } catch(e) {
            // invalid URL, use default
        }
    }

    return {
        preset_id: document.getElementById('preset-select').value,
        port: port,
        llama_server_path: document.getElementById('set-server-path').value,
        llama_server_cwd: document.getElementById('set-server-cwd').value,
        models_dir: '',
        server_endpoint: endpoint,
        remote_agent_url: document.getElementById('set-remote-agent-url')?.value.trim() || '',
        remote_agent_token: document.getElementById('set-remote-agent-token')?.value.trim() || '',
        remote_agent_ssh_autostart: !!document.getElementById('set-remote-agent-ssh-autostart')?.checked,
        remote_agent_ssh_target: document.getElementById('set-remote-agent-ssh-target')?.value.trim() || '',
        remote_agent_ssh_command: document.getElementById('set-remote-agent-ssh-command')?.value.trim() || '',
        explicit_mode_policy: document.getElementById('explicit-policy-input')?.value || '',
    };
}

function saveSettings() {
    clearTimeout(window.settingsSaveTimer);

    // Ripple effect on save button
    const saveBtn = document.querySelector('#settings-modal .btn-modal-save');
    if (saveBtn) {
        const ripple = document.createElement('span');
        ripple.classList.add('ripple');
        const rect = saveBtn.getBoundingClientRect();
        const size = Math.max(rect.width, rect.height);
        ripple.style.width = ripple.style.height = size + 'px';
        ripple.style.left = (rect.width / 2 - size / 2) + 'px';
        ripple.style.top = (rect.height / 2 - size / 2) + 'px';
        saveBtn.appendChild(ripple);
        setTimeout(() => ripple.remove(), 500);

        saveBtn.classList.add('success');
        saveBtn.textContent = '✓ Saved';
        setTimeout(() => {
            saveBtn.classList.remove('success');
            saveBtn.textContent = 'Save Settings';
        }, 1200);
    }

    clearSettingsDirty();

    window.settingsSaveTimer = setTimeout(() => {
        fetch('/api/settings', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(collectSettings()),
        }).catch(() => {});
    }, 400);
}

function applySettings(s) {
    if (!s) return;

    if (s.port) {
        const portInput = document.getElementById('port');
        if (portInput) portInput.value = s.port;
    }

    if (s.llama_server_path !== undefined) {
        const serverPathInput = document.getElementById('set-server-path');
        if (serverPathInput) serverPathInput.value = s.llama_server_path;
    }

    if (s.llama_server_cwd !== undefined) {
        const serverCwdInput = document.getElementById('set-server-cwd');
        if (serverCwdInput) serverCwdInput.value = s.llama_server_cwd;
    }

    if (s.server_endpoint) {
        const endpointInput = document.getElementById('server-endpoint');
        if (endpointInput && !endpointInput.dataset.preserved) {
            endpointInput.value = s.server_endpoint;
        }
    }

    if (s.remote_agent_url !== undefined) {
        const el = document.getElementById('set-remote-agent-url');
        if (el) el.value = s.remote_agent_url;
    }

    if (s.remote_agent_token !== undefined) {
        const el = document.getElementById('set-remote-agent-token');
        if (el) el.value = s.remote_agent_token;
    }

    if (s.remote_agent_ssh_autostart !== undefined) {
        const el = document.getElementById('set-remote-agent-ssh-autostart');
        if (el) el.checked = !!s.remote_agent_ssh_autostart;
    }

    if (s.remote_agent_ssh_target !== undefined) {
        const el = document.getElementById('set-remote-agent-ssh-target');
        if (el) el.value = s.remote_agent_ssh_target;
    }

    if (s.remote_agent_ssh_command !== undefined) {
        const el = document.getElementById('set-remote-agent-ssh-command');
        if (el) el.value = s.remote_agent_ssh_command;
    }

    if (s.explicit_mode_policy !== undefined) {
        const el = document.getElementById('explicit-policy-input');
        if (el) el.value = s.explicit_mode_policy;
    }
}

// ── Modal open/close ──────────────────────────────────────────────────────────

function openSettingsModal() {
    const modal = document.getElementById('settings-modal');
    if (!modal) return;
    modal.removeAttribute('aria-hidden');
    modal.inert = false;
    modal.classList.remove('closing');
    modal.classList.add('open');
    clearSettingsDirty();
}

function closeSettingsModal() {
    const modal = document.getElementById('settings-modal');
    if (!modal) return;
    modal.classList.add('closing');
    setTimeout(() => {
        modal.classList.remove('open', 'closing');
        modal.setAttribute('aria-hidden', 'true');
        modal.inert = true;
        clearSettingsDirty();
    }, 260);
}

// ── Event bindings (run on import) ────────────────────────────────────────────

function _bindSettingsEvents() {
    // Dirty tracking on settings modal
    const settingsModal = document.getElementById('settings-modal');
    if (settingsModal) {
        settingsModal.addEventListener('input', markSettingsDirty);
        settingsModal.addEventListener('change', markSettingsDirty);
    }

    // Keyboard shortcuts for settings modal
    document.addEventListener('keydown', (e) => {
        const modal = document.getElementById('settings-modal');
        if (!modal || !modal.classList.contains('open')) return;

        if (e.key === 'Escape') {
            e.preventDefault();
            closeSettingsModal();
        }

        if ((e.metaKey || e.ctrlKey) && e.key === 's') {
            e.preventDefault();
            saveSettings();
        }
    });

    // Auto-save on controls change
    const controls = document.getElementById('controls');
    if (controls) {
        controls.addEventListener('input', saveSettings);
        controls.addEventListener('change', saveSettings);
    }

    // Settings tabs
    document.querySelectorAll('.settings-tab').forEach(tab => {
        tab.addEventListener('click', () => {
            const target = tab.dataset.tab;
            document.querySelectorAll('.settings-tab').forEach(t => t.classList.remove('active'));
            document.querySelectorAll('.settings-pane').forEach(p => p.classList.remove('active'));
            tab.classList.add('active');
            document.getElementById('settings-' + target)?.classList.add('active');
        });
    });
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initSettings() {
    // Bind settings button
    document.getElementById('settings-btn')?.addEventListener('click', openSettingsModal);
    document.getElementById('sidebar-btn-settings')?.addEventListener('click', openSettingsModal);

    // Bind settings modal buttons
    document.getElementById('settings-modal-close')?.addEventListener('click', closeSettingsModal);
    document.getElementById('settings-modal-cancel')?.addEventListener('click', closeSettingsModal);
    document.getElementById('settings-modal-save')?.addEventListener('click', saveSettings);

    // Keep on window for cross-module calls
    window.markSettingsDirty = markSettingsDirty;
    window.collectSettings = collectSettings;
    window.saveSettings = saveSettings;
    window.applySettings = applySettings;
    window.closeSettingsModal = closeSettingsModal;
    _bindSettingsEvents();
}
