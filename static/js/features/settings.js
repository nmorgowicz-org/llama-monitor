// ── Settings ──────────────────────────────────────────────────────────────────
// Settings modal: collect, save, apply, dirty tracking, and event bindings.

import { settingsState } from '../core/app-state.js';
import { setContextCardViewPreference } from './context-card.js';
import { renderChatMessages } from './chat-render.js';
import { getAutoPollingInterval } from './network-detection.js';

const DATE_FORMAT_KEY = 'llama-monitor-date-format';

// ── Dirty tracking ────────────────────────────────────────────────────────────

export function markSettingsDirty() {
    settingsState.isDirty = true;
    clearTimeout(settingsState.saveTimer);
}

function clearSettingsDirty() {
    settingsState.isDirty = false;
}

function resolveWsPushInterval() {
    const raw = document.getElementById('settings-ws-push-interval')?.value || 'auto';
    if (raw === 'auto') {
        return getAutoPollingInterval();
    }
    return parseInt(raw) || 500;
}

// ── Collect / Save / Apply ────────────────────────────────────────────────────

export function collectSettings() {
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
        context_card_view: document.getElementById('context-view-toggle-fleet')?.classList.contains('active') ? 'fleet' : 'gauge',
        ws_push_interval_ms: resolveWsPushInterval(),
        chat_input_height: document.getElementById('chat-input')?.style.height || '',
        enabled_context_notes: !!document.getElementById('settings-enabled-context-notes')?.checked,
        enabled_suggestions: !!document.getElementById('settings-enabled-suggestions')?.checked,
        enabled_quick_guide: !!document.getElementById('settings-enabled-quick-guide')?.checked,
        default_sidebar_width: parseInt(document.getElementById('settings-sidebar-width')?.value || '280', 10),
        suggestion_count: parseInt(document.getElementById('settings-suggestion-count')?.value || '5', 10),
        context_depth: parseInt(document.getElementById('settings-context-depth')?.value || '10', 10),
        suggestion_prompts: {
            general: document.getElementById('settings-prompt-general')?.value || '',
            'plot-twist': document.getElementById('settings-prompt-plot-twist')?.value || '',
            'new-character': document.getElementById('settings-prompt-new-character')?.value || '',
            context: document.getElementById('settings-prompt-context')?.value || '',
            director: document.getElementById('settings-prompt-director')?.value || '',
            explicit: document.getElementById('settings-prompt-explicit')?.value || '',
            action: document.getElementById('settings-prompt-action')?.value || '',
            comedy: document.getElementById('settings-prompt-comedy')?.value || '',
            fantasy: document.getElementById('settings-prompt-fantasy')?.value || '',
            horror: document.getElementById('settings-prompt-horror')?.value || '',
            mystery: document.getElementById('settings-prompt-mystery')?.value || '',
            noir: document.getElementById('settings-prompt-noir')?.value || '',
            romance: document.getElementById('settings-prompt-romance')?.value || '',
            'sci-fi': document.getElementById('settings-prompt-sci-fi')?.value || '',
            thriller: document.getElementById('settings-prompt-thriller')?.value || '',
        },
    };
}

export function saveSettings() {
    clearTimeout(settingsState.saveTimer);

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

    settingsState.saveTimer = setTimeout(() => {
        fetch('/api/settings', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(collectSettings()),
        }).catch(() => {});
    }, 400);
}

export function applySettings(s) {
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

    if (s.context_card_view !== undefined) {
        setContextCardViewPreference(s.context_card_view);
    }

    if (s.ws_push_interval_ms !== undefined) {
        const el = document.getElementById('settings-ws-push-interval');
        if (el) el.value = String(s.ws_push_interval_ms);
    }

    if (s.chat_input_height) {
        const el = document.getElementById('chat-input');
        if (el) el.style.height = s.chat_input_height;
    }

    if (s.enabled_context_notes !== undefined) {
        const el = document.getElementById('settings-enabled-context-notes');
        if (el) el.checked = !!s.enabled_context_notes;
    }

    if (s.enabled_suggestions !== undefined) {
        const el = document.getElementById('settings-enabled-suggestions');
        if (el) el.checked = !!s.enabled_suggestions;
    }

    if (s.enabled_quick_guide !== undefined) {
        const el = document.getElementById('settings-enabled-quick-guide');
        if (el) el.checked = !!s.enabled_quick_guide;
    }

      if (s.default_sidebar_width !== undefined) {
        const el = document.getElementById('settings-sidebar-width');
        const valueEl = document.getElementById('settings-sidebar-width-value');
        if (el) el.value = s.default_sidebar_width;
        if (valueEl) valueEl.textContent = `${s.default_sidebar_width}px`;
    }

    if (s.suggestion_count !== undefined) {
        const el = document.getElementById('settings-suggestion-count');
        const valueEl = document.getElementById('settings-suggestion-count-value');
        if (el) el.value = s.suggestion_count;
        if (valueEl) valueEl.textContent = String(s.suggestion_count);
    }

    if (s.context_depth !== undefined) {
        const el = document.getElementById('settings-context-depth');
        const valueEl = document.getElementById('settings-context-depth-value');
        if (el) el.value = s.context_depth;
        if (valueEl) valueEl.textContent = String(s.context_depth);
    }

    if (s.suggestion_prompts) {
        const prompts = s.suggestion_prompts;
        const generalEl = document.getElementById('settings-prompt-general');
        const plotTwistEl = document.getElementById('settings-prompt-plot-twist');
        const newCharEl = document.getElementById('settings-prompt-new-character');
        if (generalEl && prompts.general) generalEl.value = prompts.general;
        if (plotTwistEl && prompts['plot-twist']) plotTwistEl.value = prompts['plot-twist'];
        if (newCharEl && prompts['new-character']) newCharEl.value = prompts['new-character'];
    }
}

// ── Live WS interval update ──────────────────────────────────────────────────
// When the polling interval setting changes, apply it immediately via settings API.

let lastAppliedInterval = null;

function applyWsIntervalLive() {
    const interval = resolveWsPushInterval();
    if (interval === lastAppliedInterval) return;
    lastAppliedInterval = interval;

    // Send just the interval change to the backend
    fetch('/api/settings', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ws_push_interval_ms: interval }),
    }).catch(() => {});
}

// ── Modal open/close ──────────────────────────────────────────────────────────

export function openSettingsModal() {
    const modal = document.getElementById('settings-modal');
    if (!modal) return;
    modal.removeAttribute('aria-hidden');
    modal.inert = false;
    modal.classList.remove('closing');
    modal.classList.add('open');
    clearSettingsDirty();

    const dateFmtEl = document.getElementById('chat-date-format');
    if (dateFmtEl) dateFmtEl.value = localStorage.getItem(DATE_FORMAT_KEY) || 'MM/DD/YY';
}

export function closeSettingsModal() {
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

    // Date format — save immediately to localStorage and re-render messages
    document.getElementById('chat-date-format')?.addEventListener('change', (e) => {
        localStorage.setItem(DATE_FORMAT_KEY, e.target.value);
        renderChatMessages();
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

    // WS push interval — apply live on change
    document.getElementById('settings-ws-push-interval')?.addEventListener('change', () => {
        applyWsIntervalLive();
        markSettingsDirty();
        saveSettings();
    });

    // Sidebar width range — update display value
    document.getElementById('settings-sidebar-width')?.addEventListener('input', (e) => {
        const valueEl = document.getElementById('settings-sidebar-width-value');
        if (valueEl) valueEl.textContent = `${e.target.value}px`;
    });

    // Suggestion count range — update display value
    document.getElementById('settings-suggestion-count')?.addEventListener('input', (e) => {
        const valueEl = document.getElementById('settings-suggestion-count-value');
        if (valueEl) valueEl.textContent = String(e.target.value);
    });

    // Context depth range — update display value
    document.getElementById('settings-context-depth')?.addEventListener('input', (e) => {
        const valueEl = document.getElementById('settings-context-depth-value');
        if (valueEl) valueEl.textContent = String(e.target.value);
    });

    // Reset prompts to defaults
    document.getElementById('settings-reset-prompts')?.addEventListener('click', () => {
        const defaults = {
            general: "You are a creative brainstorming partner. Based on the conversation below, suggest {count} varied, actionable next steps the user could take.\n\nFormat as a numbered list. Prioritize variety: dialogue, action, investigation, social, creative approaches.\n\n[conversation context]",
            'plot-twist': "You are a plot twist specialist. Based on the conversation below, suggest {count} unexpected, surprising events that could happen next.\n\nFormat as a numbered list. Prioritize: betrayals, revelations, power reversals, unexpected arrivals, hidden truths.\n\n[conversation context]",
            'new-character': "You are a character introduction specialist. Based on the conversation below, suggest {count} new characters that could enter the story.\n\nFormat as: [Character Name]: [Brief description and how they connect to current story]\n\n[conversation context]",
        };
        const generalEl = document.getElementById('settings-prompt-general');
        const plotTwistEl = document.getElementById('settings-prompt-plot-twist');
        const newCharEl = document.getElementById('settings-prompt-new-character');
        if (generalEl) generalEl.value = defaults.general;
        if (plotTwistEl) plotTwistEl.value = defaults['plot-twist'];
        if (newCharEl) newCharEl.value = defaults['new-character'];
        markSettingsDirty();
    });
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initSettings() {
    // Bind settings button (sidebar button also bound by nav.js with data-tab="settings")
    document.getElementById('settings-btn')?.addEventListener('click', openSettingsModal);

    // Bind settings modal buttons
    document.getElementById('settings-modal-close')?.addEventListener('click', closeSettingsModal);
    document.getElementById('settings-modal-cancel')?.addEventListener('click', closeSettingsModal);
    document.getElementById('settings-modal-save')?.addEventListener('click', saveSettings);

    _bindSettingsEvents();
}
